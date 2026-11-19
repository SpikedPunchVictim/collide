//! The plan pack's person-only ops through the daemon (ts-0129-uv; 0.10, Planning sources, "Only a person accepts,
//! defers, rejects or resolves"): `accept`, `defer`, `reject` and `resolve` over the daemon's real socket, with the
//! real built-in `collide/plan` provider run by `node` from a built bundle. The daemon reads the caller from its
//! process tree: this test's process is the CLI, or the app while a test says so. The clock stands still at midday of
//! 2026-06-15 (UTC), so the date the provider writes into `by:` is the daemon's, and the same on every machine.
//!
//! Every path is below the test's own root under `/tmp`, which is also the home. The daemon's captured environment
//! comes from a login shell that clears the environment. Nothing here touches the user's home, keychain or a real
//! daemon, and no `collide` binary is run.

mod common;

use collide_core::owner::{
    QueueAnswer, QueueAnswerParams, QueueAnswerResult, QueueItem, QueueKind, QueueListParams,
    QueueListResult, TaskApplyParams, TaskApplyResult, TaskGetParams, TaskGetResult,
};
use collide_core::provider::{ApplyParams, ApplyResult};
use collide_host::auth::ProcessTree;
use collide_host::clock::{Clock, Clocks, Reading, SystemClocks};
use common::StopsAtThisTest;
use common::plan_rig::{EPIC, Rig, rig_with, rpc, trust};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::fs;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// 2026-06-15 12:00:00 UTC, and the date it makes in UTC.
const MIDDAY_NANOS: u64 = 1_781_524_800 * 1_000_000_000;
const TODAY: &str = "2026-06-15";

const PROPOSALS: &str = ".collide/planning/epics/ep-001-aa-alpha/proposals";
const QUESTIONS: &str = ".collide/planning/epics/ep-001-aa-alpha/questions";
const PROPOSAL_FILE: &str = "pr-0001-aa-plan.md";
const OTHER_FILE: &str = "pr-0002-bb-other.md";
const QUESTION_FILE: &str = "qu-0001-aa-why.md";

/// The system's clocks, except that the wall clock stands still at [`MIDDAY_NANOS`].
struct MiddayClocks;

impl Clocks for MiddayClocks {
    fn read(&self, clock: Clock) -> Reading {
        match clock {
            Clock::Wall => Reading {
                clock,
                nanos: MIDDAY_NANOS,
            },
            other => SystemClocks.read(other),
        }
    }
}

/// The tree the daemon tests use, which stops at this test's process (so the test reads as the CLI), except that this
/// test's process is the app while `app` is set.
struct CallerTree {
    inner: StopsAtThisTest,
    app: Arc<AtomicBool>,
}

impl ProcessTree for CallerTree {
    fn parent(&self, pid: u32) -> Option<u32> {
        self.inner.parent(pid)
    }

    fn run_marker(&self, pid: u32) -> Option<String> {
        self.inner.run_marker(pid)
    }

    fn is_app(&self, pid: u32) -> bool {
        (pid == std::process::id() && self.app.load(Ordering::SeqCst)) || self.inner.is_app(pid)
    }
}

fn proposal(id: &str, status: &str) -> String {
    format!(
        "---\nid: {id}\nkind: proposal\nstatus: {status}\ntitle: \"Made-up proposal\"\nowners: []\n\
         links: []\n---\n\n## Features\n\n### Packs\n- **F1 Trust prompt**: a repo pack asks first.\n\n## Decision\n"
    )
}

fn question() -> String {
    "---\nid: qu-0001-aa\nkind: question\nstatus: open\ntitle: \"Made-up question\"\nowners: []\nlinks: []\n---\n\n\
     ## Question\n\n## Decision\n"
        .to_string()
}

struct Board {
    rig: Rig,
    id: String,
    app: Arc<AtomicBool>,
}

/// A trusted board over a repository with an epic, two proposed proposals and an open question.
fn board() -> Board {
    let app = Arc::new(AtomicBool::new(false));
    let flag = app.clone();
    let rig = rig_with("daemon-plan-person-ops", move |options| {
        options.process_tree = Some(Arc::new(CallerTree {
            inner: StopsAtThisTest::new(),
            app: flag,
        }));
        options.clocks = Arc::new(MiddayClocks);
        options.utc_offset_secs = Some(0);
    });
    rig.write(".collide/planning/epics/ep-001-aa-alpha/epic.md", EPIC);
    rig.write(
        &format!("{PROPOSALS}/{PROPOSAL_FILE}"),
        &proposal("pr-0001-aa", "proposed"),
    );
    rig.write(
        &format!("{PROPOSALS}/{OTHER_FILE}"),
        &proposal("pr-0002-bb", "proposed"),
    );
    rig.write(&format!("{QUESTIONS}/{QUESTION_FILE}"), &question());
    rig.commit();
    let id = rig.board();
    trust(&rig, &id);
    Board { rig, id, app }
}

impl Board {
    fn as_the_app(&self) {
        self.app.store(true, Ordering::SeqCst);
    }

    fn text(&self, folder: &str, file: &str) -> String {
        fs::read_to_string(self.rig.repo.work().join(folder).join(file)).unwrap()
    }

    /// The version of the item `key` the daemon last read from its file.
    fn version_of(&self, key: &str) -> String {
        let got: TaskGetResult = rpc(
            &self.rig,
            "task.get",
            &TaskGetParams {
                board: self.id.clone(),
                id: key.to_string(),
                checkout: None,
            },
        )
        .unwrap();
        got.task.expect("the item is listed").version
    }

    /// One `task.apply` on the plan source, with the item's current version.
    fn apply(&self, op: &str, id: &str, with: Value) -> TaskApplyResult {
        let args: BTreeMap<String, Value> = serde_json::from_value(with).unwrap();
        rpc(
            &self.rig,
            "task.apply",
            &TaskApplyParams {
                board: self.id.clone(),
                source: Some("plan".to_string()),
                apply: ApplyParams {
                    actor: None,
                    op: op.to_string(),
                    id: Some(id.to_string()),
                    args,
                    version: Some(self.version_of(&format!("plan:{id}"))),
                    checkout: None,
                },
                commit: false,
            },
        )
        .unwrap()
    }

    fn queue(&self) -> Vec<QueueItem> {
        let listed: QueueListResult = rpc(
            &self.rig,
            "queue.list",
            &QueueListParams {
                board: None,
                run: None,
                kinds: Vec::new(),
            },
        )
        .unwrap();
        listed.items
    }

    fn answer(&self, item: &str, answer: QueueAnswer) -> QueueAnswerResult {
        rpc(
            &self.rig,
            "queue.answer",
            &QueueAnswerParams {
                item: item.to_string(),
                answer,
            },
        )
        .unwrap()
    }
}

fn written(result: &TaskApplyResult) -> &ApplyResult {
    let done = result
        .result
        .as_ref()
        .unwrap_or_else(|| panic!("the write was applied: {result:?}"));
    assert!(matches!(done, ApplyResult::Ok { .. }), "{done:?}");
    done
}

fn confirmation_of(result: &TaskApplyResult) -> String {
    assert!(result.result.is_none(), "nothing was written: {result:?}");
    result
        .confirmation
        .as_ref()
        .expect("the write waits for a confirmation")
        .item
        .clone()
}

#[test]
fn from_the_app_accept_writes_by_with_the_date_of_the_daemons_clock() {
    let board = board();
    board.as_the_app();

    let result = board.apply("accept", "pr-0001-aa", json!({}));

    written(&result);
    assert_eq!(
        board.text(PROPOSALS, PROPOSAL_FILE),
        proposal("pr-0001-aa", "proposed").replace(
            "status: proposed\n",
            &format!("status: accepted\nby: human:{TODAY}\n")
        )
    );
}

#[test]
fn from_the_app_setstatus_accepted_writes_the_same_lines_as_accept() {
    let board = board();
    board.as_the_app();

    written(&board.apply("accept", "pr-0001-aa", json!({})));
    written(&board.apply("setStatus", "pr-0002-bb", json!({"status": "accepted"})));

    let by_op = board.text(PROPOSALS, PROPOSAL_FILE);
    let by_status = board.text(PROPOSALS, OTHER_FILE);
    assert_eq!(by_op.replace("pr-0001-aa", "pr-0002-bb"), by_status);
    assert!(by_status.contains(&format!("by: human:{TODAY}\n")));
}

#[test]
fn from_the_app_defer_reject_and_resolve_write_their_lines() {
    let board = board();
    board.as_the_app();

    written(&board.apply(
        "defer",
        "pr-0001-aa",
        json!({"trigger": "after the pack lock"}),
    ));
    written(&board.apply("reject", "pr-0002-bb", json!({"reason": "out of scope"})));
    written(&board.apply(
        "resolve",
        "qu-0001-aa",
        json!({"text": "Use the file store."}),
    ));

    let deferred = board.text(PROPOSALS, PROPOSAL_FILE);
    assert!(deferred.contains("status: deferred\ntrigger: \"after the pack lock\"\n"));
    let rejected = board.text(PROPOSALS, OTHER_FILE);
    assert!(rejected.contains("status: rejected\nreason: \"out of scope\"\n"));
    let resolved = board.text(QUESTIONS, QUESTION_FILE);
    assert!(resolved.contains(&format!("status: resolved\nresolved_by: human:{TODAY}\n")));
    assert!(
        resolved.ends_with("## Decision\n\nUse the file store.\n"),
        "{resolved}"
    );
}

#[test]
fn from_the_cli_accept_waits_for_a_confirmation_and_the_app_answering_it_writes_by() {
    let board = board();

    let result = board.apply("accept", "pr-0001-aa", json!({}));

    let item = confirmation_of(&result);
    assert_eq!(
        board.text(PROPOSALS, PROPOSAL_FILE),
        proposal("pr-0001-aa", "proposed"),
        "nothing was written"
    );
    let queue = board.queue();
    assert_eq!(queue.len(), 1, "{queue:?}");
    assert_eq!(queue[0].kind, QueueKind::Confirmation);
    board.as_the_app();

    let answered = board.answer(&item, QueueAnswer::Accept {});

    assert_eq!(answered, QueueAnswerResult::Done {});
    let text = board.text(PROPOSALS, PROPOSAL_FILE);
    assert!(text.contains("status: accepted\n"), "{text}");
    assert!(text.contains(&format!("by: human:{TODAY}\n")), "{text}");
    assert!(board.queue().is_empty());
}

#[test]
fn from_the_cli_each_of_the_four_ops_waits_and_no_file_changes() {
    let board = board();
    let before = (
        board.text(PROPOSALS, PROPOSAL_FILE),
        board.text(PROPOSALS, OTHER_FILE),
        board.text(QUESTIONS, QUESTION_FILE),
    );

    for (op, id, with) in [
        ("accept", "pr-0001-aa", json!({})),
        ("defer", "pr-0001-aa", json!({"trigger": "later"})),
        ("reject", "pr-0002-bb", json!({})),
        ("resolve", "qu-0001-aa", json!({"text": "Yes."})),
    ] {
        confirmation_of(&board.apply(op, id, with));
    }

    assert_eq!(board.queue().len(), 4);
    assert_eq!(
        before,
        (
            board.text(PROPOSALS, PROPOSAL_FILE),
            board.text(PROPOSALS, OTHER_FILE),
            board.text(QUESTIONS, QUESTION_FILE)
        )
    );
}
