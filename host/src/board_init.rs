//! `board.init` (ts-0102-vu; 0.11, Share with team): `.collide/board.yaml` in a board's root, and nothing else. It
//! never commits, never edits `.gitignore` and never writes the lock, which `pack add` writes with the first `git:`
//! pack. The file lists no pack, so the built-in packs still run.

use crate::atomic_write::write_atomically;
use collide_core::owner::BoardInitResult;
use std::fs;
use std::path::Path;

/// The file, relative to the board's root.
const BOARD_FILE: &str = ".collide/board.yaml";

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
    /// The folder or the file could not be made.
    Failed(String),
}

/// Writes the starter `board.yaml` into `root`, or with `dry_run` lists it and writes nothing. It refuses when
/// `.collide` exists, so a repo that is already set up is never changed.
pub fn init(root: &Path, dry_run: bool) -> Result<BoardInitResult, InitError> {
    let dir = root.join(".collide");
    if fs::symlink_metadata(&dir).is_ok() {
        return Err(InitError::Exists(format!(
            "`{}` already exists, so `init` leaves the repository as it is",
            dir.display()
        )));
    }
    let files = vec![BOARD_FILE.to_string()];
    if dry_run {
        return Ok(BoardInitResult {
            files,
            written: false,
        });
    }
    fs::create_dir(&dir)
        .map_err(|error| InitError::Failed(format!("cannot make `{}`: {error}", dir.display())))?;
    if let Err(error) = write_atomically(&dir, "board.yaml", STARTER.as_bytes(), 0o644, |file| file)
    {
        // The folder is this call's own: it did not exist a moment ago.
        let _ = fs::remove_dir(&dir);
        return Err(InitError::Failed(format!(
            "cannot write the board file: {error}"
        )));
    }
    Ok(BoardInitResult {
        files,
        written: true,
    })
}
