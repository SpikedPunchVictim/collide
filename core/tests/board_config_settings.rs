//! Tests for the board, config and settings contracts (ts-0025): validation as pure functions, the generated
//! schemas, closed structs in every file, the repo-file rule, and the fixture files.

use collide_core::board::{
    Board, BoardError, BoardKey, CollidePointer, LINE_KEYS, PointerError, check_board,
    check_pointer, line_ignored_keys,
};
use collide_core::item::Category;
use collide_core::lock::SourceError;
use collide_core::profile::RoleConfig;
use collide_core::schema;
use collide_core::settings::{
    AgentKind, BUILTIN_AGENT_ID, ConfigFile, SettingsError, ToolKind, builtin_agents,
    check_config_file, refused_acp_args, repo_file_refusals, secrets_in_file,
};
use collide_core::settings_registry::{overridable_keys, repo_file_keys};
use collide_core::view::MAX_CEL_SOURCE_BYTES;
use collide_core::yaml::{load_yaml, load_yaml_file};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

fn load<T: DeserializeOwned>(text: &str) -> T {
    load_yaml(text.as_bytes(), "test.yaml").unwrap_or_else(|error| panic!("{error}\n{text}"))
}

fn parse_error<T: DeserializeOwned>(text: &str) -> String {
    load_yaml::<T>(text.as_bytes(), "test.yaml")
        .err()
        .unwrap_or_else(|| panic!("should not parse:\n{text}"))
        .to_string()
}

fn fixture(path: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../fixtures")
        .join(path)
}

fn load_fixture<T: DeserializeOwned>(path: &str) -> T {
    load_yaml_file(&fixture(path)).unwrap_or_else(|error| panic!("{path}: {error}"))
}

fn fixture_error<T: DeserializeOwned>(path: &str) -> String {
    load_yaml_file::<T>(&fixture(path))
        .err()
        .unwrap_or_else(|| panic!("{path} should not parse"))
        .to_string()
}

fn schema_file(name: &str) -> Value {
    serde_json::from_str(&schema::registry().render()[name]).unwrap()
}

fn config_errors(path: &str) -> Vec<SettingsError> {
    check_config_file(&load_fixture::<ConfigFile>(path)).unwrap_err()
}

// board.yaml.

#[test]
fn a_board_with_every_key_loads_and_validates() {
    let board: Board = load_fixture("board/board.yaml");
    assert_eq!(check_board(&board), Ok(()));
    assert_eq!(board.packs.len(), 2);
    assert_eq!(board.workflow.as_deref(), Some("ferry/loop"));
    assert_eq!(board.profiles.default.as_deref(), Some("small-fix"));
    assert_eq!(board.profiles.rules[0].profile, "small-fix");
    let github = board.sources.github.as_ref().unwrap();
    assert_eq!(github.repos, ["org/ferry"]);
    assert_eq!(
        (github.projects[0].owner.as_str(), github.projects[0].number),
        ("org", 3)
    );
    assert_eq!(board.checkout_filter.exclude, ["scratch/*"]);
}

#[test]
fn a_board_keeps_a_status_the_plan_writes_with_an_apostrophe() {
    let board: Board = load_fixture("board/board.yaml");
    let mapping = board.status_mapping.github.unwrap();
    assert_eq!(mapping.status_field.as_deref(), Some("Status"));
    assert_eq!(mapping.categories[&Category::Dropped], ["Won't do"]);
    assert_eq!(mapping.categories[&Category::Done], ["Done", "Shipped"]);
}

#[test]
fn the_board_that_init_writes_validates_and_an_empty_board_is_valid() {
    let board: Board = load_fixture("board/minimal.yaml");
    assert_eq!(check_board(&board), Ok(()));
    let empty: Board = load("{}\n");
    assert_eq!(check_board(&empty), Ok(()));
    assert_eq!(empty, Board::default());
    assert_eq!(empty.keys_set(), []);
}

#[test]
fn an_unknown_key_in_a_board_fails_naming_it() {
    let error = fixture_error::<Board>("board/invalid/unknown-key.yaml");
    assert!(error.contains("checkoutFilter"), "{error}");
    let error = parse_error::<Board>("profiles: { rule: [] }\n");
    assert!(error.contains("rule"), "{error}");
    let error = parse_error::<Board>("sources: { github: { projectz: [] } }\n");
    assert!(error.contains("projectz"), "{error}");
}

#[test]
fn a_status_category_the_plan_does_not_name_fails() {
    let error = fixture_error::<Board>("board/invalid/unknown-status-category.yaml");
    assert!(error.contains("finished"), "{error}");
}

#[test]
fn a_rule_without_a_profile_fails() {
    let error = fixture_error::<Board>("board/invalid/rule-without-profile.yaml");
    assert!(error.contains("profile"), "{error}");
}

#[test]
fn a_board_refuses_a_dev_source() {
    let board: Board = load_fixture("board/invalid/dev-source.yaml");
    assert_eq!(
        check_board(&board),
        Err(vec![BoardError::DevSource { index: 0 }])
    );
    let message = check_board(&board).unwrap_err()[0].to_string();
    assert!(message.contains("dev:"), "{message}");
}

#[test]
fn a_board_takes_repo_git_and_unknown_schemes_through_the_pack_source_parser() {
    let board: Board = load(
        "packs:\n  - source: 'repo:.collide/packs/a'\n  - source: 'git:h/p//pack@v1'\n  - source: 'npm:@o/p@1'\n",
    );
    assert_eq!(check_board(&board), Ok(()));
    let board: Board = load("packs:\n  - source: 'github.com/org/p'\n  - source: 'git:--evil'\n");
    let errors = check_board(&board).unwrap_err();
    assert!(matches!(
        errors.as_slice(),
        [
            BoardError::BadSource {
                index: 0,
                error: SourceError::NoScheme { .. }
            },
            BoardError::BadSource {
                index: 1,
                error: SourceError::FlagLikeLocator { .. }
            },
        ]
    ));
}

#[test]
fn a_board_refuses_one_source_listed_twice() {
    let board: Board =
        load("packs:\n  - source: 'repo:a'\n  - source: 'repo:b'\n  - source: 'repo:a'\n");
    assert_eq!(
        check_board(&board),
        Err(vec![BoardError::DuplicateSource {
            source: "repo:a".into(),
            first: 0,
            second: 2
        }])
    );
}

#[test]
fn the_board_source_schema_pattern_is_the_lock_one() {
    let schema = schema_file("Board.input.json");
    let source = &schema["$defs"]["BoardPack"]["properties"]["source"];
    assert_eq!(source["pattern"], json!("^[a-z][a-z0-9+.-]*:.+$"));
    assert!(source.get("enum").is_none(), "{source}");
}

#[test]
fn a_rule_when_over_one_kilobyte_is_refused_and_one_at_the_cap_is_not() {
    let board: Board = load_fixture("board/invalid/rule-when-over-1kb.yaml");
    let errors = check_board(&board).unwrap_err();
    assert!(matches!(
        errors.as_slice(),
        [BoardError::WhenTooLong { index: 0, bytes }] if *bytes > MAX_CEL_SOURCE_BYTES
    ));
    let at_cap = "a".repeat(MAX_CEL_SOURCE_BYTES);
    let board: Board = load(&format!(
        "profiles:\n  rules:\n    - when: {at_cap}\n      profile: p\n"
    ));
    assert_eq!(check_board(&board), Ok(()));
    let board: Board = load("profiles:\n  rules:\n    - { when: ' ', profile: p }\n");
    assert_eq!(
        check_board(&board),
        Err(vec![BoardError::EmptyWhen { index: 0 }])
    );
}

#[test]
fn a_default_workflow_is_written_pack_slash_workflow() {
    let board: Board = load("workflow: review\n");
    assert_eq!(
        check_board(&board),
        Err(vec![BoardError::BadWorkflowName {
            name: "review".into()
        }])
    );
}

#[test]
fn github_names_and_project_numbers_are_checked() {
    let board: Board = load(
        "sources:\n  github:\n    repos: [ferry, 'org/a/b', 'org/ok']\n    projects:\n      - { owner: org, number: 0 }\n      - { owner: '', number: 1 }\n",
    );
    assert_eq!(
        check_board(&board),
        Err(vec![
            BoardError::BadRepo {
                name: "ferry".into()
            },
            BoardError::BadRepo {
                name: "org/a/b".into()
            },
            BoardError::BadProject { index: 0 },
            BoardError::BadProject { index: 1 },
        ])
    );
}

#[test]
fn a_project_number_has_a_floor_of_one_in_the_schema() {
    let schema = schema_file("Board.input.json");
    assert_eq!(
        schema["$defs"]["GithubProject"]["properties"]["number"]["minimum"],
        json!(1)
    );
}

#[test]
fn one_projects_status_under_two_categories_is_refused() {
    let board: Board = load_fixture("board/invalid/status-in-two-categories.yaml");
    assert_eq!(
        check_board(&board),
        Err(vec![BoardError::StatusInTwoCategories {
            status: "Doing".into(),
            first: Category::Active,
            second: Category::Review,
        }])
    );
}

#[test]
fn a_category_with_no_status_and_an_empty_glob_are_refused() {
    let board: Board = load(
        "status_mapping: { github: { status_field: ' ', categories: { done: [], ready: [''] } } }\n\
         checkout_filter: { exclude: ['scratch/*', ''] }\n",
    );
    assert_eq!(
        check_board(&board),
        Err(vec![
            BoardError::EmptyStatusField,
            BoardError::EmptyStatus {
                category: Category::Ready
            },
            BoardError::EmptyStatus {
                category: Category::Done
            },
            BoardError::EmptyGlob { index: 1 },
        ])
    );
}

#[test]
fn a_line_overrides_the_packs_the_default_workflow_and_the_profiles_only() {
    assert_eq!(
        LINE_KEYS,
        [BoardKey::Packs, BoardKey::Workflow, BoardKey::Profiles]
    );
    let board: Board = load_fixture("board/board.yaml");
    assert_eq!(
        board.keys_set(),
        [
            BoardKey::Packs,
            BoardKey::Workflow,
            BoardKey::Profiles,
            BoardKey::Sources,
            BoardKey::StatusMapping,
            BoardKey::CheckoutFilter
        ]
    );
    assert_eq!(
        line_ignored_keys(&board),
        [
            BoardKey::Sources,
            BoardKey::StatusMapping,
            BoardKey::CheckoutFilter
        ]
    );
    let line: Board =
        load("packs:\n  - source: 'repo:a'\nworkflow: p/w\nprofiles: { default: x }\n");
    assert_eq!(line_ignored_keys(&line), []);
}

#[test]
fn board_keys_are_named_as_the_file_writes_them() {
    let board: Board = load_fixture("board/board.yaml");
    let value = serde_json::to_value(&board).unwrap();
    let written: Vec<&str> = value
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    for key in board.keys_set() {
        assert!(written.contains(&key.as_str()), "{key}");
    }
}

#[test]
fn a_board_round_trips_through_its_schema_shape() {
    let board: Board = load_fixture("board/board.yaml");
    let value = serde_json::to_value(&board).unwrap();
    let again: Board = serde_json::from_value(value).unwrap();
    assert_eq!(again, board);
}

// The `.collide` pointer file.

#[test]
fn a_pointer_file_names_a_directory_relative_to_the_repo_root() {
    let pointer: CollidePointer = load_fixture("pointer/dot-collide.yaml");
    assert_eq!(pointer.dir, "tools/collide");
    assert_eq!(check_pointer(&pointer), Ok(()));
}

#[test]
fn a_pointer_with_an_unknown_key_or_no_dir_fails() {
    let error = fixture_error::<CollidePointer>("pointer/invalid/unknown-key.yaml");
    assert!(error.contains("ref"), "{error}");
    let error = fixture_error::<CollidePointer>("pointer/invalid/missing-dir.yaml");
    assert!(error.contains("dir"), "{error}");
}

#[test]
fn a_pointer_dir_that_is_absolute_empty_or_leaves_the_repo_is_refused() {
    let absolute: CollidePointer = load_fixture("pointer/invalid/absolute-dir.yaml");
    assert_eq!(
        check_pointer(&absolute),
        Err(PointerError::NotRelative {
            dir: "/etc/collide".into()
        })
    );
    let leaves: CollidePointer = load_fixture("pointer/invalid/leaves-repo.yaml");
    assert!(matches!(
        check_pointer(&leaves),
        Err(PointerError::LeavesRepo { .. })
    ));
    let empty: CollidePointer = load("dir: ' '\n");
    assert_eq!(check_pointer(&empty), Err(PointerError::EmptyDir));
    let inside: CollidePointer = load("dir: a/b..c/d\n");
    assert_eq!(check_pointer(&inside), Ok(()));
}

// config.yaml: agents and ask.

#[test]
fn the_users_config_with_three_agents_and_ask_loads_and_validates() {
    let file: ConfigFile = load_fixture("config/config.yaml");
    assert_eq!(check_config_file(&file), Ok(()));
    assert_eq!(file.agents.len(), 3);
    assert_eq!(file.ask.agent.as_deref(), Some("claude-haiku"));
    let opencode = &file.agents["local-opencode"];
    assert_eq!(opencode.kind, Some(AgentKind::Acp));
    assert_eq!(opencode.command.as_deref(), Some("opencode"));
    assert_eq!(opencode.args.as_deref(), Some(&["acp".to_string()][..]));
    assert_eq!(opencode.model.as_deref(), Some("ollama/qwen2.5:7b"));
    assert_eq!(
        opencode.env.as_ref().unwrap()["OPENCODE_DISABLE_CLAUDE_CODE"],
        "1"
    );
    assert_eq!(
        opencode.secrets.as_ref().unwrap()["SOME_API_KEY"],
        "keychain-item-name"
    );
}

#[test]
fn the_repos_config_with_ask_defaults_and_pack_settings_validates() {
    let file: ConfigFile = load_fixture("config/repo-config.yaml");
    assert_eq!(check_config_file(&file), Ok(()));
    assert_eq!(file.ask.budget_usd, Some(0.5));
    assert_eq!(file.ask.timeout_s, Some(240));
    assert_eq!(file.config["ferry"]["lanePool"], json!(2));
    assert_eq!(repo_file_refusals(&file), []);
}

#[test]
fn an_empty_config_is_valid() {
    let file: ConfigFile = load("{}\n");
    assert_eq!(check_config_file(&file), Ok(()));
    assert_eq!(file, ConfigFile::default());
}

#[test]
fn both_config_files_round_trip_through_json() {
    for path in ["config/config.yaml", "config/repo-config.yaml"] {
        let file: ConfigFile = load_fixture(path);
        let value = serde_json::to_value(&file).unwrap();
        let again: ConfigFile = serde_json::from_value(value).unwrap();
        assert_eq!(again, file, "{path}");
    }
}

#[test]
fn an_unknown_key_in_a_config_file_fails_naming_it() {
    let error = fixture_error::<ConfigFile>("config/invalid/unknown-key.yaml");
    assert!(error.contains("metrics"), "{error}");
    let error = parse_error::<ConfigFile>("ask: { agnt: x }\n");
    assert!(error.contains("agnt"), "{error}");
}

#[test]
fn an_agents_entry_with_an_unknown_key_fails_naming_it() {
    let error = fixture_error::<ConfigFile>("config/invalid/agent-unknown-key.yaml");
    assert!(error.contains("working_dir"), "{error}");
}

#[test]
fn a_tool_kind_other_than_read_or_search_fails_naming_the_key() {
    let error = fixture_error::<ConfigFile>("config/invalid/tool-kind-write.yaml");
    assert!(error.contains("write"), "{error}");
    assert!(error.contains("tools"), "{error}");
    for kind in ["execute", "edit", "fetch", "delete"] {
        let error = parse_error::<ConfigFile>(&format!(
            "agents:\n  a: {{ kind: external, tools: [{kind}] }}\n"
        ));
        assert!(error.contains(kind), "{error}");
    }
}

#[test]
fn read_and_search_are_the_only_tool_kinds_in_the_schema() {
    let schema = schema_file("ConfigFile.input.json");
    assert_eq!(
        schema["$defs"]["ToolKind"]["enum"],
        json!(["read", "search"])
    );
    assert_eq!(
        schema["$defs"]["AgentKind"]["enum"],
        json!(["claude-code", "acp", "external"])
    );
}

#[test]
fn a_timeout_outside_five_to_eighteen_hundred_fails() {
    assert_eq!(
        config_errors("config/invalid/timeout-too-small.yaml"),
        [SettingsError::TimeoutOutOfRange {
            key: "agents.claude-haiku.timeout_s".into(),
            value: 4
        }]
    );
    assert_eq!(
        config_errors("config/invalid/timeout-too-large.yaml"),
        [SettingsError::TimeoutOutOfRange {
            key: "agents.claude-haiku.timeout_s".into(),
            value: 1801
        }]
    );
    let message = config_errors("config/invalid/timeout-too-small.yaml")[0].to_string();
    assert!(message.contains("5 to 1800"), "{message}");
}

#[test]
fn the_timeout_edges_five_and_eighteen_hundred_are_valid() {
    for value in [5, 1800] {
        let file: ConfigFile = load(&format!(
            "agents:\n  claude-haiku: {{ timeout_s: {value} }}\n"
        ));
        assert_eq!(check_config_file(&file), Ok(()), "{value}");
    }
}

#[test]
fn the_ask_timeout_has_the_same_range() {
    let file: ConfigFile = load("ask: { timeout_s: 1801 }\n");
    assert_eq!(
        check_config_file(&file),
        Err(vec![SettingsError::TimeoutOutOfRange {
            key: "ask.timeout_s".into(),
            value: 1801
        }])
    );
}

#[test]
fn the_timeout_range_is_in_the_generated_schema() {
    let schema = schema_file("ConfigFile.input.json");
    for (def, key) in [("AgentEntry", "timeout_s"), ("Ask", "timeout_s")] {
        let property = &schema["$defs"][def]["properties"][key];
        assert_eq!(property["minimum"], json!(5), "{def}");
        assert_eq!(property["maximum"], json!(1800), "{def}");
    }
}

#[test]
fn an_external_agent_needs_no_command_and_an_acp_agent_needs_one() {
    let external: ConfigFile = load("agents:\n  my-client: { kind: external }\n");
    assert_eq!(check_config_file(&external), Ok(()));
    assert_eq!(
        config_errors("config/invalid/acp-without-command.yaml"),
        [SettingsError::AcpWithoutCommand {
            id: "local-opencode".into()
        }]
    );
    let blank: ConfigFile = load("agents:\n  a: { kind: acp, command: ' ' }\n");
    assert_eq!(
        check_config_file(&blank),
        Err(vec![SettingsError::AcpWithoutCommand { id: "a".into() }])
    );
}

#[test]
fn a_claude_code_entry_needs_no_command() {
    let file: ConfigFile = load("agents:\n  c: { kind: claude-code, model: sonnet }\n");
    assert_eq!(check_config_file(&file), Ok(()));
}

#[test]
fn an_entry_other_than_the_built_in_one_names_its_kind() {
    let file: ConfigFile = load("agents:\n  mine: { model: x }\n  claude-haiku: { model: opus }\n");
    assert_eq!(
        check_config_file(&file),
        Err(vec![SettingsError::MissingKind { id: "mine".into() }])
    );
}

#[test]
fn a_budget_is_for_claude_code_only() {
    assert_eq!(
        config_errors("config/invalid/budget-on-acp.yaml"),
        [SettingsError::BudgetNotEnforceable {
            id: "local-opencode".into(),
            kind: AgentKind::Acp
        }]
    );
    let external: ConfigFile = load("agents:\n  a: { kind: external, budget_usd: 1 }\n");
    assert!(check_config_file(&external).is_err());
    let claude: ConfigFile = load(
        "agents:\n  claude-haiku: { budget_usd: 2.5 }\n  c: { kind: claude-code, budget_usd: 1 }\n",
    );
    assert_eq!(check_config_file(&claude), Ok(()));
}

#[test]
fn an_env_line_with_a_secret_looking_name_and_a_literal_value_is_refused() {
    assert_eq!(
        config_errors("config/invalid/env-literal-secret.yaml"),
        [SettingsError::LiteralSecretInEnv {
            id: "local-opencode".into(),
            name: "SOME_API_KEY".into()
        }]
    );
    let message = config_errors("config/invalid/env-literal-secret.yaml")[0].to_string();
    assert!(message.contains("secrets"), "{message}");
    for name in ["GH_TOKEN", "CLIENT_SECRET", "DB_PASSWORD", "api_key"] {
        let file: ConfigFile = load(&format!(
            "agents:\n  a: {{ kind: external, env: {{ {name}: v }} }}\n"
        ));
        assert!(check_config_file(&file).is_err(), "{name}");
    }
    let fine: ConfigFile = load(
        "agents:\n  a: { kind: external, env: { MODE: fast, EMPTY_TOKEN: '', KEYBOARD: us } }\n",
    );
    assert_eq!(check_config_file(&fine), Ok(()));
}

#[test]
fn an_acp_listener_flag_is_refused_by_name() {
    let errors = config_errors("config/invalid/acp-listens-on-network.yaml");
    assert_eq!(errors.len(), 3, "{errors:?}");
    let text: Vec<String> = errors.iter().map(ToString::to_string).collect();
    assert!(text[0].contains("--hostname 0.0.0.0"), "{}", text[0]);
    assert!(text[1].contains("--mdns"), "{}", text[1]);
    assert!(text[2].contains("--cors"), "{}", text[2]);
}

#[test]
fn a_loopback_hostname_is_allowed_in_both_forms() {
    let args = |list: &[&str]| list.iter().map(ToString::to_string).collect::<Vec<_>>();
    for host in ["127.0.0.1", "localhost", "::1", "[::1]", "127.0.0.2"] {
        assert_eq!(
            refused_acp_args(&args(&["acp", "--hostname", host])),
            [],
            "{host}"
        );
        assert_eq!(
            refused_acp_args(&args(&[&format!("--hostname={host}")])),
            [],
            "{host}"
        );
    }
    for host in ["0.0.0.0", "192.168.1.5", "example.com", "::"] {
        assert_eq!(
            refused_acp_args(&args(&["--hostname", host])).len(),
            1,
            "{host}"
        );
        assert_eq!(
            refused_acp_args(&args(&[&format!("--hostname={host}")])).len(),
            1,
            "{host}"
        );
    }
    assert_eq!(refused_acp_args(&args(&["--hostname"])).len(), 1);
    assert_eq!(refused_acp_args(&args(&["--cors", "*"])).len(), 1);
    assert_eq!(refused_acp_args(&args(&["--mdns=true"])).len(), 1);
    assert_eq!(refused_acp_args(&args(&["--port", "0", "acp"])), []);
}

#[test]
fn every_problem_in_a_config_file_is_refused_in_one_pass() {
    let file: ConfigFile = load(
        "agents:\n  b: { model: x, timeout_s: 1 }\n  a: { kind: acp }\nask: { agent: ' ', timeout_s: 9999 }\nconfig: { '': { k: 1 } }\n",
    );
    assert_eq!(
        check_config_file(&file),
        Err(vec![
            SettingsError::AcpWithoutCommand { id: "a".into() },
            SettingsError::MissingKind { id: "b".into() },
            SettingsError::TimeoutOutOfRange {
                key: "agents.b.timeout_s".into(),
                value: 1
            },
            SettingsError::EmptyAskAgent,
            SettingsError::TimeoutOutOfRange {
                key: "ask.timeout_s".into(),
                value: 9999
            },
            SettingsError::EmptyConfigPack,
        ])
    );
}

// The built-in entry.

#[test]
fn the_built_in_claude_haiku_entry_validates_exactly_as_the_plan_writes_it() {
    let file: ConfigFile = load_fixture("config/builtin-claude-haiku.yaml");
    assert_eq!(check_config_file(&file), Ok(()));
    assert_eq!(file.agents, builtin_agents());
    let entry = &file.agents[BUILTIN_AGENT_ID];
    assert_eq!(entry.kind, Some(AgentKind::ClaudeCode));
    assert_eq!(entry.model.as_deref(), Some("haiku"));
    assert_eq!(entry.config, Some(RoleConfig::None));
    assert_eq!(
        entry.read.as_deref(),
        Some(&["<board root>".to_string()][..])
    );
    assert_eq!(
        entry.tools.as_deref(),
        Some(&[ToolKind::Read, ToolKind::Search][..])
    );
    assert_eq!(entry.timeout_s, Some(180));
    assert_eq!(entry.budget_usd, None);
}

#[test]
fn the_built_in_entry_set_is_one_valid_entry() {
    let built_in = ConfigFile {
        agents: builtin_agents(),
        ..ConfigFile::default()
    };
    assert_eq!(builtin_agents().len(), 1);
    assert_eq!(check_config_file(&built_in), Ok(()));
}

#[test]
fn a_user_may_change_the_built_in_entry_field_by_field() {
    let file: ConfigFile = load("agents:\n  claude-haiku: { model: sonnet, config: user }\n");
    assert_eq!(check_config_file(&file), Ok(()));
    assert_eq!(file.agents[BUILTIN_AGENT_ID].config, Some(RoleConfig::User));
}

// The repo-file rule.

#[test]
fn the_keys_a_repo_file_may_set_are_data_read_from_the_registry() {
    assert_eq!(
        repo_file_keys(),
        ["ask.timeout_s", "ask.budget_usd", "disk.floor_gib"]
    );
    assert_eq!(overridable_keys(), ["ask.timeout_s", "disk.floor_gib"]);
}

#[test]
fn a_repo_file_that_sets_an_agent_field_or_ask_agent_is_refused_naming_each_key() {
    let file: ConfigFile = load_fixture("config/invalid/repo-sets-agent.yaml");
    assert_eq!(
        check_config_file(&file),
        Ok(()),
        "the file itself is well formed"
    );
    let keys: Vec<String> = repo_file_refusals(&file)
        .into_iter()
        .map(|r| r.key)
        .collect();
    assert_eq!(
        keys,
        [
            "agents.claude-haiku.model",
            "agents.claude-haiku.timeout_s",
            "agents.evil.kind",
            "agents.evil.command",
            "ask.agent"
        ]
    );
    let first = &repo_file_refusals(&file)[0];
    assert!(
        first.to_string().contains("agents.claude-haiku.model"),
        "{first}"
    );
    assert!(first.to_string().contains("ask.budget_usd"), "{first}");
}

#[test]
fn a_repo_file_that_sets_a_per_user_key_is_refused_naming_it_and_a_team_key_is_allowed() {
    let file: ConfigFile = load_fixture("config/invalid/repo-sets-check-published.yaml");
    assert_eq!(check_config_file(&file), Ok(()));
    let refusals = repo_file_refusals(&file);
    let keys: Vec<&str> = refusals.iter().map(|r| r.key.as_str()).collect();
    assert_eq!(keys, ["claude.check_published"]);
    let message = refusals[0].to_string();
    assert!(message.contains("claude.check_published"), "{message}");
    assert!(
        message.contains("disk.floor_gib"),
        "the allowed keys are named: {message}"
    );
}

#[test]
fn a_repo_file_may_set_disk_floor_gib() {
    let file: ConfigFile = load("disk: { floor_gib: 10 }\n");
    assert_eq!(repo_file_refusals(&file), []);
}

#[test]
fn a_repo_file_may_set_ask_budget_and_ask_timeout() {
    let file: ConfigFile = load("ask: { budget_usd: 1, timeout_s: 60 }\n");
    assert_eq!(repo_file_refusals(&file), []);
}

#[test]
fn an_empty_agents_entry_in_a_repo_file_is_refused_by_its_id() {
    let file: ConfigFile = load("agents:\n  mine: {}\n");
    let keys: Vec<String> = repo_file_refusals(&file)
        .into_iter()
        .map(|r| r.key)
        .collect();
    assert_eq!(keys, ["agents.mine"]);
}

#[test]
fn a_repo_budget_on_an_agent_that_cannot_enforce_one_is_not_a_load_error() {
    // 0.7: ignored with a notice on the Agents page. The repo's file cannot know which agent a developer picked.
    let repo: ConfigFile = load("ask: { budget_usd: 1 }\n");
    assert_eq!(check_config_file(&repo), Ok(()));
    assert_eq!(repo_file_refusals(&repo), []);
}

#[test]
fn every_agent_key_is_listed_once_and_named_as_the_file_writes_it() {
    let file: ConfigFile = load(
        "agents:\n  a: { kind: acp, command: c, args: [x], model: m, config: user, read: [r], tools: [read], \
         timeout_s: 9, budget_usd: 1, env: { A: b }, secrets: { C: d } }\n",
    );
    let entry = &file.agents["a"];
    let set = entry.keys_set();
    assert_eq!(set.len(), 11);
    let value = serde_json::to_value(entry).unwrap();
    let written: BTreeSet<&str> = value
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(written, set.iter().copied().collect::<BTreeSet<_>>());
}

// Secrets.

#[test]
fn a_secret_pack_key_in_a_settings_file_is_found_in_either_file() {
    let secrets: BTreeSet<(String, String)> =
        BTreeSet::from([("ferry".to_string(), "notifyHook".to_string())]);
    for text in [
        "config:\n  ferry: { lanePool: 2, notifyHook: https://hook }\n",
        "config:\n  ferry: { notifyHook: x }\n",
    ] {
        let file: ConfigFile = load(text);
        let found = secrets_in_file(&file, &secrets);
        assert_eq!(found.len(), 1);
        assert_eq!(
            (found[0].pack.as_str(), found[0].key.as_str()),
            ("ferry", "notifyHook")
        );
        assert!(found[0].to_string().contains("--secret"), "{}", found[0]);
    }
    let clean: ConfigFile = load_fixture("config/repo-config.yaml");
    assert_eq!(secrets_in_file(&clean, &secrets), []);
    let other_pack: ConfigFile = load("config:\n  demo: { notifyHook: x }\n");
    assert_eq!(secrets_in_file(&other_pack, &secrets), []);
}

// The disk and claude sections.

#[test]
fn a_file_with_a_disk_and_a_claude_section_loads() {
    let file: ConfigFile = load_fixture("config/disk-and-claude.yaml");
    assert_eq!(check_config_file(&file), Ok(()));
    assert_eq!(file.disk.floor_gib, Some(8));
    assert_eq!(file.claude.check_published, Some(false));
    assert_eq!(
        file.collide_keys_set(),
        ["disk.floor_gib", "claude.check_published"]
    );
}

#[test]
fn a_file_with_no_disk_or_claude_section_sets_neither_and_writes_neither() {
    let file: ConfigFile = load("ask: { timeout_s: 60 }\n");
    assert_eq!(file.disk.floor_gib, None);
    assert_eq!(file.claude.check_published, None);
    let written = serde_json::to_string(&file).unwrap();
    assert!(
        !written.contains("disk") && !written.contains("claude"),
        "{written}"
    );
}

#[test]
fn a_floor_of_zero_fails_naming_the_key_and_the_minimum() {
    let errors = config_errors("config/invalid/disk-floor-zero.yaml");
    assert_eq!(
        errors,
        [SettingsError::BelowMinimum {
            key: "disk.floor_gib",
            value: 0,
            min: 1
        }]
    );
    let message = errors[0].to_string();
    assert!(
        message.contains("disk.floor_gib") && message.contains('0'),
        "{message}"
    );
}

#[test]
fn a_floor_of_one_is_valid() {
    let file: ConfigFile = load("disk: { floor_gib: 1 }\n");
    assert_eq!(check_config_file(&file), Ok(()));
}

#[test]
fn an_unknown_key_in_disk_or_claude_fails_naming_it() {
    let error = fixture_error::<ConfigFile>("config/invalid/disk-unknown-key.yaml");
    assert!(error.contains("other"), "{error}");
    let error = fixture_error::<ConfigFile>("config/invalid/claude-unknown-key.yaml");
    assert!(error.contains("check_unpublished"), "{error}");
}

#[test]
fn a_value_of_the_wrong_type_in_disk_or_claude_fails() {
    for text in [
        "disk: { floor_gib: 1.5 }\n",
        "disk: { floor_gib: -1 }\n",
        "claude: { check_published: \"yes\" }\n",
        "claude: { check_published: 1 }\n",
    ] {
        parse_error::<ConfigFile>(text);
    }
}

#[test]
fn the_disk_and_claude_sections_are_in_the_generated_schema_and_closed() {
    let schema = schema_file("ConfigFile.input.json");
    let keys: BTreeSet<&str> = schema["properties"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        keys,
        BTreeSet::from(["agents", "ask", "claude", "config", "disk", "ui_tools"])
    );
    assert_eq!(
        schema["$defs"]["DiskSettings"]["properties"]["floor_gib"]["minimum"],
        json!(1)
    );
    assert!(schema["$defs"]["ClaudeSettings"]["properties"]["check_published"].is_object());
}

// The ui_tools section.

#[test]
fn a_file_with_a_ui_tools_port_loads_and_sets_that_key() {
    let file: ConfigFile = load("ui_tools: { port: 40123 }\n");
    assert_eq!(check_config_file(&file), Ok(()));
    assert_eq!(file.ui_tools.port, Some(40123));
    assert_eq!(file.collide_keys_set(), ["ui_tools.port"]);
}

#[test]
fn a_port_under_1024_fails_naming_the_key_and_the_minimum() {
    let file: ConfigFile = load("ui_tools: { port: 80 }\n");
    let errors = check_config_file(&file).expect_err("a privileged port is refused");
    assert_eq!(
        errors,
        [SettingsError::BelowMinimum {
            key: "ui_tools.port",
            value: 80,
            min: 1024
        }]
    );
}

#[test]
fn a_port_that_is_not_a_port_number_fails_to_load() {
    for text in [
        "ui_tools: { port: 70000 }\n",
        "ui_tools: { port: -1 }\n",
        "ui_tools: { port: \"x\" }\n",
        "ui_tools: { other: 1 }\n",
    ] {
        parse_error::<ConfigFile>(text);
    }
}

// Schemas.

#[test]
fn the_board_config_and_pointer_schemas_are_registered_with_agents_and_ask() {
    let rendered = schema::registry().render();
    for name in ["Board", "ConfigFile", "CollidePointer"] {
        for side in ["input", "output"] {
            assert!(
                rendered.contains_key(&format!("{name}.{side}.json")),
                "{name} {side}"
            );
        }
    }
    let config = schema_file("ConfigFile.input.json");
    let keys: BTreeSet<&str> = config["properties"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        keys,
        BTreeSet::from(["agents", "ask", "claude", "config", "disk", "ui_tools"])
    );
    let agent_keys: BTreeSet<&str> = config["$defs"]["AgentEntry"]["properties"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        agent_keys,
        BTreeSet::from([
            "kind",
            "command",
            "args",
            "model",
            "config",
            "read",
            "tools",
            "timeout_s",
            "budget_usd",
            "env",
            "secrets"
        ])
    );
    let ask_keys: BTreeSet<&str> = config["$defs"]["Ask"]["properties"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        ask_keys,
        BTreeSet::from(["agent", "budget_usd", "timeout_s"])
    );
}

#[test]
fn every_struct_of_the_three_files_is_closed_in_the_schema() {
    for name in ["Board", "ConfigFile", "CollidePointer"] {
        for side in ["input", "output"] {
            let schema = schema_file(&format!("{name}.{side}.json"));
            let mut open = Vec::new();
            collect_open_objects(&schema, name, &mut open);
            assert_eq!(open, Vec::<String>::new(), "{name}.{side}");
        }
    }
}

fn collect_open_objects(value: &Value, path: &str, found: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            if map.contains_key("properties")
                && map.get("additionalProperties") != Some(&json!(false))
            {
                found.push(path.to_string());
            }
            for (key, child) in map {
                collect_open_objects(child, &format!("{path}/{key}"), found);
            }
        }
        Value::Array(items) => {
            for (i, child) in items.iter().enumerate() {
                collect_open_objects(child, &format!("{path}/{i}"), found);
            }
        }
        _ => {}
    }
}
