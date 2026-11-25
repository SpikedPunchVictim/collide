//! `task.changes` and `task.diff` through the daemon, and the phase heads they read (ts-0188-kb; 0.8, Phase; 0.11, Task
//! drawer, Changes): a run records the commit each phase started and ended at, a phase that is active lists its edits
//! from the working tree, and the commits on the line that name the task are listed outside every run's span. Each
//! test has a daemon of its own under a temp root (`Fixture`), a fake `claude`, and git that reads no configuration;
//! every command the test runs clears its environment, and nothing here reads the owner's files.

use super::common::{TempRoot, wait_until};
use super::file_run::{Fixture, Setup};
use super::quick_run::{
    answer, call, git_out, quick, rpc_error, run_get, started, wait_ended, wait_item, write_file,
};
use collide_core::owner::{
    ChangesPlace, DiffStatus, QueueAnswer, QueueAnswerResult, QueueKind, TaskChangesParams,
    TaskChangesResult, TaskDiffParams, TaskDiffResult, code,
};
use collide_core::state::RunState;
use serde_json::json;
use std::fs;
use std::path::Path;
use std::time::Duration;

const PLAN_TASK: &str = "plan:ts-0001-aa";
const WAIT: Duration = Duration::from_secs(60);

fn changes(fx: &mut Fixture, board: &str, task: &str) -> TaskChangesResult {
    call(
        fx,
        "task.changes",
        &TaskChangesParams {
            board: board.to_string(),
            task: task.to_string(),
        },
    )
}

fn diff_params(board: &str, task: &str, path: &str) -> TaskDiffParams {
    TaskDiffParams {
        board: board.to_string(),
        task: task.to_string(),
        run: None,
        commit: None,
        path: path.to_string(),
        whole: false,
    }
}

fn run_diff_of(
    fx: &mut Fixture,
    board: &str,
    task: &str,
    run: &str,
    path: &str,
    whole: bool,
) -> TaskDiffResult {
    let mut params = diff_params(board, task, path);
    params.run = Some(run.to_string());
    params.whole = whole;
    call(fx, "task.diff", &params)
}

fn commit_diff_of(
    fx: &mut Fixture,
    board: &str,
    task: &str,
    commit: &str,
    path: &str,
    whole: bool,
) -> TaskDiffResult {
    let mut params = diff_params(board, task, path);
    params.commit = Some(commit.to_string());
    params.whole = whole;
    call(fx, "task.diff", &params)
}

/// Makes the run a run of `task`, as a plan task's run is in the store. A prompt run is of a scratch task, whose short
/// id names no commit, so the commit tests need the run to be of a task whose id does.
fn rekey(fx: &Fixture, run: &str, task: &str) {
    let conn = rusqlite::Connection::open(fx.paths().store_dir.join("store.db")).unwrap();
    let changed = conn
        .execute(
            "UPDATE run_tasks SET task = ?2 WHERE run = ?1",
            rusqlite::params![run, task],
        )
        .unwrap();
    assert_eq!(changed, 1, "the run has one task");
}

fn heads_of(fx: &Fixture, run: &str) -> Vec<collide_host::run_store::PhaseHeads> {
    collide_host::store::Store::open(&fx.paths().store_dir)
        .unwrap()
        .phase_heads(run)
        .unwrap()
}

/// A hand commit in the person's checkout, which the daemon's board is on.
fn hand_commit(repo: &Path, file: &str, text: &str, message: &str) -> String {
    fs::write(repo.join(file), text).unwrap();
    git_out(repo, &["add", "-A"]);
    git_out(repo, &["commit", "-q", "-m", message]);
    git_out(repo, &["rev-parse", "HEAD"])
}

fn paths(files: &[collide_core::owner::DiffFile]) -> Vec<&str> {
    files.iter().map(|file| file.path.as_str()).collect()
}

// ---- the heads ----------------------------------------------------------------------------------------------------

#[test]
fn a_file_run_records_the_commit_its_phase_started_and_ended_at() {
    let root = TempRoot::new();
    let mut fx = Fixture::new(
        &super::quick_run::report_with(
            &root,
            &[write_file("a.txt", "a\n"), json!({"git_commit": "Add a"})],
        ),
        Setup::default(),
    );
    let start_commit = git_out(&fx.repo, &["rev-parse", "HEAD"]);

    let run = fx.started();
    fx.wait_final(&run);
    wait_until("the run ends", WAIT, || {
        heads_of(&fx, &run)
            .first()
            .is_some_and(|heads| heads.end_head.is_some())
    });

    let heads = heads_of(&fx, &run);
    assert_eq!(heads.len(), 1);
    assert_eq!(heads[0].phase, "build");
    assert_eq!(heads[0].start_head, start_commit);
    assert_eq!(
        heads[0].end_head.as_deref(),
        Some(git_out(&fx.workspace_dir(&run), &["rev-parse", "HEAD"]).as_str())
    );
    assert_ne!(heads[0].end_head.as_deref(), Some(start_commit.as_str()));
    let got = run_get(&mut fx, &run);
    assert_eq!(
        got.summary.phases[0].start_head.as_deref(),
        Some(start_commit.as_str())
    );
    assert_eq!(got.summary.phases[0].end_head, heads[0].end_head);
}

#[test]
fn a_prompt_run_ends_at_the_commit_collide_made_of_what_the_agent_left() {
    let root = TempRoot::new();
    let (mut fx, board, key) = quick(
        &root,
        &[
            write_file("a.txt", "a\n"),
            json!({"git_commit": "Add a"}),
            write_file("b.txt", "b\n"),
        ],
    );
    let run = started(&mut fx, &board, &key);

    wait_item(&mut fx, &run, QueueKind::Review);

    let tree = fx.workspace_dir(&run);
    let end = heads_of(&fx, &run)[0].end_head.clone();
    assert_eq!(
        end.as_deref(),
        Some(git_out(&tree, &["rev-parse", "HEAD"]).as_str())
    );
    assert_eq!(
        git_out(&tree, &["log", "-1", "--format=%s"]),
        "Commit what the agent left uncommitted"
    );
}

// ---- a task's changes ---------------------------------------------------------------------------------------------

#[test]
fn a_phase_that_is_active_lists_its_edits_from_the_working_tree_and_then_its_span() {
    let root = TempRoot::new();
    let held = root.join("held");
    let release = root.join("release");
    let (mut fx, board, key) = quick(
        &root,
        &[
            write_file("a.txt", "one\ntwo\n"),
            write_file("README", "hello\nmore\n"),
            json!({"touch": held.to_str().unwrap()}),
            json!({"wait_file": release.to_str().unwrap()}),
        ],
    );
    let run = started(&mut fx, &board, &key);
    wait_until("the agent has written", WAIT, || held.exists());

    let live = changes(&mut fx, &board, &key);

    assert_eq!(live.runs.len(), 1);
    let one = &live.runs[0];
    assert_eq!(one.run, run);
    assert_eq!(one.place, ChangesPlace::Worktree);
    assert_eq!(one.words.place, format!("Run {run}'s worktree"));
    assert_eq!(
        one.words.how,
        "since the build phase began, including edits not committed yet"
    );
    assert_eq!(paths(&one.files), ["README", "a.txt"]);
    assert_eq!(one.files[1].status, DiffStatus::Added);
    assert_eq!(one.files[1].additions, Some(2));
    assert_eq!(one.files[0].additions, Some(1));
    assert_eq!(one.note, None);
    assert_eq!(heads_of(&fx, &run)[0].end_head, None);

    fs::write(&release, "").unwrap();
    wait_item(&mut fx, &run, QueueKind::Review);
    let done = changes(&mut fx, &board, &key);

    let one = &done.runs[0];
    assert_eq!(
        one.words.how,
        "from where the build phase began to where it ended"
    );
    assert_eq!(paths(&one.files), ["README", "a.txt"]);
}

#[test]
fn what_is_written_after_the_last_phase_ended_is_not_the_runs() {
    let root = TempRoot::new();
    let (mut fx, board, key) = quick(
        &root,
        &[write_file("a.txt", "a\n"), json!({"git_commit": "Add a"})],
    );
    let run = started(&mut fx, &board, &key);
    wait_item(&mut fx, &run, QueueKind::Review);
    // The person edits the worktree after the run, and the worktree is still the run's.
    fs::write(fx.workspace_dir(&run).join("later.txt"), "later\n").unwrap();

    let found = changes(&mut fx, &board, &key);

    assert_eq!(paths(&found.runs[0].files), ["a.txt"]);
}

#[test]
fn a_run_from_before_heads_were_kept_lists_no_files_and_says_why() {
    let root = TempRoot::new();
    let (mut fx, board, key) = quick(&root, &[write_file("a.txt", "a\n")]);
    let run = started(&mut fx, &board, &key);
    wait_item(&mut fx, &run, QueueKind::Review);
    let conn = rusqlite::Connection::open(fx.paths().store_dir.join("store.db")).unwrap();
    conn.execute("DELETE FROM phase_heads WHERE run = ?1", [&run])
        .unwrap();

    let found = changes(&mut fx, &board, &key);

    assert_eq!(found.runs.len(), 1);
    assert!(found.runs[0].files.is_empty());
    assert_eq!(found.runs[0].note.as_deref(), Some("no changes recorded"));
    let got = run_get(&mut fx, &run);
    assert_eq!(got.summary.phases[0].start_head, None);
    let (error, message) = rpc_error(
        &mut fx,
        "task.diff",
        &TaskDiffParams {
            run: Some(run.clone()),
            ..diff_params(&board, &key, "a.txt")
        },
    );
    assert_eq!(error, code::NOT_FOUND);
    assert_eq!(message, "no changes recorded");
}

// ---- commits ------------------------------------------------------------------------------------------------------

#[test]
fn commits_that_name_the_task_are_listed_and_others_are_not() {
    let root = TempRoot::new();
    let (mut fx, board, _) = quick(&root, &[]);
    let repo = fx.repo.clone();
    let named = hand_commit(&repo, "a.txt", "1\n", "Fix the keel (ts-0001-aa)");
    hand_commit(&repo, "b.txt", "2\n", "Raise the mast for ts-0002-bb");
    hand_commit(&repo, "c.txt", "3\n", "Rework ts-0001-aab");
    let body = hand_commit(&repo, "d.txt", "4\n", "Tidy\n\nCloses ts-0001-aa.");

    let found = changes(&mut fx, &board, PLAN_TASK);

    assert!(found.runs.is_empty());
    let ids: Vec<&str> = found.commits.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids, [body.as_str(), named.as_str()], "newest first");
    assert_eq!(found.commits[1].subject, "Fix the keel (ts-0001-aa)");
    assert_eq!(paths(&found.commits[1].files), ["a.txt"]);
    assert_eq!(found.commits[1].files[0].status, DiffStatus::Added);
    assert!(found.commits[1].date_ms > 0);
    assert_eq!(found.commits_words.place, "Your checkout, line main");
    assert!(
        found
            .commits_words
            .how
            .starts_with("2 commits that name ts-0001-aa: "),
        "{}",
        found.commits_words.how
    );
    assert!(found.commits_words.how.ends_with("No run built it."));
}

#[test]
fn a_scratch_task_has_no_commits_because_its_id_is_a_short_number() {
    let root = TempRoot::new();
    let (mut fx, board, key) = quick(&root, &[]);
    let repo = fx.repo.clone();
    hand_commit(&repo, "a.txt", "1\n", "Fix 1");

    let found = changes(&mut fx, &board, &key);

    assert!(found.commits.is_empty());
}

#[test]
fn a_commit_inside_a_runs_span_is_listed_once_under_the_run() {
    let root = TempRoot::new();
    let (mut fx, board, key) = quick(
        &root,
        &[
            write_file("a.txt", "a\n"),
            json!({"git_commit": "Work on ts-0001-aa"}),
        ],
    );
    let run = started(&mut fx, &board, &key);
    let item = wait_item(&mut fx, &run, QueueKind::Review);
    let merged = answer(&mut fx, &item, QueueAnswer::Accept {});
    assert!(
        matches!(merged, QueueAnswerResult::Merged { .. }),
        "{merged:?}"
    );
    rekey(&fx, &run, PLAN_TASK);
    let repo = fx.repo.clone();
    let hand = hand_commit(&repo, "b.txt", "b\n", "Follow up ts-0001-aa by hand");
    // The accepted work is on the line, and its message names the task.
    assert!(git_out(&repo, &["log", "--format=%s", "main"]).contains("Work on ts-0001-aa"));

    let found = changes(&mut fx, &board, PLAN_TASK);

    assert_eq!(found.runs.len(), 1);
    assert_eq!(paths(&found.runs[0].files), ["a.txt"]);
    let ids: Vec<&str> = found.commits.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids, [hand.as_str()]);
    assert!(
        found
            .commits_words
            .how
            .starts_with("1 commit that names ts-0001-aa: "),
        "{}",
        found.commits_words.how
    );
    assert!(
        !found.commits_words.how.contains("No run"),
        "{}",
        found.commits_words.how
    );
}

// ---- task.diff ----------------------------------------------------------------------------------------------------

#[test]
fn task_diff_gives_a_runs_patch_and_the_whole_file_with_its_changed_lines_marked() {
    let root = TempRoot::new();
    let (mut fx, board, key) = quick(
        &root,
        &[
            write_file("README", "hello\nmore\n"),
            json!({"git_commit": "Edit"}),
        ],
    );
    let run = started(&mut fx, &board, &key);
    wait_item(&mut fx, &run, QueueKind::Review);

    let patch = run_diff_of(&mut fx, &board, &key, &run, "README", false);
    let whole = run_diff_of(&mut fx, &board, &key, &run, "README", true);

    let shown = patch.patch.expect("a patch");
    assert!(shown.text.contains(" hello\n+more\n"), "{}", shown.text);
    assert!(!shown.truncated && patch.whole.is_none() && patch.note.is_none());
    let all = whole.whole.expect("the whole file");
    let lines: Vec<(u32, &str, bool)> = all
        .lines
        .iter()
        .map(|l| (l.n, l.text.as_str(), l.changed))
        .collect();
    assert_eq!(lines, [(1, "hello", false), (2, "more", true)]);
    assert!(whole.patch.is_none());
}

#[test]
fn task_diff_of_a_phase_that_is_active_reads_the_working_tree() {
    let root = TempRoot::new();
    let held = root.join("held");
    let release = root.join("release");
    let (mut fx, board, key) = quick(
        &root,
        &[
            write_file("new.txt", "x\ny\n"),
            json!({"touch": held.to_str().unwrap()}),
            json!({"wait_file": release.to_str().unwrap()}),
        ],
    );
    let run = started(&mut fx, &board, &key);
    wait_until("the agent has written", WAIT, || held.exists());

    let patch = run_diff_of(&mut fx, &board, &key, &run, "new.txt", false);
    let whole = run_diff_of(&mut fx, &board, &key, &run, "new.txt", true);

    assert!(patch.patch.unwrap().text.contains("+x\n+y\n"));
    assert_eq!(whole.whole.unwrap().lines.len(), 2);
    fs::write(&release, "").unwrap();
    wait_item(&mut fx, &run, QueueKind::Review);
}

#[test]
fn task_diff_gives_a_commits_patch_and_whole_file() {
    let root = TempRoot::new();
    let (mut fx, board, _) = quick(&root, &[]);
    let repo = fx.repo.clone();
    let commit = hand_commit(&repo, "a.txt", "one\ntwo\n", "Add a for ts-0001-aa");

    let patch = commit_diff_of(&mut fx, &board, PLAN_TASK, &commit, "a.txt", false);
    let whole = commit_diff_of(&mut fx, &board, PLAN_TASK, &commit, "a.txt", true);

    assert!(patch.patch.unwrap().text.contains("+one\n+two\n"));
    let lines = whole.whole.unwrap().lines;
    assert_eq!(lines.len(), 2);
    assert!(lines.iter().all(|line| line.changed));
}

#[test]
fn task_diff_of_a_binary_file_says_it_is_not_text() {
    let root = TempRoot::new();
    let (mut fx, board, _) = quick(&root, &[]);
    let repo = fx.repo.clone();
    fs::write(repo.join("img.bin"), [0u8, 1, 2]).unwrap();
    git_out(&repo, &["add", "-A"]);
    git_out(&repo, &["commit", "-q", "-m", "Add an image"]);
    let commit = git_out(&repo, &["rev-parse", "HEAD"]);

    let patch = commit_diff_of(&mut fx, &board, PLAN_TASK, &commit, "img.bin", false);
    let whole = commit_diff_of(&mut fx, &board, PLAN_TASK, &commit, "img.bin", true);

    assert!(patch.patch.unwrap().text.contains("Binary files"));
    let all = whole.whole.unwrap();
    assert!(all.binary && all.lines.is_empty());
    assert_eq!(whole.note.as_deref(), Some("the file is not text"));
}

#[test]
fn a_whole_file_is_cut_at_the_patch_cap_with_a_note() {
    let root = TempRoot::new();
    let (mut fx, board, _) = quick(&root, &[]);
    let repo = fx.repo.clone();
    let line = format!("{}\n", "z".repeat(99));
    let text = line.repeat(collide_host::run_diff::MAX_PATCH_BYTES / 100 + 100);
    let commit = hand_commit(&repo, "big.txt", &text, "Add big");

    let whole = commit_diff_of(&mut fx, &board, PLAN_TASK, &commit, "big.txt", true);
    let patch = commit_diff_of(&mut fx, &board, PLAN_TASK, &commit, "big.txt", false);

    let all = whole.whole.unwrap();
    assert!(all.truncated);
    assert_eq!(all.bytes, text.len() as u64);
    assert!(all.lines.len() * 100 <= collide_host::run_diff::MAX_PATCH_BYTES);
    assert!(whole.note.unwrap().contains("byte cap"));
    let cut = patch.patch.unwrap();
    assert!(cut.truncated);
    assert_eq!(cut.text.len(), collide_host::run_diff::MAX_PATCH_BYTES);
    assert!(patch.note.unwrap().contains("is cut at"));
}

#[test]
fn task_diff_refuses_a_request_that_names_both_or_neither_and_a_commit_that_is_not_an_id() {
    let root = TempRoot::new();
    let (mut fx, board, key) = quick(&root, &[]);
    let neither = diff_params(&board, &key, "a.txt");
    let both = TaskDiffParams {
        run: Some("R-1".into()),
        commit: Some("abc".into()),
        ..diff_params(&board, &key, "a.txt")
    };
    let option = TaskDiffParams {
        commit: Some("--all".into()),
        ..diff_params(&board, &key, "a.txt")
    };
    let other_run = TaskDiffParams {
        run: Some("R-99".into()),
        ..diff_params(&board, &key, "a.txt")
    };

    for params in [&neither, &both, &option] {
        let (error, _) = rpc_error(&mut fx, "task.diff", params);
        assert_eq!(error, code::INVALID_PARAMS);
    }
    let (error, _) = rpc_error(&mut fx, "task.diff", &other_run);
    assert_eq!(error, code::NOT_FOUND);
}

#[test]
fn a_path_outside_the_repository_is_refused() {
    let root = TempRoot::new();
    let (mut fx, board, _) = quick(&root, &[]);
    let repo = fx.repo.clone();
    let commit = hand_commit(&repo, "a.txt", "a\n", "Add a");
    let params = TaskDiffParams {
        commit: Some(commit),
        ..diff_params(&board, PLAN_TASK, "../outside")
    };

    let (error, _) = rpc_error(&mut fx, "task.diff", &params);

    assert_eq!(error, code::INTERNAL_ERROR);
}

#[test]
fn a_diff_driver_and_a_textconv_the_repository_names_never_run_for_the_changes() {
    let root = TempRoot::new();
    let mark = root.join("driver-ran");
    let script = root.join("driver.sh");
    fs::write(
        &script,
        format!("#!/bin/sh\ntouch '{}'\ncat \"$1\"\n", mark.display()),
    )
    .unwrap();
    fs::set_permissions(&script, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    let held = root.join("held");
    let release = root.join("release");
    let (mut fx, board, key) = quick(
        &root,
        &[
            write_file("a.txt", "agent\n"),
            json!({"git_commit": "Work on ts-0001-aa"}),
            write_file("b.txt", "uncommitted\n"),
            json!({"touch": held.to_str().unwrap()}),
            json!({"wait_file": release.to_str().unwrap()}),
        ],
    );
    let repo = fx.repo.clone();
    git_out(
        &repo,
        &["config", "diff.evil.command", script.to_str().unwrap()],
    );
    git_out(
        &repo,
        &["config", "diff.evil.textconv", script.to_str().unwrap()],
    );
    fs::write(repo.join(".gitattributes"), "*.txt diff=evil\n").unwrap();
    fs::write(repo.join("a.txt"), "before\n").unwrap();
    git_out(&repo, &["add", "-A"]);
    git_out(&repo, &["commit", "-q", "-m", "Set the drivers up"]);
    let run = started(&mut fx, &board, &key);
    wait_until("the agent has written", WAIT, || held.exists());
    rekey(&fx, &run, PLAN_TASK);
    let hand = hand_commit(&repo, "c.txt", "hand\n", "Hand edit of ts-0001-aa");
    assert!(!mark.exists(), "the setup ran a driver");

    let live = changes(&mut fx, &board, PLAN_TASK);
    let live_patch = run_diff_of(&mut fx, &board, PLAN_TASK, &run, "a.txt", false);
    let live_whole = run_diff_of(&mut fx, &board, PLAN_TASK, &run, "b.txt", true);
    fs::write(&release, "").unwrap();
    wait_item(&mut fx, &run, QueueKind::Review);
    let done = changes(&mut fx, &board, PLAN_TASK);
    let done_patch = run_diff_of(&mut fx, &board, PLAN_TASK, &run, "a.txt", false);
    let by_hand = commit_diff_of(&mut fx, &board, PLAN_TASK, &hand, "c.txt", false);
    let by_hand_whole = commit_diff_of(&mut fx, &board, PLAN_TASK, &hand, "c.txt", true);

    assert_eq!(paths(&live.runs[0].files), ["a.txt", "b.txt"]);
    assert_eq!(paths(&done.runs[0].files), ["a.txt", "b.txt"]);
    assert!(live_patch.patch.unwrap().text.contains("+agent"));
    assert_eq!(live_whole.whole.unwrap().lines.len(), 1);
    assert!(done_patch.patch.unwrap().text.contains("+agent"));
    assert!(by_hand.patch.unwrap().text.contains("+hand"));
    assert_eq!(by_hand_whole.whole.unwrap().lines.len(), 1);
    assert_eq!(done.commits.len(), 1);
    assert!(!mark.exists(), "a diff driver or a textconv ran");
    // The control: the same config runs the driver when git is not told not to.
    git_out(&repo, &["diff", "HEAD~1", "HEAD"]);
    assert!(
        mark.exists(),
        "the drivers are set up, so git runs them when it is let"
    );
}

#[test]
fn a_run_that_is_final_has_its_state_in_the_changes() {
    let root = TempRoot::new();
    let (mut fx, board, key) = quick(&root, &[write_file("a.txt", "a\n")]);
    let run = started(&mut fx, &board, &key);
    wait_item(&mut fx, &run, QueueKind::Review);
    wait_ended(&mut fx, &run);

    let found = changes(&mut fx, &board, &key);

    assert!(matches!(found.runs[0].state, RunState::Ended { .. }));
    assert!(
        found.runs[0]
            .workspace
            .as_deref()
            .is_some_and(|w| w.ends_with(&run))
    );
}
