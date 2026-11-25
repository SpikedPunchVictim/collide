//! A prompt run of `collide/quick` through the daemon, up to review (ts-0078-ye; Stage 3c): `run.start` on a board
//! and a scratch task makes a worktree on `collide/<run>`, runs the builder under a shim with fake Claude as
//! `claude`, moves the task by category and ends in a `Review` Queue item; `run.diff` shows what the branch changed
//! since the commit recorded at start. Each test has a daemon of its own under a temp root (`Fixture`): git reads no
//! configuration, every command the test runs clears its environment, and nothing here reads the owner's files.

use super::common::{TempRoot, wait_until};
use super::fake::recording;
use super::file_run::{Fixture, Setup, no_result_recording};
use super::pause::{edited, written};
use collide_core::agent::PermissionReply;
use collide_core::events::EventBody;
use collide_core::item::Category;
use collide_core::owner::{
    AnswerKind, BoardOpenParams, BoardOpenResult, CollapsedFolder, DiffFile, DiffStatus,
    QueueAnswer, QueueAnswerResult, QueueItem, QueueKind, RunDiffParams, RunDiffResult,
    RunGetParams, RunGetResult, RunListParams, RunListResult, RunStartParams, RunStartResult,
    RunStopParams, RunStopResult, TaskApplyParams, TaskApplyResult, TaskGetParams, TaskGetResult,
    code,
};
use collide_core::provider::{ApplyParams, ApplyResult, Task};
use collide_core::state::{PhaseState, RunState, WorkspaceState};
use collide_host::owner_client::ClientError;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

// ---- helpers ------------------------------------------------------------------------------------------------------

pub(super) const PROMPT: &str = "Rename the widget";

/// Runs git in `dir` with a cleared environment and returns what it printed.
pub(super) fn git_out(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args([
            "-c",
            "user.name=test",
            "-c",
            "user.email=test@example.invalid",
        ])
        .args(["-c", "commit.gpgsign=false"])
        .args(args)
        .current_dir(dir)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", "/nonexistent")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

pub(super) fn call<P: Serialize, R: DeserializeOwned>(
    fx: &mut Fixture,
    method: &str,
    params: &P,
) -> R {
    fx.client
        .call(method, params, Some(Duration::from_secs(60)))
        .unwrap_or_else(|error| panic!("{method}: {error}"))
}

/// The code and message of the error the daemon answers `method` with.
pub(super) fn rpc_error<P: Serialize>(fx: &mut Fixture, method: &str, params: &P) -> (i64, String) {
    let answer: Result<Value, ClientError> =
        fx.client
            .call(method, params, Some(Duration::from_secs(60)));
    match answer {
        Err(ClientError::Rpc(error)) => (error.code, error.message),
        other => panic!("{method} should be refused, and gave {other:?}"),
    }
}

pub(super) fn open(fx: &mut Fixture, folder: &Path) -> String {
    let opened: BoardOpenResult = call(
        fx,
        "board.open",
        &BoardOpenParams {
            root: folder.to_str().unwrap().to_string(),
            init_git: false,
        },
    );
    opened.board.expect("the folder opens as a board").id
}

/// A board rooted at `folder` itself, as a build before ts-0191-nv stored one for a linked worktree. `board.open` now
/// roots a worktree's board at the main checkout, so a test of a board rooted at a worktree stores its board this way.
pub(super) fn stored_board(fx: &Fixture, folder: &Path) -> String {
    collide_host::store::Store::open(&fx.paths().store_dir)
        .unwrap()
        .open_board(folder.to_str().unwrap(), &|_| false)
        .unwrap()
}

pub(super) fn apply(
    fx: &mut Fixture,
    board: &str,
    op: &str,
    id: Option<&str>,
    args: Value,
) -> ApplyResult {
    let params = TaskApplyParams {
        board: board.to_string(),
        source: Some("scratch".to_string()),
        apply: ApplyParams {
            actor: None,
            op: op.to_string(),
            id: id.map(str::to_string),
            args: serde_json::from_value::<BTreeMap<String, Value>>(args).unwrap(),
            version: None,
            checkout: None,
        },
        commit: false,
    };
    call::<_, TaskApplyResult>(fx, "task.apply", &params)
        .result
        .expect("a scratch write is applied, never queued")
}

/// A scratch task with `prompt`, and its key.
pub(super) fn create(fx: &mut Fixture, board: &str, prompt: &str) -> String {
    match apply(fx, board, "create", None, json!({"prompt": prompt})) {
        ApplyResult::Ok { task, .. } => task.key,
        other => panic!("create should work: {other:?}"),
    }
}

pub(super) fn task(fx: &mut Fixture, board: &str, key: &str) -> Task {
    call::<_, TaskGetResult>(
        fx,
        "task.get",
        &TaskGetParams {
            board: board.to_string(),
            id: key.to_string(),
            checkout: None,
        },
    )
    .task
    .expect("the task is there")
}

pub(super) fn start_params(board: &str, tasks: &[&str], workflow: Option<&str>) -> RunStartParams {
    RunStartParams {
        board: board.to_string(),
        tasks: tasks.iter().map(ToString::to_string).collect(),
        workflow: workflow.map(str::to_string),
        line: None,
        launch: BTreeMap::new(),
        profile: None,
        mode: Default::default(),
    }
}

pub(super) fn start(fx: &mut Fixture, params: &RunStartParams) -> RunStartResult {
    call(fx, "run.start", params)
}

pub(super) fn started(fx: &mut Fixture, board: &str, key: &str) -> String {
    match start(fx, &start_params(board, &[key], None)) {
        RunStartResult::Started { run } => run,
        RunStartResult::Refused { problems } => panic!("refused: {problems:?}"),
    }
}

pub(super) fn refusal(result: RunStartResult) -> Vec<String> {
    match result {
        RunStartResult::Refused { problems } => problems
            .into_iter()
            .map(|problem| problem.message)
            .collect(),
        other => panic!("the start should be refused, and gave {other:?}"),
    }
}

pub(super) fn run_get(fx: &mut Fixture, run: &str) -> RunGetResult {
    call(
        fx,
        "run.get",
        &RunGetParams {
            run: run.to_string(),
        },
    )
}

pub(super) fn wait_state(
    fx: &mut Fixture,
    run: &str,
    want: impl Fn(&RunState) -> bool,
) -> RunState {
    let until = std::time::Instant::now() + Duration::from_secs(30);
    loop {
        let state = run_get(fx, run).summary.state;
        if want(&state) {
            return state;
        }
        assert!(
            std::time::Instant::now() < until,
            "the run stayed {state:?}"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

pub(super) fn wait_ended(fx: &mut Fixture, run: &str) -> RunState {
    wait_state(fx, run, |state| matches!(state, RunState::Ended { .. }))
}

pub(super) fn wait_item(fx: &mut Fixture, run: &str, kind: QueueKind) -> QueueItem {
    let until = std::time::Instant::now() + Duration::from_secs(30);
    loop {
        if let Some(item) = fx.queue(run).into_iter().find(|item| item.kind == kind) {
            return item;
        }
        assert!(
            std::time::Instant::now() < until,
            "no {kind:?} item for {run}"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

pub(super) fn answer(fx: &mut Fixture, item: &QueueItem, answer: QueueAnswer) -> QueueAnswerResult {
    call(
        fx,
        "queue.answer",
        &collide_core::owner::QueueAnswerParams {
            item: item.id.clone(),
            answer,
        },
    )
}

pub(super) fn diff(fx: &mut Fixture, run: &str, path: Option<&str>) -> RunDiffResult {
    call(
        fx,
        "run.diff",
        &RunDiffParams {
            run: run.to_string(),
            path: path.map(str::to_string),
        },
    )
}

/// `report.jsonl` with `steps` after the turn it waits for, in `root`.
pub(super) fn report_with(root: &TempRoot, steps: &[Value]) -> PathBuf {
    report_named(root, "quick.jsonl", PROMPT_TURN, steps)
}

/// The turn of `PROMPT`, which `report.jsonl` waits for.
pub(super) const PROMPT_TURN: &str = r#"{"expect":{"type":"user","message":{"role":"user"}}}"#;

/// `report.jsonl` as `name` in `root`, with `turn` in place of the turn it waits for and `steps` after it.
pub(super) fn report_named(root: &TempRoot, name: &str, turn: &str, steps: &[Value]) -> PathBuf {
    let text = fs::read_to_string(recording("report")).unwrap();
    let mut lines = Vec::new();
    let mut inserted = false;
    for line in text.lines() {
        if !inserted && line.starts_with("{\"expect\"") {
            lines.push(turn.to_string());
            lines.extend(steps.iter().map(Value::to_string));
            inserted = true;
        } else {
            lines.push(line.to_string());
        }
    }
    let path = root.join(name);
    fs::write(&path, lines.join("\n")).unwrap();
    path
}

pub(super) fn write_file(path: &str, text: &str) -> Value {
    json!({"write_file": {"path": path, "text": text}})
}

/// A fixture whose agent does `steps`, a board on its repository, and one scratch task with `PROMPT`.
pub(super) fn quick(root: &TempRoot, steps: &[Value]) -> (Fixture, String, String) {
    quick_in(Fixture::new(&report_with(root, steps), Setup::default()))
}

/// `fx`, a board on its repository, and one scratch task with `PROMPT`.
pub(super) fn quick_in(mut fx: Fixture) -> (Fixture, String, String) {
    let repo = fx.repo.clone();
    // The identity of the merge commits that accept makes in the checkout: the daemon's git reads no global one.
    git_out(&repo, &["config", "user.name", "test"]);
    git_out(&repo, &["config", "user.email", "test@example.invalid"]);
    let board = open(&mut fx, &repo);
    let key = create(&mut fx, &board, PROMPT);
    (fx, board, key)
}

pub(super) fn file(path: &str, status: DiffStatus, additions: u32, deletions: u32) -> DiffFile {
    DiffFile {
        path: path.to_string(),
        old_path: None,
        status,
        additions: Some(additions),
        deletions: Some(deletions),
    }
}

pub(super) fn status_moves(events: &[EventBody]) -> Vec<String> {
    events
        .iter()
        .filter_map(|body| match body {
            EventBody::TaskChanged { task, .. } => {
                task.to.as_ref().map(|state| state.status.clone())
            }
            _ => None,
        })
        .collect()
}

pub(super) fn workspace_moves(events: &[EventBody]) -> Vec<WorkspaceState> {
    events
        .iter()
        .filter_map(|body| match body {
            EventBody::WorkspaceTransition { to, .. } => Some(*to),
            _ => None,
        })
        .collect()
}

// ---- the run ------------------------------------------------------------------------------------------------------

#[test]
fn a_prompt_runs_in_a_fresh_worktree_on_its_own_branch_and_ends_in_a_review_item() {
    let root = TempRoot::new();
    let (mut fx, board, key) = quick(&root, &[]);
    let start_commit = git_out(&fx.repo, &["rev-parse", "HEAD"]);

    let run = started(&mut fx, &board, &key);
    let item = wait_item(&mut fx, &run, QueueKind::Review);
    let ended = wait_ended(&mut fx, &run);
    assert!(matches!(&ended, RunState::Ended { outcome } if outcome.as_str() == "done"));

    // The worktree is on `collide/<run>`, made from the commit the checkout was at.
    let tree = fx.workspace_dir(&run);
    assert_eq!(
        git_out(&tree, &["rev-parse", "--abbrev-ref", "HEAD"]),
        format!("collide/{run}")
    );
    assert_eq!(git_out(&tree, &["rev-parse", "HEAD"]), start_commit);

    // The task is active while the phase runs and review after it, moved by category.
    assert_eq!(status_moves(&fx.events()), ["ready", "active", "review"]);
    let held = task(&mut fx, &board, &key);
    assert_eq!(held.state.category, Category::Review);
    assert_eq!(held.body.as_deref(), Some(PROMPT));

    // One item, in Collide's words, with the three answers.
    let items = fx.queue(&run);
    assert_eq!(items, std::slice::from_ref(&item));
    assert_eq!(item.task.as_deref(), Some(key.as_str()));
    assert_eq!(item.phase, None);
    assert_eq!(
        item.answers,
        [
            AnswerKind::Accept,
            AnswerKind::Reprompt,
            AnswerKind::Discard
        ]
    );
    assert!(!item.title.contains(PROMPT), "{}", item.title);

    // The run is in the index with its board, task, line and workflow, and its worktree is idle.
    let got = run_get(&mut fx, &run);
    assert_eq!(got.summary.workflow, "collide/quick");
    assert_eq!(got.summary.line, "main");
    assert_eq!(got.summary.tasks.len(), 1);
    assert_eq!(got.summary.tasks[0].key, key);
    assert_eq!(got.summary.tasks[0].status, "ready");
    assert_eq!(got.implementations["workspace"], "builtin");
    let listed: RunListResult = call(
        &mut fx,
        "run.list",
        &RunListParams {
            board: Some(board.clone()),
            scope: Default::default(),
            task: Some(key.clone()),
            limit: None,
            cursor: None,
        },
    );
    assert_eq!(listed.runs.len(), 1);
    let asked: TaskGetResult = call(
        &mut fx,
        "task.get",
        &TaskGetParams {
            board: board.clone(),
            id: key.clone(),
            checkout: None,
        },
    );
    assert_eq!(asked.runs, std::slice::from_ref(&run));
    wait_until("the workspace goes idle", Duration::from_secs(10), || {
        workspace_moves(&fx.events()).last() == Some(&WorkspaceState::Idle)
    });
}

#[test]
fn the_prompt_is_the_first_turn() {
    let root = TempRoot::new();
    let turn = json!({"expect": {"type": "user", "message": {"role": "user",
        "content": [{"type": "text", "text": PROMPT}]}}});
    let mut steps = vec![turn];
    steps.extend(
        fs::read_to_string(recording("report"))
            .unwrap()
            .lines()
            .filter(|line| line.starts_with("{\"say\""))
            .map(|line| serde_json::from_str::<Value>(line).unwrap()),
    );
    let path = written(&root, "turn.jsonl", &steps);
    let mut fx = Fixture::new(&path, Setup::default());
    let repo = fx.repo.clone();
    let board = open(&mut fx, &repo);
    let key = create(&mut fx, &board, PROMPT);
    let run = started(&mut fx, &board, &key);
    // Fake Claude ends with 97 on a turn that is not the one it expects, so a phase that ended saw exactly this one.
    assert_eq!(fx.wait_final(&run), PhaseState::Ended {});
}

#[test]
fn the_workflow_may_be_named() {
    let root = TempRoot::new();
    let (mut fx, board, key) = quick(&root, &[]);
    let result = start(
        &mut fx,
        &start_params(&board, &[&key], Some("collide/quick")),
    );
    assert!(
        matches!(result, RunStartResult::Started { .. }),
        "{result:?}"
    );
}

// ---- run.diff -----------------------------------------------------------------------------------------------------

pub(super) fn changes_and_commits() -> Vec<Value> {
    vec![
        write_file("src/a.rs", "fn a() {}\n"),
        write_file("README", "hello\nmore\n"),
        write_file("node_modules/pkg/index.js", "module.exports = 1;\n"),
        write_file("node_modules/pkg/package.json", "{}\n"),
        write_file("node_modules/pkg/yarn.lock", "# yarn\n"),
        write_file("packages/web/node_modules/dep/x.js", "x"),
        write_file("target/debug/out", "binary\n"),
        write_file("pnpm-lock.yaml", "lockfileVersion: 9\n"),
        json!({"git_commit": "Add the pieces"}),
    ]
}

#[test]
fn run_diff_lists_the_files_with_counts_collapses_folders_and_gives_one_patch() {
    let root = TempRoot::new();
    let (mut fx, board, key) = quick(&root, &changes_and_commits());
    let start_commit = git_out(&fx.repo, &["rev-parse", "HEAD"]);
    let run = started(&mut fx, &board, &key);
    wait_item(&mut fx, &run, QueueKind::Review);

    let listed = diff(&mut fx, &run, None);
    assert_eq!(listed.base, start_commit);
    assert_eq!(
        listed.head,
        git_out(&fx.repo, &["rev-parse", &format!("collide/{run}")])
    );
    assert_eq!(
        listed.files,
        [
            file("README", DiffStatus::Modified, 1, 0),
            file("node_modules/pkg/yarn.lock", DiffStatus::Added, 1, 0),
            file("pnpm-lock.yaml", DiffStatus::Added, 1, 0),
            file("src/a.rs", DiffStatus::Added, 1, 0),
        ],
        "lock files are shown even under a collapsed folder"
    );
    assert_eq!(
        listed.collapsed,
        [
            CollapsedFolder {
                path: "node_modules".to_string(),
                files: 2,
                bytes: 20 + 3,
            },
            CollapsedFolder {
                path: "packages/web/node_modules".to_string(),
                files: 1,
                bytes: 1,
            },
            CollapsedFolder {
                path: "target".to_string(),
                files: 1,
                bytes: 7,
            },
        ]
    );
    assert_eq!(listed.more, 0);
    assert_eq!(listed.patch, None);

    let one = diff(&mut fx, &run, Some("src/a.rs"))
        .patch
        .expect("a patch");
    assert_eq!(one.path, "src/a.rs");
    assert!(one.text.contains("+++ b/src/a.rs"), "{}", one.text);
    assert!(one.text.contains("+fn a() {}"), "{}", one.text);
    assert!(!one.truncated);
    assert_eq!(one.bytes, one.text.len() as u64);
    let readme = diff(&mut fx, &run, Some("README")).patch.expect("a patch");
    assert!(readme.text.contains("+more"), "{}", readme.text);
    assert!(!readme.text.contains("a.rs"), "{}", readme.text);
}

#[test]
fn a_file_with_no_change_has_no_patch() {
    let root = TempRoot::new();
    let (mut fx, board, key) = quick(&root, &[write_file("a.txt", "a\n")]);
    let run = started(&mut fx, &board, &key);
    wait_item(&mut fx, &run, QueueKind::Review);
    let (error_code, message) = rpc_error(
        &mut fx,
        "run.diff",
        &RunDiffParams {
            run: run.clone(),
            path: Some("README".to_string()),
        },
    );
    assert_eq!(error_code, code::NOT_FOUND);
    assert!(message.contains("README"), "{message}");
}

#[test]
fn a_run_the_daemon_never_heard_of_has_no_diff() {
    let root = TempRoot::new();
    let (mut fx, _, _) = quick(&root, &[]);
    let (error_code, _) = rpc_error(
        &mut fx,
        "run.diff",
        &RunDiffParams {
            run: "run-0-0".to_string(),
            path: None,
        },
    );
    assert_eq!(error_code, code::NOT_FOUND);
}

#[test]
fn a_file_task_run_has_no_review_diff() {
    let root = TempRoot::new();
    let (mut fx, _, _) = quick(&root, &[]);
    let run = fx.started();
    fx.wait_final(&run);
    let (error_code, message) = rpc_error(&mut fx, "run.diff", &RunDiffParams { run, path: None });
    assert_eq!(error_code, code::NOT_FOUND);
    assert!(message.contains("prompt run"), "{message}");
}

#[test]
fn the_diff_is_against_the_commit_recorded_at_start_when_the_checkout_moves_on_during_the_run() {
    let root = TempRoot::new();
    let mark = root.join("mark");
    let (mut fx, board, key) = quick(
        &root,
        &[
            write_file("a.txt", "a\n"),
            json!({"touch": mark.to_str().unwrap()}),
            json!({"sleep": 2}),
        ],
    );
    let start_commit = git_out(&fx.repo, &["rev-parse", "HEAD"]);
    let run = started(&mut fx, &board, &key);
    wait_until("the agent has written", Duration::from_secs(30), || {
        mark.exists()
    });
    // The checkout moves on: a commit on `main` while the run is going.
    fs::write(fx.repo.join("later.txt"), "later\n").unwrap();
    git_out(&fx.repo, &["add", "."]);
    git_out(&fx.repo, &["commit", "-q", "-m", "later"]);
    assert_ne!(git_out(&fx.repo, &["rev-parse", "HEAD"]), start_commit);
    wait_item(&mut fx, &run, QueueKind::Review);

    let listed = diff(&mut fx, &run, None);
    assert_eq!(listed.base, start_commit);
    let paths: Vec<&str> = listed.files.iter().map(|file| file.path.as_str()).collect();
    assert_eq!(paths, ["a.txt"]);
}

// ---- what the agent left uncommitted ------------------------------------------------------------------------------

#[test]
fn what_the_agent_left_uncommitted_is_committed_by_collide_in_a_commit_that_says_so() {
    let root = TempRoot::new();
    let (mut fx, board, key) = quick(
        &root,
        &[
            write_file("a.txt", "a\n"),
            json!({"git_commit": "Add a"}),
            write_file("b.txt", "b\n"),
            write_file("README", "hello\nmore\n"),
        ],
    );
    let start_commit = git_out(&fx.repo, &["rev-parse", "HEAD"]);
    let run = started(&mut fx, &board, &key);
    wait_item(&mut fx, &run, QueueKind::Review);
    let tree = fx.workspace_dir(&run);

    assert_eq!(git_out(&tree, &["status", "--porcelain"]), "");
    let log = git_out(
        &tree,
        &["log", "--format=%an|%s", &format!("{start_commit}..HEAD")],
    );
    assert_eq!(
        log.lines().collect::<Vec<_>>(),
        [
            "Collide|Commit what the agent left uncommitted",
            "Fake Claude|Add a"
        ]
    );
    let listed = diff(&mut fx, &run, None);
    let paths: Vec<&str> = listed.files.iter().map(|file| file.path.as_str()).collect();
    assert_eq!(paths, ["README", "a.txt", "b.txt"]);
}

#[test]
fn a_phase_that_left_nothing_uncommitted_gets_no_commit_of_collides() {
    let root = TempRoot::new();
    let (mut fx, board, key) = quick(
        &root,
        &[write_file("a.txt", "a\n"), json!({"git_commit": "Add a"})],
    );
    let start_commit = git_out(&fx.repo, &["rev-parse", "HEAD"]);
    let run = started(&mut fx, &board, &key);
    wait_item(&mut fx, &run, QueueKind::Review);
    let count = git_out(
        &fx.workspace_dir(&run),
        &["rev-list", "--count", &format!("{start_commit}..HEAD")],
    );
    assert_eq!(count, "1");
}

// ---- permission requests ------------------------------------------------------------------------------------------

#[test]
fn a_permission_request_in_a_quick_run_is_a_queue_item_and_allow_lets_the_phase_go_on() {
    let mut fx = Fixture::new(&recording("permission-request"), Setup::default());
    let repo = fx.repo.clone();
    let board = open(&mut fx, &repo);
    let key = create(&mut fx, &board, PROMPT);
    let run = started(&mut fx, &board, &key);

    let item = wait_item(&mut fx, &run, QueueKind::Permission);
    assert_eq!(
        fx.phases.state(&Fixture::key(&run)),
        Some(PhaseState::Running {}),
        "the phase waits, it is not paused"
    );
    let result = answer(
        &mut fx,
        &item,
        QueueAnswer::Permission {
            reply: PermissionReply::Allow {},
        },
    );
    assert_eq!(result, QueueAnswerResult::Done {});
    let review = wait_item(&mut fx, &run, QueueKind::Review);
    assert_eq!(review.answers[0], AnswerKind::Accept);
    assert_eq!(fx.queue(&run), [review], "the request left the Queue");
}

#[test]
fn a_deny_in_a_quick_run_is_told_to_the_agent_and_the_phase_goes_on() {
    let root = TempRoot::new();
    let denied = edited(&root, "permission-request", |line| {
        if line.starts_with("{\"expect\": {\"type\": \"control_response\"")
            || line.starts_with("{\"expect\":{\"type\":\"control_response\"")
        {
            Some(
                json!({"expect": {"type": "control_response", "response": {"subtype": "success",
                    "request_id": "req-01", "response": {"behavior": "deny", "message": "Not now."}}}})
                .to_string(),
            )
        } else {
            Some(line.to_string())
        }
    });
    let mut fx = Fixture::new(&denied, Setup::default());
    let repo = fx.repo.clone();
    let board = open(&mut fx, &repo);
    let key = create(&mut fx, &board, PROMPT);
    let run = started(&mut fx, &board, &key);
    let item = wait_item(&mut fx, &run, QueueKind::Permission);
    let result = answer(
        &mut fx,
        &item,
        QueueAnswer::Permission {
            reply: PermissionReply::Deny {
                message: Some("Not now.".to_string()),
            },
        },
    );
    assert_eq!(result, QueueAnswerResult::Done {});
    // Fake Claude ends with 97 on an answer that does not match, so a phase that ended saw the deny and its message.
    wait_item(&mut fx, &run, QueueKind::Review);
    assert_eq!(fx.wait_final(&run), PhaseState::Ended {});
}

// ---- a run that does not end well ---------------------------------------------------------------------------------

#[test]
fn a_phase_that_fails_gets_a_review_item_with_only_reprompt_and_discard() {
    let root = TempRoot::new();
    let mut fx = Fixture::new(&no_result_recording(&root), Setup::default());
    let repo = fx.repo.clone();
    let board = open(&mut fx, &repo);
    let key = create(&mut fx, &board, PROMPT);
    let run = started(&mut fx, &board, &key);

    let item = wait_item(&mut fx, &run, QueueKind::Review);
    assert_eq!(item.answers, [AnswerKind::Reprompt, AnswerKind::Discard]);
    assert_eq!(item.task.as_deref(), Some(key.as_str()));
    wait_state(&mut fx, &run, |state| matches!(state, RunState::Failed {}));
    assert_eq!(task(&mut fx, &board, &key).state.category, Category::Review);
    assert_eq!(status_moves(&fx.events()), ["ready", "active", "review"]);
}

#[test]
fn a_run_that_is_stopped_gets_a_review_item_with_only_reprompt_and_discard() {
    let root = TempRoot::new();
    let hang = written(
        &root,
        "hang.jsonl",
        &[json!({"expect": {"type": "user"}}), json!({"sleep": 60})],
    );
    let mut fx = Fixture::new(&hang, Setup::default());
    let repo = fx.repo.clone();
    let board = open(&mut fx, &repo);
    let key = create(&mut fx, &board, PROMPT);
    let run = started(&mut fx, &board, &key);
    let stopped: RunStopResult = call(
        &mut fx,
        "run.stop",
        &RunStopParams {
            run: run.clone(),
            grace_secs: Some(1),
        },
    );
    assert!(matches!(
        stopped.state,
        RunState::Stopped {} | RunState::Running {}
    ));

    let item = wait_item(&mut fx, &run, QueueKind::Review);
    assert_eq!(item.answers, [AnswerKind::Reprompt, AnswerKind::Discard]);
    wait_state(&mut fx, &run, |state| matches!(state, RunState::Stopped {}));
    assert_eq!(task(&mut fx, &board, &key).state.category, Category::Review);
}

// ---- what is refused ----------------------------------------------------------------------------------------------

fn nothing_started(fx: &mut Fixture, board: &str, key: &str) {
    let listed: RunListResult = call(
        fx,
        "run.list",
        &RunListParams {
            board: Some(board.to_string()),
            scope: Default::default(),
            task: None,
            limit: None,
            cursor: None,
        },
    );
    assert!(listed.runs.is_empty(), "{:?}", listed.runs);
    assert_eq!(task(fx, board, key).state.category, Category::Ready);
    assert!(fx.queue_all().is_empty());
}

#[test]
fn a_checkout_with_a_detached_head_is_refused_in_collides_words() {
    let root = TempRoot::new();
    let (mut fx, board, key) = quick(&root, &[]);
    git_out(&fx.repo, &["checkout", "-q", "--detach"]);
    let problems = refusal(start(&mut fx, &start_params(&board, &[&key], None)));
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(problems[0].contains("not on a branch"), "{}", problems[0]);
    assert!(problems[0].contains("detached"), "{}", problems[0]);
    nothing_started(&mut fx, &board, &key);
}

#[test]
fn another_workflow_is_refused_as_not_built_yet() {
    let root = TempRoot::new();
    let (mut fx, board, key) = quick(&root, &[]);
    let problems = refusal(start(
        &mut fx,
        &start_params(&board, &[&key], Some("collide/review")),
    ));
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(problems[0].contains("collide/review"), "{}", problems[0]);
    assert!(problems[0].contains("not built yet"), "{}", problems[0]);
    assert!(problems[0].contains("Stage 8"), "{}", problems[0]);
    nothing_started(&mut fx, &board, &key);
}

#[test]
fn a_task_that_is_not_there_or_not_ready_or_not_scratch_is_refused_with_its_key() {
    let root = TempRoot::new();
    let (mut fx, board, key) = quick(&root, &[]);
    let missing = refusal(start(&mut fx, &start_params(&board, &["scratch:99"], None)));
    assert!(missing[0].contains("scratch:99"), "{}", missing[0]);
    let other = refusal(start(&mut fx, &start_params(&board, &["plan:ts-1"], None)));
    assert!(other[0].contains("plan:ts-1"), "{}", other[0]);
    let two = refusal(start(
        &mut fx,
        &start_params(&board, &[&key, "scratch:2"], None),
    ));
    assert!(two[0].contains("one task"), "{}", two[0]);
    apply(
        &mut fx,
        &board,
        "setStatus",
        Some(&key),
        json!({"status": "done"}),
    );
    let done = refusal(start(&mut fx, &start_params(&board, &[&key], None)));
    assert!(done[0].contains(&key), "{}", done[0]);
    assert!(done[0].contains("ready"), "{}", done[0]);
}

#[test]
fn a_task_with_a_run_going_cannot_be_started_again() {
    let root = TempRoot::new();
    let hang = written(
        &root,
        "hang.jsonl",
        &[json!({"expect": {"type": "user"}}), json!({"sleep": 60})],
    );
    let mut fx = Fixture::new(&hang, Setup::default());
    let repo = fx.repo.clone();
    let board = open(&mut fx, &repo);
    let key = create(&mut fx, &board, PROMPT);
    let run = started(&mut fx, &board, &key);
    let again = refusal(start(&mut fx, &start_params(&board, &[&key], None)));
    assert!(again[0].contains("active"), "{}", again[0]);
    let _: RunStopResult = call(
        &mut fx,
        "run.stop",
        &RunStopParams {
            run,
            grace_secs: Some(1),
        },
    );
}

#[test]
fn a_run_that_will_not_start_leaves_its_task_ready() {
    let mut fx = Fixture::new(&recording("report"), Setup::default());
    let repo = fx.repo.clone();
    let board = open(&mut fx, &repo);
    let key = create(&mut fx, &board, PROMPT);
    // The claude binary is gone from the path of the base environment, so the phase cannot start.
    fs::remove_file(fx.root.join("bin/claude")).unwrap();
    let problems = refusal(start(&mut fx, &start_params(&board, &[&key], None)));
    assert!(problems[0].contains("claude"), "{}", problems[0]);
    assert_eq!(task(&mut fx, &board, &key).state.category, Category::Ready);
    assert_eq!(status_moves(&fx.events()), ["ready", "active", "ready"]);
}

// ---- a board that is a linked worktree ----------------------------------------------------------------------------

#[test]
fn a_board_whose_root_is_a_linked_worktree_runs_from_the_branch_it_is_on() {
    let root = TempRoot::new();
    let (mut fx, _, _) = quick(&root, &[write_file("a.txt", "a\n")]);
    let linked = fx.root.join("linked");
    git_out(
        &fx.repo,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "feature",
            linked.to_str().unwrap(),
        ],
    );
    fs::write(linked.join("feature.txt"), "f\n").unwrap();
    git_out(&linked, &["add", "."]);
    git_out(&linked, &["commit", "-q", "-m", "feature work"]);
    let feature_head = git_out(&linked, &["rev-parse", "HEAD"]);
    let linked = fs::canonicalize(linked).unwrap();

    let board = stored_board(&fx, &linked);
    let key = create(&mut fx, &board, PROMPT);
    let run = started(&mut fx, &board, &key);
    wait_item(&mut fx, &run, QueueKind::Review);

    assert_eq!(run_get(&mut fx, &run).summary.line, "feature");
    let listed = diff(&mut fx, &run, None);
    assert_eq!(listed.base, feature_head);
    let paths: Vec<&str> = listed.files.iter().map(|file| file.path.as_str()).collect();
    assert_eq!(paths, ["a.txt"]);
    let tree = fx.workspace_dir(&run);
    assert!(
        tree.join("feature.txt").is_file(),
        "it started from `feature`"
    );
    assert_eq!(
        git_out(&tree, &["rev-parse", "--abbrev-ref", "HEAD"]),
        format!("collide/{run}")
    );
}
