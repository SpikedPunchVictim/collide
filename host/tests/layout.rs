use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("host/ sits directly under the repo root")
        .to_path_buf()
}

fn subdirs(dir: &Path) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()))
        .map(|entry| entry.expect("directory entry").path())
        .filter(|path| path.is_dir())
        .collect();
    dirs.sort();
    dirs
}

/// The two built-in packs and the fixture packs, found by listing the two pack roots.
fn pack_dirs() -> Vec<PathBuf> {
    let root = repo_root();
    let mut dirs = subdirs(&root.join("packs"));
    dirs.extend(subdirs(&root.join("fixtures/packs")));
    dirs
}

#[test]
fn there_are_five_pack_directories() {
    assert_eq!(pack_dirs().len(), 5, "found: {:?}", pack_dirs());
}

#[test]
fn the_named_pack_directories_exist() {
    let root = repo_root();
    for rel in [
        "packs/collide",
        "packs/plan",
        "fixtures/packs/loop",
        "fixtures/packs/pr-ci",
    ] {
        assert!(root.join(rel).is_dir(), "missing {rel}");
    }
}

#[test]
fn every_pack_directory_has_a_pack_yaml() {
    let dirs = pack_dirs();
    assert!(!dirs.is_empty(), "no pack directories found");
    for dir in dirs {
        assert!(
            dir.join("pack.yaml").is_file(),
            "no pack.yaml in {}",
            dir.display()
        );
    }
}
