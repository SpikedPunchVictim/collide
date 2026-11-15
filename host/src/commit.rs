//! `--commit` (ts-0031): a one-write commit, a stopgap until the pack's `record` hook exists (Stage 8). It commits
//! exactly the files that `apply` reported, in the checkout that was written, and nothing else: another file that is
//! modified, staged or untracked is left as it is, and a change the user staged in another file stays staged.
//!
//! How: a file that git does not know yet (new), or one that is gone from disk but still in the index (removed), is
//! put in the index with `git add -- <file>`; then `git commit -m <message> -- <files>`. A commit with paths takes
//! the working-tree content of those paths and leaves the rest of the index alone. Hooks run: there is no
//! `--no-verify`. When the commit fails, what `add` staged is unstaged again, so the files are left written, as
//! without `--commit`.

use std::collections::HashSet;
use std::path::Path;
use std::process::{Command, Output};

/// Runs `git` in `dir`, reading the pathspecs literally (a folder name may hold a `*`).
fn git(dir: &Path, args: &[&str], paths: &[&String]) -> Result<Output, String> {
    let mut command = Command::new("git");
    command.arg("--literal-pathspecs").args(args);
    if !paths.is_empty() {
        command.arg("--").args(paths);
    }
    command
        .current_dir(dir)
        .output()
        .map_err(|error| format!("could not run `git`: {error}"))
}

/// What git said when it failed: its messages, in the order it gave them, without the blank lines.
fn said(output: &Output) -> String {
    let mut text = String::from_utf8_lossy(&output.stderr).into_owned();
    text.push_str(&String::from_utf8_lossy(&output.stdout));
    text.trim().to_string()
}

fn run(dir: &Path, args: &[&str], paths: &[&String]) -> Result<Output, String> {
    let output = git(dir, args, paths)?;
    if output.status.success() {
        Ok(output)
    } else {
        Err(said(&output))
    }
}

/// Commits `files` (paths from the root of the checkout `dir`, as `apply` reports them) with `message`, and returns
/// the short hash of the new commit, or what git said.
pub fn commit_files(dir: &Path, files: &[String], message: &str) -> Result<String, String> {
    if files.is_empty() {
        return Err("no file was reported, so there is nothing to commit".to_string());
    }
    let mut unique: Vec<&String> = Vec::new();
    for file in files {
        if !unique.contains(&file) {
            unique.push(file);
        }
    }
    let known = run(dir, &["ls-files", "-z"], &unique)?;
    let known: HashSet<&[u8]> = known
        .stdout
        .split(|byte| *byte == 0)
        .filter(|name| !name.is_empty())
        .collect();
    let (mut to_add, mut to_commit) = (Vec::new(), Vec::new());
    for file in unique {
        let on_disk = std::fs::symlink_metadata(dir.join(file)).is_ok();
        let in_index = known.contains(file.as_bytes());
        match (on_disk, in_index) {
            (true, true) => to_commit.push(file),
            (true, false) | (false, true) => {
                to_add.push(file);
                to_commit.push(file);
            }
            // A file that was never in git and is not there now (a new file that was then renamed): nothing to record.
            (false, false) => {}
        }
    }
    if to_commit.is_empty() {
        return Err(
            "none of the reported files is in git or on disk, so there is nothing to commit"
                .to_string(),
        );
    }
    if !to_add.is_empty() {
        run(dir, &["add"], &to_add)?;
    }
    let committed = run(dir, &["commit", "-m", message], &to_commit);
    if let Err(message) = committed {
        if !to_add.is_empty() {
            let _ = git(dir, &["reset", "-q"], &to_add);
        }
        return Err(message);
    }
    let head = run(dir, &["rev-parse", "--short", "HEAD"], &[])?;
    Ok(String::from_utf8_lossy(&head.stdout).trim().to_string())
}
