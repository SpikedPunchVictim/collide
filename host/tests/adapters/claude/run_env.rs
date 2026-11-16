//! Collide's tests pass inside a Collide run (ts-0074-cu). A phase's agent runs `cargo test --workspace` with the
//! run's marker variables in its environment, and the daemon decides who is asking by walking the peer's process tree
//! and reading each process's environment (`COLLIDE_RUN` makes a run's agent, which may not start a run). A test
//! process is part of that tree, and the system's answer for it is the environment it was started with, so a test that
//! starts a run must not be seen as the agent of the run it sits in. These tests run two representative tests, one
//! with a daemon in the test's own process and one with a daemon process and the `collide` command, as children of
//! this one with the four variables set, and check each passes.
//!
//! Each child is this test binary again with `--exact` on one other test, so it cannot reach these tests and needs no
//! guard against recursion. Its environment is cleared and holds only `PATH` (the tests' own children clear theirs)
//! and the four variables; the tests it runs use a temp root for `HOME` and `COLLIDE_DAEMON_ROOT`.

use std::process::{Command, Output, Stdio};

/// The variables a phase has: `COLLIDE_RUN`, `COLLIDE_RUN_START`, `COLLIDE_SPAN` and `CLAUDE_CODE_AGENT`.
const PHASE_VARIABLES: [(&str, &str); 4] = [
    ("COLLIDE_RUN", "run-x"),
    ("COLLIDE_RUN_START", "1"),
    ("COLLIDE_SPAN", "1"),
    ("CLAUDE_CODE_AGENT", "1"),
];

/// Runs the test `name` of this binary as a child inside a phase's environment.
fn run_inside_a_phase(name: &str) -> Output {
    Command::new(std::env::current_exe().expect("the test binary's own path"))
        .args(["--exact", name, "--test-threads=1"])
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .envs(PHASE_VARIABLES)
        .stdin(Stdio::null())
        .output()
        .expect("the test binary runs")
}

fn passes_inside_a_phase(name: &str) {
    let output = run_inside_a_phase(name);
    let text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.status.success(),
        "{name} failed inside a phase:\n{text}"
    );
    assert!(
        text.contains("test result: ok. 1 passed"),
        "{name} did not run exactly once:\n{text}"
    );
}

#[test]
fn a_run_started_through_a_daemon_in_the_tests_process_works_inside_a_phase() {
    passes_inside_a_phase(
        "run_command::with_no_published_source_only_the_version_in_use_is_printed",
    );
}

#[test]
fn a_run_started_by_the_command_through_a_daemon_process_works_inside_a_phase() {
    passes_inside_a_phase(
        "run_process::a_run_through_a_daemon_the_command_starts_prints_the_version_and_the_record_and_exits_0",
    );
}
