//! Tests for `models.yaml` loading (ts-0060, 0.3 Who picks what). Every test puts `HOME` in a temp root and passes
//! it in, so none reads the owner's own `~/.config/collide/models.yaml`.

use collide_core::resolve::resolve_model_alias;
use collide_host::models::{ModelsError, load_user_models, models_path};
use std::fs;

mod common;
use common::TempRoot;

fn write_models(home: &TempRoot, text: &str) {
    let dir = home.join(".config/collide");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("models.yaml"), text).unwrap();
}

#[test]
fn the_file_is_below_the_home_directory() {
    let home = TempRoot::new();
    assert_eq!(
        models_path(Some(home.path().to_str().unwrap())).unwrap(),
        home.join(".config/collide/models.yaml")
    );
}

#[test]
fn no_home_is_an_error_and_so_is_an_empty_one() {
    for home in [None, Some("")] {
        let error = models_path(home).unwrap_err();
        assert!(matches!(error, ModelsError::NoHome), "{error}");
        assert!(error.to_string().contains("HOME"), "{error}");
    }
}

#[test]
fn a_missing_file_means_no_mappings_so_every_alias_is_unmapped() {
    let home = TempRoot::new();
    let models = load_user_models(Some(home.path().to_str().unwrap())).unwrap();
    assert!(models.mapping_for("claude").is_empty());
    let error = resolve_model_alias("fast", &models.mapping_for("claude"), None).unwrap_err();
    assert!(error.to_string().contains("fast"), "{error}");
}

#[test]
fn a_valid_file_maps_aliases_for_each_tool() {
    let home = TempRoot::new();
    write_models(
        &home,
        "tools:\n  claude:\n    fast: { name: haiku }\n    strong: { name: opus }\n",
    );
    let models = load_user_models(Some(home.path().to_str().unwrap())).unwrap();
    let claude = models.mapping_for("claude");
    assert_eq!(
        resolve_model_alias("strong", &claude, None).unwrap().name,
        "opus"
    );
    assert!(models.mapping_for("opencode").is_empty());
}

#[test]
fn a_malformed_file_fails_with_its_path_and_the_place_of_the_error() {
    let home = TempRoot::new();
    write_models(&home, "tools:\n  claude:\n    fast: { name: haiku\n");
    let error = load_user_models(Some(home.path().to_str().unwrap()))
        .unwrap_err()
        .to_string();
    assert!(error.contains(".config/collide/models.yaml"), "{error}");
    // The place is `path:line:column`.
    assert!(error.contains("models.yaml:3:"), "{error}");
}

#[test]
fn a_file_with_an_unknown_key_fails_with_its_path_and_the_key() {
    let home = TempRoot::new();
    write_models(
        &home,
        "tools:\n  claude:\n    fast: { name: haiku, speed: 9 }\n",
    );
    let error = load_user_models(Some(home.path().to_str().unwrap()))
        .unwrap_err()
        .to_string();
    assert!(error.contains("models.yaml"), "{error}");
    assert!(error.contains("speed"), "{error}");
}

#[test]
fn a_models_path_that_is_a_directory_is_an_error_not_an_empty_file() {
    let home = TempRoot::new();
    fs::create_dir_all(home.join(".config/collide/models.yaml")).unwrap();
    let error = load_user_models(Some(home.path().to_str().unwrap())).unwrap_err();
    assert!(error.to_string().contains("models.yaml"), "{error}");
}
