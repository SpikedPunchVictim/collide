//! A `git:` source resolved to a pin, and the lock files that hold pins (ts-0100-pu; 0.7, Pack sources).
//!
//! [`resolve_pin`] reads a mirror (never the remote): the ref to a commit, and the commit's tree at the pack's path.
//! [`set_pin`] puts the pin in a lock file, in place when the source is listed and at the end when it is not, so the
//! file keeps its order. Both lock files are written here: the repo's `.collide/packs.lock` (mode 0644, a file the repo
//! shares) and the user's `~/.config/collide/packs.lock` (0600). A write is atomic, and a lock that fails the checks of
//! [`check_lock`] is never written.

use crate::atomic_write::write_atomically;
use crate::git_fetch::HEAD_REF;
use crate::git_objects::{GitObjects, ObjectKind};
use collide_core::lock::{GitLocator, LockEntry, PackLock, check_lock};
use collide_core::owner::PackPin;
use collide_core::yaml::{YamlError, load_yaml_file};
use collide_core::yaml_out::to_yaml;
use std::fs;
use std::io;
use std::path::Path;

const HEADER: &str = "Each `git:` pack pinned by its source, commit and tree id. Collide writes this file: \
                      `collide pack update` moves a pin, and a load uses the pin and never the remote.";

/// The commit `reference` (or the locator's own ref) names in `mirror`, and the tree at the locator's path in it. A
/// ref of `HEAD` is the remote's `HEAD`, as the last fetch took it. The path must be a folder.
pub fn resolve_pin(
    git: &GitObjects,
    mirror: &Path,
    locator: &GitLocator,
    reference: Option<&str>,
) -> Result<PackPin, String> {
    let reference = reference.unwrap_or_else(|| locator.reference());
    let rev = if reference == "HEAD" {
        HEAD_REF
    } else {
        reference
    };
    let commit = git.resolve_commit(mirror, rev).map_err(|error| {
        format!(
            "`{reference}` is not a branch, tag or commit of {}: {error}",
            locator.url
        )
    })?;
    let found = git
        .tree_at(mirror, &commit, &locator.path)
        .map_err(|error| {
            format!(
                "`{}` is not in {} at `{reference}`: {error}",
                locator.path, locator.url
            )
        })?;
    if found.kind != ObjectKind::Tree {
        return Err(format!(
            "`{}` at `{reference}` is not a folder, and a pack is a folder",
            locator.path
        ));
    }
    Ok(PackPin {
        commit,
        tree: found.id,
    })
}

/// The lock in `file`. A file that is not there is an empty lock; one that does not read is an error naming it.
pub fn read_lock(file: &Path) -> Result<PackLock, String> {
    match load_yaml_file::<Option<PackLock>>(file) {
        Ok(lock) => Ok(lock.unwrap_or_default()),
        Err(YamlError::Read { source, .. }) if source.kind() == io::ErrorKind::NotFound => {
            Ok(PackLock::default())
        }
        Err(error) => Err(error.to_string()),
    }
}

/// Sets the pin of `source` in `file`, making the file (and its folder) when they are not there.
pub fn set_pin(file: &Path, source: &str, pin: &PackPin, mode: u32) -> Result<(), String> {
    let mut lock = read_lock(file)?;
    let entry = LockEntry {
        source: source.to_string(),
        commit: pin.commit.clone(),
        tree: pin.tree.clone(),
    };
    match lock.packs.iter_mut().find(|have| have.source == source) {
        Some(have) => *have = entry,
        None => lock.packs.push(entry),
    }
    if let Err(errors) = check_lock(&lock) {
        let lines: Vec<String> = errors.iter().map(ToString::to_string).collect();
        return Err(format!("{}: {}", file.display(), lines.join("; ")));
    }
    let text = to_yaml(&lock, HEADER).map_err(|error| format!("{}: {error}", file.display()))?;
    let dir = file.parent().unwrap_or(Path::new("."));
    let name = file
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("{} has no file name", file.display()))?;
    fs::create_dir_all(dir).map_err(|error| format!("{}: {error}", dir.display()))?;
    write_atomically(dir, name, text.as_bytes(), mode, |file| file)
        .map_err(|error| error.to_string())
}
