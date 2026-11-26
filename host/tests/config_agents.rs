//! The agent entries through `config.get` and the load checks that need the machine (ts-0099-bw; 0.7, Agent entries):
//! the layer of every field, `ask.agent`, the notice for a default budget an agent cannot enforce, a `read` folder that
//! is not there, and a `secrets` item the keychain does not hold. The daemon runs in the test's process over a fixture
//! checkout with a `MemorySecretStore`; no test touches the real keychain, home or launchd.

mod common;

use collide_core::owner::{ConfigEntry, ConfigGetParams, ConfigGetResult, SettingLayer, code};
use collide_core::settings::AGENT_KEYS;
use collide_host::owner_client::ClientError;
use collide_host::secret_store::{MemorySecretStore, Secret, SecretStore};
use common::config_fixture::Fixture;
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::Duration;

fn fixture() -> (Fixture, Arc<MemorySecretStore>) {
    let secret_store = Arc::new(MemorySecretStore::new());
    let held = secret_store.clone();
    (
        Fixture::with_options(move |options| options.secret_store = held),
        secret_store,
    )
}

fn call(
    fixture: &Fixture,
    board: Option<&str>,
    key: Option<&str>,
) -> Result<ConfigGetResult, (i64, String)> {
    let params = ConfigGetParams {
        board: board.map(str::to_string),
        key: key.map(str::to_string),
        profile: None,
        with_problems: false,
    };
    match fixture
        .client()
        .call("config.get", &params, Some(Duration::from_secs(20)))
    {
        Ok(result) => Ok(result),
        Err(ClientError::Rpc(error)) => Err((error.code, error.message)),
        Err(other) => panic!("config.get: {other}"),
    }
}

fn one(fixture: &Fixture, board: Option<&str>, key: &str) -> ConfigEntry {
    let mut result =
        call(fixture, board, Some(key)).unwrap_or_else(|error| panic!("{key}: {error:?}"));
    assert_eq!(result.entries.len(), 1, "{key}");
    result.entries.remove(0)
}

fn shown(entry: &ConfigEntry) -> (Option<Value>, SettingLayer) {
    (entry.value.clone(), entry.layer)
}

// ---- the built-in entry ----

#[test]
fn with_no_config_the_built_in_entry_is_claude_code_haiku_with_a_180_second_timeout() {
    let (fixture, _) = fixture();
    let get = |field: &str| {
        shown(&one(
            &fixture,
            None,
            &format!("agents.claude-haiku.{field}"),
        ))
    };
    assert_eq!(
        get("kind"),
        (Some(json!("claude-code")), SettingLayer::Default)
    );
    assert_eq!(get("model"), (Some(json!("haiku")), SettingLayer::Default));
    assert_eq!(get("config"), (Some(json!("none")), SettingLayer::Default));
    assert_eq!(get("timeout_s"), (Some(json!(180)), SettingLayer::Default));
    assert_eq!(
        get("read"),
        (Some(json!(["<board root>"])), SettingLayer::Default)
    );
    assert_eq!(
        get("tools"),
        (Some(json!(["read", "search"])), SettingLayer::Default)
    );
    assert_eq!(get("budget_usd"), (None, SettingLayer::Default));
}

#[test]
fn an_agent_id_alone_lists_every_field_in_order() {
    let (fixture, _) = fixture();
    let result = call(&fixture, None, Some("agents.claude-haiku")).unwrap();
    let keys: Vec<String> = result
        .entries
        .iter()
        .map(|entry| entry.key.clone())
        .collect();
    let expected: Vec<String> = AGENT_KEYS
        .iter()
        .map(|field| format!("agents.claude-haiku.{field}"))
        .collect();
    assert_eq!(keys, expected);
    assert!(result.entries.iter().all(|entry| !entry.secret));
}

#[test]
fn the_whole_listing_stays_collides_own_keys() {
    let (fixture, _) = fixture();
    let result = call(&fixture, None, None).unwrap();
    assert!(
        result
            .entries
            .iter()
            .all(|entry| !entry.key.starts_with("agents."))
    );
}

#[test]
fn a_field_or_an_agent_that_is_not_there_is_not_found() {
    let (fixture, _) = fixture();
    for key in [
        "agents.nobody.model",
        "agents.claude-haiku.nothing",
        "agents",
        "agents.nobody",
    ] {
        let (code, message) = call(&fixture, None, Some(key)).unwrap_err();
        assert_eq!(code, code::NOT_FOUND, "{key}");
        assert!(message.contains(key), "{message}");
    }
}

// ---- the layers ----

#[test]
fn the_users_file_changes_the_built_in_entry_field_by_field() {
    let (fixture, _) = fixture();
    fixture.write_user(
        "config.yaml",
        "agents:\n  claude-haiku:\n    model: sonnet\n",
    );
    let get = |field: &str| {
        shown(&one(
            &fixture,
            None,
            &format!("agents.claude-haiku.{field}"),
        ))
    };
    assert_eq!(get("model"), (Some(json!("sonnet")), SettingLayer::User));
    assert_eq!(
        get("kind"),
        (Some(json!("claude-code")), SettingLayer::Default)
    );
    assert_eq!(get("timeout_s"), (Some(json!(180)), SettingLayer::Default));
}

#[test]
fn an_entry_the_user_adds_has_its_fields_from_the_user() {
    let (fixture, _) = fixture();
    fixture.write_user(
        "config.yaml",
        "agents:\n  local:\n    kind: acp\n    command: opencode\n    args: [serve]\n    model: ollama/qwen\n",
    );
    let get = |field: &str| shown(&one(&fixture, None, &format!("agents.local.{field}")));
    assert_eq!(get("kind"), (Some(json!("acp")), SettingLayer::User));
    assert_eq!(get("args"), (Some(json!(["serve"])), SettingLayer::User));
    assert_eq!(get("read"), (None, SettingLayer::Default));
    assert_eq!(get("timeout_s"), (Some(json!(180)), SettingLayer::Default));
}

#[test]
fn the_boards_own_user_file_wins_over_the_users() {
    let (fixture, _) = fixture();
    fixture.write(".collide/board.yaml", "packs: []\n");
    fixture.commit();
    let board = fixture.board();
    fixture.write_user(
        "config.yaml",
        "agents:\n  claude-haiku:\n    model: sonnet\n",
    );
    fixture.write_user(
        &format!("boards/{board}/config.yaml"),
        "agents:\n  claude-haiku:\n    model: opus\n",
    );
    assert_eq!(
        shown(&one(&fixture, Some(&board), "agents.claude-haiku.model")),
        (Some(json!("opus")), SettingLayer::User)
    );
}

#[test]
fn the_repos_budget_wins_over_the_entrys_own_and_the_users_timeout_wins_over_the_repos() {
    let (fixture, _) = fixture();
    fixture.write(
        ".collide/config.yaml",
        "ask:\n  budget_usd: 2\n  timeout_s: 100\n",
    );
    fixture.commit();
    let board = fixture.board();
    fixture.write_user(
        "config.yaml",
        "agents:\n  claude-haiku:\n    budget_usd: 9\n    timeout_s: 30\n  other:\n    kind: claude-code\n",
    );
    let get = |id: &str, field: &str| {
        shown(&one(
            &fixture,
            Some(&board),
            &format!("agents.{id}.{field}"),
        ))
    };
    assert_eq!(
        get("claude-haiku", "budget_usd"),
        (Some(json!(2.0)), SettingLayer::Repo)
    );
    assert_eq!(
        get("claude-haiku", "timeout_s"),
        (Some(json!(30)), SettingLayer::User)
    );
    assert_eq!(
        get("other", "budget_usd"),
        (Some(json!(2.0)), SettingLayer::Repo)
    );
    assert_eq!(
        get("other", "timeout_s"),
        (Some(json!(100)), SettingLayer::Repo)
    );
}

#[test]
fn the_users_default_budget_reaches_an_entry_with_none_of_its_own() {
    let (fixture, _) = fixture();
    fixture.write_user("config.yaml", "ask:\n  budget_usd: 3\n");
    assert_eq!(
        shown(&one(&fixture, None, "agents.claude-haiku.budget_usd")),
        (Some(json!(3.0)), SettingLayer::User)
    );
}

// ---- ask.agent ----

#[test]
fn ask_agent_naming_no_entry_fails_naming_ask_agent() {
    let (fixture, _) = fixture();
    fixture.write_user("config.yaml", "ask:\n  agent: nobody\n");
    let (code, message) = call(&fixture, None, None).unwrap_err();
    assert_eq!(code, code::CONFLICT);
    assert!(
        message.contains("ask.agent") && message.contains("nobody"),
        "{message}"
    );
}

#[test]
fn ask_agent_naming_an_entry_the_user_added_loads() {
    let (fixture, _) = fixture();
    fixture.write_user(
        "config.yaml",
        "ask:\n  agent: mine\nagents:\n  mine:\n    kind: claude-code\n",
    );
    call(&fixture, None, None).unwrap();
}

// ---- notices ----

#[test]
fn a_repo_budget_for_an_agent_that_cannot_enforce_one_loads_with_a_notice_naming_it() {
    let (fixture, _) = fixture();
    fixture.write(".collide/config.yaml", "ask:\n  budget_usd: 2\n");
    fixture.commit();
    let board = fixture.board();
    fixture.write_user(
        "config.yaml",
        "agents:\n  local:\n    kind: acp\n    command: opencode\n",
    );
    let result = call(&fixture, Some(&board), Some("ask.budget_usd")).unwrap();
    assert_eq!(result.notices.len(), 1, "{:?}", result.notices);
    assert!(result.notices[0].contains("local"), "{}", result.notices[0]);
    assert_eq!(
        shown(&one(&fixture, Some(&board), "agents.local.budget_usd")),
        (None, SettingLayer::Default)
    );
}

#[test]
fn with_nothing_to_say_there_are_no_notices() {
    let (fixture, _) = fixture();
    assert!(call(&fixture, None, None).unwrap().notices.is_empty());
}

// ---- the load checks that need the machine ----

#[test]
fn a_user_file_with_a_literal_secret_in_env_fails_naming_the_file_and_the_key() {
    let (fixture, _) = fixture();
    fixture.write_user(
        "config.yaml",
        "agents:\n  mine:\n    kind: external\n    env: { OPENAI_API_KEY: sk-test }\n",
    );
    let (code, message) = call(&fixture, None, None).unwrap_err();
    assert_eq!(code, code::CONFLICT);
    assert!(
        message.contains("config.yaml") && message.contains("agents.mine.env.OPENAI_API_KEY"),
        "{message}"
    );
    assert!(message.contains("secrets"), "{message}");
    assert!(!message.contains("sk-test"));
}

#[test]
fn a_user_file_with_a_budget_on_an_acp_entry_or_a_bad_timeout_fails() {
    let (fixture, _) = fixture();
    fixture.write_user(
        "config.yaml",
        "agents:\n  local:\n    kind: acp\n    command: x\n    budget_usd: 1\n    timeout_s: 2\n",
    );
    let (code, message) = call(&fixture, None, None).unwrap_err();
    assert_eq!(code, code::CONFLICT);
    assert!(
        message.contains("only a timeout") || message.contains("timeout only"),
        "{message}"
    );
    assert!(message.contains("agents.local.timeout_s"), "{message}");
}

#[test]
fn a_read_folder_that_does_not_exist_fails_naming_the_key_and_the_folder() {
    let (fixture, _) = fixture();
    let gone = fixture.home().join("not-a-folder");
    fixture.write_user(
        "config.yaml",
        &format!(
            "agents:\n  mine:\n    kind: claude-code\n    read: [{:?}]\n",
            gone.display().to_string()
        ),
    );
    let (code, message) = call(&fixture, None, None).unwrap_err();
    assert_eq!(code, code::CONFLICT);
    assert!(
        message.contains("agents.mine.read") && message.contains("not-a-folder"),
        "{message}"
    );
}

#[test]
fn the_board_root_is_the_folder_of_the_board_and_exists() {
    let (fixture, _) = fixture();
    fixture.write(".collide/board.yaml", "packs: []\n");
    fixture.commit();
    let board = fixture.board();
    call(&fixture, Some(&board), None).unwrap();
}

#[test]
fn a_missing_keychain_item_fails_naming_the_item_and_the_key_and_a_present_one_loads() {
    let (fixture, secret_store) = fixture();
    fixture.write_user(
        "config.yaml",
        "agents:\n  mine:\n    kind: external\n    secrets: { SOME_API_KEY: my-item }\n",
    );
    let (code, message) = call(&fixture, None, None).unwrap_err();
    assert_eq!(code, code::CONFLICT);
    assert!(
        message.contains("my-item") && message.contains("agents.mine.secrets.SOME_API_KEY"),
        "{message}"
    );
    secret_store.set("my-item", &Secret::new("v")).unwrap();
    call(&fixture, None, None).unwrap();
}

#[test]
fn a_repo_file_that_sets_an_agent_entry_fails_the_load_naming_the_key() {
    let (fixture, _) = fixture();
    fixture.write(
        ".collide/config.yaml",
        "agents:\n  mine:\n    kind: claude-code\n    command: x\n",
    );
    fixture.commit();
    let board = fixture.board();
    let (code, message) = call(&fixture, Some(&board), None).unwrap_err();
    assert_eq!(code, code::CONFLICT);
    assert!(message.contains("agents.mine"), "{message}");
}
