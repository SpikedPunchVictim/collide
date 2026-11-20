//! Follow-ups through the daemon (ts-0139-wq; 0.10, Follow-ups; 0.2, Who is asking): an item a run's agent creates goes
//! to the board's private root, in the folder of its run's task's epic, with a `raised-by` link to that task and the
//! run id in `fields.run`, whatever it asked for; Keep (`move`) is for a person; and the `follow-ups` view lists the
//! open ones. The provider is the real built-in `collide/plan` pack from a built bundle, run by `node`.
//!
//! Who is asking is faked at the daemon's process tree: this test's own process reads as the CLI, or as the agent of a
//! run while a test says so. No child process is started. Every path is below the test's own root under `/tmp`, which
//! is also the home. Nothing here touches the user's home, keychain or a real daemon.

mod common;

use collide_core::change::{Actor, ActorKind};
use collide_core::events::EventBody;
use collide_core::owner::{
    RpcError, TaskApplyParams, TaskApplyResult, TaskCreateParams, TaskListResult, code,
};
use collide_core::provider::{ApplyParams, ApplyResult, Task};
use collide_core::state::RunState;
use collide_host::auth::ProcessTree;
use collide_host::board_load::pack_data_dir;
use collide_host::run_store::RunLaunch;
use collide_host::task_cli::{exit, parse_task};
use collide_host::task_daemon::run_with;
use common::StopsAtThisTest;
use common::plan_rig::{EPIC, Rig, list_until, rig_with, rpc, task, trust};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

const PLANNING: &str = ".collide/planning";
const RUN: &str = "run-follow-1";
/// The task the run works on, in epic `ep-002-bb`, and a task in epic `ep-001-aa` that no run works on.
const RUN_TASK: &str = "ts-0002-bb";
const OTHER_TASK: &str = "ts-0001-aa";
const EPIC_TWO: &str = "---\nid: ep-002-bb\nkind: epic\nstatus: active\ntitle: \"Made-up second epic\"\n\
                        owners: []\nlinks: []\n---\n";

/// The tree the other daemon tests use, which stops at this test's process (so the test reads as the CLI), except that
/// this test's process is the agent of the run `agent` holds, while it holds one.
struct AskerTree {
    inner: StopsAtThisTest,
    agent: Arc<Mutex<Option<String>>>,
}

impl ProcessTree for AskerTree {
    fn parent(&self, pid: u32) -> Option<u32> {
        self.inner.parent(pid)
    }

    fn run_marker(&self, pid: u32) -> Option<String> {
        if pid == std::process::id() {
            return self.agent.lock().unwrap().clone();
        }
        self.inner.run_marker(pid)
    }

    fn is_app(&self, pid: u32) -> bool {
        self.inner.is_app(pid)
    }
}

struct World {
    rig: Rig,
    board: String,
    agent: Arc<Mutex<Option<String>>>,
}

/// A trusted board over a repository with two epics, a task in each, and the run `RUN` working on `RUN_TASK`.
fn world() -> World {
    let agent = Arc::new(Mutex::new(None));
    let tree = agent.clone();
    let rig = rig_with("daemon-follow-ups", move |options| {
        options.process_tree = Some(Arc::new(AskerTree {
            inner: StopsAtThisTest::new(),
            agent: tree,
        }));
    });
    rig.write(&format!("{PLANNING}/epics/ep-001-aa-alpha/epic.md"), EPIC);
    rig.write(
        &format!("{PLANNING}/epics/ep-002-bb-beta/epic.md"),
        EPIC_TWO,
    );
    for (epic, id) in [
        ("ep-001-aa-alpha", OTHER_TASK),
        ("ep-002-bb-beta", RUN_TASK),
    ] {
        rig.write(
            &format!("{PLANNING}/epics/{epic}/tasks/{id}-work.md"),
            &task(id),
        );
    }
    rig.commit();
    let board = rig.board();
    trust(&rig, &board);
    list_until(&rig, &board, "the plan source lists", |list| {
        !list.refreshing && list.tasks.len() == 4
    });
    let world = World { rig, board, agent };
    world.record_run(RUN, &format!("plan:{RUN_TASK}"));
    world
}

impl World {
    /// Records that `run` is on the task `key`, as a launch does.
    fn record_run(&self, run: &str, key: &str) {
        let actor = Actor {
            kind: ActorKind::Workflow,
            run: Some(run.to_string()),
            phase: None,
        };
        self.rig
            .daemon
            .phases()
            .append_event(
                actor,
                EventBody::RunTransition {
                    run: run.to_string(),
                    from: None,
                    to: RunState::Running {},
                },
            )
            .unwrap();
        let linked = self
            .rig
            .daemon
            .store()
            .set_run_launch(
                run,
                &RunLaunch {
                    board: Some(self.board.clone()),
                    tasks: vec![collide_core::owner::RunTaskRef {
                        key: key.to_string(),
                        status: "doing".to_string(),
                    }],
                    line: "main".to_string(),
                    workflow: "collide/quick".to_string(),
                },
            )
            .unwrap();
        assert!(linked, "{run} has a run.transition before it is linked");
    }

    /// The connections made from now on are the agent of `run`, or the CLI when `None`.
    fn ask_as(&self, run: Option<&str>) {
        *self.agent.lock().unwrap() = run.map(str::to_string);
    }

    fn private_root(&self) -> PathBuf {
        let data_root = self.rig.daemon.paths().runs_dir.with_file_name("pack-data");
        pack_data_dir(&data_root, &self.board, "collide/plan")
            .unwrap()
            .join("planning")
    }

    fn apply(&self, apply: ApplyParams) -> Result<TaskApplyResult, RpcError> {
        rpc(
            &self.rig,
            "task.apply",
            &TaskApplyParams {
                board: self.board.clone(),
                source: Some("plan".to_string()),
                apply,
                commit: false,
            },
        )
    }

    /// A `create` on the plan source through `task.apply`, with the args `with` names.
    fn create(&self, with: Value) -> Result<TaskApplyResult, RpcError> {
        self.apply(ApplyParams {
            actor: None,
            op: "create".to_string(),
            id: None,
            args: args(with),
            version: None,
            checkout: None,
        })
    }

    fn list(&self, view: &str) -> TaskListResult {
        rpc(
            &self.rig,
            "task.list",
            &json!({ "board": self.board, "view": { "type": "named", "name": view } }),
        )
        .unwrap()
    }
}

fn args(value: Value) -> BTreeMap<String, Value> {
    serde_json::from_value(value).unwrap()
}

fn made(answer: Result<TaskApplyResult, RpcError>) -> (Task, String) {
    let result = answer.unwrap().result.expect("applied, not queued");
    let ApplyResult::Ok { task, version, .. } = result else {
        panic!("not applied: {result:?}")
    };
    (task, version)
}

fn links_of(task: &Task) -> Vec<(String, String)> {
    task.links
        .iter()
        .map(|link| (link.link_type.clone(), link.to.clone()))
        .collect()
}

fn pair(link_type: &str, to: &str) -> (String, String) {
    (link_type.to_string(), to.to_string())
}

fn status(world: &World) -> String {
    world.rig.repo.git(&["status", "--porcelain"])
}

fn ids(list: &TaskListResult) -> Vec<String> {
    list.tasks
        .iter()
        .filter_map(|task| task.key.strip_prefix("plan:").map(str::to_string))
        .collect()
}

// ---- the create rule ----------------------------------------------------------------------------------------------

#[test]
fn a_create_from_a_runs_agent_goes_private_in_its_tasks_epic_with_raised_by_and_run() {
    let world = world();
    world.ask_as(Some(RUN));

    // It asks for the shared root, another epic, another run and another raised-by: none is trusted.
    let (task, _) = made(world.create(json!({
        "kind": "task", "epic": "ep-001-aa", "title": "Found on the way", "root": "shared", "seed": 1,
        "raisedBy": OTHER_TASK, "run": "run-forged",
    })));

    assert!(
        task.path.as_deref().unwrap().starts_with("private:"),
        "{:?}",
        task.path
    );
    assert_eq!(
        links_of(&task),
        [pair("part-of", "ep-002-bb"), pair("raised-by", RUN_TASK)]
    );
    assert_eq!(task.fields.get("run"), Some(&json!(RUN)));
    let folder = world.private_root().join("epics/ep-002-bb/tasks");
    assert_eq!(std::fs::read_dir(folder).unwrap().count(), 1);
    assert_eq!(status(&world), "", "nothing in the repository");
}

#[test]
fn task_create_from_a_runs_agent_is_shaped_the_same_way() {
    let world = world();
    world.ask_as(Some(RUN));

    let answer: TaskApplyResult = rpc(
        &world.rig,
        "task.create",
        &TaskCreateParams {
            board: world.board.clone(),
            title: "Found by create".to_string(),
            kind: Some("question".to_string()),
            epic: Some("ep-001-aa".to_string()),
            seed: Some(1),
            root: Some("shared".to_string()),
            args: BTreeMap::new(),
            checkout: None,
        },
    )
    .unwrap();

    let Some(ApplyResult::Ok { task, .. }) = answer.result else {
        panic!("not applied: {answer:?}")
    };
    assert!(task.path.as_deref().unwrap().starts_with("private:"));
    assert_eq!(
        links_of(&task),
        [pair("part-of", "ep-002-bb"), pair("raised-by", RUN_TASK)]
    );
    assert_eq!(task.fields.get("run"), Some(&json!(RUN)));
}

#[test]
fn a_run_whose_task_is_not_in_the_planning_source_writes_run_and_no_raised_by() {
    let world = world();
    world.record_run("run-scratch", "scratch:abc");
    world.record_run("run-gone", "plan:ts-9999-zz");
    for run in ["run-scratch", "run-gone", "run-unknown"] {
        world.ask_as(Some(run));

        let (task, _) = made(world.create(json!({
            "kind": "task", "epic": "ep-001-aa", "title": format!("From {run}"), "seed": 1, "raisedBy": OTHER_TASK,
        })));

        // The epic it asked for is kept: there is no run task to take one from.
        assert_eq!(links_of(&task), [pair("part-of", "ep-001-aa")], "{run}");
        assert_eq!(task.fields.get("run"), Some(&json!(run)), "{run}");
        assert!(
            task.path.as_deref().unwrap().starts_with("private:"),
            "{run}"
        );
    }
}

#[test]
fn a_create_from_the_cli_goes_where_it_asks_with_nothing_added() {
    let world = world();
    world.ask_as(None);

    let (shared, _) = made(world.create(json!({
        "kind": "task", "epic": "ep-001-aa", "title": "By hand", "seed": 1,
    })));

    assert!(!shared.path.as_deref().unwrap().starts_with("private:"));
    assert_eq!(links_of(&shared), [pair("part-of", "ep-001-aa")]);
    assert_eq!(shared.fields.get("run"), None);
    assert!(status(&world).contains("by-hand"), "{}", status(&world));

    let (private, _) = made(world.create(json!({
        "kind": "task", "epic": "ep-001-aa", "title": "By hand, private", "root": "private", "seed": 2,
    })));
    assert!(private.path.as_deref().unwrap().starts_with("private:"));
    assert_eq!(links_of(&private), [pair("part-of", "ep-001-aa")]);
    assert_eq!(private.fields.get("run"), None);
}

// ---- Keep ---------------------------------------------------------------------------------------------------------

fn keep_request(id: &str, version: &str) -> ApplyParams {
    ApplyParams {
        actor: None,
        op: "move".to_string(),
        id: Some(id.to_string()),
        args: BTreeMap::new(),
        version: Some(version.to_string()),
        checkout: None,
    }
}

#[test]
fn keep_is_refused_to_a_runs_agent_naming_who_may_and_moves_nothing() {
    let world = world();
    world.ask_as(Some(RUN));
    let (task, version) = made(
        world.create(json!({"kind": "task", "epic": "ep-001-aa", "title": "Mine", "seed": 1})),
    );

    let error = world.apply(keep_request(&task.id, &version)).unwrap_err();

    assert_eq!(error.code, code::NOT_PERMITTED);
    assert!(error.message.contains(RUN), "{}", error.message);
    assert!(
        error.message.contains("the app or the CLI"),
        "{}",
        error.message
    );
    let folder = world.private_root().join("epics/ep-002-bb/tasks");
    assert_eq!(std::fs::read_dir(folder).unwrap().count(), 1);
    assert_eq!(status(&world), "");
}

#[test]
fn keep_from_the_cli_moves_the_item_to_the_shared_epic_folder_and_commit_holds_exactly_that_file() {
    let world = world();
    world.ask_as(Some(RUN));
    let (task, _) = made(world.create(
        json!({"kind": "question", "epic": "ep-001-aa", "title": "Which one?", "seed": 1}),
    ));
    world.ask_as(None);
    let head = world.rig.repo.git(&["rev-parse", "HEAD"]);
    // A file the commit must not take.
    world.rig.write("elsewhere.txt", "not part of the keep\n");

    let words: Vec<String> = [&format!("plan:{}", task.id), "move", "--commit"]
        .iter()
        .map(|word| word.to_string())
        .collect();
    let command = parse_task(&words).unwrap();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let exit_code = run_with(
        world.rig.daemon.paths(),
        &command,
        world.rig.repo.work(),
        &mut out,
        &mut err,
    );

    let err = String::from_utf8(err).unwrap();
    assert_eq!(exit_code, exit::OK, "{err}");
    let kept = format!(
        "{PLANNING}/epics/ep-002-bb-beta/questions/{}-which-one.md",
        task.id
    );
    assert_ne!(
        world.rig.repo.git(&["rev-parse", "HEAD"]),
        head,
        "a commit was made"
    );
    assert_eq!(
        world
            .rig
            .repo
            .git(&["show", "--name-only", "--format=", "HEAD"]),
        kept
    );
    assert_eq!(status(&world), "?? elsewhere.txt");
    assert!(world.rig.repo.work().join(&kept).is_file());
    assert!(
        !world
            .private_root()
            .join("epics/ep-002-bb/questions")
            .read_dir()
            .unwrap()
            .any(|_| true)
    );
    let text = std::fs::read_to_string(world.rig.repo.work().join(&kept)).unwrap();
    assert!(text.contains(&format!("run: \"{RUN}\"")), "{text}");
    assert!(text.contains(&format!("to: {RUN_TASK}")), "{text}");
}

// ---- the view -----------------------------------------------------------------------------------------------------

#[test]
fn the_follow_ups_view_lists_open_private_items_with_a_raised_by_link_and_no_other() {
    let world = world();
    world.ask_as(Some(RUN));
    let (open, _) = made(
        world.create(json!({"kind": "task", "epic": "ep-001-aa", "title": "Open one", "seed": 1})),
    );
    let (later, _) =
        made(world.create(
            json!({"kind": "question", "epic": "ep-001-aa", "title": "Open two", "seed": 2}),
        ));
    let (dropped, dropped_version) =
        made(world.create(
            json!({"kind": "task", "epic": "ep-001-aa", "title": "Dropped one", "seed": 3}),
        ));
    let (kept, kept_version) = made(
        world.create(json!({"kind": "task", "epic": "ep-001-aa", "title": "Kept one", "seed": 4})),
    );
    world.ask_as(None);
    let (by_hand, _) = made(world.create(json!({
        "kind": "task", "epic": "ep-001-aa", "title": "By hand", "root": "private", "seed": 5,
    })));
    let status_write = ApplyParams {
        actor: None,
        op: "setStatus".to_string(),
        id: Some(dropped.id.clone()),
        args: args(json!({"status": "dropped"})),
        version: Some(dropped_version),
        checkout: None,
    };
    world.apply(status_write).unwrap();
    world.apply(keep_request(&kept.id, &kept_version)).unwrap();

    let list = list_until(
        &world.rig,
        &world.board,
        "all seven items are listed",
        |list| !list.refreshing && list.tasks.len() == 9,
    );
    assert_eq!(list.tasks.len(), 9);
    let mut found = ids(&world.list("collide/plan/follow-ups"));
    found.sort();
    let mut expected = vec![open.id.clone(), later.id.clone()];
    expected.sort();
    assert_eq!(
        found, expected,
        "not {}, {}, {}",
        dropped.id, kept.id, by_hand.id
    );
    assert_eq!(
        ids(&world.list("follow-ups")).len(),
        2,
        "its bare name finds it too"
    );
}
