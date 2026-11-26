//! `config.get` (ts-0098-nx; 0.7, Settings and Profiles): the four layers of a board, with the layer each value came
//! from. The repo's layer and the packs' schemas come from the loader ([`load_line`]) at the checkout's `HEAD`, so an
//! edit that is not committed is not used, as for every other thing the loader reads. The user's layers come from
//! `~/.config/collide` ([`crate::settings_files`]). The merge is `core`'s ([`settings_layers`]); this file only gathers
//! its inputs and says how a failure is told.
//!
//! A failure is the error of the request: [`GetError::Refused`] (the files or packs have problems, one per line, each
//! naming its key or file) is `CONFLICT`, and [`GetError::NotFound`] (a key, a profile or a board that is not there) is
//! `NOT_FOUND`. A request that sets `with_problems` (the Settings page, ts-0193-gk) gets the problems of single
//! settings in the result instead, with the entries the layers still give; a file or a pack that does not load still
//! refuses it. With no board the answer is the user's layers over Collide's own keys, and no pack is loaded.
//!
//! The agent entries ([`collide_core::agent_layers`]) answer `agents.<id>.<field>`, and `agents.<id>` for every field
//! of one entry; the whole listing stays Collide's own keys and the packs'. A notice (a default budget an agent cannot
//! enforce) rides along in `notices`. The checks that need the machine ([`crate::agent_checks`]) run here, so a `read`
//! folder that is gone or a keychain item that is missing is a refusal naming it.
//!
//! `config.set` ([`ConfigService::set`], ts-0099-bw) shares the gathering with `get`. A secret goes to the keychain
//! and nowhere else: no value is logged, journaled, put in an error or returned. A plain value is written to the
//! user's file through `yaml_out`, which drops comments, so a file that holds one is refused and not rewritten.

use crate::agent_checks::check_agent_environment;
use crate::atomic_write::write_atomically;
use crate::board_lines::Lines;
use crate::board_load::{LoadContext, LoadProblem, LoadedLine, load_defaults, load_line};
use crate::boards;
use crate::builtin_packs::{BuiltinPacks, Environment};
use crate::disk::Floor;
use crate::git_fetch::{FetchEnv, GitFetch};
use crate::hosts::KnownHosts;
use crate::pack_cache::ensure_sdk;
use crate::pack_location::{Inputs, bundle_sdk, locate_bundle};
use crate::pack_manage::{ManageSite, pack_add, pack_enable, pack_link, pack_list};
use crate::pack_update::{PackError, UpdateSite, pack_update};
use crate::secret_store::{Secret, SecretStore, SecretStoreError, pack_item};
use crate::settings_files::{UserFiles, read_user_files, user_config_dir};
use crate::user_packs::UserScope;
use collide_core::agent_layers::{AgentInputs, AgentLayers, agent_layers};
use collide_core::owner::{
    ConfigEntry, ConfigGetParams, ConfigGetResult, ConfigLayerFile, ConfigPackAddParams,
    ConfigPackEnableParams, ConfigPackLinkParams, ConfigPackResult, ConfigPackUpdateParams,
    ConfigPackUpdateResult, ConfigSetParams, ConfigSetResult, ConfigWriteLayer, PackEntry,
    PackOrigin, SettingLayer,
};
use collide_core::profile::{FoundProfile, Profile, ProfileLookupError, find_profile};
use collide_core::resolve::is_safe_token;
use collide_core::setting_value::{check_value, value_from_text};
use collide_core::settings::ConfigFile;
use collide_core::settings_layers::{
    KeyInfo, LayerInputs, PackSettings, ProfileUse, SettingProblem, Settings, find_setting,
    settings_layers,
};
use collide_core::settings_registry::{MIN_FLOOR_GIB, collide_roots};
use collide_core::yaml_out::{has_comment, to_yaml};
use serde_json::Value;
use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Makes the loader's context for one `config.get`; the argument is the `PATH` tools are looked for on.
pub type LoadContextFactory = Arc<dyn Fn(&OsStr) -> Result<LoadContext, String> + Send + Sync>;

/// The factory a daemon uses when its options give none: the built-in packs and the SDK are found from the process
/// ([`locate_bundle`]), and the SDK is copied once into `cache` ([`ensure_sdk`]). It is read at each call, so a daemon
/// that starts before the bundle is in place still answers once it is. With a `home`, a `git:` pack that is not in the
/// cache can be fetched (`ts-0100-pu`): git runs with the fetch's own environment from the process, and the hosts the
/// user has allowed are read from `<home>/.config/collide/hosts.yaml`.
pub fn system_load_context(
    cache: PathBuf,
    bundle: Inputs,
    home: Option<PathBuf>,
) -> LoadContextFactory {
    Arc::new(move |path| {
        let bundle = locate_bundle(&bundle).map_err(|error| error.to_string())?;
        let sdk = bundle_sdk(&bundle).map_err(|error| error.to_string())?;
        let sdk = ensure_sdk(&cache, &sdk).map_err(|error| error.to_string())?;
        let context = LoadContext::new(
            cache.clone(),
            sdk,
            Environment {
                path: path.to_os_string(),
            },
            BuiltinPacks::new(bundle),
        );
        Ok(match &home {
            Some(home) => {
                let env = FetchEnv {
                    home: home.clone(),
                    ..FetchEnv::from_process()
                };
                context.with_fetch(GitFetch::new(env), KnownHosts::for_home(home))
            }
            None => context,
        })
    })
}

/// Why `config.get` gave no entries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GetError {
    /// A key, a profile or a board that is not there.
    NotFound(String),
    /// Files or packs with problems, one on each line of the message.
    Refused(String),
}

/// Why `config.set` wrote nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SetError {
    /// A board that is not there.
    NotFound(String),
    /// A key or a value that is wrong: the message names the key and never the value.
    Invalid(String),
    /// Something in the way: a repo write, a file with a comment, a file that does not read, a write that would make
    /// the settings fail.
    Refused(String),
    /// The keychain or the disk failed.
    Failed(String),
}

/// The board a request is about: the store's id for it (the name of the user's per-board folder) and its checkout.
#[derive(Debug, Clone, Copy)]
pub struct BoardRef<'a> {
    pub id: &'a str,
    pub root: &'a Path,
}

const FLOOR_KEY: &str = "disk.floor_gib";

#[derive(Clone)]
pub struct ConfigService {
    home: Option<PathBuf>,
    load_context: LoadContextFactory,
    git_env: Vec<(String, String)>,
    secret_store: Arc<dyn SecretStore>,
    workspaces: WorkspaceRoots,
}

/// The folders no `dev:` pack may be under: the worktree root and every workspace path the store holds. Asked at each
/// request, because a run makes a new one.
pub type WorkspaceRoots = Arc<dyn Fn() -> Vec<PathBuf> + Send + Sync>;

/// What a request starts from: the user's files, the repo's line, and the problems of reading them.
struct Gathered {
    user: UserFiles,
    line: Option<LoadedLine>,
    problems: Vec<String>,
}

/// The layers over one set of files.
struct Resolved {
    settings: Settings,
    agents: AgentLayers,
}

impl Resolved {
    /// Every problem of the layers and of the agent entries.
    fn problem_items(&self) -> Vec<SettingProblem> {
        let settings = self.settings.problems.iter();
        let agents = self.agents.problems.iter();
        settings.chain(agents).cloned().collect()
    }

    /// Every problem of the layers and of the agent entries, one line each.
    fn problems(&self) -> Vec<String> {
        self.problem_items()
            .iter()
            .map(ToString::to_string)
            .collect()
    }
}

/// The problems of a load, one on each line.
fn problem_lines(problems: &[LoadProblem]) -> String {
    let lines: Vec<String> = problems.iter().map(ToString::to_string).collect();
    lines.join("\n")
}

fn pack_settings(line: Option<&LoadedLine>) -> Vec<PackSettings<'_>> {
    line.map_or_else(Vec::new, |line| {
        line.packs
            .iter()
            .map(|pack| PackSettings {
                pack: &pack.name,
                config: &pack.manifest.config,
            })
            .collect()
    })
}

fn resolve(user: &UserFiles, line: Option<&LoadedLine>, profile: Option<ProfileUse>) -> Resolved {
    let packs = pack_settings(line);
    let empty = ConfigFile::default();
    let repo = line.map_or(&empty, |line| &line.config);
    let settings = settings_layers(&LayerInputs {
        packs: &packs,
        user: &user.global,
        user_board: &user.board,
        repo,
        profile,
    });
    let agents = agent_layers(&AgentInputs {
        user: &user.global,
        user_board: &user.board,
        repo,
    });
    let mut settings = settings;
    // `ask.agent` has no default of its own: with no layer setting it, it is the agent the entries resolve to.
    if let Some(entry) = settings
        .entries
        .iter_mut()
        .find(|entry| entry.key == "ask.agent" && entry.value.is_none())
    {
        entry.value = Some(Value::String(agents.ask_agent.clone()));
    }
    Resolved { settings, agents }
}

/// The floor a board's runs start above, and what is wrong with the settings it was read from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FloorRead {
    pub floor: Floor,
    pub problems: Vec<String>,
}

impl ConfigService {
    pub fn new(
        home: Option<String>,
        load_context: LoadContextFactory,
        git_env: Vec<(String, String)>,
        secret_store: Arc<dyn SecretStore>,
    ) -> ConfigService {
        ConfigService {
            home: home.filter(|home| !home.is_empty()).map(PathBuf::from),
            load_context,
            git_env,
            secret_store,
            workspaces: Arc::new(Vec::new),
        }
    }

    /// The free-space floor for `board` (the user's layers alone with none): `disk.floor_gib` through the layers, so
    /// the repo's value, or the user's when the key is overridable. A value the layers refuse is a problem and the
    /// default holds. The keychain and the agent entries are not read: only this one key matters here.
    pub fn floor(&self, board: Option<BoardRef>, path: &OsStr) -> FloorRead {
        let gathered = match self.gather(board, path) {
            Ok(gathered) => gathered,
            Err(message) => {
                return FloorRead {
                    floor: Floor::default(),
                    problems: vec![message],
                };
            }
        };
        let resolved = resolve(&gathered.user, gathered.line.as_ref(), None);
        let mut problems = gathered.problems;
        problems.extend(
            resolved
                .settings
                .problems
                .iter()
                .filter(|problem| problem.key == FLOOR_KEY)
                .map(ToString::to_string),
        );
        let gib = resolved
            .settings
            .entries
            .iter()
            .find(|entry| entry.key == FLOOR_KEY)
            .and_then(|entry| entry.value.as_ref())
            .and_then(Value::as_u64)
            .filter(|gib| *gib >= MIN_FLOOR_GIB);
        FloorRead {
            floor: gib.map_or_else(Floor::default, Floor::from_gib),
            problems,
        }
    }

    /// The settings of the pack `pack` as its provider is given them in `initialize`: each key of the pack without
    /// the pack's name, with its value through the layers, and a secret read from the secret store at start. A
    /// setting with no value is left out, and so is a secret the store has no item for. No value is logged, and no
    /// message here holds one. `line` is the line the pack was loaded from, and `board` is its board.
    pub fn pack_config(
        &self,
        board: BoardRef,
        line: &LoadedLine,
        pack: &str,
    ) -> Result<BTreeMap<String, Value>, String> {
        let user =
            read_user_files(self.home()?, Some(board.id)).map_err(|errors| errors.join("\n"))?;
        let resolved = resolve(&user, Some(line), None);
        let prefix = format!("{pack}.");
        let problems: Vec<String> = resolved
            .settings
            .problems
            .iter()
            .filter(|problem| problem.key.starts_with(&prefix))
            .map(ToString::to_string)
            .collect();
        if !problems.is_empty() {
            return Err(problems.join("\n"));
        }
        let mut config = BTreeMap::new();
        for entry in &resolved.settings.entries {
            let Some(key) = entry.key.strip_prefix(&prefix) else {
                continue;
            };
            if entry.secret {
                match self.secret_store.get(&pack_item(pack, key)) {
                    Ok(secret) => {
                        config.insert(key.to_string(), Value::String(secret.expose().to_string()));
                    }
                    Err(SecretStoreError::NotFound { .. }) => {}
                    Err(error) => return Err(error.to_string()),
                }
            } else if let Some(value) = &entry.value {
                config.insert(key.to_string(), value.clone());
            }
        }
        Ok(config)
    }

    /// The lines of boards with their trust, over the same loader, home and workspaces ([`crate::board_lines`]).
    pub fn lines(&self) -> Lines {
        Lines::new(
            self.home.clone(),
            self.load_context.clone(),
            self.git_env.clone(),
            self.workspaces.clone(),
        )
    }

    /// Where the Collide workspaces are, for the `dev:` refusal. Without it no folder is a workspace.
    pub fn with_workspaces(mut self, workspaces: WorkspaceRoots) -> ConfigService {
        self.workspaces = workspaces;
        self
    }

    fn home(&self) -> Result<&Path, String> {
        self.home.as_deref().ok_or_else(|| {
            "the daemon knows no home directory, so the user's settings cannot be found".to_string()
        })
    }

    /// The user's files and the line of `board`. What went wrong is in `problems`, and the rest is still read.
    fn gather(&self, board: Option<BoardRef>, path: &OsStr) -> Result<Gathered, String> {
        let home = self.home()?;
        let mut problems = Vec::new();
        let user = match read_user_files(home, board.map(|board| board.id)) {
            Ok(files) => files,
            Err(errors) => {
                problems.extend(errors);
                UserFiles::default()
            }
        };
        let line = board.and_then(|board| self.load(board, path, &mut problems));
        Ok(Gathered {
            user,
            line,
            problems,
        })
    }

    /// The settings of `board` (or the user's alone), with `params.profile` on top, narrowed to `params.key`.
    /// `path` is the `PATH` the packs' `requires.tools` are looked for on.
    pub fn get(
        &self,
        params: &ConfigGetParams,
        board: Option<BoardRef>,
        path: &OsStr,
    ) -> Result<ConfigGetResult, GetError> {
        let gathered = self.gather(board, path).map_err(GetError::Refused)?;
        if !gathered.problems.is_empty() {
            return Err(GetError::Refused(gathered.problems.join("\n")));
        }
        let files = self.layer_files(board, gathered.line.as_ref());
        self.answer(params, board, gathered, files)
    }

    /// [`ConfigService::get`] for a defaults connection (ts-0215-gt): the Default layer over the built-in packs alone.
    /// No settings file, no user pack and nothing of the repo is read, so none of their problems can show, and no
    /// file is named as a layer's.
    pub fn get_defaults(
        &self,
        params: &ConfigGetParams,
        board: Option<BoardRef>,
        path: &OsStr,
    ) -> Result<ConfigGetResult, GetError> {
        let context = (self.load_context)(path)
            .map_err(|message| GetError::Refused(format!("the loader cannot start: {message}")))?;
        let line = load_defaults(&context)
            .map_err(|problems| GetError::Refused(problem_lines(&problems)))?;
        let gathered = Gathered {
            user: UserFiles::default(),
            line: Some(line),
            problems: Vec::new(),
        };
        self.answer(params, board, gathered, Vec::new())
    }

    /// The answer to `config.get` over what was gathered, with `files` as the layers' files.
    fn answer(
        &self,
        params: &ConfigGetParams,
        board: Option<BoardRef>,
        gathered: Gathered,
        files: Vec<ConfigLayerFile>,
    ) -> Result<ConfigGetResult, GetError> {
        let Gathered { user, line, .. } = &gathered;

        let no_profiles = BTreeMap::new();
        let repo_profiles = line.as_ref().map_or(&no_profiles, |line| &line.profiles);
        let pack_profiles: Vec<(String, BTreeMap<String, Profile>)> =
            line.as_ref().map_or_else(Vec::new, |line| {
                line.packs
                    .iter()
                    .map(|pack| (pack.name.clone(), pack.manifest.profiles.clone()))
                    .collect()
            });
        let found: Option<(&String, FoundProfile)> = match &params.profile {
            None => None,
            Some(name) => Some((
                name,
                find_profile(name, repo_profiles, &user.profiles, &pack_profiles).map_err(
                    |error| match error {
                        ProfileLookupError::NotFound { .. } => {
                            GetError::NotFound(error.to_string())
                        }
                        ProfileLookupError::InTwoPlaces { .. } => {
                            GetError::Refused(error.to_string())
                        }
                    },
                )?,
            )),
        };
        let profile = found.as_ref().map(|(name, found)| ProfileUse {
            name,
            source: &found.source,
            profile: found.profile,
        });
        let resolved = resolve(user, line.as_ref(), profile);

        let mut problems = resolved.problem_items();
        let board_root = board.map(|board| board.root);
        problems.extend(check_agent_environment(
            &resolved.agents.agents,
            board_root,
            self.secret_store.as_ref(),
        ));

        let notices = resolved.agents.notices.clone();
        let mut narrowed = narrow(resolved, params.key.as_deref());
        if let Some(entries) = narrowed.as_mut() {
            let packs = pack_settings(line.as_ref());
            problems.extend(self.secret_state(entries, &packs));
        }
        if !params.with_problems && !problems.is_empty() {
            let lines: Vec<String> = problems.iter().map(ToString::to_string).collect();
            return Err(GetError::Refused(lines.join("\n")));
        }
        let entries = narrowed.ok_or_else(|| {
            GetError::NotFound(not_found(
                params.key.as_deref().unwrap_or(""),
                board.is_some(),
            ))
        })?;
        Ok(ConfigGetResult {
            entries,
            problems: problems.iter().map(Into::into).collect(),
            files,
            notices,
        })
    }

    /// Whether the keychain holds each secret of `entries`, put in `is_set`. Only the item's presence is asked, never
    /// its value. A keychain that cannot say is a problem naming the key, and leaves `is_set` out.
    fn secret_state(
        &self,
        entries: &mut [ConfigEntry],
        packs: &[PackSettings],
    ) -> Vec<SettingProblem> {
        let mut problems = Vec::new();
        for entry in entries.iter_mut().filter(|entry| entry.secret) {
            let Some(pack) = find_setting(packs, &entry.key).and_then(|info| info.pack) else {
                continue;
            };
            let item = pack_item(&pack, &entry.key[pack.len() + 1..]);
            match self.secret_store.contains(&item) {
                Ok(held) => entry.is_set = Some(held),
                Err(error) => problems.push(SettingProblem {
                    key: entry.key.clone(),
                    layer: SettingLayer::User,
                    message: format!("`{}`: the keychain cannot be asked: {error}", entry.key),
                }),
            }
        }
        problems
    }

    /// The files the layers are read from, as this daemon resolves them: the user's, the user's for this board, and
    /// the repo's (read at the checkout's `HEAD`, from the Collide folder the line found).
    fn layer_files(
        &self,
        board: Option<BoardRef>,
        line: Option<&LoadedLine>,
    ) -> Vec<ConfigLayerFile> {
        let Some(home) = self.home.as_deref() else {
            return Vec::new();
        };
        let user_dir = user_config_dir(home);
        let user_file = |layer, for_board, dir: PathBuf| {
            let path = dir.join("config.yaml");
            ConfigLayerFile {
                layer,
                for_board,
                exists: path.is_file(),
                path: path.display().to_string(),
            }
        };
        let mut files = vec![user_file(SettingLayer::User, false, user_dir.clone())];
        let Some(board) = board else {
            return files;
        };
        if is_safe_token(board.id) {
            let dir = user_dir.join("boards").join(board.id);
            files.push(user_file(SettingLayer::User, true, dir));
        }
        let collide_dir = line
            .and_then(|line| line.collide_dir.as_deref())
            .unwrap_or(".collide");
        let repo = board.root.join(collide_dir).join("config.yaml");
        files.push(ConfigLayerFile {
            layer: SettingLayer::Repo,
            for_board: false,
            exists: line.is_some_and(|line| {
                line.trust_inputs()
                    .iter()
                    .any(|(name, _)| name == "config.yaml")
            }),
            path: repo.display().to_string(),
        });
        files
    }

    /// `config.pack_update`: moves a `git:` pack to a new pin. See [`crate::pack_update`].
    pub fn pack_update(
        &self,
        params: &ConfigPackUpdateParams,
        board: BoardRef,
        path: &OsStr,
    ) -> Result<ConfigPackUpdateResult, PackError> {
        let home = self.home().map_err(PackError::Failed)?;
        let git_dir = boards::git_dir_of(board.root, &self.git_env).map_err(PackError::Failed)?;
        let context = (self.load_context)(path)
            .map_err(|message| PackError::Failed(format!("the loader cannot start: {message}")))?;
        let site = UpdateSite {
            git_dir: &git_dir,
            checkout: board.root,
            home,
        };
        pack_update(params, &site, &context)
    }

    /// Where a `config.pack_*` request runs, with the context the loader needs. `f` gets both.
    fn with_pack_site<T>(
        &self,
        board: BoardRef,
        path: &OsStr,
        f: impl FnOnce(&ManageSite, LoadContext) -> Result<T, PackError>,
    ) -> Result<T, PackError> {
        let home = self.home().map_err(PackError::Failed)?;
        let git_dir = boards::git_dir_of(board.root, &self.git_env).map_err(PackError::Failed)?;
        let context = (self.load_context)(path)
            .map_err(|message| PackError::Failed(format!("the loader cannot start: {message}")))?;
        let workspaces = (self.workspaces)();
        let site = ManageSite {
            git_dir: &git_dir,
            root: board.root,
            board_id: board.id,
            home,
            workspaces: &workspaces,
        };
        f(&site, context)
    }

    /// `config.pack_list`: the board's packs, the user's, and the working copies on offer. See [`crate::pack_manage`].
    pub fn pack_list(&self, board: BoardRef, path: &OsStr) -> Result<Vec<PackEntry>, PackError> {
        self.with_pack_site(board, path, pack_list)
    }

    /// [`ConfigService::pack_list`] for a defaults connection (ts-0215-gt): the built-in packs, and no other. The
    /// user's files, the board's and the repo's checkout are not read.
    pub fn pack_list_defaults(&self, path: &OsStr) -> Result<Vec<PackEntry>, PackError> {
        let context = (self.load_context)(path)
            .map_err(|message| PackError::Failed(format!("the loader cannot start: {message}")))?;
        let line = load_defaults(&context)
            .map_err(|problems| PackError::Failed(problem_lines(&problems)))?;
        Ok(line
            .packs
            .iter()
            .map(|pack| PackEntry {
                name: pack.name.clone(),
                source: "builtin".to_string(),
                origin: PackOrigin::Builtin,
                enabled: true,
                dev: false,
                working_copy: false,
                review: None,
            })
            .collect())
    }

    /// `config.pack_add`: a user pack, shown and then, with `confirm`, written.
    pub fn pack_add(
        &self,
        params: &ConfigPackAddParams,
        path: &OsStr,
    ) -> Result<ConfigPackResult, PackError> {
        let home = self.home().map_err(PackError::Failed)?;
        let context = (self.load_context)(path)
            .map_err(|message| PackError::Failed(format!("the loader cannot start: {message}")))?;
        pack_add(params, home, &(self.workspaces)(), &context)
    }

    /// `config.pack_link`: a working copy over a pack, for this user on this board.
    pub fn pack_link(
        &self,
        params: &ConfigPackLinkParams,
        board: BoardRef,
        path: &OsStr,
    ) -> Result<ConfigPackResult, PackError> {
        self.with_pack_site(board, path, |site, context| {
            pack_link(params, site, &context)
        })
    }

    /// `config.pack_enable`: a user pack on or off for this board.
    pub fn pack_enable(
        &self,
        params: &ConfigPackEnableParams,
        board: BoardRef,
        path: &OsStr,
    ) -> Result<ConfigPackResult, PackError> {
        self.with_pack_site(board, path, |site, context| {
            pack_enable(params, site, &context)
        })
    }

    /// `config.set`: one value, to the keychain (`params.secret`) or to the user's file. See the module docs.
    pub fn set(
        &self,
        params: &ConfigSetParams,
        board: Option<BoardRef>,
        path: &OsStr,
    ) -> Result<ConfigSetResult, SetError> {
        if !params.secret && params.layer == ConfigWriteLayer::Repo {
            return Err(SetError::Refused(format!(
                "`{}` cannot be written to the repo's settings from here: edit and commit `.collide/config.yaml`",
                params.key
            )));
        }
        let gathered = self.gather(board, path).map_err(SetError::Failed)?;
        if !gathered.problems.is_empty() {
            return Err(SetError::Refused(gathered.problems.join("\n")));
        }
        if params.secret {
            self.set_secret(params, board, &gathered)
        } else {
            self.set_plain(params, board, &gathered)
        }
    }

    fn set_secret(
        &self,
        params: &ConfigSetParams,
        board: Option<BoardRef>,
        gathered: &Gathered,
    ) -> Result<ConfigSetResult, SetError> {
        let key = params.key.as_str();
        let secret = match &params.value {
            Value::String(text) if !text.is_empty() => Secret::new(text.clone()),
            _ => {
                return Err(SetError::Invalid(format!(
                    "the secret for `{key}` must be text and not empty"
                )));
            }
        };
        let item = self.secret_item(key, board.is_some(), gathered)?;
        self.secret_store
            .set(&item, &secret)
            .map_err(|error| SetError::Failed(error.to_string()))?;
        Ok(ConfigSetResult {
            entry: ConfigEntry {
                key: key.to_string(),
                value: None,
                layer: SettingLayer::User,
                secret: true,
                overridden: None,
                is_set: Some(true),
            },
        })
    }

    /// The keychain item `key` is kept under: `pack.<pack>.<key>` for a pack setting marked `secret: true`, and the
    /// item an entry names for `agents.<id>.secrets.<ENV>`.
    fn secret_item(
        &self,
        key: &str,
        has_board: bool,
        gathered: &Gathered,
    ) -> Result<String, SetError> {
        let not_secret = || {
            SetError::Invalid(format!(
                "`{key}` is not a secret: only a pack setting marked `secret: true` and `agents.<id>.secrets.<ENV>` \
                 are kept in the keychain"
            ))
        };
        if let Some(rest) = key.strip_prefix("agents.") {
            let parts: Vec<&str> = rest.split('.').collect();
            let [id, "secrets", name] = parts[..] else {
                return Err(not_secret());
            };
            let resolved = resolve(&gathered.user, gathered.line.as_ref(), None);
            let item = resolved
                .agents
                .agents
                .iter()
                .find(|agent| agent.id == id)
                .and_then(|agent| agent.entry.secrets.as_ref()?.get(name).cloned());
            return item.ok_or_else(|| {
                SetError::Invalid(format!(
                    "`{key}` names no keychain item: add `{name}: <item name>` to `agents.{id}.secrets` in the \
                     user's config.yaml, then set it"
                ))
            });
        }
        let packs = pack_settings(gathered.line.as_ref());
        match find_setting(&packs, key) {
            Some(KeyInfo {
                secret: true,
                pack: Some(pack),
                ..
            }) => Ok(pack_item(&pack, &key[pack.len() + 1..])),
            Some(_) => Err(not_secret()),
            None if !has_board && !is_collide_key(key) => {
                Err(SetError::Invalid(no_board_message(key)))
            }
            None => Err(not_secret()),
        }
    }

    fn set_plain(
        &self,
        params: &ConfigSetParams,
        board: Option<BoardRef>,
        gathered: &Gathered,
    ) -> Result<ConfigSetResult, SetError> {
        let key = params.key.as_str();
        if key.starts_with("agents.") {
            return Err(SetError::Invalid(format!(
                "`{key}` is a field of an agent entry, which is set by editing the user's config.yaml; \
                 only `agents.<id>.secrets.<ENV>` is set here, with `--secret`"
            )));
        }
        let packs = pack_settings(gathered.line.as_ref());
        let Some(info) = find_setting(&packs, key) else {
            return Err(SetError::Invalid(not_found(key, board.is_some())));
        };
        if info.secret {
            return Err(SetError::Invalid(format!(
                "`{key}` is a secret and is never written to a settings file: use `--secret`, and give the value on \
                 standard input"
            )));
        }
        if info.repo_only {
            return Err(SetError::Invalid(format!(
                "`{key}` is repoOnly: it describes the repo, so it is set in the repo's config: edit and commit \
                 `.collide/config.yaml`"
            )));
        }
        let value = match &params.value {
            Value::String(text) => value_from_text(&info.schema, text),
            other => other.clone(),
        };
        let violations = check_value(&info.schema, &value);
        if !violations.is_empty() {
            let found: Vec<String> = violations.iter().map(ToString::to_string).collect();
            return Err(SetError::Invalid(format!("`{key}` {}", found.join("; "))));
        }

        let dir = self.user_dir(board)?;
        let target = dir.join("config.yaml");
        match std::fs::read_to_string(&target) {
            Ok(text) if has_comment(&text) => {
                return Err(SetError::Refused(format!(
                    "{} has a comment, and writing `{key}` would drop it: edit the file by hand and set `{key}` in it",
                    target.display()
                )));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(SetError::Refused(format!(
                    "{}: cannot read it: {error}",
                    target.display()
                )));
            }
        }

        let at: Vec<&str> = match &info.pack {
            None => key.split('.').collect(),
            Some(pack) => vec!["config", pack.as_str(), &key[pack.len() + 1..]],
        };
        let current = if board.is_some() {
            &gathered.user.board
        } else {
            &gathered.user.global
        };
        let new_file = current
            .with_value(&at, value)
            .map_err(|message| SetError::Invalid(format!("`{key}` cannot be set: {message}")))?;
        let mut after = gathered.user.clone();
        if board.is_some() {
            after.board = new_file.clone();
        } else {
            after.global = new_file.clone();
        }
        let before = resolve(&gathered.user, gathered.line.as_ref(), None).problems();
        let resolved = resolve(&after, gathered.line.as_ref(), None);
        let worse: Vec<String> = resolved
            .problems()
            .into_iter()
            .filter(|line| !before.contains(line))
            .collect();
        if !worse.is_empty() {
            return Err(SetError::Refused(format!(
                "setting `{key}` would make the settings fail, so nothing was written:\n{}",
                worse.join("\n")
            )));
        }

        let text = to_yaml(&new_file, "")
            .map_err(|error| SetError::Failed(format!("`{key}`: {error}")))?;
        std::fs::create_dir_all(&dir)
            .map_err(|error| SetError::Failed(format!("cannot make {}: {error}", dir.display())))?;
        write_atomically(&dir, "config.yaml", text.as_bytes(), 0o600, |file| file)
            .map_err(|error| SetError::Failed(error.to_string()))?;

        let entry = narrow(resolved, Some(key)).and_then(|mut entries| entries.pop());
        entry.map(|entry| ConfigSetResult { entry }).ok_or_else(|| {
            SetError::Failed(format!(
                "`{key}` was written and is not in the settings read back"
            ))
        })
    }

    /// The folder of the file a write goes to: the user's, or the board's own.
    fn user_dir(&self, board: Option<BoardRef>) -> Result<PathBuf, SetError> {
        let dir = user_config_dir(self.home().map_err(SetError::Failed)?);
        match board {
            None => Ok(dir),
            Some(board) if is_safe_token(board.id) => Ok(dir.join("boards").join(board.id)),
            Some(board) => Err(SetError::Invalid(format!(
                "the board id `{}` is not a safe token, so its settings file is not written",
                board.id
            ))),
        }
    }

    /// The line at the checkout's `HEAD`. Its problems are added to `problems`.
    fn load(
        &self,
        board: BoardRef,
        path: &OsStr,
        problems: &mut Vec<String>,
    ) -> Option<LoadedLine> {
        let git_dir = match boards::git_dir_of(board.root, &self.git_env) {
            Ok(git_dir) => git_dir,
            Err(message) => {
                problems.push(message);
                return None;
            }
        };
        let context = match (self.load_context)(path) {
            Ok(context) => context,
            Err(message) => {
                problems.push(format!(
                    "the repo's settings cannot be read, because the loader cannot start: {message}"
                ));
                return None;
            }
        };
        // The user's packs are part of the line: a working copy linked here changes what a pack's keys are.
        let context = match &self.home {
            Some(home) => context.with_user(UserScope::new(
                home.clone(),
                board.id.to_string(),
                (self.workspaces)(),
            )),
            None => context,
        };
        match load_line(&git_dir, "HEAD", &context) {
            Ok(line) => Some(line),
            Err(found) => {
                problems.extend(found.iter().map(ToString::to_string));
                None
            }
        }
    }
}

/// The entries `key` asks for: all of Collide's and the packs' with no key, the one setting `key` names, one agent
/// entry's field, or every field of the agent `agents.<id>` names. `None` when there is no such key.
fn narrow(resolved: Resolved, key: Option<&str>) -> Option<Vec<ConfigEntry>> {
    let Some(key) = key else {
        return Some(resolved.settings.entries);
    };
    if let Some(entry) = resolved
        .settings
        .entries
        .into_iter()
        .find(|entry| entry.key == key)
    {
        return Some(vec![entry]);
    }
    let agents = resolved.agents.agents;
    if let Some(found) = agents
        .iter()
        .flat_map(|agent| agent.config_entries())
        .find(|entry| entry.key == key)
    {
        return Some(vec![found]);
    }
    let id = key.strip_prefix("agents.")?;
    agents
        .iter()
        .find(|agent| agent.id == id)
        .map(|agent| agent.config_entries())
}

fn is_collide_key(key: &str) -> bool {
    let root = key.split('.').next().unwrap_or(key);
    collide_roots().contains(&root)
}

fn no_board_message(key: &str) -> String {
    format!(
        "no setting `{key}` in the user's layers: a pack's settings need a board, so run it inside a git checkout"
    )
}

fn not_found(key: &str, has_board: bool) -> String {
    if !has_board && !is_collide_key(key) {
        no_board_message(key)
    } else {
        format!("no setting `{key}`: `collide config get` lists them")
    }
}
