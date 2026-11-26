//! `board.init` (ts-0102-vu, ts-0194-qp; 0.11, Share with team): `.collide/board.yaml` in a board's root, and a copy of
//! each of the person's saved views of the board in `.collide/views/`. It never commits, never edits `.gitignore` and
//! never writes the lock, which `pack add` writes with the first `git:` pack. The board file lists no pack, so the
//! built-in packs still run. No setting is per board and per person, so there is no `config.yaml`.

use crate::atomic_write::write_atomically;
use collide_core::owner::BoardInitResult;
use collide_core::yaml::MAX_YAML_BYTES;
use std::fs;
use std::path::{Path, PathBuf};

/// The folder, relative to the board's root.
const COLLIDE_DIR: &str = ".collide";

const BOARD_FILE: &str = "board.yaml";

const STARTER: &str = "\
# The packs this repository adds to Collide. The built-in packs always run, so an empty list is a working setup.
# Add a pack with `collide pack add`. Docs: docs/tasks-and-docs.md in the Collide repository.
packs: []
";

/// Why `board.init` did nothing.
#[derive(Debug, PartialEq, Eq)]
pub enum InitError {
    /// `.collide` is already there (a folder, a pointer file or a link), and the message names it.
    Exists(String),
    /// The folder or the file could not be made, or a view could not be read.
    Failed(String),
}

/// What an init wrote (or, for a dry run, would write): each file's path relative to the board's root, and its bytes.
/// The bytes are what the trust hash of the files is made from.
#[derive(Debug, PartialEq, Eq)]
pub struct Shared {
    pub files: Vec<(String, Vec<u8>)>,
    pub written: bool,
}

impl Shared {
    /// The files as the wire lists them.
    pub fn result(&self, trusted: bool) -> BoardInitResult {
        BoardInitResult {
            files: self.files.iter().map(|(path, _)| path.clone()).collect(),
            written: self.written,
            trusted,
        }
    }

    /// The files as the trust hash names them: relative to the Collide folder.
    pub fn inputs(&self) -> Vec<(String, Vec<u8>)> {
        let prefix = format!("{COLLIDE_DIR}/");
        self.files
            .iter()
            .map(|(path, bytes)| {
                (
                    path.strip_prefix(&prefix).unwrap_or(path).to_string(),
                    bytes.clone(),
                )
            })
            .collect()
    }
}

/// Writes the starter `board.yaml` and the views of `personal_views` (the person's folder for this board, when they
/// have one) into `root`, or with `dry_run` lists them and writes nothing. It refuses when `.collide` exists, so a repo
/// that is already set up is never changed. A failed write removes what this call made.
pub fn init(
    root: &Path,
    personal_views: Option<&Path>,
    dry_run: bool,
) -> Result<Shared, InitError> {
    let dir = root.join(COLLIDE_DIR);
    if fs::symlink_metadata(&dir).is_ok() {
        return Err(InitError::Exists(format!(
            "`{}` already exists, so `init` leaves the repository as it is",
            dir.display()
        )));
    }
    let mut files = vec![(
        format!("{COLLIDE_DIR}/{BOARD_FILE}"),
        STARTER.as_bytes().to_vec(),
    )];
    for (name, bytes) in personal_view_files(personal_views)? {
        files.push((format!("{COLLIDE_DIR}/views/{name}"), bytes));
    }
    if dry_run {
        return Ok(Shared {
            files,
            written: false,
        });
    }
    let mut made = Made::new(dir.clone());
    if let Err(error) = write_all(&dir, &files, &mut made) {
        made.remove();
        return Err(error);
    }
    Ok(Shared {
        files,
        written: true,
    })
}

/// The regular `.yaml` files of the person's views folder, by name, with their bytes. A folder that is not there has
/// none. A file that is not a regular file is left out, as the picker lists it as one that cannot be read. A file over
/// the YAML cap is refused: the repo's load would not read it either.
fn personal_view_files(folder: Option<&Path>) -> Result<Vec<(String, Vec<u8>)>, InitError> {
    let Some(folder) = folder else {
        return Ok(Vec::new());
    };
    let entries = match fs::read_dir(folder) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(InitError::Failed(format!(
                "cannot read your views in `{}`: {error}",
                folder.display()
            )));
        }
    };
    let mut names: Vec<String> = entries
        .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
        .filter(|name| name.ends_with(".yaml"))
        .collect();
    names.sort();
    let mut found = Vec::new();
    for name in names {
        let path = folder.join(&name);
        if !fs::symlink_metadata(&path).is_ok_and(|meta| meta.is_file()) {
            continue;
        }
        let bytes = fs::read(&path).map_err(|error| {
            InitError::Failed(format!(
                "cannot read your view `{}`: {error}",
                path.display()
            ))
        })?;
        if bytes.len() > MAX_YAML_BYTES {
            return Err(InitError::Failed(format!(
                "your view `{}` is {} bytes, and a YAML file is capped at {MAX_YAML_BYTES}, so it cannot be shared",
                path.display(),
                bytes.len()
            )));
        }
        found.push((name, bytes));
    }
    Ok(found)
}

/// What this call made, so a failure can take it away again: the files, then the folders it made, in the order made.
struct Made {
    folders: Vec<PathBuf>,
    files: Vec<PathBuf>,
}

impl Made {
    fn new(collide: PathBuf) -> Made {
        Made {
            folders: vec![collide],
            files: Vec::new(),
        }
    }

    fn remove(self) {
        for file in self.files.iter().rev() {
            let _ = fs::remove_file(file);
        }
        // Empty folders only: these are this call's own, so there is nothing else in them.
        for folder in self.folders.iter().rev() {
            let _ = fs::remove_dir(folder);
        }
    }
}

fn write_all(dir: &Path, files: &[(String, Vec<u8>)], made: &mut Made) -> Result<(), InitError> {
    fs::create_dir(dir)
        .map_err(|error| InitError::Failed(format!("cannot make `{}`: {error}", dir.display())))?;
    let root = dir.parent().expect("the Collide folder has a parent");
    for (path, bytes) in files {
        let target = root.join(path);
        let folder = target.parent().expect("a shared file has a folder");
        if !made.folders.iter().any(|made| made == folder) {
            fs::create_dir(folder).map_err(|error| {
                InitError::Failed(format!("cannot make `{}`: {error}", folder.display()))
            })?;
            made.folders.push(folder.to_path_buf());
        }
        let name = target
            .file_name()
            .and_then(|name| name.to_str())
            .expect("a shared file has a name");
        // The temp file is this call's own too: a failed write leaves none, and a later call replaces one.
        made.files.push(folder.join(format!("{name}.tmp")));
        made.files.push(target.clone());
        write_atomically(folder, name, bytes, 0o644, |file| file).map_err(|error| {
            InitError::Failed(format!("cannot write `{}`: {error}", target.display()))
        })?;
    }
    Ok(())
}
