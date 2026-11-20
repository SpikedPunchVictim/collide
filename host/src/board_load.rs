//! The board loader (ts-0096-ph; 0.7, The Collide directory, `requires` and Pack data; 0.6, Materialization).
//!
//! [`load_line`] loads one line from its snapshot: the Collide directory, `board.yaml`, `packs.lock`, `config.yaml`,
//! the profiles, the names of the views, and every pack, each checked. It returns a [`LoadedLine`], or every problem it
//! found, each naming the file and, where one is known, the key. Nothing is read from a working tree: the ref is
//! resolved to a commit once and every read is of that commit's objects, so an edit that is not committed is not used,
//! and a ref that moves during the load cannot give it two snapshots.
//!
//! The order of a load, and of its problems, is the order of the files: the pointer, `board.yaml`, `packs.lock`,
//! `config.yaml`, `profiles/`, `views/`, then the built-in packs and the board's packs in the board's order. A file
//! that is not there is skipped (its value is the default); a file that is there and wrong is a problem, and the load
//! goes on to find the rest.
//!
//! A `git:` pack is loaded by the lock's tree, never by the ref its source names. Its cache entry is used when it is
//! there. When it is not, a context that can fetch ([`LoadContext::with_fetch`], `ts-0100-pu`) takes the pack from the
//! mirror ([`crate::git_fetch`]), fetching into it only when the mirror lacks the locked commit; a context that cannot
//! fetch reports the missing entry. A host never fetched from is a problem with [`LoadProblem::needs_host`] set, and
//! nothing is fetched.
//!
//! With a user scope ([`LoadContext::with_user`], `ts-0101-bd`) the line also loads the user's packs after the board's
//! ([`crate::user_packs`]): a working copy linked over a repo pack takes the repo pack's place in the order, then each
//! `git:` or `dev:` pack the user added that this board has switched on. A user pack that is off is not fetched, read
//! or checked. A name that two packs would share is a problem naming both sources, and so is a `dev:` folder under a
//! Collide workspace. A user pack is not a trust input of the line: the user trusts it by its own tree id.
//!
//! Left to later items: the settings layers (`ts-0098-nx`) and the daemon's use of all this (`ts-0102-vu`).

use crate::bounded_run::{cut_message, run_bounded, tail};
use crate::builtin_packs::{BuiltinPacks, Environment, is_plain_file_name};
use crate::git_fetch::{FetchError, GitFetch, mirror_path};
use crate::git_objects::{GitObjects, ObjectKind, TreeEntry};
use crate::hosts::KnownHosts;
use crate::pack_cache::{Materializer, cached_entry, sdk_version};
use crate::trust::{Trust, check_dev_folder};
use crate::user_packs::{PACKS_FILE, UserScope};
use collide_core::board::{Board, CollidePointer, check_board, check_pointer};
use collide_core::lock::{GitLocator, LockEntry, PackLock, PackSource, check_lock};
use collide_core::pack::{PackManifest, Requires};
use collide_core::pack_check::{ManifestError, check_manifest};
use collide_core::profile::{Profile, check_profile};
use collide_core::resolve::is_safe_token;
use collide_core::settings::{ConfigFile, check_config_file, repo_file_refusals};
use collide_core::settings_registry::collide_roots;
use collide_core::version_range::{Version, VersionRange};
use collide_core::yaml::{MAX_YAML_BYTES, YamlError, load_yaml};
use serde::de::DeserializeOwned;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::io::Read;
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

/// How long a tool's `--version` may run before the load gives up on it.
pub const TOOL_VERSION_TIMEOUT: Duration = Duration::from_secs(5);

/// One thing wrong with a load. `message` is the whole sentence; `file` is the file it is about (a path at the ref,
/// `pack.yaml` for a pack's manifest, or the ref itself), `key` the key in it where one is known, and `pack` the source
/// of the pack when the problem is in one. `needs_host` is set when a pack could not be fetched because its host has
/// never been fetched from: the first fetch from it needs a yes (`ts-0100-pu`). `needs_dev_trust` is set, to the
/// folder's real path, when a `dev:` folder was never trusted: it runs the person's own code and needs a yes
/// (`collide trust`) before it loads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadProblem {
    pub pack: Option<String>,
    pub file: String,
    pub key: Option<String>,
    pub message: String,
    pub needs_host: Option<String>,
    pub needs_dev_trust: Option<String>,
}

impl fmt::Display for LoadProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for LoadProblem {}

/// Where a pack came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackOrigin {
    /// In the app bundle: no pin, no cache entry, no trust (0.4).
    Builtin,
    /// `repo:`, in the line's snapshot: pinned by the tree at the ref.
    Repo,
    /// `git:`, pinned by the lock's tree.
    Git,
    /// A pack the user added (`git:` pinned by the user's lock, or `dev:`), switched on for this board.
    User,
    /// A working copy the user linked on this board: a `dev:` folder, in the place of the repo pack it overrides or
    /// after the board's packs.
    Linked,
}

/// A file of `views/`, listed and not read: [`crate::views`] reads it from its blob, when the line is trusted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewFile {
    /// The file's name, as `mine.yaml`.
    pub name: String,
    /// Its blob id.
    pub id: String,
    /// Its size in bytes, so that a file over the YAML cap is not read.
    pub size: u64,
}

#[derive(Debug, Clone)]
pub struct LoadedPack {
    /// The `name` of its manifest.
    pub name: String,
    pub origin: PackOrigin,
    /// The text `board.yaml` lists it by. `None` for a built-in pack.
    pub source: Option<String>,
    /// The tree id it is pinned at. `None` for a built-in pack.
    pub pin: Option<String>,
    /// The folder that holds `pack.yaml`: `<entry>/pack`, or the bundle's folder for a built-in pack.
    pub dir: PathBuf,
    /// The cache entry folder. `None` for a built-in pack.
    pub entry: Option<PathBuf>,
    pub manifest: PackManifest,
    /// A `dev:` folder, read from the folder as it is now and not from a snapshot: badged wherever it is shown.
    pub dev: bool,
    /// For a linked working copy, the source of the repo pack it took the place of. `None` otherwise.
    pub overrides: Option<String>,
}

/// A line, loaded and checked.
#[derive(Debug, Clone)]
pub struct LoadedLine {
    /// The commit the ref named when the load began. Every file was read from it.
    pub commit: String,
    /// The Collide directory as a path in the repo (`.collide`, or the pointer's `dir`). `None` when there is none.
    pub collide_dir: Option<String>,
    pub board: Board,
    pub lock: PackLock,
    /// The repo's own settings file (`ts-0098-nx` layers it).
    pub config: ConfigFile,
    /// The repo's profiles, by file name without `.yaml`.
    pub profiles: BTreeMap<String, Profile>,
    pub views: Vec<ViewFile>,
    /// The built-in packs first, then the board's, in the board's order.
    pub packs: Vec<LoadedPack>,
    inputs: Vec<(String, String)>,
}

impl LoadedLine {
    /// What the trust hash covers (`ts-0097-kw`; 0.9): a name and a git id for each thing the line read from the
    /// repo. The names are `pointer`, `board.yaml`, `packs.lock`, `config.yaml`, `profiles/<file>`, `views/<file>`
    /// and `pack <source>` (the tree id, which for a `git:` pack is the lock's). A file that is not there is not
    /// listed, and a built-in pack never is. Sorted, so the same line gives the same list.
    pub fn trust_inputs(&self) -> Vec<(String, String)> {
        self.inputs.clone()
    }
}

/// What a load needs besides the repository: where the cache and the SDK are, the `PATH` to look for tools on, the
/// built-in packs, and how git is run.
pub struct LoadContext {
    /// The cache folder (`<cache>/packs/` holds the entries).
    pub cache: PathBuf,
    /// The SDK folder [`crate::pack_cache::ensure_sdk`] returned.
    pub sdk: PathBuf,
    /// The captured environment: the `PATH` `requires.tools` is looked for on.
    pub env: Environment,
    pub builtin: BuiltinPacks,
    pub git: GitObjects,
    pub materializer: Materializer,
    pub tool_timeout: Duration,
    /// How a `git:` pack that is not in the cache is fetched. `None`: it is not, and the load says so.
    pub fetch: Option<FetchSetup>,
    /// The user and board whose packs are loaded after the board's. `None`: the line's own packs only.
    pub user: Option<UserScope>,
}

/// What fetching a pack needs: the rules ([`GitFetch`]) and the hosts the person has allowed ([`KnownHosts`]).
#[derive(Debug, Clone)]
pub struct FetchSetup {
    pub fetch: GitFetch,
    pub hosts: KnownHosts,
}

impl LoadContext {
    /// Git found on the process's `PATH`, the real writer and the default tool timeout.
    pub fn new(
        cache: PathBuf,
        sdk: PathBuf,
        env: Environment,
        builtin: BuiltinPacks,
    ) -> LoadContext {
        LoadContext {
            cache,
            sdk,
            env,
            builtin,
            git: GitObjects::new(),
            materializer: Materializer::new(),
            tool_timeout: TOOL_VERSION_TIMEOUT,
            fetch: None,
            user: None,
        }
    }

    /// Loads `user`'s packs after the board's, for the board the scope names.
    pub fn with_user(mut self, user: UserScope) -> LoadContext {
        self.user = Some(user);
        self
    }

    /// Lets a `git:` pack that is not in the cache be fetched, under the rules of `fetch`.
    pub fn with_fetch(mut self, fetch: GitFetch, hosts: KnownHosts) -> LoadContext {
        self.fetch = Some(FetchSetup { fetch, hosts });
        self
    }

    /// Reads and materializes with `git`.
    pub fn with_git(mut self, git: GitObjects) -> LoadContext {
        self.materializer = Materializer::new().with_git(git.clone());
        self.git = git;
        self
    }

    pub fn with_tool_timeout(mut self, timeout: Duration) -> LoadContext {
        self.tool_timeout = timeout;
        self
    }
}

/// Loads the line at `rev` of the repository whose git directory is `repo`. A ref or a commit id, never a path.
pub fn load_line(
    repo: &Path,
    rev: &str,
    context: &LoadContext,
) -> Result<LoadedLine, Vec<LoadProblem>> {
    let commit = context
        .git
        .resolve_commit(repo, rev)
        .map_err(|error| vec![problem(None, rev, None, error)])?;
    let mut load = Load {
        repo,
        context,
        commit,
        problems: Vec::new(),
        inputs: Vec::new(),
        tools: BTreeMap::new(),
        sdk: None,
    };
    let dir = load.collide_dir();
    let files = match &dir {
        Some(dir) => load.read_files(dir),
        None => Files::default(),
    };
    let mut names = Names::default();
    // A pack's setting is `<pack>.<key>`, so a pack named like a root of Collide's own keys would name two things.
    for root in collide_roots() {
        names
            .0
            .push((root.to_string(), format!("Collide's own `{root}` settings")));
    }
    let mut packs = load.builtin_packs(&mut names);
    if let (Some(dir), Some(board)) = (&dir, &files.board) {
        load.board_packs(dir, board, &files.lock, &mut names, &mut packs);
    }
    if let Some(scope) = &context.user {
        load.user_packs(scope, &mut names, &mut packs);
    }
    if !load.problems.is_empty() {
        return Err(load.problems);
    }
    let mut inputs = load.inputs;
    inputs.sort();
    Ok(LoadedLine {
        commit: load.commit,
        collide_dir: dir.map(|dir| dir.path),
        board: files.board.unwrap_or_default(),
        lock: files.lock,
        config: files.config,
        profiles: files.profiles,
        views: files.views,
        packs,
        inputs,
    })
}

/// A `dev:` folder to load as a pack: `source` is the text shown, `folder` the path it names, `expect` the name a link
/// gives it, and `overrides` the source of the repo pack a link took the place of.
struct DevFolder {
    source: String,
    folder: String,
    expect: Option<String>,
    origin: PackOrigin,
    overrides: Option<String>,
}

/// What `board.yaml` and the lock hold at a commit, read with no check and no pack loaded: the way to find a pack
/// when the line does not load (`collide pack update` moves a pack off a pin that breaks the load).
#[derive(Debug, Clone)]
pub struct BoardFiles {
    pub commit: String,
    /// The Collide directory as a path in the repo, as the loader finds it. `None` when there is none.
    pub collide_dir: Option<String>,
    /// `None` when there is no `board.yaml` or it does not read.
    pub board: Option<Board>,
    /// Empty when there is no lock or it does not read.
    pub lock: PackLock,
}

/// Reads the Collide directory, `board.yaml` and `packs.lock` at `rev`. Anything that does not read is left empty:
/// [`load_line`] is the one that reports it.
pub fn board_files(repo: &Path, rev: &str, context: &LoadContext) -> Result<BoardFiles, String> {
    let commit = context
        .git
        .resolve_commit(repo, rev)
        .map_err(|error| error.to_string())?;
    let mut load = Load {
        repo,
        context,
        commit,
        problems: Vec::new(),
        inputs: Vec::new(),
        tools: BTreeMap::new(),
        sdk: None,
    };
    let dir = load.collide_dir();
    let files = match &dir {
        Some(dir) => load.read_files(dir),
        None => Files::default(),
    };
    Ok(BoardFiles {
        commit: load.commit,
        collide_dir: dir.map(|dir| dir.path),
        board: files.board,
        lock: files.lock,
    })
}

fn problem(
    pack: Option<&str>,
    file: &str,
    key: Option<&str>,
    detail: impl fmt::Display,
) -> LoadProblem {
    let detail = detail.to_string();
    // A YAML error already starts with the file and the line.
    let place = if detail.starts_with(file) {
        String::new()
    } else {
        format!("{file}: ")
    };
    let pack_part = pack
        .map(|pack| format!("pack {pack}: "))
        .unwrap_or_default();
    let key_part = key.map(|key| format!("{key}: ")).unwrap_or_default();
    let message = format!("{pack_part}{place}{key_part}{detail}");
    LoadProblem {
        pack: pack.map(str::to_string),
        file: file.to_string(),
        key: key.map(str::to_string),
        message,
        needs_host: None,
        needs_dev_trust: None,
    }
}

/// What a tree entry is, from its mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Folder,
    File,
    Symlink,
    Submodule,
    Other,
}

impl Kind {
    fn of(entry: &TreeEntry) -> Kind {
        match entry.mode.as_str() {
            "040000" => Kind::Folder,
            "100644" | "100755" => Kind::File,
            "120000" => Kind::Symlink,
            "160000" => Kind::Submodule,
            _ => Kind::Other,
        }
    }

    fn describe(self) -> &'static str {
        match self {
            Kind::Folder => "a folder",
            Kind::File => "a file",
            Kind::Symlink => "a symlink",
            Kind::Submodule => "a submodule",
            Kind::Other => "not a file or a folder",
        }
    }
}

/// The Collide directory: its path in the repo and its tree id.
struct CollideDir {
    path: String,
    tree: String,
}

impl CollideDir {
    fn file(&self, name: &str) -> String {
        format!("{}/{name}", self.path)
    }
}

#[derive(Default)]
struct Files {
    board: Option<Board>,
    lock: PackLock,
    config: ConfigFile,
    profiles: BTreeMap<String, Profile>,
    views: Vec<ViewFile>,
}

/// The names the packs of one load have taken, each with how to say who has it.
#[derive(Default)]
struct Names(Vec<(String, String)>);

struct Sdk {
    folder_version: String,
    version: Option<Version>,
}

struct Load<'a> {
    repo: &'a Path,
    context: &'a LoadContext,
    commit: String,
    problems: Vec<LoadProblem>,
    inputs: Vec<(String, String)>,
    /// What each tool's `--version` gave, so a tool that two packs need is run once.
    tools: BTreeMap<String, Result<Version, String>>,
    /// The SDK's version, read when the first pack from the cache needs it. `Some(None)` is a read that failed.
    sdk: Option<Option<Sdk>>,
}

impl Load<'_> {
    fn push(
        &mut self,
        pack: Option<&str>,
        file: &str,
        key: Option<&str>,
        detail: impl fmt::Display,
    ) {
        self.problems.push(problem(pack, file, key, detail));
    }

    /// A problem whose message is already whole.
    fn push_text(&mut self, file: &str, message: String) {
        self.problems.push(LoadProblem {
            pack: None,
            file: file.to_string(),
            key: None,
            message,
            needs_host: None,
            needs_dev_trust: None,
        });
    }

    // ---- the Collide directory ----

    /// `.collide` as a folder, or the folder a `.collide` pointer file names. `None` with no problem when there is
    /// neither, and `None` with problems when there is one that cannot be used.
    fn collide_dir(&mut self) -> Option<CollideDir> {
        let entry = match self
            .context
            .git
            .entry_at(self.repo, &self.commit, ".collide")
        {
            Ok(Some(entry)) => entry,
            Ok(None) => return None,
            Err(error) => {
                self.push(None, ".collide", None, error);
                return None;
            }
        };
        match Kind::of(&entry) {
            Kind::Folder => Some(CollideDir {
                path: ".collide".to_string(),
                tree: entry.id,
            }),
            Kind::File => self.follow_pointer(&entry),
            other => {
                self.push(
                    None,
                    ".collide",
                    None,
                    format!(
                        "is {}; it must be a folder, or a file with `dir:` in it",
                        other.describe()
                    ),
                );
                None
            }
        }
    }

    fn follow_pointer(&mut self, entry: &TreeEntry) -> Option<CollideDir> {
        let pointer: CollidePointer = self.read_yaml(".collide", "pointer", entry)?;
        if let Err(error) = check_pointer(&pointer) {
            self.push(None, ".collide", None, error);
            return None;
        }
        let dir = pointer.dir.as_str();
        if dir.split('/').any(|part| part.is_empty() || part == ".") {
            self.push(
                None,
                ".collide",
                None,
                format!("`dir: {dir}` is not a plain relative path: no empty or `.` part, no trailing `/`"),
            );
            return None;
        }
        let found = match self.context.git.entry_at(self.repo, &self.commit, dir) {
            Ok(found) => found,
            Err(error) => {
                self.push(None, ".collide", None, format!("`dir: {dir}`: {error}"));
                return None;
            }
        };
        match found {
            None => self.push(
                None,
                ".collide",
                None,
                format!("`dir: {dir}` is not in the repo at this ref"),
            ),
            Some(found) if Kind::of(&found) == Kind::Folder => {
                return Some(CollideDir {
                    path: dir.to_string(),
                    tree: found.id,
                });
            }
            Some(found) => self.push(
                None,
                ".collide",
                None,
                format!(
                    "`dir: {dir}` is {}, not a folder",
                    Kind::of(&found).describe()
                ),
            ),
        }
        None
    }

    // ---- reading a file ----

    /// The bytes of `entry`, `path` at the ref, when it is a regular file within the YAML cap. The entry's blob id is
    /// recorded as the trust input `input`.
    fn read_bytes(&mut self, path: &str, input: &str, entry: &TreeEntry) -> Option<Vec<u8>> {
        self.inputs.push((input.to_string(), entry.id.clone()));
        let kind = Kind::of(entry);
        if kind != Kind::File {
            self.push(
                None,
                path,
                None,
                format!("is {}, and it must be a regular file", kind.describe()),
            );
            return None;
        }
        if entry.size > MAX_YAML_BYTES as u64 {
            let error = YamlError::TooLarge {
                origin: path.to_string(),
                cap: MAX_YAML_BYTES,
            };
            self.push(None, path, None, error);
            return None;
        }
        match self
            .context
            .git
            .read_blobs(self.repo, std::slice::from_ref(&entry.id))
        {
            Ok(mut bodies) => bodies.pop(),
            Err(error) => {
                self.push(None, path, None, error);
                None
            }
        }
    }

    fn read_yaml<T: DeserializeOwned>(
        &mut self,
        path: &str,
        input: &str,
        entry: &TreeEntry,
    ) -> Option<T> {
        let bytes = self.read_bytes(path, input, entry)?;
        match load_yaml(&bytes, path) {
            Ok(value) => Some(value),
            Err(error) => {
                self.push(None, path, None, error);
                None
            }
        }
    }

    // ---- the files of the Collide directory ----

    fn read_files(&mut self, dir: &CollideDir) -> Files {
        let mut files = Files::default();
        let entries = match self.context.git.list_dir(self.repo, &dir.tree) {
            Ok(entries) => entries,
            Err(error) => {
                self.push(None, &dir.path, None, error);
                return files;
            }
        };
        let find = |name: &str| entries.iter().find(|entry| entry.path == name);

        if let Some(entry) = find("board.yaml") {
            let path = dir.file("board.yaml");
            files.board = self.read_yaml::<Board>(&path, "board.yaml", entry);
            if let Some(board) = &files.board
                && let Err(errors) = check_board(board)
            {
                for error in errors {
                    self.push(None, &path, None, error);
                }
            }
        }
        if let Some(entry) = find("packs.lock") {
            let path = dir.file("packs.lock");
            if let Some(lock) = self.read_yaml::<PackLock>(&path, "packs.lock", entry) {
                if let Err(errors) = check_lock(&lock) {
                    for error in errors {
                        self.push(None, &path, None, error);
                    }
                }
                files.lock = lock;
            }
        }
        if let Some(entry) = find("config.yaml") {
            let path = dir.file("config.yaml");
            if let Some(config) = self.read_yaml::<ConfigFile>(&path, "config.yaml", entry) {
                if let Err(errors) = check_config_file(&config) {
                    for error in errors {
                        self.push(None, &path, None, error);
                    }
                }
                for refusal in repo_file_refusals(&config) {
                    self.push(None, &path, Some(&refusal.key), &refusal);
                }
                files.config = config;
            }
        }
        if let Some(entry) = find("profiles") {
            self.read_profiles(dir, entry, &mut files);
        }
        if let Some(entry) = find("views") {
            self.list_views(dir, entry, &mut files);
        }
        files
    }

    /// The `.yaml` entries of the folder `entry` (`profiles` or `views`), with the folder's name for messages.
    fn yaml_entries(&mut self, dir: &CollideDir, entry: &TreeEntry) -> Vec<TreeEntry> {
        let path = dir.file(&entry.path);
        if Kind::of(entry) != Kind::Folder {
            self.push(
                None,
                &path,
                None,
                format!("is {}, and it must be a folder", Kind::of(entry).describe()),
            );
            return Vec::new();
        }
        match self.context.git.list_dir(self.repo, &entry.id) {
            Ok(entries) => entries
                .into_iter()
                .filter(|entry| entry.path.ends_with(".yaml"))
                .collect(),
            Err(error) => {
                self.push(None, &path, None, error);
                Vec::new()
            }
        }
    }

    fn read_profiles(&mut self, dir: &CollideDir, folder: &TreeEntry, files: &mut Files) {
        for entry in self.yaml_entries(dir, folder) {
            let path = dir.file(&format!("profiles/{}", entry.path));
            let input = format!("profiles/{}", entry.path);
            let Some(profile) = self.read_yaml::<Profile>(&path, &input, &entry) else {
                continue;
            };
            if let Err(errors) = check_profile(&profile) {
                for error in errors {
                    self.push(None, &path, None, error);
                }
            }
            let name = entry.path.trim_end_matches(".yaml").to_string();
            files.profiles.insert(name, profile);
        }
    }

    /// Views are listed by name, blob id and size and not read: [`crate::views`] parses them and checks their CEL.
    fn list_views(&mut self, dir: &CollideDir, folder: &TreeEntry, files: &mut Files) {
        for entry in self.yaml_entries(dir, folder) {
            let path = dir.file(&format!("views/{}", entry.path));
            self.inputs
                .push((format!("views/{}", entry.path), entry.id.clone()));
            let kind = Kind::of(&entry);
            if kind != Kind::File {
                self.push(
                    None,
                    &path,
                    None,
                    format!("is {}, and it must be a regular file", kind.describe()),
                );
                continue;
            }
            files.views.push(ViewFile {
                name: entry.path,
                id: entry.id,
                size: entry.size,
            });
        }
    }

    // ---- the packs ----

    fn builtin_packs(&mut self, names: &mut Names) -> Vec<LoadedPack> {
        let mut loaded = Vec::new();
        for name in BuiltinPacks::names() {
            names
                .0
                .push((name.to_string(), format!("the built-in pack `{name}`")));
            let result = self.context.builtin.dir(name).and_then(|dir| {
                self.context
                    .builtin
                    .manifest(name)
                    .map(|manifest| (dir, manifest))
            });
            match result {
                Ok((dir, manifest)) => loaded.push(LoadedPack {
                    name: name.to_string(),
                    origin: PackOrigin::Builtin,
                    source: None,
                    pin: None,
                    dir,
                    entry: None,
                    manifest,
                    dev: false,
                    overrides: None,
                }),
                Err(error) => self.push(
                    None,
                    name,
                    None,
                    format!("the built-in pack `{name}` cannot be loaded: {error}"),
                ),
            }
        }
        loaded
    }

    fn board_packs(
        &mut self,
        dir: &CollideDir,
        board: &Board,
        lock: &PackLock,
        names: &mut Names,
        packs: &mut Vec<LoadedPack>,
    ) {
        let mut listed: Vec<&str> = Vec::new();
        for (index, entry) in board.packs.iter().enumerate() {
            // A source listed twice is `check_board`'s problem and is loaded once.
            if listed.contains(&entry.source.as_str()) {
                continue;
            }
            listed.push(&entry.source);
            let loaded = match PackSource::parse(&entry.source) {
                // A source that does not parse, and `dev:`, were refused by `check_board` with its message.
                Err(_) | Ok(PackSource::Dev { .. }) => None,
                Ok(PackSource::Other { scheme, .. }) => {
                    let key = format!("packs[{index}].source");
                    self.push(
                        None,
                        &dir.file("board.yaml"),
                        Some(&key),
                        format!(
                            "`{}`: the scheme `{scheme}:` is not supported; use `repo:` or `git:`",
                            entry.source
                        ),
                    );
                    None
                }
                Ok(PackSource::Repo { path }) => self.repo_pack(&entry.source, &path, names),
                Ok(PackSource::Git { .. }) => self.git_pack(
                    &dir.file("packs.lock"),
                    &entry.source,
                    lock,
                    PackOrigin::Git,
                    names,
                ),
            };
            packs.extend(loaded);
        }
    }

    /// The user's packs, after the board's: links first (each replaces the repo pack of its name in place, or follows
    /// the board's packs when there is none), then the packs the user added that this board has switched on.
    fn user_packs(&mut self, scope: &UserScope, names: &mut Names, packs: &mut Vec<LoadedPack>) {
        let state = match scope.read() {
            Ok(state) => state,
            Err(errors) => {
                for message in errors {
                    self.push_text(PACKS_FILE, message);
                }
                return;
            }
        };
        for (name, folder) in &state.board.links {
            let source = format!("dev:{folder}");
            let slot = packs
                .iter()
                .position(|pack| pack.name == *name && pack.origin == PackOrigin::Repo);
            let overrides = slot.and_then(|index| packs[index].source.clone());
            if slot.is_some() {
                // The link takes the repo pack's name, so the name is free for it.
                names.0.retain(|(taken, holder)| {
                    taken != name || !holder.starts_with("the pack `repo:")
                });
            }
            let dev = DevFolder {
                source,
                folder: folder.clone(),
                expect: Some(name.clone()),
                origin: PackOrigin::Linked,
                overrides,
            };
            let loaded = self.dev_pack(scope, dev, names);
            match (loaded, slot) {
                (Some(pack), Some(index)) => packs[index] = pack,
                (Some(pack), None) => packs.push(pack),
                (None, _) => {}
            }
        }
        let lock_path = scope.lock_file().display().to_string();
        let known: Vec<Option<String>> = state
            .sources
            .iter()
            .map(|source| {
                scope.name_of(
                    source,
                    &state.lock,
                    &self.context.git,
                    &self.context.cache,
                    &self.context.sdk,
                )
            })
            .collect();
        let is_on = |name: &str| state.board.on.iter().any(|on| on == name);
        // A pack whose name cannot be read without loading it is loaded only when this board switched on a name no
        // readable pack has, so one broken pack does not fail every board.
        let unread_may_be_on = state
            .board
            .on
            .iter()
            .any(|on| !known.iter().flatten().any(|name| name == on));
        for (source, known) in state.sources.iter().zip(known) {
            match known.as_deref() {
                Some(name) if !is_on(name) => continue,
                None if !unread_may_be_on => continue,
                _ => {}
            }
            let loaded = match PackSource::parse(source) {
                Ok(PackSource::Git { .. }) => {
                    self.git_pack(&lock_path, source, &state.lock, PackOrigin::User, names)
                }
                Ok(PackSource::Dev { path }) => {
                    let dev = DevFolder {
                        source: source.clone(),
                        folder: path,
                        expect: None,
                        origin: PackOrigin::User,
                        overrides: None,
                    };
                    self.dev_pack(scope, dev, names)
                }
                Ok(PackSource::Other { scheme, .. }) => {
                    self.push_text(
                        PACKS_FILE,
                        format!(
                            "{}: `{source}`: the scheme `{scheme}:` is not supported; use `git:` or `dev:`",
                            scope.packs_file().display()
                        ),
                    );
                    None
                }
                // `read` refused these with its own message.
                Ok(PackSource::Repo { .. }) | Err(_) => None,
            };
            let Some(pack) = loaded else {
                continue;
            };
            // A pack whose name was not known without loading it, and that is off: it was loaded to find out.
            if known.is_none() && !is_on(&pack.name) {
                let holder = format!("the pack `{source}`");
                names.0.retain(|(_, taken_by)| *taken_by != holder);
                continue;
            }
            packs.push(pack);
        }
    }

    /// A `dev:` folder, read as it is now: no snapshot, no cache entry. It must not be under a Collide workspace, its
    /// manifest must check, and its tree id (git's, over its files) is its pin. `expect` is the name a link gives it.
    fn dev_pack(
        &mut self,
        scope: &UserScope,
        dev: DevFolder,
        names: &mut Names,
    ) -> Option<LoadedPack> {
        let DevFolder {
            source,
            folder,
            expect,
            origin,
            overrides,
        } = dev;
        let (source, folder, expect) = (source.as_str(), folder.as_str(), expect.as_deref());
        let real = match check_dev_folder(&scope.expand(folder), scope.workspaces()) {
            Ok(real) => real,
            Err(error) => {
                self.push(Some(source), folder, None, error);
                return None;
            }
        };
        // Nothing of a folder is read, and none of its code is hashed or run, before the person has trusted it.
        let trust = Trust::new(scope.trust_store()).with_git(self.context.git.clone());
        match trust.dev_trusted(&real, scope.workspaces()) {
            Ok(true) => {}
            Ok(false) => {
                let mut found = problem(
                    Some(source),
                    folder,
                    None,
                    format!(
                        "the `dev:` folder {} has not been trusted: run `collide trust` to review and trust it",
                        real.display()
                    ),
                );
                found.needs_dev_trust = Some(real.display().to_string());
                self.problems.push(found);
                return None;
            }
            Err(error) => {
                self.push(Some(source), folder, None, error);
                return None;
            }
        }
        let manifest = self.read_manifest(source, &real.join("pack.yaml"))?;
        if let Some(expected) = expect
            && manifest.name != expected
        {
            self.push(
                Some(source),
                "pack.yaml",
                Some("name"),
                format!(
                    "the folder {} holds the pack `{}`, not `{expected}`, so it cannot be linked as `{expected}`",
                    real.display(),
                    manifest.name
                ),
            );
            return None;
        }
        let before = self.problems.len();
        let label = format!("`{}` ({source})", manifest.name);
        self.check_names(source, &label, &manifest, names);
        if let Some(requires) = &manifest.requires {
            self.check_requires(source, &label, requires);
        }
        if self.problems.len() > before {
            return None;
        }
        let tree = match self.context.git.tree_of_folder(&real) {
            Ok(tree) => tree,
            Err(error) => {
                self.push(Some(source), folder, None, error);
                return None;
            }
        };
        if origin == PackOrigin::Linked {
            let copies = scope.working_copies(&self.context.git, &self.context.cache);
            match copies.waiting(&real) {
                Ok(None) => {}
                Ok(Some(waiting)) => {
                    self.push_text(&real.display().to_string(), waiting.render(&real));
                    return None;
                }
                Err(error) => {
                    self.push(Some(source), folder, None, error);
                    return None;
                }
            }
        }
        Some(LoadedPack {
            name: manifest.name.clone(),
            origin,
            source: Some(source.to_string()),
            pin: Some(tree),
            dir: real,
            entry: None,
            manifest,
            dev: true,
            overrides,
        })
    }

    /// A `repo:` pack: the tree at the ref is the pin, and it is materialized.
    fn repo_pack(&mut self, source: &str, path: &str, names: &mut Names) -> Option<LoadedPack> {
        let found = match self.context.git.entry_at(self.repo, &self.commit, path) {
            Ok(found) => found,
            Err(error) => {
                self.push(Some(source), path, None, error);
                return None;
            }
        };
        let tree = match found {
            Some(found) if Kind::of(&found) == Kind::Folder => found.id,
            Some(found) => {
                let kind = Kind::of(&found).describe();
                self.push(
                    Some(source),
                    path,
                    None,
                    format!("is {kind}, and a pack is a folder"),
                );
                return None;
            }
            None => {
                self.push(Some(source), path, None, "is not in the repo at this ref");
                return None;
            }
        };
        self.sdk()?;
        let context = self.context;
        match context
            .materializer
            .materialize(&context.cache, &context.sdk, self.repo, &tree)
        {
            Ok(entry) => self.read_pack(source, PackOrigin::Repo, tree, entry, names),
            Err(error) => {
                self.push(Some(source), path, None, error);
                None
            }
        }
    }

    /// A `git:` pack: the lock's tree is the pin. Its cache entry is used when it is there; if not, it comes from the
    /// mirror ([`Load::pack_from_mirror`]).
    fn git_pack(
        &mut self,
        lock_path: &str,
        source: &str,
        lock: &PackLock,
        origin: PackOrigin,
        names: &mut Names,
    ) -> Option<LoadedPack> {
        let Some(locked) = lock.packs.iter().find(|locked| locked.source == source) else {
            let listed: Vec<String> = lock
                .packs
                .iter()
                .map(|locked| format!("`{}`", locked.source))
                .collect();
            let listed = if listed.is_empty() {
                "nothing".to_string()
            } else {
                listed.join(", ")
            };
            self.push(
                Some(source),
                lock_path,
                None,
                format!("has no entry for this exact source text; the lock lists {listed}"),
            );
            return None;
        };
        // An entry with a bad id was reported with the file; its tree is not looked up.
        let sound = LockEntry::clone(locked);
        if check_lock(&PackLock { packs: vec![sound] }).is_err() {
            return None;
        }
        let sdk = self.sdk()?.folder_version.clone();
        match cached_entry(&self.context.cache, &locked.tree, &sdk) {
            Ok(Some(entry)) => self.read_pack(source, origin, locked.tree.clone(), entry, names),
            Ok(None) => self.pack_from_mirror(lock_path, source, locked, origin, names),
            Err(error) => {
                self.push(Some(source), lock_path, None, error);
                None
            }
        }
    }

    /// A `git:` pack whose entry is not in the cache: the locked commit is taken from the mirror, fetching first when
    /// the mirror does not have it. The tree at the pack's path in that commit must be the locked tree, so a lock
    /// cannot be satisfied by other files than the ones it pinned.
    fn pack_from_mirror(
        &mut self,
        lock_path: &str,
        source: &str,
        locked: &LockEntry,
        origin: PackOrigin,
        names: &mut Names,
    ) -> Option<LoadedPack> {
        let context = self.context;
        let unreachable = |commit: &str, tree: &str| {
            format!("is pinned at commit {commit} (tree {tree}), which is not in the cache")
        };
        let Some(setup) = &context.fetch else {
            self.push(
                Some(source),
                lock_path,
                None,
                format!(
                    "{}; this load cannot fetch it",
                    unreachable(&locked.commit, &locked.tree)
                ),
            );
            return None;
        };
        let locator = match PackSource::parse(source) {
            Ok(PackSource::Git { locator }) => GitLocator::parse(&locator),
            _ => return None,
        };
        let locator = match locator {
            Ok(locator) => locator,
            Err(error) => {
                self.push(Some(source), lock_path, None, error);
                return None;
            }
        };
        let mirror = mirror_path(&context.cache, &locator.url);
        if !self.mirror_has(&mirror, &locked.commit) {
            if let Err(error) = setup.fetch.fetch(&mirror, &locator.url, &setup.hosts) {
                self.fetch_problem(lock_path, source, locked, error);
                return None;
            }
            if !self.mirror_has(&mirror, &locked.commit) {
                self.push(
                    Some(source),
                    lock_path,
                    None,
                    format!(
                        "is pinned at commit {}, which the remote no longer has: it was fetched from {} and the \
                         commit is not in it, so move the pin with `collide pack update` or restore the commit",
                        locked.commit, locator.url
                    ),
                );
                return None;
            }
        }
        let found = context.git.tree_at(&mirror, &locked.commit, &locator.path);
        match found {
            Ok(found) if found.kind == ObjectKind::Tree && found.id == locked.tree => {}
            Ok(found) => {
                self.push(
                    Some(source),
                    lock_path,
                    None,
                    format!(
                        "is pinned at tree {}, but commit {} has {} at `{}`: the lock does not match the repository",
                        locked.tree,
                        locked.commit,
                        found.id,
                        locator.path
                    ),
                );
                return None;
            }
            Err(error) => {
                self.push(
                    Some(source),
                    lock_path,
                    None,
                    format!(
                        "`{}` is not in commit {}: {error}",
                        locator.path, locked.commit
                    ),
                );
                return None;
            }
        }
        self.sdk()?;
        match context
            .materializer
            .materialize(&context.cache, &context.sdk, &mirror, &locked.tree)
        {
            Ok(entry) => self.read_pack(source, origin, locked.tree.clone(), entry, names),
            Err(error) => {
                self.push(Some(source), lock_path, None, error);
                None
            }
        }
    }

    /// Whether `mirror` is there and has the commit `id`.
    fn mirror_has(&self, mirror: &Path, id: &str) -> bool {
        mirror.exists()
            && matches!(
                self.context.git.object_kind(mirror, id),
                Ok(Some(ObjectKind::Commit))
            )
    }

    fn fetch_problem(
        &mut self,
        lock_path: &str,
        source: &str,
        locked: &LockEntry,
        error: FetchError,
    ) {
        let message = match &error {
            FetchError::NeedsApproval { host } => format!(
                "is pinned at commit {} (tree {}), which is not in the cache, and the first fetch from {host} needs \
                 a yes before anything is fetched",
                locked.commit, locked.tree
            ),
            other => format!(
                "is pinned at commit {} (tree {}), which is not in the cache, and getting it failed: {other}",
                locked.commit, locked.tree
            ),
        };
        let mut found = problem(Some(source), lock_path, None, message);
        if let FetchError::NeedsApproval { host } = error {
            found.needs_host = Some(host);
        }
        self.problems.push(found);
    }

    /// Reads and checks the manifest of the materialized pack at `entry`. The pack is returned only when it has no
    /// problem of its own.
    fn read_pack(
        &mut self,
        source: &str,
        origin: PackOrigin,
        pin: String,
        entry: PathBuf,
        names: &mut Names,
    ) -> Option<LoadedPack> {
        let before = self.problems.len();
        // The repo's trust hash covers the packs the repo lists. A user's pack is trusted by its own tree id.
        if matches!(origin, PackOrigin::Repo | PackOrigin::Git) {
            self.inputs.push((format!("pack {source}"), pin.clone()));
        }
        let dir = entry.join("pack");
        let manifest = self.read_manifest(source, &dir.join("pack.yaml"))?;
        let label = format!("`{}` ({source})", manifest.name);
        self.check_names(source, &label, &manifest, names);
        if let Some(requires) = &manifest.requires {
            self.check_requires(source, &label, requires);
        }
        if self.problems.len() > before {
            return None;
        }
        Some(LoadedPack {
            name: manifest.name.clone(),
            origin,
            source: Some(source.to_string()),
            pin: Some(pin),
            dir,
            entry: Some(entry),
            manifest,
            dev: false,
            overrides: None,
        })
    }

    fn read_manifest(&mut self, source: &str, path: &Path) -> Option<PackManifest> {
        let mut bytes = Vec::new();
        let read = fs::File::open(path)
            .and_then(|file| file.take(MAX_YAML_BYTES as u64 + 1).read_to_end(&mut bytes));
        match read {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                self.push(Some(source), "pack.yaml", None, "the pack has no pack.yaml");
                None
            }
            Err(error) => {
                self.push(
                    Some(source),
                    "pack.yaml",
                    None,
                    format!("cannot be read: {error}"),
                );
                None
            }
            Ok(_) => match load_yaml::<PackManifest>(&bytes, "pack.yaml") {
                Ok(manifest) => Some(manifest),
                Err(error) => {
                    self.push(Some(source), "pack.yaml", None, error);
                    None
                }
            },
        }
    }

    /// The manifest check, then the safe-token rules (0.5) on the pack's name and on every role, workflow, prompt and
    /// template field it declares, and the name's place among the line's packs.
    fn check_names(
        &mut self,
        source: &str,
        label: &str,
        manifest: &PackManifest,
        names: &mut Names,
    ) {
        let mut flagged = BTreeSet::new();
        if let Err(errors) = check_manifest(manifest) {
            for error in errors {
                if let ManifestError::BadName { name, .. } = &error {
                    flagged.insert(name.clone());
                }
                self.push(Some(source), "pack.yaml", None, error);
            }
        }
        let token = |load: &mut Load, key: String, name: &str| {
            if !is_safe_token(name) && !flagged.contains(name) {
                load.push(
                    Some(source),
                    "pack.yaml",
                    Some(&key),
                    format!(
                        "`{name}` is not a safe token: use 1 to 64 letters, digits, `.`, `_` or `-`, not starting \
                         with `-` or `.`, and without `..`"
                    ),
                );
            }
        };
        token(self, "name".to_string(), &manifest.name);
        for role in manifest.roles.keys() {
            token(self, format!("roles.{role}"), role);
        }
        for (workflow, body) in &manifest.workflows {
            token(self, format!("workflows.{workflow}"), workflow);
            let fields = body.accepts.iter().flat_map(|accepts| &accepts.fields);
            for (index, field) in fields.enumerate() {
                token(
                    self,
                    format!("workflows.{workflow}.accepts.fields[{index}]"),
                    field,
                );
            }
        }
        for prompt in manifest.prompts.keys() {
            token(self, format!("prompts.{prompt}"), prompt);
        }

        match names.0.iter().find(|(taken, _)| *taken == manifest.name) {
            Some((_, holder)) => {
                let holder = holder.clone();
                self.push(
                    Some(source),
                    "pack.yaml",
                    Some("name"),
                    format!(
                        "`{}` is already the name of {holder}, so pack {label} cannot have it",
                        manifest.name
                    ),
                );
            }
            None => names
                .0
                .push((manifest.name.clone(), format!("the pack `{source}`"))),
        }
    }

    // ---- requires ----

    /// The SDK's version, read once. `None` when it cannot be read, which is one problem for the line: no pack is
    /// loaded from the cache without an SDK to link.
    fn sdk(&mut self) -> Option<&Sdk> {
        if self.sdk.is_none() {
            let read = sdk_version(&self.context.sdk);
            self.sdk = Some(match read {
                Ok(text) => Some(Sdk {
                    version: Version::find_in(&text),
                    folder_version: text,
                }),
                Err(error) => {
                    self.push(
                        None,
                        "SDK",
                        None,
                        format!("the SDK cannot be used, so no pack can be loaded: {error}"),
                    );
                    None
                }
            });
        }
        self.sdk.as_ref().and_then(Option::as_ref)
    }

    fn check_requires(&mut self, source: &str, label: &str, requires: &Requires) {
        if let Some(range) = &requires.sdk {
            self.check_sdk(source, label, range);
        }
        for (index, tool) in requires.tools.iter().enumerate() {
            let key = format!("requires.tools[{index}]");
            let hint = tool
                .hint
                .as_ref()
                .map(|hint| format!(" Hint: {hint}"))
                .unwrap_or_default();
            if !is_plain_file_name(&tool.name) {
                self.push(
                    Some(source),
                    "pack.yaml",
                    Some(&format!("{key}.name")),
                    format!(
                        "`{}` is not a tool name: write the name of a program, with no path",
                        tool.name
                    ),
                );
                continue;
            }
            let range = match tool.version.as_deref().map(VersionRange::parse) {
                Some(Err(error)) => {
                    let key = format!("{key}.version");
                    self.push(
                        Some(source),
                        "pack.yaml",
                        Some(&key),
                        format!("tool `{}`: {error}", tool.name),
                    );
                    continue;
                }
                Some(Ok(range)) => Some(range),
                None => None,
            };
            let Some(program) = self.context.env.find(&tool.name) else {
                self.push(
                    Some(source),
                    "pack.yaml",
                    Some(&key),
                    format!(
                        "pack {label} needs the tool `{}` on the PATH, and it is not there.{hint}",
                        tool.name
                    ),
                );
                continue;
            };
            let (Some(range), Some(wanted)) = (range, tool.version.as_deref()) else {
                continue;
            };
            match self.tool_version(&tool.name, &program) {
                Err(error) => self.push(
                    Some(source),
                    "pack.yaml",
                    Some(&key),
                    format!("{error}.{hint}"),
                ),
                Ok(found) if !range.contains(&found) => self.push(
                    Some(source),
                    "pack.yaml",
                    Some(&key),
                    format!(
                        "pack {label} needs `{}` {wanted}, and `{}` is {found}.{hint}",
                        tool.name, tool.name
                    ),
                ),
                Ok(_) => {}
            }
        }
    }

    fn check_sdk(&mut self, source: &str, label: &str, wanted: &str) {
        let range = match VersionRange::parse(wanted) {
            Ok(parsed) => parsed,
            Err(error) => {
                self.push(Some(source), "pack.yaml", Some("requires.sdk"), error);
                return;
            }
        };
        let Some(sdk) = self.sdk() else {
            return;
        };
        let (text, version) = (sdk.folder_version.clone(), sdk.version);
        match version {
            None => self.push(
                Some(source),
                "pack.yaml",
                Some("requires.sdk"),
                format!("the SDK's version `{text}` is not major.minor[.patch], so `requires.sdk` cannot be checked"),
            ),
            Some(found) if !range.contains(&found) => self.push(
                Some(source),
                "pack.yaml",
                Some("requires.sdk"),
                format!("pack {label} needs the SDK `{wanted}`, and this Collide's SDK is {text} ({found})"),
            ),
            Some(_) => {}
        }
    }

    /// The version a tool's `--version` printed: run once per tool, within the timeout, with a cleared environment but
    /// for the captured `PATH`.
    fn tool_version(&mut self, name: &str, program: &Path) -> Result<Version, String> {
        if let Some(known) = self.tools.get(name) {
            return known.clone();
        }
        let result = self.run_tool(name, program);
        self.tools.insert(name.to_string(), result.clone());
        result
    }

    fn run_tool(&self, name: &str, program: &Path) -> Result<Version, String> {
        let mut command = Command::new(program);
        command
            .arg("--version")
            .env_clear()
            .env("PATH", &self.context.env.path)
            .current_dir("/");
        let captured = run_bounded(command, None, self.context.tool_timeout)
            .map_err(|detail| format!("running `{name} --version`: {detail}"))?;
        if captured.code != Some(0) {
            let code = captured
                .code
                .map_or("a signal".to_string(), |code| code.to_string());
            return Err(format!(
                "`{name} --version` exited with {code}: {}",
                tail(&captured.stderr)
            ));
        }
        if captured.stdout_cut {
            return Err(format!(
                "`{name} --version` ran, but {}",
                cut_message("stdout")
            ));
        }
        let mut text = String::from_utf8_lossy(&captured.stdout).to_string();
        text.push('\n');
        text.push_str(&String::from_utf8_lossy(&captured.stderr));
        Version::find_in(&text).ok_or_else(|| {
            // The version may be on the pipe that was cut, so an empty answer is not "printed no version".
            if captured.stderr_cut {
                return format!("`{name} --version` ran, but {}", cut_message("stderr"));
            }
            format!(
                "`{name} --version` printed no version (it printed: {})",
                tail(text.as_bytes())
            )
        })
    }
}

/// How a pack's data folder is named: the pack's name, where each `/` of a built-in name is `~`. A safe token never has
/// a `~`, so a built-in pack's folder cannot be the folder of another pack.
fn data_folder_name(pack: &str) -> Option<String> {
    let parts: Vec<&str> = pack.split('/').collect();
    if parts.len() > 2 || !parts.iter().all(|part| is_safe_token(part)) {
        return None;
    }
    Some(parts.join("~"))
}

/// The data folder a pack gets on one board, `<data_root>/<board>/<pack>` (0.7, Pack data): the folder
/// `COLLIDE_PACK_DATA` names. It is made on first use with mode `0700`, and a second call gives the same folder and
/// changes nothing in it. `board` and `pack` must be safe tokens (a built-in name such as `collide/plan` is allowed,
/// and its folder is `collide~plan`), and a symlink or a file in the place of a folder is refused. The daemon passes
/// the folder to a provider or hook in `ts-0102-vu`.
pub fn pack_data_dir(data_root: &Path, board: &str, pack: &str) -> Result<PathBuf, PackDataError> {
    if !is_safe_token(board) {
        return Err(PackDataError(format!(
            "the board id `{board}` is not a safe token"
        )));
    }
    let folder = data_folder_name(pack)
        .ok_or_else(|| PackDataError(format!("the pack name `{pack}` is not a safe token")))?;
    let board_dir = data_root.join(board);
    let dir = board_dir.join(&folder);
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(&dir)
        .map_err(|error| PackDataError(format!("cannot make `{}`: {error}", dir.display())))?;
    for made in [&board_dir, &dir] {
        let plain = fs::symlink_metadata(made).is_ok_and(|meta| meta.is_dir());
        if !plain {
            return Err(PackDataError(format!(
                "`{}` is not a folder (it is a symlink or a file)",
                made.display()
            )));
        }
    }
    Ok(dir)
}

/// Why a pack's data folder could not be given.
#[derive(Debug)]
pub struct PackDataError(String);

impl fmt::Display for PackDataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for PackDataError {}
