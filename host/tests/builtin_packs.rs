//! Tests for the built-in pack loader and the bundle it reads (ts-0029-ea). The bundle is built here, by the
//! repo's build script into a scratch folder under `target/`, so no test depends on a staged copy and none skips.

use collide_host::builtin_packs::{BuiltinPackError, BuiltinPacks, Environment};
use serde_json::{Value, json};
use std::ffi::OsString;
use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

mod common;
use common::{Bundle, Scratch, build_packs, repo_root, scratch};

fn bundle() -> Bundle {
    common::bundle("builtin-packs")
}

fn packs(bundle: &Bundle) -> BuiltinPacks {
    BuiltinPacks::new(&bundle.0)
}

/// A folder holding an executable `node` (a stand-in; it is never run) and a folder with none.
fn fake_path(name: &str) -> (OsString, PathBuf, PathBuf) {
    let base = scratch(name);
    let _ = fs::remove_dir_all(&base);
    let (with_node, without) = (base.join("with-node"), base.join("without"));
    fs::create_dir_all(&with_node).unwrap();
    fs::create_dir_all(&without).unwrap();
    let node = with_node.join("node");
    fs::write(&node, "#!/bin/sh\n").unwrap();
    fs::set_permissions(&node, fs::Permissions::from_mode(0o755)).unwrap();
    let path = std::env::join_paths([&without, &with_node]).unwrap();
    (path, without, with_node)
}

// The loader.

#[test]
fn the_loader_returns_the_pack_directory_and_node_from_the_given_path_with_the_compiled_entry() {
    let bundle = bundle();
    let (path, _, with_node) = fake_path("loader-found");
    let launch = packs(&bundle)
        .provider("collide/plan", "plan", &Environment { path })
        .unwrap();
    assert_eq!(launch.pack_dir, bundle.join("plan"));
    assert_eq!(launch.program, with_node.join("node"));
    assert_eq!(
        launch.args,
        vec![bundle.join("plan/src/main.js").into_os_string()]
    );
}

#[test]
fn a_node_that_is_not_executable_is_not_found() {
    let bundle = bundle();
    let base = scratch("loader-not-executable");
    let _ = fs::remove_dir_all(&base);
    fs::create_dir_all(&base).unwrap();
    fs::write(base.join("node"), "#!/bin/sh\n").unwrap();
    fs::set_permissions(base.join("node"), fs::Permissions::from_mode(0o644)).unwrap();
    let path = std::env::join_paths([&base]).unwrap();
    let error = packs(&bundle)
        .provider("collide/plan", "plan", &Environment { path })
        .unwrap_err();
    assert!(matches!(error, BuiltinPackError::NodeNotFound { .. }));
}

#[test]
fn with_no_node_on_the_path_the_error_names_the_pack_the_version_and_where_it_looked() {
    let bundle = bundle();
    let (_, without, _) = fake_path("loader-missing");
    let other = scratch("loader-missing").join("empty-too");
    fs::create_dir_all(&other).unwrap();
    let path = std::env::join_paths([&without, &other]).unwrap();
    let error = packs(&bundle)
        .provider("collide/plan", "plan", &Environment { path })
        .unwrap_err();
    assert_eq!(
        error.to_string(),
        format!(
            "built-in pack collide/plan needs `node` >=22.18 to start its provider `plan`, \
             and `node` was not found on PATH (searched: {}, {})",
            without.display(),
            other.display()
        )
    );
}

#[test]
fn an_empty_path_says_so() {
    let bundle = bundle();
    let error = packs(&bundle)
        .provider(
            "collide/plan",
            "plan",
            &Environment {
                path: OsString::new(),
            },
        )
        .unwrap_err();
    assert!(
        error
            .to_string()
            .ends_with("(searched: nothing, PATH is empty)"),
        "{error}"
    );
}

#[test]
fn an_unknown_pack_and_a_pack_without_that_provider_are_errors() {
    let bundle = bundle();
    let (path, _, _) = fake_path("loader-unknown");
    let env = Environment { path };
    let unknown = packs(&bundle)
        .provider("collide/nope", "plan", &env)
        .unwrap_err();
    assert_eq!(unknown.to_string(), "`collide/nope` is not a built-in pack");
    let none = packs(&bundle)
        .provider("collide", "plan", &env)
        .unwrap_err();
    assert_eq!(
        none.to_string(),
        "built-in pack collide has no provider `plan`"
    );
}

// The bundle.

fn files_under(dir: &Path, found: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        found.push(path.clone());
        if path.is_dir() {
            files_under(&path, found);
        }
    }
}

fn everything_under(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    files_under(dir, &mut found);
    found
}

#[test]
fn the_bundle_holds_both_packs_the_compiled_provider_and_its_own_sdk_copy() {
    let bundle = bundle();
    let plan = bundle.join("plan");
    assert!(bundle.join("collide/pack.yaml").is_file());
    assert!(plan.join("pack.yaml").is_file());
    for compiled in ["main", "provider", "scan", "item", "read-file"] {
        assert!(
            plan.join(format!("src/{compiled}.js")).is_file(),
            "{compiled}.js"
        );
    }
    assert!(
        plan.join("node_modules/@collide/pack-sdk/package.json")
            .is_file()
    );
    assert!(
        plan.join("node_modules/@collide/pack-sdk/dist/index.js")
            .is_file()
    );
    assert!(plan.join("node_modules/yaml/package.json").is_file());
    let main = fs::read_to_string(plan.join("src/main.js")).unwrap();
    assert!(
        main.contains("./provider.js"),
        "imports are rewritten: {main}"
    );
}

#[test]
fn no_typescript_file_is_in_the_bundle_and_the_sdk_copy_has_none_either() {
    let bundle = bundle();
    let ts: Vec<_> = everything_under(&bundle)
        .into_iter()
        .filter(|path| path.extension().is_some_and(|ext| ext == "ts"))
        .collect();
    assert!(ts.is_empty(), "found: {ts:?}");
    let sdk = bundle.join("plan/node_modules/@collide/pack-sdk");
    assert!(everything_under(&sdk).iter().any(|path| path.is_file()));
}

#[test]
fn the_bundle_has_no_write_bit_and_the_test_process_cannot_write_into_it() {
    let bundle = bundle();
    for path in everything_under(&bundle) {
        let mode = fs::symlink_metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o222, 0, "{} is writable ({mode:o})", path.display());
    }
    assert!(fs::write(bundle.join("plan/new.txt"), "x").is_err());
    assert!(fs::write(bundle.join("plan/pack.yaml"), "x").is_err());
}

#[test]
fn a_rebuild_replaces_a_read_only_previous_output() {
    let out = Scratch(scratch("builtin-packs-rebuilt"));
    build_packs(&out);
    build_packs(&out);
    assert!(out.join("plan/src/main.js").is_file());
    assert_eq!(
        fs::metadata(out.join("plan")).unwrap().permissions().mode() & 0o222,
        0
    );
    assert!(!out.with_extension("staging").exists());
}

// The provider, started the way the loader says.

fn exchange(command: &mut Command, requests: &[Value]) -> Vec<Value> {
    let mut child = command
        .current_dir(std::env::temp_dir())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the provider should start");
    let mut stdin = child.stdin.take().unwrap();
    for request in requests {
        writeln!(stdin, "{request}").unwrap();
    }
    drop(stdin);
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "provider exited with {}: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

#[test]
fn the_bundled_provider_answers_initialize_and_list_over_a_fixture_root() {
    let bundle = bundle();
    let launch = packs(&bundle)
        .provider("collide/plan", "plan", &Environment::from_process())
        .expect("node is on the PATH of the test process");
    let root = repo_root().join("packs/plan/test/fixtures/basic");
    let replies = exchange(
        &mut launch.command(),
        &[
            json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": { "config": {}, "root": root } }),
            json!({ "jsonrpc": "2.0", "id": 2, "method": "list", "params": {} }),
        ],
    );
    assert_eq!(replies.len(), 2);
    assert_eq!(replies[0]["result"]["scope"], "ref");
    let mut ids: Vec<&str> = replies[1]["result"]["tasks"]
        .as_array()
        .expect("list returns tasks")
        .iter()
        .map(|task| task["id"].as_str().unwrap())
        .collect();
    ids.sort();
    assert_eq!(
        ids,
        [
            "ep-001-aa",
            "ep-002-bb",
            "ts-0001-aa",
            "ts-0002-bb",
            "ts-0003-cc",
            "ts-0004-dd"
        ]
    );
}
