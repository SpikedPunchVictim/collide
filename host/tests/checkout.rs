//! Tests for finding the main checkout from git (ts-0020).

use collide_host::checkout::{CheckoutError, main_checkout};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

mod common;
use common::scratch;

fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args([
            "-c",
            "user.name=test",
            "-c",
            "user.email=test@example.invalid",
        ])
        .args(["-c", "commit.gpgsign=false"])
        .args(args)
        .current_dir(dir)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .status()
        .expect("git should run");
    assert!(status.success(), "git {args:?} failed");
}

struct Cleanup(PathBuf);

impl Drop for Cleanup {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn fresh(name: &str) -> (PathBuf, Cleanup) {
    fresh_in(scratch(&format!("checkout-{name}")))
}

fn fresh_in(dir: PathBuf) -> (PathBuf, Cleanup) {
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    // The temp folder may sit behind a symbolic link (macOS /var); git reports the real path.
    let dir = fs::canonicalize(dir).unwrap();
    (dir.clone(), Cleanup(dir))
}

#[test]
fn inside_a_repository_the_main_checkout_is_its_top() {
    let (base, _cleanup) = fresh("plain");
    git(&base, &["init", "-q"]);
    fs::create_dir_all(base.join("sub/deeper")).unwrap();
    assert_eq!(main_checkout(&base).unwrap(), base);
    assert_eq!(main_checkout(&base.join("sub/deeper")).unwrap(), base);
}

#[test]
fn inside_a_linked_worktree_the_main_checkout_is_still_the_main_one() {
    let (base, _cleanup) = fresh("linked");
    let main = base.join("main");
    fs::create_dir_all(&main).unwrap();
    git(&main, &["init", "-q"]);
    git(&main, &["commit", "-q", "--allow-empty", "-m", "start"]);
    let linked = base.join("linked");
    git(&main, &["worktree", "add", "-q", linked.to_str().unwrap()]);
    assert_eq!(main_checkout(&linked).unwrap(), main);
}

#[test]
fn outside_a_repository_the_error_names_the_directory() {
    // Outside `target/`, which sits inside this repository.
    let (base, _cleanup) = fresh_in(
        std::env::temp_dir().join(format!("collide-checkout-none-{}", std::process::id())),
    );
    let error = main_checkout(&base).unwrap_err();
    assert!(
        matches!(error, CheckoutError::NotARepository { .. }),
        "{error}"
    );
    assert!(error.to_string().contains(&base.display().to_string()));
}

#[test]
fn a_bare_repository_has_no_main_checkout() {
    let (base, _cleanup) = fresh("bare");
    git(&base, &["init", "-q", "--bare", "repo.git"]);
    let error = main_checkout(&base.join("repo.git")).unwrap_err();
    assert!(
        matches!(error, CheckoutError::NoMainCheckout { .. }),
        "{error}"
    );
}
