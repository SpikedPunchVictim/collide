//! The `initialize` probe (ts-0088-pk): what an `initialize` response implies for the capability table, with `Unknown`
//! for what it leaves out, and the merge of that answer into a profile's table.

use collide_core::agent::{McpServers, SessionResume, Support};
use collide_host::adapters::acp::probe::{Probed, probe};
use collide_host::adapters::acp::profile::AcpProfile;
use serde_json::{Value, json};

/// A successful `initialize` response whose `agentCapabilities` is `capabilities`.
fn response(capabilities: Value) -> Value {
    json!({
        "jsonrpc": "2.0", "id": 0,
        "result": {
            "protocolVersion": 1,
            "agentCapabilities": capabilities,
            "agentInfo": { "name": "stand-in-agent", "version": "0" },
            "authMethods": []
        }
    })
}

fn nothing_known() -> Probed {
    Probed {
        session_resume: SessionResume::Unknown,
        mcp_http: Support::Unknown,
    }
}

// ---- loadSession -> session_resume --------------------------------------------------------------------------------

#[test]
fn load_session_true_gives_replay_false_gives_none_and_absent_or_odd_gives_unknown() {
    let cases = [
        (json!({ "loadSession": true }), SessionResume::Replay),
        (json!({ "loadSession": false }), SessionResume::None),
        (json!({}), SessionResume::Unknown),
        (json!({ "loadSession": "yes" }), SessionResume::Unknown),
        (json!({ "loadSession": null }), SessionResume::Unknown),
    ];
    for (capabilities, want) in cases {
        let probed = probe(&response(capabilities.clone()));
        assert_eq!(probed.session_resume, want, "{capabilities}");
    }
}

// ---- mcpCapabilities -> mcp_servers -------------------------------------------------------------------------------

#[test]
fn http_is_read_from_the_mcp_capabilities_and_is_unknown_when_the_response_leaves_it_out() {
    let cases = [
        (
            json!({ "mcpCapabilities": { "http": true, "sse": false } }),
            Support::Yes,
        ),
        (
            json!({ "mcpCapabilities": { "http": false, "sse": false } }),
            Support::No,
        ),
        (
            json!({ "mcpCapabilities": { "sse": true } }),
            Support::Unknown,
        ),
        (json!({ "mcpCapabilities": {} }), Support::Unknown),
        (
            json!({ "mcpCapabilities": { "http": "yes" } }),
            Support::Unknown,
        ),
        (json!({ "mcpCapabilities": null }), Support::Unknown),
        (json!({}), Support::Unknown),
    ];
    for (capabilities, want) in cases {
        let probed = probe(&response(capabilities.clone()));
        assert_eq!(probed.mcp_http, want, "{capabilities}");
    }
}

#[test]
fn what_the_probe_says_about_mcp_servers_in_the_merged_table() {
    let base = AcpProfile::new("test-agent", "test-acp-agent", vec![]).capabilities();
    let cases = [
        (
            json!({ "mcpCapabilities": { "http": true, "sse": false } }),
            McpServers {
                support: Support::Yes,
                stdio: Support::Yes,
                http: Support::Yes,
                sse: Support::No,
            },
        ),
        // `sse` is No whatever the agent says: the engine has no `sse` form, so Collide cannot use one.
        (
            json!({ "mcpCapabilities": { "http": false, "sse": true } }),
            McpServers {
                support: Support::Yes,
                stdio: Support::Yes,
                http: Support::No,
                sse: Support::No,
            },
        ),
        // `stdio` is Yes whatever the agent says: ACP requires every agent to take it.
        (
            json!({ "mcpCapabilities": { "http": true, "stdio": false } }),
            McpServers {
                support: Support::Yes,
                stdio: Support::Yes,
                http: Support::Yes,
                sse: Support::No,
            },
        ),
        (
            json!({}),
            McpServers {
                support: Support::Yes,
                stdio: Support::Yes,
                http: Support::Unknown,
                sse: Support::No,
            },
        ),
    ];
    for (capabilities, want) in cases {
        let merged = probe(&response(capabilities.clone())).merge_into(base.clone());
        assert_eq!(merged.mcp_servers, want, "{capabilities}");
    }
}

// ---- a response that says little, and a response that says more than the probe reads ------------------------------

#[test]
fn a_response_that_says_nothing_usable_gives_unknown_for_everything_it_covers() {
    let responses = [
        json!({}),
        json!({ "jsonrpc": "2.0", "id": 0, "result": {} }),
        response(json!({})),
        response(json!(null)),
        response(json!("not an object")),
        json!({ "jsonrpc": "2.0", "id": 0, "error": { "code": -32000, "message": "auth_required" } }),
        json!(42),
        json!(null),
    ];
    for value in responses {
        assert_eq!(probe(&value), nothing_known(), "{value}");
    }
}

#[test]
fn fields_the_probe_does_not_read_change_nothing() {
    let plain = response(json!({ "loadSession": true, "mcpCapabilities": { "http": true } }));
    let busy = json!({
        "jsonrpc": "2.0", "id": 0,
        "result": {
            "protocolVersion": 1,
            "agentCapabilities": {
                "loadSession": true,
                "mcpCapabilities": { "http": true, "sse": true, "stdio": false },
                "promptCapabilities": { "image": true, "audio": true, "embeddedContext": true },
                "sessionCapabilities": { "resume": {}, "list": {}, "systemPrompt": {} },
                "_meta": { "anything": 1 }
            },
            "agentInfo": { "name": "other-agent", "version": "9" },
            "authMethods": [{ "id": "login", "name": "Log in" }],
            "unknownField": []
        }
    });
    assert_eq!(probe(&plain), probe(&busy));
}

#[test]
fn a_loud_response_with_no_load_session_does_not_say_none() {
    let probed = probe(&response(
        json!({ "promptCapabilities": { "image": true } }),
    ));
    assert_eq!(probed.session_resume, SessionResume::Unknown);
}

// ---- the merge ----------------------------------------------------------------------------------------------------

#[test]
fn the_merge_changes_only_session_resume_and_http() {
    let base = AcpProfile::new("test-agent", "test-acp-agent", vec![]).capabilities();
    let probed = Probed {
        session_resume: SessionResume::Replay,
        mcp_http: Support::Yes,
    };
    let mut want = base.clone();
    want.session_resume = SessionResume::Replay;
    want.mcp_servers.http = Support::Yes;
    assert_eq!(probed.merge_into(base), want);
}

#[test]
fn merging_a_probe_that_knew_nothing_changes_nothing() {
    let base = AcpProfile::new("test-agent", "test-acp-agent", vec![]).capabilities();
    assert_eq!(nothing_known().merge_into(base.clone()), base);
}
