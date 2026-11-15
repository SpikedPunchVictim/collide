//! Claude's row of the capability table (0.3, Capabilities). A capability that S1, S7, S9 or S6 did not measure is
//! `Unknown`, so a role that requires it is refused until a contract test says `Yes`.

use collide_core::adapter::{AgentAdapter, refuse_unmet_capabilities};
use collide_core::agent::{
    CAPABILITY_NAMES, ModelSelect, SessionResume, Support, UsageQuality, UsageScope,
};
use collide_host::adapters::claude::ClaudeAdapter;

#[test]
fn every_measured_capability_is_yes() {
    let table = ClaudeAdapter.capabilities();
    for name in [
        "headless_stream",
        "permission_host",
        "permission_host.deny_message",
        "permission_host.input_at_request",
        "permission_host.request_reason",
        "session_resume",
        "budget_native",
        "tool_allowlist",
        "named_agent",
        "usage_report",
        "usage_report.cost",
        "usage_report.cache_write_tiers",
        "usage_report.context",
        "limit_signal",
        "compaction_events",
        "hook_events",
        "refusal_source",
        "strict_config",
        "context_options",
        "context_options.compact_at",
        "context_options.bash_output_max",
        "context_options.mcp_output_max",
        "model_select",
        "take_over",
        "mcp_servers.stdio",
        "mcp_servers.http",
    ] {
        assert_eq!(table.support_of(name), Some(Support::Yes), "{name}");
    }
}

#[test]
fn a_capability_nothing_measured_is_unknown() {
    let table = ClaudeAdapter.capabilities();
    for name in ["background", "system_prompt", "mcp_servers.sse"] {
        assert_eq!(table.support_of(name), Some(Support::Unknown), "{name}");
    }
}

#[test]
fn a_resumed_session_is_silent_and_the_model_is_chosen_on_the_launch_line() {
    let table = ClaudeAdapter.capabilities();
    assert_eq!(table.session_resume, SessionResume::Silent);
    assert_eq!(table.model_select, ModelSelect::Launch);
}

#[test]
fn usage_comes_in_four_scopes_and_is_exact() {
    let usage = ClaudeAdapter.capabilities().usage_report;
    assert_eq!(
        usage.granularity,
        [
            UsageScope::Turn,
            UsageScope::Step,
            UsageScope::RunningTotal,
            UsageScope::Compaction
        ]
    );
    assert_eq!(usage.quality, Some(UsageQuality::Exact));
}

#[test]
fn every_named_capability_has_an_answer() {
    let table = ClaudeAdapter.capabilities();
    for name in CAPABILITY_NAMES {
        assert!(table.support_of(name).is_some(), "{name}");
    }
}

#[test]
fn a_role_that_requires_an_unknown_capability_is_refused_and_one_that_requires_a_yes_is_not() {
    let table = ClaudeAdapter.capabilities();
    assert!(refuse_unmet_capabilities("claude", &table, &["hook_events".into()]).is_ok());
    let refusals = refuse_unmet_capabilities(
        "claude",
        &table,
        &["background".into(), "hook_events".into()],
    )
    .unwrap_err();
    assert_eq!(refusals.len(), 1);
    assert_eq!(refusals[0].capability, "background");
    assert_eq!(refusals[0].support, Support::Unknown);
}
