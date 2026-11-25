//! A task's changes (`task.changes` and `task.diff`; ts-0188-kb; 0.8, Phase; 0.11, Task drawer, Changes): the files a
//! run changed, from the commit its first phase started at to the commit its last phase ended at, and the commits on
//! the board's line that name the task. The daemon reads the parts (the runs, their heads, the board's line) and the
//! functions here read git, so a test hands them a repository.
//!
//! - **A span ends at the working tree while a phase is active** ([`Span::WorkingTree`]): the tracked changes against
//!   the start head, and the new files git does not ignore. Nothing is added or staged to read them: git is asked for
//!   the diff, the list of new files, and the files themselves.
//! - **A span that has ended ends at the end head** ([`Span::Between`]), so what is written in the worktree after the
//!   phase is not the run's.
//! - **Commits are found by their message**: `git log --fixed-strings --grep <id>` on the line, then a word-bounded
//!   check ([`names_id`]), so `ts-0160-ab` is not found in `ts-0160-abc`. At most [`MAX_COMMITS`] are kept, newest
//!   first. A commit inside a run's span is the run's ([`commits_in`]).
//! - **Untrusted text.** Every diff and log passes `--no-ext-diff` and `--no-textconv`, so a diff driver or a textconv
//!   the repository configures never runs ([`crate::run_diff`]). The id is an argument of git, never a shell word.
//! - **The limits are `run.diff`'s**: [`MAX_FILES`], [`MAX_COLLAPSED`] and a patch cut at [`MAX_PATCH_BYTES`].

use crate::run_diff::DiffError;
use crate::run_diff::{
    Counts, Entry, Listing, MAX_FILES, MAX_PATCH_BYTES, collapsed_folder, cut_patch, diff_args,
    listing, parse_numstat, parse_raw, text,
};
use crate::run_store::{PhaseHeads, is_live};
use crate::workspace::Git;
use collide_core::owner::{
    ChangeWords, ChangesPlace, DiffStatus, FilePatch, NumberedLine, RunChanges, WholeFile,
};
use collide_core::state::RunState;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

/// The most commits a task lists.
pub(crate) const MAX_COMMITS: usize = 200;
/// How many commits git is asked for before the word-bounded check drops some, so that check cannot starve the list.
const LOG_LIMIT: &str = "1000";
/// The most commits of a span that are read, to be left out of the list of commits.
const SPAN_LIMIT: &str = "10000";
/// A new file larger than this is listed with no line counts: reading it only to count would stall the answer.
const MAX_COUNTED_BYTES: u64 = 4 * 1024 * 1024;
/// How much of a file is looked at for a NUL, which says it is not text (git's own measure).
const BINARY_SCAN_BYTES: usize = 8000;

/// What a list of changes reaches from and to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Span {
    /// Two commits: a run whose last phase ended.
    Between { base: String, head: String },
    /// A commit and the working tree: a run whose phase is active.
    WorkingTree { base: String },
    /// One commit against its first parent, or against nothing for a root commit.
    Commit { id: String },
}

/// A commit that names the task.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Commit {
    pub(crate) id: String,
    pub(crate) subject: String,
    pub(crate) date_ms: u64,
}

/// The span of a run from the heads of its phases: its first phase's start to its last phase's end, or to the working
/// tree while the last phase is active. `None` when the heads say nothing: there are none, or the run is over and its
/// last phase has no end head (the daemon did not see it end).
pub(crate) fn span_of(heads: &[PhaseHeads], live: bool) -> Option<Span> {
    let first = heads.first()?;
    let last = heads.last()?;
    match &last.end_head {
        Some(end) => Some(Span::Between {
            base: first.start_head.clone(),
            head: end.clone(),
        }),
        None if live => Some(Span::WorkingTree {
            base: first.start_head.clone(),
        }),
        None => None,
    }
}

/// What the daemon knows of one run of the task.
pub(crate) struct RunInput<'a> {
    pub(crate) run: &'a str,
    pub(crate) state: &'a RunState,
    pub(crate) heads: &'a [PhaseHeads],
    /// The folder the run works in, while it is there.
    pub(crate) workspace: Option<&'a Path>,
    pub(crate) place: ChangesPlace,
}

/// A run's span and the folder git reads it in.
pub(crate) struct RunSpan {
    pub(crate) span: Span,
    pub(crate) cwd: PathBuf,
}

/// The span of `input` and where to read it: its own folder while that is there, else the board's (`root`), which has
/// the commits of a span that ended. A span that ends at the working tree needs the folder.
pub(crate) fn run_span(input: &RunInput, root: &Path) -> Option<RunSpan> {
    let span = span_of(input.heads, is_live(input.state))?;
    let cwd = match (&span, input.workspace) {
        (_, Some(workspace)) => workspace.to_path_buf(),
        (Span::WorkingTree { .. }, None) => return None,
        (_, None) => root.to_path_buf(),
    };
    Some(RunSpan { span, cwd })
}

/// One run's changes. A run with no span lists no files and says why; a run whose git failed says that instead.
pub(crate) fn changes_of_run(git: &Git, root: &Path, input: &RunInput) -> RunChanges {
    let located = run_span(input, root);
    let (first, last) = match (input.heads.first(), input.heads.last()) {
        (Some(first), Some(last)) => (first.phase.as_str(), last.phase.as_str()),
        _ => ("", ""),
    };
    let words = run_words(
        input.run,
        input.place,
        located.as_ref().map(|found| &found.span),
        first,
        last,
    );
    let mut changes = RunChanges {
        run: input.run.to_string(),
        state: input.state.clone(),
        workspace: input
            .workspace
            .map(|path| path.to_string_lossy().into_owned()),
        place: input.place,
        words,
        files: Vec::new(),
        collapsed: Vec::new(),
        more: 0,
        note: None,
    };
    match located {
        None => changes.note = Some(NO_CHANGES_RECORDED.to_string()),
        Some(RunSpan { span, cwd }) => match list(git, &cwd, &span) {
            Ok(found) => {
                changes.files = found.files;
                changes.collapsed = found.collapsed;
                changes.more = found.more;
            }
            Err(error) => changes.note = Some(format!("the changes were not read: {error}")),
        },
    }
    changes
}

/// What a run says when it has nothing to list: it has no heads, or its work folder is gone.
pub(crate) const NO_CHANGES_RECORDED: &str = "no changes recorded";

// ---- the words ----------------------------------------------------------------------------------------------------

/// The words for a run's changes. `first` and `last` are the names of its first and last phases. `span` is `None` for
/// a run that has none.
pub(crate) fn run_words(
    run: &str,
    place: ChangesPlace,
    span: Option<&Span>,
    first: &str,
    last: &str,
) -> ChangeWords {
    let where_ = match place {
        ChangesPlace::Worktree => format!("Run {run}'s worktree"),
        ChangesPlace::Checkout => "Your checkout".to_string(),
    };
    let how = match (span, place) {
        (None, _) => "this run kept no record of where it began".to_string(),
        (Some(Span::WorkingTree { .. }), ChangesPlace::Worktree) => {
            format!("since the {first} phase began, including edits not committed yet")
        }
        (Some(Span::WorkingTree { .. }), ChangesPlace::Checkout) => format!(
            "since the {first} phase began, including edits not committed yet and your own edits"
        ),
        (Some(_), _) if first == last => {
            format!("from where the {first} phase began to where it ended")
        }
        (Some(_), _) => {
            format!("from where the {first} phase began to where the {last} phase ended")
        }
    };
    ChangeWords { place: where_, how }
}

/// The words for the commits that name a task on `line`. `runs` is how many runs the task has.
pub(crate) fn commit_words(line: &str, id: &str, commits: &[Commit], runs: usize) -> ChangeWords {
    let place = format!("Your checkout, line {line}");
    let shorts: Vec<&str> = commits.iter().map(|c| &c.id[..c.id.len().min(7)]).collect();
    let listed = match shorts.split_last() {
        None => String::new(),
        Some((only, [])) => (*only).to_string(),
        Some((last, rest)) => format!("{} and {last}", rest.join(", ")),
    };
    let how = match commits.len() {
        0 => format!("no commit on this line names {id}"),
        1 => format!("1 commit that names {id}: {listed}"),
        n => format!("{n} commits that name {id}: {listed}"),
    };
    let how = if runs == 0 && !commits.is_empty() {
        format!("{how}. No run built it.")
    } else {
        how
    };
    ChangeWords { place, how }
}

// ---- the files ----------------------------------------------------------------------------------------------------

/// The arguments of git for `span` in one `format` (`--raw` or `--numstat`).
fn list_args<'a>(span: &'a Span, format: &'a str) -> Vec<&'a str> {
    match span {
        Span::Between { base, head } => diff_args(format, &[base, head]),
        Span::WorkingTree { base } => {
            let mut args = vec!["--no-optional-locks"];
            args.extend(diff_args(format, &[base]));
            args
        }
        Span::Commit { id } => {
            let mut args = commit_args(format);
            args.push(id);
            args
        }
    }
}

/// `git diff-tree` of one commit in one `format`: a merge against its first parent, a root commit against nothing.
fn commit_args(format: &str) -> Vec<&str> {
    let mut args = vec![
        "diff-tree",
        format,
        "-M",
        "--no-ext-diff",
        "--no-textconv",
        "-r",
        "--root",
        "--no-commit-id",
        "-m",
        "--first-parent",
    ];
    if format == "--raw" {
        args.extend(["-z", "--no-abbrev"]);
    } else if format == "--numstat" {
        args.push("-z");
    }
    args
}

/// The tracked entries of `span` and their counts.
fn tracked(
    git: &Git,
    cwd: &Path,
    span: &Span,
) -> Result<(Vec<Entry>, HashMap<String, Counts>), DiffError> {
    let raw = git.run(cwd, &list_args(span, "--raw"))?;
    let numstat = git.run(cwd, &list_args(span, "--numstat"))?;
    Ok((parse_raw(&raw)?, parse_numstat(&numstat)))
}

/// The files of `span`, run in `cwd`: a worktree or checkout for a working-tree span, any folder of the repository
/// otherwise.
pub(crate) fn list(git: &Git, cwd: &Path, span: &Span) -> Result<Listing, DiffError> {
    let (mut entries, mut counts) = tracked(git, cwd, span)?;
    if matches!(span, Span::WorkingTree { .. }) {
        add_untracked(git, cwd, &mut entries, &mut counts)?;
    }
    entries.sort_by(|a, b| a.path.cmp(&b.path));
    listing(git, cwd, &entries, &counts)
}

/// The new files git does not ignore, as added entries. A file that is listed with a count is read to count its lines.
fn add_untracked(
    git: &Git,
    cwd: &Path,
    entries: &mut Vec<Entry>,
    counts: &mut HashMap<String, Counts>,
) -> Result<(), DiffError> {
    let mut counted = 0;
    for path in untracked(git, cwd)? {
        let Ok(meta) = std::fs::symlink_metadata(cwd.join(&path)) else {
            continue;
        };
        if collapsed_folder(&path).is_none() && counted < MAX_FILES {
            counted += 1;
            counts.insert(path.clone(), count_lines(cwd, &path, meta.len()));
        }
        entries.push(Entry {
            path,
            old_path: None,
            status: DiffStatus::Added,
            blob: String::new(),
            size: Some(meta.len()),
        });
    }
    Ok(())
}

fn untracked(git: &Git, cwd: &Path) -> Result<Vec<String>, DiffError> {
    let out = git.run(
        cwd,
        &[
            "--no-optional-locks",
            "ls-files",
            "--others",
            "--exclude-standard",
            "-z",
        ],
    )?;
    Ok(out
        .split(|byte| *byte == 0)
        .filter(|part| !part.is_empty())
        .map(text)
        .collect())
}

/// The lines a new file adds, as git counts them, or none for a file that is binary, large or unreadable.
fn count_lines(cwd: &Path, path: &str, len: u64) -> Counts {
    if len > MAX_COUNTED_BYTES {
        return (None, None);
    }
    match read_inside(cwd, path, MAX_COUNTED_BYTES) {
        Ok((bytes, _)) if !is_binary(&bytes) => {
            let newlines = bytes.iter().filter(|byte| **byte == b'\n').count();
            let open = usize::from(bytes.last().is_some_and(|byte| *byte != b'\n'));
            (u32::try_from(newlines + open).ok(), Some(0))
        }
        _ => (None, None),
    }
}

fn is_binary(bytes: &[u8]) -> bool {
    bytes[..bytes.len().min(BINARY_SCAN_BYTES)].contains(&0)
}

/// A file of the tree at `cwd`, or the target of a link, up to `limit` bytes, and the file's whole size. A path that
/// leaves the tree (a folder in it is a link to somewhere else) is refused.
fn read_inside(cwd: &Path, path: &str, limit: u64) -> std::io::Result<(Vec<u8>, u64)> {
    let target = cwd.join(path);
    if std::fs::symlink_metadata(&target)?.file_type().is_symlink() {
        let link = std::fs::read_link(&target)?.to_string_lossy().into_owned();
        let bytes = link.into_bytes();
        let size = bytes.len() as u64;
        return Ok((bytes, size));
    }
    let real = std::fs::canonicalize(&target)?;
    if !real.starts_with(std::fs::canonicalize(cwd)?) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            format!("{path} is outside the folder"),
        ));
    }
    let file = File::open(&real)?;
    let size = file.metadata()?.len();
    let mut bytes = Vec::new();
    file.take(limit).read_to_end(&mut bytes)?;
    Ok((bytes, size))
}

// ---- one file -----------------------------------------------------------------------------------------------------

/// A path a client names must be below the folder: not empty, not absolute, with no `..`.
fn checked_path(path: &str) -> Result<(), DiffError> {
    let bad = path.is_empty()
        || path.starts_with('/')
        || path.contains('\0')
        || path.split('/').any(|part| part == "..");
    if bad {
        return Err(DiffError::Git(format!(
            "{path:?} is not a path inside the repository"
        )));
    }
    Ok(())
}

/// One file's patch in `span`, cut at [`MAX_PATCH_BYTES`].
pub(crate) fn patch(
    git: &Git,
    cwd: &Path,
    span: &Span,
    path: &str,
) -> Result<FilePatch, DiffError> {
    checked_path(path)?;
    let (entries, _) = tracked(git, cwd, span)?;
    let full = full_patch(git, cwd, span, path, &entries)?;
    Ok(cut_patch(path, &full))
}

fn full_patch(
    git: &Git,
    cwd: &Path,
    span: &Span,
    path: &str,
    entries: &[Entry],
) -> Result<String, DiffError> {
    let Some(entry) = entries.iter().find(|entry| entry.path == path) else {
        return match span {
            Span::WorkingTree { .. } if untracked(git, cwd)?.iter().any(|p| p == path) => {
                new_file_patch(cwd, path)
            }
            _ => Err(DiffError::NoChange(path.to_string())),
        };
    };
    let mut args = vec!["--literal-pathspecs"];
    match span {
        Span::Between { base, head } => {
            args.extend(["diff", "-M", "--no-ext-diff", "--no-textconv", "--no-color"]);
            args.extend([base.as_str(), head.as_str()]);
        }
        Span::WorkingTree { base } => {
            args.extend(["--no-optional-locks", "diff", "-M", "--no-ext-diff"]);
            args.extend(["--no-textconv", "--no-color", base.as_str()]);
        }
        Span::Commit { id } => {
            let mut commit = commit_args("-p");
            commit.extend(["--no-color", id.as_str()]);
            args.extend(commit);
        }
    }
    args.extend(["--", path]);
    args.extend(entry.old_path.as_deref());
    let full = text(&git.run(cwd, &args)?);
    if full.is_empty() {
        return Err(DiffError::NoChange(path.to_string()));
    }
    Ok(full)
}

/// The patch of a new file, written here: git has nothing to compare it with. A file longer than the cap is read to the
/// cap, and the patch is cut there.
fn new_file_patch(cwd: &Path, path: &str) -> Result<String, DiffError> {
    let read = |error: std::io::Error| DiffError::Git(format!("reading {path}: {error}"));
    let (bytes, size) = read_inside(cwd, path, MAX_PATCH_BYTES as u64 + 1).map_err(read)?;
    let mode = if std::fs::symlink_metadata(cwd.join(path))
        .map_err(read)?
        .file_type()
        .is_symlink()
    {
        "120000"
    } else {
        "100644"
    };
    let head = format!("diff --git a/{path} b/{path}\nnew file mode {mode}\n");
    if is_binary(&bytes) {
        return Ok(format!(
            "{head}Binary files /dev/null and b/{path} differ\n"
        ));
    }
    let body = String::from_utf8_lossy(&bytes);
    let lines: Vec<&str> = body.lines().collect();
    let mut out = format!(
        "{head}--- /dev/null\n+++ b/{path}\n@@ -0,0 +1,{} @@\n",
        lines.len()
    );
    for line in &lines {
        out.push('+');
        out.push_str(line);
        out.push('\n');
    }
    if !body.ends_with('\n') && !body.is_empty() && size <= bytes.len() as u64 {
        out.push_str("\\ No newline at end of file\n");
    }
    Ok(out)
}

/// A file as it is after the changes of `span`, each line numbered and marked as changed or not. A file that is not
/// text has no lines. The lines stop at [`MAX_PATCH_BYTES`], and `truncated` says so. The note says what was left out.
pub(crate) fn whole(
    git: &Git,
    cwd: &Path,
    span: &Span,
    path: &str,
) -> Result<(WholeFile, Option<String>), DiffError> {
    checked_path(path)?;
    let (entries, _) = tracked(git, cwd, span)?;
    let entry = entries.iter().find(|entry| entry.path == path);
    if entry.is_some_and(|entry| entry.status == DiffStatus::Deleted) {
        let file = WholeFile {
            lines: Vec::new(),
            bytes: 0,
            truncated: false,
            binary: false,
        };
        return Ok((file, Some("the changes delete this file".to_string())));
    }
    let full_patch = full_patch(git, cwd, span, path, &entries)?;
    let (bytes, size) = after(git, cwd, span, path)?;
    if is_binary(&bytes) {
        let file = WholeFile {
            lines: Vec::new(),
            bytes: size,
            truncated: false,
            binary: true,
        };
        return Ok((file, Some("the file is not text".to_string())));
    }
    let changed = changed_lines(&full_patch);
    let body = String::from_utf8_lossy(&bytes);
    let mut lines = Vec::new();
    let mut used = 0;
    let mut truncated = size > bytes.len() as u64;
    for (index, line) in body.lines().enumerate() {
        used += line.len() + 1;
        if used > MAX_PATCH_BYTES {
            truncated = true;
            break;
        }
        let n = u32::try_from(index + 1).unwrap_or(u32::MAX);
        lines.push(NumberedLine {
            n,
            text: line.to_string(),
            changed: changed.contains(&n),
        });
    }
    let note = truncated.then(|| {
        format!(
            "the file is {size} bytes and is shown to {} lines, where the {} byte cap falls",
            lines.len(),
            MAX_PATCH_BYTES
        )
    });
    let file = WholeFile {
        lines,
        bytes: size,
        truncated,
        binary: false,
    };
    Ok((file, note))
}

/// The file's bytes after the changes (up to a little over the cap) and its whole size.
fn after(git: &Git, cwd: &Path, span: &Span, path: &str) -> Result<(Vec<u8>, u64), DiffError> {
    let rev = match span {
        Span::Between { head, .. } => head,
        Span::Commit { id } => id,
        Span::WorkingTree { .. } => {
            return read_inside(cwd, path, MAX_PATCH_BYTES as u64 + 1)
                .map_err(|error| DiffError::Git(format!("reading {path}: {error}")));
        }
    };
    let bytes = git.run(cwd, &["cat-file", "blob", &format!("{rev}:{path}")])?;
    let size = bytes.len() as u64;
    let kept = bytes.len().min(MAX_PATCH_BYTES + 1);
    Ok((bytes[..kept].to_vec(), size))
}

/// The numbers of the lines a patch adds, in the new file. Lines before the first hunk (the headers) are not read.
fn changed_lines(patch: &str) -> HashSet<u32> {
    let mut changed = HashSet::new();
    let mut next: Option<u32> = None;
    for line in patch.lines() {
        if let Some(rest) = line.strip_prefix("@@ ") {
            next = rest
                .split(' ')
                .find_map(|word| word.strip_prefix('+'))
                .and_then(|range| range.split(',').next())
                .and_then(|start| start.parse().ok());
            continue;
        }
        let Some(n) = next else { continue };
        match line.as_bytes().first() {
            Some(b'+') => {
                changed.insert(n);
                next = Some(n + 1);
            }
            Some(b' ') | None => next = Some(n + 1),
            _ => {}
        }
    }
    changed
}

// ---- commits ------------------------------------------------------------------------------------------------------

/// Whether `message` names `id` as a word of its own: the id is not inside a longer word, where a word is made of
/// letters, digits, `-` and `_`.
pub(crate) fn names_id(message: &str, id: &str) -> bool {
    if id.is_empty() {
        return false;
    }
    let word = |c: char| c.is_alphanumeric() || c == '-' || c == '_';
    message.match_indices(id).any(|(at, _)| {
        let before = message[..at].chars().next_back();
        let after = message[at + id.len()..].chars().next();
        !before.is_some_and(word) && !after.is_some_and(word)
    })
}

/// The commits on `line` whose message names `id`, newest first, at most [`MAX_COMMITS`]. `repo` is any folder of the
/// repository.
pub(crate) fn naming(
    git: &Git,
    repo: &Path,
    line: &str,
    id: &str,
) -> Result<Vec<Commit>, DiffError> {
    let grep = format!("--grep={id}");
    let out = git.run(
        repo,
        &[
            "log",
            "--no-ext-diff",
            "--no-textconv",
            "--fixed-strings",
            &grep,
            "-n",
            LOG_LIMIT,
            "-z",
            "--format=%H%x1f%ct%x1f%s%x1f%B",
            line,
            "--",
        ],
    )?;
    let mut commits = Vec::new();
    for record in out.split(|byte| *byte == 0).filter(|part| !part.is_empty()) {
        let record = text(record);
        let mut fields = record.splitn(4, '\u{1f}');
        let (Some(hash), Some(seconds), Some(subject), Some(message)) =
            (fields.next(), fields.next(), fields.next(), fields.next())
        else {
            return Err(DiffError::Git(format!(
                "git's log line {record:?} is not one this reads"
            )));
        };
        if !names_id(message, id) {
            continue;
        }
        commits.push(Commit {
            id: hash.trim().to_string(),
            subject: subject.to_string(),
            date_ms: seconds.parse::<u64>().unwrap_or(0).saturating_mul(1000),
        });
        if commits.len() == MAX_COMMITS {
            break;
        }
    }
    Ok(commits)
}

/// The commits after `base` up to `head` (a commit, or `HEAD` of the folder `cwd` is): a run's own.
pub(crate) fn commits_in(
    git: &Git,
    cwd: &Path,
    base: &str,
    head: &str,
) -> Result<HashSet<String>, DiffError> {
    let range = format!("{base}..{head}");
    let out = git.run(cwd, &["rev-list", "--max-count", SPAN_LIMIT, &range, "--"])?;
    Ok(text(&out)
        .lines()
        .map(|line| line.trim().to_string())
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::process::Command;
    use std::sync::atomic::{AtomicU32, Ordering};

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    /// A repository in a folder made for one test, with git's own config and the user's out of reach.
    struct Repo {
        dir: PathBuf,
    }

    impl Repo {
        fn new() -> Repo {
            let n = COUNTER.fetch_add(1, Ordering::SeqCst);
            let dir = std::env::temp_dir()
                .join(format!("collide-task-changes-{}-{n}", std::process::id()));
            fs::create_dir_all(dir.join("repo")).unwrap();
            let repo = Repo { dir };
            repo.git(&["init", "-q", "-b", "main"]);
            repo
        }

        fn root(&self) -> PathBuf {
            self.dir.join("repo")
        }

        fn env(&self) -> Vec<(String, String)> {
            let home = self.dir.to_string_lossy().into_owned();
            vec![
                ("HOME".into(), home),
                ("GIT_CONFIG_GLOBAL".into(), "/dev/null".into()),
                ("GIT_CONFIG_SYSTEM".into(), "/dev/null".into()),
                ("GIT_CONFIG_NOSYSTEM".into(), "1".into()),
            ]
        }

        fn git_in(&self, cwd: &Path, args: &[&str]) -> String {
            let out = Command::new("git")
                .env_clear()
                .envs(self.env())
                .env("PATH", std::env::var_os("PATH").unwrap_or_default())
                .args(["-c", "user.name=t", "-c", "user.email=t@example.invalid"])
                .args(["-c", "commit.gpgsign=false"])
                .args(args)
                .current_dir(cwd)
                .output()
                .unwrap();
            assert!(
                out.status.success(),
                "git {args:?}: {}",
                String::from_utf8_lossy(&out.stderr)
            );
            String::from_utf8_lossy(&out.stdout).trim().to_string()
        }

        fn git(&self, args: &[&str]) -> String {
            self.git_in(&self.root(), args)
        }

        fn write(&self, path: &str, text: &str) {
            let file = self.root().join(path);
            fs::create_dir_all(file.parent().unwrap()).unwrap();
            fs::write(file, text).unwrap();
        }

        fn commit(&self, message: &str) -> String {
            self.git(&["add", "-A"]);
            self.git(&["commit", "-q", "-m", message]);
            self.git(&["rev-parse", "HEAD"])
        }

        fn tool(&self) -> Git {
            Git::with_env(self.env())
        }
    }

    impl Drop for Repo {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.dir);
        }
    }

    fn paths(listing: &Listing) -> Vec<&str> {
        listing
            .files
            .iter()
            .map(|file| file.path.as_str())
            .collect()
    }

    #[test]
    fn a_working_tree_span_lists_tracked_edits_and_new_files_and_not_ignored_ones() {
        let repo = Repo::new();
        repo.write(".gitignore", "ignored.txt\n");
        repo.write("kept.txt", "one\ntwo\n");
        let base = repo.commit("start");
        repo.write("kept.txt", "one\ntwo\nthree\n");
        repo.write("new.txt", "a\nb");
        repo.write("ignored.txt", "x\n");
        repo.write("node_modules/pkg/index.js", "x\n");

        let found = list(&repo.tool(), &repo.root(), &Span::WorkingTree { base }).unwrap();

        assert_eq!(paths(&found), ["kept.txt", "new.txt"]);
        assert_eq!(found.files[0].additions, Some(1));
        assert_eq!(found.files[1].status, DiffStatus::Added);
        assert_eq!(
            (found.files[1].additions, found.files[1].deletions),
            (Some(2), Some(0)),
            "a new file's lines are counted, with or without a last newline"
        );
        assert_eq!(
            found.collapsed.len(),
            1,
            "a new dependency folder is one row"
        );
        assert_eq!(found.collapsed[0].path, "node_modules");
    }

    #[test]
    fn a_span_between_two_commits_does_not_hold_what_is_written_after_the_last() {
        let repo = Repo::new();
        repo.write("a.txt", "a\n");
        let base = repo.commit("start");
        repo.write("b.txt", "b\n");
        let head = repo.commit("work");
        repo.write("c.txt", "c\n");

        let found = list(&repo.tool(), &repo.root(), &Span::Between { base, head }).unwrap();

        assert_eq!(paths(&found), ["b.txt"]);
    }

    #[test]
    fn a_commit_span_lists_that_commits_files_and_a_root_commit_lists_all_of_its_files() {
        let repo = Repo::new();
        repo.write("a.txt", "a\n");
        let root_commit = repo.commit("start");
        repo.write("a.txt", "a\nmore\n");
        repo.write("b.txt", "b\n");
        let second = repo.commit("work");

        let of_second = list(&repo.tool(), &repo.root(), &Span::Commit { id: second }).unwrap();
        let of_root = list(
            &repo.tool(),
            &repo.root(),
            &Span::Commit { id: root_commit },
        )
        .unwrap();

        assert_eq!(paths(&of_second), ["a.txt", "b.txt"]);
        assert_eq!(paths(&of_root), ["a.txt"]);
        assert_eq!(of_root.files[0].status, DiffStatus::Added);
    }

    #[test]
    fn the_patch_of_a_new_file_is_written_with_every_line_added() {
        let repo = Repo::new();
        repo.write("a.txt", "a\n");
        let base = repo.commit("start");
        repo.write("new.txt", "one\ntwo\n");

        let patch = patch(
            &repo.tool(),
            &repo.root(),
            &Span::WorkingTree { base },
            "new.txt",
        )
        .unwrap();

        assert!(
            patch.text.contains("new file mode 100644"),
            "{}",
            patch.text
        );
        assert!(
            patch.text.contains("@@ -0,0 +1,2 @@\n+one\n+two\n"),
            "{}",
            patch.text
        );
        assert!(!patch.truncated);
    }

    #[test]
    fn the_patch_of_a_changed_file_is_gits() {
        let repo = Repo::new();
        repo.write("a.txt", "a\nb\n");
        let base = repo.commit("start");
        repo.write("a.txt", "a\nc\n");

        let patch = patch(
            &repo.tool(),
            &repo.root(),
            &Span::WorkingTree { base },
            "a.txt",
        )
        .unwrap();

        assert!(patch.text.contains("-b\n+c\n"), "{}", patch.text);
    }

    #[test]
    fn a_new_binary_file_has_a_patch_that_says_so_and_a_whole_file_with_no_lines() {
        let repo = Repo::new();
        repo.write("a.txt", "a\n");
        let base = repo.commit("start");
        fs::write(repo.root().join("img.bin"), [0u8, 1, 2, 3]).unwrap();
        let span = Span::WorkingTree { base };

        let patch = patch(&repo.tool(), &repo.root(), &span, "img.bin").unwrap();
        let (whole, note) = whole(&repo.tool(), &repo.root(), &span, "img.bin").unwrap();

        assert!(
            patch
                .text
                .contains("Binary files /dev/null and b/img.bin differ")
        );
        assert!(whole.binary && whole.lines.is_empty());
        assert_eq!(whole.bytes, 4);
        assert_eq!(note.as_deref(), Some("the file is not text"));
    }

    #[test]
    fn a_committed_binary_file_has_no_lines() {
        let repo = Repo::new();
        repo.write("a.txt", "a\n");
        let base = repo.commit("start");
        fs::write(repo.root().join("img.bin"), [0u8, 1, 2, 3]).unwrap();
        let head = repo.commit("binary");
        let span = Span::Between { base, head };

        let patch = patch(&repo.tool(), &repo.root(), &span, "img.bin").unwrap();
        let (whole, _) = whole(&repo.tool(), &repo.root(), &span, "img.bin").unwrap();

        assert!(patch.text.contains("Binary files"), "{}", patch.text);
        assert!(whole.binary);
    }

    #[test]
    fn the_whole_file_after_the_changes_marks_the_lines_the_changes_added() {
        let repo = Repo::new();
        repo.write("a.txt", "one\ntwo\nthree\n");
        let base = repo.commit("start");
        repo.write("a.txt", "one\nTWO\nthree\nfour\n");
        let head = repo.commit("work");

        let (found, note) = whole(
            &repo.tool(),
            &repo.root(),
            &Span::Between { base, head },
            "a.txt",
        )
        .unwrap();

        let marked: Vec<(u32, &str, bool)> = found
            .lines
            .iter()
            .map(|line| (line.n, line.text.as_str(), line.changed))
            .collect();
        assert_eq!(
            marked,
            [
                (1, "one", false),
                (2, "TWO", true),
                (3, "three", false),
                (4, "four", true)
            ]
        );
        assert_eq!(note, None);
    }

    #[test]
    fn the_whole_file_of_the_working_tree_is_the_file_as_it_is_now() {
        let repo = Repo::new();
        repo.write("a.txt", "one\ntwo\n");
        let base = repo.commit("start");
        repo.write("a.txt", "one\ntwo\nthree\n");

        let (found, _) = whole(
            &repo.tool(),
            &repo.root(),
            &Span::WorkingTree { base },
            "a.txt",
        )
        .unwrap();

        assert_eq!(found.lines.len(), 3);
        assert!(found.lines[2].changed && !found.lines[0].changed);
    }

    #[test]
    fn the_whole_file_stops_at_the_patch_cap_and_says_so() {
        let repo = Repo::new();
        repo.write("a.txt", "a\n");
        let base = repo.commit("start");
        let line = "x".repeat(99);
        let text = format!("{line}\n").repeat(MAX_PATCH_BYTES / 100 + 500);
        repo.write("big.txt", &text);
        let head = repo.commit("big");
        let span = Span::Between { base, head };

        let (found, note) = whole(&repo.tool(), &repo.root(), &span, "big.txt").unwrap();

        assert!(found.truncated);
        assert_eq!(found.bytes, text.len() as u64);
        assert!(found.lines.len() * 100 <= MAX_PATCH_BYTES);
        assert!(found.lines.len() * 100 > MAX_PATCH_BYTES - 200);
        assert!(note.unwrap().contains("byte cap"));
        assert!(found.lines.iter().all(|line| line.changed));
    }

    #[test]
    fn a_patch_stops_at_the_patch_cap_and_says_so() {
        let repo = Repo::new();
        repo.write("a.txt", "a\n");
        let base = repo.commit("start");
        let line = format!("{}\n", "y".repeat(99));
        repo.write("big.txt", &line.repeat(MAX_PATCH_BYTES / 100 + 500));
        let head = repo.commit("big");

        let found = patch(
            &repo.tool(),
            &repo.root(),
            &Span::Between { base, head },
            "big.txt",
        )
        .unwrap();

        assert!(found.truncated);
        assert_eq!(found.text.len(), MAX_PATCH_BYTES);
        assert!(found.bytes > MAX_PATCH_BYTES as u64);
    }

    #[test]
    fn a_deleted_file_has_a_patch_and_a_whole_file_with_no_lines_and_a_note() {
        let repo = Repo::new();
        repo.write("a.txt", "a\n");
        repo.write("b.txt", "b\n");
        let base = repo.commit("start");
        fs::remove_file(repo.root().join("b.txt")).unwrap();
        let head = repo.commit("remove");
        let span = Span::Between { base, head };

        let patch = patch(&repo.tool(), &repo.root(), &span, "b.txt").unwrap();
        let (found, note) = whole(&repo.tool(), &repo.root(), &span, "b.txt").unwrap();

        assert!(patch.text.contains("deleted file mode"), "{}", patch.text);
        assert!(found.lines.is_empty() && !found.binary);
        assert_eq!(note.as_deref(), Some("the changes delete this file"));
    }

    #[test]
    fn a_file_the_changes_did_not_touch_has_no_patch() {
        let repo = Repo::new();
        repo.write("a.txt", "a\n");
        let base = repo.commit("start");
        repo.write("b.txt", "b\n");
        let head = repo.commit("work");

        let error = patch(
            &repo.tool(),
            &repo.root(),
            &Span::Between { base, head },
            "a.txt",
        )
        .unwrap_err();

        assert!(matches!(error, DiffError::NoChange(_)), "{error}");
    }

    #[test]
    fn a_path_that_leaves_the_repository_is_refused() {
        let repo = Repo::new();
        repo.write("a.txt", "a\n");
        let base = repo.commit("start");
        let span = Span::WorkingTree { base };

        for path in ["../outside", "a/../../outside", "/etc/passwd", ""] {
            assert!(
                patch(&repo.tool(), &repo.root(), &span, path).is_err(),
                "{path:?}"
            );
            assert!(
                whole(&repo.tool(), &repo.root(), &span, path).is_err(),
                "{path:?}"
            );
        }
    }

    #[test]
    fn a_new_file_behind_a_link_to_another_folder_is_not_read() {
        let repo = Repo::new();
        repo.write("a.txt", "a\n");
        let base = repo.commit("start");
        let outside = repo.dir.join("outside");
        fs::create_dir_all(&outside).unwrap();
        fs::write(outside.join("secret.txt"), "secret\n").unwrap();
        std::os::unix::fs::symlink(&outside, repo.root().join("linked")).unwrap();
        let span = Span::WorkingTree { base };

        // Git lists the link itself, which is one entry; a path through it names no file git knows.
        let through = patch(&repo.tool(), &repo.root(), &span, "linked/secret.txt");

        assert!(through.is_err());
        assert!(read_inside(&repo.root(), "linked/secret.txt", 100).is_err());
    }

    #[test]
    fn commits_that_name_an_id_are_found_newest_first_and_others_are_not() {
        let repo = Repo::new();
        repo.write("a.txt", "a\n");
        repo.commit("start");
        repo.write("a.txt", "1\n");
        let first = repo.commit("Work on ts-0160-ab");
        repo.write("a.txt", "2\n");
        repo.commit("Work on ts-0160-abc");
        repo.write("a.txt", "3\n");
        repo.commit("Work on ts-0161-zz");
        repo.write("a.txt", "4\n");
        let last = repo.commit("Finish\n\nCloses (ts-0160-ab).");

        let found = naming(&repo.tool(), &repo.root(), "main", "ts-0160-ab").unwrap();

        let ids: Vec<&str> = found.iter().map(|commit| commit.id.as_str()).collect();
        assert_eq!(ids, [last.as_str(), first.as_str()]);
        assert_eq!(found[1].subject, "Work on ts-0160-ab");
        assert!(found[0].date_ms > 0);
    }

    #[test]
    fn at_most_two_hundred_commits_are_kept() {
        let repo = Repo::new();
        repo.write("a.txt", "a\n");
        repo.commit("start");
        for n in 0..MAX_COMMITS + 3 {
            repo.git(&["commit", "-q", "--allow-empty", "-m", &format!("ts-1 {n}")]);
        }

        let found = naming(&repo.tool(), &repo.root(), "main", "ts-1").unwrap();

        assert_eq!(found.len(), MAX_COMMITS);
        assert_eq!(found[0].subject, format!("ts-1 {}", MAX_COMMITS + 2));
    }

    #[test]
    fn an_id_that_looks_like_an_option_is_a_word_for_git_and_not_a_flag() {
        let repo = Repo::new();
        repo.write("a.txt", "a\n");
        repo.commit("start");

        let found = naming(&repo.tool(), &repo.root(), "main", "--all").unwrap();

        assert!(found.is_empty());
    }

    #[test]
    fn the_commits_of_a_span_are_those_after_the_base_up_to_the_head() {
        let repo = Repo::new();
        repo.write("a.txt", "a\n");
        let base = repo.commit("start");
        repo.write("a.txt", "1\n");
        let one = repo.commit("one");
        repo.write("a.txt", "2\n");
        let two = repo.commit("two");

        let held = commits_in(&repo.tool(), &repo.root(), &base, "HEAD").unwrap();

        assert_eq!(held, HashSet::from([one, two]));
    }

    /// A diff driver and a textconv the repository's own config names, each a script that leaves a mark when it runs.
    fn repo_with_drivers() -> (Repo, PathBuf) {
        let repo = Repo::new();
        let mark = repo.dir.join("ran");
        let script = repo.dir.join("driver.sh");
        fs::write(
            &script,
            format!("#!/bin/sh\ntouch '{}'\ncat \"$1\"\n", mark.display()),
        )
        .unwrap();
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
        repo.git(&["config", "diff.evil.command", &script.to_string_lossy()]);
        repo.git(&["config", "diff.evil.textconv", &script.to_string_lossy()]);
        repo.write(".gitattributes", "*.txt diff=evil\n");
        repo.write("a.txt", "one\n");
        (repo, mark)
    }

    #[test]
    fn the_drivers_of_the_test_do_run_for_a_plain_git_diff() {
        let (repo, mark) = repo_with_drivers();
        let base = repo.commit("start");
        repo.write("a.txt", "two\n");
        repo.commit("work");

        repo.git(&["diff", &base, "HEAD"]);

        assert!(
            mark.exists(),
            "the control: git runs the driver when it is let"
        );
    }

    #[test]
    fn a_diff_driver_and_a_textconv_the_repository_names_never_run() {
        let (repo, mark) = repo_with_drivers();
        let base = repo.commit("start");
        repo.write("a.txt", "two\n");
        let head = repo.commit("work");
        repo.write("a.txt", "three\n");
        repo.write("new.txt", "new\n");
        let git = repo.tool();
        let spans = [
            Span::Between {
                base: base.clone(),
                head: head.clone(),
            },
            Span::WorkingTree { base },
            Span::Commit { id: head },
        ];

        for span in &spans {
            let found = list(&git, &repo.root(), span).unwrap();
            assert!(!found.files.is_empty());
            let shown = patch(&git, &repo.root(), span, "a.txt").unwrap();
            assert!(shown.text.contains("@@"), "{}", shown.text);
            let (all, _) = whole(&git, &repo.root(), span, "a.txt").unwrap();
            assert!(!all.lines.is_empty());
        }
        naming(&git, &repo.root(), "main", "work").unwrap();

        assert!(!mark.exists(), "a driver or a textconv ran");
    }

    fn heads(start: &str, end: Option<&str>) -> PhaseHeads {
        PhaseHeads {
            phase: "build".to_string(),
            start_head: start.to_string(),
            end_head: end.map(str::to_string),
        }
    }

    #[test]
    fn a_span_runs_from_the_first_start_to_the_last_end() {
        let all = [heads("a", Some("b")), heads("b", Some("c"))];

        assert_eq!(
            span_of(&all, false),
            Some(Span::Between {
                base: "a".into(),
                head: "c".into()
            })
        );
    }

    #[test]
    fn a_span_with_an_active_phase_ends_at_the_working_tree() {
        let all = [heads("a", Some("b")), heads("b", None)];

        assert_eq!(
            span_of(&all, true),
            Some(Span::WorkingTree { base: "a".into() })
        );
    }

    #[test]
    fn a_run_with_no_heads_or_a_last_phase_that_never_ended_has_no_span() {
        assert_eq!(span_of(&[], true), None);
        assert_eq!(span_of(&[], false), None);
        assert_eq!(span_of(&[heads("a", None)], false), None);
    }

    #[test]
    fn an_id_is_named_when_it_is_a_word_of_its_own() {
        for message in [
            "Fix ts-0160-ab",
            "ts-0160-ab: fix",
            "Fix (ts-0160-ab).",
            "Fix plan:ts-0160-ab\n\nbody",
            "body\n\nRefs ts-0160-ab",
        ] {
            assert!(names_id(message, "ts-0160-ab"), "{message}");
        }
    }

    #[test]
    fn an_id_inside_a_longer_word_or_another_id_is_not_named() {
        for message in [
            "Fix ts-0160-abc",
            "Fix xts-0160-ab",
            "Fix re-ts-0160-ab",
            "Fix ts-0160-ab_more",
            "Fix ts-0161-ab",
            "",
        ] {
            assert!(!names_id(message, "ts-0160-ab"), "{message}");
        }
        assert!(!names_id("anything", ""));
    }

    #[test]
    fn a_later_occurrence_counts_when_the_first_is_inside_a_word() {
        assert!(names_id("ts-0160-abc and ts-0160-ab", "ts-0160-ab"));
    }

    #[test]
    fn a_patchs_added_lines_are_numbered_in_the_new_file() {
        let patch = "diff --git a/f b/f\n--- a/f\n+++ b/f\n@@ -1,3 +1,4 @@\n one\n+two\n three\n-four\n+five\n\
                     @@ -10,0 +12,1 @@\n+twelve\n";

        let mut lines: Vec<u32> = changed_lines(patch).into_iter().collect();
        lines.sort();

        assert_eq!(lines, [2, 4, 12]);
    }

    #[test]
    fn the_words_of_a_worktree_run_name_the_run_and_whether_edits_are_committed() {
        let live = Span::WorkingTree { base: "a".into() };
        let done = Span::Between {
            base: "a".into(),
            head: "b".into(),
        };

        let active = run_words("R-1", ChangesPlace::Worktree, Some(&live), "build", "build");
        let ended = run_words("R-1", ChangesPlace::Worktree, Some(&done), "build", "build");

        assert_eq!(active.place, "Run R-1's worktree");
        assert_eq!(
            active.how,
            "since the build phase began, including edits not committed yet"
        );
        assert_eq!(
            ended.how,
            "from where the build phase began to where it ended"
        );
    }

    #[test]
    fn the_words_of_a_run_in_the_persons_checkout_say_the_persons_edits_are_in_it() {
        let live = Span::WorkingTree { base: "a".into() };

        let words = run_words("R-1", ChangesPlace::Checkout, Some(&live), "build", "build");

        assert_eq!(words.place, "Your checkout");
        assert!(words.how.contains("your own edits"), "{}", words.how);
    }

    #[test]
    fn the_words_of_commits_list_their_short_ids_and_say_when_no_run_built_them() {
        let commit = |id: &str| Commit {
            id: id.to_string(),
            subject: String::new(),
            date_ms: 0,
        };
        let two = [commit("d5c95263333"), commit("a4df9771111")];
        let three = [
            commit("1111111aa"),
            commit("2222222bb"),
            commit("3333333cc"),
        ];

        let words = commit_words("main", "ts-0160", &two, 0);

        assert_eq!(words.place, "Your checkout, line main");
        assert_eq!(
            words.how,
            "2 commits that name ts-0160: d5c9526 and a4df977. No run built it."
        );
        assert_eq!(
            commit_words("main", "ts-0160", &three, 1).how,
            "3 commits that name ts-0160: 1111111, 2222222 and 3333333"
        );
        assert_eq!(
            commit_words("main", "ts-0160", &[], 0).how,
            "no commit on this line names ts-0160"
        );
    }
}
