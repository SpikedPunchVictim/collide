//! Helpers the host's integration tests share: the repo root, scratch folders under `target/tmp`, and the built-in
//! packs bundle, built once per test binary by the repo's build script into a scratch folder (so no test depends on
//! a staged copy and none skips). The bundle is read-only, so it is unlocked and deleted when the last user is done,
//! also when a test panics: `rm -rf target/tmp` and `cargo clean` must work.

#![allow(dead_code)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("host/ sits directly under the repo root")
        .to_path_buf()
}

pub fn scratch(name: &str) -> PathBuf {
    Path::new(env!("CARGO_TARGET_TMPDIR")).join(name)
}

/// The build script rebuilds the SDK's `dist/` in place, so two builds must not overlap.
static BUILD_LOCK: Mutex<()> = Mutex::new(());

/// Runs `pnpm build:packs --out <out>` from the repo root. A leftover read-only `out` from an earlier run is the
/// build's own to replace.
pub fn build_packs(out: &Path) {
    let _guard = BUILD_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let status = Command::new("node")
        .args(["scripts/build-packs.ts", "--out"])
        .arg(out)
        .current_dir(repo_root())
        .status()
        .expect("node should run the build script");
    assert!(status.success(), "build-packs exited with {status}");
}

/// Takes the write bits back so that a read-only tree can be deleted (`rm -rf target/tmp`, `cargo clean`).
pub fn make_writable(path: &Path) {
    let Ok(meta) = fs::symlink_metadata(path) else {
        return;
    };
    let mut permissions = meta.permissions();
    permissions.set_mode(permissions.mode() | 0o200);
    fs::set_permissions(path, permissions).unwrap();
    if meta.is_dir() {
        for entry in fs::read_dir(path).unwrap() {
            make_writable(&entry.unwrap().path());
        }
    }
}

pub fn remove_read_only(path: &Path) {
    make_writable(path);
    let _ = fs::remove_dir_all(path);
}

/// A built bundle that is unlocked and deleted when the guard drops, also when the test panics.
pub struct Scratch(pub PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        remove_read_only(&self.0);
    }
}

impl std::ops::Deref for Scratch {
    type Target = Path;
    fn deref(&self) -> &Path {
        &self.0
    }
}

/// The shared bundle: built by the first test that asks, and removed when the last one is done.
static SHARED_USERS: Mutex<usize> = Mutex::new(0);

pub struct Bundle(pub PathBuf);

/// The bundle in the scratch folder `name`. Each test binary uses one name.
pub fn bundle(name: &str) -> Bundle {
    let out = scratch(name);
    let mut users = SHARED_USERS.lock().unwrap_or_else(|e| e.into_inner());
    if *users == 0 {
        build_packs(&out);
    }
    *users += 1;
    Bundle(out)
}

impl Drop for Bundle {
    fn drop(&mut self) {
        let mut users = SHARED_USERS.lock().unwrap_or_else(|e| e.into_inner());
        *users -= 1;
        if *users == 0 {
            remove_read_only(&self.0);
        }
    }
}

impl std::ops::Deref for Bundle {
    type Target = Path;
    fn deref(&self) -> &Path {
        &self.0
    }
}
