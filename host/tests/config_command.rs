//! `collide config get` (ts-0098-nx): the command over a daemon in the test's own process (a fixture checkout, a temp
//! home), and through the real binary and a real daemon process for the cases that need no packs. A line is
//! `key = value  (layer)`, a secret is `key = <secret, in the keychain>  (user)`, and a refusal prints every problem
//! with a nonzero exit and nothing on standard output.

use collide_host::config_cli::{entry_line, parse, run_with};
use collide_host::daemon::DaemonPaths;
use collide_host::task_cli::exit;
use std::path::Path;

mod common;
use common::config_fixture::Fixture;

fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

/// Runs `collide config <list>` from `cwd`: the exit code, standard output and standard error.
fn config(fixture: &Fixture, cwd: &Path, list: &[&str]) -> (u8, String, String) {
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = run_with(fixture.daemon.paths(), &args(list), cwd, &mut out, &mut err);
    (
        code,
        String::from_utf8(out).unwrap(),
        String::from_utf8(err).unwrap(),
    )
}

fn in_repo(fixture: &Fixture, list: &[&str]) -> (u8, String, String) {
    config(fixture, fixture.repo.work(), list)
}

// ---- reading the command line ----

#[test]
fn get_with_a_key_and_a_profile_is_read() {
    let parsed = parse(&args(&["get", "disk.floor_gib", "--profile", "quick"])).unwrap();
    assert_eq!(parsed.key.as_deref(), Some("disk.floor_gib"));
    assert_eq!(parsed.profile.as_deref(), Some("quick"));
}

#[test]
fn get_with_nothing_else_is_every_setting() {
    let parsed = parse(&args(&["get"])).unwrap();
    assert_eq!((parsed.key, parsed.profile), (None, None));
}

#[test]
fn the_profile_may_come_before_the_key() {
    let parsed = parse(&args(&["get", "--profile", "quick", "ask.timeout_s"])).unwrap();
    assert_eq!(parsed.key.as_deref(), Some("ask.timeout_s"));
}

#[test]
fn a_bad_command_line_is_refused() {
    for bad in [
        args(&[]),
        args(&["set", "a", "b"]),
        args(&["get", "a", "b"]),
        args(&["get", "--profile"]),
        args(&["get", "--profile", ""]),
        args(&["get", "--profile", "a", "--profile", "b"]),
        args(&["get", "--json"]),
        args(&["get", ""]),
    ] {
        assert!(parse(&bad).is_err(), "{bad:?}");
    }
}

// ---- the text of one entry ----

fn entry(key: &str, value: Option<serde_json::Value>, layer: &str, secret: bool) -> String {
    let found: collide_core::owner::ConfigEntry = serde_json::from_value(serde_json::json!({
        "key": key, "value": value, "layer": layer, "secret": secret
    }))
    .unwrap();
    entry_line(&found)
}

#[test]
fn an_entry_is_key_equals_value_and_the_layer_in_brackets() {
    use serde_json::json;
    assert_eq!(
        entry("disk.floor_gib", Some(json!(9)), "repo", false),
        "disk.floor_gib = 9  (repo)"
    );
    assert_eq!(
        entry(
            "claude.check_published",
            Some(json!(true)),
            "default",
            false
        ),
        "claude.check_published = true  (default)"
    );
    assert_eq!(
        entry("ask.budget_usd", Some(json!(1.5)), "user", false),
        "ask.budget_usd = 1.5  (user)"
    );
    assert_eq!(
        entry("ferry.greeting", Some(json!("hi there")), "profile", false),
        "ferry.greeting = \"hi there\"  (profile)"
    );
}

#[test]
fn a_secret_is_never_a_value() {
    assert_eq!(
        entry("ferry.notifyHook", None, "user", true),
        "ferry.notifyHook = <secret, in the keychain>  (user)"
    );
}

#[test]
fn a_setting_with_no_value_says_so() {
    assert_eq!(
        entry("ask.agent", None, "default", false),
        "ask.agent = <not set>  (default)"
    );
}

// ---- the command, in a checkout ----

#[test]
fn a_key_prints_one_line_with_the_repos_layer() {
    let fixture = Fixture::new();
    fixture.write(".collide/config.yaml", "disk: { floor_gib: 9 }\n");
    fixture.commit();
    let (code, out, err) = in_repo(&fixture, &["get", "disk.floor_gib"]);
    assert_eq!((code, err.as_str()), (exit::OK, ""));
    assert_eq!(out, "disk.floor_gib = 9  (repo)\n");
}

#[test]
fn a_key_set_in_the_users_file_says_user() {
    let fixture = Fixture::new();
    fixture.write_user("config.yaml", "disk: { floor_gib: 3 }\n");
    let (code, out, _) = in_repo(&fixture, &["get", "disk.floor_gib"]);
    assert_eq!(code, exit::OK);
    assert_eq!(out, "disk.floor_gib = 3  (user)\n");
}

#[test]
fn a_key_set_nowhere_says_default() {
    let fixture = Fixture::new();
    let (_, out, _) = in_repo(&fixture, &["get", "disk.floor_gib"]);
    assert_eq!(out, "disk.floor_gib = 5  (default)\n");
}

#[test]
fn from_a_folder_below_the_top_the_same_board_is_found() {
    let fixture = Fixture::new();
    fixture.write(".collide/config.yaml", "disk: { floor_gib: 9 }\n");
    fixture.write("src/lib.rs", "\n");
    fixture.commit();
    let (code, out, _) = config(
        &fixture,
        &fixture.repo.work().join("src"),
        &["get", "disk.floor_gib"],
    );
    assert_eq!(code, exit::OK);
    assert_eq!(out, "disk.floor_gib = 9  (repo)\n");
}

#[test]
fn with_no_key_every_setting_is_listed_one_to_a_line_in_order() {
    let fixture = Fixture::new();
    fixture.ferry();
    fixture.commit();
    let (code, out, _) = in_repo(&fixture, &["get"]);
    assert_eq!(code, exit::OK);
    let keys: Vec<&str> = out
        .lines()
        .map(|line| line.split(" = ").next().unwrap())
        .collect();
    assert_eq!(
        keys,
        [
            "ask.agent",
            "ask.timeout_s",
            "ask.budget_usd",
            "disk.floor_gib",
            "claude.check_published",
            "ferry.greeting",
            "ferry.lanePool",
            "ferry.layout",
            "ferry.notifyHook"
        ]
    );
}

#[test]
fn a_secret_entry_prints_the_keychain_line() {
    let fixture = Fixture::new();
    fixture.ferry();
    fixture.commit();
    let (code, out, _) = in_repo(&fixture, &["get", "ferry.notifyHook"]);
    assert_eq!(code, exit::OK);
    assert_eq!(
        out,
        "ferry.notifyHook = <secret, in the keychain>  (user)\n"
    );
}

#[test]
fn a_profile_is_the_top_layer() {
    let fixture = Fixture::new();
    fixture.write_user("profiles/slow.yaml", "config: { ask: { timeout_s: 90 } }\n");
    let (code, out, _) = in_repo(&fixture, &["get", "ask.timeout_s", "--profile", "slow"]);
    assert_eq!(code, exit::OK);
    assert_eq!(out, "ask.timeout_s = 90  (profile)\n");
}

// ---- refusals ----

#[test]
fn a_refusal_prints_every_problem_on_standard_error_and_exits_nonzero() {
    let fixture = Fixture::new();
    fixture.ferry();
    fixture.write(
        ".collide/config.yaml",
        "claude: { check_published: false }\n",
    );
    fixture.commit();
    fixture.write_user("config.yaml", "config: { ferry: { lanePool: 0 } }\n");
    let (code, out, err) = in_repo(&fixture, &["get"]);
    assert_eq!(code, exit::FAILED);
    assert_eq!(out, "");
    assert!(err.contains("claude.check_published"), "{err}");
    assert!(err.contains("collide config get"), "{err}");
}

#[test]
fn a_user_profile_with_a_key_not_overridable_is_refused_naming_the_key() {
    let fixture = Fixture::new();
    fixture.write_user(
        "profiles/greedy.yaml",
        "config: { ask: { budget_usd: 50 } }\n",
    );
    let (code, out, err) = in_repo(&fixture, &["get", "--profile", "greedy"]);
    assert_eq!(code, exit::FAILED);
    assert_eq!(out, "");
    assert!(err.contains("ask.budget_usd"), "{err}");
}

#[test]
fn an_unknown_key_exits_with_the_not_found_code_naming_it() {
    let fixture = Fixture::new();
    let (code, out, err) = in_repo(&fixture, &["get", "disk.nope"]);
    assert_eq!(code, exit::ID);
    assert_eq!(out, "");
    assert!(err.contains("disk.nope"), "{err}");
}

#[test]
fn an_unknown_profile_exits_with_the_not_found_code_naming_it() {
    let fixture = Fixture::new();
    let (code, _, err) = in_repo(&fixture, &["get", "--profile", "nope"]);
    assert_eq!(code, exit::ID);
    assert!(err.contains("nope"), "{err}");
}

#[test]
fn a_bad_command_line_exits_with_the_usage_code_and_the_usage() {
    let fixture = Fixture::new();
    let (code, out, err) = in_repo(&fixture, &["get", "a", "b"]);
    assert_eq!(code, exit::USAGE);
    assert_eq!(out, "");
    assert!(err.contains("usage: collide config get"), "{err}");
}

// ---- outside a repository ----

#[test]
fn outside_a_checkout_it_shows_the_users_layers_and_says_so_on_standard_error() {
    let fixture = Fixture::new();
    fixture.write_user("config.yaml", "disk: { floor_gib: 3 }\n");
    let outside = fixture.home().join("not-a-repo");
    std::fs::create_dir(&outside).unwrap();
    let (code, out, err) = config(&fixture, &outside, &["get", "disk.floor_gib"]);
    assert_eq!(code, exit::OK);
    assert_eq!(out, "disk.floor_gib = 3  (user)\n");
    assert!(err.contains("user"), "{err}");
}

#[test]
fn outside_a_checkout_a_pack_key_is_not_found_and_says_a_board_is_needed() {
    let fixture = Fixture::new();
    let outside = fixture.home().join("not-a-repo");
    std::fs::create_dir(&outside).unwrap();
    let (code, _, err) = config(&fixture, &outside, &["get", "ferry.lanePool"]);
    assert_eq!(code, exit::ID);
    assert!(err.contains("board"), "{err}");
}

#[test]
fn the_paths_are_the_daemons_own() {
    let fixture = Fixture::new();
    let paths: &DaemonPaths = fixture.daemon.paths();
    assert!(paths.socket.starts_with(fixture.home()));
}
