//! `board.remove` (ts-0222-gx; 0.2, Boards and lines; Stage 6a, Line selector): taking a board out of Collide hides it.
//! The board leaves `board.list` and its corpus cache is dropped; its scratch tasks, its runs, the person's files
//! for it and what was trusted for it stay, and opening its folder again brings it back as the same board. A run in
//! progress, a defaults connection and a board the daemon does not list are refused in words that name the cause, and
//! every subscribed window is told. A daemon runs in the test's own process over fixture repositories in a temp root,
//! which is also the home. Every git command clears its environment and reads no configuration of the user's.
//! Nothing here touches the user's home, a real daemon or `launchd`.

mod common;

use collide_core::change::{Actor, ActorKind};
use collide_core::events::EventBody;
use collide_core::non_empty::NonEmpty;
use collide_core::owner::{
    BoardListParams, BoardListResult, BoardLocateParams, BoardOpenParams, BoardOpenResult,
    BoardSummary, Notice, TrustState, code,
};
use collide_core::state::RunState;
use collide_host::base_env::CaptureOptions;
use collide_host::daemon::{Daemon, DaemonOptions, DaemonPaths};
use collide_host::owner_client::{ClientError, OwnerClient};
use collide_host::run_store::RunLaunch;
use common::TempRoot;
use common::subscriber::{Next, Stream};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

const DEADLINE: Duration = Duration::from_secs(30);

// ---- the fixtures -------------------------------------------------------------------------------------------------

fn start(root: &TempRoot) -> Daemon {
    let shell = root
        .script("fake-shell", "#!/bin/sh\neval \"$4\"\n")
        .to_str()
        .unwrap()
        .to_string();
    let mut options = DaemonOptions::new(
        DaemonPaths::under(root.path()),
        shell,
        Box::new(std::io::sink()),
    );
    options.process_tree = common::test_process_tree();
    options.capture = CaptureOptions {
        timeout: Duration::from_secs(10),
        cap_bytes: 1024 * 1024,
        fallback: BTreeMap::from([("PATH".to_string(), "/usr/bin:/bin".to_string())]),
    };
    options.home = Some(root.path().to_str().unwrap().to_string());
    options.git_env = vec![
        (
            "HOME".to_string(),
            root.path().to_str().unwrap().to_string(),
        ),
        ("GIT_CONFIG_GLOBAL".to_string(), "/dev/null".to_string()),
        ("GIT_CONFIG_SYSTEM".to_string(), "/dev/null".to_string()),
        ("GIT_CONFIG_NOSYSTEM".to_string(), "1".to_string()),
    ];
    options.load_context = Some(common::config_fixture::plain_factory(root.path()));
    Daemon::start(options).expect("the daemon starts")
}

fn git(root: &TempRoot, dir: &Path, args: &[&str]) {
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
}

/// A checkout named `name` below the root, on `main`, with one commit that holds `content`.
fn checkout(root: &TempRoot, name: &str, content: &str) -> PathBuf {
    let dir = root.join(name);
    fs::create_dir_all(&dir).unwrap();
    git(root, &dir, &["init", "-q", "-b", "main"]);
    fs::write(dir.join("file.txt"), content).unwrap();
    git(root, &dir, &["add", "."]);
    git(root, &dir, &["commit", "-q", "-m", "start"]);
    fs::canonicalize(dir).unwrap()
}

fn connect(daemon: &Daemon) -> OwnerClient {
    OwnerClient::connect(daemon.paths(), "remove-test").expect("the client connects")
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

fn refused<P: Serialize>(client: &mut OwnerClient, method: &str, params: &P) -> (i64, String) {
    let answer: Result<Value, ClientError> =
        client.call(method, params, Some(Duration::from_secs(10)));
    match answer {
        Err(ClientError::Rpc(error)) => (error.code, error.message),
        other => panic!("{method} should be refused, and gave {other:?}"),
    }
}

fn board_of(client: &mut OwnerClient, folder: &Path) -> BoardSummary {
    let opened: BoardOpenResult = call(
        client,
        "board.open",
        &BoardOpenParams {
            root: folder.to_str().unwrap().to_string(),
            init_git: false,
        },
    );
    opened.board.expect("the folder opens as a board")
}

fn list(client: &mut OwnerClient) -> Vec<BoardSummary> {
    call::<_, BoardListResult>(client, "board.list", &BoardListParams {}).boards
}

fn listed_ids(client: &mut OwnerClient) -> Vec<String> {
    list(client).into_iter().map(|board| board.id).collect()
}

fn remove(client: &mut OwnerClient, board: &str) {
    let _: Value = call(client, "board.remove", &json!({ "board": board }));
}

fn create_task(client: &mut OwnerClient, board: &str, prompt: &str) {
    let params = json!({
        "board": board,
        "source": "scratch",
        "apply": { "op": "create", "args": { "prompt": prompt } },
        "commit": false,
    });
    let _: Value = call(client, "task.apply", &params);
}

fn first_task(client: &mut OwnerClient, board: &str) -> Option<String> {
    let answer: Value = call(
        client,
        "task.get",
        &json!({ "board": board, "id": "scratch:1" }),
    );
    answer["task"]["title"].as_str().map(str::to_string)
}

/// Lists the board's tasks, which starts its corpus, and gives how many there are once it has read them.
fn task_count(client: &mut OwnerClient, board: &str) -> usize {
    let deadline = std::time::Instant::now() + DEADLINE;
    loop {
        let answer: Value = call(client, "task.list", &json!({ "board": board }));
        let tasks = answer["tasks"].as_array().map_or(0, Vec::len);
        if tasks > 0 || std::time::Instant::now() > deadline {
            return tasks;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn view_names(client: &mut OwnerClient, board: &str) -> Vec<String> {
    let answer: Value = call(client, "task.views", &json!({ "board": board }));
    answer["views"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["view"]["name"].as_str().unwrap().to_string())
        .collect()
}

/// Records one `run.transition` through the daemon's door for events, and links the run to a board.
fn run_on(daemon: &Daemon, run: &str, board: &str, to: RunState) {
    let actor = Actor {
        kind: ActorKind::Workflow,
        run: Some(run.to_string()),
        phase: None,
    };
    daemon
        .phases()
        .append_event(
            actor,
            EventBody::RunTransition {
                run: run.to_string(),
                from: None,
                to,
            },
        )
        .unwrap();
    let launch = RunLaunch {
        board: Some(board.to_string()),
        tasks: Vec::new(),
        line: "main".to_string(),
        workflow: "collide/quick".to_string(),
    };
    assert!(daemon.store().set_run_launch(run, &launch).unwrap());
}

fn trust_of(board: &BoardSummary) -> TrustState {
    board.lines[0].trust.clone()
}

// ---- remove hides, and opening the folder brings it back ----------------------------------------------------------

#[test]
fn a_removed_board_leaves_the_list_and_the_one_beside_it_stays() {
    let root = TempRoot::new();
    let first = checkout(&root, "first", "one\n");
    let second = checkout(&root, "second", "two\n");
    let daemon = start(&root);
    let mut client = connect(&daemon);
    let one = board_of(&mut client, &first).id;
    let two = board_of(&mut client, &second).id;

    remove(&mut client, &one);

    assert_eq!(listed_ids(&mut client), [two]);
    daemon.stop();
}

#[test]
fn a_board_whose_folder_is_gone_can_be_removed_from_the_list() {
    let root = TempRoot::new();
    let first = checkout(&root, "first", "one\n");
    let daemon = start(&root);
    let mut client = connect(&daemon);
    let one = board_of(&mut client, &first).id;
    fs::rename(&first, root.join("moved")).unwrap();
    assert!(list(&mut client)[0].missing);

    remove(&mut client, &one);

    assert!(list(&mut client).is_empty());
    daemon.stop();
}

#[test]
fn opening_the_folder_again_brings_the_board_back_with_its_tasks_views_files_and_trust() {
    let root = TempRoot::new();
    let repo = checkout(&root, "repo", "one\n");
    let daemon = start(&root);
    let mut client = connect(&daemon);
    let board = board_of(&mut client, &repo);
    create_task(&mut client, &board.id, "Keep me");
    assert_eq!(task_count(&mut client, &board.id), 1);
    let hash = match trust_of(&board) {
        TrustState::Untrusted { hash } | TrustState::Trusted { hash } => hash,
    };
    let _: Value = call(
        &mut client,
        "board.trust",
        &json!({ "board": board.id, "line": "main", "hash": hash }),
    );
    let view = json!({ "name": "mine", "where": "true" });
    let _: Value = call(
        &mut client,
        "task.save_view",
        &json!({ "board": board.id, "view": view, "target": "personal" }),
    );
    let files = root.path().join(".config/collide/boards").join(&board.id);
    assert!(files.exists(), "the person's files for the board are there");
    let trusted = fs::read(root.path().join(".config/collide/trust.yaml")).unwrap();

    remove(&mut client, &board.id);

    assert!(list(&mut client).is_empty());
    assert!(files.join("views").exists(), "the person's files are kept");
    assert_eq!(
        fs::read(root.path().join(".config/collide/trust.yaml")).unwrap(),
        trusted,
        "what was trusted is kept"
    );
    let again = board_of(&mut client, &repo);
    assert_eq!(again.id, board.id, "the same board comes back");
    assert!(matches!(trust_of(&again), TrustState::Trusted { .. }));
    assert_eq!(listed_ids(&mut client), std::slice::from_ref(&board.id));
    assert_eq!(
        first_task(&mut client, &board.id).as_deref(),
        Some("Keep me")
    );
    assert!(view_names(&mut client, &board.id).contains(&"mine".to_string()));
    // The corpus began again, and lists the same task.
    assert_eq!(task_count(&mut client, &board.id), 1);
    daemon.stop();
}

#[test]
fn a_removed_board_keeps_its_runs() {
    let root = TempRoot::new();
    let repo = checkout(&root, "repo", "one\n");
    let daemon = start(&root);
    let mut client = connect(&daemon);
    let board = board_of(&mut client, &repo).id;
    let ended = RunState::Ended {
        outcome: NonEmpty::new("done").unwrap(),
    };
    run_on(&daemon, "r1", &board, ended);

    remove(&mut client, &board);
    assert!(list(&mut client).is_empty());
    board_of(&mut client, &repo);

    let runs: Value = call(&mut client, "run.list", &json!({ "board": board }));
    assert_eq!(runs["runs"].as_array().map(Vec::len), Some(1), "{runs}");
    daemon.stop();
}

#[test]
fn removing_a_board_drops_its_corpus_rows_and_keeps_the_rows_of_the_board_beside_it() {
    let root = TempRoot::new();
    let first = checkout(&root, "first", "one\n");
    let second = checkout(&root, "second", "two\n");
    let daemon = start(&root);
    let mut client = connect(&daemon);
    let one = board_of(&mut client, &first).id;
    let two = board_of(&mut client, &second).id;
    // A source of a pack that is not part of this fixture, so the board's own corpus never writes its rows.
    for board in [&one, &two] {
        let task = serde_json::from_value(json!({
            "key": "plan:1", "id": "1", "title": "A task",
            "state": { "status": "draft", "category": "ready" }, "version": "v1",
        }))
        .unwrap();
        daemon
            .store()
            .corpus_put_task(board, "plan", None, &task)
            .unwrap();
    }
    let cached = |board: &str| daemon.store().corpus_cache(board).unwrap().tasks.len();
    assert_eq!((cached(&one), cached(&two)), (1, 1));

    remove(&mut client, &one);

    assert_eq!(cached(&one), 0, "the removed board's cache is dropped");
    assert_eq!(cached(&two), 1, "the other board's is not");
    daemon.stop();
}

#[test]
fn a_removed_board_stays_removed_across_a_restart_and_the_folder_still_brings_it_back() {
    let root = TempRoot::new();
    let repo = checkout(&root, "repo", "one\n");
    let board = {
        let daemon = start(&root);
        let mut client = connect(&daemon);
        let board = board_of(&mut client, &repo).id;
        create_task(&mut client, &board, "Keep me");
        remove(&mut client, &board);
        drop(client);
        daemon.stop();
        board
    };

    let daemon = start(&root);
    let mut client = connect(&daemon);
    assert!(list(&mut client).is_empty(), "removed after a restart");
    let again = board_of(&mut client, &repo);

    assert_eq!(again.id, board);
    assert_eq!(first_task(&mut client, &board).as_deref(), Some("Keep me"));
    daemon.stop();
}

#[test]
fn a_new_board_never_takes_the_name_of_a_removed_one() {
    let root = TempRoot::new();
    let first = checkout(&root, "first", "one\n");
    let second = checkout(&root, "second", "two\n");
    let daemon = start(&root);
    let mut client = connect(&daemon);
    let one = board_of(&mut client, &first).id;
    remove(&mut client, &one);

    let two = board_of(&mut client, &second).id;

    assert_ne!(one, two);
    assert_eq!(listed_ids(&mut client), [two]);
    daemon.stop();
}

#[test]
fn a_removed_board_is_unknown_to_the_methods_that_name_a_board() {
    let root = TempRoot::new();
    let repo = checkout(&root, "repo", "one\n");
    let daemon = start(&root);
    let mut client = connect(&daemon);
    let board = board_of(&mut client, &repo).id;
    remove(&mut client, &board);

    let (listing, _) = refused(&mut client, "task.list", &json!({ "board": board }));
    let (picking, _) = refused(&mut client, "board.pick", &json!({ "board": board }));

    assert_eq!(listing, code::NOT_FOUND);
    assert_eq!(picking, code::NOT_FOUND);
    daemon.stop();
}

#[test]
fn locating_another_board_at_the_folder_of_a_removed_one_is_refused_in_words_that_name_it() {
    let root = TempRoot::new();
    let first = checkout(&root, "first", "one\n");
    let second = checkout(&root, "second", "two\n");
    let daemon = start(&root);
    let mut client = connect(&daemon);
    let one = board_of(&mut client, &first).id;
    let two = board_of(&mut client, &second).id;
    remove(&mut client, &one);
    fs::rename(&second, root.join("elsewhere")).unwrap();

    let params = BoardLocateParams {
        board: two.clone(),
        path: first.to_str().unwrap().to_string(),
    };
    let (error, message) = refused(&mut client, "board.locate", &params);

    assert_eq!(error, code::CONFLICT);
    assert!(
        message.contains(&one) && message.contains("removed"),
        "{message}"
    );
    daemon.stop();
}

// ---- what refuses it ----------------------------------------------------------------------------------------------

#[test]
fn a_run_in_progress_refuses_the_removal_naming_the_run_and_stops_nothing() {
    let root = TempRoot::new();
    let repo = checkout(&root, "repo", "one\n");
    let daemon = start(&root);
    let mut client = connect(&daemon);
    let board = board_of(&mut client, &repo).id;
    run_on(&daemon, "r-live", &board, RunState::Running {});

    let (error, message) = refused(&mut client, "board.remove", &json!({ "board": board }));

    assert_eq!(error, code::CONFLICT);
    assert!(message.contains("r-live"), "{message}");
    assert!(message.contains("in progress"), "{message}");
    assert_eq!(
        listed_ids(&mut client),
        std::slice::from_ref(&board),
        "still listed"
    );
    let run: Value = call(&mut client, "run.get", &json!({ "run": "r-live" }));
    assert_eq!(run["summary"]["state"]["state"], "running", "{run}");

    let ended = RunState::Ended {
        outcome: NonEmpty::new("done").unwrap(),
    };
    daemon
        .phases()
        .append_event(
            Actor {
                kind: ActorKind::Workflow,
                run: Some("r-live".to_string()),
                phase: None,
            },
            EventBody::RunTransition {
                run: "r-live".to_string(),
                from: Some(RunState::Running {}),
                to: ended,
            },
        )
        .unwrap();
    remove(&mut client, &board);
    assert!(list(&mut client).is_empty(), "removed once the run ended");
    daemon.stop();
}

#[test]
fn a_run_on_another_board_does_not_refuse_the_removal() {
    let root = TempRoot::new();
    let first = checkout(&root, "first", "one\n");
    let second = checkout(&root, "second", "two\n");
    let daemon = start(&root);
    let mut client = connect(&daemon);
    let one = board_of(&mut client, &first).id;
    let two = board_of(&mut client, &second).id;
    run_on(&daemon, "r-live", &two, RunState::Running {});

    remove(&mut client, &one);

    assert_eq!(listed_ids(&mut client), [two]);
    daemon.stop();
}

#[test]
fn a_defaults_connection_is_refused_naming_the_mode_and_the_board_stays() {
    let root = TempRoot::new();
    let repo = checkout(&root, "repo", "one\n");
    let daemon = start(&root);
    let mut normal = connect(&daemon);
    let board = board_of(&mut normal, &repo).id;
    let mut defaults = OwnerClient::connect_as(daemon.paths(), "defaults-remove-test", true)
        .expect("the defaults client connects");

    let (error, message) = refused(&mut defaults, "board.remove", &json!({ "board": board }));

    assert_eq!(error, code::NOT_PERMITTED);
    assert!(message.contains("defaults"), "{message}");
    assert!(message.contains("board.remove"), "{message}");
    assert_eq!(listed_ids(&mut normal), [board]);
    daemon.stop();
}

#[test]
fn a_board_the_daemon_does_not_list_is_refused_by_name() {
    let root = TempRoot::new();
    let repo = checkout(&root, "repo", "one\n");
    let daemon = start(&root);
    let mut client = connect(&daemon);
    let board = board_of(&mut client, &repo).id;

    let (error, message) = refused(
        &mut client,
        "board.remove",
        &json!({ "board": "board-nope" }),
    );

    assert_eq!(error, code::NOT_FOUND);
    assert!(message.contains("board-nope"), "{message}");
    assert_eq!(listed_ids(&mut client), [board]);
    daemon.stop();
}

#[test]
fn a_board_that_was_already_removed_is_refused_and_says_so() {
    let root = TempRoot::new();
    let repo = checkout(&root, "repo", "one\n");
    let daemon = start(&root);
    let mut client = connect(&daemon);
    let board = board_of(&mut client, &repo).id;
    remove(&mut client, &board);

    let (error, message) = refused(&mut client, "board.remove", &json!({ "board": board }));

    assert_eq!(error, code::NOT_FOUND);
    assert!(message.contains(&board), "{message}");
    assert!(message.contains("already removed"), "{message}");
    daemon.stop();
}

// ---- every window learns ------------------------------------------------------------------------------------------

#[test]
fn every_subscribed_window_is_told_which_board_was_removed() {
    let root = TempRoot::new();
    let first = checkout(&root, "first", "one\n");
    let second = checkout(&root, "second", "two\n");
    let daemon = start(&root);
    let mut client = connect(&daemon);
    let one = board_of(&mut client, &first).id;
    board_of(&mut client, &second);
    let mut windows = [
        Stream::connect(daemon.paths()),
        Stream::connect(daemon.paths()),
    ];
    for window in &mut windows {
        window.subscribe(None);
    }

    remove(&mut client, &one);

    for window in &mut windows {
        let told = loop {
            match window.next(DEADLINE) {
                Next::Notice(Notice::BoardChanged(changed)) => break changed.removed,
                Next::Notice(_) => {}
                other => panic!("no board.changed came: {other:?}"),
            }
        };
        assert_eq!(told, std::slice::from_ref(&one));
    }
    daemon.stop();
}

#[test]
fn a_refused_removal_tells_no_window() {
    let root = TempRoot::new();
    let repo = checkout(&root, "repo", "one\n");
    let daemon = start(&root);
    let mut client = connect(&daemon);
    let board = board_of(&mut client, &repo).id;
    run_on(&daemon, "r-live", &board, RunState::Running {});
    let mut window = Stream::connect(daemon.paths());
    window.subscribe(None);
    refused(&mut client, "board.remove", &json!({ "board": board }));

    // A notice that came for the refusal would be in the stream before the next ping's answer.
    let _: Value = window.call("daemon.status", json!({}));
    loop {
        match window.next(Duration::from_millis(300)) {
            Next::Notice(Notice::BoardChanged(changed)) => {
                panic!("a refusal told a window: {changed:?}")
            }
            Next::Notice(_) => {}
            Next::Quiet | Next::Closed => break,
        }
    }
    daemon.stop();
}
