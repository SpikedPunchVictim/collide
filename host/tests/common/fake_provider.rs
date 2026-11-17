//! The fake provider as a child a test starts (ts-0108-dq): `host/tests/fixtures/fake_provider.py`, which speaks
//! provider protocol v0 and crashes, hangs and sends notifications on demand (the modes are in its header). The launch
//! clears the environment: the child gets only the variables named here, a `HOME` and `COLLIDE_DAEMON_ROOT` of the
//! test's own root, and `COLLIDE_PACK_DATA`.

use super::{PYTHON, TempRoot};
use collide_host::builtin_packs::ProviderLaunch;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

pub fn fake_provider_script() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/fake_provider.py")
}

/// The environment a test gives a child: nothing of the test process's own.
pub fn base_env(root: &TempRoot) -> BTreeMap<String, String> {
    let root = root.path().display().to_string();
    BTreeMap::from([
        ("PATH".to_string(), "/usr/bin:/bin".to_string()),
        ("HOME".to_string(), root.clone()),
        ("COLLIDE_DAEMON_ROOT".to_string(), root),
    ])
}

/// A launch of the fake provider with `modes`, in a pack folder and a data folder below `root`.
pub fn fake_launch(root: &TempRoot, modes: &[&str]) -> ProviderLaunch {
    let pack_dir = root.join("pack");
    let data = root.join("data");
    fs::create_dir_all(&pack_dir).unwrap();
    fs::create_dir_all(&data).unwrap();
    let mut args = vec![fake_provider_script().into_os_string()];
    args.extend(modes.iter().map(|mode| mode.into()));
    ProviderLaunch {
        pack_dir,
        program: PathBuf::from(PYTHON),
        args,
        data: Some(data),
        env: Some(base_env(root)),
    }
}
