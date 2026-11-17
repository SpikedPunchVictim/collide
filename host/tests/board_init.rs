//! `board.init` (ts-0102-vu; 0.11, Share with team). A daemon runs in the test's own process over a fixture checkout
//! in a temp root, so nothing outside it is read or written. The call writes `.collide/board.yaml` and nothing else:
//! no commit, no `.gitignore`, no lock.

mod common;

use collide_core::board::{Board, check_board};
use collide_core::owner::{BoardInitParams, BoardInitResult, RpcError, code};
use collide_core::yaml::load_yaml_file;
use collide_host::owner_client::ClientError;
use common::config_fixture::Fixture;
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
