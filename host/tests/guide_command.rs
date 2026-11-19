//! `collide guide` through the binary (ts-0140-gk): it prints the guide with no daemon, nothing written and an empty
//! `HOME`. Each run clears the environment and puts `HOME` and `COLLIDE_DAEMON_ROOT` in a temp root, so it can reach
//! neither the real home nor the real daemon.

use collide_host::commands::{self, COMMANDS};
use collide_host::guide::{LISTED_COMMANDS, TASK_FORMS};
use std::process::{Command, Output};

mod common;
use common::TempRoot;

fn collide(root: &TempRoot, args: &[&str]) -> Output {
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

fn stdout_of(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).unwrap()
}

#[test]
fn guide_prints_the_commands_the_forms_and_the_exit_codes_and_exits_0() {
    let root = TempRoot::new();
    let output = collide(&root, &["guide"]);
    let stdout = stdout_of(&output);
    assert_eq!(output.status.code(), Some(0), "{stdout}");
    for name in LISTED_COMMANDS {
        let summary = commands::find(name).unwrap().summary;
        assert!(
            stdout.contains(&format!("collide {name}: {summary}")),
            "{name} in {stdout}"
        );
    }
    for form in &TASK_FORMS {
        assert!(stdout.contains(form.usage), "{} in {stdout}", form.usage);
    }
    for code in ["10", "11", "12"] {
        assert!(
            stdout
                .lines()
                .any(|line| line.trim_start().starts_with(&format!("{code} "))),
            "exit code {code} in {stdout}"
        );
    }
    assert!(
        stdout.contains("accept, defer, reject, resolve, reopen"),
        "{stdout}"
    );
}

#[test]
fn guide_starts_no_daemon_and_writes_nothing() {
    let root = TempRoot::new();
    let output = collide(&root, &["guide"]);
    assert_eq!(output.status.code(), Some(0));
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let left: Vec<_> = std::fs::read_dir(root.path()).unwrap().flatten().collect();
    assert!(left.is_empty(), "the daemon root and HOME hold {left:?}");
}

#[test]
fn guide_holds_no_path_and_nothing_of_the_machine_it_runs_on() {
    let root = TempRoot::new();
    let stdout = stdout_of(&collide(&root, &["guide"]));
    assert!(!stdout.is_empty());
    assert!(!stdout.contains('/'), "{stdout}");
    assert!(!stdout.contains(root.path().to_str().unwrap()), "{stdout}");
    assert_eq!(stdout, collide_host::guide::text());
}

#[test]
fn guide_takes_no_argument() {
    let root = TempRoot::new();
    let output = collide(&root, &["guide", "now"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("takes no argument"));
}

#[test]
fn help_lists_guide_with_its_summary() {
    let root = TempRoot::new();
    let stdout = stdout_of(&collide(&root, &["--help"]));
    let summary = COMMANDS.iter().find(|c| c.name == "guide").unwrap().summary;
    assert!(
        stdout
            .lines()
            .any(|line| line.trim_start().starts_with("guide") && line.contains(summary)),
        "{stdout}"
    );
}
