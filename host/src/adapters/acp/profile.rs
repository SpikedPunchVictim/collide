//! The profile of one ACP agent (0.3, Later adapters: an ACP adapter is one engine plus a profile). It is plain data:
//! what differs between agents, and what `initialize` cannot say. It names no agent. A test builds its own, and the
//! first real profile comes with Stage 6b, from a real run.

use collide_core::adapter::{ConfigPath, EnvStrip};
use collide_core::agent::{
    Capabilities, McpServers, ModelSelect, PermissionHost, Support, UsageReport,
};

/// One ACP agent, as data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcpProfile {
    /// The agent's name, as a refusal names it.
    pub agent: String,
    /// The program, and the arguments that make it speak ACP.
    pub program: String,
    pub args: Vec<String>,
    /// The variables to strip from everything Collide starts for this agent.
    pub env_strip: EnvStrip,
    /// The paths of the agent's own config. None by default.
    pub config_paths: Vec<ConfigPath>,
    /// The interactive command that resumes a session, for a take-over: the program and its leading arguments. The
    /// session's id is added as the last argument. `None` when the agent has no such command.
    pub resume_argv: Option<Vec<String>>,
    /// What the agent's usage reports give. A profile that says nothing gives `Unknown` and no scope, so usage is
    /// reported as not reported (S7: opencode's was `Partial`, with no cost and the last step only).
    pub usage_report: UsageReport,
    /// Whether the agent's `outputTokens` already include its thought tokens. `false` unless the profile says so: a
    /// thought figure is then dropped, because it could be a part of the output or an addition to it (0.3, Usage).
    pub output_includes_thoughts: bool,
    /// Whether the agent reports a usage limit with a reset time. `Unknown` unless the profile says.
    pub limit_signal: Support,
}

impl AcpProfile {
    /// A profile that has only its program: no strip list, no config paths, no resume command, and nothing claimed
    /// about usage or limits.
    pub fn new(agent: impl Into<String>, program: impl Into<String>, args: Vec<String>) -> Self {
        Self {
            agent: agent.into(),
            program: program.into(),
            args,
            env_strip: EnvStrip::default(),
            config_paths: Vec::new(),
            resume_argv: None,
            usage_report: UsageReport::default(),
            output_includes_thoughts: false,
            limit_signal: Support::Unknown,
        }
    }

    /// The capability table before any probe. It holds what the engine always has (S7, P5), what the profile claims,
    /// and `Unknown` for what only a probe or a measurement can say. A flag the engine could not have is `No`.
    ///
    /// `Unknown` here, and in no way `No`: `session_resume` and `mcp_servers.http` (the probe's, see
    /// [`super::probe`]), and what no item has measured for ACP (`system_prompt`, `compaction_events`, `hook_events`,
    /// `refusal_source`, `strict_config`, `context_options`, `permission_host.request_reason`).
    pub fn capabilities(&self) -> Capabilities {
        Capabilities {
            headless_stream: Support::Yes,
            // ACP has no message on a deny and no input at request time (S7, G5, G8).
            permission_host: PermissionHost {
                support: Support::Yes,
                deny_message: Support::No,
                input_at_request: Support::No,
                request_reason: Support::Unknown,
            },
            background: Support::No,
            budget_native: Support::No,
            tool_allowlist: Support::No,
            named_agent: Support::No,
            usage_report: self.usage_report.clone(),
            limit_signal: self.limit_signal,
            model_select: ModelSelect::Session,
            take_over: if self.resume_argv.is_some() {
                Support::Yes
            } else {
                Support::No
            },
            // ACP requires every agent to take a stdio server. The engine has no `sse` form, so it cannot use one.
            mcp_servers: McpServers {
                support: Support::Yes,
                stdio: Support::Yes,
                http: Support::Unknown,
                sse: Support::No,
            },
            ..Capabilities::default()
        }
    }
}
