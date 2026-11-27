//! `config.get` in the daemon (ts-0098-nx; 0.7, Settings): the four layers over a fixture checkout, with the layer of
//! every value, through the daemon's real socket inside the test process. The repo layer is read from the checkout's
//! `HEAD`, the user's layers from `~/.config/collide` of the test's own root, and the packs from a fixture. Nothing
//! here touches the real home, a real daemon or `launchd`.

use collide_core::owner::{
    ConfigEntry, ConfigGetParams, ConfigGetResult, ConfigProblem, OverriddenValue, SettingLayer,
    code,
};
use serde_json::{Value, json};
use std::time::Duration;

mod common;
use common::config_fixture::Fixture;

fn params(board: Option<&str>, key: Option<&str>, profile: Option<&str>) -> ConfigGetParams {
    ConfigGetParams {
        board: board.map(str::to_string),
        key: key.map(str::to_string),
        profile: profile.map(str::to_string),
        with_problems: false,
    }
}

fn get(fixture: &Fixture, params: &ConfigGetParams) -> ConfigGetResult {
    fixture
        .client()
        .call("config.get", params, Some(Duration::from_secs(20)))
        .unwrap_or_else(|error| panic!("config.get: {error}"))
}

/// The refusal: its code and message.
fn refused(fixture: &Fixture, params: &ConfigGetParams) -> (i64, String) {
    let answer: Result<Value, _> =
        fixture
            .client()
            .call("config.get", params, Some(Duration::from_secs(20)));
    match answer {
        Err(collide_host::owner_client::ClientError::Rpc(error)) => (error.code, error.message),
        other => panic!("config.get should be refused, and gave {other:?}"),
    }
}

fn entry(result: &ConfigGetResult, key: &str) -> ConfigEntry {
    result
        .entries
        .iter()
        .find(|entry| entry.key == key)
        .unwrap_or_else(|| {
            panic!(
                "no entry for {key}; keys: {:?}",
                result.entries.iter().map(|e| &e.key).collect::<Vec<_>>()
            )
        })
        .clone()
}

fn value_and_layer(result: &ConfigGetResult, key: &str) -> (Option<Value>, SettingLayer) {
    let found = entry(result, key);
    (found.value, found.layer)
}

const COLLIDE_KEYS: [&str; 6] = [
    "ask.agent",
    "ask.timeout_s",
    "ask.budget_usd",
    "disk.floor_gib",
    "claude.check_published",
    "ui_tools.port",
];

// ---- no board: the user's layers ----

#[test]
fn with_no_board_the_entries_are_collides_own_keys_with_their_defaults() {
    let fixture = Fixture::new();
    let result = get(&fixture, &params(None, None, None));
    let keys: Vec<&str> = result.entries.iter().map(|e| e.key.as_str()).collect();
    assert_eq!(keys, COLLIDE_KEYS);
    assert_eq!(
        value_and_layer(&result, "disk.floor_gib"),
        (Some(json!(5)), SettingLayer::Default)
    );
    assert_eq!(
        value_and_layer(&result, "claude.check_published"),
        (Some(json!(true)), SettingLayer::Default)
    );
    assert_eq!(
        value_and_layer(&result, "ask.agent"),
        (Some(json!("claude-haiku")), SettingLayer::Default)
    );
}

#[test]
fn with_no_board_a_value_in_the_users_file_says_user() {
    let fixture = Fixture::new();
    fixture.write_user(
        "config.yaml",
        "disk: { floor_gib: 3 }\nask: { budget_usd: 1.5 }\n",
    );
    let result = get(&fixture, &params(None, None, None));
    assert_eq!(
        value_and_layer(&result, "disk.floor_gib"),
        (Some(json!(3)), SettingLayer::User)
    );
    assert_eq!(
        value_and_layer(&result, "ask.budget_usd"),
        (Some(json!(1.5)), SettingLayer::User)
    );
}

#[test]
fn a_key_names_one_entry() {
    let fixture = Fixture::new();
    fixture.write_user("config.yaml", "disk: { floor_gib: 3 }\n");
    let result = get(&fixture, &params(None, Some("disk.floor_gib"), None));
    assert_eq!(result.entries.len(), 1);
    assert_eq!(result.entries[0].key, "disk.floor_gib");
}

#[test]
fn an_unknown_key_is_not_found_and_is_named() {
    let fixture = Fixture::new();
    let (error_code, message) = refused(&fixture, &params(None, Some("disk.nope"), None));
    assert_eq!(error_code, code::NOT_FOUND);
    assert!(message.contains("disk.nope"), "{message}");
}

#[test]
fn with_no_board_a_pack_key_says_that_it_needs_a_board() {
    let fixture = Fixture::new();
    let (error_code, message) = refused(&fixture, &params(None, Some("ferry.lanePool"), None));
    assert_eq!(error_code, code::NOT_FOUND);
    assert!(message.contains("ferry.lanePool"), "{message}");
    assert!(message.contains("board"), "{message}");
}

#[test]
fn a_user_file_that_is_malformed_fails_naming_its_path() {
    let fixture = Fixture::new();
    fixture.write_user("config.yaml", "disk: [unclosed\n");
    let (error_code, message) = refused(&fixture, &params(None, None, None));
    assert_eq!(error_code, code::CONFLICT);
    assert!(message.contains("config.yaml"), "{message}");
}

#[test]
fn a_board_the_daemon_does_not_know_is_not_found() {
    let fixture = Fixture::new();
    let (error_code, message) = refused(&fixture, &params(Some("board-99"), None, None));
    assert_eq!(error_code, code::NOT_FOUND);
    assert!(message.contains("board-99"), "{message}");
}

// ---- a board: the repo's layer and the packs ----

#[test]
fn a_repo_with_no_collide_folder_still_shows_the_defaults_and_the_users_layers() {
    let fixture = Fixture::new();
    fixture.write_user("config.yaml", "disk: { floor_gib: 3 }\n");
    let board = fixture.board();
    let result = get(&fixture, &params(Some(&board), None, None));
    assert_eq!(
        value_and_layer(&result, "disk.floor_gib"),
        (Some(json!(3)), SettingLayer::User)
    );
    assert_eq!(
        value_and_layer(&result, "ask.timeout_s"),
        (None, SettingLayer::Default)
    );
}

#[test]
fn a_key_set_in_both_files_resolves_to_the_repos_value_and_says_repo() {
    let fixture = Fixture::new();
    fixture.write_user("config.yaml", "ask: { budget_usd: 1.0 }\n");
    fixture.write(".collide/config.yaml", "ask: { budget_usd: 2.5 }\n");
    fixture.commit();
    let board = fixture.board();
    let result = get(&fixture, &params(Some(&board), None, None));
    assert_eq!(
        value_and_layer(&result, "ask.budget_usd"),
        (Some(json!(2.5)), SettingLayer::Repo)
    );
}

#[test]
fn a_key_set_only_in_the_users_file_says_user_and_one_set_in_neither_says_default() {
    let fixture = Fixture::new();
    fixture.write_user("config.yaml", "ask: { budget_usd: 1.0 }\n");
    fixture.write(".collide/config.yaml", "disk: { floor_gib: 7 }\n");
    fixture.commit();
    let board = fixture.board();
    let result = get(&fixture, &params(Some(&board), None, None));
    assert_eq!(
        value_and_layer(&result, "ask.budget_usd"),
        (Some(json!(1.0)), SettingLayer::User)
    );
    assert_eq!(
        value_and_layer(&result, "ask.timeout_s"),
        (None, SettingLayer::Default)
    );
}

#[test]
fn an_overridable_key_set_in_both_resolves_to_the_users_value_and_says_user() {
    let fixture = Fixture::new();
    fixture.write_user(
        "config.yaml",
        "disk: { floor_gib: 2 }\nask: { timeout_s: 30 }\n",
    );
    fixture.write(
        ".collide/config.yaml",
        "disk: { floor_gib: 9 }\nask: { timeout_s: 600 }\n",
    );
    fixture.commit();
    let board = fixture.board();
    let result = get(&fixture, &params(Some(&board), None, None));
    assert_eq!(
        value_and_layer(&result, "disk.floor_gib"),
        (Some(json!(2)), SettingLayer::User)
    );
    assert_eq!(
        value_and_layer(&result, "ask.timeout_s"),
        (Some(json!(30)), SettingLayer::User)
    );
}

#[test]
fn the_users_per_board_file_wins_over_the_global_one_and_says_user() {
    let fixture = Fixture::new();
    fixture.write_user("config.yaml", "disk: { floor_gib: 2 }\n");
    let board = fixture.board();
    fixture.write_user(
        &format!("boards/{board}/config.yaml"),
        "disk: { floor_gib: 7 }\n",
    );
    let result = get(&fixture, &params(Some(&board), None, None));
    assert_eq!(
        value_and_layer(&result, "disk.floor_gib"),
        (Some(json!(7)), SettingLayer::User)
    );
}

#[test]
fn another_boards_file_is_not_applied() {
    let fixture = Fixture::new();
    let board = fixture.board();
    fixture.write_user("boards/board-999/config.yaml", "disk: { floor_gib: 7 }\n");
    let result = get(&fixture, &params(Some(&board), None, None));
    assert_eq!(
        value_and_layer(&result, "disk.floor_gib"),
        (Some(json!(5)), SettingLayer::Default)
    );
}

#[test]
fn the_repos_settings_are_those_of_the_committed_head_not_the_working_tree() {
    let fixture = Fixture::new();
    fixture.write(".collide/config.yaml", "ask: { budget_usd: 2.5 }\n");
    fixture.commit();
    fixture.write(".collide/config.yaml", "ask: { budget_usd: 99 }\n");
    let board = fixture.board();
    let result = get(&fixture, &params(Some(&board), None, None));
    assert_eq!(
        value_and_layer(&result, "ask.budget_usd"),
        (Some(json!(2.5)), SettingLayer::Repo)
    );
}

#[test]
fn a_pointer_file_makes_the_repo_layer_the_pointed_folders() {
    let fixture = Fixture::new();
    fixture.write(".collide", "dir: tools/collide\n");
    fixture.write("tools/collide/config.yaml", "disk: { floor_gib: 12 }\n");
    fixture.commit();
    let board = fixture.board();
    let result = get(
        &fixture,
        &params(Some(&board), Some("disk.floor_gib"), None),
    );
    assert_eq!(
        value_and_layer(&result, "disk.floor_gib"),
        (Some(json!(12)), SettingLayer::Repo)
    );
}

#[test]
fn a_packs_keys_show_as_pack_dot_key_and_its_defaults_say_default() {
    let fixture = Fixture::new();
    fixture.ferry();
    fixture.commit();
    let board = fixture.board();
    let result = get(&fixture, &params(Some(&board), None, None));
    let keys: Vec<&str> = result.entries.iter().map(|e| e.key.as_str()).collect();
    assert_eq!(
        &keys[6..],
        [
            "collide/plan.layout",
            "collide/plan.roles",
            "ferry.greeting",
            "ferry.lanePool",
            "ferry.layout",
            "ferry.notifyHook"
        ]
    );
    assert_eq!(
        value_and_layer(&result, "ferry.lanePool"),
        (Some(json!(2)), SettingLayer::Default)
    );
    assert_eq!(
        value_and_layer(&result, "ferry.greeting"),
        (Some(json!("hello")), SettingLayer::Default)
    );
}

#[test]
fn a_pack_key_set_in_the_repo_and_in_the_users_file_follows_the_overridable_rule() {
    let fixture = Fixture::new();
    fixture.ferry();
    fixture.write(
        ".collide/config.yaml",
        "config: { ferry: { lanePool: 1, greeting: team } }\n",
    );
    fixture.commit();
    fixture.write_user(
        "config.yaml",
        "config: { ferry: { lanePool: 4, greeting: mine } }\n",
    );
    let board = fixture.board();
    let result = get(&fixture, &params(Some(&board), None, None));
    assert_eq!(
        value_and_layer(&result, "ferry.lanePool"),
        (Some(json!(4)), SettingLayer::User)
    );
    assert_eq!(
        value_and_layer(&result, "ferry.greeting"),
        (Some(json!("team")), SettingLayer::Repo)
    );
}

#[test]
fn a_secret_entry_has_no_value_says_secret_and_says_user() {
    let fixture = Fixture::new();
    fixture.ferry();
    fixture.commit();
    let board = fixture.board();
    let found = entry(
        &get(&fixture, &params(Some(&board), None, None)),
        "ferry.notifyHook",
    );
    assert_eq!(found.value, None);
    assert!(found.secret);
    assert_eq!(found.layer, SettingLayer::User);
}

// ---- refusals ----

const SECRET: &str = "sk-live-4242-4242";

#[test]
fn a_secret_in_the_users_file_fails_naming_the_key_and_never_the_value() {
    let fixture = Fixture::new();
    fixture.ferry();
    fixture.commit();
    fixture.write_user(
        "config.yaml",
        &format!("config: {{ ferry: {{ notifyHook: \"{SECRET}\" }} }}\n"),
    );
    let board = fixture.board();
    let (error_code, message) = refused(&fixture, &params(Some(&board), None, None));
    assert_eq!(error_code, code::CONFLICT);
    assert!(message.contains("ferry.notifyHook"), "{message}");
    assert!(!message.contains(SECRET), "{message}");
}

#[test]
fn a_secret_in_the_repos_file_fails_naming_the_key_and_never_the_value() {
    let fixture = Fixture::new();
    fixture.ferry();
    fixture.write(
        ".collide/config.yaml",
        &format!("config: {{ ferry: {{ notifyHook: \"{SECRET}\" }} }}\n"),
    );
    fixture.commit();
    let board = fixture.board();
    let (error_code, message) = refused(&fixture, &params(Some(&board), None, None));
    assert_eq!(error_code, code::CONFLICT);
    assert!(message.contains("ferry.notifyHook"), "{message}");
    assert!(!message.contains(SECRET), "{message}");
}

#[test]
fn a_per_user_key_in_the_repos_file_fails_naming_the_key() {
    let fixture = Fixture::new();
    fixture.write(
        ".collide/config.yaml",
        "claude: { check_published: false }\n",
    );
    fixture.commit();
    let board = fixture.board();
    let (error_code, message) = refused(&fixture, &params(Some(&board), None, None));
    assert_eq!(error_code, code::CONFLICT);
    assert!(message.contains("claude.check_published"), "{message}");
}

#[test]
fn a_repo_only_key_in_the_users_file_fails_naming_the_key_and_in_the_repos_file_loads() {
    let fixture = Fixture::new();
    fixture.ferry();
    fixture.write(
        ".collide/config.yaml",
        "config: { ferry: { layout: fancy } }\n",
    );
    fixture.commit();
    let board = fixture.board();
    let result = get(&fixture, &params(Some(&board), None, None));
    assert_eq!(
        value_and_layer(&result, "ferry.layout"),
        (Some(json!("fancy")), SettingLayer::Repo)
    );
    fixture.write_user("config.yaml", "config: { ferry: { layout: mine } }\n");
    let (error_code, message) = refused(&fixture, &params(Some(&board), None, None));
    assert_eq!(error_code, code::CONFLICT);
    assert!(message.contains("ferry.layout"), "{message}");
    assert!(message.contains("repoOnly"), "{message}");
}

#[test]
fn a_value_of_the_wrong_type_fails_naming_the_key_and_the_constraint_not_the_value() {
    let fixture = Fixture::new();
    fixture.ferry();
    fixture.commit();
    fixture.write_user(
        "config.yaml",
        &format!("config: {{ ferry: {{ lanePool: \"{SECRET}\" }} }}\n"),
    );
    let board = fixture.board();
    let (_, message) = refused(&fixture, &params(Some(&board), None, None));
    assert!(message.contains("ferry.lanePool"), "{message}");
    assert!(message.contains("type integer"), "{message}");
    assert!(!message.contains(SECRET), "{message}");
}

#[test]
fn every_problem_is_in_the_message() {
    let fixture = Fixture::new();
    fixture.ferry();
    fixture.commit();
    fixture.write_user(
        "config.yaml",
        "config: { ferry: { lanePool: 0, layout: x } }\n",
    );
    let board = fixture.board();
    let (_, message) = refused(&fixture, &params(Some(&board), None, None));
    assert!(message.contains("ferry.lanePool"), "{message}");
    assert!(message.contains("ferry.layout"), "{message}");
}

#[test]
fn a_pack_setting_with_a_keyword_the_checker_does_not_support_fails_the_load_naming_it() {
    let fixture = Fixture::new();
    fixture.ferry_with("config:\n  lanePool: { oneOf: [{ type: integer }, { type: string }] }\n");
    fixture.commit();
    let board = fixture.board();
    let (error_code, message) = refused(&fixture, &params(Some(&board), None, None));
    assert_eq!(error_code, code::CONFLICT);
    assert!(message.contains("oneOf"), "{message}");
    assert!(message.contains("lanePool"), "{message}");
}

#[test]
fn a_default_that_fails_its_own_schema_fails_the_load_naming_the_key() {
    let fixture = Fixture::new();
    fixture.ferry_with("config:\n  lanePool: { type: integer, minimum: 1, default: 0 }\n");
    fixture.commit();
    let board = fixture.board();
    let (_, message) = refused(&fixture, &params(Some(&board), None, None));
    assert!(message.contains("lanePool"), "{message}");
    assert!(message.contains("default"), "{message}");
}

#[test]
fn a_pack_named_ask_fails_the_load_naming_both() {
    let fixture = Fixture::new();
    fixture.ferry();
    fixture.write(
        ".collide/packs/ferry/pack.yaml",
        "apiVersion: collide/v0\nname: ask\nversion: 1.0.0\n",
    );
    fixture.commit();
    let board = fixture.board();
    let (error_code, message) = refused(&fixture, &params(Some(&board), None, None));
    assert_eq!(error_code, code::CONFLICT);
    assert!(message.contains("`ask`"), "{message}");
    assert!(message.contains("repo:.collide/packs/ferry"), "{message}");
}

// ---- profiles ----

#[test]
fn a_repo_profile_is_the_top_layer_and_says_profile() {
    let fixture = Fixture::new();
    fixture.ferry();
    fixture.write(
        ".collide/profiles/quick.yaml",
        "config: { ferry: { greeting: quick, layout: wide }, ask: { budget_usd: 0.5 } }\n",
    );
    fixture.write(
        ".collide/config.yaml",
        "config: { ferry: { greeting: team } }\n",
    );
    fixture.commit();
    let board = fixture.board();
    let result = get(&fixture, &params(Some(&board), None, Some("quick")));
    assert_eq!(
        value_and_layer(&result, "ferry.greeting"),
        (Some(json!("quick")), SettingLayer::Profile)
    );
    assert_eq!(
        value_and_layer(&result, "ferry.layout"),
        (Some(json!("wide")), SettingLayer::Profile)
    );
    assert_eq!(
        value_and_layer(&result, "ask.budget_usd"),
        (Some(json!(0.5)), SettingLayer::Profile)
    );
    // Without `--profile` the same board gives the layers below.
    let plain = get(&fixture, &params(Some(&board), None, None));
    assert_eq!(
        value_and_layer(&plain, "ferry.greeting"),
        (Some(json!("team")), SettingLayer::Repo)
    );
}

#[test]
fn a_user_profile_that_sets_an_overridable_key_loads_and_says_profile() {
    let fixture = Fixture::new();
    fixture.write_user(
        "profiles/slow-machine.yaml",
        "config: { disk: { floor_gib: 2 }, ask: { timeout_s: 90 } }\n",
    );
    fixture.write(".collide/config.yaml", "disk: { floor_gib: 9 }\n");
    fixture.commit();
    let board = fixture.board();
    let result = get(
        &fixture,
        &params(Some(&board), Some("disk.floor_gib"), Some("slow-machine")),
    );
    assert_eq!(
        value_and_layer(&result, "disk.floor_gib"),
        (Some(json!(2)), SettingLayer::Profile)
    );
}

#[test]
fn a_user_profile_works_with_no_board_too() {
    let fixture = Fixture::new();
    fixture.write_user(
        "profiles/slow-machine.yaml",
        "config: { ask: { timeout_s: 90 } }\n",
    );
    let result = get(
        &fixture,
        &params(None, Some("ask.timeout_s"), Some("slow-machine")),
    );
    assert_eq!(
        value_and_layer(&result, "ask.timeout_s"),
        (Some(json!(90)), SettingLayer::Profile)
    );
}

#[test]
fn a_user_profile_that_sets_a_key_not_marked_overridable_fails_naming_the_key() {
    let fixture = Fixture::new();
    fixture.write_user(
        "profiles/greedy.yaml",
        "config: { ask: { budget_usd: 50 } }\n",
    );
    let (error_code, message) = refused(&fixture, &params(None, None, Some("greedy")));
    assert_eq!(error_code, code::CONFLICT);
    assert!(message.contains("ask.budget_usd"), "{message}");
    assert!(message.contains("overridable"), "{message}");
}

#[test]
fn a_profile_named_in_two_places_fails_naming_both() {
    let fixture = Fixture::new();
    fixture.write_user("profiles/small-fix.yaml", "description: mine\n");
    fixture.write(".collide/profiles/small-fix.yaml", "description: team\n");
    fixture.commit();
    let board = fixture.board();
    let (error_code, message) = refused(&fixture, &params(Some(&board), None, Some("small-fix")));
    assert_eq!(error_code, code::CONFLICT);
    assert!(message.contains("small-fix"), "{message}");
    assert!(message.contains("repo"), "{message}");
    assert!(message.contains("user"), "{message}");
}

#[test]
fn an_unknown_profile_is_not_found_naming_the_name_and_where_it_looked() {
    let fixture = Fixture::new();
    fixture.ferry();
    fixture.commit();
    let board = fixture.board();
    let (error_code, message) = refused(&fixture, &params(Some(&board), None, Some("nope")));
    assert_eq!(error_code, code::NOT_FOUND);
    assert!(message.contains("nope"), "{message}");
    assert!(message.contains(".collide/profiles"), "{message}");
    assert!(message.contains("~/.config/collide/profiles"), "{message}");
    assert!(message.contains("ferry"), "{message}");
}

#[test]
fn a_pack_profile_is_found_by_pack_slash_name() {
    let fixture = Fixture::new();
    fixture.ferry_with(
        "config:\n  greeting: { type: string, default: hello }\n\
         profiles:\n  festive:\n    config: { ferry: { greeting: ahoy } }\n",
    );
    fixture.commit();
    let board = fixture.board();
    let result = get(
        &fixture,
        &params(Some(&board), Some("ferry.greeting"), Some("ferry/festive")),
    );
    assert_eq!(
        value_and_layer(&result, "ferry.greeting"),
        (Some(json!("ahoy")), SettingLayer::Profile)
    );
    // The bare name is not the pack's profile.
    let (error_code, _) = refused(&fixture, &params(Some(&board), None, Some("festive")));
    assert_eq!(error_code, code::NOT_FOUND);
}

#[test]
fn a_pack_profile_that_reaches_past_its_pack_fails_the_load_naming_the_key() {
    let fixture = Fixture::new();
    fixture.ferry_with("profiles:\n  nosy:\n    config: { disk: { floor_gib: 2 } }\n");
    fixture.commit();
    let board = fixture.board();
    let (error_code, message) = refused(&fixture, &params(Some(&board), None, None));
    assert_eq!(error_code, code::CONFLICT);
    assert!(message.contains("nosy"), "{message}");
    assert!(message.contains("config.disk"), "{message}");
}

// ---- what the Settings page reads (ts-0193-gk): the value a layer replaced, problems in place, the files ----

/// The request the Settings page makes: every setting, with the problems of single settings in the result.
fn page_params(board: &str, profile: Option<&str>) -> ConfigGetParams {
    ConfigGetParams {
        with_problems: true,
        ..params(Some(board), None, profile)
    }
}

fn overridden(value: Value, layer: SettingLayer) -> Option<OverriddenValue> {
    Some(OverriddenValue { value, layer })
}

fn problem_of<'a>(result: &'a ConfigGetResult, key: &str) -> &'a ConfigProblem {
    result
        .problems
        .iter()
        .find(|problem| problem.key == key)
        .unwrap_or_else(|| panic!("no problem for {key}: {:?}", result.problems))
}

#[test]
fn a_key_set_in_both_files_says_the_users_value_was_overridden() {
    let fixture = Fixture::new();
    fixture.write_user("config.yaml", "ask: { budget_usd: 1.0 }\n");
    fixture.write(".collide/config.yaml", "ask: { budget_usd: 2.5 }\n");
    fixture.commit();
    let board = fixture.board();
    let result = get(&fixture, &page_params(&board, None));
    let found = entry(&result, "ask.budget_usd");
    assert_eq!(found.value, Some(json!(2.5)));
    assert_eq!(found.layer, SettingLayer::Repo);
    assert_eq!(found.overridden, overridden(json!(1.0), SettingLayer::User));
}

#[test]
fn a_key_the_users_value_wins_says_the_repos_value_was_overridden() {
    let fixture = Fixture::new();
    fixture.write_user("config.yaml", "disk: { floor_gib: 2 }\n");
    fixture.write(".collide/config.yaml", "disk: { floor_gib: 9 }\n");
    fixture.commit();
    let board = fixture.board();
    let found = entry(&get(&fixture, &page_params(&board, None)), "disk.floor_gib");
    assert_eq!(found.layer, SettingLayer::User);
    assert_eq!(found.overridden, overridden(json!(9), SettingLayer::Repo));
}

#[test]
fn a_key_set_in_one_layer_or_none_overrides_nothing() {
    let fixture = Fixture::new();
    fixture.write_user("config.yaml", "ask: { budget_usd: 1.0 }\n");
    fixture.write(".collide/config.yaml", "ask: { timeout_s: 90 }\n");
    fixture.commit();
    let board = fixture.board();
    let result = get(&fixture, &page_params(&board, None));
    for key in ["ask.budget_usd", "ask.timeout_s", "disk.floor_gib"] {
        assert_eq!(entry(&result, key).overridden, None, "{key}");
    }
}

#[test]
fn the_users_board_file_replacing_the_users_file_overrides_nothing() {
    let fixture = Fixture::new();
    fixture.write_user("config.yaml", "ask: { budget_usd: 1.0 }\n");
    let board = fixture.board();
    fixture.write_user(
        &format!("boards/{board}/config.yaml"),
        "ask: { budget_usd: 1.5 }\n",
    );
    let found = entry(&get(&fixture, &page_params(&board, None)), "ask.budget_usd");
    assert_eq!(found.value, Some(json!(1.5)));
    assert_eq!(found.overridden, None);
}

#[test]
fn a_profile_value_says_the_layer_below_it_was_overridden() {
    let fixture = Fixture::new();
    fixture.write_user("config.yaml", "ask: { budget_usd: 1.0 }\n");
    fixture.write(".collide/config.yaml", "ask: { budget_usd: 2.5 }\n");
    fixture.write(
        ".collide/profiles/cheap.yaml",
        "config: { ask: { budget_usd: 0.1 } }\n",
    );
    fixture.commit();
    let board = fixture.board();
    let result = get(&fixture, &page_params(&board, Some("cheap")));
    let found = entry(&result, "ask.budget_usd");
    assert_eq!(found.layer, SettingLayer::Profile);
    assert_eq!(found.overridden, overridden(json!(2.5), SettingLayer::Repo));
}

#[test]
fn a_secret_has_no_overridden_value_and_none_of_its_text_is_in_the_answer() {
    let fixture = Fixture::new();
    fixture.ferry();
    fixture.write(
        ".collide/config.yaml",
        &format!("config: {{ ferry: {{ notifyHook: \"{SECRET}\" }} }}\n"),
    );
    fixture.commit();
    fixture.write_user(
        "config.yaml",
        &format!("config: {{ ferry: {{ notifyHook: \"{SECRET}-user\" }} }}\n"),
    );
    let board = fixture.board();
    let result = get(&fixture, &page_params(&board, None));
    let found = entry(&result, "ferry.notifyHook");
    assert!(found.secret);
    assert_eq!(found.overridden, None);
    let text = serde_json::to_string(&result).unwrap();
    assert!(!text.contains(SECRET), "{text}");
}

#[test]
fn a_bad_value_in_the_users_file_is_a_problem_of_the_user_layer_and_the_entries_still_come() {
    let fixture = Fixture::new();
    fixture.ferry();
    fixture.commit();
    fixture.write_user(
        "config.yaml",
        &format!("config: {{ ferry: {{ lanePool: \"{SECRET}\" }} }}\n"),
    );
    let board = fixture.board();
    let result = get(&fixture, &page_params(&board, None));
    let found = problem_of(&result, "ferry.lanePool");
    assert_eq!(found.layer, SettingLayer::User);
    assert!(found.message.contains("type integer"), "{}", found.message);
    assert!(!serde_json::to_string(&result).unwrap().contains(SECRET));
    // The refused value is not applied: the layer below it holds.
    assert_eq!(
        value_and_layer(&result, "ferry.lanePool"),
        (Some(json!(2)), SettingLayer::Default)
    );
}

#[test]
fn a_bad_value_in_the_repos_file_is_a_problem_of_the_repo_layer() {
    let fixture = Fixture::new();
    fixture.ferry();
    fixture.write(
        ".collide/config.yaml",
        "config: { ferry: { lanePool: 0 } }\n",
    );
    fixture.commit();
    let board = fixture.board();
    let result = get(&fixture, &page_params(&board, None));
    assert_eq!(
        problem_of(&result, "ferry.lanePool").layer,
        SettingLayer::Repo
    );
    assert_eq!(result.problems.len(), 1, "{:?}", result.problems);
}

#[test]
fn a_bad_value_in_a_profile_is_a_problem_of_the_profile_layer() {
    let fixture = Fixture::new();
    fixture.write_user(
        "profiles/greedy.yaml",
        "config: { ask: { budget_usd: 50 } }\n",
    );
    let board = fixture.board();
    let result = get(&fixture, &page_params(&board, Some("greedy")));
    let found = problem_of(&result, "ask.budget_usd");
    assert_eq!(found.layer, SettingLayer::Profile);
    assert!(found.message.contains("overridable"), "{}", found.message);
}

#[test]
fn a_board_with_no_problem_has_an_empty_list_and_without_the_flag_a_problem_still_refuses() {
    let fixture = Fixture::new();
    fixture.ferry();
    fixture.write(
        ".collide/config.yaml",
        "config: { ferry: { lanePool: 0 } }\n",
    );
    fixture.commit();
    let board = fixture.board();
    let (error_code, message) = refused(&fixture, &params(Some(&board), None, None));
    assert_eq!(error_code, code::CONFLICT);
    assert!(message.contains("ferry.lanePool"), "{message}");
    fixture.write(
        ".collide/config.yaml",
        "config: { ferry: { lanePool: 3 } }\n",
    );
    fixture.commit();
    let result = get(&fixture, &page_params(&board, None));
    assert_eq!(result.problems, Vec::<ConfigProblem>::new());
}

#[test]
fn a_file_that_does_not_read_still_refuses_with_the_flag() {
    let fixture = Fixture::new();
    fixture.write_user("config.yaml", "ask: [unclosed\n");
    let board = fixture.board();
    let (error_code, message) = refused(&fixture, &page_params(&board, None));
    assert_eq!(error_code, code::CONFLICT);
    assert!(message.contains("config.yaml"), "{message}");
}

fn same_path(found: &str, expected: &std::path::Path) -> bool {
    let found = std::fs::canonicalize(found).unwrap_or_else(|_| found.into());
    let expected = std::fs::canonicalize(expected).unwrap_or_else(|_| expected.into());
    found == expected
}

#[test]
fn the_answer_lists_the_files_of_the_layers_as_the_daemon_resolves_them() {
    let fixture = Fixture::new();
    fixture.write_user("config.yaml", "ask: { budget_usd: 1.0 }\n");
    fixture.write(".collide/config.yaml", "ask: { budget_usd: 2.5 }\n");
    fixture.commit();
    let board = fixture.board();
    let result = get(&fixture, &page_params(&board, None));
    let home = fixture.home();
    let user_dir = home.join(".config/collide");
    let [user, for_board, repo] = &result.files[..] else {
        panic!("three files expected: {:?}", result.files);
    };
    assert_eq!(
        (user.layer, user.for_board, user.exists),
        (SettingLayer::User, false, true)
    );
    assert!(
        same_path(&user.path, &user_dir.join("config.yaml")),
        "{}",
        user.path
    );
    assert_eq!(
        (for_board.layer, for_board.for_board, for_board.exists),
        (SettingLayer::User, true, false)
    );
    let own = user_dir.join("boards").join(&board).join("config.yaml");
    assert!(same_path(&for_board.path, &own), "{}", for_board.path);
    assert_eq!((repo.layer, repo.exists), (SettingLayer::Repo, true));
    let repo_file = fixture.repo.work().join(".collide/config.yaml");
    assert!(same_path(&repo.path, &repo_file), "{}", repo.path);
}

#[test]
fn with_no_board_the_answer_lists_only_the_users_file() {
    let fixture = Fixture::new();
    let result = get(&fixture, &params(None, None, None));
    let [user] = &result.files[..] else {
        panic!("one file expected: {:?}", result.files);
    };
    assert_eq!((user.layer, user.exists), (SettingLayer::User, false));
}
