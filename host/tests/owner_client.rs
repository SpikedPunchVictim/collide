//! What `OwnerClient` does beyond one call at a time (ts-0077-ws): says when the daemon has closed the connection, and
//! reads the notices of `daemon.subscribe` that the daemon sends unasked. The shell uses both. The daemon runs in this
//! process over a root of its own below `/tmp`: no test starts a daemon process, reads the owner's files or loads a
//! launchd job.

mod common;

use collide_core::change::{Actor, ActorKind};
use collide_core::events::{EventBody, StepKind};
use collide_core::owner::{DaemonSubscribeParams, DaemonSubscribeResult};
use collide_host::base_env::CaptureOptions;
use collide_host::daemon::{Daemon, DaemonOptions, DaemonPaths};
use collide_host::owner_client::{Incoming, OwnerClient};
use common::TempRoot;
use std::collections::BTreeMap;
use std::time::Duration;

const WAIT: Duration = Duration::from_secs(10);
const QUIET: Duration = Duration::from_millis(150);

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
    options.home = Some(root.path().display().to_string());
    options.capture = CaptureOptions {
        timeout: Duration::from_secs(10),
        cap_bytes: 1024 * 1024,
        fallback: BTreeMap::from([("PATH".to_string(), "/usr/bin:/bin".to_string())]),
    };
    Daemon::start(options).expect("the daemon starts")
}

fn append(daemon: &Daemon, count: usize) {
    for n in 0..count {
        daemon
            .phases()
            .append_event(
                Actor {
                    kind: ActorKind::Workflow,
                    run: None,
                    phase: None,
                },
                EventBody::StepIntent {
                    step: format!("step-{n}"),
                    kind: StepKind::Integrate,
                    action: "x".to_string(),
                },
            )
            .expect("the event is stored");
    }
}

fn subscribe(client: &mut OwnerClient) -> DaemonSubscribeResult {
    client
        .call(
            "daemon.subscribe",
            &DaemonSubscribeParams { since: None },
            Some(WAIT),
        )
        .expect("subscribed")
}

fn seq_of(incoming: Incoming) -> u64 {
    match incoming {
        Incoming::Notice(notice) => {
            assert_eq!(notice.method, "daemon.event");
            notice.params["seq"].as_u64().expect("an event has a seq")
        }
        other => panic!("expected a notice, got {other:?}"),
    }
}

#[test]
fn a_connection_the_daemon_has_not_closed_is_not_closed() {
    let root = TempRoot::new();
    let daemon = start(&root);
    let mut client = OwnerClient::connect(daemon.paths(), "test").unwrap();
    assert!(!client.is_closed());
}

#[test]
fn a_connection_the_daemon_closed_is_closed() {
    let root = TempRoot::new();
    let daemon = start(&root);
    let mut client = OwnerClient::connect(daemon.paths(), "test").unwrap();
    daemon.stop();
    common::wait_until("the client sees the end", WAIT, || client.is_closed());
}

#[test]
fn a_client_that_subscribed_reads_the_notices_in_order() {
    let root = TempRoot::new();
    let daemon = start(&root);
    let mut client = OwnerClient::connect(daemon.paths(), "test").unwrap();
    subscribe(&mut client);
    client.set_read_wait(Some(WAIT)).unwrap();
    append(&daemon, 3);
    let seqs: Vec<u64> = (0..3)
        .map(|_| seq_of(client.next_notice().unwrap()))
        .collect();
    assert_eq!(seqs, vec![1, 2, 3]);
}

#[test]
fn a_read_that_finds_nothing_is_quiet_and_the_next_read_still_gets_the_notice() {
    let root = TempRoot::new();
    let daemon = start(&root);
    let mut client = OwnerClient::connect(daemon.paths(), "test").unwrap();
    subscribe(&mut client);
    client.set_read_wait(Some(QUIET)).unwrap();
    assert!(matches!(client.next_notice().unwrap(), Incoming::Quiet));
    append(&daemon, 1);
    client.set_read_wait(Some(WAIT)).unwrap();
    assert_eq!(seq_of(client.next_notice().unwrap()), 1);
}

#[test]
fn the_end_of_the_stream_is_closed_after_the_notices_that_came_first() {
    let root = TempRoot::new();
    let daemon = start(&root);
    let mut client = OwnerClient::connect(daemon.paths(), "test").unwrap();
    subscribe(&mut client);
    append(&daemon, 2);
    client.set_read_wait(Some(WAIT)).unwrap();
    assert_eq!(seq_of(client.next_notice().unwrap()), 1);
    assert_eq!(seq_of(client.next_notice().unwrap()), 2);
    daemon.stop();
    assert!(matches!(client.next_notice().unwrap(), Incoming::Closed));
}

#[test]
fn a_call_still_works_on_a_connection_that_has_been_read_from() {
    let root = TempRoot::new();
    let daemon = start(&root);
    let mut client = OwnerClient::connect(daemon.paths(), "test").unwrap();
    client.set_read_wait(Some(QUIET)).unwrap();
    assert!(matches!(client.next_notice().unwrap(), Incoming::Quiet));
    let status: serde_json::Value = client
        .call("daemon.status", &serde_json::json!({}), Some(WAIT))
        .unwrap();
    assert!(status["pid"].is_u64());
}
