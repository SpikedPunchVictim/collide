//! The command tree of `collide` (ts-0041-xf): `--help` lists it, and a command that is not built yet exits with a
//! message that names its tag and stage, not a panic backtrace. The built commands (`schema`, `tasks`, `task`,
//! `sections`, `stop`, `run`) are tested in their own files.

use serde_json::Value;
use std::path::Path;
use std::process::{Command, Output};

mod common;
use common::{TempRoot, repo_root};

const TREE: [&str; 11] = [
    "run", "stop", "tasks", "task", "schema", "report", "config", "pack", "init", "trust",
    "sections",
];

/// The commands whose handler is a tagged stub (`stop` is built: `tests/stop_command.rs`; `run` is built:
/// `tests/adapters/claude/run_command.rs` and `run_process.rs`; `config` is built: `tests/config_process.rs`, which
/// runs it with an environment of its own; `pack` is built: `tests/pack_update.rs` and `tests/user_packs.rs`; `init`
/// and `trust` are built: `tests/board_cli.rs` and `tests/board_process.rs`), with the
/// tag and stage that fills each.
const STUBBED: [(&str, &str, &str); 1] = [("report", "cli: report", "7")];

/// Runs `collide` in a temp root that is also its `HOME` and its daemon root, so a stub that has since been built
/// never reaches the real home or daemon.
fn collide(args: &[&str]) -> Output {
    let root = TempRoot::new();
    Command::new(env!("CARGO_BIN_EXE_collide"))
        .args(args)
        .current_dir(root.path())
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", root.path())
        .env("COLLIDE_DAEMON_ROOT", root.path())
        .output()
        .expect("collide should run")
}

fn text(bytes: Vec<u8>) -> String {
    String::from_utf8(bytes).unwrap()
}

#[test]
fn help_lists_every_command_of_the_tree_with_a_line_of_its_own() {
    let output = collide(&["--help"]);
    assert_eq!(output.status.code(), Some(0));
    let stdout = text(output.stdout);
    let listed: Vec<&str> = stdout
        .lines()
        .skip_while(|line| *line != "commands:")
        .skip(1)
        .take_while(|line| line.starts_with("  "))
        .filter_map(|line| line.split_whitespace().next())
        .collect();
    assert_eq!(listed, TREE, "{stdout}");
}

#[test]
fn help_keeps_the_text_of_the_built_commands() {
    let stdout = text(collide(&["--help"]).stdout);
    for kept in [
        "collide tasks [--ready] [--kind <kind>] [--view <name>] [--checkout <path>] [--json]",
        "collide task <id> [--json]",
        "collide sections [--json]",
        "exit codes: 0 ok, 1 failed",
        "collide run <task-file> [--budget <usd>] [--alias <name>] [--config none|user]",
        "collide run --resume <run>",
    ] {
        assert!(stdout.contains(kept), "{kept} in {stdout}");
    }
}

#[test]
fn help_lists_every_exit_code_of_collide_run() {
    let stdout = text(collide(&["--help"]).stdout);
    for code in [
        "0 the phase ended",
        "1 failed to do its work",
        "2 wrong command line",
        "3 no such task file or run",
        "5 the daemon refused to start the run",
        "7 the run failed",
        "8 the run was stopped",
        "9 the run is waiting for a person",
    ] {
        assert!(stdout.contains(code), "{code} in {stdout}");
    }
    // The codes are the constants the command exits with.
    use collide_host::task_cli::exit;
    assert_eq!(
        [
            exit::OK,
            exit::FAILED,
            exit::USAGE,
            exit::ID,
            exit::REFUSED,
            exit::RUN_FAILED,
            exit::RUN_STOPPED,
            exit::WAITING
        ],
        [0, 1, 2, 3, 5, 7, 8, 9]
    );
}

#[test]
fn help_says_which_commands_are_not_built_yet_and_the_exit_code_they_give() {
    let stdout = text(collide(&["help"]).stdout);
    for (name, _, stage) in STUBBED {
        let line = stdout
            .lines()
            .find(|line| line.trim_start().starts_with(name) && line.contains("not built yet"))
            .unwrap_or_else(|| panic!("no unbuilt line for {name} in {stdout}"));
        assert!(line.contains(&format!("Stage {stage}")), "{line}");
    }
    assert!(stdout.contains("exits 6"), "{stdout}");
}

#[test]
fn a_stubbed_command_exits_6_with_one_line_naming_its_tag_and_stage() {
    for (name, tag, stage) in STUBBED {
        let output = collide(&[name]);
        assert_eq!(output.status.code(), Some(6), "{name}");
        assert!(output.stdout.is_empty(), "{name}");
        let stderr = text(output.stderr);
        assert_eq!(
            stderr,
            format!("collide {name}: not built yet ({tag}, Stage {stage})\n")
        );
    }
}

#[test]
fn a_stubbed_command_with_arguments_still_names_its_tag_and_prints_no_backtrace() {
    let output = collide(&["report", "span", "somewhere", "--json"]);
    assert_eq!(output.status.code(), Some(6));
    let stderr = text(output.stderr);
    assert!(stderr.contains("(cli: report, Stage 7)"), "{stderr}");
    assert!(!stderr.contains("panicked"), "{stderr}");
    assert!(!stderr.contains("RUST_BACKTRACE"), "{stderr}");
}

#[test]
fn pack_update_is_built_and_a_bad_command_line_for_it_is_a_usage_error_not_a_stub() {
    for args in [
        &["pack", "update"][..],
        &["pack", "update", "a", "b"][..],
        &["pack", "update", "a", "--nope"][..],
    ] {
        let output = collide(args);
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        let stderr = text(output.stderr);
        assert!(
            stderr.contains("usage: collide pack update <name>"),
            "{stderr}"
        );
        assert!(!stderr.contains("not built yet"), "{stderr}");
    }
}

#[test]
fn help_names_the_pack_update_command() {
    let stdout = text(collide(&["--help"]).stdout);
    assert!(
        stdout.contains("usage: collide pack update <name> [--to <ref>]"),
        "{stdout}"
    );
}

#[test]
fn an_unknown_command_is_still_a_usage_error_that_lists_the_whole_tree() {
    let output = collide(&["frobnicate"]);
    assert_eq!(output.status.code(), Some(2));
    let stderr = text(output.stderr);
    assert_eq!(stderr.lines().count(), 1, "{stderr}");
    for name in TREE {
        assert!(stderr.contains(name), "{name} in {stderr}");
    }
}

#[test]
fn the_tags_and_stages_the_binary_prints_are_the_ones_the_registry_holds() {
    let registry: Value = serde_json::from_str(
        &std::fs::read_to_string(Path::new(&repo_root()).join("stub-tags.json")).unwrap(),
    )
    .unwrap();
    for (name, tag, stage) in STUBBED {
        assert_eq!(
            registry[tag].as_str(),
            Some(stage),
            "{name}: `{tag}` is not registered under Stage {stage}"
        );
    }
}

/// The commands the owner's JSON-RPC does not carry: `schema` reads the compiled contract and `report` is the
/// hook's own channel to its run.
const NOT_OWNER_METHODS: [&str; 2] = ["schema", "report"];

#[test]
fn every_command_the_daemon_does_work_for_is_served_by_an_owner_method() {
    use collide_core::owner::Method;
    for command in TREE {
        if NOT_OWNER_METHODS.contains(&command) {
            continue;
        }
        let prefix = format!("collide {command}");
        assert!(
            Method::ALL.iter().any(|method| {
                let serves = method.serves();
                serves == prefix || serves.starts_with(&format!("{prefix},"))
            }),
            "no owner method serves `{prefix}`"
        );
    }
}
