//! The user's settings files (ts-0098-nx): `~/.config/collide/config.yaml`, the per-board file under
//! `boards/<board-id>/`, and the user's profiles. Every file is a fixture in a temp home: a missing file is empty, a
//! bad one is an error that names its path, and no test reads the real home.

use collide_host::settings_files::{read_config_file, read_user_files, user_config_dir};
use std::fs;
use std::path::{Path, PathBuf};

mod common;
use common::TempRoot;

fn write(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

fn config_path(home: &Path) -> PathBuf {
    user_config_dir(home).join("config.yaml")
}

fn board_path(home: &Path, board: &str) -> PathBuf {
    user_config_dir(home)
        .join("boards")
        .join(board)
        .join("config.yaml")
}

#[test]
fn the_user_folder_is_dot_config_collide_under_the_home() {
    assert_eq!(
        user_config_dir(Path::new("/h")),
        PathBuf::from("/h/.config/collide")
    );
}

#[test]
fn a_missing_file_is_an_empty_one() {
    let root = TempRoot::new();
    let file = read_config_file(&config_path(root.path())).unwrap();
    assert_eq!(file, Default::default());
}

#[test]
fn an_empty_file_is_an_empty_one() {
    let root = TempRoot::new();
    let path = config_path(root.path());
    write(&path, "");
    assert_eq!(read_config_file(&path).unwrap(), Default::default());
}

#[test]
fn a_file_of_only_comments_is_an_empty_one() {
    let root = TempRoot::new();
    let path = config_path(root.path());
    write(&path, "# nothing yet\n");
    assert_eq!(read_config_file(&path).unwrap(), Default::default());
}

#[test]
fn a_file_is_read_into_the_typed_settings() {
    let root = TempRoot::new();
    let path = config_path(root.path());
    write(&path, "disk: { floor_gib: 9 }\nask: { timeout_s: 60 }\n");
    let file = read_config_file(&path).unwrap();
    assert_eq!(file.disk.floor_gib, Some(9));
    assert_eq!(file.ask.timeout_s, Some(60));
}

#[test]
fn a_malformed_file_is_an_error_naming_its_path() {
    let root = TempRoot::new();
    let path = config_path(root.path());
    write(&path, "disk: [unclosed\n");
    let error = read_config_file(&path).unwrap_err();
    assert!(error.contains(&path.display().to_string()), "{error}");
}

#[test]
fn a_key_the_file_may_not_hold_is_an_error_naming_its_path_and_the_key() {
    let root = TempRoot::new();
    let path = config_path(root.path());
    write(&path, "disk: { floor_gib: 9, colour: red }\n");
    let error = read_config_file(&path).unwrap_err();
    assert!(error.contains(&path.display().to_string()), "{error}");
    assert!(error.contains("colour"), "{error}");
}

#[test]
fn a_duplicate_key_is_an_error_naming_its_path() {
    let root = TempRoot::new();
    let path = config_path(root.path());
    write(&path, "disk: { floor_gib: 9 }\ndisk: { floor_gib: 3 }\n");
    let error = read_config_file(&path).unwrap_err();
    assert!(error.contains(&path.display().to_string()), "{error}");
}

#[test]
fn a_file_over_the_cap_is_an_error_naming_its_path_and_is_not_parsed() {
    let root = TempRoot::new();
    let path = config_path(root.path());
    write(&path, &format!("# {}\n", "x".repeat(300 * 1024)));
    let error = read_config_file(&path).unwrap_err();
    assert!(error.contains(&path.display().to_string()), "{error}");
    assert!(error.contains("cap"), "{error}");
}

#[test]
fn a_folder_where_the_file_should_be_is_an_error_naming_its_path() {
    let root = TempRoot::new();
    let path = config_path(root.path());
    fs::create_dir_all(&path).unwrap();
    let error = read_config_file(&path).unwrap_err();
    assert!(error.contains(&path.display().to_string()), "{error}");
}

#[test]
fn with_no_files_at_all_every_layer_is_empty() {
    let root = TempRoot::new();
    let files = read_user_files(root.path(), Some("board-1")).unwrap();
    assert_eq!(files.global, Default::default());
    assert_eq!(files.board, Default::default());
    assert!(files.profiles.is_empty());
}

#[test]
fn the_global_and_the_per_board_files_are_read_apart() {
    let root = TempRoot::new();
    write(&config_path(root.path()), "disk: { floor_gib: 2 }\n");
    write(
        &board_path(root.path(), "board-7"),
        "disk: { floor_gib: 7 }\n",
    );
    let files = read_user_files(root.path(), Some("board-7")).unwrap();
    assert_eq!(files.global.disk.floor_gib, Some(2));
    assert_eq!(files.board.disk.floor_gib, Some(7));
}

#[test]
fn another_boards_file_is_not_read() {
    let root = TempRoot::new();
    write(
        &board_path(root.path(), "board-8"),
        "disk: { floor_gib: 8 }\n",
    );
    let files = read_user_files(root.path(), Some("board-7")).unwrap();
    assert_eq!(files.board, Default::default());
}

#[test]
fn with_no_board_the_per_board_layer_is_empty() {
    let root = TempRoot::new();
    write(
        &board_path(root.path(), "board-7"),
        "disk: { floor_gib: 7 }\n",
    );
    let files = read_user_files(root.path(), None).unwrap();
    assert_eq!(files.board, Default::default());
}

#[test]
fn a_board_id_that_is_no_safe_token_is_refused_and_no_path_is_made_of_it() {
    let root = TempRoot::new();
    for bad in ["../x", "a/b", "", ".hidden", "a..b"] {
        let errors = read_user_files(root.path(), Some(bad)).unwrap_err();
        assert!(
            errors.iter().any(|e| e.contains("board")),
            "{bad}: {errors:?}"
        );
    }
}

#[test]
fn the_users_profiles_are_read_by_file_name_without_the_extension() {
    let root = TempRoot::new();
    let dir = user_config_dir(root.path()).join("profiles");
    write(
        &dir.join("small-fix.yaml"),
        "description: a small fix\nconfig: { disk: { floor_gib: 2 } }\n",
    );
    write(&dir.join("notes.txt"), "not a profile\n");
    let files = read_user_files(root.path(), None).unwrap();
    let names: Vec<&String> = files.profiles.keys().collect();
    assert_eq!(names, ["small-fix"]);
    assert_eq!(
        files.profiles["small-fix"].description.as_deref(),
        Some("a small fix")
    );
}

#[test]
fn a_bad_profile_is_an_error_naming_its_path_and_the_others_still_load() {
    let root = TempRoot::new();
    let dir = user_config_dir(root.path()).join("profiles");
    write(&dir.join("good.yaml"), "description: ok\n");
    write(&dir.join("bad.yaml"), "frobnicate: yes\n");
    let errors = read_user_files(root.path(), None).unwrap_err();
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert!(
        errors[0].contains(&dir.join("bad.yaml").display().to_string()),
        "{errors:?}"
    );
}

#[test]
fn every_bad_file_is_reported_not_only_the_first() {
    let root = TempRoot::new();
    write(&config_path(root.path()), "disk: [\n");
    write(&board_path(root.path(), "board-1"), "ask: [\n");
    let errors = read_user_files(root.path(), Some("board-1")).unwrap_err();
    assert_eq!(errors.len(), 2, "{errors:?}");
}

#[test]
fn a_profile_that_fails_the_profile_check_is_an_error_naming_its_path() {
    let root = TempRoot::new();
    let dir = user_config_dir(root.path()).join("profiles");
    write(
        &dir.join("both.yaml"),
        "packs: { on: [ferry], off: [ferry] }\n",
    );
    let errors = read_user_files(root.path(), None).unwrap_err();
    assert!(errors[0].contains("both.yaml"), "{errors:?}");
    assert!(errors[0].contains("ferry"), "{errors:?}");
}
