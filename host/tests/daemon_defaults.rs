//! Start with defaults through the daemon (ts-0215-gt; 0.7, Layers, and Stage 6a, Start with defaults): a connection
//! that says `defaults` at `initialize` reads only the Default layer, the built-in packs and the built-in views, and
//! its writes are refused, while a normal connection on the same daemon still sees the person's files and their
//! problems.
//! `config.reset` moves the person's files aside and deletes nothing.
//!
//! The daemon runs in the test's process over a temp root that is also the home, so `~/.config/collide` is the test's
//! own folder. The keychain is a `MemorySecretStore`. Nothing here touches the real home, keychain or a real daemon.

mod common;

use collide_core::owner::{
    BoardListResult, ConfigGetResult, ConfigPackListResult, ConfigResetResult, PackOrigin,
    RpcError, SettingLayer, TaskListResult, TaskViewsResult, TrustState, ViewSource, code,
};
use collide_host::owner_client::{ClientError, OwnerClient};
use collide_host::secret_store::{MemorySecretStore, Secret, SecretStore};
use common::config_fixture::Fixture;
use common::wait_until;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

const WAIT: Option<Duration> = Some(Duration::from_secs(60));
const DEADLINE: Duration = Duration::from_secs(30);

fn call<R: DeserializeOwned>(
    client: &mut OwnerClient,
    method: &str,
    params: Value,
) -> Result<R, RpcError> {
    match client.call(method, &params, WAIT) {
        Ok(result) => Ok(result),
        Err(ClientError::Rpc(error)) => Err(error),
        Err(other) => panic!("{method}: {other}"),
    }
}

/// A board with two scratch tasks, and then the person's files broken in the three ways the item names: a settings
/// file that does not parse, a personal view that does not compile, and a user pack whose folder is gone.
struct World {
    fixture: Fixture,
    board: String,
    secrets: Arc<MemorySecretStore>,
}

impl World {
    fn new() -> World {
        let secrets = Arc::new(MemorySecretStore::new());
        let keep = secrets.clone();
        let fixture = Fixture::with_options(move |options| options.secret_store = keep);
        let board = fixture.board();
        let world = World {
            fixture,
            board,
            secrets,
        };
        world.scratch("first made-up task");
        world.scratch("second made-up task");
        world.wait_for_tasks(2);
        world
    }

    fn broken() -> World {
        let world = World::new();
        world.break_files();
        world
    }

    fn scratch(&self, prompt: &str) {
        let answer: Value = call(
            &mut self.normal(),
            "task.apply",
            json!({
                "board": self.board,
                "source": "scratch",
                "apply": { "op": "create", "args": { "prompt": prompt } },
            }),
        )
        .unwrap();
        assert_eq!(answer["result"]["outcome"], "ok", "{answer}");
    }

    fn wait_for_tasks(&self, count: usize) {
        wait_until("the scratch tasks list", DEADLINE, || {
            let list: TaskListResult = call(
                &mut self.normal(),
                "task.list",
                json!({ "board": self.board }),
            )
            .unwrap();
            !list.refreshing && list.tasks.len() == count
        });
    }

    fn break_files(&self) {
        let gone = self.fixture.home().join("devs/gone");
        self.fixture.write_user("config.yaml", "disk: [unclosed\n");
        self.fixture.write_user(
            &format!("boards/{}/views/broken.yaml", self.board),
            "name: broken\nwhere: \"status ==\"\n",
        );
        self.fixture.write_user(
            "packs.yaml",
            &format!("packs:\n  - source: dev:{}\n", gone.display()),
        );
        self.fixture
            .write_user(&format!("boards/{}/packs.yaml", self.board), "on: [gone]\n");
    }

    fn normal(&self) -> OwnerClient {
        self.fixture.client()
    }

    fn defaults(&self) -> OwnerClient {
        OwnerClient::connect_as(self.fixture.daemon.paths(), "defaults-test", true)
            .expect("the defaults client connects")
    }

    fn config_dir(&self) -> PathBuf {
        self.fixture.home().join(".config/collide")
    }

    /// Every file of the person's folder and of the repo's `.collide/`, with its bytes.
    fn files(&self) -> BTreeMap<PathBuf, Vec<u8>> {
        let mut found = BTreeMap::new();
        read_tree(&self.config_dir(), &mut found);
        read_tree(&self.fixture.repo.work().join(".collide"), &mut found);
        found
    }
}

fn read_tree(dir: &Path, found: &mut BTreeMap<PathBuf, Vec<u8>>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            read_tree(&path, found);
        } else {
            found.insert(path.clone(), fs::read(&path).unwrap());
        }
    }
}

fn refusal(result: Result<Value, RpcError>) -> RpcError {
    result.expect_err("the call should be refused")
}

// ---- the fixture itself ----

#[test]
fn the_broken_files_break_a_normal_connection() {
    let world = World::broken();
    let mut normal = world.normal();
    let settings = refusal(call(
        &mut normal,
        "config.get",
        json!({ "board": world.board }),
    ));
    assert_eq!(settings.code, code::CONFLICT, "{}", settings.message);
    assert!(
        settings.message.contains("config.yaml"),
        "{}",
        settings.message
    );
    let views = refusal(call(
        &mut normal,
        "task.views",
        json!({ "board": world.board }),
    ));
    assert_eq!(views.code, code::CONFLICT, "{}", views.message);
    let boards: BoardListResult = call(&mut normal, "board.list", json!({})).unwrap();
    assert!(
        !boards.boards[0].lines[0].problems.is_empty(),
        "{:?}",
        boards.boards[0]
    );
}

// ---- what a defaults connection reads ----

#[test]
fn defaults_show_every_setting_at_default_and_read_no_settings_file() {
    let world = World::broken();
    let got: ConfigGetResult = call(
        &mut world.defaults(),
        "config.get",
        json!({ "board": world.board }),
    )
    .unwrap();
    assert!(!got.entries.is_empty());
    for entry in &got.entries {
        assert_eq!(entry.layer, SettingLayer::Default, "{}", entry.key);
    }
    assert!(got.problems.is_empty(), "{:?}", got.problems);
    assert!(got.files.is_empty(), "{:?}", got.files);
}

#[test]
fn defaults_list_the_built_in_packs_and_no_other() {
    let world = World::broken();
    let listed: ConfigPackListResult = call(
        &mut world.defaults(),
        "config.pack_list",
        json!({ "board": world.board }),
    )
    .unwrap();
    assert!(!listed.packs.is_empty());
    for pack in &listed.packs {
        assert_eq!(pack.origin, PackOrigin::Builtin, "{}", pack.name);
    }
    let normal = refusal(call(
        &mut world.normal(),
        "config.pack_list",
        json!({ "board": world.board }),
    ));
    assert_eq!(normal.code, code::CONFLICT, "{}", normal.message);
}

#[test]
fn defaults_open_the_board_with_the_built_in_views_and_the_fixtures_tasks() {
    let world = World::broken();
    let mut defaults = world.defaults();
    let views: TaskViewsResult =
        call(&mut defaults, "task.views", json!({ "board": world.board })).unwrap();
    assert!(!views.views.is_empty());
    assert!(!views.shared, "no Collide folder is read in defaults mode");
    for entry in &views.views {
        assert!(
            matches!(
                entry.source,
                ViewSource::Builtin {} | ViewSource::Pack { .. }
            ),
            "{}: {:?}",
            entry.view.name,
            entry.source
        );
        assert_eq!(entry.error, None, "{}", entry.view.name);
    }
    assert!(views.views.iter().any(|entry| entry.view.name == "ready"));
    let list: TaskListResult = call(
        &mut defaults,
        "task.list",
        json!({ "board": world.board, "view": { "type": "named", "name": "ready" } }),
    )
    .unwrap();
    assert_eq!(list.view.as_ref().unwrap().error, None);
    let all: TaskListResult =
        call(&mut defaults, "task.list", json!({ "board": world.board })).unwrap();
    assert_eq!(all.tasks.len(), 2, "{:?}", all.tasks);
}

#[test]
fn defaults_show_no_problems_no_trust_and_no_review() {
    let world = World::broken();
    let mut defaults = world.defaults();
    let boards: BoardListResult = call(&mut defaults, "board.list", json!({})).unwrap();
    let line = &boards.boards[0].lines[0];
    assert!(line.problems.is_empty(), "{:?}", line.problems);
    assert_eq!(
        line.trust,
        TrustState::Untrusted {
            hash: String::new()
        }
    );
    let review: Value = call(
        &mut defaults,
        "board.trust_review",
        json!({ "board": world.board, "line": "main" }),
    )
    .unwrap();
    for key in ["problems", "changes", "packs", "hosts", "dev_folders"] {
        let empty = review[key].as_array().is_none_or(Vec::is_empty);
        assert!(empty, "{key}: {review}");
    }
    assert_eq!(review["trust"]["hash"], "");
}

#[test]
fn a_normal_connection_on_the_same_daemon_still_sees_the_persons_files_and_problems() {
    let world = World::broken();
    let mut defaults = world.defaults();
    let _: ConfigGetResult =
        call(&mut defaults, "config.get", json!({ "board": world.board })).unwrap();
    let mut normal = world.normal();
    let settings = refusal(call(
        &mut normal,
        "config.get",
        json!({ "board": world.board }),
    ));
    assert!(
        settings.message.contains("config.yaml"),
        "{}",
        settings.message
    );
    let boards: BoardListResult = call(&mut normal, "board.list", json!({})).unwrap();
    assert!(!boards.boards[0].lines[0].problems.is_empty());
    let _: BoardListResult = call(&mut defaults, "board.list", json!({})).unwrap();
}

// ---- writes ----

#[test]
fn every_write_is_refused_in_defaults_mode_with_words_that_name_it_and_changes_no_file() {
    let world = World::broken();
    let before = world.files();
    let mut defaults = world.defaults();
    let board = world.board.as_str();
    let writes = [
        (
            "config.set",
            json!({ "key": "disk.floor_gib", "value": 5, "layer": "user" }),
        ),
        (
            "task.save_view",
            json!({ "board": board, "view": { "name": "mine" }, "target": "personal" }),
        ),
        (
            "config.pack_add",
            json!({ "source": "dev:/nowhere/at/all" }),
        ),
        (
            "config.pack_link",
            json!({ "board": board, "name": "gone" }),
        ),
        (
            "config.pack_enable",
            json!({ "board": board, "name": "gone", "enabled": false }),
        ),
        (
            "config.pack_update",
            json!({ "board": board, "name": "gone" }),
        ),
        ("board.init", json!({ "board": board })),
        (
            "board.trust",
            json!({ "board": board, "line": "main", "hash": "abc" }),
        ),
        (
            "task.apply",
            json!({
                "board": board,
                "source": "scratch",
                "apply": { "op": "create", "args": { "prompt": "more" } },
            }),
        ),
        (
            "board.open",
            json!({ "root": "/nowhere", "init_git": true }),
        ),
    ];
    for (method, params) in writes {
        let error = refusal(call(&mut defaults, method, params));
        assert_eq!(
            error.code,
            code::NOT_PERMITTED,
            "{method}: {}",
            error.message
        );
        assert!(
            error.message.contains("defaults"),
            "{method}: {}",
            error.message
        );
        assert!(
            error.message.contains(method.split('.').next().unwrap()),
            "{method}"
        );
    }
    assert_eq!(world.files(), before, "no file changed");
    world.wait_for_tasks(2);
}

// ---- trust ----

#[test]
fn a_trusted_repos_views_and_packs_do_not_load_and_its_trust_record_is_unchanged() {
    let secrets = Arc::new(MemorySecretStore::new());
    let fixture = Fixture::with_options({
        let secrets = secrets.clone();
        move |options| options.secret_store = secrets
    });
    fixture.ferry();
    fixture.write(
        ".collide/views/mine.yaml",
        "name: mine\nwhere: \"status == 'todo'\"\n",
    );
    fixture.commit();
    let board = fixture.board();
    let mut normal = fixture.client();
    let review: Value = call(
        &mut normal,
        "board.trust_review",
        json!({ "board": board, "line": "main" }),
    )
    .unwrap();
    let hash = review["trust"]["hash"].as_str().unwrap().to_string();
    assert!(!hash.is_empty(), "{review}");
    let _: Value = call(
        &mut normal,
        "board.trust",
        json!({ "board": board, "line": "main", "hash": hash }),
    )
    .unwrap();
    let trust_file = fixture.home().join(".config/collide/trust.yaml");
    let recorded = fs::read(&trust_file).expect("the trust is recorded");
    let views: TaskViewsResult =
        call(&mut normal, "task.views", json!({ "board": board })).unwrap();
    assert!(views.views.iter().any(|entry| entry.view.name == "mine"));
    let packs: ConfigPackListResult =
        call(&mut normal, "config.pack_list", json!({ "board": board })).unwrap();
    assert!(packs.packs.iter().any(|pack| pack.name == "ferry"));

    let mut defaults =
        OwnerClient::connect_as(fixture.daemon.paths(), "defaults-test", true).unwrap();
    let views: TaskViewsResult =
        call(&mut defaults, "task.views", json!({ "board": board })).unwrap();
    assert!(
        views
            .views
            .iter()
            .all(|entry| entry.source != ViewSource::Repo {}),
        "{:?}",
        views
            .views
            .iter()
            .map(|entry| &entry.view.name)
            .collect::<Vec<_>>()
    );
    let packs: ConfigPackListResult =
        call(&mut defaults, "config.pack_list", json!({ "board": board })).unwrap();
    assert!(packs.packs.iter().all(|pack| pack.name != "ferry"));
    let got: ConfigGetResult =
        call(&mut defaults, "config.get", json!({ "board": board })).unwrap();
    assert!(
        got.entries
            .iter()
            .all(|entry| !entry.key.starts_with("ferry."))
    );
    let boards: BoardListResult = call(&mut defaults, "board.list", json!({})).unwrap();
    assert!(matches!(
        boards.boards[0].lines[0].trust,
        TrustState::Untrusted { .. }
    ));
    assert_eq!(
        fs::read(&trust_file).unwrap(),
        recorded,
        "the trust record is unchanged"
    );
    let boards: BoardListResult = call(&mut normal, "board.list", json!({})).unwrap();
    assert!(matches!(
        boards.boards[0].lines[0].trust,
        TrustState::Trusted { .. }
    ));
}

// ---- reset ----

#[test]
fn reset_moves_the_listed_files_aside_and_deletes_nothing() {
    let world = World::broken();
    let board = world.board.clone();
    let dir = world.config_dir();
    world.fixture.write_user("packs.lock", "packs: []\n");
    world.fixture.write_user("trust.yaml", "kept: true\n");
    world
        .fixture
        .write_user("boards/board-99/config.yaml", "other: board\n");
    world.fixture.write(".collide/board.yaml", "packs: []\n");
    world.fixture.commit();
    world
        .secrets
        .set("pack.ferry.token", &Secret::new("hush"))
        .unwrap();
    let before = world.files();
    let moved_files = [
        "config.yaml".to_string(),
        "packs.yaml".to_string(),
        "packs.lock".to_string(),
    ];
    let board_folder = format!("boards/{board}");

    let mut defaults = world.defaults();
    let done: ConfigResetResult =
        call(&mut defaults, "config.reset", json!({ "board": board })).unwrap();
    let folder = PathBuf::from(&done.folder);
    assert!(folder.starts_with(dir.join("set-aside")), "{}", done.folder);
    let mut expected: Vec<String> = moved_files
        .iter()
        .cloned()
        .chain([board_folder.clone()])
        .collect();
    expected.sort();
    let mut listed = done.moved.clone();
    listed.sort();
    assert_eq!(listed, expected);

    for (path, bytes) in &before {
        let relative = path.strip_prefix(&dir);
        let moved = relative.as_ref().is_ok_and(|relative| {
            let name = relative.to_string_lossy();
            moved_files.iter().any(|file| *file == name) || name.starts_with(&board_folder)
        });
        if moved {
            assert!(!path.exists(), "{} was moved", path.display());
            let now = folder.join(path.strip_prefix(&dir).unwrap());
            assert_eq!(&fs::read(&now).unwrap(), bytes, "{}", now.display());
        } else {
            assert_eq!(&fs::read(path).unwrap(), bytes, "{} stays", path.display());
        }
    }
    let after = world.files();
    let kept: Vec<_> = after
        .keys()
        .filter(|path| !path.starts_with(dir.join("set-aside")))
        .collect();
    assert!(kept.iter().any(|path| path.ends_with("trust.yaml")));
    assert!(
        kept.iter()
            .any(|path| path.ends_with("boards/board-99/config.yaml"))
    );
    assert!(
        kept.iter()
            .any(|path| path.ends_with(".collide/board.yaml"))
    );
    assert!(
        world.secrets.contains("pack.ferry.token").unwrap(),
        "the keychain is unchanged"
    );
    assert_eq!(
        world.secrets.get("pack.ferry.token").unwrap().expose(),
        "hush"
    );
}

#[test]
fn after_a_reset_a_normal_connection_reads_the_defaults_too() {
    let world = World::broken();
    let _: ConfigResetResult = call(
        &mut world.normal(),
        "config.reset",
        json!({ "board": world.board }),
    )
    .unwrap();
    let got: ConfigGetResult = call(
        &mut world.normal(),
        "config.get",
        json!({ "board": world.board }),
    )
    .unwrap();
    assert!(
        got.entries
            .iter()
            .all(|entry| entry.layer == SettingLayer::Default)
    );
    let _: TaskViewsResult = call(
        &mut world.normal(),
        "task.views",
        json!({ "board": world.board }),
    )
    .unwrap();
}

#[test]
fn a_reset_of_an_unknown_board_is_refused_and_moves_nothing() {
    let world = World::broken();
    let before = world.files();
    let error = refusal(call(
        &mut world.normal(),
        "config.reset",
        json!({ "board": "board-404" }),
    ));
    assert_eq!(error.code, code::NOT_FOUND, "{}", error.message);
    assert_eq!(world.files(), before);
}
