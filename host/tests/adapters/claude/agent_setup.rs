//! The version gate, the smoke check, the published version and the settings canary, through a daemon with fake Claude as
//! `claude` (ts-0068-wx; 0.3, A tested version range, Two version chips and Settings that fail validation are ignored
//! silently under `-p`). Each test has a daemon of its own under a temp root, as in `file_run.rs`; the registry is a
//! loopback server of this folder's own, and nothing here reaches a network or reads the owner's files.

use super::fake::recording;
use super::file_run::{Fixture, Setup, first_problem};
use super::published::{BODY, Reply, Server, source};
use collide_core::owner::{
    AnswerKind, DaemonAgentVersionsParams, DaemonAgentVersionsResult, QueueAnswer,
    QueueAnswerParams, QueueAnswerResult, QueueKind, RunStartResult,
};
use collide_core::state::PhaseState;
use collide_host::adapters::claude::version::CANARY_FAILURE;
use collide_host::claude_versions::PublishedChoice;
use std::sync::Arc;
use std::time::Duration;

fn versions(fx: &mut Fixture) -> collide_core::owner::AgentVersions {
    let mut result: DaemonAgentVersionsResult = fx
        .client
        .call(
            "daemon.agent_versions",
            &DaemonAgentVersionsParams {},
            Some(Duration::from_secs(30)),
        )
        .expect("the call is answered");
    result.agents.remove(0)
}

fn part(fx: &Fixture, run: &str, name: &str) -> serde_json::Value {
    let store = collide_host::store::Store::open(&fx.paths().store_dir).unwrap();
    let text = store
        .fingerprint_part(run, name)
        .unwrap()
        .unwrap_or_else(|| panic!("the part {name} is stored"));
    serde_json::from_str(&text).unwrap()
}

// ---- the version ---------------------------------------------------------------------------------------------------

#[test]
fn a_phase_in_the_tested_range_starts_with_no_smoke_check_and_its_version_is_in_the_fingerprint() {
    let mut fx = Fixture::new(&recording("report"), Setup::default());
    let run = fx.started();
    assert_eq!(fx.wait_final(&run), PhaseState::Ended {});
    assert_eq!(fx.claude.smoke_runs(), 0);
    let fingerprint = part(&fx, &run, "agent_version");
    assert_eq!(fingerprint["agent"], "claude");
    assert_eq!(fingerprint["version"], "2.1.325");
    assert_eq!(fingerprint["in_tested_range"], true);
    assert_eq!(fingerprint["smoke_passed"], serde_json::Value::Null);
}

#[test]
fn the_version_is_read_at_start_and_before_the_phase_and_cached_between() {
    let mut fx = Fixture::new(&recording("report"), Setup::default());
    super::common::wait_until(
        "the version is read at start",
        Duration::from_secs(20),
        || fx.claude.version_reads() >= 1,
    );
    let run = fx.started();
    fx.wait_final(&run);
    versions(&mut fx);
    assert_eq!(fx.claude.version_reads(), 1, "one read for one binary");
}

#[test]
fn a_version_outside_the_range_is_smoke_checked_before_its_phase_and_a_pass_raises_no_queue_item() {
    let smoke = recording("report");
    let mut fx = Fixture::new(
        &recording("report"),
        Setup {
            version: Some("2.2.0".into()),
            smoke: Some(smoke),
            ..Setup::default()
        },
    );
    let run = fx.started();
    assert_eq!(fx.wait_final(&run), PhaseState::Ended {});
    assert_eq!(fx.claude.smoke_runs(), 1);
    let calls = fx.claude.calls();
    let smoke_at = calls
        .iter()
        .position(|call| call.contains("--no-session-persistence"))
        .unwrap();
    let phase_at = calls
        .iter()
        .position(|call| call.contains("--agents"))
        .unwrap();
    assert!(
        smoke_at < phase_at,
        "the smoke check comes first: {calls:?}"
    );
    assert!(fx.queue(&run).is_empty(), "{:?}", fx.queue(&run));
    let fingerprint = part(&fx, &run, "agent_version");
    assert_eq!(fingerprint["version"], "2.2.0");
    assert_eq!(fingerprint["in_tested_range"], false);
    assert_eq!(fingerprint["smoke_passed"], true);
    let chip = versions(&mut fx);
    assert_eq!(chip.in_use.as_deref(), Some("2.2.0"));
    assert_eq!(chip.in_tested_range, Some(false));
    assert!(chip.smoke.expect("the chip is marked").passed);
}

#[test]
fn a_failed_smoke_check_refuses_the_phase_naming_the_version_and_range_and_the_queue_says_so() {
    let mut fx = Fixture::new(
        &recording("report"),
        Setup {
            version: Some("2.2.0".into()),
            smoke: Some(recording("refusal")),
            ..Setup::default()
        },
    );
    let problem = first_problem(fx.start(&fx.start_params()));
    assert!(problem.message.contains("2.2.0"), "{}", problem.message);
    assert!(
        problem.message.contains("2.1.323 to 2.1.325"),
        "{}",
        problem.message
    );
    assert!(
        fx.claude
            .calls()
            .iter()
            .all(|call| !call.contains("--agents")),
        "no phase was started: {:?}",
        fx.claude.calls()
    );
    let items: Vec<_> = fx
        .queue_all()
        .into_iter()
        .filter(|item| item.kind == QueueKind::AgentSetup)
        .collect();
    assert_eq!(items.len(), 1, "{items:?}");
    assert!(items[0].title.contains("2.2.0") && items[0].title.contains("2.1.323 to 2.1.325"));
    assert_eq!(items[0].answers, vec![AnswerKind::Dismiss]);
    let chip = versions(&mut fx);
    assert!(!chip.smoke.expect("the failure is shown").passed);
}

#[test]
fn the_agent_setup_item_is_dismissed_by_the_person_and_not_before() {
    let mut fx = Fixture::new(
        &recording("report"),
        Setup {
            version: Some("2.2.0".into()),
            smoke: Some(recording("refusal")),
            ..Setup::default()
        },
    );
    first_problem(fx.start(&fx.start_params()));
    let item = fx
        .queue_all()
        .into_iter()
        .find(|item| item.kind == QueueKind::AgentSetup)
        .unwrap();
    let answered: QueueAnswerResult = fx
        .client
        .call(
            "queue.answer",
            &QueueAnswerParams {
                item: item.id.clone(),
                answer: QueueAnswer::Dismiss {},
            },
            Some(Duration::from_secs(10)),
        )
        .unwrap();
    assert_eq!(answered, QueueAnswerResult::Done {});
    assert!(fx.queue_all().is_empty());
}

#[test]
fn a_binary_that_prints_no_version_refuses_the_phase_and_the_status_says_why() {
    let mut fx = Fixture::new(
        &recording("report"),
        Setup {
            version: Some("banana".into()),
            ..Setup::default()
        },
    );
    let problem = first_problem(fx.start(&fx.start_params()));
    assert!(
        problem.message.contains("printed no version"),
        "{}",
        problem.message
    );
    let chip = versions(&mut fx);
    assert_eq!(chip.in_use, None);
    assert!(chip.in_use_error.is_some());
}

// ---- the published version -----------------------------------------------------------------------------------------

#[test]
fn the_rpc_gives_both_versions_and_the_published_ones_come_from_the_source_the_daemon_was_given() {
    let server = Server::start(vec![Reply::Body(BODY.into())]);
    let published = PublishedChoice::Source(Arc::new(source(&server.url("/dist-tags"))));
    let mut fx = Fixture::new(
        &recording("report"),
        Setup {
            published: Some(published),
            ..Setup::default()
        },
    );
    super::common::wait_until(
        "the published version is fetched",
        Duration::from_secs(20),
        || {
            versions(&mut fx)
                .published
                .is_some_and(|published| published.latest.is_some())
        },
    );
    let chip = versions(&mut fx);
    assert_eq!(chip.agent, "claude");
    assert_eq!(chip.in_use.as_deref(), Some("2.1.325"));
    assert_eq!(chip.in_tested_range, Some(true));
    assert_eq!(chip.tested.from, "2.1.323");
    let published = chip.published.unwrap();
    assert_eq!(published.latest.as_deref(), Some("2.1.325"));
    assert_eq!(published.stable.as_deref(), Some("2.1.311"));
}

#[test]
fn a_daemon_given_no_source_makes_no_fetch_and_shows_no_published_chip() {
    let mut fx = Fixture::new(&recording("report"), Setup::default());
    let chip = versions(&mut fx);
    assert_eq!(chip.published, None);
    assert_eq!(chip.in_use.as_deref(), Some("2.1.325"));
}

// ---- the canary ----------------------------------------------------------------------------------------------------

#[test]
fn a_phase_whose_settings_claude_ignored_fails_with_the_canary_message_in_the_queue() {
    let mut fx = Fixture::new(
        &recording("report"),
        Setup {
            ignore_settings: true,
            ..Setup::default()
        },
    );
    let run = fx.started();
    assert_eq!(
        fx.wait_final(&run),
        PhaseState::Failed { cause: None },
        "a phase that ran without Collide's settings is failed"
    );
    assert_eq!(
        fx.record(&run).unwrap().failure.as_deref(),
        Some("settings_ignored")
    );
    // The phase is final a moment before the daemon's end hook has raised the item.
    super::common::wait_until("the item is raised", Duration::from_secs(20), || {
        !fx.queue(&run).is_empty()
    });
    let items = fx.queue(&run);
    assert_eq!(items.len(), 1, "{items:?}");
    assert_eq!(items[0].kind, QueueKind::AgentSetup);
    assert_eq!(items[0].title, CANARY_FAILURE);
    assert_eq!(items[0].title, "Claude ignored Collide's settings");
}

#[test]
fn a_phase_whose_canary_appeared_ends_with_no_agent_setup_item() {
    let mut fx = Fixture::new(&recording("report"), Setup::default());
    let run = fx.started();
    assert_eq!(fx.wait_final(&run), PhaseState::Ended {});
    assert!(fx.queue(&run).is_empty());
}

#[test]
fn the_run_start_result_type_is_unchanged_for_a_refusal() {
    let mut fx = Fixture::new(
        &recording("report"),
        Setup {
            version: Some("banana".into()),
            ..Setup::default()
        },
    );
    assert!(matches!(
        fx.start(&fx.start_params()),
        RunStartResult::Refused { .. }
    ));
}
