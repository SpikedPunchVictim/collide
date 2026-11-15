//! `~/.config/collide/models.yaml` (0.3, Who picks what): where it is and how it is read. The aliases it holds are
//! resolved in `core` ([`collide_core::resolve::resolve_model_alias`]); this is only the file. The home directory is
//! given, never read from the environment here, so a test can point it at a temp root.

use collide_core::agent::UserModels;
use collide_core::yaml::{YamlError, load_yaml_file};
use std::fmt;
use std::io::ErrorKind;
use std::path::PathBuf;

/// Why `models.yaml` could not be read.
#[derive(Debug)]
pub enum ModelsError {
    /// `HOME` is not set (or is empty), so the file cannot be found.
    NoHome,
    /// The file exists and cannot be read or is not valid. The message names the path and the place of the error.
    File(YamlError),
}

impl fmt::Display for ModelsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ModelsError::NoHome => f.write_str("HOME is not set, so models.yaml cannot be found"),
            ModelsError::File(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for ModelsError {}

/// `$HOME/.config/collide/models.yaml`, from the value of `HOME`.
pub fn models_path(home: Option<&str>) -> Result<PathBuf, ModelsError> {
    match home {
        Some(home) if !home.is_empty() => {
            Ok(PathBuf::from(home).join(".config/collide/models.yaml"))
        }
        _ => Err(ModelsError::NoHome),
    }
}

/// Reads the user's `models.yaml`. A missing file means no mappings, so every alias is unmapped. A malformed one is
/// an error with its path and the place of the error.
pub fn load_user_models(home: Option<&str>) -> Result<UserModels, ModelsError> {
    match load_yaml_file(&models_path(home)?) {
        Ok(models) => Ok(models),
        Err(YamlError::Read { source, .. }) if source.kind() == ErrorKind::NotFound => {
            Ok(UserModels::default())
        }
        Err(error) => Err(ModelsError::File(error)),
    }
}
