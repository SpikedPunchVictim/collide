//! A `git:` source resolved to a pin, and the lock files that hold it (ts-0100-pu; 0.7, Pack sources). The remote is a
//! bare repository on disk in a temp root, and the lock is a file in the same root: no test reaches the real home.

mod common;

use collide_core::lock::{GitLocator, LockEntry, PackLock, check_lock};
use collide_core::owner::PackPin;
use collide_host::git_fetch::{FetchEnv, GitFetch};
use collide_host::git_objects::GitObjects;
use collide_host::hosts::KnownHosts;
use collide_host::pack_lock::{read_lock, resolve_pin, set_pin};
use common::TempRoot;
use common::remote::Remote;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

struct Mirrored {
    remote: Remote,
    mirror: PathBuf,
    first: String,
    first_tree: String,
}

/// A remote whose `main` is two commits (the pack at `pack/`, a nested pack at `packs/ferry/v2/`), tagged `v1` at the
/// first, fetched into a mirror.
fn mirrored() -> Mirrored {
    let remote = Remote::new();
    remote.work.write("pack/pack.yaml", b"name: ferry\n");
    remote
        .work
        .write("packs/ferry/v2/pack.yaml", b"name: ferry2\n");
    let first = remote.work.commit_all("first");
    let first_tree = remote.work.git(&["rev-parse", "HEAD:pack"]);
    remote.work.git(&["tag", "v1"]);
    remote
        .work
        .write("pack/pack.yaml", b"name: ferry\nversion: 2\n");
    remote.work.commit_all("second");
    remote.publish();
    let mirror = remote.root().join("mirror");
    let fetch = GitFetch::new(FetchEnv {
        path: std::env::var_os("PATH").unwrap_or_default(),
        home: remote.root().path().to_path_buf(),
        ssh_auth_sock: None,
        xdg_config_home: None,
    })
    .without_system_config();
    fetch
        .fetch(
            &mirror,
            &remote.file_url(),
            &KnownHosts::for_home(remote.root().path()),
        )
        .unwrap();
    Mirrored {
        remote,
        mirror,
        first,
        first_tree,
    }
}

fn locator(m: &Mirrored, tail: &str) -> GitLocator {
    GitLocator::parse(&format!("{}{tail}", m.remote.file_url())).unwrap()
}

fn pin(m: &Mirrored, tail: &str, reference: Option<&str>) -> Result<PackPin, String> {
    resolve_pin(&GitObjects::new(), &m.mirror, &locator(m, tail), reference)
}

#[test]
fn a_branch_resolves_to_its_commit_and_the_tree_at_the_path() {
    let m = mirrored();
    let found = pin(&m, "//pack@main", None).unwrap();
    assert_eq!(found.commit, m.remote.work.git(&["rev-parse", "main"]));
    assert_eq!(found.tree, m.remote.work.git(&["rev-parse", "main:pack"]));
}

#[test]
fn a_tag_resolves_to_the_commit_it_names() {
    let m = mirrored();
    let found = pin(&m, "//pack@v1", None).unwrap();
    assert_eq!(found.commit, m.first);
    assert_eq!(found.tree, m.first_tree);
}

#[test]
fn a_commit_id_resolves_to_itself() {
    let m = mirrored();
    let found = pin(&m, &format!("//pack@{}", m.first), None).unwrap();
    assert_eq!(found.commit, m.first);
    assert_eq!(found.tree, m.first_tree);
}

#[test]
fn a_locator_with_no_ref_resolves_the_remotes_head() {
    let m = mirrored();
    let found = pin(&m, "//pack", None).unwrap();
    assert_eq!(found.commit, m.remote.work.git(&["rev-parse", "HEAD"]));
}

#[test]
fn a_ref_given_to_the_call_replaces_the_locators() {
    let m = mirrored();
    let found = pin(&m, "//pack@main", Some("v1")).unwrap();
    assert_eq!(found.commit, m.first);
}

#[test]
fn a_nested_path_resolves_to_the_tree_there() {
    let m = mirrored();
    let found = pin(&m, "//packs/ferry/v2@v1", None).unwrap();
    assert_eq!(
        found.tree,
        m.remote.work.git(&["rev-parse", "v1:packs/ferry/v2"])
    );
}

#[test]
fn an_empty_path_is_the_commits_own_tree() {
    let m = mirrored();
    let found = pin(&m, "@v1", None).unwrap();
    assert_eq!(found.tree, m.remote.work.git(&["rev-parse", "v1^{tree}"]));
}

#[test]
fn a_path_that_is_a_file_is_refused_because_a_pack_is_a_folder() {
    let m = mirrored();
    let error = pin(&m, "//pack/pack.yaml@main", None).unwrap_err();
    assert!(error.contains("a pack is a folder"), "{error}");
}

#[test]
fn a_ref_the_mirror_does_not_have_names_the_ref() {
    let m = mirrored();
    let error = pin(&m, "//pack@no-such-ref", None).unwrap_err();
    assert!(error.contains("no-such-ref"), "{error}");
}

#[test]
fn a_path_the_commit_does_not_have_names_the_path_and_the_ref() {
    let m = mirrored();
    let error = pin(&m, "//nowhere@main", None).unwrap_err();
    assert!(
        error.contains("nowhere") && error.contains("main"),
        "{error}"
    );
}

// ---- the lock files ----

const SOURCE: &str = "git:https://h/o/r//pack@v1";

fn a_pin(c: char, t: char) -> PackPin {
    PackPin {
        commit: c.to_string().repeat(40),
        tree: t.to_string().repeat(40),
    }
}

#[test]
fn a_lock_that_is_not_there_reads_as_empty() {
    let root = TempRoot::new();
    assert_eq!(
        read_lock(&root.join("packs.lock")).unwrap(),
        PackLock::default()
    );
}

#[test]
fn a_pin_is_written_and_read_back() {
    let root = TempRoot::new();
    let file = root.join("packs.lock");
    set_pin(&file, SOURCE, &a_pin('a', 'b'), 0o644).unwrap();
    let lock = read_lock(&file).unwrap();
    assert_eq!(
        lock.packs,
        [LockEntry {
            source: SOURCE.into(),
            commit: "a".repeat(40),
            tree: "b".repeat(40)
        }]
    );
    assert!(check_lock(&lock).is_ok());
}

#[test]
fn a_new_source_is_appended_and_a_known_one_is_replaced_in_place() {
    let root = TempRoot::new();
    let file = root.join("packs.lock");
    set_pin(&file, "git:https://h/o/a//p", &a_pin('1', '2'), 0o644).unwrap();
    set_pin(&file, "git:https://h/o/b//p", &a_pin('3', '4'), 0o644).unwrap();
    set_pin(&file, "git:https://h/o/a//p", &a_pin('5', '6'), 0o644).unwrap();
    let lock = read_lock(&file).unwrap();
    let sources: Vec<_> = lock.packs.iter().map(|e| e.source.as_str()).collect();
    assert_eq!(sources, ["git:https://h/o/a//p", "git:https://h/o/b//p"]);
    assert_eq!(lock.packs[0].commit, "5".repeat(40));
    assert_eq!(lock.packs[1].commit, "3".repeat(40));
}

#[test]
fn the_file_gets_the_mode_asked_for_and_no_temp_file_is_left() {
    let root = TempRoot::new();
    for (name, mode) in [("repo.lock", 0o644), ("user.lock", 0o600)] {
        let file = root.join(name);
        set_pin(&file, SOURCE, &a_pin('a', 'b'), mode).unwrap();
        assert_eq!(
            fs::metadata(&file).unwrap().permissions().mode() & 0o777,
            mode,
            "{name}"
        );
    }
    let mut names: Vec<_> = fs::read_dir(root.path())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    names.sort();
    assert_eq!(names, ["repo.lock", "user.lock"]);
}

#[test]
fn the_folder_of_a_user_lock_is_made_when_it_is_not_there() {
    let root = TempRoot::new();
    let file = root.join(".config/collide/packs.lock");
    set_pin(&file, SOURCE, &a_pin('a', 'b'), 0o600).unwrap();
    assert!(file.exists());
}

#[test]
fn a_pin_with_a_bad_id_is_refused_and_the_file_is_left_as_it_is() {
    let root = TempRoot::new();
    let file = root.join("packs.lock");
    set_pin(&file, SOURCE, &a_pin('a', 'b'), 0o644).unwrap();
    let before = fs::read_to_string(&file).unwrap();
    let bad = PackPin {
        commit: "nothex".into(),
        tree: "b".repeat(40),
    };
    let error = set_pin(&file, SOURCE, &bad, 0o644).unwrap_err();
    assert!(error.contains("commit"), "{error}");
    assert_eq!(fs::read_to_string(&file).unwrap(), before);
}

#[test]
fn a_lock_that_does_not_read_is_an_error_naming_the_file_and_is_not_rewritten() {
    let root = TempRoot::new();
    let file = root.join("packs.lock");
    fs::write(&file, "packs: [unclosed\n").unwrap();
    let error = read_lock(&file).unwrap_err();
    assert!(error.contains("packs.lock"), "{error}");
    assert!(set_pin(&file, SOURCE, &a_pin('a', 'b'), 0o644).is_err());
    assert_eq!(fs::read_to_string(&file).unwrap(), "packs: [unclosed\n");
}
