//! The always-recorded metrics in the store (ts-0067-kw; 0.13, Metrics). The tables are migration 4 of the store
//! ([`crate::store::MIGRATIONS`]); this file is the writes and reads on them. What is recorded and when is
//! [`crate::metrics`]'s; this is the SQL.
//!
//! - **Missing is NULL, never 0.** Every figure of a usage row is a nullable column, and a row reads back as the
//!   same [`Usage`] it was written from.
//! - **A bounded whitelist, no content.** An agent event is recorded as the few figures and names the tables hold
//!   ([`Store::record_agent_events`]); a message, a tool's input, a tool's result, a hook's output, a report's text and
//!   an `Other` event's body have no place in a table.
//! - **One connection and one transaction for a batch of events**, so a poll that finds ten events writes once.

use crate::store::{Store, StoreError, sql_error};
use collide_core::agent::{
    AgentEvent, CompactionTrigger, HookDecision, HookFailure, HookOutcome, Money, RefusalCause,
    ToolKind, Usage, UsageQuality, UsageScope,
};
use collide_core::shim::{ExitEnd, ExitRecord};
use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};

/// What a span is the length of.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpanKind {
    Run,
    Phase,
    Turn,
    /// Time the run waited rather than worked: see [`WaitKind`].
    Wait,
}

impl SpanKind {
    pub fn as_str(self) -> &'static str {
        match self {
            SpanKind::Run => "run",
            SpanKind::Phase => "phase",
            SpanKind::Turn => "turn",
            SpanKind::Wait => "wait",
        }
    }

    fn parse(text: &str) -> Option<SpanKind> {
        [
            SpanKind::Run,
            SpanKind::Phase,
            SpanKind::Turn,
            SpanKind::Wait,
        ]
        .into_iter()
        .find(|kind| kind.as_str() == text)
    }
}

/// Why a run waited. A wait span is not active time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaitKind {
    /// The agent asked for a permission and the Queue item is not answered.
    Permission,
    /// The account's usage limit paused the run until it resets.
    UsageLimit,
    /// The run's budget paused it.
    Budget,
    /// The disk was too full to go on.
    Disk,
    /// The machine slept: the sampler found a gap over its threshold.
    Sleep,
}

impl WaitKind {
    pub fn as_str(self) -> &'static str {
        match self {
            WaitKind::Permission => "permission",
            WaitKind::UsageLimit => "usage_limit",
            WaitKind::Budget => "budget",
            WaitKind::Disk => "disk",
            WaitKind::Sleep => "sleep",
        }
    }

    fn parse(text: &str) -> Option<WaitKind> {
        [
            WaitKind::Permission,
            WaitKind::UsageLimit,
            WaitKind::Budget,
            WaitKind::Disk,
            WaitKind::Sleep,
        ]
        .into_iter()
        .find(|kind| kind.as_str() == text)
    }
}

/// A span to open.
#[derive(Debug, Clone, Copy)]
pub struct NewSpan<'a> {
    pub run: &'a str,
    pub phase: Option<&'a str>,
    pub parent: Option<i64>,
    pub kind: SpanKind,
    pub wait: Option<WaitKind>,
    /// The Queue item the wait is for, when it is for one.
    pub item: Option<&'a str>,
    /// The wall clock, in milliseconds since the Unix epoch: for display.
    pub wall_ms: i64,
    /// The continuous clock, in nanoseconds: what lengths are measured on.
    pub ns: u64,
}

/// One span as stored. An open span has no `ended_ns`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    pub id: i64,
    pub run: String,
    pub phase: Option<String>,
    pub parent: Option<i64>,
    pub kind: SpanKind,
    pub wait: Option<WaitKind>,
    pub item: Option<String>,
    pub started_wall_ms: i64,
    pub started_ns: u64,
    pub ended_wall_ms: Option<i64>,
    pub ended_ns: Option<u64>,
    pub outcome: Option<String>,
}

/// One usage event as stored, with the turn span it came in.
#[derive(Debug, Clone, PartialEq)]
pub struct UsageRow {
    pub phase: String,
    pub turn: Option<i64>,
    pub usage: Usage,
}

/// An agent event with the wall time it is recorded at, in milliseconds.
pub type TimedEvent<'a> = (i64, &'a AgentEvent);

const SPAN_COLUMNS: &str = "id, run, phase, parent, kind, wait, item, started_wall_ms, started_ns, \
                            ended_wall_ms, ended_ns, outcome";

impl Store {
    /// Opens a span and returns its id.
    pub fn open_span(&self, span: &NewSpan) -> Result<i64, StoreError> {
        let conn = self.connect()?;
        conn.execute(
            "INSERT INTO spans (run, phase, parent, kind, wait, item, started_wall_ms, started_ns)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                span.run,
                span.phase,
                span.parent,
                span.kind.as_str(),
                span.wait.map(WaitKind::as_str),
                span.item,
                span.wall_ms,
                span.ns as i64
            ],
        )
        .map_err(|e| sql_error("opening a span", e))?;
        Ok(conn.last_insert_rowid())
    }

    /// Writes a closed span: one that is known whole, as a sleep is when the sampler finds it.
    pub fn add_closed_span(
        &self,
        span: &NewSpan,
        ended_wall_ms: i64,
        ended_ns: u64,
    ) -> Result<i64, StoreError> {
        let id = self.open_span(span)?;
        self.close_span(id, ended_wall_ms, ended_ns, None)?;
        Ok(id)
    }

    /// Ends a span. A span that is already ended keeps its end.
    pub fn close_span(
        &self,
        id: i64,
        wall_ms: i64,
        ns: u64,
        outcome: Option<&str>,
    ) -> Result<(), StoreError> {
        self.connect()?
            .execute(
                "UPDATE spans SET ended_wall_ms = ?2, ended_ns = ?3, outcome = ?4
                 WHERE id = ?1 AND ended_ns IS NULL",
                params![id, wall_ms, ns as i64, outcome],
            )
            .map_err(|e| sql_error("closing a span", e))?;
        Ok(())
    }

    /// Every span of the run, in the order they were opened.
    pub fn spans(&self, run: &str) -> Result<Vec<Span>, StoreError> {
        self.read_spans(
            &format!("SELECT {SPAN_COLUMNS} FROM spans WHERE run = ?1 ORDER BY id"),
            params![run],
        )
    }

    /// The run's phase spans, in the order they were opened: when each phase began and ended, for a list of runs.
    pub fn phase_spans(&self, run: &str) -> Result<Vec<Span>, StoreError> {
        self.read_spans(
            &format!(
                "SELECT {SPAN_COLUMNS} FROM spans WHERE run = ?1 AND kind = 'phase' ORDER BY id"
            ),
            params![run],
        )
    }

    /// The spans of the run that have no end yet.
    pub fn open_spans(&self, run: &str) -> Result<Vec<Span>, StoreError> {
        self.read_spans(
            &format!(
                "SELECT {SPAN_COLUMNS} FROM spans WHERE run = ?1 AND ended_ns IS NULL ORDER BY id"
            ),
            params![run],
        )
    }

    /// The runs that have a span with no end: what a restarted daemon picks up.
    pub fn runs_with_open_spans(&self) -> Result<Vec<String>, StoreError> {
        let conn = self.connect()?;
        let mut statement = conn
            .prepare("SELECT DISTINCT run FROM spans WHERE ended_ns IS NULL ORDER BY run")
            .map_err(|e| sql_error("reading the open spans", e))?;
        let rows = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|e| sql_error("reading the open spans", e))?;
        rows.collect::<Result<_, _>>()
            .map_err(|e| sql_error("reading the open spans", e))
    }

    fn read_spans(
        &self,
        sql: &str,
        params: impl rusqlite::Params,
    ) -> Result<Vec<Span>, StoreError> {
        let conn = self.connect()?;
        let mut statement = conn
            .prepare(sql)
            .map_err(|e| sql_error("reading spans", e))?;
        let rows = statement
            .query_map(params, |row| {
                let kind: String = row.get(4)?;
                let wait: Option<String> = row.get(5)?;
                Ok(Span {
                    id: row.get(0)?,
                    run: row.get(1)?,
                    phase: row.get(2)?,
                    parent: row.get(3)?,
                    kind: SpanKind::parse(&kind).unwrap_or(SpanKind::Wait),
                    wait: wait.as_deref().and_then(WaitKind::parse),
                    item: row.get(6)?,
                    started_wall_ms: row.get(7)?,
                    started_ns: row.get::<_, i64>(8)? as u64,
                    ended_wall_ms: row.get(9)?,
                    ended_ns: row.get::<_, Option<i64>>(10)?.map(|ns| ns as u64),
                    outcome: row.get(11)?,
                })
            })
            .map_err(|e| sql_error("reading spans", e))?;
        rows.collect::<Result<_, _>>()
            .map_err(|e| sql_error("reading spans", e))
    }

    /// The usage events of a run, or of one of its phases, in the order they were recorded.
    pub fn usage_rows(&self, run: &str, phase: Option<&str>) -> Result<Vec<UsageRow>, StoreError> {
        let conn = self.connect()?;
        let mut statement = conn
            .prepare(
                "SELECT phase, turn, scope, quality, input, output, cache_read, cache_write, cache_write_1h,
                        cache_write_5m, thought, context_used, context_size, cost_amount, cost_currency,
                        model_time_ms
                 FROM usage_rows WHERE run = ?1 AND (?2 IS NULL OR phase = ?2) ORDER BY id",
            )
            .map_err(|e| sql_error("reading usage", e))?;
        let rows = statement
            .query_map(params![run, phase], |row| {
                let scope: String = row.get(2)?;
                let quality: String = row.get(3)?;
                let amount: Option<f64> = row.get(13)?;
                let currency: Option<String> = row.get(14)?;
                let figure = |at: usize| row.get::<_, Option<i64>>(at).map(|v| v.map(|v| v as u64));
                Ok(UsageRow {
                    phase: row.get(0)?,
                    turn: row.get(1)?,
                    usage: Usage {
                        scope: parse_scope(&scope),
                        quality: parse_quality(&quality),
                        input: figure(4)?,
                        output: figure(5)?,
                        cache_read: figure(6)?,
                        cache_write: figure(7)?,
                        cache_write_1h: figure(8)?,
                        cache_write_5m: figure(9)?,
                        thought: figure(10)?,
                        context_used: figure(11)?,
                        context_size: figure(12)?,
                        cost: amount
                            .zip(currency)
                            .map(|(amount, currency)| Money { amount, currency }),
                        model_time_ms: figure(15)?,
                    },
                })
            })
            .map_err(|e| sql_error("reading usage", e))?;
        rows.collect::<Result<_, _>>()
            .map_err(|e| sql_error("reading usage", e))
    }

    /// Records what the whitelist keeps of `events`, which came in `phase` of `run`, in one transaction. Each event
    /// carries the wall time it is recorded at.
    pub fn record_agent_events(
        &self,
        run: &str,
        phase: &str,
        events: &[TimedEvent],
    ) -> Result<(), StoreError> {
        let mut conn = self.connect()?;
        // Immediate: a deferred transaction that reads before it writes is refused at once, with no wait for the busy
        // timeout, when another writer committed in between, and the batch would be lost.
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|e| sql_error("recording events", e))?;
        for (wall_ms, event) in events {
            record_event(&tx, run, phase, *wall_ms, event)
                .map_err(|e| sql_error("recording an event", e))?;
        }
        tx.commit().map_err(|e| sql_error("recording events", e))
    }

    /// Writes how a phase attempt ended: its state, and the shim's exit totals when it gave them.
    pub fn add_phase_end(
        &self,
        run: &str,
        phase: &str,
        wall_ms: i64,
        state: &str,
        exit: Option<&ExitRecord>,
    ) -> Result<(), StoreError> {
        let (code, signal) = match exit.map(|e| e.end) {
            Some(ExitEnd::Exited { code }) => (Some(code), None),
            Some(ExitEnd::Signaled { signal }) => (None, Some(signal)),
            None => (None, None),
        };
        self.connect()?
            .execute(
                "INSERT INTO phase_ends (run, phase, wall_ms, state, exit_code, signal, cpu_user_us,
                                         cpu_system_us, max_rss_bytes)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    run,
                    phase,
                    wall_ms,
                    state,
                    code,
                    signal,
                    exit.map(|e| e.cpu_user_us as i64),
                    exit.map(|e| e.cpu_system_us as i64),
                    exit.map(|e| e.max_rss_bytes as i64)
                ],
            )
            .map_err(|e| sql_error("writing a phase end", e))?;
        Ok(())
    }

    /// Writes the workspace's size in bytes on the phase's latest end and returns how many rows it updated: 0 when the
    /// phase has no end row, so nothing was recorded.
    pub fn set_workspace_bytes(
        &self,
        run: &str,
        phase: &str,
        bytes: u64,
    ) -> Result<usize, StoreError> {
        self.connect()?
            .execute(
                "UPDATE phase_ends SET workspace_bytes = ?3
                 WHERE id = (SELECT MAX(id) FROM phase_ends WHERE run = ?1 AND phase = ?2)",
                params![run, phase, bytes as i64],
            )
            .map_err(|e| sql_error("writing a workspace size", e))
    }

    /// Writes a cold return: the run came back to the model after `idle_ms` away, longer than the cache lives.
    pub fn add_cold_return(
        &self,
        run: &str,
        phase: &str,
        cause: &str,
        idle_ms: i64,
        wall_ms: i64,
    ) -> Result<(), StoreError> {
        self.connect()?
            .execute(
                "INSERT INTO cold_returns (run, phase, cause, idle_ms, wall_ms) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![run, phase, cause, idle_ms, wall_ms],
            )
            .map_err(|e| sql_error("writing a cold return", e))?;
        Ok(())
    }

    /// Writes one part of a run's fingerprint, replacing the part it had.
    pub fn set_fingerprint_part(
        &self,
        run: &str,
        part: &str,
        json: &str,
    ) -> Result<(), StoreError> {
        self.connect()?
            .execute(
                "INSERT INTO fingerprints (run, part, json) VALUES (?1, ?2, ?3)
                 ON CONFLICT (run, part) DO UPDATE SET json = excluded.json",
                params![run, part, json],
            )
            .map_err(|e| sql_error("writing a fingerprint", e))?;
        Ok(())
    }

    /// One part of a run's fingerprint, or `None`.
    pub fn fingerprint_part(&self, run: &str, part: &str) -> Result<Option<String>, StoreError> {
        self.connect()?
            .query_row(
                "SELECT json FROM fingerprints WHERE run = ?1 AND part = ?2",
                params![run, part],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| sql_error("reading a fingerprint", e))
    }
}

fn parse_scope(text: &str) -> UsageScope {
    match text {
        "turn" => UsageScope::Turn,
        "step" => UsageScope::Step,
        "compaction" => UsageScope::Compaction,
        "other_calls" => UsageScope::OtherCalls,
        _ => UsageScope::RunningTotal,
    }
}

fn scope_text(scope: UsageScope) -> &'static str {
    match scope {
        UsageScope::Turn => "turn",
        UsageScope::Step => "step",
        UsageScope::RunningTotal => "running_total",
        UsageScope::Compaction => "compaction",
        UsageScope::OtherCalls => "other_calls",
    }
}

fn parse_quality(text: &str) -> UsageQuality {
    match text {
        "exact" => UsageQuality::Exact,
        "partial" => UsageQuality::Partial,
        _ => UsageQuality::Unreported,
    }
}

fn quality_text(quality: UsageQuality) -> &'static str {
    match quality {
        UsageQuality::Exact => "exact",
        UsageQuality::Partial => "partial",
        UsageQuality::Unreported => "unreported",
    }
}

/// The open turn span of the phase, if there is one.
fn open_turn(tx: &Transaction, run: &str, phase: &str) -> rusqlite::Result<Option<i64>> {
    tx.query_row(
        "SELECT id FROM spans WHERE run = ?1 AND phase = ?2 AND kind = 'turn' AND ended_ns IS NULL
         ORDER BY id DESC LIMIT 1",
        params![run, phase],
        |row| row.get(0),
    )
    .optional()
}

/// The cost the turn's own usage gave, if it has given one: what a finding of the turn is said to have cost.
fn turn_cost(tx: &Transaction, turn: Option<i64>) -> rusqlite::Result<Option<(f64, String)>> {
    let Some(turn) = turn else {
        return Ok(None);
    };
    tx.query_row(
        "SELECT cost_amount, cost_currency FROM usage_rows
         WHERE turn = ?1 AND scope = 'turn' AND cost_amount IS NOT NULL ORDER BY id DESC LIMIT 1",
        params![turn],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )
    .optional()
}

fn record_event(
    tx: &Transaction,
    run: &str,
    phase: &str,
    wall_ms: i64,
    event: &AgentEvent,
) -> rusqlite::Result<()> {
    let turn = open_turn(tx, run, phase)?;
    match event {
        AgentEvent::Usage { usage } => record_usage(tx, run, phase, turn, wall_ms, usage)?,
        AgentEvent::Compaction {
            trigger,
            context_before,
            duration_ms,
            mid_task,
        } => {
            tx.execute(
                "INSERT INTO compactions (run, phase, turn, wall_ms, trigger_kind, tokens_before, duration_ms, mid_task)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    run,
                    phase,
                    turn,
                    wall_ms,
                    trigger.map(|t| match t {
                        CompactionTrigger::Manual => "manual",
                        CompactionTrigger::Auto => "auto",
                    }),
                    context_before.map(|n| n as i64),
                    duration_ms.map(|n| n as i64),
                    mid_task
                ],
            )?;
        }
        AgentEvent::ToolCall { id, tool, kind, .. } => {
            tx.execute(
                "INSERT INTO tool_calls (run, phase, call_id, tool, kind) VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT (run, phase, call_id) DO UPDATE SET
                     tool = COALESCE(excluded.tool, tool), kind = excluded.kind",
                params![run, phase, id, tool, tool_kind_text(*kind)],
            )?;
        }
        AgentEvent::ToolResultSize {
            tool_call_id,
            bytes,
        } => {
            tx.execute(
                "INSERT INTO tool_calls (run, phase, call_id, bytes) VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT (run, phase, call_id) DO UPDATE SET bytes = excluded.bytes",
                params![run, phase, tool_call_id, *bytes as i64],
            )?;
        }
        AgentEvent::ToolRefused { id, cause } => {
            let text = refusal_text(*cause);
            tx.execute(
                "INSERT INTO tool_calls (run, phase, call_id, refused) VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT (run, phase, call_id) DO UPDATE SET refused = excluded.refused",
                params![run, phase, id, text],
            )?;
            if *cause == RefusalCause::MissingTool {
                add_finding(
                    tx,
                    run,
                    phase,
                    turn,
                    &Finding {
                        detector: "missing_tool",
                        name: None,
                        event: None,
                        tool_call_id: Some(id),
                        cause: text,
                    },
                )?;
            }
        }
        AgentEvent::HookCall {
            name,
            event,
            outcome,
            exit_code,
            decision,
        } => {
            tx.execute(
                "INSERT INTO hook_calls (run, phase, turn, name, event, outcome, exit_code, decision)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    run,
                    phase,
                    turn,
                    name,
                    event,
                    match outcome {
                        HookOutcome::Success => "success",
                        HookOutcome::Error => "error",
                        HookOutcome::Cancelled => "cancelled",
                    },
                    exit_code,
                    decision.map(|d| match d {
                        HookDecision::Allow => "allow",
                        HookDecision::Deny => "deny",
                        HookDecision::Ask => "ask",
                        HookDecision::Block => "block",
                    })
                ],
            )?;
        }
        AgentEvent::HookFailedOpen {
            name,
            event,
            tool_call_id,
            cause,
        } => {
            add_finding(
                tx,
                run,
                phase,
                turn,
                &Finding {
                    detector: "failed_open",
                    name: Some(name),
                    event: Some(event),
                    tool_call_id: Some(tool_call_id),
                    cause: match cause {
                        HookFailure::ExitError => "exit_error",
                        HookFailure::NoVerdict => "no_verdict",
                    },
                },
            )?;
        }
        AgentEvent::Other { raw_type, .. } => {
            tx.execute(
                "INSERT INTO other_counts (run, raw_type, count) VALUES (?1, ?2, 1)
                 ON CONFLICT (run, raw_type) DO UPDATE SET count = count + 1",
                params![run, raw_type],
            )?;
        }
        // Not recorded: a message, a permission's input, a report and the like are content, and the rest is in the
        // event log or is another table's.
        AgentEvent::Message { .. }
        | AgentEvent::PermissionRequest { .. }
        | AgentEvent::PermissionUnanswerable { .. }
        | AgentEvent::SessionBound { .. }
        | AgentEvent::TurnEnd { .. }
        | AgentEvent::FinalReport { .. }
        | AgentEvent::Limited { .. } => {}
    }
    Ok(())
}

fn record_usage(
    tx: &Transaction,
    run: &str,
    phase: &str,
    turn: Option<i64>,
    wall_ms: i64,
    usage: &Usage,
) -> rusqlite::Result<()> {
    let figure = |value: Option<u64>| value.map(|v| v as i64);
    tx.execute(
        "INSERT INTO usage_rows (run, phase, turn, wall_ms, scope, quality, input, output, cache_read,
                                 cache_write, cache_write_1h, cache_write_5m, thought, context_used,
                                 context_size, cost_amount, cost_currency, model_time_ms)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)",
        params![
            run,
            phase,
            turn,
            wall_ms,
            scope_text(usage.scope),
            quality_text(usage.quality),
            figure(usage.input),
            figure(usage.output),
            figure(usage.cache_read),
            figure(usage.cache_write),
            figure(usage.cache_write_1h),
            figure(usage.cache_write_5m),
            figure(usage.thought),
            figure(usage.context_used),
            figure(usage.context_size),
            usage.cost.as_ref().map(|c| c.amount),
            usage.cost.as_ref().map(|c| c.currency.as_str()),
            figure(usage.model_time_ms),
        ],
    )?;
    // The size after a compaction is the context of the next response, which is a step's.
    if usage.scope == UsageScope::Step
        && let Some(context) = usage.context_used
    {
        tx.execute(
            "UPDATE compactions SET tokens_after = ?3
             WHERE id = (SELECT MAX(id) FROM compactions WHERE run = ?1 AND phase = ?2 AND tokens_after IS NULL)",
            params![run, phase, context as i64],
        )?;
    }
    // A finding of the turn is given the turn's cost once the turn's own usage has it.
    if usage.scope == UsageScope::Turn
        && let (Some(turn), Some(cost)) = (turn, &usage.cost)
    {
        tx.execute(
            "UPDATE findings SET cost_amount = ?2, cost_currency = ?3 WHERE turn = ?1 AND cost_amount IS NULL",
            params![turn, cost.amount, cost.currency],
        )?;
    }
    Ok(())
}

struct Finding<'a> {
    detector: &'a str,
    name: Option<&'a str>,
    event: Option<&'a str>,
    tool_call_id: Option<&'a str>,
    cause: &'a str,
}

/// A detector's finding. Its cost is what the turn it came in cost, which is the closest figure there is: the guard
/// that failed did not run a model call of its own. It is NULL until the turn's usage gives one.
fn add_finding(
    tx: &Transaction,
    run: &str,
    phase: &str,
    turn: Option<i64>,
    finding: &Finding,
) -> rusqlite::Result<()> {
    let cost = turn_cost(tx, turn)?;
    tx.execute(
        "INSERT INTO findings (run, phase, turn, detector, name, event, tool_call_id, cause, cost_amount,
                               cost_currency, cost_basis)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 'turn')",
        params![
            run,
            phase,
            turn,
            finding.detector,
            finding.name,
            finding.event,
            finding.tool_call_id,
            finding.cause,
            cost.as_ref().map(|(amount, _)| *amount),
            cost.as_ref().map(|(_, currency)| currency.as_str()),
        ],
    )?;
    Ok(())
}

fn tool_kind_text(kind: ToolKind) -> &'static str {
    match kind {
        ToolKind::Read => "read",
        ToolKind::Edit => "edit",
        ToolKind::Delete => "delete",
        ToolKind::Move => "move",
        ToolKind::Search => "search",
        ToolKind::Execute => "execute",
        ToolKind::Think => "think",
        ToolKind::Fetch => "fetch",
        ToolKind::Other => "other",
    }
}

fn refusal_text(cause: RefusalCause) -> &'static str {
    match cause {
        RefusalCause::Hook => "hook",
        RefusalCause::Host => "host",
        RefusalCause::Rule => "rule",
        RefusalCause::MissingTool => "missing_tool",
        RefusalCause::Unknown => "unknown",
    }
}
