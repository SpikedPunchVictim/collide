//! `run.list`, `run.get` and the indexed read of a run's latest state (ts-0076-cw; 0.8, Run; Stage 3c, closing the
//! window and reopening it shows the same run). The daemon runs inside the test process, on a socket and a store below
//! the test's own root under `/tmp`. Runs are made by writing their `run.transition` events through the daemon's own
//! `Phases::append_event`, and linked to a board and tasks the way `run.start` will link them. Nothing here reads the
//! owner's files, starts a daemon process or loads a launchd job.

mod common;

use collide_core::change::{Actor, ActorKind};
use collide_core::events::{Event, EventBody};
use collide_core::non_empty::NonEmpty;
use collide_core::owner::{
    RunGetParams, RunGetResult, RunListParams, RunListResult, RunScope, RunSummary, RunTaskRef,
    code,
};
use collide_core::state::{PauseCause, RunState};
use collide_host::base_env::CaptureOptions;
use collide_host::daemon::{Daemon, DaemonOptions, DaemonPaths};
use collide_host::run_store::RunLaunch;
use collide_host::store::Store;
use common::TempRoot;
use common::subscriber::Stream;
use rusqlite::params;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::time::Duration;

fn start(root: &TempRoot) -> Daemon {
    let shell = root
        .script("fake-shell", "#!/bin/sh\neval \"$4\"\n")
        .to_str()
        .unwrap()
        .to_string();
    let mut options = DaemonOptions::new(
        DaemonPaths::under(root.path()),
        shell,
        Box::new(std::io::sink()),
    );
    options.process_tree = common::test_process_tree();
    options.capture = CaptureOptions {
        timeout: Duration::from_secs(10),
        cap_bytes: 1024 * 1024,
        fallback: BTreeMap::from([("PATH".to_string(), "/usr/bin:/bin".to_string())]),
    };
    Daemon::start(options).expect("the daemon starts")
}

fn ended() -> RunState {
    RunState::Ended {
        outcome: NonEmpty::new("done").unwrap(),
    }
}

/// Records one `run.transition` through the daemon's door for events.
fn transition(daemon: &Daemon, run: &str, from: Option<RunState>, to: RunState) {
    daemon
        .phases()
        .append_event(
            Actor {
                kind: ActorKind::Workflow,
                run: Some(run.to_string()),
                phase: None,
            },
            EventBody::RunTransition {
                run: run.to_string(),
                from,
                to,
            },
        )
        .unwrap();
}

fn launch(daemon: &Daemon, run: &str, board: &str, tasks: &[(&str, &str)]) {
    let linked = daemon
        .store()
        .set_run_launch(
            run,
            &RunLaunch {
                board: Some(board.to_string()),
                tasks: tasks
                    .iter()
                    .map(|(key, status)| RunTaskRef {
                        key: key.to_string(),
                        status: status.to_string(),
                    })
                    .collect(),
                line: "main".to_string(),
                workflow: "collide/quick".to_string(),
            },
        )
        .unwrap();
    assert!(linked, "{run} has a run.transition before it is linked");
}

/// Five runs, oldest first: `r1` ended on board-1, `r2` live on board-1, `r3` failed on board-2, `r4` a stopped file
/// run with no board, and `r5` a file run paused for disk. Returns the two boards' names.
fn five_runs(daemon: &Daemon) -> (String, String) {
    let store = daemon.store();
    let one = store.open_board("/work/one").unwrap();
    let two = store.open_board("/work/two").unwrap();
    transition(daemon, "r1", None, RunState::Running {});
    launch(daemon, "r1", &one, &[("scratch:1", "status-a")]);
    transition(daemon, "r1", Some(RunState::Running {}), ended());
    transition(daemon, "r2", None, RunState::Running {});
    launch(
        daemon,
        "r2",
        &one,
        &[("scratch:2", "status-a"), ("scratch:3", "status-b")],
    );
    transition(daemon, "r3", None, RunState::Running {});
    launch(daemon, "r3", &two, &[("scratch:1", "status-a")]);
    transition(
        daemon,
        "r3",
        Some(RunState::Running {}),
        RunState::Failed {},
    );
    transition(daemon, "r4", None, RunState::Running {});
    transition(
        daemon,
        "r4",
        Some(RunState::Running {}),
        RunState::Stopped {},
    );
    transition(daemon, "r5", None, RunState::Running {});
    transition(
        daemon,
        "r5",
        Some(RunState::Running {}),
        RunState::Paused {
            cause: PauseCause::Disk,
        },
    );
    (one, two)
}

fn list(client: &mut Stream, params: Value) -> RunListResult {
    client.call("run.list", params)
}

fn ids(result: &RunListResult) -> Vec<&str> {
    result.runs.iter().map(|run| run.id.as_str()).collect()
}

fn get(client: &mut Stream, run: &str) -> RunGetResult {
    client.call(
        "run.get",
        serde_json::to_value(RunGetParams {
            run: run.to_string(),
        })
        .unwrap(),
    )
}

// ---- run.list and run.get ------------------------------------------------------------------------------------------

#[test]
fn run_list_gives_the_runs_newest_first_with_the_file_runs_that_have_no_board() {
    let root = TempRoot::new();
    let daemon = start(&root);
    five_runs(&daemon);
    let mut client = Stream::connect(daemon.paths());
    let all = list(&mut client, json!({}));
    assert_eq!(ids(&all), ["r5", "r4", "r3", "r2", "r1"]);
    assert_eq!(all.next_cursor, None);
}

#[test]
fn run_list_by_board_gives_the_runs_of_that_board_only() {
    let root = TempRoot::new();
    let daemon = start(&root);
    let (one, two) = five_runs(&daemon);
    let mut client = Stream::connect(daemon.paths());
    assert_eq!(ids(&list(&mut client, json!({"board": one}))), ["r2", "r1"]);
    assert_eq!(ids(&list(&mut client, json!({"board": two}))), ["r3"]);
}

#[test]
fn run_list_by_task_gives_the_runs_that_name_the_task_and_with_a_board_those_of_the_board() {
    let root = TempRoot::new();
    let daemon = start(&root);
    let (one, _) = five_runs(&daemon);
    let mut client = Stream::connect(daemon.paths());
    assert_eq!(
        ids(&list(&mut client, json!({"task": "scratch:1"}))),
        ["r3", "r1"]
    );
    assert_eq!(
        ids(&list(
            &mut client,
            json!({"task": "scratch:1", "board": one})
        )),
        ["r1"]
    );
    assert_eq!(
        ids(&list(&mut client, json!({"task": "scratch:3"}))),
        ["r2"]
    );
    assert!(
        list(&mut client, json!({"task": "scratch:9"}))
            .runs
            .is_empty()
    );
}

#[test]
fn run_list_by_scope_separates_the_live_runs_from_the_finished_ones() {
    let root = TempRoot::new();
    let daemon = start(&root);
    let (one, _) = five_runs(&daemon);
    let mut client = Stream::connect(daemon.paths());
    assert_eq!(
        ids(&list(&mut client, json!({"scope": "live"}))),
        ["r5", "r2"]
    );
    assert_eq!(
        ids(&list(&mut client, json!({"scope": "finished"}))),
        ["r4", "r3", "r1"]
    );
    assert_eq!(
        ids(&list(
            &mut client,
            json!({"scope": "finished", "board": one})
        )),
        ["r1"]
    );
    let all = RunListParams {
        board: None,
        scope: RunScope::All,
        task: None,
        limit: None,
        cursor: None,
    };
    assert_eq!(
        ids(&list(&mut client, serde_json::to_value(all).unwrap())).len(),
        5
    );
}

#[test]
fn run_list_pages_with_a_cursor_and_the_last_page_has_none() {
    let root = TempRoot::new();
    let daemon = start(&root);
    five_runs(&daemon);
    let mut client = Stream::connect(daemon.paths());
    let first = list(&mut client, json!({"limit": 2}));
    assert_eq!(ids(&first), ["r5", "r4"]);
    let second = list(
        &mut client,
        json!({"limit": 2, "cursor": first.next_cursor}),
    );
    assert_eq!(ids(&second), ["r3", "r2"]);
    let third = list(
        &mut client,
        json!({"limit": 2, "cursor": second.next_cursor}),
    );
    assert_eq!(ids(&third), ["r1"]);
    assert_eq!(third.next_cursor, None);
}

#[test]
fn run_list_refuses_a_board_the_daemon_has_not_opened_and_a_cursor_it_did_not_give() {
    let root = TempRoot::new();
    let daemon = start(&root);
    let mut client = Stream::connect(daemon.paths());
    let error = client
        .call_raw("run.list", json!({"board": "board-9"}))
        .unwrap_err();
    assert_eq!(error["code"], code::NOT_FOUND);
    let error = client
        .call_raw("run.list", json!({"cursor": "not a cursor"}))
        .unwrap_err();
    assert_eq!(error["code"], code::INVALID_PARAMS);
}

fn summary_of<'a>(list: &'a RunListResult, run: &str) -> &'a RunSummary {
    list.runs.iter().find(|r| r.id == run).unwrap()
}

#[test]
fn a_summary_has_the_tasks_the_state_the_times_and_the_line_and_workflow_the_run_was_launched_with()
{
    let root = TempRoot::new();
    let daemon = start(&root);
    five_runs(&daemon);
    let mut client = Stream::connect(daemon.paths());
    let all = list(&mut client, json!({}));

    let r1 = summary_of(&all, "r1");
    assert_eq!(r1.state, ended());
    assert_eq!(
        (r1.line.as_str(), r1.workflow.as_str()),
        ("main", "collide/quick")
    );
    assert_eq!(
        r1.tasks,
        [RunTaskRef {
            key: "scratch:1".into(),
            status: "status-a".into()
        }]
    );
    assert!(r1.ended_ms.is_some_and(|end| end >= r1.started_ms));

    let r2 = summary_of(&all, "r2");
    assert_eq!(r2.state, RunState::Running {});
    assert_eq!(r2.ended_ms, None);
    let keys: Vec<&str> = r2.tasks.iter().map(|t| t.key.as_str()).collect();
    assert_eq!(
        keys,
        ["scratch:2", "scratch:3"],
        "the tasks keep their order"
    );

    let r4 = summary_of(&all, "r4");
    assert!(r4.tasks.is_empty());
    assert_eq!((r4.line.as_str(), r4.workflow.as_str()), ("", ""));
    assert_eq!(
        summary_of(&all, "r5").state,
        RunState::Paused {
            cause: PauseCause::Disk
        }
    );
}

#[test]
fn run_get_gives_the_summary_of_one_run_and_not_found_for_a_run_it_has_not_heard_of() {
    let root = TempRoot::new();
    let daemon = start(&root);
    five_runs(&daemon);
    let mut client = Stream::connect(daemon.paths());
    let all = list(&mut client, json!({}));
    assert_eq!(&get(&mut client, "r3").summary, summary_of(&all, "r3"));
    let error = client
        .call_raw("run.get", json!({"run": "no-such-run"}))
        .unwrap_err();
    assert_eq!(error["code"], code::NOT_FOUND);
}

#[test]
fn run_list_and_run_get_give_the_same_answers_after_the_daemon_restarts() {
    let root = TempRoot::new();
    let daemon = start(&root);
    let (one, _) = five_runs(&daemon);
    let mut client = Stream::connect(daemon.paths());
    let before_all = list(&mut client, json!({}));
    let before_board = list(&mut client, json!({"board": one}));
    let before_live = list(&mut client, json!({"scope": "live"}));
    let before_get: Vec<RunGetResult> = ["r1", "r2", "r5"].map(|run| get(&mut client, run)).into();
    drop(client);
    daemon.stop();

    let daemon = start(&root);
    let mut client = Stream::connect(daemon.paths());
    assert_eq!(list(&mut client, json!({})), before_all);
    assert_eq!(list(&mut client, json!({"board": one})), before_board);
    assert_eq!(list(&mut client, json!({"scope": "live"})), before_live);
    let after_get: Vec<RunGetResult> = ["r1", "r2", "r5"].map(|run| get(&mut client, run)).into();
    assert_eq!(after_get, before_get);
}

// ---- the indexed read of a run's latest state ----------------------------------------------------------------------

fn event(seq: u64, run: &str, from: Option<RunState>, to: RunState) -> Event {
    Event {
        seq,
        wall_ms: 1_000 + seq,
        actor: Actor {
            kind: ActorKind::Workflow,
            run: Some(run.to_string()),
            phase: None,
        },
        experiment: None,
        body: EventBody::RunTransition {
            run: run.to_string(),
            from,
            to,
        },
    }
}

/// What `Phases::recorded_run_state` did before the index: the `to` of the run's last `run.transition` in the log.
fn scan(store: &Store, run: &str) -> Option<RunState> {
    store
        .events_since(0)
        .unwrap()
        .into_iter()
        .rev()
        .find_map(|event| match event.body {
            EventBody::RunTransition { run: named, to, .. } if named == run => Some(to),
            _ => None,
        })
}

/// A store with `runs` runs whose transitions are interleaved, so no run's last event is next to another's.
fn store_with_runs(root: &TempRoot, runs: u32) -> (Store, Vec<String>) {
    let store = Store::open(&root.join("store")).unwrap();
    let names: Vec<String> = (0..runs).map(|n| format!("run-{n}")).collect();
    let mut seq = 0;
    let mut next = || {
        seq += 1;
        seq
    };
    for name in &names {
        store
            .append_event(&event(next(), name, None, RunState::Running {}))
            .unwrap();
    }
    for (n, name) in names.iter().enumerate() {
        let to = match n % 4 {
            0 => ended(),
            1 => RunState::Failed {},
            2 => RunState::Paused {
                cause: PauseCause::UsageLimit,
            },
            _ => continue,
        };
        store
            .append_event(&event(next(), name, Some(RunState::Running {}), to))
            .unwrap();
    }
    (store, names)
}

#[test]
fn the_indexed_read_gives_what_the_scan_gave_for_every_run_of_a_store_with_many() {
    let root = TempRoot::new();
    let (store, names) = store_with_runs(&root, 300);
    for name in &names {
        assert_eq!(store.run_state(name).unwrap(), scan(&store, name), "{name}");
    }
    assert_eq!(store.run_state("no-such-run").unwrap(), None);
    assert_eq!(scan(&store, "no-such-run"), None);
}

#[test]
fn the_indexed_read_does_not_read_the_event_log() {
    let root = TempRoot::new();
    let (store, names) = store_with_runs(&root, 20);
    // An unrelated event this build cannot read: the scan fails on it, and a read that does not scan the log does not.
    let conn = rusqlite::Connection::open(store.db_path()).unwrap();
    conn.execute(
        "UPDATE events SET line = ?1 WHERE seq = 1",
        params!["not an event"],
    )
    .unwrap();
    assert!(store.events_since(0).is_err());
    assert_eq!(
        store.run_state(&names[7]).unwrap(),
        Some(RunState::Running {})
    );
    assert_eq!(store.run_state(&names[8]).unwrap(), Some(ended()));
}
