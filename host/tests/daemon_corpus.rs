//! The corpus in the daemon (ts-0116-wh; 0.10, Identity, Protocol, Ref-scoped sources and Corpus): `task.list`, the
//! full `task.get`, the startup reconcile, the notification pump, the cache that outlives a stop, and the write that
//! is one event. The daemon runs inside the test process over its real socket. A repo pack's provider is
//! `host/tests/fixtures/corpus_pack_provider.mjs`, a ref-scoped source over `tasks/**/*.json` files, so a test makes,
//! edits and removes tasks with plain file writes and sends the notifications by appending to a file.
//!
//! Every path is below the test's own root under `/tmp`, which is also the home. The daemon's captured environment is
//! made by a login shell that clears the environment, so a provider gets only `HOME` and `COLLIDE_DAEMON_ROOT` of that
//! root, a `PATH` and its pack's data folder. Git reads no user or system config. Nothing here touches the user's
//! home, keychain or a real daemon.

mod common;

use collide_core::change::{Actor, ActorKind};
use collide_core::events::{Event, EventBody};
use collide_core::item::LinkDir;
use collide_core::non_empty::NonEmpty;
use collide_core::owner::{
    BoardListParams, BoardListResult, BoardOpenParams, BoardOpenResult, BoardTrustParams,
    BoardTrustResult, BoardTrustReviewParams, BoardTrustReviewResult, RpcError, TaskApplyParams,
    TaskApplyResult, TaskGetParams, TaskGetResult, TaskListResult, TaskSaveViewResult,
    TaskViewsParams, TaskViewsResult, TrustState, ViewEntry, ViewSource,
};
use collide_core::provider::{ApplyParams, ApplyResult};
use collide_core::state::RunState;
use collide_core::view::View;
use collide_core::yaml::load_yaml_file;
use collide_host::corpus_service::GONE_PERIODS;
use collide_host::corpus_store::CorpusCache;
use collide_host::daemon::{DaemonOptions, DaemonPaths};
use collide_host::owner_client::{ClientError, OwnerClient};
use collide_host::run_store::RunLaunch;
use collide_host::secret_store::MemorySecretStore;
use collide_host::store::Store;
use collide_host::supervisor::{BackoffPolicy, SupervisorPolicy};
use collide_host::task_cli::{parse_task, parse_tasks};
use collide_host::task_daemon::{WAIT_NOTICE_AFTER, run_noticing_after};
use common::config_fixture::{CapturedLog, Fixture};
use common::git_repo::{GitRepo, git_command};
use common::{node_missing_message, wait_until};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

const WAIT: Option<Duration> = Some(Duration::from_secs(60));
const DEADLINE: Duration = Duration::from_secs(30);
const RECHECK: Duration = Duration::from_millis(150);

const PROVIDER: &str = include_str!("fixtures/corpus_pack_provider.mjs");

// ---- the rig ------------------------------------------------------------------------------------------------------

/// A login shell that prints a cleared environment, as the capture asks.
fn clean_shell(root: &Path) -> String {
    let path = std::env::var("PATH").expect("the tests run with a PATH");
    let home = root.display();
    let script = format!(
        "#!/bin/sh\nexec /usr/bin/env -i HOME='{home}' COLLIDE_DAEMON_ROOT='{home}' PATH='{path}' /bin/sh -c \"$4\"\n"
    );
    let file = root.join("clean-shell");
    fs::write(&file, script).unwrap();
    fs::set_permissions(&file, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    file.to_str().unwrap().to_string()
}

type Configure = Arc<dyn Fn(&mut DaemonOptions) + Send + Sync>;

struct Rig {
    fixture: Option<Fixture>,
    log: CapturedLog,
    configure: Configure,
}

fn rig() -> Rig {
    rig_with(|_| {})
}

fn rig_with(extra: impl Fn(&mut DaemonOptions) + Send + Sync + 'static) -> Rig {
    if let Some(message) = node_missing_message(&std::env::var_os("PATH").unwrap_or_default()) {
        panic!("{message}");
    }
    let secrets = Arc::new(MemorySecretStore::new());
    let log = CapturedLog::default();
    let configure: Configure = {
        let (held, writer) = (secrets.clone(), log.clone());
        Arc::new(move |options: &mut DaemonOptions| {
            let root = PathBuf::from(options.home.clone().expect("the fixture sets a home"));
            options.shell = clean_shell(&root);
            options.secret_store = held.clone();
            options.log = Box::new(writer.clone());
            options.corpus_recheck = RECHECK;
            options.workspaces_dir = Some(root.join("workspaces"));
            extra(options);
        })
    };
    let fixture = {
        let configure = configure.clone();
        Fixture::with_options(move |options| configure(options))
    };
    Rig {
        fixture: Some(fixture),
        log,
        configure,
    }
}

fn rpc<R: DeserializeOwned>(
    rig: &Rig,
    method: &str,
    params: &impl Serialize,
) -> Result<R, RpcError> {
    match rig.f().client().call(method, params, WAIT) {
        Ok(result) => Ok(result),
        Err(ClientError::Rpc(error)) => Err(error),
        Err(other) => panic!("{method}: {other}"),
    }
}

/// What a test says about one source: its pack, its name, whether it declares `watch`, and its page size.
struct Src {
    pack: &'static str,
    source: &'static str,
    watch: bool,
    page: usize,
    /// Settings the pack declares, as the lines of a `config:` section.
    config: &'static str,
}

const FILES: Src = Src {
    pack: "quay",
    source: "files",
    watch: true,
    page: 100,
    config: "",
};

/// A source that declares no `watch`: the corpus lists it again at each recheck.
const DOCS: Src = Src {
    pack: "ferry",
    source: "docs",
    watch: false,
    page: 100,
    config: "",
};

/// A setting a pack declares, and what `config.set` changes.
const GREETING: &str = "config:\n  greeting: { type: string, default: hello }\n";

impl Rig {
    fn f(&self) -> &Fixture {
        self.fixture.as_ref().expect("the rig has a daemon")
    }

    /// Stops the daemon and starts another over the same folders.
    fn restart(&mut self) {
        let fixture = self.fixture.take().expect("the rig has a daemon");
        let configure = self.configure.clone();
        self.fixture = Some(fixture.restart(move |options| configure(options)));
    }

    /// Writes the repo pack of `src`.
    fn pack(&self, src: &Src) {
        let dir = format!(".collide/packs/{}", src.pack);
        let manifest = format!(
            "apiVersion: collide/v0\nname: {}\nversion: 1.0.0\nproviders:\n  {}: {{ script: providers/{}.mjs }}\n{}",
            src.pack, src.source, src.source, src.config
        );
        self.f().write(&format!("{dir}/pack.yaml"), &manifest);
        self.f().write(
            &format!("{dir}/providers/{}.mjs", src.source),
            &PROVIDER
                .replace("__SOURCE__", src.source)
                .replace("__WATCH__", if src.watch { "true" } else { "false" })
                .replace("__PAGE__", &src.page.to_string()),
        );
    }

    /// Lists the packs in `board.yaml`, with `extra` after them.
    fn board_yaml(&self, packs: &[&str], extra: &str) {
        let list: String = packs
            .iter()
            .map(|pack| format!("  - source: repo:.collide/packs/{pack}\n"))
            .collect();
        self.f()
            .write(".collide/board.yaml", &format!("packs:\n{list}{extra}"));
    }

    /// A trusted board whose line has the packs of `sources`. Returns its id.
    fn board_with(&self, sources: &[&Src], extra: &str) -> String {
        for src in sources {
            self.pack(src);
        }
        let packs: Vec<&str> = sources.iter().map(|src| src.pack).collect();
        self.board_yaml(&packs, extra);
        self.f().commit();
        let board = self.f().board();
        self.trust(&board);
        board
    }

    /// The board's root as the daemon keeps it.
    fn root(&self) -> PathBuf {
        fs::canonicalize(self.f().repo.work()).unwrap()
    }

    fn trust(&self, board: &str) {
        let review: BoardTrustReviewResult = rpc(
            self,
            "board.trust_review",
            &BoardTrustReviewParams {
                board: board.to_string(),
                line: "main".to_string(),
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
                line: "main".to_string(),
                hash,
                hosts: Vec::new(),
                dev_folders: Vec::new(),
            },
        )
        .unwrap();
    }

    /// `task.list` with the given extra params.
    fn list(&self, board: &str, extra: Value) -> TaskListResult {
        let mut params = json!({ "board": board });
        for (key, value) in extra.as_object().expect("params are an object") {
            params[key] = value.clone();
        }
        rpc(self, "task.list", &params).unwrap()
    }

    /// Polls `task.list` until `done` holds of the first page, and returns that page.
    fn list_until(
        &self,
        board: &str,
        what: &str,
        done: impl Fn(&TaskListResult) -> bool,
    ) -> TaskListResult {
        let mut last = None;
        wait_until(what, DEADLINE, || {
            let result = self.list(board, json!({}));
            let ok = done(&result);
            last = Some(result);
            ok
        });
        last.unwrap()
    }

    /// The first page once the first listing ended.
    fn settled(&self, board: &str) -> TaskListResult {
        self.list_until(board, "the first listing ends", |list| !list.refreshing)
    }

    fn keys(list: &TaskListResult) -> Vec<String> {
        list.tasks.iter().map(|task| task.key.clone()).collect()
    }

    fn get(&self, board: &str, id: &str) -> TaskGetResult {
        rpc(
            self,
            "task.get",
            &TaskGetParams {
                board: board.to_string(),
                id: id.to_string(),
                checkout: None,
            },
        )
        .unwrap()
    }

    fn set_status(
        &self,
        board: &str,
        source: &str,
        id: &str,
        status: &str,
        checkout: Option<&Path>,
    ) -> ApplyResult {
        let mut args = BTreeMap::new();
        args.insert("status".to_string(), json!(status));
        let answer: TaskApplyResult = rpc(
            self,
            "task.apply",
            &TaskApplyParams {
                board: board.to_string(),
                source: Some(source.to_string()),
                apply: ApplyParams {
                    actor: None,
                    op: "setStatus".to_string(),
                    id: Some(id.to_string()),
                    args,
                    version: None,
                    checkout: checkout.map(|path| path.display().to_string()),
                },
                commit: false,
            },
        )
        .unwrap();
        answer.result.expect("the write was applied, not queued")
    }

    /// The data folder of `src`'s provider on `board`.
    fn data(&self, board: &str, src: &Src) -> PathBuf {
        self.f()
            .home()
            .join("data/pack-data")
            .join(board)
            .join(src.pack)
    }

    /// The pids of the starts of `src`'s provider, one for each `initialize` it answered.
    fn started(&self, board: &str, src: &Src) -> Vec<String> {
        fs::read_to_string(self.data(board, src).join("started"))
            .map(|text| text.lines().map(str::to_string).collect())
            .unwrap_or_default()
    }

    /// The params of the provider's last `initialize`.
    fn init(&self, board: &str, src: &Src) -> Value {
        let text = fs::read_to_string(self.data(board, src).join("init.json")).unwrap();
        serde_json::from_str(&text).unwrap()
    }

    /// The lines the provider logged of its `list` and `watch` calls.
    fn calls(&self, board: &str, src: &Src) -> Vec<String> {
        fs::read_to_string(self.data(board, src).join("calls.log"))
            .map(|text| text.lines().map(str::to_string).collect())
            .unwrap_or_default()
    }

    /// Waits until the provider has been told to watch, which is after it listed and after it read its start offset.
    fn wait_watched(&self, board: &str, src: &Src) {
        wait_until("the provider is told to watch", DEADLINE, || {
            self.calls(board, src)
                .iter()
                .any(|line| line.starts_with("watch "))
        });
    }

    /// Sends the provider a notification: a line it reads from its control file.
    fn notify(&self, board: &str, src: &Src, method: &str, params: Value) {
        use std::io::Write;
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.data(board, src).join("notify.jsonl"))
            .unwrap();
        let line = format!("{}\n", json!({ "method": method, "params": params }));
        file.write_all(line.as_bytes()).unwrap();
    }

    /// Makes, or replaces, the task file `rel` below `checkout/tasks`.
    fn task(&self, checkout: &Path, rel: &str, short: &str, status: &str, version: &str) {
        let path = checkout.join("tasks").join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let data = json!({
            "short": short, "title": format!("Task {short}"), "status": status, "version": version,
        });
        fs::write(path, format!("{data}\n")).unwrap();
    }

    fn file(&self, checkout: &Path, rel: &str, text: &str) {
        let path = checkout.join("tasks").join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    fn remove(&self, checkout: &Path, rel: &str) {
        fs::remove_file(checkout.join("tasks").join(rel)).unwrap();
    }

    /// Every event in the store, oldest first.
    fn events(&self) -> Vec<Event> {
        self.f().daemon.events_since(0).unwrap()
    }

    /// The task events of `source`, oldest first: what the corpus recorded.
    fn source_events(&self, source: &str) -> Vec<Event> {
        self.events()
            .into_iter()
            .filter(|event| match &event.body {
                EventBody::TaskChanged { source: held, .. }
                | EventBody::TaskRenamed { source: held, .. }
                | EventBody::TaskRemoved { source: held, .. }
                | EventBody::TaskInvalid { source: held, .. } => held == source,
                _ => false,
            })
            .collect()
    }

    fn last_seq(&self) -> u64 {
        self.events().last().map_or(0, |event| event.seq)
    }

    /// The task events of `source` after `seq`.
    fn source_events_after(&self, source: &str, seq: u64) -> Vec<Event> {
        self.source_events(source)
            .into_iter()
            .filter(|event| event.seq > seq)
            .collect()
    }

    /// The task events of `source` after `seq`, once there are `count` of them. The corpus records an event just after
    /// it changes the list, so a test that saw the list change waits for the event.
    fn wait_events(&self, source: &str, seq: u64, count: usize) -> Vec<Event> {
        wait_until("the events are recorded", DEADLINE, || {
            self.source_events_after(source, seq).len() >= count
        });
        self.source_events_after(source, seq)
    }

    /// Records a run of the workflow `collide/quick` on `board` for the tasks `keys`, and ends it.
    fn run(&self, run: &str, board: &str, keys: &[&str], to: RunState) {
        let phases = self.f().daemon.phases();
        let actor = Actor {
            kind: ActorKind::Workflow,
            run: Some(run.to_string()),
            phase: None,
        };
        let transition = |from: Option<RunState>, to: RunState| {
            phases
                .append_event(
                    actor.clone(),
                    EventBody::RunTransition {
                        run: run.to_string(),
                        from,
                        to,
                    },
                )
                .unwrap();
        };
        transition(None, RunState::Running {});
        let linked = self
            .f()
            .daemon
            .store()
            .set_run_launch(
                run,
                &RunLaunch {
                    board: Some(board.to_string()),
                    tasks: keys
                        .iter()
                        .map(|key| collide_core::owner::RunTaskRef {
                            key: key.to_string(),
                            status: "doing".to_string(),
                        })
                        .collect(),
                    line: "main".to_string(),
                    workflow: "collide/quick".to_string(),
                },
            )
            .unwrap();
        assert!(linked, "{run} has a run.transition before it is linked");
        if !matches!(to, RunState::Running {}) {
            transition(Some(RunState::Running {}), to);
        }
    }

    /// A second worktree of the board's repository on a new branch. One in `workspaces` is a workspace of the board
    /// and one beside it is any other worktree. Returns its real path.
    fn worktree(&self, name: &str, branch: &str, workspace: bool) -> PathBuf {
        let base = if workspace {
            self.f().home().join("workspaces")
        } else {
            self.f().home().to_path_buf()
        };
        fs::create_dir_all(&base).unwrap();
        let path = base.join(name);
        self.f().repo.git(&[
            "worktree",
            "add",
            "-q",
            "-b",
            branch,
            path.to_str().unwrap(),
        ]);
        fs::canonicalize(path).unwrap()
    }

    /// What the store holds as the board's corpus cache, read through a handle of its own.
    fn cache(&self, board: &str) -> CorpusCache {
        let store = Store::open(&self.f().daemon.paths().store_dir).unwrap();
        store.corpus_cache(board).unwrap()
    }
}

fn ended(outcome: &str) -> RunState {
    RunState::Ended {
        outcome: NonEmpty::new(outcome).unwrap(),
    }
}

// ---- list, paging and checkouts -----------------------------------------------------------------------------------

#[test]
fn a_list_reads_every_page_of_the_provider_and_names_what_it_could_not_read() {
    let rig = rig();
    let paged = Src { page: 2, ..FILES };
    let board = rig.board_with(&[&paged], "");
    let root = rig.root();
    for short in ["1", "2", "3", "4", "5"] {
        rig.task(&root, &format!("{short}.json"), short, "todo", "v1");
    }
    rig.file(&root, "bad.json", "{ this is not json");

    let list = rig.settled(&board);

    assert_eq!(
        Rig::keys(&list),
        ["files:1", "files:2", "files:3", "files:4", "files:5"]
    );
    assert_eq!(list.invalid.len(), 1, "{:?}", list.invalid);
    assert_eq!(list.invalid[0].path, "tasks/bad.json");
    assert!(list.stale_sources.is_empty(), "{:?}", list.stale_sources);
    let lists: Vec<String> = rig
        .calls(&board, &paged)
        .into_iter()
        .filter(|line| line.starts_with("list "))
        .collect();
    let root_text = root.display();
    assert_eq!(
        lists,
        [
            format!("list {root_text} "),
            format!("list {root_text} 2"),
            format!("list {root_text} 4"),
        ]
    );
}

#[test]
fn pages_of_a_limit_follow_the_cursor_and_only_the_first_names_the_board_wide_lists() {
    let rig = rig();
    let board = rig.board_with(&[&FILES], "");
    let root = rig.root();
    for short in ["1", "2", "3", "4", "5"] {
        rig.task(&root, &format!("{short}.json"), short, "todo", "v1");
    }
    rig.file(&root, "bad.json", "nope");
    rig.settled(&board);

    let first = rig.list(&board, json!({ "limit": 2 }));
    assert_eq!(Rig::keys(&first), ["files:1", "files:2"]);
    assert_eq!(first.invalid.len(), 1);
    let second = rig.list(
        &board,
        json!({ "limit": 2, "cursor": first.next_cursor.clone().expect("a cursor") }),
    );
    assert_eq!(Rig::keys(&second), ["files:3", "files:4"]);
    assert!(second.invalid.is_empty());
    let third = rig.list(
        &board,
        json!({ "limit": 2, "cursor": second.next_cursor.clone().expect("a cursor") }),
    );
    assert_eq!(Rig::keys(&third), ["files:5"]);
    assert_eq!(third.next_cursor, None);
}

#[test]
fn the_ready_and_checkout_filters_narrow_the_list() {
    let rig = rig();
    let board = rig.board_with(&[&FILES], "");
    let root = rig.root();
    rig.task(&root, "1.json", "1", "todo", "v1");
    rig.task(&root, "2.json", "2", "doing", "v1");
    rig.task(&root, "3.json", "3", "done", "v1");
    rig.settled(&board);

    let ready = rig.list(&board, json!({ "ready": true }));
    assert_eq!(Rig::keys(&ready), ["files:1"]);
    let here = rig.list(&board, json!({ "checkout": root.display().to_string() }));
    assert_eq!(here.tasks.len(), 3);
    let nowhere = rig.list(&board, json!({ "checkout": "/no/such/checkout" }));
    assert!(nowhere.tasks.is_empty());
}

#[test]
fn a_worktree_is_a_checkout_and_its_copy_that_differs_is_named_divergent() {
    let rig = rig();
    let board = rig.board_with(&[&FILES], "");
    let root = rig.root();
    let other = rig.worktree("wt-a", "feature/a", true);
    rig.task(&root, "1.json", "1", "todo", "v1");
    rig.task(&root, "2.json", "2", "todo", "v1");
    rig.task(&other, "1.json", "1", "doing", "v2");
    rig.task(&other, "2.json", "2", "todo", "v1");

    let list = rig.settled(&board);

    assert_eq!(
        Rig::keys(&list),
        ["files:1", "files:2"],
        "one row for each key"
    );
    assert_eq!(list.divergent, ["files:1"]);
    let there = rig.list(&board, json!({ "checkout": other.display().to_string() }));
    let one = there
        .tasks
        .iter()
        .find(|task| task.key == "files:1")
        .unwrap();
    assert_eq!(one.state.status, "doing");
    assert_eq!(one.checkout.as_deref(), Some(other.to_str().unwrap()));
    let both = rig.get(&board, "files:1");
    let mut checkouts: Vec<&str> = both
        .copies
        .iter()
        .map(|copy| copy.checkout.as_str())
        .collect();
    checkouts.sort_unstable();
    let mut expected = [root.to_str().unwrap(), other.to_str().unwrap()];
    expected.sort_unstable();
    assert_eq!(checkouts, expected);
}

#[test]
fn a_worktree_the_boards_filter_excludes_is_not_listed() {
    let rig = rig();
    let board = rig.board_with(&[&FILES], "checkout_filter:\n  exclude: ['scratch/*']\n");
    let root = rig.root();
    let skipped = rig.worktree("wt-skip", "scratch/one", false);
    rig.task(&root, "1.json", "1", "todo", "v1");
    rig.task(&skipped, "9.json", "9", "todo", "v1");

    let list = rig.settled(&board);

    assert_eq!(Rig::keys(&list), ["files:1"]);
    let watched: Vec<String> = rig
        .calls(&board, &FILES)
        .into_iter()
        .filter(|line| line.contains(skipped.to_str().unwrap()))
        .collect();
    assert!(
        watched.is_empty(),
        "the excluded checkout is never read: {watched:?}"
    );
}

// ---- the startup reconcile ----------------------------------------------------------------------------------------

/// What an event is, in a line a test compares: `changed files:1 startup`, `removed files:2`, `invalid tasks/bad.json`.
fn describe(event: &Event) -> String {
    match &event.body {
        EventBody::TaskChanged { task, .. } => format!(
            "changed {} {:?}->{:?} {}",
            task.key,
            task.from.as_ref().map(|state| state.status.as_str()),
            task.to.as_ref().map(|state| state.status.as_str()),
            serde_json::to_value(task.cause).unwrap().as_str().unwrap()
        ),
        EventBody::TaskRemoved { task, .. } => format!("removed {}", task.key),
        EventBody::TaskRenamed { rename, .. } => format!("renamed {} {}", rename.from, rename.to),
        EventBody::TaskInvalid { invalid, .. } => format!("invalid {}", invalid.path),
        other => panic!("not a task event: {other:?}"),
    }
}

fn described(events: &[Event]) -> Vec<String> {
    let mut lines: Vec<String> = events.iter().map(describe).collect();
    lines.sort();
    lines
}

#[test]
fn a_restart_records_what_changed_while_the_daemon_was_stopped_and_the_next_restart_records_nothing()
 {
    let mut rig = rig();
    let board = rig.board_with(&[&FILES], "");
    let root = rig.root();
    rig.task(&root, "1.json", "1", "todo", "v1");
    rig.task(&root, "2.json", "2", "todo", "v1");
    rig.task(&root, "3.json", "3", "todo", "v1");
    rig.file(&root, "bad.json", "not json");
    rig.settled(&board);
    // The first start has an empty cache, so each task is new to it (the plan's rule, with no exception).
    assert_eq!(
        described(&rig.source_events("files")),
        [
            "changed files:1 None->Some(\"todo\") startup",
            "changed files:2 None->Some(\"todo\") startup",
            "changed files:3 None->Some(\"todo\") startup",
            "invalid tasks/bad.json",
        ]
    );
    let before = rig.last_seq();

    rig.task(&root, "1.json", "1", "doing", "v2");
    rig.remove(&root, "2.json");
    rig.task(&root, "4.json", "4", "todo", "v1");
    rig.file(&root, "worse.json", "{");
    rig.restart();
    rig.settled(&board);

    assert_eq!(
        described(&rig.source_events_after("files", before)),
        [
            "changed files:1 Some(\"todo\")->Some(\"doing\") startup",
            "changed files:4 None->Some(\"todo\") startup",
            "invalid tasks/worse.json",
            "removed files:2",
        ]
    );
    let after_second = rig.last_seq();

    rig.restart();
    let list = rig.settled(&board);

    assert_eq!(Rig::keys(&list), ["files:1", "files:3", "files:4"]);
    assert!(
        rig.source_events_after("files", after_second).is_empty(),
        "a start that finds the same tasks records nothing"
    );
}

#[test]
fn the_list_a_restart_serves_while_it_refreshes_is_the_cached_one() {
    let mut rig = rig();
    let board = rig.board_with(&[&FILES], "");
    let root = rig.root();
    rig.task(&root, "1.json", "1", "todo", "v1");
    rig.task(&root, "2.json", "2", "todo", "v1");
    rig.settled(&board);
    let cached = rig.cache(&board);
    assert_eq!(cached.tasks.len(), 2);

    rig.restart();
    // Whatever moment the list is asked at, it never has fewer tasks than the cache: refreshing or not.
    let list = rig.list(&board, json!({}));

    assert_eq!(
        Rig::keys(&list),
        ["files:1", "files:2"],
        "refreshing: {}",
        list.refreshing
    );
}

// ---- the command that waits for the relist (ts-0155-wb) ------------------------------------------------------------

const WAITING_LINE: &str =
    "collide: the daemon is still listing the board's sources; waiting for the full list\n";

/// What a command wrote to standard error, less the line about the built-in `plan` source: this rig has no bundle, so
/// that source never starts.
fn without_plan_line(text: &str) -> String {
    text.lines()
        .filter(|line| !line.starts_with("collide: source plan could not start"))
        .map(|line| format!("{line}\n"))
        .collect()
}

/// How long a test lets a command that waits on a held listing go on asking, so that it asks more than once.
const SEVERAL_POLLS: Duration = Duration::from_millis(500);

/// `collide <words>` from the board's folder, on a thread of its own: it ends with its exit code and standard output.
/// What it wrote to standard error is read through the second value while it runs.
fn start_cli(
    rig: &Rig,
    words: &[&str],
    notice_after: Duration,
) -> (JoinHandle<(u8, String)>, CapturedLog) {
    let (paths, cwd) = (rig.f().daemon.paths().clone(), rig.root());
    let words: Vec<String> = words.iter().map(|word| word.to_string()).collect();
    let err = CapturedLog::default();
    let handle = {
        let mut err = err.clone();
        std::thread::spawn(move || {
            let rest = &words[1..];
            let command = match words[0].as_str() {
                "tasks" => parse_tasks(rest),
                _ => parse_task(rest),
            }
            .unwrap_or_else(|error| panic!("{words:?}: {}", error.message));
            let mut out = Vec::new();
            let code = run_noticing_after(&paths, &command, &cwd, &mut out, &mut err, notice_after);
            (code, String::from_utf8(out).unwrap())
        })
    };
    (handle, err)
}

/// A board with two tasks whose list the daemon has cached. Returns the rig, the board and the hold-back file.
fn board_with_a_cached_list() -> (Rig, String, PathBuf) {
    let rig = rig();
    let board = rig.board_with(&[&FILES], "");
    let root = rig.root();
    rig.task(&root, "1.json", "1", "todo", "v1");
    rig.task(&root, "2.json", "2", "todo", "v1");
    rig.settled(&board);
    let hold = rig.data(&board, &FILES).join("hold-list");
    (rig, board, hold)
}

/// Restarts the daemon with its first listing held back by the file `hold`.
fn restart_with_the_list_held(rig: &mut Rig, board: &str, hold: &Path) {
    fs::write(hold, "").unwrap();
    rig.restart();
    assert!(
        rig.list(board, json!({})).refreshing,
        "a held listing is refreshing"
    );
}

#[test]
fn a_command_waiting_for_the_relist_after_a_restart_says_so_once_and_prints_what_it_would_have() {
    let (mut rig, board, hold) = board_with_a_cached_list();
    for words in [
        vec!["tasks"],
        vec!["tasks", "--json"],
        vec!["task", "files:1"],
        vec!["task", "1"],
    ] {
        restart_with_the_list_held(&mut rig, &board, &hold);
        // A notice time of zero: the line comes with the first answer that says `refreshing`, whatever the clock.
        let (waiting, err) = start_cli(&rig, &words, Duration::ZERO);
        wait_until("the command says it waits", DEADLINE, || {
            err.text().contains("still listing")
        });
        let said = Instant::now();
        wait_until("the command has asked again", DEADLINE, || {
            said.elapsed() >= SEVERAL_POLLS
        });
        assert!(!waiting.is_finished(), "{words:?} waits for the list");

        fs::remove_file(&hold).unwrap();
        let (code, out) = waiting.join().unwrap();

        assert_eq!(code, 0, "{words:?}: {}", err.text());
        assert_eq!(without_plan_line(&err.text()), WAITING_LINE, "{words:?}");
        let (settled, quiet) = start_cli(&rig, &words, Duration::ZERO);
        let (settled_code, settled_out) = settled.join().unwrap();
        assert_eq!(settled_code, 0, "{words:?}");
        assert_eq!(without_plan_line(&quiet.text()), "", "{words:?}");
        assert_eq!(out, settled_out, "{words:?}: standard output is the same");
    }
}

#[test]
fn a_listing_that_settles_before_the_notice_time_says_nothing() {
    let (mut rig, board, hold) = board_with_a_cached_list();
    restart_with_the_list_held(&mut rig, &board, &hold);
    let (waiting, err) = start_cli(&rig, &["tasks"], Duration::from_secs(3600));
    let started = Instant::now();
    wait_until("the command has asked again", DEADLINE, || {
        started.elapsed() >= SEVERAL_POLLS
    });

    fs::remove_file(&hold).unwrap();
    let (code, out) = waiting.join().unwrap();

    assert_eq!(code, 0);
    assert_eq!(without_plan_line(&err.text()), "");
    assert_eq!(out.lines().count(), 2, "{out}");
}

#[test]
fn the_notice_comes_after_half_a_second() {
    assert_eq!(WAIT_NOTICE_AFTER, Duration::from_millis(500));
}

// ---- notifications ------------------------------------------------------------------------------------------------

fn state_json(status: &str) -> Value {
    let category = match status {
        "todo" => "ready",
        "doing" => "active",
        _ => "done",
    };
    json!({ "status": status, "category": category })
}

/// A `task.changed` notification as the provider sends it.
fn changed(
    key: &str,
    version: &str,
    from: Option<&str>,
    to: Option<&str>,
    checkout: &Path,
) -> Value {
    json!({
        "key": key, "version": version, "diff": {}, "cause": "worktree",
        "from": from.map(state_json), "to": to.map(state_json),
        "checkout": checkout.display().to_string(),
    })
}

#[test]
fn a_watched_change_updates_the_list_and_the_cache_and_records_one_event_tagged_with_its_source() {
    let rig = rig();
    let board = rig.board_with(&[&FILES], "");
    let root = rig.root();
    rig.task(&root, "1.json", "1", "todo", "v1");
    rig.settled(&board);
    rig.wait_watched(&board, &FILES);
    let before = rig.last_seq();

    rig.task(&root, "1.json", "1", "doing", "v2");
    rig.notify(
        &board,
        &FILES,
        "task.changed",
        changed("files:1", "v2", Some("todo"), Some("doing"), &root),
    );
    let list = rig.list_until(&board, "the change is listed", |list| {
        list.tasks
            .first()
            .is_some_and(|task| task.state.status == "doing")
    });

    assert_eq!(
        list.tasks[0].version, "v2",
        "the copy takes the version of the notice"
    );
    assert_eq!(
        described(&rig.wait_events("files", before, 1)),
        ["changed files:1 Some(\"todo\")->Some(\"doing\") worktree"]
    );
    let cached = rig.cache(&board);
    assert_eq!(cached.tasks[0].task.state.status, "doing");
}

#[test]
fn a_task_apply_write_is_one_event_with_the_connections_actor_and_the_watchers_notice_of_it_is_dropped()
 {
    let rig = rig();
    let board = rig.board_with(&[&FILES], "");
    let root = rig.root();
    rig.task(&root, "1.json", "1", "todo", "v1");
    rig.settled(&board);
    rig.wait_watched(&board, &FILES);
    let before = rig.last_seq();

    let written = rig.set_status(&board, "files", "files:1", "doing", None);
    let version = match written {
        ApplyResult::Ok { version, .. } => version,
        other => panic!("expected ok: {other:?}"),
    };
    // The write is in the list at once: no wait for the watcher.
    let list = rig.list(&board, json!({}));
    assert_eq!(list.tasks[0].state.status, "doing");
    // The watcher says the same thing, then something newer.
    rig.notify(
        &board,
        &FILES,
        "task.changed",
        changed("files:1", &version, Some("todo"), Some("doing"), &root),
    );
    rig.notify(
        &board,
        &FILES,
        "task.changed",
        changed("files:1", "v9", Some("doing"), Some("done"), &root),
    );
    rig.list_until(&board, "the newer change is listed", |list| {
        list.tasks[0].state.status == "done"
    });

    let events = rig.wait_events("files", before, 2);
    assert_eq!(
        described(&events),
        [
            "changed files:1 Some(\"doing\")->Some(\"done\") worktree",
            "changed files:1 Some(\"todo\")->Some(\"doing\") worktree",
        ]
    );
    let write = events
        .iter()
        .find(|event| describe(event).contains("Some(\"todo\")"))
        .unwrap();
    assert_eq!(
        write.actor.kind,
        ActorKind::Person,
        "the connection's actor"
    );
    let watched = events
        .iter()
        .find(|event| describe(event).contains("Some(\"done\")"))
        .unwrap();
    assert_eq!(watched.actor.kind, ActorKind::Workflow, "the daemon's own");
}

#[test]
fn an_unparseable_file_is_an_invalid_item_with_no_queue_item_and_a_repair_brings_the_task_back() {
    let rig = rig();
    let board = rig.board_with(&[&FILES], "");
    let root = rig.root();
    rig.task(&root, "1.json", "1", "todo", "v1");
    rig.task(&root, "2.json", "2", "todo", "v1");
    rig.settled(&board);
    rig.wait_watched(&board, &FILES);
    let before = rig.last_seq();

    rig.file(&root, "1.json", "{ broken");
    rig.notify(
        &board,
        &FILES,
        "task.invalid",
        json!({ "path": "tasks/1.json", "checkout": root.display().to_string(), "reason": "not JSON" }),
    );
    let broken = rig.list_until(&board, "the item is invalid", |list| {
        list.invalid.len() == 1
    });

    assert_eq!(Rig::keys(&broken), ["files:2"]);
    assert_eq!(broken.invalid[0].path, "tasks/1.json");
    assert_eq!(
        described(&rig.wait_events("files", before, 1)),
        ["invalid tasks/1.json"]
    );
    assert!(
        rig.f().daemon.queue_items(None).is_empty(),
        "an invalid item is an event and a count"
    );

    rig.task(&root, "1.json", "1", "todo", "v2");
    rig.notify(
        &board,
        &FILES,
        "task.changed",
        changed("files:1", "v2", None, Some("todo"), &root),
    );
    let repaired = rig.list_until(&board, "the task is back", |list| list.invalid.is_empty());

    assert_eq!(Rig::keys(&repaired), ["files:1", "files:2"]);
    assert_eq!(
        repaired.tasks[0].version, "v2",
        "the task was fetched, since the notice has no task"
    );
    assert!(rig.cache(&board).invalid.is_empty());
}

#[test]
fn a_deleted_file_gives_task_removed_and_leaves_the_list_and_the_cache() {
    let rig = rig();
    let board = rig.board_with(&[&FILES], "");
    let root = rig.root();
    rig.task(&root, "1.json", "1", "todo", "v1");
    rig.task(&root, "2.json", "2", "todo", "v1");
    rig.settled(&board);
    rig.wait_watched(&board, &FILES);
    let before = rig.last_seq();

    rig.remove(&root, "2.json");
    rig.notify(
        &board,
        &FILES,
        "task.removed",
        json!({ "key": "files:2", "cause": "deleted", "checkout": root.display().to_string() }),
    );
    let list = rig.list_until(&board, "the task is gone", |list| list.tasks.len() == 1);

    assert_eq!(Rig::keys(&list), ["files:1"]);
    assert_eq!(
        described(&rig.wait_events("files", before, 1)),
        ["removed files:2"]
    );
    assert_eq!(rig.cache(&board).tasks.len(), 1);
}

#[test]
fn a_rename_gives_task_renamed_the_old_key_resolves_to_the_new_one_and_the_runs_follow_the_key() {
    let rig = rig();
    let board = rig.board_with(&[&FILES], "");
    let root = rig.root();
    rig.task(&root, "1.json", "1", "todo", "v1");
    rig.settled(&board);
    rig.wait_watched(&board, &FILES);
    rig.run("r1", &board, &["files:1"], ended("done"));
    let before = rig.last_seq();

    // The file keeps its path and now holds the new short id.
    rig.task(&root, "1.json", "7", "todo", "v1");
    rig.notify(
        &board,
        &FILES,
        "task.renamed",
        json!({ "from": "files:1", "to": "files:7", "checkout": root.display().to_string() }),
    );
    let list = rig.list_until(&board, "the new key is listed", |list| {
        Rig::keys(list) == ["files:7"]
    });

    assert_eq!(
        list.tasks[0].id, "7",
        "the renamed copy is fetched, so its id is the new one"
    );
    assert_eq!(
        described(&rig.wait_events("files", before, 1)),
        ["renamed files:1 files:7"]
    );
    let old = rig.get(&board, "files:1");
    assert_eq!(
        old.task.as_ref().unwrap().key,
        "files:7",
        "the old key resolves to the new one"
    );
    assert_eq!(old.runs, ["r1"], "the run of the old key follows the task");
    let latest = old.latest_run.expect("r1 ended");
    assert_eq!(
        (latest.id.as_str(), latest.outcome.as_str()),
        ("r1", "done")
    );
    assert_eq!(latest.workflow, "collide/quick");
    assert!(latest.state.is_empty());
    let cached = rig.cache(&board);
    assert_eq!(
        cached
            .tasks
            .iter()
            .map(|held| held.task.key.as_str())
            .collect::<Vec<_>>(),
        ["files:7"]
    );
}

#[test]
fn a_collision_has_its_short_id_at_two_paths_in_get_and_in_the_list_and_no_event() {
    let rig = rig();
    let board = rig.board_with(&[&FILES], "");
    let root = rig.root();
    let other = rig.worktree("wt-c", "feature/c", true);
    rig.task(&root, "a/12.json", "12", "todo", "v1");
    rig.task(&other, "b/12.json", "12", "todo", "v1");
    rig.settled(&board);
    rig.wait_watched(&board, &FILES);
    let before = rig.last_seq();
    let listed = |rig: &Rig| {
        rig.calls(&board, &FILES)
            .iter()
            .filter(|l| l.starts_with("list "))
            .count()
    };
    let lists = listed(&rig);

    let list = rig.list(&board, json!({}));
    assert_eq!(list.collisions.len(), 1);
    assert_eq!(list.collisions[0].short_id, "12");
    assert_eq!(list.collisions[0].locations.len(), 2);
    let got = rig.get(&board, "files:12");
    assert!(got.task.is_none());
    assert_eq!(got.ambiguous, ["files:12"]);
    assert_eq!(got.copies.len(), 2);
    assert_eq!(got.collisions.len(), 1);

    // The provider's notice is only a hint: the source is listed again, and nothing is recorded for it.
    rig.notify(
        &board,
        &FILES,
        "task.collision",
        json!({ "shortId": "12", "locations": [] }),
    );
    wait_until("the source is listed again", DEADLINE, || {
        listed(&rig) > lists
    });
    assert_eq!(rig.list(&board, json!({})).collisions.len(), 1);
    assert!(rig.source_events_after("files", before).is_empty());
}

#[test]
fn a_tasks_runs_come_newest_first_and_its_latest_run_is_the_newest_that_ended() {
    let rig = rig();
    let board = rig.board_with(&[&FILES], "");
    let root = rig.root();
    rig.task(&root, "1.json", "1", "todo", "v1");
    rig.settled(&board);
    rig.run("r1", &board, &["files:1"], ended("done"));
    rig.run("r2", &board, &["files:1"], RunState::Failed {});
    rig.run("r3", &board, &["files:1"], RunState::Running {});

    let got = rig.get(&board, "files:1");

    assert_eq!(got.runs, ["r3", "r2", "r1"]);
    let latest = got.latest_run.unwrap();
    assert_eq!(
        (latest.id.as_str(), latest.outcome.as_str()),
        ("r2", "failed")
    );
    assert_eq!(
        got.task.as_ref().unwrap().body.as_deref(),
        Some("body of 1"),
        "the body comes from the source"
    );
    assert_eq!(got.copies.len(), 1);
}

// ---- scratch ------------------------------------------------------------------------------------------------------

impl Rig {
    /// Opens a second, separate repository as a board of the daemon. Returns its id and keeps the repository.
    fn second_board(&self) -> (String, GitRepo) {
        let repo = GitRepo::new();
        repo.write("README.md", b"second\n");
        repo.commit_all("start");
        let opened: BoardOpenResult = rpc(
            self,
            "board.open",
            &BoardOpenParams {
                root: fs::canonicalize(repo.work()).unwrap().display().to_string(),
                init_git: false,
            },
        )
        .unwrap();
        (opened.board.expect("the folder is a board").id, repo)
    }

    fn create_scratch(&self, board: &str, prompt: &str) -> String {
        let mut args = BTreeMap::new();
        args.insert("prompt".to_string(), json!(prompt));
        let answer: TaskApplyResult = rpc(
            self,
            "task.apply",
            &TaskApplyParams {
                board: board.to_string(),
                source: Some("scratch".to_string()),
                apply: ApplyParams {
                    actor: None,
                    op: "create".to_string(),
                    id: None,
                    args,
                    version: None,
                    checkout: None,
                },
                commit: false,
            },
        )
        .unwrap();
        match answer.result.expect("a scratch write is applied") {
            ApplyResult::Ok { task, .. } => task.key,
            other => panic!("create should work: {other:?}"),
        }
    }
}

#[test]
fn scratch_tasks_are_listed_for_their_own_board_only_and_a_write_shows_at_once() {
    let rig = rig();
    let first = rig.board_with(&[&FILES], "");
    let (second, _keep) = rig.second_board();
    rig.settled(&first);
    rig.settled(&second);

    let one = rig.create_scratch(&first, "on the first board");
    let two = rig.create_scratch(&first, "also on the first");
    let other = rig.create_scratch(&second, "on the second");

    // No wait between the write and the list.
    let on_first = rig.list(&first, json!({}));
    let on_second = rig.list(&second, json!({}));
    let mut expected = vec![one, two];
    expected.sort();
    assert_eq!(Rig::keys(&on_first), expected);
    assert_eq!(Rig::keys(&on_second), [other]);
    let got = rig.get(&first, &Rig::keys(&on_first)[0]);
    assert!(got.task.is_some());
}

// ---- sources that fail --------------------------------------------------------------------------------------------

#[test]
fn a_source_whose_provider_is_gone_is_stale_with_its_last_tasks_listed_and_a_restart_brings_it_back()
 {
    let mut rig = rig_with(|options| {
        options.supervisor = SupervisorPolicy {
            backoff: BackoffPolicy {
                initial: Duration::ZERO,
                max: Duration::ZERO,
                max_crashes: 3,
            },
            ..SupervisorPolicy::default()
        };
    });
    let board = rig.board_with(&[&FILES, &DOCS], "");
    let root = rig.root();
    rig.task(&root, "1.json", "1", "todo", "v1");
    let docs_data = |rig: &Rig| rig.data(&board, &DOCS);
    rig.settled(&board);
    // The sources of the two packs are told apart by their names.
    rig.list_until(&board, "both sources are listed", |list| {
        Rig::keys(list) == ["docs:1", "files:1"]
    });
    wait_until("the docs provider has started", DEADLINE, || {
        docs_data(&rig).join("started").exists()
    });

    // From now on its `list` ends the process, over and over, until it is given up on.
    fs::write(docs_data(&rig).join("die-on-list"), "").unwrap();
    let list = rig.list_until(&board, "docs is stale", |list| {
        !list.stale_sources.is_empty()
    });

    assert_eq!(list.stale_sources.len(), 1);
    assert_eq!(list.stale_sources[0].source, "docs");
    assert_eq!(
        Rig::keys(&list),
        ["docs:1", "files:1"],
        "its last tasks stay listed"
    );
    // The other source still answers.
    let written = rig.set_status(&board, "files", "files:1", "doing", None);
    assert!(matches!(written, ApplyResult::Ok { .. }));

    // A restart, with the fault gone, starts the provider again.
    fs::remove_file(docs_data(&rig).join("die-on-list")).unwrap();
    rig.restart();
    let list = rig.list_until(&board, "docs is fresh", |list| {
        !list.refreshing && list.stale_sources.is_empty()
    });
    assert_eq!(Rig::keys(&list), ["docs:1", "files:1"]);
}

#[test]
fn a_stale_notice_marks_the_source_stale_until_a_look_lists_it_again() {
    let rig = rig();
    let board = rig.board_with(&[&FILES], "");
    let root = rig.root();
    rig.task(&root, "1.json", "1", "todo", "v1");
    rig.settled(&board);
    rig.wait_watched(&board, &FILES);
    // Listing keeps failing, so the source stays stale for a test to see.
    fs::write(rig.data(&board, &FILES).join("fail-list"), "").unwrap();

    rig.notify(
        &board,
        &FILES,
        "source.stale",
        json!({ "reason": "the watcher lost a folder", "checkout": root.display().to_string() }),
    );
    let list = rig.list_until(&board, "the source is stale", |list| {
        !list.stale_sources.is_empty()
    });

    assert_eq!(list.stale_sources[0].source, "files");
    assert_eq!(Rig::keys(&list), ["files:1"], "its last tasks stay listed");
    fs::remove_file(rig.data(&board, &FILES).join("fail-list")).unwrap();
    rig.list_until(&board, "the source is listed again", |list| {
        list.stale_sources.is_empty()
    });
}

// ---- the cache ----------------------------------------------------------------------------------------------------

#[test]
fn the_cache_a_clean_stop_leaves_is_the_cache_that_was_there_while_the_daemon_ran() {
    let rig = rig();
    let board = rig.board_with(&[&FILES], "");
    let root = rig.root();
    rig.task(&root, "1.json", "1", "todo", "v1");
    rig.task(&root, "2.json", "2", "todo", "v1");
    rig.file(&root, "bad.json", "nope");
    rig.settled(&board);
    rig.wait_watched(&board, &FILES);
    // A write and a watched change, so the cache has been changed one task at a time as well as listed.
    rig.set_status(&board, "files", "files:1", "doing", None);
    rig.notify(
        &board,
        &FILES,
        "task.changed",
        changed("files:2", "v2", Some("todo"), Some("done"), &root),
    );
    rig.list_until(&board, "both changes are listed", |list| {
        list.tasks.iter().all(|task| task.state.status != "todo")
    });
    // A kill leaves what the last write left: read it through a handle of its own while the daemon runs.
    let running = rig.cache(&board);
    let store_dir = rig.f().daemon.paths().store_dir.clone();

    let Fixture { repo, daemon } = {
        let mut rig = rig;
        rig.fixture.take().unwrap()
    };
    drop(daemon);
    let stopped = Store::open(&store_dir)
        .unwrap()
        .corpus_cache(&board)
        .unwrap();

    assert_eq!(running.tasks.len(), 2);
    assert_eq!(running, stopped);
    drop(repo);
}

#[test]
fn a_cache_row_that_does_not_read_is_dropped_with_a_log_line_and_the_start_goes_on() {
    let mut rig = rig();
    let board = rig.board_with(&[&FILES], "");
    let root = rig.root();
    rig.task(&root, "1.json", "1", "todo", "v1");
    rig.task(&root, "2.json", "2", "todo", "v1");
    rig.settled(&board);
    let store_dir = rig.f().daemon.paths().store_dir.clone();
    let fixture = rig.fixture.take().unwrap();
    let Fixture { repo, daemon } = fixture;
    drop(daemon);
    let db = Store::open(&store_dir).unwrap().db_path();
    let conn = rusqlite::Connection::open(db).unwrap();
    conn.execute(
        "UPDATE corpus_tasks SET json = '{not json' WHERE key = 'files:2'",
        [],
    )
    .unwrap();
    drop(conn);
    rig.fixture = Some(Fixture::restart_over(repo, {
        let configure = rig.configure.clone();
        move |options| configure(options)
    }));

    let list = rig.settled(&board);

    assert_eq!(
        Rig::keys(&list),
        ["files:1", "files:2"],
        "the source lists it again"
    );
    let log = rig.log.text();
    assert!(
        log.contains("the cached task files:2 of source files did not read, so it was dropped"),
        "{log}"
    );
}

// ---- trust, settings and checkouts --------------------------------------------------------------------------------

#[test]
fn a_line_that_is_not_trusted_starts_nothing_until_it_is_trusted() {
    let rig = rig();
    rig.pack(&FILES);
    rig.board_yaml(&["quay"], "");
    rig.f().commit();
    let board = rig.f().board();
    let root = rig.root();
    rig.task(&root, "1.json", "1", "todo", "v1");

    let list = rig.settled(&board);

    assert!(list.tasks.is_empty());
    // Several looks later the provider has still never been started.
    let until = std::time::Instant::now() + RECHECK * 8;
    while std::time::Instant::now() < until {
        assert!(
            rig.started(&board, &FILES).is_empty(),
            "an untrusted line's provider is never started"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(!rig.data(&board, &FILES).join("started").exists());

    rig.trust(&board);
    let list = rig.list_until(&board, "the pack is listed once it is trusted", |list| {
        !list.tasks.is_empty()
    });
    assert_eq!(Rig::keys(&list), ["files:1"]);
}

#[test]
fn a_source_that_never_started_is_named_with_its_reason_until_it_starts() {
    let rig = rig();
    rig.pack(&FILES);
    rig.board_yaml(&["quay"], "");
    rig.f().commit();
    let board = rig.f().board();
    let root = rig.root();
    rig.task(&root, "1.json", "1", "todo", "v1");

    let list = rig.settled(&board);

    assert!(list.tasks.is_empty());
    // This rig has no built bundle, so the built-in plan source does not start either, and says why.
    let names: Vec<&str> = list.not_started.iter().map(|s| s.source.as_str()).collect();
    assert_eq!(names, ["files", "plan"], "{:?}", list.not_started);
    assert!(
        list.not_started[0].reason.contains("not trusted"),
        "{:?}",
        list.not_started
    );
    assert!(
        list.not_started[1].reason.contains("not in the bundle"),
        "{:?}",
        list.not_started
    );
    assert_eq!(list.sources_started, 0);
    assert!(list.stale_sources.is_empty(), "{:?}", list.stale_sources);

    rig.trust(&board);
    let list = rig.list_until(&board, "the source starts once it is trusted", |list| {
        !list.tasks.is_empty()
    });
    let names: Vec<&str> = list.not_started.iter().map(|s| s.source.as_str()).collect();
    assert_eq!(names, ["plan"], "{:?}", list.not_started);
    assert_eq!(list.sources_started, 1);
}

#[test]
fn a_retrust_lists_the_source_at_once_and_the_first_listing_after_it_is_not_stale() {
    // A look every 30 s: after a failed start the next try waits 60 s. Only a trust that wakes the corpus and ends
    // that wait lists the source inside the deadline.
    let mut rig = rig_with(|options| options.corpus_recheck = Duration::from_secs(30));
    let board = rig.board_with(&[&FILES], "");
    let root = rig.root();
    rig.task(&root, "1.json", "1", "todo", "v1");
    rig.list_until(&board, "the source lists", |list| !list.tasks.is_empty());

    // A committed change to what runs makes the line untrusted; the restart cannot start the source again.
    rig.f().write(".collide/packs/quay/README", "changed\n");
    rig.f().commit();
    rig.restart();
    let list = rig.list_until(&board, "the source is stale", |list| {
        !list.refreshing && !list.stale_sources.is_empty()
    });
    assert_eq!(list.stale_sources[0].source, "files");
    assert!(
        list.stale_sources[0].reason.contains("not trusted"),
        "{:?}",
        list.stale_sources
    );

    rig.trust(&board);
    let list = rig.list_until(
        &board,
        "the first listing after the trust is fresh",
        |list| !list.refreshing && list.stale_sources.is_empty(),
    );
    assert_eq!(Rig::keys(&list), ["files:1"]);
}

#[test]
fn a_source_the_line_no_longer_names_is_not_listed_as_unstarted() {
    let rig = rig();
    rig.pack(&FILES);
    rig.board_yaml(&["quay"], "");
    rig.f().commit();
    let board = rig.f().board();
    let list = rig.list_until(&board, "files is named", |list| {
        !list.refreshing && list.not_started.iter().any(|s| s.source == "files")
    });
    assert_eq!(list.not_started[0].source, "files");

    rig.f().write(".collide/board.yaml", "packs: []\n");
    rig.f().commit();

    rig.list_until(&board, "files is no longer named", |list| {
        !list.not_started.iter().any(|s| s.source == "files")
    });
}

#[test]
fn a_source_that_failed_after_it_declared_is_stale_and_not_in_the_list_of_those_that_did_not_start()
{
    let rig = rig();
    let board = rig.board_with(&[&FILES], "");
    let root = rig.root();
    rig.task(&root, "1.json", "1", "todo", "v1");
    rig.list_until(&board, "the source lists", |list| !list.tasks.is_empty());

    // A committed change to what runs makes the line untrusted: the next look cannot start the source again.
    rig.f().write(".collide/packs/quay/README", "changed\n");
    rig.f().commit();

    let list = rig.list_until(&board, "the source is stale", |list| {
        !list.stale_sources.is_empty()
    });
    assert_eq!(list.stale_sources[0].source, "files");
    let names: Vec<&str> = list.not_started.iter().map(|s| s.source.as_str()).collect();
    assert_eq!(
        names,
        ["plan"],
        "files is stale, not unstarted: {:?}",
        list.not_started
    );
    assert_eq!(
        Rig::keys(&list),
        ["files:1"],
        "it keeps what it last listed"
    );
}

#[test]
fn a_line_that_does_not_load_says_why_in_the_listing_until_it_loads_again() {
    let rig = rig();
    let board = rig.board_with(&[&FILES], "");
    let root = rig.root();
    rig.task(&root, "1.json", "1", "todo", "v1");
    let list = rig.list_until(&board, "the source lists", |list| !list.tasks.is_empty());
    assert_eq!(list.sources_unread, None);

    rig.f().write(".collide/board.yaml", "packs: [\n");
    rig.f().commit();

    let list = rig.list_until(&board, "the line is named as unread", |list| {
        list.sources_unread.is_some()
    });
    let reason = list.sources_unread.clone().unwrap();
    assert!(reason.contains("line does not load"), "{reason}");
    assert_eq!(
        Rig::keys(&list),
        ["files:1"],
        "it keeps what it last listed"
    );

    rig.board_yaml(&["quay"], "");
    rig.f().commit();

    rig.list_until(&board, "the line reads again and says nothing", |list| {
        list.sources_unread.is_none()
    });
}

#[test]
fn a_line_that_does_not_load_at_the_start_says_why_before_any_look() {
    // A look every 30 s: only the start's own reading of the line can name the reason inside the deadline.
    let mut rig = rig_with(|options| options.corpus_recheck = Duration::from_secs(30));
    let board = rig.board_with(&[&FILES], "");
    rig.f().write(".collide/board.yaml", "packs: [\n");
    rig.f().commit();
    rig.restart();

    let list = rig.settled(&board);

    let reason = list
        .sources_unread
        .expect("the start names the line as unread");
    assert!(reason.contains("line does not load"), "{reason}");
}

/// A recheck short enough that a few of the long waits of a board whose folder is gone fit in a test.
const QUICK: Duration = Duration::from_millis(10);

fn log_lines_with(rig: &Rig, text: &str) -> usize {
    rig.log
        .text()
        .lines()
        .filter(|line| line.contains(text))
        .count()
}

/// What `board.list` says of `board` is unread, if anything.
fn unread_in_board_list(rig: &Rig, board: &str) -> Option<String> {
    let listed: BoardListResult = rpc(rig, "board.list", &BoardListParams {}).unwrap();
    listed
        .boards
        .into_iter()
        .find(|summary| summary.id == board)
        .expect("the board is listed")
        .sources_unread
}

#[test]
fn a_board_whose_folder_is_gone_is_logged_once_over_many_looks_and_again_when_it_is_read_again() {
    let rig = rig_with(|options| options.corpus_recheck = QUICK);
    let board = rig.board_with(&[&FILES], "");
    let root = rig.root();
    rig.task(&root, "1.json", "1", "todo", "v1");
    rig.settled(&board);
    let aside = rig.f().home().join("aside");

    fs::rename(&root, &aside).unwrap();

    wait_until("the corpus says its sources are not read", DEADLINE, || {
        log_lines_with(&rig, "its sources are not read") > 0
    });
    let said = Instant::now();
    wait_until("many of the long waits have passed", DEADLINE, || {
        said.elapsed() >= QUICK * GONE_PERIODS * 3
    });
    assert_eq!(
        log_lines_with(&rig, "its sources are not read"),
        1,
        "{}",
        rig.log.text()
    );

    fs::rename(&aside, &root).unwrap();

    wait_until("the corpus says it reads the board again", DEADLINE, || {
        log_lines_with(&rig, "its sources are read again") > 0
    });
    assert_eq!(log_lines_with(&rig, "its sources are read again"), 1);
    assert_eq!(
        log_lines_with(&rig, "its sources are not read"),
        1,
        "{}",
        rig.log.text()
    );
}

#[test]
fn board_list_keeps_a_board_whose_folder_is_gone_with_its_reason_until_it_is_read_again() {
    let rig = rig_with(|options| options.corpus_recheck = QUICK);
    let board = rig.board_with(&[&FILES], "");
    let root = rig.root();
    rig.settled(&board);
    assert_eq!(unread_in_board_list(&rig, &board), None);
    let aside = rig.f().home().join("aside");

    fs::rename(&root, &aside).unwrap();

    wait_until("board.list gives the reason", DEADLINE, || {
        unread_in_board_list(&rig, &board).is_some_and(|reason| reason.contains("is not a folder"))
    });

    fs::rename(&aside, &root).unwrap();

    wait_until("board.list gives no reason", DEADLINE, || {
        unread_in_board_list(&rig, &board).is_none()
    });
}

#[test]
fn a_stop_with_a_listing_in_flight_logs_no_listing_failure() {
    // One task to a page and no `watch`: each recheck lists again, and a list that ends while the daemon stops
    // finds its next page after the stop.
    let docs = Src { page: 1, ..DOCS };
    let mut rig = rig();
    let board = rig.board_with(&[&docs], "");
    let root = rig.root();
    rig.task(&root, "1.json", "1", "todo", "v1");
    rig.task(&root, "2.json", "2", "todo", "v1");
    rig.settled(&board);
    let hold = rig.data(&board, &docs).join("hold-list");
    fs::write(&hold, "").unwrap();
    wait_until("a list is in flight", DEADLINE, || {
        rig.data(&board, &docs).join("holding").exists()
    });
    let Fixture { repo, daemon } = rig.fixture.take().unwrap();

    let stopping = std::thread::spawn(move || daemon.stop());
    // The daemon no longer accepts once it is stopping: the list is held until then.
    wait_until("the daemon is stopping", DEADLINE, || {
        rig.log.text().contains("daemon stopped")
    });
    fs::remove_file(&hold).unwrap();
    stopping.join().unwrap();

    let log = rig.log.text();
    assert!(!log.contains("could not be listed"), "{log}");
    drop(repo);
}

#[test]
fn a_config_set_of_a_packs_key_stops_that_packs_providers_on_every_board_and_no_other_pack() {
    let rig = rig();
    let quay = Src {
        config: GREETING,
        ..FILES
    };
    let other = Src {
        pack: "quay2",
        source: "more",
        config: GREETING,
        ..FILES
    };
    let first = rig.board_with(&[&quay, &other], "");
    let root = rig.root();
    rig.task(&root, "1.json", "1", "todo", "v1");
    // The second board has the same pack, with a repository of its own.
    let (second, repo) = rig.second_board();
    repo.write(
        ".collide/packs/quay/pack.yaml",
        fs::read(rig.root().join(".collide/packs/quay/pack.yaml"))
            .unwrap()
            .as_slice(),
    );
    repo.write(
        ".collide/packs/quay/providers/files.mjs",
        fs::read(rig.root().join(".collide/packs/quay/providers/files.mjs"))
            .unwrap()
            .as_slice(),
    );
    repo.write(
        ".collide/board.yaml",
        b"packs:\n  - source: repo:.collide/packs/quay\n",
    );
    repo.commit_all("a pack");
    rig.trust(&second);
    rig.settled(&first);
    rig.settled(&second);
    wait_until("every provider has started", DEADLINE, || {
        !rig.started(&first, &quay).is_empty()
            && !rig.started(&first, &other).is_empty()
            && !rig.started(&second, &quay).is_empty()
    });
    let pid = |text: &str| text.parse::<u32>().unwrap();
    let old_first = pid(&rig.started(&first, &quay)[0]);
    let old_second = pid(&rig.started(&second, &quay)[0]);
    let kept = pid(&rig.started(&first, &other)[0]);
    assert_eq!(rig.init(&first, &quay)["config"]["greeting"], "hello");

    let _: Value = rpc(
        &rig,
        "config.set",
        &json!({ "key": "quay.greeting", "value": "bonjour", "layer": "user", "board": first }),
    )
    .unwrap();

    wait_until("the providers of the pack end", DEADLINE, || {
        !common::pid_alive(old_first) && !common::pid_alive(old_second)
    });
    assert!(
        common::pid_alive(kept),
        "another pack's provider keeps running"
    );
    // They start again, with the new settings, at the next look.
    wait_until("the providers start again", DEADLINE, || {
        rig.started(&first, &quay).len() == 2 && rig.started(&second, &quay).len() == 2
    });
    assert_eq!(rig.init(&first, &quay)["config"]["greeting"], "bonjour");
    // The user's value of a pack's setting is kept for the board it was set on: the other board's provider is
    // restarted all the same, since the daemon does not tell which boards read a user-scope key, and reads its own.
    assert_eq!(rig.init(&second, &quay)["config"]["greeting"], "hello");
    assert_eq!(rig.started(&first, &other).len(), 1);
}

#[test]
fn a_new_worktree_is_listed_and_watched_at_the_next_look_and_a_removed_one_leaves() {
    let rig = rig();
    let board = rig.board_with(&[&FILES], "");
    let root = rig.root();
    rig.task(&root, "1.json", "1", "todo", "v1");
    rig.settled(&board);
    rig.wait_watched(&board, &FILES);
    let before = rig.last_seq();

    let other = rig.worktree("wt-n", "feature/n", true);
    rig.task(&other, "9.json", "9", "todo", "v1");
    rig.list_until(&board, "the new checkout is listed", |list| {
        Rig::keys(list) == ["files:1", "files:9"]
    });
    wait_until("the new set is watched", DEADLINE, || {
        rig.calls(&board, &FILES)
            .iter()
            .any(|line| line.starts_with("watch ") && line.contains(other.to_str().unwrap()))
    });
    let events = rig.wait_events("files", before, 1);
    assert_eq!(
        described(&events),
        ["changed files:9 None->Some(\"todo\") worktree"]
    );

    rig.f()
        .repo
        .git(&["worktree", "remove", "--force", other.to_str().unwrap()]);
    rig.list_until(&board, "the removed checkout's tasks are gone", |list| {
        Rig::keys(list) == ["files:1"]
    });
    assert!(
        described(&rig.wait_events("files", before, 2)).contains(&"removed files:9".to_string())
    );
}

#[test]
fn a_source_without_watch_is_listed_again_at_each_look_and_a_change_is_recorded() {
    let rig = rig();
    let board = rig.board_with(&[&DOCS], "");
    let root = rig.root();
    rig.task(&root, "1.json", "1", "todo", "v1");
    rig.settled(&board);
    let before = rig.last_seq();

    rig.task(&root, "1.json", "1", "doing", "v2");
    let list = rig.list_until(&board, "the change is listed", |list| {
        list.tasks
            .first()
            .is_some_and(|task| task.state.status == "doing")
    });

    assert_eq!(Rig::keys(&list), ["docs:1"]);
    assert_eq!(
        described(&rig.wait_events("docs", before, 1)),
        ["changed docs:1 Some(\"todo\")->Some(\"doing\") worktree"]
    );
    assert!(
        !rig.calls(&board, &DOCS)
            .iter()
            .any(|line| line.starts_with("watch ")),
        "a source without the capability is never told to watch"
    );
}

// ---- feature links (ts-0133-kg) -----------------------------------------------------------------------------------

const PROPOSAL_ONE_FEATURE: &str = r#"{"short":"10","kind":"proposal","version":"v1",
    "features":[{"id":"F1","title":"First","section":"","summary":""}]}"#;
const PROPOSAL_TWO_FEATURES: &str = r#"{"short":"10","kind":"proposal","version":"v2",
    "features":[{"id":"F1","title":"First","section":"","summary":""},
                {"id":"F2","title":"Second","section":"","summary":""}]}"#;
const COVERING_TASK: &str = r#"{"short":"11","links":[
    {"type":"covers","to":"10#F1"},{"type":"covers","to":"10#F2"}]}"#;

impl Rig {
    /// `(feature, missing)` of each link of `id` in `dir` ("out" or "in"), in the order `task.get` gave them.
    fn feature_links(&self, board: &str, id: &str, dir: &str) -> Vec<(String, bool)> {
        let wanted = if dir == "out" {
            LinkDir::Out
        } else {
            LinkDir::In
        };
        self.get(board, id)
            .links
            .iter()
            .filter(|link| link.dir == wanted)
            .filter_map(|link| Some((link.feature.clone()?, link.missing)))
            .collect()
    }
}

#[test]
fn feature_links_and_link_types_come_through_task_get_and_the_cache_serves_them_after_a_restart() {
    let mut rig = rig();
    let board = rig.board_with(&[&FILES], "");
    let root = rig.root();
    rig.file(&root, "10.json", PROPOSAL_ONE_FEATURE);
    rig.file(&root, "11.json", COVERING_TASK);
    rig.list_until(&board, "both tasks are listed", |list| {
        !list.refreshing && Rig::keys(list).len() == 2
    });
    // The proposal has no F2, so the task's link is missing; the proposal sees the task, which is there, in both.
    let expected_out = [("F1".to_string(), false), ("F2".to_string(), true)];
    let expected_in = [("F1".to_string(), false), ("F2".to_string(), false)];
    assert_eq!(rig.feature_links(&board, "files:11", "out"), expected_out);
    assert_eq!(rig.feature_links(&board, "files:10", "in"), expected_in);
    let got = rig.get(&board, "files:11");
    let covers = got.link_types.iter().find(|decl| decl.name == "covers");
    assert_eq!(
        covers.and_then(|decl| decl.inverse.as_deref()),
        Some("covered by")
    );
    let held = got
        .links
        .iter()
        .find(|link| link.feature.as_deref() == Some("F1"));
    assert_eq!(held.map(|link| link.kind.as_str()), Some("proposal"));

    // The next start cannot list the source, so what it serves is what the cache held.
    fs::write(rig.data(&board, &FILES).join("fail-list"), "").unwrap();
    rig.restart();
    rig.list_until(&board, "the source is stale", |list| {
        !list.stale_sources.is_empty()
    });
    assert_eq!(rig.feature_links(&board, "files:11", "out"), expected_out);
    assert_eq!(rig.feature_links(&board, "files:10", "in"), expected_in);
    let cached = rig.get(&board, "files:11");
    let covers = cached.link_types.iter().find(|decl| decl.name == "covers");
    assert_eq!(
        covers.and_then(|decl| decl.inverse.as_deref()),
        Some("covered by")
    );
}

#[test]
fn a_feature_added_to_a_proposal_turns_a_missing_link_to_resolved_at_the_next_look() {
    let rig = rig();
    let board = rig.board_with(&[&DOCS], "");
    let root = rig.root();
    rig.file(&root, "10.json", PROPOSAL_ONE_FEATURE);
    rig.file(&root, "11.json", COVERING_TASK);
    rig.list_until(&board, "both tasks are listed", |list| {
        !list.refreshing && Rig::keys(list).len() == 2
    });
    assert_eq!(
        rig.feature_links(&board, "docs:11", "out"),
        [("F1".to_string(), false), ("F2".to_string(), true)]
    );

    rig.file(&root, "10.json", PROPOSAL_TWO_FEATURES);
    wait_until("the link resolves", DEADLINE, || {
        rig.feature_links(&board, "docs:11", "out")
            == [("F1".to_string(), false), ("F2".to_string(), false)]
    });
}

#[test]
fn listing_sixteen_hundred_tasks_through_the_daemon_is_timed_and_complete() {
    let rig = rig();
    let paged = Src { page: 100, ..FILES };
    let board = rig.board_with(&[&paged], "");
    let root = rig.root();
    for dir in 0..16 {
        for n in 0..100 {
            let short = format!("{}", dir * 100 + n + 1);
            rig.task(
                &root,
                &format!("d{dir:02}/{short}.json"),
                &short,
                "todo",
                "v1",
            );
        }
    }
    let started = std::time::Instant::now();
    let list = rig.settled(&board);
    let listing = started.elapsed();
    let events = rig.source_events("files").len();

    eprintln!("1600 tasks listed in {listing:?}, with {events} startup events");
    assert_eq!(list.tasks.len(), 1600);
    assert_eq!(events, 1600);
}

// ---- views (ts-0119-tj; 0.10, Views) ------------------------------------------------------------------------------

/// A source whose pack has a view of its own. The pack's name is `quay`, so the view is `quay/open`.
const WITH_VIEW: Src = Src {
    config: "views:\n  open:\n    name: open\n    where: \"status == 'todo'\"\n",
    ..FILES
};

const TODO_VIEW: &str = "name: todo\nwhere: \"status == 'todo'\"\n";

impl Rig {
    fn views(&self, board: &str) -> Vec<ViewEntry> {
        let params = TaskViewsParams {
            board: board.to_string(),
        };
        let result: TaskViewsResult = rpc(self, "task.views", &params).unwrap();
        result.views
    }

    /// `task.views` until `done` holds of it: the counts are worked out once the corpus is.
    fn views_until(
        &self,
        board: &str,
        what: &str,
        done: impl Fn(&[ViewEntry]) -> bool,
    ) -> Vec<ViewEntry> {
        let mut last = Vec::new();
        wait_until(what, DEADLINE, || {
            last = self.views(board);
            done(&last)
        });
        last
    }

    /// The hash `board.trust_review` shows for the line `main`.
    fn hash(&self, board: &str) -> (String, bool) {
        let review: BoardTrustReviewResult = rpc(
            self,
            "board.trust_review",
            &BoardTrustReviewParams {
                board: board.to_string(),
                line: "main".to_string(),
            },
        )
        .unwrap();
        match review.trust {
            TrustState::Trusted { hash } => (hash, true),
            TrustState::Untrusted { hash } => (hash, false),
        }
    }

    /// A task file with any fields, such as `labels`, `links` and `kind`.
    fn task_json(&self, checkout: &Path, rel: &str, data: Value) {
        self.file(checkout, rel, &format!("{data}\n"));
    }

    fn view_list(
        &self,
        board: &str,
        view: Value,
        extra: Value,
    ) -> Result<TaskListResult, RpcError> {
        let mut params = json!({ "board": board, "view": view });
        for (key, value) in extra.as_object().expect("params are an object") {
            params[key] = value.clone();
        }
        rpc(self, "task.list", &params)
    }

    fn named(&self, board: &str, name: &str, extra: Value) -> Result<TaskListResult, RpcError> {
        self.view_list(board, json!({ "type": "named", "name": name }), extra)
    }

    fn save_view(
        &self,
        board: &str,
        view: Value,
        target: &str,
        overwrite: bool,
    ) -> Result<TaskSaveViewResult, RpcError> {
        let params =
            json!({ "board": board, "view": view, "target": target, "overwrite": overwrite });
        rpc(self, "task.save_view", &params)
    }

    /// A file of the user's board folder, as a person's personal view is.
    fn personal(&self, board: &str, file: &str, text: &str) {
        self.f()
            .write_user(&format!("boards/{board}/views/{file}"), text);
    }

    fn personal_dir(&self, board: &str) -> PathBuf {
        self.f()
            .home()
            .join(".config/collide/boards")
            .join(board)
            .join("views")
    }

    fn head(&self) -> String {
        self.f().repo.git(&["rev-parse", "HEAD"])
    }
}

fn entry<'a>(views: &'a [ViewEntry], name: &str) -> &'a ViewEntry {
    let found: Vec<&ViewEntry> = views.iter().filter(|view| view.view.name == name).collect();
    assert_eq!(found.len(), 1, "one view called {name}: {views:?}");
    found[0]
}

fn entry_from<'a>(views: &'a [ViewEntry], name: &str, source: &ViewSource) -> &'a ViewEntry {
    views
        .iter()
        .find(|view| view.view.name == name && &view.source == source)
        .unwrap_or_else(|| panic!("no view {name} from {source:?}: {views:?}"))
}

fn error_of(error: &Option<String>) -> &str {
    error.as_deref().expect("the view has an error")
}

#[test]
fn a_committed_view_is_used_and_an_edit_in_the_working_tree_is_not() {
    let rig = rig();
    rig.f().write(
        ".collide/views/mine.yaml",
        &TODO_VIEW.replace("name: todo", "name: mine"),
    );
    let board = rig.board_with(&[&FILES], "");
    let root = rig.root();
    rig.task(&root, "1.json", "1", "todo", "v1");
    rig.task(&root, "2.json", "2", "doing", "v1");
    rig.settled(&board);

    let views = rig.views_until(&board, "the view counts its task", |views| {
        views
            .iter()
            .any(|view| view.view.name == "mine" && view.task_count == 1)
    });
    let mine = entry(&views, "mine");
    assert_eq!(mine.source, ViewSource::Repo {});
    assert_eq!(mine.error, None);

    // An edit that is not committed, and a file that is not, are not the line's.
    rig.f().write(
        ".collide/views/mine.yaml",
        "name: mine\nwhere: \"status == 'doing'\"\n",
    );
    rig.f().write(
        ".collide/views/extra.yaml",
        "name: extra\ninclude: [files:1]\n",
    );
    let views = rig.views(&board);
    assert_eq!(
        entry(&views, "mine").view.where_.as_deref(),
        Some("status == 'todo'")
    );
    assert!(
        views.iter().all(|view| view.view.name != "extra"),
        "{views:?}"
    );

    // Committed, and trusted again because its hash changed, the same files are the line's.
    rig.f().commit();
    rig.trust(&board);
    let views = rig.views_until(&board, "the committed view is used", |views| {
        views.iter().any(|view| view.view.name == "extra")
    });
    let mine = entry(&views, "mine");
    assert_eq!(mine.view.where_.as_deref(), Some("status == 'doing'"));
    assert_eq!(mine.error, None);
    assert_eq!(entry(&views, "extra").task_count, 1);
}

#[test]
fn a_view_on_another_branch_is_not_used() {
    let rig = rig();
    rig.f().write(
        ".collide/views/here.yaml",
        "name: here\ninclude: [files:1]\n",
    );
    let board = rig.board_with(&[&FILES], "");
    rig.settled(&board);
    rig.f().repo.git(&["checkout", "--quiet", "-b", "other"]);
    rig.f().write(
        ".collide/views/elsewhere.yaml",
        "name: elsewhere\ninclude: [files:1]\n",
    );
    rig.f().commit();
    rig.f().repo.git(&["checkout", "--quiet", "main"]);

    let views = rig.views(&board);

    assert_eq!(entry(&views, "here").error, None);
    assert!(
        views.iter().all(|view| view.view.name != "elsewhere"),
        "{views:?}"
    );
}

#[test]
fn a_changed_view_blob_changes_the_trust_hash_and_an_untrusted_line_reads_no_view() {
    let rig = rig();
    rig.f().write(
        ".collide/views/mine.yaml",
        &TODO_VIEW.replace("name: todo", "name: mine"),
    );
    let board = rig.board_with(&[&WITH_VIEW], "");
    let root = rig.root();
    rig.task(&root, "1.json", "1", "todo", "v1");
    rig.settled(&board);
    let (trusted_hash, trusted) = rig.hash(&board);
    assert!(trusted);
    let views = rig.views(&board);
    assert_eq!(entry(&views, "mine").error, None);
    assert_eq!(entry(&views, "quay/open").error, None);

    rig.f().write(
        ".collide/views/mine.yaml",
        "name: mine\nwhere: \"status == 'doing'\"\n",
    );
    rig.f().commit();

    let (changed_hash, trusted) = rig.hash(&board);
    assert_ne!(changed_hash, trusted_hash, "a view blob is in the hash");
    assert!(!trusted);
    let views = rig.views(&board);
    for name in ["mine", "quay/open"] {
        let view = entry(&views, name);
        assert!(error_of(&view.error).contains("not trusted"), "{view:?}");
        assert_eq!(view.task_count, 0);
        assert_eq!(view.view.where_, None, "{name} is listed and not read");
    }
    let listed = rig.named(&board, "mine", json!({})).unwrap();
    assert!(listed.tasks.is_empty());
    assert!(error_of(&listed.view.expect("a report").error).contains("not trusted"));

    rig.trust(&board);
    let views = rig.views(&board);
    assert_eq!(entry(&views, "mine").error, None);
    assert_eq!(
        entry(&views, "mine").view.where_.as_deref(),
        Some("status == 'doing'")
    );
}

#[test]
fn four_broken_views_show_with_their_errors_next_to_a_good_one() {
    let rig = rig_with(|options| {
        options.view_before_parse = Some(Arc::new(|text: &str| {
            assert!(!text.contains("boom"), "the parse of a view panicked");
        }));
    });
    rig.f()
        .write(".collide/views/broken.yaml", "name: [not closed\n");
    rig.f().write(
        ".collide/views/long.yaml",
        &format!("name: long\nwhere: \"{}\"\n", "x".repeat(2000)),
    );
    rig.f().write(
        ".collide/views/corpus.yaml",
        "name: corpus\nwhere: \"tasks.size() > 0\"\n",
    );
    rig.f().write(
        ".collide/views/panic.yaml",
        "name: panic\nwhere: \"status == 'boom'\"\n",
    );
    rig.f().write(
        ".collide/views/good.yaml",
        &TODO_VIEW.replace("name: todo", "name: good"),
    );
    let board = rig.board_with(&[&FILES], "");
    let root = rig.root();
    rig.task(&root, "1.json", "1", "todo", "v1");
    rig.settled(&board);

    let views = rig.views_until(&board, "the good view counts", |views| {
        views
            .iter()
            .any(|view| view.view.name == "good" && view.task_count == 1)
    });

    assert_eq!(entry(&views, "good").error, None);
    for (name, word) in [
        ("broken", "broken.yaml"),
        ("long", "1024"),
        ("corpus", "tasks"),
        ("panic", "panic"),
    ] {
        let view = entry(&views, name);
        assert!(error_of(&view.error).contains(word), "{name}: {view:?}");
        assert_eq!(view.task_count, 0, "{name} has no members");
    }
    let listed = rig.named(&board, "long", json!({})).unwrap();
    assert!(listed.tasks.is_empty());
    assert!(listed.view.expect("a report").error.is_some());
}

#[test]
fn the_personal_view_wins_over_a_repo_view_of_the_same_name() {
    let rig = rig();
    rig.f().write(
        ".collide/views/shared.yaml",
        &TODO_VIEW.replace("name: todo", "name: shared"),
    );
    let board = rig.board_with(&[&FILES], "");
    let root = rig.root();
    rig.task(&root, "1.json", "1", "todo", "v1");
    rig.task(&root, "2.json", "2", "doing", "v1");
    rig.personal(
        &board,
        "shared.yaml",
        "name: shared\nwhere: \"status == 'doing'\"\n",
    );
    rig.settled(&board);

    let views = rig.views_until(&board, "the personal view counts", |views| {
        views
            .iter()
            .any(|view| view.source == ViewSource::Personal {} && view.task_count == 1)
    });

    let personal = entry_from(&views, "shared", &ViewSource::Personal {});
    assert_eq!(personal.error, None);
    assert_eq!(personal.view.where_.as_deref(), Some("status == 'doing'"));
    let repo = entry_from(&views, "shared", &ViewSource::Repo {});
    assert!(error_of(&repo.error).contains("personal"), "{repo:?}");
    assert_eq!(repo.task_count, 0);
    let listed = rig.named(&board, "shared", json!({})).unwrap();
    assert_eq!(Rig::keys(&listed), ["files:2"]);
}

#[test]
fn a_packs_view_is_listed_as_pack_slash_name_and_listed_by_it() {
    let rig = rig();
    let board = rig.board_with(&[&WITH_VIEW], "");
    let root = rig.root();
    rig.task(&root, "1.json", "1", "todo", "v1");
    rig.settled(&board);

    let views = rig.views_until(&board, "the pack's view counts", |views| {
        views
            .iter()
            .any(|view| view.view.name == "quay/open" && view.task_count == 1)
    });

    let open = entry(&views, "quay/open");
    assert_eq!(
        open.source,
        ViewSource::Pack {
            pack: "quay".into()
        }
    );
    assert_eq!(open.error, None);
    let listed = rig.named(&board, "quay/open", json!({})).unwrap();
    assert_eq!(Rig::keys(&listed), ["files:1"]);
}

#[test]
fn a_bad_personal_view_shows_its_error_and_the_others_list() {
    let rig = rig();
    let board = rig.board_with(&[&FILES], "");
    rig.personal(&board, "broken.yaml", "name: [not closed\n");
    rig.personal(&board, "fine.yaml", "name: fine\ninclude: [files:1]\n");
    rig.settled(&board);

    let views = rig.views(&board);

    let broken = entry(&views, "broken");
    assert_eq!(broken.source, ViewSource::Personal {});
    assert!(
        error_of(&broken.error).contains("broken.yaml"),
        "{broken:?}"
    );
    assert_eq!(entry(&views, "fine").error, None);
}

#[test]
fn task_list_by_name_gives_members_in_corpus_order_with_the_report_and_the_invalid_items() {
    let rig = rig();
    rig.f().write(".collide/views/todo.yaml", TODO_VIEW);
    rig.f().write(
        ".collide/views/typo.yaml",
        "name: typo\ninclude: [files:99]\n",
    );
    let board = rig.board_with(&[&FILES], "");
    let root = rig.root();
    for (short, status) in [("3", "todo"), ("1", "todo"), ("2", "doing"), ("4", "done")] {
        rig.task(&root, &format!("{short}.json"), short, status, "v1");
    }
    rig.file(&root, "bad.json", "{ not json");
    rig.settled(&board);

    let listed = rig.named(&board, "todo", json!({})).unwrap();

    assert_eq!(Rig::keys(&listed), ["files:1", "files:3"]);
    let report = listed.view.expect("a view brings its report");
    assert_eq!(report.name, "todo");
    assert_eq!(report.error, None);
    assert_eq!(report.members, 2);
    assert_eq!(report.invalid_items, 1);
    assert_eq!(listed.invalid.len(), 1);
    let typo = rig.named(&board, "typo", json!({})).unwrap();
    assert_eq!(typo.view.expect("a report").unmatched_include, ["files:99"]);
    assert!(typo.tasks.is_empty());
    let unknown = rig.named(&board, "nothing-like-it", json!({})).unwrap_err();
    assert_eq!(unknown.code, -32010);
}

#[test]
fn the_ready_kind_and_checkout_filters_apply_on_top_of_a_view() {
    let rig = rig();
    rig.f().write(
        ".collide/views/open.yaml",
        "name: open\nwhere: \"status != 'done'\"\n",
    );
    let board = rig.board_with(&[&FILES], "");
    let root = rig.root();
    rig.task_json(
        &root,
        "1.json",
        json!({ "short": "1", "status": "todo", "kind": "bug" }),
    );
    rig.task_json(
        &root,
        "2.json",
        json!({ "short": "2", "status": "todo", "kind": "story" }),
    );
    rig.task_json(
        &root,
        "3.json",
        json!({ "short": "3", "status": "doing", "kind": "bug" }),
    );
    rig.task_json(
        &root,
        "4.json",
        json!({ "short": "4", "status": "todo", "kind": "bug" }),
    );
    rig.task_json(&root, "5.json", json!({ "short": "5", "status": "done" }));
    rig.settled(&board);
    let keys = |extra: Value| Rig::keys(&rig.named(&board, "open", extra).unwrap());

    assert_eq!(
        keys(json!({})),
        ["files:1", "files:2", "files:3", "files:4"]
    );
    assert_eq!(
        keys(json!({ "ready": true })),
        ["files:1", "files:2", "files:4"]
    );
    assert_eq!(
        keys(json!({ "kind": "bug" })),
        ["files:1", "files:3", "files:4"]
    );
    assert_eq!(
        keys(json!({ "ready": true, "kind": "bug" })),
        ["files:1", "files:4"]
    );
    let there = root.display().to_string();
    assert_eq!(keys(json!({ "checkout": there })).len(), 4);
    assert!(keys(json!({ "checkout": "/nowhere/at/all" })).is_empty());
    assert_eq!(keys(json!({ "limit": 2 })), ["files:1", "files:2"]);
}

#[test]
fn an_inline_view_evaluates_without_saving_and_a_bad_one_is_refused_with_the_reason() {
    let rig = rig_with(|options| {
        options.view_before_parse = Some(Arc::new(|text: &str| {
            assert!(!text.contains("boom"), "the parse of a view panicked");
        }));
    });
    let board = rig.board_with(&[&FILES], "");
    let root = rig.root();
    rig.task(&root, "1.json", "1", "todo", "v1");
    rig.task(&root, "2.json", "2", "doing", "v1");
    rig.settled(&board);
    let inline =
        |view: Value| rig.view_list(&board, json!({ "type": "inline", "view": view }), json!({}));

    let listed = inline(json!({ "name": "adhoc", "where": "status == 'doing'" })).unwrap();

    assert_eq!(Rig::keys(&listed), ["files:2"]);
    assert_eq!(listed.view.expect("a report").name, "adhoc");
    assert!(
        !root.join(".collide/views").exists(),
        "nothing was saved in the repo"
    );
    assert!(
        !rig.personal_dir(&board).exists(),
        "nothing was saved for the person"
    );
    for (view, word) in [
        (json!({ "name": "x", "where": "tasks.size() > 0" }), "tasks"),
        (json!({ "name": "x", "where": "status == 'boom'" }), "panic"),
        (json!({ "name": "x", "where": "   " }), "where"),
        (json!({ "name": "" }), "name"),
    ] {
        let refused = inline(view.clone()).unwrap_err();
        assert_eq!(refused.code, -32602, "{view}");
        assert!(
            refused.message.contains(word),
            "{view}: {}",
            refused.message
        );
    }
}

/// The `partProblems` of a spike with a problem at each of `lines`, as a provider reports them.
fn problems_at(lines: &[u32]) -> Value {
    let found: Vec<Value> = lines
        .iter()
        .map(
            |line| json!({ "part": "claims", "line": line, "reason": "a claim block is not read" }),
        )
        .collect();
    json!(found)
}

#[test]
fn a_view_over_spikes_with_part_problems_reports_their_count_in_task_list_and_task_views() {
    let rig = rig();
    rig.f().write(
        ".collide/views/spikes.yaml",
        "name: spikes\ninclude: [files:1, files:2, files:3]\n",
    );
    rig.f().write(
        ".collide/views/clean.yaml",
        "name: clean\ninclude: [files:3]\n",
    );
    let board = rig.board_with(&[&FILES], "");
    let root = rig.root();
    for (short, lines) in [
        ("1", vec![12, 17]),
        ("2", vec![9]),
        ("3", vec![]),
        ("4", vec![3, 4, 5]),
    ] {
        let mut data = json!({ "short": short, "kind": "spike", "status": "todo" });
        if !lines.is_empty() {
            data["partProblems"] = problems_at(&lines);
        }
        rig.task_json(&root, &format!("{short}.json"), data);
    }
    rig.settled(&board);

    let inline = rig
        .view_list(
            &board,
            json!({ "type": "inline", "view": { "name": "adhoc", "include": ["files:1", "files:2", "files:3"] } }),
            json!({}),
        )
        .unwrap();
    let named = rig.named(&board, "clean", json!({})).unwrap();

    assert_eq!(inline.view.expect("a report").part_problems, 3);
    let clean = named.view.expect("a report");
    assert_eq!(clean.part_problems, 0);
    let written = serde_json::to_value(&clean).unwrap();
    assert!(written.get("part_problems").is_none(), "{written}");
    assert_eq!(
        inline.tasks[0].part_problems.len(),
        2,
        "the problems are on the task, as the provider gave them"
    );
    let views = rig.views_until(&board, "the counts are in", |views| {
        views
            .iter()
            .any(|view| view.view.name == "spikes" && view.problem_count == 3)
    });
    assert_eq!(entry(&views, "clean").problem_count, 0);
    assert_eq!(entry(&views, "spikes").task_count, 3);
    assert_eq!(entry(&views, "spikes").error_count, 0);
}

#[test]
fn the_count_of_a_view_is_the_length_of_its_list_and_follows_the_corpus() {
    let rig = rig();
    rig.f().write(".collide/views/todo.yaml", TODO_VIEW);
    rig.f()
        .write(".collide/views/all.yaml", "name: all\nwhere: \"true\"\n");
    let board = rig.board_with(&[&DOCS], "");
    let root = rig.root();
    rig.task(&root, "1.json", "1", "todo", "v1");
    rig.task(&root, "2.json", "2", "doing", "v1");
    rig.settled(&board);

    let views = rig.views_until(&board, "the counts are in", |views| {
        views
            .iter()
            .any(|view| view.view.name == "all" && view.task_count == 2)
    });
    for view in &views {
        let listed = rig.named(&board, &view.view.name, json!({})).unwrap();
        assert_eq!(listed.tasks.len() as u32, view.task_count, "{view:?}");
    }
    assert_eq!(entry(&views, "todo").task_count, 1);

    rig.task(&root, "3.json", "3", "todo", "v1");
    let views = rig.views_until(&board, "the new task is counted", |views| {
        views
            .iter()
            .any(|view| view.view.name == "todo" && view.task_count == 2)
    });
    assert_eq!(entry(&views, "all").task_count, 3);
}

#[test]
fn a_closure_view_over_the_fixture_corpus_has_the_numbers_of_the_pure_test() {
    let rig = rig();
    rig.f().write(
        ".collide/views/run-order.yaml",
        "name: run-order\nclosure: { goal: files:042, follow: depends_on }\n",
    );
    let board = rig.board_with(&[&FILES], "");
    let root = rig.root();
    let task = |short: &str, status: &str, blocked_by: &[&str]| {
        let links: Vec<Value> = blocked_by
            .iter()
            .map(|to| json!({ "type": "blocked-by", "to": to }))
            .collect();
        rig.task_json(
            &root,
            &format!("{short}.json"),
            json!({ "short": short, "status": status, "links": links }),
        );
    };
    task("042", "todo", &["040", "041"]);
    task("040", "doing", &[]);
    task("041", "done", &["044"]);
    task("043", "todo", &[]);
    task("044", "todo", &["045", "046"]);
    task("045", "todo", &[]);
    task("046", "todo", &[]);
    rig.settled(&board);

    let listed = rig.named(&board, "run-order", json!({})).unwrap();

    assert_eq!(Rig::keys(&listed), ["files:040", "files:042"]);
    let report = listed.view.expect("a report");
    assert_eq!(report.steps, [vec!["files:040"], vec!["files:042"]]);
    assert!(report.cycle.is_empty());
    let views = rig.views(&board);
    assert_eq!(entry(&views, "run-order").task_count, 2);
}

#[test]
fn a_line_that_does_not_load_refuses_the_views_and_a_named_list_and_serves_an_inline_one() {
    let rig = rig();
    let board = rig.board_with(&[&FILES], "");
    rig.settled(&board);
    rig.f()
        .write(".collide/board.yaml", "packs: [this is not a pack list\n");
    rig.f().commit();
    let params = TaskViewsParams {
        board: board.clone(),
    };

    let views: Result<TaskViewsResult, RpcError> = rpc(&rig, "task.views", &params);
    let named = rig.named(&board, "todo", json!({}));
    let inline = rig.view_list(
        &board,
        json!({ "type": "inline", "view": { "name": "adhoc", "where": "true" } }),
        json!({}),
    );

    assert_eq!(views.unwrap_err().code, -32011);
    assert_eq!(named.unwrap_err().code, -32011);
    assert!(inline.is_ok(), "an inline view needs no line");
}

// ---- save_view ----------------------------------------------------------------------------------------------------

fn saved() -> Value {
    json!({
        "name": "mine",
        "where": "status == 'todo'",
        "include": ["files:1"],
        "layout": { "default": "board", "sort": { "by": "title", "dir": "asc" } },
    })
}

#[test]
fn save_view_to_personal_writes_the_board_folder_and_the_file_reloads_to_the_same_view() {
    let rig = rig();
    let board = rig.board_with(&[&FILES], "");
    rig.settled(&board);
    let head = rig.head();

    let result = rig.save_view(&board, saved(), "personal", false).unwrap();

    let expected = rig.personal_dir(&board).join("mine.yaml");
    assert_eq!(PathBuf::from(&result.path), expected);
    let written: View = load_yaml_file(&expected).unwrap();
    assert_eq!(written, serde_json::from_value::<View>(saved()).unwrap());
    let views = rig.views(&board);
    assert_eq!(entry(&views, "mine").source, ViewSource::Personal {});
    assert_eq!(entry(&views, "mine").view, written);
    assert_eq!(rig.head(), head, "nothing is committed");
    assert_eq!(rig.f().repo.git(&["status", "--porcelain"]), "");
}

#[test]
fn save_view_to_the_repo_writes_a_file_that_is_not_committed_and_the_next_load_reads_it() {
    let rig = rig();
    let board = rig.board_with(&[&FILES], "");
    rig.settled(&board);
    let head = rig.head();

    let result = rig.save_view(&board, saved(), "repo", false).unwrap();

    let expected = rig.root().join(".collide/views/mine.yaml");
    assert_eq!(PathBuf::from(&result.path), expected);
    assert_eq!(rig.head(), head, "nothing is committed");
    assert!(
        rig.f()
            .repo
            .git(&["status", "--porcelain"])
            .contains(".collide/views")
    );
    assert!(
        rig.views(&board)
            .iter()
            .all(|view| view.view.name != "mine"),
        "not on the line yet"
    );
    let written: View = load_yaml_file(&expected).unwrap();

    rig.f().commit();
    rig.trust(&board);
    let views = rig.views(&board);
    assert_eq!(entry(&views, "mine").source, ViewSource::Repo {});
    assert_eq!(entry(&views, "mine").view, written);
    assert_eq!(written, serde_json::from_value::<View>(saved()).unwrap());
}

#[test]
fn save_view_to_the_repo_of_a_root_in_a_rebase_is_refused_with_the_reason_and_a_personal_save_still_works()
 {
    let rig = rig();
    let board = rig.board_with(&[&FILES], "");
    rig.settled(&board);
    let repo = &rig.f().repo;
    repo.write("line.txt", b"one\n");
    repo.commit_all("fixture");
    repo.git(&["checkout", "--quiet", "-b", "topic"]);
    repo.write("line.txt", b"topic\n");
    repo.commit_all("topic edits the line");
    repo.git(&["checkout", "--quiet", "main"]);
    repo.write("line.txt", b"main\n");
    repo.commit_all("main edits the line");
    repo.git(&["checkout", "--quiet", "topic"]);
    let rebase = git_command(repo.root().path(), repo.work())
        .args(["rebase", "main"])
        .output()
        .expect("git runs");
    assert!(
        !rebase.status.success(),
        "the rebase should stop on the conflict"
    );

    let refused = rig.save_view(&board, saved(), "repo", false).unwrap_err();

    assert_eq!(refused.code, -32011, "{}", refused.message);
    assert!(
        refused.message.contains("a rebase is in progress"),
        "names what is going on: {}",
        refused.message
    );
    assert!(
        refused.message.contains("personal view"),
        "says what the person can do: {}",
        refused.message
    );
    assert!(
        !rig.root().join(".collide/views").exists(),
        "nothing is written to the busy root"
    );
    let personal = rig.save_view(&board, saved(), "personal", false).unwrap();
    assert!(Path::new(&personal.path).is_file());
    git_command(repo.root().path(), repo.work())
        .args(["rebase", "--abort"])
        .output()
        .expect("git runs");
    rig.save_view(&board, saved(), "repo", false)
        .expect("the root is free again");
}

#[test]
fn save_view_to_the_repo_refuses_a_views_folder_that_is_a_link_and_writes_nothing_where_it_points()
{
    let rig = rig();
    let board = rig.board_with(&[&FILES], "");
    rig.settled(&board);
    let outside = rig.f().repo.root().path().join("outside");
    fs::create_dir_all(&outside).unwrap();
    fs::create_dir_all(rig.root().join(".collide")).unwrap();
    std::os::unix::fs::symlink(&outside, rig.root().join(".collide/views")).unwrap();

    let refused = rig.save_view(&board, saved(), "repo", true).unwrap_err();

    assert_eq!(refused.code, -32011, "{}", refused.message);
    assert!(
        refused.message.contains(".collide/views"),
        "{}",
        refused.message
    );
    assert!(fs::read_dir(&outside).unwrap().next().is_none());
}

/// Set in the environment of the child that plays a run's agent: the daemon root and the calls it makes, as JSON.
const AGENT_CALLS: &str = "COLLIDE_TEST_AGENT_CALLS";
const AGENT_REPLY: &str = "AGENT-REPLY ";

/// Not a test on its own: it is the client that [`as_a_run_agent`] starts in a child process of this test binary, whose
/// environment carries the run marker, so the daemon sees a run's agent at the other end of the socket. Run on its own
/// (no `AGENT_CALLS`) it does nothing. Each call is answered with a line holding the error code, or `ok`.
#[test]
fn agent_client_child() {
    let Ok(text) = std::env::var(AGENT_CALLS) else {
        return;
    };
    let spec: Value = serde_json::from_str(&text).unwrap();
    let paths = DaemonPaths::under(Path::new(spec["root"].as_str().unwrap()));
    let mut client = OwnerClient::connect(&paths, "agent-child").expect("the child connects");
    for call in spec["calls"].as_array().unwrap() {
        let answer = client.call::<_, Value>(
            call["method"].as_str().unwrap(),
            &call["params"],
            Some(Duration::from_secs(30)),
        );
        let reply = match answer {
            Ok(_) => json!("ok"),
            Err(ClientError::Rpc(error)) => json!(error.code),
            Err(other) => json!(format!("{other}")),
        };
        println!("{AGENT_REPLY}{reply}");
    }
}

/// Makes `calls` (`(method, params)`) from a process whose environment has the run marker, as a run's agent has, and
/// returns what each came back as: the error code, or `"ok"`.
fn as_a_run_agent(rig: &Rig, run: &str, calls: &[(&str, Value)]) -> Vec<Value> {
    let root = rig.f().home();
    let spec = json!({
        "root": root,
        "calls": calls.iter().map(|(method, params)| json!({"method": method, "params": params})).collect::<Vec<_>>(),
    });
    let output = Command::new(std::env::current_exe().unwrap())
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
        .output()
        .expect("the child runs");
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

#[test]
fn a_run_agent_may_not_save_a_view_and_nothing_is_written() {
    use collide_core::owner::code::NOT_PERMITTED;
    let rig = rig();
    let board = rig.board_with(&[&FILES], "");
    rig.settled(&board);
    let call = |target: &str| json!({ "board": board, "view": saved(), "target": target, "overwrite": true });

    let replies = as_a_run_agent(
        &rig,
        "run-1",
        &[
            ("task.save_view", call("personal")),
            ("task.save_view", call("repo")),
        ],
    );

    assert_eq!(
        replies,
        [json!(NOT_PERMITTED), json!(NOT_PERMITTED)],
        "{replies:?}"
    );
    assert!(!rig.personal_dir(&board).join("mine.yaml").exists());
    assert!(!rig.root().join(".collide/views").exists());
    // The refusal is about the caller: the person's own client saves with the same calls.
    rig.save_view(&board, saved(), "personal", true).unwrap();
    rig.save_view(&board, saved(), "repo", true).unwrap();
}

#[test]
fn save_view_refuses_an_invalid_view_an_unsafe_name_and_an_overwrite_with_no_flag() {
    let rig = rig();
    let board = rig.board_with(&[&FILES], "");
    rig.settled(&board);
    let folders = [
        ("repo", rig.root().join(".collide/views")),
        ("personal", rig.personal_dir(&board)),
    ];
    for (target, folder) in &folders {
        for view in [
            json!({ "name": "bad", "where": "   " }),
            json!({ "name": "bad", "where": "tasks.size() > 0" }),
            json!({ "name": "../evil" }),
            json!({ "name": "a/b" }),
            json!({ "name": "has space" }),
            json!({ "name": ".hidden" }),
            json!({ "name": "" }),
        ] {
            let refused = rig
                .save_view(&board, view.clone(), target, false)
                .unwrap_err();
            assert_eq!(refused.code, -32602, "{target} {view}: {}", refused.message);
        }
        assert!(
            !folder.exists() || fs::read_dir(folder).unwrap().next().is_none(),
            "{target}: a refused view writes nothing"
        );
        let first = rig.save_view(&board, saved(), target, false).unwrap();
        let before = fs::read_to_string(&first.path).unwrap();
        let mut changed = saved();
        changed["include"] = json!(["files:2"]);

        let again = rig
            .save_view(&board, changed.clone(), target, false)
            .unwrap_err();

        assert_eq!(again.code, -32011, "{target}: {}", again.message);
        assert_eq!(fs::read_to_string(&first.path).unwrap(), before);
        rig.save_view(&board, changed, target, true).unwrap();
        assert_ne!(fs::read_to_string(&first.path).unwrap(), before);
    }
}

// ---- a changed item brings its whole copy (ts-0157-hd) --------------------------------------------------------------

/// A `task.changed` whose `diff` names `diff`, for a task that stays in `status`.
fn changed_naming(key: &str, version: &str, status: &str, diff: Value, checkout: &Path) -> Value {
    json!({
        "key": key, "version": version, "diff": diff, "cause": "worktree",
        "from": state_json(status), "to": state_json(status),
        "checkout": checkout.display().to_string(),
    })
}

/// A task file with a title and `blocked-by` links to the short ids in `blocked_by`.
fn task_with(
    rig: &Rig,
    checkout: &Path,
    short: &str,
    title: &str,
    version: &str,
    blocked_by: &[&str],
) {
    let links: Vec<Value> = blocked_by
        .iter()
        .map(|to| json!({ "type": "blocked-by", "to": to }))
        .collect();
    let data = json!({
        "short": short, "title": title, "status": "todo", "version": version, "links": links,
    });
    rig.file(checkout, &format!("{short}.json"), &format!("{data}\n"));
}

fn gets(rig: &Rig, board: &str) -> usize {
    rig.calls(board, &FILES)
        .iter()
        .filter(|line| line.starts_with("get "))
        .count()
}

#[test]
fn a_title_changed_on_disk_lists_with_the_new_title_and_the_cache_keeps_it() {
    let rig = rig();
    let board = rig.board_with(&[&FILES], "");
    let root = rig.root();
    task_with(&rig, &root, "1", "The old title", "v1", &[]);
    rig.settled(&board);
    rig.wait_watched(&board, &FILES);

    task_with(&rig, &root, "1", "The new title", "v2", &[]);
    let diff = json!({ "title": { "from": "The old title", "to": "The new title" } });
    rig.notify(
        &board,
        &FILES,
        "task.changed",
        changed_naming("files:1", "v2", "todo", diff, &root),
    );
    let list = rig.list_until(&board, "the new title is listed", |list| {
        list.tasks
            .first()
            .is_some_and(|task| task.title == "The new title")
    });

    assert_eq!(list.tasks[0].version, "v2");
    assert_eq!(rig.cache(&board).tasks[0].task.title, "The new title");
    assert_eq!(gets(&rig, &board), 1, "the one task was fetched");
}

#[test]
fn a_blocked_by_link_added_on_disk_leaves_ready_and_one_removed_brings_the_task_back() {
    let rig = rig();
    let board = rig.board_with(&[&FILES], "");
    let root = rig.root();
    task_with(&rig, &root, "1", "First", "v1", &[]);
    task_with(&rig, &root, "2", "Second", "v1", &[]);
    rig.settled(&board);
    rig.wait_watched(&board, &FILES);
    let ready = |rig: &Rig| Rig::keys(&rig.list(&board, json!({ "ready": true })));
    assert_eq!(ready(&rig), ["files:1", "files:2"]);

    task_with(&rig, &root, "2", "Second", "v2", &["1"]);
    let added = json!({ "depends_on": { "added": ["1"], "removed": [] } });
    rig.notify(
        &board,
        &FILES,
        "task.changed",
        changed_naming("files:2", "v2", "todo", added, &root),
    );
    wait_until("the blocked task leaves ready", DEADLINE, || {
        ready(&rig) == ["files:1"]
    });

    task_with(&rig, &root, "2", "Second", "v3", &[]);
    let removed = json!({ "depends_on": { "added": [], "removed": ["1"] } });
    rig.notify(
        &board,
        &FILES,
        "task.changed",
        changed_naming("files:2", "v3", "todo", removed, &root),
    );
    wait_until("the task is ready again", DEADLINE, || {
        ready(&rig) == ["files:1", "files:2"]
    });
}

#[test]
fn a_change_of_the_status_alone_fetches_nothing() {
    let rig = rig();
    let board = rig.board_with(&[&FILES], "");
    let root = rig.root();
    rig.task(&root, "1.json", "1", "todo", "v1");
    rig.settled(&board);
    rig.wait_watched(&board, &FILES);

    rig.task(&root, "1.json", "1", "doing", "v2");
    let mut event = changed("files:1", "v2", Some("todo"), Some("doing"), &root);
    event["diff"] = json!({ "status": { "from": "todo", "to": "doing" } });
    rig.notify(&board, &FILES, "task.changed", event);
    rig.list_until(&board, "the status is listed", |list| {
        list.tasks
            .first()
            .is_some_and(|task| task.state.status == "doing")
    });

    assert_eq!(gets(&rig, &board), 0);
}

#[test]
fn a_change_whose_task_cannot_be_fetched_still_moves_the_state_and_the_version() {
    let rig = rig();
    let board = rig.board_with(&[&FILES], "");
    let root = rig.root();
    task_with(&rig, &root, "1", "The old title", "v1", &[]);
    rig.settled(&board);
    rig.wait_watched(&board, &FILES);
    fs::write(rig.data(&board, &FILES).join("fail-get"), "").unwrap();

    let mut event = changed("files:1", "v2", Some("todo"), Some("doing"), &root);
    event["diff"] = json!({
        "status": { "from": "todo", "to": "doing" },
        "title": { "from": "The old title", "to": "The new title" },
    });
    rig.notify(&board, &FILES, "task.changed", event);
    let list = rig.list_until(&board, "the status is listed", |list| {
        list.tasks
            .first()
            .is_some_and(|task| task.state.status == "doing")
    });

    assert_eq!(list.tasks[0].version, "v2");
    assert_eq!(
        list.tasks[0].title, "The old title",
        "the title waits for a listing"
    );
    assert_eq!(gets(&rig, &board), 1);
}

#[test]
fn a_changed_item_keeps_the_collections_that_collections_changed_gave_it() {
    let rig = rig();
    let board = rig.board_with(&[&FILES], "");
    let root = rig.root();
    task_with(&rig, &root, "1", "The old title", "v1", &[]);
    rig.settled(&board);
    rig.wait_watched(&board, &FILES);
    rig.notify(
        &board,
        &FILES,
        "collections.changed",
        json!({
            "collections": [],
            "members": [{ "key": "files:1", "collections": ["epic-a"] }],
            "checkout": root.display().to_string(),
        }),
    );
    rig.list_until(&board, "the task is in its collection", |list| {
        list.tasks
            .first()
            .is_some_and(|task| task.collections == ["epic-a"])
    });

    // The fetched task has no collections: the source says them in `collections.changed`, and said them.
    task_with(&rig, &root, "1", "The new title", "v2", &[]);
    let diff = json!({ "title": { "from": "The old title", "to": "The new title" } });
    rig.notify(
        &board,
        &FILES,
        "task.changed",
        changed_naming("files:1", "v2", "todo", diff, &root),
    );
    let list = rig.list_until(&board, "the new title is listed", |list| {
        list.tasks
            .first()
            .is_some_and(|task| task.title == "The new title")
    });

    assert_eq!(list.tasks[0].collections, ["epic-a"]);
    assert_eq!(rig.cache(&board).tasks[0].task.collections, ["epic-a"]);
}

// ---- what the Field needs from each source and copy (ts-0180-ad) --------------------------------------------------

/// A source that declares eight statuses, two of them in `review`, and two fields, one with a `label`. The pack's
/// `declare` setting is what the provider answers `initialize` with in their place (see the fixture's header).
const EIGHT: Src = Src {
    pack: "octo",
    source: "eight",
    watch: false,
    page: 100,
    config: concat!(
        "config:\n  declare:\n    type: string\n    default: '",
        r#"{"statuses":[{"name":"idea","category":"backlog"},{"name":"triage","category":"backlog"},"#,
        r#"{"name":"queued","category":"ready"},{"name":"doing","category":"active"},"#,
        r#"{"name":"in_review","category":"review"},{"name":"qa","category":"review"},"#,
        r#"{"name":"done","category":"done"},{"name":"dropped","category":"dropped"}],"#,
        r#""categoryStatus":{"ready":"queued","active":"doing","review":"in_review"},"#,
        r#""fields":[{"id":"story_points","type":"number","filterable":true,"label":"Points"},"#,
        r#"{"id":"epic","type":"string","filterable":true}]}"#,
        "'\n"
    ),
};

fn source_named<'a>(
    list: &'a TaskListResult,
    name: &str,
) -> &'a collide_core::owner::task::BoardSource {
    list.sources
        .iter()
        .find(|source| source.name == name)
        .unwrap_or_else(|| panic!("no source {name} in {:?}", list.sources))
}

#[test]
fn a_source_with_eight_statuses_is_listed_with_its_order_categories_fields_labels_and_pack() {
    use collide_core::item::Category;
    let rig = rig();
    let board = rig.board_with(&[&EIGHT], "");
    let root = rig.root();
    rig.task(&root, "1.json", "1", "qa", "v1");

    let list = rig.list_until(&board, "the source lists its task", |list| {
        !list.tasks.is_empty()
    });

    let source = source_named(&list, "eight");
    assert_eq!(source.pack.as_deref(), Some("octo"));
    assert_eq!(source.scope, collide_core::provider::Scope::Ref);
    let statuses: Vec<(&str, Category)> = source
        .statuses
        .iter()
        .map(|decl| (decl.name.as_str(), decl.category))
        .collect();
    assert_eq!(
        statuses,
        [
            ("idea", Category::Backlog),
            ("triage", Category::Backlog),
            ("queued", Category::Ready),
            ("doing", Category::Active),
            ("in_review", Category::Review),
            ("qa", Category::Review),
            ("done", Category::Done),
            ("dropped", Category::Dropped),
        ]
    );
    assert_eq!(source.category_status[&Category::Review], "in_review");
    assert_eq!(source.fields[0].label.as_deref(), Some("Points"));
    assert_eq!(source.fields[1].label, None);
    assert!(source.blocking_links, "the fixture declares `blocked-by`");
    assert_eq!(source.id_pattern, "^[a-z0-9-]+$");
    assert_eq!(source.short_id_pattern, "^[0-9]+$");
    assert_eq!(
        list.tasks[0].state.category,
        Category::Review,
        "the task has the declared category"
    );
}

#[test]
fn the_scratch_source_is_listed_like_any_other_and_a_source_that_did_not_start_is_left_out() {
    let rig = rig();
    rig.pack(&EIGHT);
    rig.board_yaml(&["octo"], "");
    rig.f().commit();
    let board = rig.f().board();

    let list = rig.settled(&board);

    // The line is not trusted, so its pack did not start: `not_started` says why, and `sources` has no word of it.
    assert!(
        list.not_started
            .iter()
            .any(|source| source.source == "eight")
    );
    let names: Vec<&str> = list
        .sources
        .iter()
        .map(|source| source.name.as_str())
        .collect();
    assert_eq!(names, ["scratch"], "{:?}", list.sources);
    let scratch = source_named(&list, "scratch");
    assert_eq!(scratch.pack, None);
    assert_eq!(scratch.scope, collide_core::provider::Scope::Global);
    assert!(scratch.statuses.len() >= 5, "{:?}", scratch.statuses);
    assert!(
        !scratch.blocking_links,
        "scratch tasks have no dependencies"
    );

    rig.trust(&board);
    let list = rig.list_until(&board, "the source starts once it is trusted", |list| {
        list.sources.iter().any(|source| source.name == "eight")
    });
    let names: Vec<&str> = list
        .sources
        .iter()
        .map(|source| source.name.as_str())
        .collect();
    assert_eq!(names, ["eight", "scratch"]);
}

#[test]
fn a_status_write_in_a_worktree_gives_a_divergence_entry_naming_its_status_and_checkout_and_no_run()
{
    use collide_core::item::Category;
    let rig = rig();
    let board = rig.board_with(&[&EIGHT], "");
    let root = rig.root();
    let other = rig.worktree("wt-a", "feature/a", true);
    rig.task(&root, "1.json", "1", "idea", "v1");
    rig.task(&other, "1.json", "1", "qa", "v2");
    rig.task(&root, "2.json", "2", "idea", "v1");
    rig.task(&other, "2.json", "2", "idea", "v1");

    let list = rig.list_until(&board, "the divergence shows", |list| {
        !list.divergence.is_empty()
    });

    assert_eq!(
        list.divergent,
        ["eight:1"],
        "the key list the CLI prints is as it was"
    );
    assert_eq!(list.divergence.len(), 1);
    let entry = &list.divergence[0];
    assert_eq!(entry.key, "eight:1");
    assert_eq!(entry.copies.len(), 1);
    let copy = &entry.copies[0];
    assert_eq!(copy.checkout, other.display().to_string());
    assert_eq!(
        (copy.status.as_str(), copy.category),
        ("qa", Category::Review)
    );
    assert_eq!(copy.run, None, "no run made this worktree");
}

#[test]
fn task_get_on_a_scratch_task_lists_only_the_views_that_hold_it() {
    let rig = rig();
    let board = rig.board_with(&[&FILES], "");
    let held = rig.create_scratch(&board, "held by a repo view");
    let plain = rig.create_scratch(&board, "held by none but ready");
    rig.f().write(
        ".collide/views/mine.yaml",
        &format!("name: mine\ninclude: [{held}]\n"),
    );
    rig.f().commit();
    rig.trust(&board);
    rig.list_until(&board, "both scratch tasks list", |list| {
        list.tasks.len() == 2
    });
    let get = |id: &str| -> TaskGetResult {
        rpc(
            &rig,
            "task.get",
            &TaskGetParams {
                board: board.clone(),
                id: id.to_string(),
                checkout: None,
            },
        )
        .unwrap()
    };

    // Both are ready; only the first is named by the repo's view. `mine` is listed once the view's file is read.
    let first = get(&held);
    let second = get(&plain);

    assert_eq!(first.views, ["ready", "mine"]);
    assert_eq!(second.views, ["ready"]);
}

// ---- the Field's live updates (ts-0182-ad): what a change of collections or links records ---------------------------

/// A task file with its collections and `part-of` links, which the plan reports as a link to its epic.
fn task_in(rig: &Rig, checkout: &Path, short: &str, collections: &[&str], part_of: &[&str]) {
    let links: Vec<Value> = part_of
        .iter()
        .map(|to| json!({ "type": "part-of", "to": to }))
        .collect();
    let data = json!({
        "short": short, "title": format!("Task {short}"), "status": "todo", "version": "v1",
        "collections": collections, "links": links,
    });
    rig.file(checkout, &format!("{short}.json"), &format!("{data}\n"));
}

/// The `TaskChanged` events of `source` after `seq`, as their bodies.
fn changes_after(rig: &Rig, source: &str, seq: u64) -> Vec<collide_core::provider::TaskChanged> {
    rig.source_events_after(source, seq)
        .into_iter()
        .filter_map(|event| match event.body {
            EventBody::TaskChanged { task, .. } => Some(task),
            _ => None,
        })
        .collect()
}

#[test]
fn a_relist_that_changes_only_a_tasks_collections_records_a_task_changed() {
    let rig = rig();
    let board = rig.board_with(&[&DOCS], "");
    let root = rig.root();
    task_in(&rig, &root, "1", &["epic-a"], &[]);
    rig.settled(&board);
    let before = rig.last_seq();

    task_in(&rig, &root, "1", &["epic-b"], &[]);
    let events = rig.wait_events("docs", before, 1);

    assert_eq!(events.len(), 1, "{events:?}");
    let changes = changes_after(&rig, "docs", before);
    assert_eq!(changes[0].key, "docs:1");
    assert!(changes[0].diff.contains_key("collections"), "{changes:?}");
}

#[test]
fn a_relist_that_changes_only_a_tasks_links_records_a_task_changed() {
    let rig = rig();
    let board = rig.board_with(&[&DOCS], "");
    let root = rig.root();
    task_in(&rig, &root, "1", &[], &["ep-001-aa"]);
    rig.settled(&board);
    let before = rig.last_seq();

    // The epic move of the plan: the task is part of another epic, and nothing else about it changed.
    task_in(&rig, &root, "1", &[], &["ep-002-bb"]);
    let events = rig.wait_events("docs", before, 1);

    assert_eq!(events.len(), 1, "{events:?}");
    let changes = changes_after(&rig, "docs", before);
    assert_eq!(changes[0].key, "docs:1");
}

#[test]
fn a_collections_changed_notice_records_a_task_changed_for_each_task_it_moved() {
    let rig = rig();
    let board = rig.board_with(&[&FILES], "");
    let root = rig.root();
    task_in(&rig, &root, "1", &["epic-a"], &[]);
    task_in(&rig, &root, "2", &["epic-a"], &[]);
    task_in(&rig, &root, "3", &["epic-a"], &[]);
    rig.settled(&board);
    rig.wait_watched(&board, &FILES);
    let before = rig.last_seq();

    // Tasks 1 and 2 move to another collection; 3 is named and does not move.
    rig.notify(
        &board,
        &FILES,
        "collections.changed",
        json!({
            "collections": [],
            "members": [
                { "key": "files:1", "collections": ["epic-b"] },
                { "key": "files:2", "collections": ["epic-b", "epic-c"] },
                { "key": "files:3", "collections": ["epic-a"] },
            ],
            "checkout": root.display().to_string(),
        }),
    );
    let events = rig.wait_events("files", before, 2);

    let mut changes = changes_after(&rig, "files", before);
    changes.sort_by(|a, b| a.key.cmp(&b.key));
    assert_eq!(events.len(), 2, "task 3 did not move: {events:?}");
    assert_eq!(changes[0].key, "files:1");
    assert_eq!(changes[1].key, "files:2");
    assert!(changes[0].diff.contains_key("collections"), "{changes:?}");
    assert_eq!(changes[0].checkout.as_deref(), root.to_str());
}
