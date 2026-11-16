//! The loader for built-in packs (0.4, 0.6). A built-in pack is compiled into the app bundle, read-only, with its
//! own copy of the SDK: it is not materialized into the cache, has no tree id and asks for no trust. The loader
//! finds a pack's directory in the bundle and builds the command that starts one of its providers, with `node`
//! taken from the environment it is given (the CLI's own until the daemon exists).
//!
//! The loader reads the whole `pack.yaml` through the hardened YAML loader and runs the manifest check
//! ([`collide_core::pack_check::check_manifest`]) before it uses a provider entry or the `node` line of
//! `requires.tools`. Checking a tool's version is the board loader's (`board_load`, Stage 4a).

use collide_core::pack::PackManifest;
use collide_core::pack_check::{ManifestError, check_manifest};
use collide_core::yaml::{YamlError, load_yaml_file};
use std::ffi::OsString;
use std::fmt;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

/// The built-in packs, by name, with the folder each has in the bundle (the same as under `packs/`).
const BUILTIN: [(&str, &str); 2] = [("collide", "collide"), ("collide/plan", "plan")];

/// What the loader reads from the environment it is given: so far `PATH`.
#[derive(Debug, Clone)]
pub struct Environment {
    pub path: OsString,
}

impl Environment {
    /// This process's own environment.
    pub fn from_process() -> Self {
        Self {
            path: std::env::var_os("PATH").unwrap_or_default(),
        }
    }

    /// The first executable file called `tool` in the absolute `PATH` directories. `None` for a name that is not
    /// one plain file name (it has a `/`, or is `.` or `..`), so a pack's `requires.tools` cannot reach outside them.
    pub fn find(&self, tool: &str) -> Option<PathBuf> {
        if !is_plain_file_name(tool) {
            return None;
        }
        let name = format!("{tool}{}", std::env::consts::EXE_SUFFIX);
        path_dirs(&self.path)
            .into_iter()
            .map(|dir| dir.join(&name))
            .find(|candidate| is_executable_file(candidate))
    }
}

/// A provider's directory and the command that starts it: `program` is `node`, and `args` is the compiled entry.
#[derive(Debug, Clone, PartialEq)]
pub struct ProviderLaunch {
    pub pack_dir: PathBuf,
    pub program: PathBuf,
    pub args: Vec<OsString>,
}

impl ProviderLaunch {
    /// The command, with the caller's choice of working directory, environment and pipes still to set.
    pub fn command(&self) -> Command {
        let mut command = Command::new(&self.program);
        command.args(&self.args);
        command
    }
}

#[derive(Debug)]
pub enum BuiltinPackError {
    /// The name is not a built-in pack.
    UnknownPack(String),
    /// The pack's `pack.yaml` did not load.
    Manifest(YamlError),
    /// The manifest is not valid: every refusal of the manifest check.
    InvalidManifest {
        pack: String,
        errors: Vec<ManifestError>,
    },
    /// The manifest's `name` is not the pack's.
    WrongName { expected: String, found: String },
    /// The manifest has no such provider.
    NoProvider { pack: String, provider: String },
    /// The provider's script is not a path inside the pack.
    BadScript { pack: String, script: String },
    /// The compiled entry is not in the bundle: the bundle was not built, or not completely.
    MissingEntry { pack: String, entry: PathBuf },
    /// There is no `node` in the `PATH` directories searched.
    NodeNotFound {
        pack: String,
        provider: String,
        version: Option<String>,
        searched: Vec<PathBuf>,
    },
}

impl fmt::Display for BuiltinPackError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownPack(name) => write!(f, "`{name}` is not a built-in pack"),
            Self::Manifest(error) => write!(f, "{error}"),
            Self::InvalidManifest { pack, errors } => {
                write!(f, "the manifest of built-in pack {pack} is not valid")?;
                for error in errors {
                    write!(f, "; {error}")?;
                }
                Ok(())
            }
            Self::WrongName { expected, found } => {
                write!(f, "the manifest of built-in pack {expected} names {found}")
            }
            Self::NoProvider { pack, provider } => {
                write!(f, "built-in pack {pack} has no provider `{provider}`")
            }
            Self::BadScript { pack, script } => write!(
                f,
                "built-in pack {pack}: provider script `{script}` is not a path inside the pack"
            ),
            Self::MissingEntry { pack, entry } => write!(
                f,
                "built-in pack {pack}: {} is not in the bundle (run `pnpm build:packs`)",
                entry.display()
            ),
            Self::NodeNotFound {
                pack,
                provider,
                version,
                searched,
            } => {
                let version = version
                    .as_ref()
                    .map(|v| format!(" {v}"))
                    .unwrap_or_default();
                let looked = if searched.is_empty() {
                    "nothing, PATH is empty".to_string()
                } else {
                    searched
                        .iter()
                        .map(|dir| dir.display().to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                };
                write!(
                    f,
                    "built-in pack {pack} needs `node`{version} to start its provider `{provider}`, \
                     and `node` was not found on PATH (searched: {looked})"
                )
            }
        }
    }
}

impl std::error::Error for BuiltinPackError {}

/// The built-in packs in one bundle directory (`app/src-tauri/resources/builtin-packs` when staged).
#[derive(Debug, Clone)]
pub struct BuiltinPacks {
    root: PathBuf,
}

impl BuiltinPacks {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// The names of the built-in packs, in the order they load.
    pub fn names() -> [&'static str; 2] {
        [BUILTIN[0].0, BUILTIN[1].0]
    }

    /// The folder of the built-in pack `pack` in the bundle.
    pub fn dir(&self, pack: &str) -> Result<PathBuf, BuiltinPackError> {
        BUILTIN
            .iter()
            .find(|(name, _)| *name == pack)
            .map(|(_, folder)| self.root.join(folder))
            .ok_or_else(|| BuiltinPackError::UnknownPack(pack.to_string()))
    }

    /// The manifest of the built-in pack `pack`, read with the hardened loader and checked: it must pass
    /// [`check_manifest`] and carry the pack's own name.
    pub fn manifest(&self, pack: &str) -> Result<PackManifest, BuiltinPackError> {
        let pack_dir = self.dir(pack)?;
        let manifest: PackManifest =
            load_yaml_file(&pack_dir.join("pack.yaml")).map_err(BuiltinPackError::Manifest)?;
        check_manifest(&manifest).map_err(|errors| BuiltinPackError::InvalidManifest {
            pack: pack.to_string(),
            errors,
        })?;
        if manifest.name != pack {
            return Err(BuiltinPackError::WrongName {
                expected: pack.to_string(),
                found: manifest.name,
            });
        }
        Ok(manifest)
    }

    /// The command that starts `provider` of the built-in pack `pack`, with `node` found on `env`'s `PATH`.
    /// The bundle holds compiled JavaScript, so a `.ts` script in `pack.yaml` (what the pack runs from source
    /// as) names the `.js` file beside it.
    pub fn provider(
        &self,
        pack: &str,
        provider: &str,
        env: &Environment,
    ) -> Result<ProviderLaunch, BuiltinPackError> {
        let pack_dir = self.dir(pack)?;
        let manifest = self.manifest(pack)?;
        let script = &manifest
            .providers
            .get(provider)
            .ok_or_else(|| BuiltinPackError::NoProvider {
                pack: pack.to_string(),
                provider: provider.to_string(),
            })?
            .script;
        let entry = pack_dir.join(compiled_script(pack, script)?);
        if !entry.is_file() {
            return Err(BuiltinPackError::MissingEntry {
                pack: pack.to_string(),
                entry,
            });
        }
        let version = manifest
            .requires
            .and_then(|requires| requires.tools.into_iter().find(|tool| tool.name == "node"))
            .and_then(|tool| tool.version);
        let (program, searched) = find_node(&env.path);
        let program = program.ok_or_else(|| BuiltinPackError::NodeNotFound {
            pack: pack.to_string(),
            provider: provider.to_string(),
            version,
            searched,
        })?;
        Ok(ProviderLaunch {
            pack_dir,
            program,
            args: vec![entry.into_os_string()],
        })
    }
}

fn compiled_script(pack: &str, script: &str) -> Result<PathBuf, BuiltinPackError> {
    let path = Path::new(script);
    let inside = path
        .components()
        .all(|part| matches!(part, Component::Normal(_)));
    if !inside || script.is_empty() {
        return Err(BuiltinPackError::BadScript {
            pack: pack.to_string(),
            script: script.to_string(),
        });
    }
    Ok(match path.extension() {
        Some(ext) if ext == "ts" => path.with_extension("js"),
        _ => path.to_path_buf(),
    })
}

/// Whether `name` is one plain file name: not empty, with no `/` and no NUL, and not `.` or `..`. A tool a pack
/// `requires` is looked for as a file of that name in the `PATH` folders, so a name with a path in it is never one.
pub fn is_plain_file_name(name: &str) -> bool {
    let one_part = Path::new(name)
        .components()
        .eq([Component::Normal(name.as_ref())]);
    one_part && !name.contains('\0')
}

/// The first executable file named `node` in the `PATH` directories, and the directories searched.
fn find_node(path: &OsString) -> (Option<PathBuf>, Vec<PathBuf>) {
    let found = Environment { path: path.clone() }.find("node");
    (found, path_dirs(path))
}

/// The absolute directories of `path`. A relative entry, the empty one included, is skipped: it would be read from
/// whatever folder the daemon runs in.
fn path_dirs(path: &OsString) -> Vec<PathBuf> {
    std::env::split_paths(path)
        .filter(|dir| dir.is_absolute())
        .collect()
}

#[cfg(unix)]
fn is_executable_file(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.metadata()
        .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_executable_file(path: &Path) -> bool {
    path.is_file()
}
