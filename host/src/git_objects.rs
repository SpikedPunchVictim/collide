//! Git objects, read (ts-0094-av). Everything Stage 4a loads (a pack, `board.yaml`, `config.yaml`) is read from git
//! objects at a ref, never from a working tree (0.6, 0.7). This module answers four questions about a repository:
//!
//! - [`GitObjects::tree_at`]: the tree id of `<rev>:<path>`, which is a pack's pin;
//! - [`GitObjects::list`]: the files in a tree, with their modes as git writes them;
//! - [`GitObjects::read_blobs`] and [`GitObjects::blob_at`]: what files hold;
//! - [`GitObjects::tree_of_folder`]: the id git would give a folder's files, for `dev:` packs and the materializer's
//!   re-hash.
//!
//! The `repo` argument is a git directory: the `.git` of a checkout, or a bare mirror. Nothing here reads a working
//! tree. Every call goes through [`GitObjects::command`], so one place decides how git is run: a cleared
//! environment, the user's git config switched off, replace refs ignored, and a timeout. The free functions at the
//! end use the process's own `PATH`; a [`GitObjects`] built with [`GitObjects::with_path`] is for tests that put a
//! different `git` first.
//!
//! This is not for fetching. A `git:` fetch needs the user's credentials (ts-0100-pu) and builds its own
//! environment.

use crate::bounded_run::{Captured, run_bounded, tail};
use collide_core::yaml::MAX_YAML_BYTES;
use std::ffi::OsString;
use std::fmt;
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// The most files a tree may hold. S4's packs had 184; a line's `.collide/` folder is listed too.
pub const MAX_TREE_ENTRIES: usize = 2_000;
/// The largest blob [`GitObjects::list`] and [`GitObjects::read_blobs`] accept.
pub const MAX_BLOB_BYTES: u64 = 8 * 1024 * 1024;
/// The most bytes all the blobs of one tree may hold, so a tree cannot fill the cache.
pub const MAX_TREE_BYTES: u64 = 32 * 1024 * 1024;
/// How long one git process may run.
pub const GIT_TIMEOUT: Duration = Duration::from_secs(30);

/// Ids sent to one `cat-file --batch` process. A chunk bounds how many blobs one reply holds in memory before they
/// are checked. `run_bounded` writes the input while it reads the output, so the chunk is not limited by a pipe's size.
const CAT_FILE_CHUNK: usize = 200;

/// The id of the empty tree, which git knows without storing it. `tree_of_folder` reads no attributes from the
/// folder: it names this tree as the place they come from, and it has none.
const EMPTY_TREE_SHA1: &str = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";

/// What kind of object an id names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectKind {
    Blob,
    Tree,
    /// A submodule's commit, which `ls-tree` lists in the place of a folder.
    Commit,
}

impl ObjectKind {
    fn parse(word: &str) -> Option<ObjectKind> {
        match word {
            "blob" => Some(ObjectKind::Blob),
            "tree" => Some(ObjectKind::Tree),
            "commit" => Some(ObjectKind::Commit),
            _ => None,
        }
    }

    fn name(self) -> &'static str {
        match self {
            ObjectKind::Blob => "blob",
            ObjectKind::Tree => "tree",
            ObjectKind::Commit => "commit",
        }
    }
}

/// What `<rev>:<path>` is: its id and whether it is a folder or a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeAt {
    pub id: String,
    pub kind: ObjectKind,
}

/// One file of a tree. `mode` is as git writes it: `100644`, `100755`, `120000` (a symlink) or `160000` (a submodule).
/// `size` is the blob's, and 0 for a submodule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeEntry {
    pub mode: String,
    pub kind: ObjectKind,
    pub id: String,
    pub path: String,
    pub size: u64,
}

/// Why an object could not be read. Each variant names what it was about.
#[derive(Debug)]
pub enum GitError {
    /// An argument that git was never given: a `rev` that starts with `-`, a path that leaves the tree, an id that is
    /// not hex.
    Refused {
        what: &'static str,
        value: String,
        reason: &'static str,
    },
    /// `git` did not start, or did not end within the timeout. `detail` says which.
    Run {
        command: String,
        detail: String,
    },
    /// `git` ended with an error. `stderr` is the tail of what it said.
    Failed {
        command: String,
        code: Option<i32>,
        stderr: String,
    },
    /// `git` ended well and said something this module does not know how to read.
    BadReply {
        command: String,
        detail: String,
    },
    NoSuchRev {
        rev: String,
    },
    NoSuchPath {
        rev: String,
        path: String,
    },
    NotABlob {
        path: String,
        kind: ObjectKind,
    },
    MissingObject {
        id: String,
    },
    WrongObject {
        id: String,
        kind: String,
    },
    /// A path that is not UTF-8, named with its bad bytes replaced.
    NotUtf8 {
        path: String,
    },
    TooManyEntries {
        count: usize,
        cap: usize,
    },
    /// `name` is a path where one is known and an id where not.
    BlobTooLarge {
        name: String,
        size: u64,
        cap: u64,
    },
    TreeTooLarge {
        size: u64,
        cap: u64,
    },
    /// A folder that could not be hashed: it is not there, it is not a folder, or the scratch folder failed.
    Folder {
        folder: String,
        reason: String,
    },
}

impl fmt::Display for GitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GitError::Refused {
                what,
                value,
                reason,
            } => write!(f, "refused the {what} {value:?}: {reason}"),
            GitError::Run { command, detail } => {
                write!(f, "`{command}` did not run to its end: {detail}")
            }
            GitError::Failed {
                command,
                code,
                stderr,
            } => {
                let code = code.map_or("no exit code".to_string(), |code| {
                    format!("exit code {code}")
                });
                write!(f, "`{command}` failed ({code}): {stderr}")
            }
            GitError::BadReply { command, detail } => {
                write!(f, "`{command}` gave a reply that cannot be read: {detail}")
            }
            GitError::NoSuchRev { rev } => write!(f, "the repository has no commit `{rev}`"),
            GitError::NoSuchPath { rev, path } => write!(
                f,
                "`{path}` is not in `{rev}` (a submodule has no object here either)"
            ),
            GitError::NotABlob { path, kind } => {
                write!(f, "`{path}` is a {}, not a file", kind.name())
            }
            GitError::MissingObject { id } => write!(f, "the repository has no object `{id}`"),
            GitError::WrongObject { id, kind } => write!(f, "`{id}` is a {kind}, not a file"),
            GitError::NotUtf8 { path } => write!(f, "the path `{path}` is not UTF-8"),
            GitError::TooManyEntries { count, cap } => {
                write!(f, "the tree has {count} files, over the cap of {cap}")
            }
            GitError::BlobTooLarge { name, size, cap } => {
                write!(f, "`{name}` is {size} bytes, over the cap of {cap}")
            }
            GitError::TreeTooLarge { size, cap } => {
                write!(
                    f,
                    "the tree's files hold {size} bytes, over the cap of {cap}"
                )
            }
            GitError::Folder { folder, reason } => write!(f, "cannot hash `{folder}`: {reason}"),
        }
    }
}

impl std::error::Error for GitError {}

/// The reader. It holds only how git is found and how long it may run.
#[derive(Debug, Clone)]
pub struct GitObjects {
    path: OsString,
    timeout: Duration,
    scratch: Option<PathBuf>,
}

impl Default for GitObjects {
    fn default() -> GitObjects {
        GitObjects::new()
    }
}

impl GitObjects {
    /// A reader that finds `git` on the process's own `PATH`.
    pub fn new() -> GitObjects {
        GitObjects {
            path: std::env::var_os("PATH").unwrap_or_else(|| "/usr/bin:/bin".into()),
            timeout: GIT_TIMEOUT,
            scratch: None,
        }
    }

    /// Finds `git` on `path` instead, and gives it that `PATH`.
    pub fn with_path(mut self, path: impl Into<OsString>) -> GitObjects {
        self.path = path.into();
        self
    }

    pub fn with_timeout(mut self, timeout: Duration) -> GitObjects {
        self.timeout = timeout;
        self
    }

    /// The folder [`GitObjects::tree_of_folder`] makes its temporary folder in. The default is the system's.
    pub fn with_scratch_dir(mut self, dir: impl Into<PathBuf>) -> GitObjects {
        self.scratch = Some(dir.into());
        self
    }

    /// The tree id of `<rev>:<path>`, and whether it is a folder or a file. `rev` is resolved to a commit first, then
    /// the path in that commit. An empty `path` is the commit's own tree.
    pub fn tree_at(&self, repo: &Path, rev: &str, path: &str) -> Result<TreeAt, GitError> {
        check_rev(rev)?;
        check_path(path)?;
        let commit = self.commit_of(repo, rev)?;
        let found = self.resolve(repo, &commit, rev, path)?;
        Ok(TreeAt {
            id: found.id,
            kind: found.kind,
        })
    }

    /// The files of `tree` (a full hex id), at every depth, sorted by path. Refused when the tree has more than
    /// [`MAX_TREE_ENTRIES`] files, when a blob is over [`MAX_BLOB_BYTES`], when the blobs together are over
    /// [`MAX_TREE_BYTES`], or when a path is not UTF-8.
    pub fn list(&self, repo: &Path, tree: &str) -> Result<Vec<TreeEntry>, GitError> {
        check_id("tree id", tree)?;
        let mut command = self.command(repo);
        command.args(["ls-tree", "-r", "-z", "-l", "--full-tree", tree]);
        let out = self.run_ok(command, "git ls-tree", None)?;
        let records: Vec<&[u8]> = out
            .split(|byte| *byte == 0)
            .filter(|record| !record.is_empty())
            .collect();
        if records.len() > MAX_TREE_ENTRIES {
            return Err(GitError::TooManyEntries {
                count: records.len(),
                cap: MAX_TREE_ENTRIES,
            });
        }
        let mut entries = Vec::with_capacity(records.len());
        let mut total = 0u64;
        for record in records {
            let entry = parse_entry(record)?;
            if entry.size > MAX_BLOB_BYTES {
                return Err(GitError::BlobTooLarge {
                    name: entry.path,
                    size: entry.size,
                    cap: MAX_BLOB_BYTES,
                });
            }
            total += entry.size;
            entries.push(entry);
        }
        if total > MAX_TREE_BYTES {
            return Err(GitError::TreeTooLarge {
                size: total,
                cap: MAX_TREE_BYTES,
            });
        }
        entries.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(entries)
    }

    /// What the blobs `ids` hold, in the order asked. One `cat-file --batch` process answers each chunk of ids.
    /// A missing object, an id that is not a blob and a blob over [`MAX_BLOB_BYTES`] are errors naming the id. The
    /// caller's ids should come from [`GitObjects::list`], which caps the sizes before anything is read: the reply of
    /// one chunk is held in memory.
    pub fn read_blobs(&self, repo: &Path, ids: &[String]) -> Result<Vec<Vec<u8>>, GitError> {
        for id in ids {
            check_id("object id", id)?;
        }
        let mut bodies = Vec::with_capacity(ids.len());
        for chunk in ids.chunks(CAT_FILE_CHUNK) {
            let mut input = chunk.join("\n");
            input.push('\n');
            let mut command = self.command(repo);
            command.args(["cat-file", "--batch"]);
            let out = self.run_ok(command, "git cat-file", Some(input.as_bytes()))?;
            read_batch(&out, chunk, &mut bodies)?;
        }
        Ok(bodies)
    }

    /// The file at `<rev>:<path>`: `None` when the path is not there, an error when it is a folder or is over the YAML
    /// size cap ([`MAX_YAML_BYTES`]). It is for `board.yaml`, `config.yaml` and the like.
    pub fn blob_at(&self, repo: &Path, rev: &str, path: &str) -> Result<Option<Vec<u8>>, GitError> {
        check_rev(rev)?;
        check_path(path)?;
        let commit = self.commit_of(repo, rev)?;
        let found = match self.resolve(repo, &commit, rev, path) {
            Ok(found) => found,
            Err(GitError::NoSuchPath { .. }) => return Ok(None),
            Err(error) => return Err(error),
        };
        if found.kind != ObjectKind::Blob {
            return Err(GitError::NotABlob {
                path: path.to_string(),
                kind: found.kind,
            });
        }
        if found.size > MAX_YAML_BYTES as u64 {
            return Err(GitError::BlobTooLarge {
                name: path.to_string(),
                size: found.size,
                cap: MAX_YAML_BYTES as u64,
            });
        }
        let mut bodies = self.read_blobs(repo, &[found.id])?;
        Ok(bodies.pop())
    }

    /// The commit `rev` names, as a full id. A line is loaded at this id, so a ref that moves during a load cannot
    /// give one load two snapshots.
    pub fn resolve_commit(&self, repo: &Path, rev: &str) -> Result<String, GitError> {
        check_rev(rev)?;
        self.commit_of(repo, rev)
    }

    /// The entry at `<rev>:<path>` with its mode, so a symlink is told from a file: `None` when the path is not
    /// there. `path` is not empty (the commit's own tree has no entry; use [`GitObjects::tree_at`]), and does not
    /// start with `:`, which `ls-tree` would read as pathspec magic.
    pub fn entry_at(
        &self,
        repo: &Path,
        rev: &str,
        path: &str,
    ) -> Result<Option<TreeEntry>, GitError> {
        check_rev(rev)?;
        check_path(path)?;
        if path.is_empty() {
            return Err(refused(
                "path",
                path,
                "it is empty; the commit's own tree has no entry",
            ));
        }
        if path.starts_with(':') {
            return Err(refused("path", path, "it starts with `:`"));
        }
        let commit = self.commit_of(repo, rev)?;
        let mut command = self.command(repo);
        command
            .env("GIT_LITERAL_PATHSPECS", "1")
            .args(["ls-tree", "-z", "-l", "--full-tree", &commit, "--"])
            .arg(path);
        let out = self.run_ok(command, "git ls-tree", None)?;
        let mut records = out
            .split(|byte| *byte == 0)
            .filter(|record| !record.is_empty());
        let Some(record) = records.next() else {
            return Ok(None);
        };
        let entry = parse_entry(record)?;
        if records.next().is_some() || entry.path != path {
            return Err(GitError::BadReply {
                command: "git ls-tree".to_string(),
                detail: format!("asked for {path:?}, got more than that"),
            });
        }
        Ok(Some(entry))
    }

    /// The entries directly in `tree` (a full hex id), one level, sorted by name. Refused above
    /// [`MAX_TREE_ENTRIES`] entries. A blob's size is as `ls-tree` gives it and is not capped here.
    pub fn list_dir(&self, repo: &Path, tree: &str) -> Result<Vec<TreeEntry>, GitError> {
        check_id("tree id", tree)?;
        let mut command = self.command(repo);
        command.args(["ls-tree", "-z", "-l", "--full-tree", tree]);
        let out = self.run_ok(command, "git ls-tree", None)?;
        let records: Vec<&[u8]> = out
            .split(|byte| *byte == 0)
            .filter(|record| !record.is_empty())
            .collect();
        if records.len() > MAX_TREE_ENTRIES {
            return Err(GitError::TooManyEntries {
                count: records.len(),
                cap: MAX_TREE_ENTRIES,
            });
        }
        let mut entries = records
            .into_iter()
            .map(parse_entry)
            .collect::<Result<Vec<_>, _>>()?;
        entries.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(entries)
    }

    /// The tree id git would give the files in `folder`, computed by git in a temporary bare repository that is
    /// removed afterwards. The folder's own repository, config, ignore files and attributes are never consulted, and
    /// an ignored file or a `node_modules` folder is counted. Empty folders are absent, as git has them. The id is a
    /// SHA-1 id.
    pub fn tree_of_folder(&self, folder: &Path) -> Result<String, GitError> {
        let folder_error = |reason: String| GitError::Folder {
            folder: folder.display().to_string(),
            reason,
        };
        match std::fs::metadata(folder) {
            Ok(meta) if meta.is_dir() => {}
            Ok(_) => return Err(folder_error("it is not a folder".to_string())),
            Err(error) => return Err(folder_error(error.to_string())),
        }
        let parent = self.scratch.clone().unwrap_or_else(std::env::temp_dir);
        let scratch = ScratchFolder::make(&parent).map_err(|error| {
            folder_error(format!(
                "cannot make a temporary folder in `{}`: {error}",
                parent.display()
            ))
        })?;
        let bare = scratch.path().join("bare");
        let index = bare.join("index");

        let mut init = self.command_without_repo();
        init.args([
            "-c",
            "init.defaultBranch=main",
            "init",
            "--bare",
            "--quiet",
            "--object-format=sha1",
            "--template=",
        ])
        .arg(&bare);
        self.run_ok(init, "git init", None)?;

        // The settings that decide what an add stores are given here, so the answer never depends on what `init`
        // found about the file system. `--attr-source` names an empty tree as the place attributes come from, so a
        // `.gitattributes` in the folder (an `eol` rule, `ident`, an encoding) cannot change a byte that is hashed.
        let mut add = self.command(&bare);
        add.env("GIT_WORK_TREE", folder)
            .env("GIT_INDEX_FILE", &index)
            .args([
                "-c",
                "core.filemode=true",
                "-c",
                "core.symlinks=true",
                "-c",
                "core.ignorecase=false",
                "-c",
                "core.precomposeunicode=false",
                "-c",
                "core.autocrlf=false",
                "--attr-source",
                EMPTY_TREE_SHA1,
                "add",
                "--all",
                "--force",
            ]);
        self.run_ok(add, "git add", None)?;

        let mut write_tree = self.command(&bare);
        write_tree.env("GIT_INDEX_FILE", &index).arg("write-tree");
        let out = self.run_ok(write_tree, "git write-tree", None)?;
        let id = String::from_utf8_lossy(&out).trim().to_string();
        check_id("tree id", &id).map_err(|_| GitError::BadReply {
            command: "git write-tree".to_string(),
            detail: format!("expected a tree id, got {id:?}"),
        })?;
        Ok(id)
    }

    /// `git` for a repository: the one place that decides how git is run. The environment is cleared and these are
    /// set: `PATH`; `LC_ALL=C`; no prompt; no optional locks, so a read never writes; no replace refs, so a replace
    /// ref cannot change the content under a tree id; and no global or system config or attributes, so the user's git
    /// config cannot change what is read. `HOME` is not set, so no file below it is found either.
    fn command(&self, repo: &Path) -> Command {
        let mut command = self.command_without_repo();
        command.env("GIT_DIR", repo);
        command
    }

    fn command_without_repo(&self) -> Command {
        let mut command = Command::new("git");
        command
            .env_clear()
            .env("PATH", &self.path)
            .env("LC_ALL", "C")
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GIT_OPTIONAL_LOCKS", "0")
            .env("GIT_NO_REPLACE_OBJECTS", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_SYSTEM", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_ATTR_NOSYSTEM", "1");
        command
    }

    /// Runs `command` within the timeout and returns what it printed, whatever its exit code was.
    fn run(
        &self,
        command: Command,
        name: &str,
        stdin: Option<&[u8]>,
    ) -> Result<Captured, GitError> {
        run_bounded(command, stdin, self.timeout).map_err(|detail| GitError::Run {
            command: name.to_string(),
            detail,
        })
    }

    /// Like [`GitObjects::run`], and an exit code other than 0 is a [`GitError::Failed`].
    fn run_ok(
        &self,
        command: Command,
        name: &str,
        stdin: Option<&[u8]>,
    ) -> Result<Vec<u8>, GitError> {
        let captured = self.run(command, name, stdin)?;
        if captured.code == Some(0) {
            Ok(captured.stdout)
        } else {
            Err(failed(name, &captured))
        }
    }

    /// The commit `rev` names, as a full id.
    fn commit_of(&self, repo: &Path, rev: &str) -> Result<String, GitError> {
        let mut command = self.command(repo);
        command
            .args(["rev-parse", "--verify", "--quiet", "--end-of-options"])
            .arg(format!("{rev}^{{commit}}"));
        let captured = self.run(command, "git rev-parse", None)?;
        match captured.code {
            Some(0) => {
                let id = String::from_utf8_lossy(&captured.stdout).trim().to_string();
                check_id("commit id", &id).map_err(|_| GitError::BadReply {
                    command: "git rev-parse".to_string(),
                    detail: format!("expected a commit id, got {id:?}"),
                })?;
                Ok(id)
            }
            // `--quiet` makes a revision that does not name a commit exit 1 without a message.
            Some(1) => Err(GitError::NoSuchRev {
                rev: rev.to_string(),
            }),
            _ => Err(failed("git rev-parse", &captured)),
        }
    }

    /// What `<commit>:<path>` is. The name goes to git on its standard input, so it is never an option.
    fn resolve(
        &self,
        repo: &Path,
        commit: &str,
        rev: &str,
        path: &str,
    ) -> Result<Resolved, GitError> {
        let mut command = self.command(repo);
        command.args(["cat-file", "--batch-check"]);
        let input = format!("{commit}:{path}\n");
        let out = self.run_ok(command, "git cat-file", Some(input.as_bytes()))?;
        let line = String::from_utf8_lossy(&out).trim_end().to_string();
        let words: Vec<&str> = line.split(' ').collect();
        let bad = |detail: String| GitError::BadReply {
            command: "git cat-file".to_string(),
            detail,
        };
        match words.as_slice() {
            [_, "missing"] => Err(GitError::NoSuchPath {
                rev: rev.to_string(),
                path: path.to_string(),
            }),
            [id, kind, size] => {
                let kind = ObjectKind::parse(kind).ok_or_else(|| bad(line.clone()))?;
                let size = size.parse().map_err(|_| bad(line.clone()))?;
                check_id("object id", id).map_err(|_| bad(line.clone()))?;
                Ok(Resolved {
                    id: id.to_string(),
                    kind,
                    size,
                })
            }
            _ => Err(bad(line.clone())),
        }
    }
}

struct Resolved {
    id: String,
    kind: ObjectKind,
    size: u64,
}

fn failed(name: &str, captured: &Captured) -> GitError {
    GitError::Failed {
        command: name.to_string(),
        code: captured.code,
        stderr: tail(&captured.stderr),
    }
}

fn refused(what: &'static str, value: &str, reason: &'static str) -> GitError {
    GitError::Refused {
        what,
        value: value.to_string(),
        reason,
    }
}

/// A `rev` that could be read as an option, or that cannot be one argument, never reaches git.
fn check_rev(rev: &str) -> Result<(), GitError> {
    if rev.is_empty() {
        return Err(refused("revision", rev, "it is empty"));
    }
    if rev.starts_with('-') {
        return Err(refused("revision", rev, "it starts with `-`"));
    }
    if rev.contains(['\0', '\n']) {
        return Err(refused("revision", rev, "it has a NUL or a newline"));
    }
    Ok(())
}

/// A path inside a tree: `a/b/c`, or empty for the tree itself. No `..` or `.` part (git reads `rev:./x` as relative to
/// the folder it runs in), no empty part, no leading `/`, and no NUL or newline (the name goes to git as one line).
fn check_path(path: &str) -> Result<(), GitError> {
    if path.is_empty() {
        return Ok(());
    }
    if path.contains(['\0', '\n']) {
        return Err(refused("path", path, "it has a NUL or a newline"));
    }
    if path.starts_with('/') {
        return Err(refused("path", path, "it starts with `/`"));
    }
    for part in path.split('/') {
        match part {
            "" => return Err(refused("path", path, "it has an empty part")),
            "." | ".." => return Err(refused("path", path, "it has a `.` or `..` part")),
            _ => {}
        }
    }
    Ok(())
}

/// A full object id: 40 hex digits (SHA-1) or 64 (SHA-256), lower case, as git prints it.
pub(crate) fn check_id(what: &'static str, id: &str) -> Result<(), GitError> {
    let hex = id
        .bytes()
        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
    if hex && (id.len() == 40 || id.len() == 64) {
        Ok(())
    } else {
        Err(refused(what, id, "it is not a full lower-case hex id"))
    }
}

/// One record of `ls-tree -r -z -l`: `<mode> <type> <id> <size>\t<path>`, where the size is padded with spaces and is
/// `-` for a submodule.
fn parse_entry(record: &[u8]) -> Result<TreeEntry, GitError> {
    let bad = |detail: String| GitError::BadReply {
        command: "git ls-tree".to_string(),
        detail,
    };
    let tab = record
        .iter()
        .position(|byte| *byte == b'\t')
        .ok_or_else(|| bad(String::from_utf8_lossy(record).to_string()))?;
    let (meta, path) = (&record[..tab], &record[tab + 1..]);
    let path = String::from_utf8(path.to_vec()).map_err(|_| GitError::NotUtf8 {
        path: String::from_utf8_lossy(path).to_string(),
    })?;
    let meta = String::from_utf8_lossy(meta).to_string();
    let words: Vec<&str> = meta.split_whitespace().collect();
    let [mode, kind, id, size] = words.as_slice() else {
        return Err(bad(meta.clone()));
    };
    let kind = ObjectKind::parse(kind).ok_or_else(|| bad(meta.clone()))?;
    let size = if *size == "-" {
        0
    } else {
        size.parse().map_err(|_| bad(meta.clone()))?
    };
    check_id("object id", id).map_err(|_| bad(meta.clone()))?;
    Ok(TreeEntry {
        mode: mode.to_string(),
        kind,
        id: id.to_string(),
        path,
        size,
    })
}

/// Reads the reply of one `cat-file --batch` over `ids`: `<id> blob <size>`, the bytes and a newline for each, or
/// `<id> missing`. It checks the id, the type and the size of each reply against what was asked and what came.
fn read_batch(out: &[u8], ids: &[String], bodies: &mut Vec<Vec<u8>>) -> Result<(), GitError> {
    let bad = |detail: String| GitError::BadReply {
        command: "git cat-file".to_string(),
        detail,
    };
    let mut at = 0;
    for id in ids {
        let end = out[at..]
            .iter()
            .position(|byte| *byte == b'\n')
            .ok_or_else(|| bad(format!("no reply for `{id}`")))?;
        let header = String::from_utf8_lossy(&out[at..at + end]).to_string();
        at += end + 1;
        let words: Vec<&str> = header.split(' ').collect();
        match words.as_slice() {
            [named, "missing"] if named == id => {
                return Err(GitError::MissingObject { id: id.clone() });
            }
            [named, kind, size] if named == id => {
                if *kind != "blob" {
                    return Err(GitError::WrongObject {
                        id: id.clone(),
                        kind: kind.to_string(),
                    });
                }
                let size: u64 = size.parse().map_err(|_| bad(header.clone()))?;
                if size > MAX_BLOB_BYTES {
                    return Err(GitError::BlobTooLarge {
                        name: id.clone(),
                        size,
                        cap: MAX_BLOB_BYTES,
                    });
                }
                let size = size as usize;
                if out.len() < at + size + 1 || out[at + size] != b'\n' {
                    return Err(bad(format!("the reply for `{id}` is cut short")));
                }
                bodies.push(out[at..at + size].to_vec());
                at += size + 1;
            }
            _ => return Err(bad(format!("asked for `{id}`, got {header:?}"))),
        }
    }
    Ok(())
}

static SCRATCH_COUNTER: AtomicU32 = AtomicU32::new(0);

/// A folder made for one `tree_of_folder`, removed when it is dropped, also after an error.
struct ScratchFolder(PathBuf);

impl ScratchFolder {
    fn make(parent: &Path) -> std::io::Result<ScratchFolder> {
        let n = SCRATCH_COUNTER.fetch_add(1, Ordering::SeqCst);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |since| since.subsec_nanos());
        let path = parent.join(format!("collide-tree-{}-{n}-{nanos}", std::process::id()));
        // `create` fails when the folder is there, so the folder is always this call's own.
        std::fs::DirBuilder::new().mode(0o700).create(&path)?;
        Ok(ScratchFolder(path))
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for ScratchFolder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// [`GitObjects::tree_at`] with `git` found on the process's `PATH`.
pub fn tree_at(repo: &Path, rev: &str, path: &str) -> Result<TreeAt, GitError> {
    GitObjects::new().tree_at(repo, rev, path)
}

/// [`GitObjects::list`] with `git` found on the process's `PATH`.
pub fn list(repo: &Path, tree: &str) -> Result<Vec<TreeEntry>, GitError> {
    GitObjects::new().list(repo, tree)
}

/// [`GitObjects::read_blobs`] with `git` found on the process's `PATH`.
pub fn read_blobs(repo: &Path, ids: &[String]) -> Result<Vec<Vec<u8>>, GitError> {
    GitObjects::new().read_blobs(repo, ids)
}

/// [`GitObjects::blob_at`] with `git` found on the process's `PATH`.
pub fn blob_at(repo: &Path, rev: &str, path: &str) -> Result<Option<Vec<u8>>, GitError> {
    GitObjects::new().blob_at(repo, rev, path)
}

/// [`GitObjects::tree_of_folder`] with `git` found on the process's `PATH`.
pub fn tree_of_folder(folder: &Path) -> Result<String, GitError> {
    GitObjects::new().tree_of_folder(folder)
}
