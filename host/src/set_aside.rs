//! Reset (ts-0215-gt; Stage 6a, Start with defaults): the person's own files, moved aside so the next start finds the
//! defaults, and kept so nothing is lost.
//!
//! What moves is named here and nowhere else: the user's `config.yaml`, `packs.yaml` and `packs.lock`, and one board's
//! own folder (`boards/<board id>/`: its settings, its packs and its saved views), all under `~/.config/collide/`. They
//! go to `set-aside/<date>-<time>/` under the same folder, with the same paths below it. Nothing is deleted. What
//! stays is every other board's folder, `trust.yaml`, the profiles, the repo's own `.collide/` and the keychain.
//!
//! A move is a rename inside one folder tree, so a file is whole at its old place or at its new one. The folder of a
//! reset is made with `create_dir`, which fails when the name is taken, so two resets in one second get two folders
//! (`…-2`, `…-3`) and neither takes the other's.

use crate::clock::civil_date;
use crate::user_packs::{LOCK_FILE, PACKS_FILE};
use collide_core::resolve::is_safe_token;
use std::fs;
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};

/// The folder under `~/.config/collide/` that holds what was moved.
pub const SET_ASIDE_DIR: &str = "set-aside";

/// The files of the user's folder a reset moves, in the order they are moved.
pub const GLOBAL_FILES: [&str; 3] = ["config.yaml", PACKS_FILE, LOCK_FILE];

const DIR_MODE: u32 = 0o700;

/// What a reset did: the folder it made, and each thing it moved, as a path below `config_dir` and below the folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetAside {
    pub folder: PathBuf,
    pub moved: Vec<String>,
}

/// The name of a reset's folder for a wall reading (nanoseconds since the Unix epoch) in a zone `utc_offset_secs` east
/// of UTC: `YYYY-MM-DD-HHMMSS`, which sorts as the resets happened.
pub fn stamp(wall_nanos: u64, utc_offset_secs: i64) -> String {
    let secs = i64::try_from(wall_nanos / 1_000_000_000).unwrap_or(i64::MAX) + utc_offset_secs;
    let in_day = secs.rem_euclid(86_400);
    let (hour, minute, second) = (in_day / 3600, in_day % 3600 / 60, in_day % 60);
    format!(
        "{}-{hour:02}{minute:02}{second:02}",
        civil_date(wall_nanos, utc_offset_secs)
    )
}

/// Moves the global files, and the folder of `board` when there is one, from `config_dir` into a new folder named for
/// `stamp`. A file or folder that is not there is left out of `moved`. The folder is made even when nothing moves, so
/// the person is told where a reset went. A move that fails stops the reset and says which, and what had moved stays
/// where it went: nothing is moved back and nothing is deleted.
pub fn reset(config_dir: &Path, board: Option<&str>, stamp: &str) -> Result<SetAside, String> {
    let mut items: Vec<String> = GLOBAL_FILES.iter().map(|name| name.to_string()).collect();
    if let Some(board) = board {
        if !is_safe_token(board) {
            return Err(format!(
                "the board id {board:?} is not a safe name for a folder"
            ));
        }
        items.push(format!("boards/{board}"));
    }
    let folder = make_folder(config_dir, stamp)?;
    let mut moved = Vec::new();
    for item in items {
        let from = config_dir.join(&item);
        if fs::symlink_metadata(&from).is_err() {
            continue;
        }
        let to = folder.join(&item);
        if let Some(parent) = to.parent() {
            make_dirs(parent)?;
        }
        fs::rename(&from, &to).map_err(|error| {
            format!(
                "{} was not moved to {}: {error}{}",
                from.display(),
                to.display(),
                already_moved(&moved)
            )
        })?;
        moved.push(item);
    }
    Ok(SetAside { folder, moved })
}

fn already_moved(moved: &[String]) -> String {
    if moved.is_empty() {
        String::new()
    } else {
        format!("; already moved and left there: {}", moved.join(", "))
    }
}

/// `set-aside/<stamp>`, or `set-aside/<stamp>-<n>` for the first `n` from 2 whose name is free.
fn make_folder(config_dir: &Path, stamp: &str) -> Result<PathBuf, String> {
    let parent = config_dir.join(SET_ASIDE_DIR);
    make_dirs(&parent)?;
    for attempt in 1.. {
        let name = if attempt == 1 {
            stamp.to_string()
        } else {
            format!("{stamp}-{attempt}")
        };
        let folder = parent.join(name);
        match fs::DirBuilder::new().mode(DIR_MODE).create(&folder) {
            Ok(()) => return Ok(folder),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(format!("{} was not made: {error}", folder.display())),
        }
    }
    unreachable!("the loop ends by returning")
}

fn make_dirs(dir: &Path) -> Result<(), String> {
    fs::DirBuilder::new()
        .recursive(true)
        .mode(DIR_MODE)
        .create(dir)
        .map_err(|error| format!("{} was not made: {error}", dir.display()))
}
