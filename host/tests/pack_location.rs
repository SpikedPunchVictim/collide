//! Tests for the rule that finds the built-in packs bundle from a checkout or an install (ts-0020).

use collide_host::pack_location::{Inputs, bundle_sdk, locate_bundle};
use std::fs;
use std::path::{Path, PathBuf};

mod common;

/// A folder that is a bundle: it holds `plan/pack.yaml`.
fn make_bundle(dir: &Path) {
    fs::create_dir_all(dir.join("plan")).unwrap();
    fs::write(dir.join("plan/pack.yaml"), "name: collide/plan\n").unwrap();
}

/// A fresh scratch folder for one test. It is outside the repository: the search walks every folder above the
/// executable, and from `target/tmp` it would reach this checkout's own staged bundle after `pnpm build:packs`.
fn fresh(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "collide-pack-location-{name}-{}",
        std::process::id()
    ));
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

/// Puts a compiled SDK where `scripts/build-packs.ts` puts it: in the `node_modules` of the pack that depends on it.
fn make_sdk(bundle: &Path, pack: &str) -> PathBuf {
    let sdk = bundle.join(pack).join("node_modules/@collide/pack-sdk");
    fs::create_dir_all(sdk.join("dist")).unwrap();
    fs::write(sdk.join("package.json"), "{\"version\":\"0.1.0\"}").unwrap();
    sdk
}

#[test]
fn the_sdk_is_found_in_the_pack_that_holds_it() {
    let base = fresh("sdk-found");
    let _cleanup = Cleanup(base.clone());
    make_bundle(&base);
    let sdk = make_sdk(&base, "plan");
    assert_eq!(bundle_sdk(&base).unwrap(), sdk);
}

#[test]
fn the_first_pack_in_name_order_that_holds_an_sdk_gives_it() {
    let base = fresh("sdk-order");
    let _cleanup = Cleanup(base.clone());
    make_bundle(&base);
    fs::create_dir_all(base.join("collide")).unwrap();
    let sdk = make_sdk(&base, "other");
    make_sdk(&base, "plan");
    assert_eq!(bundle_sdk(&base).unwrap(), sdk);
}

#[test]
fn a_bundle_with_no_sdk_is_an_error_that_names_the_bundle_folder() {
    let base = fresh("sdk-none");
    let _cleanup = Cleanup(base.clone());
    make_bundle(&base);
    fs::create_dir_all(base.join("plan/node_modules/@collide/pack-sdk")).unwrap();
    let error = bundle_sdk(&base).unwrap_err().to_string();
    assert!(error.contains(&base.display().to_string()), "{error}");
    assert!(error.contains("pack-sdk"), "{error}");
}

#[test]
fn a_bundle_folder_that_is_not_there_is_an_error_that_names_it() {
    let base = fresh("sdk-missing");
    let _cleanup = Cleanup(base.clone());
    let error = bundle_sdk(&base.join("nowhere")).unwrap_err().to_string();
    assert!(error.contains("nowhere"), "{error}");
}
