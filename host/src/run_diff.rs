//! A run's diff for review (`run.diff`; ts-0078-ye; 0.11, Drill-down): the commit recorded at the run's start
//! against the head of the run's branch, as the list of changed files with their counts, or as one file's patch.
//!
//! - **Collapsed folders.** Dependency stores and build output that `.gitignore` does not hide are one row each: their
//!   file count and size, not their files ([`COLLAPSED`]; the plan's list, which packs and boards will add to). A lock
//!   file is always listed, whatever folder it is in, because a dependency change is part of the review.
//! - **Capped.** The list holds at most [`MAX_FILES`] files and [`MAX_COLLAPSED`] folders and the rest is counted in
//!   `more`, so a diff that is mostly a committed dependency store cannot grow a message past what a client reads (the
//!   overflow Ferry's runner had). A patch is cut at [`MAX_PATCH_BYTES`] and says so.
//! - **Untrusted text.** A patch is the repository's own text (0.9). Git runs with `--no-ext-diff` and `--no-textconv`,
//!   so a diff driver the repository configures never runs here.
//! - Both ends are commits, so the answer does not depend on where any checkout is now. The head is read from the
//!   branch, so a run that is still going shows what its agent has committed so far.

use crate::workspace::{Git, WorkspaceError};
use collide_core::owner::{CollapsedFolder, DiffFile, DiffStatus, FilePatch, RunDiffResult};
use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::path::Path;

/// The most files a diff lists. The rest are counted.
pub const MAX_FILES: usize = 1000;
/// The most collapsed folders a diff lists. The files of the rest are counted.
pub const MAX_COLLAPSED: usize = 100;
/// The most of a patch the result carries.
pub const MAX_PATCH_BYTES: usize = 256 * 1024;

/// The folders a diff collapses (0.11): a path that has one of these as a folder in it is not listed.
pub const COLLAPSED: [&str; 5] = ["node_modules", ".pnpm-store", "target", "dist", "coverage"];

/// Lock files that do not end in `.lock`. Any file that does is a lock file too.
const LOCK_FILES: [&str; 5] = [
    "package-lock.json",
    "npm-shrinkwrap.json",
    "pnpm-lock.yaml",
    "bun.lockb",
    "go.sum",
];

#[derive(Debug)]
pub enum DiffError {
    /// The branch of the run is not there any more: the run was discarded or accepted.
    BranchGone(String),
    /// `path` has no change between the two commits.
    NoChange(String),
    /// Git failed, or said something this does not read.
    Git(String),
}

impl fmt::Display for DiffError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DiffError::BranchGone(branch) => write!(
                f,
                "the run's branch {branch} is gone, so there is nothing left to compare"
            ),
            DiffError::NoChange(path) => write!(f, "the run's diff has no change to {path}"),
            DiffError::Git(message) => write!(f, "{message}"),
        }
    }
}

impl From<WorkspaceError> for DiffError {
    fn from(error: WorkspaceError) -> DiffError {
        DiffError::Git(error.to_string())
    }
}

/// One entry of git's raw diff.
struct Entry {
    path: String,
    old_path: Option<String>,
    status: DiffStatus,
    /// The blob to measure: the new one, or the old one for a file the run deleted.
    blob: String,
}

/// The diff of `base` (a commit) against the head of `branch`, in the repository `repo`. With `path`, the result also
/// holds that file's patch.
pub(crate) fn diff(
    git: &Git,
    repo: &Path,
    base: &str,
    branch: &str,
    path: Option<&str>,
) -> Result<RunDiffResult, DiffError> {
    let head = git
        .text(
            repo,
            &[
                "rev-parse",
                "--verify",
                &format!("refs/heads/{branch}^{{commit}}"),
            ],
        )
        .map_err(|_| DiffError::BranchGone(branch.to_string()))?;
    let range = [base, head.as_str()];
    let raw = git.run(repo, &diff_args("--raw", &range))?;
    let numstat = git.run(repo, &diff_args("--numstat", &range))?;
    let entries = parse_raw(&raw)?;
    let counts = parse_numstat(&numstat);

    let mut files = Vec::new();
    let mut folders: BTreeMap<String, Vec<&Entry>> = BTreeMap::new();
    for entry in &entries {
        match collapsed_folder(&entry.path) {
            Some(folder) => folders.entry(folder).or_default().push(entry),
            None => {
                let (additions, deletions) =
                    counts.get(&entry.path).copied().unwrap_or((None, None));
                files.push(DiffFile {
                    path: entry.path.clone(),
                    old_path: entry.old_path.clone(),
                    status: entry.status,
                    additions,
                    deletions,
                });
            }
        }
    }
    let sizes = blob_sizes(git, repo, &folders)?;
    let mut collapsed: Vec<CollapsedFolder> = folders
        .iter()
        .map(|(folder, inside)| CollapsedFolder {
            path: folder.clone(),
            files: inside.len() as u32,
            bytes: inside
                .iter()
                .map(|entry| sizes.get(&entry.blob).copied().unwrap_or(0))
                .sum(),
        })
        .collect();

    let more = cap(&mut files, &mut collapsed);

    let patch = path
        .map(|path| patch_of(git, repo, &range, path, &entries))
        .transpose()?;
    Ok(RunDiffResult {
        base: base.to_string(),
        head,
        files,
        collapsed,
        more,
        patch,
    })
}

/// Cuts the lists to [`MAX_FILES`] and [`MAX_COLLAPSED`] and returns how many files that left out.
fn cap(files: &mut Vec<DiffFile>, collapsed: &mut Vec<CollapsedFolder>) -> u32 {
    let mut more = files.len().saturating_sub(MAX_FILES);
    files.truncate(MAX_FILES);
    if collapsed.len() > MAX_COLLAPSED {
        more += collapsed[MAX_COLLAPSED..]
            .iter()
            .map(|folder| folder.files as usize)
            .sum::<usize>();
        collapsed.truncate(MAX_COLLAPSED);
    }
    u32::try_from(more).unwrap_or(u32::MAX)
}

/// The arguments of `git diff` in one `format` between two commits: renames found, no program the repository names run,
/// and `-z` so a path is never quoted.
fn diff_args<'a>(format: &'a str, range: &[&'a str; 2]) -> Vec<&'a str> {
    let mut args = vec!["diff", format, "-z", "-M", "--no-ext-diff", "--no-textconv"];
    if format == "--raw" {
        args.push("--no-abbrev");
    }
    args.extend(range);
    args
}

/// The folder a collapsed path is collapsed into, when it is: the outermost folder of the path that is one of
/// [`COLLAPSED`]. A lock file is never collapsed.
fn collapsed_folder(path: &str) -> Option<String> {
    let parts: Vec<&str> = path.split('/').collect();
    let (name, folders) = parts.split_last()?;
    if name.ends_with(".lock") || LOCK_FILES.contains(name) {
        return None;
    }
    let at = folders.iter().position(|part| COLLAPSED.contains(part))?;
    Some(folders[..=at].join("/"))
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// `git diff --raw -z --no-abbrev`: `:<modes> <old blob> <new blob> <status>` and then one path, or two for a rename.
fn parse_raw(raw: &[u8]) -> Result<Vec<Entry>, DiffError> {
    let mut parts = raw.split(|byte| *byte == 0).filter(|part| !part.is_empty());
    let mut entries = Vec::new();
    while let Some(meta) = parts.next() {
        let meta = text(meta);
        let fields: Vec<&str> = meta.trim_start_matches(':').split(' ').collect();
        let [_, _, old_blob, new_blob, status] = fields[..] else {
            return Err(DiffError::Git(format!(
                "git's diff line {meta:?} is not one this reads"
            )));
        };
        let mut next_path = || {
            parts
                .next()
                .map(text)
                .ok_or_else(|| DiffError::Git("git's diff ended inside a line".to_string()))
        };
        let (status, old_path, path) = match status.chars().next() {
            Some('A') => (DiffStatus::Added, None, next_path()?),
            Some('D') => (DiffStatus::Deleted, None, next_path()?),
            Some('R') => {
                let old = next_path()?;
                (DiffStatus::Renamed, Some(old), next_path()?)
            }
            _ => (DiffStatus::Modified, None, next_path()?),
        };
        let blob = if status == DiffStatus::Deleted {
            old_blob
        } else {
            new_blob
        };
        entries.push(Entry {
            path,
            old_path,
            status,
            blob: blob.to_string(),
        });
    }
    Ok(entries)
}

type Counts = (Option<u32>, Option<u32>);

/// `git diff --numstat -z`: `<added>\t<deleted>\t<path>`, with `-` for both of a binary file. A rename has an empty
/// path there and the two paths as the next two parts.
fn parse_numstat(numstat: &[u8]) -> HashMap<String, Counts> {
    let mut counts = HashMap::new();
    let mut parts = numstat
        .split(|byte| *byte == 0)
        .filter(|part| !part.is_empty());
    while let Some(line) = parts.next() {
        let line = text(line);
        let mut fields = line.splitn(3, '\t');
        let (Some(added), Some(deleted), Some(path)) =
            (fields.next(), fields.next(), fields.next())
        else {
            continue;
        };
        let path = if path.is_empty() {
            parts.next();
            parts.next().map(text).unwrap_or_default()
        } else {
            path.to_string()
        };
        counts.insert(path, (added.parse().ok(), deleted.parse().ok()));
    }
    counts
}

/// The size of each blob the collapsed folders hold, from one `git cat-file --batch-check`. A blob git does not have
/// (a submodule's commit) counts as nothing.
fn blob_sizes(
    git: &Git,
    repo: &Path,
    folders: &BTreeMap<String, Vec<&Entry>>,
) -> Result<HashMap<String, u64>, DiffError> {
    let blobs: Vec<&str> = folders
        .values()
        .flatten()
        .map(|entry| entry.blob.as_str())
        .collect();
    if blobs.is_empty() {
        return Ok(HashMap::new());
    }
    let input = blobs
        .iter()
        .map(|blob| format!("{blob}\n"))
        .collect::<String>();
    let output = git.run_with_input(
        repo,
        &["cat-file", "--batch-check=%(objectsize)"],
        input.into_bytes(),
    )?;
    let output = text(&output);
    Ok(blobs
        .iter()
        .zip(output.lines())
        .map(|(blob, line)| ((*blob).to_string(), line.trim().parse().unwrap_or(0)))
        .collect())
}

/// One file's patch, cut at [`MAX_PATCH_BYTES`] on a character boundary. A rename's patch names both paths.
fn patch_of(
    git: &Git,
    repo: &Path,
    range: &[&str; 2],
    path: &str,
    entries: &[Entry],
) -> Result<FilePatch, DiffError> {
    let mut args = vec![
        "--literal-pathspecs",
        "diff",
        "-M",
        "--no-ext-diff",
        "--no-textconv",
        "--no-color",
    ];
    args.extend(range);
    args.extend(["--", path]);
    let old_path = entries
        .iter()
        .find(|entry| entry.path == path)
        .and_then(|entry| entry.old_path.as_deref());
    args.extend(old_path);
    let full = text(&git.run(repo, &args)?);
    if full.is_empty() {
        return Err(DiffError::NoChange(path.to_string()));
    }
    let bytes = full.len() as u64;
    let mut cut = full.len().min(MAX_PATCH_BYTES);
    while !full.is_char_boundary(cut) {
        cut -= 1;
    }
    Ok(FilePatch {
        path: path.to_string(),
        text: full[..cut].to_string(),
        bytes,
        truncated: cut < full.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_path_below_a_collapsed_folder_collapses_into_the_outermost_one() {
        assert_eq!(
            collapsed_folder("node_modules/a/b.js").as_deref(),
            Some("node_modules")
        );
        assert_eq!(
            collapsed_folder("packages/web/node_modules/dep/node_modules/x.js").as_deref(),
            Some("packages/web/node_modules")
        );
        assert_eq!(collapsed_folder("src/a.rs"), None);
    }

    #[test]
    fn a_file_named_like_a_collapsed_folder_is_not_a_folder() {
        assert_eq!(collapsed_folder("src/target"), None);
        assert_eq!(collapsed_folder("dist"), None);
    }

    #[test]
    fn a_lock_file_is_never_collapsed() {
        for path in [
            "node_modules/pkg/yarn.lock",
            "target/Cargo.lock",
            "dist/package-lock.json",
            "coverage/go.sum",
            "node_modules/.pnpm/pnpm-lock.yaml",
        ] {
            assert_eq!(collapsed_folder(path), None, "{path}");
        }
    }

    #[test]
    fn the_lists_are_capped_and_the_files_left_out_are_counted() {
        let file = |n: usize| DiffFile {
            path: format!("f{n}"),
            old_path: None,
            status: DiffStatus::Added,
            additions: Some(1),
            deletions: Some(0),
        };
        let folder = |n: usize| CollapsedFolder {
            path: format!("d{n}/node_modules"),
            files: 3,
            bytes: 1,
        };
        let mut files: Vec<DiffFile> = (0..MAX_FILES + 5).map(file).collect();
        let mut collapsed: Vec<CollapsedFolder> = (0..MAX_COLLAPSED + 2).map(folder).collect();
        assert_eq!(cap(&mut files, &mut collapsed), 5 + 2 * 3);
        assert_eq!((files.len(), collapsed.len()), (MAX_FILES, MAX_COLLAPSED));
    }

    #[test]
    fn a_rename_is_read_with_both_of_its_paths_and_its_counts() {
        let raw =
            b":100644 100644 aaa bbb R100\0old.txt\0new.txt\0:100644 000000 ccc 000 D\0gone.txt\0";
        let entries = parse_raw(raw).unwrap();
        assert_eq!(entries[0].status, DiffStatus::Renamed);
        assert_eq!(entries[0].old_path.as_deref(), Some("old.txt"));
        assert_eq!(entries[0].path, "new.txt");
        assert_eq!(entries[1].status, DiffStatus::Deleted);
        assert_eq!(
            entries[1].blob, "ccc",
            "a deleted file is measured by its old blob"
        );
        let numstat = b"0\t0\t\0old.txt\0new.txt\0-\t-\tbin.dat\0";
        let counts = parse_numstat(numstat);
        assert_eq!(counts["new.txt"], (Some(0), Some(0)));
        assert_eq!(counts["bin.dat"], (None, None));
    }
}
