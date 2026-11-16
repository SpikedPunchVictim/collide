//! How the daemon learns where the app is (ts-0077-ws; 0.2, Who is asking). The app writes its own executable path to a
//! file in the daemon's run folder before it connects, and the daemon reads that file when it checks a peer, so a
//! daemon
//! that the CLI started first still knows the app. These tests use real files under a root of their own below `/tmp`
//! and
//! the system's answer for this test process. No daemon runs, and nothing outside the root is read.

mod common;

use collide_host::auth::{
    ProcessTree, SystemProcessTree, daemon_uid, read_app_executable, write_app_executable,
};
use collide_host::daemon::DaemonPaths;
use common::TempRoot;
use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt, symlink};
use std::path::{Path, PathBuf};

fn this_executable() -> PathBuf {
    fs::canonicalize(std::env::current_exe().unwrap()).unwrap()
}

fn mode(path: &Path) -> u32 {
    fs::metadata(path).unwrap().mode() & 0o7777
}

fn this_process_is_the_app(tree: &SystemProcessTree) -> bool {
    tree.is_app(std::process::id())
}

#[test]
fn the_app_file_is_in_the_run_folder_beside_the_socket() {
    let root = TempRoot::new();
    let paths = DaemonPaths::under(root.path());
    assert_eq!(
        paths.app_executable_file(),
        paths.socket_dir.join("app-executable")
    );
}

#[test]
fn writing_makes_a_private_file_that_holds_the_path_in_a_private_folder() {
    let root = TempRoot::new();
    let paths = DaemonPaths::under(root.path());
    write_app_executable(&paths, Path::new("/Applications/Collide/collide-app")).unwrap();
    let file = paths.app_executable_file();
    assert_eq!(
        fs::read_to_string(&file).unwrap(),
        "/Applications/Collide/collide-app"
    );
    assert_eq!(mode(&file), 0o600);
    assert_eq!(mode(&paths.socket_dir), 0o700);
}

#[test]
fn writing_again_replaces_the_path_and_leaves_no_other_file() {
    let root = TempRoot::new();
    let paths = DaemonPaths::under(root.path());
    write_app_executable(&paths, Path::new("/one")).unwrap();
    write_app_executable(&paths, Path::new("/two")).unwrap();
    assert_eq!(
        read_app_executable(&paths, daemon_uid()),
        Some(PathBuf::from("/two"))
    );
    let names: Vec<_> = fs::read_dir(&paths.socket_dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert_eq!(names, vec![std::ffi::OsString::from("app-executable")]);
}

#[test]
fn a_folder_of_the_wrong_mode_is_not_made_safe_by_the_write_and_the_write_says_so() {
    let root = TempRoot::new();
    let paths = DaemonPaths::under(root.path());
    fs::create_dir(&paths.socket_dir).unwrap();
    fs::set_permissions(&paths.socket_dir, fs::Permissions::from_mode(0o755)).unwrap();
    let error = write_app_executable(&paths, Path::new("/app")).unwrap_err();
    assert!(
        error.to_string().contains("0755"),
        "the message names the mode: {error}"
    );
    assert!(!paths.app_executable_file().exists());
}

#[test]
fn the_daemon_reads_the_path_back() {
    let root = TempRoot::new();
    let paths = DaemonPaths::under(root.path());
    write_app_executable(&paths, Path::new("/Applications/Collide/collide-app")).unwrap();
    assert_eq!(
        read_app_executable(&paths, daemon_uid()),
        Some(PathBuf::from("/Applications/Collide/collide-app"))
    );
}

#[test]
fn a_file_the_daemon_does_not_trust_names_no_app() {
    let root = TempRoot::new();
    let paths = DaemonPaths::under(root.path());
    fs::create_dir(&paths.socket_dir).unwrap();
    fs::set_permissions(&paths.socket_dir, fs::Permissions::from_mode(0o700)).unwrap();
    let file = paths.app_executable_file();

    assert_eq!(read_app_executable(&paths, daemon_uid()), None, "no file");

    fs::write(&file, "/app").unwrap();
    fs::set_permissions(&file, fs::Permissions::from_mode(0o644)).unwrap();
    assert_eq!(
        read_app_executable(&paths, daemon_uid()),
        None,
        "others can read it"
    );

    fs::set_permissions(&file, fs::Permissions::from_mode(0o600)).unwrap();
    fs::write(&file, "").unwrap();
    assert_eq!(read_app_executable(&paths, daemon_uid()), None, "empty");

    fs::remove_file(&file).unwrap();
    let target = root.join("elsewhere");
    fs::write(&target, "/app").unwrap();
    fs::set_permissions(&target, fs::Permissions::from_mode(0o600)).unwrap();
    symlink(&target, &file).unwrap();
    assert_eq!(read_app_executable(&paths, daemon_uid()), None, "a link");
}

#[test]
fn a_file_of_another_owner_names_no_app() {
    let root = TempRoot::new();
    let paths = DaemonPaths::under(root.path());
    write_app_executable(&paths, Path::new("/app")).unwrap();
    let mine = fs::metadata(paths.app_executable_file()).unwrap().uid();
    assert_eq!(read_app_executable(&paths, mine), Some("/app".into()));
    assert_eq!(read_app_executable(&paths, mine + 1), None);
}

#[test]
fn a_tree_with_the_file_knows_this_process_is_the_app_when_the_file_names_this_executable() {
    let root = TempRoot::new();
    let paths = DaemonPaths::under(root.path());
    write_app_executable(&paths, &this_executable()).unwrap();
    let tree = SystemProcessTree::new(None).with_app_file(&paths);
    assert!(this_process_is_the_app(&tree));
}

#[test]
fn a_file_written_after_the_tree_was_made_is_seen_because_it_is_read_at_each_check() {
    let root = TempRoot::new();
    let paths = DaemonPaths::under(root.path());
    let tree = SystemProcessTree::new(None).with_app_file(&paths);
    assert!(!this_process_is_the_app(&tree), "no file yet");
    write_app_executable(&paths, &this_executable()).unwrap();
    assert!(this_process_is_the_app(&tree), "the app wrote it");
    write_app_executable(&paths, Path::new("/somewhere/else")).unwrap();
    assert!(!this_process_is_the_app(&tree), "the app moved");
}

#[test]
fn a_file_of_the_wrong_mode_does_not_make_this_process_the_app() {
    let root = TempRoot::new();
    let paths = DaemonPaths::under(root.path());
    write_app_executable(&paths, &this_executable()).unwrap();
    fs::set_permissions(
        paths.app_executable_file(),
        fs::Permissions::from_mode(0o666),
    )
    .unwrap();
    let tree = SystemProcessTree::new(None).with_app_file(&paths);
    assert!(!this_process_is_the_app(&tree));
}

#[test]
fn a_path_given_to_the_tree_wins_over_the_file() {
    let root = TempRoot::new();
    let paths = DaemonPaths::under(root.path());
    write_app_executable(&paths, Path::new("/somewhere/else")).unwrap();
    let tree = SystemProcessTree::new(Some(this_executable())).with_app_file(&paths);
    assert!(this_process_is_the_app(&tree));
}
