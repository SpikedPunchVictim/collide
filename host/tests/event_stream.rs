//! The daemon's event stream (ts-0076-cw; 0.2, The wire): `daemon.subscribe` and the `daemon.event` notices, over the
//! daemon's real socket inside the test process. The events are written through the daemon's own
//! `Phases::append_event`, the one door every event of the log goes through. Every path is below the test's own root
//! under `/tmp`; no test reads the owner's files, starts a daemon process or loads a launchd job.

mod common;

use collide_core::change::{Actor, ActorKind};
use collide_core::events::{EventBody, StepKind};
use collide_core::owner::{DaemonStatusResult, Notice};
use collide_host::base_env::CaptureOptions;
use collide_host::daemon::{Daemon, DaemonOptions, DaemonPaths};
use collide_host::phases::Phases;
use common::TempRoot;
use common::subscriber::{Next, Stream};
use serde_json::json;
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

const WAIT: Duration = Duration::from_secs(10);
const QUIET: Duration = Duration::from_millis(300);

fn start(root: &TempRoot, queue: Option<usize>) -> Daemon {
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
    if let Some(queue) = queue {
        options.subscriber_queue = queue;
    }
    Daemon::start(options).expect("the daemon starts")
}

/// Appends `count` events whose text is `size` bytes of ASCII, and returns the longest one append took.
fn append(phases: &Phases, count: usize, size: usize) -> Duration {
    let mut longest = Duration::ZERO;
    for n in 0..count {
        let begun = Instant::now();
        phases
            .append_event(
                Actor {
                    kind: ActorKind::Workflow,
                    run: None,
                    phase: None,
                },
                EventBody::StepIntent {
                    step: format!("step-{n}"),
                    kind: StepKind::Integrate,
                    action: "x".repeat(size),
                },
            )
            .expect("the event is stored");
        longest = longest.max(begun.elapsed());
    }
    longest
}

fn head(daemon: &Daemon) -> u64 {
    daemon.store().last_seq().unwrap()
}

fn range(first: u64, last: u64) -> Vec<u64> {
    (first..=last).collect()
}

#[test]
fn a_subscriber_that_gives_since_gets_each_later_event_once_in_order() {
    let root = TempRoot::new();
    let daemon = start(&root, None);
    append(&daemon.phases(), 5, 10);
    let mut client = Stream::connect(daemon.paths());
    let result = client.subscribe(Some(2));
    assert_eq!(
        (result.head_seq, result.oldest_seq, result.replayed),
        (5, 1, 3)
    );
    assert_eq!(client.event_seqs(3, WAIT), range(3, 5));
    assert!(
        matches!(client.next(QUIET), Next::Quiet),
        "nothing comes twice"
    );
}

#[test]
fn the_answer_comes_before_the_replay() {
    let root = TempRoot::new();
    let daemon = start(&root, None);
    append(&daemon.phases(), 3, 10);
    let mut client = Stream::connect(daemon.paths());
    // `call` keeps a notice that arrives before the answer, so a notice that came first is told apart by its place.
    let answer = client
        .call_raw("daemon.subscribe", json!({"since": 0}))
        .expect("answered");
    assert_eq!(answer["replayed"], 3);
    assert_eq!(client.event_seqs(3, WAIT), range(1, 3));
}

#[test]
fn events_appended_while_the_replay_runs_come_once_and_in_order_with_none_missing() {
    let root = TempRoot::new();
    let daemon = start(&root, None);
    let phases = daemon.phases();
    append(&phases, 300, 1000);
    let appending = {
        let phases = phases.clone();
        std::thread::spawn(move || append(&phases, 300, 1000))
    };
    let mut client = Stream::connect(daemon.paths());
    client.subscribe(Some(0));
    appending.join().unwrap();
    let last = head(&daemon);
    assert_eq!(last, 600);
    assert_eq!(client.event_seqs(600, WAIT), range(1, 600));
    assert!(matches!(client.next(QUIET), Next::Quiet), "no event twice");
}

#[test]
fn with_no_since_only_the_events_after_the_answer_come() {
    let root = TempRoot::new();
    let daemon = start(&root, None);
    append(&daemon.phases(), 3, 10);
    let mut client = Stream::connect(daemon.paths());
    let result = client.subscribe(None);
    assert_eq!((result.head_seq, result.replayed), (3, 0));
    assert!(matches!(client.next(QUIET), Next::Quiet), "no old event");
    append(&daemon.phases(), 2, 10);
    assert_eq!(client.event_seqs(2, WAIT), range(4, 5));
}

#[test]
fn an_empty_log_says_so_in_its_answer() {
    let root = TempRoot::new();
    let daemon = start(&root, None);
    let mut client = Stream::connect(daemon.paths());
    let result = client.subscribe(Some(0));
    assert_eq!(
        (result.head_seq, result.oldest_seq, result.replayed),
        (0, 1, 0)
    );
}

#[test]
fn subscribing_again_replaces_the_first_subscription() {
    let root = TempRoot::new();
    let daemon = start(&root, None);
    append(&daemon.phases(), 2, 10);
    let mut client = Stream::connect(daemon.paths());
    client.subscribe(None);
    client.subscribe(Some(1));
    assert_eq!(client.event_seqs(1, WAIT), vec![2]);
    append(&daemon.phases(), 2, 10);
    assert_eq!(client.event_seqs(2, WAIT), range(3, 4));
    assert!(
        matches!(client.next(QUIET), Next::Quiet),
        "each event comes once, not once for each subscription"
    );
}

#[test]
fn events_go_to_the_connection_that_subscribed_and_to_no_other() {
    let root = TempRoot::new();
    let daemon = start(&root, None);
    let mut watching = Stream::connect(daemon.paths());
    let mut other = Stream::connect(daemon.paths());
    watching.subscribe(None);
    append(&daemon.phases(), 2, 10);
    assert_eq!(watching.event_seqs(2, WAIT), range(1, 2));
    assert!(matches!(other.next(QUIET), Next::Quiet));
}

#[test]
fn a_closed_connection_ends_its_subscription_and_the_daemon_goes_on() {
    let root = TempRoot::new();
    let daemon = start(&root, None);
    let mut gone = Stream::connect(daemon.paths());
    gone.subscribe(None);
    let mut staying = Stream::connect(daemon.paths());
    staying.subscribe(None);
    drop(gone);
    let longest = append(&daemon.phases(), 20, 10);
    assert!(
        longest < Duration::from_secs(2),
        "an append took {longest:?}"
    );
    assert_eq!(staying.event_seqs(20, WAIT), range(1, 20));
    let status: DaemonStatusResult = staying.call("daemon.status", json!({}));
    assert_eq!(status.head_seq, 20);
}

#[test]
fn a_subscriber_that_reads_nothing_blocks_no_one_and_ends_and_can_subscribe_again_from_its_last_seq()
 {
    let root = TempRoot::new();
    let daemon = start(&root, Some(64));
    let mut slow = Stream::connect(daemon.paths());
    slow.subscribe(None);
    let mut quick = Stream::connect(daemon.paths());
    quick.subscribe(None);
    let reading = std::thread::spawn(move || {
        let seqs = quick.event_seqs(300, WAIT);
        (quick, seqs)
    });

    // 300 events of 4 KB are more than the socket holds and more than the queue does, and `slow` reads none.
    let longest = append(&daemon.phases(), 300, 4000);
    assert!(
        longest < Duration::from_secs(2),
        "an append waited for the slow subscriber: {longest:?}"
    );
    let (mut quick, seen) = reading.join().unwrap();
    assert_eq!(seen, range(1, 300), "the other subscriber got every event");
    let status: DaemonStatusResult = quick.call("daemon.status", json!({}));
    assert_eq!(status.head_seq, 300);

    // The slow one reads now: a run of events from 1 with no gap, then the daemon closes the connection.
    let mut got = Vec::new();
    loop {
        match slow.next(WAIT) {
            Next::Notice(Notice::DaemonEvent(event)) => got.push(event.seq),
            Next::Notice(other) => panic!("{other:?}"),
            Next::Closed => break,
            Next::Quiet => panic!("the daemon neither sent nor closed"),
        }
    }
    assert!(!got.is_empty() && got.len() < 300, "got {}", got.len());
    assert_eq!(got, range(1, got.len() as u64), "no gap before the end");

    // It subscribes again from the last seq it has, and gets the rest, once and in order.
    let last = *got.last().unwrap();
    let mut again = Stream::connect(daemon.paths());
    let result = again.subscribe(Some(last));
    assert_eq!(result.replayed, 300 - last);
    assert_eq!(
        again.event_seqs(300 - last as usize, WAIT),
        range(last + 1, 300)
    );
}
