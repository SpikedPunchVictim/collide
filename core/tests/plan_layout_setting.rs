//! The `layout` setting of the plan pack (ts-0124-xm; 0.7, Settings; 0.10, Planning sources). The real
//! `packs/plan/pack.yaml` is read as the pack loader reads it: it must pass the manifest checks and the schema subset,
//! and the layers must accept `layout` in the repo's file and refuse it in the user's files. Nothing is read from the
//! user's home: every file is a string in the test.

use collide_core::owner::SettingLayer;
use collide_core::pack::PackManifest;
use collide_core::pack_check::check_manifest;
use collide_core::setting_value::{check_setting_schema, check_value};
use collide_core::settings::ConfigFile;
use collide_core::settings_layers::{LayerInputs, PackSettings, Settings, settings_layers};
use collide_core::yaml::{load_yaml, load_yaml_file};
use serde_json::{Value, json};
use std::path::Path;

const KEY: &str = "collide/plan.layout";

fn manifest() -> PackManifest {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../packs/plan/pack.yaml");
    load_yaml_file(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

fn file(text: &str) -> ConfigFile {
    load_yaml(text.as_bytes(), "config.yaml").unwrap_or_else(|error| panic!("{error}\n{text}"))
}

/// The layers over the plan pack's real settings, with the given user and repo files.
fn layers(user: &str, repo: &str) -> Settings {
    let manifest = manifest();
    let packs = [PackSettings {
        pack: "collide/plan",
        config: &manifest.config,
    }];
    settings_layers(&LayerInputs {
        packs: &packs,
        user: &file(user),
        user_board: &ConfigFile::default(),
        repo: &file(repo),
        profile: None,
    })
}

fn layout_schema() -> Value {
    manifest().config["layout"].0.clone()
}

fn violations(value: Value) -> Vec<String> {
    check_value(&layout_schema(), &value)
        .iter()
        .map(ToString::to_string)
        .collect()
}

#[test]
fn the_plan_manifest_passes_the_manifest_checks() {
    assert_eq!(check_manifest(&manifest()), Ok(()));
}

#[test]
fn layout_is_declared_and_its_schema_stays_inside_the_supported_subset() {
    let manifest = manifest();
    assert!(manifest.config.contains_key("layout"));
    for (key, schema) in &manifest.config {
        let problems = check_setting_schema(&format!("config.{key}"), &schema.0);
        assert_eq!(problems, vec![], "config.{key}");
    }
}

#[test]
fn layout_is_repo_only_and_not_overridable() {
    let schema = layout_schema();
    assert_eq!(schema["repoOnly"], json!(true));
    assert_ne!(schema.get("overridable"), Some(&json!(true)));
    assert_ne!(schema.get("secret"), Some(&json!(true)));
}

#[test]
fn the_default_layout_names_the_default_root_and_says_default() {
    let settings = layers("", "");
    let entry = settings
        .entries
        .iter()
        .find(|entry| entry.key == KEY)
        .unwrap_or_else(|| panic!("no entry for {KEY}"));
    assert_eq!(entry.value, Some(json!({ "root": ".collide/planning" })));
    assert_eq!(entry.layer, SettingLayer::Default);
    assert_eq!(settings.problems, vec![]);
}

#[test]
fn layout_in_the_repos_file_is_accepted_and_says_repo() {
    let settings = layers(
        "",
        "config:\n  collide/plan:\n    layout:\n      root: docs/plan\n      previous: [.collide/planning]\n",
    );
    assert_eq!(settings.problems, vec![]);
    let entry = settings
        .entries
        .iter()
        .find(|entry| entry.key == KEY)
        .unwrap();
    assert_eq!(
        entry.value,
        Some(json!({ "root": "docs/plan", "previous": [".collide/planning"] }))
    );
    assert_eq!(entry.layer, SettingLayer::Repo);
}

#[test]
fn layout_in_the_users_file_is_refused_naming_the_key_and_not_applied() {
    let settings = layers(
        "config:\n  collide/plan:\n    layout:\n      root: docs/plan\n",
        "",
    );
    assert_eq!(settings.problems.len(), 1, "{:?}", settings.problems);
    assert_eq!(settings.problems[0].key, KEY);
    assert!(settings.problems[0].message.contains("repoOnly"));
    assert!(settings.problems[0].message.contains(KEY));
    let entry = settings
        .entries
        .iter()
        .find(|entry| entry.key == KEY)
        .unwrap();
    assert_eq!(entry.layer, SettingLayer::Default);
}

#[test]
fn layout_in_the_users_file_for_one_board_is_refused_too() {
    let manifest = manifest();
    let packs = [PackSettings {
        pack: "collide/plan",
        config: &manifest.config,
    }];
    let board = file("config:\n  collide/plan:\n    layout:\n      root: docs/plan\n");
    let settings = settings_layers(&LayerInputs {
        packs: &packs,
        user: &ConfigFile::default(),
        user_board: &board,
        repo: &ConfigFile::default(),
        profile: None,
    });
    assert_eq!(settings.problems.len(), 1, "{:?}", settings.problems);
    assert_eq!(settings.problems[0].key, KEY);
}

#[test]
fn the_schema_accepts_every_key_of_the_layout() {
    let all = json!({
        "root": "docs/plan",
        "paths": { "spike": "docs/decisions" },
        "code": "spikes",
        "previous": [".collide/planning", "old/plan"],
    });
    assert_eq!(violations(all), Vec::<String>::new());
}

#[test]
fn the_schema_refuses_a_key_it_does_not_list_and_a_value_of_the_wrong_type() {
    for bad in [
        json!({ "rooot": "docs/plan" }),
        json!({ "root": 7 }),
        json!({ "previous": "old" }),
        json!({ "previous": [3] }),
        json!({ "paths": "docs" }),
        json!({ "paths": { "spike": 3 } }),
        json!({ "code": ["spikes"] }),
        json!("docs/plan"),
    ] {
        assert!(!violations(bad.clone()).is_empty(), "{bad}");
    }
}
