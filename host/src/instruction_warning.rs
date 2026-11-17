//! The warning for a phase that starts after its run changed an instruction file (ts-0104-gx; 0.3, The repository's
//! own instructions). A phase with `repo_instructions: worktree` reads the worktree's `CLAUDE.md` files and skills, so
//! text the run itself wrote reaches the next phase as instructions; the person is told which files.
//!
//! "Changed" is a difference from the commit the workspace was made from: committed, staged or left in the tree, new
//! or deleted. Which paths count is [`is_instruction_path`]. Nothing here writes to git or to the workspace.

use crate::adapters::claude::instructions::is_instruction_path;
use crate::workspace::Git;
use collide_core::owner::{RunWarning, RunWarningKind};
use collide_core::pack::Role;
use collide_core::profile::RepoInstructions;
use std::path::Path;

/// The instruction files that differ between `base` (a commit) and `workspace`, each once, in path order.
/// `git_env` is added to the environment of git.
pub fn changed_instruction_files(
    git_env: &[(String, String)],
    workspace: &Path,
    base: &str,
) -> Result<Vec<String>, String> {
    let git = Git::with_env(git_env.to_vec());
    let read = |args: &[&str]| {
        git.run(workspace, args)
            .map_err(|error| format!("reading what changed since {base}: {error}"))
    };
    let tracked = read(&["diff", "--name-only", "--no-renames", "-z", base, "--"])?;
    let untracked = read(&["ls-files", "--others", "--exclude-standard", "-z"])?;
    let mut files: Vec<String> = [tracked, untracked]
        .iter()
        .flat_map(|names| names.split(|byte| *byte == 0))
        .filter(|name| !name.is_empty())
        .map(|name| String::from_utf8_lossy(name).into_owned())
        .filter(|name| is_instruction_path(name))
        .collect();
    files.sort();
    files.dedup();
    Ok(files)
}

/// The warning for `phase` about to start in `workspace`, or `None` when no instruction file changed since `base`.
/// A role with `repo_instructions: none` never reads them, so it has none.
pub fn instruction_warning(
    role: &Role,
    git_env: &[(String, String)],
    workspace: &Path,
    base: &str,
    phase: &str,
) -> Result<Option<RunWarning>, String> {
    if role.repo_instructions == Some(RepoInstructions::None) {
        return Ok(None);
    }
    let files = changed_instruction_files(git_env, workspace, base)?;
    Ok((!files.is_empty()).then(|| RunWarning {
        kind: RunWarningKind::InstructionFilesChanged,
        phase: phase.to_string(),
        files,
    }))
}

/// `files` without the ones an earlier warning of the run named: a phase that is resumed is not warned of the same file
/// again.
pub fn not_yet_warned(earlier: &[RunWarning], files: Vec<String>) -> Vec<String> {
    files
        .into_iter()
        .filter(|file| !earlier.iter().any(|warning| warning.files.contains(file)))
        .collect()
}
