//! The atomic write the shim, `trust.yaml` and the user's settings file share (ts-0099-bw): a temp file beside the
//! target, its mode set on the open file, then a rename. Every test writes under its own `TempRoot`.

mod common;

use collide_host::atomic_write::{Step, write_atomically};
use common::TempRoot;
use std::fs::{self, File};
use std::io::{self, Write};
use std::os::unix::fs::PermissionsExt;

fn mode_of(path: &std::path::Path) -> u32 {
    fs::metadata(path).unwrap().permissions().mode() & 0o777
}

/// A file that takes no bytes: the seam that makes a write fail after the temp file exists.
struct Refuses(#[allow(dead_code)] File);

impl Write for Refuses {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        Err(io::Error::other("the disk is full"))
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn the_bytes_land_in_the_target_with_the_mode_asked_for() {
    let root = TempRoot::new();
    write_atomically(root.path(), "a.yaml", b"one\n", 0o600, |file| file).unwrap();
    assert_eq!(fs::read(root.join("a.yaml")).unwrap(), b"one\n");
    assert_eq!(mode_of(&root.join("a.yaml")), 0o600);
}

#[test]
fn a_wider_mode_is_given_as_asked() {
    let root = TempRoot::new();
    write_atomically(root.path(), "a.yaml", b"x", 0o644, |file| file).unwrap();
    assert_eq!(mode_of(&root.join("a.yaml")), 0o644);
}

#[test]
fn a_second_write_replaces_the_first_whole() {
    let root = TempRoot::new();
    write_atomically(root.path(), "a.yaml", b"a long first text\n", 0o600, |f| f).unwrap();
    write_atomically(root.path(), "a.yaml", b"short\n", 0o600, |f| f).unwrap();
    assert_eq!(fs::read(root.join("a.yaml")).unwrap(), b"short\n");
}

#[test]
fn no_temp_file_is_left_after_a_write() {
    let root = TempRoot::new();
    write_atomically(root.path(), "a.yaml", b"x", 0o600, |file| file).unwrap();
    assert!(!root.join("a.yaml.tmp").exists());
}

#[test]
fn a_temp_file_an_earlier_write_left_open_is_closed_by_the_new_mode() {
    let root = TempRoot::new();
    fs::write(root.join("a.yaml.tmp"), "stale").unwrap();
    fs::set_permissions(root.join("a.yaml.tmp"), fs::Permissions::from_mode(0o666)).unwrap();
    write_atomically(root.path(), "a.yaml", b"fresh", 0o600, |file| file).unwrap();
    assert_eq!(fs::read(root.join("a.yaml")).unwrap(), b"fresh");
    assert_eq!(mode_of(&root.join("a.yaml")), 0o600);
}

#[test]
fn a_write_that_fails_leaves_the_old_file_whole_and_no_temp_file() {
    let root = TempRoot::new();
    write_atomically(root.path(), "a.yaml", b"old\n", 0o600, |file| file).unwrap();
    let error = write_atomically(root.path(), "a.yaml", b"new\n", 0o600, Refuses).unwrap_err();
    assert_eq!(error.step, Step::WriteTemp);
    assert_eq!(error.path, root.join("a.yaml.tmp"));
    assert_eq!(fs::read(root.join("a.yaml")).unwrap(), b"old\n");
    assert!(!root.join("a.yaml.tmp").exists());
}

#[test]
fn a_folder_that_is_not_there_fails_at_the_temp_file_naming_it() {
    let root = TempRoot::new();
    let missing = root.join("nope");
    let error = write_atomically(&missing, "a.yaml", b"x", 0o600, |file| file).unwrap_err();
    assert_eq!(error.step, Step::CreateTemp);
    assert_eq!(error.path, missing.join("a.yaml.tmp"));
}

#[test]
fn a_target_that_cannot_be_replaced_fails_at_the_rename_and_removes_the_temp_file() {
    let root = TempRoot::new();
    fs::create_dir(root.join("a.yaml")).unwrap();
    let error = write_atomically(root.path(), "a.yaml", b"x", 0o600, |file| file).unwrap_err();
    assert_eq!(error.step, Step::Rename);
    assert_eq!(error.path, root.join("a.yaml"));
    assert!(!root.join("a.yaml.tmp").exists());
}

#[test]
fn the_error_says_what_was_being_done_and_where() {
    let root = TempRoot::new();
    let error =
        write_atomically(&root.join("nope"), "a.yaml", b"x", 0o600, |file| file).unwrap_err();
    let text = error.to_string();
    assert!(text.starts_with("creating "), "{text}");
    assert!(text.contains("a.yaml.tmp"), "{text}");
}
