//! What the daemon keeps of a run's time, usage and detectors, with fake Claude behind it (ts-0067-kw; 0.13): the span
//! tree, the split between active and waiting time (a permission wait, a usage-limit pause and a machine sleep all
//! count as waiting), usage per turn, missing values as nulls, hook calls and findings, the fingerprint's user-config
//! part, and what must never reach the store.
//!
//! Each test has a daemon of its own under a temp root (`pause.rs`'s `Rig`), with the same fake clocks: `advance`
//! moves all three clocks (time that passed awake), `jump` moves the wall and continuous clocks only (a machine that
//! slept), so nothing waits an hour in real time. The tests read the store the daemon writes, through its own
//! methods or, for a column no reader has, with plain SQL on a connection of the test's own.

use super::common::TempRoot;
use super::fake::recording;
use super::pause::{Rig, Settings, edited, nanos, permission_item};
use collide_core::agent::PermissionReply;
use collide_core::events::EventBody;
use collide_core::owner::{QueueAnswer, QueueKind};
use collide_core::state::PhaseState;
use collide_host::clock::{Clock, Clocks};
use collide_host::file_run::PHASE_NAME;
use collide_host::metrics::{span_times, usage_totals};
use collide_host::metrics_store::{Span, SpanKind, WaitKind};
use collide_host::store::Store;
use rusqlite::{Connection, types::ValueRef};
use std::fs;
use std::sync::Arc;
use std::time::Duration;

use super::common::wait_until;

const PATIENCE: Duration = Duration::from_secs(20);
const MINUTE: Duration = Duration::from_secs(60);
const HOUR: Duration = Duration::from_secs(3600);

fn store(rig: &Rig) -> Arc<Store> {
    rig.daemon.as_ref().expect("a daemon runs").store()
}

fn spans(rig: &Rig, run: &str) -> Vec<Span> {
    store(rig).spans(run).expect("the spans are readable")
}

fn of_kind(all: &[Span], kind: SpanKind) -> Vec<&Span> {
    all.iter().filter(|span| span.kind == kind).collect()
}

fn waits_of(all: &[Span], wait: WaitKind) -> Vec<&Span> {
    all.iter()
        .filter(|span| span.kind == SpanKind::Wait && span.wait == Some(wait))
        .collect()
}

/// Waits until the store holds a span of `kind` for the run that `test` accepts.
fn wait_for_span(rig: &Rig, run: &str, what: &str, test: impl Fn(&Span) -> bool) -> Span {
    let mut found = None;
    wait_until(what, PATIENCE, || {
        found = spans(rig, run).into_iter().find(|span| test(span));
        found.is_some()
    });
    found.unwrap()
}

fn now_ns(rig: &Rig) -> u64 {
    rig.clocks.read(Clock::Continuous).nanos
}

/// The run's phase and its times once every span has ended.
fn phase_span(all: &[Span]) -> &Span {
    of_kind(all, SpanKind::Phase)
        .into_iter()
        .next()
        .expect("a phase span")
}

fn closed(span: &Span) -> bool {
    span.ended_ns.is_some() && span.ended_wall_ms.is_some()
}

/// Waits until every turn and the phase have closed in the store: the recorder polls the feed, so it can lag the
/// phase's final state by a poll.
fn wait_closed(rig: &Rig, run: &str) {
    wait_until("every span but the run's has closed", PATIENCE, || {
        let all = spans(rig, run);
        !of_kind(&all, SpanKind::Phase).is_empty()
            && all
                .iter()
                .filter(|span| span.kind != SpanKind::Run)
                .all(closed)
    });
}

fn sql_rows<T>(
    rig: &Rig,
    query: &str,
    row: impl Fn(&rusqlite::Row) -> rusqlite::Result<T>,
) -> Vec<T> {
    let connection = Connection::open(store(rig).db_path()).expect("the store opens");
    let mut statement = connection.prepare(query).expect("the query is valid");
    statement
        .query_map([], row)
        .expect("the query runs")
        .collect::<Result<_, _>>()
        .expect("the rows read")
}

#[test]
fn a_one_turn_run_has_a_run_a_phase_and_a_turn_span_and_its_usage_is_in_the_turn() {
    let mut rig = Rig::new(
        &recording("report"),
        &recording("report"),
        Settings::default(),
    );
    let run = rig.started(None);
    assert_eq!(rig.wait_final(&run), PhaseState::Ended {});
    wait_closed(&rig, &run);
    let all = spans(&rig, &run);

    let runs = of_kind(&all, SpanKind::Run);
    let phases = of_kind(&all, SpanKind::Phase);
    let turns = of_kind(&all, SpanKind::Turn);
    assert_eq!((runs.len(), phases.len(), turns.len()), (1, 1, 1));
    assert_eq!(phases[0].parent, Some(runs[0].id));
    assert_eq!(turns[0].parent, Some(phases[0].id));
    assert_eq!(phases[0].phase.as_deref(), Some(PHASE_NAME));
    assert_eq!(phases[0].outcome.as_deref(), Some("ended"));
    assert_eq!(turns[0].outcome.as_deref(), Some("end_turn"));
    assert!(closed(phases[0]) && closed(turns[0]));
    assert!(turns[0].ended_ns >= Some(turns[0].started_ns));

    let usage = store(&rig).usage_rows(&run, None).unwrap();
    assert!(!usage.is_empty(), "the turn's usage is stored");
    assert!(
        usage.iter().any(|row| row.turn == Some(turns[0].id)),
        "a usage row names the turn it came in"
    );
    // The recording's result line says what the turn cost and how long the model worked.
    let totals = usage_totals(&usage);
    assert!(totals.is_exact());
    let turn_row = usage
        .iter()
        .find(|row| row.usage.model_time_ms.is_some())
        .expect("a row with the model time");
    assert_eq!(turn_row.usage.model_time_ms, Some(1300));
    assert_eq!(turn_row.usage.cost.as_ref().map(|c| c.amount), Some(0.0041));
}

#[test]
fn a_permission_wait_is_a_wait_span_for_its_item_and_is_not_active_time() {
    let mut rig = Rig::new(
        &recording("permission-request"),
        &recording("report"),
        Settings::default(),
    );
    let run = rig.started(None);
    let item = permission_item(&mut rig, &run);
    wait_for_span(&rig, &run, "the permission wait opens", |span| {
        span.wait == Some(WaitKind::Permission) && span.ended_ns.is_none()
    });

    rig.clocks.advance(30 * MINUTE);
    let answer = QueueAnswer::Permission {
        reply: PermissionReply::Allow {},
    };
    rig.answer(&item, answer);
    assert_eq!(rig.wait_final(&run), PhaseState::Ended {});
    wait_closed(&rig, &run);

    let all = spans(&rig, &run);
    let waits = waits_of(&all, WaitKind::Permission);
    assert_eq!(waits.len(), 1);
    assert_eq!(waits[0].item.as_deref(), Some(item.id.as_str()));
    assert!(closed(waits[0]));
    let waited = waits[0].ended_ns.unwrap() - waits[0].started_ns;
    assert!(waited >= nanos(30 * MINUTE), "waited {waited} ns");

    let times = span_times(phase_span(&all), &all, now_ns(&rig));
    assert!(times.waiting_ns.unwrap() >= nanos(30 * MINUTE));
    assert!(
        times.active_ns().unwrap() < nanos(MINUTE),
        "active {} ns",
        times.active_ns().unwrap()
    );
    // Half an hour is under the cache's life: not a cold return.
    let cold = sql_rows(&rig, "SELECT cause FROM cold_returns", |r| {
        r.get::<_, String>(0)
    });
    assert!(cold.is_empty(), "{cold:?}");
}

#[test]
fn a_permission_wait_over_an_hour_is_a_cold_return_for_the_queue_wait() {
    let mut rig = Rig::new(
        &recording("permission-request"),
        &recording("report"),
        Settings::default(),
    );
    let run = rig.started(None);
    let item = permission_item(&mut rig, &run);
    wait_for_span(&rig, &run, "the permission wait opens", |span| {
        span.wait == Some(WaitKind::Permission) && span.ended_ns.is_none()
    });

    rig.clocks.advance(HOUR + 5 * MINUTE);
    rig.answer(
        &item,
        QueueAnswer::Permission {
            reply: PermissionReply::Allow {},
        },
    );
    assert_eq!(rig.wait_final(&run), PhaseState::Ended {});
    wait_closed(&rig, &run);

    let cold = sql_rows(&rig, "SELECT cause, idle_ms FROM cold_returns", |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
    });
    assert_eq!(cold.len(), 1, "{cold:?}");
    assert_eq!(cold[0].0, "queue_wait");
    assert!(cold[0].1 >= (HOUR + 5 * MINUTE).as_millis() as i64);
}

#[test]
fn a_usage_limit_pause_is_a_wait_and_resuming_after_the_reset_is_a_cold_return() {
    let mut rig = Rig::new(
        &recording("limit"),
        &recording("report"),
        Settings::default(),
    );
    let run = rig.started(Some(1.0));
    rig.wait_item(&run, QueueKind::UsageLimit);
    wait_for_span(&rig, &run, "the limit wait opens", |span| {
        span.wait == Some(WaitKind::UsageLimit)
    });

    rig.clocks.set_step(Duration::from_secs(600));
    assert_eq!(rig.wait_final(&run), PhaseState::Ended {});
    wait_closed(&rig, &run);

    let all = spans(&rig, &run);
    let limit = waits_of(&all, WaitKind::UsageLimit);
    assert_eq!(limit.len(), 1);
    assert!(closed(limit[0]));
    let waited = limit[0].ended_ns.unwrap() - limit[0].started_ns;
    assert!(waited >= nanos(HOUR), "the limit wait was {waited} ns");
    let times = span_times(phase_span(&all), &all, now_ns(&rig));
    assert!(times.waiting_ns.unwrap() >= nanos(HOUR));
    assert!(times.active_ns().unwrap() < nanos(MINUTE));

    let cold = sql_rows(&rig, "SELECT cause FROM cold_returns", |r| {
        r.get::<_, String>(0)
    });
    assert_eq!(cold, ["usage_limit"]);
}

#[test]
fn a_machine_sleep_the_sampler_finds_is_a_sleep_wait_and_not_active_time() {
    let root = TempRoot::new();
    let mark = root.join("running");
    let first = edited(&root, "report", |line| {
        if line.contains("\"subtype\":\"init\"") {
            Some(format!(
                "{line}\n{}\n{{\"sleep\":2}}",
                serde_json::json!({ "touch": mark.to_str().unwrap() })
            ))
        } else {
            Some(line.to_string())
        }
    });
    let mut rig = Rig::new(&first, &recording("report"), Settings::default());
    let run = rig.started(None);
    wait_until("the phase is under way", PATIENCE, || mark.exists());

    rig.clocks.jump(10 * MINUTE);
    let sleep = wait_for_span(&rig, &run, "the sleep is a wait", |span| {
        span.wait == Some(WaitKind::Sleep)
    });
    assert_eq!(rig.wait_final(&run), PhaseState::Ended {});
    wait_closed(&rig, &run);

    let all = spans(&rig, &run);
    assert!(
        sleep.ended_ns.is_some() || closed(waits_of(&all, WaitKind::Sleep)[0]),
        "a sleep interval is a closed span"
    );
    let slept: u64 = waits_of(&all, WaitKind::Sleep)
        .iter()
        .map(|span| span.ended_ns.unwrap() - span.started_ns)
        .sum();
    assert!(
        slept >= nanos(10 * MINUTE) - nanos(MINUTE),
        "slept {slept} ns"
    );

    let phase = phase_span(&all);
    let times = span_times(phase, &all, now_ns(&rig));
    assert!(times.elapsed_ns.unwrap() >= nanos(10 * MINUTE));
    assert!(
        times.active_ns().unwrap() < nanos(MINUTE),
        "a sleep of ten minutes left {} ns active",
        times.active_ns().unwrap()
    );
}

#[test]
fn a_figure_the_recording_lacks_is_stored_as_null_and_not_as_zero() {
    let root = TempRoot::new();
    let first = edited(&root, "report", |line| {
        if line.contains("\"type\":\"result\"") {
            let mut step: serde_json::Value = serde_json::from_str(line).unwrap();
            let result = &mut step["say"];
            for key in ["total_cost_usd", "duration_api_ms", "usage", "modelUsage"] {
                result.as_object_mut().unwrap().remove(key);
            }
            Some(step.to_string())
        } else {
            Some(line.to_string())
        }
    });
    let mut rig = Rig::new(&first, &recording("report"), Settings::default());
    let run = rig.started(None);
    assert_eq!(rig.wait_final(&run), PhaseState::Ended {});
    wait_closed(&rig, &run);

    let rows = store(&rig).usage_rows(&run, None).unwrap();
    assert!(!rows.is_empty());
    for row in &rows {
        if row.usage.cost.is_none() {
            continue;
        }
        // A cost that is stored came from the stream, not from a default.
        assert!(row.usage.cost.as_ref().unwrap().amount != 0.0);
    }
    assert!(
        rows.iter().all(|row| row.usage.model_time_ms.is_none()),
        "no row has a model time the stream did not give"
    );
    let nulls = sql_rows(
        &rig,
        "SELECT COUNT(*) FROM usage_rows WHERE model_time_ms = 0 OR cost_amount = 0",
        |r| r.get::<_, i64>(0),
    );
    assert_eq!(nulls, [0]);
}

#[test]
fn a_hook_that_failed_open_is_stored_with_its_finding_and_the_cost_of_its_turn() {
    let mut rig = Rig::new(
        &recording("hook-failure"),
        &recording("report"),
        Settings::default(),
    );
    let run = rig.started(None);
    let item = rig.wait_item(&run, QueueKind::Permission);
    rig.answer(
        &item,
        QueueAnswer::Permission {
            reply: PermissionReply::Allow {},
        },
    );
    assert_eq!(rig.wait_final(&run), PhaseState::Ended {});
    wait_closed(&rig, &run);

    let hooks = sql_rows(
        &rig,
        "SELECT name, event, outcome, exit_code FROM hook_calls",
        |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, Option<i64>>(3)?,
            ))
        },
    );
    assert!(!hooks.is_empty(), "the failing hook is a row");
    assert!(
        hooks
            .iter()
            .any(|(_, event, _, code)| event == "PreToolUse" && *code == Some(1)),
        "{hooks:?}"
    );
    let findings = sql_rows(
        &rig,
        "SELECT detector, cause, cost_amount, cost_basis FROM findings",
        |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<f64>>(2)?,
                r.get::<_, String>(3)?,
            ))
        },
    );
    assert!(
        !findings.is_empty(),
        "the hook that failed open is a finding"
    );
    assert!(
        findings.iter().all(|(_, _, _, basis)| basis == "turn"),
        "{findings:?}"
    );
}

#[test]
fn no_hook_output_or_tool_result_text_reaches_the_store() {
    let mut rig = Rig::new(
        &recording("hook-deny-json"),
        &recording("report"),
        Settings::default(),
    );
    let run = rig.started(None);
    let outcome = rig.wait_final(&run);
    assert!(
        matches!(outcome, PhaseState::Ended {} | PhaseState::Failed { .. }),
        "{outcome:?}"
    );
    wait_closed(&rig, &run);
    // Let every row land before the store is read: the daemon polls the feed.
    std::thread::sleep(Duration::from_millis(300));

    let marker = "SECRET-STDOUT-MARKER";
    let db = store(&rig).db_path();
    for suffix in ["", "-wal", "-shm"] {
        let mut name = db.clone().into_os_string();
        name.push(suffix);
        if let Ok(bytes) = fs::read(std::path::PathBuf::from(name)) {
            assert!(
                !bytes.windows(marker.len()).any(|w| w == marker.as_bytes()),
                "{marker} is in store.db{suffix}"
            );
        }
    }
    // The same check over every table, as the values a reader would see.
    let connection = Connection::open(&db).unwrap();
    let tables: Vec<String> = connection
        .prepare("SELECT name FROM sqlite_master WHERE type = 'table'")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert!(tables.iter().any(|t| t == "hook_calls"));
    for table in tables {
        let mut statement = connection
            .prepare(&format!("SELECT * FROM \"{table}\""))
            .unwrap();
        let columns = statement.column_count();
        let mut rows = statement.query([]).unwrap();
        while let Some(row) = rows.next().unwrap() {
            for at in 0..columns {
                let text = match row.get_ref(at).unwrap() {
                    ValueRef::Text(bytes) | ValueRef::Blob(bytes) => {
                        String::from_utf8_lossy(bytes).into_owned()
                    }
                    other => format!("{other:?}"),
                };
                assert!(!text.contains(marker), "{marker} is in {table}");
            }
        }
    }
}

#[test]
fn a_phases_end_keeps_the_shims_exit_totals_and_the_workspaces_disk_space() {
    let mut rig = Rig::new(
        &recording("report"),
        &recording("report"),
        Settings::default(),
    );
    let run = rig.started(None);
    assert_eq!(rig.wait_final(&run), PhaseState::Ended {});
    // The row is written when the phase moves; the workspace's size is added by the run's end hook just after.
    wait_until("the workspace's size is stored", PATIENCE, || {
        !sql_rows(
            &rig,
            "SELECT id FROM phase_ends WHERE workspace_bytes IS NOT NULL",
            |r| r.get::<_, i64>(0),
        )
        .is_empty()
    });

    let ends = sql_rows(
        &rig,
        "SELECT state, exit_code, cpu_user_us, max_rss_bytes, workspace_bytes FROM phase_ends",
        |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, Option<i64>>(1)?,
                r.get::<_, Option<i64>>(2)?,
                r.get::<_, Option<i64>>(3)?,
                r.get::<_, Option<i64>>(4)?,
            ))
        },
    );
    assert_eq!(ends.len(), 1, "{ends:?}");
    let (state, code, cpu, rss, workspace) = &ends[0];
    assert_eq!(state, "ended");
    assert_eq!(*code, Some(0));
    assert!(cpu.is_some() && rss.is_some(), "the shim's totals are kept");
    assert!(workspace.is_some_and(|bytes| bytes > 0), "{workspace:?}");
}

#[test]
fn the_fingerprints_user_config_part_names_hooks_and_the_endpoint_host_and_nothing_secret() {
    let mut rig = Rig::new(
        &recording("report"),
        &recording("report"),
        Settings::default(),
    );
    let fixture = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/adapters/claude/fixtures/user-settings.json");
    let claude = rig.root.join("home/.claude");
    fs::create_dir_all(&claude).unwrap();
    fs::copy(fixture, claude.join("settings.json")).unwrap();

    let run = rig.started(None);
    assert_eq!(rig.wait_final(&run), PhaseState::Ended {});
    let part = store(&rig)
        .fingerprint_part(&run, "user_config")
        .unwrap()
        .expect("the part is stored when the run starts");
    let json: serde_json::Value = serde_json::from_str(&part).unwrap();

    let events: Vec<&str> = json["hooks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|hook| hook["event"].as_str().unwrap())
        .collect();
    for event in [
        "SessionStart",
        "UserPromptSubmit",
        "PreToolUse",
        "PostToolUse",
        "Stop",
    ] {
        assert!(events.contains(&event), "{event} missing from {events:?}");
    }
    assert_eq!(json["endpoint"]["host"], "gateway.example.com");

    for secret in [
        "FIXTURE-KEY-0000",
        "FIXTURE-HOOK-SECRET",
        "/v1/proxy",
        "8443",
    ] {
        assert!(!part.contains(secret), "{secret} is in {part}");
        let db = fs::read(store(&rig).db_path()).unwrap();
        assert!(
            !db.windows(secret.len()).any(|w| w == secret.as_bytes()),
            "{secret} is in the store file"
        );
    }
}

#[test]
fn an_open_wait_survives_a_daemon_restart_and_closes_at_its_answer() {
    let mut rig = Rig::new(
        &recording("permission-request"),
        &recording("report"),
        Settings::default(),
    );
    let run = rig.started(None);
    permission_item(&mut rig, &run);
    let before = wait_for_span(&rig, &run, "the permission wait opens", |span| {
        span.wait == Some(WaitKind::Permission)
    });

    rig.restart();
    let item = permission_item(&mut rig, &run);
    let still = spans(&rig, &run);
    let waits = waits_of(&still, WaitKind::Permission);
    assert_eq!(waits.len(), 1, "a restart does not open a second wait");
    assert_eq!(waits[0].id, before.id);
    assert!(waits[0].ended_ns.is_none(), "the wait is still open");

    rig.answer(
        &item,
        QueueAnswer::Permission {
            reply: PermissionReply::Allow {},
        },
    );
    assert_eq!(rig.wait_final(&run), PhaseState::Ended {});
    wait_closed(&rig, &run);
    let all = spans(&rig, &run);
    let waits = waits_of(&all, WaitKind::Permission);
    assert_eq!(waits.len(), 1);
    assert!(closed(waits[0]));
    assert_eq!(of_kind(&all, SpanKind::Run).len(), 1);
    assert!(
        rig.events()
            .iter()
            .any(|body| matches!(body, EventBody::PermissionAnswered { .. }))
    );
}
