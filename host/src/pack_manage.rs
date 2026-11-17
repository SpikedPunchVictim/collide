//! `collide pack add|link|enable|list` through the daemon (ts-0101-bd; 0.7, Repo packs and user packs, Working copies,
//! Switching packs). `pack update` is [`crate::pack_update`]'s, and the files are [`crate::user_packs`]'s.
//!
//! - [`pack_add`]: resolves a `git:` or `dev:` source, makes the pack ready (a `git:` one is fetched and materialized,
//!   a `dev:` folder is hashed), reads its manifest for the name and shows what the person would trust
//!   ([`pack_review`]). Without `confirm` nothing is written. With it, `packs.yaml`, the user's `packs.lock` and
//!   `trust.yaml` (at the tree id that was shown) are. A pack that is already added changes nothing and says so. A
//!   pack is on no board until [`pack_enable`].
//! - [`pack_link`]: overrides a pack with a working copy, for this user on this board: a `dev:` folder in the board's
//!   `packs.yaml`. With no folder it is the pack's own folder in the user's checkout.
//! - [`pack_enable`]: switches a user pack on or off for this board. A pack the board lists is switched through
//!   `board.yaml`, so it is refused.
//! - [`pack_list`]: the board's packs, then the user's that are on, then those that are off, then the folders of the
//!   checkout offered as working copies. It loads the line, so a load problem refuses it.

use crate::board_load::{LoadContext, LoadedPack, PackOrigin as LoadedOrigin, load_line};
use crate::builtin_packs::BuiltinPacks;
use crate::git_objects::GitObjects;
use crate::pack_lock::{resolve_pin, set_pin};
use crate::pack_update::{Fetched, Kind, PackError, UpdateSite, candidates, fetch_for};
use crate::trust::{Trust, check_dev_folder};
use crate::user_packs::{FILE_MODE, UserScope, manifest_name, read_global_files};
use collide_core::board::CollidePointer;
use collide_core::lock::{GitLocator, PackSource};
use collide_core::owner::{
    ConfigPackAddParams, ConfigPackEnableParams, ConfigPackLinkParams, ConfigPackResult, PackEntry,
    PackOrigin, PackReviewData, PackReviewLine,
};
use collide_core::pack::PackManifest;
use collide_core::pack_check::check_manifest;
use collide_core::settings_registry::collide_roots;
use collide_core::trust::{ReviewEntry, pack_review};
use collide_core::yaml::{MAX_YAML_BYTES, load_yaml, load_yaml_file};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

/// Where a request about one board runs: the line's git directory, the board's checkout, its id in the store, the home
/// and the paths of the Collide workspaces (which a `dev:` folder may not be under).
pub struct ManageSite<'a> {
    pub git_dir: &'a Path,
    pub root: &'a Path,
    pub board_id: &'a str,
    pub home: &'a Path,
    pub workspaces: &'a [PathBuf],
}

impl ManageSite<'_> {
    pub fn scope(&self) -> UserScope {
        UserScope::new(
            self.home.to_path_buf(),
            self.board_id.to_string(),
            self.workspaces.to_vec(),
        )
    }

    fn update_site(&self) -> UpdateSite<'_> {
        UpdateSite {
            git_dir: self.git_dir,
            checkout: self.root,
            home: self.home,
        }
    }
}

fn refused(message: impl Into<String>) -> PackError {
    PackError::Refused(message.into())
}

fn failed(message: impl ToString) -> PackError {
    PackError::Failed(message.to_string())
}

// ---- a folder as the person writes it ----

/// A folder written in a request or a source: absolute, or `~/` and the home. A relative path is the caller's to
/// make absolute (the daemon does not know where the person is).
fn folder_of(scope: &UserScope, text: &str) -> Result<PathBuf, PackError> {
    if text.is_empty() {
        return Err(refused("the folder is empty"));
    }
    if !text.starts_with('/') && !text.starts_with("~/") {
        return Err(refused(format!(
            "`{text}` is not an absolute folder: write it from `/` or `~/`"
        )));
    }
    Ok(scope.expand(text))
}

/// The folder's real path when it is a folder outside every workspace.
fn dev_folder(scope: &UserScope, folder: &Path) -> Result<PathBuf, PackError> {
    check_dev_folder(folder, scope.workspaces()).map_err(|error| refused(error.to_string()))
}

/// The manifest at `path`, read and checked. Every problem is in the refusal.
fn read_manifest(path: &Path) -> Result<PackManifest, PackError> {
    let mut bytes = Vec::new();
    fs::File::open(path)
        .and_then(|file| file.take(MAX_YAML_BYTES as u64 + 1).read_to_end(&mut bytes))
        .map_err(|error| refused(format!("{}: cannot be read: {error}", path.display())))?;
    let manifest: PackManifest = load_yaml(&bytes, "pack.yaml")
        .map_err(|error| refused(format!("{}: {error}", path.display())))?;
    if let Err(errors) = check_manifest(&manifest) {
        let lines: Vec<String> = errors.iter().map(ToString::to_string).collect();
        return Err(refused(format!("pack.yaml: {}", lines.join("; "))));
    }
    Ok(manifest)
}

// ---- the review ----

fn review_data(manifest: &PackManifest, commit: Option<&str>, tree: &str) -> PackReviewData {
    let review = pack_review(manifest);
    let lines = |entries: Vec<ReviewEntry>| -> Vec<PackReviewLine> {
        entries
            .into_iter()
            .map(|entry| PackReviewLine {
                key: entry.key,
                script: entry.script,
            })
            .collect()
    };
    PackReviewData {
        name: review.name,
        version: review.version,
        commit: commit.map(str::to_string),
        tree: tree.to_string(),
        hooks: lines(review.hooks),
        providers: lines(review.providers),
        observers: lines(review.observers),
        requires: review.requires,
    }
}

// ---- add ----

/// What `add` found out about the source before anything is written.
struct Prepared {
    manifest: PackManifest,
    tree: String,
    commit: Option<String>,
}

pub fn pack_add(
    params: &ConfigPackAddParams,
    home: &Path,
    workspaces: &[PathBuf],
    context: &LoadContext,
) -> Result<ConfigPackResult, PackError> {
    let source = params.source.as_str();
    let scope = UserScope::new(home.to_path_buf(), String::new(), workspaces.to_vec());
    let parsed =
        PackSource::parse(source).map_err(|error| refused(format!("`{source}`: {error}")))?;
    match &parsed {
        PackSource::Repo { .. } => {
            return Err(refused(format!(
                "`{source}` is a repo pack: the repo lists it in its board.yaml, and `pack add` is for your own"
            )));
        }
        PackSource::Other { scheme, .. } => {
            return Err(refused(format!(
                "`{source}`: the scheme `{scheme}:` is not supported; use `git:` or `dev:`"
            )));
        }
        PackSource::Git { .. } | PackSource::Dev { .. } => {}
    }
    let (state, problems) = read_global_files(home);
    if !problems.is_empty() {
        return Err(refused(problems.join("\n")));
    }
    let is_dev = matches!(parsed, PackSource::Dev { .. });
    if state.sources.iter().any(|have| have == source) {
        let name = scope
            .name_of(
                source,
                &state.lock,
                &context.git,
                &context.cache,
                &context.sdk,
            )
            .unwrap_or_else(|| source.to_string());
        return Ok(ConfigPackResult {
            pack: user_entry(&name, source, false, is_dev),
            written: false,
            note: Some(format!("`{source}` is already added: nothing changed")),
            needs_host: None,
        });
    }

    let prepared = match &parsed {
        PackSource::Dev { path } => prepare_dev(&scope, path)?,
        PackSource::Git { locator } => {
            match prepare_git(locator, &params.approve_hosts, context)? {
                Ok(prepared) => prepared,
                Err(host) => {
                    return Ok(ConfigPackResult {
                        pack: user_entry(source, source, false, false),
                        written: false,
                        note: None,
                        needs_host: Some(host),
                    });
                }
            }
        }
        PackSource::Repo { .. } | PackSource::Other { .. } => unreachable!("refused above"),
    };
    let name = prepared.manifest.name.clone();
    check_name_free(&name, source, &scope, &state, context)?;

    let mut entry = user_entry(&name, source, false, is_dev);
    entry.review = Some(review_data(
        &prepared.manifest,
        prepared.commit.as_deref(),
        &prepared.tree,
    ));
    if !params.confirm {
        return Ok(ConfigPackResult {
            pack: entry,
            written: false,
            note: Some("nothing was written: confirm to add it".to_string()),
            needs_host: None,
        });
    }

    let trust = Trust::new(scope.trust_store()).with_git(context.git.clone());
    if let (PackSource::Git { .. }, Some(commit)) = (&parsed, &prepared.commit) {
        let pin = collide_core::owner::PackPin {
            commit: commit.clone(),
            tree: prepared.tree.clone(),
        };
        set_pin(&scope.lock_file(), source, &pin, FILE_MODE).map_err(failed)?;
    }
    let mut sources = state.sources.clone();
    sources.push(source.to_string());
    scope.write_sources(&sources).map_err(failed)?;
    match &parsed {
        PackSource::Dev { path } => {
            let real = dev_folder(&scope, &scope.expand(path))?;
            trust
                .trust_dev(&real, workspaces)
                .map_err(|error| failed(error.to_string()))?;
        }
        _ => trust
            .trust_pack(source, &prepared.tree)
            .map_err(|error| failed(error.to_string()))?,
    }
    Ok(ConfigPackResult {
        pack: entry,
        written: true,
        note: None,
        needs_host: None,
    })
}

fn user_entry(name: &str, source: &str, enabled: bool, dev: bool) -> PackEntry {
    PackEntry {
        name: name.to_string(),
        source: source.to_string(),
        origin: PackOrigin::User,
        enabled,
        dev,
        working_copy: false,
        review: None,
    }
}

fn prepare_dev(scope: &UserScope, path: &str) -> Result<Prepared, PackError> {
    let folder = folder_of(scope, path)?;
    let real = dev_folder(scope, &folder)?;
    let manifest = read_manifest(&real.join("pack.yaml"))?;
    let tree = GitObjects::new()
        .tree_of_folder(&real)
        .map_err(|error| failed(error.to_string()))?;
    Ok(Prepared {
        manifest,
        tree,
        commit: None,
    })
}

/// A `git:` pack, fetched, resolved to a pin and materialized. `Err(host)` is a host that needs the person's yes.
fn prepare_git(
    locator: &str,
    approve_hosts: &[String],
    context: &LoadContext,
) -> Result<Result<Prepared, String>, PackError> {
    let locator = GitLocator::parse(locator).map_err(|error| refused(error.to_string()))?;
    let mirror = match fetch_for(context, &locator, approve_hosts, "pack add")? {
        Fetched::Ready(mirror) => mirror,
        Fetched::NeedsHost(host) => return Ok(Err(host)),
    };
    let pin = resolve_pin(&context.git, &mirror, &locator, None).map_err(refused)?;
    let entry = context
        .materializer
        .materialize(&context.cache, &context.sdk, &mirror, &pin.tree)
        .map_err(|error| refused(error.to_string()))?;
    let manifest = read_manifest(&entry.join("pack").join("pack.yaml"))?;
    Ok(Ok(Prepared {
        manifest,
        tree: pin.tree,
        commit: Some(pin.commit),
    }))
}

/// A user pack cannot take the name of a built-in pack, of a root of Collide's own settings, or of another pack of the
/// user's. (A repo pack's name is a load problem, not an add one: `add` has no board.)
fn check_name_free(
    name: &str,
    source: &str,
    scope: &UserScope,
    state: &crate::user_packs::UserState,
    context: &LoadContext,
) -> Result<(), PackError> {
    if BuiltinPacks::names().contains(&name) {
        return Err(refused(format!(
            "`{name}` is the name of a built-in pack, so a pack of yours cannot have it"
        )));
    }
    if collide_roots().contains(&name) {
        return Err(refused(format!(
            "`{name}` is a root of Collide's own settings, so a pack cannot have that name"
        )));
    }
    for other in state.sources.iter().filter(|other| *other != source) {
        let found = scope.name_of(
            other,
            &state.lock,
            &context.git,
            &context.cache,
            &context.sdk,
        );
        if found.as_deref() == Some(name) {
            return Err(refused(format!(
                "`{name}` is already the name of your pack `{other}`, so `{source}` cannot have it"
            )));
        }
    }
    Ok(())
}

// ---- the checkout's working copies ----

/// The Collide directory of the checkout as it is on disk: `.collide`, or the folder a `.collide` pointer file names.
pub fn collide_dir_on_disk(root: &Path) -> String {
    let path = root.join(".collide");
    if path.is_file()
        && let Ok(pointer) = load_yaml_file::<CollidePointer>(&path)
    {
        return pointer.dir;
    }
    ".collide".to_string()
}

/// The folders of `<collide dir>/packs/` in the checkout that hold a `pack.yaml`, with their names (the manifest's, or
/// the folder's when the manifest does not read), in name order.
fn checkout_pack_folders(root: &Path) -> Vec<(String, PathBuf)> {
    let packs = root.join(collide_dir_on_disk(root)).join("packs");
    let Ok(entries) = fs::read_dir(&packs) else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for entry in entries.flatten() {
        let folder = entry.path();
        let Ok(bytes) = fs::read(folder.join("pack.yaml")) else {
            continue;
        };
        let fallback = entry.file_name().to_string_lossy().into_owned();
        let name = manifest_name(&bytes, "pack.yaml").unwrap_or(fallback);
        found.push((name, folder));
    }
    found.sort();
    found
}

// ---- link ----

pub fn pack_link(
    params: &ConfigPackLinkParams,
    site: &ManageSite,
    context: &LoadContext,
) -> Result<ConfigPackResult, PackError> {
    let name = params.name.as_str();
    let scope = site.scope();
    if BuiltinPacks::names().contains(&name) {
        return Err(refused(format!(
            "`{name}` is a built-in pack: it changes with Collide, and a working copy cannot replace it"
        )));
    }
    let known = candidates(&site.update_site(), context)?;
    let board_pack = known.iter().find(|candidate| {
        candidate.origin == PackOrigin::Repo && candidate.name.as_deref() == Some(name)
    });
    if let Some(candidate) = board_pack
        && matches!(candidate.kind, Kind::Git(_))
    {
        return Err(refused(format!(
            "`{name}` is a git pack ({}): a working copy replaces a repo pack, and a git pack moves with `pack update`",
            candidate.source
        )));
    }

    let folder_text = match &params.folder {
        Some(text) => text.clone(),
        None => default_folder(name, board_pack.map(|candidate| &candidate.kind), site)?,
    };
    let folder = folder_of(&scope, &folder_text)?;
    let real = dev_folder(&scope, &folder)?;
    let manifest = read_manifest(&real.join("pack.yaml"))?;
    if manifest.name != name {
        return Err(refused(format!(
            "the folder {} holds the pack `{}`, not `{name}`, so it cannot be linked as `{name}`",
            real.display(),
            manifest.name
        )));
    }

    let mut board = scope
        .read_board()
        .map_err(|errors| refused(errors.join("\n")))?;
    let mut entry = linked_entry(name, &folder_text, board_pack.is_some());
    if board.links.get(name) == Some(&folder_text) {
        return Ok(ConfigPackResult {
            pack: entry,
            written: false,
            note: Some(format!(
                "`{name}` is already linked to {folder_text}: nothing changed"
            )),
            needs_host: None,
        });
    }
    board.links.insert(name.to_string(), folder_text.clone());
    scope.write_board(&board).map_err(failed)?;
    // The person sees the folder as it is now, and trusts it as a `dev:` folder.
    scope
        .working_copies(&context.git, &context.cache)
        .see(&real)
        .map_err(|error| failed(error.to_string()))?;
    Trust::new(scope.trust_store())
        .with_git(context.git.clone())
        .trust_dev(&real, site.workspaces)
        .map_err(|error| failed(error.to_string()))?;
    entry.enabled = true;
    Ok(ConfigPackResult {
        pack: entry,
        written: true,
        note: None,
        needs_host: None,
    })
}

fn linked_entry(name: &str, folder_text: &str, replaces_repo_pack: bool) -> PackEntry {
    PackEntry {
        name: name.to_string(),
        source: format!("dev:{folder_text}"),
        origin: if replaces_repo_pack {
            PackOrigin::Repo
        } else {
            PackOrigin::User
        },
        enabled: true,
        dev: true,
        working_copy: false,
        review: None,
    }
}

/// The folder a link with no folder uses: the pack's own in the user's checkout, which for a pack the board lists is
/// its `repo:` path, and for a folder `board.yaml` does not list is the one in the checkout with that name.
fn default_folder(
    name: &str,
    listed: Option<&Kind>,
    site: &ManageSite,
) -> Result<String, PackError> {
    let folder = match listed {
        Some(Kind::Repo(path)) => site.root.join(path),
        _ => checkout_pack_folders(site.root)
            .into_iter()
            .find(|(found, _)| found == name)
            .map(|(_, folder)| folder)
            .ok_or_else(|| {
                PackError::NotFound(format!(
                    "no pack named `{name}` is listed by this board, and no folder under `{}/packs/` in {} is a \
                     working copy of that name",
                    collide_dir_on_disk(site.root),
                    site.root.display()
                ))
            })?,
    };
    folder
        .to_str()
        .map(str::to_string)
        .ok_or_else(|| refused(format!("the folder {} is not UTF-8", folder.display())))
}

// ---- enable ----

pub fn pack_enable(
    params: &ConfigPackEnableParams,
    site: &ManageSite,
    context: &LoadContext,
) -> Result<ConfigPackResult, PackError> {
    let name = params.name.as_str();
    let scope = site.scope();
    if BuiltinPacks::names().contains(&name) {
        return Err(refused(format!(
            "`{name}` is a built-in pack: it is always on"
        )));
    }
    let known = candidates(&site.update_site(), context)?;
    if let Some(listed) = known.iter().find(|candidate| {
        candidate.origin == PackOrigin::Repo && candidate.name.as_deref() == Some(name)
    }) {
        return Err(refused(format!(
            "`{name}` is listed by this board ({}): a board's packs are switched on and off in its board.yaml, \
             not with `pack enable`",
            listed.source
        )));
    }
    let Some(own) = known.iter().find(|candidate| {
        candidate.origin == PackOrigin::User && candidate.name.as_deref() == Some(name)
    }) else {
        let mut board = scope
            .read_board()
            .map_err(|errors| refused(errors.join("\n")))?;
        if board.links.remove(name).is_some() {
            return Err(refused(format!(
                "`{name}` is a working copy you linked: it is on while it is linked"
            )));
        }
        return Err(PackError::NotFound(format!(
            "no pack named `{name}` is listed by this board or added by you"
        )));
    };
    let mut board = scope
        .read_board()
        .map_err(|errors| refused(errors.join("\n")))?;
    let is_on = board.on.iter().any(|on| on == name);
    let entry = user_entry(
        name,
        &own.source,
        params.enabled,
        own.source.starts_with("dev:"),
    );
    if is_on == params.enabled {
        let state = if params.enabled { "on" } else { "off" };
        return Ok(ConfigPackResult {
            pack: entry,
            written: false,
            note: Some(format!(
                "`{name}` is already {state} for this board: nothing changed"
            )),
            needs_host: None,
        });
    }
    if params.enabled {
        board.on.push(name.to_string());
    } else {
        board.on.retain(|on| on != name);
    }
    scope.write_board(&board).map_err(failed)?;
    Ok(ConfigPackResult {
        pack: entry,
        written: true,
        note: None,
        needs_host: None,
    })
}

// ---- list ----

pub fn pack_list(site: &ManageSite, context: LoadContext) -> Result<Vec<PackEntry>, PackError> {
    let scope = site.scope();
    let context = context.with_user(scope.clone());
    let line = load_line(site.git_dir, "HEAD", &context).map_err(|problems| {
        let lines: Vec<String> = problems.iter().map(ToString::to_string).collect();
        refused(format!(
            "the board's packs have problems, so they cannot be listed:\n{}",
            lines.join("\n")
        ))
    })?;
    let state = scope.read().map_err(|errors| refused(errors.join("\n")))?;

    let mut entries: Vec<PackEntry> = line
        .packs
        .iter()
        .map(|pack| loaded_entry(pack, site, &context.git))
        .collect();
    for source in &state.sources {
        if line
            .packs
            .iter()
            .any(|pack| pack.source.as_deref() == Some(source.as_str()))
        {
            continue;
        }
        let name = scope
            .name_of(
                source,
                &state.lock,
                &context.git,
                &context.cache,
                &context.sdk,
            )
            .unwrap_or_else(|| source.clone());
        entries.push(user_entry(&name, source, false, source.starts_with("dev:")));
    }
    for (name, folder) in checkout_pack_folders(site.root) {
        if entries.iter().any(|entry| entry.name == name) {
            continue;
        }
        entries.push(PackEntry {
            name,
            source: format!("dev:{}", folder.display()),
            origin: PackOrigin::Offered,
            enabled: false,
            dev: false,
            working_copy: false,
            review: None,
        });
    }
    Ok(entries)
}

fn loaded_entry(pack: &LoadedPack, site: &ManageSite, git: &GitObjects) -> PackEntry {
    let (origin, source) = match pack.origin {
        LoadedOrigin::Builtin => (PackOrigin::Builtin, "builtin".to_string()),
        LoadedOrigin::Repo | LoadedOrigin::Git => {
            (PackOrigin::Repo, pack.source.clone().unwrap_or_default())
        }
        LoadedOrigin::User => (PackOrigin::User, pack.source.clone().unwrap_or_default()),
        LoadedOrigin::Linked => (
            if pack.overrides.is_some() {
                PackOrigin::Repo
            } else {
                PackOrigin::User
            },
            pack.source.clone().unwrap_or_default(),
        ),
    };
    PackEntry {
        name: pack.name.clone(),
        source,
        origin,
        enabled: true,
        dev: pack.dev,
        working_copy: has_working_copy(pack, site, git),
        review: None,
    }
}

/// A repo pack whose folder in the user's checkout holds other files than the line's ref does.
fn has_working_copy(pack: &LoadedPack, site: &ManageSite, git: &GitObjects) -> bool {
    if pack.origin != LoadedOrigin::Repo {
        return false;
    }
    let Some(PackSource::Repo { path }) = pack
        .source
        .as_deref()
        .and_then(|source| PackSource::parse(source).ok())
    else {
        return false;
    };
    let folder = site.root.join(path);
    folder.is_dir()
        && git
            .tree_of_folder(&folder)
            .is_ok_and(|tree| pack.pin.as_deref() != Some(tree.as_str()))
}
