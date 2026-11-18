//! Boards and scratch tasks in the daemon (ts-0075-hj; 0.1, 0.10; Stage 3c): `board.open`, `board.list`, `task.apply`
//! with `create` and `setStatus`, and `task.get`, over the daemon's real socket inside the test process. Every path is
//! below the test's own root under `/tmp`; the daemon's git runs with the home and config of that root, and every git
//! command the test runs clears its environment. Nothing here touches the user's home, a real daemon or `launchd`.

mod common;

use collide_core::change::ActorKind;
use collide_core::events::EventBody;
use collide_core::item::Category;
use collide_core::owner::{
    BoardListParams, BoardListResult, BoardOpenParams, BoardOpenResult, BoardSummary, CheckId,
    TaskApplyParams, TaskApplyResult, TaskGetParams, TaskGetResult, code,
};
use collide_core::ports::{ProviderError, TaskProvider};
use collide_core::provider::{
    ApplyParams, ApplyResult, ChangeCause, GetParams, InitializeParams, ListParams, Task,
    WatchParams,
};
use collide_host::base_env::CaptureOptions;
use collide_host::daemon::{Daemon, DaemonOptions, DaemonPaths};
use collide_host::owner_client::{ClientError, OwnerClient};
use collide_host::scratch::ScratchSource;
use collide_host::store::Store;
use common::TempRoot;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

const FIVE: [&str; 5] = ["ready", "active", "review", "done", "dropped"];

// ---- the fixtures -------------------------------------------------------------------------------------------------

/// A login shell that prints the environment, as the capture asks.
fn fake_shell(root: &TempRoot) -> String {
    root.script("fake-shell", "#!/bin/sh\neval \"$4\"\n")
        .to_str()
        .unwrap()
        .to_string()
}

/// What git reads: the root of the test as home, and no configuration of the user's or the system's.
fn git_env(root: &TempRoot) -> Vec<(String, String)> {
    vec![
        (
            "HOME".to_string(),
            root.path().to_str().unwrap().to_string(),
        ),
        ("GIT_CONFIG_GLOBAL".to_string(), "/dev/null".to_string()),
        ("GIT_CONFIG_SYSTEM".to_string(), "/dev/null".to_string()),
        ("GIT_CONFIG_NOSYSTEM".to_string(), "1".to_string()),
    ]
}

fn start(root: &TempRoot) -> Daemon {
    let mut options = DaemonOptions::new(
        DaemonPaths::under(root.path()),
        fake_shell(root),
        Box::new(std::io::sink()),
    );
    options.process_tree = common::test_process_tree();
    options.capture = CaptureOptions {
        timeout: Duration::from_secs(10),
        cap_bytes: 1024 * 1024,
        fallback: BTreeMap::from([("PATH".to_string(), "/usr/bin:/bin".to_string())]),
    };
    options.home = Some(root.path().to_str().unwrap().to_string());
    options.git_env = git_env(root);
    // The board's line is loaded from the source packs, so no test depends on a staged bundle.
    options.load_context = Some(common::config_fixture::plain_factory(root.path()));
    Daemon::start(options).expect("the daemon starts")
}

/// Runs git in `dir` with a cleared environment and returns what it printed.
fn git(root: &TempRoot, dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args([
            "-c",
            "user.name=test",
            "-c",
            "user.email=test@example.invalid",
        ])
        .args(["-c", "commit.gpgsign=false"])
        .args(args)
        .current_dir(dir)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", root.path())
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .env("COLLIDE_DAEMON_ROOT", root.path())
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

/// A checkout named `name` below the root, on `main`, with one commit. Returns its real path.
fn checkout(root: &TempRoot, name: &str) -> PathBuf {
    let dir = root.join(name);
    fs::create_dir_all(dir.join("src")).unwrap();
    git(root, &dir, &["init", "-q", "-b", "main"]);
    fs::write(dir.join("src/lib.rs"), "fn main() {}\n").unwrap();
    git(root, &dir, &["add", "."]);
    git(root, &dir, &["commit", "-q", "-m", "start"]);
    fs::canonicalize(dir).unwrap()
}

fn connect(daemon: &Daemon) -> OwnerClient {
    OwnerClient::connect(daemon.paths(), "scratch-test").expect("the client connects")
}

fn call<P: Serialize, R: DeserializeOwned>(
    client: &mut OwnerClient,
    method: &str,
    params: &P,
) -> R {
    client
        .call(method, params, Some(Duration::from_secs(10)))
        .unwrap_or_else(|e| panic!("{method}: {e}"))
}

fn rpc_error<P: Serialize>(client: &mut OwnerClient, method: &str, params: &P) -> (i64, String) {
    let answer: Result<Value, ClientError> =
        client.call(method, params, Some(Duration::from_secs(10)));
    match answer {
        Err(ClientError::Rpc(error)) => (error.code, error.message),
        other => panic!("{method} should be refused, and gave {other:?}"),
    }
}

fn open(client: &mut OwnerClient, folder: &Path) -> BoardOpenResult {
    call(
        client,
        "board.open",
        &BoardOpenParams {
            root: folder.to_str().unwrap().to_string(),
            init_git: false,
        },
    )
}

fn board_of(opened: BoardOpenResult) -> BoardSummary {
    opened.board.expect("the folder opens as a board")
}

fn list(client: &mut OwnerClient) -> Vec<BoardSummary> {
    call::<_, BoardListResult>(client, "board.list", &BoardListParams {}).boards
}

fn apply(
    client: &mut OwnerClient,
    board: &str,
    op: &str,
    id: Option<&str>,
    args: Value,
) -> ApplyResult {
    let args: BTreeMap<String, Value> = serde_json::from_value(args).unwrap();
    let params = TaskApplyParams {
        board: board.to_string(),
        source: Some("scratch".to_string()),
        apply: ApplyParams {
            op: op.to_string(),
            id: id.map(str::to_string),
            args,
            version: None,
            checkout: None,
        },
        commit: false,
    };
    call::<_, TaskApplyResult>(client, "task.apply", &params)
        .result
        .expect("a scratch write is applied, never queued")
}

fn create(client: &mut OwnerClient, board: &str, prompt: &str) -> Task {
    match apply(client, board, "create", None, json!({"prompt": prompt})) {
        ApplyResult::Ok { task, .. } => task,
        other => panic!("create should work: {other:?}"),
    }
}

fn set_status(client: &mut OwnerClient, board: &str, id: &str, status: &str) -> ApplyResult {
    apply(
        client,
        board,
        "setStatus",
        Some(id),
        json!({"status": status}),
    )
}

fn get(client: &mut OwnerClient, board: &str, id: &str) -> Option<Task> {
    call::<_, TaskGetResult>(
        client,
        "task.get",
        &TaskGetParams {
            board: board.to_string(),
            id: id.to_string(),
            checkout: None,
        },
    )
    .task
}

fn refusal(result: ApplyResult) -> String {
    match result {
        ApplyResult::Refused { message } => message,
        other => panic!("the write should be refused: {other:?}"),
    }
}

// ---- scratch tasks over the socket ------------------------------------------------------------------------------

#[test]
fn a_scratch_task_is_created_ready_with_its_key_and_moves_to_each_category() {
    let root = TempRoot::new();
    let repo = checkout(&root, "repo");
    let daemon = start(&root);
    let mut client = connect(&daemon);
    let board = board_of(open(&mut client, &repo)).id;

    let task = create(
        &mut client,
        &board,
        "Fix the login page\nIt hangs on a slow network.",
    );
    assert_eq!(task.key, "scratch:1");
    assert_eq!(task.title, "Fix the login page");
    assert_eq!(task.state.status, "ready");
    assert_eq!(task.state.category, Category::Ready);

    for status in ["active", "review", "done", "dropped", "ready"] {
        match set_status(&mut client, &board, "scratch:1", status) {
            ApplyResult::Ok { task, .. } => assert_eq!(task.state.status, status),
            other => panic!("{status}: {other:?}"),
        }
        let held = get(&mut client, &board, "scratch:1").expect("the task is still there");
        assert_eq!(held.state.status, status);
    }
}

#[test]
fn the_title_is_the_first_line_of_a_one_line_prompt_and_of_a_long_one() {
    let root = TempRoot::new();
    let repo = checkout(&root, "repo");
    let daemon = start(&root);
    let mut client = connect(&daemon);
    let board = board_of(open(&mut client, &repo)).id;

    let one = create(&mut client, &board, "Rename the settings page");
    assert_eq!(one.title, "Rename the settings page");

    let long_prompt = format!(
        "  Add a dark theme  \n\n{}\nlast line",
        "A paragraph of detail.\n".repeat(200)
    );
    let long = create(&mut client, &board, &long_prompt);
    assert_eq!(long.title, "Add a dark theme");

    let blank_first = create(
        &mut client,
        &board,
        "\n\n  Start with blank lines\nthen more",
    );
    assert_eq!(blank_first.title, "Start with blank lines");
}

#[test]
fn task_get_gives_the_whole_prompt() {
    let root = TempRoot::new();
    let repo = checkout(&root, "repo");
    let daemon = start(&root);
    let mut client = connect(&daemon);
    let board = board_of(open(&mut client, &repo)).id;
    let prompt =
        "Add a dark theme\n\nUse the tokens in `theme.css`.\nKeep the old one as a fallback.\n";
    create(&mut client, &board, prompt);

    let held = get(&mut client, &board, "scratch:1").expect("found by its key");
    assert_eq!(held.body.as_deref(), Some(prompt));
    assert_eq!(held.title, "Add a dark theme");
    let by_number = get(&mut client, &board, "1").expect("found by its number");
    assert_eq!(by_number.key, "scratch:1");
}

#[test]
fn a_prompt_with_no_text_is_refused() {
    let root = TempRoot::new();
    let repo = checkout(&root, "repo");
    let daemon = start(&root);
    let mut client = connect(&daemon);
    let board = board_of(open(&mut client, &repo)).id;

    let message = refusal(apply(
        &mut client,
        &board,
        "create",
        None,
        json!({"prompt": " \n\n "}),
    ));
    assert!(message.contains("prompt"), "{message}");
    let message = refusal(apply(&mut client, &board, "create", None, json!({})));
    assert!(message.contains("prompt"), "{message}");
}

#[test]
fn an_unknown_category_is_refused_and_the_message_names_the_five() {
    let root = TempRoot::new();
    let repo = checkout(&root, "repo");
    let daemon = start(&root);
    let mut client = connect(&daemon);
    let board = board_of(open(&mut client, &repo)).id;
    create(&mut client, &board, "Tidy the build");

    // `backlog` is a category of the corpus, and not one of a scratch task.
    for unknown in ["blocked", "backlog", "Ready", ""] {
        let message = refusal(set_status(&mut client, &board, "scratch:1", unknown));
        for word in FIVE {
            assert!(message.contains(word), "{unknown:?}: {message}");
        }
    }
    let held = get(&mut client, &board, "scratch:1").unwrap();
    assert_eq!(held.state.status, "ready");
    assert_eq!(held.version, "1");
}

fn set_category(client: &mut OwnerClient, board: &str, id: &str, category: &str) -> ApplyResult {
    apply(
        client,
        board,
        "setStatus",
        Some(id),
        json!({"category": category}),
    )
}

#[test]
fn set_status_by_category_moves_the_task_to_each_of_the_five() {
    let root = TempRoot::new();
    let repo = checkout(&root, "repo");
    let daemon = start(&root);
    let mut client = connect(&daemon);
    let board = board_of(open(&mut client, &repo)).id;
    create(&mut client, &board, "Move me by category");

    for category in ["active", "review", "done", "dropped", "ready"] {
        match set_category(&mut client, &board, "scratch:1", category) {
            ApplyResult::Ok { task, .. } => {
                assert_eq!(json!(task.state.category), json!(category));
                assert!(FIVE.contains(&task.state.status.as_str()), "{task:?}");
            }
            other => panic!("{category}: {other:?}"),
        }
        let held = get(&mut client, &board, "scratch:1").unwrap();
        assert_eq!(json!(held.state.category), json!(category));
    }
}

#[test]
fn a_category_with_no_scratch_status_is_refused_and_the_message_names_the_ones_covered() {
    let root = TempRoot::new();
    let repo = checkout(&root, "repo");
    let daemon = start(&root);
    let mut client = connect(&daemon);
    let board = board_of(open(&mut client, &repo)).id;
    create(&mut client, &board, "Stay where you are");

    for unknown in ["backlog", "blocked"] {
        let message = refusal(set_category(&mut client, &board, "scratch:1", unknown));
        assert!(message.contains("category"), "{message}");
        for word in FIVE {
            assert!(message.contains(word), "{unknown}: {message}");
        }
    }
    let held = get(&mut client, &board, "scratch:1").unwrap();
    assert_eq!(held.state.status, "ready");
    assert_eq!(held.version, "1");
}

#[test]
fn set_status_takes_exactly_one_of_status_and_category() {
    let root = TempRoot::new();
    let repo = checkout(&root, "repo");
    let daemon = start(&root);
    let mut client = connect(&daemon);
    let board = board_of(open(&mut client, &repo)).id;
    create(&mut client, &board, "Name it once");

    let both = json!({"status": "done", "category": "done"});
    let message = refusal(apply(
        &mut client,
        &board,
        "setStatus",
        Some("scratch:1"),
        both,
    ));
    assert!(message.contains("one of"), "{message}");
    let message = refusal(apply(
        &mut client,
        &board,
        "setStatus",
        Some("scratch:1"),
        json!({}),
    ));
    assert!(message.contains("one of"), "{message}");
    assert_eq!(get(&mut client, &board, "scratch:1").unwrap().version, "1");
}

#[test]
fn a_task_that_is_not_on_the_board_is_refused_and_a_missing_status_is_refused() {
    let root = TempRoot::new();
    let repo = checkout(&root, "repo");
    let daemon = start(&root);
    let mut client = connect(&daemon);
    let board = board_of(open(&mut client, &repo)).id;

    let message = refusal(set_status(&mut client, &board, "scratch:9", "done"));
    assert!(message.contains('9'), "{message}");
    let message = refusal(apply(
        &mut client,
        &board,
        "setStatus",
        Some("scratch:9"),
        json!({}),
    ));
    assert!(message.contains("status"), "{message}");
    let message = refusal(apply(&mut client, &board, "frobnicate", None, json!({})));
    assert!(message.contains("frobnicate"), "{message}");
}

#[test]
fn a_key_from_one_board_is_not_found_on_another() {
    let root = TempRoot::new();
    let first_repo = checkout(&root, "first");
    let second_repo = checkout(&root, "second");
    let daemon = start(&root);
    let mut client = connect(&daemon);
    let first = board_of(open(&mut client, &first_repo)).id;
    let second = board_of(open(&mut client, &second_repo)).id;
    assert_ne!(first, second);

    create(&mut client, &first, "Only on the first board");
    assert!(get(&mut client, &first, "scratch:1").is_some());
    assert!(get(&mut client, &second, "scratch:1").is_none());
    let message = refusal(set_status(&mut client, &second, "scratch:1", "done"));
    assert!(message.contains('1'), "{message}");
    assert_eq!(
        get(&mut client, &first, "scratch:1").unwrap().state.status,
        "ready"
    );

    // Each board counts its own tasks.
    let other = create(&mut client, &second, "First on the second board");
    assert_eq!(other.key, "scratch:1");
    assert_eq!(other.title, "First on the second board");
}

#[test]
fn a_task_of_a_source_the_line_does_not_have_is_not_found_and_a_write_to_it_is_refused() {
    let root = TempRoot::new();
    let repo = checkout(&root, "repo");
    let daemon = start(&root);
    let mut client = connect(&daemon);
    let board = board_of(open(&mut client, &repo)).id;
    create(&mut client, &board, "A scratch task");

    assert!(get(&mut client, &board, "github:1").is_none());
    let params = TaskApplyParams {
        board: board.clone(),
        source: None,
        apply: ApplyParams {
            op: "setStatus".to_string(),
            id: Some("github:1".to_string()),
            args: BTreeMap::from([("status".to_string(), json!("done"))]),
            version: None,
            checkout: None,
        },
        commit: false,
    };
    let (number, message) = rpc_error(&mut client, "task.apply", &params);
    assert_eq!(number, code::INVALID_PARAMS);
    assert!(message.contains("github"), "{message}");
}

#[test]
fn a_write_names_its_source_one_way_or_another() {
    let root = TempRoot::new();
    let repo = checkout(&root, "repo");
    let daemon = start(&root);
    let mut client = connect(&daemon);
    let board = board_of(open(&mut client, &repo)).id;
    create(&mut client, &board, "A scratch task");

    let mut params = TaskApplyParams {
        board: board.clone(),
        source: None,
        apply: ApplyParams {
            op: "create".to_string(),
            id: None,
            args: BTreeMap::from([("prompt".to_string(), json!("No source named"))]),
            version: None,
            checkout: None,
        },
        commit: false,
    };
    let (number, _) = rpc_error(&mut client, "task.apply", &params);
    assert_eq!(number, code::INVALID_PARAMS);

    // The id names the source, so `source` may be left out.
    params.apply.op = "setStatus".to_string();
    params.apply.id = Some("scratch:1".to_string());
    params.apply.args = BTreeMap::from([("status".to_string(), json!("active"))]);
    let result: TaskApplyResult = call(&mut client, "task.apply", &params);
    assert!(matches!(result.result, Some(ApplyResult::Ok { .. })));

    // A `source` that disagrees with the id's is refused.
    params.source = Some("plan".to_string());
    let (number, _) = rpc_error(&mut client, "task.apply", &params);
    assert_eq!(number, code::INVALID_PARAMS);
}

#[test]
fn a_write_with_a_stale_version_is_a_conflict_that_carries_the_current_task() {
    let root = TempRoot::new();
    let repo = checkout(&root, "repo");
    let daemon = start(&root);
    let mut client = connect(&daemon);
    let board = board_of(open(&mut client, &repo)).id;
    create(&mut client, &board, "Version me");
    set_status(&mut client, &board, "scratch:1", "active");

    let params = TaskApplyParams {
        board: board.clone(),
        source: Some("scratch".to_string()),
        apply: ApplyParams {
            op: "setStatus".to_string(),
            id: Some("1".to_string()),
            args: BTreeMap::from([("status".to_string(), json!("done"))]),
            version: Some("1".to_string()),
            checkout: None,
        },
        commit: false,
    };
    let result: TaskApplyResult = call(&mut client, "task.apply", &params);
    match result.result {
        Some(ApplyResult::Conflict { current }) => {
            assert_eq!(current.version, "2");
            assert_eq!(current.state.status, "active");
        }
        other => panic!("expected a conflict: {other:?}"),
    }
    assert_eq!(
        get(&mut client, &board, "1").unwrap().state.status,
        "active"
    );
}

#[test]
fn a_scratch_task_is_never_committed_so_commit_is_refused() {
    let root = TempRoot::new();
    let repo = checkout(&root, "repo");
    let daemon = start(&root);
    let mut client = connect(&daemon);
    let board = board_of(open(&mut client, &repo)).id;
    let before = git(&root, &repo, &["rev-parse", "HEAD"]);

    let params = TaskApplyParams {
        board,
        source: Some("scratch".to_string()),
        apply: ApplyParams {
            op: "create".to_string(),
            id: None,
            args: BTreeMap::from([("prompt".to_string(), json!("Commit me"))]),
            version: None,
            checkout: None,
        },
        commit: true,
    };
    let (number, message) = rpc_error(&mut client, "task.apply", &params);
    assert_eq!(number, code::INVALID_PARAMS);
    assert!(message.contains("never committed"), "{message}");
    assert_eq!(git(&root, &repo, &["rev-parse", "HEAD"]), before);
}

#[test]
fn a_request_for_a_board_the_daemon_has_not_opened_is_not_found() {
    let root = TempRoot::new();
    let daemon = start(&root);
    let mut client = connect(&daemon);

    let get_params = TaskGetParams {
        board: "board-7".to_string(),
        id: "1".to_string(),
        checkout: None,
    };
    let (number, message) = rpc_error(&mut client, "task.get", &get_params);
    assert_eq!(number, code::NOT_FOUND);
    assert!(message.contains("board-7"), "{message}");

    let apply_params = TaskApplyParams {
        board: "nonsense".to_string(),
        source: Some("scratch".to_string()),
        apply: ApplyParams {
            op: "create".to_string(),
            id: None,
            args: BTreeMap::from([("prompt".to_string(), json!("Nowhere"))]),
            version: None,
            checkout: None,
        },
        commit: false,
    };
    let (number, _) = rpc_error(&mut client, "task.apply", &apply_params);
    assert_eq!(number, code::NOT_FOUND);
}

// ---- the events -------------------------------------------------------------------------------------------------

#[test]
fn each_write_records_a_task_changed_event_with_the_actor_of_the_connection() {
    let root = TempRoot::new();
    let repo = checkout(&root, "repo");
    let daemon = start(&root);
    let mut client = connect(&daemon);
    let board = board_of(open(&mut client, &repo)).id;

    create(&mut client, &board, "Watch me change");
    set_status(&mut client, &board, "scratch:1", "active");
    // A write that changes nothing records nothing.
    set_status(&mut client, &board, "scratch:1", "active");
    refusal(set_status(&mut client, &board, "scratch:1", "blocked"));
    set_status(&mut client, &board, "scratch:1", "done");

    let changes: Vec<_> = daemon
        .events_since(0)
        .unwrap()
        .into_iter()
        .filter_map(|event| match event.body {
            EventBody::TaskChanged { source, task } => Some((event.actor, source, task)),
            _ => None,
        })
        .collect();
    assert_eq!(changes.len(), 3, "{changes:?}");
    for (actor, source, task) in &changes {
        assert_eq!(actor.kind, ActorKind::Person);
        assert_eq!(source, "scratch");
        // The change is in Collide's own store: no checkout, remote or journal of a source made it.
        assert_eq!(task.cause, ChangeCause::Store);
    }
    let (_, _, created) = &changes[0];
    assert_eq!(created.key, "scratch:1");
    assert_eq!(created.version, "1");
    assert!(created.from.is_none());
    assert_eq!(created.to.as_ref().unwrap().status, "ready");
    let (_, _, activated) = &changes[1];
    assert_eq!(activated.version, "2");
    assert_eq!(activated.from.as_ref().unwrap().status, "ready");
    assert_eq!(activated.to.as_ref().unwrap().status, "active");
    let (_, _, finished) = &changes[2];
    assert_eq!(finished.version, "3");
    assert_eq!(finished.to.as_ref().unwrap().status, "done");
}

// ---- restart ----------------------------------------------------------------------------------------------------

#[test]
fn boards_and_scratch_tasks_survive_a_daemon_restart() {
    let root = TempRoot::new();
    let first_repo = checkout(&root, "first");
    let second_repo = checkout(&root, "second");
    let (first, second) = {
        let daemon = start(&root);
        let mut client = connect(&daemon);
        let first = board_of(open(&mut client, &first_repo)).id;
        let second = board_of(open(&mut client, &second_repo)).id;
        create(&mut client, &first, "Keep me\nwith a second line");
        create(&mut client, &first, "Keep me too");
        set_status(&mut client, &first, "scratch:2", "review");
        create(&mut client, &second, "On the other board");
        drop(client);
        daemon.stop();
        (first, second)
    };

    let daemon = start(&root);
    let mut client = connect(&daemon);
    let boards = list(&mut client);
    assert_eq!(
        boards
            .iter()
            .map(|board| board.id.as_str())
            .collect::<Vec<_>>(),
        [first.as_str(), second.as_str()]
    );
    let again = board_of(open(&mut client, &first_repo));
    assert_eq!(again.id, first);

    let kept = get(&mut client, &first, "scratch:1").expect("the task is kept");
    assert_eq!(kept.title, "Keep me");
    assert_eq!(kept.body.as_deref(), Some("Keep me\nwith a second line"));
    assert_eq!(
        get(&mut client, &first, "scratch:2").unwrap().state.status,
        "review"
    );
    assert!(get(&mut client, &second, "scratch:2").is_none());
    // Numbering goes on where it stopped.
    assert_eq!(
        create(&mut client, &first, "After the restart").key,
        "scratch:3"
    );
}

// ---- board.open and board.list ----------------------------------------------------------------------------------

#[test]
fn a_folder_that_is_not_a_checkout_opens_no_board_and_fails_the_git_repo_check_with_the_fix() {
    let root = TempRoot::new();
    let plain = root.join("plain");
    fs::create_dir(&plain).unwrap();
    let daemon = start(&root);
    let mut client = connect(&daemon);

    let opened = open(&mut client, &plain);
    assert!(opened.board.is_none());
    assert_eq!(opened.checks.len(), 1);
    let check = &opened.checks[0];
    assert_eq!(check.id, CheckId::GitRepo);
    assert!(!check.ok);
    assert!(check.required);
    assert!(
        check.message.contains("not inside a git checkout"),
        "{}",
        check.message
    );
    assert!(check.message.contains("git init"), "{}", check.message);
    assert!(list(&mut client).is_empty());
}

#[test]
fn a_folder_that_is_not_there_opens_no_board() {
    let root = TempRoot::new();
    let daemon = start(&root);
    let mut client = connect(&daemon);

    let opened = open(&mut client, &root.join("missing"));
    assert!(opened.board.is_none());
    assert!(!opened.checks[0].ok);
    assert!(
        opened.checks[0].message.contains("not a folder"),
        "{}",
        opened.checks[0].message
    );
}

#[test]
fn a_relative_root_is_refused() {
    let root = TempRoot::new();
    let daemon = start(&root);
    let mut client = connect(&daemon);

    let params = BoardOpenParams {
        root: "some/folder".to_string(),
        init_git: false,
    };
    let (number, message) = rpc_error(&mut client, "board.open", &params);
    assert_eq!(number, code::INVALID_PARAMS);
    assert!(message.contains("absolute"), "{message}");
}

#[test]
fn init_git_is_refused_as_not_built_yet() {
    let root = TempRoot::new();
    let plain = root.join("plain");
    fs::create_dir(&plain).unwrap();
    let daemon = start(&root);
    let mut client = connect(&daemon);

    let params = BoardOpenParams {
        root: plain.to_str().unwrap().to_string(),
        init_git: true,
    };
    let (number, message) = rpc_error(&mut client, "board.open", &params);
    assert_eq!(number, code::INVALID_PARAMS);
    assert!(message.contains("not built yet"), "{message}");
    assert!(message.contains("git init"), "{message}");
    assert!(!plain.join(".git").exists(), "nothing was initialised");
    assert!(list(&mut client).is_empty());
}

#[test]
fn a_checkouts_top_level_and_a_folder_inside_it_open_the_same_board_on_the_branch() {
    let root = TempRoot::new();
    let repo = checkout(&root, "repo");
    let daemon = start(&root);
    let mut client = connect(&daemon);

    let top = open(&mut client, &repo);
    assert!(top.checks[0].ok);
    let top = board_of(top);
    assert_eq!(top.root, repo.to_str().unwrap());
    assert_eq!(top.lines.len(), 1);
    assert_eq!(top.lines[0].ref_name, "main");
    assert_eq!(top.lines[0].root, repo.to_str().unwrap());

    let inside = board_of(open(&mut client, &repo.join("src")));
    assert_eq!(inside, top);
    let again = board_of(open(&mut client, &repo));
    assert_eq!(again, top);
    assert_eq!(list(&mut client).len(), 1);
}

#[test]
fn board_list_gives_the_boards_opened_before_with_the_branch_each_checkout_is_on_now() {
    let root = TempRoot::new();
    let first_repo = checkout(&root, "first");
    let second_repo = checkout(&root, "second");
    let daemon = start(&root);
    let mut client = connect(&daemon);
    assert!(list(&mut client).is_empty());

    let first = board_of(open(&mut client, &first_repo));
    let second = board_of(open(&mut client, &second_repo));
    assert_eq!(list(&mut client), vec![first.clone(), second.clone()]);

    git(&root, &second_repo, &["checkout", "-q", "-b", "feature"]);
    let boards = list(&mut client);
    assert_eq!(boards[0].lines[0].ref_name, "main");
    assert_eq!(boards[1].lines[0].ref_name, "feature");
    assert_eq!(boards[1].id, second.id);

    // A checkout on no branch has the line `HEAD`.
    let head = git(&root, &second_repo, &["rev-parse", "HEAD"]);
    git(&root, &second_repo, &["checkout", "-q", "--detach", &head]);
    assert_eq!(list(&mut client)[1].lines[0].ref_name, "HEAD");
}

#[test]
fn a_board_whose_folder_is_gone_is_still_listed_with_no_line() {
    let root = TempRoot::new();
    let repo = checkout(&root, "repo");
    let daemon = start(&root);
    let mut client = connect(&daemon);
    let board = board_of(open(&mut client, &repo));
    create(&mut client, &board.id, "Outlives the folder");

    fs::remove_dir_all(&repo).unwrap();
    let boards = list(&mut client);
    assert_eq!(boards.len(), 1);
    assert_eq!(boards[0].id, board.id);
    assert!(boards[0].lines.is_empty());
    assert!(get(&mut client, &board.id, "scratch:1").is_some());
}

// ---- the repository ---------------------------------------------------------------------------------------------

#[test]
fn the_checkout_is_unchanged_by_create_and_by_every_set_status() {
    let root = TempRoot::new();
    let repo = checkout(&root, "repo");
    let status_of = || git(&root, &repo, &["status", "--porcelain", "--ignored"]);
    let head_of = || git(&root, &repo, &["rev-parse", "HEAD"]);
    let files_of = || {
        let mut names: Vec<String> = fs::read_dir(&repo)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    };
    assert_eq!(status_of(), "");
    let (head, files) = (head_of(), files_of());

    let daemon = start(&root);
    let mut client = connect(&daemon);
    let board = board_of(open(&mut client, &repo)).id;
    assert_eq!(status_of(), "", "opening the board wrote nothing");

    create(&mut client, &board, "Leave the repository alone");
    assert_eq!(status_of(), "", "create wrote into the checkout");
    for status in FIVE {
        set_status(&mut client, &board, "scratch:1", status);
        assert_eq!(
            status_of(),
            "",
            "setStatus {status} wrote into the checkout"
        );
    }
    assert_eq!(head_of(), head);
    assert_eq!(files_of(), files);
}

// ---- the source as a provider -----------------------------------------------------------------------------------

#[test]
fn the_scratch_source_lists_one_board_and_declines_watch_and_preview() {
    let root = TempRoot::new();
    let store = Arc::new(Store::open(&root.join("store")).unwrap());
    let first = store.open_board("/some/first").unwrap();
    let second = store.open_board("/some/second").unwrap();
    let on_first = ScratchSource::new(store.clone(), &first);
    let on_second = ScratchSource::new(store.clone(), &second);

    let create = |source: &ScratchSource, prompt: &str| {
        source
            .apply(&ApplyParams {
                op: "create".to_string(),
                id: None,
                args: BTreeMap::from([("prompt".to_string(), json!(prompt))]),
                version: None,
                checkout: None,
            })
            .unwrap()
    };
    create(&on_first, "One\nwith detail");
    create(&on_first, "Two");
    create(&on_second, "Elsewhere");

    let listed = on_first
        .list(&ListParams {
            checkout: None,
            cursor: None,
        })
        .unwrap();
    let keys: Vec<_> = listed.tasks.iter().map(|task| task.key.as_str()).collect();
    assert_eq!(keys, ["scratch:1", "scratch:2"]);
    assert!(
        listed.tasks.iter().all(|task| task.body.is_none()),
        "a list carries no body"
    );
    let held = on_first
        .get(&GetParams {
            id: "1".to_string(),
            checkout: None,
        })
        .unwrap();
    assert_eq!(held.task.unwrap().body.as_deref(), Some("One\nwith detail"));

    assert!(matches!(
        on_first.watch(&WatchParams { checkouts: vec![] }),
        Err(ProviderError::Unsupported { .. })
    ));
    let preview = ApplyParams {
        op: "create".to_string(),
        id: None,
        args: BTreeMap::new(),
        version: None,
        checkout: None,
    };
    assert!(matches!(
        on_first.preview(&preview),
        Err(ProviderError::Unsupported { .. })
    ));

    let described = on_first
        .initialize(&InitializeParams {
            config: BTreeMap::new(),
            root: "/some/first".to_string(),
        })
        .unwrap();
    assert!(described.capabilities.create);
    assert!(!described.capabilities.preview);
    let statuses: Vec<_> = described.statuses.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(statuses, FIVE);
    let ops: Vec<_> = described.ops.iter().map(|op| op.name.as_str()).collect();
    assert_eq!(ops, ["create", "setStatus"]);
    let schema = described.ops[1].args.as_ref().unwrap();
    assert!(schema["properties"]["status"].is_object(), "{schema}");
    assert!(schema["properties"]["category"].is_object(), "{schema}");
}
