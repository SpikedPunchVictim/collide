//! The hosts a fetch may contact (ts-0100-pu; 0.7, Fetch rules): `~/.config/collide/hosts.yaml`. The first fetch from a
//! host the file does not list asks the person; their yes records the host here, and later fetches from it do not ask.
//! A host is matched in lower case, with no user and no port. A repository on disk has no host and is never listed.
//!
//! The file is `hosts:`, a list of host names. Collide writes it, atomically and for the user alone. A file that does
//! not read is an error that names it and is never rewritten, so a hand edit is not lost to a record.

use crate::atomic_write::write_atomically;
use crate::settings_files::user_config_dir;
use collide_core::yaml::{MAX_YAML_BYTES, YamlError, load_yaml_file};
use collide_core::yaml_out::to_yaml;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const FILE_NAME: &str = "hosts.yaml";
const FILE_MODE: u32 = 0o600;
const HEADER: &str = "The hosts Collide may fetch git packs from. Collide writes this file when you say yes to a first \
                      fetch from a host. Remove a line to be asked again.";

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct HostsFile {
    #[serde(default)]
    hosts: Vec<String>,
}

/// Why the file could not be read or written. The message names the file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostsError(String);

impl fmt::Display for HostsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for HostsError {}

/// The user's list of hosts. The file is a parameter, so a test never touches the real one.
#[derive(Debug, Clone)]
pub struct KnownHosts {
    file: PathBuf,
}

impl KnownHosts {
    /// `<home>/.config/collide/hosts.yaml`.
    pub fn for_home(home: &Path) -> KnownHosts {
        KnownHosts {
            file: user_config_dir(home).join(FILE_NAME),
        }
    }

    pub fn file(&self) -> &Path {
        &self.file
    }

    /// Whether `host` is listed. A file that is not there lists nothing.
    pub fn is_known(&self, host: &str) -> Result<bool, HostsError> {
        let host = host.to_ascii_lowercase();
        Ok(self.read()?.hosts.contains(&host))
    }

    /// Adds `host` to the list, once. The folder is made when it is not there.
    pub fn record(&self, host: &str) -> Result<(), HostsError> {
        let host = host.to_ascii_lowercase();
        let mut file = self.read()?;
        if file.hosts.contains(&host) {
            return Ok(());
        }
        file.hosts.push(host);
        let text = to_yaml(&file, HEADER).map_err(|error| self.error(error))?;
        if text.len() > MAX_YAML_BYTES {
            return Err(self.error("the list is too long to read back"));
        }
        let dir = self.file.parent().unwrap_or(Path::new("."));
        fs::create_dir_all(dir).map_err(|error| self.error(error))?;
        write_atomically(dir, FILE_NAME, text.as_bytes(), FILE_MODE, |file| file)
            .map_err(|error| self.error(error))
    }

    fn read(&self) -> Result<HostsFile, HostsError> {
        match load_yaml_file::<Option<HostsFile>>(&self.file) {
            Ok(file) => Ok(file.unwrap_or_default()),
            Err(YamlError::Read { source, .. }) if source.kind() == io::ErrorKind::NotFound => {
                Ok(HostsFile::default())
            }
            Err(error) => Err(HostsError(error.to_string())),
        }
    }

    fn error(&self, what: impl fmt::Display) -> HostsError {
        HostsError(format!("{}: {what}", self.file.display()))
    }
}
