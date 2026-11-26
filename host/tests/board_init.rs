//! `board.init` (ts-0102-vu, ts-0194-qp; 0.11, Share with team). A daemon runs in the test's own process over a fixture
//! checkout in a temp root, so nothing outside it is read or written. The call writes `.collide/board.yaml` and a copy
//! of the person's saved views of the board, and nothing else: no commit, no `.gitignore`, no lock, no `config.yaml`.
//! It records the hash of what it wrote as trusted for the person, so their own commit asks nothing.

mod common;

use collide_core::board::{Board, check_board};
use collide_core::owner::{
    BoardInitParams, BoardInitResult, BoardListParams, BoardListResult, BoardOpenParams,
    BoardOpenResult, RpcError, TrustState, code,
};
use collide_core::yaml::load_yaml_file;
use collide_host::owner_client::ClientError;
use common::config_fixture::Fixture;
use common::git_repo::git_command;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::time::Duration;

fn init(fixture: &Fixture, board: &str, dry_run: bool) -> Result<BoardInitResult, RpcError> {
    let params = BoardInitParams {
        board: board.to_string(),
        dry_run,
    };
    match fixture
        .client()
        .call("board.init", &params, Some(Duration::from_secs(10)))
    {
        Ok(result) => Ok(result),
        Err(ClientError::Rpc(error)) => Err(error),
        Err(other) => panic!("{other}"),
    }
}

#[test]
fn a_dry_run_lists_the_file_and_writes_nothing() {
    let fixture = Fixture::new();
    let board = fixture.board();
    let result = init(&fixture, &board, true).unwrap();
    assert_eq!(result.files, vec![".collide/board.yaml".to_string()]);
    assert!(!result.written);
    assert!(!fixture.repo.work().join(".collide").exists());
}

#[test]
fn init_writes_one_file_that_loads_with_the_board_check() {
    let fixture = Fixture::new();
    let board = fixture.board();
    let result = init(&fixture, &board, false).unwrap();
    assert_eq!(result.files, vec![".collide/board.yaml".to_string()]);
    assert!(result.written);
    let path = fixture.repo.work().join(".collide/board.yaml");
    let loaded: Board = load_yaml_file(&path).unwrap();
    assert_eq!(check_board(&loaded), Ok(()));
    assert!(loaded.packs.is_empty());
}

#[test]
fn the_file_starts_with_a_comment_and_says_packs_is_empty() {
    let fixture = Fixture::new();
    let board = fixture.board();
    init(&fixture, &board, false).unwrap();
    let text = fs::read_to_string(fixture.repo.work().join(".collide/board.yaml")).unwrap();
    assert!(text.starts_with('#'), "{text}");
    assert!(text.lines().any(|line| line == "packs: []"), "{text}");
    assert!(text.lines().all(|line| line.len() <= 120), "{text}");
}

#[test]
fn init_makes_no_other_file_and_commits_nothing() {
    let fixture = Fixture::new();
    let board = fixture.board();
    let before = fixture.repo.git(&["rev-parse", "HEAD"]);
    init(&fixture, &board, false).unwrap();
    assert_eq!(fixture.repo.git(&["rev-parse", "HEAD"]), before);
    assert_eq!(
        fixture.repo.git(&["status", "--porcelain", "-uall"]).trim(),
        "?? .collide/board.yaml"
    );
    assert!(!fixture.repo.work().join(".gitignore").exists());
    let made: Vec<_> = fs::read_dir(fixture.repo.work().join(".collide"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert_eq!(made, vec![std::ffi::OsString::from("board.yaml")]);
}

#[test]
fn a_second_init_refuses_and_names_the_folder() {
    let fixture = Fixture::new();
    let board = fixture.board();
    init(&fixture, &board, false).unwrap();
    let error = init(&fixture, &board, false).unwrap_err();
    assert_eq!(error.code, code::CONFLICT);
    assert!(error.message.contains(".collide"), "{}", error.message);
}

#[test]
fn a_dry_run_refuses_too_when_the_folder_is_there() {
    let fixture = Fixture::new();
    let board = fixture.board();
    fs::create_dir(fixture.repo.work().join(".collide")).unwrap();
    let error = init(&fixture, &board, true).unwrap_err();
    assert_eq!(error.code, code::CONFLICT);
}

#[test]
fn a_collide_pointer_file_refuses_and_is_left_as_it_was() {
    let fixture = Fixture::new();
    let board = fixture.board();
    let pointer = fixture.repo.work().join(".collide");
    fs::write(&pointer, "../elsewhere\n").unwrap();
    let error = init(&fixture, &board, false).unwrap_err();
    assert_eq!(error.code, code::CONFLICT);
    assert!(error.message.contains(".collide"), "{}", error.message);
    assert_eq!(fs::read_to_string(&pointer).unwrap(), "../elsewhere\n");
}

#[test]
fn a_board_the_daemon_does_not_know_is_not_found() {
    let fixture = Fixture::new();
    let error = init(&fixture, "board-99", false).unwrap_err();
    assert_eq!(error.code, code::NOT_FOUND);
}

/// Puts the folder's mode back, so the temp root can be removed.
struct Restore(std::path::PathBuf);

impl Drop for Restore {
    fn drop(&mut self) {
        let _ = fs::set_permissions(&self.0, fs::Permissions::from_mode(0o755));
    }
}

#[test]
fn a_root_that_cannot_be_written_is_an_error_that_names_the_path() {
    let fixture = Fixture::new();
    let board = fixture.board();
    let work = fixture.repo.work().to_path_buf();
    let _restore = Restore(work.clone());
    fs::set_permissions(&work, fs::Permissions::from_mode(0o555)).unwrap();
    let error = init(&fixture, &board, false).unwrap_err();
    assert_eq!(error.code, code::INTERNAL_ERROR);
    assert!(error.message.contains(".collide"), "{}", error.message);
    assert!(!work.join(".collide/board.yaml").exists());
}

// ---- the person's saved views (ts-0194-qp) ----

const MINE: &str = "name: mine\nwhere: \"status == 'ready'\"\n";
const OTHER: &str = "name: other\nwhere: \"true\"\n";

/// Saves a view of the person's own for `board`, as `task.save_view` does.
fn personal_view(fixture: &Fixture, board: &str, file: &str, text: &str) {
    fixture.write_user(&format!("boards/{board}/views/{file}"), text);
}

fn work_files(fixture: &Fixture, dir: &str) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(fixture.repo.work().join(dir))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    names
}

#[test]
fn a_dry_run_lists_the_board_file_and_each_saved_view_and_writes_nothing() {
    let fixture = Fixture::new();
    let board = fixture.board();
    personal_view(&fixture, &board, "other.yaml", OTHER);
    personal_view(&fixture, &board, "mine.yaml", MINE);
    let result = init(&fixture, &board, true).unwrap();
    assert_eq!(
        result.files,
        vec![
            ".collide/board.yaml".to_string(),
            ".collide/views/mine.yaml".to_string(),
            ".collide/views/other.yaml".to_string(),
        ]
    );
    assert!(!result.written);
    assert!(!fixture.repo.work().join(".collide").exists());
}

#[test]
fn init_copies_the_saved_views_of_this_board_byte_for_byte_and_leaves_the_originals() {
    let fixture = Fixture::new();
    let board = fixture.board();
    personal_view(&fixture, &board, "mine.yaml", MINE);
    let result = init(&fixture, &board, false).unwrap();
    assert!(result.written);
    assert_eq!(result.files.len(), 2, "{:?}", result.files);
    let copy = fixture.repo.work().join(".collide/views/mine.yaml");
    assert_eq!(fs::read_to_string(&copy).unwrap(), MINE);
    assert_eq!(
        fs::metadata(&copy).unwrap().permissions().mode() & 0o777,
        0o644
    );
    let original = fixture
        .home()
        .join(format!(".config/collide/boards/{board}/views/mine.yaml"));
    assert_eq!(fs::read_to_string(original).unwrap(), MINE);
    assert_eq!(work_files(&fixture, ".collide"), ["board.yaml", "views"]);
}

#[test]
fn a_view_saved_as_slow_spots_is_shared_under_its_made_file_name_with_its_name_kept() {
    let fixture = Fixture::new();
    let board = fixture.board();
    let text = "name: Slow spots\ninclude: [plan:1]\n";
    personal_view(&fixture, &board, "slow-spots.yaml", text);
    let result = init(&fixture, &board, false).unwrap();
    assert!(
        result
            .files
            .contains(&".collide/views/slow-spots.yaml".to_string()),
        "{:?}",
        result.files
    );
    let copy = fixture.repo.work().join(".collide/views/slow-spots.yaml");
    assert_eq!(fs::read_to_string(copy).unwrap(), text);
}

#[test]
fn another_boards_views_a_note_and_a_link_are_not_copied() {
    let fixture = Fixture::new();
    let board = fixture.board();
    personal_view(&fixture, "board-99", "theirs.yaml", OTHER);
    personal_view(&fixture, &board, "notes.txt", "not a view\n");
    personal_view(&fixture, &board, "mine.yaml", MINE);
    let elsewhere = fixture.home().join("elsewhere.yaml");
    fs::write(&elsewhere, OTHER).unwrap();
    let folder = fixture
        .home()
        .join(format!(".config/collide/boards/{board}/views"));
    std::os::unix::fs::symlink(&elsewhere, folder.join("link.yaml")).unwrap();
    let result = init(&fixture, &board, false).unwrap();
    assert_eq!(
        result.files,
        vec![
            ".collide/board.yaml".to_string(),
            ".collide/views/mine.yaml".to_string()
        ]
    );
    assert_eq!(work_files(&fixture, ".collide/views"), ["mine.yaml"]);
}

#[test]
fn a_view_over_the_yaml_cap_is_refused_and_nothing_is_made() {
    let fixture = Fixture::new();
    let board = fixture.board();
    personal_view(
        &fixture,
        &board,
        "big.yaml",
        &format!("name: big\n# {}\n", "x".repeat(300_000)),
    );
    let error = init(&fixture, &board, false).unwrap_err();
    assert_eq!(error.code, code::INTERNAL_ERROR);
    assert!(error.message.contains("big.yaml"), "{}", error.message);
    assert!(!fixture.repo.work().join(".collide").exists());
}

// ---- trusting what was written (ts-0194-qp) ----

fn trust_of(fixture: &Fixture, root: &std::path::Path) -> TrustState {
    let boards: BoardListResult = fixture
        .client()
        .call(
            "board.list",
            &BoardListParams {},
            Some(Duration::from_secs(20)),
        )
        .unwrap();
    let root = fs::canonicalize(root)
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    let board = boards
        .boards
        .iter()
        .find(|board| board.root == root)
        .expect("the board is listed");
    assert!(
        board.lines[0].problems.is_empty(),
        "{:?}",
        board.lines[0].problems
    );
    board.lines[0].trust.clone()
}

fn is_trusted(state: &TrustState) -> bool {
    matches!(state, TrustState::Trusted { .. })
}

#[test]
fn what_init_wrote_is_trusted_once_the_person_commits_exactly_that() {
    let fixture = Fixture::new();
    let board = fixture.board();
    personal_view(&fixture, &board, "mine.yaml", MINE);
    let result = init(&fixture, &board, false).unwrap();
    assert!(result.trusted);
    // Nothing is committed yet, so the line is as it was: it holds no Collide folder, and nothing to trust.
    assert!(!is_trusted(&trust_of(&fixture, fixture.repo.work())));
    fixture.commit();
    assert!(
        is_trusted(&trust_of(&fixture, fixture.repo.work())),
        "the person's own commit asks nothing"
    );
}

#[test]
fn a_board_file_alone_is_trusted_the_same_way() {
    let fixture = Fixture::new();
    let board = fixture.board();
    assert!(init(&fixture, &board, false).unwrap().trusted);
    fixture.commit();
    assert!(is_trusted(&trust_of(&fixture, fixture.repo.work())));
}

#[test]
fn a_commit_that_adds_anything_else_asks() {
    let fixture = Fixture::new();
    let board = fixture.board();
    personal_view(&fixture, &board, "mine.yaml", MINE);
    init(&fixture, &board, false).unwrap();
    fixture.write(".collide/views/extra.yaml", OTHER);
    fixture.commit();
    assert!(!is_trusted(&trust_of(&fixture, fixture.repo.work())));
}

#[test]
fn a_commit_that_changes_a_written_file_asks() {
    let fixture = Fixture::new();
    let board = fixture.board();
    personal_view(&fixture, &board, "mine.yaml", MINE);
    init(&fixture, &board, false).unwrap();
    fixture.write(".collide/views/mine.yaml", "name: mine\nwhere: \"true\"\n");
    fixture.commit();
    assert!(!is_trusted(&trust_of(&fixture, fixture.repo.work())));
}

#[test]
fn a_clone_of_the_shared_repo_is_not_trusted() {
    let fixture = Fixture::new();
    let board = fixture.board();
    personal_view(&fixture, &board, "mine.yaml", MINE);
    init(&fixture, &board, false).unwrap();
    fixture.commit();
    let clone = fixture.home().join("clone");
    let source = fixture.repo.work().to_str().unwrap().to_string();
    let made = git_command(fixture.home(), fixture.home())
        .args(["clone", "--quiet", &source, clone.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        made.status.success(),
        "{}",
        String::from_utf8_lossy(&made.stderr)
    );
    let opened: BoardOpenResult = fixture
        .client()
        .call(
            "board.open",
            &BoardOpenParams {
                root: fs::canonicalize(&clone)
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .to_string(),
                init_git: false,
            },
            Some(Duration::from_secs(20)),
        )
        .unwrap();
    let line = &opened.board.expect("the clone opens").lines[0];
    assert!(!is_trusted(&line.trust), "{:?}", line.trust);
    // The person's own checkout stays trusted.
    assert!(is_trusted(&trust_of(&fixture, fixture.repo.work())));
}

#[test]
fn a_dry_run_trusts_nothing() {
    let fixture = Fixture::new();
    let board = fixture.board();
    let result = init(&fixture, &board, true).unwrap();
    assert!(!result.trusted);
    assert!(!fixture.home().join(".config/collide/trust.yaml").exists());
}

#[test]
fn a_line_that_already_commits_a_collide_folder_records_nothing() {
    let fixture = Fixture::new();
    fixture.write(".collide/board.yaml", "packs: []\n");
    fixture.write(".collide/views/theirs.yaml", OTHER);
    fixture.commit();
    let board = fixture.board();
    // The folder is gone from the working tree, not from the line: what the line holds is not the person's own.
    fs::remove_dir_all(fixture.repo.work().join(".collide")).unwrap();
    let result = init(&fixture, &board, false).unwrap();
    assert!(result.written);
    assert!(!result.trusted);
    assert!(!fixture.home().join(".config/collide/trust.yaml").exists());
}

#[test]
fn what_was_trusted_before_is_replaced_for_this_line_only() {
    let fixture = Fixture::new();
    let board = fixture.board();
    let other = fixture.home().join("other-repo");
    fs::create_dir(&other).unwrap();
    let made = git_command(fixture.home(), &other)
        .args(["init", "--quiet", "-b", "main"])
        .output()
        .unwrap();
    assert!(made.status.success());
    fs::write(other.join("README"), "x\n").unwrap();
    for args in [
        vec!["add", "--all"],
        vec!["commit", "--quiet", "--message", "start"],
    ] {
        assert!(
            git_command(fixture.home(), &other)
                .args(args)
                .output()
                .unwrap()
                .status
                .success()
        );
    }
    let opened: BoardOpenResult = fixture
        .client()
        .call(
            "board.open",
            &BoardOpenParams {
                root: fs::canonicalize(&other)
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .to_string(),
                init_git: false,
            },
            Some(Duration::from_secs(20)),
        )
        .unwrap();
    assert!(!is_trusted(&opened.board.unwrap().lines[0].trust));
    init(&fixture, &board, false).unwrap();
    fixture.commit();
    assert!(is_trusted(&trust_of(&fixture, fixture.repo.work())));
    assert!(
        !is_trusted(&trust_of(&fixture, &other)),
        "the other board is as it was"
    );
}
