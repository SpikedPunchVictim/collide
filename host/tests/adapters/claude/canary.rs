//! The settings canary at the session (ts-0068-wx; 0.3, Settings that fail validation are ignored silently under
//! `-p`). The snapshot's settings carry a no-op `SessionStart` hook that writes a marker to stderr. A session that
//! expects the canary fails a phase whose turn ended without the hook's `hook_response` carrying it, with
//! `SettingsIgnored`. A user's own `SessionStart` hook is not the canary. The lines are built from the measured shape
//! of `hook_response` (sp-0007), never copied from a recording of the owner's.

use super::fake::assigned;
use super::session::{exit_with, line, result_line};
use collide_core::adapter::{AgentSession, PhaseFailure, PhaseOutcome};
use collide_host::adapters::claude::session::ClaudeSession;
use collide_host::adapters::claude::version::{CANARY_FAILURE, CANARY_MARKER};
use serde_json::{Value, json};

fn expecting() -> ClaudeSession {
    ClaudeSession::new(assigned()).expecting_canary()
}

fn session_start_response(stdout: &str, stderr: &str, outcome: &str) -> Vec<u8> {
    line(json!({
        "type": "system", "subtype": "hook_response", "hook_id": "h1", "hook_name": "SessionStart:startup",
        "hook_event": "SessionStart", "output": stdout, "stdout": stdout, "stderr": stderr, "exit_code": 0,
        "outcome": outcome,
    }))
}

fn the_canary() -> Vec<u8> {
    session_start_response("", &format!("{CANARY_MARKER}\n"), "success")
}

fn users_hook() -> Vec<u8> {
    session_start_response("CHISLE ACTIVE\n", "", "success")
}

fn run(session: &mut ClaudeSession, lines: &[Vec<u8>]) -> PhaseOutcome {
    session.send_turn("work");
    for line in lines {
        session.feed(line, 1);
    }
    session.finish(&exit_with(0)).outcome
}

#[test]
fn a_turn_that_ended_after_the_canary_is_an_ordinary_end() {
    let outcome = run(
        &mut expecting(),
        &[the_canary(), result_line(Value::Object(Default::default()))],
    );
    assert_eq!(outcome, PhaseOutcome::Ended);
}

#[test]
fn a_turn_that_ended_with_no_canary_fails_as_settings_ignored() {
    let outcome = run(
        &mut expecting(),
        &[result_line(Value::Object(Default::default()))],
    );
    assert_eq!(
        outcome,
        PhaseOutcome::Failed {
            cause: PhaseFailure::SettingsIgnored
        }
    );
}

#[test]
fn a_users_own_session_start_hook_does_not_count_as_the_canary() {
    let outcome = run(
        &mut expecting(),
        &[users_hook(), result_line(Value::Object(Default::default()))],
    );
    assert_eq!(
        outcome,
        PhaseOutcome::Failed {
            cause: PhaseFailure::SettingsIgnored
        }
    );
}

#[test]
fn a_canary_hook_that_failed_does_not_count() {
    let failed = session_start_response("", &format!("{CANARY_MARKER}\n"), "error");
    let outcome = run(
        &mut expecting(),
        &[failed, result_line(Value::Object(Default::default()))],
    );
    assert_eq!(
        outcome,
        PhaseOutcome::Failed {
            cause: PhaseFailure::SettingsIgnored
        }
    );
}

#[test]
fn a_session_that_does_not_expect_the_canary_ends_as_before() {
    let mut session = ClaudeSession::new(assigned());
    let outcome = run(
        &mut session,
        &[result_line(Value::Object(Default::default()))],
    );
    assert_eq!(outcome, PhaseOutcome::Ended);
}

#[test]
fn a_child_that_ended_with_no_turn_result_is_no_result_and_not_settings_ignored() {
    let outcome = run(&mut expecting(), &[]);
    assert_eq!(
        outcome,
        PhaseOutcome::Failed {
            cause: PhaseFailure::NoResult
        }
    );
}

#[test]
fn a_lost_child_is_lost_whatever_the_canary_says() {
    let mut session = expecting();
    session.send_turn("work");
    let lost = collide_core::adapter::ExitInfo {
        record: None,
        stderr_tail: String::new(),
    };
    assert_eq!(session.finish(&lost).outcome, PhaseOutcome::Lost);
}

#[test]
fn a_session_rebuilt_from_the_journal_has_seen_the_canary_the_journal_holds() {
    // A rebuilt session reads the journal's lines as the live one did, so the canary of the first process counts for a
    // second process of the same phase that has no hook of its own to show.
    let mut rebuilt = expecting();
    rebuilt.feed(&the_canary(), 1);
    rebuilt.send_turn("again");
    rebuilt.feed(&result_line(Value::Object(Default::default())), 9);
    assert_eq!(rebuilt.finish(&exit_with(0)).outcome, PhaseOutcome::Ended);
}

#[test]
fn the_queue_words_for_the_failure_are_the_plans() {
    assert_eq!(CANARY_FAILURE, "Claude ignored Collide's settings");
}
