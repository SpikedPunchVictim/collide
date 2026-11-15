//! Tests for finding the main checkout from git (ts-0020).

use collide_host::checkout::{CheckoutError, main_checkout, resolve_checkout};
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

fn repo_with_a_worktree(name: &str) -> (PathBuf, PathBuf, Cleanup) {
    let (base, cleanup) = fresh(name);
    let main = base.join("main");
    fs::create_dir_all(&main).unwrap();
    git(&main, &["init", "-q"]);
    git(&main, &["commit", "-q", "--allow-empty", "-m", "start"]);
    let linked = base.join("linked");
    git(&main, &["worktree", "add", "-q", linked.to_str().unwrap()]);
    (main, linked, cleanup)
}

#[test]
fn a_worktree_of_the_repository_is_a_checkout_and_so_is_the_main_one() {
    let (main, linked, _cleanup) = repo_with_a_worktree("resolve");
    assert_eq!(
        resolve_checkout(&main, &main, linked.to_str().unwrap()).unwrap(),
        linked
    );
    assert_eq!(
        resolve_checkout(&main, &linked, main.to_str().unwrap()).unwrap(),
        main
    );
    // A relative path is taken from the working directory.
    assert_eq!(resolve_checkout(&main, &main, "../linked").unwrap(), linked);
}

#[test]
fn a_symbolic_link_to_a_worktree_is_that_checkout() {
    let (main, linked, _cleanup) = repo_with_a_worktree("resolve-link");
    let alias = main.parent().unwrap().join("alias");
    std::os::unix::fs::symlink(&linked, &alias).unwrap();
    assert_eq!(
        resolve_checkout(&main, &main, alias.to_str().unwrap()).unwrap(),
        linked
    );
}

#[test]
fn anything_else_is_not_a_checkout_and_the_error_lists_the_ones_that_are() {
    let (main, linked, _cleanup) = repo_with_a_worktree("resolve-refused");
    let inside = linked.join("sub");
    fs::create_dir_all(&inside).unwrap();
    let elsewhere = main.parent().unwrap().join("elsewhere");
    fs::create_dir_all(&elsewhere).unwrap();
    for given in [inside, elsewhere, main.join("nope")] {
        let error = resolve_checkout(&main, &main, given.to_str().unwrap()).unwrap_err();
        assert!(
            matches!(
                error,
                CheckoutError::NotACheckout { .. } | CheckoutError::UnreadableCheckout { .. }
            ),
            "{error}"
        );
        let text = error.to_string();
        assert!(text.contains(given.to_str().unwrap()), "{text}");
    }
    let error =
        resolve_checkout(&main, &main, main.parent().unwrap().to_str().unwrap()).unwrap_err();
    assert!(
        error.to_string().contains(linked.to_str().unwrap()),
        "{error}"
    );
}
