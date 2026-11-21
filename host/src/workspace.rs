//! The built-in workspace (0.4, workspace implementations; 0.8, Workspace; Stage 3a): `git worktree add` on a new
//! branch, returning `{ path, gitDir }` as core's [`HookWorkspace`], and a removal that refuses a worktree that holds
//! work. A workflow that names no workspace implementation uses this one. Pack workspace hooks are Stage 7.
//!
//! ## States
//!
//! A [`BuiltinWorkspace`] moves through core's workspace states only by [`workspace_transition`], so a move core does
//! not allow is a [`WorkspaceError::Transition`] that names both states and changes nothing:
//!
//! | call | moves | on the way |
//! |---|---|---|
//! | [`BuiltinWorkspace::provision`] | `provisioning → ready`, or `→ failed` when git fails | the floor, then `git worktree add` |
//! | [`BuiltinWorkspace::start_phase`] | `ready` or `idle → in_use` | the floor, before each phase (0.2) |
//! | [`BuiltinWorkspace::end_phase`] | `in_use → idle` | measures the disk it uses (0.13) |
//! | [`BuiltinWorkspace::remove`] | `idle` or `ready → removable → removed` | the two refusals, then `git worktree remove` |
//! | [`BuiltinWorkspace::discard`] | the same moves | no refusal: the folder, git's record of it, the branch |
//!
//! A pause for disk leaves the state as it was: nothing was made or started, and the call is tried again when space
//! returns. A workspace that was made and never used (`ready`) is removed like an idle one: core's table has the move
//! from `ready` to `removable` since ts-0065-kx.
//!
//! ## Removal
//!
//! Removal refuses, and says which, when the worktree has
//! - **uncommitted changes**: a change to a tracked file, or an untracked file that is not ignored. An untracked file
//!   counts, because removing the worktree would delete work that git has no copy of. An ignored file (build output)
//!   does not, because it can be made again; removal deletes it;
//! - **commits not merged into its base**: `git rev-list --count <base>..HEAD` is not zero, and the base does not
//!   already have the branch's change. A branch that was squash merged or rebased onto the base has commits the base
//!   does not contain, so the count alone would refuse it. It counts as merged when a merge of the two is clean and
//!   makes the base's own tree (`git merge-tree --write-tree`): the base already has the change (0.2: a merge that
//!   finds its change at the target is a success). A change that the base has not got, or took out again, is still
//!   refused.
//!
//! A worktree that is dirty and unmerged is called dirty. When removal goes ahead the branch Collide made is deleted
//! with it (`git branch -D` is safe, since every change is in the base).
//!
//! ## Discard
//!
//! [`BuiltinWorkspace::discard`] is the removal of a person who has decided the work goes: it refuses only a workspace
//! a run is using. It deletes the folder with everything in it (committed, uncommitted, untracked and ignored work),
//! has git forget the worktree (`git worktree prune`), and deletes the branch. It deletes the folder itself, not
//! through `git worktree remove`, because that command forgets the worktree before it deletes the folder, and a folder
//! it could not delete (a file that is locked) would then be a worktree git no longer knows, which the same command
//! cannot remove again. Here, the same call after the cause is gone finishes the job. The folder is deleted only for a
//! worktree this workspace made: its git directory is one of the repository's `worktrees/<name>`. The branch's last
//! commit is read first ([`BuiltinWorkspace::branch_head`]), so the caller can record it. A repository whose folder is
//! gone ([`BuiltinWorkspace::repository_gone`]) has no branch and no record to delete, so only the folder goes.
//!
//! The answers come from git's machine-readable output (`status --porcelain=v1 -z`, `rev-list --count`,
//! `rev-parse`), run with `LC_ALL=C`, never from text meant for people.
//!
//! ## Disk space
//!
//! [`disk_use`] is what `end_phase` returns: the bytes the worktree's folder occupies, like `du`. It counts
//! allocated blocks, not file lengths. Everything under the folder counts, ignored files included (a provisioned
//! worktree's dependencies are most of its 4.1 GB, R3). A hard-linked file counts once, a symbolic link counts as
//! the link and is not followed, and the git directory the worktree shares with its repository
//! (`<repo>/.git/worktrees/<name>`, and the object store) is outside the folder and not counted.
//!
//! The daemon records the number with the phase's totals (0.13). It is measured by the daemon after the shim has
//! written `exit.json`, not by the shim, so a large tree never delays that record.

use crate::disk::{Before, DiskError, DiskGuard};
use collide_core::hook::{HookWorkspace, WorkspaceKind};
use collide_core::owner::RemoveRefusal;
use collide_core::state::WorkspaceState;
use collide_core::transition::{TransitionError, WorkspaceVia, workspace_transition};
use std::collections::HashSet;
use std::fmt;
use std::fs;
use std::io::{self, Write};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// What a workspace is made from. The daemon chooses the layout (a repo's worktrees go under one root, 0.4), so `path`
/// is the whole folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceSpec {
    pub kind: WorkspaceKind,
    /// The main checkout, or any checkout of the repository.
    pub repo: PathBuf,
    /// The worktree's folder, which must not exist yet. Missing parents are made.
    pub path: PathBuf,
    /// The new branch the worktree is made on.
    pub branch: String,
    /// The ref the branch starts from, and the one removal checks it is merged into.
    pub base: String,
}

#[derive(Debug)]
pub enum WorkspaceError {
    /// `git` did not run.
    GitMissing(io::Error),
    /// A git command failed. `stderr` is git's own words.
    Git { args: String, stderr: String },
    /// Free space is under the floor, or could not be read.
    Disk(DiskError),
    /// Removal refused, with Collide's words for it.
    Refused {
        reason: RemoveRefusal,
        message: String,
    },
    /// core does not allow the move.
    Transition(TransitionError),
    /// A branch or base that is empty or starts with `-`, so git could read it as an option.
    BadSpec { field: &'static str, value: String },
    /// The folder could not be measured or resolved.
    Io(io::Error),
}

impl fmt::Display for WorkspaceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WorkspaceError::GitMissing(error) => write!(f, "could not run `git`: {error}"),
            WorkspaceError::Git { args, stderr } => write!(f, "`git {args}` failed: {stderr}"),
            WorkspaceError::Disk(error) => write!(f, "{error}"),
            WorkspaceError::Refused { message, .. } => write!(f, "{message}"),
            WorkspaceError::Transition(error) => write!(f, "{error}"),
            WorkspaceError::BadSpec { field, value } => write!(
                f,
                "the workspace's {field} {value:?} is empty or starts with `-`"
            ),
            WorkspaceError::Io(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for WorkspaceError {}

impl From<DiskError> for WorkspaceError {
    fn from(error: DiskError) -> WorkspaceError {
        WorkspaceError::Disk(error)
    }
}

/// How git is run: `LC_ALL=C`, no prompts, and the environment a test or the daemon adds. The prompt run and its diff
/// run git this way too, so one place decides how.
#[derive(Debug, Clone, Default)]
pub(crate) struct Git {
    env: Vec<(String, String)>,
}

impl Git {
    pub(crate) fn with_env(env: Vec<(String, String)>) -> Git {
        Git { env }
    }

    fn command(&self, cwd: &Path, args: &[&str]) -> Command {
        let mut command = Command::new("git");
        command
            .args(args)
            .current_dir(cwd)
            .env("LC_ALL", "C")
            .env("GIT_TERMINAL_PROMPT", "0")
            .envs(self.env.iter().map(|(k, v)| (k, v)));
        command
    }

    pub(crate) fn run(&self, cwd: &Path, args: &[&str]) -> Result<Vec<u8>, WorkspaceError> {
        let output = self
            .command(cwd, args)
            .output()
            .map_err(WorkspaceError::GitMissing)?;
        Git::stdout_of(args, output)
    }

    /// Like [`Git::run`], with `input` on git's standard input. It is written on a thread of its own, so a command that
    /// answers while it is still reading cannot fill its output pipe and stop both.
    pub(crate) fn run_with_input(
        &self,
        cwd: &Path,
        args: &[&str],
        input: Vec<u8>,
    ) -> Result<Vec<u8>, WorkspaceError> {
        let mut child = self
            .command(cwd, args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(WorkspaceError::GitMissing)?;
        let mut stdin = child.stdin.take().expect("stdin was piped");
        let writer = std::thread::spawn(move || {
            let _ = stdin.write_all(&input);
        });
        let output = child
            .wait_with_output()
            .map_err(WorkspaceError::GitMissing)?;
        let _ = writer.join();
        Git::stdout_of(args, output)
    }

    fn stdout_of(args: &[&str], output: std::process::Output) -> Result<Vec<u8>, WorkspaceError> {
        if output.status.success() {
            Ok(output.stdout)
        } else {
            // Some commands say why on standard output (a merge's conflicts), so it is the fallback.
            let said = if output.stderr.iter().all(u8::is_ascii_whitespace) {
                &output.stdout
            } else {
                &output.stderr
            };
            Err(WorkspaceError::Git {
                args: args.join(" "),
                stderr: String::from_utf8_lossy(said).trim().to_string(),
            })
        }
    }

    pub(crate) fn text(&self, cwd: &Path, args: &[&str]) -> Result<String, WorkspaceError> {
        let bytes = self.run(cwd, args)?;
        Ok(String::from_utf8_lossy(&bytes).trim().to_string())
    }
}

/// What `git merge-tree --write-tree` found for a merge of `theirs` into `ours`, without touching any checkout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum MergeCheck {
    /// The merge is clean, and its tree is `ours`' own: `ours` already has the change of `theirs`. It covers a branch
    /// merged, squash merged or rebased, and one that changed nothing.
    InBase,
    /// The merge is clean and makes a new tree.
    Clean,
    /// The merge conflicts in these files, sorted.
    Conflict { files: Vec<String> },
}

impl Git {
    /// Checks a merge of `theirs` into `ours` (both refs, or commits) with `git merge-tree --write-tree`. It writes
    /// objects and changes no ref, no index and no file.
    pub(crate) fn merge_check(
        &self,
        cwd: &Path,
        ours: &str,
        theirs: &str,
    ) -> Result<MergeCheck, WorkspaceError> {
        let args = [
            "merge-tree",
            "--write-tree",
            "--name-only",
            "-z",
            ours,
            theirs,
        ];
        let output = self
            .command(cwd, &args)
            .output()
            .map_err(WorkspaceError::GitMissing)?;
        // The output is the merged tree, then the conflicted files, each ended by a NUL, then an empty field.
        let mut fields = output.stdout.split(|byte| *byte == 0);
        let tree = String::from_utf8_lossy(fields.next().unwrap_or_default()).into_owned();
        match output.status.code() {
            Some(0) => {
                let ours_tree =
                    self.text(cwd, &["rev-parse", "--verify", &format!("{ours}^{{tree}}")])?;
                Ok(if tree == ours_tree {
                    MergeCheck::InBase
                } else {
                    MergeCheck::Clean
                })
            }
            Some(1) => {
                let mut files: Vec<String> = fields
                    .take_while(|field| !field.is_empty())
                    .map(|field| String::from_utf8_lossy(field).into_owned())
                    .collect();
                files.sort();
                files.dedup();
                Ok(MergeCheck::Conflict { files })
            }
            _ => Err(Git::stdout_of(&args, output).expect_err("a failed command is an error")),
        }
    }
}

/// A git worktree Collide made for a run, and its state (0.8).
#[derive(Debug)]
pub struct BuiltinWorkspace {
    spec: WorkspaceSpec,
    git_dir: Option<PathBuf>,
    state: WorkspaceState,
    git: Git,
}

impl BuiltinWorkspace {
    /// A workspace that is `provisioning`: nothing is made until [`BuiltinWorkspace::provision`].
    pub fn new(spec: WorkspaceSpec) -> BuiltinWorkspace {
        BuiltinWorkspace {
            spec,
            git_dir: None,
            state: WorkspaceState::Provisioning,
            git: Git::default(),
        }
    }

    /// A workspace as an earlier daemon left it, read back from what that daemon stored: `state` is the state it was
    /// in, and `git_dir` is the worktree's git directory when it was made. Nothing is checked against the disk here.
    pub fn restored(
        spec: WorkspaceSpec,
        git_dir: Option<PathBuf>,
        state: WorkspaceState,
    ) -> BuiltinWorkspace {
        BuiltinWorkspace {
            spec,
            git_dir,
            state,
            git: Git::default(),
        }
    }

    /// Adds to the environment of every git command this workspace runs. A test passes
    /// `GIT_CONFIG_GLOBAL=/dev/null` here so the user's own git configuration is never read.
    pub fn with_git_env(mut self, env: Vec<(String, String)>) -> BuiltinWorkspace {
        self.git.env = env;
        self
    }

    pub fn state(&self) -> WorkspaceState {
        self.state
    }

    /// Changes the ref that removal checks the branch is merged into. A workspace made from a commit is removed against
    /// the branch that commit was on: after accept the commits are in that branch, and not ahead of the commit.
    pub fn set_base(&mut self, base: &str) {
        self.spec.base = base.to_string();
    }

    /// `{ path, gitDir }`, once the worktree is made.
    pub fn location(&self) -> Option<HookWorkspace> {
        let git_dir = self.git_dir.as_ref()?;
        Some(HookWorkspace {
            kind: self.spec.kind,
            path: self.spec.path.to_string_lossy().into_owned(),
            git_dir: git_dir.to_string_lossy().into_owned(),
        })
    }

    fn check_move(&self, to: WorkspaceState) -> Result<(), WorkspaceError> {
        workspace_transition(self.state, to, WorkspaceVia::Engine)
            .map_err(WorkspaceError::Transition)
    }

    fn move_to(&mut self, to: WorkspaceState) -> Result<(), WorkspaceError> {
        self.check_move(to)?;
        self.state = to;
        Ok(())
    }

    /// Makes the worktree: checks the floor, then `git worktree add -b <branch> <path> <base>`. `provisioning →
    /// ready`, or `→ failed` when git fails. Under the floor nothing is made and the state stays `provisioning`.
    pub fn provision(&mut self, disk: &DiskGuard) -> Result<HookWorkspace, WorkspaceError> {
        self.check_move(WorkspaceState::Ready)?;
        for (field, value) in [("branch", &self.spec.branch), ("base", &self.spec.base)] {
            if value.is_empty() || value.starts_with('-') {
                return Err(WorkspaceError::BadSpec {
                    field,
                    value: value.clone(),
                });
            }
        }
        disk.check(&self.spec.path, Before::Workspace)?;
        match self.add_worktree() {
            Ok(()) => {
                self.move_to(WorkspaceState::Ready)?;
                Ok(self.location().expect("the worktree was just made"))
            }
            Err(error) => {
                self.move_to(WorkspaceState::Failed)?;
                Err(error)
            }
        }
    }

    fn add_worktree(&mut self) -> Result<(), WorkspaceError> {
        let path = self.spec.path.to_string_lossy().into_owned();
        self.git.run(
            &self.spec.repo,
            &[
                "worktree",
                "add",
                "-b",
                &self.spec.branch,
                "--",
                &path,
                &self.spec.base,
            ],
        )?;
        // Git reports real paths, and the temp folder may sit behind a symbolic link.
        self.spec.path = fs::canonicalize(&self.spec.path).map_err(WorkspaceError::Io)?;
        let git_dir = self
            .git
            .text(&self.spec.path, &["rev-parse", "--absolute-git-dir"])?;
        self.git_dir = Some(PathBuf::from(git_dir));
        Ok(())
    }

    /// Before each phase (0.2): checks the floor, then `ready` or `idle → in_use`. Under the floor the phase does not
    /// start and the state is unchanged.
    pub fn start_phase(&mut self, disk: &DiskGuard) -> Result<(), WorkspaceError> {
        self.check_move(WorkspaceState::InUse)?;
        disk.check(&self.spec.path, Before::Phase)?;
        self.move_to(WorkspaceState::InUse)
    }

    /// At phase end: `in_use → idle`, and the disk the workspace uses, in bytes (0.13, see [`disk_use`]). The state
    /// moves first, because the phase is over whether or not the folder can be measured.
    pub fn end_phase(&mut self) -> Result<u64, WorkspaceError> {
        self.move_to(WorkspaceState::Idle)?;
        disk_use(&self.spec.path).map_err(WorkspaceError::Io)
    }

    /// Removes the worktree and its branch, unless it holds work (see the module notes). `idle → removable →
    /// removed`; a `ready` workspace goes the same way. A refusal, or a git failure, leaves the worktree as it was.
    pub fn remove(&mut self) -> Result<(), WorkspaceError> {
        match self.state {
            WorkspaceState::InUse => {
                return Err(WorkspaceError::Refused {
                    reason: RemoveRefusal::InUse,
                    message: format!("{} is in use by a run", self.spec.path.display()),
                });
            }
            WorkspaceState::Idle | WorkspaceState::Ready => {
                self.check_move(WorkspaceState::Removable)?
            }
            _ => self.check_move(WorkspaceState::Removed)?,
        }
        self.refuse_work()?;
        if matches!(self.state, WorkspaceState::Idle | WorkspaceState::Ready) {
            self.move_to(WorkspaceState::Removable)?;
        }
        let path = self.spec.path.to_string_lossy().into_owned();
        self.git
            .run(&self.spec.repo, &["worktree", "remove", "--", &path])?;
        self.git
            .run(&self.spec.repo, &["branch", "-D", "--", &self.spec.branch])?;
        self.move_to(WorkspaceState::Removed)
    }

    /// Deletes the worktree and its branch whatever the worktree holds (see the module notes). `idle → removable →
    /// removed`, as [`BuiltinWorkspace::remove`]. A workspace a run is using is refused. A failure leaves the state at
    /// `removable`, and the same call is made again once its cause is gone.
    pub fn discard(&mut self) -> Result<(), WorkspaceError> {
        match self.state {
            WorkspaceState::InUse => {
                return Err(WorkspaceError::Refused {
                    reason: RemoveRefusal::InUse,
                    message: format!("{} is in use by a run", self.spec.path.display()),
                });
            }
            WorkspaceState::Idle | WorkspaceState::Ready => {
                self.check_move(WorkspaceState::Removable)?;
                self.move_to(WorkspaceState::Removable)?;
            }
            _ => self.check_move(WorkspaceState::Removed)?,
        }
        self.delete_folder()?;
        // A repository that is gone took its branch and its record of the worktree with it: nothing else to delete.
        if self.repository_gone() {
            return self.move_to(WorkspaceState::Removed);
        }
        // Git forgets the worktree first: it does not delete a branch that a worktree has checked out.
        self.git.run(&self.spec.repo, &["worktree", "prune"])?;
        if self.branch_head()?.is_some() {
            self.git
                .run(&self.spec.repo, &["branch", "-D", "--", &self.spec.branch])?;
        }
        self.move_to(WorkspaceState::Removed)
    }

    /// Whether the repository's folder is gone, which takes the branch and git's record of the worktree with it. Only
    /// a missing folder counts: a folder that is there and that git cannot read is git's to explain.
    pub fn repository_gone(&self) -> bool {
        !self.spec.repo.is_dir()
    }

    /// The commit the workspace's branch points at, or `None` when the branch is not there (never made, or deleted).
    pub fn branch_head(&self) -> Result<Option<String>, WorkspaceError> {
        let pattern = format!("refs/heads/{}", self.spec.branch);
        let found = self.git.text(
            &self.spec.repo,
            &["for-each-ref", "--format=%(objectname)", &pattern],
        )?;
        Ok(found.lines().next().map(str::to_string))
    }

    /// Deletes the worktree's folder, which is already gone for a worktree someone deleted by hand. Only a worktree
    /// this workspace made is deleted: its git directory is `<repository>/.git/worktrees/<name>`, so a path that is a
    /// checkout of its own is never removed.
    fn delete_folder(&self) -> Result<(), WorkspaceError> {
        let linked = self
            .git_dir
            .as_ref()
            .and_then(|dir| dir.parent())
            .and_then(Path::file_name)
            .is_some_and(|name| name == "worktrees");
        if !linked {
            return Err(WorkspaceError::Io(io::Error::other(format!(
                "{} is not a worktree this workspace made, so it is not deleted",
                self.spec.path.display()
            ))));
        }
        match fs::remove_dir_all(&self.spec.path) {
            Err(error) if error.kind() != io::ErrorKind::NotFound => {
                Err(WorkspaceError::Io(io::Error::new(
                    error.kind(),
                    format!("{} was not deleted: {error}", self.spec.path.display()),
                )))
            }
            _ => Ok(()),
        }
    }

    fn refuse_work(&self) -> Result<(), WorkspaceError> {
        self.refuse_dirty()?;
        self.refuse_unmerged()
    }

    /// Refuses a worktree that has uncommitted changes or untracked files that git does not ignore.
    pub fn refuse_dirty(&self) -> Result<(), WorkspaceError> {
        let path = &self.spec.path;
        let status = self.git.run(
            path,
            &["status", "--porcelain=v1", "-z", "--untracked-files=all"],
        )?;
        let entries: Vec<&[u8]> = status
            .split(|byte| *byte == 0)
            .filter(|entry| !entry.is_empty())
            .collect();
        let untracked = entries.iter().any(|entry| entry.starts_with(b"?? "));
        let tracked = entries.iter().any(|entry| !entry.starts_with(b"?? "));
        if tracked || untracked {
            let what = match (tracked, untracked) {
                (true, true) => "uncommitted changes and untracked files",
                (true, false) => "uncommitted changes",
                _ => "untracked files",
            };
            return Err(WorkspaceError::Refused {
                reason: RemoveRefusal::Dirty,
                message: format!(
                    "{} has {what}; commit or discard them first",
                    path.display()
                ),
            });
        }
        Ok(())
    }

    fn refuse_unmerged(&self) -> Result<(), WorkspaceError> {
        let path = &self.spec.path;
        let range = format!("{}..HEAD", self.spec.base);
        let count = self
            .git
            .text(path, &["rev-list", "--count", &range])?
            .parse::<u64>()
            .map_err(|error| WorkspaceError::Git {
                args: format!("rev-list --count {range}"),
                stderr: format!("the count is not a number: {error}"),
            })?;
        if count > 0 && !self.change_is_in_base()? {
            let commits = if count == 1 { "commit" } else { "commits" };
            return Err(WorkspaceError::Refused {
                reason: RemoveRefusal::Unmerged,
                message: format!(
                    "{} has {count} {commits} not merged into {}; merge or discard them first",
                    path.display(),
                    self.spec.base
                ),
            });
        }
        Ok(())
    }
}

impl BuiltinWorkspace {
    /// Whether the base already has the change of the worktree's `HEAD`, though `HEAD` has commits it does not
    /// contain: a squash or a rebase (see the module notes).
    fn change_is_in_base(&self) -> Result<bool, WorkspaceError> {
        let check = self
            .git
            .merge_check(&self.spec.path, &self.spec.base, "HEAD")?;
        Ok(check == MergeCheck::InBase)
    }
}

/// The bytes a folder occupies on disk (see the module notes): allocated blocks of everything under it, a hard link
/// once, a symbolic link as itself.
pub fn disk_use(root: &Path) -> io::Result<u64> {
    let named = |path: &Path, error: io::Error| {
        io::Error::new(error.kind(), format!("{}: {error}", path.display()))
    };
    let mut total = 0u64;
    let mut seen: HashSet<(u64, u64)> = HashSet::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(path) = pending.pop() {
        let meta = fs::symlink_metadata(&path).map_err(|e| named(&path, e))?;
        if meta.nlink() > 1 && !seen.insert((meta.dev(), meta.ino())) {
            continue;
        }
        total = total.saturating_add(meta.blocks().saturating_mul(512));
        if meta.is_dir() {
            for entry in fs::read_dir(&path).map_err(|e| named(&path, e))? {
                pending.push(entry.map_err(|e| named(&path, e))?.path());
            }
        }
    }
    Ok(total)
}
