//! A usage limit gives one `Limited` (ts-0064-hw; 0.3, Usage and rate limits; S1, Q8). A `rate_limit_event` that is
//! `rejected`, an assistant message with `error: "rate_limit"` and a 429 error result are three lines about one limit, so
//! the session keeps the limit until the turn's `result`, takes the reset time from whichever line has one, and gives
//! one event. The recording is played through fake Claude under the real shim; the other shapes are fed by hand.

use super::fake::{assigned, play};
use super::session::{
    answer_nothing, events, exit_with, line, named, result_line, session, turn_end,
};
use collide_core::adapter::AgentSession;
use collide_core::agent::{AgentEvent, TurnEndReason};
use collide_host::adapters::claude::session::ClaudeSession;
use serde_json::json;

// ---- lines ---------------------------------------------------------------------------------------------------------

fn rate_limit_event(status: &str, resets_at: Option<u64>) -> Vec<u8> {
    let mut info = json!({"status": status, "rateLimitType": "seven_day", "isUsingOverage": false});
    if let Some(seconds) = resets_at {
        info["resetsAt"] = json!(seconds);
    }
    line(json!({"type": "rate_limit_event", "rate_limit_info": info}))
}

/// The message Claude writes for a 429 with quota information, with the reset time in `quotaLimits` when `resets_at`
/// is given (S1 saw it in transcripts).
fn limit_message(resets_at: Option<u64>) -> Vec<u8> {
    let mut message = json!({"type": "assistant", "error": "rate_limit", "isApiErrorMessage": true,
        "message": {"content": [{"type": "text", "text": "You've hit your weekly limit"}]}});
    if let Some(seconds) = resets_at {
        message["quotaLimits"] = json!({"status": "rejected", "resetsAt": seconds});
    }
    line(message)
}

fn result_429() -> Vec<u8> {
    result_line(
        json!({"is_error": true, "api_error_status": 429, "result": "You've hit your weekly limit",
        "stop_reason": "stop_sequence"}),
    )
}

fn limited_in(events: &[AgentEvent]) -> Vec<Option<u64>> {
    events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::Limited { reset_at_ms } => Some(*reset_at_ms),
            _ => None,
        })
        .collect()
}

/// Feeds each line, and returns the events of all of them.
fn feed_all(session: &mut ClaudeSession, lines: &[Vec<u8>]) -> Vec<AgentEvent> {
    lines
        .iter()
        .flat_map(|bytes| events(&session.feed(bytes, 5)))
        .collect()
}

// ---- one limit, one Limited ----------------------------------------------------------------------------------------

#[test]
fn the_limit_recording_gives_one_limited_with_the_reset_time_in_milliseconds_and_no_report() {
    let played = play("limit", assigned(), &["go"], answer_nothing);
    let all = named(&played);
    assert_eq!(limited_in(&all), [Some(1_794_960_000_000)]);
    assert!(
        !all.iter()
            .any(|e| matches!(e, AgentEvent::FinalReport { .. }))
    );
    assert_eq!(
        all.last(),
        Some(&turn_end(
            TurnEndReason::Error { status: Some(429) },
            false,
            false
        ))
    );
    let at = all
        .iter()
        .position(|e| matches!(e, AgentEvent::Limited { .. }))
        .unwrap();
    assert_eq!(
        at,
        all.len() - 2,
        "the limit comes just before the end of the turn"
    );
}

#[test]
fn the_three_lines_about_one_limit_give_one_limited_carrying_the_reset_time_whichever_line_has_it()
{
    let lines = [
        (
            rate_limit_event("rejected", Some(1000)),
            limit_message(None),
            result_429(),
        ),
        (
            rate_limit_event("rejected", None),
            limit_message(Some(1000)),
            result_429(),
        ),
        (
            rate_limit_event("rejected", None),
            limit_message(None),
            result_429(),
        ),
    ];
    for (n, (event, message, result)) in lines.into_iter().enumerate() {
        let mut session = session();
        let all = feed_all(&mut session, &[event, message, result]);
        let expected = if n < 2 { Some(1_000_000) } else { None };
        assert_eq!(limited_in(&all), [expected], "case {n}");
    }
}

#[test]
fn each_line_alone_is_a_limit() {
    for lines in [
        vec![
            rate_limit_event("rejected", Some(7)),
            result_line(json!({})),
        ],
        vec![limit_message(Some(7)), result_line(json!({}))],
        vec![limit_message(None), result_line(json!({}))],
        vec![result_429()],
    ] {
        let mut session = session();
        assert_eq!(limited_in(&feed_all(&mut session, &lines)).len(), 1);
    }
}

#[test]
fn the_limit_is_held_until_the_result_so_no_line_before_it_gives_one() {
    let mut session = session();
    for bytes in [
        rate_limit_event("rejected", Some(7)),
        limit_message(Some(7)),
    ] {
        assert!(limited_in(&events(&session.feed(&bytes, 5))).is_empty());
    }
    let at_result = events(&session.feed(&result_429(), 6));
    assert_eq!(limited_in(&at_result), [Some(7000)]);
}

#[test]
fn a_later_line_with_a_reset_time_fills_in_one_that_had_none_without_a_second_event() {
    let mut session = session();
    let all = feed_all(
        &mut session,
        &[
            limit_message(None),
            rate_limit_event("rejected", Some(1000)),
            result_429(),
        ],
    );
    assert_eq!(limited_in(&all), [Some(1_000_000)]);
}

#[test]
fn the_first_reset_time_wins_when_two_lines_give_different_ones() {
    let mut session = session();
    let all = feed_all(
        &mut session,
        &[
            rate_limit_event("rejected", Some(1000)),
            limit_message(Some(2000)),
            result_429(),
        ],
    );
    assert_eq!(limited_in(&all), [Some(1_000_000)]);
}

#[test]
fn two_rejected_events_in_one_turn_are_one_limit() {
    let mut session = session();
    let all = feed_all(
        &mut session,
        &[
            rate_limit_event("rejected", Some(5)),
            rate_limit_event("rejected", Some(5)),
            result_429(),
        ],
    );
    assert_eq!(limited_in(&all), [Some(5000)]);
}

#[test]
fn a_reset_time_that_is_not_a_whole_number_of_seconds_is_none() {
    for bad in [
        json!("soon"),
        json!(-5),
        json!(1.5),
        json!(null),
        json!(u64::MAX),
    ] {
        let mut session = session();
        let event = line(json!({"type": "rate_limit_event",
            "rate_limit_info": {"status": "rejected", "resetsAt": bad}}));
        let all = feed_all(&mut session, &[event, result_429()]);
        assert_eq!(limited_in(&all), [None], "{bad}");
    }
}

#[test]
fn the_next_turn_has_its_own_limit() {
    let mut session = session();
    let first = feed_all(
        &mut session,
        &[rate_limit_event("rejected", Some(5)), result_429()],
    );
    session.send_turn("again");
    let second = feed_all(&mut session, &[result_429()]);
    assert_eq!(
        (limited_in(&first), limited_in(&second)),
        (vec![Some(5000)], vec![None])
    );
}

#[test]
fn a_limit_that_no_result_followed_is_dropped_when_a_new_turn_is_sent() {
    let mut session = session();
    session.feed(&rate_limit_event("rejected", Some(5)), 1);
    session.send_turn("again");
    let all = feed_all(&mut session, &[result_line(json!({}))]);
    assert!(limited_in(&all).is_empty());
}

// ---- what is not a limit -------------------------------------------------------------------------------------------

#[test]
fn allowed_and_allowed_warning_are_recorded_and_do_not_pause() {
    let mut session = session();
    for (status, name) in [
        ("allowed_warning", "rate_limit_event/allowed_warning"),
        ("allowed", "rate_limit_event"),
        ("something_new", "rate_limit_event"),
    ] {
        let all = events(&session.feed(&rate_limit_event(status, Some(9)), 5));
        assert_eq!(
            all,
            [AgentEvent::Other {
                raw_type: name.into(),
                raw: None
            }],
            "{status}"
        );
    }
    let all = feed_all(&mut session, &[result_line(json!({}))]);
    assert!(limited_in(&all).is_empty());
}

#[test]
fn a_rate_limit_event_with_no_status_is_other_and_no_limit() {
    let mut session = session();
    let all = events(&session.feed(&line(json!({"type": "rate_limit_event"})), 5));
    assert_eq!(
        all,
        [AgentEvent::Other {
            raw_type: "rate_limit_event".into(),
            raw: None
        }]
    );
}

#[test]
fn another_error_on_an_assistant_message_is_no_limit() {
    let mut session = session();
    let message = line(json!({"type": "assistant", "error": "overloaded",
        "message": {"content": [{"type": "text", "text": "Overloaded"}]}}));
    let all = feed_all(&mut session, &[message, result_line(json!({}))]);
    assert!(limited_in(&all).is_empty());
}

#[test]
fn a_limit_message_still_gives_its_text_as_a_message_and_makes_the_turn_not_empty() {
    let mut session = session();
    let all = events(&session.feed(&limit_message(None), 5));
    assert!(matches!(&all[..], [AgentEvent::Message { .. }]));
}

// ---- a phase cut before its result ---------------------------------------------------------------------------------

#[test]
fn a_limit_with_no_result_after_it_is_given_once_when_the_phase_finishes() {
    let mut session = session();
    session.send_turn("go");
    session.feed(&rate_limit_event("rejected", Some(1000)), 1);
    session.feed(&limit_message(None), 2);
    let finished = session.finish(&exit_with(1));
    assert_eq!(limited_in(&events(&finished.step)), [Some(1_000_000)]);
    assert!(
        finished
            .step
            .events
            .iter()
            .all(|stamped| stamped.wall_ns.is_none())
    );
}

#[test]
fn a_limit_the_result_gave_is_not_given_again_when_the_phase_finishes() {
    let mut session = session();
    session.send_turn("go");
    feed_all(
        &mut session,
        &[rate_limit_event("rejected", Some(1000)), result_429()],
    );
    assert!(limited_in(&events(&session.finish(&exit_with(1)).step)).is_empty());
}
