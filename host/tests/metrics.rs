//! The metrics recorder over a real store and clocks a test sets (ts-0067-kw; 0.13): the span lifecycle, how active
//! time and waiting time are split (sleep included), cold returns and a restart. Nothing waits in real time.

mod common;

use collide_core::adapter::StampedEvent;
use collide_core::agent::{AgentEvent, Money, TurnEndReason, Usage, UsageQuality, UsageScope};
use collide_host::clock::{Clock, ClockPair, Clocks, Reading};
use collide_host::metrics::{Metrics, span_times, time_totals, usage_totals};
use collide_host::metrics_store::{SpanKind, WaitKind};
use collide_host::store::Store;
use common::TempRoot;
use rusqlite::Connection;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const SECOND: u64 = 1_000_000_000;

/// Clocks a test moves: `awake` moves all three, `sleep` moves the continuous and the wall clock only.
struct Set(Mutex<[u64; 3]>);

impl Set {
    fn new() -> Arc<Set> {
        Arc::new(Set(Mutex::new([
            1000 * SECOND,
            1000 * SECOND,
            5000 * SECOND,
        ])))
    }

    fn awake(&self, seconds: u64) {
        for clock in self.0.lock().unwrap().iter_mut() {
            *clock += seconds * SECOND;
        }
    }

    /// The machine rebooted: the uptime and continuous clocks start again from `to`, and the wall clock goes on.
    fn reboot(&self, to: u64, wall_seconds: u64) {
        let mut clocks = self.0.lock().unwrap();
        clocks[0] = to * SECOND;
        clocks[1] = to * SECOND;
        clocks[2] += wall_seconds * SECOND;
    }

    fn sleep(&self, seconds: u64) {
        let mut clocks = self.0.lock().unwrap();
        clocks[1] += seconds * SECOND;
        clocks[2] += seconds * SECOND;
    }

    fn pair(&self) -> ClockPair {
        ClockPair::read(self)
    }
}

impl Clocks for Set {
    fn read(&self, clock: Clock) -> Reading {
        let clocks = self.0.lock().unwrap();
        let nanos = match clock {
            Clock::Uptime => clocks[0],
            Clock::Continuous => clocks[1],
            Clock::Wall => clocks[2],
        };
        Reading { clock, nanos }
    }
}

struct Rig {
    _root: TempRoot,
    store: Arc<Store>,
    clocks: Arc<Set>,
    metrics: Metrics,
    log: Arc<Mutex<Vec<String>>>,
}

impl Rig {
    fn new() -> Rig {
        let root = TempRoot::new();
        let store = Arc::new(Store::open(&root.join("store")).unwrap());
        let clocks = Set::new();
        let log = Arc::new(Mutex::new(Vec::new()));
        let metrics = metrics_over(&store, &clocks, &log);
        Rig {
            _root: root,
            store,
            clocks,
            metrics,
            log,
        }
    }

    fn times(&self, kind: SpanKind, phase: Option<&str>) -> (u64, u64, u64) {
        let spans = self.store.spans("r1").unwrap();
        let span = spans
            .iter()
            .find(|span| span.kind == kind && span.phase.as_deref() == phase)
            .unwrap();
        let now = self.clocks.read(Clock::Continuous).nanos;
        let times = span_times(span, &spans, now);
        let known = |value: Option<u64>| value.expect("the length is known") / SECOND;
        (
            known(times.elapsed_ns),
            known(times.waiting_ns),
            known(times.active_ns()),
        )
    }

    fn raw(&self) -> Connection {
        Connection::open(self.store.db_path()).unwrap()
    }
}

fn metrics_over(store: &Arc<Store>, clocks: &Arc<Set>, log: &Arc<Mutex<Vec<String>>>) -> Metrics {
    let log = Arc::clone(log);
    Metrics::new(
        Arc::clone(store),
        Arc::clone(clocks) as Arc<dyn Clocks + Send + Sync>,
        Arc::new(move |text| log.lock().unwrap().push(text.to_string())),
    )
}

fn stamped(event: AgentEvent) -> StampedEvent {
    StampedEvent {
        wall_ns: None,
        event,
    }
}

fn turn_usage(cost: f64) -> AgentEvent {
    AgentEvent::Usage {
        usage: Usage {
            scope: UsageScope::Turn,
            input: Some(1),
            output: Some(2),
            cache_read: None,
            cache_write: None,
            cache_write_1h: None,
            cache_write_5m: None,
            thought: None,
            context_used: None,
            context_size: None,
            cost: Some(Money {
                amount: cost,
                currency: "USD".into(),
            }),
            model_time_ms: Some(900),
            quality: UsageQuality::Exact,
        },
    }
}

fn turn_end() -> AgentEvent {
    AgentEvent::TurnEnd {
        reason: TurnEndReason::EndTurn {},
        cancel_requested: false,
        empty: false,
    }
}

fn started(rig: &Rig) {
    rig.metrics.run_started("r1");
    rig.metrics.phase_running("r1", "build");
}

fn count(rig: &Rig, sql: &str) -> i64 {
    rig.raw().query_row(sql, [], |row| row.get(0)).unwrap()
}

#[test]
fn a_run_with_its_phase_and_turn_has_the_span_tree_run_phase_turn() {
    let rig = Rig::new();
    started(&rig);
    rig.metrics.turn_started("r1", "build");
    let spans = rig.store.open_spans("r1").unwrap();
    let kinds: Vec<SpanKind> = spans.iter().map(|span| span.kind).collect();
    assert_eq!(kinds, [SpanKind::Run, SpanKind::Phase, SpanKind::Turn]);
    assert_eq!(spans[1].parent, Some(spans[0].id));
    assert_eq!(spans[2].parent, Some(spans[1].id));
}

#[test]
fn starting_a_run_or_a_phase_that_is_started_changes_nothing() {
    let rig = Rig::new();
    started(&rig);
    started(&rig);
    assert_eq!(rig.store.spans("r1").unwrap().len(), 2);
}

#[test]
fn an_awake_phase_is_all_active_time() {
    let rig = Rig::new();
    started(&rig);
    rig.clocks.awake(30);
    assert_eq!(rig.times(SpanKind::Phase, Some("build")), (30, 0, 30));
}

#[test]
fn a_permission_wait_is_waiting_time_and_not_active_time() {
    let rig = Rig::new();
    started(&rig);
    rig.clocks.awake(10);
    rig.metrics
        .wait_started("r1", "build", WaitKind::Permission, Some("q-1"));
    rig.clocks.awake(25);
    rig.metrics.wait_ended_for_item("r1", "build", "q-1");
    rig.clocks.awake(5);
    assert_eq!(rig.times(SpanKind::Phase, Some("build")), (40, 25, 15));
}

#[test]
fn the_machine_sleeping_is_waiting_time_and_not_active_time() {
    let rig = Rig::new();
    started(&rig);
    rig.clocks.awake(10);
    let before = rig.clocks.pair();
    rig.clocks.sleep(600);
    let now = rig.clocks.pair();
    assert!(collide_host::clock::sleep_gap(&before, &now) >= Duration::from_secs(600));
    rig.metrics.sleep_interval(Duration::from_secs(600), &now);
    rig.clocks.awake(5);
    assert_eq!(rig.times(SpanKind::Phase, Some("build")), (615, 600, 15));
}

#[test]
fn a_sleep_inside_a_permission_wait_is_counted_once() {
    let rig = Rig::new();
    started(&rig);
    rig.metrics
        .wait_started("r1", "build", WaitKind::Permission, Some("q-1"));
    rig.clocks.awake(10);
    rig.clocks.sleep(100);
    rig.metrics
        .sleep_interval(Duration::from_secs(100), &rig.clocks.pair());
    rig.clocks.awake(10);
    rig.metrics.wait_ended_for_item("r1", "build", "q-1");
    assert_eq!(rig.times(SpanKind::Phase, Some("build")), (120, 120, 0));
}

#[test]
fn a_turn_has_its_own_active_time_apart_from_the_waits_in_it() {
    let rig = Rig::new();
    started(&rig);
    rig.metrics.turn_started("r1", "build");
    rig.clocks.awake(4);
    rig.metrics
        .wait_started("r1", "build", WaitKind::Permission, Some("q-1"));
    rig.clocks.awake(6);
    rig.metrics.wait_ended_for_item("r1", "build", "q-1");
    rig.metrics.events("r1", "build", &[stamped(turn_end())]);
    assert_eq!(rig.times(SpanKind::Turn, Some("build")), (10, 6, 4));
}

#[test]
fn a_sleep_is_a_wait_for_every_open_phase_and_none_for_one_that_ended() {
    let rig = Rig::new();
    rig.metrics.run_started("r1");
    rig.metrics.phase_running("r1", "a");
    rig.metrics.phase_running("r1", "b");
    rig.metrics.phase_ended("r1", "a", "ended");
    rig.clocks.sleep(100);
    rig.metrics
        .sleep_interval(Duration::from_secs(100), &rig.clocks.pair());
    let sleeps: Vec<Option<String>> = rig
        .store
        .spans("r1")
        .unwrap()
        .into_iter()
        .filter(|span| span.wait == Some(WaitKind::Sleep))
        .map(|span| span.phase)
        .collect();
    assert_eq!(sleeps, [Some("b".to_string())]);
}

#[test]
fn a_turn_end_closes_the_turn_with_its_reason_after_its_usage_is_recorded_in_it() {
    let rig = Rig::new();
    started(&rig);
    rig.metrics.turn_started("r1", "build");
    rig.metrics.events(
        "r1",
        "build",
        &[stamped(turn_end()), stamped(turn_usage(0.5))],
    );
    let spans = rig.store.spans("r1").unwrap();
    let turn = spans.iter().find(|s| s.kind == SpanKind::Turn).unwrap();
    assert_eq!(turn.outcome.as_deref(), Some("end_turn"));
    assert_eq!(
        rig.store.usage_rows("r1", None).unwrap()[0].turn,
        Some(turn.id)
    );
}

#[test]
fn a_new_turn_cuts_one_the_last_left_open() {
    let rig = Rig::new();
    started(&rig);
    rig.metrics.turn_started("r1", "build");
    rig.metrics.turn_started("r1", "build");
    let outcomes: Vec<Option<String>> = rig
        .store
        .spans("r1")
        .unwrap()
        .into_iter()
        .filter(|span| span.kind == SpanKind::Turn)
        .map(|span| span.outcome)
        .collect();
    assert_eq!(outcomes, [Some("cut".to_string()), None]);
}

#[test]
fn a_phase_that_ends_closes_its_turn_and_waits_and_leaves_the_run_open() {
    let rig = Rig::new();
    started(&rig);
    rig.metrics.turn_started("r1", "build");
    rig.metrics
        .wait_started("r1", "build", WaitKind::Permission, Some("q-1"));
    rig.metrics.phase_ended("r1", "build", "failed");
    let open = rig.store.open_spans("r1").unwrap();
    assert_eq!(open.len(), 1);
    assert_eq!(open[0].kind, SpanKind::Run);
    let phase = rig
        .store
        .spans("r1")
        .unwrap()
        .into_iter()
        .find(|s| s.kind == SpanKind::Phase)
        .unwrap();
    assert_eq!(phase.outcome.as_deref(), Some("failed"));
}

#[test]
fn a_run_that_ends_closes_everything_it_had_open() {
    let rig = Rig::new();
    started(&rig);
    rig.metrics.turn_started("r1", "build");
    rig.metrics.run_ended("r1", "ended");
    assert!(rig.store.open_spans("r1").unwrap().is_empty());
}

#[test]
fn a_limit_pause_longer_than_the_cache_lives_ends_in_a_cold_return() {
    let rig = Rig::new();
    started(&rig);
    rig.metrics
        .wait_started("r1", "build", WaitKind::UsageLimit, Some("q-limit"));
    rig.clocks.awake(3601);
    rig.metrics.phase_running("r1", "build");
    let row: (String, i64) = rig
        .raw()
        .query_row("SELECT cause, idle_ms FROM cold_returns", [], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })
        .unwrap();
    assert_eq!(row, ("usage_limit".to_string(), 3_601_000));
}

#[test]
fn a_wait_for_the_owner_longer_than_the_cache_lives_is_a_queue_wait_cold_return() {
    let rig = Rig::new();
    started(&rig);
    rig.metrics
        .wait_started("r1", "build", WaitKind::Permission, Some("q-1"));
    rig.clocks.awake(4000);
    rig.metrics.wait_ended_for_item("r1", "build", "q-1");
    assert_eq!(
        count(
            &rig,
            "SELECT COUNT(*) FROM cold_returns WHERE cause = 'queue_wait'"
        ),
        1
    );
}

#[test]
fn a_wait_of_exactly_the_cache_life_or_less_is_not_a_cold_return() {
    let rig = Rig::new();
    started(&rig);
    rig.metrics
        .wait_started("r1", "build", WaitKind::UsageLimit, Some("q"));
    rig.clocks.awake(3600);
    rig.metrics.phase_running("r1", "build");
    assert_eq!(count(&rig, "SELECT COUNT(*) FROM cold_returns"), 0);
}

#[test]
fn a_phase_open_across_a_reboot_has_no_length_and_a_total_counts_it_as_missing() {
    let rig = Rig::new();
    started(&rig);
    rig.clocks.awake(100);
    // The machine rebooted: the continuous clock starts again, below the phase's start.
    rig.clocks.reboot(5, 3600);
    rig.metrics.phase_ended("r1", "build", "ended");

    let spans = rig.store.spans("r1").unwrap();
    let now = rig.clocks.read(Clock::Continuous).nanos;
    let phase = spans
        .iter()
        .find(|span| span.kind == SpanKind::Phase)
        .unwrap();
    assert!(phase.ended_ns.unwrap() < phase.started_ns);
    let times = span_times(phase, &spans, now);
    assert_eq!(
        (times.elapsed_ns, times.waiting_ns, times.active_ns()),
        (None, None, None),
        "an unknown length is null, not zero"
    );
    let totals = time_totals(SpanKind::Phase, &spans, now);
    assert_eq!((totals.elapsed.sum, totals.elapsed.missing), (None, 1));
    assert_eq!((totals.active.sum, totals.active.missing), (None, 1));
}

#[test]
fn a_wait_that_ends_after_a_reboot_records_no_cold_return() {
    let rig = Rig::new();
    started(&rig);
    rig.metrics
        .wait_started("r1", "build", WaitKind::Permission, Some("q-1"));
    rig.clocks.awake(100);
    rig.clocks.reboot(5, 7200);
    rig.metrics.wait_ended_for_item("r1", "build", "q-1");
    assert_eq!(count(&rig, "SELECT COUNT(*) FROM cold_returns"), 0);
}

#[test]
fn a_sleep_alone_is_not_a_cold_return() {
    let rig = Rig::new();
    started(&rig);
    rig.clocks.sleep(7200);
    rig.metrics
        .sleep_interval(Duration::from_secs(7200), &rig.clocks.pair());
    rig.metrics.phase_ended("r1", "build", "ended");
    assert_eq!(count(&rig, "SELECT COUNT(*) FROM cold_returns"), 0);
}

#[test]
fn a_restarted_daemon_goes_on_with_the_spans_the_last_one_left_open() {
    let rig = Rig::new();
    started(&rig);
    rig.metrics
        .wait_started("r1", "build", WaitKind::Permission, Some("q-1"));
    rig.clocks.awake(20);
    let second = metrics_over(&rig.store, &rig.clocks, &rig.log);
    second.run_started("r1");
    second.phase_running("r1", "build");
    rig.clocks.awake(5);
    second.wait_ended_for_item("r1", "build", "q-1");
    assert_eq!(rig.store.spans("r1").unwrap().len(), 3);
    assert_eq!(rig.times(SpanKind::Phase, Some("build")), (25, 25, 0));
}

#[test]
fn a_store_that_fails_is_logged_and_does_not_panic() {
    let rig = Rig::new();
    std::fs::remove_file(rig.store.db_path()).unwrap();
    rig.metrics.run_started("r1");
    rig.metrics.events("r1", "build", &[stamped(turn_end())]);
    assert!(
        rig.log
            .lock()
            .unwrap()
            .iter()
            .any(|line| line.starts_with("metrics:")),
        "{:?}",
        rig.log.lock().unwrap()
    );
}

#[test]
fn the_totals_of_a_phase_come_from_its_turn_rows_only() {
    let rig = Rig::new();
    started(&rig);
    let step = AgentEvent::Usage {
        usage: Usage {
            scope: UsageScope::Step,
            output: Some(100),
            ..match turn_usage(1.0) {
                AgentEvent::Usage { usage } => usage,
                _ => unreachable!(),
            }
        },
    };
    rig.metrics
        .events("r1", "build", &[stamped(step), stamped(turn_usage(0.5))]);
    let totals = usage_totals(&rig.store.usage_rows("r1", Some("build")).unwrap());
    assert_eq!(totals.exact.output.sum, Some(2));
    assert_eq!(totals.exact.cost.sum, Some(0.5));
}
