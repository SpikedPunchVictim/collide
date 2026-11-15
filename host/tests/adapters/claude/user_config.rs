//! The user's own config a phase loads under `config: user` (0.3, The user's own tooling; revision 15): the hooks and
//! the settings `env`, read for the run preview's flags, and the model endpoint's host for the fingerprint. The
//! settings are the fixture `fixtures/user-settings.json` copied below a temp `HOME`, never the owner's own file.

use crate::common::TempRoot;
use collide_core::profile::RoleConfig;
use collide_host::adapters::claude::user_config::{
    Endpoint, EndpointSource, EnvFlagKind, HookFlag, UserConfig, UserConfigError, endpoint_host,
    endpoint_in_environment, read_user_config, user_config_fingerprint,
};
use std::collections::BTreeMap;
use std::fs;

const FIXTURE: &str = include_str!("fixtures/user-settings.json");

fn home_with(settings: &str) -> TempRoot {
    let home = TempRoot::new();
    fs::create_dir_all(home.join(".claude")).unwrap();
    fs::write(home.join(".claude/settings.json"), settings).unwrap();
    home
}

fn read(home: &TempRoot, config: Option<RoleConfig>) -> Result<UserConfig, UserConfigError> {
    read_user_config(home.path().to_str(), config)
}

#[test]
fn each_hook_is_listed_with_its_flag() {
    let home = home_with(FIXTURE);
    let config = read(&home, Some(RoleConfig::User)).unwrap();
    let seen: Vec<(&str, Option<HookFlag>)> = config
        .hooks
        .iter()
        .map(|hook| (hook.name.as_str(), hook.flag))
        .collect();
    assert_eq!(
        seen,
        [
            ("PostToolUse:Read|Grep", Some(HookFlag::PostToolUse)),
            ("PreToolUse:Bash", Some(HookFlag::PreToolUse)),
            ("SessionStart", Some(HookFlag::SessionStart)),
            ("Stop", None),
            ("UserPromptSubmit", Some(HookFlag::UserPromptSubmit)),
        ]
    );
}

#[test]
fn the_four_flagged_events_each_say_what_the_hook_can_change() {
    for flag in [
        HookFlag::PreToolUse,
        HookFlag::UserPromptSubmit,
        HookFlag::SessionStart,
        HookFlag::PostToolUse,
    ] {
        assert!(!flag.can_change().is_empty());
    }
}

#[test]
fn the_endpoint_is_recorded_as_its_host_alone() {
    let home = home_with(FIXTURE);
    let config = read(&home, Some(RoleConfig::User)).unwrap();
    assert_eq!(
        config.endpoint,
        Some(Endpoint {
            source: EndpointSource::SettingsEnv,
            host: Some("gateway.example.com".into()),
        })
    );
}

#[test]
fn nothing_but_the_host_of_the_url_survives_anywhere_in_what_is_read() {
    let home = home_with(FIXTURE);
    let config = read(&home, Some(RoleConfig::User)).unwrap();
    let everything = format!("{config:?}");
    for leaked in [
        "FIXTURE-KEY",
        "api_key",
        "v1/proxy",
        "8443",
        "FIXTURE-HOOK-SECRET",
        "fixture-rewrite",
    ] {
        assert!(!everything.contains(leaked), "{leaked} in {everything}");
    }
}

#[test]
fn a_settings_env_that_sets_a_context_variable_or_the_endpoint_is_flagged_by_name_only() {
    let home = home_with(FIXTURE);
    let config = read(&home, Some(RoleConfig::User)).unwrap();
    let flags: Vec<(&str, EnvFlagKind)> = config
        .env_flags
        .iter()
        .map(|flag| (flag.name.as_str(), flag.kind))
        .collect();
    assert_eq!(
        flags,
        [
            ("ANTHROPIC_BASE_URL", EnvFlagKind::Endpoint),
            (
                "CLAUDE_CODE_AUTO_COMPACT_WINDOW",
                EnvFlagKind::ContextVariable
            ),
        ]
    );
}

#[test]
fn config_none_reads_nothing_even_when_the_file_is_there() {
    let home = home_with(FIXTURE);
    assert_eq!(
        read(&home, Some(RoleConfig::None)).unwrap(),
        UserConfig::default()
    );
}

#[test]
fn a_role_with_no_config_key_reads_the_user_config_as_the_launch_line_loads_it() {
    let home = home_with(FIXTURE);
    assert_eq!(read(&home, None).unwrap().hooks.len(), 5);
}

#[test]
fn a_missing_file_is_an_empty_config() {
    let home = TempRoot::new();
    assert_eq!(
        read(&home, Some(RoleConfig::User)).unwrap(),
        UserConfig::default()
    );
}

#[test]
fn a_settings_file_that_is_not_json_fails_with_its_path_and_the_place() {
    let home = home_with("{ \"hooks\": ");
    let error = read(&home, Some(RoleConfig::User)).unwrap_err().to_string();
    assert!(error.contains(".claude/settings.json"), "{error}");
    assert!(error.contains("line 1"), "{error}");
}

#[test]
fn a_settings_file_of_the_wrong_shape_fails_with_its_path_and_what_was_wrong() {
    for (text, what) in [
        ("[]", "object"),
        ("{ \"hooks\": [] }", "hooks"),
        ("{ \"env\": 3 }", "env"),
        ("{ \"hooks\": { \"Stop\": 1 } }", "Stop"),
    ] {
        let home = home_with(text);
        let error = read(&home, Some(RoleConfig::User)).unwrap_err().to_string();
        assert!(error.contains(".claude/settings.json"), "{error}");
        assert!(error.contains(what), "{what}: {error}");
    }
}

#[test]
fn no_home_is_an_error_under_config_user_and_not_under_config_none() {
    for home in [None, Some("")] {
        let error = read_user_config(home, Some(RoleConfig::User)).unwrap_err();
        assert!(error.to_string().contains("HOME"), "{error}");
        assert_eq!(
            read_user_config(home, Some(RoleConfig::None)).unwrap(),
            UserConfig::default()
        );
    }
}

#[test]
fn a_directory_in_place_of_the_file_is_an_error_and_not_an_empty_config() {
    let home = TempRoot::new();
    fs::create_dir_all(home.join(".claude/settings.json")).unwrap();
    assert!(read(&home, Some(RoleConfig::User)).is_err());
}

// ---- the endpoint, from a URL and from the captured environment --------------------------------------------------

#[test]
fn the_host_drops_the_path_the_query_the_port_and_a_key_in_the_user_part() {
    for (url, host) in [
        ("https://gateway.example.com", "gateway.example.com"),
        (
            "https://Gateway.Example.com:8443/v1/proxy?api_key=K",
            "gateway.example.com",
        ),
        (
            "https://user:FIXTURE-KEY@gateway.example.com/v1",
            "gateway.example.com",
        ),
        ("http://127.0.0.1:4000", "127.0.0.1"),
        ("http://[::1]:4000/x", "[::1]"),
        ("https://gateway.example.com?key=K", "gateway.example.com"),
        ("https://gateway.example.com#frag", "gateway.example.com"),
    ] {
        assert_eq!(endpoint_host(url).as_deref(), Some(host), "{url}");
    }
}

#[test]
fn a_value_that_is_not_a_url_with_a_plain_host_records_no_host_rather_than_a_guess() {
    for url in [
        "",
        "gateway.example.com/v1?key=K",
        "https://",
        "https://bad host/x",
        "https://[::1/x",
        "https://exa mple.com",
    ] {
        assert_eq!(endpoint_host(url), None, "{url:?}");
    }
}

#[test]
fn the_same_variable_in_the_captured_environment_is_recorded_the_same_way() {
    let env = BTreeMap::from([
        (
            "ANTHROPIC_BASE_URL".to_string(),
            "https://gateway.example.com:8443/v1/proxy?api_key=FIXTURE-KEY-0000".to_string(),
        ),
        ("PATH".to_string(), "/usr/bin".to_string()),
    ]);
    let endpoint = endpoint_in_environment(&env).unwrap();
    assert_eq!(endpoint.source, EndpointSource::Environment);
    assert_eq!(endpoint.host.as_deref(), Some("gateway.example.com"));
    assert!(!format!("{endpoint:?}").contains("FIXTURE-KEY"));
}

#[test]
fn an_environment_without_the_variable_has_no_endpoint() {
    assert_eq!(endpoint_in_environment(&BTreeMap::new()), None);
}

#[test]
fn the_fingerprint_part_names_the_hooks_the_flagged_env_and_the_endpoint_host() {
    let home = home_with(FIXTURE);
    let config = read(&home, Some(RoleConfig::User)).unwrap();
    let part = user_config_fingerprint(&config);
    assert_eq!(
        part["hooks"][1],
        serde_json::json!({"name": "PreToolUse:Bash", "event": "PreToolUse", "flag": "pre_tool_use"})
    );
    assert_eq!(
        part["hooks"][3]["flag"],
        serde_json::Value::Null,
        "Stop is not flagged"
    );
    assert_eq!(
        part["env_flags"],
        serde_json::json!([
            {"name": "ANTHROPIC_BASE_URL", "kind": "endpoint"},
            {"name": "CLAUDE_CODE_AUTO_COMPACT_WINDOW", "kind": "context_variable"},
        ])
    );
    assert_eq!(
        part["endpoint"],
        serde_json::json!({"source": "settings_env", "host": "gateway.example.com"})
    );
}

#[test]
fn the_fingerprint_part_holds_no_command_key_path_or_port() {
    let home = home_with(FIXTURE);
    let config = read(&home, Some(RoleConfig::User)).unwrap();
    let text = user_config_fingerprint(&config).to_string();
    for leaked in [
        "FIXTURE-KEY",
        "api_key",
        "v1/proxy",
        "8443",
        "FIXTURE-HOOK-SECRET",
        "fixture-rewrite",
    ] {
        assert!(!text.contains(leaked), "{leaked} in {text}");
    }
}

#[test]
fn an_empty_config_has_a_part_with_empty_lists_and_no_endpoint() {
    let part = user_config_fingerprint(&UserConfig::default());
    assert_eq!(
        part,
        serde_json::json!({"hooks": [], "env_flags": [], "endpoint": null})
    );
}
