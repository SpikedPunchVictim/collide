//! The ACP engine (0.3, Two core traits; Stage 3d): the second [`collide_core::adapter::AgentSession`]. ACP is a
//! stateful JSON-RPC conversation, so the session is the client: it holds the handshake, the request ids and the
//! requests the agent has not had answered. The protocol is written by hand; no ACP crate is used.
//!
//! | tag | where | what it does |
//! |---|---|---|
//! | `acp: options` | [`options::AcpOptions`] | the session's own data: working folder, first prompt, MCP servers |
//! | `acp: session` | [`session::AcpSession`] | the handshake, a prompt turn, the turn's end, replay from the journal |
//!
//! Nothing here names an agent. The adapter and the profile of an agent are `ts-0088-pk`.

pub mod options;
pub mod session;
