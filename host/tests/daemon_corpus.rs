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
use collide_core::non_empty::NonEmpty;
use collide_core::owner::{
    BoardOpenParams, BoardOpenResult, BoardTrustParams, BoardTrustResult, BoardTrustReviewParams,
    BoardTrustReviewResult, RpcError, TaskApplyParams, TaskApplyResult, TaskGetParams,
    TaskGetResult, TaskListResult, TrustState,
};
use collide_core::provider::{ApplyParams, ApplyResult};
use collide_core::state::RunState;
use collide_host::corpus_store::CorpusCache;
use collide_host::daemon::DaemonOptions;
use collide_host::owner_client::ClientError;
use collide_host::run_store::RunLaunch;
use collide_host::secret_store::MemorySecretStore;
use collide_host::store::Store;
use collide_host::supervisor::{BackoffPolicy, SupervisorPolicy};
use common::config_fixture::{CapturedLog, Fixture};
use common::git_repo::GitRepo;
use common::{node_missing_message, wait_until};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

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
fn a_view_is_not_served_yet_and_says_which_item_serves_it() {
    let rig = rig();
    let board = rig.board_with(&[&FILES], "");

    let error = rpc::<TaskListResult>(
        &rig,
        "task.list",
        &json!({ "board": board, "view": { "type": "named", "name": "all" } }),
    )
    .unwrap_err();

    assert!(error.message.contains("ts-0118-wp"), "{}", error.message);
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

    let events = rig.source_events_after("files", before);
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
