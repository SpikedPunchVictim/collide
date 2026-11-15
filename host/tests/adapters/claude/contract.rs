//! The `AgentSession` contract of `core` (`collide_core::testing::contract`), run against the Claude session. It is
//! the suite the two session doubles pass, so a session that passes it behaves as every other does to the engine.
//!
//! A Claude session binds in `start` (the id is the one Collide passed on the launch line), so the dialect says so
//! with `bound_at_start`, and the `system/init` line in its handshake must add no second `SessionBound`.

use super::ASSIGNED;
use collide_core::agent::SessionStart;
use collide_core::testing::contract::ChildDialect;
use collide_host::adapters::claude::session::ClaudeSession;
use serde_json::{Value, json};

struct ClaudeDialect;

fn line(value: Value) -> Vec<u8> {
    let mut bytes = serde_json::to_vec(&value).unwrap();
    bytes.push(b'\n');
    bytes
}

impl ChildDialect for ClaudeDialect {
    type Session = ClaudeSession;

    fn session(&self) -> ClaudeSession {
        ClaudeSession::new(SessionStart::Assign {
            uuid: ASSIGNED.to_string(),
        })
    }

    fn bound_at_start(&self) -> Option<String> {
        Some(ASSIGNED.to_string())
    }

    fn handshake(&self) -> Vec<Vec<u8>> {
        vec![line(
            json!({"type": "system", "subtype": "init", "session_id": ASSIGNED}),
        )]
    }

    fn message(&self, text: &str) -> Vec<u8> {
        line(json!({"type": "assistant", "message": {"content": [{"type": "text", "text": text}]}}))
    }

    fn permission_request(&self, id: u32) -> Vec<u8> {
        line(json!({
            "type": "control_request",
            "request_id": id.to_string(),
            "request": {
                "subtype": "can_use_tool", "tool_name": "Bash", "input": {"command": "ls"},
                "tool_use_id": format!("toolu_{id}"),
            },
        }))
    }

    fn turn_end(&self, _turn: u32) -> Vec<u8> {
        line(json!({
            "type": "result", "subtype": "success", "is_error": false, "result": "done",
            "stop_reason": "end_turn", "usage": {"input_tokens": 3, "output_tokens": 5},
        }))
    }
}

collide_core::session_contract_tests!(claude_session, ClaudeDialect);
