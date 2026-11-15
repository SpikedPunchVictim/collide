//! The three checks over this repository itself. They run with `cargo test --workspace`, so a change that breaks one
//! fails the same test run as the unit tests; CI also runs each one on its own to print the stub counts.

use collide_checks::{deps, names, stubs};
use std::path::{Path, PathBuf};

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

#[test]
fn no_name_check_passes_on_the_repository() {
    let found = names::check(&repo()).unwrap();
    assert!(
        found.is_empty(),
        "{}",
        found
            .iter()
            .map(|v| v.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn core_has_no_tauri_sqlite_or_process_dependency() {
    let found = deps::check(&repo()).unwrap();
    assert!(found.is_empty(), "{}", found.join("\n"));
}

#[test]
fn every_stub_of_the_repository_is_registered_under_a_stage() {
    let report = stubs::check(&repo()).unwrap();
    assert!(report.problems.is_empty(), "{}", report.problems.join("\n"));
}
