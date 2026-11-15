//! The record of a file-task run (`run.record`; ts-0069-bn; 0.13, Missing is not zero): what `collide run` prints at the
//! end of a run, and when it reattaches to one. It is read from the store's metrics tables ([`crate::metrics_store`])
//! and the phase's record ([`crate::store::PhaseRecord`]); nothing here is stored again.
//!
//! **One line for each model call.** A `Turn` usage row is a turn, a `Compaction` row and an `OtherCalls` row are
//! model calls of their own, and none is summed into another ([`crate::metrics::usage_totals`]'s rule). A figure the row
//! lacks is absent in the line, so the CLI says "not reported" and never prints a zero.
//!
//! **Wall time is Collide's own.** A turn span exists for each turn the daemon sent. Usage rows are tagged with the turn
//! span that is open when the daemon polls the agent's stream, so a poll that finds several turns' rows (a session
//! that answers a `/compact` and a blocked prompt at once) tags all of them with one span. The span's length therefore
//! goes to the first `Turn` row that names it, and the others have no wall time: Collide did not send those turns and
//! has no clock of its own for them.
//!
//! **A turn cut before its `result`** (a phase stopped or killed) has no `Turn` row. Its `Step` rows, one for each
//! response the agent got to, are the tokens it did spend: their input and cache counts are summed into one `step`
//! line with `partial` quality, whose output, cost and model time are absent.

use crate::metrics::span_times;
use crate::metrics_store::{Span, SpanKind, UsageRow};
use collide_core::agent::{UsageQuality, UsageScope};
use collide_core::owner::RecordedCall;
use std::collections::{HashMap, HashSet};

fn ms(ns: Option<u64>) -> Option<u64> {
    ns.map(|ns| ns / 1_000_000)
}

/// The wall time of the run's span, in milliseconds, or `None` when the clock cannot give it.
pub fn run_wall_ms(spans: &[Span], now_ns: u64) -> Option<u64> {
    let run = spans.iter().find(|span| span.kind == SpanKind::Run)?;
    ms(span_times(run, spans, now_ns).elapsed_ns)
}

fn sum(rows: &[&UsageRow], figure: impl Fn(&UsageRow) -> Option<u64>) -> Option<u64> {
    rows.iter()
        .filter_map(|row| figure(row))
        .fold(None, |total, value| Some(total.unwrap_or(0) + value))
}

/// The model calls of a run's one phase, in the order they were recorded.
pub fn recorded_calls(spans: &[Span], rows: &[UsageRow], now_ns: u64) -> Vec<RecordedCall> {
    let turns: HashMap<i64, &Span> = spans
        .iter()
        .filter(|span| span.kind == SpanKind::Turn)
        .map(|span| (span.id, span))
        .collect();
    let wall = |id: i64| {
        turns
            .get(&id)
            .and_then(|turn| ms(span_times(turn, spans, now_ns).elapsed_ns))
    };
    // A turn span that a `Turn` row names has its tokens from `result.usage`: its steps are the same tokens again.
    let with_turn_row: HashSet<i64> = rows
        .iter()
        .filter(|row| row.usage.scope == UsageScope::Turn)
        .filter_map(|row| row.turn)
        .collect();
    // The last step of a turn that was cut, which is where its line goes.
    let mut last_step: HashMap<i64, usize> = HashMap::new();
    for (at, row) in rows.iter().enumerate() {
        if let (UsageScope::Step, Some(turn)) = (row.usage.scope, row.turn)
            && !with_turn_row.contains(&turn)
        {
            last_step.insert(turn, at);
        }
    }
    let mut walled: HashSet<i64> = HashSet::new();
    let mut calls = Vec::new();
    for (at, row) in rows.iter().enumerate() {
        let usage = &row.usage;
        match usage.scope {
            UsageScope::Turn | UsageScope::Compaction | UsageScope::OtherCalls => {
                let wall_ms = match (usage.scope, row.turn) {
                    (UsageScope::Turn, Some(turn)) if walled.insert(turn) => wall(turn),
                    _ => None,
                };
                calls.push(RecordedCall {
                    scope: usage.scope,
                    quality: usage.quality,
                    wall_ms,
                    model_ms: usage.model_time_ms,
                    input: usage.input,
                    output: usage.output,
                    cache_read: usage.cache_read,
                    cache_write: usage.cache_write,
                    cost: usage.cost.clone(),
                });
            }
            UsageScope::Step => {
                let Some(turn) = row.turn else { continue };
                if last_step.get(&turn) != Some(&at) {
                    continue;
                }
                let steps: Vec<&UsageRow> = rows
                    .iter()
                    .filter(|r| r.usage.scope == UsageScope::Step && r.turn == Some(turn))
                    .collect();
                calls.push(RecordedCall {
                    scope: UsageScope::Step,
                    quality: UsageQuality::Partial,
                    wall_ms: wall(turn),
                    model_ms: None,
                    input: sum(&steps, |r| r.usage.input),
                    output: None,
                    cache_read: sum(&steps, |r| r.usage.cache_read),
                    cache_write: sum(&steps, |r| r.usage.cache_write),
                    cost: None,
                });
            }
            UsageScope::RunningTotal => {}
        }
    }
    calls
}

#[cfg(test)]
mod tests {
    use super::*;
    use collide_core::agent::{Money, Usage};

    fn span(id: i64, kind: SpanKind, started_ns: u64, ended_ns: Option<u64>) -> Span {
        Span {
            id,
            run: "r".to_string(),
            phase: (kind != SpanKind::Run).then(|| "build".to_string()),
            parent: None,
            kind,
            wait: None,
            item: None,
            started_wall_ms: 0,
            started_ns,
            ended_wall_ms: None,
            ended_ns,
            outcome: None,
        }
    }

    fn usage(scope: UsageScope, quality: UsageQuality) -> Usage {
        Usage {
            scope,
            quality,
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
        }
    }

    fn row(turn: Option<i64>, usage: Usage) -> UsageRow {
        UsageRow {
            phase: "build".to_string(),
            turn,
            usage,
        }
    }

    fn turn_usage(model: Option<u64>, cost: Option<f64>) -> Usage {
        Usage {
            input: Some(4),
            output: Some(270),
            cache_read: Some(46_000),
            cache_write: Some(38_000),
            cost: cost.map(|amount| Money {
                amount,
                currency: "USD".to_string(),
            }),
            model_time_ms: model,
            ..usage(UsageScope::Turn, UsageQuality::Exact)
        }
    }

    const SECOND: u64 = 1_000_000_000;

    #[test]
    fn a_turns_line_has_the_spans_wall_time_and_the_rows_own_figures() {
        let spans = [span(1, SpanKind::Turn, SECOND, Some(4 * SECOND))];
        let rows = [row(Some(1), turn_usage(Some(2300), Some(0.0084)))];
        let calls = recorded_calls(&spans, &rows, 10 * SECOND);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].wall_ms, Some(3000));
        assert_eq!(calls[0].model_ms, Some(2300));
        assert_eq!(calls[0].output, Some(270));
        assert_eq!(calls[0].cost.as_ref().map(|c| c.amount), Some(0.0084));
    }

    #[test]
    fn a_figure_the_row_lacks_stays_absent_and_is_never_zero() {
        let spans = [span(1, SpanKind::Turn, 0, Some(SECOND))];
        let rows = [row(
            Some(1),
            Usage {
                input: Some(2),
                ..usage(UsageScope::Turn, UsageQuality::Partial)
            },
        )];
        let call = &recorded_calls(&spans, &rows, SECOND)[0];
        assert_eq!(
            (
                call.output,
                call.cache_read,
                call.cost.clone(),
                call.model_ms
            ),
            (None, None, None, None)
        );
        assert_eq!(call.input, Some(2));
    }

    #[test]
    fn rows_tagged_with_one_turn_span_give_its_wall_time_to_the_first_turn_row_only() {
        let spans = [span(1, SpanKind::Turn, 0, Some(5 * SECOND))];
        let rows = [
            row(Some(1), turn_usage(Some(2300), Some(0.0084))),
            row(Some(1), usage(UsageScope::Compaction, UsageQuality::Exact)),
            row(Some(1), turn_usage(None, Some(0.0))),
            row(Some(1), turn_usage(Some(8000), Some(0.005))),
        ];
        let calls = recorded_calls(&spans, &rows, 9 * SECOND);
        let walls: Vec<Option<u64>> = calls.iter().map(|c| c.wall_ms).collect();
        assert_eq!(walls, [Some(5000), None, None, None]);
        let models: Vec<Option<u64>> = calls.iter().map(|c| c.model_ms).collect();
        assert_eq!(models, [Some(2300), None, None, Some(8000)]);
        assert_eq!(calls[1].scope, UsageScope::Compaction);
    }

    #[test]
    fn a_turn_cut_before_its_result_is_one_partial_line_from_its_steps() {
        let spans = [span(1, SpanKind::Turn, 0, Some(2 * SECOND))];
        let step = |input, cache_read, cache_write| {
            row(
                Some(1),
                Usage {
                    input: Some(input),
                    cache_read: Some(cache_read),
                    cache_write: Some(cache_write),
                    ..usage(UsageScope::Step, UsageQuality::Partial)
                },
            )
        };
        let rows = [step(2, 14_000, 18_000), step(2, 32_000, 20_000)];
        let calls = recorded_calls(&spans, &rows, 3 * SECOND);
        assert_eq!(calls.len(), 1);
        let call = &calls[0];
        assert_eq!(call.scope, UsageScope::Step);
        assert_eq!(call.quality, UsageQuality::Partial);
        assert_eq!(
            (call.input, call.cache_read, call.cache_write),
            (Some(4), Some(46_000), Some(38_000))
        );
        assert_eq!(
            (call.output, call.cost.clone(), call.model_ms),
            (None, None, None)
        );
        assert_eq!(call.wall_ms, Some(2000));
    }

    #[test]
    fn the_steps_of_a_turn_that_has_a_result_are_not_counted_again() {
        let spans = [span(1, SpanKind::Turn, 0, Some(SECOND))];
        let rows = [
            row(
                Some(1),
                Usage {
                    input: Some(2),
                    ..usage(UsageScope::Step, UsageQuality::Partial)
                },
            ),
            row(Some(1), turn_usage(Some(1), Some(0.1))),
            row(
                Some(1),
                usage(UsageScope::RunningTotal, UsageQuality::Exact),
            ),
        ];
        let calls = recorded_calls(&spans, &rows, SECOND);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].scope, UsageScope::Turn);
    }

    #[test]
    fn a_span_the_clock_cannot_measure_gives_no_wall_time() {
        // Closed at a reading below its start: the continuous clock restarted with a boot.
        let spans = [span(1, SpanKind::Turn, 10 * SECOND, Some(SECOND))];
        let rows = [row(Some(1), turn_usage(Some(5), Some(0.1)))];
        assert_eq!(recorded_calls(&spans, &rows, 20 * SECOND)[0].wall_ms, None);
    }

    #[test]
    fn the_runs_wall_time_is_its_spans_length() {
        let spans = [span(1, SpanKind::Run, SECOND, Some(8 * SECOND))];
        assert_eq!(run_wall_ms(&spans, 20 * SECOND), Some(7000));
        assert_eq!(run_wall_ms(&[], 20 * SECOND), None);
    }
}
