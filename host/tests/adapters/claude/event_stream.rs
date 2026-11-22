//! The event stream and the run reads of a file-task run (ts-0076-cw): `queue.changed` to a subscriber when a
//! permission request comes and when it is answered, `daemon.event` for the run's own events, and `run.list` and
//! `run.get` for a run that fake Claude played, also after the daemon restarted. The daemon is a `Rig` of `pause.rs`:
//! its own temp root, `env_clear()` plus what the test sets, `HOME` in the root, git without configuration, and fake
//! Claude as `claude`.

use super::common::subscriber::{Next, Stream};
use super::common::wait_until;
use super::fake::recording;
use super::pause::{Rig, Settings, permission_item};
use collide_core::agent::PermissionReply;
use collide_core::events::{Event, EventBody};
use collide_core::owner::{
    Notice, PhaseKindWire, QueueAnswer, QueueChanged, RunGetResult, RunListResult,
};
use collide_core::state::{PhaseState, RunState};
use collide_host::daemon::DaemonPaths;
use collide_host::file_run::PHASE_NAME;
use serde_json::json;
use std::time::Duration;

const WAIT: Duration = Duration::from_secs(30);

fn watcher(rig: &Rig) -> Stream {
    let mut stream = Stream::connect(&DaemonPaths::under(rig.root.path()));
    stream.subscribe(None);
    stream
}

/// Reads notices until a `queue.changed`, and returns it with the events that came before it.
fn until_queue_changed(stream: &mut Stream) -> (QueueChanged, Vec<Event>) {
    let mut events = Vec::new();
    loop {
        match stream.next(WAIT) {
            Next::Notice(Notice::QueueChanged(changed)) => return (changed, events),
            Next::Notice(Notice::DaemonEvent(event)) => events.push(*event),
            Next::Notice(other) => panic!("{other:?}"),
            Next::Quiet | Next::Closed => panic!("no queue.changed came"),
        }
    }
}

fn is_permission(event: &Event, answered: bool) -> bool {
    match &event.body {
        EventBody::PermissionRequested { .. } => !answered,
        EventBody::PermissionAnswered { .. } => answered,
        _ => false,
    }
}

/// Reads notices until the `permission.answered` event.
fn until_permission_answered(stream: &mut Stream) {
    loop {
        match stream.next(WAIT) {
            Next::Notice(Notice::DaemonEvent(event)) if is_permission(&event, true) => return,
            Next::Notice(_) => {}
            Next::Quiet | Next::Closed => panic!("no permission.answered event came"),
        }
    }
}

#[test]
fn a_permission_request_and_its_answer_send_queue_changed_to_a_subscriber() {
    let mut rig = Rig::new(
        &recording("permission-request"),
        &recording("report"),
        Settings::default(),
    );
    let mut watching = watcher(&rig);
    let run = rig.started(None);
    let item = permission_item(&mut rig, &run);

    let (asked, events) = until_queue_changed(&mut watching);
    assert_eq!(
        asked,
        QueueChanged {
            count: 1,
            added: vec![item.id.clone()],
            removed: vec![]
        }
    );
    assert!(
        events.iter().any(
            |e| matches!(&e.body, EventBody::RunTransition { run: named, .. } if *named == run)
        ),
        "the run's own events came as daemon.event: {events:?}"
    );

    rig.answer(
        &item,
        QueueAnswer::Permission {
            reply: PermissionReply::Allow {},
        },
    );
    let (answered, events) = until_queue_changed(&mut watching);
    assert_eq!(
        answered,
        QueueChanged {
            count: 0,
            added: vec![],
            removed: vec![item.id.clone()]
        }
    );
    if !events.iter().any(|e| is_permission(e, true)) {
        until_permission_answered(&mut watching);
    }
}

#[test]
fn queue_changed_goes_to_each_subscriber_and_to_no_other_connection() {
    let mut rig = Rig::new(
        &recording("permission-request"),
        &recording("report"),
        Settings::default(),
    );
    let mut first = watcher(&rig);
    let mut second = watcher(&rig);
    let mut idle = Stream::connect(&DaemonPaths::under(rig.root.path()));
    let run = rig.started(None);
    let item = permission_item(&mut rig, &run);
    for stream in [&mut first, &mut second] {
        let (changed, _) = until_queue_changed(stream);
        assert_eq!(changed.added, vec![item.id.clone()]);
    }
    assert!(matches!(idle.next(Duration::from_millis(300)), Next::Quiet));
}

fn run_get(stream: &mut Stream, run: &str) -> RunGetResult {
    stream.call("run.get", json!({"run": run}))
}

#[test]
fn run_get_and_run_list_give_a_file_runs_phase_and_end_and_the_same_after_a_restart() {
    let mut rig = Rig::new(
        &recording("report"),
        &recording("report"),
        Settings::default(),
    );
    let mut stream = Stream::connect(&DaemonPaths::under(rig.root.path()));
    let run = rig.started(None);
    assert_eq!(rig.wait_final(&run), PhaseState::Ended {});
    wait_until("the run's end is recorded", WAIT, || {
        matches!(
            run_get(&mut stream, &run).summary.state,
            RunState::Ended { .. }
        )
    });

    let got = run_get(&mut stream, &run);
    let summary = &got.summary;
    assert_eq!(summary.id, run);
    assert!(summary.tasks.is_empty());
    assert_eq!(summary.task_file.as_deref(), rig.task.to_str());
    assert!(
        summary
            .ended_ms
            .is_some_and(|end| end >= summary.started_ms)
    );
    assert!(
        summary
            .workspace
            .as_deref()
            .is_some_and(|w| w.ends_with(&run))
    );
    assert_eq!(summary.phases.len(), 1);
    let phase = &summary.phases[0];
    assert_eq!(
        (phase.name.as_str(), phase.kind, phase.round),
        (PHASE_NAME, PhaseKindWire::Agent, 1)
    );
    assert_eq!(phase.instances.len(), 1);
    assert_eq!(phase.instances[0].state, PhaseState::Ended {});
    assert!(phase.started_ms.is_some() && phase.ended_ms >= phase.started_ms);
    assert_eq!(got.rounds.get(PHASE_NAME), Some(&1));

    let listed: RunListResult = stream.call("run.list", json!({"scope": "finished"}));
    assert_eq!(listed.runs, vec![got.summary.clone()]);
    let live: RunListResult = stream.call("run.list", json!({"scope": "live"}));
    assert!(live.runs.is_empty());

    rig.restart();
    let mut stream = Stream::connect(&DaemonPaths::under(rig.root.path()));
    assert_eq!(run_get(&mut stream, &run), got);
    let listed: RunListResult = stream.call("run.list", json!({}));
    assert_eq!(listed.runs, vec![got.summary]);
}

#[test]
fn a_run_waiting_on_a_permission_request_is_live_and_finished_once_it_ends() {
    let mut rig = Rig::new(
        &recording("permission-request"),
        &recording("report"),
        Settings::default(),
    );
    let mut stream = Stream::connect(&DaemonPaths::under(rig.root.path()));
    let run = rig.started(None);
    let item = permission_item(&mut rig, &run);
    let live: RunListResult = stream.call("run.list", json!({"scope": "live"}));
    assert_eq!(live.runs.len(), 1);
    assert_eq!(live.runs[0].id, run);
    assert_eq!(live.runs[0].state, RunState::Running {});
    assert_eq!(
        live.runs[0].phases[0].instances[0].state,
        PhaseState::Running {}
    );

    rig.answer(
        &item,
        QueueAnswer::Permission {
            reply: PermissionReply::Allow {},
        },
    );
    assert_eq!(rig.wait_final(&run), PhaseState::Ended {});
    wait_until("the run is finished", WAIT, || {
        let finished: RunListResult = stream.call("run.list", json!({"scope": "finished"}));
        finished.runs.len() == 1
    });
    let live: RunListResult = stream.call("run.list", json!({"scope": "live"}));
    assert!(live.runs.is_empty());
}
