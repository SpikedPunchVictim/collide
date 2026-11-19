//! The folder that puts `collide` first on a run agent's `PATH` (ts-0140-gk): its link, its replacement on a resumed
//! attempt, and the order of the `PATH`. Every folder is in a temp root.

use collide_host::file_run::collide_path::{DIR_NAME, LINK_NAME, link_dir, path_with_first};
use std::fs;
use std::os::unix::fs::symlink;
use std::path::Path;

mod common;
use common::TempRoot;

fn exe_in(root: &TempRoot, name: &str) -> std::path::PathBuf {
    root.script(name, "#!/bin/sh\nexit 0\n")
}

#[test]
fn the_folder_holds_one_link_named_collide_to_the_binary() {
    let root = TempRoot::new();
    let exe = exe_in(&root, "the-binary");
    let run_dir = root.join("run-1");
    fs::create_dir(&run_dir).unwrap();
    let dir = link_dir(&run_dir, &exe).unwrap();
    assert_eq!(dir, run_dir.join(DIR_NAME));
    let names: Vec<_> = fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .map(|e| e.file_name())
        .collect();
    assert_eq!(names, [LINK_NAME]);
    assert_eq!(fs::read_link(dir.join(LINK_NAME)).unwrap(), exe);
}

#[test]
fn a_second_attempt_replaces_the_link_with_the_binary_the_daemon_runs_from_now() {
    let root = TempRoot::new();
    let (old, new) = (exe_in(&root, "old"), exe_in(&root, "new"));
    let run_dir = root.join("run-1");
    fs::create_dir(&run_dir).unwrap();
    link_dir(&run_dir, &old).unwrap();
    let dir = link_dir(&run_dir, &new).unwrap();
    assert_eq!(fs::read_link(dir.join(LINK_NAME)).unwrap(), new);
}

#[test]
fn something_else_at_the_link_name_is_replaced_not_followed() {
    let root = TempRoot::new();
    let exe = exe_in(&root, "the-binary");
    let run_dir = root.join("run-1");
    fs::create_dir_all(run_dir.join(DIR_NAME)).unwrap();
    let elsewhere = root.join("elsewhere");
    fs::write(&elsewhere, "keep me").unwrap();
    symlink(&elsewhere, run_dir.join(DIR_NAME).join(LINK_NAME)).unwrap();
    link_dir(&run_dir, &exe).unwrap();
    assert_eq!(fs::read_to_string(&elsewhere).unwrap(), "keep me");
    assert_eq!(
        fs::read_link(run_dir.join(DIR_NAME).join(LINK_NAME)).unwrap(),
        exe
    );
}

#[test]
fn a_binary_that_is_not_an_absolute_path_is_refused_with_its_name() {
    let root = TempRoot::new();
    let error = link_dir(root.path(), Path::new("collide")).unwrap_err();
    assert!(error.contains("collide"), "{error}");
    assert!(!root.join(DIR_NAME).exists());
}

#[test]
fn the_folder_comes_first_and_the_rest_of_the_path_keeps_its_order() {
    let dir = Path::new("/run/bin");
    assert_eq!(
        path_with_first(dir, Some("/a:/b")).unwrap(),
        "/run/bin:/a:/b"
    );
}

#[test]
fn a_missing_or_empty_path_becomes_the_folder_alone() {
    let dir = Path::new("/run/bin");
    assert_eq!(path_with_first(dir, None).unwrap(), "/run/bin");
    assert_eq!(path_with_first(dir, Some("")).unwrap(), "/run/bin");
}
