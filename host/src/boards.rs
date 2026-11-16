//! What a board is made of in 3c (ts-0075-hj; 0.8, Board; 0.11, First run works with no pack and no tasks): a git
//! checkout, found from any folder in it, and the one line it has, the branch the checkout is on now. The boards the
//! user opened are the store's ([`crate::scratch_store`]); this file reads git.
//!
//! Only the `git_repo` check is made here. The checks for `claude` come with the no-setup open (Stage 6a), and
//! `git init` for a folder that is not a checkout is that stage's too.

use collide_core::owner::{BoardSummary, CheckId, FirstRunCheck, LineSummary, TrustState};
use std::path::{Path, PathBuf};
use std::process::Command;

/// A git checkout: the top level of its working tree and the branch it is on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checkout {
    pub root: PathBuf,
    /// The branch name, or `HEAD` when the checkout is on no branch.
    pub branch: String,
}

/// Runs `git` in `folder` and returns what it printed when it succeeded. `None` when it failed or did not run. `env` is
/// added to the environment, as for the workspace's git ([`crate::daemon::DaemonOptions::git_env`]).
fn git(folder: &Path, env: &[(String, String)], args: &[&str]) -> Result<Option<String>, String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(folder)
        .env("LC_ALL", "C")
        .env("GIT_TERMINAL_PROMPT", "0")
        .envs(env.iter().map(|(name, value)| (name, value)))
        .output()
        .map_err(|error| format!("`git` did not run ({error}); install git and try again"))?;
    if !output.status.success() {
        return Ok(None);
    }
    Ok(Some(
        String::from_utf8_lossy(&output.stdout).trim().to_string(),
    ))
}

/// The checkout that `folder` is in, or Collide's message for why it is not in one. The message names the fix.
pub fn checkout_of(folder: &Path, env: &[(String, String)]) -> Result<Checkout, String> {
    if !folder.is_dir() {
        return Err(format!(
            "`{}` is not a folder, so there is nothing to open. Choose a folder inside a git checkout.",
            folder.display()
        ));
    }
    let not_a_checkout = || {
        format!(
            "`{}` is not inside a git checkout, so Collide cannot open it as a board. \
             Run `git init` in it, or open a folder that is inside a checkout.",
            folder.display()
        )
    };
    let Some(top) = git(folder, env, &["rev-parse", "--show-toplevel"])? else {
        return Err(not_a_checkout());
    };
    let root = std::fs::canonicalize(&top).map_err(|error| {
        format!(
            "the checkout at `{top}` cannot be read ({error}); check that the folder is still there"
        )
    })?;
    let branch = git(&root, env, &["symbolic-ref", "--quiet", "--short", "HEAD"])?
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "HEAD".to_string());
    Ok(Checkout { root, branch })
}

/// The git directory of the checkout at `root`, as an absolute path (for a linked worktree it is not `root/.git`).
pub fn git_dir_of(root: &Path, env: &[(String, String)]) -> Result<PathBuf, String> {
    match git(root, env, &["rev-parse", "--absolute-git-dir"])? {
        Some(dir) if !dir.is_empty() => Ok(PathBuf::from(dir)),
        _ => Err(format!(
            "`{}` has no git directory; is it still a git checkout?",
            root.display()
        )),
    }
}

/// The `git_repo` check, passed.
pub fn git_repo_ok(checkout: &Checkout) -> FirstRunCheck {
    git_repo_check(
        true,
        format!(
            "`{}` is a git checkout, on `{}`.",
            checkout.root.display(),
            checkout.branch
        ),
    )
}

/// The `git_repo` check, failed with `message`.
pub fn git_repo_failed(message: String) -> FirstRunCheck {
    git_repo_check(false, message)
}

fn git_repo_check(ok: bool, message: String) -> FirstRunCheck {
    FirstRunCheck {
        id: CheckId::GitRepo,
        ok,
        required: true,
        message,
    }
}

/// The board `id` at `root` with its one line. Nothing in a checkout is read for trust in 3c (no repo pack is loaded),
/// so the line is untrusted at an empty hash, which a later stage replaces with the hash of 0.9.
pub fn summary(id: &str, root: &str, checkout: Option<&Checkout>) -> BoardSummary {
    BoardSummary {
        id: id.to_string(),
        root: root.to_string(),
        lines: checkout
            .map(|checkout| LineSummary {
                ref_name: checkout.branch.clone(),
                root: checkout.root.display().to_string(),
                snapshot: None,
                trust: TrustState::Untrusted {
                    hash: String::new(),
                },
            })
            .into_iter()
            .collect(),
    }
}
