//! The actor on the wire and the person-only rule in the daemon (ts-0128-bb; 0.2, Who is asking; 0.10, Planning
//! sources, "Only a person accepts, defers, rejects or resolves"). The daemon runs in the test's process over its real
//! socket. A repo pack's provider is `host/tests/fixtures/fake_person_only_provider.mjs`, which keeps every `apply` it
//! is asked for, so a test sees what the provider was asked and whether it was asked at all.
//!
//! Who is asking is faked at the daemon's process tree: the test's own process reads as the CLI, or as the app when a
//! test says so, and a run's agent is a child process of this binary with the run marker in its environment.
//!
//! Every path is below the test's own root under `/tmp`, which is also the home. The daemon's captured environment is
//! made by a login shell that clears the environment, and the child that plays a run's agent starts with a cleared
//! environment, a temp `HOME` and a temp `COLLIDE_DAEMON_ROOT`. Nothing here touches the user's home, keychain or a
//! real daemon.

mod common;

use collide_core::change::ActorKind;
use collide_core::events::EventBody;
use collide_core::owner::{
    BoardTrustParams, BoardTrustResult, BoardTrustReviewParams, BoardTrustReviewResult,
    QueueAnswer, QueueAnswerParams, QueueAnswerResult, QueueItem, QueueKind, QueueListParams,
    QueueListResult, RpcError, TaskApplyParams, TaskApplyResult, TaskPreviewParams, TrustState,
    code,
};
use collide_core::provider::{ApplyActor, ApplyActorKind, ApplyParams, ApplyResult, ChangeCause};
use collide_host::auth::ProcessTree;
use collide_host::clock::{Clock, Clocks, Reading, SystemClocks};
use collide_host::daemon::DaemonOptions;
use collide_host::owner_client::ClientError;
use common::config_fixture::Fixture;
use common::{StopsAtThisTest, node_missing_message, wait_until};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

const WAIT: Option<Duration> = Some(Duration::from_secs(60));
const DEADLINE: Duration = Duration::from_secs(20);

const PROVIDER: &str = include_str!("fixtures/fake_person_only_provider.mjs");

/// 2026-06-15 12:00:00 UTC, the wall clock of every daemon here, and the date it makes in the zone `utc_offset_secs`
/// gives (UTC), so a test does not depend on where it runs.
const MIDDAY_NANOS: u64 = 1_781_524_800 * 1_000_000_000;
const TODAY: &str = "2026-06-15";

// ---- the rig ------------------------------------------------------------------------------------------------------

/// The system's clocks, except that the wall clock stands still at [`MIDDAY_NANOS`].
struct MiddayClocks;

impl Clocks for MiddayClocks {
    fn read(&self, clock: Clock) -> Reading {
        match clock {
            Clock::Wall => Reading {
                clock,
                nanos: MIDDAY_NANOS,
            },
            other => SystemClocks.read(other),
        }
    }
}

/// The tree the other daemon tests use, which stops at this test's process (so the test reads as the CLI), except
/// that this test's process is the app while `app` is set. A child process is read as the system reads it.
struct CallerTree {
    inner: StopsAtThisTest,
    app: Arc<AtomicBool>,
}

impl ProcessTree for CallerTree {
    fn parent(&self, pid: u32) -> Option<u32> {
        self.inner.parent(pid)
    }

    fn run_marker(&self, pid: u32) -> Option<String> {
        self.inner.run_marker(pid)
    }

    fn is_app(&self, pid: u32) -> bool {
        (pid == std::process::id() && self.app.load(Ordering::SeqCst)) || self.inner.is_app(pid)
    }
}

/// A login shell that prints a cleared environment, as the capture asks: `HOME` and `COLLIDE_DAEMON_ROOT` of the
/// test's root and the `PATH` the tests run with, and nothing else of the test process's own.
fn clean_shell(root: &Path) -> String {
    let path = std::env::var("PATH").expect("the tests run with a PATH");
    let home = root.display();
    let script = format!(
        "#!/bin/sh\nexec /usr/bin/env -i HOME='{home}' COLLIDE_DAEMON_ROOT='{home}' PATH='{path}' /bin/sh -c \"$4\"\n"
    );
    let file = root.join("clean-shell");
    fs::write(&file, script).unwrap();
    fs::set_permissions(&file, fs::Permissions::from_mode(0o755)).unwrap();
    file.to_str().unwrap().to_string()
}

fn pack_yaml() -> &'static str {
    "apiVersion: collide/v0\nname: ferry\nversion: 1.0.0\nproviders:\n  fake: { script: providers/fake.mjs }\n"
}

struct Rig {
    fixture: Fixture,
    /// Whether this test's process is read as the app, for the connections made from now on.
    app: Arc<AtomicBool>,
    board: String,
}

fn configure(app: &Arc<AtomicBool>) -> impl FnOnce(&mut DaemonOptions) + use<> {
    let app = app.clone();
    move |options| {
        let root = PathBuf::from(options.home.clone().expect("the fixture sets a home"));
        options.shell = clean_shell(&root);
        options.process_tree = Some(Arc::new(CallerTree {
            inner: StopsAtThisTest::new(),
            app,
        }));
        options.clocks = Arc::new(MiddayClocks);
        options.utc_offset_secs = Some(0);
    }
}

fn rig() -> Rig {
    if let Some(message) = node_missing_message(&std::env::var_os("PATH").unwrap_or_default()) {
        panic!("{message}");
    }
    let app = Arc::new(AtomicBool::new(false));
    let fixture = Fixture::with_options(configure(&app));
    fixture.write(".collide/packs/ferry/pack.yaml", pack_yaml());
    fixture.write(".collide/packs/ferry/providers/fake.mjs", PROVIDER);
    fixture.write(
        ".collide/board.yaml",
        "packs:\n  - source: repo:.collide/packs/ferry\n",
    );
    fixture.commit();
    let board = fixture.board();
    let rig = Rig {
        fixture,
        app,
        board,
    };
    rig.trust();
    rig
}

impl Rig {
    fn as_the_app(&self) {
        self.app.store(true, Ordering::SeqCst);
    }

    /// Records trust in the line `main` at the hash it has now, as the owner does in the app.
    fn trust(&self) {
        let review: BoardTrustReviewResult = self
            .call(
                "board.trust_review",
                &BoardTrustReviewParams {
                    board: self.board.clone(),
                    line: "main".to_string(),
                },
            )
            .unwrap();
        let hash = match review.trust {
            TrustState::Trusted { hash } | TrustState::Untrusted { hash } => hash,
        };
        let _: BoardTrustResult = self
            .call(
                "board.trust",
                &BoardTrustParams {
                    board: self.board.clone(),
                    line: "main".to_string(),
                    hash,
                    hosts: Vec::new(),
                    dev_folders: Vec::new(),
                },
            )
            .unwrap();
    }

    fn call<R: DeserializeOwned>(
        &self,
        method: &str,
        params: &impl Serialize,
    ) -> Result<R, RpcError> {
        match self.fixture.client().call(method, params, WAIT) {
            Ok(result) => Ok(result),
            Err(ClientError::Rpc(error)) => Err(error),
            Err(other) => panic!("{method}: {other}"),
        }
    }

    /// The `task.apply` params of `op` on `id` of the source `fake`, with `status` as its `status` argument.
    fn params(&self, op: &str, id: &str, status: Option<&str>) -> TaskApplyParams {
        let args = status
            .map(|status| BTreeMap::from([("status".to_string(), json!(status))]))
            .unwrap_or_default();
        TaskApplyParams {
            board: self.board.clone(),
            source: Some("fake".to_string()),
            apply: ApplyParams {
                actor: None,
                op: op.to_string(),
                id: Some(id.to_string()),
                args,
                version: None,
                checkout: None,
            },
            commit: false,
        }
    }

    fn apply(&self, op: &str, id: &str, status: Option<&str>) -> Result<TaskApplyResult, RpcError> {
        self.call("task.apply", &self.params(op, id, status))
    }

    fn queue(&self) -> Vec<QueueItem> {
        let listed: QueueListResult = self
            .call(
                "queue.list",
                &QueueListParams {
                    board: None,
                    run: None,
                    kinds: Vec::new(),
                },
            )
            .unwrap();
        listed.items
    }

    fn answer(&self, item: &str, answer: QueueAnswer) -> Result<QueueAnswerResult, RpcError> {
        self.call(
            "queue.answer",
            &QueueAnswerParams {
                item: item.to_string(),
                answer,
            },
        )
    }

    fn checkout(&self) -> PathBuf {
        fs::canonicalize(self.fixture.repo.work()).unwrap()
    }

    /// What the provider wrote for `id`, or `None` when it wrote nothing.
    fn written(&self, id: &str) -> Option<String> {
        fs::read_to_string(self.checkout().join(format!("written-{id}")))
            .ok()
            .map(|text| text.trim().to_string())
    }

    /// Every `apply` the provider was asked for, as the JSON it got, oldest first.
    fn applies(&self) -> Vec<Value> {
        let log = self
            .fixture
            .home()
            .join("data/pack-data")
            .join(&self.board)
            .join("ferry")
            .join("applies");
        fs::read_to_string(log)
            .map(|text| {
                text.lines()
                    .map(|line| serde_json::from_str(line).unwrap())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The writes the daemon recorded, oldest first: the task, who made it and the run of an agent. The events the
    /// corpus records at startup for the tasks the board already has are not writes and are left out.
    fn changes(&self) -> Vec<(String, ActorKind, Option<String>)> {
        self.fixture
            .daemon
            .events_since(0)
            .unwrap()
            .into_iter()
            .filter_map(|event| match event.body {
                EventBody::TaskChanged { task, .. } if task.cause != ChangeCause::Startup => {
                    Some((task.key, event.actor.kind, event.actor.run))
                }
                _ => None,
            })
            .collect()
    }

    /// A daemon started again over the same folders and store, as a restart does. The caller is the CLI again.
    fn restart(self) -> Rig {
        let Rig {
            fixture,
            app,
            board,
        } = self;
        app.store(false, Ordering::SeqCst);
        let fixture = fixture.restart(configure(&app));
        let rig = Rig {
            fixture,
            app,
            board,
        };
        assert_eq!(rig.fixture.board(), rig.board, "the board has the same id");
        rig.trust();
        rig
    }

    /// Makes `calls` (`(method, params)`) from a child process of this test whose environment has the run marker, as a
    /// run's agent has, and returns what each came back as: `"ok"`, or the error as `{code, message}`.
    fn as_a_run_agent(&self, run: &str, calls: &[(&str, Value)]) -> Vec<Value> {
        let root = self.fixture.home();
        let spec = json!({
            "root": root,
            "calls": calls
                .iter()
                .map(|(method, params)| json!({"method": method, "params": params}))
                .collect::<Vec<_>>(),
        });
        let child = Command::new(std::env::current_exe().unwrap())
            .args([
                "agent_client_child",
                "--exact",
                "--nocapture",
                "--test-threads=1",
            ])
            .env_clear()
            .env("HOME", root)
            .env("COLLIDE_DAEMON_ROOT", root)
            .env(collide_host::auth::RUN_MARKER, run)
            .env(AGENT_CALLS, spec.to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("the child starts");
        let output = Reaped(Some(child)).wait();
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            output.status.success(),
            "{stdout}{}",
            String::from_utf8_lossy(&output.stderr)
        );
        stdout
            .lines()
            // The harness prints the test's name on the same line as the first output.
            .filter_map(|line| line.split_once(AGENT_REPLY).map(|(_, reply)| reply))
            .map(|reply| serde_json::from_str(reply).unwrap())
            .collect()
    }
}

/// A child process that is killed and reaped when this goes out of scope, so a test that fails first leaves none.
struct Reaped(Option<Child>);

impl Reaped {
    /// Waits for the child, which has [`DEADLINE`] to end, and returns what it printed.
    fn wait(mut self) -> std::process::Output {
        let mut child = self.0.take().expect("the child is there");
        let until = std::time::Instant::now() + DEADLINE;
        while child.try_wait().expect("the child can be asked").is_none() {
            if std::time::Instant::now() > until {
                let _ = child.kill();
                let _ = child.wait();
                panic!("the child did not end within {DEADLINE:?}");
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        child
            .wait_with_output()
            .expect("the child's output is read")
    }
}

impl Drop for Reaped {
    fn drop(&mut self) {
        if let Some(child) = &mut self.0 {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// Set in the environment of the child that plays a run's agent: the daemon root and the calls it makes, as JSON.
const AGENT_CALLS: &str = "COLLIDE_TEST_PERSON_ONLY_CALLS";
const AGENT_REPLY: &str = "AGENT-REPLY ";

/// Not a test on its own: it is the client that [`Rig::as_a_run_agent`] starts in a child process of this test
/// binary, whose environment carries the run marker, so the daemon sees a run's agent at the other end of the socket.
/// Run on its own (no `AGENT_CALLS`) it does nothing.
#[test]
fn agent_client_child() {
    let Ok(text) = std::env::var(AGENT_CALLS) else {
        return;
    };
    let spec: Value = serde_json::from_str(&text).unwrap();
    let paths = collide_host::daemon::DaemonPaths::under(Path::new(spec["root"].as_str().unwrap()));
    let mut client = collide_host::owner_client::OwnerClient::connect(&paths, "agent-child")
        .expect("the child connects");
    for call in spec["calls"].as_array().unwrap() {
        let answer = client.call::<_, Value>(
            call["method"].as_str().unwrap(),
            &call["params"],
            Some(Duration::from_secs(30)),
        );
        let reply = match answer {
            Ok(_) => json!("ok"),
            Err(ClientError::Rpc(error)) => json!({"code": error.code, "message": error.message}),
            Err(other) => json!({"code": 0, "message": format!("{other}")}),
        };
        println!("{AGENT_REPLY}{reply}");
    }
}

fn applied(result: &TaskApplyResult) -> &ApplyResult {
    result
        .result
        .as_ref()
        .unwrap_or_else(|| panic!("the write was applied: {result:?}"))
}

fn confirmation_item(result: &TaskApplyResult) -> String {
    assert!(result.result.is_none(), "nothing was written: {result:?}");
    assert!(result.queued.is_none(), "{result:?}");
    result
        .confirmation
        .as_ref()
        .expect("the write waits for a confirmation")
        .item
        .clone()
}

fn actor(applies: &Value) -> ApplyActor {
    serde_json::from_value(applies["actor"].clone())
        .unwrap_or_else(|error| panic!("the provider got an actor: {error} in {applies}"))
}

// ---- the app --------------------------------------------------------------------------------------------------

#[test]
fn from_the_app_accept_on_a_proposal_is_written_and_the_event_names_a_person() {
    let rig = rig();
    rig.as_the_app();

    let result = rig.apply("setStatus", "p1", Some("accepted")).unwrap();

    assert!(matches!(applied(&result), ApplyResult::Ok { .. }));
    assert_eq!(rig.written("p1").as_deref(), Some("accepted"));
    assert_eq!(rig.changes().len(), 1);
    assert_eq!(rig.changes()[0].1, ActorKind::Person);
    assert!(rig.queue().is_empty());
}

// ---- the actor the provider is told ------------------------------------------------------------------------------

#[test]
fn the_provider_is_told_the_actor_the_daemon_read_and_the_date_of_its_clock() {
    let rig = rig();
    rig.as_the_app();
    rig.apply("setStatus", "t1", Some("doing")).unwrap();

    let applies = rig.applies();

    assert_eq!(applies.len(), 1);
    assert_eq!(
        actor(&applies[0]),
        ApplyActor {
            kind: ApplyActorKind::Person,
            at: TODAY.to_string(),
            run: None,
        }
    );
}

#[test]
fn an_actor_a_client_sends_is_replaced_by_the_one_the_daemon_read() {
    let rig = rig();
    let mut params = rig.params("setStatus", "t1", Some("doing"));
    params.apply.actor = Some(ApplyActor {
        kind: ApplyActorKind::Agent,
        at: "1999-01-01".to_string(),
        run: Some("run-forged".to_string()),
    });

    let _: TaskApplyResult = rig.call("task.apply", &params).unwrap();

    let applies = rig.applies();
    assert_eq!(applies.len(), 1);
    let seen = actor(&applies[0]);
    assert_eq!(seen.kind, ApplyActorKind::Person);
    assert_eq!(seen.at, TODAY);
    assert_eq!(seen.run, None);
    assert_eq!(rig.changes()[0].1, ActorKind::Person);
}

#[test]
fn a_preview_is_told_the_actor_the_daemon_read_and_never_the_one_a_client_sends() {
    let rig = rig();
    let mut params = rig.params("setStatus", "t1", Some("doing"));
    params.apply.actor = Some(ApplyActor {
        kind: ApplyActorKind::Agent,
        at: "1999-01-01".to_string(),
        run: Some("run-forged".to_string()),
    });
    let preview = TaskPreviewParams {
        board: params.board.clone(),
        source: params.source.clone(),
        apply: params.apply.clone(),
    };

    let _: Value = rig.call("task.preview", &preview).unwrap();

    let log = rig
        .fixture
        .home()
        .join("data/pack-data")
        .join(&rig.board)
        .join("ferry")
        .join("previews");
    let text = fs::read_to_string(log).expect("the provider recorded the preview");
    let previews: Vec<Value> = text
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(previews.len(), 1);
    let seen = actor(&previews[0]);
    assert_eq!(seen.kind, ApplyActorKind::Person);
    assert_eq!(seen.at, TODAY);
    assert_eq!(seen.run, None);
}

#[test]
fn a_run_agents_write_that_is_not_person_only_reaches_the_provider_as_an_agent_of_its_run() {
    let rig = rig();
    let params = serde_json::to_value(rig.params("setStatus", "t1", Some("doing"))).unwrap();

    let replies = rig.as_a_run_agent("run-7", &[("task.apply", params)]);

    assert_eq!(replies, [json!("ok")], "{replies:?}");
    let applies = rig.applies();
    assert_eq!(applies.len(), 1);
    assert_eq!(
        actor(&applies[0]),
        ApplyActor {
            kind: ApplyActorKind::Agent,
            at: TODAY.to_string(),
            run: Some("run-7".to_string()),
        }
    );
    assert_eq!(
        rig.changes(),
        [(
            "fake:t1".to_string(),
            ActorKind::Agent,
            Some("run-7".to_string())
        )]
    );
}

// ---- a run's agent --------------------------------------------------------------------------------------------

#[test]
fn from_a_run_agent_accept_is_refused_naming_the_run_and_the_op_and_the_provider_is_not_asked() {
    let rig = rig();
    let params = serde_json::to_value(rig.params("setStatus", "p1", Some("accepted"))).unwrap();

    let replies = rig.as_a_run_agent("run-7", &[("task.apply", params)]);

    assert_eq!(
        replies[0]["code"],
        json!(code::NOT_PERMITTED),
        "{replies:?}"
    );
    let message = replies[0]["message"].as_str().unwrap();
    assert!(
        message.contains("run-7") && message.contains("accept"),
        "{message}"
    );
    assert!(rig.applies().is_empty(), "the provider was not asked");
    assert_eq!(rig.written("p1"), None, "nothing changed on disk");
    assert!(rig.changes().is_empty());
    assert!(rig.queue().is_empty(), "a refusal is not a confirmation");
}

/// Each row: the op, the item, the status argument, and the op the refusal names.
const PERSON_ONLY: [(&str, &str, Option<&str>, &str); 7] = [
    ("setStatus", "p1", Some("accepted"), "accept"),
    ("setStatus", "p1", Some("deferred"), "defer"),
    ("setStatus", "p1", Some("rejected"), "reject"),
    ("setStatus", "p2", Some("proposed"), "reopen"),
    ("accept", "p1", None, "accept"),
    ("resolve", "q1", None, "resolve"),
    ("defer", "q1", None, "defer"),
];

/// Writes that any actor may make: a status write that is not a person's move, and every op on a task.
const FOR_ANYONE: [(&str, &str, Option<&str>); 4] = [
    ("setStatus", "p1", Some("proposed")),
    ("setStatus", "q1", Some("open")),
    ("setStatus", "t1", Some("doing")),
    ("accept", "t1", None),
];

#[test]
fn every_person_only_write_is_refused_to_a_run_agent_and_the_others_pass() {
    let rig = rig();
    let calls = |rows: &[(&str, &str, Option<&str>)]| -> Vec<(&str, Value)> {
        rows.iter()
            .map(|(op, id, status)| {
                (
                    "task.apply",
                    serde_json::to_value(rig.params(op, id, *status)).unwrap(),
                )
            })
            .collect()
    };
    let refused: Vec<_> = PERSON_ONLY
        .iter()
        .map(|(op, id, status, _)| (*op, *id, *status))
        .collect();

    let replies = rig.as_a_run_agent("run-7", &calls(&refused));

    for ((op, id, _, named), reply) in PERSON_ONLY.iter().zip(&replies) {
        assert_eq!(
            reply["code"],
            json!(code::NOT_PERMITTED),
            "{op} {id}: {reply}"
        );
        assert!(
            reply["message"].as_str().unwrap().contains(named),
            "{op} {id}: {reply}"
        );
    }
    assert!(rig.applies().is_empty(), "the provider was not asked");
    let passing = rig.as_a_run_agent("run-7", &calls(&FOR_ANYONE));
    assert_eq!(passing, vec![json!("ok"); FOR_ANYONE.len()], "{passing:?}");
    assert_eq!(rig.applies().len(), FOR_ANYONE.len());
}

// ---- the CLI --------------------------------------------------------------------------------------------------

#[test]
fn from_the_cli_a_person_only_write_writes_nothing_and_adds_one_confirmation_without_task_text() {
    let rig = rig();

    let result = rig.apply("setStatus", "p1", Some("rejected")).unwrap();

    let item = confirmation_item(&result);
    assert!(rig.applies().is_empty(), "the provider was not asked");
    assert_eq!(rig.written("p1"), None);
    assert!(rig.changes().is_empty());
    let queue = rig.queue();
    assert_eq!(queue.len(), 1, "{queue:?}");
    assert_eq!(queue[0].id, item);
    assert_eq!(queue[0].kind, QueueKind::Confirmation);
    assert_eq!(queue[0].task.as_deref(), Some("fake:p1"));
    assert!(
        queue[0].title.contains("reject") && !queue[0].title.contains("A proposal"),
        "the title says what is asked, with no task text: {}",
        queue[0].title
    );
    assert_eq!(
        queue[0].answers,
        [
            collide_core::owner::AnswerKind::Accept,
            collide_core::owner::AnswerKind::Dismiss
        ]
    );
}

#[test]
fn from_the_cli_each_person_only_write_waits_and_the_others_are_written() {
    let rig = rig();
    for (op, id, status, _) in PERSON_ONLY {
        let result = rig.apply(op, id, status).unwrap();
        confirmation_item(&result);
    }
    assert_eq!(rig.queue().len(), PERSON_ONLY.len());
    assert!(rig.applies().is_empty(), "the provider was not asked");

    for (op, id, status) in FOR_ANYONE {
        let result = rig.apply(op, id, status).unwrap();
        assert!(
            matches!(applied(&result), ApplyResult::Ok { .. }),
            "{op} {id}"
        );
    }

    assert_eq!(rig.applies().len(), FOR_ANYONE.len());
    assert_eq!(rig.queue().len(), PERSON_ONLY.len(), "no other write asked");
}

#[test]
fn a_scratch_task_is_never_judged() {
    let rig = rig();
    let mut params = rig.params("create", "x", None);
    params.source = Some("scratch".to_string());
    params.apply.id = None;
    params.apply.args = BTreeMap::from([("prompt".to_string(), json!("a scratch task"))]);

    let result: TaskApplyResult = rig.call("task.apply", &params).unwrap();

    assert!(matches!(applied(&result), ApplyResult::Ok { .. }));
    assert!(rig.queue().is_empty());
}

#[test]
fn accept_applies_the_stored_write_once_as_a_person() {
    let rig = rig();
    let item = confirmation_item(&rig.apply("setStatus", "p1", Some("accepted")).unwrap());
    rig.as_the_app();

    let answered = rig.answer(&item, QueueAnswer::Accept {}).unwrap();

    assert_eq!(answered, QueueAnswerResult::Done {});
    assert_eq!(rig.written("p1").as_deref(), Some("accepted"));
    let applies = rig.applies();
    assert_eq!(applies.len(), 1);
    assert_eq!(actor(&applies[0]).kind, ApplyActorKind::Person);
    assert_eq!(applies[0]["op"], json!("setStatus"));
    assert_eq!(applies[0]["args"], json!({"status": "accepted"}));
    assert_eq!(rig.changes().len(), 1);
    assert_eq!(rig.changes()[0].1, ActorKind::Person);
    assert!(rig.queue().is_empty());
}

#[test]
fn answering_a_confirmation_twice_applies_it_once() {
    let rig = rig();
    let item = confirmation_item(&rig.apply("setStatus", "p1", Some("accepted")).unwrap());
    rig.as_the_app();
    rig.answer(&item, QueueAnswer::Accept {}).unwrap();

    let again = rig.answer(&item, QueueAnswer::Accept {}).unwrap_err();

    assert_eq!(again.code, code::NOT_FOUND, "{}", again.message);
    assert_eq!(rig.applies().len(), 1);
    let dismissed = rig.answer(&item, QueueAnswer::Dismiss {}).unwrap_err();
    assert_eq!(dismissed.code, code::NOT_FOUND, "{}", dismissed.message);
    assert_eq!(rig.changes().len(), 1);
}

#[test]
fn dismiss_drops_the_stored_write_and_leaves_the_file_as_it_was() {
    let rig = rig();
    let item = confirmation_item(&rig.apply("setStatus", "p1", Some("accepted")).unwrap());
    rig.as_the_app();

    let answered = rig.answer(&item, QueueAnswer::Dismiss {}).unwrap();

    assert_eq!(answered, QueueAnswerResult::Done {});
    assert!(rig.queue().is_empty());
    assert!(rig.applies().is_empty(), "the provider was never asked");
    assert_eq!(rig.written("p1"), None);
    assert!(rig.answer(&item, QueueAnswer::Accept {}).is_err());
    assert!(rig.applies().is_empty());
}

#[test]
fn the_cli_cannot_accept_its_own_confirmation() {
    let rig = rig();
    let item = confirmation_item(&rig.apply("setStatus", "p1", Some("accepted")).unwrap());

    let refused = rig.answer(&item, QueueAnswer::Accept {}).unwrap_err();

    assert_eq!(refused.code, code::NOT_PERMITTED, "{}", refused.message);
    assert!(rig.applies().is_empty());
    assert_eq!(rig.queue().len(), 1, "the confirmation still waits");
    rig.as_the_app();
    rig.answer(&item, QueueAnswer::Accept {}).unwrap();
    assert_eq!(rig.written("p1").as_deref(), Some("accepted"));
}

#[test]
fn a_confirmation_takes_no_answer_but_accept_and_dismiss() {
    let rig = rig();
    let item = confirmation_item(&rig.apply("setStatus", "p1", Some("accepted")).unwrap());
    rig.as_the_app();

    let retried = rig.answer(&item, QueueAnswer::Retry {}).unwrap_err();

    assert_eq!(retried.code, code::INVALID_PARAMS, "{}", retried.message);
    assert_eq!(rig.queue().len(), 1);
    assert!(rig.applies().is_empty());
}

#[test]
fn a_later_write_to_the_checkout_does_not_apply_a_waiting_confirmation() {
    let rig = rig();
    let item = confirmation_item(&rig.apply("setStatus", "p1", Some("accepted")).unwrap());

    rig.apply("setStatus", "t1", Some("doing")).unwrap();

    assert_eq!(rig.written("p1"), None, "only a person's Accept writes it");
    assert_eq!(rig.written("t1").as_deref(), Some("doing"));
    let queue = rig.queue();
    assert_eq!(queue.len(), 1, "{queue:?}");
    assert_eq!(queue[0].id, item);
}

#[test]
fn a_confirmation_survives_a_restart_and_is_applied_once_when_accepted_after_it() {
    let rig = rig();
    let item = confirmation_item(&rig.apply("setStatus", "p2", Some("proposed")).unwrap());

    let rig = rig.restart();

    let queue = rig.queue();
    assert_eq!(queue.len(), 1, "{queue:?}");
    assert_eq!(queue[0].id, item);
    assert_eq!(queue[0].kind, QueueKind::Confirmation);
    wait_until("the daemon to settle", DEADLINE, || rig.queue().len() == 1);
    assert!(rig.applies().is_empty(), "a restart writes nothing");
    rig.as_the_app();
    assert_eq!(
        rig.answer(&item, QueueAnswer::Accept {}).unwrap(),
        QueueAnswerResult::Done {}
    );
    assert_eq!(rig.written("p2").as_deref(), Some("proposed"));
    assert_eq!(rig.applies().len(), 1);
    assert!(rig.answer(&item, QueueAnswer::Accept {}).is_err());
    assert_eq!(rig.applies().len(), 1);
}

#[test]
fn accepting_while_the_checkout_is_busy_queues_the_write_like_any_other_and_it_is_applied_when_free()
 {
    let rig = rig();
    let item = confirmation_item(&rig.apply("setStatus", "p1", Some("accepted")).unwrap());
    let live = rig.fixture.daemon.live_phases();
    live.started("R-7", &rig.checkout(), false);
    rig.as_the_app();

    let answered = rig.answer(&item, QueueAnswer::Accept {}).unwrap();

    let QueueAnswerResult::Queued { item: waiting } = answered else {
        panic!("the write waits for the phase: {answered:?}");
    };
    assert_ne!(waiting, item);
    assert!(
        rig.applies().is_empty(),
        "nothing is written while a phase runs"
    );
    let queue = rig.queue();
    assert_eq!(queue.len(), 1, "{queue:?}");
    assert_eq!(queue[0].kind, QueueKind::Conflict);
    live.ended("R-7");
    assert_eq!(
        rig.answer(&waiting, QueueAnswer::Retry {}).unwrap(),
        QueueAnswerResult::Done {}
    );
    wait_until("the write to land", DEADLINE, || {
        rig.written("p1").is_some()
    });
    let applies = rig.applies();
    assert_eq!(applies.len(), 1);
    assert_eq!(actor(&applies[0]).kind, ApplyActorKind::Person);
    assert!(rig.queue().is_empty());
}
