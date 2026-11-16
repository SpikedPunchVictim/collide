//! An ACP phase under the real shim and the daemon's phases (ts-0092-tw; Stage 3d): the fake ACP child
//! (`tests/adapters/acp/fixtures/fake_acp.py`) runs as an agent phase of an in-process daemon, with the ACP session of
//! `collide_host::adapters::acp` as its feed. The whole of an ACP phase is checked at the `Phases` level: `collide run`
//! and `file_run.rs` keep Claude until Stage 6b.
//!
//! What is checked: one prompt turn from the daemon's `initialize` to the end, the first prompt written once; a
//! permission answer (allow, deny, cancel) reaching the child as the right option or outcome; a restart over a pending
//! request, in the handshake, and at each cut that leaves a write owed (the daemon of the first half writes less than
//! the session decided, as one that died would have); a stop mid-turn, of a child that ignores the cancel; a child that
//! goes first; an empty turn; and the `initialize` probe through the shim.
//!
//! Each test has a folder of its own under `/tmp`, an environment cleared and set to it, an in-process daemon
//! built like `phases.rs`'s, and a guard that stops the shims and children the test started.

mod common;

use collide_core::adapter::{AgentAdapter, AgentSession, ExitInfo, Finished, ReplayError, Step};
use collide_core::agent::{PermissionReply, PermissionRequest};
use collide_core::journal::{Direction, JournalRecord, read_journal};
use collide_core::owner::{QueueItem, QueueKind};
use collide_core::shim::ExitEnd;
use collide_core::state::PhaseState;
use collide_core::testing::sample_launch_request;
use collide_host::acp_probe::probe_through_shim;
use collide_host::adapters::acp::AcpAdapter;
use collide_host::adapters::acp::options::AcpOptions;
use collide_host::adapters::acp::probe::{Probed, probe};
use collide_host::adapters::acp::profile::AcpProfile;
use collide_host::adapters::acp::session::AcpSession;
use collide_host::agent_feed::AgentFeed;
use collide_host::base_env::CaptureOptions;
use collide_host::daemon::{Daemon, DaemonOptions, DaemonPaths};
use collide_host::phases::{FeedRebuilder, PhaseKey, PhaseOptions, PhaseSpec, Phases};
use collide_host::shim::{ChildStdio, files};
use common::procs::Guarded;
use common::{PYTHON, TempRoot, kill_child_of, signal_if_same, wait_until};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const PHASE: &str = "agent";
const PATIENCE: Duration = Duration::from_secs(20);

// ---- the rig ---------------------------------------------------------------------------------------------------

#[derive(Clone, Default)]
struct LogBuf(Arc<Mutex<Vec<u8>>>);

impl Write for LogBuf {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/adapters/acp/fixtures")
}

fn recording(name: &str) -> PathBuf {
    fixtures().join("recordings").join(format!("{name}.jsonl"))
}

/// A run name no other process on the machine carries: a stop finds processes by their run id.
fn unique_run(name: &str) -> String {
    static PREFIX: OnceLock<String> = OnceLock::new();
    let prefix = PREFIX.get_or_init(|| {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        format!("test-{}-{nanos}", std::process::id())
    });
    format!("{prefix}-{name}")
}

/// Stops what a test started: each tracked shim by pid and start time, and each phase's child by `child.json`.
struct Cleanup {
    dirs: Vec<PathBuf>,
    shims: Vec<(u32, u64)>,
}

impl Drop for Cleanup {
    fn drop(&mut self) {
        for (pid, started) in &self.shims {
            signal_if_same(*pid, *started, libc::SIGCONT);
            signal_if_same(*pid, *started, libc::SIGKILL);
        }
        for dir in &self.dirs {
            kill_child_of(dir);
        }
    }
}

fn adapter() -> AcpAdapter {
    AcpAdapter::new(AcpProfile::new("test-agent", "test-acp-agent", vec![]))
}

/// The launch request every phase of these tests has: the first prompt is the same for the daemon that starts a phase
/// and for the one that rebuilds it, which is what a real run's request is.
fn request(cwd: &Path) -> collide_core::adapter::LaunchRequest {
    let mut request = sample_launch_request();
    request.cwd = cwd.to_path_buf();
    request.prompt = "do the task".into();
    request
}

struct Rig {
    root: TempRoot,
    // Declared before `daemon` so the shims are stopped last, after the daemon is gone.
    cleanup: Cleanup,
    daemon: Option<Daemon>,
    log: LogBuf,
    key: PhaseKey,
}

fn fast_phases() -> PhaseOptions {
    PhaseOptions {
        interrupt_wait: Duration::from_millis(400),
        close_wait: Duration::from_secs(3),
        ..PhaseOptions::default()
    }
}

impl Rig {
    /// A rig with a daemon that rebuilds no feed.
    fn new(name: &str) -> Rig {
        let root = TempRoot::new();
        let key = PhaseKey::new(&unique_run(name), PHASE).expect("a good name");
        let mut rig = Rig {
            root,
            cleanup: Cleanup {
                dirs: Vec::new(),
                shims: Vec::new(),
            },
            daemon: None,
            log: LogBuf::default(),
            key,
        };
        rig.start_daemon(None);
        rig
    }

    fn start_daemon(&mut self, rebuild: Option<FeedRebuilder>) {
        let paths = DaemonPaths::under(self.root.path());
        let shell = self
            .root
            .script("fake-shell", "#!/bin/sh\neval \"$4\"\n")
            .to_str()
            .unwrap()
            .to_string();
        let mut options = DaemonOptions::new(paths.clone(), shell, Box::new(self.log.clone()));
        options.process_tree = common::test_process_tree();
        options.capture = CaptureOptions {
            timeout: Duration::from_secs(10),
            cap_bytes: 1024 * 1024,
            fallback: BTreeMap::from([("PATH".to_string(), "/usr/bin:/bin".to_string())]),
        };
        options.shim_exe = Some(PathBuf::from(env!("CARGO_BIN_EXE_collide")));
        options.phases = fast_phases();
        options.processes = Arc::new(Guarded::under(paths.runs_dir, Vec::new()));
        options.feed_rebuilder = rebuild;
        self.daemon = Some(Daemon::start(options).expect("the daemon starts"));
    }

    /// Stops the daemon and leaves the shims, which belong to no daemon.
    fn stop_daemon(&mut self) {
        self.daemon.take().expect("a daemon runs").stop();
    }

    fn daemon(&self) -> &Daemon {
        self.daemon.as_ref().expect("a daemon runs")
    }

    fn phases(&self) -> Phases {
        self.daemon().phases()
    }

    fn dir(&self) -> PathBuf {
        DaemonPaths::under(self.root.path())
            .runs_dir
            .join(&self.key.run)
            .join(&self.key.phase)
    }

    fn rebuilder(&self) -> FeedRebuilder {
        let adapter = adapter();
        let request = request(self.root.path());
        Box::new(move |_, dir| adapter.rebuild_feed(&request, dir).ok())
    }

    /// The feed a daemon of these tests starts a phase with.
    fn feed(&self) -> AgentFeed {
        adapter().feed(&request(self.root.path()), &self.dir())
    }

    /// The fake ACP child on `name` as a phase spec, in the rig's folder.
    fn spec(&self, name: &str) -> PhaseSpec {
        spec_of(self.root.path(), &recording(name))
    }

    /// Starts the phase on the recording `name` with `feed`, and remembers its shim for the cleanup.
    fn start_with(&mut self, name: &str, feed: AgentFeed) {
        let spec = self.spec(name);
        let phases = self.phases();
        phases
            .start_agent_phase(&self.key, &spec, feed)
            .expect("the phase starts");
        self.track();
    }

    fn start(&mut self, name: &str) {
        let feed = self.feed();
        self.start_with(name, feed);
    }

    fn track(&mut self) {
        self.cleanup.dirs.push(self.dir());
        let phases = self.phases();
        let key = self.key.clone();
        let mut pid = None;
        wait_until("the shim's pid is known", PATIENCE, || {
            pid = phases.shim_pid(&key);
            pid.is_some()
        });
        let pid = pid.unwrap();
        if let Some(started) = collide_host::sysproc::start_time_us(pid) {
            self.cleanup.shims.push((pid, started));
        }
    }

    fn state(&self) -> Option<PhaseState> {
        self.phases().state(&self.key)
    }

    fn wait_state(&self, what: &str, want: impl Fn(&PhaseState) -> bool) -> PhaseState {
        let phases = self.phases();
        let key = self.key.clone();
        wait_until(what, PATIENCE, || {
            phases.state(&key).is_some_and(|state| want(&state))
        });
        self.state().unwrap()
    }

    fn wait_ended(&self) {
        self.wait_state("the phase ends", |state| {
            matches!(state, PhaseState::Ended {})
        });
    }

    fn records(&self) -> Vec<JournalRecord> {
        let bytes = fs::read(self.dir().join(files::JOURNAL)).unwrap_or_default();
        read_journal(&bytes).records
    }

    /// The messages of the journal in a direction, as JSON.
    fn messages(&self, direction: Direction) -> Vec<Value> {
        self.records()
            .iter()
            .filter(|record| record.direction == direction)
            .filter_map(|record| serde_json::from_slice(&record.bytes).ok())
            .collect()
    }

    /// How many requests of `method` the daemon wrote to the child, by the journal's `in` records.
    fn written(&self, method: &str) -> usize {
        self.messages(Direction::In)
            .iter()
            .filter(|message| message["method"] == method)
            .count()
    }

    fn wait_journal(&self, what: &str, done: impl Fn(&Rig) -> bool) {
        wait_until(what, PATIENCE, || done(self));
    }

    fn permission_item(&self) -> QueueItem {
        let mut found = None;
        wait_until("the permission request is in the Queue", PATIENCE, || {
            found = self
                .daemon()
                .queue_items(Some(&self.key.run))
                .into_iter()
                .find(|item| item.kind == QueueKind::Permission);
            found.is_some()
        });
        found.unwrap()
    }

    fn queue_is_empty(&self) -> bool {
        self.daemon().queue_items(Some(&self.key.run)).is_empty()
    }

    fn record(&self) -> collide_host::store::PhaseRecord {
        self.daemon()
            .store()
            .phase_record(&self.key.run, &self.key.phase)
            .unwrap()
            .expect("the phase has a record")
    }

    fn exit_end(&self) -> ExitEnd {
        self.phases()
            .exit_record(&self.key)
            .expect("the phase has an exit record")
            .end
    }
}

fn spec_of(cwd: &Path, recording: &Path) -> PhaseSpec {
    PhaseSpec {
        program: PYTHON.into(),
        args: vec![
            "-u".into(),
            fixtures().join("fake_acp.py").display().to_string(),
            recording.display().to_string(),
        ],
        env: BTreeMap::from([
            ("PATH".to_string(), "/usr/bin:/bin".to_string()),
            ("HOME".to_string(), cwd.display().to_string()),
            ("COLLIDE_DAEMON_ROOT".to_string(), cwd.display().to_string()),
        ]),
        cwd: cwd.to_path_buf(),
        stdio: ChildStdio::Pipes,
    }
}

// ---- one turn --------------------------------------------------------------------------------------------------

#[test]
fn one_prompt_turn_runs_end_to_end_and_each_write_of_the_handshake_is_made_once() {
    let mut rig = Rig::new("turn");
    rig.start("prompt-turn");
    rig.wait_ended();
    assert_eq!(rig.written("initialize"), 1);
    assert_eq!(rig.written("session/new"), 1);
    assert_eq!(
        rig.written("session/prompt"),
        1,
        "the first turn is written once"
    );
    assert_eq!(rig.exit_end(), ExitEnd::Exited { code: 0 });
    let record = rig.record();
    assert_eq!(record.report.as_deref(), Some("The notes say hello."));
    assert!(record.usage.is_some());
    assert_eq!(record.failure, None);
}

#[test]
fn a_send_turn_after_the_first_turn_ended_is_a_second_turn() {
    let mut rig = Rig::new("second");
    // No close after the turn, so the phase is there for a second one.
    let feed =
        AgentFeed::new(adapter().session(&request(rig.root.path())), &rig.dir()).start_on_launch();
    rig.start_with("prompt-turn-held", feed);
    rig.wait_journal("the first turn ends", |rig| {
        rig.messages(Direction::Out)
            .iter()
            .any(|message| message["result"]["stopReason"] == "end_turn")
    });
    assert_eq!(rig.written("session/prompt"), 1);
    rig.phases().send_turn(&rig.key, "and then this").unwrap();
    let prompts: Vec<Value> = rig
        .messages(Direction::In)
        .into_iter()
        .filter(|message| message["method"] == "session/prompt")
        .collect();
    assert_eq!(prompts.len(), 2);
    assert_eq!(prompts[0]["id"], 2);
    assert_eq!(prompts[1]["id"], 3);
    assert_eq!(prompts[1]["params"]["prompt"][0]["text"], "and then this");
    rig.phases().close_stdin(&rig.key).unwrap();
    // The session judges the phase by the last turn it saw end, so a second turn with no response ends it as the
    // first did (see Decisions in the item).
    rig.wait_ended();
}

// ---- a permission answer ---------------------------------------------------------------------------------------

fn answered(name: &str, reply: PermissionReply) -> Rig {
    let mut rig = Rig::new(name);
    rig.start(name);
    let item = rig.permission_item();
    let request: PermissionRequest = item.permission.expect("the item holds the request");
    assert_eq!(request.id, "7");
    rig.phases()
        .answer_permission(&rig.key, &request.id, reply)
        .expect("the answer is written");
    // The recording expects the answer it names, and ends the child with 97 for any other.
    rig.wait_ended();
    assert_eq!(rig.exit_end(), ExitEnd::Exited { code: 0 });
    assert!(rig.queue_is_empty());
    rig
}

#[test]
fn an_allow_reaches_the_child_as_the_one_time_allow_option() {
    let rig = answered("permission-request", PermissionReply::Allow {});
    assert_eq!(rig.written("session/prompt"), 1);
}

#[test]
fn a_deny_reaches_the_child_as_the_one_time_reject_option() {
    answered(
        "permission-deny",
        PermissionReply::Deny {
            message: Some("not that".into()),
        },
    );
}

#[test]
fn a_cancel_reaches_the_child_as_the_cancelled_outcome() {
    answered("permission-reply-cancel", PermissionReply::Cancel {});
}

// ---- a restart -------------------------------------------------------------------------------------------------

#[test]
fn a_restart_with_a_permission_pending_puts_it_to_the_person_again_and_the_answer_reaches_the_child()
 {
    let mut rig = Rig::new("restart-permission");
    rig.start("permission-request");
    let before = rig.permission_item();
    rig.stop_daemon();

    let rebuild = rig.rebuilder();
    rig.start_daemon(Some(rebuild));
    let after = rig.permission_item();
    assert_eq!(after.id, before.id);
    rig.phases()
        .answer_permission(&rig.key, "7", PermissionReply::Allow {})
        .expect("the rebuilt session answers");
    rig.wait_ended();
    assert_eq!(rig.exit_end(), ExitEnd::Exited { code: 0 });
    assert_eq!(
        rig.written("session/prompt"),
        1,
        "the restart wrote no second prompt"
    );
    assert!(rig.queue_is_empty());
}

#[test]
fn a_restart_in_the_handshake_reads_the_answer_that_comes_after_it_and_goes_on() {
    let mut rig = Rig::new("restart-handshake");
    rig.start("handshake-held");
    rig.wait_journal("initialize is written", |rig| {
        rig.written("initialize") == 1
    });
    rig.stop_daemon();

    let rebuild = rig.rebuilder();
    rig.start_daemon(Some(rebuild));
    // The agent was holding its answer to `initialize` while the daemon was away, and now goes on.
    fs::write(rig.root.join("go"), "").unwrap();
    rig.wait_ended();
    assert_eq!(
        rig.written("initialize"),
        1,
        "the restart wrote no second initialize"
    );
    assert_eq!(rig.written("session/new"), 1);
    assert_eq!(rig.written("session/prompt"), 1);
}

/// What the daemon that "died" leaves undone: it did not write what the session decided at one cut.
#[derive(Clone, Copy)]
enum Cut {
    /// The `initialize` the start would write.
    Start,
    /// The write of the nth line the feed was given (1 is the answer to `initialize`).
    Line(usize),
}

/// A session that is the ACP one, except that at its cut it says it wrote nothing, as a daemon that was killed between
/// the line and the write would have. What it says it wrote is still counted by the session, which is never used again.
struct Forgetful {
    inner: AcpSession,
    cut: Cut,
    lines: usize,
    forgot: Arc<AtomicBool>,
}

impl AgentSession for Forgetful {
    fn start(&mut self) -> Step {
        let mut step = self.inner.start();
        if matches!(self.cut, Cut::Start) {
            step.writes.clear();
            self.forgot.store(true, Ordering::SeqCst);
        }
        step
    }

    fn feed(&mut self, line: &[u8], wall_ns: u64) -> Step {
        self.lines += 1;
        let mut step = self.inner.feed(line, wall_ns);
        if matches!(self.cut, Cut::Line(at) if at == self.lines) {
            step.writes.clear();
            self.forgot.store(true, Ordering::SeqCst);
        }
        step
    }

    fn send_turn(&mut self, text: &str) -> Step {
        self.inner.send_turn(text)
    }

    fn answer(&mut self, request_id: &str, reply: PermissionReply) -> Step {
        self.inner.answer(request_id, reply)
    }

    fn pending_permissions(&self) -> Vec<PermissionRequest> {
        self.inner.pending_permissions()
    }

    fn cancel(&mut self) -> Step {
        self.inner.cancel()
    }

    fn finish(&mut self, exit: &ExitInfo) -> Finished {
        self.inner.finish(exit)
    }

    fn replay(&mut self, records: &[JournalRecord]) -> Result<Step, ReplayError> {
        self.inner.replay(records)
    }
}

/// Runs `recording` under a daemon that forgets one write, stops it, starts another that rebuilds the phase from the
/// journal, and checks that the phase goes on to its end, with each request written once.
fn restart_over_a_forgotten_write(name: &str, recording: &str, cut: Cut) -> Rig {
    let mut rig = Rig::new(name);
    let forgot = Arc::new(AtomicBool::new(false));
    let session = Forgetful {
        inner: AcpSession::new(AcpOptions::new(rig.root.path(), "do the task")),
        cut,
        lines: 0,
        forgot: forgot.clone(),
    };
    // No close after the turn: the daemon that died did not get so far.
    let feed = AgentFeed::new(Box::new(session), &rig.dir()).start_on_launch();
    rig.start_with(recording, feed);
    wait_until("the daemon forgot the write", PATIENCE, || {
        forgot.load(Ordering::SeqCst)
    });
    rig.stop_daemon();

    let rebuild = rig.rebuilder();
    rig.start_daemon(Some(rebuild));
    // The agent waits for the write, so only the daemon that owes it can end the phase.
    rig.wait_ended();
    assert_eq!(rig.exit_end(), ExitEnd::Exited { code: 0 });
    assert_eq!(rig.written("initialize"), 1);
    assert_eq!(rig.written("session/new"), 1);
    assert_eq!(
        rig.written("session/prompt"),
        1,
        "the first prompt is written once"
    );
    rig
}

#[test]
fn a_restart_before_the_initialize_was_written_owes_it_and_writes_it_once() {
    restart_over_a_forgotten_write("owed-initialize", "prompt-turn", Cut::Start);
}

#[test]
fn a_restart_before_session_new_was_written_owes_it_and_writes_it_once() {
    restart_over_a_forgotten_write("owed-new", "prompt-turn", Cut::Line(1));
}

#[test]
fn a_restart_before_the_first_prompt_was_written_owes_it_and_writes_it_once() {
    restart_over_a_forgotten_write("owed-prompt", "prompt-turn", Cut::Line(2));
}

#[test]
fn a_restart_before_the_not_found_answer_was_written_owes_it_and_writes_it_once() {
    let rig = restart_over_a_forgotten_write("owed-not-found", "unserved-request", Cut::Line(3));
    let answers: Vec<Value> = rig
        .messages(Direction::In)
        .into_iter()
        .filter(|message| message["id"] == 9)
        .collect();
    assert_eq!(answers.len(), 1, "{answers:?}");
    assert_eq!(answers[0]["error"]["code"], -32601);
}

#[test]
fn a_restart_before_the_cancelled_answer_at_the_turns_end_was_written_owes_it_before_stdin_closes()
{
    // The turn ends with a request open. The rebuilt feed closes stdin once the turn has ended, and the agent still
    // waits for its answer: the owed write goes first, or the agent ends with 98 and the phase is not `ended`.
    let rig = restart_over_a_forgotten_write(
        "owed-cancelled",
        "permission-open-at-turn-end",
        Cut::Line(4),
    );
    let answers: Vec<Value> = rig
        .messages(Direction::In)
        .into_iter()
        .filter(|message| message["id"] == 7)
        .collect();
    assert_eq!(answers.len(), 1, "{answers:?}");
}

#[test]
fn a_request_still_open_when_the_turn_ends_is_taken_out_of_the_queue() {
    // The session answers it `cancelled` when the turn ends, so there is nothing left to ask the person.
    let mut rig = Rig::new("open-at-end");
    rig.start("permission-open-at-turn-end");
    rig.wait_ended();
    wait_until("the Queue is empty", PATIENCE, || rig.queue_is_empty());
    assert_eq!(rig.exit_end(), ExitEnd::Exited { code: 0 });
}

#[test]
fn a_restart_that_finds_nothing_owed_writes_nothing_of_its_own() {
    // The same journal read with no write forgotten: the daemon that restarts adds no record to it.
    let mut rig = Rig::new("owed-none");
    let feed =
        AgentFeed::new(adapter().session(&request(rig.root.path())), &rig.dir()).start_on_launch();
    rig.start_with("permission-request", feed);
    let _ = rig.permission_item();
    let before = rig.records().len();
    rig.stop_daemon();
    let rebuild = rig.rebuilder();
    rig.start_daemon(Some(rebuild));
    let _ = rig.permission_item();
    assert_eq!(rig.records().len(), before, "a restart wrote to the child");
}

// ---- a stop ----------------------------------------------------------------------------------------------------

#[test]
fn a_stop_mid_turn_writes_session_cancel_and_the_phase_is_stopped_with_no_signal() {
    let mut rig = Rig::new("stop");
    rig.start("cancel-mid-turn");
    rig.wait_journal("the agent has spoken", |rig| {
        rig.messages(Direction::Out)
            .iter()
            .any(|message| message["method"] == "session/update")
    });
    let report = rig
        .phases()
        .stop_run(&rig.key.run, Some(Duration::from_millis(300)))
        .expect("the run is stopped");
    assert!(report.signalled.is_empty(), "{report:?}");
    assert_eq!(rig.written("session/cancel"), 1);
    rig.wait_state("the phase is stopped", |state| {
        matches!(state, PhaseState::Stopped {})
    });
    assert_eq!(rig.exit_end(), ExitEnd::Exited { code: 0 });
}

#[test]
fn a_stop_of_a_child_that_ignores_the_cancel_closes_stdin_and_the_phase_is_stopped() {
    let mut rig = Rig::new("stop-ignored");
    rig.start("cancel-ignored");
    rig.wait_journal("the agent has spoken", |rig| {
        rig.messages(Direction::Out)
            .iter()
            .any(|message| message["method"] == "session/update")
    });
    let report = rig
        .phases()
        .stop_run(&rig.key.run, Some(Duration::from_millis(300)))
        .expect("the run is stopped");
    assert!(
        report.signalled.is_empty(),
        "closing stdin was enough: {report:?}"
    );
    assert_eq!(rig.written("session/cancel"), 1);
    rig.wait_state("the phase is stopped", |state| {
        matches!(state, PhaseState::Stopped {})
    });
}

// ---- a child that goes first -----------------------------------------------------------------------------------

#[test]
fn a_child_that_exits_0_with_a_request_pending_ends_lost() {
    let mut rig = Rig::new("lost-pending");
    rig.start("lost-request-pending");
    rig.wait_state("the phase is lost", |state| {
        matches!(state, PhaseState::Lost {})
    });
    assert_eq!(rig.exit_end(), ExitEnd::Exited { code: 0 });
}

#[test]
fn a_child_that_exits_1_in_the_handshake_ends_lost() {
    let mut rig = Rig::new("lost-handshake");
    rig.start("lost-in-handshake");
    rig.wait_state("the phase is lost", |state| {
        matches!(state, PhaseState::Lost {})
    });
    assert_eq!(rig.exit_end(), ExitEnd::Exited { code: 1 });
}

#[test]
fn a_child_that_exits_1_in_the_turn_ends_lost() {
    let mut rig = Rig::new("lost-exit-1");
    rig.start("lost-exit-1");
    rig.wait_state("the phase is lost", |state| {
        matches!(state, PhaseState::Lost {})
    });
}

// ---- an empty turn ---------------------------------------------------------------------------------------------

#[test]
fn an_empty_turn_fails_with_the_cause_empty_turn_and_the_agents_own_words_as_evidence() {
    let mut rig = Rig::new("empty");
    rig.start("empty-turn");
    rig.wait_state("the phase fails", |state| {
        matches!(state, PhaseState::Failed { .. })
    });
    let record = rig.record();
    assert_eq!(record.failure.as_deref(), Some("empty_turn"));
    assert!(
        record
            .failure_evidence
            .as_deref()
            .is_some_and(|text| text.contains("stand-in-model:1b")),
        "{record:?}"
    );
}

// ---- the probe through the shim --------------------------------------------------------------------------------

/// What a handshake recording answers `initialize` with, as the response to request 0.
fn answer_of(name: &str) -> Value {
    let text = fs::read_to_string(recording(name)).unwrap();
    let said = text
        .lines()
        .filter(|line| line.starts_with("{\"say\""))
        .map(|line| serde_json::from_str::<Value>(line).unwrap()["say"].clone())
        .next()
        .expect("the recording says something");
    let mut said = said;
    said["id"] = json!(0);
    said
}

fn leftovers(rig: &Rig) -> Vec<String> {
    let runs = DaemonPaths::under(rig.root.path()).runs_dir;
    fs::read_dir(runs)
        .map(|entries| {
            entries
                .flatten()
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn the_probe_reads_each_handshake_recording_through_the_shim_and_leaves_nothing_behind() {
    for name in [
        "handshake-config-options",
        "handshake-models",
        "handshake-bare",
    ] {
        let rig = Rig::new(&format!("probe-{name}"));
        let spec = rig.spec(name);
        let probed = probe_through_shim(&rig.phases(), &spec, Duration::from_secs(15))
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        assert_eq!(probed, probe(&answer_of(name)), "{name}");
        assert_eq!(rig.phases().live(), 0, "{name}: the child is stopped");
        assert!(leftovers(&rig).is_empty(), "{name}: {:?}", leftovers(&rig));
    }
}

#[test]
fn the_probe_reads_what_the_recording_says_and_not_a_default() {
    // `handshake-config-options` has `loadSession` and `http`, and the bare recording has neither, so a probe that
    // returned the default for both would pass the test above for the bare one only.
    let rig = Rig::new("probe-says");
    let probed = probe_through_shim(
        &rig.phases(),
        &rig.spec("handshake-config-options"),
        Duration::from_secs(15),
    )
    .unwrap();
    assert_ne!(probed, Probed::default());
}

#[test]
fn a_probe_of_an_agent_that_never_answers_ends_within_its_wait_and_stops_the_child() {
    let rig = Rig::new("probe-silent");
    let mut spec = rig.spec("handshake-bare");
    // It reads nothing and ignores the close of stdin, so only a signal stops it.
    spec.program = PYTHON.into();
    spec.args = vec!["-c".into(), "import time; time.sleep(60)".into()];
    let started = std::time::Instant::now();
    let error = probe_through_shim(&rig.phases(), &spec, Duration::from_millis(800))
        .expect_err("a silent agent has no answer");
    assert!(
        started.elapsed() < Duration::from_secs(20),
        "{:?}",
        started.elapsed()
    );
    assert!(error.to_string().contains("answer"), "{error}");
    assert_eq!(rig.phases().live(), 0, "the child is stopped");
    assert!(leftovers(&rig).is_empty(), "{:?}", leftovers(&rig));
}
