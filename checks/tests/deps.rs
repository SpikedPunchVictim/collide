//! Positive controls for the dependency-direction check (0.15): `core` may not reach a Tauri, SQLite or
//! process-spawning crate, directly or through another crate.

mod support;

use collide_checks::deps;
use support::Tree;

const CLEAN_LOCK: &str = r#"
version = 4

[[package]]
name = "collide-core"
version = "0.0.0"
dependencies = [
 "serde",
]

[[package]]
name = "serde"
version = "1.0.0"

[[package]]
name = "collide-host"
version = "0.0.0"
dependencies = [
 "collide-core",
 "rusqlite",
]

[[package]]
name = "rusqlite"
version = "0.30.0"
"#;

fn lock_with(extra_core_dependency: &str, extra_packages: &str) -> String {
    CLEAN_LOCK.replacen(
        " \"serde\",\n",
        &format!(" \"serde\",\n \"{extra_core_dependency}\",\n"),
        1,
    ) + extra_packages
}

#[test]
fn a_lock_where_only_host_has_sqlite_passes() {
    assert_eq!(deps::forbidden_in_core(CLEAN_LOCK), Vec::<String>::new());
}

#[test]
fn a_direct_tauri_dependency_fails() {
    let lock = lock_with(
        "tauri",
        "\n[[package]]\nname = \"tauri\"\nversion = \"2.0.0\"\n",
    );
    let found = deps::forbidden_in_core(&lock);
    assert_eq!(found.len(), 1);
    assert!(found[0].contains("collide-core -> tauri"), "{found:?}");
}

#[test]
fn a_direct_sqlite_dependency_fails() {
    let lock = lock_with("rusqlite", "");
    assert_eq!(deps::forbidden_in_core(&lock).len(), 1);
}

#[test]
fn a_process_spawning_dependency_fails() {
    let lock = lock_with(
        "duct",
        "\n[[package]]\nname = \"duct\"\nversion = \"1.0.0\"\n",
    );
    assert_eq!(deps::forbidden_in_core(&lock).len(), 1);
}

#[test]
fn a_forbidden_crate_reached_through_another_crate_fails_and_shows_the_chain() {
    let lock = lock_with(
        "helper",
        "\n[[package]]\nname = \"helper\"\nversion = \"1.0.0\"\ndependencies = [\n \"tauri-runtime 2.0.0\",\n]\n\n\
         [[package]]\nname = \"tauri-runtime\"\nversion = \"2.0.0\"\n",
    );
    let found = deps::forbidden_in_core(&lock);
    assert_eq!(found.len(), 1);
    assert!(
        found[0].contains("collide-core -> helper -> tauri-runtime"),
        "{found:?}"
    );
}

#[test]
fn a_dependency_with_a_version_and_source_is_read_by_name() {
    let lock = lock_with(
        "libsqlite3-sys 0.28.0 (registry+https://github.com/rust-lang/crates.io-index)",
        "",
    );
    assert_eq!(deps::forbidden_in_core(&lock).len(), 1);
}

#[test]
fn a_dependency_cycle_does_not_hang_the_walk() {
    let lock = lock_with(
        "a",
        "\n[[package]]\nname = \"a\"\nversion = \"1.0.0\"\ndependencies = [\n \"collide-core\",\n]\n",
    );
    assert_eq!(deps::forbidden_in_core(&lock), Vec::<String>::new());
}

#[test]
fn a_lock_with_no_core_package_is_an_error_not_a_pass() {
    let tree = Tree::new();
    tree.write("Cargo.lock", "version = 4\n");
    tree.write("core/src/lib.rs", "");
    assert!(deps::check(tree.root()).is_err());
}

#[test]
fn spawning_a_process_from_the_standard_library_in_core_fails() {
    let tree = Tree::new();
    tree.write("Cargo.lock", CLEAN_LOCK);
    tree.write(
        "core/src/lib.rs",
        "pub fn f() { let _ = std::process::Command::new(\"ls\"); }\n",
    );
    let found = deps::check(tree.root()).unwrap();
    assert_eq!(found.len(), 1);
    assert!(found[0].starts_with("core/src/lib.rs:1:"), "{found:?}");
}

#[test]
fn a_clean_core_passes() {
    let tree = Tree::new();
    tree.write("Cargo.lock", CLEAN_LOCK);
    tree.write("core/src/lib.rs", "pub fn f() {}\n");
    assert_eq!(deps::check(tree.root()).unwrap(), Vec::<String>::new());
}
