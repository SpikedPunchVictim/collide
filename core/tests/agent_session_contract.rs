//! The `AgentSession` contract (ts-0019-vm, ts-0022-mc): one suite, written once in
//! `collide_core::testing::contract`, runs against both session doubles, so a session that breaks the contract
//! fails the same test whichever protocol it speaks.
//!
//! | double | speaks |
//! |---|---|
//! | [`NormalizedSession`] (the fake adapter's) | one normalized event as JSON on each line |
//! | [`AcpSession`] | the ACP handshake, request ids and permission requests (S7) |
//!
//! The suite drives `start`, `feed`, `send_turn`, `answer`, `cancel`, `finish` and `replay` with fixed `wall_ns`
//! values. Nothing here reads a clock or does I/O, which is what "sans-IO" means; the last test scans `core/src`
//! to keep it so.

use collide_core::adapter::AgentSession;
use collide_core::session_contract_tests;
use collide_core::testing::{AcpDialect, AcpSession, NormalizedDialect, NormalizedSession};
use std::path::{Path, PathBuf};

session_contract_tests!(normalized_session, NormalizedDialect);
session_contract_tests!(acp_session, AcpDialect);

#[test]
fn both_doubles_are_sessions_a_host_can_hold_as_trait_objects() {
    let sessions: Vec<Box<dyn AgentSession>> = vec![
        Box::new(NormalizedSession::new()),
        Box::new(AcpSession::new("/work", "do the task")),
    ];
    assert_eq!(sessions.len(), 2);
}

/// Every `.rs` file below `dir`, in name order.
fn rust_files(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            found.extend(rust_files(&path));
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            found.push(path);
        }
    }
    found
}

/// The lines of the code below `dir` (comments left out) that contain one of `names`, as `path:line`.
fn uses_of(dir: &Path, names: &[String]) -> Vec<String> {
    let mut found = Vec::new();
    for path in rust_files(dir) {
        for (number, line) in std::fs::read_to_string(&path).unwrap().lines().enumerate() {
            if line.trim_start().starts_with("//") {
                continue;
            }
            if names.iter().any(|name| line.contains(name.as_str())) {
                found.push(format!("{}:{}", path.display(), number + 1));
            }
        }
    }
    found
}

fn core_src() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

#[test]
fn core_reads_no_clock() {
    // The names are joined at run time so that this test does not match itself.
    let forbidden = [
        ["System", "Time"].concat(),
        ["Instant", "::now"].concat(),
        ["UNIX", "_EPOCH"].concat(),
    ];
    let found = uses_of(&core_src(), &forbidden);
    assert!(found.is_empty(), "core reads a clock at {found:?}");
}

#[test]
fn the_doubles_use_no_files_no_processes_no_threads_and_no_randomness() {
    let forbidden = [
        ["std::", "fs"].concat(),
        ["std::", "process"].concat(),
        ["std::", "net"].concat(),
        ["std::", "thread"].concat(),
        ["std::", "env"].concat(),
        ["rand", "::"].concat(),
        ["Random", "State"].concat(),
        ["Hash", "Map"].concat(),
    ];
    let found = uses_of(&core_src().join("testing"), &forbidden);
    assert!(found.is_empty(), "a double does I/O or varies at {found:?}");
}
