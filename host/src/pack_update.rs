//! `config.pack_update` (ts-0100-pu; 0.7, Switching packs): `collide pack update <name> [--to <ref>]`.
//!
//! The pack is found by the name in its manifest among the sources the board lists and the user added, **without
//! loading the line**: a line that does not load (a pin whose commit the remote no longer has, a broken pack beside
//! this one) must not stop the update that is the way out of it. A `git:` pack's name is read from the cache entry or
//! from the mirror at its locked commit; when neither has it, each `git:` pack whose name is not known is tried in
//! turn: it is fetched, the ref is resolved, and the manifest at the new commit must have the name asked for.
//!
//! The pack's repository is fetched into the mirror, and the pack's ref (or `to`: a branch, a tag or a commit) is
//! resolved to a new pin. What comes back is the old pin, the new one and the difference between the two trees, the
//! manifest, the hooks and every other file, from the trust diff ([`Trust::diff`]). The lock is written only when
//! `confirm` is set:
//!
//! - a pack the board lists (a `git:` source in `board.yaml`): the checkout's `.collide/packs.lock`, in the working
//!   tree and uncommitted, for the person to commit. Trust for the new tree is asked again, as for any change.
//! - a pack the user added (`~/.config/collide/packs.yaml`): the user's `packs.lock`. The new tree is not trusted
//!   until the person says so: `pack add` recorded the one they saw.
//!
//! Refused, with the reason: a built-in pack (it changes with Collide), a `repo:` pack ("a repo pack changes by a
//! commit alone": the commit is the pin) and a `dev:` pack (a working copy: it changes when its folder does).
//!
//! The first fetch from a host never fetched from is not made: the result says `needs_host` and nothing is fetched or
//! written. A caller that has the person's yes sends the host in `approve_hosts`; it is recorded only when it is the
//! host this fetch needs, and the fetch goes on ([`fetch_for`]).

use crate::board_load::{LoadContext, board_files};
use crate::builtin_packs::BuiltinPacks;
use crate::git_fetch::{FetchError, mirror_path};
use crate::pack_lock::{resolve_pin, set_pin};
use crate::settings_files::user_config_dir;
use crate::trust::{Trust, TrustStore};
use crate::user_packs::{git_pack_name, manifest_name, manifest_path, read_global_files};
use collide_core::lock::{GitLocator, LockEntry, PackLock, PackSource};
use collide_core::owner::{
    ConfigPackUpdateParams, ConfigPackUpdateResult, PackEntry, PackFileChange, PackFileChangeKind,
    PackOrigin, PackPin,
};
use collide_core::trust::{DiffDetail, FileChangeKind, FileDiff, TrustInput};
use std::path::{Path, PathBuf};

/// Why a `config.pack_*` request gave no result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackError {
    /// No pack by that name.
    NotFound(String),
    /// The request cannot be done: the pack cannot be changed this way, the ref is not there, a file is wrong.
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

/// What a source of the board or of the user is, as far as an update is concerned.
pub(crate) enum Kind {
    Repo(String),
    Git(GitLocator),
    Dev(String),
}

/// A source that may be the pack to move: what it is, its name when it can be read without a load, and the lock that
/// holds its pin.
pub(crate) struct Candidate {
    pub(crate) source: String,
    pub(crate) kind: Kind,
    pub(crate) origin: PackOrigin,
    pub(crate) name: Option<String>,
    locked: Option<LockEntry>,
    lock_file: PathBuf,
    lock_mode: u32,
}

/// The mode of the repo's lock (shared with the team) and of the user's.
const REPO_LOCK_MODE: u32 = 0o644;
const USER_LOCK_MODE: u32 = crate::user_packs::FILE_MODE;

pub fn pack_update(
    params: &ConfigPackUpdateParams,
    site: &UpdateSite,
    context: &LoadContext,
) -> Result<ConfigPackUpdateResult, PackError> {
    let name = &params.name;
    if BuiltinPacks::names().contains(&name.as_str()) {
        return Err(PackError::Refused(format!(
            "`{name}` is a built-in pack: it changes with Collide and not with `pack update`"
        )));
    }
    let candidates = candidates(site, context)?;
    if let Some(found) = candidates
        .iter()
        .find(|candidate| candidate.name.as_deref() == Some(name))
    {
        return update(params, site, context, found, false)
            .map(|done| done.expect("a pack found by its name is not checked again"));
    }
    // A name no source gave without a fetch: each git pack whose name is not known is asked.
    let mut failed = Vec::new();
    for candidate in candidates
        .iter()
        .filter(|candidate| candidate.name.is_none() && matches!(candidate.kind, Kind::Git(_)))
    {
        match update(params, site, context, candidate, true) {
            Ok(Some(done)) => return Ok(done),
            Ok(None) => {}
            Err(PackError::NotFound(_)) => {}
            Err(PackError::Refused(why) | PackError::Failed(why)) => {
                failed.push(format!("{}: {why}", candidate.source));
            }
        }
    }
    let mut message = format!("no pack named `{name}` is listed by this board or added by you");
    if !failed.is_empty() {
        message.push_str(&format!(
            "; these sources could not be read to see whether one is `{name}`:\n{}",
            failed.join("\n")
        ));
    }
    Err(PackError::NotFound(message))
}

/// Every source the board lists and the user added that an update could be about, board first. No pack is loaded:
/// a name is read from the repo's files, the cache or the mirror, or is `None`.
pub(crate) fn candidates(
    site: &UpdateSite,
    context: &LoadContext,
) -> Result<Vec<Candidate>, PackError> {
    let files = board_files(site.git_dir, "HEAD", context).map_err(PackError::Refused)?;
    let dir = files.collide_dir.as_deref().unwrap_or(".collide");
    let board_lock = site.checkout.join(dir).join("packs.lock");
    let mut found = Vec::new();
    let mut listed: Vec<&str> = Vec::new();
    for entry in files.board.iter().flat_map(|board| &board.packs) {
        if listed.contains(&entry.source.as_str()) {
            continue;
        }
        listed.push(&entry.source);
        let kind = match PackSource::parse(&entry.source) {
            Ok(PackSource::Repo { path }) => Kind::Repo(path),
            Ok(PackSource::Git { locator }) => match GitLocator::parse(&locator) {
                Ok(locator) => Kind::Git(locator),
                Err(_) => continue,
            },
            _ => continue,
        };
        let name = match &kind {
            Kind::Repo(path) => context
                .git
                .blob_at(site.git_dir, &files.commit, &format!("{path}/pack.yaml"))
                .ok()
                .flatten()
                .and_then(|bytes| manifest_name(&bytes, "pack.yaml")),
            _ => git_name(context, &entry.source, &files.lock),
        };
        found.push(Candidate {
            source: entry.source.clone(),
            kind,
            origin: PackOrigin::Repo,
            name,
            locked: files
                .lock
                .packs
                .iter()
                .find(|locked| locked.source == entry.source)
                .cloned(),
            lock_file: board_lock.clone(),
            lock_mode: REPO_LOCK_MODE,
        });
    }

    let (user, problems) = read_global_files(site.home);
    if !problems.is_empty() {
        return Err(PackError::Refused(problems.join("\n")));
    }
    let user_lock = user_config_dir(site.home).join("packs.lock");
    for source in &user.sources {
        let (kind, name) = match PackSource::parse(source) {
            Ok(PackSource::Dev { path }) => {
                let folder = match path.strip_prefix("~/") {
                    Some(rest) => site.home.join(rest),
                    None => PathBuf::from(&path),
                };
                let name = std::fs::read(folder.join("pack.yaml"))
                    .ok()
                    .and_then(|bytes| manifest_name(&bytes, "pack.yaml"));
                (Kind::Dev(folder.display().to_string()), name)
            }
            Ok(PackSource::Git { locator }) => match GitLocator::parse(&locator) {
                Ok(locator) => (Kind::Git(locator), git_name(context, source, &user.lock)),
                Err(_) => continue,
            },
            _ => continue,
        };
        found.push(Candidate {
            source: source.clone(),
            kind,
            origin: PackOrigin::User,
            name,
            locked: user
                .lock
                .packs
                .iter()
                .find(|locked| locked.source == *source)
                .cloned(),
            lock_file: user_lock.clone(),
            lock_mode: USER_LOCK_MODE,
        });
    }
    Ok(found)
}

fn git_name(context: &LoadContext, source: &str, lock: &PackLock) -> Option<String> {
    git_pack_name(&context.git, &context.cache, &context.sdk, source, lock)
}

/// The result of fetching a pack's repository into its mirror.
pub enum Fetched {
    Ready(PathBuf),
    /// The first fetch from this host needs the person's yes; nothing was fetched.
    NeedsHost(String),
}

/// Fetches the repository of `locator` into its mirror. A host never fetched from stops here unless `approve_hosts`
/// names it, in which case it is recorded and the fetch goes on.
pub fn fetch_for(
    context: &LoadContext,
    locator: &GitLocator,
    approve_hosts: &[String],
    what: &str,
) -> Result<Fetched, PackError> {
    let setup = context.fetch.as_ref().ok_or_else(|| {
        PackError::Failed("this daemon cannot fetch git packs: it has no fetch setup".to_string())
    })?;
    let mirror = mirror_path(&context.cache, &locator.url);
    let mut fetched = setup.fetch.fetch(&mirror, &locator.url, &setup.hosts);
    if let Err(FetchError::NeedsApproval { host }) = &fetched {
        if !approve_hosts.contains(host) {
            return Ok(Fetched::NeedsHost(host.clone()));
        }
        setup
            .hosts
            .record(host)
            .map_err(|error| PackError::Failed(format!("{what}: {error}")))?;
        fetched = setup.fetch.fetch(&mirror, &locator.url, &setup.hosts);
    }
    fetched.map_err(|error| PackError::Failed(format!("{what}: {error}")))?;
    Ok(Fetched::Ready(mirror))
}

/// Moves `target`. With `verify` the pack is only the one asked for if the manifest at the new commit has its name:
/// `Ok(None)` when it does not.
fn update(
    params: &ConfigPackUpdateParams,
    site: &UpdateSite,
    context: &LoadContext,
    target: &Candidate,
    verify: bool,
) -> Result<Option<ConfigPackUpdateResult>, PackError> {
    let name = &params.name;
    let locator = match &target.kind {
        Kind::Repo(_) => {
            return Err(PackError::Refused(format!(
                "`{name}` is a repo pack ({}): a repo pack changes by a commit alone, so edit it and commit",
                target.source
            )));
        }
        Kind::Dev(folder) => {
            return Err(PackError::Refused(format!(
                "`{name}` is a dev: pack, a working copy ({folder}): it changes when its folder does"
            )));
        }
        Kind::Git(locator) => locator,
    };
    let locked = target.locked.as_ref().ok_or_else(|| {
        PackError::Failed(format!("`{}` has no entry in the lock", target.source))
    })?;
    let old = PackPin {
        commit: locked.commit.clone(),
        tree: locked.tree.clone(),
    };
    let entry = PackEntry {
        name: name.clone(),
        source: target.source.clone(),
        origin: target.origin,
        enabled: true,
        dev: false,
        working_copy: false,
        review: None,
    };
    let mirror = match fetch_for(
        context,
        locator,
        &params.approve_hosts,
        &format!("pack `{name}`"),
    )? {
        Fetched::Ready(mirror) => mirror,
        Fetched::NeedsHost(host) => {
            return Ok(Some(ConfigPackUpdateResult {
                pack: entry,
                old,
                new: None,
                needs_host: Some(host),
                changes: Vec::new(),
                note: None,
                written: false,
                lock_file: None,
            }));
        }
    };
    let new = resolve_pin(&context.git, &mirror, locator, params.to.as_deref())
        .map_err(|error| PackError::Refused(format!("pack `{name}`: {error}")))?;
    if verify {
        let named = context
            .git
            .blob_at(&mirror, &new.commit, &manifest_path(locator))
            .ok()
            .flatten()
            .and_then(|bytes| manifest_name(&bytes, "pack.yaml"));
        if named.as_deref() != Some(name.as_str()) {
            return Ok(None);
        }
    }

    let source = target.source.clone();
    let (changes, mut note) = if new == old {
        (
            Vec::new(),
            Some(format!("`{name}` is already at that commit")),
        )
    } else {
        let trust =
            Trust::new(TrustStore::new(user_config_dir(site.home))).with_git(context.git.clone());
        tree_changes(
            &trust,
            &mirror,
            &format!("pack {source}"),
            &old.tree,
            &new.tree,
        )
    };
    if new.tree == old.tree && new.commit != old.commit {
        note = Some("the pack's files are the same: only the commit differs".to_string());
    }
    let written = params.confirm && new != old;
    if written {
        set_pin(&target.lock_file, &source, &new, target.lock_mode).map_err(PackError::Failed)?;
    }
    Ok(Some(ConfigPackUpdateResult {
        pack: entry,
        old,
        new: Some(new),
        needs_host: None,
        changes,
        note,
        written,
        lock_file: written.then(|| target.lock_file.display().to_string()),
    }))
}

/// A folder's changed files as the wire has them: a binary file and lines left out are said in the note.
pub fn file_changes(files: Vec<FileDiff>) -> Vec<PackFileChange> {
    files
        .into_iter()
        .map(|file| {
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
            PackFileChange {
                path: file.path,
                kind: match file.kind {
                    FileChangeKind::Added => PackFileChangeKind::Added,
                    FileChangeKind::Deleted => PackFileChangeKind::Deleted,
                    FileChangeKind::Modified => PackFileChangeKind::Modified,
                    FileChangeKind::TypeChanged => PackFileChangeKind::TypeChanged,
                },
                lines,
                note: (!notes.is_empty()).then(|| notes.join("; ")),
            }
        })
        .collect()
}

/// The files that differ between two trees of `repo` (named `label` for the trust diff), and a note when the diff
/// could not be made in full.
pub fn tree_changes(
    trust: &Trust,
    repo: &Path,
    label: &str,
    old_tree: &str,
    new_tree: &str,
) -> (Vec<PackFileChange>, Option<String>) {
    let found = trust.diff(
        repo,
        &[TrustInput::new(label, old_tree)],
        &[TrustInput::new(label, new_tree)],
    );
    let mut changes = Vec::new();
    let mut note = None;
    for input in found {
        match input.detail {
            DiffDetail::Files(files) => changes.extend(file_changes(files)),
            DiffDetail::Unavailable(why) => note = Some(why),
            DiffDetail::Lines(_) | DiffDetail::None => {}
        }
    }
    (changes, note)
}
