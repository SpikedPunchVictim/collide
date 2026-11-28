//! The ask's launch spec (ts-0200-gs; 0.3, Claude Code's launch line): the pinned line, the ask folder's files, the
//! checks before launch and the environment. No test starts a process: the command lookup reads a `PATH` the test
//! gives it, and the only "claude" it finds is a file the test wrote.

use super::common::{TempRoot, write_executable};
use collide_core::adapter::LaunchError;
use collide_core::agent::Support;
use collide_core::profile::RoleConfig;
use collide_host::adapters::claude::ask::{
    ASK_PROMPT, AskProfile, ask_launch_spec, check_before_launch, remove_ask_folder,
    write_ask_folder,
};
use collide_host::adapters::claude::{capabilities, env_strip};
use collide_host::base_env::{BaseEnvironment, EnvSource};
use collide_host::clock::{Clock, Reading};
use serde_json::{Value, json};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

const TOKEN: &str = "ask-token-7f3a91c2d4e65b08";
const REQUEST: &str = "regroup the larder tasks by the file they would change";
const ENDPOINT: &str = "http://127.0.0.1:41873/mcp";

fn profile(root: &TempRoot) -> AskProfile {
    AskProfile {
        model: "haiku".into(),
        read: vec![root.join("code")],
        config: RoleConfig::None,
        budget_usd: None,
        dir: root.join("ask"),
    }
}

fn argv_of(profile: &AskProfile) -> Vec<String> {
    ask_launch_spec(profile).expect("a launch spec").argv
}

fn words(list: &[&str]) -> Vec<String> {
    list.iter().map(|word| word.to_string()).collect()
}

fn text(path: &Path) -> &str {
    path.to_str().expect("a UTF-8 path")
}

fn mode_of(path: &Path) -> u32 {
    fs::metadata(path).unwrap().permissions().mode() & 0o777
}

fn refusal(result: Result<impl std::fmt::Debug, LaunchError>) -> String {
    match result {
        Err(LaunchError::Invalid { message }) => message,
        other => panic!("expected a refusal, got {other:?}"),
    }
}

// ---- the pinned line ---------------------------------------------------------------------------------------------

#[test]
fn the_argv_is_the_pinned_line_word_for_word_for_the_default_entry() {
    let root = TempRoot::new();
    let ask = root.join("ask");
    let expected = vec![
        "-p".to_string(),
        "--output-format".into(),
        "stream-json".into(),
        "--verbose".into(),
        "--model".into(),
        "haiku".into(),
        "--setting-sources".into(),
        "".into(),
        "--settings".into(),
        format!("{}/settings.json", ask.display()),
        "--mcp-config".into(),
        format!("{}/mcp.json", ask.display()),
        "--strict-mcp-config".into(),
        "--permission-prompts".into(),
        "none".into(),
        "--restricted".into(),
        "--tools".into(),
        "Read,Grep,Glob".into(),
        "--allowedTools".into(),
        "Read,Grep,Glob,mcp__collide__query,mcp__collide__set_view,mcp__collide__show_groups,\
         mcp__collide__highlight,mcp__collide__open"
            .into(),
        "--no-session-persistence".into(),
        "--append-system-prompt".into(),
        ASK_PROMPT.to_string(),
    ];
    assert_eq!(argv_of(&profile(&root)), expected);
}

#[test]
fn the_working_directory_is_the_first_readable_folder() {
    let root = TempRoot::new();
    let mut entry = profile(&root);
    entry.read = vec![root.join("code"), root.join("docs")];
    let spec = ask_launch_spec(&entry).unwrap();
    assert_eq!(spec.cwd, root.join("code"));
}

#[test]
fn the_argv_holds_neither_the_ask_token_nor_the_request_text() {
    let root = TempRoot::new();
    let entry = profile(&root);
    write_ask_folder(&entry.dir, ENDPOINT, TOKEN).unwrap();
    let spec = ask_launch_spec(&entry).unwrap();
    assert!(
        spec.argv.len() > 20,
        "a real line, not an empty one: {:?}",
        spec.argv
    );
    for word in &spec.argv {
        assert!(!word.contains(TOKEN), "the token is in argv: {word}");
        assert!(!word.contains("larder"), "request text is in argv: {word}");
        assert!(!word.contains(REQUEST), "request text is in argv: {word}");
    }
    assert!(spec.env.values().all(|value| !value.contains(TOKEN)));
    assert!(spec.env.keys().all(|name| !name.contains(TOKEN)));
}

#[test]
fn the_ask_gets_its_request_on_stdin_so_the_line_has_no_input_format_and_no_prompt_argument() {
    let root = TempRoot::new();
    let argv = argv_of(&profile(&root));
    assert!(!argv.iter().any(|word| word == "--input-format"));
    assert_eq!(
        argv.last().unwrap(),
        ASK_PROMPT,
        "the last word is the fixed prompt"
    );
}

#[test]
fn add_dir_is_passed_once_per_readable_folder_after_the_first() {
    let root = TempRoot::new();
    let folders: Vec<PathBuf> = ["one", "two", "three"]
        .iter()
        .map(|name| root.join(name))
        .collect();
    for count in 1..=3 {
        let mut entry = profile(&root);
        entry.read = folders[..count].to_vec();
        let argv = argv_of(&entry);
        let mut expected = Vec::new();
        for extra in &folders[1..count] {
            expected.push("--add-dir".to_string());
            expected.push(text(extra).to_string());
        }
        let strict = argv
            .iter()
            .position(|word| word == "--strict-mcp-config")
            .unwrap();
        let prompts = argv
            .iter()
            .position(|word| word == "--permission-prompts")
            .unwrap();
        assert_eq!(argv[strict + 1..prompts], expected[..], "{count} folders");
        let flags = argv.iter().filter(|word| *word == "--add-dir").count();
        assert_eq!(flags, count - 1, "{count} folders");
    }
}

#[test]
fn permission_prompts_are_off_and_neither_slash_commands_nor_bare_are_touched() {
    let root = TempRoot::new();
    let argv = argv_of(&profile(&root));
    let at = argv
        .iter()
        .position(|word| word == "--permission-prompts")
        .unwrap();
    assert_eq!(argv[at + 1], "none");
    assert!(!argv.iter().any(|word| word == "--disable-slash-commands"));
    assert!(!argv.iter().any(|word| word == "--bare"));
}

#[test]
fn restricted_follows_permission_prompts_on_its_own_word() {
    let root = TempRoot::new();
    let argv = argv_of(&profile(&root));
    let at = argv
        .iter()
        .position(|word| word == "--permission-prompts")
        .unwrap();
    assert_eq!(
        argv[at..at + 3],
        words(&["--permission-prompts", "none", "--restricted"])[..]
    );
}

#[test]
fn config_user_changes_only_the_setting_sources() {
    let root = TempRoot::new();
    let plain = argv_of(&profile(&root));
    let mut entry = profile(&root);
    entry.config = RoleConfig::User;
    let user = argv_of(&entry);
    assert_eq!(plain.len(), user.len());
    let differing: Vec<usize> = (0..plain.len())
        .filter(|at| plain[*at] != user[*at])
        .collect();
    let sources = plain
        .iter()
        .position(|word| word == "--setting-sources")
        .unwrap();
    assert_eq!(differing, [sources + 1]);
    assert_eq!(plain[sources + 1], "");
    assert_eq!(user[sources + 1], "user");
}

#[test]
fn the_budget_flag_is_on_the_line_only_when_the_entry_sets_a_budget() {
    let root = TempRoot::new();
    let without = argv_of(&profile(&root));
    assert!(!without.iter().any(|word| word == "--max-budget-usd"));
    let mut entry = profile(&root);
    entry.budget_usd = Some(0.25);
    let with = argv_of(&entry);
    assert_eq!(with[..with.len() - 2], without[..]);
    assert_eq!(
        with[with.len() - 2..],
        words(&["--max-budget-usd", "0.25"])[..]
    );
}

#[test]
fn a_budget_that_claude_would_not_enforce_is_refused() {
    let root = TempRoot::new();
    for budget in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        let mut entry = profile(&root);
        entry.budget_usd = Some(budget);
        let message = refusal(ask_launch_spec(&entry));
        assert!(message.contains("budget"), "{budget}: {message}");
    }
}

#[test]
fn a_profile_with_no_folder_a_relative_folder_or_an_unsafe_model_is_refused() {
    let root = TempRoot::new();
    let mut none = profile(&root);
    none.read.clear();
    assert!(refusal(ask_launch_spec(&none)).contains("folder"));

    let mut relative = profile(&root);
    relative.read = vec![root.join("code"), PathBuf::from("-rf")];
    assert!(refusal(ask_launch_spec(&relative)).contains("-rf"));

    for model in ["", "--bare", "a b", "../x"] {
        let mut entry = profile(&root);
        entry.model = model.into();
        assert!(
            refusal(ask_launch_spec(&entry)).contains("model"),
            "{model:?}"
        );
    }
}

// ---- what S6's launch tests checked, ported ----------------------------------------------------------------------

#[test]
fn it_uses_only_the_collide_mcp_server_and_the_cheap_model() {
    let root = TempRoot::new();
    let argv = argv_of(&profile(&root));
    let after = |flag: &str| {
        let at = argv.iter().position(|word| word == flag).unwrap();
        argv[at + 1].clone()
    };
    assert!(argv.iter().any(|word| word == "--strict-mcp-config"));
    assert_eq!(
        after("--mcp-config"),
        format!("{}/mcp.json", root.join("ask").display())
    );
    assert_eq!(after("--model"), "haiku");
    assert!(argv.iter().any(|word| word == "--no-session-persistence"));
    assert_eq!(after("--output-format"), "stream-json");
}

#[test]
fn it_gives_the_agent_reading_tools_and_the_ui_tools_and_no_way_to_run_or_write() {
    let root = TempRoot::new();
    let argv = argv_of(&profile(&root));
    let after = |flag: &str| {
        let at = argv.iter().position(|word| word == flag).unwrap();
        argv[at + 1].clone()
    };
    assert_eq!(after("--tools"), "Read,Grep,Glob");
    let allowed: Vec<String> = after("--allowedTools")
        .split(',')
        .map(String::from)
        .collect();
    let ui = ["query", "set_view", "show_groups", "highlight", "open"]
        .map(|tool| format!("mcp__collide__{tool}"));
    let expected: Vec<String> = ["Read", "Grep", "Glob"]
        .map(String::from)
        .into_iter()
        .chain(ui)
        .collect();
    assert_eq!(allowed, expected);
    assert!(allowed.iter().all(|tool| {
        !["Bash", "Edit", "Write"]
            .iter()
            .any(|bad| tool.contains(bad))
    }));
}

#[test]
fn the_fixed_prompt_says_to_read_first_name_what_was_left_out_and_never_guess_an_id() {
    for phrase in [
        "Glob",
        "Read or Grep",
        "before you group or rank",
        "left out",
        "never guess",
    ] {
        assert!(
            ASK_PROMPT.to_lowercase().contains(&phrase.to_lowercase()),
            "the prompt lacks `{phrase}`"
        );
    }
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
fn the_environment_drops_every_claude_variable_and_keeps_the_rest_of_the_base_environment() {
    let root = TempRoot::new();
    let base = captured(&[
        ("CLAUDECODE", "1"),
        ("CLAUDE_PID", "4242"),
        ("CLAUDE_CODE_ENTRYPOINT", "cli"),
        ("ANTHROPIC_BASE_URL", "https://gateway.example.com"),
        ("PATH", "/usr/bin"),
        ("HOME", "/home/x"),
        ("KEEP", "z"),
    ]);
    let spec = ask_launch_spec(&profile(&root)).unwrap();
    assert_eq!(spec.env_strip, env_strip());
    let child = base.for_child(&spec.env_strip, &spec.env);
    let names: Vec<&str> = child.keys().map(String::as_str).collect();
    assert_eq!(names, ["ANTHROPIC_BASE_URL", "HOME", "KEEP", "PATH"]);
    assert_eq!(child["KEEP"], "z");
}

// ---- the checks before launch ------------------------------------------------------------------------------------

fn with_fake_claude(root: &TempRoot) -> (PathBuf, String) {
    let bin = root.join("bin");
    fs::create_dir(&bin).unwrap();
    let fake = bin.join("claude");
    write_executable(&fake, "#!/bin/sh\nexit 0\n");
    (fake, text(&bin).to_string())
}

#[test]
fn the_command_is_found_on_the_path_it_is_given() {
    let root = TempRoot::new();
    let (fake, path) = with_fake_claude(&root);
    let found = check_before_launch("claude-code", &capabilities(), Some(&path)).unwrap();
    assert_eq!(found, fake);
}

#[test]
fn a_missing_command_is_refused_with_the_plain_text() {
    let root = TempRoot::new();
    let empty = root.join("empty");
    fs::create_dir(&empty).unwrap();
    let message = refusal(check_before_launch(
        "claude-code",
        &capabilities(),
        Some(text(&empty)),
    ));
    assert_eq!(message, "claude was not found on PATH");
    let message = refusal(check_before_launch("claude-code", &capabilities(), None));
    assert_eq!(message, "claude was not found on PATH");
}

#[test]
fn an_agent_without_http_mcp_is_refused_with_the_plain_text_and_the_agent_is_named() {
    let root = TempRoot::new();
    let (_, path) = with_fake_claude(&root);
    for support in [Support::No, Support::Unknown] {
        let mut table = capabilities();
        table.mcp_servers.http = support;
        let message = refusal(check_before_launch("my-agent", &table, Some(&path)));
        assert!(
            message.contains("this agent cannot use HTTP MCP servers"),
            "{message}"
        );
        assert!(message.contains("my-agent"), "{message}");
    }
}

#[test]
fn the_capability_is_checked_before_the_command_is_looked_up() {
    let mut table = capabilities();
    table.mcp_servers.http = Support::No;
    let message = refusal(check_before_launch("my-agent", &table, None));
    assert!(message.contains("HTTP MCP"), "{message}");
}

// ---- the ask folder ----------------------------------------------------------------------------------------------

fn json_at(path: &Path) -> Value {
    serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
}

#[test]
fn the_ask_folder_holds_settings_and_mcp_files_of_mode_0600_and_is_closed_to_others() {
    let root = TempRoot::new();
    let dir = root.join("ask");
    write_ask_folder(&dir, ENDPOINT, TOKEN).unwrap();
    let mut names: Vec<String> = fs::read_dir(&dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    assert_eq!(names, ["mcp.json", "settings.json"]);
    assert_eq!(mode_of(&dir.join("mcp.json")), 0o600);
    assert_eq!(mode_of(&dir.join("settings.json")), 0o600);
    assert_eq!(mode_of(&dir), 0o700);
}

#[test]
fn mcp_json_names_the_collide_server_with_the_endpoint_and_a_bearer_header() {
    let root = TempRoot::new();
    let dir = root.join("ask");
    write_ask_folder(&dir, ENDPOINT, TOKEN).unwrap();
    let expected = json!({
        "mcpServers": {
            "collide": {
                "type": "http",
                "url": ENDPOINT,
                "headers": { "Authorization": format!("Bearer {TOKEN}") }
            }
        }
    });
    assert_eq!(json_at(&dir.join("mcp.json")), expected);
}

#[test]
fn settings_json_turns_auto_memory_off_and_holds_no_token() {
    let root = TempRoot::new();
    let dir = root.join("ask");
    write_ask_folder(&dir, ENDPOINT, TOKEN).unwrap();
    assert_eq!(
        json_at(&dir.join("settings.json")),
        json!({ "autoMemoryEnabled": false })
    );
    assert!(
        !fs::read_to_string(dir.join("settings.json"))
            .unwrap()
            .contains(TOKEN)
    );
}

#[test]
fn a_token_that_would_break_the_json_is_escaped_not_pasted() {
    let root = TempRoot::new();
    let dir = root.join("ask");
    write_ask_folder(&dir, ENDPOINT, "a\"b\\c").unwrap();
    let value = json_at(&dir.join("mcp.json"));
    assert_eq!(
        value["mcpServers"]["collide"]["headers"]["Authorization"],
        "Bearer a\"b\\c"
    );
}

#[test]
fn an_empty_endpoint_or_token_is_refused_and_writes_nothing() {
    let root = TempRoot::new();
    for (endpoint, token) in [("", TOKEN), (ENDPOINT, "")] {
        let dir = root.join("ask");
        let error = write_ask_folder(&dir, endpoint, token)
            .expect_err("a refusal")
            .to_string();
        assert!(error.contains("empty"), "{error}");
        assert!(!dir.join("mcp.json").exists());
    }
}

#[test]
fn removing_the_folder_removes_it_with_its_token_and_nothing_beside_it() {
    let root = TempRoot::new();
    let dir = root.join("ask");
    let sibling = root.join("ask-other");
    write_ask_folder(&dir, ENDPOINT, TOKEN).unwrap();
    write_ask_folder(&sibling, ENDPOINT, "another-ask-token").unwrap();
    fs::write(root.join("beside.txt"), "kept").unwrap();

    remove_ask_folder(&dir).unwrap();

    assert!(!dir.exists(), "the folder is gone");
    assert!(!dir.join("mcp.json").exists(), "and the token with it");
    assert!(
        sibling.join("mcp.json").exists(),
        "a folder beside it stays"
    );
    assert_eq!(fs::read_to_string(root.join("beside.txt")).unwrap(), "kept");
}

#[test]
fn removing_a_folder_that_is_not_there_is_not_an_error() {
    let root = TempRoot::new();
    remove_ask_folder(&root.join("never-made")).unwrap();
}

#[test]
fn a_relative_path_is_refused_by_the_remove_and_removes_nothing() {
    let root = TempRoot::new();
    let dir = root.join("ask");
    write_ask_folder(&dir, ENDPOINT, TOKEN).unwrap();
    let error = remove_ask_folder(Path::new("ask")).expect_err("a refusal");
    assert!(error.to_string().contains("absolute"), "{error}");
    assert!(dir.join("mcp.json").exists());
}

#[test]
fn a_symlink_is_not_followed_by_the_remove() {
    let root = TempRoot::new();
    let real = root.join("real");
    write_ask_folder(&real, ENDPOINT, TOKEN).unwrap();
    let link = root.join("link");
    std::os::unix::fs::symlink(&real, &link).unwrap();
    assert!(remove_ask_folder(&link).is_err());
    assert!(
        real.join("mcp.json").exists(),
        "what the link points at is untouched"
    );
}
