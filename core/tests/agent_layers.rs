//! Tests for the agent entries through the layers (ts-0099-bw; 0.7, Agent entries): every field's value and layer, the
//! repo's budget over the user's, the user's timeout over the repo's, an entry the user adds, a change to the built-in
//! entry, `ask.agent` naming a missing entry, and the notice for an agent that cannot enforce a budget. Also each
//! refusal of an entry's file (through `check_config_file` and the repo-file refusals) and the secret-name rule. The
//! layers are small in-memory values; nothing is read from disk.

use collide_core::agent_layers::{AgentInputs, AgentLayers, AgentView, agent_layers};
use collide_core::owner::SettingLayer::{self, Default as Built, Repo, User};
use collide_core::settings::{
    AGENT_KEYS, AgentKind, BUILTIN_AGENT_ID, ConfigFile, check_agents, check_config_file,
    is_secret_name, repo_file_refusals,
};
use collide_core::yaml::load_yaml;
use serde_json::{Value, json};

fn file(text: &str) -> ConfigFile {
    load_yaml(text.as_bytes(), "config.yaml").unwrap_or_else(|error| panic!("{error}\n{text}"))
}

/// The three files the layers read, as YAML text. An empty text is no file.
fn layers(user: &str, board: &str, repo: &str) -> AgentLayers {
    let (user, board, repo) = (file(user), file(board), file(repo));
    agent_layers(&AgentInputs {
        user: &user,
        user_board: &board,
        repo: &repo,
    })
}

fn agent<'a>(found: &'a AgentLayers, id: &str) -> &'a AgentView {
    found
        .agents
        .iter()
        .find(|agent| agent.id == id)
        .unwrap_or_else(|| panic!("no agent {id}"))
}

/// The value of `field` of the entry `id`, as JSON, and its layer.
fn field(found: &AgentLayers, id: &str, field: &str) -> (Option<Value>, SettingLayer) {
    let key = format!("agents.{id}.{field}");
    let entry = agent(found, id)
        .config_entries()
        .into_iter()
        .find(|entry| entry.key == key)
        .unwrap_or_else(|| panic!("no entry {key}"));
    (entry.value, entry.layer)
}

// ---- the built-in entry ----

#[test]
fn with_no_config_the_built_in_entry_is_claude_code_with_haiku_and_every_field_says_default() {
    let found = layers("", "", "");
    assert_eq!(found.agents.len(), 1);
    assert_eq!(
        agent(&found, BUILTIN_AGENT_ID).kind(),
        Some(AgentKind::ClaudeCode)
    );
    for (name, value) in [
        ("kind", Some(json!("claude-code"))),
        ("model", Some(json!("haiku"))),
        ("config", Some(json!("none"))),
        ("read", Some(json!(["<board root>"]))),
        ("tools", Some(json!(["read", "search"]))),
        ("timeout_s", Some(json!(180))),
        ("budget_usd", None),
        ("command", None),
        ("args", None),
        ("env", None),
        ("secrets", None),
    ] {
        assert_eq!(
            field(&found, BUILTIN_AGENT_ID, name),
            (value, Built),
            "{name}"
        );
    }
    assert!(found.problems.is_empty() && found.notices.is_empty());
}

#[test]
fn ask_agent_with_no_config_names_the_built_in_entry() {
    let found = layers("", "", "");
    assert_eq!(
        (found.ask_agent.as_str(), found.ask_agent_layer),
        (BUILTIN_AGENT_ID, Built)
    );
}

#[test]
fn every_entry_lists_every_field_in_the_order_of_the_agent_keys() {
    let found = layers("", "", "");
    let keys: Vec<String> = agent(&found, BUILTIN_AGENT_ID)
        .config_entries()
        .into_iter()
        .map(|entry| entry.key)
        .collect();
    let expected: Vec<String> = AGENT_KEYS
        .iter()
        .map(|key| format!("agents.claude-haiku.{key}"))
        .collect();
    assert_eq!(keys, expected);
}

#[test]
fn a_change_to_the_built_in_entry_is_one_field_and_the_rest_stay_default() {
    let found = layers("agents: { claude-haiku: { model: sonnet } }\n", "", "");
    assert_eq!(
        field(&found, "claude-haiku", "model"),
        (Some(json!("sonnet")), User)
    );
    assert_eq!(
        field(&found, "claude-haiku", "kind"),
        (Some(json!("claude-code")), Built)
    );
    assert_eq!(
        field(&found, "claude-haiku", "timeout_s"),
        (Some(json!(180)), Built)
    );
    assert_eq!(found.agents.len(), 1);
}

#[test]
fn the_boards_own_user_file_wins_over_the_users_file_and_both_say_user() {
    let found = layers(
        "agents: { claude-haiku: { model: sonnet, config: user } }\n",
        "agents: { claude-haiku: { model: opus } }\n",
        "",
    );
    assert_eq!(
        field(&found, "claude-haiku", "model"),
        (Some(json!("opus")), User)
    );
    assert_eq!(
        field(&found, "claude-haiku", "config"),
        (Some(json!("user")), User)
    );
}

// ---- an entry the user adds ----

const OPENCODE: &str = "agents:\n  local-opencode:\n    kind: acp\n    command: opencode\n    args: [acp]\n    \
                        model: \"ollama/qwen2.5:7b\"\n    env: { OPENCODE_DISABLE_CLAUDE_CODE: \"1\" }\n    \
                        secrets: { SOME_API_KEY: keychain-item-name }\n";

#[test]
fn an_entry_the_user_adds_has_the_users_fields_as_user_and_the_rest_as_default() {
    let found = layers(OPENCODE, "", "");
    assert_eq!(found.agents.len(), 2);
    assert_eq!(found.agents[0].id, BUILTIN_AGENT_ID);
    assert_eq!(
        field(&found, "local-opencode", "kind"),
        (Some(json!("acp")), User)
    );
    assert_eq!(
        field(&found, "local-opencode", "command"),
        (Some(json!("opencode")), User)
    );
    assert_eq!(
        field(&found, "local-opencode", "args"),
        (Some(json!(["acp"])), User)
    );
    assert_eq!(
        field(&found, "local-opencode", "env"),
        (Some(json!({ "OPENCODE_DISABLE_CLAUDE_CODE": "1" })), User)
    );
    assert_eq!(
        field(&found, "local-opencode", "secrets"),
        (Some(json!({ "SOME_API_KEY": "keychain-item-name" })), User)
    );
    assert_eq!(field(&found, "local-opencode", "read"), (None, Built));
    assert_eq!(
        field(&found, "local-opencode", "timeout_s"),
        (Some(json!(180)), Built)
    );
    assert!(found.problems.is_empty());
}

#[test]
fn entries_other_than_the_built_in_one_come_after_it_in_the_order_of_their_ids() {
    let found = layers(
        "agents: { zed: { kind: external }, alpha: { kind: external } }\n",
        "",
        "",
    );
    let ids: Vec<&str> = found.agents.iter().map(|agent| agent.id.as_str()).collect();
    assert_eq!(ids, ["claude-haiku", "alpha", "zed"]);
}

// ---- the budget: the repo's wins ----

#[test]
fn the_repos_budget_wins_over_an_entrys_own_and_says_repo() {
    let found = layers(
        "agents: { claude-haiku: { budget_usd: 5.0 } }\n",
        "",
        "ask: { budget_usd: 1.5 }\n",
    );
    assert_eq!(
        field(&found, "claude-haiku", "budget_usd"),
        (Some(json!(1.5)), Repo)
    );
}

#[test]
fn the_repos_budget_wins_over_the_users_ask_budget() {
    let found = layers(
        "ask: { budget_usd: 9.0 }\n",
        "",
        "ask: { budget_usd: 2.0 }\n",
    );
    assert_eq!(
        field(&found, "claude-haiku", "budget_usd"),
        (Some(json!(2.0)), Repo)
    );
}

#[test]
fn an_entrys_own_budget_is_the_users_when_the_repo_sets_none() {
    let found = layers("agents: { claude-haiku: { budget_usd: 5.0 } }\n", "", "");
    assert_eq!(
        field(&found, "claude-haiku", "budget_usd"),
        (Some(json!(5.0)), User)
    );
}

#[test]
fn the_users_ask_budget_is_the_default_of_an_entry_that_has_none() {
    let found = layers("ask: { budget_usd: 3.0 }\n", "", "");
    assert_eq!(
        field(&found, "claude-haiku", "budget_usd"),
        (Some(json!(3.0)), User)
    );
}

#[test]
fn an_entrys_own_budget_wins_over_the_users_ask_budget() {
    let found = layers(
        "ask: { budget_usd: 3.0 }\nagents: { claude-haiku: { budget_usd: 0.5 } }\n",
        "",
        "",
    );
    assert_eq!(
        field(&found, "claude-haiku", "budget_usd"),
        (Some(json!(0.5)), User)
    );
}

// ---- the timeout: the user's wins ----

#[test]
fn an_entrys_own_timeout_wins_over_the_repos_default() {
    let found = layers(
        "agents: { claude-haiku: { timeout_s: 600 } }\n",
        "",
        "ask: { timeout_s: 60 }\n",
    );
    assert_eq!(
        field(&found, "claude-haiku", "timeout_s"),
        (Some(json!(600)), User)
    );
}

#[test]
fn the_users_ask_timeout_wins_over_the_repos() {
    let found = layers("ask: { timeout_s: 400 }\n", "", "ask: { timeout_s: 60 }\n");
    assert_eq!(
        field(&found, "claude-haiku", "timeout_s"),
        (Some(json!(400)), User)
    );
}

#[test]
fn the_repos_timeout_is_the_default_that_beats_the_built_in_entrys_180() {
    let found = layers("", "", "ask: { timeout_s: 60 }\n");
    assert_eq!(
        field(&found, "claude-haiku", "timeout_s"),
        (Some(json!(60)), Repo)
    );
}

#[test]
fn the_repos_timeout_reaches_an_entry_the_user_added() {
    let found = layers(OPENCODE, "", "ask: { timeout_s: 90 }\n");
    assert_eq!(
        field(&found, "local-opencode", "timeout_s"),
        (Some(json!(90)), Repo)
    );
}

// ---- a budget that reaches an agent that cannot enforce one ----

#[test]
fn a_repo_budget_that_reaches_an_acp_entry_is_ignored_with_a_notice_naming_it() {
    let found = layers(OPENCODE, "", "ask: { budget_usd: 2.0 }\n");
    assert_eq!(field(&found, "local-opencode", "budget_usd"), (None, Built));
    assert_eq!(
        field(&found, "claude-haiku", "budget_usd"),
        (Some(json!(2.0)), Repo)
    );
    assert!(found.problems.is_empty(), "{:?}", found.problems);
    assert_eq!(found.notices.len(), 1, "{:?}", found.notices);
    assert!(
        found.notices[0].contains("local-opencode"),
        "{}",
        found.notices[0]
    );
    assert!(found.notices[0].contains("budget"), "{}", found.notices[0]);
}

#[test]
fn an_external_entry_gets_the_same_notice() {
    let found = layers(
        "agents: { my-client: { kind: external } }\n",
        "",
        "ask: { budget_usd: 2.0 }\n",
    );
    assert_eq!(found.notices.len(), 1);
    assert!(found.notices[0].contains("my-client"));
}

#[test]
fn no_budget_in_any_layer_gives_no_notice() {
    assert!(layers(OPENCODE, "", "").notices.is_empty());
}

// ---- ask.agent ----

#[test]
fn ask_agent_naming_a_missing_entry_is_a_problem_naming_the_key_and_the_agent() {
    let found = layers("ask: { agent: nobody }\n", "", "");
    assert_eq!(found.problems.len(), 1, "{:?}", found.problems);
    assert_eq!(found.problems[0].key, "ask.agent");
    assert!(
        found.problems[0].message.contains("ask.agent"),
        "{}",
        found.problems[0].message
    );
    assert!(
        found.problems[0].message.contains("nobody"),
        "{}",
        found.problems[0].message
    );
}

#[test]
fn ask_agent_naming_an_entry_the_user_added_is_the_users_choice() {
    let found = layers(
        &format!("ask: {{ agent: local-opencode }}\n{OPENCODE}"),
        "",
        "",
    );
    assert_eq!(
        (found.ask_agent.as_str(), found.ask_agent_layer),
        ("local-opencode", User)
    );
    assert!(found.problems.is_empty());
}

#[test]
fn the_boards_user_file_picks_the_ask_agent_over_the_users_file() {
    let found = layers(
        &format!("ask: {{ agent: claude-haiku }}\n{OPENCODE}"),
        "ask: { agent: local-opencode }\n",
        "",
    );
    assert_eq!(found.ask_agent, "local-opencode");
}

#[test]
fn an_agent_the_boards_file_adds_can_be_named_by_the_boards_ask_agent() {
    let found = layers(
        "",
        "ask: { agent: mine }\nagents: { mine: { kind: external } }\n",
        "",
    );
    assert!(found.problems.is_empty(), "{:?}", found.problems);
    assert_eq!(found.ask_agent, "mine");
}

// ---- what the repo's file cannot do ----

#[test]
fn a_repo_entry_and_a_repo_ask_agent_are_not_applied() {
    let found = layers(
        "",
        "",
        "ask: { agent: sneaky }\nagents: { claude-haiku: { model: opus }, sneaky: { kind: external } }\n",
    );
    assert_eq!(
        field(&found, "claude-haiku", "model"),
        (Some(json!("haiku")), Built)
    );
    assert_eq!(found.ask_agent, BUILTIN_AGENT_ID);
    assert_eq!(found.agents.len(), 1);
}

// ---- each refusal, one case each ----

fn errors_of(text: &str) -> Vec<String> {
    check_agents(&file(text))
        .iter()
        .map(ToString::to_string)
        .collect()
}

#[test]
fn a_tool_kind_other_than_read_and_search_does_not_parse_and_names_the_key() {
    let error = load_yaml::<ConfigFile>(
        b"agents: { x: { kind: external, tools: [write] } }\n",
        "c.yaml",
    )
    .unwrap_err()
    .to_string();
    assert!(error.contains("tools"), "{error}");
    assert!(error.contains("write"), "{error}");
}

#[test]
fn a_timeout_outside_5_to_1800_fails_naming_the_key() {
    for text in [
        "agents: { x: { kind: external, timeout_s: 4 } }\n",
        "agents: { x: { kind: external, timeout_s: 1801 } }\n",
    ] {
        let errors = errors_of(text);
        assert_eq!(errors.len(), 1, "{errors:?}");
        assert!(errors[0].contains("agents.x.timeout_s"), "{}", errors[0]);
    }
}

#[test]
fn a_budget_on_an_acp_entry_fails_with_only_a_timeout_applies() {
    let errors = errors_of("agents: { x: { kind: acp, command: c, budget_usd: 1.0 } }\n");
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert!(errors[0].contains("agents.x.budget_usd"), "{}", errors[0]);
    assert!(errors[0].contains("timeout only"), "{}", errors[0]);
}

#[test]
fn an_env_line_with_a_secret_name_and_a_literal_value_fails_with_the_hint_to_use_secrets() {
    let errors = errors_of("agents: { x: { kind: external, env: { OPENAI_API_KEY: sk-test } } }\n");
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert!(
        errors[0].contains("agents.x.env.OPENAI_API_KEY"),
        "{}",
        errors[0]
    );
    assert!(errors[0].contains("secrets"), "{}", errors[0]);
    assert!(
        !errors[0].contains("sk-test"),
        "the value must not be repeated: {}",
        errors[0]
    );
}

#[test]
fn an_acp_entry_with_a_refused_argument_fails_naming_the_flag() {
    let errors = errors_of("agents: { x: { kind: acp, command: c, args: [\"--mdns\"] } }\n");
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert!(errors[0].contains("--mdns"), "{}", errors[0]);
}

#[test]
fn check_config_file_reports_the_same_entry_errors_as_check_agents() {
    let text = "agents: { x: { kind: acp, command: c, budget_usd: 1.0 } }\n";
    let from_file: Vec<String> = check_config_file(&file(text))
        .unwrap_err()
        .iter()
        .map(ToString::to_string)
        .collect();
    assert_eq!(from_file, errors_of(text));
}

#[test]
fn a_clean_entry_has_no_errors() {
    assert!(errors_of(OPENCODE).is_empty());
}

fn repo_refusal_keys(text: &str) -> Vec<String> {
    repo_file_refusals(&file(text))
        .into_iter()
        .map(|refusal| refusal.key)
        .collect()
}

#[test]
fn a_repo_file_that_sets_an_entry_field_fails_naming_each_key() {
    for (field, text) in [
        ("command", "agents: { x: { command: c } }\n"),
        ("args", "agents: { x: { args: [a] } }\n"),
        ("env", "agents: { x: { env: { A: b } } }\n"),
        ("secrets", "agents: { x: { secrets: { A: item } } }\n"),
        ("read", "agents: { x: { read: [\"/tmp\"] } }\n"),
        ("tools", "agents: { x: { tools: [read] } }\n"),
        ("model", "agents: { x: { model: m } }\n"),
        ("config", "agents: { x: { config: none } }\n"),
    ] {
        assert_eq!(
            repo_refusal_keys(text),
            [format!("agents.x.{field}")],
            "{field}"
        );
    }
}

#[test]
fn a_repo_file_that_sets_ask_agent_fails_naming_it() {
    assert_eq!(repo_refusal_keys("ask: { agent: x }\n"), ["ask.agent"]);
}

#[test]
fn a_repo_files_budget_and_timeout_are_allowed() {
    assert!(repo_refusal_keys("ask: { budget_usd: 1.0, timeout_s: 60 }\n").is_empty());
}

// ---- the secret-name rule ----

#[test]
fn names_that_should_match_as_secrets_do_in_either_case() {
    for name in [
        "API_KEY",
        "GITHUB_TOKEN",
        "DB_PASSWORD",
        "CLIENT_SECRET",
        "OPENAI_API_KEY",
    ] {
        assert!(is_secret_name(name), "{name}");
        assert!(
            is_secret_name(&name.to_lowercase()),
            "{}",
            name.to_lowercase()
        );
    }
}

#[test]
fn names_that_should_not_match_as_secrets_do_not() {
    for name in [
        "KEYBOARD",
        "TOKENIZER",
        "PASSWORD_FILE",
        "SECRETARY",
        "PATH",
        "HOME",
        "",
    ] {
        assert!(!is_secret_name(name), "{name}");
    }
}

#[test]
fn the_rule_is_a_suffix_match_as_built_so_monkey_matches() {
    assert!(is_secret_name("MONKEY"));
}
