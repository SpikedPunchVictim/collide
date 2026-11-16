//! The ACP session's handshake, request ids and the lines it must not trust, fed by hand (ts-0087-mf). The bytes the
//! session writes are checked as JSON-RPC messages; the agent's lines are the scripted double's of `core`, or written
//! out here when a test needs a shape the double has no function for.

use collide_core::adapter::{
    AgentSession, ExitInfo, PhaseFailure, PhaseOutcome, StampedEvent, Step,
};
use collide_core::agent::{AgentEvent, TurnEndReason};
use collide_core::shim::{ExitEnd, ExitRecord, SHIM_PROTOCOL};
use collide_core::testing::acp::{initialize_response, new_session_response, prompt_response};
use collide_host::adapters::acp::options::{AcpOptions, McpServer};
use collide_host::adapters::acp::session::AcpSession;
use serde_json::{Value, json};

pub const SESSION: &str = "ses-stand-in-1";

pub fn session() -> AcpSession {
    AcpSession::new(AcpOptions::new("/work/repo", "do the task"))
}

/// A session that is started and has read the handshake (ids 0 and 1), so the first prompt (id 2) is out.
pub fn ready() -> AcpSession {
    let mut session = session();
    session.start();
    session.feed(&initialize_response(0), 100);
    session.feed(&new_session_response(1, SESSION), 110);
    session
}

/// One line of the agent's: the JSON and a newline.
pub fn line(value: Value) -> Vec<u8> {
    let mut bytes = serde_json::to_vec(&value).unwrap();
    bytes.push(b'\n');
    bytes
}

/// What a step writes, as the JSON-RPC messages they are. Each write is one line.
pub fn written(step: &Step) -> Vec<Value> {
    step.writes
        .iter()
        .map(|bytes| {
            assert!(bytes.ends_with(b"\n"), "a write ends with a newline");
            let body = &bytes[..bytes.len() - 1];
            assert!(!body.contains(&b'\n'), "a write is one line");
            serde_json::from_slice(body).expect("a write is JSON")
        })
        .collect()
}

pub fn events_of(step: &Step) -> Vec<AgentEvent> {
    step.events
        .iter()
        .map(|stamped| stamped.event.clone())
        .collect()
}

pub fn exited() -> ExitInfo {
    ExitInfo {
        record: Some(ExitRecord {
            protocol: SHIM_PROTOCOL,
            end: ExitEnd::Exited { code: 0 },
            cpu_user_us: 10,
            cpu_system_us: 5,
            max_rss_bytes: 4096,
        }),
        stderr_tail: String::new(),
    }
}

fn error_response(id: u64, code: i64, message: &str) -> Vec<u8> {
    line(json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } }))
}

// ---- what the session writes ---------------------------------------------------------------------------------------

#[test]
fn start_writes_initialize_with_protocol_version_1_the_client_and_the_session_flags() {
    let step = session().start();
    assert!(step.events.is_empty());
    assert_eq!(
        written(&step),
        vec![json!({
            "jsonrpc": "2.0", "id": 0, "method": "initialize",
            "params": {
                "protocolVersion": 1,
                "clientInfo": { "name": "collide", "version": env!("CARGO_PKG_VERSION") },
                "clientCapabilities": {
                    "session": { "notices": {}, "compaction": {}, "configOptions": {} }
                }
            }
        })]
    );
}

#[test]
fn initialize_names_the_client_and_its_version_and_nothing_else_about_it() {
    let step = session().start();
    let info = written(&step)[0]["params"]["clientInfo"].clone();
    assert_eq!(
        info,
        json!({ "name": "collide", "version": env!("CARGO_PKG_VERSION") })
    );
}

#[test]
fn initialize_holds_no_fs_terminal_or_elicitation() {
    let step = session().start();
    let message = &written(&step)[0];
    let capabilities = message["params"]["clientCapabilities"].as_object().unwrap();
    for name in ["fs", "terminal", "elicitation"] {
        assert!(!capabilities.contains_key(name), "{name} is advertised");
    }
    let text = String::from_utf8(step.writes[0].clone()).unwrap();
    for name in ["\"fs\"", "\"terminal\"", "\"elicitation\""] {
        assert!(!text.contains(name), "{name} is in {text}");
    }
}

#[test]
fn the_initialize_response_makes_it_write_session_new_for_the_working_folder_with_no_servers() {
    let mut session = session();
    session.start();
    let step = session.feed(&initialize_response(0), 100);
    assert!(step.events.is_empty());
    assert_eq!(
        written(&step),
        vec![json!({
            "jsonrpc": "2.0", "id": 1, "method": "session/new",
            "params": { "cwd": "/work/repo", "mcpServers": [] }
        })]
    );
}

#[test]
fn session_new_holds_the_servers_the_options_give_in_the_two_kinds_acp_defines() {
    let options = AcpOptions::new("/work/repo", "go").with_mcp_servers(vec![
        McpServer::Http {
            name: "ask".into(),
            url: "http://127.0.0.1:4100/mcp".into(),
            headers: vec![("Authorization".into(), "Bearer stand-in".into())],
        },
        McpServer::Stdio {
            name: "files".into(),
            command: "/stand-in/bin/files-server".into(),
            args: vec!["--root".into(), "/work/repo".into()],
            env: vec![("MODE".into(), "read".into())],
        },
    ]);
    let mut session = AcpSession::new(options);
    session.start();
    let step = session.feed(&initialize_response(0), 100);
    assert_eq!(
        written(&step)[0]["params"],
        json!({
            "cwd": "/work/repo",
            "mcpServers": [
                {
                    "type": "http", "name": "ask", "url": "http://127.0.0.1:4100/mcp",
                    "headers": [{ "name": "Authorization", "value": "Bearer stand-in" }]
                },
                {
                    "name": "files", "command": "/stand-in/bin/files-server",
                    "args": ["--root", "/work/repo"], "env": [{ "name": "MODE", "value": "read" }]
                }
            ]
        })
    );
}

#[test]
fn a_server_with_no_headers_args_or_env_sends_empty_lists() {
    let servers = [
        McpServer::Http {
            name: "ask".into(),
            url: "http://127.0.0.1:4100/mcp".into(),
            headers: Vec::new(),
        },
        McpServer::Stdio {
            name: "files".into(),
            command: "/stand-in/bin/files-server".into(),
            args: Vec::new(),
            env: Vec::new(),
        },
    ];
    assert_eq!(servers[0].to_json()["headers"], json!([]));
    assert_eq!(servers[1].to_json()["args"], json!([]));
    assert_eq!(servers[1].to_json()["env"], json!([]));
}

#[test]
fn the_session_new_response_binds_the_agents_id_with_its_stored_time_and_writes_the_first_prompt() {
    let mut session = session();
    session.start();
    session.feed(&initialize_response(0), 100);
    let step = session.feed(&new_session_response(1, SESSION), 1_700_000_000_123);
    assert_eq!(
        step.events,
        vec![StampedEvent {
            wall_ns: Some(1_700_000_000_123),
            event: AgentEvent::SessionBound { id: SESSION.into() }
        }]
    );
    assert_eq!(
        written(&step),
        vec![json!({
            "jsonrpc": "2.0", "id": 2, "method": "session/prompt",
            "params": { "sessionId": SESSION, "prompt": [{ "type": "text", "text": "do the task" }] }
        })]
    );
}

// ---- request ids ---------------------------------------------------------------------------------------------------

#[test]
fn the_first_requests_are_0_1_and_2_and_the_next_turn_takes_3() {
    let mut session = session();
    let mut ids = Vec::new();
    let mut take = |step: &Step| ids.extend(written(step).iter().map(|m| m["id"].clone()));
    take(&session.start());
    take(&session.feed(&initialize_response(0), 100));
    take(&session.feed(&new_session_response(1, SESSION), 110));
    take(&session.send_turn("again"));
    assert_eq!(ids, vec![json!(0), json!(1), json!(2), json!(3)]);
}

#[test]
fn a_response_that_matches_no_request_changes_nothing() {
    // Before `start`, nothing was asked, so nothing can be answered.
    let mut fresh = session();
    let before = fresh.clone();
    assert_eq!(fresh.feed(&initialize_response(0), 100), Step::default());
    assert_eq!(fresh, before);

    // After the handshake, an unknown id and a response that was already read are both nothing.
    let mut live = ready();
    let before = live.clone();
    for stray in [
        initialize_response(0),
        new_session_response(1, "ses-other"),
        new_session_response(77, "ses-other"),
        prompt_response(78, "end_turn", None),
        error_response(79, -32000, "no such request"),
        line(json!({ "jsonrpc": "2.0", "id": "text-id", "result": {} })),
        line(json!({ "jsonrpc": "2.0", "result": {} })),
    ] {
        assert_eq!(live.feed(&stray, 200), Step::default(), "{stray:?}");
    }
    assert_eq!(live, before);
}

#[test]
fn a_response_is_matched_to_its_request_by_id_not_by_order() {
    // The prompt (id 2) is answered while another turn (id 3) is out: the answer to 3 first, then 2.
    let mut session = ready();
    session.send_turn("second");
    let first = session.feed(&prompt_response(3, "max_tokens", None), 300);
    let second = session.feed(&prompt_response(2, "end_turn", None), 310);
    let reasons: Vec<TurnEndReason> = [first, second]
        .iter()
        .map(|step| match events_of(step).last() {
            Some(AgentEvent::TurnEnd { reason, .. }) => reason.clone(),
            other => panic!("no turn end last, got {other:?}"),
        })
        .collect();
    assert_eq!(
        reasons,
        vec![TurnEndReason::MaxTokens {}, TurnEndReason::EndTurn {}]
    );
}

// ---- requests of the agent that the engine does not serve -----------------------------------------------------------

#[test]
fn an_agent_request_the_engine_does_not_serve_is_answered_with_method_not_found_and_reported() {
    for method in [
        "fs/read_text_file",
        "fs/write_text_file",
        "terminal/create",
        "terminal/output",
        "elicitation/create",
        "x/made-up-method",
    ] {
        let mut session = ready();
        let request = line(json!({
            "jsonrpc": "2.0", "id": 41, "method": method,
            "params": { "path": "/stand-in/notes.txt", "content": "TOP-SECRET-STAND-IN" }
        }));
        let step = session.feed(&request, 500);
        assert_eq!(
            written(&step),
            vec![json!({
                "jsonrpc": "2.0", "id": 41,
                "error": { "code": -32601, "message": "Method not found" }
            })],
            "{method}"
        );
        assert_eq!(
            step.events,
            vec![StampedEvent {
                wall_ns: Some(500),
                event: AgentEvent::Other {
                    raw_type: method.into(),
                    raw: None
                }
            }],
            "{method}"
        );
    }
}

#[test]
fn an_unserved_request_keeps_the_agents_id_as_it_wrote_it_and_none_of_its_params() {
    let mut session = ready();
    let request = line(json!({
        "jsonrpc": "2.0", "id": "req-stand-in-9", "method": "terminal/create",
        "params": { "command": "TOP-SECRET-STAND-IN" }
    }));
    let step = session.feed(&request, 500);
    assert_eq!(written(&step)[0]["id"], json!("req-stand-in-9"));
    let kept = format!("{:?}", step.events);
    assert!(!kept.contains("TOP-SECRET-STAND-IN"), "{kept}");
}

#[test]
fn an_unserved_request_changes_no_request_the_session_waits_on() {
    let mut session = ready();
    session.feed(
        &line(json!({ "jsonrpc": "2.0", "id": 2, "method": "fs/read_text_file", "params": {} })),
        500,
    );
    // The agent's request 2 is not the session's request 2: the prompt is still out, and its response ends the turn.
    let step = session.feed(&prompt_response(2, "end_turn", None), 510);
    assert!(
        events_of(&step)
            .iter()
            .any(|event| matches!(event, AgentEvent::TurnEnd { .. }))
    );
}

#[test]
fn a_notification_the_engine_does_not_know_is_reported_and_answered_with_nothing() {
    let mut session = ready();
    let step = session.feed(
        &line(json!({ "jsonrpc": "2.0", "method": "x/progress", "params": { "text": "NOISE" } })),
        500,
    );
    assert!(step.writes.is_empty());
    assert_eq!(
        events_of(&step),
        vec![AgentEvent::Other {
            raw_type: "x/progress".into(),
            raw: None
        }]
    );
}

#[test]
fn a_very_long_method_or_update_kind_is_cut_in_the_name_the_event_carries() {
    let long = "x".repeat(10_000);
    let mut session = ready();
    let request = session.feed(
        &line(json!({ "jsonrpc": "2.0", "id": 5, "method": long, "params": {} })),
        500,
    );
    let update = session.feed(
        &line(json!({
            "jsonrpc": "2.0", "method": "session/update",
            "params": { "sessionId": SESSION, "update": { "sessionUpdate": long } }
        })),
        510,
    );
    for step in [request, update] {
        let events = events_of(&step);
        let [AgentEvent::Other { raw_type, raw }] = events.as_slice() else {
            panic!("{step:?}");
        };
        assert!(
            raw_type.len() <= 64 && raw_type.starts_with("xxxx"),
            "{raw_type}"
        );
        assert_eq!(raw, &None);
    }
}

// ---- lines that are not what they should be ------------------------------------------------------------------------

#[test]
fn a_stdout_line_that_is_not_json_is_an_other_with_no_content() {
    for text in [
        &b"loading model weights SECRET-STAND-IN\n"[..],
        &b"\n"[..],
        &b"{\"jsonrpc\": \n"[..],
        &b"[1, 2, 3]\n"[..],
        &b"42\n"[..],
    ] {
        let mut session = ready();
        let step = session.feed(text, 600);
        assert!(step.writes.is_empty());
        assert_eq!(
            step.events,
            vec![StampedEvent {
                wall_ns: Some(600),
                event: AgentEvent::Other {
                    raw_type: "unparsed_line".into(),
                    raw: None
                }
            }],
            "{text:?}"
        );
    }
}

// ---- a handshake that fails ----------------------------------------------------------------------------------------

#[test]
fn an_error_in_answer_to_initialize_writes_nothing_more_and_is_an_other() {
    let mut session = session();
    session.start();
    let step = session.feed(
        &error_response(0, -32000, "auth_required TEXT-STAND-IN"),
        100,
    );
    assert!(step.writes.is_empty());
    assert_eq!(
        step.events,
        vec![StampedEvent {
            wall_ns: Some(100),
            event: AgentEvent::Other {
                raw_type: "initialize_error".into(),
                raw: None
            }
        }]
    );
}

#[test]
fn an_error_in_answer_to_session_new_writes_nothing_more_and_is_an_other() {
    let mut session = session();
    session.start();
    session.feed(&initialize_response(0), 100);
    let step = session.feed(&error_response(1, -32602, "bad cwd TEXT-STAND-IN"), 110);
    assert!(step.writes.is_empty());
    assert_eq!(
        step.events,
        vec![StampedEvent {
            wall_ns: Some(110),
            event: AgentEvent::Other {
                raw_type: "session_new_error".into(),
                raw: None
            }
        }]
    );
}

#[test]
fn a_failed_handshake_makes_finish_say_failed_with_an_api_cause_and_no_status() {
    let failed = PhaseOutcome::Failed {
        cause: PhaseFailure::Api { status: None },
    };

    let mut at_initialize = session();
    at_initialize.start();
    at_initialize.feed(&error_response(0, -32000, "auth_required"), 100);
    assert_eq!(at_initialize.finish(&exited()).outcome, failed);

    let mut at_new = session();
    at_new.start();
    at_new.feed(&initialize_response(0), 100);
    at_new.feed(&error_response(1, -32602, "bad cwd"), 110);
    assert_eq!(at_new.finish(&exited()).outcome, failed);
}

#[test]
fn a_session_new_response_with_no_session_id_is_malformed_and_fails_the_handshake() {
    let mut session = session();
    session.start();
    session.feed(&initialize_response(0), 100);
    let step = session.feed(
        &line(json!({ "jsonrpc": "2.0", "id": 1, "result": {} })),
        110,
    );
    assert!(step.writes.is_empty());
    assert_eq!(
        events_of(&step),
        vec![AgentEvent::Other {
            raw_type: "malformed_response".into(),
            raw: None
        }]
    );
    assert_eq!(
        session.finish(&exited()).outcome,
        PhaseOutcome::Failed {
            cause: PhaseFailure::Api { status: None }
        }
    );
}

#[test]
fn a_child_that_went_away_before_any_prompt_response_is_lost() {
    let mut session = ready();
    assert_eq!(session.finish(&exited()).outcome, PhaseOutcome::Lost);
}
