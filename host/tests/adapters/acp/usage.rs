//! What an ACP turn spent (ts-0091-pz; 0.3, Usage; S7, O2, O3). The figures come from `session/prompt`'s `usage`
//! (unstable in ACP), the quality is the profile's claim and not the session's measurement, a figure the agent did not
//! give is absent and never 0, and a turn with no `usage` still gives one `Unreported` event.

use super::fake::{play, recording};
use super::session::{SESSION, events_of, ready_with};
use collide_core::adapter::{AgentAdapter, AgentSession, Step};
use collide_core::agent::{AgentEvent, Usage, UsageQuality, UsageReport, UsageScope};
use collide_core::replay::replay_journal_bytes;
use collide_core::testing::acp::{message_chunk, prompt_error, prompt_response, tool_call};
use collide_core::testing::sample_launch_request;
use collide_host::adapters::acp::AcpAdapter;
use collide_host::adapters::acp::options::AcpOptions;
use collide_host::adapters::acp::profile::AcpProfile;
use collide_host::adapters::acp::session::AcpSession;
use serde_json::{Value, json};

fn claiming(quality: UsageQuality) -> AcpOptions {
    AcpOptions::new("/work/repo", "do the task").with_usage(quality, false)
}

fn usages(step: &Step) -> Vec<Usage> {
    events_of(step)
        .into_iter()
        .filter_map(|event| match event {
            AgentEvent::Usage { usage } => Some(usage),
            _ => None,
        })
        .collect()
}

/// The one usage event the response to the first prompt gives.
fn usage_of(options: AcpOptions, response: Vec<u8>) -> Usage {
    let mut session = ready_with(options);
    let step = session.feed(&response, 600);
    let mut all = usages(&step);
    assert_eq!(
        all.len(),
        1,
        "one usage event for each turn: {:?}",
        step.events
    );
    all.remove(0)
}

fn with_usage(usage: Value) -> Vec<u8> {
    prompt_response(2, "end_turn", Some(usage))
}

fn full() -> Value {
    json!({
        "totalTokens": 9999, "inputTokens": 34, "outputTokens": 49,
        "cachedReadTokens": 9331, "cachedWriteTokens": 7, "thoughtTokens": 11
    })
}

/// A usage with no figure at all and the quality `quality`: what the session makes when nothing is reported.
fn nothing(quality: UsageQuality) -> Usage {
    Usage {
        scope: UsageScope::Turn,
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

// ---- partial and exact ----------------------------------------------------------------------------------------------

#[test]
fn a_response_with_usage_gives_a_partial_turn_figure_when_the_profile_says_partial() {
    let usage = usage_of(claiming(UsageQuality::Partial), with_usage(full()));
    assert_eq!(
        usage,
        Usage {
            input: Some(34),
            output: Some(49),
            cache_read: Some(9331),
            cache_write: Some(7),
            ..nothing(UsageQuality::Partial)
        }
    );
}

#[test]
fn a_response_with_usage_gives_an_exact_figure_when_the_profile_says_exact() {
    let usage = usage_of(claiming(UsageQuality::Exact), with_usage(full()));
    assert_eq!(usage.quality, UsageQuality::Exact);
    assert_eq!(usage.scope, UsageScope::Turn);
    assert_eq!(usage.input, Some(34));
}

#[test]
fn each_figure_is_absent_when_the_agent_did_not_send_it_and_the_others_stay() {
    for (key, absent) in [
        ("inputTokens", "input"),
        ("outputTokens", "output"),
        ("cachedReadTokens", "cache_read"),
        ("cachedWriteTokens", "cache_write"),
    ] {
        let mut body = full();
        body.as_object_mut().unwrap().remove(key);
        let usage = usage_of(claiming(UsageQuality::Partial), with_usage(body));
        let figures = [
            ("input", usage.input),
            ("output", usage.output),
            ("cache_read", usage.cache_read),
            ("cache_write", usage.cache_write),
        ];
        for (name, figure) in figures {
            assert_eq!(
                figure.is_none(),
                name == absent,
                "{key}: {name} is {figure:?}"
            );
        }
        assert_eq!(usage.quality, UsageQuality::Partial, "{key}");
    }
}

#[test]
fn a_figure_of_zero_the_agent_sent_stays_zero_and_one_it_did_not_send_stays_absent() {
    let usage = usage_of(
        claiming(UsageQuality::Partial),
        with_usage(json!({ "inputTokens": 0, "outputTokens": 12 })),
    );
    assert_eq!(usage.input, Some(0));
    assert_eq!(usage.output, Some(12));
    assert_eq!(usage.cache_read, None);
    assert_eq!(usage.cache_write, None);
}

#[test]
fn a_turn_cancelled_after_a_zero_usage_reports_the_zeros_the_agent_said() {
    // S7, O2: opencode gave zeros after a cancel although a step had run. They are the agent's figure, partial.
    let mut session = ready_with(claiming(UsageQuality::Partial));
    session.cancel();
    let step = session.feed(
        &with_usage(json!({ "inputTokens": 0, "outputTokens": 0 })),
        600,
    );
    let usage = usages(&step).remove(0);
    assert_eq!((usage.input, usage.output), (Some(0), Some(0)));
    assert_eq!(usage.quality, UsageQuality::Partial);
}

#[test]
fn a_figure_that_is_not_a_whole_non_negative_number_is_absent() {
    let usage = usage_of(
        claiming(UsageQuality::Partial),
        with_usage(json!({
            "inputTokens": "34", "outputTokens": -3, "cachedReadTokens": 1.5, "cachedWriteTokens": 8
        })),
    );
    assert_eq!(
        (
            usage.input,
            usage.output,
            usage.cache_read,
            usage.cache_write
        ),
        (None, None, None, Some(8))
    );
}

#[test]
fn the_session_never_makes_a_cost_a_model_time_or_a_context_figure() {
    let mut body = full();
    body["cost"] = json!({ "amount": 0.5, "currency": "USD" });
    body["used"] = json!(1000);
    let usage = usage_of(claiming(UsageQuality::Exact), with_usage(body));
    assert_eq!(usage.cost, None, "the host never invents a cost");
    assert_eq!(usage.model_time_ms, None);
    assert_eq!((usage.context_used, usage.context_size), (None, None));
    assert_eq!((usage.cache_write_1h, usage.cache_write_5m), (None, None));
}

// ---- thought tokens -------------------------------------------------------------------------------------------------

#[test]
fn thought_tokens_are_dropped_unless_the_profile_says_the_output_includes_them() {
    let dropped = usage_of(claiming(UsageQuality::Partial), with_usage(full()));
    assert_eq!(dropped.thought, None);
    assert_eq!(dropped.output, Some(49), "and never added to the output");
    let options =
        AcpOptions::new("/work/repo", "do the task").with_usage(UsageQuality::Partial, true);
    let kept = usage_of(options, with_usage(full()));
    assert_eq!(kept.thought, Some(11), "a part of the output, as reported");
    assert_eq!(
        kept.output,
        Some(49),
        "the output is as the agent reported it"
    );
}

#[test]
fn a_thought_figure_alone_is_a_turn_that_spent_something() {
    // Dropped from the figures, but the agent did spend: the turn is not empty.
    let mut session = ready_with(claiming(UsageQuality::Partial));
    let step = session.feed(&with_usage(json!({ "thoughtTokens": 5 })), 600);
    assert!(matches!(
        events_of(&step).last(),
        Some(AgentEvent::TurnEnd { empty: false, .. })
    ));
}

// ---- unreported -----------------------------------------------------------------------------------------------------

#[test]
fn a_response_with_no_usage_gives_an_unreported_event_with_every_figure_absent() {
    for quality in [
        UsageQuality::Partial,
        UsageQuality::Exact,
        UsageQuality::Unreported,
    ] {
        let usage = usage_of(claiming(quality), prompt_response(2, "end_turn", None));
        assert_eq!(usage, nothing(UsageQuality::Unreported), "{quality:?}");
    }
}

#[test]
fn a_usage_with_no_figure_the_session_reads_is_unreported() {
    for body in [
        json!({}),
        json!({ "totalTokens": 12 }),
        json!(null),
        json!("lots"),
    ] {
        let usage = usage_of(claiming(UsageQuality::Partial), with_usage(body.clone()));
        assert_eq!(usage, nothing(UsageQuality::Unreported), "{body}");
    }
}

#[test]
fn a_profile_that_says_nothing_gives_unreported_for_all_and_drops_the_figures() {
    // The default options claim nothing. Figures with no claim behind them would show as exact in a total.
    let usage = usage_of(
        AcpOptions::new("/work/repo", "do the task"),
        with_usage(full()),
    );
    assert_eq!(usage, nothing(UsageQuality::Unreported));
}

#[test]
fn an_unreported_claim_in_the_profile_is_the_same_as_none() {
    let usage = usage_of(claiming(UsageQuality::Unreported), with_usage(full()));
    assert_eq!(usage, nothing(UsageQuality::Unreported));
}

#[test]
fn a_turn_that_ended_in_an_error_response_still_gives_one_unreported_event() {
    let usage = usage_of(
        claiming(UsageQuality::Partial),
        prompt_error(2, -32603, "x"),
    );
    assert_eq!(usage, nothing(UsageQuality::Unreported));
}

#[test]
fn usage_the_agent_sent_with_no_words_still_counts_for_the_empty_turn_check_when_the_profile_says_nothing()
 {
    let mut session = ready_with(AcpOptions::new("/work/repo", "do the task"));
    let step = session.feed(&with_usage(json!({ "inputTokens": 34 })), 600);
    assert!(matches!(
        events_of(&step).last(),
        Some(AgentEvent::TurnEnd { empty: false, .. })
    ));
}

// ---- where the event sits -------------------------------------------------------------------------------------------

#[test]
fn the_usage_comes_before_the_final_report_and_the_turn_end_and_has_the_lines_time() {
    let mut session = ready_with(claiming(UsageQuality::Partial));
    session.feed(&message_chunk(SESSION, "Let me look. "), 500);
    session.feed(&tool_call(SESSION, "c1", "Read a", "read"), 510);
    session.feed(&message_chunk(SESSION, "The notes say hello."), 520);
    let step = session.feed(&with_usage(full()), 777);
    let kinds: Vec<&str> = events_of(&step)
        .iter()
        .map(|event| match event {
            AgentEvent::Usage { .. } => "usage",
            AgentEvent::FinalReport { .. } => "final_report",
            AgentEvent::TurnEnd { .. } => "turn_end",
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(kinds, ["usage", "final_report", "turn_end"]);
    assert!(step.events.iter().all(|event| event.wall_ns == Some(777)));
}

#[test]
fn each_turn_gives_its_own_usage_event() {
    let mut session = ready_with(claiming(UsageQuality::Partial));
    let first = session.feed(&with_usage(json!({ "inputTokens": 1 })), 600);
    session.send_turn("again");
    let second = session.feed(&prompt_response(3, "end_turn", None), 700);
    assert_eq!(usages(&first).len(), 1);
    assert_eq!(usages(&second), vec![nothing(UsageQuality::Unreported)]);
}

// ---- the adapter hands the profile's claims to its session ----------------------------------------------------------

fn adapter_with(quality: Option<UsageQuality>, includes_thoughts: bool) -> AcpAdapter {
    let mut profile = AcpProfile::new("test-agent", "test-acp-agent", vec![]);
    profile.usage_report = UsageReport {
        quality,
        ..UsageReport::default()
    };
    profile.output_includes_thoughts = includes_thoughts;
    AcpAdapter::new(profile)
}

fn through_adapter(adapter: &AcpAdapter) -> Usage {
    let mut request = sample_launch_request();
    request.cwd = "/work/repo".into();
    let mut session = adapter.session(&request);
    session.start();
    session.feed(&collide_core::testing::acp::initialize_response(0), 100);
    session.feed(
        &collide_core::testing::acp::new_session_response(1, SESSION),
        110,
    );
    let step = session.feed(&with_usage(full()), 600);
    usages(&step).remove(0)
}

#[test]
fn the_adapters_session_has_the_profiles_usage_quality() {
    for quality in [UsageQuality::Partial, UsageQuality::Exact] {
        let usage = through_adapter(&adapter_with(Some(quality), false));
        assert_eq!(usage.quality, quality);
        assert_eq!(usage.input, Some(34));
    }
    let usage = through_adapter(&adapter_with(None, false));
    assert_eq!(usage, nothing(UsageQuality::Unreported));
}

#[test]
fn the_adapters_session_has_the_profiles_thought_rule() {
    let usage = through_adapter(&adapter_with(Some(UsageQuality::Partial), true));
    assert_eq!(usage.thought, Some(11));
    let usage = through_adapter(&adapter_with(Some(UsageQuality::Partial), false));
    assert_eq!(usage.thought, None);
}

// ---- the fake child under the real shim -----------------------------------------------------------------------------

fn options() -> AcpOptions {
    claiming(UsageQuality::Partial)
}

#[test]
fn a_recording_with_usage_gives_a_partial_event_with_only_the_figures_it_carried() {
    let played = play(&recording("prompt-turn"), options(), &[], |_, _| {
        Step::default()
    });
    let usage = played
        .agent_events()
        .into_iter()
        .find_map(|event| match event {
            AgentEvent::Usage { usage } => Some(usage),
            _ => None,
        })
        .expect("a usage event");
    assert_eq!(
        usage,
        Usage {
            input: Some(180),
            output: Some(30),
            cache_read: Some(120),
            ..nothing(UsageQuality::Partial)
        },
        "no cache write: the recording sent none, and it is not 0"
    );
}

#[test]
fn a_recording_with_no_usage_gives_an_unreported_event() {
    let played = play(
        &recording("prompt-turn-no-usage"),
        options(),
        &[],
        |_, _| Step::default(),
    );
    let all: Vec<Usage> = played
        .agent_events()
        .into_iter()
        .filter_map(|event| match event {
            AgentEvent::Usage { usage } => Some(usage),
            _ => None,
        })
        .collect();
    assert_eq!(all, vec![nothing(UsageQuality::Unreported)]);
}

#[test]
fn replayed_from_the_journal_the_usage_events_are_the_live_ones() {
    for name in ["prompt-turn", "prompt-turn-no-usage"] {
        let played = play(&recording(name), options(), &[], |_, _| Step::default());
        let mut rebuilt = AcpSession::new(options());
        let replay =
            replay_journal_bytes(&mut rebuilt, &played.journal).expect("the journal replays");
        assert_eq!(replay.step.events, played.fed_events(), "{name}");
        assert_eq!(rebuilt, played.session, "{name}");
    }
}
