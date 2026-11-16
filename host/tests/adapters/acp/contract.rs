//! The `AgentSession` contract of `core` (`collide_core::testing::contract`), run against the ACP session. It is the
//! suite the two session doubles and the Claude session pass, so the second real session keeps the trait honest.
//!
//! An ACP session binds on the `session/new` response, so the dialect has no `bound_at_start`. Its lines are the
//! agent's side as the scripted double of `core` writes it (`collide_core::testing::acp`), which the fake child's
//! recordings also follow.

use collide_core::testing::acp::{
    initialize_response, message_chunk, new_session_response, permission_request, prompt_response,
    standard_options,
};
use collide_core::testing::contract::ChildDialect;
use collide_host::adapters::acp::options::AcpOptions;
use collide_host::adapters::acp::session::AcpSession;
use serde_json::json;

const SESSION: &str = "ses-stand-in-1";

struct AcpDialect;

impl ChildDialect for AcpDialect {
    type Session = AcpSession;

    fn session(&self) -> AcpSession {
        AcpSession::new(AcpOptions::new("/work", "do the task"))
    }

    fn handshake(&self) -> Vec<Vec<u8>> {
        vec![initialize_response(0), new_session_response(1, SESSION)]
    }

    fn message(&self, text: &str) -> Vec<u8> {
        message_chunk(SESSION, text)
    }

    fn permission_request(&self, id: u32) -> Vec<u8> {
        let call = json!({
            "toolCallId": format!("call-{id}"), "title": "bash", "kind": "execute",
            "rawInput": { "command": "ls" }
        });
        permission_request(u64::from(id), SESSION, call, standard_options())
    }

    fn turn_end(&self, turn: u32) -> Vec<u8> {
        // The first prompt is request 2, and each later turn takes the next number.
        prompt_response(2 + u64::from(turn), "end_turn", None)
    }
}

collide_core::session_contract_tests!(acp_session, AcpDialect);
