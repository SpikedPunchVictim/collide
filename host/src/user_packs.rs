//! The user's packs (ts-0101-bd; 0.7, Repo packs and user packs, Switching packs).
//!
//! Three files, all Collide's own (written by `yaml_out`, atomically, mode `0600`):
//!
//! - `~/.config/collide/packs.yaml` ([`UserPacks`]): the sources the user added, `git:` or `dev:`, in the order they
//!   load after the board's. A pack is on no board until a board switches it on.
//! - `~/.config/collide/packs.lock`: the pins of the `git:` ones (the same format as the repo's lock).
//! - `~/.config/collide/boards/<board-id>/packs.yaml` ([`BoardPacks`]): for one board, the names switched on (`on`) and
//!   the working copies linked (`links`: a pack's name to a folder). `<board-id>` is the store's name for the board,
//!   as for the board's `config.yaml` ([`crate::settings_files`]).
//!
//! [`UserScope`] is what a load needs to use them: the home, the board's id and the paths of the Collide workspaces
//! (the worktree root, and the workspace paths the store holds), which a `dev:` folder may not be under.
//! [`UserScope::read`] reads and checks the files; [`UserScope::name_of`] finds a pack's name without materializing it,
//! so a pack that is off is not fetched and not checked.

use crate::atomic_write::write_atomically;
use crate::git_fetch::mirror_path;
use crate::git_objects::GitObjects;
use crate::pack_cache::{cached_entry, sdk_version};
use crate::pack_lock::read_lock;
use crate::settings_files::user_config_dir;
use crate::trust::TrustStore;
use crate::working_copy::WorkingCopies;
use collide_core::lock::{
    GitLocator, PackLock, PackSource, UserPackEntry, UserPacks, check_lock, check_user_packs,
};
use collide_core::resolve::is_safe_token;
use collide_core::yaml::{YamlError, load_yaml, load_yaml_file};
use collide_core::yaml_out::to_yaml;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};

pub const PACKS_FILE: &str = "packs.yaml";
pub const LOCK_FILE: &str = "packs.lock";
/// The mode of every file here: the lock is the user's, and the other two name folders on their machine.
pub const FILE_MODE: u32 = 0o600;
const DIR_MODE: u32 = 0o700;

const USER_HEADER: &str = "The packs you added with `collide pack add`. Collide writes this file; a board switches a \
                           pack on with `collide pack enable`.";
const BOARD_HEADER: &str = "For this board: the user packs switched on (`on`) and the working copies linked \
                            (`links`). Collide writes this file: use `collide pack enable|disable|link`.";

/// `packs.yaml` of one board's folder.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BoardPacks {
    /// The names of the user packs that are on, in the order they were switched on.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub on: Vec<String>,
    /// A pack's name to the folder of its working copy, as the person wrote it (`~/` is the home).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub links: BTreeMap<String, String>,
}

/// What the three files hold.
#[derive(Debug, Clone, Default)]
pub struct UserState {
    pub sources: Vec<String>,
    pub lock: PackLock,
    pub board: BoardPacks,
}

/// The user, the board and the paths a `dev:` folder may not be under.
#[derive(Debug, Clone)]
pub struct UserScope {
    home: PathBuf,
    board_id: String,
    workspaces: Vec<PathBuf>,
}

impl UserScope {
    pub fn new(home: PathBuf, board_id: String, workspaces: Vec<PathBuf>) -> UserScope {
        UserScope {
            home,
            board_id,
            workspaces,
        }
    }

    pub fn home(&self) -> &Path {
        &self.home
    }

    pub fn board_id(&self) -> &str {
        &self.board_id
    }

    pub fn workspaces(&self) -> &[PathBuf] {
        &self.workspaces
    }

    pub fn config_dir(&self) -> PathBuf {
        user_config_dir(&self.home)
    }

    pub fn packs_file(&self) -> PathBuf {
        self.config_dir().join(PACKS_FILE)
    }

    pub fn lock_file(&self) -> PathBuf {
        self.config_dir().join(LOCK_FILE)
    }

    /// The board's folder, or why the board's id cannot name one.
    pub fn board_dir(&self) -> Result<PathBuf, String> {
        if !is_safe_token(&self.board_id) {
            return Err(format!(
                "the board id {:?} is not a safe name for a folder",
                self.board_id
            ));
        }
        Ok(self.config_dir().join("boards").join(&self.board_id))
    }

    pub fn board_file(&self) -> Result<PathBuf, String> {
        self.board_dir().map(|dir| dir.join(PACKS_FILE))
    }

    /// A folder as written in a source or a link: `~/` is the home.
    pub fn expand(&self, path: &str) -> PathBuf {
        match path.strip_prefix("~/") {
            Some(rest) => self.home.join(rest),
            None => PathBuf::from(path),
        }
    }

    pub fn trust_store(&self) -> TrustStore {
        TrustStore::new(self.config_dir())
    }

    /// The working copies of this user: seen and integrated trees in `trust.yaml`, objects in the cache.
    pub fn working_copies(&self, git: &GitObjects, cache: &Path) -> WorkingCopies {
        WorkingCopies::new(
            self.trust_store(),
            git.clone(),
            cache.join("working-copies.git"),
        )
    }

    /// The board's own `packs.yaml`, checked. Empty when there is none.
    pub fn read_board(&self) -> Result<BoardPacks, Vec<String>> {
        let file = self.board_file().map_err(|error| vec![error])?;
        let found = read_optional::<BoardPacks>(&file)
            .map_err(|error| vec![error])?
            .unwrap_or_default();
        let errors = check_board_packs(&found, &file);
        if errors.is_empty() {
            Ok(found)
        } else {
            Err(errors)
        }
    }

    /// Reads and checks the three files. Every problem is returned, each naming its file.
    pub fn read(&self) -> Result<UserState, Vec<String>> {
        let (mut state, mut errors) = read_global_files(&self.home);
        match self.read_board() {
            Ok(board) => state.board = board,
            Err(found) => errors.extend(found),
        }
        if errors.is_empty() {
            Ok(state)
        } else {
            Err(errors)
        }
    }

    /// The name in the manifest of the pack `source`, read where it is without making anything ([`git_pack_name`] for
    /// a `git:` pack, the folder's `pack.yaml` for a `dev:` one). `None` when it cannot be read, and then the load
    /// goes and looks.
    pub fn name_of(
        &self,
        source: &str,
        lock: &PackLock,
        git: &GitObjects,
        cache: &Path,
        sdk: &Path,
    ) -> Option<String> {
        match PackSource::parse(source).ok()? {
            PackSource::Git { .. } => git_pack_name(git, cache, sdk, source, lock),
            PackSource::Dev { path } => {
                let bytes = fs::read(self.expand(&path).join("pack.yaml")).ok()?;
                manifest_name(&bytes, "pack.yaml")
            }
            PackSource::Repo { .. } | PackSource::Other { .. } => None,
        }
    }

    /// Writes the user's `packs.yaml`.
    pub fn write_sources(&self, sources: &[String]) -> Result<(), String> {
        let packs = UserPacks {
            packs: sources
                .iter()
                .map(|source| UserPackEntry {
                    source: source.clone(),
                })
                .collect(),
        };
        write_owned(&self.packs_file(), &packs, USER_HEADER)
    }

    /// Writes the board's `packs.yaml`.
    pub fn write_board(&self, board: &BoardPacks) -> Result<(), String> {
        write_owned(&self.board_file()?, board, BOARD_HEADER)
    }
}

/// The user's `packs.yaml` and `packs.lock`, which no board's folder is needed for. Every problem is returned in the
/// second place, each naming its file.
pub fn read_global_files(home: &Path) -> (UserState, Vec<String>) {
    let dir = user_config_dir(home);
    let mut errors = Vec::new();
    let mut state = UserState::default();

    let packs = dir.join(PACKS_FILE);
    match read_optional::<UserPacks>(&packs) {
        Ok(found) => {
            let found = found.unwrap_or_default();
            if let Err(problems) = check_user_packs(&found) {
                errors.extend(
                    problems
                        .iter()
                        .map(|problem| format!("{}: {problem}", packs.display())),
                );
            }
            state.sources = found.packs.into_iter().map(|entry| entry.source).collect();
        }
        Err(error) => errors.push(error),
    }

    let lock = dir.join(LOCK_FILE);
    match read_lock(&lock) {
        Ok(found) => {
            if let Err(problems) = check_lock(&found) {
                errors.extend(
                    problems
                        .iter()
                        .map(|problem| format!("{}: {problem}", lock.display())),
                );
            }
            state.lock = found;
        }
        Err(error) => errors.push(error),
    }
    (state, errors)
}

/// The name of the `git:` pack `source` at the pin `lock` holds for it, read without making anything: from the
/// cache entry when it is there, else from the mirror at the locked commit. `None` when neither has it.
pub fn git_pack_name(
    git: &GitObjects,
    cache: &Path,
    sdk: &Path,
    source: &str,
    lock: &PackLock,
) -> Option<String> {
    let locked = lock.packs.iter().find(|locked| locked.source == source)?;
    if let Ok(version) = sdk_version(sdk)
        && let Ok(Some(entry)) = cached_entry(cache, &locked.tree, &version)
        && let Ok(bytes) = fs::read(entry.join("pack").join("pack.yaml"))
    {
        return manifest_name(&bytes, "pack.yaml");
    }
    let PackSource::Git { locator } = PackSource::parse(source).ok()? else {
        return None;
    };
    let locator = GitLocator::parse(&locator).ok()?;
    let mirror = mirror_path(cache, &locator.url);
    let bytes = git
        .blob_at(&mirror, &locked.commit, &manifest_path(&locator))
        .ok()??;
    manifest_name(&bytes, "pack.yaml")
}

/// The path of a pack's manifest in its repository.
pub fn manifest_path(locator: &GitLocator) -> String {
    if locator.path.is_empty() {
        "pack.yaml".to_string()
    } else {
        format!("{}/pack.yaml", locator.path)
    }
}

#[derive(Deserialize)]
struct ManifestName {
    name: String,
}

/// The `name` of a manifest, or `None` when it does not read.
pub fn manifest_name(bytes: &[u8], origin: &str) -> Option<String> {
    load_yaml::<ManifestName>(bytes, origin)
        .ok()
        .map(|manifest| manifest.name)
}

fn read_optional<T: serde::de::DeserializeOwned>(path: &Path) -> Result<Option<T>, String> {
    match load_yaml_file::<Option<T>>(path) {
        Ok(found) => Ok(found),
        Err(YamlError::Read { source, .. }) if source.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.to_string()),
    }
}

/// Every name in `on` and `links` is a safe token, and a name is listed once.
fn check_board_packs(board: &BoardPacks, file: &Path) -> Vec<String> {
    let mut errors = Vec::new();
    let mut seen: Vec<&str> = Vec::new();
    for name in &board.on {
        if !is_safe_token(name) {
            errors.push(format!(
                "{}: `on` holds {name:?}, which is not a pack name",
                file.display()
            ));
        }
        if seen.contains(&name.as_str()) {
            errors.push(format!("{}: `on` lists `{name}` twice", file.display()));
        }
        seen.push(name);
    }
    for (name, folder) in &board.links {
        if !is_safe_token(name) {
            errors.push(format!(
                "{}: `links` holds {name:?}, which is not a pack name",
                file.display()
            ));
        }
        if folder.is_empty() {
            errors.push(format!(
                "{}: the link of `{name}` has no folder",
                file.display()
            ));
        }
    }
    errors
}

/// Writes `value` as the YAML file `file`, in a folder of mode `0700`: a temp file beside it, then a rename.
fn write_owned<T: Serialize>(file: &Path, value: &T, header: &str) -> Result<(), String> {
    let text = to_yaml(value, header).map_err(|error| format!("{}: {error}", file.display()))?;
    let dir = file.parent().unwrap_or(Path::new("."));
    let name = file
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("{} has no file name", file.display()))?;
    fs::DirBuilder::new()
        .recursive(true)
        .mode(DIR_MODE)
        .create(dir)
        .map_err(|error| format!("{}: {error}", dir.display()))?;
    write_atomically(dir, name, text.as_bytes(), FILE_MODE, |file| file)
        .map_err(|error| error.to_string())
}
