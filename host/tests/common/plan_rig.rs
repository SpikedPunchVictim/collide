//! The rig of the tests that run the real built `collide/plan` pack through the daemon (`daemon_plan_layout.rs`,
//! `daemon_plan_private.rs`): a daemon over a fixture repository, a built bundle whose provider it runs, and the calls
//! those tests make. Every path is below the test's own root under `/tmp`, which is also the home. The daemon's
//! captured environment is made by a login shell that clears the environment, so the provider gets only `HOME` and
//! `COLLIDE_DAEMON_ROOT` of that root, a `PATH`, and the pack's data folder. Nothing here touches the user's home,
//! keychain or a real daemon.

use super::config_fixture::Fixture;
use super::{Bundle, node_missing_message, wait_until};
use collide_core::owner::{
    BoardTrustParams, BoardTrustResult, BoardTrustReviewParams, BoardTrustReviewResult, RpcError,
    TaskListResult, TrustState,
};
use collide_host::board_load::LoadContext;
use collide_host::builtin_packs::{BuiltinPacks, Environment};
use collide_host::config_get::LoadContextFactory;
use collide_host::owner_client::ClientError;
use collide_host::pack_cache::ensure_sdk;
use collide_host::pack_location::bundle_sdk;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

pub const WAIT: Option<Duration> = Some(Duration::from_secs(60));
pub const DEADLINE: Duration = Duration::from_secs(30);

pub const EPIC: &str = "---\nid: ep-001-aa\nkind: epic\nstatus: active\ntitle: \"Made-up epic\"\n\
                    owners: []\nlinks: []\n---\n";

pub fn task(id: &str) -> String {
    format!(
        "---\nid: {id}\nkind: task\nstatus: ready\ntitle: \"Made-up item\"\nowners: []\nlinks: []\n---\n\n## Goal\n"
    )
}

/// A login shell that prints a cleared environment, as the capture asks.
pub fn clean_shell(root: &Path) -> String {
    let path = std::env::var("PATH").expect("the tests run with a PATH");
    let home = root.display();
    let script = format!(
        "#!/bin/sh\nexec /usr/bin/env -i HOME='{home}' COLLIDE_DAEMON_ROOT='{home}' PATH='{path}' /bin/sh -c \"$4\"\n"
    );
    let file = root.join("clean-shell");
    fs::write(&file, script).unwrap();
    fs::set_permissions(&file, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    file.to_str().unwrap().to_string()
}

/// The daemon, and the built bundle whose `collide/plan` pack it runs (the bundle is removed when the rig drops).
pub struct Rig {
    pub fixture: Fixture,
    _bundle: Bundle,
}

impl std::ops::Deref for Rig {
    type Target = Fixture;
    fn deref(&self) -> &Fixture {
        &self.fixture
    }
}

pub fn rig(bundle_name: &str) -> Rig {
    if let Some(message) = node_missing_message(&std::env::var_os("PATH").unwrap_or_default()) {
        panic!("{message}");
    }
    let bundle = super::bundle(bundle_name);
    let built = bundle.0.clone();
    let fixture = Fixture::with_options(move |options| {
        let root = PathBuf::from(options.home.clone().expect("the fixture sets a home"));
        options.shell = clean_shell(&root);
        let cache = root.join("cache");
        let sdk = ensure_sdk(&cache, &bundle_sdk(&built).unwrap()).unwrap();
        let factory: LoadContextFactory = Arc::new(move |path| {
            Ok(LoadContext::new(
                cache.clone(),
                sdk.clone(),
                Environment {
                    path: path.to_os_string(),
                },
                BuiltinPacks::new(built.clone()),
            ))
        });
        options.load_context = Some(factory);
    });
    Rig {
        fixture,
        _bundle: bundle,
    }
}

pub fn rpc<R: DeserializeOwned>(
    fixture: &Fixture,
    method: &str,
    params: &impl Serialize,
) -> Result<R, RpcError> {
    match fixture.client().call(method, params, WAIT) {
        Ok(result) => Ok(result),
        Err(ClientError::Rpc(error)) => Err(error),
        Err(other) => panic!("{method}: {other}"),
    }
}

pub fn trust(fixture: &Fixture, board: &str) {
    let review: BoardTrustReviewResult = rpc(
        fixture,
        "board.trust_review",
        &BoardTrustReviewParams {
            board: board.to_string(),
            line: "main".to_string(),
        },
    )
    .unwrap();
    let hash = match review.trust {
        TrustState::Trusted { hash } | TrustState::Untrusted { hash } => hash,
    };
    assert!(
        !hash.is_empty(),
        "the line has a hash: {:?}",
        review.problems
    );
    let _: BoardTrustResult = rpc(
        fixture,
        "board.trust",
        &BoardTrustParams {
            board: board.to_string(),
            line: "main".to_string(),
            hash,
            hosts: Vec::new(),
            dev_folders: Vec::new(),
        },
    )
    .unwrap();
}

/// `task.list` until `done` holds of the first page, which is returned.
pub fn list_until(
    fixture: &Fixture,
    board: &str,
    what: &str,
    done: impl Fn(&TaskListResult) -> bool,
) -> TaskListResult {
    let mut last = None;
    wait_until(what, DEADLINE, || {
        let result: TaskListResult = rpc(fixture, "task.list", &json!({ "board": board })).unwrap();
        let ok = done(&result);
        last = Some(result);
        ok
    });
    last.unwrap()
}

pub fn keys(list: &TaskListResult) -> Vec<String> {
    let mut keys: Vec<String> = list.tasks.iter().map(|task| task.key.clone()).collect();
    keys.sort();
    keys
}
