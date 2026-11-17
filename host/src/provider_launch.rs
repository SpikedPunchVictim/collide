//! The launch of a loaded pack's provider (0.4, 0.6; Stage 4b). A provider is `node <the pack's folder>/<script>`, the
//! `.ts` script run as it is (Node strips the types, 0.6). The pack's folder is the one the loader found: the cache's
//! copy of the line's snapshot for a repo, `git:` or user pack, so an edit in the working copy is not what runs, and
//! the person's own folder for a `dev:` pack, read as it is now. The script is the manifest's
//! `providers.<name>.script`, and it must be a plain path inside the pack, whatever the manifest check at load said.
//!
//! A built-in pack keeps its own loader ([`BuiltinPacks::provider`]), which runs the compiled copy in the bundle: this
//! module hands it the call, so a caller has one function for every pack.
//!
//! The launch has no environment and no data folder: the caller gives it the captured base environment
//! ([`ProviderLaunch::with_env`]) and the pack's data folder ([`ProviderLaunch::with_pack_data`]).

use crate::board_load::{LoadContext, LoadedPack, PackOrigin};
use crate::builtin_packs::{
    BuiltinPackError, ProviderLaunch, find_node, is_plain_pack_path, node_missing_text,
    node_version_wanted,
};
use collide_core::pack_check::{PackPath, parse_pack_path};
use std::fmt;
use std::path::PathBuf;

/// Why a provider could not be launched.
#[derive(Debug)]
pub enum ProviderLaunchError {
    /// The manifest has no such provider.
    NoProvider { pack: String, provider: String },
    /// The script is not a plain path inside the pack.
    BadScript { pack: String, script: String },
    /// The script is not a file in the pack's folder.
    MissingScript { pack: String, script: PathBuf },
    /// There is no `node` in the `PATH` directories searched.
    NodeNotFound {
        pack: String,
        provider: String,
        version: Option<String>,
        searched: Vec<PathBuf>,
    },
    /// A built-in pack's own loader refused.
    Builtin(BuiltinPackError),
}

impl fmt::Display for ProviderLaunchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoProvider { pack, provider } => {
                write!(f, "pack {pack} has no provider `{provider}`")
            }
            Self::BadScript { pack, script } => write!(
                f,
                "pack {pack}: provider script `{script}` is not a plain path inside the pack"
            ),
            Self::MissingScript { pack, script } => write!(
                f,
                "pack {pack}: provider script {} is not a file in the pack",
                script.display()
            ),
            Self::NodeNotFound {
                pack,
                provider,
                version,
                searched,
            } => f.write_str(&node_missing_text(
                &format!("pack {pack}"),
                provider,
                version.as_deref(),
                searched,
            )),
            Self::Builtin(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for ProviderLaunchError {}

/// The command that starts `provider` of the loaded `pack`, with `node` found on the `PATH` of `context`'s
/// environment.
pub fn provider_launch(
    context: &LoadContext,
    pack: &LoadedPack,
    provider: &str,
) -> Result<ProviderLaunch, ProviderLaunchError> {
    if pack.origin == PackOrigin::Builtin {
        return context
            .builtin
            .provider(&pack.name, provider, &context.env)
            .map_err(ProviderLaunchError::Builtin);
    }
    let script = &pack
        .manifest
        .providers
        .get(provider)
        .ok_or_else(|| ProviderLaunchError::NoProvider {
            pack: pack.name.clone(),
            provider: provider.to_string(),
        })?
        .script;
    let in_pack = matches!(parse_pack_path(script), Ok(PackPath::InPack(_)));
    if !in_pack || !is_plain_pack_path(script) {
        return Err(ProviderLaunchError::BadScript {
            pack: pack.name.clone(),
            script: script.clone(),
        });
    }
    let entry = pack.dir.join(script);
    if !entry.is_file() {
        return Err(ProviderLaunchError::MissingScript {
            pack: pack.name.clone(),
            script: entry,
        });
    }
    let (program, searched) = find_node(&context.env.path);
    let program = program.ok_or_else(|| ProviderLaunchError::NodeNotFound {
        pack: pack.name.clone(),
        provider: provider.to_string(),
        version: node_version_wanted(&pack.manifest),
        searched,
    })?;
    Ok(ProviderLaunch {
        pack_dir: pack.dir.clone(),
        program,
        args: vec![entry.into_os_string()],
        data: None,
        env: None,
    })
}
