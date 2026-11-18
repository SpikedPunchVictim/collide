//! The plan pack's `layout` setting through the daemon (ts-0124-xm; 0.10, Planning sources): a repository whose
//! committed `.collide/config.yaml` sets `layout.root` is listed from that folder by `task.list`, and a layout the
//! provider refuses fails the source's start with the reason. The provider is the real built-in `collide/plan` pack
//! from a built bundle, run by `node`.
//!
//! Every path is below the test's own root under `/tmp`, which is also the home. The daemon's captured environment is
//! made by a login shell that clears the environment, so the provider gets only `HOME` and `COLLIDE_DAEMON_ROOT` of
//! that root and a `PATH`. Nothing here touches the user's home, keychain or a real daemon.

mod common;

use collide_core::owner::{
    BoardTrustParams, BoardTrustResult, BoardTrustReviewParams, BoardTrustReviewResult, RpcError,
    TaskGetParams, TaskGetResult, TaskListResult, TrustState,
};
use collide_host::board_load::LoadContext;
use collide_host::builtin_packs::{BuiltinPacks, Environment};
use collide_host::config_get::LoadContextFactory;
use collide_host::owner_client::ClientError;
use collide_host::pack_cache::ensure_sdk;
use collide_host::pack_location::bundle_sdk;
use common::config_fixture::Fixture;
use common::{Bundle, node_missing_message, wait_until};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

const WAIT: Option<Duration> = Some(Duration::from_secs(60));
const DEADLINE: Duration = Duration::from_secs(30);

const EPIC: &str = "---\nid: ep-001-aa\nkind: epic\nstatus: active\ntitle: \"Made-up epic\"\n\
                    owners: []\nlinks: []\n---\n";

fn task(id: &str) -> String {
    format!(
        "---\nid: {id}\nkind: task\nstatus: ready\ntitle: \"Made-up item\"\nowners: []\nlinks: []\n---\n\n## Goal\n"
    )
}

/// A login shell that prints a cleared environment, as the capture asks.
fn clean_shell(root: &Path) -> String {
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
struct Rig {
    fixture: Fixture,
    _bundle: Bundle,
}

impl std::ops::Deref for Rig {
    type Target = Fixture;
    fn deref(&self) -> &Fixture {
        &self.fixture
    }
}

fn rig() -> Rig {
    if let Some(message) = node_missing_message(&std::env::var_os("PATH").unwrap_or_default()) {
        panic!("{message}");
    }
    let bundle = common::bundle("daemon-plan-layout");
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

fn rpc<R: DeserializeOwned>(
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

fn trust(fixture: &Fixture, board: &str) {
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
fn list_until(
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

fn keys(list: &TaskListResult) -> Vec<String> {
    let mut keys: Vec<String> = list.tasks.iter().map(|task| task.key.clone()).collect();
    keys.sort();
    keys
}

/// A trusted board over a repository with `config` as its `.collide/config.yaml`, items at `docs/plan`, and one item
/// left at the default root.
fn board_with(fixture: &Fixture, config: &str) -> String {
    fixture.write(".collide/config.yaml", config);
    fixture.write("docs/plan/epics/ep-001-aa-alpha/epic.md", EPIC);
    for id in ["ts-0001-aa", "ts-0002-bb"] {
        fixture.write(
            &format!("docs/plan/epics/ep-001-aa-alpha/tasks/{id}-in-docs.md"),
            &task(id),
        );
    }
    fixture.write(
        ".collide/planning/epics/ep-009-zz-left/tasks/ts-0090-zz-left.md",
        &task("ts-0090-zz"),
    );
    fixture.commit();
    let board = fixture.board();
    trust(fixture, &board);
    board
}

#[test]
fn task_list_shows_the_items_of_the_root_the_repos_layout_names() {
    let fixture = rig();
    let board = board_with(
        &fixture,
        "config:\n  collide/plan:\n    layout:\n      root: docs/plan\n",
    );
    let list = list_until(&fixture, &board, "the plan source lists", |list| {
        !list.refreshing && !list.tasks.is_empty()
    });
    assert_eq!(
        keys(&list),
        ["plan:ep-001-aa", "plan:ts-0001-aa", "plan:ts-0002-bb"]
    );
    assert!(list.stale_sources.is_empty(), "{:?}", list.stale_sources);
}

#[test]
fn a_layout_the_provider_refuses_is_an_error_on_the_source_naming_the_rule_and_the_file() {
    let fixture = rig();
    let board = board_with(
        &fixture,
        "config:\n  collide/plan:\n    layout:\n      root: .collide/views\n",
    );
    let error = rpc::<TaskGetResult>(
        &fixture,
        "task.get",
        &TaskGetParams {
            board,
            id: "plan:ts-0001-aa".to_string(),
            checkout: None,
        },
    )
    .unwrap_err();
    assert!(error.message.contains("layout.root"), "{}", error.message);
    assert!(
        error.message.contains("would cover .collide/views"),
        "{}",
        error.message
    );
}
