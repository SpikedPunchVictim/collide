//! Reset's move (ts-0215-gt): the listed files go to `set-aside/<stamp>/` with their paths kept and their bytes whole,
//! nothing is deleted, other boards' folders stay, the folder of a reset is never moved by the next, and two resets
//! with the same stamp make two folders. Every path is below the test's own root under `/tmp`.

mod common;

use collide_host::set_aside::{SET_ASIDE_DIR, reset, stamp};
use common::TempRoot;
use std::fs;
use std::path::Path;

fn put(dir: &Path, rel: &str, text: &str) {
    let path = dir.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

fn fill(dir: &Path) {
    put(dir, "config.yaml", "ask:\n  agent: made-up\n");
    put(dir, "packs.yaml", "packs: []\n");
    put(dir, "packs.lock", "packs: []\n");
    put(dir, "trust.yaml", "kept: true\n");
    put(dir, "boards/board-1/config.yaml", "mine: 1\n");
    put(dir, "boards/board-1/packs.yaml", "on: []\n");
    put(dir, "boards/board-1/views/mine.yaml", "name: mine\n");
    put(dir, "boards/board-2/config.yaml", "other: 2\n");
}

#[test]
fn the_listed_files_move_with_their_paths_and_bytes_and_nothing_else_does() {
    let root = TempRoot::new();
    let dir = root.path();
    fill(dir);
    let done = reset(dir, Some("board-1"), "2026-11-26-101500").unwrap();
    assert_eq!(
        done.folder,
        dir.join(SET_ASIDE_DIR).join("2026-11-26-101500")
    );
    assert_eq!(
        done.moved,
        ["config.yaml", "packs.yaml", "packs.lock", "boards/board-1"]
    );
    for rel in [
        "config.yaml",
        "packs.yaml",
        "packs.lock",
        "boards/board-1/config.yaml",
        "boards/board-1/packs.yaml",
        "boards/board-1/views/mine.yaml",
    ] {
        assert!(!dir.join(rel).exists(), "{rel} moved");
        assert!(
            done.folder.join(rel).is_file(),
            "{rel} is in the set-aside folder"
        );
    }
    assert_eq!(
        fs::read_to_string(done.folder.join("boards/board-1/views/mine.yaml")).unwrap(),
        "name: mine\n"
    );
    assert_eq!(
        fs::read_to_string(dir.join("trust.yaml")).unwrap(),
        "kept: true\n"
    );
    assert_eq!(
        fs::read_to_string(dir.join("boards/board-2/config.yaml")).unwrap(),
        "other: 2\n"
    );
}

#[test]
fn with_no_board_only_the_global_files_move() {
    let root = TempRoot::new();
    let dir = root.path();
    fill(dir);
    let done = reset(dir, None, "2026-11-26-101500").unwrap();
    assert_eq!(done.moved, ["config.yaml", "packs.yaml", "packs.lock"]);
    assert!(dir.join("boards/board-1/config.yaml").is_file());
}

#[test]
fn a_file_that_is_not_there_is_not_listed_and_the_folder_is_still_made() {
    let root = TempRoot::new();
    let dir = root.path();
    put(dir, "config.yaml", "a: 1\n");
    let done = reset(dir, Some("board-1"), "2026-11-26-101500").unwrap();
    assert_eq!(done.moved, ["config.yaml"]);
    let empty = reset(dir, Some("board-1"), "2026-11-26-101501").unwrap();
    assert!(empty.moved.is_empty());
    assert!(empty.folder.is_dir());
}

#[test]
fn two_resets_in_the_same_second_make_two_folders_and_the_first_is_not_moved() {
    let root = TempRoot::new();
    let dir = root.path();
    fill(dir);
    let first = reset(dir, Some("board-1"), "2026-11-26-101500").unwrap();
    fill(dir);
    put(dir, "config.yaml", "second: true\n");
    let second = reset(dir, Some("board-1"), "2026-11-26-101500").unwrap();
    assert_ne!(first.folder, second.folder);
    assert_eq!(
        second.folder,
        dir.join(SET_ASIDE_DIR).join("2026-11-26-101500-2")
    );
    assert_eq!(
        fs::read_to_string(first.folder.join("config.yaml")).unwrap(),
        "ask:\n  agent: made-up\n"
    );
    assert_eq!(
        fs::read_to_string(second.folder.join("config.yaml")).unwrap(),
        "second: true\n"
    );
    let third = reset(dir, None, "2026-11-26-101500").unwrap();
    assert_eq!(
        third.folder,
        dir.join(SET_ASIDE_DIR).join("2026-11-26-101500-3")
    );
}

#[test]
fn a_board_id_that_cannot_name_a_folder_is_refused_before_anything_moves() {
    let root = TempRoot::new();
    let dir = root.path();
    fill(dir);
    for board in ["../x", "a/b", "", ".hidden", "set-aside/x"] {
        let error = reset(dir, Some(board), "2026-11-26-101500").unwrap_err();
        assert!(error.contains("safe name"), "{board}: {error}");
    }
    assert!(dir.join("config.yaml").is_file());
    assert!(!dir.join(SET_ASIDE_DIR).exists());
}

#[test]
fn the_stamp_is_the_zones_date_and_time_and_sorts_as_the_resets_happened() {
    const SECOND: u64 = 1_000_000_000;
    assert_eq!(stamp(0, 0), "1970-01-01-000000");
    assert_eq!(stamp(1_795_688_107 * SECOND, 0), "2026-11-26-101507");
    // 23:59:59 UTC, in a zone an hour east: the next day.
    assert_eq!(stamp(1_795_737_599 * SECOND, 3600), "2026-11-27-005959");
    assert!(stamp(1_795_688_107 * SECOND, 0) < stamp(1_795_688_108 * SECOND, 0));
}
