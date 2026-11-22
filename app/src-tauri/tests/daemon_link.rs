//! The shell's way to the daemon (ts-0077-ws; 0.2, The UI speaks the owner's protocol, Starting the daemon, The base
//! environment): `SocketLink` makes each call over the owner socket with the token it read itself, starts the daemon
//! when
//! the socket does not answer, and reconnects after a drop; `pump_daemon_events` carries the daemon's events and the
//! Queue's notices to the event channel.
//!
//! Every path is below the test's own root under `/tmp`. The daemon runs in this process or is a fake that a test
//! scripts. No test starts a daemon process, reads the owner's files or loads a launchd job; the real starter is not
//! used here.

mod common;

use collide_app::shell::{DaemonAccess, DaemonLink, SocketLink};
use collide_core::owner::code;
use collide_host::daemon::DaemonPaths;
use common::{
    AlreadyThere, Channel, FailingStarter, FakeDaemon, InProcessStarter, PumpRun, SharedLog,
    TempRoot, TestClocks, access, append, request, result_of, start_daemon, wait_until,
};
use serde_json::{Value, json};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

const WAIT: Duration = Duration::from_secs(10);
const FIVE_MINUTES: Duration = Duration::from_secs(5 * 60);
const RETRY: Duration = Duration::from_millis(50);

fn link(root: &TempRoot, starter: impl collide_app::shell::DaemonStarter + 'static) -> SocketLink {
    SocketLink::new(access(root, starter, &TestClocks::default()))
}

fn call(
    link: &mut SocketLink,
    id: i64,
    method: &str,
) -> Result<Value, collide_core::owner::RpcError> {
    result_of(link.call(&request(id, method, Value::Null)))
}

// ---- a call reaches the daemon ----------------------------------------------------------------------------------

#[test]
fn a_call_reaches_a_daemon_and_its_answer_comes_back() {
    let root = TempRoot::new();
    let _daemon = start_daemon(&root, Box::new(std::io::sink()), None);
    let mut link = link(&root, AlreadyThere);
    let status = call(&mut link, 1, "daemon.status").expect("the daemon answers");
    assert_eq!(status["pid"], std::process::id());
}

#[test]
fn an_error_of_the_daemon_comes_back_as_its_own_code_and_message() {
    let root = TempRoot::new();
    let _daemon = start_daemon(&root, Box::new(std::io::sink()), None);
    let mut link = link(&root, AlreadyThere);
    let error = call(&mut link, 1, "nothing.here").expect_err("no such method");
    assert_eq!(error.code, code::METHOD_NOT_FOUND);
    assert!(error.message.contains("nothing.here"), "{}", error.message);
}

#[test]
fn the_answer_has_the_id_of_the_request() {
    let root = TempRoot::new();
    let _daemon = start_daemon(&root, Box::new(std::io::sink()), None);
    let mut link = link(&root, AlreadyThere);
    let response = link.call(&request(41, "daemon.status", Value::Null));
    assert_eq!(
        serde_json::to_value(&response).unwrap()["id"],
        json!(41),
        "{response:?}"
    );
}

#[test]
fn a_token_file_of_the_wrong_mode_is_refused_and_says_why() {
    use std::os::unix::fs::PermissionsExt;
    let root = TempRoot::new();
    let daemon = start_daemon(&root, Box::new(std::io::sink()), None);
    let token_file = daemon.paths().token_file.clone();
    std::fs::set_permissions(&token_file, std::fs::Permissions::from_mode(0o644)).unwrap();
    let token = std::fs::read_to_string(&token_file).unwrap();
    let mut link = link(&root, AlreadyThere);
    let error = call(&mut link, 1, "daemon.status").expect_err("the file is not trusted");
    assert_eq!(error.code, code::UNAVAILABLE);
    assert!(error.message.contains("0644"), "{}", error.message);
    assert!(
        error.message.contains(token_file.to_str().unwrap()),
        "{}",
        error.message
    );
    assert!(
        !error.message.contains(token.trim()),
        "the token is not in it"
    );
}

// ---- the daemon is started on demand ----------------------------------------------------------------------------

#[test]
fn with_no_daemon_the_first_call_starts_one_and_then_succeeds() {
    let root = TempRoot::new();
    let starter = InProcessStarter::new(&root);
    let mut link = link(&root, starter.clone());
    let status = call(&mut link, 1, "daemon.status").expect("started and answered");
    assert_eq!(status["pid"], std::process::id());
    assert_eq!(starter.starts(), 1);
    call(&mut link, 2, "daemon.status").unwrap();
    assert_eq!(
        starter.starts(),
        1,
        "a daemon that answers is not started again"
    );
}

#[test]
fn a_daemon_that_ended_is_started_again_by_the_next_call() {
    let root = TempRoot::new();
    let starter = InProcessStarter::new(&root);
    let mut link = link(&root, starter.clone());
    call(&mut link, 1, "daemon.status").unwrap();
    starter.stop_daemon();
    // The daemon's sessions look at its stop flag every 100 ms, so one that is still open can answer for a moment.
    std::thread::sleep(Duration::from_millis(500));
    call(&mut link, 2, "daemon.status").expect("the daemon is started again and answers");
    assert_eq!(starter.starts(), 2);
}

#[test]
fn a_start_that_fails_says_why_and_the_call_fails() {
    let root = TempRoot::new();
    let mut link = link(&root, FailingStarter::default());
    let error = call(&mut link, 1, "daemon.status").expect_err("no daemon");
    assert_eq!(error.code, code::UNAVAILABLE);
    assert!(
        error.message.contains("no collide-daemon next to the app"),
        "{}",
        error.message
    );
}

#[test]
fn a_home_that_names_no_folder_is_refused_in_words_and_nothing_is_made_or_started() {
    let root = TempRoot::new();
    let home = root.path().join("gone");
    // The root is below the missing home, so any folder the link made would make the home.
    let paths = DaemonPaths::under(&home.join("daemon"));
    let starter = FailingStarter::default();
    let access = DaemonAccess::new(
        paths,
        Box::new(starter.clone()),
        Arc::new(TestClocks::default()),
        common::this_executable(),
    )
    .with_home(Some(home.display().to_string()));
    let mut link = SocketLink::new(Arc::new(access));
    let error = call(&mut link, 1, "daemon.status").expect_err("the home is gone");
    assert_eq!(error.code, code::UNAVAILABLE);
    for part in ["could not be started", "HOME", home.to_str().unwrap()] {
        assert!(
            error.message.contains(part),
            "`{part}` is not in: {}",
            error.message
        );
    }
    assert_eq!(starter.asked(), 0, "the starter was asked");
    assert!(!home.exists(), "the link made HOME");
}

#[test]
fn after_three_failed_starts_in_five_minutes_the_link_stops_trying_and_says_so() {
    let root = TempRoot::new();
    let starter = FailingStarter::default();
    let mut link = link(&root, starter.clone());
    for id in 1..=3 {
        call(&mut link, id, "daemon.status").expect_err("no daemon");
    }
    assert_eq!(starter.asked(), 3);
    let error = call(&mut link, 4, "daemon.status").expect_err("it has stopped trying");
    assert_eq!(starter.asked(), 3, "no fourth start");
    assert_eq!(error.code, code::UNAVAILABLE);
    for part in [
        "stopped trying",
        "3 times in 5 minutes",
        "no collide-daemon next to the app",
        root.paths().log_file.to_str().unwrap(),
    ] {
        assert!(
            error.message.contains(part),
            "`{part}` is not in: {}",
            error.message
        );
    }
}

#[test]
fn the_link_tries_again_once_five_minutes_have_passed() {
    let root = TempRoot::new();
    let starter = FailingStarter::default();
    let clocks = TestClocks::default();
    let mut link = SocketLink::new(access(&root, starter.clone(), &clocks));
    for id in 1..=3 {
        call(&mut link, id, "daemon.status").expect_err("no daemon");
    }
    clocks.advance(FIVE_MINUTES - Duration::from_secs(1));
    call(&mut link, 4, "daemon.status").expect_err("still stopped");
    assert_eq!(starter.asked(), 3);
    clocks.advance(Duration::from_secs(2));
    call(&mut link, 5, "daemon.status").expect_err("no daemon, but it tried");
    assert_eq!(starter.asked(), 4);
}

#[test]
fn failed_starts_more_than_five_minutes_apart_do_not_add_up() {
    let root = TempRoot::new();
    let starter = FailingStarter::default();
    let clocks = TestClocks::default();
    let mut link = SocketLink::new(access(&root, starter.clone(), &clocks));
    for id in 1..=2 {
        call(&mut link, id, "daemon.status").expect_err("no daemon");
        clocks.advance(Duration::from_secs(150));
    }
    // The first failure is now 300 s old.
    call(&mut link, 3, "daemon.status").expect_err("no daemon");
    call(&mut link, 4, "daemon.status").expect_err("no daemon");
    assert_eq!(starter.asked(), 4, "two in the window is not three");
}

#[test]
fn a_daemon_that_is_there_is_used_even_after_the_link_stopped_trying_to_start_one() {
    let root = TempRoot::new();
    let starter = FailingStarter::default();
    let mut link = link(&root, starter.clone());
    for id in 1..=3 {
        call(&mut link, id, "daemon.status").expect_err("no daemon");
    }
    let _daemon = start_daemon(&root, Box::new(std::io::sink()), None);
    call(&mut link, 4, "daemon.status").expect("a daemon someone else started answers");
    assert_eq!(starter.asked(), 3);
}

// ---- the daemon takes the app as the app ------------------------------------------------------------------------

#[test]
fn the_daemon_takes_a_connection_from_the_app_as_the_app() {
    let root = TempRoot::new();
    let log = SharedLog::default();
    let _daemon = start_daemon(&root, Box::new(log.clone()), None);
    let mut link = link(&root, AlreadyThere);
    call(&mut link, 1, "daemon.status").unwrap();
    wait_until("the connection is logged", WAIT, || {
        log.text().contains("connected as")
    });
    let text = log.text();
    assert_eq!(text.matches("connected as App").count(), 1, "{text}");
    assert_eq!(text.matches("connected as Cli").count(), 0, "{text}");
}

#[test]
fn a_daemon_that_was_started_before_the_app_still_knows_it() {
    // The CLI started the daemon first, so the daemon was told nothing about the app. The app writes its file when it
    // connects, and the daemon reads it when it checks the peer.
    let root = TempRoot::new();
    let starter = InProcessStarter::new(&root);
    let log = SharedLog::default();
    let _daemon = start_daemon(&root, Box::new(log.clone()), None);
    assert!(!root.paths().app_executable_file().exists());
    let mut link = link(&root, starter);
    call(&mut link, 1, "daemon.status").unwrap();
    assert!(root.paths().app_executable_file().exists());
    wait_until("the connection is logged", WAIT, || {
        log.text().contains("connected as App")
    });
}

#[test]
fn the_same_process_is_the_cli_when_it_has_not_written_the_app_file() {
    let root = TempRoot::new();
    let log = SharedLog::default();
    let daemon = start_daemon(&root, Box::new(log.clone()), None);
    collide_host::owner_client::OwnerClient::connect(daemon.paths(), "a terminal").unwrap();
    wait_until("the connection is logged", WAIT, || {
        log.text().contains("connected as")
    });
    assert!(log.text().contains("connected as Cli"), "{}", log.text());
}

// ---- a connection that drops ------------------------------------------------------------------------------------

/// A fake daemon whose first connection reads one request and closes without answering, and whose later connections
/// answer every request with `{"ok": true}`.
fn drops_the_first_call(root: &TempRoot) -> FakeDaemon {
    FakeDaemon::start(root, |conn| {
        conn.initialize();
        if conn.index == 0 {
            conn.read();
            return;
        }
        while let Some(request) = conn.read() {
            conn.answer(&request, json!({"ok": true}));
        }
    })
}

#[test]
fn a_call_whose_request_was_sent_is_not_sent_again_when_the_connection_drops() {
    let root = TempRoot::new();
    let fake = drops_the_first_call(&root);
    let mut link = link(&root, AlreadyThere);
    let error = call(&mut link, 1, "run.stop").expect_err("the connection dropped");
    assert_eq!(error.code, code::UNAVAILABLE);
    assert!(error.message.contains("dropped"), "{}", error.message);
    assert!(error.message.contains("run.stop"), "{}", error.message);
    wait_until("the fake daemon read the request", WAIT, || {
        !fake.requests("run.stop").is_empty()
    });
    assert_eq!(
        fake.requests("run.stop").len(),
        1,
        "the request was sent once: {:?}",
        fake.methods()
    );
}

#[test]
fn the_next_call_after_a_drop_makes_the_connection_again() {
    let root = TempRoot::new();
    let fake = drops_the_first_call(&root);
    let mut link = link(&root, AlreadyThere);
    call(&mut link, 1, "run.stop").expect_err("the connection dropped");
    let result = call(&mut link, 2, "run.stop").expect("a new connection answers");
    assert_eq!(result, json!({"ok": true}));
    let seen = fake.requests("run.stop");
    assert_eq!(
        seen.iter().map(|s| s.connection).collect::<Vec<_>>(),
        vec![0, 1],
        "one request on each connection"
    );
}

#[test]
fn a_connection_the_daemon_closed_between_calls_is_made_again_before_the_next_call_is_sent() {
    let root = TempRoot::new();
    let fake = FakeDaemon::start(&root, |conn| {
        conn.initialize();
        let first = conn.read().expect("a call");
        conn.answer(&first, json!({"on": conn.index}));
        if conn.index == 0 {
            return;
        }
        while let Some(request) = conn.read() {
            conn.answer(&request, json!({"on": conn.index}));
        }
    });
    let mut link = link(&root, AlreadyThere);
    assert_eq!(
        call(&mut link, 1, "daemon.status").unwrap(),
        json!({"on": 0})
    );
    wait_until("the daemon closed the connection", WAIT, || {
        fake.finished.load(Ordering::SeqCst) == 1
    });
    assert_eq!(
        call(&mut link, 2, "daemon.status").expect("made again, not failed"),
        json!({"on": 1})
    );
    let calls = fake.requests("daemon.status");
    assert_eq!(
        calls.len(),
        2,
        "each call was sent once: {:?}",
        fake.methods()
    );
}

#[test]
fn a_daemon_that_does_not_answer_fails_the_call_after_the_wait_and_the_next_call_goes_on() {
    let root = TempRoot::new();
    let fake = FakeDaemon::start(&root, |conn| {
        conn.initialize();
        if conn.index == 0 {
            conn.read();
            std::thread::sleep(Duration::from_millis(1500));
            return;
        }
        while let Some(request) = conn.read() {
            conn.answer(&request, json!({"ok": true}));
        }
    });
    let mut link = link(&root, AlreadyThere).with_call_wait(Duration::from_millis(300));
    let begun = Instant::now();
    let error = call(&mut link, 1, "run.stop").expect_err("no answer");
    assert!(
        begun.elapsed() < Duration::from_secs(5),
        "{:?}",
        begun.elapsed()
    );
    assert_eq!(error.code, code::UNAVAILABLE);
    assert!(
        error.message.contains("did not answer"),
        "{}",
        error.message
    );
    call(&mut link, 2, "daemon.status").expect("the next call connects again");
    assert_eq!(fake.requests("run.stop").len(), 1);
}

// ---- the event pump ---------------------------------------------------------------------------------------------

#[test]
fn the_pump_asks_for_the_environment_once_and_subscribes_again_from_the_last_seq() {
    let root = TempRoot::new();
    let fake = FakeDaemon::start(&root, |conn| {
        conn.initialize();
        if conn.index == 0 {
            conn.answer_recapture();
            conn.answer_subscribe(4);
            for seq in 5..=7 {
                conn.event(seq);
            }
            return;
        }
        conn.answer_subscribe(9);
        // 7 was forwarded already, and a replay that repeats it is not forwarded twice.
        for seq in 7..=9 {
            conn.event(seq);
        }
        while conn.read().is_some() {}
    });
    let channel = Channel::default();
    let pump = PumpRun::start(
        access(&root, AlreadyThere, &TestClocks::default()),
        channel.clone(),
        RETRY,
    );
    wait_until("the events come", WAIT, || channel.seqs().len() >= 5);
    pump.stop();
    assert_eq!(channel.seqs(), vec![5, 6, 7, 8, 9]);
    assert_eq!(
        fake.methods(),
        vec![
            (0, "initialize".to_string()),
            (0, "daemon.recapture_environment".to_string()),
            (0, "daemon.subscribe".to_string()),
            (1, "initialize".to_string()),
            (1, "daemon.subscribe".to_string()),
        ]
    );
    let subscribes = fake.requests("daemon.subscribe");
    assert_eq!(
        subscribes[0].params,
        json!({}),
        "the first one asks for live events only"
    );
    assert_eq!(subscribes[1].params, json!({"since": 7}));
}

#[test]
fn a_subscription_that_forwarded_nothing_subscribes_again_from_the_head_it_was_given() {
    let root = TempRoot::new();
    let fake = FakeDaemon::start(&root, |conn| {
        conn.initialize();
        if conn.index == 0 {
            conn.answer_recapture();
            conn.answer_subscribe(4);
            return;
        }
        conn.answer_subscribe(4);
        while conn.read().is_some() {}
    });
    let channel = Channel::default();
    let pump = PumpRun::start(
        access(&root, AlreadyThere, &TestClocks::default()),
        channel.clone(),
        RETRY,
    );
    wait_until("the second subscription", WAIT, || {
        fake.requests("daemon.subscribe").len() == 2
    });
    pump.stop();
    assert_eq!(
        fake.requests("daemon.subscribe")[1].params,
        json!({"since": 4})
    );
}

#[test]
fn a_notice_that_is_not_an_event_is_forwarded_as_it_came_and_a_notice_cut_in_two_is_read_whole() {
    let root = TempRoot::new();
    let queue_changed = json!({"count": 1, "added": ["q-1"], "removed": []});
    let sent = queue_changed.clone();
    let _fake = FakeDaemon::start(&root, move |conn| {
        conn.initialize();
        conn.answer_recapture();
        conn.answer_subscribe(0);
        conn.send(json!({"jsonrpc": "2.0", "method": "queue.changed", "params": sent}));
        let event = common::event_notice(1).to_string();
        let (first, rest) = event.split_at(20);
        conn.send_raw(first);
        // Longer than the pump's wait for one read, so the read that finds half a line ends first.
        std::thread::sleep(Duration::from_millis(700));
        conn.send_raw(&format!("{rest}\n"));
        while conn.read().is_some() {}
    });
    let channel = Channel::default();
    let pump = PumpRun::start(
        access(&root, AlreadyThere, &TestClocks::default()),
        channel.clone(),
        RETRY,
    );
    wait_until("both notices", WAIT, || channel.events().len() >= 2);
    pump.stop();
    let events = channel.events();
    assert_eq!(events[0].method, "queue.changed");
    assert_eq!(events[0].params, queue_changed);
    assert_eq!(
        (events[1].method.as_str(), channel.seqs()),
        ("daemon.event", vec![1])
    );
}

#[test]
fn the_pump_stops_when_told_to_even_while_the_daemon_is_quiet() {
    let root = TempRoot::new();
    let _fake = FakeDaemon::start(&root, |conn| {
        conn.initialize();
        conn.answer_recapture();
        conn.answer_subscribe(0);
        while conn.read().is_some() {}
    });
    let channel = Channel::default();
    let pump = PumpRun::start(
        access(&root, AlreadyThere, &TestClocks::default()),
        channel,
        RETRY,
    );
    std::thread::sleep(Duration::from_millis(300));
    let begun = Instant::now();
    pump.stop();
    assert!(
        begun.elapsed() < Duration::from_secs(3),
        "{:?}",
        begun.elapsed()
    );
}

#[test]
fn a_pump_that_finds_no_daemon_waits_and_connects_when_one_comes() {
    let root = TempRoot::new();
    let starter = FailingStarter::default();
    let channel = Channel::default();
    let pump = PumpRun::start(
        access(&root, starter.clone(), &TestClocks::default()),
        channel.clone(),
        RETRY,
    );
    wait_until("the pump has tried", WAIT, || starter.asked() >= 1);
    let daemon = start_daemon(&root, Box::new(std::io::sink()), None);
    // The pump's next try finds the daemon. Events appended until one gets through show it subscribed.
    wait_until("an event gets through", WAIT, || {
        append(&daemon, 1);
        !channel.seqs().is_empty()
    });
    pump.stop();
    assert!(
        starter.asked() <= 3,
        "it stopped asking after three: {}",
        starter.asked()
    );
}

#[test]
fn the_pump_forwards_a_real_daemons_events_in_seq_order_and_after_a_restart_none_are_missing() {
    let root = TempRoot::new();
    let daemon = start_daemon(&root, Box::new(std::io::sink()), None);
    let channel = Channel::default();
    // A starter that never starts a daemon: the test restarts it, as a crash and a new start would.
    let pump = PumpRun::start(
        access(&root, FailingStarter::default(), &TestClocks::default()),
        channel.clone(),
        Duration::from_millis(300),
    );
    wait_until("the pump is subscribed", WAIT, || {
        append(&daemon, 1);
        !channel.seqs().is_empty()
    });
    append(&daemon, 5);
    let before = daemon.store().last_seq().unwrap();
    wait_until("the events before the restart", WAIT, || {
        channel.seqs().last() == Some(&before)
    });

    drop(daemon);
    let daemon = start_daemon(&root, Box::new(std::io::sink()), None);
    // These are appended while the pump may not have subscribed again: they are in the log, and it replays them.
    append(&daemon, 5);
    let after = daemon.store().last_seq().unwrap();
    wait_until("the events after the restart", WAIT, || {
        channel.seqs().last() == Some(&after)
    });
    pump.stop();

    let seqs = channel.seqs();
    let first = seqs[0];
    assert_eq!(
        seqs,
        (first..=after).collect::<Vec<u64>>(),
        "in order, none missing, none twice"
    );
}

#[test]
fn the_app_asks_a_real_daemon_for_the_environment_when_it_starts_and_only_then() {
    let root = TempRoot::new();
    let runs = root.path().join("shell-runs");
    let daemon = start_daemon(&root, Box::new(std::io::sink()), Some(&runs));
    let count = || std::fs::read_to_string(&runs).map_or(0, |text| text.lines().count());
    wait_until("the daemon's own capture", WAIT, || count() == 1);
    let channel = Channel::default();
    let pump = PumpRun::start(
        access(&root, AlreadyThere, &TestClocks::default()),
        channel.clone(),
        RETRY,
    );
    wait_until("the app's capture", WAIT, || count() == 2);
    // The stream ends and the pump subscribes again: that is not the app starting.
    append(&daemon, 1);
    drop(daemon);
    std::thread::sleep(Duration::from_millis(500));
    pump.stop();
    assert_eq!(count(), 2);
}

// ---- the window is told when the stream ends and when it comes back --------------------------------------------

#[test]
fn a_stream_that_ends_is_told_to_the_window_and_so_is_its_return_before_the_replay() {
    let root = TempRoot::new();
    let daemon = start_daemon(&root, Box::new(std::io::sink()), None);
    let channel = Channel::default();
    let pump = PumpRun::start(
        access(&root, FailingStarter::default(), &TestClocks::default()),
        channel.clone(),
        Duration::from_millis(300),
    );
    wait_until("the pump is subscribed", WAIT, || {
        append(&daemon, 1);
        !channel.seqs().is_empty()
    });
    // The daemon may not have answered the pump's very first try, which is told as a loss and a return. That is the
    // pump's, and what follows is the restart's.
    let told_before = channel.connection_states().len();

    drop(daemon);
    wait_until("the end of the stream", WAIT, || {
        channel.connection_states().len() == told_before + 1
    });
    let daemon = start_daemon(&root, Box::new(std::io::sink()), None);
    // Appended while the pump may not have subscribed again: the replay carries them after the notice of the return.
    append(&daemon, 3);
    let after = daemon.store().last_seq().unwrap();
    wait_until("the events after the restart", WAIT, || {
        channel.seqs().last() == Some(&after)
    });
    pump.stop();

    assert_eq!(channel.connection_states()[told_before..], ["lost", "back"]);
    let events = channel.events();
    let back = events
        .iter()
        .rposition(|event| {
            event.method == collide_app::shell::CONNECTION_EVENT && event.params["state"] == "back"
        })
        .unwrap();
    let first_replayed = events
        .iter()
        .rposition(|event| event.method == "daemon.event" && event.params["seq"] == after - 2)
        .unwrap();
    assert!(
        back < first_replayed,
        "the window hears of the return, then reloads, then the replay follows: {back} {first_replayed}"
    );
}

#[test]
fn a_notice_of_the_connection_has_the_state_and_nothing_else() {
    let root = TempRoot::new();
    let daemon = start_daemon(&root, Box::new(std::io::sink()), None);
    let channel = Channel::default();
    let pump = PumpRun::start(
        access(&root, FailingStarter::default(), &TestClocks::default()),
        channel.clone(),
        RETRY,
    );
    wait_until("the pump is subscribed", WAIT, || {
        append(&daemon, 1);
        !channel.seqs().is_empty()
    });
    drop(daemon);
    wait_until("the end of the stream", WAIT, || {
        !channel.connection_states().is_empty()
    });
    pump.stop();
    let notice = channel
        .events()
        .into_iter()
        .find(|event| event.method == collide_app::shell::CONNECTION_EVENT)
        .unwrap();
    assert_eq!(notice.params, json!({"state": "lost"}));
}

#[test]
fn trouble_that_goes_on_is_told_once_and_a_pump_that_is_stopped_says_nothing() {
    let root = TempRoot::new();
    let starter = FailingStarter::default();
    let channel = Channel::default();
    let pump = PumpRun::start(
        access(&root, starter.clone(), &TestClocks::default()),
        channel.clone(),
        RETRY,
    );
    wait_until("the pump has tried", WAIT, || starter.asked() >= 1);
    wait_until("the trouble is told", WAIT, || {
        channel.connection_states() == ["lost"]
    });
    std::thread::sleep(Duration::from_millis(300));
    pump.stop();
    assert_eq!(channel.connection_states(), ["lost"]);
}

#[test]
fn a_pump_that_found_no_daemon_tells_the_window_when_the_daemon_comes() {
    let root = TempRoot::new();
    let starter = FailingStarter::default();
    let channel = Channel::default();
    let pump = PumpRun::start(
        access(&root, starter.clone(), &TestClocks::default()),
        channel.clone(),
        RETRY,
    );
    wait_until("the pump has tried", WAIT, || starter.asked() >= 1);
    let daemon = start_daemon(&root, Box::new(std::io::sink()), None);
    wait_until("the return is told", WAIT, || {
        channel.connection_states() == ["lost", "back"]
    });
    pump.stop();
    drop(daemon);
}

#[test]
fn a_pump_that_is_stopped_while_the_stream_is_up_says_nothing() {
    let root = TempRoot::new();
    let _fake = FakeDaemon::start(&root, |conn| {
        conn.initialize();
        conn.answer_recapture();
        conn.answer_subscribe(0);
        while conn.read().is_some() {}
    });
    let channel = Channel::default();
    let pump = PumpRun::start(
        access(&root, AlreadyThere, &TestClocks::default()),
        channel.clone(),
        RETRY,
    );
    std::thread::sleep(Duration::from_millis(300));
    pump.stop();
    assert_eq!(channel.connection_states(), Vec::<String>::new());
}

#[test]
fn a_first_connection_that_works_says_nothing_and_the_return_after_it_says_back() {
    let root = TempRoot::new();
    let _fake = FakeDaemon::start(&root, |conn| {
        conn.initialize();
        if conn.index == 0 {
            conn.answer_recapture();
            conn.answer_subscribe(4);
            conn.event(5);
            return;
        }
        conn.answer_subscribe(6);
        conn.event(6);
        while conn.read().is_some() {}
    });
    let channel = Channel::default();
    let pump = PumpRun::start(
        access(&root, AlreadyThere, &TestClocks::default()),
        channel.clone(),
        RETRY,
    );
    wait_until("both streams", WAIT, || channel.seqs() == [5, 6]);
    pump.stop();
    let told: Vec<String> = channel
        .events()
        .iter()
        .map(|event| match event.method.as_str() {
            "daemon.event" => format!("event {}", event.params["seq"]),
            _ => event.params["state"].as_str().unwrap().to_string(),
        })
        .collect();
    assert_eq!(told, ["event 5", "lost", "back", "event 6"]);
}
