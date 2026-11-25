//! A board's name and where it lives (ts-0191-nv; 0.2, After a restart, Boards and lines): a new board has a random
//! name that no earlier folder of the user's has, a linked worktree opens the board of its main checkout, a board whose
//! folder is gone is listed `missing`, and `board.locate` points it at the folder again without deleting anything. A
//! daemon runs in the test's own process over fixture repositories in a temp root, which is also the home. Every git
//! command clears its environment and reads no configuration of the user's. Nothing here touches the user's home, a
//! real daemon or `launchd`.

mod common;

use collide_core::owner::{
    BoardListParams, BoardListResult, BoardLocateParams, BoardLocateResult, BoardOpenParams,
    BoardOpenResult, BoardSummary, TrustState, code,
};
use collide_host::base_env::CaptureOptions;
use collide_host::board_lines::Lines;
use collide_host::daemon::{Daemon, DaemonOptions, DaemonPaths};
use collide_host::owner_client::{ClientError, OwnerClient};
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

// ---- the fixtures -------------------------------------------------------------------------------------------------

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
    options.load_context = Some(common::config_fixture::plain_factory(root.path()));
    Daemon::start(options).expect("the daemon starts")
}

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

/// A checkout named `name` below the root, on `main`, with one commit that holds `content`. Two repositories made with
/// the same content in the same second would share a root commit, so a test that needs two says so in the content.
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
    OwnerClient::connect(daemon.paths(), "identity-test").expect("the client connects")
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

fn board_of(client: &mut OwnerClient, folder: &Path) -> BoardSummary {
    open(client, folder)
        .board
        .expect("the folder opens as a board")
}

fn list(client: &mut OwnerClient) -> Vec<BoardSummary> {
    call::<_, BoardListResult>(client, "board.list", &BoardListParams {}).boards
}

fn listed(client: &mut OwnerClient, board: &str) -> BoardSummary {
    list(client)
        .into_iter()
        .find(|each| each.id == board)
        .unwrap_or_else(|| panic!("{board} is not listed"))
}

fn locate_params(board: &str, path: &Path) -> BoardLocateParams {
    BoardLocateParams {
        board: board.to_string(),
        path: path.to_str().unwrap().to_string(),
    }
}

fn locate(client: &mut OwnerClient, board: &str, path: &Path) -> BoardLocateResult {
    call(client, "board.locate", &locate_params(board, path))
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

/// The title of the board's first scratch task, or `None` when it has none.
fn first_task(client: &mut OwnerClient, board: &str) -> Option<String> {
    let params = json!({ "board": board, "id": "scratch:1" });
    let answer: Value = call(client, "task.get", &params);
    answer["task"]["title"].as_str().map(str::to_string)
}

/// Lists the board's tasks, which starts its corpus, and gives how many there are once it has read them.
fn task_count(client: &mut OwnerClient, board: &str) -> usize {
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
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

fn pack_data(root: &TempRoot) -> PathBuf {
    DaemonPaths::under(root.path())
        .runs_dir
        .with_file_name("pack-data")
}

/// A board that was opened by an earlier build, as the store holds it: no record of its repository.
fn old_board(root: &TempRoot, folder: &Path) -> String {
    let store = Store::open(&DaemonPaths::under(root.path()).store_dir).unwrap();
    let id = store
        .open_board(folder.to_str().unwrap(), &|_| false)
        .unwrap();
    assert_eq!(store.root_commits(&id).unwrap(), None);
    id
}

fn stored(root: &TempRoot) -> Vec<collide_host::scratch_store::StoredBoard> {
    Store::open(&DaemonPaths::under(root.path()).store_dir)
        .unwrap()
        .boards()
        .unwrap()
}

// ---- the name ---------------------------------------------------------------------------------------------------

#[test]
fn a_new_board_is_named_with_six_random_letters_and_the_same_root_gives_the_same_name() {
    let root = TempRoot::new();
    let first = checkout(&root, "first", "one\n");
    let second = checkout(&root, "second", "two\n");
    let daemon = start(&root);
    let mut client = connect(&daemon);

    let one = board_of(&mut client, &first).id;
    let two = board_of(&mut client, &second).id;
    let again = board_of(&mut client, &first).id;

    let letters = |id: &str| {
        id.strip_prefix("board-")
            .is_some_and(|rest| rest.len() == 6 && rest.chars().all(|c| c.is_ascii_lowercase()))
    };
    assert!(letters(&one) && letters(&two), "{one} {two}");
    assert_ne!(one, two);
    assert_eq!(again, one);
    daemon.stop();
}

#[test]
fn a_name_taken_in_the_store_or_on_disk_is_never_given_to_a_new_board() {
    let root = TempRoot::new();
    let store = Store::open(&root.join("store")).unwrap();
    let mut letters = ["aaaaaa", "aaaaaa", "bbbbbb", "cccccc"].into_iter();
    let mut next = || letters.next().unwrap().to_string();
    let on_disk = |name: &str| name == "bbbbbb" || name == "board-bbbbbb";

    let first = store
        .open_board_with("/one", &|_| false, &mut next)
        .unwrap();
    // `aaaaaa` is the store's, `bbbbbb` is a folder of an earlier store's views.
    let second = store.open_board_with("/two", &on_disk, &mut next).unwrap();

    assert_eq!(first, "board-aaaaaa");
    assert_eq!(second, "board-cccccc");
}

#[test]
fn a_board_folder_of_an_earlier_store_is_a_name_the_lines_say_is_taken() {
    let root = TempRoot::new();
    let home = root.path().to_path_buf();
    let lines = Lines::new(
        Some(home.clone()),
        common::config_fixture::plain_factory(root.path()),
        Vec::new(),
        Arc::new(Vec::new),
    );
    fs::create_dir_all(home.join(".config/collide/boards/board-abcdef")).unwrap();

    assert!(lines.board_folder_exists("board-abcdef"));
    assert!(!lines.board_folder_exists("board-ghijkl"));
}

#[test]
fn a_store_that_cannot_give_a_free_name_says_so_and_adds_no_board() {
    let root = TempRoot::new();
    let store = Store::open(&root.join("store")).unwrap();

    let error = store
        .open_board_with("/one", &|_| true, &mut || "zzzzzz".to_string())
        .unwrap_err()
        .to_string();

    assert!(error.contains("no free board name"), "{error}");
    assert!(store.boards().unwrap().is_empty());
}

// ---- a linked worktree --------------------------------------------------------------------------------------------

fn worktree(root: &TempRoot, main: &Path, name: &str, branch: &str) -> PathBuf {
    let dir = root.join(name);
    git(
        root,
        main,
        &["worktree", "add", "-q", "-b", branch, dir.to_str().unwrap()],
    );
    fs::canonicalize(dir).unwrap()
}

#[test]
fn a_linked_worktree_opens_the_board_of_the_main_checkout_with_itself_as_a_line() {
    let root = TempRoot::new();
    let main = checkout(&root, "main-checkout", "main\n");
    let linked = worktree(&root, &main, "linked", "feature");
    let daemon = start(&root);
    let mut client = connect(&daemon);

    let board = board_of(&mut client, &linked);

    assert_eq!(board.root, main.to_str().unwrap());
    let lines: Vec<(&str, &str)> = board
        .lines
        .iter()
        .map(|line| (line.ref_name.as_str(), line.root.as_str()))
        .collect();
    assert_eq!(
        lines,
        [
            ("main", main.to_str().unwrap()),
            ("feature", linked.to_str().unwrap())
        ]
    );
    daemon.stop();
}

#[test]
fn opening_the_main_checkout_after_its_worktree_finds_the_same_board() {
    let root = TempRoot::new();
    let main = checkout(&root, "main-checkout", "main\n");
    let linked = worktree(&root, &main, "linked", "feature");
    let daemon = start(&root);
    let mut client = connect(&daemon);

    let from_worktree = board_of(&mut client, &linked).id;
    let from_main = board_of(&mut client, &main);

    assert_eq!(from_main.id, from_worktree);
    assert_eq!(list(&mut client).len(), 1);
    // Asked for from the main checkout, the board has the one line it is: the main checkout's.
    assert_eq!(from_main.lines.len(), 1);
    daemon.stop();
}

#[test]
fn opening_the_worktree_after_the_main_checkout_finds_the_same_board() {
    let root = TempRoot::new();
    let main = checkout(&root, "main-checkout", "main\n");
    let linked = worktree(&root, &main, "linked", "feature");
    let daemon = start(&root);
    let mut client = connect(&daemon);

    let from_main = board_of(&mut client, &main).id;
    let from_worktree = board_of(&mut client, &linked);

    assert_eq!(from_worktree.id, from_main);
    assert_eq!(from_worktree.root, main.to_str().unwrap());
    assert_eq!(from_worktree.lines.len(), 2);
    daemon.stop();
}

#[test]
fn a_board_that_has_a_worktree_as_its_root_is_left_as_it_is_and_the_main_checkout_is_another_board()
{
    let root = TempRoot::new();
    let main = checkout(&root, "main-checkout", "main\n");
    let linked = worktree(&root, &main, "linked", "feature");
    // An earlier build rooted a board at the worktree.
    let old = old_board(&root, &linked);
    let daemon = start(&root);
    let mut client = connect(&daemon);

    let found = listed(&mut client, &old);
    let main_board = board_of(&mut client, &main);

    assert_eq!(found.root, linked.to_str().unwrap());
    assert_ne!(main_board.id, old);
    daemon.stop();
}

// ---- a missing board ----------------------------------------------------------------------------------------------

#[test]
fn a_board_whose_folder_moved_is_listed_missing_and_nothing_is_deleted() {
    let root = TempRoot::new();
    let repo = checkout(&root, "repo", "one\n");
    let daemon = start(&root);
    let mut client = connect(&daemon);
    let board = board_of(&mut client, &repo);
    assert!(!board.missing);
    create_task(&mut client, &board.id, "Outlives the folder");
    let data = pack_data(&root).join(&board.id).join("some-pack");
    fs::create_dir_all(&data).unwrap();
    fs::write(data.join("state.json"), "{}").unwrap();

    let moved = root.join("moved");
    fs::rename(&repo, &moved).unwrap();
    let found = listed(&mut client, &board.id);

    assert!(found.missing);
    assert_eq!(found.root, repo.to_str().unwrap());
    assert!(found.lines.is_empty());
    assert_eq!(
        first_task(&mut client, &board.id).as_deref(),
        Some("Outlives the folder")
    );
    assert!(data.join("state.json").exists());
    assert!(moved.is_dir());
    daemon.stop();
}

#[test]
fn a_board_with_its_folder_is_not_missing() {
    let root = TempRoot::new();
    let repo = checkout(&root, "repo", "one\n");
    let daemon = start(&root);
    let mut client = connect(&daemon);
    let board = board_of(&mut client, &repo);

    assert!(!listed(&mut client, &board.id).missing);
    daemon.stop();
}

// ---- locate -------------------------------------------------------------------------------------------------------

#[test]
fn locate_keeps_the_name_the_tasks_the_trust_the_views_and_the_pack_data() {
    let root = TempRoot::new();
    let repo = checkout(&root, "repo", "one\n");
    let daemon = start(&root);
    let mut client = connect(&daemon);
    let board = board_of(&mut client, &repo);
    create_task(&mut client, &board.id, "Keep me");
    // The board's corpus is running at the old root.
    assert_eq!(task_count(&mut client, &board.id), 1);
    let hash = match &board.lines[0].trust {
        TrustState::Untrusted { hash } | TrustState::Trusted { hash } => hash.clone(),
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
    let data = pack_data(&root).join(&board.id).join("some-pack");
    fs::create_dir_all(&data).unwrap();
    fs::write(data.join("state.json"), "{}").unwrap();
    let moved = root.join("moved");
    fs::rename(&repo, &moved).unwrap();
    let moved = fs::canonicalize(moved).unwrap();
    assert!(listed(&mut client, &board.id).missing);

    let located = locate(&mut client, &board.id, &moved);

    assert_eq!(located.note, None);
    assert_eq!(located.board.id, board.id);
    assert_eq!(located.board.root, moved.to_str().unwrap());
    assert!(!located.board.missing);
    assert!(matches!(
        located.board.lines[0].trust,
        TrustState::Trusted { .. }
    ));
    let again = listed(&mut client, &board.id);
    assert!(!again.missing);
    assert_eq!(again.root, moved.to_str().unwrap());
    assert!(matches!(again.lines[0].trust, TrustState::Trusted { .. }));
    assert_eq!(
        first_task(&mut client, &board.id).as_deref(),
        Some("Keep me")
    );
    assert!(view_names(&mut client, &board.id).contains(&"mine".to_string()));
    assert!(data.join("state.json").exists());
    // The corpus was started again at the new root, and lists the same task.
    assert_eq!(task_count(&mut client, &board.id), 1);
    // Opening the new folder finds the same board, so no second one was made.
    assert_eq!(board_of(&mut client, &moved).id, board.id);
    assert_eq!(list(&mut client).len(), 1);
    daemon.stop();
}

#[test]
fn locate_survives_a_daemon_restart() {
    let root = TempRoot::new();
    let repo = checkout(&root, "repo", "one\n");
    let moved = root.join("moved");
    let board = {
        let daemon = start(&root);
        let mut client = connect(&daemon);
        let board = board_of(&mut client, &repo);
        fs::rename(&repo, &moved).unwrap();
        let moved = fs::canonicalize(&moved).unwrap();
        locate(&mut client, &board.id, &moved);
        drop(client);
        daemon.stop();
        board
    };

    let daemon = start(&root);
    let mut client = connect(&daemon);
    let found = listed(&mut client, &board.id);

    assert_eq!(
        found.root,
        fs::canonicalize(&moved).unwrap().to_str().unwrap()
    );
    assert!(!found.missing);
    daemon.stop();
}

#[test]
fn locate_refuses_a_different_repository_with_the_reason_and_the_board_stays_missing() {
    let root = TempRoot::new();
    let repo = checkout(&root, "repo", "one\n");
    let other = checkout(&root, "other", "something else entirely\n");
    let daemon = start(&root);
    let mut client = connect(&daemon);
    let board = board_of(&mut client, &repo);
    create_task(&mut client, &board.id, "Stays");
    fs::rename(&repo, root.join("moved")).unwrap();

    let (error, message) = refused(
        &mut client,
        "board.locate",
        &locate_params(&board.id, &other),
    );

    assert_eq!(error, code::CONFLICT);
    assert!(message.contains("different repository"), "{message}");
    let found = listed(&mut client, &board.id);
    assert!(found.missing);
    assert_eq!(found.root, repo.to_str().unwrap());
    assert_eq!(first_task(&mut client, &board.id).as_deref(), Some("Stays"));
    assert!(other.is_dir() && root.join("moved").is_dir());
    daemon.stop();
}

#[test]
fn locate_refuses_a_folder_that_is_another_boards_root() {
    let root = TempRoot::new();
    let repo = checkout(&root, "repo", "one\n");
    let daemon = start(&root);
    let mut client = connect(&daemon);
    let first = board_of(&mut client, &repo);
    let moved = root.join("moved");
    fs::rename(&repo, &moved).unwrap();
    let moved = fs::canonicalize(moved).unwrap();
    let second = board_of(&mut client, &moved);
    assert_ne!(first.id, second.id);

    let (error, message) = refused(
        &mut client,
        "board.locate",
        &locate_params(&first.id, &moved),
    );

    assert_eq!(error, code::CONFLICT);
    assert!(message.contains(&second.id), "{message}");
    assert!(listed(&mut client, &first.id).missing);
    daemon.stop();
}

#[test]
fn locate_refuses_a_folder_that_is_not_a_checkout_and_a_board_the_daemon_does_not_have() {
    let root = TempRoot::new();
    let repo = checkout(&root, "repo", "one\n");
    let plain = root.join("plain");
    fs::create_dir_all(&plain).unwrap();
    let daemon = start(&root);
    let mut client = connect(&daemon);
    let board = board_of(&mut client, &repo);

    let (not_checkout, _) = refused(
        &mut client,
        "board.locate",
        &locate_params(&board.id, &plain),
    );
    let (no_board, _) = refused(
        &mut client,
        "board.locate",
        &locate_params("board-nope", &repo),
    );
    let (relative, _) = refused(
        &mut client,
        "board.locate",
        &json!({ "board": board.id, "path": "relative/path" }),
    );

    assert_eq!(not_checkout, code::INVALID_PARAMS);
    assert_eq!(no_board, code::NOT_FOUND);
    assert_eq!(relative, code::INVALID_PARAMS);
    daemon.stop();
}

#[test]
fn locate_to_a_folder_inside_the_moved_checkout_roots_the_board_at_its_top_level() {
    let root = TempRoot::new();
    let repo = checkout(&root, "repo", "one\n");
    let daemon = start(&root);
    let mut client = connect(&daemon);
    let board = board_of(&mut client, &repo);
    let moved = root.join("moved");
    fs::rename(&repo, &moved).unwrap();
    let moved = fs::canonicalize(moved).unwrap();
    fs::create_dir_all(moved.join("sub")).unwrap();

    let located = locate(&mut client, &board.id, &moved.join("sub"));

    assert_eq!(located.board.root, moved.to_str().unwrap());
    daemon.stop();
}

#[test]
fn a_board_with_no_record_of_its_repository_is_located_with_a_note_that_the_check_could_not_run() {
    let root = TempRoot::new();
    let repo = checkout(&root, "repo", "one\n");
    let moved = root.join("moved");
    fs::rename(&repo, &moved).unwrap();
    let moved = fs::canonicalize(moved).unwrap();
    // The board was opened by a build before records were kept, and its folder is gone before the daemon sees it.
    let id = old_board(&root, &repo);
    let daemon = start(&root);
    let mut client = connect(&daemon);
    assert!(listed(&mut client, &id).missing);

    let located = locate(&mut client, &id, &moved);

    let note = located.note.expect("a note says the check could not run");
    assert!(note.contains("no record"), "{note}");
    assert_eq!(located.board.root, moved.to_str().unwrap());
    // The new folder's commits are recorded now, so the next locate is checked.
    assert!(stored(&root)[0].root_commits.is_some());
    daemon.stop();
}

#[test]
fn a_board_whose_folder_is_there_gets_its_record_when_it_is_listed() {
    let root = TempRoot::new();
    let repo = checkout(&root, "repo", "one\n");
    let id = old_board(&root, &repo);
    let daemon = start(&root);
    let mut client = connect(&daemon);

    assert!(!listed(&mut client, &id).missing);

    let expected = git(&root, &repo, &["rev-list", "--max-parents=0", "HEAD"]);
    assert_eq!(stored(&root)[0].root_commits, Some(vec![expected]));
    daemon.stop();
}

#[test]
fn the_record_of_a_repository_with_two_root_commits_is_both_and_sharing_one_is_the_same_repository()
{
    let root = TempRoot::new();
    let repo = checkout(&root, "repo", "one\n");
    // Merged histories: a second root commit joined in.
    git(&root, &repo, &["checkout", "-q", "--orphan", "other"]);
    fs::write(repo.join("other.txt"), "other\n").unwrap();
    git(&root, &repo, &["add", "."]);
    git(&root, &repo, &["commit", "-q", "-m", "second root"]);
    git(&root, &repo, &["checkout", "-q", "main"]);
    git(
        &root,
        &repo,
        &[
            "merge",
            "-q",
            "--allow-unrelated-histories",
            "-m",
            "join",
            "other",
        ],
    );
    let daemon = start(&root);
    let mut client = connect(&daemon);
    let board = board_of(&mut client, &repo);
    assert_eq!(
        stored(&root)[0].root_commits.as_ref().map(Vec::len),
        Some(2)
    );
    // The moved checkout is on the branch that has only the second root commit: it shares one, so it is the same.
    let moved = root.join("moved");
    fs::rename(&repo, &moved).unwrap();
    let moved = fs::canonicalize(moved).unwrap();
    git(&root, &moved, &["checkout", "-q", "other"]);

    let located = locate(&mut client, &board.id, &moved);

    assert_eq!(located.note, None);
    daemon.stop();
}

#[test]
fn a_repository_with_no_commit_records_nothing_and_is_not_taken_as_the_same_repository() {
    let root = TempRoot::new();
    let repo = checkout(&root, "repo", "one\n");
    let empty = root.join("empty");
    fs::create_dir_all(&empty).unwrap();
    git(&root, &empty, &["init", "-q", "-b", "main"]);
    let empty = fs::canonicalize(empty).unwrap();
    let daemon = start(&root);
    let mut client = connect(&daemon);
    let board = board_of(&mut client, &repo);

    let (error, message) = refused(
        &mut client,
        "board.locate",
        &locate_params(&board.id, &empty),
    );
    let nothing = board_of(&mut client, &empty);

    assert_eq!(error, code::CONFLICT);
    assert!(message.contains("no commit yet"), "{message}");
    assert!(nothing.lines.is_empty());
    let records: Vec<_> = stored(&root)
        .into_iter()
        .map(|b| b.root_commits.is_some())
        .collect();
    assert_eq!(records, [true, false]);
    daemon.stop();
}
