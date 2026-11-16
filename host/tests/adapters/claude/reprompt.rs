//! Re-prompt on a quick run's `Review` item (ts-0080-tq; Stage 3c): `queue.answer` with `reprompt` and the person's
//! words starts a new run of the same scratch task. The new run takes over the worktree and the branch, resumes the
//! earlier phase's session by id with the words as its turn, and ends in a `Review` item of its own. Each test has a
//! daemon of its own under a temp root and fake Claude as `claude`, which plays one recording for a first launch and
//! another for a launch with `--resume` (`quick_run.rs` and `accept.rs` have the helpers): git reads no configuration,
//! every command the test runs clears its environment, and nothing here reads the owner's files.

use super::accept::{
    Reviewed, adds, commit_file, review_back_after_an_unknown_outcome, reviewed_in, step_events,
};
use super::common::TempRoot;
use super::discard::stopped_for_disk;
use super::file_run::{Fixture, Setup, no_result_recording};
use super::pause::written;
use super::quick_run::{
    answer, call, diff, file, git_out, quick_in, report_named, report_with, rpc_error, run_get,
    started, status_moves, wait_ended, wait_item, wait_state, workspace_moves, write_file,
};
use super::value_of;
use collide_core::events::{EventBody, StepResult};
use collide_core::item::Category;
use collide_core::owner::{
    AnswerKind, DiffStatus, QueueAnswer, QueueAnswerParams, QueueAnswerResult, QueueItem,
    QueueKind, RunListParams, RunListResult, RunStopParams, RunStopResult, code,
};
use collide_core::state::{RunState, WorkspaceState};
use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};

// ---- helpers ------------------------------------------------------------------------------------------------------

const WORDS: &str = "Also add b.txt";

/// The turn a resumed session is told: the person's words.
fn words_turn(words: &str) -> String {
    json!({"expect": {"type": "user", "message": {"role": "user",
        "content": [{"type": "text", "text": words}]}}})
    .to_string()
}

/// A recording for a resumed session: it waits for `WORDS`, does `steps` and reports.
fn resumed_report(root: &TempRoot, steps: &[Value]) -> PathBuf {
    report_named(root, "resumed.jsonl", &words_turn(WORDS), steps)
}

/// A fixture whose `claude` plays `first` and, started with `--resume`, `resumed`; with its board and scratch task.
fn playing(first: &Path, resumed: Option<PathBuf>) -> (Fixture, String, String) {
    let setup = Setup {
        resumed,
        ..Setup::default()
    };
    quick_in(Fixture::new(first, setup))
}

/// A run whose phase failed, with its `Review` item. `resumed` is what a resumed `claude` plays.
pub(super) fn failed_run(root: &TempRoot, resumed: Option<PathBuf>) -> Reviewed {
    let (mut fx, board, key) = playing(&no_result_recording(root), resumed);
    let run = started(&mut fx, &board, &key);
    let item = wait_item(&mut fx, &run, QueueKind::Review);
    wait_state(&mut fx, &run, |state| matches!(state, RunState::Failed {}));
    Reviewed {
        fx,
        board,
        key,
        run,
        item,
    }
}

/// A run that was stopped while its agent waited, with its `Review` item.
pub(super) fn stopped_run(root: &TempRoot, resumed: Option<PathBuf>) -> Reviewed {
    let hang = written(
        root,
        "hang.jsonl",
        &[json!({"expect": {"type": "user"}}), json!({"sleep": 60})],
    );
    let (mut fx, board, key) = playing(&hang, resumed);
    let run = started(&mut fx, &board, &key);
    let _: RunStopResult = call(
        &mut fx,
        "run.stop",
        &RunStopParams {
            run: run.clone(),
            grace_secs: Some(1),
        },
    );
    let item = wait_item(&mut fx, &run, QueueKind::Review);
    wait_state(&mut fx, &run, |state| matches!(state, RunState::Stopped {}));
    Reviewed {
        fx,
        board,
        key,
        run,
        item,
    }
}

/// A run whose agent did `first`, ended; its `claude` does `second` when it is resumed.
fn reviewed_twice(root: &TempRoot, first: &[Value], second: &[Value]) -> Reviewed {
    reviewed_in(playing(
        &report_with(root, first),
        Some(resumed_report(root, second)),
    ))
}

/// What a re-prompt started: the new run and its `Review` item.
struct Followed {
    run: String,
    item: QueueItem,
}

/// The launch lines of `claude` that began a phase, in order.
fn launches(fx: &Fixture) -> Vec<Vec<String>> {
    fx.claude
        .calls()
        .iter()
        .filter(|call| call.contains("--session-id") || call.contains("--resume"))
        .map(|call| call.split_whitespace().map(str::to_string).collect())
        .collect()
}

/// The ids of the runs of the task, newest first.
fn runs_of(fx: &mut Fixture, board: &str, key: &str) -> Vec<String> {
    let listed: RunListResult = call(
        fx,
        "run.list",
        &RunListParams {
            board: Some(board.to_string()),
            scope: Default::default(),
            task: Some(key.to_string()),
            limit: None,
            cursor: None,
        },
    );
    listed.runs.into_iter().map(|run| run.id).collect()
}

fn subjects(dir: &Path) -> Vec<String> {
    git_out(dir, &["log", "--format=%s", "main..HEAD"])
        .lines()
        .map(str::to_string)
        .collect()
}

impl Reviewed {
    /// The answer of a re-prompt with `words`, without waiting for what it starts.
    fn reprompt_answer(&mut self, words: &str) -> QueueAnswerResult {
        answer(
            &mut self.fx,
            &self.item,
            QueueAnswer::Reprompt {
                prompt: words.to_string(),
            },
        )
    }

    /// A re-prompt with `words` that is taken: the new run, once its phase has ended and its item is in the Queue.
    fn reprompted(&mut self, words: &str) -> Followed {
        let result = self.reprompt_answer(words);
        assert_eq!(result, QueueAnswerResult::Done {});
        self.newest()
    }

    /// The newest run of the task, once its `Review` item is in the Queue.
    fn newest(&mut self) -> Followed {
        let run = runs_of(&mut self.fx, &self.board, &self.key).remove(0);
        assert_ne!(run, self.run, "the new run is not the one it follows");
        let item = wait_item(&mut self.fx, &run, QueueKind::Review);
        Followed { run, item }
    }
}

// ---- the new run --------------------------------------------------------------------------------------------------

#[test]
fn a_reprompt_resumes_the_session_and_sends_the_words_as_the_turn_in_a_new_run() {
    let root = TempRoot::new();
    let mut first = reviewed_twice(&root, &adds("a.txt"), &adds("b.txt"));

    // Fake Claude ends with 97 on a turn that is not the one it expects, so a run that ended done saw WORDS.
    let second = first.reprompted(WORDS);
    let ended = wait_ended(&mut first.fx, &second.run);
    assert!(matches!(&ended, RunState::Ended { outcome } if outcome.as_str() == "done"));

    let lines = launches(&first.fx);
    assert_eq!(lines.len(), 2, "{lines:?}");
    let session = value_of(&lines[0], "--session-id").expect("the first phase assigns a session");
    assert_eq!(value_of(&lines[1], "--resume"), Some(session));
    assert!(!lines[1].iter().any(|word| word == "--session-id"));

    // The branch has the commits of both phases, in the worktree the first run made.
    let tree = first.fx.workspace_dir(&first.run);
    assert_eq!(subjects(&tree), ["Add b.txt", "Add a.txt"]);
    assert!(!first.fx.workspace_dir(&second.run).exists());
    let got = run_get(&mut first.fx, &second.run);
    assert_eq!(
        got.summary.workspace.as_deref().map(Path::new),
        Some(tree.as_path())
    );
    assert_eq!(got.summary.workflow, "collide/quick");

    // The old item is gone and the new one has the three answers.
    assert!(first.fx.queue(&first.run).is_empty());
    assert_eq!(
        first.fx.queue(&second.run),
        std::slice::from_ref(&second.item)
    );
    assert_eq!(
        second.item.answers,
        [
            AnswerKind::Accept,
            AnswerKind::Reprompt,
            AnswerKind::Discard
        ]
    );
    assert_eq!(second.item.task.as_deref(), Some(first.key.as_str()));

    // The task went back to active and is in review again; the old run is as it was.
    assert_eq!(
        status_moves(&first.fx.events()),
        ["ready", "active", "review", "active", "review"]
    );
    assert_eq!(first.category(), Category::Review);
    let old = run_get(&mut first.fx, &first.run).summary.state;
    assert!(matches!(old, RunState::Ended { outcome } if outcome.as_str() == "done"));
    assert_eq!(
        runs_of(&mut first.fx, &first.board, &first.key),
        [second.run.clone(), first.run.clone()]
    );
}

#[test]
fn the_worktree_is_one_workspace_whose_events_carry_the_run_that_made_it() {
    let root = TempRoot::new();
    let mut first = reviewed_twice(&root, &adds("a.txt"), &adds("b.txt"));
    let second = first.reprompted(WORDS);
    wait_ended(&mut first.fx, &second.run);

    let events = first.fx.daemon.as_ref().unwrap().events_since(0).unwrap();
    let of_workspace: Vec<_> = events
        .iter()
        .filter_map(|event| match &event.body {
            EventBody::WorkspaceTransition { workspace, to, .. } => {
                Some((workspace.clone(), *to, event.actor.run.clone()))
            }
            _ => None,
        })
        .collect();
    assert!(
        of_workspace.iter().all(|(name, _, _)| *name == first.run),
        "one workspace, named for the run that made it: {of_workspace:?}"
    );
    let by_the_second: Vec<WorkspaceState> = of_workspace
        .iter()
        .filter(|(_, _, run)| run.as_deref() == Some(second.run.as_str()))
        .map(|(_, to, _)| *to)
        .collect();
    assert_eq!(by_the_second, [WorkspaceState::InUse, WorkspaceState::Idle]);
}

#[test]
fn run_diff_of_the_new_run_shows_both_phases_against_the_base_recorded_at_the_first_start() {
    let root = TempRoot::new();
    let mut first = reviewed_twice(&root, &adds("a.txt"), &adds("b.txt"));
    let base = git_out(&first.fx.repo, &["rev-parse", "HEAD"]);
    // The checkout moves on between the two phases; the diff stays against the commit recorded at the first start.
    commit_file(&first.fx.repo, "later.txt", "later\n", "later");

    let second = first.reprompted(WORDS);

    let listed = diff(&mut first.fx, &second.run, None);
    assert_eq!(listed.base, base);
    assert_eq!(
        listed.head,
        git_out(&first.fx.repo, &["rev-parse", &first.branch()])
    );
    assert_eq!(
        listed.files,
        [
            file("a.txt", DiffStatus::Added, 1, 0),
            file("b.txt", DiffStatus::Added, 1, 0)
        ]
    );
}

#[test]
fn a_reprompt_of_a_failed_run_works_the_same_way() {
    let root = TempRoot::new();
    let mut first = failed_run(&root, Some(resumed_report(&root, &adds("b.txt"))));
    assert_eq!(
        first.item.answers,
        [AnswerKind::Reprompt, AnswerKind::Discard]
    );

    let second = first.reprompted(WORDS);

    wait_ended(&mut first.fx, &second.run);
    assert_eq!(subjects(&first.fx.workspace_dir(&first.run)), ["Add b.txt"]);
    assert_eq!(
        second.item.answers,
        [
            AnswerKind::Accept,
            AnswerKind::Reprompt,
            AnswerKind::Discard
        ]
    );
    assert_eq!(first.category(), Category::Review);
}

#[test]
fn a_reprompt_of_a_stopped_run_works_the_same_way() {
    let root = TempRoot::new();
    let mut first = stopped_run(&root, Some(resumed_report(&root, &adds("b.txt"))));

    let second = first.reprompted(WORDS);

    wait_ended(&mut first.fx, &second.run);
    assert_eq!(subjects(&first.fx.workspace_dir(&first.run)), ["Add b.txt"]);
}

#[test]
fn a_reprompt_of_a_reprompted_run_resumes_the_same_session_in_the_same_worktree() {
    let root = TempRoot::new();
    let mut first = reviewed_twice(&root, &adds("a.txt"), &adds("b.txt"));
    let mut second = first.reprompted(WORDS);
    let state = wait_ended(&mut first.fx, &second.run);
    assert!(matches!(state, RunState::Ended { .. }));

    // The second re-prompt is answered from the new run's own item. Its recording is the same one, so its turn is
    // WORDS again and it commits b.txt again: nothing is added, and the run still ends.
    let result = answer(
        &mut first.fx,
        &second.item,
        QueueAnswer::Reprompt {
            prompt: WORDS.to_string(),
        },
    );
    assert_eq!(result, QueueAnswerResult::Done {});
    let third = runs_of(&mut first.fx, &first.board, &first.key).remove(0);
    assert_ne!(third, second.run);
    second.item = wait_item(&mut first.fx, &third, QueueKind::Review);

    let lines = launches(&first.fx);
    assert_eq!(lines.len(), 3, "{lines:?}");
    let session = value_of(&lines[0], "--session-id").unwrap();
    assert_eq!(value_of(&lines[1], "--resume"), Some(session));
    assert_eq!(value_of(&lines[2], "--resume"), Some(session));
    assert!(!first.fx.workspace_dir(&third).exists());
    assert!(first.fx.workspace_dir(&first.run).is_dir());
}

// ---- a session that cannot be resumed -----------------------------------------------------------------------------

#[test]
fn a_session_that_cannot_be_resumed_fails_the_new_phase_and_the_review_item_comes_back() {
    let root = TempRoot::new();
    let gone = written(
        &root,
        "gone.jsonl",
        &[
            json!({"expect": {"type": "user"}}),
            json!({"stderr": "No conversation found with session ID"}),
            json!({"exit": 1}),
        ],
    );
    let mut first = reviewed_in(playing(&report_with(&root, &adds("a.txt")), Some(gone)));

    let second = first.reprompted(WORDS);

    // The new phase failed with its cause, and the run says so.
    wait_state(&mut first.fx, &second.run, |state| {
        matches!(state, RunState::Failed {})
    });
    let record = first
        .fx
        .record(&second.run)
        .expect("the new phase has a record");
    assert_eq!(record.failure.as_deref(), Some("no_result"));
    assert!(
        second.item.title.contains("no_result"),
        "the item says why: {}",
        second.item.title
    );
    // The item has re-prompt and discard, and the worktree is not left behind: discard removes it.
    assert_eq!(
        second.item.answers,
        [AnswerKind::Reprompt, AnswerKind::Discard]
    );
    assert_eq!(first.category(), Category::Review);
    assert!(first.fx.workspace_dir(&first.run).is_dir());
    let result = answer(&mut first.fx, &second.item, QueueAnswer::Discard {});
    assert_eq!(result, QueueAnswerResult::Done {});
    first.assert_removed();
}

#[test]
fn a_reprompt_that_cannot_start_puts_the_item_back_and_the_task_stays_in_review() {
    let root = TempRoot::new();
    let mut first = reviewed_twice(&root, &adds("a.txt"), &adds("b.txt"));
    let away = first.fx.claude.path.with_extension("away");
    fs::rename(&first.fx.claude.path, &away).unwrap();

    let message = match first.reprompt_answer(WORDS) {
        QueueAnswerResult::Refused { message } => message,
        other => panic!("the re-prompt should be refused, and gave {other:?}"),
    };

    assert!(message.contains("claude"), "{message}");
    assert_eq!(
        first.fx.queue(&first.run),
        std::slice::from_ref(&first.item)
    );
    assert_eq!(first.category(), Category::Review);
    assert!(first.fx.workspace_dir(&first.run).is_dir());
    let runs = runs_of(&mut first.fx, &first.board, &first.key);
    assert_eq!(runs.len(), 2, "the refused start is a failed run: {runs:?}");
    assert!(matches!(
        run_get(&mut first.fx, &runs[0]).summary.state,
        RunState::Failed {}
    ));
    // Once `claude` is back the same item works.
    fs::rename(&away, &first.fx.claude.path).unwrap();
    let second = first.reprompted(WORDS);
    wait_ended(&mut first.fx, &second.run);
    assert_eq!(
        subjects(&first.fx.workspace_dir(&first.run)),
        ["Add b.txt", "Add a.txt"]
    );
}

// ---- what is refused ----------------------------------------------------------------------------------------------

#[test]
fn a_second_answer_to_the_same_item_is_refused_and_changes_nothing() {
    let root = TempRoot::new();
    let mut first = reviewed_twice(&root, &adds("a.txt"), &adds("b.txt"));
    let second = first.reprompted(WORDS);
    wait_ended(&mut first.fx, &second.run);

    for reply in [
        QueueAnswer::Reprompt {
            prompt: WORDS.to_string(),
        },
        QueueAnswer::Discard {},
        QueueAnswer::Accept {},
    ] {
        let params = QueueAnswerParams {
            item: first.item.id.clone(),
            answer: reply,
        };
        let (refused, _) = rpc_error(&mut first.fx, "queue.answer", &params);
        assert_eq!(refused, code::NOT_FOUND);
    }

    assert_eq!(runs_of(&mut first.fx, &first.board, &first.key).len(), 2);
    assert_eq!(launches(&first.fx).len(), 2);
    assert!(first.fx.workspace_dir(&first.run).is_dir());
}

#[test]
fn a_run_whose_agent_never_started_has_no_session_to_resume_and_the_reprompt_is_refused() {
    let root = TempRoot::new();
    // Stopped for disk after the worktree was made and before the phase started.
    let mut run = stopped_for_disk(&root, &[u64::MAX, u64::MAX, u64::MAX, 0]);

    let result = run.reprompt_answer(WORDS);

    assert!(
        matches!(&result, QueueAnswerResult::Refused { message } if message.contains("never started")),
        "{result:?}"
    );
    assert_eq!(run.fx.queue(&run.run), std::slice::from_ref(&run.item));
    assert_eq!(runs_of(&mut run.fx, &run.board, &run.key).len(), 1);
    assert!(run.fx.workspace_dir(&run.run).is_dir());
}

#[test]
fn words_that_say_nothing_are_refused_and_the_item_stays() {
    let root = TempRoot::new();
    let mut first = reviewed_twice(&root, &adds("a.txt"), &adds("b.txt"));

    let result = first.reprompt_answer("  \n ");

    assert!(
        matches!(&result, QueueAnswerResult::Refused { message } if message.contains("words")),
        "{result:?}"
    );
    assert_eq!(
        first.fx.queue(&first.run),
        std::slice::from_ref(&first.item)
    );
    assert_eq!(runs_of(&mut first.fx, &first.board, &first.key).len(), 1);
    assert_eq!(launches(&first.fx).len(), 1);
    assert_eq!(first.category(), Category::Review);
}

// ---- the conflict refusal -----------------------------------------------------------------------------------------

#[test]
fn a_conflict_refusal_names_the_reprompt_and_the_reprompt_resolves_it() {
    let root = TempRoot::new();
    let edit = [
        write_file("README", "from the agent\n"),
        json!({"git_commit": "Edit README"}),
    ];
    // The agent resolves the conflict the way a person tells it to: it brings `main` in and writes the file it wants.
    let resolve = [
        json!({"git": ["merge", "--no-commit", "main"]}),
        write_file("README", "resolved\n"),
        json!({"git_commit": "Resolve README"}),
    ];
    let mut first = reviewed_twice(&root, &edit, &resolve);
    commit_file(
        &first.fx.repo,
        "README",
        "from main\n",
        "Edit README on main",
    );

    let message = first.refused();
    assert!(message.contains("conflict"), "{message}");
    assert!(message.contains("re-prompt"), "{message}");

    let second = first.reprompted(WORDS);
    wait_ended(&mut first.fx, &second.run);
    let merged = answer(&mut first.fx, &second.item, QueueAnswer::Accept {});

    assert!(
        matches!(merged, QueueAnswerResult::Merged { .. }),
        "{merged:?}"
    );
    assert_eq!(
        fs::read_to_string(first.fx.repo.join("README")).unwrap(),
        "resolved\n"
    );
    assert_eq!(git_out(&first.fx.repo, &["status", "--porcelain"]), "");
    first.assert_removed();
    assert_eq!(first.category(), Category::Done);
}

// ---- accept after a re-prompt, and a restart ----------------------------------------------------------------------

#[test]
fn accept_of_a_reprompted_run_merges_both_phases_and_removes_the_shared_worktree() {
    let root = TempRoot::new();
    let mut first = reviewed_twice(&root, &adds("a.txt"), &adds("b.txt"));
    let second = first.reprompted(WORDS);
    wait_ended(&mut first.fx, &second.run);

    let merged = answer(&mut first.fx, &second.item, QueueAnswer::Accept {});

    assert!(
        matches!(merged, QueueAnswerResult::Merged { .. }),
        "{merged:?}"
    );
    assert!(first.fx.repo.join("a.txt").is_file());
    assert!(first.fx.repo.join("b.txt").is_file());
    first.assert_removed();
    assert_eq!(first.category(), Category::Done);
    let moves = workspace_moves(&first.fx.events());
    assert_eq!(
        moves[moves.len() - 2..],
        [WorkspaceState::Removable, WorkspaceState::Removed]
    );
    let steps = step_events(&first.fx.events());
    assert!(
        matches!(
            steps.last(),
            Some(EventBody::StepOutcome {
                result: StepResult::Succeeded,
                ..
            })
        ),
        "{steps:?}"
    );
}

#[test]
fn after_a_restart_a_reprompted_run_still_owns_the_worktree_and_can_be_accepted() {
    let root = TempRoot::new();
    let mut first = reviewed_twice(&root, &adds("a.txt"), &adds("b.txt"));
    let second = first.reprompted(WORDS);
    wait_ended(&mut first.fx, &second.run);

    first.fx.stop_daemon();
    first.fx.start_daemon();

    // The restart does not settle the old run's worktree as an accept left half-way: its item stays gone.
    assert!(first.fx.queue(&first.run).is_empty());
    let back = wait_item(&mut first.fx, &second.run, QueueKind::Review);
    assert_eq!(back, second.item);
    let got = run_get(&mut first.fx, &second.run);
    assert_eq!(got.implementations["workspace"], "builtin");
    let merged = answer(&mut first.fx, &back, QueueAnswer::Accept {});
    assert!(
        matches!(merged, QueueAnswerResult::Merged { .. }),
        "{merged:?}"
    );
    first.assert_removed();
}

#[test]
fn a_dismissed_outcome_unknown_gives_back_a_review_item_that_can_be_reprompted() {
    let root = TempRoot::new();
    let mut first = reviewed_twice(&root, &adds("a.txt"), &adds("b.txt"));

    first.item = review_back_after_an_unknown_outcome(&mut first);
    let second = first.reprompted(WORDS);

    wait_ended(&mut first.fx, &second.run);
    assert_eq!(
        subjects(&first.fx.workspace_dir(&first.run)),
        ["Add b.txt", "Add a.txt"]
    );
}
