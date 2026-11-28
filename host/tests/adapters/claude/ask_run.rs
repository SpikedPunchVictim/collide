//! An ask through the daemon (ts-0201-us): `ask.start` runs fake Claude under a shim, sends each event and the
//! outcome to the connection that started it, and keeps a record in `asks/<ask id>/`. Each test has a daemon of its own
//! under a temp root that is also its home: its environment is `env_clear()` plus what the test sets, the keychain is
//! a `MemorySecretStore`, and nothing here reads the owner's files. `Rig`'s `Drop` stops every process the test
//! started, by identity.

use super::common;
use super::fake::{BinarySpec, ClaudeBinary, recording};
use super::file_run::{Setup, daemon_options};
use collide_core::agent::{AgentEvent, UsageQuality};
use collide_core::ask::{AskEnd, AskEventNotice, AskNotice, AskOutcomeNotice};
use collide_core::owner::{Ack, RpcError, code};
use collide_host::daemon::Daemon;
use collide_host::owner_client::{ClientError, Incoming, OwnerClient};
use collide_host::phases::{PhaseKey, Phases};
use collide_host::secret_store::{MemorySecretStore, Secret, SecretStore};
use common::config_fixture::CapturedLog;
use common::{TempRoot, pid_alive, signal_if_same, wait_until, write_executable};
use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

/// The token the ask is given in these tests: 32 bytes, hex, as the shell makes one.
const TOKEN: &str = "5f2c9a41e07b3d68a1c4f90b72e5d3860c1f7a94b2d6e8350a4c9e1f7b3d5a62";
const ENDPOINT: &str = "http://127.0.0.1:9/mcp";
/// How long a test waits for a notice, an end or a process.
const PATIENCE: Duration = Duration::from_secs(90);
const PHASE: &str = "ask";

/// Each test's ask has an id of its own: a sweep by the run marker finds a process of any test that has the same id.
static ASKS: AtomicU32 = AtomicU32::new(0);

struct Rig {
    /// The id this test's asks use.
    ask: String,
    root: TempRoot,
    claude: ClaudeBinary,
    daemon: Option<Daemon>,
    phases: Phases,
    secrets: Arc<MemorySecretStore>,
    log: CapturedLog,
    folder: PathBuf,
}

impl Rig {
    /// A daemon whose `claude` is fake Claude playing `recording_name`, with `agents` under `agents:` of the user's
    /// config file. `$FOLDER` in `agents` is the folder the asks read, which is the child's working directory.
    fn new(recording_name: &str, agents: &str) -> Rig {
        Rig::build(recording_name, agents, true)
    }

    fn build(recording_name: &str, agents: &str, claude_on_path: bool) -> Rig {
        let root = TempRoot::new();
        let bin = root.join("bin");
        fs::create_dir(&bin).unwrap();
        let recording_path = recording(recording_name);
        let claude = ClaudeBinary::write(&bin, &BinarySpec::new("2.1.325", &recording_path));
        let path = if claude_on_path {
            format!("{}:/usr/bin:/bin", bin.display())
        } else {
            "/usr/bin:/bin".to_string()
        };
        let shell = root.join("fake-shell");
        write_executable(
            &shell,
            &format!(
                "#!/bin/sh\nexec /usr/bin/env -i HOME='{home}' COLLIDE_DAEMON_ROOT='{home}' PATH='{path}' \
                 /bin/sh -c \"$4\"\n",
                home = root.path().display()
            ),
        );
        let home = root.join("home");
        fs::create_dir_all(home.join(".config/collide")).unwrap();
        let folder = root.join("read");
        fs::create_dir(&folder).unwrap();
        let folder = fs::canonicalize(folder).unwrap();
        let yaml = agents.replace("$FOLDER", folder.to_str().unwrap());
        fs::write(home.join(".config/collide/config.yaml"), yaml).unwrap();
        let secrets = Arc::new(MemorySecretStore::new());
        let log = CapturedLog::default();
        let mut options = daemon_options(&root, &shell, &home, Setup::default());
        options.secret_store = secrets.clone();
        options.log = Box::new(log.clone());
        let daemon = Daemon::start(options).expect("the daemon starts");
        let phases = daemon.phases();
        Rig {
            ask: format!(
                "ask{}-{}",
                std::process::id(),
                ASKS.fetch_add(1, Ordering::SeqCst)
            ),
            root,
            claude,
            daemon: Some(daemon),
            phases,
            secrets,
            log,
            folder,
        }
    }

    fn id(&self) -> &str {
        &self.ask
    }

    fn client(&self) -> OwnerClient {
        OwnerClient::connect(self.daemon.as_ref().unwrap().paths(), "ask-run-test")
            .expect("connects")
    }

    /// Starts an ask on a connection of its own and returns the connection, read from the answer on.
    fn start(&self, ask: &str, agent: &str, request: &str) -> Stream {
        self.start_with(ask, agent, request, TOKEN)
    }

    fn start_with(&self, ask: &str, agent: &str, request: &str, token: &str) -> Stream {
        let mut client = self.client();
        let result: Result<Ack, ClientError> = client.call(
            "ask.start",
            &json!({ "ask": ask, "agent": agent, "request": request, "endpoint": ENDPOINT, "token": token }),
            Some(Duration::from_secs(60)),
        );
        result.expect("the ask is accepted");
        client
            .set_read_wait(Some(Duration::from_millis(250)))
            .unwrap();
        Stream {
            client,
            seen: Vec::new(),
        }
    }

    /// What `ask.start` answers with, for a start that is refused.
    fn refusal(&self, params: Value) -> RpcError {
        let mut client = self.client();
        match client.call::<_, Ack>("ask.start", &params, Some(Duration::from_secs(60))) {
            Err(ClientError::Rpc(error)) => error,
            other => panic!("expected the daemon to refuse the start, got {other:?}"),
        }
    }

    fn key(ask: &str) -> PhaseKey {
        PhaseKey::new(ask, PHASE).unwrap()
    }

    fn ask_dir(&self, ask: &str) -> PathBuf {
        self.root.join("data/asks").join(ask)
    }

    fn record(&self, ask: &str) -> Value {
        let path = self.ask_dir(ask).join("record.json");
        let text = fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        serde_json::from_str(&text).expect("the record is JSON")
    }

    /// The pid the fake wrote in its working folder, waited for.
    fn child_pid(&self) -> u32 {
        let file = self.folder.join("fake.pid");
        wait_until("the fake wrote its pid", PATIENCE, || {
            fs::read_to_string(&file).is_ok_and(|text| text.trim().parse::<u32>().is_ok())
        });
        fs::read_to_string(&file).unwrap().trim().parse().unwrap()
    }

    fn assert_child_gone(&self) {
        let pid = self.child_pid();
        wait_until("the child is gone", PATIENCE, || !pid_alive(pid));
    }
}

impl Drop for Rig {
    fn drop(&mut self) {
        // The shims go on when the daemon stops, so each phase's shim and child are stopped by identity.
        if let Ok(entries) = fs::read_dir(self.root.join("data/runs")) {
            for run in entries.flatten() {
                let phase_dir = run.path().join(PHASE);
                if let Ok(text) = fs::read_to_string(phase_dir.join("shim.json"))
                    && let Ok(value) = serde_json::from_str::<Value>(&text)
                    && let Some(identity) = collide_host::shim::ChildIdentity::from_json(&value)
                {
                    signal_if_same(identity.pid, identity.started_unix_us, libc::SIGKILL);
                }
                common::kill_child_of(&phase_dir);
            }
        }
        drop(self.daemon.take());
    }
}

/// The notices of one ask, read from its connection.
struct Stream {
    client: OwnerClient,
    seen: Vec<AskNotice>,
}

impl Stream {
    /// Reads until a notice satisfies `done`, and returns it.
    fn until(&mut self, what: &str, done: impl Fn(&AskNotice) -> bool) -> AskNotice {
        let until = Instant::now() + PATIENCE;
        if let Some(found) = self.seen.iter().find(|notice| done(notice)) {
            return found.clone();
        }
        while Instant::now() < until {
            match self.client.next_notice().expect("a notice is readable") {
                Incoming::Notice(notification) => {
                    let notice =
                        AskNotice::from_notification(&notification).expect("an ask notice");
                    self.seen.push(notice.clone());
                    if done(&notice) {
                        return notice;
                    }
                }
                Incoming::Quiet => {}
                Incoming::Closed => {
                    panic!("the connection closed before {what}; saw {:?}", self.seen)
                }
            }
        }
        panic!("timed out waiting for {what}; saw {:?}", self.seen)
    }

    fn outcome(&mut self) -> AskOutcomeNotice {
        match self.until("the outcome", |notice| {
            matches!(notice, AskNotice::Outcome(_))
        }) {
            AskNotice::Outcome(outcome) => *outcome,
            AskNotice::Event(_) => unreachable!(),
        }
    }

    fn end(&mut self) -> AskEnd {
        self.outcome().outcome.end
    }

    fn events(&self) -> Vec<AskEventNotice> {
        self.seen
            .iter()
            .filter_map(|notice| match notice {
                AskNotice::Event(event) => Some((**event).clone()),
                AskNotice::Outcome(_) => None,
            })
            .collect()
    }

    fn messages(&self) -> Vec<String> {
        self.events()
            .into_iter()
            .filter_map(|notice| match notice.event {
                AgentEvent::Message { text } => Some(text),
                _ => None,
            })
            .collect()
    }

    fn wait_message(&mut self, text: &str) {
        let wanted = text.to_string();
        self.until(&format!("the message {text:?}"), |notice| {
            matches!(notice, AskNotice::Event(event)
                if matches!(&event.event, AgentEvent::Message { text } if text.contains(&wanted)))
        });
    }
}

fn agents(extra: &str) -> String {
    let extra: String = extra.lines().map(|line| format!("    {line}\n")).collect();
    format!(
        "agents:\n  mine:\n    kind: claude-code\n    model: haiku\n    read: [$FOLDER]\n{extra}  outside:\n    \
         kind: external\n"
    )
}

fn files_under(dir: &Path, found: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        match fs::symlink_metadata(&path) {
            Ok(meta) if meta.is_dir() => files_under(&path, found),
            Ok(meta) if meta.is_file() => found.push(path),
            _ => {}
        }
    }
}

/// Every file under `root` that holds `needle`.
fn files_holding(root: &Path, needle: &str) -> Vec<PathBuf> {
    let mut files = Vec::new();
    files_under(root, &mut files);
    files
        .into_iter()
        .filter(|file| {
            fs::read(file).is_ok_and(|bytes| {
                bytes
                    .windows(needle.len())
                    .any(|window| window == needle.as_bytes())
            })
        })
        .collect()
}

// ---- the run -----------------------------------------------------------------------------------------------------

#[test]
fn an_ask_finishes_and_its_events_and_outcome_reach_the_connection() {
    let rig = Rig::new("ask-finish", &agents(""));
    let mut stream = rig.start(rig.id(), "mine", "group the ready tasks");
    let notice = stream.outcome();
    assert_eq!(notice.ask.as_str(), rig.id());
    assert_eq!(notice.outcome.end, AskEnd::Finished {});
    assert_eq!(stream.messages(), vec!["Done.".to_string()]);
    assert!(
        stream
            .events()
            .iter()
            .all(|event| event.ask.as_str() == rig.id())
    );
    assert!(
        stream
            .events()
            .iter()
            .any(|event| matches!(event.event, AgentEvent::TurnEnd { .. })),
        "the turn's end is an event"
    );
    assert_eq!(
        fs::read_to_string(rig.folder.join("request.txt")).unwrap(),
        "group the ready tasks",
        "the request is written once, as text"
    );
    rig.assert_child_gone();
}

#[test]
fn the_phase_is_final_when_the_outcome_arrives() {
    let rig = Rig::new("ask-finish", &agents(""));
    let mut stream = rig.start(rig.id(), "mine", "go");
    assert_eq!(stream.end(), AskEnd::Finished {});
    let state = rig
        .phases
        .state(&Rig::key(rig.id()))
        .expect("the ask is a phase");
    assert!(
        !matches!(state, collide_core::state::PhaseState::Running {}),
        "the outcome came while the phase was {state:?}"
    );
}

#[test]
fn a_cancel_ends_the_ask_cancelled_and_the_child_is_gone() {
    let rig = Rig::new("ask-hold", &agents(""));
    let mut stream = rig.start(rig.id(), "mine", "go");
    stream.wait_message("Working.");
    // The shell cancels on the connection it makes calls on, which is not the ask's own.
    let mut other = rig.client();
    let _: Ack = other
        .call(
            "ask.cancel",
            &json!({ "ask": rig.id() }),
            Some(Duration::from_secs(60)),
        )
        .expect("the cancel is answered");
    assert_eq!(stream.outcome().outcome.end, AskEnd::Cancelled {});
    rig.assert_child_gone();
}

#[test]
fn a_cancel_of_an_ask_that_is_not_running_is_answered() {
    let rig = Rig::new("ask-finish", &agents(""));
    let mut client = rig.client();
    let _: Ack = client
        .call(
            "ask.cancel",
            &json!({ "ask": "never-started" }),
            Some(Duration::from_secs(60)),
        )
        .expect("the window and the daemon race at the end of every ask");
}

#[test]
fn collides_timeout_ends_the_ask_timed_out_and_the_child_is_gone() {
    let rig = Rig::new("ask-hold", &agents("timeout_s: 5"));
    let mut stream = rig.start(rig.id(), "mine", "go");
    assert_eq!(
        stream.outcome().outcome.end,
        AskEnd::TimedOut { after_s: 5 }
    );
    rig.assert_child_gone();
}

#[test]
fn a_budget_stop_ends_the_ask_stopped_at_the_budget_with_the_cost() {
    let rig = Rig::new("ask-budget", &agents("budget_usd: 0.0001"));
    let mut stream = rig.start(rig.id(), "mine", "go");
    let outcome = stream.outcome().outcome;
    match outcome.end {
        AskEnd::StoppedAtBudget { budget } => {
            assert_eq!(budget.amount, 0.0001);
            assert_eq!(budget.currency, "USD");
        }
        other => panic!("expected the budget stop, got {other:?}"),
    }
    assert_ne!(
        outcome.quality,
        UsageQuality::Unreported,
        "the fake reported a cost"
    );
    rig.assert_child_gone();
}

#[test]
fn a_missing_command_ends_the_ask_failed_with_its_text_and_starts_nothing() {
    let rig = Rig::build("ask-finish", &agents(""), false);
    let mut stream = rig.start(rig.id(), "mine", "go");
    assert_eq!(
        stream.outcome().outcome.end,
        AskEnd::Failed {
            text: "claude was not found on PATH".to_string()
        }
    );
    assert!(
        !rig.folder.join("fake.pid").exists(),
        "no child was started"
    );
    assert!(
        rig.phases.state(&Rig::key(rig.id())).is_none(),
        "no phase was made"
    );
    assert!(
        !rig.ask_dir(rig.id()).join("mcp.json").exists(),
        "the token's file is gone"
    );
}

#[test]
fn an_agent_error_ends_the_ask_failed_with_its_text_and_the_child_is_gone() {
    let rig = Rig::new("ask-api-error", &agents(""));
    let mut stream = rig.start(rig.id(), "mine", "go");
    match stream.outcome().outcome.end {
        AskEnd::Failed { text } => {
            assert!(text.starts_with("agent error: "), "{text}");
            assert!(text.contains("500"), "{text}");
        }
        other => panic!("expected a failure, got {other:?}"),
    }
    rig.assert_child_gone();
}

#[test]
fn an_agent_that_exits_with_no_result_ends_the_ask_failed() {
    let rig = Rig::new("ask-crash", &agents(""));
    let mut stream = rig.start(rig.id(), "mine", "go");
    match stream.outcome().outcome.end {
        AskEnd::Failed { text } => assert!(text.starts_with("agent error: "), "{text}"),
        other => panic!("expected a failure, got {other:?}"),
    }
}

#[test]
fn stopping_the_daemon_stops_a_live_ask_and_its_child() {
    let mut rig = Rig::new("ask-hold", &agents(""));
    let mut stream = rig.start(rig.id(), "mine", "go");
    stream.wait_message("Working.");
    let pid = rig.child_pid();
    drop(rig.daemon.take());
    wait_until("the child is gone", PATIENCE, || !pid_alive(pid));
    drop(stream);
}

#[test]
fn a_dropped_connection_cancels_its_ask_and_the_child_is_gone() {
    let rig = Rig::new("ask-hold", &agents(""));
    let mut stream = rig.start(rig.id(), "mine", "go");
    stream.wait_message("Working.");
    let pid = rig.child_pid();
    drop(stream);
    wait_until("the child is gone", PATIENCE, || !pid_alive(pid));
    wait_until("the record is written", PATIENCE, || {
        rig.ask_dir(rig.id()).join("record.json").is_file()
    });
    assert_eq!(
        rig.record(rig.id())["outcome"]["end"],
        json!({ "type": "cancelled" })
    );
}

#[test]
fn a_killed_shim_ends_the_ask_lost() {
    let rig = Rig::new("ask-hold", &agents(""));
    let mut stream = rig.start(rig.id(), "mine", "go");
    stream.wait_message("Working.");
    let key = Rig::key(rig.id());
    let pid = rig.phases.shim_pid(&key).expect("the shim's pid is known");
    let started = collide_host::sysproc::start_time_us(pid).expect("the shim is running");
    signal_if_same(pid, started, libc::SIGKILL);
    assert_eq!(stream.outcome().outcome.end, AskEnd::Lost {});
    rig.assert_child_gone();
}

// ---- what is kept ------------------------------------------------------------------------------------------------

#[test]
fn the_record_holds_argv_timings_tool_calls_usage_and_the_outcome_and_no_token() {
    let rig = Rig::new("ask-finish", &agents(""));
    let mut stream = rig.start(rig.id(), "mine", "go");
    assert_eq!(stream.end(), AskEnd::Finished {});
    let record = rig.record(rig.id());
    assert_eq!(record["ask"], rig.id());
    assert_eq!(record["agent"], "mine");
    assert_eq!(record["outcome"]["end"], json!({ "type": "finished" }));
    let argv = record["argv"].as_array().expect("the argv is a list");
    assert!(argv.iter().any(|word| word == "--mcp-config"));
    assert!(record["started_ms"].as_u64().unwrap() > 0);
    assert!(record["ended_ms"].as_u64().unwrap() >= record["started_ms"].as_u64().unwrap());
    assert!(record["tool_calls"].is_array());
    assert!(
        record["usage"].is_object(),
        "the last usage, with its quality"
    );
    assert_eq!(record["usage"]["quality"], record["outcome"]["quality"]);
    assert!(
        files_holding(&rig.ask_dir(rig.id()), TOKEN).is_empty(),
        "no token in the ask's folder"
    );
}

#[test]
fn the_ask_folder_holds_no_mcp_json_after_the_ask() {
    let rig = Rig::new("ask-finish", &agents(""));
    let mut stream = rig.start(rig.id(), "mine", "go");
    assert_eq!(stream.end(), AskEnd::Finished {});
    let dir = rig.ask_dir(rig.id());
    assert!(dir.join("record.json").is_file());
    assert!(!dir.join("mcp.json").exists());
}

#[test]
fn neither_the_token_nor_the_secret_is_in_the_argv_or_the_environment_the_child_got() {
    let rig = Rig::new("ask-finish", &agents(""));
    let mut stream = rig.start(rig.id(), "mine", "go");
    assert_eq!(stream.end(), AskEnd::Finished {});
    let launch: Value =
        serde_json::from_str(&fs::read_to_string(rig.folder.join("launch.json")).unwrap()).unwrap();
    let text = launch.to_string();
    assert!(!text.contains(TOKEN), "the token is in the file only");
    let env = launch["env"].as_object().unwrap();
    assert_eq!(
        env["COLLIDE_RUN"],
        rig.id(),
        "the ask's id is the run marker"
    );
    assert!(env.contains_key("COLLIDE_RUN_START"));
    let argv = launch["argv"].as_array().unwrap();
    let words: Vec<&str> = argv.iter().filter_map(Value::as_str).collect();
    let mcp_file = format!("/{}/mcp.json", rig.id());
    assert!(
        words.iter().any(|word| word.ends_with(&mcp_file)),
        "{words:?}"
    );
    assert!(
        !words.contains(&"go"),
        "the request is on stdin, never in argv"
    );
}

#[test]
fn the_calls_of_claude_hold_no_token() {
    let rig = Rig::new("ask-finish", &agents(""));
    let mut stream = rig.start(rig.id(), "mine", "go");
    assert_eq!(stream.end(), AskEnd::Finished {});
    assert!(rig.claude.calls().iter().all(|call| !call.contains(TOKEN)));
    assert!(!rig.log.text().contains(TOKEN), "the daemon's log");
}

#[test]
fn a_token_the_agent_echoes_shows_as_removed_in_the_events_and_the_record() {
    let rig = Rig::new("ask-echo-token", &agents(""));
    let mut stream = rig.start(rig.id(), "mine", "go");
    assert_eq!(stream.end(), AskEnd::Finished {});
    let echoed: Vec<String> = stream
        .messages()
        .into_iter()
        .filter(|text| text.starts_with("the token is "))
        .collect();
    assert_eq!(echoed.len(), 1, "{:?}", stream.messages());
    assert!(
        echoed[0].starts_with("the token is [removed] at "),
        "{}",
        echoed[0]
    );
    assert!(
        !format!("{:?}", stream.seen).contains(TOKEN),
        "no event holds the token"
    );
    let record = fs::read_to_string(rig.ask_dir(rig.id()).join("record.json")).unwrap();
    assert!(!record.contains(TOKEN), "the record holds no token");
    assert!(!rig.log.text().contains(TOKEN));
}

#[test]
fn a_seeded_secret_reaches_the_childs_environment_and_nothing_else() {
    let rig = Rig::new(
        "ask-report-env",
        &agents("secrets: { SEEDED_KEY: my-item }"),
    );
    let value = "seeded-value-7f3a91c2d85e4b06";
    rig.secrets.set("my-item", &Secret::new(value)).unwrap();
    let mut stream = rig.start(rig.id(), "mine", "go");
    assert_eq!(stream.end(), AskEnd::Finished {});
    let sum: u64 = value.bytes().map(u64::from).sum::<u64>() % 65521;
    let wanted = format!("env SEEDED_KEY: set, length {}, sum {sum}", value.len());
    assert!(
        stream.messages().contains(&wanted),
        "{:?}",
        stream.messages()
    );
    assert_eq!(
        files_holding(rig.root.path(), value),
        Vec::<PathBuf>::new(),
        "no file under the temp root holds the value"
    );
    assert!(!rig.log.text().contains(value), "the daemon's log");
    assert!(
        rig.claude.calls().iter().all(|call| !call.contains(value)),
        "claude's argv"
    );
    assert!(!format!("{:?}", stream.seen).contains(value), "an event");
}

#[test]
fn a_secret_the_keychain_lacks_ends_the_ask_failed_naming_the_item() {
    let rig = Rig::new(
        "ask-report-env",
        &agents("secrets: { SEEDED_KEY: my-item }"),
    );
    let mut stream = rig.start(rig.id(), "mine", "go");
    match stream.outcome().outcome.end {
        AskEnd::Failed { text } => assert!(text.contains("my-item"), "{text}"),
        other => panic!("expected a failure, got {other:?}"),
    }
    assert!(
        rig.phases.state(&Rig::key(rig.id())).is_none(),
        "nothing was started"
    );
}

// ---- what is refused ---------------------------------------------------------------------------------------------

fn start_params(ask: &str, agent: &str) -> Value {
    json!({ "ask": ask, "agent": agent, "request": "go", "endpoint": ENDPOINT, "token": TOKEN })
}

#[test]
fn an_unknown_agent_is_refused_before_launch_and_the_text_names_the_entries() {
    let rig = Rig::new("ask-finish", &agents(""));
    let error = rig.refusal(start_params(rig.id(), "nobody"));
    assert_eq!(error.code, code::NOT_FOUND);
    assert!(error.message.contains("nobody"), "{}", error.message);
    assert!(error.message.contains("mine"), "{}", error.message);
    assert!(rig.phases.state(&Rig::key(rig.id())).is_none());
    assert!(!rig.ask_dir(rig.id()).exists(), "nothing was written");
}

#[test]
fn an_external_agent_is_refused_before_launch_in_plain_text() {
    let rig = Rig::new("ask-finish", &agents(""));
    let error = rig.refusal(start_params(rig.id(), "outside"));
    assert_eq!(error.code, code::CONFLICT);
    assert!(error.message.contains("external"), "{}", error.message);
    assert!(rig.phases.state(&Rig::key(rig.id())).is_none());
    assert!(!rig.ask_dir(rig.id()).exists(), "nothing was written");
}

#[test]
fn a_problem_in_the_asked_entry_refuses_the_ask_with_its_text() {
    let rig = Rig::new(
        "ask-finish",
        "agents:\n  mine:\n    kind: claude-code\n    model: haiku\n    read: [$FOLDER]\n    timeout_s: 1\n",
    );
    let error = rig.refusal(start_params(rig.id(), "mine"));
    assert_eq!(error.code, code::CONFLICT);
    assert!(error.message.contains("timeout_s"), "{}", error.message);
    assert!(rig.phases.state(&Rig::key(rig.id())).is_none());
}

#[test]
fn a_problem_in_another_entry_refuses_the_ask_too_as_config_get_refuses_the_config() {
    let rig = Rig::new(
        "ask-finish",
        "agents:\n  mine:\n    kind: claude-code\n    model: haiku\n    read: [$FOLDER]\n  broken:\n    kind: \
         claude-code\n    timeout_s: 1\n",
    );
    let error = rig.refusal(start_params(rig.id(), "mine"));
    assert_eq!(error.code, code::CONFLICT);
    assert!(error.message.contains("broken"), "{}", error.message);
}

#[test]
fn an_empty_request_and_a_token_in_the_wrong_shape_are_refused_as_params() {
    let rig = Rig::new("ask-finish", &agents(""));
    for params in [
        json!({ "ask": rig.id(), "agent": "mine", "request": "", "endpoint": ENDPOINT, "token": TOKEN }),
        json!({ "ask": rig.id(), "agent": "mine", "request": "go", "endpoint": ENDPOINT }),
        json!({ "ask": rig.id(), "agent": "mine", "request": "go" }),
    ] {
        assert_eq!(rig.refusal(params).code, code::INVALID_PARAMS);
    }
}

#[test]
fn an_ask_id_that_cannot_name_a_folder_is_refused() {
    let rig = Rig::new("ask-finish", &agents(""));
    for ask in ["../escape", ".hidden", "a/b"] {
        assert_eq!(
            rig.refusal(start_params(ask, "mine")).code,
            code::INVALID_PARAMS,
            "{ask}"
        );
    }
}

#[test]
fn an_ask_id_that_is_running_or_has_run_is_refused() {
    let rig = Rig::new("ask-hold", &agents(""));
    let mut stream = rig.start(rig.id(), "mine", "go");
    stream.wait_message("Working.");
    assert_eq!(
        rig.refusal(start_params(rig.id(), "mine")).code,
        code::CONFLICT
    );
    let mut other = rig.client();
    let _: Ack = other
        .call(
            "ask.cancel",
            &json!({ "ask": rig.id() }),
            Some(Duration::from_secs(60)),
        )
        .unwrap();
    assert_eq!(stream.outcome().outcome.end, AskEnd::Cancelled {});
    assert_eq!(
        rig.refusal(start_params(rig.id(), "mine")).code,
        code::CONFLICT
    );
}

#[test]
fn an_acp_agent_ends_the_ask_failed_because_its_profile_is_not_built() {
    let yaml = format!(
        "{}  acpish:\n    kind: acp\n    command: /usr/bin/true\n    read: [$FOLDER]\n",
        agents("")
    );
    let rig = Rig::new("ask-finish", &yaml);
    let mut stream = rig.start(rig.id(), "acpish", "go");
    match stream.outcome().outcome.end {
        AskEnd::Failed { text } => assert!(text.contains("ACP"), "{text}"),
        other => panic!("expected a failure, got {other:?}"),
    }
}

#[test]
fn a_connection_that_asked_for_defaults_may_not_start_an_ask() {
    let rig = Rig::new("ask-finish", &agents(""));
    let mut client =
        OwnerClient::connect_as(rig.daemon.as_ref().unwrap().paths(), "ask-run-test", true)
            .expect("connects");
    match client.call::<_, Ack>(
        "ask.start",
        &start_params(rig.id(), "mine"),
        Some(Duration::from_secs(60)),
    ) {
        Err(ClientError::Rpc(error)) => assert_eq!(error.code, code::NOT_PERMITTED),
        other => panic!("expected a refusal, got {other:?}"),
    }
}

#[test]
fn the_ask_is_started_with_the_pinned_line() {
    let rig = Rig::new("ask-finish", &agents(""));
    let mut stream = rig.start(rig.id(), "mine", "go");
    assert_eq!(stream.end(), AskEnd::Finished {});
    let record = rig.record(rig.id());
    let argv: Vec<&str> = record["argv"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
        .collect();
    for word in [
        "--restricted",
        "--strict-mcp-config",
        "--no-session-persistence",
        "--model",
        "haiku",
    ] {
        assert!(argv.contains(&word), "{word} is in the argv: {argv:?}");
    }
}
