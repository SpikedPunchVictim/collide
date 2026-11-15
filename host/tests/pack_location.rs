//! Tests for the rule that finds the built-in packs bundle from a checkout or an install (ts-0020).

use collide_host::pack_location::{Inputs, locate_bundle};
use std::fs;
use std::path::{Path, PathBuf};

mod common;
use common::scratch;

/// A folder that is a bundle: it holds `plan/pack.yaml`.
fn make_bundle(dir: &Path) {
    fs::create_dir_all(dir.join("plan")).unwrap();
    fs::write(dir.join("plan/pack.yaml"), "name: collide/plan\n").unwrap();
}

/// A fresh scratch folder for one test.
fn fresh(name: &str) -> PathBuf {
    let dir = scratch(&format!("pack-location-{name}"));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

struct Cleanup(PathBuf);

impl Drop for Cleanup {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn the_variable_wins_over_every_other_place() {
    let base = fresh("variable");
    let _cleanup = Cleanup(base.clone());
    make_bundle(&base.join("chosen"));
    make_bundle(&base.join("app/src-tauri/resources/builtin-packs"));
    let found = locate_bundle(&Inputs {
        override_dir: Some(base.join("chosen")),
        exe: Some(base.join("target/debug/collide")),
    })
    .unwrap();
    assert_eq!(found, base.join("chosen"));
}

#[test]
fn a_variable_that_names_no_bundle_is_an_error_and_never_falls_back() {
    let base = fresh("variable-wrong");
    let _cleanup = Cleanup(base.clone());
    make_bundle(&base.join("app/src-tauri/resources/builtin-packs"));
    let error = locate_bundle(&Inputs {
        override_dir: Some(base.join("empty")),
        exe: Some(base.join("target/debug/collide")),
    })
    .unwrap_err();
    let message = error.to_string();
    assert!(
        message.starts_with("COLLIDE_BUILTIN_PACKS is set to "),
        "{message}"
    );
    assert!(
        message.contains(&base.join("empty").display().to_string()),
        "{message}"
    );
}

#[test]
fn the_staged_bundle_above_the_executable_is_found_in_a_checkout() {
    let base = fresh("staged");
    let _cleanup = Cleanup(base.clone());
    let staged = base.join("app/src-tauri/resources/builtin-packs");
    make_bundle(&staged);
    let exe = base.join("target/debug/collide");
    fs::create_dir_all(exe.parent().unwrap()).unwrap();
    let found = locate_bundle(&Inputs {
        override_dir: None,
        exe: Some(exe),
    })
    .unwrap();
    assert_eq!(found, staged);
}

#[test]
fn the_staged_bundle_wins_over_the_copy_beside_the_executable() {
    let base = fresh("staged-first");
    let _cleanup = Cleanup(base.clone());
    let staged = base.join("app/src-tauri/resources/builtin-packs");
    make_bundle(&staged);
    make_bundle(&base.join("target/debug/builtin-packs"));
    let found = locate_bundle(&Inputs {
        override_dir: None,
        exe: Some(base.join("target/debug/collide")),
    })
    .unwrap();
    assert_eq!(found, staged);
}

#[test]
fn the_copy_beside_the_executable_is_found_when_nothing_is_staged() {
    let base = fresh("beside");
    let _cleanup = Cleanup(base.clone());
    make_bundle(&base.join("bin/builtin-packs"));
    let found = locate_bundle(&Inputs {
        override_dir: None,
        exe: Some(base.join("bin/collide")),
    })
    .unwrap();
    assert_eq!(found, base.join("bin/builtin-packs"));
}

#[test]
fn an_app_bundle_keeps_its_packs_in_resources_beside_macos() {
    let base = fresh("app-bundle");
    let _cleanup = Cleanup(base.clone());
    make_bundle(&base.join("Collide.app/Contents/Resources/builtin-packs"));
    fs::create_dir_all(base.join("Collide.app/Contents/MacOS")).unwrap();
    let found = locate_bundle(&Inputs {
        override_dir: None,
        exe: Some(base.join("Collide.app/Contents/MacOS/collide")),
    })
    .unwrap();
    assert!(
        found.ends_with("Contents/MacOS/../Resources/builtin-packs"),
        "{found:?}"
    );
    assert!(found.join("plan/pack.yaml").is_file());
}

#[test]
fn a_folder_without_plan_pack_yaml_is_not_a_bundle() {
    let base = fresh("not-a-bundle");
    let _cleanup = Cleanup(base.clone());
    fs::create_dir_all(base.join("bin/builtin-packs/plan")).unwrap();
    let error = locate_bundle(&Inputs {
        override_dir: None,
        exe: Some(base.join("bin/collide")),
    })
    .unwrap_err();
    assert!(!error.looked.is_empty());
}

#[test]
fn when_nothing_is_found_the_error_names_each_place_and_what_to_do() {
    let base = fresh("nothing");
    let _cleanup = Cleanup(base.clone());
    let error = locate_bundle(&Inputs {
        override_dir: None,
        exe: Some(base.join("bin/collide")),
    })
    .unwrap_err()
    .to_string();
    assert!(
        error.contains(&base.join("bin/builtin-packs").display().to_string()),
        "{error}"
    );
    assert!(
        error.contains(
            &base
                .join("app/src-tauri/resources/builtin-packs")
                .display()
                .to_string()
        ),
        "{error}"
    );
    assert!(error.contains("pnpm build:packs"), "{error}");
    assert!(error.contains("COLLIDE_BUILTIN_PACKS"), "{error}");
}

#[test]
fn an_unknown_executable_path_says_so() {
    let error = locate_bundle(&Inputs::default()).unwrap_err().to_string();
    assert!(
        error.contains("the executable's path is unknown"),
        "{error}"
    );
}
