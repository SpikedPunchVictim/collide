//! What a board is made of in 3c (ts-0075-hj; 0.8, Board; 0.11, First run works with no pack and no tasks): a git
//! checkout, found from any folder in it, and the branch it is on now. The boards the user opened are the store's
//! ([`crate::scratch_store`]); this file reads git. The line a board has, with its trust, is [`crate::board_lines`].
//!
//! The `git_repo` check is made here, and `git init` for a folder that is not a checkout (Make it one, ts-0192-es). The
//! checks for `claude` are [`crate::first_run`]'s.

use crate::bounded_run::tail;
use crate::checkout::{CheckoutError, main_checkout_with, root_busy};
use collide_core::owner::{CheckId, FirstRunCheck};
use std::path::{Path, PathBuf};
use std::process::Command;

/// The checkout a board of `checkout` is rooted at (ts-0191-nv): the main checkout of its repository, so a linked
/// worktree opens as a line of the board of the repository it belongs to, and the main checkout opens the same board.
/// A checkout whose main checkout git cannot name (a bare repository, or a git directory kept elsewhere) is its own
/// root. `Err` says that the main checkout cannot be read.
pub fn board_checkout(checkout: &Checkout, env: &[(String, String)]) -> Result<Checkout, String> {
    let main = match main_checkout_with(&checkout.root, env) {
        Ok(main) => main,
        Err(CheckoutError::NoMainCheckout { .. }) => return Ok(checkout.clone()),
        Err(other) => return Err(other.to_string()),
    };
    let main = std::fs::canonicalize(&main).map_err(|error| {
        format!(
            "the main checkout `{}` of `{}` cannot be read ({error}); check that the folder is still there",
            main.display(),
            checkout.root.display()
        )
    })?;
    if main == checkout.root {
        return Ok(checkout.clone());
    }
    checkout_of(&main, env)
}

/// The root commits of the repository checked out at `root`: the commits `HEAD` reaches that have no parent, in the
/// order git lists them. A repository with more than one (merged histories) has several. Empty when `HEAD` names no
/// commit yet. `Err` says why git could not be asked.
pub fn root_commits(root: &Path, env: &[(String, String)]) -> Result<Vec<String>, String> {
    if git(
        root,
        env,
        &["rev-parse", "--verify", "--quiet", "HEAD^{commit}"],
    )?
    .is_none()
    {
        return Ok(Vec::new());
    }
    match git(root, env, &["rev-list", "--max-parents=0", "HEAD"])? {
        Some(text) => Ok(text.lines().map(str::to_string).collect()),
        None => Err(format!(
            "`git rev-list` failed in `{}`; is it still a git checkout?",
            root.display()
        )),
    }
}

/// Whether the repository at `folder` is the one a board recorded (`board.locate`): the two share a root commit.
/// `Ok(true)` says the check ran and passed, and `Ok(false)` that it could not run because nothing was recorded.
/// `Err` is the reason the folder is refused.
pub fn same_repository(
    recorded: Option<&[String]>,
    found: &[String],
    folder: &Path,
) -> Result<bool, String> {
    let Some(recorded) = recorded else {
        return Ok(false);
    };
    if found.is_empty() {
        return Err(format!(
            "`{}` has no commit yet, so Collide cannot tell that it is the board's repository. \
             Choose the folder that holds the repository's history.",
            folder.display()
        ));
    }
    if found.iter().any(|commit| recorded.contains(commit)) {
        return Ok(true);
    }
    let short = |commits: &[String]| {
        commits
            .iter()
            .map(|commit| commit.chars().take(12).collect::<String>())
            .collect::<Vec<_>>()
            .join(", ")
    };
    Err(format!(
        "`{}` is a different repository: its first commit is {}, and the board's is {}. \
         Choose the folder the board's repository moved to.",
        folder.display(),
        short(found),
        short(recorded)
    ))
}

/// A git checkout: the top level of its working tree and the branch it is on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checkout {
    pub root: PathBuf,
    /// The branch name, or `HEAD` when the checkout is on no branch.
    pub branch: String,
    /// The branch git holds while `HEAD` is detached for a rebase or a bisect (ts-0146-se): the line stays that
    /// branch, with its trust and its own commit. `None` on a branch, and on a detached `HEAD` with no operation.
    pub held: Option<String>,
}

impl Checkout {
    /// The branch the checkout's line is: the held one while git has `HEAD` detached for an operation, else
    /// [`Checkout::branch`] (`HEAD` when the checkout is on no branch).
    pub fn line_branch(&self) -> &str {
        self.held.as_deref().unwrap_or(&self.branch)
    }
}

/// Runs `git` in `folder` and returns what it printed when it succeeded. `None` when it failed or did not run. `env` is
/// added to the environment, as for the workspace's git ([`crate::daemon::DaemonOptions::git_env`]). `Err` says that
/// the folder is gone, or that git is not installed: a spawn in a folder that is gone fails with `NotFound` too, so the
/// folder is checked first.
fn git(folder: &Path, env: &[(String, String)], args: &[&str]) -> Result<Option<String>, String> {
    if !folder.is_dir() {
        return Err(format!(
            "`{}` is not a folder (it is gone or was moved), so `git` cannot run in it; \
             check that the folder is still there",
            folder.display()
        ));
    }
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

/// Whether `folder` is in a git checkout. `Err` says that the folder is gone, or that git is not installed.
pub fn in_checkout(folder: &Path, env: &[(String, String)]) -> Result<bool, String> {
    Ok(git(folder, env, &["rev-parse", "--show-toplevel"])?.is_some())
}

/// Makes `folder` a git repository (`git init`), with `env` as for the workspace's git. Nothing else in the folder is
/// touched. `Err` is Collide's message for why it could not: the folder is gone, git is not installed, or git said no.
pub fn init_repository(folder: &Path, env: &[(String, String)]) -> Result<(), String> {
    if !folder.is_dir() {
        return Err(format!(
            "`{}` is not a folder (it is gone or was moved), so there is nothing to make a repository of; \
             check that the folder is still there",
            folder.display()
        ));
    }
    let output = Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(folder)
        .env("LC_ALL", "C")
        .env("GIT_TERMINAL_PROMPT", "0")
        .envs(env.iter().map(|(name, value)| (name, value)))
        .output()
        .map_err(|error| format!("`git` did not run ({error}); install git and try again"))?;
    if output.status.success() {
        return Ok(());
    }
    Err(format!(
        "`git init` failed in `{}`: {}",
        folder.display(),
        tail(&output.stderr)
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
    let held = if branch == "HEAD" {
        held_branch(&root, env)?
    } else {
        None
    };
    Ok(Checkout { root, branch, held })
}

/// The branch git holds while `HEAD` is detached for a rebase (its `head-name`) or a bisect (its `BISECT_START`), found
/// in the files of this checkout's own git directory. That a rebase or a bisect is in progress is
/// [`root_busy`]'s answer. `None` when neither is, when the file does not name a branch, and when the branch is not
/// in the repository: the line is then `HEAD`, which is never trusted.
fn held_branch(root: &Path, env: &[(String, String)]) -> Result<Option<String>, String> {
    let files: &[&str] = match root_busy(env, root).map_err(|error| error.to_string())? {
        Some("a rebase") => &["rebase-merge/head-name", "rebase-apply/head-name"],
        Some("a bisect") => &["BISECT_START"],
        _ => return Ok(None),
    };
    for file in files {
        let Some(path) = git(root, env, &["rev-parse", "--git-path", file])? else {
            continue;
        };
        let Ok(text) = std::fs::read_to_string(root.join(path)) else {
            continue;
        };
        let text = text.trim();
        // A rebase writes the full ref (`detached HEAD` for a rebase of no branch); a bisect writes the bare name.
        let name = if *file == "BISECT_START" {
            Some(text)
        } else {
            text.strip_prefix("refs/heads/")
        };
        if let Some(name) = name.filter(|name| !name.is_empty())
            && branch_commit(root, env, name).is_some()
        {
            return Ok(Some(name.to_string()));
        }
    }
    Ok(None)
}

/// The commit the branch `name` names, or `None` when there is no such branch.
fn branch_commit(root: &Path, env: &[(String, String)], name: &str) -> Option<String> {
    let reference = format!("refs/heads/{name}^{{commit}}");
    git(root, env, &["rev-parse", "--verify", "--quiet", &reference])
        .ok()
        .flatten()
        .filter(|commit| !commit.is_empty())
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

/// The commit the line of `checkout` names: its held branch's own commit while git has `HEAD` detached for an
/// operation, else the commit `HEAD` names. `None` when it names none or git cannot be run.
pub fn line_commit(checkout: &Checkout, env: &[(String, String)]) -> Option<String> {
    match &checkout.held {
        Some(name) => branch_commit(&checkout.root, env, name),
        None => head_commit(&checkout.root, env),
    }
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
        copy: None,
        could_not_check: false,
    }
}
