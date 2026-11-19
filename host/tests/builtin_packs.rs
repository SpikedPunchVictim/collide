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

#[test]
fn a_built_in_manifest_that_fails_the_manifest_check_is_refused_with_every_reason() {
    let root = scratch("loader-invalid-manifest");
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("plan")).unwrap();
    fs::write(
        root.join("plan/pack.yaml"),
        "apiVersion: collide/v0\nname: collide/plan\nproviders:\n  plan: { script: src/main.ts }\n\
         gates:\n  g: { script: /usr/bin/check }\n\
         bindings:\n  - { point: run.after_end, gate: g }\n",
    )
    .unwrap();
    let (path, _, _) = fake_path("loader-invalid-manifest-path");
    let error = BuiltinPacks::new(&root)
        .provider("collide/plan", "plan", &Environment { path })
        .unwrap_err();
    let text = error.to_string();
    assert!(
        matches!(error, BuiltinPackError::InvalidManifest { .. }),
        "{text}"
    );
    assert!(text.contains("built-in pack collide/plan"), "{text}");
    assert!(
        text.contains("/usr/bin/check") && text.contains("run.after_end"),
        "{text}"
    );
    let _ = fs::remove_dir_all(&root);
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
fn the_sdk_copy_in_the_bundle_carries_yaml_in_its_own_node_modules_as_real_files() {
    let bundle = bundle();
    let yaml = bundle.join("plan/node_modules/@collide/pack-sdk/node_modules/yaml");
    let package: serde_json::Value =
        serde_json::from_slice(&fs::read(yaml.join("package.json")).unwrap()).unwrap();
    assert_eq!(package["version"], "2.9.1");
    for path in everything_under(&yaml) {
        let kind = fs::symlink_metadata(&path).unwrap().file_type();
        assert!(!kind.is_symlink(), "{} is a symlink", path.display());
    }
    assert!(
        bundle
            .join("plan/node_modules/@collide/pack-sdk/dist/yaml.js")
            .is_file()
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
            "pr-0001-aa",
            "qu-0001-aa",
            "sp-0001-aa",
            "ts-0001-aa",
            "ts-0002-bb",
            "ts-0003-cc",
            "ts-0004-dd"
        ]
    );
}

// ---- a test that needs `node` and cannot find it (ts-0074-cu) -----------------------------------------------------

#[test]
fn the_message_for_a_missing_node_names_node_and_every_folder_searched() {
    let (path, without, _) = fake_path("test-missing-node");
    let other = scratch("test-missing-node").join("empty-too");
    fs::create_dir_all(&other).unwrap();
    let path_without = std::env::join_paths([&without, &other]).unwrap();
    assert_eq!(common::node_missing_message(&path), None);
    let message = common::node_missing_message(&path_without).expect("no node there");
    assert!(message.contains("`node`"), "{message}");
    assert!(
        message.contains(&format!(
            "searched: {}, {}",
            without.display(),
            other.display()
        )),
        "{message}"
    );
}

#[test]
fn a_test_that_needs_node_fails_naming_node_and_the_path_when_the_path_has_none() {
    let empty = scratch("test-no-node-path");
    let _ = fs::remove_dir_all(&empty);
    fs::create_dir_all(&empty).unwrap();
    // This binary again, on one other test that builds the bundle, with a `PATH` that holds nothing.
    let output = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "the_bundle_holds_both_packs_the_compiled_provider_and_its_own_sdk_copy",
        ])
        .env_clear()
        .env("PATH", &empty)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    let text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !output.status.success(),
        "it was skipped or passed:\n{text}"
    );
    assert!(text.contains("needs `node`"), "{text}");
    assert!(
        text.contains(&format!("searched: {}", empty.display())),
        "{text}"
    );
    let _ = fs::remove_dir_all(&empty);
}

// ---- the names, the manifest and the folder of a built-in pack, and tool lookup (ts-0096-ph) ------------------------

#[test]
fn the_built_in_names_are_collide_then_collide_plan() {
    assert_eq!(BuiltinPacks::names(), ["collide", "collide/plan"]);
}

#[test]
fn the_manifest_of_each_built_in_pack_loads_with_its_own_name() {
    let bundle = bundle();
    let packs = packs(&bundle);
    for name in BuiltinPacks::names() {
        assert_eq!(packs.manifest(name).unwrap().name, name);
    }
}

#[test]
fn the_folder_of_a_built_in_pack_is_its_folder_in_the_bundle() {
    let bundle = bundle();
    let packs = packs(&bundle);
    assert_eq!(packs.dir("collide").unwrap(), bundle.join("collide"));
    assert_eq!(packs.dir("collide/plan").unwrap(), bundle.join("plan"));
}

#[test]
fn the_manifest_and_folder_of_a_pack_that_is_not_built_in_are_refused_by_name() {
    let bundle = bundle();
    let packs = packs(&bundle);
    let error = packs.manifest("ferry").unwrap_err().to_string();
    assert!(error.contains("`ferry` is not a built-in pack"), "{error}");
    assert!(matches!(
        packs.dir("ferry"),
        Err(BuiltinPackError::UnknownPack(_))
    ));
}

fn tool_in(root: &common::TempRoot, folder: &str, name: &str, mode: u32) -> PathBuf {
    let dir = root.join(folder);
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    fs::write(&path, "#!/bin/sh\n").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(mode)).unwrap();
    path
}

#[test]
fn a_tool_is_found_in_the_first_path_folder_that_has_it_as_an_executable() {
    let root = common::TempRoot::new();
    tool_in(&root, "plain", "mytool", 0o644);
    let first = tool_in(&root, "first", "mytool", 0o755);
    tool_in(&root, "second", "mytool", 0o755);
    let path = std::env::join_paths([root.join("plain"), root.join("first"), root.join("second")])
        .unwrap();
    assert_eq!(Environment { path }.find("mytool"), Some(first));
}

#[test]
fn a_tool_that_is_not_on_the_path_is_none() {
    let root = common::TempRoot::new();
    tool_in(&root, "bin", "other", 0o755);
    let path = std::env::join_paths([root.join("bin")]).unwrap();
    assert_eq!(Environment { path }.find("mytool"), None);
}

#[test]
fn a_tool_name_with_a_slash_or_a_dot_part_is_none_even_when_the_file_is_there() {
    let root = common::TempRoot::new();
    tool_in(&root, "bin", "mytool", 0o755);
    tool_in(&root, "other", "x", 0o755);
    let path = std::env::join_paths([root.join("bin")]).unwrap();
    let env = Environment { path };
    for name in [
        "../other/x",
        "other/x",
        "/tmp/anything",
        ".",
        "..",
        "",
        "my\0tool",
    ] {
        assert_eq!(env.find(name), None, "{name:?}");
    }
}

#[test]
fn a_relative_path_entry_is_skipped_even_when_it_leads_to_the_tool() {
    let root = common::TempRoot::new();
    let tool = tool_in(&root, "bin", "mytool", 0o755);
    let cwd = std::env::current_dir().unwrap();
    let up = "../".repeat(cwd.components().count() - 1);
    let relative = PathBuf::from(up).join(root.join("bin").strip_prefix("/").unwrap());
    assert!(
        relative.join("mytool").is_file(),
        "the relative entry reaches {}",
        tool.display()
    );
    let path = std::env::join_paths([relative]).unwrap();
    assert_eq!(Environment { path }.find("mytool"), None);
}

#[test]
fn an_empty_path_entry_is_skipped_and_never_means_the_current_folder() {
    let root = common::TempRoot::new();
    let in_cwd = std::env::current_dir()
        .unwrap()
        .join("collide-no-such-tool-here");
    assert!(!in_cwd.exists());
    let path = std::env::join_paths([PathBuf::new(), root.join("none")]).unwrap();
    assert_eq!(Environment { path }.find("collide-no-such-tool-here"), None);
}

// The pack's data folder (ts-0102-vu; 0.7, Pack data).

fn data_of(command: &Command) -> Option<PathBuf> {
    command
        .get_envs()
        .find(|(name, _)| *name == "COLLIDE_PACK_DATA")
        .and_then(|(_, value)| value.map(PathBuf::from))
}

/// Each test names its own folder: the tests run at once, and `fake_path` clears its folder first.
fn plan_launch(name: &str) -> collide_host::builtin_packs::ProviderLaunch {
    let (path, _, _) = fake_path(name);
    packs(&bundle())
        .provider("collide/plan", "plan", &Environment { path })
        .unwrap()
}

#[test]
fn a_command_with_no_pack_data_sets_no_variable() {
    assert_eq!(data_of(&plan_launch("data-none").command()), None);
}

#[test]
fn a_command_for_a_pack_on_a_board_sets_the_pack_data_variable_to_a_private_folder() {
    let root = common::TempRoot::new();
    let launch = plan_launch("data-private")
        .with_pack_data(&root.join("data"), "board-1", "collide/plan")
        .unwrap();
    let dir = data_of(&launch.command()).expect("the variable is set");
    assert!(dir.is_dir(), "{}", dir.display());
    assert_eq!(
        fs::metadata(&dir).unwrap().permissions().mode() & 0o777,
        0o700
    );
}

#[test]
fn two_packs_and_two_boards_have_four_different_data_folders() {
    let root = common::TempRoot::new();
    let data = root.join("data");
    let dir = |board: &str, pack: &str| {
        let launch = plan_launch("data-four")
            .with_pack_data(&data, board, pack)
            .unwrap();
        data_of(&launch.command()).expect("the variable is set")
    };
    let all = [
        dir("board-1", "collide/plan"),
        dir("board-1", "ferry"),
        dir("board-2", "collide/plan"),
        dir("board-2", "ferry"),
    ];
    for (i, a) in all.iter().enumerate() {
        for b in &all[i + 1..] {
            assert_ne!(a, b);
        }
    }
}

#[test]
fn a_pack_name_that_is_not_a_safe_token_gets_no_launch() {
    let root = common::TempRoot::new();
    let result = plan_launch("data-unsafe").with_pack_data(&root.join("data"), "board-1", "../x");
    assert!(result.is_err());
}
