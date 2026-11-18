//! What a board is made of in 3c (ts-0075-hj; 0.8, Board; 0.11, First run works with no pack and no tasks): a git
//! checkout, found from any folder in it, and the branch it is on now. The boards the user opened are the store's
//! ([`crate::scratch_store`]); this file reads git. The line a board has, with its trust, is [`crate::board_lines`].
//!
//! Only the `git_repo` check is made here. The checks for `claude` come with the no-setup open (Stage 6a), and
//! `git init` for a folder that is not a checkout is that stage's too.

use collide_core::owner::{CheckId, FirstRunCheck};
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

/// The commit `HEAD` names in the checkout at `root`, or `None` when it names none (a checkout with no commit) or git
/// cannot be run.
pub fn head_commit(root: &Path, env: &[(String, String)]) -> Option<String> {
    git(
        root,
        env,
        &["rev-parse", "--verify", "--quiet", "HEAD^{commit}"],
    )
    .ok()
    .flatten()
    .filter(|commit| !commit.is_empty())
}

/// One entry of `git worktree list`: a checkout of the repository, and the branch it is on (`None` when it is on no
/// branch).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Worktree {
    pub path: PathBuf,
    pub branch: Option<String>,
}

/// The checkouts of the repository at `root`, as `git worktree list --porcelain` gives them, with symbolic links
/// resolved. An entry whose folder is gone, and a bare one, is left out.
pub fn worktrees(root: &Path, env: &[(String, String)]) -> Result<Vec<Worktree>, String> {
    let Some(text) = git(root, env, &["worktree", "list", "--porcelain"])? else {
        return Err(format!(
            "`git worktree list` failed in `{}`; is it still a git checkout?",
            root.display()
        ));
    };
    let mut found = Vec::new();
    for block in text.split("\n\n") {
        let mut path = None;
        let mut branch = None;
        let mut bare = false;
        for line in block.lines() {
            if let Some(listed) = line.strip_prefix("worktree ") {
                path = Some(listed.to_string());
            } else if let Some(name) = line.strip_prefix("branch ") {
                branch = Some(name.strip_prefix("refs/heads/").unwrap_or(name).to_string());
            } else if line == "bare" {
                bare = true;
            }
        }
        if let (Some(path), false) = (path, bare)
            && let Ok(path) = std::fs::canonicalize(&path)
        {
            found.push(Worktree { path, branch });
        }
    }
    Ok(found)
}

/// Whether the board's `checkout_filter.exclude` names `worktree`: a glob (`*` for any run of characters, `?` for
/// one) that matches its branch name or its path. A checkout on no branch is matched by its path alone.
pub fn checkout_excluded(globs: &[String], worktree: &Worktree) -> bool {
    let path = worktree.path.to_string_lossy();
    globs.iter().any(|glob| {
        glob_matches(glob, &path)
            || worktree
                .branch
                .as_deref()
                .is_some_and(|branch| glob_matches(glob, branch))
    })
}

fn glob_matches(glob: &str, text: &str) -> bool {
    fn at(glob: &[char], text: &[char]) -> bool {
        match glob.split_first() {
            None => text.is_empty(),
            Some(('*', rest)) => (0..=text.len()).any(|skip| at(rest, &text[skip..])),
            Some(('?', rest)) => !text.is_empty() && at(rest, &text[1..]),
            Some((one, rest)) => text.first() == Some(one) && at(rest, &text[1..]),
        }
    }
    let glob: Vec<char> = glob.chars().collect();
    let text: Vec<char> = text.chars().collect();
    at(&glob, &text)
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
