//! `config.get` (ts-0098-nx; 0.7, Settings and Profiles): the four layers of a board, with the layer each value came
//! from. The repo's layer and the packs' schemas come from the loader ([`load_line`]) at the checkout's `HEAD`, so an
//! edit that is not committed is not used, as for every other thing the loader reads. The user's layers come from
//! `~/.config/collide` ([`crate::settings_files`]). The merge is `core`'s ([`settings_layers`]); this file only gathers
//! its inputs and says how a failure is told.
//!
//! The wire has no place for problems, so a failure is the error of the request: [`GetError::Refused`] (the files or
//! packs have problems, one per line, each naming its key or file) is `CONFLICT`, and [`GetError::NotFound`] (a key, a
//! profile or a board that is not there) is `NOT_FOUND`. With no board the answer is the user's layers over Collide's
//! own keys, and no pack is loaded.

use crate::board_load::{LoadContext, load_line};
use crate::boards;
use crate::builtin_packs::{BuiltinPacks, Environment};
use crate::pack_cache::ensure_sdk;
use crate::pack_location::{Inputs, bundle_sdk, locate_bundle};
use crate::settings_files::{UserFiles, read_user_files};
use collide_core::owner::{ConfigGetParams, ConfigGetResult};
use collide_core::profile::{Profile, ProfileLookupError, find_profile};
use collide_core::settings::ConfigFile;
use collide_core::settings_layers::{LayerInputs, PackSettings, ProfileUse, settings_layers};
use collide_core::settings_registry::collide_roots;
use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Makes the loader's context for one `config.get`; the argument is the `PATH` tools are looked for on.
pub type LoadContextFactory = Arc<dyn Fn(&OsStr) -> Result<LoadContext, String> + Send + Sync>;

/// The factory a daemon uses when its options give none: the built-in packs and the SDK are found from the process
/// ([`locate_bundle`]), and the SDK is copied once into `cache` ([`ensure_sdk`]). It is read at each call, so a daemon
/// that starts before the bundle is in place still answers once it is.
pub fn system_load_context(cache: PathBuf, bundle: Inputs) -> LoadContextFactory {
    Arc::new(move |path| {
        let bundle = locate_bundle(&bundle).map_err(|error| error.to_string())?;
        let sdk = bundle_sdk(&bundle).map_err(|error| error.to_string())?;
        let sdk = ensure_sdk(&cache, &sdk).map_err(|error| error.to_string())?;
        Ok(LoadContext::new(
            cache.clone(),
            sdk,
            Environment {
                path: path.to_os_string(),
            },
            BuiltinPacks::new(bundle),
        ))
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

/// The board a request is about: the store's id for it (the name of the user's per-board folder) and its checkout.
#[derive(Debug, Clone, Copy)]
pub struct BoardRef<'a> {
    pub id: &'a str,
    pub root: &'a Path,
}

pub struct ConfigService {
    home: Option<PathBuf>,
    load_context: LoadContextFactory,
    git_env: Vec<(String, String)>,
}

impl ConfigService {
    pub fn new(
        home: Option<String>,
        load_context: LoadContextFactory,
        git_env: Vec<(String, String)>,
    ) -> ConfigService {
        ConfigService {
            home: home.filter(|home| !home.is_empty()).map(PathBuf::from),
            load_context,
            git_env,
        }
    }

    /// The settings of `board` (or the user's alone), with `params.profile` on top, narrowed to `params.key`.
    /// `path` is the `PATH` the packs' `requires.tools` are looked for on.
    pub fn get(
        &self,
        params: &ConfigGetParams,
        board: Option<BoardRef>,
        path: &OsStr,
    ) -> Result<ConfigGetResult, GetError> {
        let Some(home) = &self.home else {
            return Err(GetError::Refused(
                "the daemon knows no home directory, so the user's settings cannot be found".into(),
            ));
        };
        let mut problems = Vec::new();
        let user = match read_user_files(home, board.map(|board| board.id)) {
            Ok(files) => files,
            Err(errors) => {
                problems.extend(errors);
                UserFiles::default()
            }
        };
        let line = board.and_then(|board| self.load(board, path, &mut problems));
        if !problems.is_empty() {
            return Err(GetError::Refused(problems.join("\n")));
        }

        let no_profiles = BTreeMap::new();
        let repo_profiles = line.as_ref().map_or(&no_profiles, |line| &line.profiles);
        let pack_profiles: Vec<(String, BTreeMap<String, Profile>)> =
            line.as_ref().map_or_else(Vec::new, |line| {
                line.packs
                    .iter()
                    .map(|pack| (pack.name.clone(), pack.manifest.profiles.clone()))
                    .collect()
            });
        let found = match &params.profile {
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

        let packs: Vec<PackSettings> = line.as_ref().map_or_else(Vec::new, |line| {
            line.packs
                .iter()
                .map(|pack| PackSettings {
                    pack: &pack.name,
                    config: &pack.manifest.config,
                })
                .collect()
        });
        let empty = ConfigFile::default();
        let settings = settings_layers(&LayerInputs {
            packs: &packs,
            user: &user.global,
            user_board: &user.board,
            repo: line.as_ref().map_or(&empty, |line| &line.config),
            profile: found.as_ref().map(|(name, found)| ProfileUse {
                name,
                source: &found.source,
                profile: found.profile,
            }),
        });
        if !settings.problems.is_empty() {
            let lines: Vec<String> = settings.problems.iter().map(ToString::to_string).collect();
            return Err(GetError::Refused(lines.join("\n")));
        }

        let Some(key) = &params.key else {
            return Ok(ConfigGetResult {
                entries: settings.entries,
            });
        };
        match settings.entries.into_iter().find(|entry| entry.key == *key) {
            Some(entry) => Ok(ConfigGetResult {
                entries: vec![entry],
            }),
            None => Err(GetError::NotFound(not_found(key, board.is_some()))),
        }
    }

    /// The line at the checkout's `HEAD`. Its problems are added to `problems`.
    fn load(
        &self,
        board: BoardRef,
        path: &OsStr,
        problems: &mut Vec<String>,
    ) -> Option<crate::board_load::LoadedLine> {
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
        match load_line(&git_dir, "HEAD", &context) {
            Ok(line) => Some(line),
            Err(found) => {
                problems.extend(found.iter().map(ToString::to_string));
                None
            }
        }
    }
}

fn not_found(key: &str, has_board: bool) -> String {
    let root = key.split('.').next().unwrap_or(key);
    if !has_board && !collide_roots().contains(&root) {
        format!(
            "no setting `{key}` in the user's layers: a pack's settings need a board, so run it inside a git checkout"
        )
    } else {
        format!("no setting `{key}`: `collide config get` lists them")
    }
}
