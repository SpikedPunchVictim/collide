//! Where the CLI finds the built-in packs when it runs from a checkout or an installed bundle (ts-0020). The
//! loader (`builtin_packs`) takes the bundle directory as a parameter; this is the rule that picks it, with its
//! inputs given so that it needs no process state. A directory counts as the bundle when it holds `plan/pack.yaml`.
//!
//! In order:
//! 1. `COLLIDE_BUILTIN_PACKS`, a bundle directory. When it is set it is the only place looked at, so a wrong
//!    value is an error and never a silent fallback.
//! 2. `app/src-tauri/resources/builtin-packs/` in a folder above the executable: what `pnpm build:packs` stages,
//!    found from `target/<profile>/collide` in any checkout or worktree. It comes before the copies below because
//!    those can be older than the staged bundle.
//! 3. `builtin-packs/` beside the executable (where the Tauri build puts its copy), and `../Resources/builtin-packs/`
//!    (the macOS app bundle, with the executable in `Contents/MacOS`).

use std::fmt;
use std::path::{Path, PathBuf};

/// The environment variable that names a bundle directory.
pub const OVERRIDE_VAR: &str = "COLLIDE_BUILTIN_PACKS";

const STAGED: &str = "app/src-tauri/resources/builtin-packs";

/// What the rule reads: the variable's value and the executable's path, each as the caller found it.
#[derive(Debug, Clone, Default)]
pub struct Inputs {
    pub override_dir: Option<PathBuf>,
    pub exe: Option<PathBuf>,
}

impl Inputs {
    /// This process's own: the variable and `std::env::current_exe`.
    pub fn from_process() -> Self {
        Self {
            override_dir: std::env::var_os(OVERRIDE_VAR)
                .filter(|value| !value.is_empty())
                .map(PathBuf::from),
            exe: std::env::current_exe().ok(),
        }
    }
}

/// No bundle was found: what was looked at, in order.
#[derive(Debug)]
pub struct NotFound {
    pub looked: Vec<PathBuf>,
    pub from_override: bool,
}

impl fmt::Display for NotFound {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let list = |dirs: &[PathBuf]| {
            dirs.iter()
                .map(|dir| dir.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        };
        if self.from_override {
            write!(
                f,
                "{OVERRIDE_VAR} is set to {}, which holds no built-in packs (it needs plan/pack.yaml)",
                list(&self.looked)
            )
        } else {
            let looked = if self.looked.is_empty() {
                "nothing: the executable's path is unknown".to_string()
            } else {
                list(&self.looked)
            };
            write!(
                f,
                "could not find the built-in packs (looked for plan/pack.yaml in: {looked}). \
                 Run `pnpm build:packs` in the Collide checkout, or set {OVERRIDE_VAR} to a bundle directory"
            )
        }
    }
}

impl std::error::Error for NotFound {}

fn is_bundle(dir: &Path) -> bool {
    dir.join("plan/pack.yaml").is_file()
}

/// The bundle directory by the rule above.
pub fn locate_bundle(inputs: &Inputs) -> Result<PathBuf, NotFound> {
    if let Some(dir) = &inputs.override_dir {
        return if is_bundle(dir) {
            Ok(dir.clone())
        } else {
            Err(NotFound {
                looked: vec![dir.clone()],
                from_override: true,
            })
        };
    }
    let mut candidates = Vec::new();
    if let Some(exe_dir) = inputs.exe.as_deref().and_then(Path::parent) {
        candidates.extend(exe_dir.ancestors().map(|above| above.join(STAGED)));
        candidates.push(exe_dir.join("builtin-packs"));
        candidates.push(exe_dir.join("../Resources/builtin-packs"));
    }
    match candidates.iter().find(|dir| is_bundle(dir)) {
        Some(found) => Ok(found.clone()),
        None => Err(NotFound {
            looked: candidates,
            from_override: false,
        }),
    }
}

/// Where `scripts/build-packs.ts` puts the compiled SDK: in the `node_modules` of each pack that depends on it.
const SDK_IN_PACK: &str = "node_modules/@collide/pack-sdk";

/// No compiled SDK was found in a bundle.
#[derive(Debug)]
pub struct NoSdk {
    pub bundle: PathBuf,
}

impl fmt::Display for NoSdk {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "the bundle {} holds no compiled SDK (a pack's {SDK_IN_PACK}/package.json). \
             Run `pnpm build:packs` in the Collide checkout",
            self.bundle.display()
        )
    }
}

impl std::error::Error for NoSdk {}

/// The compiled SDK a bundle holds: `<pack>/node_modules/@collide/pack-sdk` of the first pack, in name order, that has
/// one. Every pack that depends on the SDK carries the same build, so which one is read does not matter.
pub fn bundle_sdk(bundle: &Path) -> Result<PathBuf, NoSdk> {
    let none = || NoSdk {
        bundle: bundle.to_path_buf(),
    };
    let mut packs: Vec<PathBuf> = std::fs::read_dir(bundle)
        .map_err(|_| none())?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .collect();
    packs.sort();
    packs
        .iter()
        .map(|pack| pack.join(SDK_IN_PACK))
        .find(|sdk| sdk.join("package.json").is_file())
        .ok_or_else(none)
}
