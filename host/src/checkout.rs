//! The main checkout's path, from git (ts-0020). A provider's `initialize.root` is the main checkout, also when the
//! CLI runs inside a linked worktree, so it is not the working directory and not `--show-toplevel`: it is the parent
//! of the common git directory, which every worktree of a repository shares.

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
