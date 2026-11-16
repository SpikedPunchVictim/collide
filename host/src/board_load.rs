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
//! Left to later items: trust (`ts-0097-kw` hashes [`LoadedLine::trust_inputs`]), the settings layers
//! (`ts-0098-nx`), fetching a `git:` pack (`ts-0100-pu`: until then its entry must already be in the cache), user packs
//! (`ts-0101-bd`) and the daemon's use of all this (`ts-0102-vu`).

use crate::bounded_run::{run_bounded, tail};
use crate::builtin_packs::{BuiltinPacks, Environment, is_plain_file_name};
use crate::git_objects::{GitObjects, TreeEntry};
use crate::pack_cache::{Materializer, cached_entry, sdk_version};
use collide_core::board::{Board, CollidePointer, check_board, check_pointer};
use collide_core::lock::{LockEntry, PackLock, PackSource, check_lock};
use collide_core::pack::{PackManifest, Requires};
use collide_core::pack_check::{ManifestError, check_manifest};
use collide_core::profile::{Profile, check_profile};
use collide_core::resolve::is_safe_token;
use collide_core::settings::{ConfigFile, check_config_file, repo_file_refusals};
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
/// of the pack when the problem is in one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadProblem {
    pub pack: Option<String>,
    pub file: String,
    pub key: Option<String>,
    pub message: String,
}

impl fmt::Display for LoadProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for LoadProblem {}

/// Where a pack came from. User packs and `dev:` folders join this in `ts-0101-bd`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackOrigin {
    /// In the app bundle: no pin, no cache entry, no trust (0.4).
    Builtin,
    /// `repo:`, in the line's snapshot: pinned by the tree at the ref.
    Repo,
    /// `git:`, pinned by the lock's tree.
    Git,
}

/// A file of `views/`, listed and not read: Stage 4b parses views.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewFile {
    /// The file's name, as `mine.yaml`.
    pub name: String,
    /// Its blob id.
    pub id: String,
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
        }
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
    let mut packs = load.builtin_packs(&mut names);
    if let (Some(dir), Some(board)) = (&dir, &files.board) {
        load.board_packs(dir, board, &files.lock, &mut names, &mut packs);
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

    /// Views are listed by name and blob id and not read: Stage 4b parses them and checks their CEL.
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
                Ok(PackSource::Git { .. }) => self.git_pack(dir, &entry.source, lock, names),
            };
            packs.extend(loaded);
        }
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

    /// A `git:` pack: the lock's tree is the pin. Until the mirror (`ts-0100-pu`) there is nowhere to fetch from, so
    /// its entry must already be in the cache.
    fn git_pack(
        &mut self,
        dir: &CollideDir,
        source: &str,
        lock: &PackLock,
        names: &mut Names,
    ) -> Option<LoadedPack> {
        let lock_path = dir.file("packs.lock");
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
                &lock_path,
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
            Ok(Some(entry)) => {
                self.read_pack(source, PackOrigin::Git, locked.tree.clone(), entry, names)
            }
            Ok(None) => {
                self.push(
                    Some(source),
                    &lock_path,
                    None,
                    format!(
                        "is pinned at commit {} (tree {}), which is not in the cache; fetching a `git:` pack comes \
                         with the pack mirror, so this load cannot get it",
                        locked.commit, locked.tree
                    ),
                );
                None
            }
            Err(error) => {
                self.push(Some(source), &lock_path, None, error);
                None
            }
        }
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
        self.inputs.push((format!("pack {source}"), pin.clone()));
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
        let mut text = String::from_utf8_lossy(&captured.stdout).to_string();
        text.push('\n');
        text.push_str(&String::from_utf8_lossy(&captured.stderr));
        Version::find_in(&text).ok_or_else(|| {
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
