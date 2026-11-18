//! The main checkout's path, from git (ts-0020). A provider's `initialize.root` is the main checkout, also when the
//! CLI runs inside a linked worktree, so it is not the working directory and not `--show-toplevel`: it is the parent
//! of the common git directory, which every worktree of a repository shares. `--checkout <path>` (ts-0031) names
//! another checkout, and is accepted only when it is an entry of `git worktree list` for the same repository.

use crate::workspace::{Git, WorkspaceError};
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug)]
pub enum CheckoutError {
    /// `git` did not run.
    GitMissing(std::io::Error),
    /// `cwd` is not inside a git repository.
    NotARepository { cwd: PathBuf, detail: String },
    /// The common git directory is not named `.git` (a bare repository, or a git directory kept elsewhere), so no
    /// main checkout follows from it.
    NoMainCheckout { git_dir: PathBuf },
    /// `--checkout` names a path that cannot be read (it does not exist, or is not a folder).
    UnreadableCheckout { given: String, detail: String },
    /// `--checkout` names a path that is not an entry of `git worktree list`.
    NotACheckout { given: String, listed: Vec<PathBuf> },
}

impl fmt::Display for CheckoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GitMissing(error) => {
                write!(f, "could not run `git` to find the repository: {error}")
            }
            Self::NotARepository { cwd, detail } => write!(
                f,
                "{} is not inside a git repository ({detail}); run collide from a checkout",
                cwd.display()
            ),
            Self::NoMainCheckout { git_dir } => write!(
                f,
                "the git directory {} is not named .git, so there is no main checkout to read tasks from",
                git_dir.display()
            ),
            Self::UnreadableCheckout { given, detail } => {
                write!(
                    f,
                    "`{given}` is not a checkout of this repository: it cannot be read ({detail})"
                )
            }
            Self::NotACheckout { given, listed } => {
                let listed: Vec<String> = listed.iter().map(|p| p.display().to_string()).collect();
                write!(
                    f,
                    "`{given}` is not a checkout of this repository; `git worktree list` shows: {}",
                    listed.join(", ")
                )
            }
        }
    }
}

impl std::error::Error for CheckoutError {}

/// The main checkout of the repository that `cwd` is in.
pub fn main_checkout(cwd: &Path) -> Result<PathBuf, CheckoutError> {
    let output = Command::new("git")
        .args(["rev-parse", "--path-format=absolute", "--git-common-dir"])
        .current_dir(cwd)
        .output()
        .map_err(CheckoutError::GitMissing)?;
    if !output.status.success() {
        return Err(CheckoutError::NotARepository {
            cwd: cwd.to_path_buf(),
            detail: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        });
    }
    let git_dir = PathBuf::from(String::from_utf8_lossy(&output.stdout).trim());
    match git_dir.file_name() {
        Some(name) if name == ".git" => git_dir
            .parent()
            .map(Path::to_path_buf)
            .ok_or(CheckoutError::NoMainCheckout { git_dir }),
        _ => Err(CheckoutError::NoMainCheckout { git_dir }),
    }
}

/// The checkouts of the repository whose main checkout is `root`, as `git worktree list --porcelain` gives them,
/// with symbolic links resolved. A listed checkout that is gone from disk is left out.
fn checkouts(root: &Path) -> Result<Vec<PathBuf>, CheckoutError> {
    let output = Command::new("git")
        .args(["worktree", "list", "--porcelain"])
        .current_dir(root)
        .output()
        .map_err(CheckoutError::GitMissing)?;
    if !output.status.success() {
        return Err(CheckoutError::NotARepository {
            cwd: root.to_path_buf(),
            detail: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.strip_prefix("worktree "))
        .filter_map(|path| std::fs::canonicalize(path).ok())
        .collect())
}

/// The checkout that `given` names: a path (relative to `cwd`) that is an entry of `git worktree list` for the
/// repository whose main checkout is `root`, compared with symbolic links resolved. Anything else is refused.
pub fn resolve_checkout(root: &Path, cwd: &Path, given: &str) -> Result<PathBuf, CheckoutError> {
    let path = std::fs::canonicalize(cwd.join(given)).map_err(|error| {
        CheckoutError::UnreadableCheckout {
            given: given.to_string(),
            detail: error.to_string(),
        }
    })?;
    let listed = checkouts(root)?;
    if listed.contains(&path) {
        Ok(path)
    } else {
        Err(CheckoutError::NotACheckout {
            given: given.to_string(),
            listed,
        })
    }
}

/// What a checkout can be in the middle of, as the file or folder git keeps while it is, and how to say it. `revert`
/// is here with the four of 0.4: a revert stopped on a conflict blocks a write as a merge does (ts-0114-hr).
const IN_PROGRESS: [(&str, &str); 6] = [
    ("rebase-merge", "a rebase"),
    ("rebase-apply", "a rebase"),
    ("MERGE_HEAD", "a merge"),
    ("CHERRY_PICK_HEAD", "a cherry-pick"),
    ("REVERT_HEAD", "a revert"),
    ("BISECT_LOG", "a bisect"),
];

/// What `checkout` is in the middle of, in words (`a rebase`, `a merge`, `a cherry-pick`, `a revert`, `a bisect`), or
/// `None` when nothing is. A write to a checkout waits while this is `Some` (0.4, Root busy), and accept refuses.
/// `env` is added to the environment of git. A linked worktree has its own state files, which `git rev-parse
/// --git-path` finds, so this reads the checkout it is given and no other.
pub fn root_busy(
    env: &[(String, String)],
    checkout: &Path,
) -> Result<Option<&'static str>, WorkspaceError> {
    let mut args = vec!["rev-parse"];
    for (name, _) in IN_PROGRESS {
        args.push("--git-path");
        args.push(name);
    }
    let paths = Git::with_env(env.to_vec()).text(checkout, &args)?;
    Ok(paths
        .lines()
        .zip(IN_PROGRESS)
        .find(|(path, _)| checkout.join(path).exists())
        .map(|(_, (_, what))| what))
}
