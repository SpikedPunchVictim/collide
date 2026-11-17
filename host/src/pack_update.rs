//! `config.pack_update` (ts-0100-pu; 0.7, Switching packs): `collide pack update <name> [--to <ref>]`.
//!
//! The line at the checkout's `HEAD` is loaded, the pack is found by the name in its manifest, its repository is
//! fetched into the mirror, and the pack's ref (or `to`: a branch, a tag or a commit) is resolved to a new pin. What
//! comes back is the old pin, the new one and the difference between the two trees, the manifest, the hooks and every
//! other file, from the trust diff ([`Trust::diff`]). The lock is written only when `confirm` is set:
//!
//! - a pack the board lists (a `git:` source in `board.yaml`): the checkout's `.collide/packs.lock`, in the working
//!   tree and uncommitted, for the person to commit. Trust for the new tree is asked again, as for any change.
//! - a pack the user added (`~/.config/collide/packs.yaml`): the user's `packs.lock`. The loader does not load user
//!   packs until `ts-0101-bd`, so a user pack is found here by reading the manifest at its locked commit in the mirror.
//!
//! Refused, with the reason: a built-in pack (it changes with Collide), a `repo:` pack ("a repo pack changes by a
//! commit alone": the commit is the pin) and a `dev:` pack (a working copy: it changes when its folder does).
//!
//! The first fetch from a host never fetched from is not made: the result says `needs_host` and nothing is fetched or
//! written. A caller that has the person's yes sends the host in `approve_hosts`; it is recorded only when it is the
//! host this fetch needs, and the fetch goes on.

use crate::board_load::{LoadContext, PackOrigin as LoadedOrigin, load_line};
use crate::git_fetch::{FetchError, mirror_path};
use crate::pack_lock::{read_lock, resolve_pin, set_pin};
use crate::settings_files::user_config_dir;
use crate::trust::{Trust, TrustStore};
use collide_core::lock::{GitLocator, PackSource, UserPacks};
use collide_core::owner::{
    ConfigPackUpdateParams, ConfigPackUpdateResult, PackEntry, PackFileChange, PackFileChangeKind,
    PackOrigin, PackPin,
};
use collide_core::trust::{DiffDetail, FileChangeKind, TrustInput};
use collide_core::yaml::{YamlError, load_yaml, load_yaml_file};
use serde::Deserialize;
use std::path::{Path, PathBuf};

/// Why `config.pack_update` gave no result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackUpdateError {
    /// No pack by that name.
    NotFound(String),
    /// The pack cannot be updated this way, or the line has problems, or the ref is not there.
    Refused(String),
    /// Git, the disk or the daemon's setup failed.
    Failed(String),
}

/// Where the request runs: the line's git directory, the checkout (where a repo pack's lock is edited) and the home.
pub struct UpdateSite<'a> {
    pub git_dir: &'a Path,
    pub checkout: &'a Path,
    pub home: &'a Path,
}

/// The pack to move: what it is listed as, its pin now, and the lock that holds the pin.
struct Target {
    entry: PackEntry,
    locator: GitLocator,
    old: PackPin,
    lock_file: PathBuf,
    lock_mode: u32,
}

/// The mode of the repo's lock (shared with the team) and of the user's.
const REPO_LOCK_MODE: u32 = 0o644;
const USER_LOCK_MODE: u32 = 0o600;

pub fn pack_update(
    params: &ConfigPackUpdateParams,
    site: &UpdateSite,
    context: &LoadContext,
) -> Result<ConfigPackUpdateResult, PackUpdateError> {
    let line = load_line(site.git_dir, "HEAD", context).map_err(|problems| {
        let lines: Vec<String> = problems.iter().map(ToString::to_string).collect();
        PackUpdateError::Refused(format!(
            "the board's packs have problems, so nothing can be updated:\n{}",
            lines.join("\n")
        ))
    })?;
    let target = match line.packs.iter().find(|pack| pack.name == params.name) {
        Some(pack) => from_board(pack, &line, site)?,
        None => from_user(&params.name, site, context)?,
    };
    let setup = context.fetch.as_ref().ok_or_else(|| {
        PackUpdateError::Failed(
            "this daemon cannot fetch git packs: it has no fetch setup".to_string(),
        )
    })?;
    let name = &params.name;
    let mirror = mirror_path(&context.cache, &target.locator.url);
    let mut fetched = setup
        .fetch
        .fetch(&mirror, &target.locator.url, &setup.hosts);
    if let Err(FetchError::NeedsApproval { host }) = &fetched {
        if !params.approve_hosts.contains(host) {
            return Ok(ConfigPackUpdateResult {
                pack: target.entry,
                old: target.old,
                new: None,
                needs_host: Some(host.clone()),
                changes: Vec::new(),
                note: None,
                written: false,
                lock_file: None,
            });
        }
        setup
            .hosts
            .record(host)
            .map_err(|error| PackUpdateError::Failed(format!("pack `{name}`: {error}")))?;
        fetched = setup
            .fetch
            .fetch(&mirror, &target.locator.url, &setup.hosts);
    }
    fetched.map_err(|error| PackUpdateError::Failed(format!("pack `{name}`: {error}")))?;
    let new = resolve_pin(&context.git, &mirror, &target.locator, params.to.as_deref())
        .map_err(|error| PackUpdateError::Refused(format!("pack `{name}`: {error}")))?;

    let source = target.entry.source.clone();
    let (changes, mut note) = if new == target.old {
        (
            Vec::new(),
            Some(format!("`{name}` is already at that commit")),
        )
    } else {
        diff(site, context, &mirror, &source, &target.old, &new)
    };
    if new.tree == target.old.tree && new.commit != target.old.commit {
        note = Some("the pack's files are the same: only the commit differs".to_string());
    }
    let written = params.confirm && new != target.old;
    if written {
        set_pin(&target.lock_file, &source, &new, target.lock_mode)
            .map_err(PackUpdateError::Failed)?;
    }
    Ok(ConfigPackUpdateResult {
        pack: target.entry,
        old: target.old,
        new: Some(new),
        needs_host: None,
        changes,
        note,
        written,
        lock_file: written.then(|| target.lock_file.display().to_string()),
    })
}

/// A pack the loaded line has: only a `git:` pack that the board lists can be moved.
fn from_board(
    pack: &crate::board_load::LoadedPack,
    line: &crate::board_load::LoadedLine,
    site: &UpdateSite,
) -> Result<Target, PackUpdateError> {
    let name = &pack.name;
    match pack.origin {
        LoadedOrigin::Builtin => Err(PackUpdateError::Refused(format!(
            "`{name}` is a built-in pack: it changes with Collide and not with `pack update`"
        ))),
        LoadedOrigin::Repo => Err(PackUpdateError::Refused(format!(
            "`{name}` is a repo pack ({}): a repo pack changes by a commit alone, so edit it and commit",
            pack.source.as_deref().unwrap_or("repo:")
        ))),
        LoadedOrigin::Git => {
            let source = pack.source.clone().unwrap_or_default();
            let locked = line
                .lock
                .packs
                .iter()
                .find(|locked| locked.source == source)
                .ok_or_else(|| {
                    PackUpdateError::Failed(format!("`{source}` has no entry in the lock"))
                })?;
            let dir = line.collide_dir.as_deref().unwrap_or(".collide");
            Ok(Target {
                entry: PackEntry {
                    name: name.clone(),
                    source: source.clone(),
                    origin: PackOrigin::Repo,
                    enabled: true,
                    dev: false,
                },
                locator: locator_of(&source)?,
                old: PackPin {
                    commit: locked.commit.clone(),
                    tree: locked.tree.clone(),
                },
                lock_file: site.checkout.join(dir).join("packs.lock"),
                lock_mode: REPO_LOCK_MODE,
            })
        }
    }
}

fn locator_of(source: &str) -> Result<GitLocator, PackUpdateError> {
    match PackSource::parse(source) {
        Ok(PackSource::Git { locator }) => {
            GitLocator::parse(&locator).map_err(|error| PackUpdateError::Refused(error.to_string()))
        }
        _ => Err(PackUpdateError::Failed(format!(
            "`{source}` is not a `git:` source"
        ))),
    }
}

#[derive(Deserialize)]
struct ManifestName {
    name: String,
}

fn name_of(bytes: &[u8], origin: &str) -> Option<String> {
    load_yaml::<ManifestName>(bytes, origin)
        .ok()
        .map(|manifest| manifest.name)
}

/// A pack of the user's, found by the name in its manifest: a `git:` pack's is read from the mirror at its locked
/// commit, a `dev:` pack's from its folder (and refused).
fn from_user(
    name: &str,
    site: &UpdateSite,
    context: &LoadContext,
) -> Result<Target, PackUpdateError> {
    let dir = user_config_dir(site.home);
    let packs = match load_yaml_file::<Option<UserPacks>>(&dir.join("packs.yaml")) {
        Ok(found) => found.unwrap_or_default(),
        Err(YamlError::Read { source, .. }) if source.kind() == std::io::ErrorKind::NotFound => {
            UserPacks::default()
        }
        Err(error) => return Err(PackUpdateError::Refused(error.to_string())),
    };
    let lock_file = dir.join("packs.lock");
    let lock = read_lock(&lock_file).map_err(PackUpdateError::Refused)?;
    for entry in &packs.packs {
        match PackSource::parse(&entry.source) {
            Ok(PackSource::Dev { path }) => {
                let folder = match path.strip_prefix("~/") {
                    Some(rest) => site.home.join(rest),
                    None => PathBuf::from(&path),
                };
                let manifest = folder.join("pack.yaml");
                let found = std::fs::read(&manifest)
                    .ok()
                    .and_then(|bytes| name_of(&bytes, &manifest.display().to_string()));
                if found.as_deref() == Some(name) {
                    return Err(PackUpdateError::Refused(format!(
                        "`{name}` is a dev: pack, a working copy ({}): it changes when its folder does",
                        folder.display()
                    )));
                }
            }
            Ok(PackSource::Git { locator }) => {
                let Some(locked) = lock
                    .packs
                    .iter()
                    .find(|locked| locked.source == entry.source)
                else {
                    continue;
                };
                let Ok(parsed) = GitLocator::parse(&locator) else {
                    continue;
                };
                let mirror = mirror_path(&context.cache, &parsed.url);
                let manifest_path = if parsed.path.is_empty() {
                    "pack.yaml".to_string()
                } else {
                    format!("{}/pack.yaml", parsed.path)
                };
                let bytes = context.git.blob_at(&mirror, &locked.commit, &manifest_path);
                let found = bytes
                    .ok()
                    .flatten()
                    .and_then(|bytes| name_of(&bytes, &manifest_path));
                if found.as_deref() == Some(name) {
                    return Ok(Target {
                        entry: PackEntry {
                            name: name.to_string(),
                            source: entry.source.clone(),
                            origin: PackOrigin::User,
                            enabled: true,
                            dev: false,
                        },
                        locator: parsed,
                        old: PackPin {
                            commit: locked.commit.clone(),
                            tree: locked.tree.clone(),
                        },
                        lock_file,
                        lock_mode: USER_LOCK_MODE,
                    });
                }
            }
            _ => {}
        }
    }
    Err(PackUpdateError::NotFound(format!(
        "no pack named `{name}` is listed by this board or added by you"
    )))
}

/// The files that differ between the two trees, and a note when the diff could not be made in full.
fn diff(
    site: &UpdateSite,
    context: &LoadContext,
    mirror: &Path,
    source: &str,
    old: &PackPin,
    new: &PackPin,
) -> (Vec<PackFileChange>, Option<String>) {
    let name = format!("pack {source}");
    let trust =
        Trust::new(TrustStore::new(user_config_dir(site.home))).with_git(context.git.clone());
    let found = trust.diff(
        mirror,
        &[TrustInput::new(name.clone(), old.tree.clone())],
        &[TrustInput::new(name, new.tree.clone())],
    );
    let mut changes = Vec::new();
    let mut note = None;
    for input in found {
        match input.detail {
            DiffDetail::Files(files) => {
                for file in files {
                    let mut notes: Vec<String> = file.note.into_iter().collect();
                    let lines = match file.lines {
                        Some(lines) => {
                            if lines.binary {
                                notes.push("a binary file".to_string());
                            }
                            if lines.omitted > 0 {
                                notes.push(format!(
                                    "{} more changed lines are not shown",
                                    lines.omitted
                                ));
                            }
                            lines.lines
                        }
                        None => Vec::new(),
                    };
                    changes.push(PackFileChange {
                        path: file.path,
                        kind: match file.kind {
                            FileChangeKind::Added => PackFileChangeKind::Added,
                            FileChangeKind::Deleted => PackFileChangeKind::Deleted,
                            FileChangeKind::Modified => PackFileChangeKind::Modified,
                            FileChangeKind::TypeChanged => PackFileChangeKind::TypeChanged,
                        },
                        lines,
                        note: (!notes.is_empty()).then(|| notes.join("; ")),
                    });
                }
            }
            DiffDetail::Unavailable(why) => note = Some(why),
            DiffDetail::Lines(_) | DiffDetail::None => {}
        }
    }
    (changes, note)
}
