//! Tests for the snapshot materializer and the SDK copy (ts-0095-kp). Every repository is built by `git` in a temp root
//! with a clean environment (`common::git_repo`), and the cache and the SDK stand-in sit in the same root, so no test
//! reads the real home, the real cache or the real git config. The reader clears the environment of its own git calls.
//! A child process of the race test gets a cleared environment with `HOME` and `COLLIDE_DAEMON_ROOT` set to the root.

use collide_host::git_objects::{GitObjects, MAX_TREE_ENTRIES, TreeEntry, tree_of_folder};
use collide_host::pack_cache::{
    CacheError, FsWriter, MARKER, Materializer, PackWriter, check_entries, ensure_sdk, entry_name,
    materialize,
};
use collide_host::pack_location::bundle_sdk;
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime};

mod common;
use common::git_repo::GitRepo;
use common::{TempRoot, find_on_path, node_missing_message};

const EMPTY_TREE: &str = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";

/// The environment variables the race test's child process reads. A child has them; the parent test run has none.
const CHILD_CACHE: &str = "COLLIDE_TEST_PACK_CACHE";
const CHILD_SDK: &str = "COLLIDE_TEST_PACK_SDK";
const CHILD_REPO: &str = "COLLIDE_TEST_PACK_REPO";
const CHILD_TREE: &str = "COLLIDE_TEST_PACK_TREE";
const CHILD_GO: &str = "COLLIDE_TEST_PACK_GO";
const CHILD_OUT: &str = "COLLIDE_TEST_PACK_OUT";

/// A pack repository with the cache and an SDK stand-in beside it.
struct Fixture {
    repo: GitRepo,
    cache: PathBuf,
    sdk: PathBuf,
}

impl Fixture {
    fn new() -> Fixture {
        let repo = GitRepo::new();
        let cache = repo.root().join("cache");
        let sdk = repo.root().join("sdk-copy");
        make_sdk(&sdk, "0.1.0");
        Fixture { repo, cache, sdk }
    }

    /// The `ordinary` pack, committed.
    fn ordinary() -> Fixture {
        let fixture = Fixture::new();
        write_ordinary(&fixture.repo);
        fixture.repo.commit_all("one");
        fixture
    }

    fn tree(&self) -> String {
        self.repo.git(&["rev-parse", "HEAD^{tree}"])
    }

    fn git_dir(&self) -> PathBuf {
        self.repo.git_dir()
    }

    fn materialize(&self, tree: &str) -> Result<PathBuf, CacheError> {
        materialize(&self.cache, &self.sdk, &self.git_dir(), tree)
    }

    fn with(&self, materializer: &Materializer, tree: &str) -> Result<PathBuf, CacheError> {
        materializer.materialize(&self.cache, &self.sdk, &self.git_dir(), tree)
    }

    /// What the cache's `packs` folder holds, by name. Empty when the folder is not there.
    fn packs(&self) -> Vec<String> {
        names_in(&self.cache.join("packs"))
    }

    fn entry(&self, tree: &str) -> PathBuf {
        self.cache.join("packs").join(entry_name(tree, "0.1.0"))
    }
}

fn names_in(dir: &Path) -> Vec<String> {
    let Ok(read) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut names: Vec<String> = read
        .map(|entry| entry.unwrap().file_name().to_string_lossy().to_string())
        .collect();
    names.sort();
    names
}

fn make_sdk(dir: &Path, version: &str) {
    fs::create_dir_all(dir.join("dist")).unwrap();
    fs::write(
        dir.join("package.json"),
        format!(
            "{{\"name\":\"@collide/pack-sdk\",\"version\":\"{version}\",\"type\":\"module\"}}\n"
        ),
    )
    .unwrap();
    fs::write(
        dir.join("dist/index.js"),
        "export const name = 'stand-in'\n",
    )
    .unwrap();
}

const NUL_BYTES: &[u8] = b"\0\x01\0binary\0\xff\xfe\0";

fn write_ordinary(repo: &GitRepo) {
    repo.write("pack.yaml", b"name: ordinary\n");
    repo.write_executable("hooks/run.sh", b"#!/bin/sh\necho hi\n");
    repo.write("hooks/lib/deep/util.ts", b"export const util = 1\n");
    repo.write("empty.txt", b"");
    repo.write("data/nul.bin", NUL_BYTES);
    repo.write("a name with spaces.md", b"words\n");
}

/// Every file below `dir`, by path: its bytes and its mode. Symlinks are not followed and are not listed.
fn files_under(dir: &Path) -> BTreeMap<String, (Vec<u8>, u32)> {
    fn walk(base: &Path, dir: &Path, found: &mut BTreeMap<String, (Vec<u8>, u32)>) {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            let meta = fs::symlink_metadata(&path).unwrap();
            if meta.is_dir() {
                walk(base, &path, found);
            } else if meta.is_file() {
                let rel = path
                    .strip_prefix(base)
                    .unwrap()
                    .to_string_lossy()
                    .to_string();
                found.insert(
                    rel,
                    (fs::read(&path).unwrap(), meta.permissions().mode() & 0o777),
                );
            }
        }
    }
    let mut found = BTreeMap::new();
    walk(dir, dir, &mut found);
    found
}

fn mode_of(path: &Path) -> u32 {
    fs::symlink_metadata(path).unwrap().permissions().mode() & 0o777
}

/// A tree object of `(mode, kind, id, name)` entries, names as raw bytes. `--missing` lets a submodule's commit id name
/// an object the repository does not hold, as it does in a real tree.
fn mktree(repo: &GitRepo, entries: &[(&str, &str, &str, &[u8])]) -> String {
    let mut input = Vec::new();
    for (mode, kind, id, name) in entries {
        input.extend_from_slice(format!("{mode} {kind} {id}\t").as_bytes());
        input.extend_from_slice(name);
        input.push(0);
    }
    repo.git_with_input(&["mktree", "-z", "--missing"], &input)
}

fn text(error: CacheError) -> String {
    error.to_string()
}

/// A folder `bin` in the temp root holding a `git` made from `body`, and the `PATH` that finds it first.
fn fake_git(root: &TempRoot, body: &str) -> PathBuf {
    let dir = root.join("fake-bin");
    fs::create_dir_all(&dir).unwrap();
    root.script("fake-bin/git", &format!("#!/bin/sh\n{body}\n"));
    dir
}

fn assert_nothing_left(fixture: &Fixture) {
    assert_eq!(
        fixture.packs(),
        Vec::<String>::new(),
        "the packs folder is not clean"
    );
}

// ---- the happy path ----

#[test]
fn an_ordinary_tree_materializes_with_its_bytes_and_its_executable_bits() {
    let fixture = Fixture::ordinary();
    let entry = fixture.materialize(&fixture.tree()).unwrap();
    assert_eq!(entry, fixture.entry(&fixture.tree()));
    let found = files_under(&entry.join("pack"));
    let paths: Vec<&str> = found.keys().map(String::as_str).collect();
    assert_eq!(
        paths,
        [
            "a name with spaces.md",
            "data/nul.bin",
            "empty.txt",
            "hooks/lib/deep/util.ts",
            "hooks/run.sh",
            "pack.yaml"
        ]
    );
    assert_eq!(found["data/nul.bin"].0, NUL_BYTES);
    assert_eq!(found["empty.txt"].0, b"");
    assert_eq!(found["hooks/run.sh"].0, b"#!/bin/sh\necho hi\n");
    assert_eq!(
        found["hooks/run.sh"].1 & 0o111,
        0o111,
        "the executable keeps its bit"
    );
    assert_eq!(found["pack.yaml"].1 & 0o111, 0, "a plain file gets none");
}

#[test]
fn the_entry_holds_collides_package_json_the_sdk_link_and_the_marker() {
    let fixture = Fixture::ordinary();
    let entry = fixture.materialize(&fixture.tree()).unwrap();
    assert_eq!(
        fs::read_to_string(entry.join("package.json"))
            .unwrap()
            .trim_end(),
        r#"{"private":true,"type":"module"}"#
    );
    let link = entry.join("node_modules/@collide/pack-sdk");
    assert!(
        fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(fs::read_link(&link).unwrap(), fixture.sdk);
    assert!(
        link.join("package.json").is_file(),
        "the link resolves to the SDK copy"
    );
    assert!(entry.join(MARKER).is_file());
    let mut names = names_in(&entry);
    names.sort();
    assert_eq!(names, [MARKER, "node_modules", "pack", "package.json"]);
}

#[test]
fn the_entry_hashes_to_its_tree_id() {
    let fixture = Fixture::ordinary();
    let entry = fixture.materialize(&fixture.tree()).unwrap();
    assert_eq!(tree_of_folder(&entry.join("pack")).unwrap(), fixture.tree());
}

#[test]
fn the_entry_is_read_only_down_to_the_top_folder() {
    let fixture = Fixture::ordinary();
    let entry = fixture.materialize(&fixture.tree()).unwrap();
    assert_eq!(mode_of(&entry), 0o555);
    assert_eq!(mode_of(&entry.join("pack")), 0o555);
    assert_eq!(mode_of(&entry.join("pack/hooks/lib/deep")), 0o555);
    assert_eq!(mode_of(&entry.join("pack/hooks/run.sh")), 0o555);
    assert_eq!(mode_of(&entry.join("pack/pack.yaml")), 0o444);
    assert_eq!(mode_of(&entry.join("package.json")), 0o444);
    assert_eq!(mode_of(&entry.join(MARKER)), 0o444);
    assert_eq!(mode_of(&entry.join("node_modules/@collide")), 0o555);
    assert!(fs::write(entry.join("pack/pack.yaml"), "x").is_err());
    assert!(fs::write(entry.join("pack/new.txt"), "x").is_err());
}

#[test]
fn an_empty_tree_makes_an_empty_pack() {
    let fixture = Fixture::new();
    let tree = fixture.repo.tree_of(&[]);
    assert_eq!(tree, EMPTY_TREE);
    let entry = fixture.materialize(&tree).unwrap();
    assert!(names_in(&entry.join("pack")).is_empty());
    assert!(entry.join(MARKER).is_file());
}

#[test]
fn a_tree_of_a_folder_below_the_top_materializes_as_its_own_pack() {
    let fixture = Fixture::ordinary();
    let sub = fixture.repo.git(&["rev-parse", "HEAD:hooks"]);
    let entry = fixture.materialize(&sub).unwrap();
    let found = files_under(&entry.join("pack"));
    assert_eq!(
        found.keys().map(String::as_str).collect::<Vec<_>>(),
        ["lib/deep/util.ts", "run.sh"]
    );
}

#[test]
fn a_tree_id_that_is_not_an_id_is_refused_and_nothing_is_made() {
    let fixture = Fixture::ordinary();
    for bad in ["../outside", "main", "", "ABCDEF", "4b825dc6"] {
        let error = text(fixture.materialize(bad).unwrap_err());
        assert!(error.contains("tree id"), "{bad}: {error}");
    }
    assert!(!fixture.cache.exists());
}

// ---- refusals ----

/// A fixture and a tree made by hand in its repository. An entry whose id is `"blob"` gets the id of a real blob: a
/// tree needs the blob in the repository for `ls-tree` to give its size.
fn hand_made(entries: &[(&str, &str, &str, &[u8])]) -> (Fixture, String) {
    let fixture = Fixture::new();
    let blob = fixture.repo.blob(b"content\n");
    let entries: Vec<(&str, &str, &str, &[u8])> = entries
        .iter()
        .map(|(mode, kind, id, name)| {
            (
                *mode,
                *kind,
                if *id == "blob" { blob.as_str() } else { *id },
                *name,
            )
        })
        .collect();
    let tree = mktree(&fixture.repo, &entries);
    (fixture, tree)
}

fn refused_naming(fixture: &Fixture, tree: &str, path: &str) -> String {
    let error = text(fixture.materialize(tree).unwrap_err());
    assert!(
        error.contains(&format!("`{path}`")),
        "the message names {path}: {error}"
    );
    assert_nothing_left(fixture);
    error
}

#[test]
fn a_symlink_is_refused_naming_its_path_and_nothing_is_written() {
    let fixture = Fixture::new();
    fixture.repo.write("ok.txt", b"ok\n");
    fixture.repo.symlink("hooks/link", "/etc/passwd");
    fixture.repo.commit_all("one");
    let error = refused_naming(&fixture, &fixture.tree(), "hooks/link");
    assert!(error.contains("symlink"), "{error}");
}

#[test]
fn a_submodule_is_refused_naming_its_path() {
    let commit = "1".repeat(40);
    let (fixture, tree) = hand_made(&[
        ("160000", "commit", &commit, b"vendor"),
        ("100644", "blob", "blob", b"x"),
    ]);
    let error = refused_naming(&fixture, &tree, "vendor");
    assert!(error.contains("submodule"), "{error}");
}

#[test]
fn a_file_under_node_modules_is_refused_naming_its_path() {
    let fixture = Fixture::new();
    fixture.repo.write("node_modules/x/y.js", b"export {}\n");
    fixture.repo.write("pack.yaml", b"name: x\n");
    fixture.repo.commit_all("one");
    let error = refused_naming(&fixture, &fixture.tree(), "node_modules/x/y.js");
    assert!(error.contains("node_modules"), "{error}");
}

#[test]
fn a_node_modules_folder_below_the_top_is_refused_too() {
    let fixture = Fixture::new();
    fixture
        .repo
        .write("hooks/node_modules/x/y.js", b"export {}\n");
    fixture.repo.commit_all("one");
    refused_naming(&fixture, &fixture.tree(), "hooks/node_modules/x/y.js");
}

#[test]
fn a_committed_sdk_is_refused_and_none_of_the_pack_is_written() {
    let fixture = Fixture::new();
    fixture.repo.write("pack.yaml", b"name: x\n");
    fixture.repo.write("hooks/a.ts", b"export {}\n");
    fixture.repo.write(
        "node_modules/@collide/pack-sdk/dist/index.js",
        b"export const name = 'mine'\n",
    );
    fixture
        .repo
        .write("node_modules/@collide/pack-sdk/package.json", b"{}\n");
    fixture.repo.commit_all("one");
    let error = text(fixture.materialize(&fixture.tree()).unwrap_err());
    assert!(error.contains("node_modules/@collide/pack-sdk/"), "{error}");
    assert_nothing_left(&fixture);
}

#[test]
fn a_package_json_at_any_depth_is_refused_naming_its_path() {
    for path in ["hooks/package.json", "package.json", "a/b/c/package.json"] {
        let fixture = Fixture::new();
        fixture.repo.write(path, b"{}\n");
        fixture.repo.commit_all("one");
        let error = refused_naming(&fixture, &fixture.tree(), path);
        assert!(error.contains("package.json"), "{error}");
    }
}

#[test]
fn a_package_json_or_node_modules_in_another_case_is_refused() {
    for path in ["hooks/Package.JSON", "NODE_MODULES/x.js"] {
        let fixture = Fixture::new();
        fixture.repo.write(path, b"{}\n");
        fixture.repo.commit_all("one");
        refused_naming(&fixture, &fixture.tree(), path);
    }
}

#[test]
fn a_package_json_folder_or_a_longer_name_is_not_refused() {
    let fixture = Fixture::new();
    fixture.repo.write("package.json.md", b"notes\n");
    fixture.repo.write("my-node_modules/x.js", b"export {}\n");
    fixture.repo.write("node_modules.txt", b"x\n");
    fixture.repo.commit_all("one");
    fixture.materialize(&fixture.tree()).unwrap();
}

#[test]
fn a_dot_git_part_is_refused_in_any_case_and_names_the_path() {
    for name in [".git", ".GIT", ".Git"] {
        let (fixture, inner) = hand_made(&[("100644", "blob", "blob", b"hook")]);
        let tree = mktree(
            &fixture.repo,
            &[("040000", "tree", &inner, name.as_bytes())],
        );
        refused_naming(&fixture, &tree, &format!("{name}/hook"));
    }
}

#[test]
fn a_dot_dot_part_is_refused_naming_the_path_and_nothing_is_written_outside() {
    let (fixture, inner) = hand_made(&[("100644", "blob", "blob", b"escape.txt")]);
    let tree = mktree(&fixture.repo, &[("040000", "tree", &inner, b"..")]);
    refused_naming(&fixture, &tree, "../escape.txt");
    assert!(!fixture.cache.join("escape.txt").exists());
    assert!(!fixture.cache.join("packs/escape.txt").exists());
}

#[test]
fn a_dot_part_and_a_backslash_are_refused() {
    let (fixture, inner) = hand_made(&[("100644", "blob", "blob", b"x")]);
    let tree = mktree(&fixture.repo, &[("040000", "tree", &inner, b".")]);
    refused_naming(&fixture, &tree, "./x");
    let (fixture, tree) = hand_made(&[("100644", "blob", "blob", b"a\\b")]);
    refused_naming(&fixture, &tree, "a\\b");
}

fn entry_at(mode: &str, path: &str) -> TreeEntry {
    use collide_host::git_objects::ObjectKind;
    TreeEntry {
        mode: mode.to_string(),
        kind: ObjectKind::Blob,
        id: "6".repeat(40),
        path: path.to_string(),
        size: 1,
    }
}

#[test]
fn paths_with_an_empty_part_a_nul_or_a_leading_slash_are_refused_by_the_check() {
    for path in [
        "a//b",
        "/abs",
        "trailing/",
        "a\0b",
        "x/./y",
        "x/../y",
        ".git/config",
        "a/.GiT/b",
        "a\\b",
    ] {
        let error = check_entries(&[entry_at("100644", path)])
            .unwrap_err()
            .to_string();
        assert!(
            error.contains(&path.replace('\0', "\\0")),
            "{path:?}: {error}"
        );
    }
    check_entries(&[entry_at("100644", "a/b.txt"), entry_at("100755", "run.sh")]).unwrap();
}

#[test]
fn a_mode_that_is_not_a_files_is_refused_by_the_check() {
    let error = check_entries(&[entry_at("100664", "odd")])
        .unwrap_err()
        .to_string();
    assert!(error.contains("`odd`"), "{error}");
}

#[test]
fn a_tree_over_the_entry_cap_is_refused_before_any_write() {
    let fixture = Fixture::new();
    for n in 0..=MAX_TREE_ENTRIES {
        fixture.repo.write(&format!("f{n:05}.txt"), b"x");
    }
    fixture.repo.commit_all("one");
    let error = text(fixture.materialize(&fixture.tree()).unwrap_err());
    assert!(
        error.contains(&(MAX_TREE_ENTRIES + 1).to_string()),
        "{error}"
    );
    assert!(!fixture.cache.exists(), "not even the cache folder is made");
}

#[test]
fn a_blob_over_the_size_cap_is_refused_before_any_write() {
    let fixture = Fixture::new();
    fixture
        .repo
        .write("big.bin", &vec![b'x'; 8 * 1024 * 1024 + 1]);
    fixture.repo.commit_all("one");
    let error = text(fixture.materialize(&fixture.tree()).unwrap_err());
    assert!(error.contains("big.bin"), "{error}");
    assert!(!fixture.cache.exists());
}

#[test]
fn a_tree_that_is_not_in_the_repository_is_an_error() {
    let fixture = Fixture::ordinary();
    let error = text(fixture.materialize(&"7".repeat(40)).unwrap_err());
    assert!(
        error.contains(&"7".repeat(40)) || error.contains("tree"),
        "{error}"
    );
    assert_nothing_left(&fixture);
}

// ---- the re-hash ----

/// A writer that changes what it is given, so the re-hash is seen catching it.
enum Alter {
    /// Flips one bit of the first byte of the file whose path ends with the name.
    Byte(&'static str),
    /// Gives the named file the other executable bit.
    Mode(&'static str),
    /// Writes each file to a lower-cased name over what is there, as a volume that folds case does.
    FoldCase,
}

struct Altering(Alter);

impl PackWriter for Altering {
    fn write_file(&self, path: &Path, bytes: &[u8], mode: u32) -> io::Result<()> {
        match &self.0 {
            Alter::Byte(name) if path.ends_with(name) => {
                let mut changed = bytes.to_vec();
                changed[0] ^= 1;
                FsWriter.write_file(path, &changed, mode)
            }
            Alter::Mode(name) if path.ends_with(name) => {
                FsWriter.write_file(path, bytes, mode ^ 0o111)
            }
            Alter::FoldCase => {
                let name = path.file_name().unwrap().to_string_lossy().to_lowercase();
                let folded = path.with_file_name(name);
                fs::write(&folded, bytes)?;
                fs::set_permissions(&folded, fs::Permissions::from_mode(mode))
            }
            _ => FsWriter.write_file(path, bytes, mode),
        }
    }
}

fn mismatch_of(error: CacheError) -> (String, String) {
    match error {
        CacheError::Mismatch { expected, actual } => (expected, actual),
        other => panic!("expected a mismatch, got: {other}"),
    }
}

#[test]
fn a_writer_that_changes_one_byte_is_a_mismatch_naming_both_ids_and_leaves_no_entry() {
    let fixture = Fixture::ordinary();
    let tree = fixture.tree();
    let materializer =
        Materializer::new().with_writer(Box::new(Altering(Alter::Byte("hooks/lib/deep/util.ts"))));
    let error = fixture.with(&materializer, &tree).unwrap_err();
    let message = error.to_string();
    let (expected, actual) = mismatch_of(error);
    assert_eq!(expected, tree);
    assert_ne!(actual, tree);
    assert!(
        message.contains(&expected) && message.contains(&actual),
        "{message}"
    );
    assert_nothing_left(&fixture);
}

#[test]
fn a_writer_that_changes_a_mode_is_a_mismatch_and_leaves_no_entry() {
    let fixture = Fixture::ordinary();
    let materializer =
        Materializer::new().with_writer(Box::new(Altering(Alter::Mode("hooks/run.sh"))));
    mismatch_of(fixture.with(&materializer, &fixture.tree()).unwrap_err());
    assert_nothing_left(&fixture);
}

#[test]
fn a_volume_that_merges_names_that_differ_in_case_is_a_mismatch_on_a_fake() {
    let fixture = Fixture::new();
    let (upper, lower) = (fixture.repo.blob(b"upper\n"), fixture.repo.blob(b"lower\n"));
    let tree = mktree(
        &fixture.repo,
        &[
            ("100644", "blob", &upper, b"Foo.txt"),
            ("100644", "blob", &lower, b"foo.txt"),
        ],
    );
    let materializer = Materializer::new().with_writer(Box::new(Altering(Alter::FoldCase)));
    mismatch_of(fixture.with(&materializer, &tree).unwrap_err());
    assert_nothing_left(&fixture);
}

/// Whether the volume `dir` is on treats `a` and `A` as one name.
fn folds_case(dir: &Path) -> bool {
    let probe = dir.join("case-probe");
    fs::write(&probe, "x").unwrap();
    let folds = dir.join("CASE-PROBE").exists();
    fs::remove_file(&probe).unwrap();
    folds
}

#[test]
fn on_a_volume_that_folds_case_the_real_writer_does_not_merge_names() {
    let fixture = Fixture::new();
    let (upper, lower) = (fixture.repo.blob(b"upper\n"), fixture.repo.blob(b"lower\n"));
    let tree = mktree(
        &fixture.repo,
        &[
            ("100644", "blob", &upper, b"Foo.txt"),
            ("100644", "blob", &lower, b"foo.txt"),
        ],
    );
    fs::create_dir_all(&fixture.cache).unwrap();
    if !folds_case(&fixture.cache) {
        eprintln!(
            "this volume is case-sensitive: the pair is written as two files; checking that instead"
        );
        let entry = fixture.materialize(&tree).unwrap();
        assert_eq!(files_under(&entry.join("pack")).len(), 2);
        return;
    }
    let error = fixture.materialize(&tree).unwrap_err();
    assert!(
        matches!(error, CacheError::Io { .. } | CacheError::Mismatch { .. }),
        "{error}"
    );
    assert!(
        error.to_string().contains("oo.txt") || matches!(error, CacheError::Mismatch { .. }),
        "{error}"
    );
    assert_nothing_left(&fixture);
}

// ---- hit and miss ----

#[test]
fn a_hit_returns_without_running_git() {
    let fixture = Fixture::ordinary();
    let tree = fixture.tree();
    let entry = fixture.materialize(&tree).unwrap();
    let ran = fixture.repo.root().join("git-ran");
    let bin = fake_git(
        fixture.repo.root(),
        &format!("echo ran > {}\nexit 1", ran.display()),
    );
    let materializer = Materializer::new().with_git(GitObjects::new().with_path(&bin));
    assert_eq!(fixture.with(&materializer, &tree).unwrap(), entry);
    assert!(!ran.exists(), "git was started on a hit");
}

#[test]
fn a_miss_runs_git_and_the_fake_proves_the_test_would_notice() {
    let fixture = Fixture::ordinary();
    let ran = fixture.repo.root().join("git-ran");
    let bin = fake_git(
        fixture.repo.root(),
        &format!("echo ran > {}\nexit 1", ran.display()),
    );
    let materializer = Materializer::new().with_git(GitObjects::new().with_path(&bin));
    assert!(fixture.with(&materializer, &fixture.tree()).is_err());
    assert!(ran.exists());
    assert_nothing_left(&fixture);
}

#[test]
fn a_miss_then_a_hit_gives_the_same_entry_and_writes_nothing_the_second_time() {
    let fixture = Fixture::ordinary();
    let tree = fixture.tree();
    let first = fixture.materialize(&tree).unwrap();
    let marker_time = fs::metadata(first.join(MARKER))
        .unwrap()
        .modified()
        .unwrap();
    let before = files_under(&first);
    let second = fixture.materialize(&tree).unwrap();
    assert_eq!(first, second);
    assert_eq!(
        fs::metadata(second.join(MARKER))
            .unwrap()
            .modified()
            .unwrap(),
        marker_time
    );
    assert_eq!(files_under(&second), before);
    assert_eq!(fixture.packs(), [entry_name(&tree, "0.1.0")]);
}

#[test]
fn two_trees_make_two_entries_and_share_nothing() {
    let fixture = Fixture::ordinary();
    let one = fixture.tree();
    fixture.repo.write("more.txt", b"more\n");
    fixture.repo.commit_all("two");
    let two = fixture.tree();
    fixture.materialize(&one).unwrap();
    fixture.materialize(&two).unwrap();
    let mut want = vec![entry_name(&one, "0.1.0"), entry_name(&two, "0.1.0")];
    want.sort();
    assert_eq!(fixture.packs(), want);
}

fn age(path: &Path, by: Duration) {
    let file = fs::File::open(path).unwrap();
    file.set_modified(SystemTime::now() - by).unwrap();
}

/// A staging folder as a loader that died would leave it: half a pack, in a folder named for a pid that is gone.
fn dead_staging(fixture: &Fixture, name: &str, locked: bool) -> PathBuf {
    let dir = fixture.cache.join("packs").join(name);
    fs::create_dir_all(dir.join("pack/sub")).unwrap();
    fs::write(dir.join("pack/sub/half.txt"), "half").unwrap();
    if locked {
        fs::set_permissions(dir.join("pack/sub"), fs::Permissions::from_mode(0o555)).unwrap();
        fs::set_permissions(dir.join("pack"), fs::Permissions::from_mode(0o555)).unwrap();
    }
    dir
}

#[test]
fn a_staging_folder_older_than_an_hour_is_swept_on_a_miss_and_a_younger_one_is_not() {
    let fixture = Fixture::ordinary();
    let old = dead_staging(&fixture, ".staging-dead-1", false);
    let young = dead_staging(&fixture, ".staging-live-2", false);
    age(&old, Duration::from_secs(2 * 3600));
    age(&young, Duration::from_secs(10 * 60));
    fixture.materialize(&fixture.tree()).unwrap();
    assert!(!old.exists(), "the old one is swept");
    assert!(young.exists(), "the young one may be a live loader's");
}

#[test]
fn a_read_only_old_staging_folder_is_swept_too() {
    let fixture = Fixture::ordinary();
    let old = dead_staging(&fixture, ".staging-dead-3", true);
    age(&old, Duration::from_secs(3 * 3600));
    fixture.materialize(&fixture.tree()).unwrap();
    assert!(!old.exists());
}

#[test]
fn only_staging_folders_are_swept_and_a_hit_sweeps_nothing() {
    let fixture = Fixture::ordinary();
    let tree = fixture.tree();
    fixture.materialize(&tree).unwrap();
    let old = dead_staging(&fixture, ".staging-dead-4", false);
    let other = fixture.cache.join("packs/not-staging");
    fs::create_dir_all(&other).unwrap();
    age(&old, Duration::from_secs(2 * 3600));
    age(&other, Duration::from_secs(2 * 3600));
    fixture.materialize(&tree).unwrap();
    assert!(old.exists(), "a hit does no housekeeping");
    fs::write(fixture.repo.work().join("new.txt"), "x").unwrap();
    fixture.repo.commit_all("two");
    fixture.materialize(&fixture.tree()).unwrap();
    assert!(!old.exists());
    assert!(
        other.exists(),
        "a folder that is not a staging folder is left alone"
    );
}

#[test]
fn an_entry_without_a_marker_is_an_error_and_is_not_replaced() {
    let fixture = Fixture::ordinary();
    let tree = fixture.tree();
    let entry = fixture.entry(&tree);
    fs::create_dir_all(entry.join("pack")).unwrap();
    fs::write(entry.join("pack/stray.txt"), "x").unwrap();
    let error = text(fixture.materialize(&tree).unwrap_err());
    assert!(error.contains(&entry.display().to_string()), "{error}");
    assert!(entry.join("pack/stray.txt").exists());
    assert_eq!(fixture.packs(), [entry_name(&tree, "0.1.0")]);
}

// ---- the race ----

#[test]
fn eight_threads_on_one_tree_all_get_a_complete_entry_and_no_staging_is_left() {
    let fixture = Fixture::ordinary();
    let tree = fixture.tree();
    let barrier = std::sync::Barrier::new(8);
    let entries: Vec<PathBuf> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..8)
            .map(|_| {
                scope.spawn(|| {
                    barrier.wait();
                    fixture.materialize(&tree).unwrap()
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect()
    });
    for entry in &entries {
        assert_eq!(entry, &fixture.entry(&tree));
        assert!(entry.join(MARKER).is_file());
        assert_eq!(tree_of_folder(&entry.join("pack")).unwrap(), tree);
    }
    assert_eq!(fixture.packs(), [entry_name(&tree, "0.1.0")]);
}

/// The entry point of the child processes the next test starts. Run on its own it does nothing.
#[test]
fn race_child_materializes_when_started_as_a_child() {
    let Some(cache) = std::env::var_os(CHILD_CACHE) else {
        return;
    };
    let var = |name: &str| std::env::var_os(name).unwrap_or_else(|| panic!("{name} is not set"));
    let go = PathBuf::from(var(CHILD_GO));
    let until = std::time::Instant::now() + Duration::from_secs(30);
    while !go.exists() {
        assert!(
            std::time::Instant::now() < until,
            "the parent never said go"
        );
        std::thread::sleep(Duration::from_millis(2));
    }
    let tree = var(CHILD_TREE).to_string_lossy().to_string();
    let entry = materialize(
        Path::new(&cache),
        Path::new(&var(CHILD_SDK)),
        Path::new(&var(CHILD_REPO)),
        &tree,
    )
    .unwrap();
    assert!(entry.join(MARKER).is_file());
    fs::write(var(CHILD_OUT), entry.to_string_lossy().as_bytes()).unwrap();
}

#[test]
fn two_processes_on_one_tree_both_get_a_complete_entry_and_no_staging_is_left() {
    let fixture = Fixture::ordinary();
    let tree = fixture.tree();
    let root = fixture.repo.root();
    let go = root.join("go");
    let exe = std::env::current_exe().unwrap();
    let mut children = Vec::new();
    for n in 0..2 {
        let out = root.join(&format!("child-{n}.out"));
        let mut command = Command::new(&exe);
        command
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .env("HOME", root.path())
            .env("COLLIDE_DAEMON_ROOT", root.path())
            .env(CHILD_CACHE, &fixture.cache)
            .env(CHILD_SDK, &fixture.sdk)
            .env(CHILD_REPO, fixture.git_dir())
            .env(CHILD_TREE, &tree)
            .env(CHILD_GO, &go)
            .env(CHILD_OUT, &out)
            .args([
                "--exact",
                "race_child_materializes_when_started_as_a_child",
                "--test-threads=1",
            ])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped());
        children.push((command.spawn().expect("the test binary should start"), out));
    }
    fs::write(&go, "go").unwrap();
    for (child, out) in children {
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "child failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            fs::read_to_string(out).unwrap(),
            fixture.entry(&tree).to_string_lossy()
        );
    }
    assert_eq!(
        tree_of_folder(&fixture.entry(&tree).join("pack")).unwrap(),
        tree
    );
    assert_eq!(fixture.packs(), [entry_name(&tree, "0.1.0")]);
}

// ---- running a hook from the entry ----

#[test]
fn a_hook_that_imports_the_sdk_runs_from_the_entry() {
    let path = std::env::var_os("PATH").unwrap_or_default();
    if let Some(message) = node_missing_message(&path) {
        panic!("{message}");
    }
    let bundle = common::bundle("pack-cache");
    let bundle_sdk = bundle_sdk(&bundle).unwrap();
    let fixture = Fixture::new();
    let sdk = ensure_sdk(&fixture.cache, &bundle_sdk).unwrap();
    fixture.repo.write(
        "hooks/x.ts",
        b"import { name, defineHook } from '@collide/pack-sdk'\nconsole.log(name, typeof defineHook)\n",
    );
    fixture.repo.commit_all("one");
    let entry = materialize(&fixture.cache, &sdk, &fixture.git_dir(), &fixture.tree()).unwrap();
    let output = Command::new("node")
        .env_clear()
        .env("PATH", &path)
        .env("HOME", fixture.repo.root().path())
        .env("COLLIDE_DAEMON_ROOT", fixture.repo.root().path())
        .arg(entry.join("pack/hooks/x.ts"))
        .current_dir(fixture.repo.root().path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "@collide/pack-sdk function"
    );
    assert!(find_on_path("node", &path).is_some());
}

// ---- ensure_sdk ----

#[test]
fn the_first_ensure_sdk_copies_the_sdk_read_only_under_its_version() {
    let fixture = Fixture::new();
    let bundled = fixture.repo.root().join("bundled-sdk");
    make_sdk(&bundled, "1.2.3");
    let copy = ensure_sdk(&fixture.cache, &bundled).unwrap();
    assert_eq!(copy, fixture.cache.join("sdk/1.2.3"));
    assert_eq!(
        files_under(&copy)
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        [MARKER, "dist/index.js", "package.json"]
    );
    assert_eq!(
        fs::read(copy.join("dist/index.js")).unwrap(),
        fs::read(bundled.join("dist/index.js")).unwrap()
    );
    assert_eq!(mode_of(&copy), 0o555);
    assert_eq!(mode_of(&copy.join("dist")), 0o555);
    assert_eq!(mode_of(&copy.join("dist/index.js")), 0o444);
    assert_eq!(mode_of(&copy.join(MARKER)), 0o444);
    assert!(fs::write(copy.join("dist/index.js"), "x").is_err());
    assert_eq!(names_in(&fixture.cache.join("sdk")), ["1.2.3"]);
}

#[test]
fn a_second_ensure_sdk_writes_nothing() {
    let fixture = Fixture::new();
    let bundled = fixture.repo.root().join("bundled-sdk");
    make_sdk(&bundled, "1.2.3");
    let copy = ensure_sdk(&fixture.cache, &bundled).unwrap();
    let before = files_under(&copy);
    let marker_time = fs::metadata(copy.join(MARKER)).unwrap().modified().unwrap();
    fs::write(
        bundled.join("dist/index.js"),
        "export const name = 'changed'\n",
    )
    .unwrap();
    assert_eq!(ensure_sdk(&fixture.cache, &bundled).unwrap(), copy);
    assert_eq!(files_under(&copy), before);
    assert_eq!(
        fs::metadata(copy.join(MARKER)).unwrap().modified().unwrap(),
        marker_time
    );
    assert_eq!(names_in(&fixture.cache.join("sdk")), ["1.2.3"]);
}

#[test]
fn a_new_version_gets_its_own_folder_and_the_old_one_stays() {
    let fixture = Fixture::new();
    let bundled = fixture.repo.root().join("bundled-sdk");
    make_sdk(&bundled, "1.2.3");
    let old = ensure_sdk(&fixture.cache, &bundled).unwrap();
    make_sdk(&bundled, "1.3.0");
    let new = ensure_sdk(&fixture.cache, &bundled).unwrap();
    assert_ne!(old, new);
    assert!(old.join(MARKER).is_file() && new.join(MARKER).is_file());
    assert_eq!(names_in(&fixture.cache.join("sdk")), ["1.2.3", "1.3.0"]);
}

#[test]
fn a_new_sdk_version_makes_a_new_entry_for_the_same_tree_and_links_the_new_sdk() {
    let fixture = Fixture::ordinary();
    let tree = fixture.tree();
    let bundled = fixture.repo.root().join("bundled-sdk");
    make_sdk(&bundled, "1.2.3");
    let old_sdk = ensure_sdk(&fixture.cache, &bundled).unwrap();
    let old = materialize(&fixture.cache, &old_sdk, &fixture.git_dir(), &tree).unwrap();
    make_sdk(&bundled, "1.3.0");
    let new_sdk = ensure_sdk(&fixture.cache, &bundled).unwrap();
    let new = materialize(&fixture.cache, &new_sdk, &fixture.git_dir(), &tree).unwrap();
    assert_ne!(
        old, new,
        "an entry made with one SDK is not reused with another"
    );
    let link =
        |entry: &Path| fs::canonicalize(entry.join("node_modules/@collide/pack-sdk")).unwrap();
    assert_eq!(link(&old), fs::canonicalize(&old_sdk).unwrap());
    assert_eq!(link(&new), fs::canonicalize(&new_sdk).unwrap());
    assert_eq!(
        fixture.packs(),
        [format!("{tree}-1.2.3"), format!("{tree}-1.3.0")]
    );
}

#[test]
fn threads_that_copy_the_sdk_at_once_all_get_the_one_folder() {
    let fixture = Fixture::new();
    let bundled = fixture.repo.root().join("bundled-sdk");
    make_sdk(&bundled, "2.0.0");
    let barrier = std::sync::Barrier::new(6);
    let copies: Vec<PathBuf> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..6)
            .map(|_| {
                scope.spawn(|| {
                    barrier.wait();
                    ensure_sdk(&fixture.cache, &bundled).unwrap()
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect()
    });
    assert!(
        copies
            .iter()
            .all(|copy| copy == &fixture.cache.join("sdk/2.0.0"))
    );
    assert_eq!(names_in(&fixture.cache.join("sdk")), ["2.0.0"]);
}

#[test]
fn an_sdk_whose_package_json_has_no_usable_version_is_an_error_naming_the_folder() {
    let fixture = Fixture::new();
    let bundled = fixture.repo.root().join("bundled-sdk");
    for body in [
        "{}",
        "not json",
        "{\"version\":\"\"}",
        "{\"version\":\"../x\"}",
        "{\"version\":\".hid\"}",
    ] {
        fs::create_dir_all(&bundled).unwrap();
        fs::write(bundled.join("package.json"), body).unwrap();
        let error = text(ensure_sdk(&fixture.cache, &bundled).unwrap_err());
        assert!(
            error.contains(&bundled.display().to_string()),
            "{body}: {error}"
        );
    }
    assert!(!fixture.cache.exists());
}

#[test]
fn an_sdk_folder_with_no_package_json_is_an_error_naming_the_folder() {
    let fixture = Fixture::new();
    let missing = fixture.repo.root().join("nowhere");
    let error = text(ensure_sdk(&fixture.cache, &missing).unwrap_err());
    assert!(error.contains(&missing.display().to_string()), "{error}");
}

#[test]
fn a_symlink_in_the_bundled_sdk_is_refused_and_nothing_is_left() {
    let fixture = Fixture::new();
    let bundled = fixture.repo.root().join("bundled-sdk");
    make_sdk(&bundled, "1.2.3");
    std::os::unix::fs::symlink("/etc/passwd", bundled.join("dist/link")).unwrap();
    let error = text(ensure_sdk(&fixture.cache, &bundled).unwrap_err());
    assert!(error.contains("dist/link"), "{error}");
    assert_eq!(names_in(&fixture.cache.join("sdk")), Vec::<String>::new());
}

#[test]
fn the_staging_folder_of_a_dead_sdk_copy_is_swept_after_an_hour() {
    let fixture = Fixture::new();
    let bundled = fixture.repo.root().join("bundled-sdk");
    make_sdk(&bundled, "1.2.3");
    let old = fixture.cache.join("sdk/.staging-1.2.3-dead");
    fs::create_dir_all(old.join("dist")).unwrap();
    age(&old, Duration::from_secs(2 * 3600));
    ensure_sdk(&fixture.cache, &bundled).unwrap();
    assert_eq!(names_in(&fixture.cache.join("sdk")), ["1.2.3"]);
}
