//! Tests for the git object reader (ts-0094-av). Every repository is built by `git` in a temp root with a clean
//! environment (`common::git_repo`), so no test reads the real home or the real git config. The reader clears the
//! environment of its own git calls, so none of them sees the test's either.

use collide_core::yaml::MAX_YAML_BYTES;
use collide_host::git_objects::{
    GitError, GitObjects, MAX_BLOB_BYTES, MAX_TREE_BYTES, MAX_TREE_ENTRIES, ObjectKind,
};
use std::collections::BTreeSet;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

mod common;
use common::git_repo::{GitRepo, git_bytes};
use common::{TempRoot, find_on_path};

const EMPTY_TREE: &str = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";

fn reader() -> GitObjects {
    GitObjects::new()
}

fn text(error: GitError) -> String {
    error.to_string()
}

/// A repository with `a/b/c.txt` and `top.txt` committed on `main`.
fn nested() -> GitRepo {
    let repo = GitRepo::new();
    repo.write("a/b/c.txt", b"deep\n");
    repo.write("top.txt", b"top\n");
    repo.commit_all("one");
    repo
}

/// A folder `bin` in the temp root holding a `git` made from `body`, and the `PATH` that finds it first.
fn fake_git(root: &TempRoot, body: &str) -> OsPath {
    let dir = root.join("bin");
    fs::create_dir_all(&dir).unwrap();
    let script = dir.join("git");
    fs::write(&script, format!("#!/bin/sh\n{body}\n")).unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
    OsPath(dir)
}

struct OsPath(PathBuf);

fn real_git() -> PathBuf {
    find_on_path("git", &std::env::var_os("PATH").unwrap()).expect("git is on PATH")
}

// ---- tree_at ----

#[test]
fn tree_at_a_nested_path_equals_rev_parse() {
    let repo = nested();
    let found = reader().tree_at(&repo.git_dir(), "main", "a/b").unwrap();
    assert_eq!(found.id, repo.git(&["rev-parse", "main:a/b"]));
    assert_eq!(found.kind, ObjectKind::Tree);
}

#[test]
fn tree_at_the_empty_path_is_the_commit_tree() {
    let repo = nested();
    let found = reader().tree_at(&repo.git_dir(), "main", "").unwrap();
    assert_eq!(found.id, repo.git(&["rev-parse", "main^{tree}"]));
    assert_eq!(found.kind, ObjectKind::Tree);
}

#[test]
fn the_same_files_at_two_commits_give_the_same_id() {
    let repo = nested();
    repo.write("top.txt", b"changed\n");
    repo.commit_all("two");
    let before = reader().tree_at(&repo.git_dir(), "HEAD~1", "a").unwrap();
    let after = reader().tree_at(&repo.git_dir(), "HEAD", "a").unwrap();
    assert_eq!(before.id, after.id);
    let root_before = reader().tree_at(&repo.git_dir(), "HEAD~1", "").unwrap();
    let root_after = reader().tree_at(&repo.git_dir(), "HEAD", "").unwrap();
    assert_ne!(root_before.id, root_after.id);
}

#[test]
fn tree_at_a_file_says_blob() {
    let repo = nested();
    let found = reader()
        .tree_at(&repo.git_dir(), "main", "a/b/c.txt")
        .unwrap();
    assert_eq!(found.kind, ObjectKind::Blob);
    assert_eq!(found.id, repo.git(&["rev-parse", "main:a/b/c.txt"]));
}

#[test]
fn tree_at_a_missing_path_is_an_error_naming_the_path() {
    let repo = nested();
    let error = text(
        reader()
            .tree_at(&repo.git_dir(), "main", "a/nope")
            .unwrap_err(),
    );
    assert!(error.contains("a/nope"), "{error}");
}

#[test]
fn tree_at_a_missing_ref_is_an_error_naming_the_ref() {
    let repo = nested();
    let error = text(
        reader()
            .tree_at(&repo.git_dir(), "no-such-branch", "a")
            .unwrap_err(),
    );
    assert!(error.contains("no-such-branch"), "{error}");
}

#[test]
fn a_rev_that_starts_with_a_dash_is_refused_and_git_does_not_run() {
    let repo = nested();
    let marker = repo.root().join("ran");
    let fake = fake_git(repo.root(), &format!("touch {}", marker.display()));
    let error = text(
        reader()
            .with_path(fake.0.clone())
            .tree_at(&repo.git_dir(), "--output=x", "a")
            .unwrap_err(),
    );
    assert!(error.contains("--output=x"), "{error}");
    assert!(!marker.exists(), "git ran");
}

#[test]
fn a_path_that_could_leave_the_tree_or_break_the_framing_is_refused() {
    let repo = nested();
    for bad in ["../x", "a/../b", "/abs", "a\nb", "a\0b", "./a", "a//b"] {
        let error = reader().tree_at(&repo.git_dir(), "main", bad).unwrap_err();
        assert!(
            matches!(error, GitError::Refused { .. }),
            "{bad:?} gave {error}"
        );
    }
}

#[test]
fn a_bare_repository_is_read_as_a_checkout_is() {
    let repo = nested();
    let bare = repo.root().join("mirror.git");
    repo.git(&["clone", "--quiet", "--bare", ".", bare.to_str().unwrap()]);
    let found = reader().tree_at(&bare, "main", "a/b").unwrap();
    assert_eq!(found.id, repo.git(&["rev-parse", "main:a/b"]));
}

// ---- the working tree is not the snapshot ----

#[test]
fn an_uncommitted_edit_a_deleted_file_and_an_untracked_file_do_not_change_what_is_read() {
    let repo = GitRepo::new();
    repo.write("kept.txt", b"committed\n");
    repo.write("gone.txt", b"also committed\n");
    repo.commit_all("one");
    let tree = repo.git(&["rev-parse", "HEAD^{tree}"]);
    repo.write("kept.txt", b"edited, not committed\n");
    repo.remove("gone.txt");
    repo.write("new.txt", b"untracked\n");

    let entries = reader().list(&repo.git_dir(), &tree).unwrap();
    let paths: Vec<&str> = entries.iter().map(|e| e.path.as_str()).collect();
    assert_eq!(paths, ["gone.txt", "kept.txt"]);
    let ids: Vec<String> = entries.iter().map(|e| e.id.clone()).collect();
    let bodies = reader().read_blobs(&repo.git_dir(), &ids).unwrap();
    assert_eq!(
        bodies,
        [b"also committed\n".to_vec(), b"committed\n".to_vec()]
    );
    let kept = reader()
        .blob_at(&repo.git_dir(), "HEAD", "kept.txt")
        .unwrap();
    assert_eq!(kept.as_deref(), Some(&b"committed\n"[..]));
    let new = reader()
        .blob_at(&repo.git_dir(), "HEAD", "new.txt")
        .unwrap();
    assert_eq!(new, None);
}

// ---- list ----

#[test]
fn list_keeps_the_modes_git_writes() {
    let repo = GitRepo::new();
    repo.write("plain.txt", b"p\n");
    repo.write_executable("run.sh", b"#!/bin/sh\n");
    repo.symlink("link", "plain.txt");
    repo.git(&["add", "--all"]);
    repo.git(&[
        "update-index",
        "--add",
        "--cacheinfo",
        "160000,1234567890123456789012345678901234567890,sub",
    ]);
    repo.git(&["commit", "--quiet", "--message", "one"]);
    let tree = repo.git(&["rev-parse", "HEAD^{tree}"]);
    let entries = reader().list(&repo.git_dir(), &tree).unwrap();
    let seen: Vec<(&str, ObjectKind, &str)> = entries
        .iter()
        .map(|e| (e.path.as_str(), e.kind, e.mode.as_str()))
        .collect();
    assert_eq!(
        seen,
        [
            ("link", ObjectKind::Blob, "120000"),
            ("plain.txt", ObjectKind::Blob, "100644"),
            ("run.sh", ObjectKind::Blob, "100755"),
            ("sub", ObjectKind::Commit, "160000"),
        ]
    );
}

#[test]
fn list_gives_a_blob_its_id_and_size() {
    let repo = nested();
    let tree = repo.git(&["rev-parse", "main^{tree}"]);
    let entries = reader().list(&repo.git_dir(), &tree).unwrap();
    let deep = entries.iter().find(|e| e.path == "a/b/c.txt").unwrap();
    assert_eq!(deep.id, repo.git(&["rev-parse", "main:a/b/c.txt"]));
    assert_eq!(deep.size, 5);
}

#[test]
fn list_returns_a_name_with_a_space_a_newline_or_a_quote_whole() {
    let repo = GitRepo::new();
    for name in ["with space.txt", "line\nbreak.txt", "quote\"d.txt"] {
        repo.write(name, name.as_bytes());
    }
    repo.commit_all("one");
    let tree = repo.git(&["rev-parse", "HEAD^{tree}"]);
    let entries = reader().list(&repo.git_dir(), &tree).unwrap();
    let paths: Vec<&str> = entries.iter().map(|e| e.path.as_str()).collect();
    assert_eq!(paths, ["line\nbreak.txt", "quote\"d.txt", "with space.txt"]);
}

#[test]
fn list_refuses_a_name_that_is_not_utf8_naming_it_lossily() {
    let repo = GitRepo::new();
    let blob = repo.blob(b"x\n");
    let tree = repo.tree_of(&[("100644", &blob, b"caf\xe9.txt")]);
    let error = text(reader().list(&repo.git_dir(), &tree).unwrap_err());
    assert!(error.contains("caf\u{fffd}.txt"), "{error}");
}

#[test]
fn list_is_sorted_by_path_with_folders_flattened() {
    let repo = GitRepo::new();
    for name in [
        "b.txt",
        "a/z.txt",
        "a/b.txt",
        "a-b.txt",
        "a.txt",
        "c/d/e.txt",
    ] {
        repo.write(name, name.as_bytes());
    }
    repo.commit_all("one");
    let tree = repo.git(&["rev-parse", "HEAD^{tree}"]);
    let entries = reader().list(&repo.git_dir(), &tree).unwrap();
    let paths: Vec<&str> = entries.iter().map(|e| e.path.as_str()).collect();
    let mut sorted = paths.clone();
    sorted.sort();
    assert_eq!(paths, sorted);
    assert_eq!(paths.len(), 6);
    assert!(paths.contains(&"c/d/e.txt"));
}

#[test]
fn list_of_an_empty_tree_is_empty() {
    let repo = GitRepo::new();
    let tree = repo.tree_of(&[]);
    assert_eq!(tree, EMPTY_TREE);
    assert!(reader().list(&repo.git_dir(), &tree).unwrap().is_empty());
}

#[test]
fn list_refuses_a_tree_over_the_entry_cap_naming_the_count() {
    let repo = GitRepo::new();
    for n in 0..=MAX_TREE_ENTRIES {
        repo.write(&format!("f{n:05}.txt"), b"x");
    }
    repo.commit_all("one");
    let tree = repo.git(&["rev-parse", "HEAD^{tree}"]);
    let error = text(reader().list(&repo.git_dir(), &tree).unwrap_err());
    assert!(
        error.contains(&(MAX_TREE_ENTRIES + 1).to_string()),
        "{error}"
    );
}

#[test]
fn list_allows_a_tree_of_exactly_the_entry_cap() {
    let repo = GitRepo::new();
    for n in 0..MAX_TREE_ENTRIES {
        repo.write(&format!("f{n:05}.txt"), b"x");
    }
    repo.commit_all("one");
    let tree = repo.git(&["rev-parse", "HEAD^{tree}"]);
    assert_eq!(
        reader().list(&repo.git_dir(), &tree).unwrap().len(),
        MAX_TREE_ENTRIES
    );
}

#[test]
fn list_refuses_a_blob_over_the_size_cap_naming_its_path() {
    let repo = GitRepo::new();
    repo.write("pack/big.bin", &vec![b'a'; MAX_BLOB_BYTES as usize + 1]);
    repo.commit_all("one");
    let tree = repo.git(&["rev-parse", "HEAD^{tree}"]);
    let error = text(reader().list(&repo.git_dir(), &tree).unwrap_err());
    assert!(error.contains("pack/big.bin"), "{error}");
}

#[test]
fn list_refuses_a_tree_over_the_total_size_cap() {
    let repo = GitRepo::new();
    let each = 7 * 1024 * 1024;
    assert!(5 * each as u64 > MAX_TREE_BYTES && (each as u64) < MAX_BLOB_BYTES);
    for n in 0..5u8 {
        repo.write(&format!("part{n}.bin"), &vec![n; each]);
    }
    repo.commit_all("one");
    let tree = repo.git(&["rev-parse", "HEAD^{tree}"]);
    let error = text(reader().list(&repo.git_dir(), &tree).unwrap_err());
    assert!(error.contains(&MAX_TREE_BYTES.to_string()), "{error}");
}

#[test]
fn list_refuses_a_tree_that_is_not_a_full_hex_id_and_git_does_not_run() {
    let repo = nested();
    let marker = repo.root().join("ran");
    let fake = fake_git(repo.root(), &format!("touch {}", marker.display()));
    for bad in ["--help", "main", "HEAD:a", "abc123", ""] {
        let error = reader()
            .with_path(fake.0.clone())
            .list(&repo.git_dir(), bad)
            .unwrap_err();
        assert!(
            matches!(error, GitError::Refused { .. }),
            "{bad:?}: {error}"
        );
    }
    assert!(!marker.exists(), "git ran");
}

#[test]
fn list_of_a_tree_id_that_is_not_there_is_an_error() {
    let repo = nested();
    let missing = "0123456789012345678901234567890123456789";
    let error = text(reader().list(&repo.git_dir(), missing).unwrap_err());
    assert!(error.contains("git ls-tree"), "{error}");
}

// ---- read_blobs ----

#[test]
fn read_blobs_gives_the_bytes_git_shows_for_text_and_for_binary() {
    let repo = GitRepo::new();
    let text_id = repo.blob(b"plain text\nwith lines\n");
    let binary = [0u8, 1, 2, 0, 255, 10, 0, 13, 10, 0];
    let binary_id = repo.blob(&binary);
    let got = reader()
        .read_blobs(&repo.git_dir(), &[text_id.clone(), binary_id.clone()])
        .unwrap();
    let shown = |id: &str| {
        git_bytes(
            repo.root().path(),
            repo.work(),
            &["cat-file", "blob", id],
            None,
        )
    };
    assert_eq!(got[0], shown(&text_id));
    assert_eq!(got[1], shown(&binary_id));
    assert_eq!(got[1], binary);
}

#[test]
fn read_blobs_with_no_ids_is_empty_and_runs_no_git() {
    let repo = nested();
    let marker = repo.root().join("ran");
    let fake = fake_git(repo.root(), &format!("touch {}", marker.display()));
    let got = reader()
        .with_path(fake.0.clone())
        .read_blobs(&repo.git_dir(), &[])
        .unwrap();
    assert!(got.is_empty());
    assert!(!marker.exists(), "git ran");
}

/// Writes `count` blobs of about `size` bytes each, all different, and returns their ids and contents.
fn many_blobs(repo: &GitRepo, count: usize, size: usize) -> (Vec<String>, Vec<Vec<u8>>) {
    let folder = repo.root().join("blobs");
    fs::create_dir(&folder).unwrap();
    let mut paths = String::new();
    let mut bodies = Vec::new();
    for n in 0..count {
        let mut body = format!("blob number {n}\n").into_bytes();
        body.resize(size, b'x');
        let path = folder.join(format!("{n:05}"));
        fs::write(&path, &body).unwrap();
        paths.push_str(&format!("{}\n", path.display()));
        bodies.push(body);
    }
    let out = repo.git_with_input(&["hash-object", "-w", "--stdin-paths"], paths.as_bytes());
    let ids: Vec<String> = out.lines().map(str::to_string).collect();
    assert_eq!(ids.len(), count);
    (ids, bodies)
}

#[test]
fn read_blobs_returns_one_blob_in_the_order_asked() {
    let repo = GitRepo::new();
    let (ids, bodies) = many_blobs(&repo, 3, 100);
    let reversed = vec![ids[2].clone(), ids[0].clone(), ids[1].clone()];
    let got = reader().read_blobs(&repo.git_dir(), &reversed).unwrap();
    assert_eq!(
        got,
        [bodies[2].clone(), bodies[0].clone(), bodies[1].clone()]
    );
    let one = reader().read_blobs(&repo.git_dir(), &ids[..1]).unwrap();
    assert_eq!(one, [bodies[0].clone()]);
}

#[test]
fn read_blobs_reads_1000_blobs_of_4_kib_in_order() {
    let repo = GitRepo::new();
    let (ids, bodies) = many_blobs(&repo, 1_000, 4096);
    assert_eq!(reader().read_blobs(&repo.git_dir(), &ids).unwrap(), bodies);
}

#[test]
fn read_blobs_reads_5000_blobs_in_order() {
    let repo = GitRepo::new();
    let (ids, bodies) = many_blobs(&repo, 5_000, 1024);
    assert_eq!(reader().read_blobs(&repo.git_dir(), &ids).unwrap(), bodies);
}

#[test]
fn read_blobs_with_a_missing_object_names_the_id() {
    let repo = GitRepo::new();
    let (mut ids, _) = many_blobs(&repo, 2, 10);
    let missing = "0123456789012345678901234567890123456789".to_string();
    ids.insert(1, missing.clone());
    let error = text(reader().read_blobs(&repo.git_dir(), &ids).unwrap_err());
    assert!(error.contains(&missing), "{error}");
}

#[test]
fn read_blobs_with_a_tree_id_is_an_error_naming_the_id() {
    let repo = nested();
    let tree = repo.git(&["rev-parse", "main:a"]);
    let error = text(
        reader()
            .read_blobs(&repo.git_dir(), std::slice::from_ref(&tree))
            .unwrap_err(),
    );
    assert!(error.contains(&tree) && error.contains("tree"), "{error}");
}

#[test]
fn read_blobs_refuses_an_id_that_is_not_a_full_hex_id() {
    let repo = nested();
    for bad in [
        "main:top.txt",
        "abc",
        "HEAD",
        "0123456789012345678901234567890123456789\nHEAD",
        "",
    ] {
        let error = reader()
            .read_blobs(&repo.git_dir(), &[bad.to_string()])
            .unwrap_err();
        assert!(
            matches!(error, GitError::Refused { .. }),
            "{bad:?}: {error}"
        );
    }
}

#[test]
fn read_blobs_refuses_a_blob_over_the_size_cap_naming_the_id() {
    let repo = GitRepo::new();
    let id = repo.blob(&vec![b'a'; MAX_BLOB_BYTES as usize + 1]);
    let error = text(
        reader()
            .read_blobs(&repo.git_dir(), std::slice::from_ref(&id))
            .unwrap_err(),
    );
    assert!(error.contains(&id), "{error}");
}

#[test]
fn read_blobs_gives_the_original_when_a_replace_ref_swaps_a_blob() {
    let repo = GitRepo::new();
    let original = repo.blob(b"original\n");
    let swapped = repo.blob(b"swapped\n");
    repo.git(&["replace", &original, &swapped]);
    let by_git = git_bytes(
        repo.root().path(),
        repo.work(),
        &["cat-file", "blob", &original],
        None,
    );
    assert_eq!(
        by_git, b"swapped\n",
        "the fixture's own git follows the replace ref"
    );
    let got = reader().read_blobs(&repo.git_dir(), &[original]).unwrap();
    assert_eq!(got, [b"original\n".to_vec()]);
}

// ---- blob_at ----

#[test]
fn blob_at_a_file_gives_its_bytes() {
    let repo = nested();
    let got = reader()
        .blob_at(&repo.git_dir(), "main", "a/b/c.txt")
        .unwrap();
    assert_eq!(got.as_deref(), Some(&b"deep\n"[..]));
}

#[test]
fn blob_at_a_missing_file_is_none() {
    let repo = nested();
    assert_eq!(
        reader()
            .blob_at(&repo.git_dir(), "main", "a/none.yaml")
            .unwrap(),
        None
    );
}

#[test]
fn blob_at_a_missing_ref_is_an_error() {
    let repo = nested();
    let error = text(
        reader()
            .blob_at(&repo.git_dir(), "nope", "top.txt")
            .unwrap_err(),
    );
    assert!(error.contains("nope"), "{error}");
}

#[test]
fn blob_at_a_folder_is_an_error_naming_the_path() {
    let repo = nested();
    let error = text(
        reader()
            .blob_at(&repo.git_dir(), "main", "a/b")
            .unwrap_err(),
    );
    assert!(error.contains("a/b"), "{error}");
}

#[test]
fn blob_at_a_file_over_the_yaml_cap_is_an_error_naming_the_path() {
    let repo = GitRepo::new();
    repo.write("board.yaml", &vec![b'#'; MAX_YAML_BYTES + 1]);
    repo.write("ok.yaml", &vec![b'#'; MAX_YAML_BYTES]);
    repo.commit_all("one");
    let error = text(
        reader()
            .blob_at(&repo.git_dir(), "main", "board.yaml")
            .unwrap_err(),
    );
    assert!(error.contains("board.yaml"), "{error}");
    let ok = reader()
        .blob_at(&repo.git_dir(), "main", "ok.yaml")
        .unwrap();
    assert_eq!(ok.map(|bytes| bytes.len()), Some(MAX_YAML_BYTES));
}

// ---- tree_of_folder ----

/// What git says for the files in the repository's folder: added by `git add --all --force`, so an ignored file
/// counts, then `write-tree`. The folder's own `.git` is never added by git.
fn git_write_tree(repo: &GitRepo) -> String {
    repo.git(&["add", "--all", "--force"]);
    repo.git(&["write-tree"])
}

fn assert_folder_matches_git(repo: &GitRepo) {
    let expected = git_write_tree(repo);
    let got = reader().tree_of_folder(repo.work()).unwrap();
    assert_eq!(got, expected);
}

#[test]
fn a_folder_of_plain_files_has_the_id_git_gives() {
    let repo = GitRepo::new();
    repo.write("pack.yaml", b"name: x\n");
    repo.write("hooks/a.ts", b"export {}\n");
    assert_folder_matches_git(&repo);
}

#[test]
fn an_executable_file_changes_the_id_and_matches_git() {
    let repo = GitRepo::new();
    repo.write("run.sh", b"#!/bin/sh\n");
    let plain = reader().tree_of_folder(repo.work()).unwrap();
    repo.write_executable("run.sh", b"#!/bin/sh\n");
    assert_folder_matches_git(&repo);
    assert_ne!(reader().tree_of_folder(repo.work()).unwrap(), plain);
}

#[test]
fn an_ignored_file_and_a_node_modules_folder_are_counted() {
    let repo = GitRepo::new();
    repo.write(".gitignore", b"secret.txt\nnode_modules/\n");
    repo.write("secret.txt", b"counted anyway\n");
    repo.write("node_modules/dep/index.js", b"x\n");
    repo.write("kept.txt", b"k\n");
    assert_folder_matches_git(&repo);
    let without = GitRepo::new();
    without.write(".gitignore", b"secret.txt\nnode_modules/\n");
    without.write("kept.txt", b"k\n");
    assert_ne!(
        reader().tree_of_folder(repo.work()).unwrap(),
        reader().tree_of_folder(without.work()).unwrap()
    );
}

#[test]
fn a_symlink_is_hashed_as_a_link_and_not_followed() {
    let repo = GitRepo::new();
    repo.write("real.txt", b"r\n");
    repo.symlink("link", "real.txt");
    repo.symlink("dangling", "/no/such/place");
    repo.symlink("out", "..");
    assert_folder_matches_git(&repo);
}

#[test]
fn nested_folders_match_and_an_empty_folder_is_absent() {
    let repo = GitRepo::new();
    repo.write("a/b/c/d.txt", b"d\n");
    let without_empty = reader().tree_of_folder(repo.work()).unwrap();
    fs::create_dir_all(repo.work().join("empty/inner")).unwrap();
    assert_folder_matches_git(&repo);
    assert_eq!(reader().tree_of_folder(repo.work()).unwrap(), without_empty);
}

#[test]
fn a_folder_with_no_files_has_the_empty_tree_id() {
    let repo = GitRepo::new();
    assert_eq!(reader().tree_of_folder(repo.work()).unwrap(), EMPTY_TREE);
}

#[test]
fn two_folders_with_equal_files_have_the_same_id_with_or_without_a_repository() {
    let one = GitRepo::new();
    let two = GitRepo::new();
    for repo in [&one, &two] {
        repo.write("pack.yaml", b"name: x\n");
        repo.write_executable("bin/run", b"#!/bin/sh\n");
    }
    let plain = one.root().join("plain");
    fs::create_dir_all(plain.join("bin")).unwrap();
    fs::write(plain.join("pack.yaml"), b"name: x\n").unwrap();
    fs::write(plain.join("bin/run"), b"#!/bin/sh\n").unwrap();
    fs::set_permissions(plain.join("bin/run"), fs::Permissions::from_mode(0o755)).unwrap();
    let id = reader().tree_of_folder(one.work()).unwrap();
    assert_eq!(reader().tree_of_folder(two.work()).unwrap(), id);
    assert_eq!(reader().tree_of_folder(&plain).unwrap(), id);
}

#[test]
fn a_folder_has_the_id_of_the_commit_that_holds_its_files() {
    let repo = nested();
    let committed = reader().tree_at(&repo.git_dir(), "main", "").unwrap();
    assert_eq!(reader().tree_of_folder(repo.work()).unwrap(), committed.id);
}

#[test]
fn a_gitattributes_file_does_not_change_the_bytes_that_are_hashed() {
    let repo = GitRepo::new();
    repo.write(".gitattributes", b"* text eol=lf\n*.txt ident\n");
    repo.write("crlf.txt", b"one\r\ntwo\r\n");
    repo.write("id.txt", b"$Id$\n");
    let attributes = repo.blob(b"* text eol=lf\n*.txt ident\n");
    let crlf = repo.blob(b"one\r\ntwo\r\n");
    let ident = repo.blob(b"$Id$\n");
    let expected = repo.tree_of(&[
        ("100644", &attributes, b".gitattributes"),
        ("100644", &crlf, b"crlf.txt"),
        ("100644", &ident, b"id.txt"),
    ]);
    assert_eq!(reader().tree_of_folder(repo.work()).unwrap(), expected);
}

#[test]
fn a_repository_config_that_names_an_fsmonitor_is_never_run_when_a_folder_is_hashed() {
    let repo = GitRepo::new();
    repo.write("kept.txt", b"k\n");
    let marker = repo.root().join("fsmonitor-ran");
    let script = repo.root().script(
        "fsmonitor.sh",
        &format!("touch {}\nprintf '\\0'\n", marker.display()),
    );
    repo.git(&["config", "core.fsmonitor", script.to_str().unwrap()]);
    repo.git(&["status", "--short"]);
    assert!(
        marker.exists(),
        "the fixture's own git runs the fsmonitor, so the test can see it"
    );
    fs::remove_file(&marker).unwrap();

    reader().tree_of_folder(repo.work()).unwrap();
    assert!(!marker.exists(), "the fsmonitor ran");
}

#[test]
fn a_repository_config_that_names_an_fsmonitor_is_never_run_when_objects_are_read() {
    let repo = nested();
    let marker = repo.root().join("fsmonitor-ran");
    let script = repo.root().script(
        "fsmonitor.sh",
        &format!("touch {}\nprintf '\\0'\n", marker.display()),
    );
    repo.git(&["config", "core.fsmonitor", script.to_str().unwrap()]);
    repo.git(&["status", "--short"]);
    assert!(marker.exists());
    fs::remove_file(&marker).unwrap();

    let tree = reader().tree_at(&repo.git_dir(), "main", "").unwrap();
    let entries = reader().list(&repo.git_dir(), &tree.id).unwrap();
    let ids: Vec<String> = entries.into_iter().map(|e| e.id).collect();
    reader().read_blobs(&repo.git_dir(), &ids).unwrap();
    reader()
        .blob_at(&repo.git_dir(), "main", "top.txt")
        .unwrap();
    assert!(!marker.exists(), "the fsmonitor ran");
}

#[test]
fn a_folder_that_is_not_there_or_is_a_file_is_an_error_naming_it() {
    let repo = nested();
    let missing = repo.root().join("nowhere");
    let error = text(reader().tree_of_folder(&missing).unwrap_err());
    assert!(error.contains("nowhere"), "{error}");
    let error = text(
        reader()
            .tree_of_folder(&repo.work().join("top.txt"))
            .unwrap_err(),
    );
    assert!(error.contains("top.txt"), "{error}");
}

fn entries_of(folder: &Path) -> BTreeSet<String> {
    fs::read_dir(folder)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().to_string())
        .collect()
}

#[test]
fn tree_of_folder_writes_only_in_its_scratch_folder_and_removes_it() {
    let repo = nested();
    let scratch = repo.root().join("scratch");
    fs::create_dir(&scratch).unwrap();
    let before = entries_of(repo.work());
    let id = reader()
        .with_scratch_dir(&scratch)
        .tree_of_folder(repo.work())
        .unwrap();
    assert_eq!(id.len(), 40);
    assert!(
        entries_of(&scratch).is_empty(),
        "left {:?}",
        entries_of(&scratch)
    );
    assert_eq!(entries_of(repo.work()), before);
}

#[test]
fn tree_of_folder_removes_its_scratch_folder_when_git_fails() {
    let repo = nested();
    let scratch = repo.root().join("scratch");
    fs::create_dir(&scratch).unwrap();
    let real = real_git();
    let fake = fake_git(
        repo.root(),
        &format!(
            "case \"$*\" in *' add '*) echo 'add failed' >&2; exit 2;; esac\nexec {} \"$@\"",
            real.display()
        ),
    );
    let error = text(
        reader()
            .with_path(fake.0.clone())
            .with_scratch_dir(&scratch)
            .tree_of_folder(repo.work())
            .unwrap_err(),
    );
    assert!(error.contains("add failed"), "{error}");
    assert!(
        entries_of(&scratch).is_empty(),
        "left {:?}",
        entries_of(&scratch)
    );
}

// ---- how git is run ----

/// The variable names and values of one recorded call: `(args, env)`.
fn calls(log: &Path) -> Vec<(String, Vec<String>)> {
    let log = fs::read_to_string(log).unwrap();
    let mut out = Vec::new();
    let mut current: Option<(String, Vec<String>)> = None;
    for line in log.lines() {
        if let Some(args) = line.strip_prefix("ARGS ") {
            current = Some((args.to_string(), Vec::new()));
        } else if line == "END" {
            out.extend(current.take());
        } else if let Some((_, env)) = current.as_mut() {
            // The shell that runs the fake adds these itself.
            let name = line.split('=').next().unwrap();
            if !["PWD", "OLDPWD", "SHLVL", "_"].contains(&name) {
                env.push(line.to_string());
            }
        }
    }
    out
}

fn recording_git(root: &TempRoot) -> (OsPath, PathBuf) {
    let log = root.join("calls.log");
    let real = real_git();
    let fake = fake_git(
        root,
        &format!(
            "{{ echo \"ARGS $*\"; /usr/bin/env; echo END; }} >> {}\nexec {} \"$@\"",
            log.display(),
            real.display()
        ),
    );
    (fake, log)
}

fn names(env: &[String]) -> BTreeSet<&str> {
    env.iter()
        .map(|line| line.split('=').next().unwrap())
        .collect()
}

#[test]
fn git_is_run_with_a_clean_environment_and_the_hardening_variables() {
    let repo = nested();
    let (fake, log) = recording_git(repo.root());
    let git_dir = repo.git_dir();
    reader()
        .with_path(fake.0.clone())
        .tree_at(&git_dir, "main", "a")
        .unwrap();
    let calls = calls(&log);
    assert!(!calls.is_empty());
    let expected: BTreeSet<&str> = [
        "PATH",
        "LC_ALL",
        "GIT_TERMINAL_PROMPT",
        "GIT_OPTIONAL_LOCKS",
        "GIT_NO_REPLACE_OBJECTS",
        "GIT_CONFIG_GLOBAL",
        "GIT_CONFIG_SYSTEM",
        "GIT_CONFIG_NOSYSTEM",
        "GIT_ATTR_NOSYSTEM",
        "GIT_DIR",
    ]
    .into();
    for (args, env) in &calls {
        assert_eq!(names(env), expected, "for `git {args}`");
        for line in [
            "LC_ALL=C",
            "GIT_TERMINAL_PROMPT=0",
            "GIT_OPTIONAL_LOCKS=0",
            "GIT_NO_REPLACE_OBJECTS=1",
            "GIT_CONFIG_GLOBAL=/dev/null",
            "GIT_CONFIG_SYSTEM=/dev/null",
            &format!("PATH={}", fake.0.display()),
            &format!("GIT_DIR={}", git_dir.display()),
        ] {
            assert!(env.contains(&line.to_string()), "`git {args}` lacks {line}");
        }
    }
}

#[test]
fn the_add_of_a_folder_hash_names_the_private_repository_and_the_folder() {
    let repo = nested();
    let (fake, log) = recording_git(repo.root());
    let scratch = repo.root().join("scratch");
    fs::create_dir(&scratch).unwrap();
    reader()
        .with_path(fake.0.clone())
        .with_scratch_dir(&scratch)
        .tree_of_folder(repo.work())
        .unwrap();
    let calls = calls(&log);
    let (_, env) = calls
        .iter()
        .find(|(args, _)| args.contains(" add "))
        .expect("an add");
    let value = |name: &str| {
        env.iter()
            .find_map(|line| line.strip_prefix(&format!("{name}=")))
            .unwrap_or_else(|| panic!("no {name}"))
            .to_string()
    };
    let git_dir = value("GIT_DIR");
    assert!(git_dir.starts_with(scratch.to_str().unwrap()), "{git_dir}");
    assert_eq!(value("GIT_WORK_TREE"), repo.work().display().to_string());
    assert_eq!(value("GIT_INDEX_FILE"), format!("{git_dir}/index"));
    assert_eq!(value("GIT_NO_REPLACE_OBJECTS"), "1");
    for (args, env) in &calls {
        assert!(!names(env).contains("HOME"), "`git {args}` has HOME");
    }
}

#[test]
fn a_git_that_hangs_fails_within_the_timeout_naming_the_command() {
    let repo = nested();
    let fake = fake_git(repo.root(), "exec sleep 60");
    let started = Instant::now();
    let error = text(
        reader()
            .with_path(fake.0.clone())
            .with_timeout(Duration::from_millis(300))
            .tree_at(&repo.git_dir(), "main", "a")
            .unwrap_err(),
    );
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "{:?}",
        started.elapsed()
    );
    assert!(error.contains("git rev-parse"), "{error}");
}

#[test]
fn a_git_that_exits_non_zero_gives_the_command_the_code_and_the_stderr_tail() {
    let repo = nested();
    let fake = fake_git(repo.root(), "echo 'fatal: it broke badly' >&2\nexit 3");
    let error = text(
        reader()
            .with_path(fake.0.clone())
            .tree_at(&repo.git_dir(), "main", "a")
            .unwrap_err(),
    );
    assert!(error.contains("git rev-parse"), "{error}");
    assert!(error.contains('3'), "{error}");
    assert!(error.contains("fatal: it broke badly"), "{error}");
}

#[test]
fn a_missing_git_is_an_error_naming_the_command() {
    let repo = nested();
    let empty = repo.root().join("empty-bin");
    fs::create_dir(&empty).unwrap();
    let error = text(
        reader()
            .with_path(empty)
            .tree_at(&repo.git_dir(), "main", "a")
            .unwrap_err(),
    );
    assert!(error.contains("git rev-parse"), "{error}");
}
