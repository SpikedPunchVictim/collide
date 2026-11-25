//! Trust: the hash, `trust.yaml` and the diff (ts-0097-kw; 0.9, Trust).
//!
//! What a person has trusted is kept outside the repo, in `~/.config/collide/trust.yaml` ([`TrustStore`]). A line's
//! trust hash ([`Trust::line_hash`]) is git's own hash of a text that lists, sorted, each thing the line read from
//! the repo and its git id: [`LoadedLine::trust_inputs`], and the tree of each protected folder at the line's commit.
//! So the hash is of the committed snapshot and never of the working tree, and it changes only when a commit
//! changes something it covers. When it changes the person is asked again, and [`Trust::diff`] says what changed.
//!
//! This module has no prompt and runs no repo code. The daemon and the CLI that ask (`ts-0102-vu`) call it, and the
//! stages that run repo code call [`Trust::require_trusted`].

use crate::atomic_write::{Step, write_atomically};
use crate::board_load::LoadedLine;
use crate::git_fetch::mirror_path;
use crate::git_objects::{GitError, GitObjects, ObjectKind, TreeChangeKind, check_id};
use collide_core::lock::GitLocator;
use collide_core::owner::TrustState;
use collide_core::trust::{
    ChangeKind, DiffDetail, FileChangeKind, FileDiff, InputChange, InputDiff, InputError, LineDiff,
    LineTrust, TrustFile, TrustInput, canonical_text, diff_inputs, protected_root,
};
use collide_core::yaml::{MAX_YAML_BYTES, YamlError, load_yaml_file};
use collide_core::yaml_out::to_yaml;
use std::collections::BTreeMap;
use std::fmt;
use std::fs::{self, File};
use std::io::{self, Write};
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::path::{Path, PathBuf};

const FILE_MODE: u32 = 0o600;
const DIR_MODE: u32 = 0o700;

/// The comment at the top of `trust.yaml`.
const HEADER: &str = "What you have trusted in Collide. Collide writes this file. To ask again about a board, \
delete its entry.\nThe hashes are git ids; the inputs are what each hash was made from, so a change can be shown.";

/// The file's name in the store's folder.
pub const TRUST_FILE: &str = "trust.yaml";

/// The most changed lines of one file's diff the prompt gets.
pub const MAX_DIFF_LINES: usize = 200;

/// The most files of one changed folder whose lines are read: the rest are listed with a note.
pub const MAX_FILES_WITH_LINES: usize = 20;

/// Why `trust.yaml` could not be read or written. Each names the file.
#[derive(Debug)]
pub enum StoreError {
    /// The file is there and cannot be used: unreadable, over the YAML cap, malformed, a duplicate or unknown key.
    Invalid(YamlError),
    /// What would be written is over the YAML cap, so it could not be read back. Nothing was written.
    TooLarge { file: PathBuf, bytes: usize },
    Write {
        file: PathBuf,
        what: &'static str,
        source: io::Error,
    },
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StoreError::Invalid(error) => write!(f, "{error}"),
            StoreError::TooLarge { file, bytes } => write!(
                f,
                "{}: the trust file would be {bytes} bytes, over the cap, so it was not written",
                file.display()
            ),
            StoreError::Write { file, what, source } => {
                write!(f, "{}: cannot {what}: {source}", file.display())
            }
        }
    }
}

impl std::error::Error for StoreError {}

/// Why a trust question has no answer, or a trust request was refused.
#[derive(Debug)]
pub enum TrustError {
    Git(GitError),
    Inputs(InputError),
    Store(StoreError),
    /// `trust` was given a hash that is not the line's current one: the person saw something else.
    Stale {
        given: String,
        current: String,
    },
    /// The line is not trusted at its current hash, which is the one to ask about.
    Untrusted {
        hash: String,
    },
    /// A `dev:` folder inside a Collide workspace, where an agent could edit its own pack.
    DevInWorkspace {
        folder: PathBuf,
        workspace: PathBuf,
    },
    /// A `dev:` folder that is not a usable folder.
    DevFolder {
        folder: PathBuf,
        reason: String,
    },
}

impl fmt::Display for TrustError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TrustError::Git(error) => write!(f, "trust: {error}"),
            TrustError::Inputs(error) => write!(f, "trust: {error}"),
            TrustError::Store(error) => write!(f, "{error}"),
            TrustError::Stale { given, current } => write!(
                f,
                "the line's trust hash is {current}, not {given}: the repo changed after it was shown, so look at \
                 what changed before trusting"
            ),
            TrustError::Untrusted { hash } => {
                write!(f, "this line is not trusted at its current hash {hash}")
            }
            TrustError::DevInWorkspace { folder, workspace } => write!(
                f,
                "the `dev:` folder {} is inside the Collide workspace {}, where an agent could edit its own pack, \
                 so it is refused",
                folder.display(),
                workspace.display()
            ),
            TrustError::DevFolder { folder, reason } => {
                write!(f, "the `dev:` folder {}: {reason}", folder.display())
            }
        }
    }
}

impl std::error::Error for TrustError {}

impl From<GitError> for TrustError {
    fn from(error: GitError) -> TrustError {
        TrustError::Git(error)
    }
}

impl From<InputError> for TrustError {
    fn from(error: InputError) -> TrustError {
        TrustError::Inputs(error)
    }
}

impl From<StoreError> for TrustError {
    fn from(error: StoreError) -> TrustError {
        TrustError::Store(error)
    }
}

/// The folder that holds `trust.yaml`. It is a parameter, so a test never touches the real one.
#[derive(Debug, Clone)]
pub struct TrustStore {
    dir: PathBuf,
}

impl TrustStore {
    pub fn new(dir: impl Into<PathBuf>) -> TrustStore {
        TrustStore { dir: dir.into() }
    }

    pub fn file(&self) -> PathBuf {
        self.dir.join(TRUST_FILE)
    }

    /// The file's content, or an empty one when there is no file.
    pub fn load(&self) -> Result<TrustFile, StoreError> {
        match load_yaml_file::<TrustFile>(&self.file()) {
            Ok(file) => Ok(file),
            Err(YamlError::Read { source, .. }) if source.kind() == io::ErrorKind::NotFound => {
                Ok(TrustFile::default())
            }
            Err(error) => Err(StoreError::Invalid(error)),
        }
    }

    /// Writes the file: a temp file beside it, mode `0600`, then a rename, in a folder of mode `0700`.
    pub fn save(&self, file: &TrustFile) -> Result<(), StoreError> {
        self.save_through(file, |file| file)
    }

    /// [`TrustStore::save`] with the seam a test uses to make the write fail after the temp file is made: the file
    /// the bytes go to is passed through `wrap`.
    fn save_through<W: Write>(
        &self,
        file: &TrustFile,
        wrap: impl FnOnce(File) -> W,
    ) -> Result<(), StoreError> {
        let target = self.file();
        let failed = |what: &'static str, source: io::Error| StoreError::Write {
            file: target.clone(),
            what,
            source,
        };
        let text =
            to_yaml(file, HEADER).map_err(|error| failed("write", io::Error::other(error)))?;
        // A file over the cap could not be read back, and every line would then be untrusted for good.
        if text.len() > MAX_YAML_BYTES {
            return Err(StoreError::TooLarge {
                file: target,
                bytes: text.len(),
            });
        }
        // The folder is Collide's own: it is made `0700`, and one that was made open is closed.
        fs::DirBuilder::new()
            .recursive(true)
            .mode(DIR_MODE)
            .create(&self.dir)
            .and_then(|()| fs::set_permissions(&self.dir, fs::Permissions::from_mode(DIR_MODE)))
            .map_err(|source| failed("prepare its folder", source))?;

        write_atomically(&self.dir, TRUST_FILE, text.as_bytes(), FILE_MODE, wrap).map_err(|error| {
            StoreError::Write {
                file: target.clone(),
                what: match error.step {
                    Step::CreateTemp => "create a temp file",
                    Step::WriteTemp => "write the temp file",
                    Step::Rename => "replace the file",
                    Step::SyncDir => "sync its folder",
                },
                source: error.source,
            }
        })
    }
}

/// Which line of which board a question is about: the git directory the line is read from, the board's root path
/// (as the store keys it) and the line's ref.
#[derive(Debug, Clone, Copy)]
pub struct LineSite<'a> {
    pub repo: &'a Path,
    pub root: &'a str,
    pub ref_name: &'a str,
}

/// A line's trust hash and the inputs it is of.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineHash {
    pub hash: String,
    pub inputs: Vec<TrustInput>,
}

pub struct Trust {
    store: TrustStore,
    git: GitObjects,
}

impl Trust {
    pub fn new(store: TrustStore) -> Trust {
        Trust {
            store,
            git: GitObjects::new(),
        }
    }

    pub fn with_git(mut self, git: GitObjects) -> Trust {
        self.git = git;
        self
    }

    pub fn store(&self) -> &TrustStore {
        &self.store
    }

    /// The trust hash of `line`, read from `repo`, and the inputs it is of.
    pub fn line_hash(&self, repo: &Path, line: &LoadedLine) -> Result<LineHash, TrustError> {
        let mut inputs: Vec<TrustInput> = line
            .trust_inputs()
            .into_iter()
            .map(|(name, id)| TrustInput::new(name, id))
            .collect();
        inputs.extend(self.protected_inputs(repo, line)?);
        inputs.sort();
        inputs.dedup();
        let text = canonical_text(&inputs)?;
        let hash = digest(&self.git, repo, &text)?;
        Ok(LineHash { hash, inputs })
    }

    /// The tree (or file) id at the line's commit of the folder each loaded pack's `protectedPaths` glob starts in.
    /// A glob's folder that is not in the commit is not an input: its appearing adds one.
    fn protected_inputs(
        &self,
        repo: &Path,
        line: &LoadedLine,
    ) -> Result<Vec<TrustInput>, TrustError> {
        let mut found: BTreeMap<String, String> = BTreeMap::new();
        let globs = line
            .packs
            .iter()
            .filter_map(|pack| pack.manifest.protected_paths.as_ref())
            .flat_map(|protected| protected.paths());
        for glob in globs {
            let root = protected_root(glob);
            if root.is_empty() {
                let tree = self.git.tree_at(repo, &line.commit, "")?;
                found.insert("protected .".to_string(), tree.id);
            } else if let Some(entry) = self.git.entry_at(repo, &line.commit, &root)? {
                found.insert(format!("protected {root}"), entry.id);
            }
        }
        Ok(found
            .into_iter()
            .map(|(name, id)| TrustInput::new(name, id))
            .collect())
    }

    /// `Trusted` when the line's hash is the one stored for its board and ref, else `Untrusted` with the current
    /// hash. A `trust.yaml` that cannot be read is never read as trusted: the state is `Untrusted`, and
    /// [`Trust::store_problem`] says why.
    pub fn trust_state(
        &self,
        site: &LineSite,
        line: &LoadedLine,
    ) -> Result<TrustState, TrustError> {
        let current = self.line_hash(site.repo, line)?;
        let trusted = self
            .store
            .load()
            .is_ok_and(|file| is_trusted(&file, site, &current.hash));
        Ok(if trusted {
            TrustState::Trusted { hash: current.hash }
        } else {
            TrustState::Untrusted { hash: current.hash }
        })
    }

    /// What is wrong with `trust.yaml`, naming the file, or `None` when it reads (or is not there).
    pub fn store_problem(&self) -> Option<StoreError> {
        self.store.load().err()
    }

    /// The hash to run repo code under: `Ok` only when the line is trusted at its current hash. When it is not, the
    /// error holds the hash to ask about. A `trust.yaml` that cannot be read is an error that names it.
    pub fn require_trusted(
        &self,
        site: &LineSite,
        line: &LoadedLine,
    ) -> Result<String, TrustError> {
        let current = self.line_hash(site.repo, line)?;
        let file = self.store.load()?;
        if is_trusted(&file, site, &current.hash) {
            Ok(current.hash)
        } else {
            Err(TrustError::Untrusted { hash: current.hash })
        }
    }

    /// Trusts `line` at `hash`, which must be the hash the line has now: it is computed again, and a different one is
    /// [`TrustError::Stale`] with nothing stored. The inputs it was made from are stored with it. A `trust.yaml` that
    /// cannot be read is an error that names it, and the file is left as it is.
    pub fn trust(
        &self,
        site: &LineSite,
        line: &LoadedLine,
        hash: &str,
    ) -> Result<TrustState, TrustError> {
        let current = self.line_hash(site.repo, line)?;
        if current.hash != hash {
            return Err(TrustError::Stale {
                given: hash.to_string(),
                current: current.hash,
            });
        }
        let mut file = self.store.load()?;
        file.set_line(
            site.root,
            site.ref_name,
            LineTrust {
                hash: current.hash.clone(),
                inputs: current.inputs,
            },
        );
        self.store.save(&file)?;
        Ok(TrustState::Trusted { hash: current.hash })
    }

    /// What changed in `line` since the person trusted it: everything is added when nothing was trusted.
    pub fn changes_since_trust(
        &self,
        site: &LineSite,
        line: &LoadedLine,
    ) -> Result<Vec<InputDiff>, TrustError> {
        let current = self.line_hash(site.repo, line)?;
        let file = self.store.load()?;
        let before = file
            .line(site.root, site.ref_name)
            .map(|trusted| trusted.inputs.clone())
            .unwrap_or_default();
        Ok(self.diff(site.repo, &before, &current.inputs))
    }

    /// As [`Trust::changes_since_trust`], for a line with `git:` packs: the trees of such a pack are in its mirror
    /// under `cache` ([`mirror_path`]), not in the line's repository, so the diff of that input is read there.
    pub fn changes_since_trust_with_mirrors(
        &self,
        site: &LineSite,
        line: &LoadedLine,
        cache: &Path,
    ) -> Result<Vec<InputDiff>, TrustError> {
        let current = self.line_hash(site.repo, line)?;
        let file = self.store.load()?;
        let before = file
            .line(site.root, site.ref_name)
            .map(|trusted| trusted.inputs.clone())
            .unwrap_or_default();
        let repo_of = |input: &TrustInput| -> PathBuf {
            let url = input
                .name
                .strip_prefix("pack git:")
                .and_then(|locator| GitLocator::parse(locator).ok())
                .map(|locator| locator.url);
            match url {
                Some(url) => mirror_path(cache, &url),
                None => site.repo.to_path_buf(),
            }
        };
        // One diff for each repository the inputs live in, then every change in name order.
        let mut by_repo: BTreeMap<PathBuf, (Vec<TrustInput>, Vec<TrustInput>)> = BTreeMap::new();
        for input in before {
            by_repo.entry(repo_of(&input)).or_default().0.push(input);
        }
        for input in current.inputs {
            by_repo.entry(repo_of(&input)).or_default().1.push(input);
        }
        let mut changes: Vec<InputDiff> = by_repo
            .into_iter()
            .flat_map(|(repo, (old, new))| self.diff(&repo, &old, &new))
            .collect();
        changes.sort_by(|a, b| a.change.name.cmp(&b.change.name));
        Ok(changes)
    }

    /// The difference of two input lists, in name order, with what each changed input shows (the files of a folder,
    /// the lines of a file), read from `repo`. An input whose old objects `repo` no longer has is listed as changed,
    /// and says so.
    pub fn diff(&self, repo: &Path, old: &[TrustInput], new: &[TrustInput]) -> Vec<InputDiff> {
        diff_inputs(old, new)
            .into_iter()
            .map(|change| {
                let detail = self.detail(repo, &change);
                InputDiff { change, detail }
            })
            .collect()
    }

    fn detail(&self, repo: &Path, change: &InputChange) -> DiffDetail {
        match (change.kind, &change.old, &change.new) {
            (ChangeKind::Changed, Some(old), Some(new)) => self.changed(repo, old, new),
            (ChangeKind::Added, _, Some(new)) => self.added(repo, new),
            _ => DiffDetail::None,
        }
    }

    /// What kind of object `id` is, or the sentence that says why it cannot be told.
    fn kind_of(&self, repo: &Path, id: &str, which: &str) -> Result<ObjectKind, String> {
        match self.git.object_kind(repo, id) {
            Ok(Some(kind)) => Ok(kind),
            Ok(None) => Err(format!(
                "the {which} version ({id}) is not in this repository (a pruned clone?), so there is no diff to show"
            )),
            Err(error) => Err(format!("the {which} version cannot be read: {error}")),
        }
    }

    fn changed(&self, repo: &Path, old: &str, new: &str) -> DiffDetail {
        let kinds = self
            .kind_of(repo, old, "old")
            .and_then(|old_kind| Ok((old_kind, self.kind_of(repo, new, "new")?)));
        match kinds {
            Err(why) => DiffDetail::Unavailable(why),
            Ok((ObjectKind::Tree, ObjectKind::Tree)) => self.folder(repo, old, new),
            Ok((ObjectKind::Blob, ObjectKind::Blob)) => match self.file_lines(repo, old, new) {
                Ok(lines) => DiffDetail::Lines(lines),
                Err(why) => DiffDetail::Unavailable(why),
            },
            // A folder that became a file, or the other way round: the names say it.
            Ok(_) => DiffDetail::None,
        }
    }

    /// A file that is new: all of it is changed lines. A new folder has no detail here: its review is the pack's.
    fn added(&self, repo: &Path, new: &str) -> DiffDetail {
        match self.git.object_kind(repo, new) {
            Ok(Some(ObjectKind::Blob)) => {}
            _ => return DiffDetail::None,
        }
        match self.all_added(repo, new) {
            Ok(lines) => DiffDetail::Lines(lines),
            Err(why) => DiffDetail::Unavailable(why),
        }
    }

    /// A file that is new: every line of it is a changed line. The empty file is not an object `git diff` can be
    /// given, so the lines are read from the blob and written as the diff from nothing.
    fn all_added(&self, repo: &Path, id: &str) -> Result<LineDiff, String> {
        let mut bodies = self
            .git
            .read_blobs(repo, &[id.to_string()])
            .map_err(|error| format!("the new file cannot be read: {error}"))?;
        let body = bodies.pop().unwrap_or_default();
        if body.contains(&0) {
            return Ok(LineDiff {
                binary: true,
                ..LineDiff::default()
            });
        }
        let text = String::from_utf8_lossy(&body);
        let mut raw = format!("@@ -0,0 +1,{} @@\n", text.lines().count());
        for line in text.lines() {
            raw.push('+');
            raw.push_str(line);
            raw.push('\n');
        }
        Ok(shape_diff(&raw))
    }

    fn file_lines(&self, repo: &Path, old: &str, new: &str) -> Result<LineDiff, String> {
        self.git
            .diff_blobs(repo, old, new)
            .map(|raw| shape_diff(&raw))
            .map_err(|error| format!("the diff cannot be read: {error}"))
    }

    /// The files that differ between two trees. The first [`MAX_FILES_WITH_LINES`] text files that were added or
    /// changed carry their changed lines.
    fn folder(&self, repo: &Path, old: &str, new: &str) -> DiffDetail {
        let changes = match self.git.diff_tree(repo, old, new) {
            Ok(changes) => changes,
            Err(error) => {
                return DiffDetail::Unavailable(format!(
                    "the changed files cannot be listed: {error}"
                ));
            }
        };
        let is_text = |mode: &str| mode == "100644" || mode == "100755";
        let mut with_lines = 0;
        let mut files = Vec::new();
        for change in changes {
            let kind = match change.kind {
                TreeChangeKind::Added => FileChangeKind::Added,
                TreeChangeKind::Deleted => FileChangeKind::Deleted,
                TreeChangeKind::Modified => FileChangeKind::Modified,
                TreeChangeKind::TypeChanged => FileChangeKind::TypeChanged,
            };
            let mut lines = None;
            let mut note = None;
            let readable = matches!(kind, FileChangeKind::Added | FileChangeKind::Modified);
            if kind == FileChangeKind::Deleted {
                // Nothing to show of a file that is gone.
            } else if change.new_mode == "120000" {
                note = Some("is a symlink".to_string());
            } else if change.new_mode == "160000" {
                note = Some("is a submodule".to_string());
            } else if !readable || !is_text(&change.new_mode) {
                note = Some(format!(
                    "changed type (mode {} to {})",
                    change.old_mode, change.new_mode
                ));
            } else if change.old_id == change.new_id {
                note = Some(format!(
                    "only the file mode changed (from {} to {})",
                    change.old_mode, change.new_mode
                ));
            } else if with_lines >= MAX_FILES_WITH_LINES {
                note = Some(format!(
                    "lines not shown: only the first {MAX_FILES_WITH_LINES} changed files show them"
                ));
            } else if let Some(new_id) = &change.new_id {
                with_lines += 1;
                let read = match &change.old_id {
                    Some(old_id) => self.file_lines(repo, old_id, new_id),
                    None => self.all_added(repo, new_id),
                };
                match read {
                    Ok(read) => lines = Some(read),
                    Err(why) => note = Some(why),
                }
            }
            files.push(FileDiff {
                kind,
                path: change.path,
                lines,
                note,
            });
        }
        DiffDetail::Files(files)
    }

    /// Records that the person trusted the user pack `source` at `tree` (when it was added).
    pub fn trust_pack(&self, source: &str, tree: &str) -> Result<(), TrustError> {
        check_id("tree id", tree)?;
        let mut file = self.store.load()?;
        file.set_pack(source, tree);
        Ok(self.store.save(&file)?)
    }

    /// `Trusted` when `tree` is the tree the person trusted for `source`; `Untrusted` otherwise, and when
    /// `trust.yaml` cannot be read. The hash of either is the tree id.
    pub fn pack_state(&self, source: &str, tree: &str) -> TrustState {
        let hash = tree.to_string();
        let trusted = self
            .store
            .load()
            .is_ok_and(|file| file.packs.get(source).is_some_and(|pack| pack.tree == tree));
        if trusted {
            TrustState::Trusted { hash }
        } else {
            TrustState::Untrusted { hash }
        }
    }

    /// What changed in the user pack `source` since the person trusted it, with its tree at `repo`.
    pub fn pack_changes(
        &self,
        repo: &Path,
        source: &str,
        tree: &str,
    ) -> Result<Vec<InputDiff>, TrustError> {
        let name = format!("pack {source}");
        let file = self.store.load()?;
        let before: Vec<TrustInput> = file
            .packs
            .get(source)
            .map(|trusted| TrustInput::new(name.clone(), trusted.tree.clone()))
            .into_iter()
            .collect();
        Ok(self.diff(repo, &before, &[TrustInput::new(name, tree)]))
    }

    /// Trusts the `dev:` folder, by its real path. A folder inside one of `workspaces` is refused.
    pub fn trust_dev(&self, folder: &Path, workspaces: &[PathBuf]) -> Result<(), TrustError> {
        let real = check_dev_folder(folder, workspaces)?;
        let mut file = self.store.load()?;
        file.add_dev(&dev_key(&real)?);
        Ok(self.store.save(&file)?)
    }

    /// Removes what was trusted for the board at `root`, for a board that is removed. Nothing is written when there
    /// is nothing to remove, so a missing `trust.yaml` stays missing.
    pub fn forget_board(&self, root: &str) -> Result<(), TrustError> {
        let mut file = self.store.load()?;
        if file.remove_board(root) {
            self.store.save(&file)?;
        }
        Ok(())
    }

    /// Moves what was trusted for the board at `from` to the board at `to`, for a board that was located. What `to`
    /// had is replaced. Nothing is written when there is nothing at `from`.
    pub fn move_board(&self, from: &str, to: &str) -> Result<(), TrustError> {
        let mut file = self.store.load()?;
        if file.move_board(from, to) {
            self.store.save(&file)?;
        }
        Ok(())
    }

    /// Whether the `dev:` folder is trusted. A change inside the folder does not matter: the trust is of the folder.
    /// A folder inside one of `workspaces` is refused.
    pub fn dev_trusted(&self, folder: &Path, workspaces: &[PathBuf]) -> Result<bool, TrustError> {
        let real = check_dev_folder(folder, workspaces)?;
        let key = dev_key(&real)?;
        Ok(self.store.load()?.dev.contains(&key))
    }
}

fn is_trusted(file: &TrustFile, site: &LineSite, hash: &str) -> bool {
    file.line(site.root, site.ref_name)
        .is_some_and(|trusted| trusted.hash == hash)
}

/// The hash of the text: git's own, in the object format of the repository (SHA-1, or SHA-256 in a SHA-256 one). The
/// inputs are git ids, so no hash of them is stronger than git. The one place to swap it.
fn digest(git: &GitObjects, repo: &Path, text: &str) -> Result<String, GitError> {
    git.hash_object(repo, text.as_bytes())
}

/// The folder's real path, when it is a folder outside every Collide workspace: an agent could edit a pack it runs
/// from inside its own workspace (0.9).
pub fn check_dev_folder(folder: &Path, workspaces: &[PathBuf]) -> Result<PathBuf, TrustError> {
    let refused = |reason: String| TrustError::DevFolder {
        folder: folder.to_path_buf(),
        reason,
    };
    let real = fs::canonicalize(folder).map_err(|error| refused(error.to_string()))?;
    if !real.is_dir() {
        return Err(refused("it is not a folder".to_string()));
    }
    for workspace in workspaces {
        let root = fs::canonicalize(workspace).unwrap_or_else(|_| workspace.clone());
        if real.starts_with(&root) {
            return Err(TrustError::DevInWorkspace {
                folder: real,
                workspace: workspace.clone(),
            });
        }
    }
    Ok(real)
}

fn dev_key(real: &Path) -> Result<String, TrustError> {
    real.to_str()
        .map(str::to_string)
        .ok_or_else(|| TrustError::DevFolder {
            folder: real.to_path_buf(),
            reason: "its path is not UTF-8".to_string(),
        })
}

/// The changed lines of a `git diff -U0`: the `@@` lines and the `-` and `+` lines after the header. The first
/// [`MAX_DIFF_LINES`] changed lines are kept and the rest are counted.
fn shape_diff(raw: &str) -> LineDiff {
    let mut diff = LineDiff::default();
    let mut in_hunks = false;
    let mut changed = 0;
    for line in raw.lines() {
        if line.starts_with("Binary files ") || line == "GIT binary patch" {
            return LineDiff {
                binary: true,
                ..LineDiff::default()
            };
        }
        if line.starts_with("@@") {
            in_hunks = true;
        }
        if !in_hunks {
            continue;
        }
        let is_change = line.starts_with('+') || line.starts_with('-');
        if is_change {
            changed += 1;
        }
        if changed > MAX_DIFF_LINES {
            if is_change {
                diff.omitted += 1;
            }
        } else {
            diff.lines.push(line.to_string());
        }
    }
    diff
}

#[cfg(test)]
mod tests {
    use super::*;
    use collide_core::trust::LineTrust;
    use std::fs;

    /// A file that takes `limit` bytes and then fails, as a full disk does.
    struct FailsAfter {
        file: File,
        limit: usize,
    }

    impl Write for FailsAfter {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if self.limit == 0 {
                return Err(io::Error::other("no space left on device"));
            }
            let take = bytes.len().min(self.limit);
            self.limit -= take;
            self.file.write(&bytes[..take])
        }

        fn flush(&mut self) -> io::Result<()> {
            self.file.flush()
        }
    }

    fn folder(name: &str) -> PathBuf {
        let dir = PathBuf::from(format!("/tmp/cdt-trust-unit-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn file_with(hash: &str) -> TrustFile {
        let mut file = TrustFile::default();
        file.set_line(
            "/work/main-repo",
            "main",
            LineTrust {
                hash: hash.to_string(),
                inputs: vec![TrustInput::new("board.yaml", "b".repeat(40))],
            },
        );
        file
    }

    #[test]
    fn a_write_that_fails_after_the_temp_file_leaves_the_old_file_and_no_temp_file() {
        let dir = folder("crash");
        let store = TrustStore::new(dir.join("collide"));
        store.save(&file_with(&"a".repeat(40))).unwrap();
        let before = fs::read(store.file()).unwrap();

        let error = store
            .save_through(&file_with(&"c".repeat(40)), |file| FailsAfter {
                file,
                limit: 20,
            })
            .unwrap_err();
        assert!(matches!(error, StoreError::Write { .. }), "{error}");
        assert!(error.to_string().contains("trust.yaml"), "{error}");
        assert_eq!(
            fs::read(store.file()).unwrap(),
            before,
            "the old file is whole"
        );
        assert_eq!(store.load().unwrap(), file_with(&"a".repeat(40)));
        let names: Vec<_> = fs::read_dir(dir.join("collide"))
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(
            names,
            vec![std::ffi::OsString::from("trust.yaml")],
            "no temp file is left"
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_write_that_fails_on_a_first_save_leaves_no_file() {
        let dir = folder("first");
        let store = TrustStore::new(dir.join("collide"));
        let result = store.save_through(&file_with(&"c".repeat(40)), |file| FailsAfter {
            file,
            limit: 0,
        });
        assert!(result.is_err());
        assert!(!store.file().exists());
        assert_eq!(store.load().unwrap(), TrustFile::default());
        fs::remove_dir_all(&dir).unwrap();
    }
}
