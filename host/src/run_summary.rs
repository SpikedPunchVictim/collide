//! A run as `run.list` and `run.get` give it (ts-0076-cw; 0.8, Run): the run's row in the store ([`crate::run_store`]),
//! its phases and the times of their spans ([`crate::metrics_store`]), and its workspace folder. The function here is
//! plain: the daemon reads the parts and this puts them together, so a test hands it the parts.
//!
//! - **A run's state is its row's**, not the phases': the row is what the run's own `run.transition` events said, and
//!   it is the same after a restart.
//! - **A phase is `agent`, with one instance.** Every phase the daemon has run is an agent phase: the other kinds come
//!   with the workflow engine. The instance is named for the phase, and has no model or session id here.
//! - **A phase's times are its spans'**: it started when its first span opened, and ended when its last one closed. A
//!   phase whose last span is open has no end. A phase entered again has a span for each round, so its round is the
//!   number of its spans (at least one).
//! - **The text of a task's status is not read.** The status is carried as it was given.

use crate::metrics_store::{Span, SpanKind};
use crate::run_store::RunRow;
use collide_core::owner::{InstanceSummary, PhaseKindWire, PhaseSummary, RunSummary};
use collide_core::state::PhaseState;
use std::collections::BTreeMap;

/// The summary of the run, and how often each of its phases was entered.
pub fn summarize(
    row: &RunRow,
    phases: &[(String, PhaseState)],
    spans: &[Span],
    workspace: Option<String>,
) -> (RunSummary, BTreeMap<String, u32>) {
    let mut rounds = BTreeMap::new();
    let mut summaries: Vec<PhaseSummary> = phases
        .iter()
        .map(|(name, state)| {
            let own: Vec<&Span> = spans
                .iter()
                .filter(|span| span.kind == SpanKind::Phase && span.phase.as_deref() == Some(name))
                .collect();
            let round = u32::try_from(own.len()).unwrap_or(u32::MAX).max(1);
            rounds.insert(name.clone(), round);
            PhaseSummary {
                name: name.clone(),
                kind: PhaseKindWire::Agent,
                round,
                instances: vec![InstanceSummary {
                    id: name.clone(),
                    model: None,
                    session_id: None,
                    state: state.clone(),
                }],
                started_ms: own.first().map(|span| wall_ms(span.started_wall_ms)),
                ended_ms: own.last().and_then(|span| span.ended_wall_ms).map(wall_ms),
                gate_results: BTreeMap::new(),
                take_over: None,
            }
        })
        .collect();
    summaries.sort_by(|a, b| (a.started_ms, &a.name).cmp(&(b.started_ms, &b.name)));
    let summary = RunSummary {
        id: row.run.clone(),
        tasks: row.tasks.clone(),
        line: row.line.clone(),
        workflow: row.workflow.clone(),
        state: row.state.clone(),
        started_ms: row.started_ms,
        ended_ms: row.ended_ms,
        workspace,
        phases: summaries,
        warnings: Vec::new(),
    };
    (summary, rounds)
}

fn wall_ms(ms: i64) -> u64 {
    u64::try_from(ms).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use collide_core::owner::RunTaskRef;
    use collide_core::state::RunState;

    fn row() -> RunRow {
        RunRow {
            run: "run-1".to_string(),
            board: Some("board-1".to_string()),
            line: "main".to_string(),
            workflow: "collide/quick".to_string(),
            state: RunState::Running {},
            opened_seq: 4,
            started_ms: 1000,
            ended_ms: None,
            tasks: vec![RunTaskRef {
                key: "scratch:1".to_string(),
                status: "status-a".to_string(),
            }],
        }
    }

    fn phase_span(phase: &str, started: i64, ended: Option<i64>) -> Span {
        Span {
            id: started,
            run: "run-1".to_string(),
            phase: Some(phase.to_string()),
            parent: None,
            kind: SpanKind::Phase,
            wait: None,
            item: None,
            started_wall_ms: started,
            started_ns: 0,
            ended_wall_ms: ended,
            ended_ns: ended.map(|_| 1),
            outcome: None,
        }
    }

    #[test]
    fn the_row_gives_the_run_and_a_phase_gives_one_agent_instance_with_its_spans_times() {
        let phases = [("work".to_string(), PhaseState::Ended {})];
        let spans = [phase_span("work", 1100, Some(1900))];
        let (summary, rounds) = summarize(&row(), &phases, &spans, Some("/w/run-1".to_string()));
        assert_eq!(summary.id, "run-1");
        assert_eq!(summary.workspace.as_deref(), Some("/w/run-1"));
        assert_eq!((summary.line.as_str(), summary.tasks.len()), ("main", 1));
        let phase = &summary.phases[0];
        assert_eq!(phase.kind, PhaseKindWire::Agent);
        assert_eq!((phase.started_ms, phase.ended_ms), (Some(1100), Some(1900)));
        assert_eq!(phase.instances.len(), 1);
        assert_eq!(phase.instances[0].state, PhaseState::Ended {});
        assert_eq!(rounds["work"], 1);
    }

    #[test]
    fn a_phase_entered_twice_is_round_two_and_ends_with_its_last_span() {
        let phases = [("work".to_string(), PhaseState::Running {})];
        let spans = [
            phase_span("work", 1100, Some(1200)),
            phase_span("work", 1300, None),
        ];
        let (summary, rounds) = summarize(&row(), &phases, &spans, None);
        let phase = &summary.phases[0];
        assert_eq!(
            (phase.round, phase.started_ms, phase.ended_ms),
            (2, Some(1100), None)
        );
        assert_eq!(rounds["work"], 2);
    }

    #[test]
    fn a_phase_with_no_span_has_no_times_and_is_round_one() {
        let phases = [("work".to_string(), PhaseState::Pending {})];
        let (summary, _) = summarize(&row(), &phases, &[], None);
        let phase = &summary.phases[0];
        assert_eq!(
            (phase.round, phase.started_ms, phase.ended_ms),
            (1, None, None)
        );
    }

    #[test]
    fn phases_are_in_the_order_they_started() {
        let phases = [
            ("a".to_string(), PhaseState::Ended {}),
            ("b".to_string(), PhaseState::Ended {}),
        ];
        let spans = [
            phase_span("b", 1100, Some(1200)),
            phase_span("a", 1300, Some(1400)),
        ];
        let (summary, _) = summarize(&row(), &phases, &spans, None);
        let names: Vec<&str> = summary.phases.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["b", "a"]);
    }

    #[test]
    fn a_run_with_no_phases_has_none() {
        let (summary, rounds) = summarize(&row(), &[], &[], None);
        assert!(summary.phases.is_empty() && rounds.is_empty());
    }
}
