//! The pack cache (ts-0095-kp): a pack's git tree id becomes a folder a hook can run from, by the recipe S4 measured
//! and 0.6 sets down. One code path serves every source (`repo:`, `git:`, `dev:`): each ends at a tree id and a git
//! directory to read it from.
//!
//! [`Materializer::materialize`] builds `<cache>/packs/<tree id>-<SDK version>/` ([`entry_name`]). The SDK version is
//! in the name because the entry links one SDK copy: a new SDK makes new entries, so the SDK a pack's `requires.sdk`
//! is checked against is the one its hooks import.
//!
//! - `pack/`, the files of the tree, written from git objects and read-only;
//! - `package.json`, Collide's own (`{"private":true,"type":"module"}`);
//! - `node_modules/@collide/pack-sdk`, a symlink to the SDK copy [`ensure_sdk`] made;
//! - `.collide-verified`, the marker, written last.
//!
//! The order is: a hit (the marker is there) returns at once, with no git and no hash. A miss lists the tree and
//! refuses, before any file is written, whatever could escape the folder or shadow the SDK ([`check_entries`]). It then
//! writes into `<cache>/packs/.staging-<tree id>-<pid>-<n>/`, hashes the written `pack/` again and compares it with the
//! tree id ([`CacheError::Mismatch`]), takes the write bits off and renames the staging folder into place. A loader
//! that loses the rename keeps the winner's entry. A staging folder that a dead loader left is removed on the next
//! miss when it is more than an hour old.
//!
//! The top folder of an entry gets its write bits off after the rename and not before: macOS refuses to rename a
//! folder that cannot be written. The folders and files below it are locked before.
//!
//! An entry never changes, so a cache cleaner must give write bits back before it removes one. None is built here.

use crate::git_objects::{GitError, GitObjects, ObjectKind, TreeEntry, check_id};
use std::fmt;
use std::fs;
use std::io::{self, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, SystemTime};

/// The file whose presence means an entry is complete and verified. It is written last.
pub const MARKER: &str = ".collide-verified";
/// How old a staging folder must be before the next miss removes it.
pub const STALE_STAGING: Duration = Duration::from_secs(60 * 60);

const STAGING_PREFIX: &str = ".staging-";
/// Collide's own `package.json`, so a hook's module type never depends on a `package.json` above the cache.
const PACKAGE_JSON: &[u8] = b"{\"private\":true,\"type\":\"module\"}\n";
const SDK_LINK: &str = "node_modules/@collide/pack-sdk";
/// The longest SDK version that is made into a folder name.
const MAX_VERSION_LEN: usize = 64;

/// Why a pack or the SDK could not be put in the cache.
#[derive(Debug)]
pub enum CacheError {
    /// Git could not list or read the tree, or hash the written folder. Includes the caps on files and bytes.
    Git(GitError),
    /// A path of the tree (or of the SDK) that is never written, and why.
    Refused { path: String, reason: &'static str },
    /// A file system call failed. `action` says which.
    Io {
        action: &'static str,
        path: PathBuf,
        error: io::Error,
    },
    /// The folder written hashes to another id than the tree asked for: the write changed a byte or a mode, or the
    /// volume merged two names.
    Mismatch { expected: String, actual: String },
    /// The SDK folder cannot be used.
    BadSdk { folder: PathBuf, reason: String },
    /// An entry's folder is in the cache with no marker. It is not replaced, since it is not Collide's to judge.
    Incomplete { entry: PathBuf },
}

impl fmt::Display for CacheError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CacheError::Git(error) => write!(f, "{error}"),
            CacheError::Refused { path, reason } => {
                write!(f, "refused `{}`: {reason}", printable(path))
            }
            CacheError::Io {
                action,
                path,
                error,
            } => write!(f, "cannot {action} `{}`: {error}", path.display()),
            CacheError::Mismatch { expected, actual } => write!(
                f,
                "the files written for tree {expected} hash to {actual}: a write changed a byte or a mode, or the \
                 volume merged two names"
            ),
            CacheError::BadSdk { folder, reason } => {
                write!(f, "the SDK at `{}`: {reason}", folder.display())
            }
            CacheError::Incomplete { entry } => write!(
                f,
                "`{}` is in the cache with no `{MARKER}` marker; remove it and load again",
                entry.display()
            ),
        }
    }
}

impl std::error::Error for CacheError {}

/// A path of a hand-made tree can hold control characters, which a terminal would act on. They are shown escaped.
fn printable(path: &str) -> String {
    path.chars()
        .flat_map(|c| {
            if c.is_control() {
                c.escape_debug().collect::<Vec<char>>()
            } else {
                vec![c]
            }
        })
        .collect()
}

impl From<GitError> for CacheError {
    fn from(error: GitError) -> CacheError {
        CacheError::Git(error)
    }
}

fn io_error(action: &'static str, path: &Path) -> impl FnOnce(io::Error) -> CacheError {
    let path = path.to_path_buf();
    move |error| CacheError::Io {
        action,
        path,
        error,
    }
}

/// Writes one file of a pack. The materializer re-hashes what was written, so a writer that fails to write what it was
/// given is caught there. It is a trait so a test can hand over a writer that does.
pub trait PackWriter: Send + Sync {
    /// Creates `path`, which must not be there yet, with `bytes` and exactly `mode`.
    fn write_file(&self, path: &Path, bytes: &[u8], mode: u32) -> io::Result<()>;
}

/// The writer the daemon uses. The file is created new, so a name that the volume already holds (a case-insensitive
/// volume that has `Foo` when `foo` is written) is an error and never an overwrite.
pub struct FsWriter;

impl PackWriter for FsWriter {
    fn write_file(&self, path: &Path, bytes: &[u8], mode: u32) -> io::Result<()> {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(mode)
            .open(path)?;
        file.write_all(bytes)?;
        // The mode given to `open` is cut by the process's umask; this one is not.
        file.set_permissions(fs::Permissions::from_mode(mode))
    }
}

/// Refuses a tree that must never be written, naming the first path (in path order) and why. Nothing is read from the
/// repository for this: it is over what `list` gave.
pub fn check_entries(entries: &[TreeEntry]) -> Result<(), CacheError> {
    for entry in entries {
        if let Some(reason) = refusal(entry) {
            return Err(CacheError::Refused {
                path: entry.path.clone(),
                reason,
            });
        }
    }
    Ok(())
}

fn refusal(entry: &TreeEntry) -> Option<&'static str> {
    let parts: Vec<&str> = entry.path.split('/').collect();
    for part in &parts {
        if part.is_empty() {
            return Some("it has an empty part");
        }
        if *part == "." || *part == ".." {
            return Some("it has a `.` or `..` part");
        }
        if part.eq_ignore_ascii_case(".git") {
            return Some("it has a `.git` part");
        }
        if part.contains('\\') {
            return Some("it has a backslash");
        }
        if part.contains('\0') {
            return Some("it has a NUL");
        }
        if part.eq_ignore_ascii_case("node_modules") {
            return Some("it is under `node_modules`, which would shadow Collide's SDK");
        }
    }
    if parts
        .last()
        .is_some_and(|name| name.eq_ignore_ascii_case("package.json"))
    {
        return Some("Collide writes its own `package.json`, and Node uses the nearest one");
    }
    match (entry.mode.as_str(), entry.kind) {
        ("100644" | "100755", ObjectKind::Blob) => None,
        ("120000", _) => Some("it is a symlink"),
        ("160000", _) => Some("it is a submodule"),
        _ => Some("its mode is not that of a file"),
    }
}

/// Builds cache entries. It holds how git is run and how files are written.
pub struct Materializer {
    git: GitObjects,
    writer: Box<dyn PackWriter>,
}

impl Default for Materializer {
    fn default() -> Materializer {
        Materializer::new()
    }
}

impl Materializer {
    /// Git on the process's own `PATH`, and the real writer.
    pub fn new() -> Materializer {
        Materializer {
            git: GitObjects::new(),
            writer: Box::new(FsWriter),
        }
    }

    pub fn with_git(mut self, git: GitObjects) -> Materializer {
        self.git = git;
        self
    }

    pub fn with_writer(mut self, writer: Box<dyn PackWriter>) -> Materializer {
        self.writer = writer;
        self
    }

    /// The entry folder for `tree` with the SDK `sdk` (the folder [`ensure_sdk`] returned). The tree is read from the
    /// git directory `repo` when its entry is not there yet.
    pub fn materialize(
        &self,
        cache: &Path,
        sdk: &Path,
        repo: &Path,
        tree: &str,
    ) -> Result<PathBuf, CacheError> {
        check_id("tree id", tree)?;
        let sdk = absolute_sdk(sdk)?;
        let packs = cache.join("packs");
        let entry = packs.join(entry_name(tree, &sdk_version(&sdk)?));
        if is_complete(&entry) {
            return Ok(entry);
        }
        let files = self.git.list(repo, tree)?;
        check_entries(&files)?;

        make_dirs(&packs)?;
        sweep_stale_staging(&packs);
        let staging = Staging::new(&packs, tree);
        self.build(staging.path(), repo, &files, &sdk, tree)?;
        let actual = self.git.tree_of_folder(&staging.path().join("pack"))?;
        if actual != tree {
            return Err(CacheError::Mismatch {
                expected: tree.to_string(),
                actual,
            });
        }
        publish(staging.path(), &entry)?;
        Ok(entry)
    }

    /// Writes the entry's files below `staging`, the marker last.
    fn build(
        &self,
        staging: &Path,
        repo: &Path,
        files: &[TreeEntry],
        sdk: &Path,
        tree: &str,
    ) -> Result<(), CacheError> {
        let pack = staging.join("pack");
        make_dirs(&pack)?;
        let ids: Vec<String> = files.iter().map(|file| file.id.clone()).collect();
        let bodies = self.git.read_blobs(repo, &ids)?;
        for (file, bytes) in files.iter().zip(&bodies) {
            let rel = Path::new(&file.path);
            // `check_entries` has refused these already; a path that leaves `pack/` must never be joined.
            if !rel
                .components()
                .all(|part| matches!(part, Component::Normal(_)))
            {
                return Err(CacheError::Refused {
                    path: file.path.clone(),
                    reason: "it is not a path below the pack",
                });
            }
            let dest = pack.join(rel);
            if let Some(parent) = dest.parent() {
                make_dirs(parent)?;
            }
            let mode = if file.mode == "100755" { 0o755 } else { 0o644 };
            self.writer
                .write_file(&dest, bytes, mode)
                .map_err(io_error("write", &dest))?;
        }
        let package_json = staging.join("package.json");
        fs::write(&package_json, PACKAGE_JSON).map_err(io_error("write", &package_json))?;
        let link = staging.join(SDK_LINK);
        if let Some(parent) = link.parent() {
            make_dirs(parent)?;
        }
        std::os::unix::fs::symlink(sdk, &link).map_err(io_error("link the SDK at", &link))?;
        let marker = staging.join(MARKER);
        fs::write(&marker, format!("{tree}\n")).map_err(io_error("write", &marker))
    }
}

/// [`Materializer::materialize`] with git on the process's `PATH` and the real writer.
pub fn materialize(
    cache: &Path,
    sdk: &Path,
    repo: &Path,
    tree: &str,
) -> Result<PathBuf, CacheError> {
    Materializer::new().materialize(cache, sdk, repo, tree)
}

/// Copies the compiled SDK the app bundle holds (`bundle_sdk`, found by `pack_location::bundle_sdk`) into
/// `<cache>/sdk/<version>/`, once, read-only and with its own marker, and returns that folder. The version is the
/// `version` of the SDK's `package.json`. An entry is outside the app bundle, so a pack entry's symlink does not point
/// into an app that an update replaces. A folder that is already complete is not read or written again, so a build
/// whose contents change without a new version is not copied (a new version is its own folder).
pub fn ensure_sdk(cache: &Path, bundle_sdk: &Path) -> Result<PathBuf, CacheError> {
    let version = sdk_version(bundle_sdk)?;
    let sdks = cache.join("sdk");
    let entry = sdks.join(&version);
    if is_complete(&entry) {
        return Ok(entry);
    }
    make_dirs(&sdks)?;
    sweep_stale_staging(&sdks);
    let staging = Staging::new(&sdks, &version);
    make_dirs(staging.path())?;
    copy_tree(bundle_sdk, staging.path(), "")?;
    let marker = staging.path().join(MARKER);
    fs::write(&marker, format!("{version}\n")).map_err(io_error("write", &marker))?;
    publish(staging.path(), &entry)?;
    Ok(entry)
}

/// The name of a pack's entry folder under `<cache>/packs/`: the tree id and the version of the SDK it links.
pub fn entry_name(tree: &str, sdk_version: &str) -> String {
    format!("{tree}-{sdk_version}")
}

/// The entry of `tree` for the SDK version `sdk_version`, when it is already in the cache and complete: no git, no
/// hash, no write. `None` when it is not there. A `git:` pack that is "in the cache" is this entry.
pub fn cached_entry(
    cache: &Path,
    tree: &str,
    sdk_version: &str,
) -> Result<Option<PathBuf>, CacheError> {
    check_id("tree id", tree)?;
    let entry = cache.join("packs").join(entry_name(tree, sdk_version));
    Ok(is_complete(&entry).then_some(entry))
}

/// The SDK folder as the entry's symlink will name it: absolute, and a folder with a `package.json`.
fn absolute_sdk(sdk: &Path) -> Result<PathBuf, CacheError> {
    let bad = |reason: String| CacheError::BadSdk {
        folder: sdk.to_path_buf(),
        reason,
    };
    if !sdk.join("package.json").is_file() {
        return Err(bad("it is not a folder with a package.json".to_string()));
    }
    std::path::absolute(sdk).map_err(|error| bad(error.to_string()))
}

/// The `version` of `<sdk>/package.json`, checked to be safe as a folder name. It is the version a pack's
/// `requires.sdk` is checked against.
pub fn sdk_version(sdk: &Path) -> Result<String, CacheError> {
    let bad = |reason: String| CacheError::BadSdk {
        folder: sdk.to_path_buf(),
        reason,
    };
    let path = sdk.join("package.json");
    let bytes =
        fs::read(&path).map_err(|error| bad(format!("cannot read package.json: {error}")))?;
    let json: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|error| bad(format!("package.json is not JSON: {error}")))?;
    let version = json
        .get("version")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| bad("package.json has no `version` string".to_string()))?;
    let safe = !version.is_empty()
        && version.len() <= MAX_VERSION_LEN
        && version.starts_with(|c: char| c.is_ascii_alphanumeric())
        && version
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '+' | '_'));
    if safe {
        Ok(version.to_string())
    } else {
        Err(bad(format!(
            "the version {version:?} is not usable as a folder name"
        )))
    }
}

/// Copies the files and folders of `from` into `to`, which exists. Anything else (a symlink, a socket) is refused,
/// naming its path below `from`: the bundle is built with its links followed.
fn copy_tree(from: &Path, to: &Path, rel: &str) -> Result<(), CacheError> {
    for item in fs::read_dir(from).map_err(io_error("read", from))? {
        let item = item.map_err(io_error("read", from))?;
        let name = item.file_name().to_string_lossy().to_string();
        let rel = if rel.is_empty() {
            name.clone()
        } else {
            format!("{rel}/{name}")
        };
        let (source, dest) = (item.path(), to.join(&name));
        let kind = item.file_type().map_err(io_error("read", &source))?;
        if kind.is_dir() {
            make_dirs(&dest)?;
            copy_tree(&source, &dest, &rel)?;
        } else if kind.is_file() {
            fs::copy(&source, &dest).map_err(io_error("copy", &source))?;
            let executable = fs::metadata(&source)
                .map_err(io_error("read", &source))?
                .permissions()
                .mode()
                & 0o111
                != 0;
            set_mode(&dest, if executable { 0o755 } else { 0o644 })?;
        } else {
            return Err(CacheError::Refused {
                path: rel,
                reason: "it is not a file or a folder, and the SDK copy takes only those",
            });
        }
    }
    Ok(())
}

fn is_complete(entry: &Path) -> bool {
    entry.join(MARKER).is_file()
}

fn make_dirs(path: &Path) -> Result<(), CacheError> {
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o755)
        .create(path)
        .map_err(io_error("make the folder", path))
}

fn set_mode(path: &Path, mode: u32) -> Result<(), CacheError> {
    fs::set_permissions(path, fs::Permissions::from_mode(mode))
        .map_err(io_error("set the mode of", path))
}

/// Takes the write bits off everything below `dir`, children first: files `0444` or `0555` (by the executable bit),
/// folders `0555`. A symlink is left alone, since a mode change would follow it. `dir` itself is not changed.
fn lock_below(dir: &Path) -> Result<(), CacheError> {
    for item in fs::read_dir(dir).map_err(io_error("read", dir))? {
        let path = item.map_err(io_error("read", dir))?.path();
        let meta = fs::symlink_metadata(&path).map_err(io_error("read", &path))?;
        if meta.is_dir() {
            lock_below(&path)?;
            set_mode(&path, 0o555)?;
        } else if meta.is_file() {
            let executable = meta.permissions().mode() & 0o111 != 0;
            set_mode(&path, if executable { 0o555 } else { 0o444 })?;
        }
    }
    Ok(())
}

/// Locks `staging` and renames it to `entry`. A rename that fails because another loader's `entry` is already there
/// and complete is not an error: the winner's entry stands, and the caller's staging folder is removed when its guard
/// drops.
fn publish(staging: &Path, entry: &Path) -> Result<(), CacheError> {
    lock_below(staging)?;
    match fs::rename(staging, entry) {
        Ok(()) => set_mode(entry, 0o555),
        Err(_) if is_complete(entry) => Ok(()),
        Err(_) if entry.exists() => Err(CacheError::Incomplete {
            entry: entry.to_path_buf(),
        }),
        Err(error) => Err(io_error("rename into place", entry)(error)),
    }
}

static STAGING_COUNTER: AtomicU32 = AtomicU32::new(0);

/// A staging folder's path, removed when dropped. After a rename into place there is nothing left to remove. The name
/// holds the pid and a counter, so two threads of one process never share a folder.
struct Staging(PathBuf);

impl Staging {
    fn new(parent: &Path, label: &str) -> Staging {
        let n = STAGING_COUNTER.fetch_add(1, Ordering::SeqCst);
        let name = format!("{STAGING_PREFIX}{label}-{}-{n}", std::process::id());
        Staging(parent.join(name))
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Staging {
    fn drop(&mut self) {
        remove_tree(&self.0);
    }
}

/// Removes a folder that may be read-only, by its own path. Folders are made writable first; a symlink is never
/// followed. It cleans up after a failed or lost build, so a failure to remove is not reported: the next miss
/// removes the folder when it is old.
fn remove_tree(path: &Path) {
    open_folders(path);
    let _ = fs::remove_dir_all(path);
}

fn open_folders(path: &Path) {
    let Ok(meta) = fs::symlink_metadata(path) else {
        return;
    };
    if !meta.is_dir() {
        return;
    }
    let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o755));
    if let Ok(items) = fs::read_dir(path) {
        for item in items.flatten() {
            open_folders(&item.path());
        }
    }
}

/// Removes the staging folders in `dir` that are more than [`STALE_STAGING`] old. A younger one may be a loader that
/// is still running. It is housekeeping for other loaders' leftovers, so a failure is not reported.
fn sweep_stale_staging(dir: &Path) {
    let Ok(items) = fs::read_dir(dir) else {
        return;
    };
    for item in items.flatten() {
        if !item
            .file_name()
            .to_string_lossy()
            .starts_with(STAGING_PREFIX)
        {
            continue;
        }
        let Ok(meta) = item.metadata() else {
            continue;
        };
        let age = meta
            .modified()
            .ok()
            .and_then(|modified| SystemTime::now().duration_since(modified).ok());
        if meta.is_dir() && age.is_some_and(|age| age > STALE_STAGING) {
            remove_tree(&item.path());
        }
    }
}
