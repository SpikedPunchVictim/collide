//! The repository's own instructions on the launch line (ts-0104-gx; 0.3, The repository's own instructions): the
//! `repo_instructions` role key picks the line `ts-0103-sa` and `ts-0106-nr` measured, the plugin writer makes the
//! skills plugin the line names, and the path list is the one the warning reads. Fake Claude records what it was
//! started with. Nothing here runs `claude` or touches the owner's files: every folder is in a temp root.

use super::common::TempRoot;
use super::repo_instructions::{
    ADD_DIR_VAR, Candidate, Fixture, Line, PLUGIN_NAME, PLUGINS_DIR, candidate_line,
    holds_only_the_skills_link, keeps_the_snapshot_line, through_the_fake,
};
use super::{default_role, plain_role, request, value_of};
use collide_core::adapter::{AgentAdapter, LaunchError};
use collide_host::adapters::claude::ClaudeAdapter;
use collide_host::adapters::claude::instructions::{
    PLUGIN_ON_THE_LINE, is_instruction_path, skills_plugin_dir, write_skills_plugin,
};
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};

fn line_of(role: serde_json::Value, cwd: &Path, snapshot: &Path) -> Line {
    let mut request = request(role);
    request.cwd = cwd.to_path_buf();
    request.snapshot_dir = snapshot.to_path_buf();
    Line::of(ClaudeAdapter.launch(&request).expect("the launch line"))
}

/// A start folder with a skill in it, and an empty snapshot folder, both in `root`.
fn folders(root: &TempRoot, with_skills: bool) -> (PathBuf, PathBuf) {
    let start = root.join("start");
    let snapshot = root.join("snapshot");
    fs::create_dir_all(&snapshot).unwrap();
    fs::create_dir_all(&start).unwrap();
    if with_skills {
        fs::create_dir_all(start.join(".claude/skills/stand-in-skill")).unwrap();
        fs::write(start.join(".claude/skills/stand-in-skill/SKILL.md"), "x\n").unwrap();
    }
    (start, snapshot)
}

// ---- the path list -----------------------------------------------------------------------------------------------

#[test]
fn claude_md_at_any_depth_and_the_skills_folder_are_instruction_files() {
    for path in [
        "CLAUDE.md",
        "sub/CLAUDE.md",
        "a/b/c/CLAUDE.md",
        ".claude/skills/stand-in-skill/SKILL.md",
        ".claude/skills/stand-in-skill/helper.py",
    ] {
        assert!(is_instruction_path(path), "{path}");
    }
}

#[test]
fn readme_code_commands_and_settings_are_not_instruction_files() {
    for path in [
        "README.md",
        "src/main.rs",
        "docs/guide.md",
        ".claude/commands/deploy.md",
        ".claude/settings.json",
        ".claude/agents/stand-in.md",
        "CLAUDE.md.bak",
        "NOT-CLAUDE.md",
        "claude.md",
        ".claude/skills",
        "pkg/.claude/skills/other/SKILL.md",
    ] {
        assert!(!is_instruction_path(path), "{path}");
    }
}

// ---- the plugin writer -------------------------------------------------------------------------------------------

#[test]
fn the_plugin_holds_a_manifest_and_a_link_to_the_start_folders_skills() {
    let root = TempRoot::new();
    let (start, snapshot) = folders(&root, true);
    assert!(write_skills_plugin(&snapshot, &start).unwrap());
    let folder = skills_plugin_dir(&snapshot);
    assert_eq!(folder, snapshot.join(PLUGINS_DIR).join(PLUGIN_NAME));
    assert_eq!(holds_only_the_skills_link(&folder, &start), Ok(()));
}

#[test]
fn no_plugin_is_written_when_the_start_folder_has_no_skills() {
    let root = TempRoot::new();
    let (start, snapshot) = folders(&root, false);
    assert!(!write_skills_plugin(&snapshot, &start).unwrap());
    assert!(!skills_plugin_dir(&snapshot).exists());
}

#[test]
fn a_skills_file_instead_of_a_folder_writes_no_plugin() {
    let root = TempRoot::new();
    let (start, snapshot) = folders(&root, false);
    fs::create_dir_all(start.join(".claude")).unwrap();
    fs::write(start.join(".claude/skills"), "not a folder\n").unwrap();
    assert!(!write_skills_plugin(&snapshot, &start).unwrap());
    assert!(!skills_plugin_dir(&snapshot).exists());
}

#[test]
fn writing_the_plugin_again_gives_the_same_plugin() {
    let root = TempRoot::new();
    let (start, snapshot) = folders(&root, true);
    write_skills_plugin(&snapshot, &start).unwrap();
    assert!(write_skills_plugin(&snapshot, &start).unwrap());
    assert_eq!(
        holds_only_the_skills_link(&skills_plugin_dir(&snapshot), &start),
        Ok(())
    );
}

#[test]
fn a_plugin_from_an_earlier_attempt_is_removed_when_the_skills_are_gone() {
    let root = TempRoot::new();
    let (start, snapshot) = folders(&root, true);
    write_skills_plugin(&snapshot, &start).unwrap();
    fs::remove_dir_all(start.join(".claude/skills")).unwrap();
    assert!(!write_skills_plugin(&snapshot, &start).unwrap());
    assert!(!skills_plugin_dir(&snapshot).exists());
    assert!(
        fs::read_dir(snapshot.join(PLUGINS_DIR))
            .map(|mut dir| dir.next().is_none())
            .unwrap_or(true),
        "the plugins folder holds nothing else of this item's"
    );
}

#[test]
fn the_plugin_link_follows_the_start_folder_it_was_written_for() {
    let root = TempRoot::new();
    let (start, snapshot) = folders(&root, true);
    let other = root.join("other");
    fs::create_dir_all(other.join(".claude/skills")).unwrap();
    write_skills_plugin(&snapshot, &start).unwrap();
    write_skills_plugin(&snapshot, &other).unwrap();
    assert_eq!(
        holds_only_the_skills_link(&skills_plugin_dir(&snapshot), &other),
        Ok(())
    );
    assert!(holds_only_the_skills_link(&skills_plugin_dir(&snapshot), &start).is_err());
}

// ---- the line ----------------------------------------------------------------------------------------------------

#[test]
fn the_default_role_adds_the_start_folder_and_the_variable_to_the_line() {
    let root = TempRoot::new();
    let (start, snapshot) = folders(&root, false);
    let line = line_of(default_role(), &start, &snapshot);
    assert_eq!(line.value_of("--add-dir"), start.to_str());
    assert_eq!(line.env.get(ADD_DIR_VAR).map(String::as_str), Some("1"));
    assert!(!line.argv.iter().any(|word| word == "--plugin-dir"));
}

#[test]
fn the_worktree_value_is_the_same_line_as_no_key() {
    let root = TempRoot::new();
    let (start, snapshot) = folders(&root, true);
    write_skills_plugin(&snapshot, &start).unwrap();
    let default = line_of(default_role(), &start, &snapshot);
    let named = line_of(
        json!({ "agent": "claude", "repo_instructions": "worktree" }),
        &start,
        &snapshot,
    );
    assert_eq!(default, named);
}

#[test]
fn with_a_skills_plugin_in_the_snapshot_the_line_names_it_after_the_added_folder_while_the_seam_is_on()
 {
    let root = TempRoot::new();
    let (start, snapshot) = folders(&root, true);
    write_skills_plugin(&snapshot, &start).unwrap();
    let line = line_of(default_role(), &start, &snapshot);
    if !PLUGIN_ON_THE_LINE {
        assert_eq!(line.value_of("--plugin-dir"), None);
        return;
    }
    let plugin = skills_plugin_dir(&snapshot);
    assert_eq!(line.value_of("--plugin-dir"), plugin.to_str());
    let at = |flag: &str| line.argv.iter().position(|word| word == flag).unwrap();
    assert!(at("--add-dir") < at("--plugin-dir"));
    assert!(at("--disallowed-tools") < at("--add-dir"));
}

#[test]
fn none_is_the_line_before_this_item_word_for_word() {
    let (start, snapshot) = (PathBuf::from("/work"), PathBuf::from("/snap"));
    let line = line_of(plain_role(), &start, &snapshot);
    let expected: Vec<String> = [
        "-p",
        "--input-format",
        "stream-json",
        "--output-format",
        "stream-json",
        "--verbose",
        "--include-hook-events",
        "--permission-prompts",
        "host",
        "--permission-prompt-tool",
        "stdio",
        "--strict-mcp-config",
        "--session-id",
        super::ASSIGNED,
        "--model",
        "sonnet",
        "--agents",
        "/snap/agents.json",
        "--agent",
        "collide-demo-builder",
        "--setting-sources",
        "user",
        "--settings",
        "/snap/settings.json",
        "--mcp-config",
        "/snap/mcp.json",
        "--disallowed-tools",
        "CronCreate",
        "ScheduleWakeup",
        "RemoteTrigger",
        "PushNotification",
        "EnterWorktree",
        "ExitWorktree",
        "Workflow",
        "SendMessage",
    ]
    .map(String::from)
    .to_vec();
    assert_eq!(line.argv, expected);
    assert!(line.env.is_empty());
    assert_eq!(line.cwd, start);
}

#[test]
fn none_adds_no_plugin_even_when_the_snapshot_has_one() {
    let root = TempRoot::new();
    let (start, snapshot) = folders(&root, true);
    write_skills_plugin(&snapshot, &start).unwrap();
    let line = line_of(plain_role(), &start, &snapshot);
    assert!(!line.argv.iter().any(|word| word == "--plugin-dir"));
    assert!(!line.argv.iter().any(|word| word == "--add-dir"));
    assert!(!line.env.contains_key(ADD_DIR_VAR));
}

#[test]
fn the_snapshot_flags_are_on_the_line_for_both_values_and_with_config_none() {
    let root = TempRoot::new();
    let (start, snapshot) = folders(&root, true);
    write_skills_plugin(&snapshot, &start).unwrap();
    for role in [
        default_role(),
        plain_role(),
        json!({ "agent": "claude", "config": "none" }),
        json!({ "agent": "claude", "config": "none", "repo_instructions": "none" }),
    ] {
        let line = line_of(role.clone(), &start, &snapshot);
        assert_eq!(
            keeps_the_snapshot_line(&line, &snapshot, &start),
            Ok(()),
            "{role}"
        );
    }
}

#[test]
fn a_start_folder_that_is_not_utf8_is_refused_on_the_worktree_line() {
    use std::os::unix::ffi::OsStringExt;
    let mut req = request(default_role());
    req.cwd = PathBuf::from(std::ffi::OsString::from_vec(vec![b'/', 0xff, b'x']));
    match ClaudeAdapter.launch(&req) {
        Err(LaunchError::Invalid { message }) => assert!(message.contains("UTF-8"), "{message}"),
        other => panic!("expected an invalid request, got {other:?}"),
    }
}

#[test]
fn the_default_line_is_the_chosen_candidate_and_fake_claude_sees_it() {
    let fixture = Fixture::new();
    let start = fixture.worktree("phase-start");
    let with_skills_line = if PLUGIN_ON_THE_LINE {
        Candidate::AddDirSkillsPlugin
    } else {
        Candidate::AddDir
    };
    for (candidate, with_skills) in [(with_skills_line, true), (Candidate::AddDir, false)] {
        if !with_skills {
            fs::remove_dir_all(start.join(".claude/skills")).unwrap();
            let _ = fs::remove_dir_all(fixture.snapshot.join(PLUGINS_DIR));
        }
        let request = fixture.request(default_role(), &start);
        let chosen = candidate_line(&fixture, candidate, &request);
        // The candidate's own test helper wrote its plugin; the writer under test makes it the same way.
        write_skills_plugin(&fixture.snapshot, &start).unwrap();
        let built = Line::of(ClaudeAdapter.launch(&request).unwrap());
        assert_eq!(built, chosen, "{candidate:?}");
        let seen = through_the_fake(fixture.root(), &built);
        assert_eq!(seen.argv, built.argv);
        assert_eq!(seen.env.get(ADD_DIR_VAR).map(String::as_str), Some("1"));
        assert_eq!(
            value_of(&seen.argv, "--add-dir").map(PathBuf::from),
            Some(start.clone())
        );
        assert_eq!(
            fs::canonicalize(&seen.cwd).unwrap(),
            fs::canonicalize(&start).unwrap()
        );
    }
}
