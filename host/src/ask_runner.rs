//! The daemon's asks (ts-0201-us; 0.2 The UI-tools endpoint, 0.3 The ask agent is a separate role, 0.7 secrets).
//!
//! An ask is one agent turn over a request typed in the ask box. It is not a run: it has no task, workspace or queue
//! item. It runs as one phase under a shim, so a daemon that stops or dies does not leave a model running with nobody
//! to read it, and the stop and the sweep that end a run end an ask. The phase's run id is the ask's id, which is also
//! the child's `COLLIDE_RUN`.
//!
//! ## One ask
//!
//! The daemon resolves the entry and refuses what it can before it answers ([`crate::daemon`]). It then answers, and
//! only then starts [`PendingAsk::run`], so a client reads its answer before the first notice. One thread runs the
//! ask:
//!
//! 1. **Launch.** The pinned line ([`ask_launch_spec`]), the base environment minus the adapter's strip, the entry's
//!    `env` and the run markers, then the entry's `secrets` from the keychain. A secret goes into the child's
//!    environment through the shim's own ([`Phases::start_phase_with_secrets`]) and is in no file. The ask folder
//!    holds `settings.json`, `mcp.json` (the ask's token) and, at the end, `record.json`. Anything that fails here
//!    ends the ask `failed` with its text, and nothing is started.
//! 2. **The request** is written once, as text, and stdin is closed.
//! 3. **Watching.** The thread reads the phase's journal with an [`AgentFeed`] over a [`ClaudeSession`], which is the
//!    parser a run uses, and sends each event to the connection that started the ask, with the ask's token replaced
//!    by `[removed]` ([`AskNotice::scrub_token`]). It stops the phase when the ask is cancelled or the entry's
//!    timeout passes, and ends when the phase is final.
//! 4. **The end.** `mcp.json` is removed (the token is on disk for as long as the child runs and not after), the
//!    record is written, and `ask.outcome` is sent. The child's process group is swept by the run marker.
//!
//! ## The connection rule
//!
//! An ask belongs to the connection that carried `ask.start`. [`AskRegistry::cancel_connection`] cancels its asks
//! when that connection drops. `ask.cancel` names an ask and may come on any connection: the shell sends it for the
//! asks of a closed window on its call connection, not on the ask's own.
//!
//! ## What the record holds
//!
//! `asks/<ask id>/record.json`: the argv without secrets (there are none in it), the program and working folder,
//! start and end times, each tool call, the last usage with its quality, and the outcome. The ask's token is replaced
//! in it as in an event. It is written once, when the ask ends, so an ask the daemon did not see end has none.
//!
//! The phase's own folder under `runs/<ask id>/ask` keeps the shim's journal and the child's output as they were
//! written, which is what a run's phase keeps too.

use crate::adapters::claude::ask::{
    AskProfile, ask_launch_spec, check_before_launch, remove_mcp_file, write_ask_folder,
};
use crate::adapters::claude::session::ClaudeSession;
use crate::adapters::claude::{capabilities, env_strip};
use crate::agent_checks::read_folder;
use crate::agent_feed::AgentFeed;
use crate::atomic_write::write_atomically;
use crate::base_env::BaseEnvironment;
use crate::file_run::layered_env;
use crate::phases::{PhaseKey, PhaseSpec, Phases};
use crate::secret_store::SecretStore;
use crate::shim::ChildStdio;
use collide_core::adapter::{PhaseFailure, PhaseOutcome, StampedEvent};
use collide_core::agent::{AgentEvent, Money, SessionStart, Usage, UsageQuality};
use collide_core::agent_layers::AgentView;
use collide_core::ask::{
    AskCost, AskEnd, AskEventNotice, AskNotice, AskOutcome, AskOutcomeNotice, REMOVED_TEXT,
};
use collide_core::non_empty::NonEmpty;
use collide_core::profile::RoleConfig;
use collide_core::settings::DEFAULT_TIMEOUT_S;
use collide_core::state::PhaseState;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::fs::DirBuilder;
use std::io;
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// The name of an ask's one phase.
pub const PHASE_NAME: &str = "ask";

/// The ask's record, in its folder.
pub const RECORD_FILE: &str = "record.json";

/// The model an entry that names none uses.
const DEFAULT_MODEL: &str = "haiku";

/// How often the thread of an ask looks at its phase, its cancel flag and its clock.
const POLL: Duration = Duration::from_millis(50);

/// How long the thread waits for a phase to settle after it stopped it, before it says what it knows.
const SETTLE_WAIT: Duration = Duration::from_secs(30);

/// The mode of the ask folder and of the record in it.
const DIR_MODE: u32 = 0o700;
const FILE_MODE: u32 = 0o600;

/// Where the daemon sends an ask's notices: the connection that started it.
pub trait AskSink: Send + Sync {
    /// Sends one notice. `false` when it could not be written, which is a connection that is gone.
    fn send(&self, notice: &AskNotice) -> bool;
}

/// One line for the daemon's log.
pub type Logger = Arc<dyn Fn(&str) + Send + Sync>;

/// The base environment, waited for until the first capture has finished.
pub type BaseSource = Arc<dyn Fn() -> Option<BaseEnvironment> + Send + Sync>;

/// What the daemon lends an ask.
#[derive(Clone)]
pub struct AskHost {
    pub phases: Phases,
    pub secrets: Arc<dyn SecretStore>,
    pub base: BaseSource,
    /// The folder of every ask's folder: `asks/` beside `runs/`.
    pub asks_dir: PathBuf,
    pub log: Logger,
}

/// An ask the daemon accepted: its entry, its request and its two secrets of the endpoint (the URL and the token).
pub struct AskPlan {
    pub ask: String,
    pub request: String,
    /// The entry as the layers resolve it, which is not an External one.
    pub agent: AgentView,
    /// The root `<board root>` stands for in the entry's `read`, when the daemon has a board to name.
    pub board_root: Option<PathBuf>,
    pub endpoint: String,
    pub token: String,
}

/// The asks that are running, by id.
#[derive(Default)]
pub struct AskRegistry {
    live: Mutex<BTreeMap<String, Arc<LiveAsk>>>,
}

/// What the registry holds of a running ask.
pub struct LiveAsk {
    connection: u64,
    cancel: AtomicBool,
}

impl AskRegistry {
    fn live(&self) -> MutexGuard<'_, BTreeMap<String, Arc<LiveAsk>>> {
        self.live.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Takes the id for an ask of `connection`. `None` when an ask with that id is running.
    pub fn admit(&self, ask: &str, connection: u64) -> Option<Arc<LiveAsk>> {
        let mut live = self.live();
        if live.contains_key(ask) {
            return None;
        }
        let entry = Arc::new(LiveAsk {
            connection,
            cancel: AtomicBool::new(false),
        });
        live.insert(ask.to_string(), entry.clone());
        Some(entry)
    }

    /// Frees the id of an ask that did not start or has ended.
    pub fn finish(&self, ask: &str) {
        self.live().remove(ask);
    }

    /// Cancels the ask `ask`. An ask that is not running is ignored: the window and the daemon race at its end.
    pub fn cancel(&self, ask: &str) {
        if let Some(live) = self.live().get(ask) {
            live.cancel.store(true, Ordering::SeqCst);
        }
    }

    /// Cancels every ask that `connection` started: the connection rule.
    pub fn cancel_connection(&self, connection: u64) {
        for live in self.live().values().filter(|l| l.connection == connection) {
            live.cancel.store(true, Ordering::SeqCst);
        }
    }

    /// Waits until no ask is running, up to `within`. Whether none is.
    pub fn wait_idle(&self, within: Duration) -> bool {
        let until = Instant::now() + within;
        while Instant::now() < until {
            if self.live().is_empty() {
                return true;
            }
            std::thread::sleep(POLL);
        }
        self.live().is_empty()
    }
}

struct Inner {
    host: AskHost,
    registry: Arc<AskRegistry>,
    live: Arc<LiveAsk>,
    plan: AskPlan,
    sink: Arc<dyn AskSink>,
}

/// An ask that has an answer to be written first. Dropped without [`PendingAsk::run`] (the answer could not be
/// written), it frees the ask's id.
pub struct PendingAsk(Option<Inner>);

impl PendingAsk {
    pub fn new(
        host: AskHost,
        registry: Arc<AskRegistry>,
        live: Arc<LiveAsk>,
        plan: AskPlan,
        sink: Arc<dyn AskSink>,
    ) -> PendingAsk {
        PendingAsk(Some(Inner {
            host,
            registry,
            live,
            plan,
            sink,
        }))
    }

    /// Starts the thread of the ask. A thread that cannot be started frees the id and is the error.
    pub fn run(mut self) -> io::Result<()> {
        let Some(inner) = self.0.take() else {
            return Ok(());
        };
        let registry = inner.registry.clone();
        let ask = inner.plan.ask.clone();
        let log = inner.host.log.clone();
        let started = std::thread::Builder::new()
            .name("collide-ask".to_string())
            .spawn(move || {
                drive(&inner);
                inner.registry.finish(&inner.plan.ask);
            });
        match started {
            Ok(_) => Ok(()),
            Err(error) => {
                registry.finish(&ask);
                log(&format!("ask {ask}: its thread was not started: {error}"));
                Err(error)
            }
        }
    }
}

impl Drop for PendingAsk {
    fn drop(&mut self) {
        if let Some(inner) = self.0.take() {
            inner.registry.finish(&inner.plan.ask);
        }
    }
}

/// What the phase was started with, for the watcher.
struct Launched {
    key: PhaseKey,
    feed: AgentFeed,
    timeout: Duration,
    budget_usd: Option<f64>,
}

/// What the record keeps of how the ask was launched.
struct Launch {
    argv: Vec<String>,
    program: PathBuf,
    cwd: PathBuf,
}

/// Why the thread stopped the phase itself.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Stopping {
    Cancelled,
    TimedOut,
}

/// What the watcher found at the end.
struct Watched {
    end: AskEnd,
    usage: Option<Usage>,
}

/// The ask's own tool calls, for the record.
#[derive(Default)]
struct Calls(Vec<Value>);

fn drive(inner: &Inner) {
    let Inner {
        host,
        live,
        plan,
        sink,
        ..
    } = inner;
    let log = |text: &str| (host.log)(&format!("ask {}: {text}", plan.ask));
    let dir = host.asks_dir.join(&plan.ask);
    let started_ms = wall_ms();
    let mut launch = None;
    let mut calls = Calls::default();
    let watched = match start(host, plan, &dir) {
        Err(text) => {
            log(&format!("not started: {}", scrubbed(&text, &plan.token)));
            Watched {
                end: AskEnd::Failed { text },
                usage: None,
            }
        }
        Ok((launched, how)) => {
            log("started");
            launch = Some(how);
            watch(host, plan, live, sink.as_ref(), launched, &mut calls)
        }
    };
    let end = match watched.end {
        AskEnd::Failed { text } => AskEnd::Failed {
            text: scrubbed(&text, &plan.token),
        },
        other => other,
    };
    let cost = match watched.usage.as_ref().and_then(|usage| usage.cost.clone()) {
        Some(cost) => AskCost::Reported { cost },
        None => AskCost::NotReported {},
    };
    let quality = watched
        .usage
        .as_ref()
        .map_or(UsageQuality::Unreported, |usage| usage.quality);
    let outcome = AskOutcome { end, cost, quality };
    // The token's file goes first: the record and the outcome say the ask is over.
    if let Err(error) = remove_mcp_file(&dir) {
        log(&format!("the token's file was not removed: {error}"));
    }
    let record = record_of(
        plan,
        launch.as_ref(),
        started_ms,
        calls,
        &watched.usage,
        &outcome,
    );
    if let Err(error) = write_record(&dir, &record, &plan.token) {
        log(&format!("the record was not written: {error}"));
    }
    log(&format!(
        "ended: {}",
        scrubbed(&format!("{:?}", outcome.end), &plan.token)
    ));
    let notice = AskNotice::Outcome(Box::new(AskOutcomeNotice {
        ask: NonEmpty::new(plan.ask.clone()).expect("an ask id is not empty"),
        outcome,
    }));
    if !sink.send(&notice) {
        log("the connection was gone before the outcome");
    }
}

/// Everything before the child runs. The text of an error is the ask's `failed` text.
fn start(host: &AskHost, plan: &AskPlan, dir: &Path) -> Result<(Launched, Launch), String> {
    let entry = &plan.agent.entry;
    if let Some(kind) = entry.kind
        && kind != collide_core::settings::AgentKind::ClaudeCode
    {
        return Err(format!(
            "an agent of kind `{}` cannot answer an ask yet: only Claude Code can (the ACP profile is not built)",
            kind.as_str()
        ));
    }
    let read: Vec<PathBuf> = entry
        .read
        .iter()
        .flatten()
        .filter_map(|folder| read_folder(folder, plan.board_root.as_deref()))
        .collect();
    let profile = AskProfile {
        model: entry
            .model
            .clone()
            .unwrap_or_else(|| DEFAULT_MODEL.to_string()),
        read,
        config: entry.config.unwrap_or(RoleConfig::None),
        budget_usd: entry.budget_usd,
        dir: dir.to_path_buf(),
    };
    let spec = ask_launch_spec(&profile).map_err(|error| error.to_string())?;
    let base =
        (host.base)().ok_or_else(|| "the base environment was not captured in time".to_string())?;
    let program = check_before_launch(
        &plan.agent.id,
        &capabilities(),
        base.vars.get("PATH").map(String::as_str),
    )
    .map_err(|error| error.to_string())?;
    let mut secrets = BTreeMap::new();
    for (name, item) in entry.secrets.iter().flatten() {
        let secret = host.secrets.get(item).map_err(|error| error.to_string())?;
        secrets.insert(name.clone(), secret.expose().to_string());
    }
    let key =
        PhaseKey::new(&plan.ask, PHASE_NAME).map_err(|error| format!("the ask's id: {error}"))?;
    if let Err(error) = write_ask_folder(dir, &plan.endpoint, &plan.token) {
        let _ = crate::adapters::claude::ask::remove_ask_folder(dir);
        return Err(error.to_string());
    }
    let extra = layered_env(
        &entry.env.clone().unwrap_or_default(),
        &spec.env,
        &plan.ask,
        wall_ms().saturating_mul(1000),
    );
    let phase = PhaseSpec {
        program: program.clone(),
        args: spec.argv.clone(),
        env: base.for_child(&env_strip(), &extra),
        cwd: spec.cwd.clone(),
        stdio: ChildStdio::Pipes,
    };
    host.phases
        .start_phase_with_secrets(&key, &phase, &secrets)
        .map_err(|error| format!("starting the agent: {error}"))?;
    let feed = AgentFeed::new(
        Box::new(ClaudeSession::new(SessionStart::AgentAssigned {})),
        &host.phases.phase_dir(&key),
    );
    let launched = Launched {
        key,
        feed,
        timeout: Duration::from_secs(entry.timeout_s.unwrap_or(DEFAULT_TIMEOUT_S)),
        budget_usd: entry.budget_usd,
    };
    let how = Launch {
        argv: spec.argv,
        program,
        cwd: spec.cwd,
    };
    Ok((launched, how))
}

/// Sends the request, then sends the events until the phase is final or the ask has to be stopped, and says how it
/// ended.
fn watch(
    host: &AskHost,
    plan: &AskPlan,
    live: &LiveAsk,
    sink: &dyn AskSink,
    launched: Launched,
    calls: &mut Calls,
) -> Watched {
    let Launched {
        key,
        mut feed,
        timeout,
        budget_usd,
    } = launched;
    let log = |text: &str| (host.log)(&format!("ask {}: {text}", plan.ask));
    let started = Instant::now();
    let mut stopping = None;
    let mut forwarding = true;
    let sent = host
        .phases
        .write(&key, &plan.request)
        .map(|_| ())
        .and_then(|()| host.phases.close_stdin(&key));
    let request_failed = sent
        .err()
        .map(|error| format!("the request was not sent to the agent: {error}"));
    if request_failed.is_some() {
        stopping = Some(Stopping::Cancelled);
    }
    let mut stopped_at: Option<Instant> = None;
    loop {
        if let Some(problem) = feed.pump().problem {
            log(&problem);
        }
        forwarding &= forward(plan, sink, feed.take_events(), calls);
        if !forwarding {
            // Nobody reads the ask: the connection rule.
            live.cancel.store(true, Ordering::SeqCst);
        }
        if host
            .phases
            .state(&key)
            .is_some_and(|state| is_final(&state))
        {
            break;
        }
        if stopping.is_none() {
            if live.cancel.load(Ordering::SeqCst) {
                stopping = Some(Stopping::Cancelled);
            } else if started.elapsed() >= timeout {
                stopping = Some(Stopping::TimedOut);
            }
        }
        if stopping.is_some() && stopped_at.is_none() {
            stop(host, plan);
            stopped_at = Some(Instant::now());
            continue;
        }
        if stopped_at.is_some_and(|at| at.elapsed() >= SETTLE_WAIT) {
            log("the phase did not settle after the stop");
            break;
        }
        std::thread::sleep(POLL);
    }
    let exit = host.phases.exit_record(&key);
    let (outcome, record, problem) = feed.settle(exit);
    if let Some(problem) = problem {
        log(&problem);
    }
    forward(plan, sink, feed.take_events(), calls);
    if stopped_at.is_none() {
        // What a finished child left running is swept the way a run's is.
        stop(host, plan);
    }
    let end = match (stopping, request_failed) {
        (_, Some(text)) => AskEnd::Failed { text },
        (Some(Stopping::Cancelled), None) => AskEnd::Cancelled {},
        (Some(Stopping::TimedOut), None) => AskEnd::TimedOut {
            after_s: timeout.as_secs(),
        },
        (None, None) => end_of(&outcome, record.failure_evidence.as_deref(), budget_usd),
    };
    Watched {
        end,
        usage: record.usage,
    }
}

fn stop(host: &AskHost, plan: &AskPlan) {
    if let Err(error) = host.phases.stop_run(&plan.ask, None) {
        (host.log)(&format!("ask {}: the stop: {error}", plan.ask));
    }
}

fn is_final(state: &PhaseState) -> bool {
    matches!(
        state,
        PhaseState::Ended {}
            | PhaseState::Failed { .. }
            | PhaseState::Stopped {}
            | PhaseState::Lost {}
    )
}

/// How an ask whose child ran to its own end came out.
fn end_of(outcome: &PhaseOutcome, evidence: Option<&str>, budget_usd: Option<f64>) -> AskEnd {
    match outcome {
        PhaseOutcome::Ended => AskEnd::Finished {},
        PhaseOutcome::Stopped => AskEnd::Cancelled {},
        PhaseOutcome::Lost => AskEnd::Lost {},
        PhaseOutcome::Failed {
            cause: PhaseFailure::Budget,
        } => match budget_usd {
            Some(amount) => AskEnd::StoppedAtBudget {
                budget: Money {
                    amount,
                    currency: "USD".to_string(),
                },
            },
            None => AskEnd::Failed {
                text: "agent error: the agent stopped at a budget, and the entry sets none"
                    .to_string(),
            },
        },
        PhaseOutcome::Failed { cause } => {
            let said = failure_words(cause);
            AskEnd::Failed {
                text: match evidence {
                    Some(evidence) if !evidence.is_empty() => {
                        format!("agent error: {said} ({evidence})")
                    }
                    _ => format!("agent error: {said}"),
                },
            }
        }
    }
}

fn failure_words(cause: &PhaseFailure) -> String {
    match cause {
        PhaseFailure::EmptyTurn => "the agent answered with nothing".to_string(),
        PhaseFailure::Refusal => "the agent refused the request".to_string(),
        PhaseFailure::MaxTokens => "the agent ran out of tokens".to_string(),
        PhaseFailure::MaxTurns => "the agent ran out of turns".to_string(),
        PhaseFailure::Budget => "the agent stopped at its budget".to_string(),
        PhaseFailure::Blocked { reason } => match reason {
            Some(reason) => format!("a hook blocked the request: {reason}"),
            None => "a hook blocked the request".to_string(),
        },
        PhaseFailure::Api {
            status: Some(status),
        } => format!("the API answered with status {status}"),
        PhaseFailure::Api { status: None } => "the API answered with an error".to_string(),
        PhaseFailure::NoResult => "the agent ended without a result".to_string(),
        PhaseFailure::SettingsIgnored => "the agent did not read Collide's settings".to_string(),
        PhaseFailure::ModelUnavailable => "the model is not available to the agent".to_string(),
    }
}

/// Sends each event, token taken out, and notes the tool calls for the record. `false` when the connection is gone.
fn forward(
    plan: &AskPlan,
    sink: &dyn AskSink,
    events: Vec<StampedEvent>,
    calls: &mut Calls,
) -> bool {
    let token = NonEmpty::new(plan.token.clone()).expect("a token is not empty");
    let ask = NonEmpty::new(plan.ask.clone()).expect("an ask id is not empty");
    let mut delivered = true;
    for stamped in events {
        let mut notice = AskNotice::Event(Box::new(AskEventNotice {
            ask: ask.clone(),
            event: stamped.event,
        }));
        notice.scrub_token(&token);
        if let AskNotice::Event(event) = &notice
            && let AgentEvent::ToolCall {
                id,
                tool,
                kind,
                input: Some(input),
            } = &event.event
        {
            calls.0.push(json!({
                "id": id,
                "tool": tool,
                "kind": kind,
                "input": input,
                "at_ns": stamped.wall_ns,
            }));
        }
        // A connection that is gone stays gone: the rest are not tried, and the ask is cancelled.
        if delivered && !sink.send(&notice) {
            delivered = false;
        }
    }
    delivered
}

fn record_of(
    plan: &AskPlan,
    launch: Option<&Launch>,
    started_ms: u64,
    calls: Calls,
    usage: &Option<Usage>,
    outcome: &AskOutcome,
) -> Value {
    let ended_ms = wall_ms();
    json!({
        "ask": plan.ask,
        "agent": plan.agent.id,
        "argv": launch.map(|l| l.argv.clone()),
        "program": launch.map(|l| l.program.display().to_string()),
        "cwd": launch.map(|l| l.cwd.display().to_string()),
        "started_ms": started_ms,
        "ended_ms": ended_ms,
        "duration_ms": ended_ms.saturating_sub(started_ms),
        "tool_calls": calls.0,
        "usage": usage,
        "outcome": outcome,
    })
}

/// Writes the record into the ask's folder, which is made when the ask never got as far as making it.
fn write_record(dir: &Path, record: &Value, token: &str) -> io::Result<()> {
    DirBuilder::new()
        .recursive(true)
        .mode(DIR_MODE)
        .create(dir)?;
    let text = scrubbed(
        &serde_json::to_string_pretty(record).expect("a record is JSON"),
        token,
    );
    write_atomically(dir, RECORD_FILE, text.as_bytes(), FILE_MODE, |file| file)
        .map_err(io::Error::other)
}

/// `text` with the ask's token replaced, as an event's strings are.
fn scrubbed(text: &str, token: &str) -> String {
    text.replace(token, REMOVED_TEXT)
}

fn wall_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| {
            u64::try_from(since.as_millis()).unwrap_or(u64::MAX)
        })
}

/// The folder of the asks beside a daemon's runs folder.
pub fn asks_dir_beside(runs_dir: &Path) -> PathBuf {
    runs_dir.with_file_name("asks")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_id_that_is_running_is_not_admitted_again_until_it_ends() {
        let registry = AskRegistry::default();
        assert!(registry.admit("a", 1).is_some());
        assert!(registry.admit("a", 2).is_none());
        registry.finish("a");
        assert!(registry.admit("a", 2).is_some());
    }

    #[test]
    fn a_dropped_connection_cancels_only_its_own_asks() {
        let registry = AskRegistry::default();
        let mine = registry.admit("mine", 1).unwrap();
        let theirs = registry.admit("theirs", 2).unwrap();
        registry.cancel_connection(1);
        assert!(mine.cancel.load(Ordering::SeqCst));
        assert!(!theirs.cancel.load(Ordering::SeqCst));
    }

    #[test]
    fn a_cancel_of_an_ask_that_is_not_there_is_ignored() {
        AskRegistry::default().cancel("never");
    }

    #[test]
    fn the_failure_text_names_the_status_and_the_evidence() {
        let end = end_of(
            &PhaseOutcome::Failed {
                cause: PhaseFailure::Api { status: Some(500) },
            },
            Some("overloaded"),
            None,
        );
        assert_eq!(
            end,
            AskEnd::Failed {
                text: "agent error: the API answered with status 500 (overloaded)".to_string()
            }
        );
    }

    #[test]
    fn a_budget_stop_names_the_entrys_budget() {
        let end = end_of(
            &PhaseOutcome::Failed {
                cause: PhaseFailure::Budget,
            },
            None,
            Some(0.25),
        );
        assert!(matches!(end, AskEnd::StoppedAtBudget { budget } if budget.amount == 0.25));
    }

    #[test]
    fn a_token_in_a_text_is_replaced() {
        assert_eq!(scrubbed("a tok b tok", "tok"), "a [removed] b [removed]");
    }
}
