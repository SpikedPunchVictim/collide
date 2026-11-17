//! The warning for a phase that starts after its run changed an instruction file (ts-0104-gx; 0.3, The repository's own
//! instructions). Two levels: the function that reads the changes against a base commit, over a temp git repository,
//! and a daemon with fake Claude as `claude`, where a re-prompt run starts in the worktree the run before it changed.
//! Git reads no configuration and every command clears its environment (`common::git_repo`); the daemon tests are
//! built as the other file-run tests are (`file_run.rs`, `reprompt.rs`).

use super::common::TempRoot;
use super::common::git_repo::GitRepo;
use super::file_run::{Fixture, Setup, git};
use super::quick_run::{call, quick_in, report_with, run_get, wait_ended};
use super::reprompt::{WORDS, resumed_report};
use collide_core::owner::{
    RunListParams, RunListResult, RunRecordParams, RunRecordResult, RunWarning, RunWarningKind,
};
use collide_core::pack::Role;
use collide_host::instruction_warning::{
    changed_instruction_files, instruction_warning, not_yet_warned,
};
use serde_json::{Value, json};
use std::fs;

// ---- reading the changes -----------------------------------------------------------------------------------------

/// What git is given in a daemon: no global or system configuration, a home in the temp root.
fn git_env(repo: &GitRepo) -> Vec<(String, String)> {
    let home = repo.root().path().to_str().unwrap().to_string();
    vec![
        ("HOME".to_string(), home),
        ("GIT_CONFIG_GLOBAL".to_string(), "/dev/null".to_string()),
        ("GIT_CONFIG_SYSTEM".to_string(), "/dev/null".to_string()),
        ("GIT_CONFIG_NOSYSTEM".to_string(), "1".to_string()),
    ]
}

/// A repository with a root `CLAUDE.md`, a skill, a README and code, committed; and that commit's id.
fn repo_with_instructions() -> (GitRepo, String) {
    let repo = GitRepo::new();
    repo.write("CLAUDE.md", b"root\n");
    repo.write(".claude/skills/stand-in/SKILL.md", b"skill\n");
    repo.write(".claude/commands/stand-in.md", b"command\n");
    repo.write("README.md", b"readme\n");
    repo.write("src/lib.rs", b"fn a() {}\n");
    let base = repo.commit_all("base");
    (repo, base)
}

fn changed(repo: &GitRepo, base: &str) -> Vec<String> {
    changed_instruction_files(&git_env(repo), repo.work(), base).expect("git reads the changes")
}

#[test]
fn a_workspace_that_changed_nothing_has_no_changed_files() {
    let (repo, base) = repo_with_instructions();
    assert_eq!(changed(&repo, &base), Vec::<String>::new());
}

#[test]
fn a_committed_edit_of_claude_md_is_a_change() {
    let (repo, base) = repo_with_instructions();
    repo.write("CLAUDE.md", b"edited\n");
    repo.commit_all("edit");
    assert_eq!(changed(&repo, &base), ["CLAUDE.md"]);
}

#[test]
fn an_uncommitted_edit_is_a_change() {
    let (repo, base) = repo_with_instructions();
    repo.write("CLAUDE.md", b"edited\n");
    assert_eq!(changed(&repo, &base), ["CLAUDE.md"]);
}

#[test]
fn a_staged_edit_is_a_change() {
    let (repo, base) = repo_with_instructions();
    repo.write(".claude/skills/stand-in/SKILL.md", b"edited\n");
    repo.git(&["add", "--all"]);
    assert_eq!(changed(&repo, &base), [".claude/skills/stand-in/SKILL.md"]);
}

#[test]
fn a_new_nested_claude_md_that_is_not_added_is_a_change() {
    let (repo, base) = repo_with_instructions();
    repo.write("pkg/deep/CLAUDE.md", b"new\n");
    assert_eq!(changed(&repo, &base), ["pkg/deep/CLAUDE.md"]);
}

#[test]
fn a_new_skill_file_is_a_change() {
    let (repo, base) = repo_with_instructions();
    repo.write(".claude/skills/another/SKILL.md", b"new\n");
    assert_eq!(changed(&repo, &base), [".claude/skills/another/SKILL.md"]);
}

#[test]
fn a_deleted_skill_is_a_change_committed_or_not() {
    let (repo, base) = repo_with_instructions();
    repo.remove(".claude/skills/stand-in/SKILL.md");
    assert_eq!(changed(&repo, &base), [".claude/skills/stand-in/SKILL.md"]);
    repo.commit_all("remove the skill");
    assert_eq!(changed(&repo, &base), [".claude/skills/stand-in/SKILL.md"]);
}

#[test]
fn a_deleted_claude_md_is_a_change() {
    let (repo, base) = repo_with_instructions();
    repo.remove("CLAUDE.md");
    assert_eq!(changed(&repo, &base), ["CLAUDE.md"]);
}

#[test]
fn a_change_to_code_to_the_readme_or_to_a_command_is_not_a_change() {
    let (repo, base) = repo_with_instructions();
    repo.write("README.md", b"edited\n");
    repo.write("src/lib.rs", b"fn b() {}\n");
    repo.write(".claude/commands/stand-in.md", b"edited\n");
    repo.write("notes/new.md", b"new\n");
    repo.commit_all("edits");
    repo.write("src/more.rs", b"fn c() {}\n");
    assert_eq!(changed(&repo, &base), Vec::<String>::new());
}

#[test]
fn each_file_is_named_once_in_path_order_whatever_the_ways_it_changed() {
    let (repo, base) = repo_with_instructions();
    repo.write("CLAUDE.md", b"first\n");
    repo.commit_all("first edit");
    repo.write("CLAUDE.md", b"second\n");
    repo.write("a dir/CLAUDE.md", b"a name with a space\n");
    repo.write("z/CLAUDE.md", b"last\n");
    assert_eq!(
        changed(&repo, &base),
        ["CLAUDE.md", "a dir/CLAUDE.md", "z/CLAUDE.md"]
    );
}

#[test]
fn a_file_that_is_back_as_it_was_is_not_a_change() {
    let (repo, base) = repo_with_instructions();
    repo.write("CLAUDE.md", b"edited\n");
    repo.commit_all("edit");
    repo.write("CLAUDE.md", b"root\n");
    repo.commit_all("put it back");
    assert_eq!(changed(&repo, &base), Vec::<String>::new());
}

#[test]
fn a_base_that_is_not_a_commit_is_an_error_naming_it() {
    let (repo, _) = repo_with_instructions();
    let error =
        changed_instruction_files(&git_env(&repo), repo.work(), "not-a-commit").unwrap_err();
    assert!(error.contains("not-a-commit"), "{error}");
}

// ---- the warning -------------------------------------------------------------------------------------------------

fn role(text: Value) -> Role {
    serde_json::from_value(text).expect("a role")
}

#[test]
fn a_changed_instruction_file_gives_a_warning_with_the_phase_and_the_file() {
    let (repo, base) = repo_with_instructions();
    repo.write("CLAUDE.md", b"edited\n");
    for role in [
        role(json!({ "agent": "claude" })),
        role(json!({ "agent": "claude", "repo_instructions": "worktree" })),
    ] {
        let warning = instruction_warning(&role, &git_env(&repo), repo.work(), &base, "build")
            .unwrap()
            .expect("a warning");
        assert_eq!(
            warning,
            RunWarning {
                kind: RunWarningKind::InstructionFilesChanged,
                phase: "build".to_string(),
                files: vec!["CLAUDE.md".to_string()],
            }
        );
    }
}

#[test]
fn no_changed_instruction_file_gives_no_warning() {
    let (repo, base) = repo_with_instructions();
    repo.write("README.md", b"edited\n");
    let none = instruction_warning(
        &role(json!({ "agent": "claude" })),
        &git_env(&repo),
        repo.work(),
        &base,
        "build",
    )
    .unwrap();
    assert_eq!(none, None);
}

#[test]
fn a_role_with_repo_instructions_none_gives_no_warning_whatever_changed() {
    let (repo, base) = repo_with_instructions();
    repo.write("CLAUDE.md", b"edited\n");
    repo.write("sub/CLAUDE.md", b"new\n");
    let none = instruction_warning(
        &role(json!({ "agent": "claude", "repo_instructions": "none" })),
        &git_env(&repo),
        repo.work(),
        &base,
        "build",
    )
    .unwrap();
    assert_eq!(none, None);
}

#[test]
fn a_file_the_run_was_warned_of_is_not_named_again() {
    let earlier = [RunWarning {
        kind: RunWarningKind::InstructionFilesChanged,
        phase: "build".to_string(),
        files: vec!["CLAUDE.md".to_string()],
    }];
    let files = |names: &[&str]| {
        names
            .iter()
            .map(|name| name.to_string())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        not_yet_warned(&earlier, files(&["CLAUDE.md", "sub/CLAUDE.md"])),
        files(&["sub/CLAUDE.md"])
    );
    assert_eq!(
        not_yet_warned(&earlier, files(&["CLAUDE.md"])),
        Vec::<String>::new()
    );
    assert_eq!(
        not_yet_warned(&[], files(&["CLAUDE.md"])),
        files(&["CLAUDE.md"])
    );
}

// ---- through the daemon ------------------------------------------------------------------------------------------

fn write_and_commit(path: &str, text: &str, message: &str) -> [Value; 2] {
    [
        super::quick_run::write_file(path, text),
        json!({ "git_commit": message }),
    ]
}

/// A run whose agent does `first`, then a re-prompt run whose agent does nothing more: the warnings of both.
fn warnings_after(
    root: &TempRoot,
    first: &[Value],
    prepare: impl FnOnce(&std::path::Path),
) -> (
    Vec<RunWarning>,
    Vec<RunWarning>,
    super::accept::Reviewed,
    String,
) {
    let first_recording = report_with(root, first);
    let resumed = resumed_report(root, &[]);
    let (fx, board, key) = {
        let fx = Fixture::new(
            &first_recording,
            Setup {
                resumed: Some(resumed),
                ..Setup::default()
            },
        );
        prepare(&fx.repo);
        quick_in(fx)
    };
    let mut reviewed = super::accept::reviewed_in((fx, board, key));
    let before = run_get(&mut reviewed.fx, &reviewed.run).summary.warnings;
    let second = reviewed.reprompted(WORDS);
    wait_ended(&mut reviewed.fx, &second.run);
    let after = run_get(&mut reviewed.fx, &second.run).summary.warnings;
    (before, after, reviewed, second.run)
}

#[test]
fn a_reprompt_after_a_run_that_changed_no_instruction_file_has_no_warning() {
    let root = TempRoot::new();
    let mut steps = write_and_commit("a.txt", "from the agent\n", "Add a.txt").to_vec();
    steps.extend(write_and_commit("README", "changed\n", "Edit the readme"));
    let (before, after, _reviewed, _) = warnings_after(&root, &steps, |_| {});
    assert_eq!(before, Vec::<RunWarning>::new());
    assert_eq!(after, Vec::<RunWarning>::new());
}

#[test]
fn a_reprompt_after_a_run_added_claude_md_warns_with_the_phase_and_the_file() {
    let root = TempRoot::new();
    let steps = write_and_commit("CLAUDE.md", "new rules\n", "Add CLAUDE.md");
    let (before, after, _reviewed, _) = warnings_after(&root, &steps, |_| {});
    assert_eq!(
        before,
        Vec::<RunWarning>::new(),
        "the first run's phase started before the change"
    );
    assert_eq!(
        after,
        [RunWarning {
            kind: RunWarningKind::InstructionFilesChanged,
            phase: "build".to_string(),
            files: vec!["CLAUDE.md".to_string()],
        }]
    );
}

#[test]
fn a_nested_claude_md_a_changed_skill_and_a_deleted_skill_are_each_named_once() {
    let root = TempRoot::new();
    let mut steps =
        write_and_commit("sub/CLAUDE.md", "nested\n", "Add a nested CLAUDE.md").to_vec();
    steps.extend(write_and_commit(
        ".claude/skills/edited/SKILL.md",
        "after\n",
        "Edit a skill",
    ));
    steps.push(json!({ "git": ["rm", "-q", "-r", ".claude/skills/removed"] }));
    steps.push(json!({ "git_commit": "Remove a skill" }));
    steps.extend(write_and_commit("src/code.rs", "fn a() {}\n", "Add code"));
    let (_, after, _reviewed, _) = warnings_after(&root, &steps, |repo| {
        for skill in ["edited", "removed"] {
            let dir = repo.join(".claude/skills").join(skill);
            fs::create_dir_all(&dir).unwrap();
            fs::write(dir.join("SKILL.md"), "before\n").unwrap();
        }
        git(repo, &["add", "."]);
        git(repo, &["commit", "-q", "-m", "skills"]);
    });
    assert_eq!(after.len(), 1, "{after:?}");
    assert_eq!(
        after[0].files,
        [
            ".claude/skills/edited/SKILL.md",
            ".claude/skills/removed/SKILL.md",
            "sub/CLAUDE.md"
        ]
    );
}

#[test]
fn an_edit_the_agent_left_uncommitted_is_named_too() {
    let root = TempRoot::new();
    let steps = [super::quick_run::write_file(
        "CLAUDE.md",
        "left in the tree\n",
    )];
    let (_, after, _reviewed, _) = warnings_after(&root, &steps, |_| {});
    assert_eq!(after.len(), 1, "{after:?}");
    assert_eq!(after[0].files, ["CLAUDE.md"]);
}

#[test]
fn the_warning_is_in_the_run_list_and_the_run_record_and_the_old_run_has_none() {
    let root = TempRoot::new();
    let steps = write_and_commit("CLAUDE.md", "new rules\n", "Add CLAUDE.md");
    let (_, after, mut reviewed, second) = warnings_after(&root, &steps, |_| {});
    assert_eq!(after.len(), 1, "{after:?}");
    let listed: RunListResult = call(
        &mut reviewed.fx,
        "run.list",
        &RunListParams {
            board: Some(reviewed.board.clone()),
            scope: Default::default(),
            task: Some(reviewed.key.clone()),
            limit: None,
            cursor: None,
        },
    );
    let of = |run: &str| {
        listed
            .runs
            .iter()
            .find(|summary| summary.id == run)
            .unwrap()
            .warnings
            .clone()
    };
    assert_eq!(of(&second), after);
    assert_eq!(of(&reviewed.run), Vec::<RunWarning>::new());
    let record: RunRecordResult = call(
        &mut reviewed.fx,
        "run.record",
        &RunRecordParams {
            run: second.clone(),
        },
    );
    assert_eq!(record.warnings, after);
}

#[test]
fn the_warning_is_still_there_after_the_daemon_starts_again() {
    let root = TempRoot::new();
    let steps = write_and_commit("CLAUDE.md", "new rules\n", "Add CLAUDE.md");
    let (_, after, mut reviewed, second) = warnings_after(&root, &steps, |_| {});
    assert_eq!(after.len(), 1);
    reviewed.fx.stop_daemon();
    reviewed.fx.start_daemon();
    assert_eq!(run_get(&mut reviewed.fx, &second).summary.warnings, after);
    let record: RunRecordResult = call(
        &mut reviewed.fx,
        "run.record",
        &RunRecordParams { run: second },
    );
    assert_eq!(record.warnings, after);
}

#[test]
fn the_first_phase_of_a_file_run_in_a_fresh_workspace_has_no_warning_even_with_instruction_files() {
    let root = TempRoot::new();
    let recording = report_with(&root, &[]);
    let mut fx = Fixture::new(&recording, Setup::default());
    let repo = fx.repo.clone();
    fs::write(repo.join("CLAUDE.md"), "committed rules\n").unwrap();
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-q", "-m", "rules"]);
    let run = fx.started();
    fx.wait_final(&run);
    let summary = run_get(&mut fx, &run).summary;
    assert_eq!(summary.warnings, Vec::<RunWarning>::new());
}
