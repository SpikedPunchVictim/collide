//! `config.set` in the daemon (ts-0099-bw; 0.7, Settings and Secrets): a secret goes to the keychain and nowhere else,
//! and a plain setting is written to the user's file through `yaml_out`. The daemon runs in the test's process over a
//! fixture checkout with a `MemorySecretStore` the test holds, so no test touches the real keychain, home or launchd.
//! `~/.config/collide` is a folder of the test's own root.

mod common;

use collide_core::owner::{
    ConfigGetParams, ConfigGetResult, ConfigSetParams, ConfigSetResult, ConfigWriteLayer,
    SettingLayer, code,
};
use collide_host::owner_client::ClientError;
use collide_host::secret_store::{MemorySecretStore, Secret, SecretStore};
use common::config_fixture::{CapturedLog, Fixture};
use serde_json::{Value, json};
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

/// A value no setting, log or file may ever hold.
const CANARY: &str = "canary-6f0e2d-never-written";

struct Rig {
    fixture: Fixture,
    secret_store: Arc<MemorySecretStore>,
    log: CapturedLog,
}

fn rig() -> Rig {
    let secret_store = Arc::new(MemorySecretStore::new());
    let log = CapturedLog::default();
    let (held, writer) = (secret_store.clone(), log.clone());
    let fixture = Fixture::with_options(move |options| {
        options.secret_store = held;
        options.log = Box::new(writer);
    });
    Rig {
        fixture,
        secret_store,
        log,
    }
}

fn params(
    board: Option<&str>,
    key: &str,
    value: Value,
    layer: ConfigWriteLayer,
    secret: bool,
) -> ConfigSetParams {
    ConfigSetParams {
        board: board.map(str::to_string),
        key: key.to_string(),
        value,
        layer,
        secret,
    }
}

impl Rig {
    fn set(&self, params: &ConfigSetParams) -> Result<ConfigSetResult, (i64, String)> {
        match self.fixture.client().call::<_, ConfigSetResult>(
            "config.set",
            params,
            Some(Duration::from_secs(20)),
        ) {
            Ok(result) => Ok(result),
            Err(ClientError::Rpc(error)) => Err((error.code, error.message)),
            Err(other) => panic!("config.set: {other}"),
        }
    }

    fn user(
        &self,
        board: Option<&str>,
        key: &str,
        value: Value,
    ) -> Result<ConfigSetResult, (i64, String)> {
        self.set(&params(board, key, value, ConfigWriteLayer::User, false))
    }

    fn secret(
        &self,
        board: Option<&str>,
        key: &str,
        value: &str,
    ) -> Result<ConfigSetResult, (i64, String)> {
        self.set(&params(
            board,
            key,
            json!(value),
            ConfigWriteLayer::User,
            true,
        ))
    }

    fn get(&self, board: Option<&str>, key: &str) -> ConfigGetResult {
        self.fixture
            .client()
            .call(
                "config.get",
                &ConfigGetParams {
                    board: board.map(str::to_string),
                    key: Some(key.to_string()),
                    profile: None,
                    with_problems: false,
                },
                Some(Duration::from_secs(20)),
            )
            .unwrap_or_else(|error| panic!("config.get {key}: {error}"))
    }

    fn user_file(&self, rel: &str) -> String {
        fs::read_to_string(self.fixture.home().join(".config/collide").join(rel))
            .unwrap_or_default()
    }

    /// A checkout with the pack `ferry` committed, opened as a board.
    fn board_with_ferry(&self) -> String {
        self.fixture.ferry();
        self.fixture.commit();
        self.fixture.board()
    }
}

/// Every file under `dir`, read.
fn files_holding(dir: &Path, needle: &str, found: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let kind = entry.file_type().unwrap();
        if kind.is_dir() {
            files_holding(&path, needle, found);
        } else if kind.is_file()
            && fs::read(&path)
                .is_ok_and(|bytes| bytes.windows(needle.len()).any(|w| w == needle.as_bytes()))
        {
            found.push(path.display().to_string());
        }
    }
}

// ---- a secret goes to the keychain only ----

#[test]
fn a_pack_secret_is_kept_in_the_keychain_under_the_pack_item() {
    let rig = rig();
    let board = rig.board_with_ferry();
    let result = rig
        .secret(Some(&board), "ferry.notifyHook", CANARY)
        .unwrap();
    assert_eq!(
        rig.secret_store
            .get("pack.ferry.notifyHook")
            .unwrap()
            .expose(),
        CANARY
    );
    assert_eq!(result.entry.key, "ferry.notifyHook");
    assert!(result.entry.secret);
    assert_eq!(result.entry.value, None);
}

#[test]
fn the_secret_is_in_no_file_no_log_and_no_answer() {
    let rig = rig();
    let board = rig.board_with_ferry();
    let result = rig
        .secret(Some(&board), "ferry.notifyHook", CANARY)
        .unwrap();
    // A second call of each kind, so the log has had every chance to hold it.
    let _ = rig.secret(Some(&board), "ferry.notifyHook", CANARY);
    let _ = rig.secret(Some(&board), "ferry.nothing", CANARY);
    let _ = rig.user(Some(&board), "ferry.notifyHook", json!(CANARY));
    let _ = rig.get(Some(&board), "ferry.notifyHook");

    let mut found = Vec::new();
    files_holding(rig.fixture.home(), CANARY, &mut found);
    assert!(found.is_empty(), "the value is in {found:?}");
    assert!(!rig.log.text().contains(CANARY), "the log holds the value");
    assert!(!serde_json::to_string(&result).unwrap().contains(CANARY));
    assert!(!rig.log.text().is_empty(), "the capture works");
}

#[test]
fn a_refusal_never_holds_the_value() {
    let rig = rig();
    let board = rig.board_with_ferry();
    for (key, secret) in [
        ("ferry.notifyHook", false),
        ("ferry.nothing", true),
        ("disk.floor_gib", true),
    ] {
        let (_, message) = rig
            .set(&params(
                Some(&board),
                key,
                json!(CANARY),
                ConfigWriteLayer::User,
                secret,
            ))
            .unwrap_err();
        assert!(!message.contains(CANARY), "{key}: {message}");
    }
}

#[test]
fn config_get_shows_the_secret_as_set_without_its_value() {
    let rig = rig();
    let board = rig.board_with_ferry();
    rig.secret(Some(&board), "ferry.notifyHook", CANARY)
        .unwrap();
    let entry = rig.get(Some(&board), "ferry.notifyHook").entries.remove(0);
    assert!(entry.secret);
    assert_eq!(entry.value, None);
}

/// The Settings page's request: the one key, with the problems in the result.
fn secret_state(rig: &Rig, board: &str, key: &str) -> Option<bool> {
    let mut result: ConfigGetResult = rig
        .fixture
        .client()
        .call(
            "config.get",
            &ConfigGetParams {
                board: Some(board.to_string()),
                key: Some(key.to_string()),
                profile: None,
                with_problems: true,
            },
            Some(Duration::from_secs(20)),
        )
        .unwrap_or_else(|error| panic!("config.get {key}: {error}"));
    result.entries.remove(0).is_set
}

#[test]
fn config_get_says_not_set_before_a_secret_is_set_and_set_after() {
    let rig = rig();
    let board = rig.board_with_ferry();
    assert_eq!(secret_state(&rig, &board, "ferry.notifyHook"), Some(false));
    rig.secret(Some(&board), "ferry.notifyHook", CANARY)
        .unwrap();
    assert_eq!(secret_state(&rig, &board, "ferry.notifyHook"), Some(true));
}

#[test]
fn config_set_of_a_secret_answers_set_and_a_plain_setting_has_no_state() {
    let rig = rig();
    let board = rig.board_with_ferry();
    let result = rig
        .secret(Some(&board), "ferry.notifyHook", CANARY)
        .unwrap();
    assert_eq!(result.entry.is_set, Some(true));
    let plain = rig.get(Some(&board), "ferry.greeting").entries.remove(0);
    assert_eq!(plain.is_set, None);
}

#[test]
fn the_listing_the_page_reads_holds_the_secret_nowhere() {
    let rig = rig();
    let board = rig.board_with_ferry();
    rig.secret(Some(&board), "ferry.notifyHook", CANARY)
        .unwrap();
    let listing = rig
        .fixture
        .client()
        .call::<_, Value>(
            "config.get",
            &ConfigGetParams {
                board: Some(board.clone()),
                key: None,
                profile: None,
                with_problems: true,
            },
            Some(Duration::from_secs(20)),
        )
        .unwrap();
    assert!(!listing.to_string().contains(CANARY), "{listing}");
    assert!(!rig.log.text().contains(CANARY));
}

#[test]
fn a_second_secret_replaces_the_first() {
    let rig = rig();
    let board = rig.board_with_ferry();
    rig.secret(Some(&board), "ferry.notifyHook", "one").unwrap();
    rig.secret(Some(&board), "ferry.notifyHook", "two").unwrap();
    assert_eq!(
        rig.secret_store
            .get("pack.ferry.notifyHook")
            .unwrap()
            .expose(),
        "two"
    );
}

const AGENT: &str = "agents:\n  mine:\n    kind: external\n    command: x\n    secrets: { SOME_API_KEY: my-item }\n";

#[test]
fn an_agents_secret_is_kept_under_the_item_its_entry_names() {
    let rig = rig();
    rig.fixture.write_user("config.yaml", AGENT);
    let result = rig
        .secret(None, "agents.mine.secrets.SOME_API_KEY", CANARY)
        .unwrap();
    assert_eq!(rig.secret_store.get("my-item").unwrap().expose(), CANARY);
    assert!(result.entry.secret);
    assert_eq!(result.entry.value, None);
    assert_eq!(
        rig.user_file("config.yaml"),
        AGENT,
        "the file is not touched"
    );
}

#[test]
fn an_agents_secret_works_while_its_item_is_still_missing() {
    // The load refuses an entry whose item is missing, and this is how the item gets made.
    let rig = rig();
    rig.fixture.write_user("config.yaml", AGENT);
    assert!(rig.secret_store.contains("my-item") == Ok(false));
    rig.secret(None, "agents.mine.secrets.SOME_API_KEY", "v")
        .unwrap();
}

#[test]
fn an_agent_secret_the_entry_does_not_name_is_refused_naming_the_key() {
    let rig = rig();
    rig.fixture.write_user("config.yaml", AGENT);
    for key in [
        "agents.mine.secrets.OTHER_KEY",
        "agents.nobody.secrets.SOME_API_KEY",
    ] {
        let (code, message) = rig.secret(None, key, "v").unwrap_err();
        assert_eq!(code, code::INVALID_PARAMS, "{key}");
        assert!(message.contains(key), "{message}");
    }
}

#[test]
fn a_key_that_is_no_secret_is_refused_with_secret_naming_the_key() {
    let rig = rig();
    let board = rig.board_with_ferry();
    for key in [
        "disk.floor_gib",
        "ferry.greeting",
        "nothing.at.all",
        "agents.mine.model",
        "agents.mine.secrets",
    ] {
        let (code, message) = rig.secret(Some(&board), key, "v").unwrap_err();
        assert_eq!(code, code::INVALID_PARAMS, "{key}");
        assert!(message.contains(key), "{message}");
    }
    assert_eq!(rig.secret_store.contains("pack.ferry.greeting"), Ok(false));
}

#[test]
fn a_pack_secret_needs_a_board_and_says_so() {
    let rig = rig();
    let (code, message) = rig.secret(None, "ferry.notifyHook", "v").unwrap_err();
    assert_eq!(code, code::INVALID_PARAMS);
    assert!(
        message.contains("ferry.notifyHook") && message.contains("board"),
        "{message}"
    );
}

#[test]
fn an_empty_secret_or_one_that_is_not_text_is_refused() {
    let rig = rig();
    let board = rig.board_with_ferry();
    for value in [json!(""), json!(5), json!(null), json!(["a"])] {
        let (code, _) = rig
            .set(&params(
                Some(&board),
                "ferry.notifyHook",
                value,
                ConfigWriteLayer::User,
                true,
            ))
            .unwrap_err();
        assert_eq!(code, code::INVALID_PARAMS);
    }
    assert_eq!(
        rig.secret_store.contains("pack.ferry.notifyHook"),
        Ok(false)
    );
}

#[test]
fn a_secret_for_a_board_that_is_not_there_is_not_found() {
    let rig = rig();
    let (code, _) = rig
        .secret(Some("board-99"), "ferry.notifyHook", "v")
        .unwrap_err();
    assert_eq!(code, code::NOT_FOUND);
}

#[test]
fn a_secret_asked_for_with_layer_repo_is_still_a_keychain_write() {
    // `layer` is ignored with `secret`, as the wire says.
    let rig = rig();
    let board = rig.board_with_ferry();
    rig.set(&params(
        Some(&board),
        "ferry.notifyHook",
        json!("v"),
        ConfigWriteLayer::Repo,
        true,
    ))
    .unwrap();
    assert_eq!(
        rig.secret_store
            .get("pack.ferry.notifyHook")
            .unwrap()
            .expose(),
        "v"
    );
}

#[test]
fn a_keychain_that_fails_is_reported_without_the_value() {
    struct Failing;
    impl SecretStore for Failing {
        fn set(
            &self,
            item: &str,
            _: &Secret,
        ) -> Result<(), collide_host::secret_store::SecretStoreError> {
            Err(collide_host::secret_store::SecretStoreError::Failed {
                item: item.to_string(),
                what: "store",
                reason: "locked".into(),
            })
        }
        fn contains(&self, _: &str) -> Result<bool, collide_host::secret_store::SecretStoreError> {
            Ok(false)
        }
        fn get(&self, item: &str) -> Result<Secret, collide_host::secret_store::SecretStoreError> {
            Err(collide_host::secret_store::SecretStoreError::NotFound {
                item: item.to_string(),
            })
        }
        fn delete(&self, _: &str) -> Result<(), collide_host::secret_store::SecretStoreError> {
            Ok(())
        }
    }
    let fixture = Fixture::with_options(|options| options.secret_store = Arc::new(Failing));
    fixture.ferry();
    fixture.commit();
    let board = fixture.board();
    let answer: Result<ConfigSetResult, _> = fixture.client().call(
        "config.set",
        &params(
            Some(&board),
            "ferry.notifyHook",
            json!(CANARY),
            ConfigWriteLayer::User,
            true,
        ),
        Some(Duration::from_secs(20)),
    );
    match answer {
        Err(ClientError::Rpc(error)) => {
            assert!(
                error.message.contains("pack.ferry.notifyHook") && error.message.contains("locked")
            );
            assert!(!error.message.contains(CANARY));
        }
        other => panic!("{other:?}"),
    }
}

// ---- a plain setting is written to the user's file ----

#[test]
fn a_plain_setting_is_written_to_the_users_file_and_read_back_as_the_users() {
    let rig = rig();
    let result = rig.user(None, "disk.floor_gib", json!("7")).unwrap();
    assert_eq!(result.entry.value, Some(json!(7)));
    assert_eq!(result.entry.layer, SettingLayer::User);
    assert_eq!(rig.user_file("config.yaml"), "disk:\n  floor_gib: 7\n");
    let got = rig.get(None, "disk.floor_gib").entries.remove(0);
    assert_eq!((got.value, got.layer), (Some(json!(7)), SettingLayer::User));
}

#[test]
fn the_users_file_is_private_to_the_user() {
    use std::os::unix::fs::PermissionsExt;
    let rig = rig();
    rig.user(None, "disk.floor_gib", json!(7)).unwrap();
    let mode = fs::metadata(rig.fixture.home().join(".config/collide/config.yaml"))
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o600);
}

#[test]
fn the_typed_value_is_read_as_the_settings_type() {
    let rig = rig();
    assert_eq!(
        rig.user(None, "claude.check_published", json!("false"))
            .unwrap()
            .entry
            .value,
        Some(json!(false))
    );
    assert_eq!(
        rig.user(None, "ask.budget_usd", json!("0.5"))
            .unwrap()
            .entry
            .value,
        Some(json!(0.5))
    );
    assert_eq!(
        rig.user(None, "ask.agent", json!("claude-haiku"))
            .unwrap()
            .entry
            .value,
        Some(json!("claude-haiku"))
    );
}

#[test]
fn a_value_already_of_the_right_type_is_kept() {
    let rig = rig();
    assert_eq!(
        rig.user(None, "disk.floor_gib", json!(9))
            .unwrap()
            .entry
            .value,
        Some(json!(9))
    );
}

#[test]
fn a_second_write_keeps_what_the_file_already_held() {
    let rig = rig();
    rig.user(None, "disk.floor_gib", json!(7)).unwrap();
    rig.user(None, "ask.timeout_s", json!(60)).unwrap();
    assert_eq!(
        rig.user_file("config.yaml"),
        "ask:\n  timeout_s: 60\ndisk:\n  floor_gib: 7\n"
    );
}

#[test]
fn a_pack_setting_is_written_under_config_and_the_pack() {
    let rig = rig();
    let board = rig.board_with_ferry();
    let result = rig
        .user(Some(&board), "ferry.greeting", json!("hi there"))
        .unwrap();
    assert_eq!(result.entry.value, Some(json!("hi there")));
    assert_eq!(result.entry.layer, SettingLayer::User);
    assert_eq!(
        rig.user_file(&format!("boards/{board}/config.yaml")),
        "config:\n  ferry:\n    greeting: \"hi there\"\n"
    );
    assert_eq!(
        rig.user_file("config.yaml"),
        "",
        "the user's own file is not touched"
    );
}

#[test]
fn with_a_board_a_collide_setting_goes_to_the_boards_own_user_file() {
    let rig = rig();
    let board = rig.board_with_ferry();
    rig.user(Some(&board), "disk.floor_gib", json!(8)).unwrap();
    assert_eq!(
        rig.user_file(&format!("boards/{board}/config.yaml")),
        "disk:\n  floor_gib: 8\n"
    );
}

#[test]
fn the_result_is_the_setting_after_the_write_so_a_repo_value_that_wins_is_shown() {
    let rig = rig();
    rig.fixture
        .write(".collide/config.yaml", "ask:\n  budget_usd: 1\n");
    rig.fixture.commit();
    let board = rig.fixture.board();
    let result = rig
        .user(Some(&board), "ask.budget_usd", json!("5"))
        .unwrap();
    assert_eq!(result.entry.value, Some(json!(1.0)));
    assert_eq!(result.entry.layer, SettingLayer::Repo);
    assert!(
        rig.user_file(&format!("boards/{board}/config.yaml"))
            .contains("budget_usd: 5")
    );
}

#[test]
fn an_invalid_value_is_refused_naming_the_key_and_writes_nothing() {
    let rig = rig();
    let board = rig.board_with_ferry();
    for (key, value) in [
        ("disk.floor_gib", json!("0")),
        ("disk.floor_gib", json!("seven")),
        ("ask.timeout_s", json!("4")),
        ("claude.check_published", json!("maybe")),
        ("ferry.lanePool", json!("0")),
    ] {
        let (code, message) = rig.user(Some(&board), key, value.clone()).unwrap_err();
        assert_eq!(code, code::INVALID_PARAMS, "{key} {value}");
        assert!(message.contains(key), "{message}");
    }
    assert_eq!(rig.user_file("config.yaml"), "");
    assert_eq!(rig.user_file(&format!("boards/{board}/config.yaml")), "");
}

#[test]
fn an_unknown_key_is_refused_naming_it() {
    let rig = rig();
    let board = rig.board_with_ferry();
    for key in [
        "nothing.here",
        "ferry.nothing",
        "disk",
        "disk.floor_gib.x",
        "ask.agent.x",
        "",
    ] {
        let (code, message) = rig.user(Some(&board), key, json!("1")).unwrap_err();
        assert_eq!(code, code::INVALID_PARAMS, "{key:?}");
        assert!(message.contains(&format!("`{key}`")), "{message}");
    }
}

#[test]
fn a_pack_key_with_no_board_is_refused_and_the_message_says_a_board_is_needed() {
    let rig = rig();
    let (code, message) = rig.user(None, "ferry.greeting", json!("x")).unwrap_err();
    assert_eq!(code, code::INVALID_PARAMS);
    assert!(
        message.contains("ferry.greeting") && message.contains("board"),
        "{message}"
    );
}

#[test]
fn a_secret_key_without_secret_is_refused_and_says_to_use_secret() {
    let rig = rig();
    let board = rig.board_with_ferry();
    let (code, message) = rig
        .user(Some(&board), "ferry.notifyHook", json!(CANARY))
        .unwrap_err();
    assert_eq!(code, code::INVALID_PARAMS);
    assert!(
        message.contains("--secret") && message.contains("ferry.notifyHook"),
        "{message}"
    );
    assert!(!message.contains(CANARY));
    assert_eq!(
        rig.secret_store.contains("pack.ferry.notifyHook"),
        Ok(false)
    );
    assert_eq!(rig.user_file(&format!("boards/{board}/config.yaml")), "");
}

#[test]
fn a_repo_only_key_is_refused_in_the_users_file() {
    let rig = rig();
    let board = rig.board_with_ferry();
    let (code, message) = rig
        .user(Some(&board), "ferry.layout", json!("wide"))
        .unwrap_err();
    assert_eq!(code, code::INVALID_PARAMS);
    assert!(
        message.contains("ferry.layout") && message.contains("repo"),
        "{message}"
    );
}

#[test]
fn an_agent_field_is_not_set_here_and_the_message_says_to_edit_the_file() {
    let rig = rig();
    let (code, message) = rig
        .user(None, "agents.claude-haiku.model", json!("sonnet"))
        .unwrap_err();
    assert_eq!(code, code::INVALID_PARAMS);
    assert!(
        message.contains("agents.claude-haiku.model") && message.contains("config.yaml"),
        "{message}"
    );
}

// ---- refusals that leave the file alone ----

#[test]
fn a_file_with_a_comment_is_not_rewritten_and_the_message_says_why() {
    let rig = rig();
    let text = "# my settings\ndisk:\n  floor_gib: 3\n";
    rig.fixture.write_user("config.yaml", text);
    let (code, message) = rig.user(None, "ask.timeout_s", json!(60)).unwrap_err();
    assert_eq!(code, code::CONFLICT);
    assert!(
        message.contains("comment") && message.contains("edit"),
        "{message}"
    );
    assert!(message.contains("config.yaml"), "{message}");
    assert_eq!(rig.user_file("config.yaml"), text);
}

#[test]
fn a_hash_inside_a_value_is_not_a_comment() {
    let rig = rig();
    rig.fixture.write_user(
        "config.yaml",
        "ask:\n  agent: \"a # b\"\nagents:\n  \"a # b\":\n    kind: claude-code\n",
    );
    rig.user(None, "disk.floor_gib", json!(4)).unwrap();
    assert!(rig.user_file("config.yaml").contains("floor_gib: 4"));
}

#[test]
fn a_repo_write_is_refused_and_says_to_edit_and_commit_the_repo_file() {
    let rig = rig();
    let board = rig.board_with_ferry();
    let (code, message) = rig
        .set(&params(
            Some(&board),
            "ask.timeout_s",
            json!(60),
            ConfigWriteLayer::Repo,
            false,
        ))
        .unwrap_err();
    assert_eq!(code, code::CONFLICT);
    assert!(
        message.contains("edit and commit `.collide/config.yaml`"),
        "{message}"
    );
    assert_eq!(rig.user_file("config.yaml"), "");
    assert_eq!(rig.user_file(&format!("boards/{board}/config.yaml")), "");
}

#[test]
fn a_malformed_users_file_is_refused_naming_it_and_left_as_it_was() {
    let rig = rig();
    rig.fixture.write_user("config.yaml", "disk: [unclosed\n");
    let (code, message) = rig.user(None, "ask.timeout_s", json!(60)).unwrap_err();
    assert_eq!(code, code::CONFLICT);
    assert!(message.contains("config.yaml"), "{message}");
    assert_eq!(rig.user_file("config.yaml"), "disk: [unclosed\n");
}

#[test]
fn a_write_that_would_make_the_settings_worse_is_refused_and_not_written() {
    let rig = rig();
    let (code, message) = rig.user(None, "ask.agent", json!("nobody")).unwrap_err();
    assert_eq!(code, code::CONFLICT);
    assert!(message.contains("ask.agent"), "{message}");
    assert_eq!(rig.user_file("config.yaml"), "");
}

#[test]
fn a_problem_the_file_already_had_does_not_stop_a_write_that_adds_none() {
    // Otherwise the way out of a bad `ask.agent` would be to edit the file by hand.
    let rig = rig();
    rig.fixture
        .write_user("config.yaml", "ask:\n  agent: nobody\n");
    rig.user(None, "disk.floor_gib", json!(4)).unwrap();
    rig.user(None, "ask.agent", json!("claude-haiku")).unwrap();
    assert!(
        rig.user_file("config.yaml")
            .contains("agent: \"claude-haiku\"")
    );
}

#[test]
fn a_board_id_that_is_not_a_safe_name_is_refused() {
    let rig = rig();
    let board = rig.board_with_ferry();
    let _ = board;
    let (code, _) = rig
        .user(Some("../escape"), "disk.floor_gib", json!(4))
        .unwrap_err();
    assert!(
        code == code::NOT_FOUND || code == code::INVALID_PARAMS,
        "{code}"
    );
    assert!(!rig.fixture.home().join(".config/escape").exists());
}
