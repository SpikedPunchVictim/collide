//! The `initialize` probe (0.3, What the adapter declares; S7, P5): what an agent's `initialize` response implies for
//! the capability table. [`probe`] is a plain function, not a method of `AgentAdapter` (0.3 has none there, and a
//! change to the trait reaches `core` and Claude). The call that sends `initialize` and reads the response is the
//! daemon's ([`crate::acp_probe::probe_through_shim`], `ts-0092-tw`); this module only reads the response.
//!
//! A field the response leaves out is `Unknown`, never `No`, so a role that requires it is refused as 0.3 says. The
//! probe reads only what it uses: `agentCapabilities.loadSession` and `agentCapabilities.mcpCapabilities.http`.

use collide_core::agent::{Capabilities, SessionResume, Support};
use serde_json::Value;

/// What a response implies. [`Probed::default`] is a response that said nothing usable: everything `Unknown`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Probed {
    pub session_resume: SessionResume,
    pub mcp_http: Support,
}

/// Reads an `initialize` response: the JSON-RPC message, with its `result`. A response that is an error, or is not an
/// object, gives [`Probed::default`].
pub fn probe(response: &Value) -> Probed {
    let capabilities = response.pointer("/result/agentCapabilities");
    let flag = |pointer: &str| {
        capabilities
            .and_then(|found| found.pointer(pointer))
            .and_then(Value::as_bool)
    };
    Probed {
        session_resume: match flag("/loadSession") {
            Some(true) => SessionResume::Replay,
            Some(false) => SessionResume::None,
            None => SessionResume::Unknown,
        },
        mcp_http: support_of(flag("/mcpCapabilities/http")),
    }
}

fn support_of(flag: Option<bool>) -> Support {
    match flag {
        Some(true) => Support::Yes,
        Some(false) => Support::No,
        None => Support::Unknown,
    }
}

impl Probed {
    /// The table with what the probe learned put in. Nothing else of it changes.
    pub fn merge_into(&self, mut capabilities: Capabilities) -> Capabilities {
        capabilities.session_resume = self.session_resume;
        capabilities.mcp_servers.http = self.mcp_http;
        capabilities
    }
}
