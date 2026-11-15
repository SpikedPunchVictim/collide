//! The launch line (0.3, S1) built from a `LaunchRequest`: the argv, the context options, the variables to strip.

use super::{ASSIGNED, plain_role, request, value_of};
use collide_core::adapter::{AgentAdapter, LaunchError};
use collide_core::agent::SessionStart;
use collide_host::adapters::claude::ClaudeAdapter;
use collide_host::adapters::claude::launch::{
    BASH_OUTPUT_MAX_VAR, COMPACT_AT_FLOOR, COMPACT_AT_VAR, DISALLOWED_TOOLS, MCP_OUTPUT_MAX_VAR,
    disallowed_tools_without,
};
use collide_host::base_env::BaseEnvironment;
use collide_host::base_env::EnvSource;
use collide_host::clock::{Clock, Reading};
use serde_json::json;
use std::path::PathBuf;

fn refusal_message(role: serde_json::Value) -> String {
    match ClaudeAdapter.launch(&request(role)) {
        Err(LaunchError::Invalid { message }) => message,
        other => panic!("expected an invalid request, got {other:?}"),
    }
}

// ---- context options ---------------------------------------------------------------------------------------------

#[test]
fn a_compact_at_of_99_999_is_refused_and_the_message_names_the_floor() {
    let message = refusal_message(json!({ "agent": "claude", "compact_at": 99_999 }));
    assert!(message.contains("100,000"), "{message}");
    assert!(message.contains("compact_at"), "{message}");
    assert!(message.contains("99,999"), "{message}");
}

#[test]
fn a_compact_at_of_exactly_the_floor_is_accepted_and_sets_the_window_variable() {
    let spec = ClaudeAdapter
        .launch(&request(
            json!({ "agent": "claude", "compact_at": COMPACT_AT_FLOOR }),
        ))
        .unwrap();
    assert_eq!(spec.env[COMPACT_AT_VAR], "100000");
}

#[test]
fn the_output_caps_become_variables_and_an_unset_option_sets_nothing() {
    let spec = ClaudeAdapter
        .launch(&request(
            json!({ "agent": "claude", "bash_output_max": 8000, "mcp_output_max": 12000 }),
        ))
        .unwrap();
    assert_eq!(spec.env[BASH_OUTPUT_MAX_VAR], "8000");
    assert_eq!(spec.env[MCP_OUTPUT_MAX_VAR], "12000");
    assert!(!spec.env.contains_key(COMPACT_AT_VAR));

    let plain = ClaudeAdapter.launch(&request(plain_role())).unwrap();
    assert!(plain.env.is_empty());
}

#[test]
fn an_output_cap_of_zero_is_refused_rather_than_ignored() {
    for key in ["bash_output_max", "mcp_output_max"] {
        let message = refusal_message(json!({ "agent": "claude", key: 0 }));
        assert!(message.contains(key), "{message}");
    }
}

#[test]
fn config_none_passes_an_empty_setting_sources_and_keeps_strict_mcp_config() {
    for (role, sources) in [
        (json!({ "agent": "claude" }), "user"),
        (json!({ "agent": "claude", "config": "user" }), "user"),
        (json!({ "agent": "claude", "config": "none" }), ""),
    ] {
        let spec = ClaudeAdapter.launch(&request(role)).unwrap();
        assert_eq!(value_of(&spec.argv, "--setting-sources"), Some(sources));
        assert!(spec.argv.contains(&"--strict-mcp-config".to_string()));
    }
}

// ---- the argv ----------------------------------------------------------------------------------------------------

#[test]
fn an_assigned_session_passes_session_id_and_never_resume() {
    let mut req = request(plain_role());
    req.budget_left_usd = Some(0.5);
    let spec = ClaudeAdapter.launch(&req).unwrap();
    let argv = &spec.argv;
    assert_eq!(value_of(argv, "--session-id"), Some(ASSIGNED));
    assert!(!argv.contains(&"--resume".to_string()));
    assert_eq!(value_of(argv, "--model"), Some("sonnet"));
    assert_eq!(value_of(argv, "--max-budget-usd"), Some("0.5"));
    assert_eq!(value_of(argv, "--agent"), Some("collide-demo-builder"));
    assert_eq!(value_of(argv, "--agents"), Some("/snap/agents.json"));
    assert_eq!(value_of(argv, "--settings"), Some("/snap/settings.json"));
    assert_eq!(value_of(argv, "--mcp-config"), Some("/snap/mcp.json"));
    assert_eq!(value_of(argv, "--setting-sources"), Some("user"));
    assert_eq!(value_of(argv, "--permission-prompt-tool"), Some("stdio"));
    assert_eq!(value_of(argv, "--input-format"), Some("stream-json"));
    assert_eq!(value_of(argv, "--output-format"), Some("stream-json"));
    for flag in ["-p", "--verbose", "--include-hook-events"] {
        assert!(argv.contains(&flag.to_string()), "{flag}");
    }
    assert_eq!(spec.cwd, PathBuf::from("/work"));
}

#[test]
fn a_resumed_session_passes_resume_and_never_session_id() {
    let mut req = request(plain_role());
    req.session = SessionStart::Resume {
        id: ASSIGNED.into(),
    };
    let argv = ClaudeAdapter.launch(&req).unwrap().argv;
    assert_eq!(value_of(&argv, "--resume"), Some(ASSIGNED));
    assert!(!argv.contains(&"--session-id".to_string()));
}

#[test]
fn a_session_the_tool_would_name_itself_is_refused() {
    let mut req = request(plain_role());
    req.session = SessionStart::AgentAssigned {};
    let error = ClaudeAdapter.launch(&req).unwrap_err().to_string();
    assert!(error.contains("--session-id"), "{error}");
}

#[test]
fn a_session_id_that_could_become_a_flag_is_refused() {
    let mut req = request(plain_role());
    req.session = SessionStart::Resume {
        id: "--dangerous".into(),
    };
    assert!(ClaudeAdapter.launch(&req).is_err());
}

#[test]
fn no_prompt_text_appears_in_argv() {
    let mut req = request(plain_role());
    req.prompt = "TOPSECRET-PROMPT-TEXT".into();
    let argv = ClaudeAdapter.launch(&req).unwrap().argv;
    assert!(argv.iter().all(|word| !word.contains("TOPSECRET")));
}

#[test]
fn a_request_with_no_budget_or_model_passes_neither_flag() {
    let mut req = request(plain_role());
    req.model = None;
    let argv = ClaudeAdapter.launch(&req).unwrap().argv;
    assert!(!argv.contains(&"--max-budget-usd".to_string()));
    assert!(!argv.contains(&"--model".to_string()));
}

#[test]
fn a_budget_that_is_spent_or_not_a_number_is_refused() {
    for left in [0.0, -0.01, f64::NAN, f64::INFINITY] {
        let mut req = request(plain_role());
        req.budget_left_usd = Some(left);
        assert!(ClaudeAdapter.launch(&req).is_err(), "{left}");
    }
}

#[test]
fn a_model_name_that_fails_the_safe_token_never_reaches_argv() {
    let mut req = request(plain_role());
    req.model = Some(collide_core::agent::ModelRef {
        name: "--evil".into(),
        id: Some("ollama/qwen2.5:7b".into()),
    });
    assert!(ClaudeAdapter.launch(&req).is_err());
}

#[test]
fn the_model_id_never_reaches_argv_or_env() {
    let mut req = request(plain_role());
    req.model = Some(collide_core::agent::ModelRef {
        name: "sonnet".into(),
        id: Some("OPAQUE/ID:1".into()),
    });
    let spec = ClaudeAdapter.launch(&req).unwrap();
    assert!(spec.argv.iter().all(|word| !word.contains("OPAQUE")));
    assert!(spec.env.values().all(|value| !value.contains("OPAQUE")));
}

#[test]
fn the_disallowed_tools_follow_their_flag_one_to_a_word() {
    let argv = ClaudeAdapter.launch(&request(plain_role())).unwrap().argv;
    let at = argv.iter().position(|w| w == "--disallowed-tools").unwrap();
    assert_eq!(
        argv[at + 1..at + 1 + DISALLOWED_TOOLS.len()],
        DISALLOWED_TOOLS
    );
}

#[test]
fn a_tool_a_role_asks_for_is_left_off_the_list_and_asking_for_all_drops_the_flag() {
    let kept = disallowed_tools_without(&["Workflow".to_string()]);
    assert!(!kept.contains(&"Workflow"));
    assert_eq!(kept.len(), DISALLOWED_TOOLS.len() - 1);
    let every: Vec<String> = DISALLOWED_TOOLS.iter().map(|t| t.to_string()).collect();
    assert!(disallowed_tools_without(&every).is_empty());
}

#[test]
fn a_role_name_that_is_not_pack_slash_role_is_refused() {
    for name in [
        "builder",
        "demo/",
        "/builder",
        "demo/build/er",
        "de mo/builder",
        "-x/builder",
    ] {
        let mut req = request(plain_role());
        req.role_name = name.into();
        assert!(ClaudeAdapter.launch(&req).is_err(), "{name}");
    }
}

#[test]
fn two_calls_with_the_same_request_give_the_same_spec() {
    let mut req = request(json!({ "agent": "claude", "compact_at": 150000 }));
    req.budget_left_usd = Some(1.25);
    assert_eq!(
        ClaudeAdapter.launch(&req).unwrap(),
        ClaudeAdapter.launch(&req).unwrap()
    );
}

// ---- the environment ---------------------------------------------------------------------------------------------

fn captured(vars: &[(&str, &str)]) -> BaseEnvironment {
    BaseEnvironment {
        vars: vars
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
        source: EnvSource::LoginShell {
            shell: "/bin/zsh".into(),
        },
        captured_at: Reading {
            clock: Clock::Wall,
            nanos: 0,
        },
    }
}

#[test]
fn the_claude_marker_and_every_claude_variable_are_stripped_and_the_endpoint_is_kept() {
    let base = captured(&[
        ("CLAUDECODE", "1"),
        ("CLAUDE_PID", "4242"),
        ("CLAUDE_CODE_ENTRYPOINT", "cli"),
        ("ANTHROPIC_BASE_URL", "https://gateway.example.com"),
        ("PATH", "/usr/bin"),
    ]);
    let spec = ClaudeAdapter.launch(&request(plain_role())).unwrap();
    let child = base.for_child(&spec.env_strip, &spec.env);
    for gone in ["CLAUDECODE", "CLAUDE_PID", "CLAUDE_CODE_ENTRYPOINT"] {
        assert!(!child.contains_key(gone), "{gone} reached the child");
    }
    assert_eq!(child["ANTHROPIC_BASE_URL"], "https://gateway.example.com");
    assert_eq!(child["PATH"], "/usr/bin");
}

#[test]
fn the_context_variables_are_set_after_the_strip_so_an_inherited_value_loses() {
    let base = captured(&[
        (COMPACT_AT_VAR, "5000"),
        (BASH_OUTPUT_MAX_VAR, "1"),
        (MCP_OUTPUT_MAX_VAR, "1"),
    ]);
    let spec = ClaudeAdapter
        .launch(&request(json!({
            "agent": "claude", "compact_at": 120000, "bash_output_max": 9000, "mcp_output_max": 7000
        })))
        .unwrap();
    let child = base.for_child(&spec.env_strip, &spec.env);
    assert_eq!(child[COMPACT_AT_VAR], "120000");
    assert_eq!(child[BASH_OUTPUT_MAX_VAR], "9000");
    assert_eq!(child[MCP_OUTPUT_MAX_VAR], "7000");
}

#[test]
fn an_inherited_window_is_gone_when_the_role_sets_none() {
    let base = captured(&[(COMPACT_AT_VAR, "5000")]);
    let spec = ClaudeAdapter.launch(&request(plain_role())).unwrap();
    let child = base.for_child(&spec.env_strip, &spec.env);
    assert!(!child.contains_key(COMPACT_AT_VAR));
}
