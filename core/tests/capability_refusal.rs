//! Tests for the capability refusal (ts-0060, 0.3 Capabilities): a role that requires a capability is refused
//! unless the adapter says `Yes`, and the refusal names the agent and the capability as the role wrote it.

use collide_core::adapter::{CapabilityRefusal, refuse_unmet_capabilities};
use collide_core::agent::{
    CAPABILITY_NAMES, Capabilities, ContextOptions, McpServers, ModelSelect, PermissionHost,
    SessionResume, Support, SystemPrompt, UsageReport,
};

fn names(list: &[&str]) -> Vec<String> {
    list.iter().map(|name| name.to_string()).collect()
}

fn refusal(agent: &str, capability: &str, support: Support) -> CapabilityRefusal {
    CapabilityRefusal {
        agent: agent.into(),
        capability: capability.into(),
        support,
    }
}

#[test]
fn a_capability_the_adapter_says_yes_to_is_met() {
    let table = Capabilities {
        session_resume: SessionResume::Replay,
        headless_stream: Support::Yes,
        ..Capabilities::default()
    };
    let result = refuse_unmet_capabilities(
        "claude",
        &table,
        &names(&["headless_stream", "session_resume"]),
    );
    assert_eq!(result, Ok(()));
}

#[test]
fn a_capability_the_adapter_says_no_to_is_refused_with_what_it_said() {
    let table = Capabilities {
        background: Support::No,
        ..Capabilities::default()
    };
    let result = refuse_unmet_capabilities("opencode", &table, &names(&["background"]));
    assert_eq!(
        result,
        Err(vec![refusal("opencode", "background", Support::No)])
    );
}

#[test]
fn a_capability_the_adapter_has_not_probed_is_refused_as_unknown() {
    let result =
        refuse_unmet_capabilities("goose", &Capabilities::default(), &names(&["limit_signal"]));
    assert_eq!(
        result,
        Err(vec![refusal("goose", "limit_signal", Support::Unknown)])
    );
}

#[test]
fn an_empty_requirement_is_met_by_a_table_of_unknowns() {
    assert_eq!(
        refuse_unmet_capabilities("goose", &Capabilities::default(), &[]),
        Ok(())
    );
}

#[test]
fn every_unmet_capability_is_returned_in_the_order_the_role_wrote_them_and_once() {
    let table = Capabilities {
        headless_stream: Support::Yes,
        hook_events: Support::No,
        ..Capabilities::default()
    };
    let result = refuse_unmet_capabilities(
        "tool",
        &table,
        &names(&[
            "hook_events",
            "headless_stream",
            "compaction_events",
            "hook_events",
        ]),
    );
    assert_eq!(
        result,
        Err(vec![
            refusal("tool", "hook_events", Support::No),
            refusal("tool", "compaction_events", Support::Unknown),
        ])
    );
}

#[test]
fn a_sub_flag_is_named_as_the_role_wrote_it() {
    let table = Capabilities {
        permission_host: PermissionHost {
            support: Support::Yes,
            deny_message: Support::Yes,
            input_at_request: Support::No,
            request_reason: Support::Unknown,
        },
        mcp_servers: McpServers {
            http: Support::Yes,
            ..McpServers::default()
        },
        ..Capabilities::default()
    };
    let result = refuse_unmet_capabilities(
        "tool",
        &table,
        &names(&[
            "permission_host",
            "permission_host.deny_message",
            "permission_host.input_at_request",
            "permission_host.request_reason",
            "mcp_servers.http",
            "mcp_servers.sse",
        ]),
    );
    assert_eq!(
        result,
        Err(vec![
            refusal("tool", "permission_host.input_at_request", Support::No),
            refusal("tool", "permission_host.request_reason", Support::Unknown),
            refusal("tool", "mcp_servers.sse", Support::Unknown),
        ])
    );
}

#[test]
fn a_capability_with_a_mode_is_met_by_any_mode_but_none_and_unknown() {
    let met = Capabilities {
        session_resume: SessionResume::Silent,
        system_prompt: SystemPrompt::Append,
        model_select: ModelSelect::Launch,
        ..Capabilities::default()
    };
    let required = names(&["session_resume", "system_prompt", "model_select"]);
    assert_eq!(refuse_unmet_capabilities("tool", &met, &required), Ok(()));

    let none = Capabilities {
        session_resume: SessionResume::None,
        system_prompt: SystemPrompt::None,
        model_select: ModelSelect::None,
        ..Capabilities::default()
    };
    assert_eq!(
        refuse_unmet_capabilities("tool", &none, &required),
        Err(vec![
            refusal("tool", "session_resume", Support::No),
            refusal("tool", "system_prompt", Support::No),
            refusal("tool", "model_select", Support::No),
        ])
    );

    assert_eq!(
        refuse_unmet_capabilities("tool", &Capabilities::default(), &required),
        Err(vec![
            refusal("tool", "session_resume", Support::Unknown),
            refusal("tool", "system_prompt", Support::Unknown),
            refusal("tool", "model_select", Support::Unknown),
        ])
    );
}

#[test]
fn the_usage_and_context_sub_flags_are_read() {
    let table = Capabilities {
        usage_report: UsageReport {
            support: Support::Yes,
            cost: Support::Yes,
            ..UsageReport::default()
        },
        context_options: ContextOptions {
            compact_at: Support::Yes,
            ..ContextOptions::default()
        },
        ..Capabilities::default()
    };
    assert_eq!(
        refuse_unmet_capabilities(
            "tool",
            &table,
            &names(&[
                "usage_report",
                "usage_report.cost",
                "context_options.compact_at"
            ])
        ),
        Ok(())
    );
    assert_eq!(
        refuse_unmet_capabilities(
            "tool",
            &table,
            &names(&["usage_report.context", "context_options.bash_output_max"])
        ),
        Err(vec![
            refusal("tool", "usage_report.context", Support::Unknown),
            refusal("tool", "context_options.bash_output_max", Support::Unknown),
        ])
    );
}

#[test]
fn a_name_that_is_not_a_capability_is_refused_rather_than_met() {
    let result = refuse_unmet_capabilities("tool", &Capabilities::default(), &names(&["teleport"]));
    assert_eq!(
        result,
        Err(vec![refusal("tool", "teleport", Support::Unknown)])
    );
}

#[test]
fn every_named_capability_has_a_reading() {
    let table = Capabilities::default();
    for name in CAPABILITY_NAMES {
        assert!(table.support_of(name).is_some(), "{name}");
    }
}

#[test]
fn the_refusal_message_names_the_agent_and_the_capability() {
    let text = refusal("opencode", "permission_host.deny_message", Support::No).to_string();
    assert!(text.contains("opencode"), "{text}");
    assert!(text.contains("permission_host.deny_message"), "{text}");
}
