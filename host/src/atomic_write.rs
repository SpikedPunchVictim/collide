//! One atomic file write (ts-0099-bw): a temp file in the same folder, its mode set on the open file, the bytes
//! written and synced, a rename over the target, and a sync of the folder. A write that fails removes the temp file
//! and leaves what was at the target whole.
//!
//! The shim's records ([`crate::shim`]), `trust.yaml` ([`crate::trust`]) and the user's settings file
//! ([`crate::config_set`]) use it. Each keeps its own error type: [`AtomicError`] says which step failed and on which
//! path, and the caller words it.

use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

/// The step of the write that failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// Opening the temp file, setting its mode, or cloning its handle.
    CreateTemp,
    /// Writing, flushing or syncing the temp file.
    WriteTemp,
    /// Renaming the temp file over the target.
    Rename,
    /// Syncing the folder, after the rename.
    SyncDir,
}

/// Why a write failed. `path` is the temp file for [`Step::CreateTemp`] and [`Step::WriteTemp`], the target for
/// [`Step::Rename`] and the folder for [`Step::SyncDir`].
#[derive(Debug)]
pub struct AtomicError {
    pub step: Step,
    pub path: PathBuf,
    pub source: io::Error,
}

impl AtomicError {
    /// What was being done, as a message starts it: `creating`, `writing`, `renaming to` or `syncing`.
    pub fn verb(&self) -> &'static str {
        match self.step {
            Step::CreateTemp => "creating",
            Step::WriteTemp => "writing",
            Step::Rename => "renaming to",
            Step::SyncDir => "syncing",
        }
    }
}

impl fmt::Display for AtomicError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {}: {}",
            self.verb(),
            self.path.display(),
            self.source
        )
    }
}

impl std::error::Error for AtomicError {}

/// The temp file's name for `name`: beside it, so the rename stays on one file system.
fn temp_name(name: &str) -> String {
    format!("{name}.tmp")
}

/// Writes `bytes` to `dir/name` with `mode`. `dir` must exist. `wrap` is the seam a test uses to make the write fail:
/// the bytes go through what it makes of the open file, and callers that have no use for it pass `|file| file`.
///
/// A temp file that an earlier write left is replaced, and its mode is set on the open file, so a file that was made
/// open stays no wider than `mode`.
pub fn write_atomically<W: Write>(
    dir: &Path,
    name: &str,
    bytes: &[u8],
    mode: u32,
    wrap: impl FnOnce(File) -> W,
) -> Result<(), AtomicError> {
    let temp = dir.join(temp_name(name));
    let target = dir.join(name);
    let failed = |step, path: &Path, source| AtomicError {
        step,
        path: path.to_path_buf(),
        source,
    };
    let file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(mode)
        .open(&temp)
        .and_then(|file| {
            file.set_permissions(fs::Permissions::from_mode(mode))?;
            Ok(file)
        })
        .map_err(|source| failed(Step::CreateTemp, &temp, source))?;
    let sync_handle = file
        .try_clone()
        .map_err(|source| failed(Step::CreateTemp, &temp, source))?;
    let mut sink = wrap(file);
    let written = sink
        .write_all(bytes)
        .and_then(|()| sink.flush())
        .and_then(|()| sync_handle.sync_all());
    drop(sink);
    if let Err(source) = written {
        let _ = fs::remove_file(&temp);
        return Err(failed(Step::WriteTemp, &temp, source));
    }
    if let Err(source) = fs::rename(&temp, &target) {
        let _ = fs::remove_file(&temp);
        return Err(failed(Step::Rename, &target, source));
    }
    File::open(dir)
        .and_then(|directory| directory.sync_all())
        .map_err(|source| failed(Step::SyncDir, dir, source))
}
