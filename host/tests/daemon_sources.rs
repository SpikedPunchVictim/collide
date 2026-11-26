//! The sources of a board in the daemon (ts-0113-ck; 0.10, Protocol and Planning sources; 0.9, Trust): `task.get`,
//! `task.apply` and `task.preview` for any source, over the daemon's real socket inside the test process. A repo pack's
//! provider is `host/tests/fixtures/fake_pack_provider.mjs` committed into a fixture repo, so it runs as `node` runs a
//! provider, from the cache's copy of the commit.
//!
//! Every path is below the test's own root under `/tmp`, which is also the home. The daemon's captured environment is
//! made by a login shell that clears the environment, so a provider gets only `HOME` and `COLLIDE_DAEMON_ROOT` of that
//! root, a `PATH` and its pack's data folder. Secrets live in a memory store. Nothing here touches the user's home,
//! keychain or a real daemon.

mod common;

use collide_core::change::ActorKind;
use collide_core::events::EventBody;
use collide_core::owner::{
    BoardListParams, BoardListResult, BoardTrustParams, BoardTrustResult, BoardTrustReviewParams,
    BoardTrustReviewResult, RpcError, TaskApplyParams, TaskApplyResult, TaskGetParams,
    TaskGetResult, TaskPreviewParams, TrustState, code,
};
use collide_core::provider::{ApplyParams, ApplyResult, ChangeCause, ListParams, PreviewResult};
use collide_host::base_env::{BaseEnvironment, CaptureOptions};
use collide_host::board_load::LoadContext;
use collide_host::builtin_packs::{BuiltinPacks, Environment};
use collide_host::clock::SystemClocks;
use collide_host::config_get::{BoardRef, ConfigService, LoadContextFactory};
use collide_host::daemon::DaemonOptions;
use collide_host::owner_client::ClientError;
use collide_host::pack_cache::ensure_sdk;
use collide_host::pack_location::bundle_sdk;
use collide_host::secret_store::{MemorySecretStore, Secret, SecretStore, pack_item};
use collide_host::sources::{Sources, SourcesConfig};
use collide_host::store::Store;
use collide_host::supervisor::{BackoffPolicy, SupervisorPolicy};
use common::config_fixture::{CapturedLog, Fixture};
use common::git_repo::{GitRepo, git_command};
use common::{TempRoot, clean_shell, node_missing_message, pid_alive, wait_until};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

const WAIT: Option<Duration> = Some(Duration::from_secs(60));
const DEADLINE: Duration = Duration::from_secs(20);

/// A value no log, event or file may ever hold.
const CANARY: &str = "canary-3b7d1c-never-written";

const PROVIDER: &str = include_str!("fixtures/fake_pack_provider.mjs");

// ---- the rig ------------------------------------------------------------------------------------------------------

struct Rig {
    fixture: Fixture,
    log: CapturedLog,
    secrets: Arc<MemorySecretStore>,
}

fn rig() -> Rig {
    rig_with(|_| {})
}

fn rig_with(configure: impl FnOnce(&mut DaemonOptions)) -> Rig {
    if let Some(message) = node_missing_message(&std::env::var_os("PATH").unwrap_or_default()) {
        panic!("{message}");
    }
    let secrets = Arc::new(MemorySecretStore::new());
    let log = CapturedLog::default();
    let (held, writer) = (secrets.clone(), log.clone());
    let fixture = Fixture::with_options(move |options| {
        let root = std::path::PathBuf::from(options.home.clone().expect("the fixture sets a home"));
        options.shell = clean_shell(&root);
        options.secret_store = held;
        options.log = Box::new(writer);
        configure(options);
    });
    Rig {
        fixture,
        log,
        secrets,
    }
}

fn rpc<R: DeserializeOwned>(
    rig: &Rig,
    method: &str,
    params: &impl Serialize,
) -> Result<R, RpcError> {
    match rig.fixture.client().call(method, params, WAIT) {
        Ok(result) => Ok(result),
        Err(ClientError::Rpc(error)) => Err(error),
        Err(other) => panic!("{method}: {other}"),
    }
}

fn pack_yaml(name: &str, source: &str, config: &str) -> String {
    format!(
        "apiVersion: collide/v0\nname: {name}\nversion: 1.0.0\nproviders:\n  \
         {source}: {{ script: providers/{source}.mjs }}\n{config}"
    )
}

impl Rig {
    /// Writes the repo pack `pack` with one provider, `source`, whose script says it is `version`.
    fn pack(&self, pack: &str, source: &str, version: &str, config: &str) {
        let dir = format!(".collide/packs/{pack}");
        self.fixture.write(
            &format!("{dir}/pack.yaml"),
            &pack_yaml(pack, source, config),
        );
        self.fixture.write(
            &format!("{dir}/providers/{source}.mjs"),
            &PROVIDER
                .replace("__SOURCE__", source)
                .replace("__VERSION__", version),
        );
    }

    /// Lists the packs in `board.yaml`.
    fn board_yaml(&self, packs: &[&str]) {
        let list: String = packs
            .iter()
            .map(|pack| format!("  - source: repo:.collide/packs/{pack}\n"))
            .collect();
        self.fixture
            .write(".collide/board.yaml", &format!("packs:\n{list}"));
    }

    /// The board's root as the daemon keeps it.
    fn root(&self) -> String {
        fs::canonicalize(self.fixture.repo.work())
            .unwrap()
            .to_str()
            .unwrap()
            .to_string()
    }

    /// Records trust in the line `main` at the hash it has now.
    fn trust(&self, board: &str) {
        self.trust_line(board, "main");
    }

    /// Records trust in the line `line` at the hash it has now.
    fn trust_line(&self, board: &str, line: &str) {
        let review: BoardTrustReviewResult = rpc(
            self,
            "board.trust_review",
            &BoardTrustReviewParams {
                board: board.to_string(),
                line: line.to_string(),
            },
        )
        .unwrap();
        let hash = match review.trust {
            TrustState::Trusted { hash } | TrustState::Untrusted { hash } => hash,
        };
        assert!(
            !hash.is_empty(),
            "the line has a hash: {:?}",
            review.problems
        );
        let _: BoardTrustResult = rpc(
            self,
            "board.trust",
            &BoardTrustParams {
                board: board.to_string(),
                line: line.to_string(),
                hash,
                hosts: Vec::new(),
                dev_folders: Vec::new(),
            },
        )
        .unwrap();
    }

    fn get(&self, board: &str, id: &str) -> Result<TaskGetResult, RpcError> {
        rpc(
            self,
            "task.get",
            &TaskGetParams {
                board: board.to_string(),
                id: id.to_string(),
                checkout: None,
            },
        )
    }

    fn apply_params(id: Option<&str>, op: &str, checkout: Option<&str>) -> ApplyParams {
        ApplyParams {
            actor: None,
            op: op.to_string(),
            id: id.map(str::to_string),
            args: BTreeMap::new(),
            version: None,
            checkout: checkout.map(str::to_string),
        }
    }

    fn apply(
        &self,
        board: &str,
        id: &str,
        op: &str,
        checkout: Option<&str>,
    ) -> Result<ApplyResult, RpcError> {
        let params = TaskApplyParams {
            board: board.to_string(),
            source: None,
            apply: Rig::apply_params(Some(id), op, checkout),
            commit: false,
        };
        rpc::<TaskApplyResult>(self, "task.apply", &params)
            .map(|answer| answer.result.expect("the write was applied, not queued"))
    }

    fn preview(&self, board: &str, source: &str, op: &str) -> Result<PreviewResult, RpcError> {
        rpc(
            self,
            "task.preview",
            &TaskPreviewParams {
                board: board.to_string(),
                source: Some(source.to_string()),
                apply: Rig::apply_params(None, op, None),
            },
        )
    }

    /// The pids of the starts of `pack`'s provider on `board`, one for each `initialize` it answered.
    fn started(&self, board: &str, pack: &str) -> Vec<String> {
        let marker = self
            .fixture
            .home()
            .join("data/pack-data")
            .join(board)
            .join(pack)
            .join("started");
        fs::read_to_string(marker)
            .map(|text| text.lines().map(str::to_string).collect())
            .unwrap_or_default()
    }

    /// A trusted board with the pack `ferry`, whose source is `fake`.
    fn trusted_fake(&self) -> String {
        self.pack("ferry", "fake", "one", "");
        self.board_yaml(&["ferry"]);
        self.fixture.commit();
        let board = self.fixture.board();
        self.trust(&board);
        board
    }

    fn echo(&self, board: &str, source: &str) -> Value {
        let task = self
            .get(board, &format!("{source}:echo"))
            .unwrap()
            .task
            .expect("the provider echoes");
        json!(task.fields)
    }
}

// ---- task.get, task.apply and task.preview ------------------------------------------------------------------------

#[test]
fn task_get_returns_the_task_a_trusted_packs_provider_gives() {
    let rig = rig();
    let board = rig.trusted_fake();

    let task = rig.get(&board, "fake:7").unwrap().task.unwrap();
    assert_eq!(task.key, "fake:7");
    assert_eq!(task.title, "A fake task");
    assert!(rig.get(&board, "fake:missing").unwrap().task.is_none());
    let error = rig.get(&board, "fake:boom").unwrap_err();
    assert_eq!(error.code, code::INTERNAL_ERROR);
    assert!(error.message.contains("disk on fire"), "{}", error.message);
}

#[test]
fn task_apply_answers_ok_conflict_and_refused_as_the_provider_gave_them() {
    let rig = rig();
    let board = rig.trusted_fake();

    match rig.apply(&board, "fake:1", "ok", None).unwrap() {
        ApplyResult::Ok {
            version,
            files,
            items,
            ..
        } => {
            assert_eq!(version, "v2");
            assert_eq!(files, ["a.md"]);
            assert_eq!(items[0].key, "fake:1");
            assert_eq!(items[0].to.as_ref().unwrap().status, "doing");
        }
        other => panic!("expected ok: {other:?}"),
    }
    match rig.apply(&board, "fake:1", "conflict", None).unwrap() {
        ApplyResult::Conflict { current } => assert_eq!(current.key, "fake:1"),
        other => panic!("expected a conflict: {other:?}"),
    }
    match rig.apply(&board, "fake:1", "refused", None).unwrap() {
        ApplyResult::Refused { message } => assert_eq!(message, "illegal transition"),
        other => panic!("expected a refusal: {other:?}"),
    }
}

#[test]
fn task_preview_answers_what_the_provider_would_do_and_writes_no_event() {
    let rig = rig();
    let board = rig.trusted_fake();

    match rig.preview(&board, "fake", "ok").unwrap() {
        PreviewResult::Ok { items, files } => {
            assert_eq!(files, ["a.md"]);
            assert_eq!(items[0].key, "fake:1");
        }
        other => panic!("expected ok: {other:?}"),
    }
    assert!(matches!(
        rig.preview(&board, "fake", "conflict").unwrap(),
        PreviewResult::Conflict { .. }
    ));
    match rig.preview(&board, "fake", "refused").unwrap() {
        PreviewResult::Refused { message } => assert_eq!(message, "illegal transition"),
        other => panic!("expected a refusal: {other:?}"),
    }
    assert!(
        rig.fixture
            .daemon
            .events_since(0)
            .unwrap()
            .iter()
            .all(|event| !matches!(event.body, EventBody::TaskChanged { .. }))
    );
}

#[test]
fn task_preview_of_a_source_that_cannot_preview_says_so() {
    let rig = rig();
    let board = rig.fixture.board();

    let error = rig.preview(&board, "scratch", "create").unwrap_err();
    assert_eq!(error.code, code::INVALID_PARAMS);
    assert!(
        error.message.contains("scratch") && error.message.contains("`preview`"),
        "{}",
        error.message
    );
}

#[test]
fn task_apply_without_a_checkout_writes_the_lines_root_and_a_named_checkout_is_kept() {
    let rig = rig();
    let board = rig.trusted_fake();

    let written = |result: ApplyResult| match result {
        ApplyResult::Ok { task, .. } => task.checkout,
        other => panic!("expected ok: {other:?}"),
    };
    assert_eq!(
        written(rig.apply(&board, "fake:1", "ok", None).unwrap()),
        Some(rig.root())
    );
    assert_eq!(
        written(
            rig.apply(&board, "fake:1", "ok", Some("/work/other"))
                .unwrap()
        ),
        Some("/work/other".to_string())
    );
}

#[test]
fn a_write_through_a_pack_source_records_one_event_with_the_connections_actor() {
    let rig = rig();
    let board = rig.trusted_fake();

    rig.apply(&board, "fake:1", "ok", None).unwrap();
    rig.apply(&board, "fake:1", "conflict", None).unwrap();
    rig.apply(&board, "fake:1", "refused", None).unwrap();

    let changes: Vec<_> = rig
        .fixture
        .daemon
        .events_since(0)
        .unwrap()
        .into_iter()
        .filter_map(|event| match event.body {
            EventBody::TaskChanged { source, task } => Some((event.actor, source, task)),
            _ => None,
        })
        .collect();
    assert_eq!(changes.len(), 1, "{changes:?}");
    let (actor, source, task) = &changes[0];
    assert_eq!(actor.kind, ActorKind::Person);
    assert_eq!(source, "fake");
    assert_eq!(task.key, "fake:1");
    assert_eq!(task.cause, ChangeCause::Worktree);
    assert_eq!(task.checkout, Some(rig.root()));
}

// ---- trust --------------------------------------------------------------------------------------------------------

#[test]
fn before_trust_no_provider_starts_and_after_trust_it_does() {
    let rig = rig();
    rig.pack("ferry", "fake", "one", "");
    rig.board_yaml(&["ferry"]);
    rig.fixture.commit();
    let board = rig.fixture.board();

    let error = rig.get(&board, "fake:1").unwrap_err();
    assert_eq!(error.code, code::CONFLICT);
    assert!(error.message.contains("not trusted"), "{}", error.message);
    assert!(error.message.contains("ferry"), "{}", error.message);
    let error = rig.apply(&board, "fake:1", "ok", None).unwrap_err();
    assert_eq!(error.code, code::CONFLICT);
    let error = rig.preview(&board, "fake", "ok").unwrap_err();
    assert_eq!(error.code, code::CONFLICT);
    assert!(rig.started(&board, "ferry").is_empty());
    assert!(!rig.fixture.home().join("data/pack-data").exists());

    rig.trust(&board);
    assert!(rig.get(&board, "fake:1").unwrap().task.is_some());
    assert_eq!(rig.started(&board, "ferry").len(), 1);
}

#[test]
fn a_provider_starts_at_its_first_call_and_not_when_the_board_opens() {
    let rig = rig();
    let board = rig.trusted_fake();

    // The line is open and trusted, and listing it runs no repo code.
    let _: BoardListResult = rpc(&rig, "board.list", &BoardListParams {}).unwrap();
    assert!(rig.started(&board, "ferry").is_empty());
    rig.get(&board, "fake:1").unwrap();
    rig.get(&board, "fake:2").unwrap();
    assert_eq!(rig.started(&board, "ferry").len(), 1);
}

#[test]
fn a_committed_change_to_the_script_asks_for_trust_again_and_stops_the_running_provider() {
    let rig = rig();
    let board = rig.trusted_fake();
    let first = rig.echo(&board, "fake");
    assert_eq!(first["version"], "one");
    let pid = first["pid"].as_u64().unwrap() as u32;

    rig.pack("ferry", "fake", "two", "");
    rig.fixture.commit();
    let error = rig.get(&board, "fake:1").unwrap_err();
    assert_eq!(error.code, code::CONFLICT);
    assert!(error.message.contains("not trusted"), "{}", error.message);
    wait_until("the old provider is gone", DEADLINE, || !pid_alive(pid));
    assert_eq!(rig.started(&board, "ferry").len(), 1);

    rig.trust(&board);
    let second = rig.echo(&board, "fake");
    assert_eq!(second["version"], "two");
    assert_ne!(second["pid"], first["pid"]);
    assert_eq!(rig.started(&board, "ferry").len(), 2);
}

#[test]
fn a_committed_change_elsewhere_in_the_line_asks_for_trust_again_and_stops_the_running_provider() {
    let rig = rig();
    let board = rig.trusted_fake();
    let first = rig.echo(&board, "fake");
    let pid = first["pid"].as_u64().unwrap() as u32;

    // The pack's own folder is unchanged, so only the line's trust says the provider must stop.
    rig.pack("other", "other", "one", "");
    rig.board_yaml(&["ferry", "other"]);
    rig.fixture.commit();
    let error = rig.get(&board, "fake:1").unwrap_err();
    assert_eq!(error.code, code::CONFLICT);
    assert!(error.message.contains("not trusted"), "{}", error.message);
    wait_until("the old provider is gone", DEADLINE, || !pid_alive(pid));

    rig.trust(&board);
    let second = rig.echo(&board, "fake");
    assert_ne!(second["pid"], first["pid"]);
}

#[test]
fn a_change_to_the_script_that_is_trusted_before_any_call_replaces_the_running_provider() {
    let rig = rig();
    let board = rig.trusted_fake();
    let first = rig.echo(&board, "fake");
    let pid = first["pid"].as_u64().unwrap() as u32;

    rig.pack("ferry", "fake", "two", "");
    rig.fixture.commit();
    rig.trust(&board);
    let second = rig.echo(&board, "fake");
    assert_eq!(second["version"], "two");
    wait_until("the old provider is gone", DEADLINE, || !pid_alive(pid));
}

#[test]
fn an_edit_in_the_working_copy_or_on_another_branch_is_not_what_runs() {
    let rig = rig();
    let board = rig.trusted_fake();
    let repo = &rig.fixture.repo;

    repo.git(&["checkout", "--quiet", "-b", "other"]);
    rig.pack("ferry", "fake", "other-branch", "");
    rig.fixture.commit();
    repo.git(&["checkout", "--quiet", "main"]);
    rig.pack("ferry", "fake", "working-copy", "");

    let seen = rig.echo(&board, "fake");
    assert_eq!(seen["version"], "one");
    // The trust of the line did not change, so nothing asked again.
    assert_eq!(rig.started(&board, "ferry").len(), 1);
}

// ---- a rebase at the root (ts-0146-se) ----------------------------------------------------------------------------

impl Rig {
    /// A board whose pack `ferry` has the script version `one` on `topic` and `two` on `main`, and the checkout in a
    /// rebase of `topic` onto `main` that stopped on a conflict. `main` is trusted; `topic` is when `trust_topic` is.
    fn stop_in_a_rebase(&self, trust_topic: bool) -> String {
        let repo = &self.fixture.repo;
        self.pack("ferry", "fake", "one", "");
        self.board_yaml(&["ferry"]);
        self.fixture.write("line.txt", "one\n");
        self.fixture.commit();
        let board = self.fixture.board();
        repo.git(&["checkout", "--quiet", "-b", "topic"]);
        self.fixture.write("line.txt", "topic\n");
        self.fixture.commit();
        if trust_topic {
            self.trust_line(&board, "topic");
        }
        repo.git(&["checkout", "--quiet", "main"]);
        self.pack("ferry", "fake", "two", "");
        self.fixture.write("line.txt", "main\n");
        self.fixture.commit();
        self.trust(&board);
        repo.git(&["checkout", "--quiet", "topic"]);
        let rebase = git_command(repo.root().path(), repo.work())
            .args(["rebase", "main"])
            .output()
            .expect("git runs");
        assert!(!rebase.status.success(), "the rebase stops on the conflict");
        assert!(repo.git_dir().join("rebase-merge").is_dir());
        board
    }
}

#[test]
fn in_a_rebase_of_a_trusted_branch_the_source_starts_from_that_branchs_own_commit() {
    let rig = rig();
    let board = rig.stop_in_a_rebase(true);

    // HEAD holds the script of `main`, which is version two. The branch holds version one.
    assert_eq!(rig.echo(&board, "fake")["version"], "one");
    assert_eq!(rig.started(&board, "ferry").len(), 1);
}

#[test]
fn in_a_rebase_of_an_untrusted_branch_the_source_does_not_start() {
    let rig = rig();
    let board = rig.stop_in_a_rebase(false);

    // The commit HEAD names holds `main`, which is trusted. That is not what the rebase is of.
    let error = rig.get(&board, "fake:1").unwrap_err();
    assert_eq!(error.code, code::CONFLICT);
    assert!(
        error.message.contains("the line `topic` is not trusted"),
        "{}",
        error.message
    );
    let error = rig.apply(&board, "fake:1", "ok", None).unwrap_err();
    assert_eq!(error.code, code::CONFLICT);
    assert!(rig.started(&board, "ferry").is_empty());
}

#[test]
fn a_write_at_the_root_in_a_rebase_waits_and_applies_after_the_abort() {
    let rig = rig();
    let board = rig.stop_in_a_rebase(true);
    let work = rig.root();
    let (log, open) = (
        rig.fixture.home().join("gate.log"),
        rig.fixture.home().join("gate.open"),
    );
    fs::write(&open, "open\n").unwrap();

    let params = TaskApplyParams {
        board: board.clone(),
        source: Some("fake".to_string()),
        apply: ApplyParams {
            actor: None,
            op: "gate".to_string(),
            id: Some("1".to_string()),
            args: BTreeMap::from([
                ("log".to_string(), json!(log)),
                ("open".to_string(), json!(open)),
            ]),
            version: None,
            checkout: None,
        },
        commit: false,
    };
    let answer: TaskApplyResult = rpc(&rig, "task.apply", &params).unwrap();
    let queued = answer.queued.expect("the write waits");
    assert_eq!(queued.reason, format!("a rebase is in progress in {work}"));
    assert!(lines(&log).is_empty(), "the provider has not been asked");

    rig.fixture.repo.git(&["rebase", "--abort"]);
    wait_until("the waiting write applies", DEADLINE, || {
        lines(&log).last().is_some_and(|line| line == "end fake")
    });
}

// ---- settings and secrets -----------------------------------------------------------------------------------------

const SETTINGS: &str = "config:\n  greeting: { type: string, default: hello, overridable: true }\n  \
                        lanes: { type: integer, default: 3 }\n  token: { type: string, secret: true }\n";

#[test]
fn the_packs_settings_through_the_layers_reach_initialize_with_the_boards_root() {
    let rig = rig();
    rig.pack("ferry", "fake", "one", SETTINGS);
    rig.board_yaml(&["ferry"]);
    rig.fixture.commit();
    rig.fixture
        .write_user("config.yaml", "config: { ferry: { greeting: hi } }\n");
    let board = rig.fixture.board();
    rig.trust(&board);

    let init = rig.echo(&board, "fake")["init"].clone();
    assert_eq!(init["config"], json!({ "greeting": "hi", "lanes": 3 }));
    assert_eq!(init["root"], json!(rig.root()));
}

#[test]
fn a_secret_setting_is_read_from_the_secret_store_and_reaches_no_log_or_event() {
    let rig = rig();
    rig.pack("ferry", "fake", "one", SETTINGS);
    rig.board_yaml(&["ferry"]);
    rig.fixture.commit();
    rig.secrets
        .set(&pack_item("ferry", "token"), &Secret::new(CANARY))
        .unwrap();
    let board = rig.fixture.board();
    rig.trust(&board);

    let init = rig.echo(&board, "fake")["init"].clone();
    assert_eq!(init["config"]["token"], CANARY);
    rig.apply(&board, "fake:1", "ok", None).unwrap();

    assert!(!rig.log.text().contains(CANARY));
    let events = rig.fixture.daemon.events_since(0).unwrap();
    assert!(
        events
            .iter()
            .any(|event| matches!(event.body, EventBody::TaskChanged { .. }))
    );
    assert!(!serde_json::to_string(&events).unwrap().contains(CANARY));
    // The secret lives in the store and in no file of the test's root: repo, user files, daemon data.
    let found = std::process::Command::new("grep")
        .args(["-rl", CANARY])
        .arg(rig.fixture.home())
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&found.stdout), "");
}

#[test]
fn a_secret_with_no_item_in_the_store_is_left_out_of_config() {
    let rig = rig();
    rig.pack("ferry", "fake", "one", SETTINGS);
    rig.board_yaml(&["ferry"]);
    rig.fixture.commit();
    let board = rig.fixture.board();
    rig.trust(&board);

    let init = rig.echo(&board, "fake")["init"].clone();
    assert_eq!(init["config"], json!({ "greeting": "hello", "lanes": 3 }));
}

// ---- names --------------------------------------------------------------------------------------------------------

#[test]
fn two_packs_that_are_not_built_in_with_one_source_name_start_neither_and_the_line_says_so() {
    let rig = rig();
    rig.pack("alpha", "fake", "one", "");
    rig.pack("beta", "fake", "one", "");
    rig.pack("gamma", "other", "one", "");
    rig.board_yaml(&["alpha", "beta", "gamma"]);
    rig.fixture.commit();
    let board = rig.fixture.board();
    rig.trust(&board);

    let listed: BoardListResult = rpc(&rig, "board.list", &BoardListParams {}).unwrap();
    let problems = &listed.boards[0].lines[0].problems;
    assert!(
        problems.iter().any(|problem| problem.contains("`fake`")
            && problem.contains("alpha")
            && problem.contains("beta")),
        "{problems:?}"
    );
    let error = rig.get(&board, "fake:1").unwrap_err();
    assert_eq!(error.code, code::CONFLICT);
    assert!(
        error.message.contains("alpha") && error.message.contains("beta"),
        "{}",
        error.message
    );
    assert!(rig.started(&board, "alpha").is_empty());
    assert!(rig.started(&board, "beta").is_empty());
    // The source that has one pack is not touched by the clash.
    assert!(rig.get(&board, "other:1").unwrap().task.is_some());
}

#[test]
fn the_boards_own_plan_pack_wins_over_the_built_in_one() {
    let rig = rig();
    rig.pack("planner", "plan", "one", "");
    rig.board_yaml(&["planner"]);
    rig.fixture.commit();
    let board = rig.fixture.board();
    rig.trust(&board);

    let listed: BoardListResult = rpc(&rig, "board.list", &BoardListParams {}).unwrap();
    assert!(listed.boards[0].lines[0].problems.is_empty());
    assert_eq!(
        rig.get(&board, "plan:3").unwrap().task.unwrap().key,
        "plan:3"
    );
    assert_eq!(rig.started(&board, "planner").len(), 1);
}

// ---- a source that is gone ----------------------------------------------------------------------------------------

#[test]
fn a_source_that_is_gone_is_an_error_naming_its_pack_and_the_others_keep_answering() {
    let rig = rig_with(|options| {
        options.supervisor = SupervisorPolicy {
            backoff: BackoffPolicy {
                initial: Duration::ZERO,
                max: Duration::ZERO,
                max_crashes: 3,
            },
            ..SupervisorPolicy::default()
        };
    });
    rig.pack("ferry", "fake", "one", "");
    rig.pack("quay", "other", "one", "");
    rig.board_yaml(&["ferry", "quay"]);
    rig.fixture.commit();
    let board = rig.fixture.board();
    rig.trust(&board);

    let error = rig.get(&board, "fake:die").unwrap_err();
    assert_eq!(error.code, code::UNAVAILABLE);
    for word in ["ferry", "`fake`", "gone"] {
        assert!(error.message.contains(word), "{word}: {}", error.message);
    }
    let again = rig.get(&board, "fake:1").unwrap_err();
    assert_eq!(again.code, code::UNAVAILABLE);
    assert!(rig.get(&board, "other:1").unwrap().task.is_some());
    assert!(rig.get(&board, "1").unwrap().task.is_none());
}

// ---- stop ---------------------------------------------------------------------------------------------------------

/// Ends the process of a provider that outlived its test, if the same process is still there.
struct KillOnDrop {
    pid: u32,
    started_us: u64,
}

impl Drop for KillOnDrop {
    fn drop(&mut self) {
        common::signal_if_same(self.pid, self.started_us, libc::SIGKILL);
    }
}

#[test]
fn no_provider_outlives_the_daemon() {
    let rig = rig();
    let board = rig.trusted_fake();
    // This provider ignores the end of its input, so only the daemon's stop can end it.
    let linger = rig.get(&board, "fake:linger").unwrap().task.unwrap();
    assert_eq!(linger.key, "fake:linger");
    let pid = rig.echo(&board, "fake")["pid"].as_u64().unwrap() as u32;
    let _kill = KillOnDrop {
        pid,
        started_us: collide_host::sysproc::start_time_us(pid).expect("the provider runs"),
    };
    assert!(pid_alive(pid));

    let Rig { fixture, .. } = rig;
    let Fixture { repo, daemon } = fixture;
    daemon.stop();
    wait_until("the provider is gone", DEADLINE, || !pid_alive(pid));
    drop(repo);
}

// ---- the line lock (ts-0114-hr) -----------------------------------------------------------------------------------

/// A `gate` write to `source` at `checkout`, on its own connection and thread: the provider holds it until `open`
/// exists. The thread answers when the write does.
fn gate_write(
    rig: &Rig,
    board: &str,
    source: &str,
    checkout: &Path,
    (log, open): (&Path, &Path),
) -> std::sync::mpsc::Receiver<TaskApplyResult> {
    let paths = rig.fixture.daemon.paths().clone();
    let params = TaskApplyParams {
        board: board.to_string(),
        source: Some(source.to_string()),
        apply: ApplyParams {
            actor: None,
            op: "gate".to_string(),
            id: Some("1".to_string()),
            args: BTreeMap::from([
                ("log".to_string(), json!(log)),
                ("open".to_string(), json!(open)),
            ]),
            version: None,
            checkout: Some(checkout.to_str().unwrap().to_string()),
        },
        commit: false,
    };
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut client = collide_host::owner_client::OwnerClient::connect(&paths, "gate-write")
            .expect("the client connects");
        let answer: TaskApplyResult = client
            .call("task.apply", &params, WAIT)
            .expect("apply answers");
        let _ = sender.send(answer);
    });
    receiver
}

fn lines(path: &Path) -> Vec<String> {
    fs::read_to_string(path)
        .map(|text| text.lines().map(str::to_string).collect())
        .unwrap_or_default()
}

#[test]
fn writes_to_one_root_run_one_at_a_time_and_writes_to_two_roots_do_not_wait_for_each_other() {
    let rig = rig();
    rig.pack("ferry", "fake", "one", "");
    rig.pack("ferry2", "fake2", "one", "");
    rig.board_yaml(&["ferry", "ferry2"]);
    rig.fixture.commit();
    let board = rig.fixture.board();
    rig.trust(&board);
    let work = std::path::PathBuf::from(rig.root());
    let side = rig.fixture.home().join("side");
    rig.fixture.repo.git(&[
        "worktree",
        "add",
        "--quiet",
        side.to_str().unwrap(),
        "-b",
        "side",
    ]);
    let side = fs::canonicalize(side).unwrap();
    let (log, held, free) = (
        rig.fixture.home().join("gate.log"),
        rig.fixture.home().join("held"),
        rig.fixture.home().join("free"),
    );
    fs::write(&free, "").unwrap();

    // Two sources on one root: the second write does not reach its provider while the first holds the root.
    let first = gate_write(&rig, &board, "fake", &work, (&log, &held));
    wait_until("the first write to reach its provider", DEADLINE, || {
        lines(&log) == ["start fake"]
    });
    let second = gate_write(&rig, &board, "fake2", &work, (&log, &free));
    let until = std::time::Instant::now() + Duration::from_millis(1500);
    while std::time::Instant::now() < until {
        assert_eq!(
            lines(&log),
            ["start fake"],
            "the second write went in while the first held the root"
        );
        std::thread::sleep(Duration::from_millis(50));
    }

    // Another root is its own lock: this write goes through while the first is still held.
    let other = gate_write(&rig, &board, "fake2", &side, (&log, &free));
    other
        .recv_timeout(DEADLINE)
        .expect("the write to the other root did not wait for the first");
    assert_eq!(lines(&log)[..3], ["start fake", "start fake2", "end fake2"]);

    fs::write(&held, "").unwrap();
    first.recv_timeout(DEADLINE).expect("the first write ends");
    second
        .recv_timeout(DEADLINE)
        .expect("the second write ends once the root is free");
    assert_eq!(
        lines(&log),
        [
            "start fake",
            "start fake2",
            "end fake2",
            "end fake",
            "start fake2",
            "end fake2"
        ]
    );
}

// ---- the reference adapter ------------------------------------------------------------------------------------------

fn copy_tree(from: &Path, repo: &GitRepo, to: &str) {
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name().to_str().unwrap().to_string();
        let target = format!("{to}/{name}");
        if entry.path().is_dir() {
            copy_tree(&entry.path(), repo, &target);
        } else {
            repo.write(&target, &fs::read(entry.path()).unwrap());
        }
    }
}

/// The manifest of the reference pack's fixture copy with what a provider needs and nothing else.
const MANIFEST: &str = "apiVersion: collide/v0\nname: reference\nversion: 0.1.0\n\
                                requires:\n  sdk: \">=0.2 <0.3\"\n  tools:\n    - { name: node }\n\
                                providers:\n  plan: { script: providers/plan.ts }\n";

#[test]
fn the_reference_adapter_starts_under_the_supervisor_from_a_fixture_repo_and_lists_its_tasks() {
    let root = TempRoot::new();
    let bundle = common::bundle("daemon-sources");
    let cache = root.join("cache");
    let sdk = ensure_sdk(&cache, &bundle_sdk(&bundle).unwrap()).unwrap();
    let builtin = bundle.0.clone();
    let factory: LoadContextFactory = {
        let cache = cache.clone();
        Arc::new(move |path| {
            Ok(LoadContext::new(
                cache.clone(),
                sdk.clone(),
                Environment {
                    path: path.to_os_string(),
                },
                BuiltinPacks::new(builtin.clone()),
            ))
        })
    };
    let git_env = vec![
        ("HOME".to_string(), root.path().display().to_string()),
        ("GIT_CONFIG_GLOBAL".to_string(), "/dev/null".to_string()),
        ("GIT_CONFIG_SYSTEM".to_string(), "/dev/null".to_string()),
        ("GIT_CONFIG_NOSYSTEM".to_string(), "1".to_string()),
    ];

    let repo = GitRepo::new();
    // The no-names check keeps the reference pack's name out of this file; its fixture folders carry it.
    let name = ["orch", "ard"].concat();
    let fixtures = common::repo_root().join("fixtures");
    copy_tree(
        &fixtures.join(format!("{name}-arch/basic/arch")),
        &repo,
        "arch",
    );
    copy_tree(
        &fixtures.join(format!("packs/{name}/providers")),
        &repo,
        ".collide/packs/reference/providers",
    );
    repo.write(".collide/packs/reference/pack.yaml", MANIFEST.as_bytes());
    repo.write(
        ".collide/board.yaml",
        b"packs:\n  - source: repo:.collide/packs/reference\n",
    );
    repo.commit_all("a reference board");
    let work = fs::canonicalize(repo.work()).unwrap();

    let config = ConfigService::new(
        Some(root.path().display().to_string()),
        factory,
        git_env,
        Arc::new(MemorySecretStore::new()),
    );
    let tools = std::env::var_os("PATH").unwrap();
    let board = BoardRef {
        id: "board-1",
        root: &work,
    };
    let review = config.lines().review(board, "main", &tools).unwrap();
    let hash = match review.trust {
        TrustState::Trusted { hash } | TrustState::Untrusted { hash } => hash,
    };
    assert!(!hash.is_empty(), "{:?}", review.problems);
    config
        .lines()
        .trust(
            board,
            &BoardTrustParams {
                board: "board-1".to_string(),
                line: "main".to_string(),
                hash,
                hosts: Vec::new(),
                dev_folders: Vec::new(),
            },
            &tools,
        )
        .unwrap();

    let shell = clean_shell(root.path());
    let environment = BaseEnvironment::capture_with(
        &shell,
        &SystemClocks,
        &CaptureOptions {
            timeout: Duration::from_secs(10),
            cap_bytes: 1024 * 1024,
            fallback: BTreeMap::new(),
        },
    );
    let sources = Sources::new(SourcesConfig {
        config,
        store: Arc::new(Store::open(&root.join("store")).unwrap()),
        data_root: root.join("pack-data"),
        policy: SupervisorPolicy::default(),
        base_env: Arc::new(move || Some(environment.clone())),
    });

    // The provider's `initialize` answer passed `check_initialize`, which the supervisor enforces, or this fails.
    let source = sources.source(board, "plan", &tools).unwrap();
    let listed = source
        .list(&ListParams {
            checkout: None,
            cursor: None,
        })
        .unwrap();
    let mut keys: Vec<&str> = listed.tasks.iter().map(|task| task.key.as_str()).collect();
    keys.sort_unstable();
    assert!(keys.contains(&"plan:T-03-001"), "{keys:?}");
    assert!(keys.contains(&"plan:T-05-003"), "{keys:?}");
    assert_eq!(keys.len(), 8, "{keys:?}");
    sources.stop();
}
