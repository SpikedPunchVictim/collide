//! The metrics tables of the store (ts-0067-kw; 0.13): migration 4, spans, usage rows with NULL for what was not
//! reported, and the whitelist that keeps content out. Real SQLite files in a test folder; nothing else is touched.

mod common;

use collide_core::agent::{
    AgentEvent, CompactionTrigger, HookFailure, HookOutcome, Money, RefusalCause, ToolKind, Usage,
    UsageQuality, UsageScope,
};
use collide_core::shim::{ExitEnd, ExitRecord};
use collide_host::metrics_store::{NewSpan, SpanKind, WaitKind};
use collide_host::store::{DB_FILE, MIGRATIONS, Store};
use common::TempRoot;
use rusqlite::Connection;
use serde_json::json;
use std::sync::Arc;

fn store(root: &TempRoot) -> Store {
    Store::open(&root.join("store")).unwrap()
}

fn raw(root: &TempRoot) -> Connection {
    Connection::open(root.join("store").join(DB_FILE)).unwrap()
}

fn blank(scope: UsageScope, quality: UsageQuality) -> Usage {
    Usage {
        scope,
        input: None,
        output: None,
        cache_read: None,
        cache_write: None,
        cache_write_1h: None,
        cache_write_5m: None,
        thought: None,
        context_used: None,
        context_size: None,
        cost: None,
        model_time_ms: None,
        quality,
    }
}

fn span<'a>(run: &'a str, phase: Option<&'a str>, kind: SpanKind, ns: u64) -> NewSpan<'a> {
    NewSpan {
        run,
        phase,
        parent: None,
        kind,
        wait: None,
        item: None,
        wall_ms: 1_000,
        ns,
    }
}

fn record(store: &Store, events: &[AgentEvent]) {
    let timed: Vec<(i64, &AgentEvent)> = events.iter().map(|event| (5_000, event)).collect();
    store.record_agent_events("r1", "build", &timed).unwrap();
}

fn count(conn: &Connection, sql: &str) -> i64 {
    conn.query_row(sql, [], |row| row.get(0)).unwrap()
}

#[test]
fn migration_four_makes_the_metrics_tables() {
    let root = TempRoot::new();
    let _store = store(&root);
    let conn = raw(&root);
    assert_eq!(MIGRATIONS.last().unwrap().name, "metrics");
    for table in [
        "spans",
        "usage_rows",
        "tool_calls",
        "hook_calls",
        "findings",
        "compactions",
        "other_counts",
        "phase_ends",
        "cold_returns",
        "fingerprints",
    ] {
        assert_eq!(
            count(
                &conn,
                &format!(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = '{table}'"
                )
            ),
            1,
            "{table}"
        );
    }
}

#[test]
fn a_span_is_open_until_it_is_closed_and_keeps_its_first_end() {
    let root = TempRoot::new();
    let store = store(&root);
    let id = store
        .open_span(&span("r1", None, SpanKind::Run, 100))
        .unwrap();
    assert_eq!(store.open_spans("r1").unwrap().len(), 1);
    store.close_span(id, 2_000, 900, Some("ended")).unwrap();
    store.close_span(id, 3_000, 950, Some("again")).unwrap();
    let spans = store.spans("r1").unwrap();
    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].ended_ns, Some(900));
    assert_eq!(spans[0].outcome.as_deref(), Some("ended"));
    assert!(store.open_spans("r1").unwrap().is_empty());
}

#[test]
fn a_wait_span_keeps_its_kind_and_its_queue_item() {
    let root = TempRoot::new();
    let store = store(&root);
    store
        .open_span(&NewSpan {
            wait: Some(WaitKind::Permission),
            item: Some("q-1"),
            ..span("r1", Some("build"), SpanKind::Wait, 5)
        })
        .unwrap();
    let found = &store.open_spans("r1").unwrap()[0];
    assert_eq!(found.wait, Some(WaitKind::Permission));
    assert_eq!(found.item.as_deref(), Some("q-1"));
    assert_eq!(store.runs_with_open_spans().unwrap(), ["r1"]);
}

#[test]
fn a_figure_the_agent_did_not_report_is_null_in_the_row_and_none_when_read_back() {
    let root = TempRoot::new();
    let store = store(&root);
    let mut usage = blank(UsageScope::Turn, UsageQuality::Partial);
    usage.output = Some(7);
    record(
        &store,
        &[AgentEvent::Usage {
            usage: usage.clone(),
        }],
    );
    let conn = raw(&root);
    assert_eq!(
        count(
            &conn,
            "SELECT COUNT(*) FROM usage_rows WHERE input IS NULL AND cost_amount IS NULL AND model_time_ms IS NULL AND output = 7"
        ),
        1
    );
    let rows = store.usage_rows("r1", Some("build")).unwrap();
    assert_eq!(rows[0].usage, usage);
}

#[test]
fn a_cost_is_stored_with_its_currency_and_reads_back() {
    let root = TempRoot::new();
    let store = store(&root);
    let mut usage = blank(UsageScope::Turn, UsageQuality::Exact);
    usage.cost = Some(Money {
        amount: 0.0123,
        currency: "USD".into(),
    });
    record(
        &store,
        &[AgentEvent::Usage {
            usage: usage.clone(),
        }],
    );
    assert_eq!(store.usage_rows("r1", None).unwrap()[0].usage, usage);
}

#[test]
fn a_usage_row_names_the_turn_span_it_came_in() {
    let root = TempRoot::new();
    let store = store(&root);
    let turn = store
        .open_span(&span("r1", Some("build"), SpanKind::Turn, 10))
        .unwrap();
    record(
        &store,
        &[AgentEvent::Usage {
            usage: blank(UsageScope::Step, UsageQuality::Exact),
        }],
    );
    assert_eq!(store.usage_rows("r1", None).unwrap()[0].turn, Some(turn));
}

#[test]
fn the_size_after_a_compaction_is_the_context_of_the_next_step_and_not_a_turns() {
    let root = TempRoot::new();
    let store = store(&root);
    let mut turn = blank(UsageScope::Turn, UsageQuality::Exact);
    turn.context_used = Some(99);
    let mut step = blank(UsageScope::Step, UsageQuality::Exact);
    step.context_used = Some(4_000);
    record(
        &store,
        &[
            AgentEvent::Compaction {
                trigger: Some(CompactionTrigger::Auto),
                context_before: Some(67_000),
                duration_ms: Some(10),
                mid_task: true,
            },
            AgentEvent::Usage { usage: turn },
            AgentEvent::Usage { usage: step },
        ],
    );
    let conn = raw(&root);
    let row: (String, i64, i64, i64, i64) = conn
        .query_row(
            "SELECT trigger_kind, tokens_before, tokens_after, duration_ms, mid_task FROM compactions",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
        )
        .unwrap();
    assert_eq!(row, ("auto".to_string(), 67_000, 4_000, 10, 1));
}

#[test]
fn a_compaction_whose_line_lacked_figures_has_them_null() {
    let root = TempRoot::new();
    let store = store(&root);
    record(
        &store,
        &[AgentEvent::Compaction {
            trigger: None,
            context_before: None,
            duration_ms: None,
            mid_task: false,
        }],
    );
    assert_eq!(
        count(
            &raw(&root),
            "SELECT COUNT(*) FROM compactions WHERE trigger_kind IS NULL AND tokens_before IS NULL AND tokens_after IS NULL AND duration_ms IS NULL AND mid_task = 0"
        ),
        1
    );
}

#[test]
fn a_finding_is_given_the_cost_of_its_turn_whichever_comes_first() {
    let root = TempRoot::new();
    let store = store(&root);
    let turn = store
        .open_span(&span("r1", Some("build"), SpanKind::Turn, 10))
        .unwrap();
    let failed_open = |call: &str| AgentEvent::HookFailedOpen {
        name: "PreToolUse:Bash".into(),
        event: "PreToolUse".into(),
        tool_call_id: call.into(),
        cause: HookFailure::ExitError,
    };
    let mut usage = blank(UsageScope::Turn, UsageQuality::Exact);
    usage.cost = Some(Money {
        amount: 0.5,
        currency: "USD".into(),
    });
    record(&store, &[failed_open("c1")]);
    record(&store, &[AgentEvent::Usage { usage }]);
    record(&store, &[failed_open("c2")]);
    let conn = raw(&root);
    assert_eq!(
        count(
            &conn,
            "SELECT COUNT(*) FROM findings WHERE cost_amount = 0.5 AND cost_currency = 'USD' AND cost_basis = 'turn' AND detector = 'failed_open'"
        ),
        2
    );
    assert_eq!(
        count(
            &conn,
            &format!("SELECT COUNT(*) FROM findings WHERE turn = {turn}")
        ),
        2
    );
}

#[test]
fn batches_written_at_the_same_time_are_all_kept() {
    // A batch that reads (the turn's cost) before it writes must wait for another writer, not be refused.
    let root = TempRoot::new();
    let store = Arc::new(store(&root));
    let writers: Vec<_> = (0..8)
        .map(|writer| {
            let store = Arc::clone(&store);
            std::thread::spawn(move || {
                for call in 0..25 {
                    let event = AgentEvent::HookFailedOpen {
                        name: "n".into(),
                        event: "e".into(),
                        tool_call_id: format!("c{writer}-{call}"),
                        cause: HookFailure::ExitError,
                    };
                    store
                        .record_agent_events("r1", "build", &[(1, &event)])
                        .expect("a batch is not lost to another writer");
                }
            })
        })
        .collect();
    for writer in writers {
        writer.join().unwrap();
    }
    assert_eq!(count(&raw(&root), "SELECT COUNT(*) FROM findings"), 200);
}

#[test]
fn a_finding_with_no_turn_cost_has_a_null_cost() {
    let root = TempRoot::new();
    let store = store(&root);
    record(
        &store,
        &[AgentEvent::HookFailedOpen {
            name: "n".into(),
            event: "e".into(),
            tool_call_id: "c".into(),
            cause: HookFailure::NoVerdict,
        }],
    );
    assert_eq!(
        count(
            &raw(&root),
            "SELECT COUNT(*) FROM findings WHERE cost_amount IS NULL AND cause = 'no_verdict'"
        ),
        1
    );
}

#[test]
fn a_refusal_for_a_missing_tool_is_a_finding_and_other_refusals_are_not() {
    let root = TempRoot::new();
    let store = store(&root);
    record(
        &store,
        &[
            AgentEvent::ToolRefused {
                id: "c1".into(),
                cause: RefusalCause::MissingTool,
            },
            AgentEvent::ToolRefused {
                id: "c2".into(),
                cause: RefusalCause::Hook,
            },
        ],
    );
    let conn = raw(&root);
    assert_eq!(
        count(
            &conn,
            "SELECT COUNT(*) FROM findings WHERE detector = 'missing_tool' AND tool_call_id = 'c1'"
        ),
        1
    );
    assert_eq!(count(&conn, "SELECT COUNT(*) FROM findings"), 1);
    assert_eq!(
        count(
            &conn,
            "SELECT COUNT(*) FROM tool_calls WHERE refused IS NOT NULL"
        ),
        2
    );
}

#[test]
fn a_tool_call_holds_its_name_kind_and_result_size_and_never_its_input() {
    let root = TempRoot::new();
    let store = store(&root);
    record(
        &store,
        &[
            AgentEvent::ToolCall {
                id: "c1".into(),
                tool: Some("Read".into()),
                kind: ToolKind::Read,
                input: Some(json!({"file_path": "SECRET-INPUT-MARKER"})),
            },
            AgentEvent::ToolResultSize {
                tool_call_id: "c1".into(),
                bytes: 2048,
            },
        ],
    );
    let conn = raw(&root);
    let row: (String, String, i64) = conn
        .query_row(
            "SELECT tool, kind, bytes FROM tool_calls WHERE call_id = 'c1'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(row, ("Read".to_string(), "read".to_string(), 2048));
}

#[test]
fn the_other_events_of_a_run_are_counted_by_type_and_their_body_is_not_kept() {
    let root = TempRoot::new();
    let store = store(&root);
    let other = |raw_type: &str| AgentEvent::Other {
        raw_type: raw_type.into(),
        raw: Some(json!({"secret": "SECRET-OTHER-MARKER"})),
    };
    record(&store, &[other("a"), other("a"), other("b")]);
    let conn = raw(&root);
    assert_eq!(
        count(&conn, "SELECT count FROM other_counts WHERE raw_type = 'a'"),
        2
    );
    assert_eq!(
        count(
            &conn,
            "SELECT SUM(count) FROM other_counts WHERE run = 'r1'"
        ),
        3
    );
}

#[test]
fn a_hook_call_is_stored_with_its_outcome_and_decision() {
    let root = TempRoot::new();
    let store = store(&root);
    record(
        &store,
        &[AgentEvent::HookCall {
            name: "PreToolUse:Bash".into(),
            event: "PreToolUse".into(),
            outcome: HookOutcome::Error,
            exit_code: Some(1),
            decision: None,
        }],
    );
    assert_eq!(
        count(
            &raw(&root),
            "SELECT COUNT(*) FROM hook_calls WHERE outcome = 'error' AND exit_code = 1 AND decision IS NULL"
        ),
        1
    );
}

#[test]
fn content_has_no_place_in_any_table() {
    let root = TempRoot::new();
    let store = store(&root);
    record(
        &store,
        &[
            AgentEvent::Message {
                text: "SECRET-MESSAGE-MARKER".into(),
            },
            AgentEvent::FinalReport {
                text: "SECRET-REPORT-MARKER".into(),
                source: collide_core::agent::FinalReportSource::ResultField,
            },
            AgentEvent::ToolCall {
                id: "c1".into(),
                tool: None,
                kind: ToolKind::Execute,
                input: Some(json!({"command": "SECRET-INPUT-MARKER"})),
            },
            AgentEvent::Other {
                raw_type: "x".into(),
                raw: Some(json!({"body": "SECRET-OTHER-MARKER"})),
            },
        ],
    );
    drop(store);
    let conn = raw(&root);
    let mut tables = conn
        .prepare("SELECT name FROM sqlite_master WHERE type = 'table'")
        .unwrap();
    let names: Vec<String> = tables
        .query_map([], |row| row.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    for name in names {
        let mut rows = conn.prepare(&format!("SELECT * FROM {name}")).unwrap();
        let width = rows.column_count();
        let mut found = rows.query([]).unwrap();
        while let Some(row) = found.next().unwrap() {
            for column in 0..width {
                if let Ok(text) = row.get::<_, String>(column) {
                    assert!(!text.contains("SECRET"), "{name} holds {text}");
                }
            }
        }
    }
}

#[test]
fn a_phase_end_holds_the_exit_totals_and_takes_the_workspace_size_later() {
    let root = TempRoot::new();
    let store = store(&root);
    let exit = ExitRecord {
        protocol: 1,
        end: ExitEnd::Exited { code: 3 },
        cpu_user_us: 11,
        cpu_system_us: 22,
        max_rss_bytes: 33,
    };
    store
        .add_phase_end("r1", "build", 9_000, "failed", Some(&exit))
        .unwrap();
    assert_eq!(store.set_workspace_bytes("r1", "build", 4096).unwrap(), 1);
    assert_eq!(
        store.set_workspace_bytes("r1", "other", 1).unwrap(),
        0,
        "a phase with no end row has nothing to update"
    );
    let row: (String, i64, Option<i64>, i64, i64, i64, i64) = raw(&root)
        .query_row(
            "SELECT state, exit_code, signal, cpu_user_us, cpu_system_us, max_rss_bytes, workspace_bytes FROM phase_ends",
            [],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                ))
            },
        )
        .unwrap();
    assert_eq!(row, ("failed".to_string(), 3, None, 11, 22, 33, 4096));
}

#[test]
fn a_phase_end_without_an_exit_record_has_null_totals() {
    let root = TempRoot::new();
    let store = store(&root);
    store
        .add_phase_end("r1", "build", 9_000, "lost", None)
        .unwrap();
    assert_eq!(
        count(
            &raw(&root),
            "SELECT COUNT(*) FROM phase_ends WHERE exit_code IS NULL AND signal IS NULL AND cpu_user_us IS NULL AND max_rss_bytes IS NULL AND workspace_bytes IS NULL"
        ),
        1
    );
}

#[test]
fn a_fingerprint_part_is_replaced_by_a_second_write_of_the_same_part() {
    let root = TempRoot::new();
    let store = store(&root);
    store
        .set_fingerprint_part("r1", "user_config", "{\"a\":1}")
        .unwrap();
    store
        .set_fingerprint_part("r1", "user_config", "{\"a\":2}")
        .unwrap();
    assert_eq!(
        store
            .fingerprint_part("r1", "user_config")
            .unwrap()
            .as_deref(),
        Some("{\"a\":2}")
    );
    assert_eq!(
        store.fingerprint_part("r1", "claude_version").unwrap(),
        None
    );
}

#[test]
fn a_cold_return_is_stored_with_its_cause_and_idle_time() {
    let root = TempRoot::new();
    let store = store(&root);
    store
        .add_cold_return("r1", "build", "usage_limit", 3_700_000, 7)
        .unwrap();
    let conn = raw(&root);
    let (cause, idle): (String, i64) = conn
        .query_row("SELECT cause, idle_ms FROM cold_returns", [], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })
        .unwrap();
    assert_eq!((cause.as_str(), idle), ("usage_limit", 3_700_000));
}
