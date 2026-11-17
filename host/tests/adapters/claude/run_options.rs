//! What a file-task run starts `claude` with, through the daemon and fake Claude (ts-0104-gx): `collide run --config`
//! reaches the line, and the default role's line has the repository's instructions on it (0.3, The repository's own
//! instructions). Fake Claude records its argv, environment and folder. Each test has a daemon of its own under a temp
//! root (`file_run.rs`): nothing here reads the owner's files.

use super::file_run::{Fixture, Setup, git};
use super::quick_run::report_with;
use super::repo_instructions::{ADD_DIR_VAR, PLUGIN_NAME, PLUGINS_DIR, holds_only_the_skills_link};
use super::value_of;
use collide_core::profile::RoleConfig;
use collide_host::adapters::claude::instructions::PLUGIN_ON_THE_LINE;
use serde_json::{Value, json};
use std::fs;
use std::path::PathBuf;

/// What fake Claude saw: its words (after the recording's name), its environment and its folder.
struct Launched {
    argv: Vec<String>,
    env: Value,
    cwd: PathBuf,
}

/// A fixture whose `claude` records its launch in `launch.json` of the root.
fn recording_fixture(root: &super::common::TempRoot) -> (Fixture, PathBuf) {
    let launch = root.join("launch.json");
    let recording = report_with(root, &[json!({ "dump_launch": launch })]);
    (Fixture::new(&recording, Setup::default()), launch)
}

fn read(launch: &PathBuf) -> Launched {
    let seen: Value = serde_json::from_str(&fs::read_to_string(launch).unwrap()).unwrap();
    Launched {
        argv: seen["argv"]
            .as_array()
            .unwrap()
            .iter()
            .map(|word| word.as_str().unwrap().to_string())
            .collect(),
        env: seen["env"].clone(),
        cwd: PathBuf::from(seen["cwd"].as_str().unwrap()),
    }
}

/// The run's launch, once its phase has ended.
fn launched(fx: &mut Fixture, config: Option<RoleConfig>, launch: &PathBuf) -> (String, Launched) {
    let mut params = fx.start_params();
    params.config = config;
    let run = match fx.start(&params) {
        collide_core::owner::RunStartResult::Started { run } => run,
        other => panic!("not started: {other:?}"),
    };
    fx.wait_final(&run);
    (run, read(launch))
}

#[test]
fn config_none_reaches_the_launch_line_as_an_empty_setting_sources() {
    let root = super::common::TempRoot::new();
    let (mut fx, launch) = recording_fixture(&root);
    let (_, seen) = launched(&mut fx, Some(RoleConfig::None), &launch);
    assert_eq!(value_of(&seen.argv, "--setting-sources"), Some(""));
    assert!(seen.argv.contains(&"--strict-mcp-config".to_string()));
}

#[test]
fn config_user_and_no_config_both_give_user() {
    for config in [Some(RoleConfig::User), None] {
        let root = super::common::TempRoot::new();
        let (mut fx, launch) = recording_fixture(&root);
        let (_, seen) = launched(&mut fx, config, &launch);
        assert_eq!(
            value_of(&seen.argv, "--setting-sources"),
            Some("user"),
            "{config:?}"
        );
    }
}

#[test]
fn the_default_role_adds_the_worktree_and_the_variable_and_no_plugin_when_there_are_no_skills() {
    let root = super::common::TempRoot::new();
    let (mut fx, launch) = recording_fixture(&root);
    let (run, seen) = launched(&mut fx, None, &launch);
    let tree = fx.workspace_dir(&run);
    assert_eq!(
        value_of(&seen.argv, "--add-dir").map(|dir| fs::canonicalize(dir).unwrap()),
        Some(fs::canonicalize(&tree).unwrap())
    );
    assert_eq!(seen.env[ADD_DIR_VAR], "1");
    assert_eq!(
        fs::canonicalize(&seen.cwd).unwrap(),
        fs::canonicalize(&tree).unwrap()
    );
    assert!(!seen.argv.iter().any(|word| word == "--plugin-dir"));
    for flag in [
        "--setting-sources",
        "--settings",
        "--mcp-config",
        "--strict-mcp-config",
    ] {
        assert!(seen.argv.contains(&flag.to_string()), "{flag}");
    }
}

#[test]
fn a_repository_with_skills_gets_the_plugin_of_the_runs_own_snapshot_linked_to_the_worktrees_skills()
 {
    let root = super::common::TempRoot::new();
    let (mut fx, launch) = recording_fixture(&root);
    let skill = fx.repo.join(".claude/skills/stand-in");
    fs::create_dir_all(&skill).unwrap();
    fs::write(skill.join("SKILL.md"), "a stand-in skill\n").unwrap();
    git(&fx.repo, &["add", "."]);
    git(&fx.repo, &["commit", "-q", "-m", "a skill"]);
    let (run, seen) = launched(&mut fx, None, &launch);
    let tree = fx.workspace_dir(&run);
    let snapshot = fx.paths().runs_dir.join(&run).join("snapshot");
    let plugin = snapshot.join(PLUGINS_DIR).join(PLUGIN_NAME);
    let named = PLUGIN_ON_THE_LINE.then(|| plugin.to_str().unwrap());
    assert_eq!(value_of(&seen.argv, "--plugin-dir"), named);
    assert_eq!(holds_only_the_skills_link(&plugin, &tree), Ok(()));
    assert!(tree.join(".claude/skills/stand-in/SKILL.md").exists());
}
