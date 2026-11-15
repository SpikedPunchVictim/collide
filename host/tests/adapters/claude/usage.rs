//! Usage, compaction and model time (ts-0064-hw; 0.3, Usage and Context and cache; 0.13, Token and context metrics and
//! Missing is not zero; S9). Input and cache tokens are counted once per response (`message.id`), the turn's totals
//! come from `result.usage`, a compaction and any other model call come from `model_usage_since_last_result`, and a
//! turn's model time and cost are differences of running totals. The recording is played through fake Claude under the
//! real shim; the shapes it does not hold are fed by hand.

use super::fake::{assigned, play};
use super::session::{answer_nothing, events, exit_with, line, named, result_line, session};
use collide_core::adapter::AgentSession;
use collide_core::agent::{
    AgentEvent, CompactionTrigger, Money, SessionStart, TurnEndReason, Usage, UsageQuality,
    UsageScope,
};
use collide_host::adapters::claude::session::ClaudeSession;
use serde_json::{Value, json};

// ---- lines ---------------------------------------------------------------------------------------------------------

/// What a stream row of one response carries: input and cache from the start of the response, and its first output count.
fn stream_usage(input: u64, write_1h: u64, read: u64, output: u64) -> Value {
    json!({
        "input_tokens": input, "cache_creation_input_tokens": write_1h, "cache_read_input_tokens": read,
        "cache_creation": {"ephemeral_5m_input_tokens": 0, "ephemeral_1h_input_tokens": write_1h},
        "output_tokens": output, "service_tier": "standard",
    })
}

fn response(id: &str, block: Value, usage: Value) -> Vec<u8> {
    line(json!({"type": "assistant", "parent_tool_use_id": null,
        "message": {"id": id, "model": "claude-haiku-5-5", "content": [block], "usage": usage}}))
}

fn text(words: &str) -> Value {
    json!({"type": "text", "text": words})
}

fn tool_use(id: &str) -> Value {
    json!({"type": "tool_use", "id": id, "name": "Read", "input": {"file_path": "a.txt"}})
}

/// One model's entry of `model_usage_since_last_result`, as Claude writes it (camel case).
fn model_usage(
    model: &str,
    input: u64,
    output: u64,
    read: u64,
    write_1h: u64,
    write_5m: u64,
    cost: f64,
) -> Value {
    json!({model: {
        "inputTokens": input, "outputTokens": output, "cacheReadInputTokens": read,
        "cacheCreationInputTokens": write_1h + write_5m, "costUSD": cost, "contextWindow": 1_000_000,
        "thinkingTokens": 0, "cacheCreationEphemeral5mInputTokens": write_5m,
        "cacheCreationEphemeral1hInputTokens": write_1h,
    }})
}

fn result_usage(input: u64, output: u64, read: u64, write_1h: u64, thinking: u64) -> Value {
    json!({
        "input_tokens": input, "output_tokens": output, "cache_read_input_tokens": read,
        "cache_creation_input_tokens": write_1h,
        "cache_creation": {"ephemeral_5m_input_tokens": 0, "ephemeral_1h_input_tokens": write_1h},
        "output_tokens_details": {"thinking_tokens": thinking},
    })
}

fn compact_boundary() -> Vec<u8> {
    line(json!({"type": "system", "subtype": "compact_boundary",
        "compact_metadata": {"trigger": "auto", "pre_tokens": 67000, "post_tokens": 15000}}))
}

fn resumed() -> ClaudeSession {
    ClaudeSession::new(SessionStart::Resume { id: "s-1".into() })
}

// ---- what the events hold ------------------------------------------------------------------------------------------

fn usages(events: &[AgentEvent]) -> Vec<Usage> {
    events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::Usage { usage } => Some(usage.clone()),
            _ => None,
        })
        .collect()
}

/// The usage of `scope` among `events`, which must hold exactly one.
fn only(events: &[AgentEvent], scope: UsageScope) -> Usage {
    let mut found: Vec<Usage> = usages(events)
        .into_iter()
        .filter(|usage| usage.scope == scope)
        .collect();
    assert_eq!(found.len(), 1, "one {scope:?} usage in {events:?}");
    found.remove(0)
}

fn none_of(events: &[AgentEvent], scope: UsageScope) -> bool {
    usages(events).iter().all(|usage| usage.scope != scope)
}

fn amount(usage: &Usage) -> Option<f64> {
    usage.cost.as_ref().map(|cost| cost.amount)
}

fn assert_cost(usage: &Usage, expected: Option<f64>) {
    match (amount(usage), expected) {
        (Some(got), Some(want)) => assert!((got - want).abs() < 1e-9, "cost {got}, wanted {want}"),
        (got, want) => assert_eq!(got, want),
    }
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

/// Feeds a result built from `fields` and returns the events it gives.
fn feed_result(session: &mut ClaudeSession, fields: Value) -> Vec<AgentEvent> {
    events(&session.feed(&result_line(fields), 9))
}

// ---- once per response ---------------------------------------------------------------------------------------------

#[test]
fn the_content_blocks_of_one_response_give_one_step_usage_and_not_one_each() {
    let mut session = session();
    let usage = stream_usage(2, 18000, 14000, 21);
    let mut steps = Vec::new();
    for block in [
        json!({"type": "thinking", "thinking": ""}),
        text("Reading."),
        tool_use("toolu_1"),
    ] {
        let step = session.feed(&response("msg_01", block, usage.clone()), 1);
        steps.extend(usages(&events(&step)));
    }
    assert_eq!(steps.len(), 1, "three rows, one response");
    assert_eq!(steps[0].scope, UsageScope::Step);
}

#[test]
fn a_step_has_the_responses_input_and_cache_tokens_its_context_and_no_output_or_cost() {
    let mut session = session();
    let step = session.feed(
        &response("msg_01", text("hi"), stream_usage(2, 18000, 14000, 21)),
        1,
    );
    assert_eq!(
        usages(&events(&step)),
        [Usage {
            input: Some(2),
            cache_read: Some(14000),
            cache_write: Some(18000),
            cache_write_1h: Some(18000),
            cache_write_5m: Some(0),
            context_used: Some(32002),
            ..blank(UsageScope::Step, UsageQuality::Partial)
        }],
        "the stream's output count is partial (S9), so a step has none, and a step's cost is not reported"
    );
}

#[test]
fn the_usage_event_comes_before_the_blocks_of_the_line_that_carries_it() {
    let mut session = session();
    let step = session.feed(
        &response("msg_01", text("hi"), stream_usage(2, 0, 100, 1)),
        1,
    );
    let all = events(&step);
    assert!(matches!(all[0], AgentEvent::Usage { .. }));
    assert!(matches!(all[1], AgentEvent::Message { .. }));
}

#[test]
fn each_response_has_a_step_of_its_own() {
    let mut session = session();
    let first = session.feed(
        &response("msg_01", text("a"), stream_usage(2, 10, 20, 1)),
        1,
    );
    let second = session.feed(&response("msg_02", text("b"), stream_usage(3, 0, 40, 1)), 2);
    assert_eq!(
        only(&events(&first), UsageScope::Step).context_used,
        Some(32)
    );
    assert_eq!(
        only(&events(&second), UsageScope::Step).context_used,
        Some(43)
    );
}

#[test]
fn a_response_with_no_id_or_no_usage_gives_no_step() {
    let mut session = session();
    let no_id = line(
        json!({"type": "assistant", "message": {"content": [text("a")],
        "usage": stream_usage(2, 0, 100, 1)}}),
    );
    let no_usage =
        line(json!({"type": "assistant", "message": {"id": "msg_01", "content": [text("a")]}}));
    for row in [no_id, no_usage] {
        assert!(usages(&events(&session.feed(&row, 1))).is_empty());
    }
}

#[test]
fn a_step_without_cache_tiers_leaves_them_out_and_keeps_the_cache_total() {
    let mut session = session();
    let usage =
        json!({"input_tokens": 3, "cache_read_input_tokens": 7, "cache_creation_input_tokens": 5});
    let step = session.feed(&response("msg_01", text("a"), usage), 1);
    let step = only(&events(&step), UsageScope::Step);
    assert_eq!(
        (step.cache_write, step.cache_write_1h, step.cache_write_5m),
        (Some(5), None, None)
    );
}

#[test]
fn a_subagents_response_is_counted_but_is_not_the_context_of_the_session() {
    let mut session = session();
    let row = line(json!({"type": "assistant", "parent_tool_use_id": "toolu_9",
        "message": {"id": "msg_s1", "content": [text("a")], "usage": stream_usage(2, 0, 100, 1)}}));
    let step = only(&events(&session.feed(&row, 1)), UsageScope::Step);
    assert_eq!((step.input, step.context_used), (Some(2), None));
}

// ---- the turn's totals ---------------------------------------------------------------------------------------------

#[test]
fn a_turn_takes_its_totals_from_result_usage_and_not_from_the_rows_of_the_stream() {
    let mut session = session();
    for (id, block) in [
        ("msg_01", text("a")),
        ("msg_01", tool_use("t1")),
        ("msg_02", text("b")),
    ] {
        session.feed(&response(id, block, stream_usage(2, 18000, 14000, 21)), 1);
    }
    let all = feed_result(
        &mut session,
        json!({"usage": result_usage(4, 270, 28000, 36000, 60), "total_cost_usd": 0.0084,
               "duration_api_ms": 2300}),
    );
    assert_eq!(
        only(&all, UsageScope::Turn),
        Usage {
            input: Some(4),
            output: Some(270),
            cache_read: Some(28000),
            cache_write: Some(36000),
            cache_write_1h: Some(36000),
            cache_write_5m: Some(0),
            thought: Some(60),
            context_used: Some(32002),
            cost: Some(Money {
                amount: 0.0084,
                currency: "USD".into()
            }),
            model_time_ms: Some(2300),
            ..blank(UsageScope::Turn, UsageQuality::Exact)
        },
        "output is the result's 270, not the stream's 21; the context is the last response's"
    );
}

#[test]
fn the_turns_context_size_is_the_window_of_the_model_that_answered() {
    let mut session = session();
    session.feed(
        &response("msg_01", text("a"), stream_usage(2, 0, 100, 1)),
        1,
    );
    let all = feed_result(
        &mut session,
        json!({"usage": result_usage(2, 5, 100, 0, 0),
               "model_usage_since_last_result": model_usage("claude-haiku-5-5", 2, 5, 100, 0, 0, 0.001)}),
    );
    assert_eq!(only(&all, UsageScope::Turn).context_size, Some(1_000_000));
}

#[test]
fn a_context_window_of_some_other_model_is_not_the_answering_models() {
    let mut session = session();
    session.feed(
        &response("msg_01", text("a"), stream_usage(2, 0, 100, 1)),
        1,
    );
    let all = feed_result(
        &mut session,
        json!({"usage": result_usage(2, 5, 100, 0, 0),
               "model_usage_since_last_result": model_usage("claude-other", 2, 5, 100, 0, 0, 0.001)}),
    );
    assert_eq!(only(&all, UsageScope::Turn).context_size, None);
}

#[test]
fn every_result_gives_the_running_cost_as_it_was_reported() {
    let mut session = session();
    let all = feed_result(&mut session, json!({"total_cost_usd": 0.0084}));
    let total = only(&all, UsageScope::RunningTotal);
    assert_eq!(
        total,
        Usage {
            cost: Some(Money {
                amount: 0.0084,
                currency: "USD".into()
            }),
            ..blank(UsageScope::RunningTotal, UsageQuality::Exact)
        }
    );
    let none = feed_result(&mut session, json!({}));
    assert!(
        none_of(&none, UsageScope::RunningTotal),
        "no cost reported, none given"
    );
}

#[test]
fn the_usage_events_come_after_the_report_and_just_before_the_end_of_the_turn() {
    let mut session = session();
    let all = feed_result(&mut session, json!({"total_cost_usd": 0.001}));
    let last = all.len() - 1;
    assert!(matches!(all[last], AgentEvent::TurnEnd { .. }));
    assert!(
        all[..last]
            .iter()
            .any(|e| matches!(e, AgentEvent::FinalReport { .. }))
    );
    assert!(matches!(all[last - 1], AgentEvent::Usage { .. }));
}

// ---- quality -------------------------------------------------------------------------------------------------------

#[test]
fn a_result_whose_usage_has_every_count_is_exact_and_one_that_lacks_some_is_partial() {
    let mut session = session();
    let whole = feed_result(&mut session, json!({"usage": result_usage(1, 2, 3, 4, 0)}));
    assert_eq!(only(&whole, UsageScope::Turn).quality, UsageQuality::Exact);
    // `result_line`'s own usage has no cache counts.
    let partial = feed_result(&mut session, json!({}));
    let turn = only(&partial, UsageScope::Turn);
    assert_eq!(turn.quality, UsageQuality::Partial);
    assert_eq!(
        (turn.input, turn.output, turn.cache_read, turn.cache_write),
        (Some(4), Some(9), None, None)
    );
}

#[test]
fn a_result_with_no_usage_is_unreported_with_every_number_none_and_never_zero() {
    let mut session = session();
    let mut result = serde_json::from_slice::<Value>(&result_line(json!({"result": ""}))).unwrap();
    result.as_object_mut().unwrap().remove("usage");
    let all = events(&session.feed(&line(result), 9));
    assert_eq!(
        only(&all, UsageScope::Turn),
        blank(UsageScope::Turn, UsageQuality::Unreported)
    );
}

#[test]
fn a_result_with_no_usage_after_responses_keeps_their_input_and_cache_tokens_as_partial() {
    let mut session = session();
    session.feed(
        &response("msg_01", text("a"), stream_usage(2, 10, 20, 1)),
        1,
    );
    session.feed(
        &response("msg_01", tool_use("t"), stream_usage(2, 10, 20, 1)),
        1,
    );
    session.feed(&response("msg_02", text("b"), stream_usage(3, 0, 40, 1)), 2);
    let mut result = serde_json::from_slice::<Value>(&result_line(json!({}))).unwrap();
    result.as_object_mut().unwrap().remove("usage");
    let all = events(&session.feed(&line(result), 9));
    let turn = only(&all, UsageScope::Turn);
    assert_eq!(turn.quality, UsageQuality::Partial);
    assert_eq!(
        (turn.input, turn.cache_read, turn.cache_write),
        (Some(5), Some(60), Some(10)),
        "once per response"
    );
    assert_eq!((turn.output, turn.cost), (None, None));
}

#[test]
fn a_new_turn_does_not_carry_the_responses_of_the_one_before() {
    let mut session = session();
    session.feed(
        &response("msg_01", text("a"), stream_usage(2, 10, 20, 1)),
        1,
    );
    feed_result(&mut session, json!({}));
    let mut result = serde_json::from_slice::<Value>(&result_line(json!({}))).unwrap();
    result.as_object_mut().unwrap().remove("usage");
    let all = events(&session.feed(&line(result), 9));
    assert_eq!(
        only(&all, UsageScope::Turn).quality,
        UsageQuality::Unreported
    );
}

// ---- running totals as differences ---------------------------------------------------------------------------------

fn turn_of(session: &mut ClaudeSession, fields: Value) -> Usage {
    only(&feed_result(session, fields), UsageScope::Turn)
}

#[test]
fn a_turns_model_time_and_cost_are_the_differences_of_the_running_totals() {
    let mut session = session();
    let first = turn_of(
        &mut session,
        json!({"duration_api_ms": 2300, "total_cost_usd": 0.0084}),
    );
    assert_eq!(first.model_time_ms, Some(2300));
    assert_cost(&first, Some(0.0084));
    let second = turn_of(
        &mut session,
        json!({"duration_api_ms": 10300, "total_cost_usd": 0.0124}),
    );
    assert_eq!(second.model_time_ms, Some(8000));
    assert_cost(&second, Some(0.0040));
}

#[test]
fn a_result_with_no_turns_has_no_model_time_and_the_difference_is_taken_from_the_last_result_with_turns()
 {
    let mut session = session();
    turn_of(
        &mut session,
        json!({"duration_api_ms": 2300, "total_cost_usd": 0.0084}),
    );
    // A `/compact`: zero turns, and `duration_api_ms` reports 0.
    let compact = turn_of(
        &mut session,
        json!({"num_turns": 0, "duration_api_ms": 0, "total_cost_usd": 0.0118, "usage": result_usage(0, 0, 0, 0, 0)}),
    );
    assert_eq!(compact.model_time_ms, None);
    let next = turn_of(
        &mut session,
        json!({"duration_api_ms": 10300, "total_cost_usd": 0.0158}),
    );
    assert_eq!(
        next.model_time_ms,
        Some(8000),
        "from 2300, the last result with turns"
    );
}

#[test]
fn the_cost_of_a_turn_is_taken_from_the_result_before_it_so_a_compaction_is_not_counted_twice() {
    let mut session = session();
    turn_of(
        &mut session,
        json!({"duration_api_ms": 2300, "total_cost_usd": 0.0084}),
    );
    feed_result(
        &mut session,
        json!({"num_turns": 0, "duration_api_ms": 0, "total_cost_usd": 0.0118, "usage": result_usage(0, 0, 0, 0, 0),
               "model_usage_since_last_result": model_usage("claude-haiku-5-5", 22500, 1500, 32000, 0, 181, 0.0034)}),
    );
    let next = turn_of(
        &mut session,
        json!({"duration_api_ms": 10300, "total_cost_usd": 0.0158}),
    );
    assert_cost(&next, Some(0.0040));
}

#[test]
fn a_result_without_duration_api_ms_has_no_model_time_and_so_has_the_next_one() {
    let mut session = session();
    turn_of(&mut session, json!({"duration_api_ms": 2300}));
    let lacking = turn_of(&mut session, json!({}));
    assert_eq!(lacking.model_time_ms, None, "never 0");
    let after = turn_of(&mut session, json!({"duration_api_ms": 9000}));
    assert_eq!(
        after.model_time_ms, None,
        "a difference over a missing value would count two turns as one"
    );
    let then = turn_of(&mut session, json!({"duration_api_ms": 9500}));
    assert_eq!(then.model_time_ms, Some(500));
}

#[test]
fn a_running_total_that_goes_backwards_gives_no_model_time() {
    let mut session = session();
    turn_of(&mut session, json!({"duration_api_ms": 9000}));
    assert_eq!(
        turn_of(&mut session, json!({"duration_api_ms": 100})).model_time_ms,
        None
    );
}

#[test]
fn a_result_with_no_cost_has_no_cost_and_the_next_one_takes_the_turns_own_figure() {
    let mut session = session();
    turn_of(&mut session, json!({"total_cost_usd": 0.0084}));
    let lacking = turn_of(&mut session, json!({}));
    assert_cost(&lacking, None);
    let after = turn_of(
        &mut session,
        json!({"total_cost_usd": 0.0300,
               "model_usage_since_last_result": model_usage("claude-haiku-5-5", 1, 1, 1, 1, 0, 0.0040)}),
    );
    assert_cost(&after, Some(0.0040));
}

#[test]
fn the_first_result_of_a_resumed_process_takes_the_turns_own_cost_not_the_session_total() {
    // S1: a `--resume`d process continues the session's total, the earlier cost included.
    let mut session = resumed();
    let first = turn_of(
        &mut session,
        json!({"duration_api_ms": 50_000, "total_cost_usd": 0.2000,
               "model_usage_since_last_result": model_usage("claude-haiku-5-5", 1, 1, 1, 1, 0, 0.0057)}),
    );
    assert_cost(&first, Some(0.0057));
    assert_eq!(
        first.model_time_ms, None,
        "the process' first total may include the earlier ones"
    );
    let second = turn_of(
        &mut session,
        json!({"duration_api_ms": 52_000, "total_cost_usd": 0.2100}),
    );
    assert_cost(&second, Some(0.0100));
    assert_eq!(second.model_time_ms, Some(2000));
}

#[test]
fn the_first_result_of_a_resumed_process_with_no_figure_of_its_own_has_no_cost() {
    let mut session = resumed();
    assert_cost(&turn_of(&mut session, json!({"total_cost_usd": 0.2})), None);
}

#[test]
fn the_running_total_event_is_the_session_total_for_a_resumed_process_too() {
    let mut session = resumed();
    let all = feed_result(&mut session, json!({"total_cost_usd": 0.2}));
    assert_cost(&only(&all, UsageScope::RunningTotal), Some(0.2));
}

// ---- compaction and other model calls ------------------------------------------------------------------------------

#[test]
fn a_manual_compact_is_its_own_usage_with_the_whole_spend_and_the_turn_is_zero() {
    let mut session = session();
    turn_of(
        &mut session,
        json!({"duration_api_ms": 2300, "total_cost_usd": 0.0084}),
    );
    let all = feed_result(
        &mut session,
        json!({"num_turns": 0, "duration_api_ms": 0, "total_cost_usd": 0.0118, "result": "",
               "local_command": "compact", "usage": result_usage(0, 0, 0, 0, 0),
               "model_usage_since_last_result": model_usage("claude-haiku-5-5", 22500, 1500, 32000, 0, 181, 0.0034)}),
    );
    let compaction = only(&all, UsageScope::Compaction);
    assert_eq!(
        (
            compaction.input,
            compaction.output,
            compaction.cache_read,
            compaction.cache_write
        ),
        (Some(22500), Some(1500), Some(32000), Some(181))
    );
    assert_eq!(
        (compaction.cache_write_5m, compaction.cache_write_1h),
        (Some(181), Some(0))
    );
    assert_eq!(compaction.quality, UsageQuality::Exact);
    assert_cost(&compaction, Some(0.0034));
    let turn = only(&all, UsageScope::Turn);
    assert_eq!(
        (turn.input, turn.output, turn.cache_read, turn.cache_write),
        (Some(0), Some(0), Some(0), Some(0))
    );
    assert_cost(&turn, Some(0.0));
}

#[test]
fn an_automatic_compaction_inside_a_turn_is_the_difference_between_the_two_usages_and_is_not_counted_in_the_turn()
 {
    // S9: the turn's result.usage shows input 4 and output 464; model_usage shows the summarizer call as well.
    let mut session = session();
    session.feed(
        &response("msg_01", text("a"), stream_usage(2, 0, 100, 1)),
        1,
    );
    session.feed(&compact_boundary(), 2);
    let all = feed_result(
        &mut session,
        json!({"duration_api_ms": 4000, "total_cost_usd": 0.0230,
               "usage": result_usage(4, 464, 30000, 5000, 0),
               "model_usage_since_last_result": model_usage("claude-haiku-5-5", 22157, 2508, 30000, 5000, 181, 0.0230)}),
    );
    let compaction = only(&all, UsageScope::Compaction);
    assert_eq!(
        (
            compaction.input,
            compaction.output,
            compaction.cache_read,
            compaction.cache_write
        ),
        (Some(22153), Some(2044), Some(0), Some(181))
    );
    assert_eq!(
        (compaction.cache_write_5m, compaction.cache_write_1h),
        (Some(181), Some(0))
    );
    let turn = only(&all, UsageScope::Turn);
    assert_eq!(
        (turn.input, turn.output),
        (Some(4), Some(464)),
        "the turn's own, from result.usage"
    );
    assert_cost(&compaction, None);
    assert_cost(&turn, Some(0.0230));
}

fn compactions(events: &[AgentEvent]) -> Vec<AgentEvent> {
    events
        .iter()
        .filter(|event| matches!(event, AgentEvent::Compaction { .. }))
        .cloned()
        .collect()
}

#[test]
fn a_compact_boundary_is_a_compaction_with_the_trigger_the_size_before_and_the_duration() {
    let mut session = session();
    let boundary = line(json!({"type": "system", "subtype": "compact_boundary",
        "compact_metadata": {"trigger": "manual", "pre_tokens": 53376, "post_tokens": 10422, "duration_ms": 10314}}));
    let found = compactions(&events(&session.feed(&boundary, 2)));
    assert_eq!(
        found,
        [AgentEvent::Compaction {
            trigger: Some(CompactionTrigger::Manual),
            context_before: Some(53376),
            duration_ms: Some(10314),
            mid_task: false,
        }],
        "the tokens after are the next response's context, not post_tokens"
    );
}

#[test]
fn a_compaction_while_a_tool_call_is_open_is_mid_task() {
    let mut session = session();
    session.feed(
        &response("msg_01", tool_use("toolu_1"), stream_usage(2, 0, 100, 1)),
        1,
    );
    let found = compactions(&events(&session.feed(&compact_boundary(), 2)));
    assert!(
        matches!(found[..], [AgentEvent::Compaction { mid_task: true, .. }]),
        "{found:?}"
    );
}

#[test]
fn a_boundary_whose_line_lacks_the_metadata_has_every_figure_absent_and_not_zero() {
    let mut session = session();
    let boundary = line(json!({"type": "system", "subtype": "compact_boundary"}));
    let found = compactions(&events(&session.feed(&boundary, 2)));
    assert_eq!(
        found,
        [AgentEvent::Compaction {
            trigger: None,
            context_before: None,
            duration_ms: None,
            mid_task: false,
        }]
    );
}

#[test]
fn a_trigger_this_build_does_not_know_is_absent() {
    let mut session = session();
    let boundary = line(json!({"type": "system", "subtype": "compact_boundary",
        "compact_metadata": {"trigger": "scheduled", "pre_tokens": 5}}));
    let found = compactions(&events(&session.feed(&boundary, 2)));
    assert!(
        matches!(
            found[..],
            [AgentEvent::Compaction {
                trigger: None,
                context_before: Some(5),
                ..
            }]
        ),
        "{found:?}"
    );
}

#[test]
fn the_compaction_usage_comes_before_the_turns_and_the_cost_is_given_once() {
    let mut session = session();
    session.feed(&compact_boundary(), 2);
    let all = feed_result(
        &mut session,
        json!({"total_cost_usd": 0.0034, "usage": result_usage(0, 0, 0, 0, 0),
               "model_usage_since_last_result": model_usage("claude-haiku-5-5", 10, 5, 0, 0, 181, 0.0034)}),
    );
    let order: Vec<UsageScope> = usages(&all).iter().map(|usage| usage.scope).collect();
    assert_eq!(
        order,
        [
            UsageScope::Compaction,
            UsageScope::Turn,
            UsageScope::RunningTotal
        ]
    );
    let spent: f64 = usages(&all)
        .iter()
        .filter(|usage| usage.scope != UsageScope::RunningTotal)
        .filter_map(amount)
        .sum();
    assert!(
        (spent - 0.0034).abs() < 1e-9,
        "the cost of Turn and Compaction adds up to the spend, not twice it"
    );
}

#[test]
fn a_model_call_that_is_not_a_compaction_is_other_calls() {
    // Ferry's fixture: `total_cost_usd` includes a haiku call that `result.usage` lacks.
    let mut session = session();
    let usage = result_usage(4, 100, 1000, 0, 0);
    let mut both = model_usage("claude-sonnet-5", 4, 100, 1000, 0, 0, 0.0100);
    both["claude-haiku-5-5"] =
        model_usage("claude-haiku-5-5", 300, 20, 0, 0, 0, 0.0004)["claude-haiku-5-5"].clone();
    let all = feed_result(
        &mut session,
        json!({"usage": usage, "total_cost_usd": 0.0104, "model_usage_since_last_result": both}),
    );
    let other = only(&all, UsageScope::OtherCalls);
    assert_eq!(
        (
            other.input,
            other.output,
            other.cache_read,
            other.cache_write
        ),
        (Some(300), Some(20), Some(0), Some(0))
    );
    assert!(none_of(&all, UsageScope::Compaction));
    assert_eq!(only(&all, UsageScope::Turn).input, Some(4));
}

#[test]
fn a_turn_whose_model_usage_equals_its_usage_has_no_other_model_call() {
    let mut session = session();
    let all = feed_result(
        &mut session,
        json!({"usage": result_usage(4, 270, 46000, 38000, 0), "total_cost_usd": 0.0084,
               "model_usage_since_last_result": model_usage("claude-haiku-5-5", 4, 270, 46000, 38000, 0, 0.0084)}),
    );
    assert!(none_of(&all, UsageScope::Compaction) && none_of(&all, UsageScope::OtherCalls));
}

#[test]
fn model_usage_over_several_models_is_summed_before_the_turns_own_usage_is_taken_away() {
    let mut session = session();
    let mut models = model_usage("claude-sonnet-5", 4, 200, 1000, 0, 0, 0.01);
    models["claude-haiku-5-5"] =
        model_usage("claude-haiku-5-5", 0, 70, 0, 0, 0, 0.001)["claude-haiku-5-5"].clone();
    let all = feed_result(
        &mut session,
        json!({"usage": result_usage(4, 200, 1000, 0, 0), "model_usage_since_last_result": models}),
    );
    assert_eq!(only(&all, UsageScope::OtherCalls).output, Some(70));
}

#[test]
fn a_result_with_model_usage_but_no_usage_of_its_own_gives_no_other_call() {
    let mut session = session();
    let mut result = serde_json::from_slice::<Value>(&result_line(json!({
        "model_usage_since_last_result": model_usage("claude-haiku-5-5", 10, 5, 0, 0, 0, 0.001)})))
    .unwrap();
    result.as_object_mut().unwrap().remove("usage");
    let all = events(&session.feed(&line(result), 9));
    assert!(none_of(&all, UsageScope::Compaction) && none_of(&all, UsageScope::OtherCalls));
}

// ---- a phase cut before its result ---------------------------------------------------------------------------------

#[test]
fn a_phase_cut_before_its_result_keeps_its_responses_input_and_cache_tokens_and_no_cost() {
    let mut session = session();
    session.send_turn("go");
    session.feed(
        &response("msg_01", text("a"), stream_usage(2, 10, 20, 1)),
        1,
    );
    session.feed(
        &response("msg_01", tool_use("t"), stream_usage(2, 10, 20, 1)),
        1,
    );
    session.feed(&response("msg_02", text("b"), stream_usage(3, 0, 40, 1)), 2);
    let finished = session.finish(&exit_with(143));
    let all = events(&finished.step);
    assert_eq!(
        usages(&all),
        [Usage {
            input: Some(5),
            cache_read: Some(60),
            cache_write: Some(10),
            cache_write_1h: Some(10),
            cache_write_5m: Some(0),
            context_used: Some(43),
            ..blank(UsageScope::Turn, UsageQuality::Partial)
        }],
        "output and cost are not reported, never 0; each response once"
    );
    assert!(
        finished
            .step
            .events
            .iter()
            .all(|stamped| stamped.wall_ns.is_none()),
        "the session reads no clock"
    );
}

#[test]
fn a_phase_cut_with_a_turn_sent_and_no_response_is_unreported() {
    let mut session = session();
    session.send_turn("go");
    let all = events(&session.finish(&exit_with(143)).step);
    assert_eq!(
        usages(&all),
        [blank(UsageScope::Turn, UsageQuality::Unreported)]
    );
}

#[test]
fn a_phase_with_no_turn_sent_has_no_usage_to_give() {
    let mut session = session();
    assert!(events(&session.finish(&exit_with(0)).step).is_empty());
}

#[test]
fn a_phase_whose_last_turn_ended_gives_no_second_usage_at_finish() {
    let mut session = session();
    session.send_turn("go");
    session.feed(
        &response("msg_01", text("a"), stream_usage(2, 10, 20, 1)),
        1,
    );
    feed_result(&mut session, json!({}));
    assert!(usages(&events(&session.finish(&exit_with(0)).step)).is_empty());
}

#[test]
fn a_phase_lost_with_no_exit_record_keeps_what_it_saw_too() {
    let mut session = session();
    session.send_turn("go");
    session.feed(
        &response("msg_01", text("a"), stream_usage(2, 10, 20, 1)),
        1,
    );
    let lost = collide_core::adapter::ExitInfo {
        record: None,
        stderr_tail: String::new(),
    };
    let all = events(&session.finish(&lost).step);
    assert_eq!(only(&all, UsageScope::Turn).input, Some(2));
}

// ---- an error result's tokens --------------------------------------------------------------------------------------

#[test]
fn an_error_results_tokens_are_carried_as_the_turns_like_any_other_results() {
    let mut session = session();
    let all = feed_result(
        &mut session,
        json!({"is_error": true, "subtype": "error_max_budget_usd", "result": null, "stop_reason": "tool_use",
               "total_cost_usd": 0.00353, "duration_api_ms": 1800, "usage": result_usage(4, 30, 9000, 0, 0)}),
    );
    let turn = only(&all, UsageScope::Turn);
    assert_eq!(
        (turn.input, turn.output, turn.cache_read),
        (Some(4), Some(30), Some(9000))
    );
    assert_cost(&turn, Some(0.00353));
    assert_eq!(turn.model_time_ms, Some(1800));
    assert_cost(&only(&all, UsageScope::RunningTotal), Some(0.00353));
}

#[test]
fn an_error_result_with_no_cost_still_carries_its_tokens_and_gives_no_running_total() {
    let mut session = session();
    let all = feed_result(
        &mut session,
        json!({"is_error": true, "api_error_status": 500, "result": "API Error: 500", "usage": result_usage(1, 2, 3, 4, 0)}),
    );
    assert_eq!(only(&all, UsageScope::Turn).output, Some(2));
    assert_cost(&only(&all, UsageScope::Turn), None);
    assert!(none_of(&all, UsageScope::RunningTotal));
}

#[test]
fn nothing_an_error_result_spent_is_in_two_events_of_the_same_kind_and_a_steps_are_not_added_to_it()
{
    let mut session = session();
    session.feed(
        &response("msg_01", text("a"), stream_usage(2, 0, 100, 1)),
        1,
    );
    let all = feed_result(
        &mut session,
        json!({"is_error": true, "api_error_status": 500, "result": "API Error: 500",
               "usage": result_usage(2, 7, 100, 0, 0), "total_cost_usd": 0.001}),
    );
    assert_eq!(
        usages(&all)
            .iter()
            .filter(|u| u.scope == UsageScope::Turn)
            .count(),
        1
    );
    assert_eq!(
        only(&all, UsageScope::Turn).input,
        Some(2),
        "once, from result.usage"
    );
}

// ---- the recordings ------------------------------------------------------------------------------------------------

/// The Turn, Compaction and RunningTotal usages among the events of `played`, in order, for each turn.
fn per_turn(all: &[AgentEvent]) -> Vec<Vec<Usage>> {
    let mut turns = vec![Vec::new()];
    for event in all {
        match event {
            AgentEvent::Usage { usage } if usage.scope != UsageScope::Step => {
                turns.last_mut().unwrap().push(usage.clone());
            }
            AgentEvent::TurnEnd { .. } => turns.push(Vec::new()),
            _ => {}
        }
    }
    turns.pop();
    turns
}

fn scope_of(usages: &[Usage], scope: UsageScope) -> &Usage {
    usages
        .iter()
        .find(|usage| usage.scope == scope)
        .expect("a usage of that scope")
}

#[test]
fn the_usage_session_recording_gives_each_turn_its_differences_a_compaction_of_its_own_and_a_blocked_prompt_nothing()
 {
    let played = play(
        "usage-session",
        assigned(),
        &["build it", "/compact", "do the task", "continue"],
        answer_nothing,
    );
    let all = named(&played);
    let turns = per_turn(&all);
    assert_eq!(turns.len(), 4);

    // 1. Two responses over three rows: the turn's tokens are result.usage's.
    let first = scope_of(&turns[0], UsageScope::Turn);
    assert_eq!(
        (
            first.input,
            first.output,
            first.cache_read,
            first.cache_write
        ),
        (Some(4), Some(270), Some(46000), Some(38000))
    );
    assert_eq!(first.model_time_ms, Some(2300));
    assert_cost(first, Some(0.0084));
    let steps: Vec<Usage> = all
        .iter()
        .take_while(|e| !matches!(e, AgentEvent::TurnEnd { .. }))
        .filter_map(|e| match e {
            AgentEvent::Usage { usage } if usage.scope == UsageScope::Step => Some(usage.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(steps.len(), 2, "msg_01 over two rows, and msg_02");
    assert_eq!(
        steps.iter().filter_map(|s| s.input).sum::<u64>(),
        4,
        "input counted once per response"
    );

    // 2. /compact: the compaction's own usage, nothing in the turn's.
    let compaction = scope_of(&turns[1], UsageScope::Compaction);
    assert_eq!(
        (
            compaction.input,
            compaction.output,
            compaction.cache_write_5m
        ),
        (Some(22500), Some(1500), Some(181))
    );
    assert_cost(compaction, Some(0.0034));
    let compact_turn = scope_of(&turns[1], UsageScope::Turn);
    assert_eq!(
        (
            compact_turn.input,
            compact_turn.output,
            compact_turn.cache_read
        ),
        (Some(0), Some(0), Some(0))
    );
    assert_eq!(compact_turn.model_time_ms, None);

    // 3. The blocked prompt spent nothing, and has no model time.
    let blocked = scope_of(&turns[2], UsageScope::Turn);
    assert_eq!(blocked.model_time_ms, None);
    assert_cost(blocked, Some(0.0));
    assert!(
        turns[2]
            .iter()
            .all(|usage| usage.scope != UsageScope::Compaction)
    );

    // 4. Differences from the last result with turns (model time) and from the result before (cost).
    let last = scope_of(&turns[3], UsageScope::Turn);
    assert_eq!(last.model_time_ms, Some(8000));
    assert_cost(last, Some(0.0040));
    assert_eq!((last.input, last.output), (Some(2), Some(170)));

    // The cost of every turn and the compaction adds up to the session's last running total.
    let spent: f64 = turns
        .iter()
        .flatten()
        .filter(|u| u.scope != UsageScope::RunningTotal)
        .filter_map(amount)
        .sum();
    assert!((spent - 0.0158).abs() < 1e-9, "{spent}");
    assert!(matches!(
        all.last(),
        Some(AgentEvent::TurnEnd {
            reason: TurnEndReason::EndTurn {},
            ..
        })
    ));
}
