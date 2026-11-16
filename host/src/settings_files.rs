//! The user's settings files (ts-0098-nx; 0.7, Layers and sources; 0.9, YAML is untrusted input).
//!
//! - `~/.config/collide/config.yaml`: the user's settings for every board.
//! - `~/.config/collide/boards/<board-id>/config.yaml`: the user's settings for one board, which win over the first.
//!   `<board-id>` is the daemon store's name for the board (`board-N`), so the file is per machine, as the store is.
//! - `~/.config/collide/profiles/<name>.yaml`: the user's profiles.
//!
//! Every file is read with the hardened loader (a size cap, no aliases, a duplicate key is an error). A file that is
//! not there is empty, as is a file that holds nothing, or only comments. A file that is there and wrong is an error
//! that names its path, and the other files are still read, so one call reports every bad file.
//!
//! The agent entries of the two settings files are checked as they are read ([`check_agents`], which `core` owns), each
//! message naming its file. The checks that need the machine (a `read` folder, a keychain item) are
//! [`crate::agent_checks`]'s.

use collide_core::profile::{Profile, check_profile};
use collide_core::resolve::is_safe_token;
use collide_core::settings::{ConfigFile, check_agents};
use collide_core::yaml::{YamlError, load_yaml_file};
use serde::de::DeserializeOwned;
use std::collections::BTreeMap;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

/// `~/.config/collide`, for `home`.
pub fn user_config_dir(home: &Path) -> PathBuf {
    home.join(".config").join("collide")
}

/// The file at `path`, or `None` when there is none or it holds nothing.
fn read_optional<T: DeserializeOwned>(path: &Path) -> Result<Option<T>, String> {
    match load_yaml_file::<Option<T>>(path) {
        Ok(found) => Ok(found),
        Err(YamlError::Read { source, .. }) if source.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.to_string()),
    }
}

/// A settings file. Missing, empty, or only comments is the empty file; anything else wrong is an error naming `path`.
pub fn read_config_file(path: &Path) -> Result<ConfigFile, String> {
    read_optional::<ConfigFile>(path).map(Option::unwrap_or_default)
}

/// A settings file of the user's, with its agent entries checked: every refusal names `path`.
fn read_user_config_file(path: &Path, errors: &mut Vec<String>) -> Option<ConfigFile> {
    match read_config_file(path) {
        Ok(file) => {
            let refused = check_agents(&file);
            errors.extend(
                refused
                    .iter()
                    .map(|error| format!("{}: {error}", path.display())),
            );
            refused.is_empty().then_some(file)
        }
        Err(error) => {
            errors.push(error);
            None
        }
    }
}

/// The user's layers.
#[derive(Debug, Clone, Default)]
pub struct UserFiles {
    /// `config.yaml`.
    pub global: ConfigFile,
    /// The board's own `config.yaml`. Empty when no board is named.
    pub board: ConfigFile,
    /// `profiles/`, by file name without `.yaml`.
    pub profiles: BTreeMap<String, Profile>,
}

/// Reads the user's files for `board` (the store's id, or `None` for the user's layers alone). Every problem is
/// returned, each naming its file.
pub fn read_user_files(home: &Path, board: Option<&str>) -> Result<UserFiles, Vec<String>> {
    let dir = user_config_dir(home);
    let mut errors = Vec::new();
    let mut files = UserFiles::default();
    if let Some(file) = read_user_config_file(&dir.join("config.yaml"), &mut errors) {
        files.global = file;
    }
    if let Some(board) = board {
        if is_safe_token(board) {
            let path = dir.join("boards").join(board).join("config.yaml");
            if let Some(file) = read_user_config_file(&path, &mut errors) {
                files.board = file;
            }
        } else {
            errors.push(format!(
                "the board id `{board}` is not a safe token, so its settings file is not read"
            ));
        }
    }
    read_profiles(&dir.join("profiles"), &mut files.profiles, &mut errors);
    if errors.is_empty() {
        Ok(files)
    } else {
        Err(errors)
    }
}

/// The `.yaml` files of `dir`, each a profile. A folder that is not there holds none.
fn read_profiles(dir: &Path, profiles: &mut BTreeMap<String, Profile>, errors: &mut Vec<String>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == ErrorKind::NotFound => return,
        Err(error) => {
            errors.push(format!(
                "{}: cannot read the folder: {error}",
                dir.display()
            ));
            return;
        }
    };
    let mut paths: Vec<PathBuf> = entries
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "yaml"))
        .collect();
    paths.sort();
    for path in paths {
        let Some(name) = path.file_stem().and_then(|stem| stem.to_str()) else {
            errors.push(format!("{}: the file name is not text", path.display()));
            continue;
        };
        let profile = match read_optional::<Profile>(&path) {
            Ok(found) => found.unwrap_or_default(),
            Err(error) => {
                errors.push(error);
                continue;
            }
        };
        if let Err(problems) = check_profile(&profile) {
            for problem in problems {
                errors.push(format!("{}: {problem}", path.display()));
            }
            continue;
        }
        profiles.insert(name.to_string(), profile);
    }
}
