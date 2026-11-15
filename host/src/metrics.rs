//! The always-recorded metrics (ts-0067-kw; 0.13, Metrics). The daemon records them for every run, whatever the
//! observers are set to, in the store ([`crate::metrics_store`]).
//!
//! **Spans.** A run has one span, each of its phases a span inside it, each turn of a phase a span inside that, and
//! each wait a span beside them:
//!
//! ```text
//! run
//! └─ phase
//!    ├─ turn ...
//!    └─ wait (permission | usage_limit | budget | disk | sleep) ...
//! ```
//!
//! A span's length is on the continuous clock ([`Clock::Continuous`]), which keeps counting through sleep; the wall
//! clock is kept for display. **Active time is the length of a span minus the time it spent waiting** ([`span_times`]):
//! the union of the wait spans inside it, so two waits that overlap (a permission wait that the machine slept through)
//! count once. A permission wait runs from the Queue item's push to its answer; a usage-limit, budget or disk pause
//! from the pause to the resume; a sleep is what the sampler finds ([`Metrics::sleep_interval`]).
//!
//! **A restart does not lose a span.** The open spans are in the store, and the recorder holds no state of its own: the
//! next daemon closes and opens spans by looking them up. A span the daemon was down for is as long as the clock says.
//!
//! **A reboot is not a zero.** The continuous clock restarts with a boot, so a span open across one closes at a reading
//! below its start. Its length, waiting time and active time are `None` ("not reported"), and [`time_totals`] counts it
//! among the spans that lack the value. A wait that ends in that state records no cold return.
//!
//! **Tokens and cost** ([`usage_totals`]) sum the `Turn`, `Compaction` and `OtherCalls` usage of a phase or a run and
//! never a `Step` or a `RunningTotal`, which are the same tokens counted another way. Figures from `partial` usage are
//! "at least" and are kept in their own sums, never added to the `exact` ones. A figure a row lacks is not a zero: the
//! sum counts the rows that lacked it.
//!
//! **Cold returns.** A run that waited longer than [`CACHE_TTL`] comes back to a cold prompt cache. The wait's end
//! records it, with the cause (`usage_limit` for a limit pause, `queue_wait` for the other waits on the owner).
//!
//! A failure to record is logged and never fails a run.

use crate::clock::{Clock, ClockPair, Clocks};
use crate::metrics_store::{NewSpan, Span, SpanKind, UsageRow, WaitKind};
use crate::store::Store;
use collide_core::adapter::StampedEvent;
use collide_core::agent::{AgentEvent, TurnEndReason, UsageQuality, UsageScope};
use collide_core::shim::ExitRecord;
use std::sync::Arc;
use std::time::Duration;

/// How long the prompt cache lives: a wait longer than this ends in a cold return.
pub const CACHE_TTL: Duration = Duration::from_secs(60 * 60);

/// The lengths of a span, in nanoseconds on the continuous clock. A length is `None` when the clock cannot give it:
/// the span closed (or is read) at a continuous reading earlier than its open, because the clock restarted with a
/// boot. Missing is not zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Times {
    /// From the span's start to its end, or to `now` when it is open.
    pub elapsed_ns: Option<u64>,
    /// The time inside the span that some wait covers, each moment once. `None` when the span's length is unknown or
    /// a wait that counts has an unknown length.
    pub waiting_ns: Option<u64>,
}

impl Times {
    /// What the span worked: its length less its waiting.
    pub fn active_ns(&self) -> Option<u64> {
        Some(self.elapsed_ns?.saturating_sub(self.waiting_ns?))
    }
}

/// The lengths of `span`, given every span of its run. The waits that count are the wait spans of the same phase
/// (every wait for a run's span, and a wait that belongs to no phase for any), each clipped to the span.
pub fn span_times(span: &Span, all: &[Span], now_ns: u64) -> Times {
    let start = span.started_ns;
    let end = span.ended_ns.unwrap_or(now_ns);
    if end < start {
        return Times {
            elapsed_ns: None,
            waiting_ns: None,
        };
    }
    let counted = all
        .iter()
        .filter(|other| other.kind == SpanKind::Wait && other.run == span.run)
        .filter(|other| span.phase.is_none() || other.phase.is_none() || other.phase == span.phase);
    let mut waits: Vec<(u64, u64)> = Vec::new();
    for other in counted {
        let other_end = other.ended_ns.unwrap_or(now_ns);
        if other_end < other.started_ns {
            return Times {
                elapsed_ns: Some(end - start),
                waiting_ns: None,
            };
        }
        let from = other.started_ns.max(start);
        let to = other_end.min(end);
        if from < to {
            waits.push((from, to));
        }
    }
    waits.sort_unstable();
    let mut waiting = 0;
    let mut covered_to = start;
    for (from, to) in waits {
        let from = from.max(covered_to);
        if to > from {
            waiting += to - from;
            covered_to = to;
        }
    }
    Times {
        elapsed_ns: Some(end - start),
        waiting_ns: Some(waiting),
    }
}

/// A length summed over spans. `sum` is `None` when no span gave it, and `missing` counts the spans that did not: a
/// length with `missing` above 0 is "at least" `sum`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Nanos {
    pub sum: Option<u64>,
    pub missing: u32,
}

impl Nanos {
    fn add(&mut self, value: Option<u64>) {
        match value {
            Some(value) => self.sum = Some(self.sum.unwrap_or(0) + value),
            None => self.missing += 1,
        }
    }
}

/// The lengths of every span of one kind, summed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TimeTotals {
    pub elapsed: Nanos,
    pub waiting: Nanos,
    pub active: Nanos,
}

/// Sums [`span_times`] over the spans of `kind`. A span whose length is unknown counts among those that lack it.
pub fn time_totals(kind: SpanKind, all: &[Span], now_ns: u64) -> TimeTotals {
    let mut totals = TimeTotals::default();
    for span in all.iter().filter(|span| span.kind == kind) {
        let times = span_times(span, all, now_ns);
        totals.elapsed.add(times.elapsed_ns);
        totals.waiting.add(times.waiting_ns);
        totals.active.add(times.active_ns());
    }
    totals
}

/// A token figure summed over rows. `sum` is `None` when no row reported it, and `missing` counts the rows that did
/// not: a figure with `missing` above 0 is "at least" `sum`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Tokens {
    pub sum: Option<u64>,
    pub missing: u32,
}

impl Tokens {
    fn add(&mut self, value: Option<u64>) {
        match value {
            Some(value) => self.sum = Some(self.sum.unwrap_or(0) + value),
            None => self.missing += 1,
        }
    }
}

/// A cost in US dollars summed over rows. A cost in another currency is not added to it and counts as missing.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Cost {
    pub sum: Option<f64>,
    pub missing: u32,
}

/// The rows of one quality, summed.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Sums {
    pub rows: u32,
    pub input: Tokens,
    pub output: Tokens,
    pub cache_read: Tokens,
    pub cache_write: Tokens,
    pub thought: Tokens,
    pub cost: Cost,
}

impl Sums {
    fn add(&mut self, row: &UsageRow) {
        let usage = &row.usage;
        self.rows += 1;
        self.input.add(usage.input);
        self.output.add(usage.output);
        self.cache_read.add(usage.cache_read);
        self.cache_write.add(usage.cache_write);
        self.thought.add(usage.thought);
        match &usage.cost {
            Some(cost) if cost.currency == "USD" => {
                self.cost.sum = Some(self.cost.sum.unwrap_or(0.0) + cost.amount);
            }
            _ => self.cost.missing += 1,
        }
    }
}

/// What a phase or a run spent. `exact` and `partial` are separate sums: a total that has `partial` rows is shown as
/// "at least", and its figures are never added to the exact ones.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Totals {
    pub exact: Sums,
    pub partial: Sums,
    /// Rows the agent gave no figures for.
    pub unreported_rows: u32,
}

impl Totals {
    /// Whether every row counted is exact, so the sums are the spend and not a floor.
    pub fn is_exact(&self) -> bool {
        self.partial.rows == 0 && self.unreported_rows == 0
    }
}

/// Sums the rows that are a model call of their own: a `Turn`, a `Compaction` and `OtherCalls`. A `Step` is a part of
/// a turn and a `RunningTotal` is all the turns so far, so adding either would count tokens twice.
pub fn usage_totals(rows: &[UsageRow]) -> Totals {
    let mut totals = Totals::default();
    for row in rows {
        if !matches!(
            row.usage.scope,
            UsageScope::Turn | UsageScope::Compaction | UsageScope::OtherCalls
        ) {
            continue;
        }
        match row.usage.quality {
            UsageQuality::Exact => totals.exact.add(row),
            UsageQuality::Partial => totals.partial.add(row),
            UsageQuality::Unreported => totals.unreported_rows += 1,
        }
    }
    totals
}

/// The recorder: the daemon's one place that opens and closes spans and records what an agent's events hold.
pub struct Metrics {
    store: Arc<Store>,
    clocks: Arc<dyn Clocks + Send + Sync>,
    log: Arc<dyn Fn(&str) + Send + Sync>,
}

impl Metrics {
    pub fn new(
        store: Arc<Store>,
        clocks: Arc<dyn Clocks + Send + Sync>,
        log: Arc<dyn Fn(&str) + Send + Sync>,
    ) -> Metrics {
        Metrics { store, clocks, log }
    }

    fn wall_ms(&self) -> i64 {
        (self.clocks.read(Clock::Wall).nanos / 1_000_000) as i64
    }

    fn ns(&self) -> u64 {
        self.clocks.read(Clock::Continuous).nanos
    }

    /// Logs the failure of `what` and goes on: a metric is never worth a run.
    fn keep<T>(&self, what: &str, result: Result<T, crate::store::StoreError>) -> Option<T> {
        match result {
            Ok(value) => Some(value),
            Err(error) => {
                (self.log)(&format!("metrics: {what} was not recorded: {error}"));
                None
            }
        }
    }

    fn open_spans(&self, run: &str) -> Vec<Span> {
        self.keep("reading the open spans", self.store.open_spans(run))
            .unwrap_or_default()
    }

    fn close(&self, span: &Span, outcome: Option<&str>) {
        let closed = self
            .store
            .close_span(span.id, self.wall_ms(), self.ns(), outcome);
        self.keep("closing a span", closed);
    }

    fn open(&self, span: NewSpan) -> Option<i64> {
        self.keep("opening a span", self.store.open_span(&span))
    }

    /// The run began. A run that already has an open span (a daemon that restarted) goes on with it.
    pub fn run_started(&self, run: &str) {
        let open = self.open_spans(run);
        if open.iter().any(|span| span.kind == SpanKind::Run) {
            return;
        }
        self.open(NewSpan {
            run,
            phase: None,
            parent: None,
            kind: SpanKind::Run,
            wait: None,
            item: None,
            wall_ms: self.wall_ms(),
            ns: self.ns(),
        });
    }

    /// The run is over: every span it still has open ends with it.
    pub fn run_ended(&self, run: &str, outcome: &str) {
        for span in self.open_spans(run) {
            let outcome = (span.kind == SpanKind::Run).then_some(outcome);
            self.close(&span, outcome);
        }
    }

    /// The phase is running, for the first time or again after a pause. A phase that is already running keeps its span.
    /// The waits of a pause end here.
    pub fn phase_running(&self, run: &str, phase: &str) {
        let open = self.open_spans(run);
        for wait in open.iter().filter(|span| {
            span.kind == SpanKind::Wait
                && span.phase.as_deref() == Some(phase)
                && matches!(
                    span.wait,
                    Some(WaitKind::UsageLimit | WaitKind::Budget | WaitKind::Disk)
                )
        }) {
            self.end_wait(wait, phase);
        }
        if open
            .iter()
            .any(|span| span.kind == SpanKind::Phase && span.phase.as_deref() == Some(phase))
        {
            return;
        }
        let parent = open
            .iter()
            .find(|span| span.kind == SpanKind::Run)
            .map(|span| span.id);
        self.open(NewSpan {
            run,
            phase: Some(phase),
            parent,
            kind: SpanKind::Phase,
            wait: None,
            item: None,
            wall_ms: self.wall_ms(),
            ns: self.ns(),
        });
    }

    /// The phase reached a final state: its turn, its waits and its span end.
    pub fn phase_ended(&self, run: &str, phase: &str, state: &str) {
        for span in self.open_spans(run) {
            if span.phase.as_deref() != Some(phase) {
                continue;
            }
            match span.kind {
                SpanKind::Phase => self.close(&span, Some(state)),
                SpanKind::Turn => self.close(&span, Some("cut")),
                SpanKind::Wait => self.end_wait(&span, phase),
                SpanKind::Run => {}
            }
        }
    }

    /// A turn was sent to the phase's session. A turn the last one left open is cut.
    pub fn turn_started(&self, run: &str, phase: &str) {
        let open = self.open_spans(run);
        for stale in open
            .iter()
            .filter(|span| span.kind == SpanKind::Turn && span.phase.as_deref() == Some(phase))
        {
            self.close(stale, Some("cut"));
        }
        let parent = open
            .iter()
            .find(|span| span.kind == SpanKind::Phase && span.phase.as_deref() == Some(phase))
            .map(|span| span.id);
        self.open(NewSpan {
            run,
            phase: Some(phase),
            parent,
            kind: SpanKind::Turn,
            wait: None,
            item: None,
            wall_ms: self.wall_ms(),
            ns: self.ns(),
        });
    }

    /// The run began to wait. `item` is the Queue item the wait is for.
    pub fn wait_started(&self, run: &str, phase: &str, kind: WaitKind, item: Option<&str>) {
        let open = self.open_spans(run);
        let parent = open
            .iter()
            .find(|span| span.kind == SpanKind::Phase && span.phase.as_deref() == Some(phase))
            .map(|span| span.id);
        self.open(NewSpan {
            run,
            phase: Some(phase),
            parent,
            kind: SpanKind::Wait,
            wait: Some(kind),
            item,
            wall_ms: self.wall_ms(),
            ns: self.ns(),
        });
    }

    /// The wait for `item` is over (a permission answered). A wait that is not open is not an error.
    pub fn wait_ended_for_item(&self, run: &str, phase: &str, item: &str) {
        for span in self.open_spans(run) {
            if span.kind == SpanKind::Wait
                && span.phase.as_deref() == Some(phase)
                && span.item.as_deref() == Some(item)
            {
                self.end_wait(&span, phase);
            }
        }
    }

    /// Ends a wait span, and records the cold return when it was longer than the cache lives.
    fn end_wait(&self, wait: &Span, phase: &str) {
        self.close(wait, None);
        // A reading before the wait's start means the clock restarted with a boot: the idle time is unknown, and a
        // reboot is not a cold return here.
        let Some(idle) = self
            .ns()
            .checked_sub(wait.started_ns)
            .map(Duration::from_nanos)
        else {
            return;
        };
        let cause = match wait.wait {
            Some(WaitKind::UsageLimit) => "usage_limit",
            Some(WaitKind::Permission | WaitKind::Budget | WaitKind::Disk) => "queue_wait",
            Some(WaitKind::Sleep) | None => return,
        };
        if idle > CACHE_TTL {
            let recorded = self.store.add_cold_return(
                &wait.run,
                phase,
                cause,
                idle.as_millis() as i64,
                self.wall_ms(),
            );
            self.keep("a cold return", recorded);
        }
    }

    /// The sampler found that the machine slept for `length` up to `now`: a sleep wait, recorded for every phase that
    /// is open. A span that began during the sleep is clipped to its own start when the times are read.
    pub fn sleep_interval(&self, length: Duration, now: &ClockPair) {
        let ended_ns = now.continuous_nanos();
        let started_ns = ended_ns.saturating_sub(length.as_nanos() as u64);
        let ended_wall_ms = self.wall_ms();
        let started_wall_ms = ended_wall_ms - length.as_millis() as i64;
        let runs = self
            .keep("reading the open spans", self.store.runs_with_open_spans())
            .unwrap_or_default();
        for run in runs {
            for phase in self
                .open_spans(&run)
                .iter()
                .filter(|span| span.kind == SpanKind::Phase)
            {
                let sleep = NewSpan {
                    run: &run,
                    phase: phase.phase.as_deref(),
                    parent: Some(phase.id),
                    kind: SpanKind::Wait,
                    wait: Some(WaitKind::Sleep),
                    item: None,
                    wall_ms: started_wall_ms,
                    ns: started_ns,
                };
                let added = self.store.add_closed_span(&sleep, ended_wall_ms, ended_ns);
                self.keep("a sleep", added);
            }
        }
    }

    /// Records what the events of one poll hold. A `TurnEnd` among them ends the turn span, after the rest are
    /// recorded, so the usage that comes with the turn's end is the turn's.
    pub fn events(&self, run: &str, phase: &str, events: &[StampedEvent]) {
        if events.is_empty() {
            return;
        }
        let now = self.wall_ms();
        let timed: Vec<(i64, &AgentEvent)> = events
            .iter()
            .map(|stamped| {
                let ms = stamped.wall_ns.map_or(now, |ns| (ns / 1_000_000) as i64);
                (ms, &stamped.event)
            })
            .collect();
        self.keep(
            "an agent's events",
            self.store.record_agent_events(run, phase, &timed),
        );
        for stamped in events {
            if let AgentEvent::TurnEnd { reason, .. } = &stamped.event {
                for turn in self.open_spans(run).iter().filter(|span| {
                    span.kind == SpanKind::Turn && span.phase.as_deref() == Some(phase)
                }) {
                    self.close(turn, Some(&reason_text(reason)));
                }
            }
        }
    }

    /// How a phase attempt ended, with the shim's exit totals.
    pub fn phase_exit(&self, run: &str, phase: &str, state: &str, exit: Option<&ExitRecord>) {
        let written = self
            .store
            .add_phase_end(run, phase, self.wall_ms(), state, exit);
        self.keep("a phase's end", written);
    }

    /// The workspace's size when the phase ended.
    pub fn workspace_bytes(&self, run: &str, phase: &str, bytes: u64) {
        let updated = self.keep(
            "a workspace's size",
            self.store.set_workspace_bytes(run, phase, bytes),
        );
        if updated == Some(0) {
            (self.log)(&format!(
                "metrics: the size of {run}/{phase}'s workspace has no phase end to go in"
            ));
        }
    }

    /// One part of the run's fingerprint.
    pub fn fingerprint_part(&self, run: &str, part: &str, json: &str) {
        self.keep(
            "a fingerprint",
            self.store.set_fingerprint_part(run, part, json),
        );
    }
}

fn reason_text(reason: &TurnEndReason) -> String {
    match reason {
        TurnEndReason::EndTurn {} => "end_turn".to_string(),
        TurnEndReason::MaxTokens {} => "max_tokens".to_string(),
        TurnEndReason::MaxTurns {} => "max_turns".to_string(),
        TurnEndReason::Budget {} => "budget".to_string(),
        TurnEndReason::Refusal {} => "refusal".to_string(),
        TurnEndReason::Cancelled {} => "cancelled".to_string(),
        TurnEndReason::Error {
            status: Some(status),
        } => format!("error_{status}"),
        TurnEndReason::Error { status: None } => "error".to_string(),
        TurnEndReason::Blocked { .. } => "blocked".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics_store::UsageRow;
    use collide_core::agent::{Money, Usage};

    const SECOND: u64 = 1_000_000_000;

    fn span(kind: SpanKind, phase: Option<&str>, start: u64, end: Option<u64>) -> Span {
        Span {
            id: 0,
            run: "r".into(),
            phase: phase.map(str::to_string),
            parent: None,
            kind,
            wait: (kind == SpanKind::Wait).then_some(WaitKind::Permission),
            item: None,
            started_wall_ms: 0,
            started_ns: start * SECOND,
            ended_wall_ms: None,
            ended_ns: end.map(|e| e * SECOND),
            outcome: None,
        }
    }

    fn wait(phase: &str, start: u64, end: u64) -> Span {
        span(SpanKind::Wait, Some(phase), start, Some(end))
    }

    #[test]
    fn a_span_with_no_waits_is_all_active() {
        let phase = span(SpanKind::Phase, Some("p"), 10, Some(30));
        let times = span_times(&phase, std::slice::from_ref(&phase), 99 * SECOND);
        assert_eq!(
            (times.elapsed_ns, times.waiting_ns, times.active_ns()),
            (Some(20 * SECOND), Some(0), Some(20 * SECOND))
        );
    }

    #[test]
    fn a_wait_inside_the_span_is_taken_off_its_active_time() {
        let phase = span(SpanKind::Phase, Some("p"), 0, Some(100));
        let times = span_times(&phase, &[phase.clone(), wait("p", 10, 40)], 0);
        assert_eq!(times.waiting_ns, Some(30 * SECOND));
        assert_eq!(times.active_ns(), Some(70 * SECOND));
    }

    #[test]
    fn two_waits_that_overlap_count_once() {
        let phase = span(SpanKind::Phase, Some("p"), 0, Some(100));
        let all = [phase.clone(), wait("p", 10, 40), wait("p", 30, 60)];
        assert_eq!(span_times(&phase, &all, 0).waiting_ns, Some(50 * SECOND));
    }

    #[test]
    fn a_wait_that_began_before_the_span_is_clipped_to_it() {
        let turn = span(SpanKind::Turn, Some("p"), 50, Some(80));
        let all = [turn.clone(), wait("p", 0, 60)];
        assert_eq!(span_times(&turn, &all, 0).waiting_ns, Some(10 * SECOND));
    }

    #[test]
    fn a_wait_of_another_phase_does_not_count() {
        let phase = span(SpanKind::Phase, Some("p"), 0, Some(100));
        let all = [phase.clone(), wait("other", 10, 40)];
        assert_eq!(span_times(&phase, &all, 0).waiting_ns, Some(0));
    }

    #[test]
    fn the_run_counts_the_waits_of_every_phase() {
        let run = span(SpanKind::Run, None, 0, Some(100));
        let all = [run.clone(), wait("a", 10, 20), wait("b", 30, 50)];
        assert_eq!(span_times(&run, &all, 0).waiting_ns, Some(30 * SECOND));
    }

    #[test]
    fn an_open_span_and_an_open_wait_run_to_now() {
        let phase = span(SpanKind::Phase, Some("p"), 0, None);
        let open_wait = span(SpanKind::Wait, Some("p"), 40, None);
        let times = span_times(&phase, &[phase.clone(), open_wait], 100 * SECOND);
        assert_eq!(
            (times.elapsed_ns, times.waiting_ns),
            (Some(100 * SECOND), Some(60 * SECOND))
        );
    }

    #[test]
    fn a_clock_that_reads_before_the_start_gives_no_length_and_not_a_zero() {
        let phase = span(SpanKind::Phase, Some("p"), 50, None);
        let times = span_times(&phase, std::slice::from_ref(&phase), 10 * SECOND);
        assert_eq!(
            (times.elapsed_ns, times.waiting_ns, times.active_ns()),
            (None, None, None)
        );
    }

    #[test]
    fn a_span_that_closed_before_it_opened_is_not_reported_and_a_total_counts_it_as_missing() {
        // The first phase was open across a reboot: the continuous clock restarted and it closed at a lower reading.
        let mut rebooted = span(SpanKind::Phase, Some("p1"), 500, Some(20));
        rebooted.id = 1;
        let mut fine = span(SpanKind::Phase, Some("p2"), 30, Some(50));
        fine.id = 2;
        let all = [rebooted.clone(), fine];
        assert_eq!(span_times(&rebooted, &all, 99 * SECOND).elapsed_ns, None);
        let totals = time_totals(SpanKind::Phase, &all, 99 * SECOND);
        assert_eq!(
            (totals.elapsed.sum, totals.elapsed.missing),
            (Some(20 * SECOND), 1)
        );
        assert_eq!(
            (totals.active.sum, totals.active.missing),
            (Some(20 * SECOND), 1)
        );
    }

    #[test]
    fn a_wait_of_unknown_length_leaves_the_waiting_and_active_time_unknown() {
        let phase = span(SpanKind::Phase, Some("p"), 0, Some(100));
        let rebooted = span(SpanKind::Wait, Some("p"), 500, Some(20));
        let times = span_times(&phase, &[phase.clone(), rebooted], 0);
        assert_eq!(
            (times.elapsed_ns, times.waiting_ns, times.active_ns()),
            (Some(100 * SECOND), None, None)
        );
    }

    fn row(
        scope: UsageScope,
        quality: UsageQuality,
        output: Option<u64>,
        cost: Option<f64>,
    ) -> UsageRow {
        UsageRow {
            phase: "p".into(),
            turn: None,
            usage: Usage {
                scope,
                input: None,
                output,
                cache_read: None,
                cache_write: None,
                cache_write_1h: None,
                cache_write_5m: None,
                thought: None,
                context_used: None,
                context_size: None,
                cost: cost.map(|amount| Money {
                    amount,
                    currency: "USD".into(),
                }),
                model_time_ms: None,
                quality,
            },
        }
    }

    #[test]
    fn totals_sum_a_turn_a_compaction_and_other_calls() {
        let rows = [
            row(UsageScope::Turn, UsageQuality::Exact, Some(10), Some(0.5)),
            row(
                UsageScope::Compaction,
                UsageQuality::Exact,
                Some(20),
                Some(0.25),
            ),
            row(
                UsageScope::OtherCalls,
                UsageQuality::Exact,
                Some(30),
                Some(0.125),
            ),
        ];
        let totals = usage_totals(&rows);
        assert_eq!(totals.exact.output.sum, Some(60));
        assert_eq!(totals.exact.cost.sum, Some(0.875));
        assert!(totals.is_exact());
    }

    #[test]
    fn totals_never_add_a_step_or_a_running_total() {
        let rows = [
            row(UsageScope::Step, UsageQuality::Exact, Some(7), Some(1.0)),
            row(
                UsageScope::RunningTotal,
                UsageQuality::Exact,
                Some(9),
                Some(2.0),
            ),
            row(UsageScope::Turn, UsageQuality::Exact, Some(10), Some(0.5)),
        ];
        let totals = usage_totals(&rows);
        assert_eq!((totals.exact.rows, totals.exact.output.sum), (1, Some(10)));
        assert_eq!(totals.exact.cost.sum, Some(0.5));
    }

    #[test]
    fn partial_rows_are_their_own_sum_and_never_added_to_the_exact_ones() {
        let rows = [
            row(UsageScope::Turn, UsageQuality::Exact, Some(10), None),
            row(UsageScope::Turn, UsageQuality::Partial, Some(5), None),
        ];
        let totals = usage_totals(&rows);
        assert_eq!(totals.exact.output.sum, Some(10));
        assert_eq!(totals.partial.output.sum, Some(5));
        assert!(!totals.is_exact());
    }

    #[test]
    fn an_unreported_row_is_counted_and_adds_no_figure() {
        let rows = [row(UsageScope::Turn, UsageQuality::Unreported, None, None)];
        let totals = usage_totals(&rows);
        assert_eq!(totals.unreported_rows, 1);
        assert_eq!(totals.exact.rows + totals.partial.rows, 0);
        assert_eq!(totals.exact.output.sum, None);
    }

    #[test]
    fn a_figure_that_no_row_reported_is_none_and_not_zero_and_a_partly_missing_one_says_how_many() {
        let rows = [
            row(UsageScope::Turn, UsageQuality::Exact, Some(4), None),
            row(UsageScope::Turn, UsageQuality::Exact, None, Some(0.5)),
        ];
        let totals = usage_totals(&rows);
        assert_eq!(
            totals.exact.input,
            Tokens {
                sum: None,
                missing: 2
            }
        );
        assert_eq!(
            totals.exact.output,
            Tokens {
                sum: Some(4),
                missing: 1
            }
        );
        assert_eq!(
            totals.exact.cost,
            Cost {
                sum: Some(0.5),
                missing: 1
            }
        );
    }

    #[test]
    fn a_cost_in_another_currency_is_not_added_to_dollars() {
        let mut euros = row(UsageScope::Turn, UsageQuality::Exact, None, Some(3.0));
        euros.usage.cost.as_mut().unwrap().currency = "EUR".into();
        let rows = [
            row(UsageScope::Turn, UsageQuality::Exact, None, Some(1.0)),
            euros,
        ];
        let totals = usage_totals(&rows);
        assert_eq!(
            totals.exact.cost,
            Cost {
                sum: Some(1.0),
                missing: 1
            }
        );
    }

    #[test]
    fn a_turn_end_reason_has_a_name_with_the_status_of_an_error() {
        assert_eq!(reason_text(&TurnEndReason::EndTurn {}), "end_turn");
        assert_eq!(
            reason_text(&TurnEndReason::Error { status: Some(429) }),
            "error_429"
        );
        assert_eq!(reason_text(&TurnEndReason::Error { status: None }), "error");
    }
}
