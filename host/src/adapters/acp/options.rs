//! The data an ACP session is built from. A session holds all of it itself, so a session rebuilt from the journal has
//! the first prompt it must have written (it is not queued by a `send_turn` that a restart would lose).

use serde_json::{Value, json};
use std::path::PathBuf;

/// One MCP server the agent is asked to connect to in `session/new`. ACP defines three kinds; `sse` is left out until
/// something uses it. The shapes are the schema's, so a caller cannot send one the agent rejects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum McpServer {
    /// A server reached over HTTP. `headers` are sent with every request.
    Http {
        name: String,
        url: String,
        headers: Vec<(String, String)>,
    },
    /// A server the agent starts itself. `env` is added to the agent's own environment for it.
    Stdio {
        name: String,
        command: String,
        args: Vec<String>,
        env: Vec<(String, String)>,
    },
}

impl McpServer {
    /// The server as `session/new` carries it. A stdio server has no `type`, as the schema has it.
    pub fn to_json(&self) -> Value {
        match self {
            McpServer::Http { name, url, headers } => json!({
                "type": "http", "name": name, "url": url, "headers": pairs(headers)
            }),
            McpServer::Stdio {
                name,
                command,
                args,
                env,
            } => json!({
                "name": name, "command": command, "args": args, "env": pairs(env)
            }),
        }
    }
}

/// ACP writes a header and an environment variable as `{ "name": ..., "value": ... }`.
fn pairs(pairs: &[(String, String)]) -> Value {
    pairs
        .iter()
        .map(|(name, value)| json!({ "name": name, "value": value }))
        .collect()
}

/// What a session needs to know: where the agent works, what its first turn says, and the servers it may use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcpOptions {
    /// The working folder, sent as `cwd` in `session/new`. ACP wants an absolute path as text; a path that is not
    /// valid UTF-8 is sent with its bytes replaced, as JSON has no other way to carry it.
    pub cwd: PathBuf,
    /// The first turn, written when `session/new` has answered.
    pub first_prompt: String,
    /// The servers for `session/new`; none by default.
    pub mcp_servers: Vec<McpServer>,
    /// The model the request names, as the agent knows it (`ModelRef::adapter_id`). It is data for a JSON-RPC body
    /// only: it is in no argument, variable or file name, so it needs no safe token. `None` keeps the agent's own.
    pub model: Option<String>,
}

impl AcpOptions {
    /// Options with no MCP servers and no model.
    pub fn new(cwd: impl Into<PathBuf>, first_prompt: impl Into<String>) -> Self {
        Self {
            cwd: cwd.into(),
            first_prompt: first_prompt.into(),
            mcp_servers: Vec::new(),
            model: None,
        }
    }

    /// The same options with these servers.
    pub fn with_mcp_servers(mut self, servers: Vec<McpServer>) -> Self {
        self.mcp_servers = servers;
        self
    }

    /// The same options that ask the agent for this model after `session/new`.
    pub fn with_model(mut self, id: impl Into<String>) -> Self {
        self.model = Some(id.into());
        self
    }
}
