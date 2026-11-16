//! The `initialize` probe through the shim (0.3, What the adapter declares; S7, P5; ts-0092-tw).
//!
//! [`probe_through_shim`] starts the agent as a phase of a run of its own, writes `initialize`, reads the response from
//! the shim's journal, stops the child, and removes the phase's folder. The response is read by
//! [`crate::adapters::acp::probe::probe`]. Nothing in a file run calls it yet: the adapter choice there waits for
//! Stage 6b.
//!
//! The child is stopped by closing its stdin, which an ACP agent takes as the end; one that does not end within a short
//! wait is stopped as a run is (`stop_run`), so no probe leaves a process. The phase stays in the table, as every phase
//! does; only its folder goes. The run name is unique to the probe, so it never meets a real run.

use crate::adapters::acp::options::AcpOptions;
use crate::adapters::acp::probe::{Probed, probe};
use crate::adapters::acp::session::AcpSession;
use crate::phases::{LinkError, PhaseError, PhaseKey, PhaseSpec, Phases};
use crate::shim::files;
use collide_core::adapter::AgentSession;
use collide_core::journal::{Direction, read_journal};
use collide_core::state::PhaseState;
use serde_json::Value;
use std::fmt;
use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread::sleep;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// How long the child has to end after its stdin is closed, before it is stopped as a run is.
const CLOSE_WAIT: Duration = Duration::from_millis(1500);
/// The time between SIGTERM and SIGKILL for a child that did not end.
const STOP_GRACE: Duration = Duration::from_millis(300);
const POLL: Duration = Duration::from_millis(20);

/// Why a probe has no answer.
#[derive(Debug)]
pub enum ProbeError {
    Start(PhaseError),
    Write(LinkError),
    /// The agent did not answer `initialize` in time.
    NoAnswer(Duration),
}

impl fmt::Display for ProbeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProbeError::Start(error) => {
                write!(f, "the agent could not be started for the probe: {error}")
            }
            ProbeError::Write(error) => write!(f, "initialize could not be written: {error}"),
            ProbeError::NoAnswer(within) => {
                write!(
                    f,
                    "no answer to initialize within {} ms",
                    within.as_millis()
                )
            }
        }
    }
}

impl std::error::Error for ProbeError {}

/// Starts the agent of `spec` through the shim, writes `initialize`, waits up to `within` for the response, and reads
/// it. The agent is stopped and the phase's folder removed whatever the result.
pub fn probe_through_shim(
    phases: &Phases,
    spec: &PhaseSpec,
    within: Duration,
) -> Result<Probed, ProbeError> {
    let key = probe_key();
    phases.start_phase(&key, spec).map_err(ProbeError::Start)?;
    let result = ask(phases, &key, spec, within);
    stop(phases, &key);
    // The folder is this probe's own, by the run name made for it.
    let _ = fs::remove_dir_all(
        phases
            .phase_dir(&key)
            .parent()
            .unwrap_or(&phases.phase_dir(&key)),
    );
    result
}

fn ask(
    phases: &Phases,
    key: &PhaseKey,
    spec: &PhaseSpec,
    within: Duration,
) -> Result<Probed, ProbeError> {
    let initialize = AcpSession::new(AcpOptions::new(&spec.cwd, "")).start();
    for bytes in initialize.writes {
        phases
            .write(key, &String::from_utf8_lossy(&bytes))
            .map_err(ProbeError::Write)?;
    }
    let journal = phases.phase_dir(key).join(files::JOURNAL);
    let deadline = Instant::now() + within;
    loop {
        if let Some(response) = response_to_initialize(&journal) {
            return Ok(probe(&response));
        }
        if Instant::now() >= deadline {
            return Err(ProbeError::NoAnswer(within));
        }
        sleep(POLL);
    }
}

/// The first message the agent said with the id of the `initialize` request, which is request 0.
fn response_to_initialize(journal: &std::path::Path) -> Option<Value> {
    let bytes = fs::read(journal).ok()?;
    read_journal(&bytes)
        .records
        .iter()
        .filter(|record| record.direction == Direction::Out)
        .filter_map(|record| serde_json::from_slice::<Value>(&record.bytes).ok())
        .find(|message| message["id"] == 0 && message.get("method").is_none())
}

/// Closes stdin, waits a short while for the child to end, and stops the run if it has not.
fn stop(phases: &Phases, key: &PhaseKey) {
    let _ = phases.close_stdin(key);
    let deadline = Instant::now() + CLOSE_WAIT;
    while Instant::now() < deadline {
        if is_final(phases, key) {
            return;
        }
        sleep(POLL);
    }
    let _ = phases.stop_run(&key.run, Some(STOP_GRACE));
}

fn is_final(phases: &Phases, key: &PhaseKey) -> bool {
    !matches!(
        phases.state(key),
        Some(PhaseState::Pending {} | PhaseState::Running {} | PhaseState::Paused { .. })
    )
}

fn probe_key() -> PhaseKey {
    static COUNT: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let run = format!(
        "probe-{}-{nanos}-{}",
        std::process::id(),
        COUNT.fetch_add(1, Ordering::SeqCst)
    );
    PhaseKey::new(&run, "probe").expect("a probe name is a good name")
}
