//! The Claude adapter (0.3, v0 ships one adapter, Claude, for runs; Stage 3b). [`ClaudeAdapter`] is the stateless
//! [`AgentAdapter`] and [`session::ClaudeSession`] its [`collide_core::adapter::AgentSession`]. A Claude session is
//! nearly empty: every turn, the first included, is a `user` message on stdin, and `feed` is the stream parser
//! measured by S1.
//!
//! This module and its children are the only place a Claude environment variable name may appear (the no-names check
//! of ts-0013-nz): the variables the adapter strips ([`env_strip`]) and the three it sets for context options
//! ([`launch`]). Also here: the snapshot files the line names ([`snapshot`]) and the user's own hooks and settings
//! `env` a phase loads ([`user_config`]).
//!
//! | tag | where | what it does |
//! |---|---|---|
//! | `claude: capabilities` | [`ClaudeAdapter::capabilities`] | the capability table, `Yes`, `No` and `Unknown` |
//! | `claude: launch line` | [`launch::launch_spec`] | argv, env and the stripped variables for a role and phase |
//! | `claude: session start` | [`session::ClaudeSession`] `start` | `SessionBound` at once for `Assign` and `Resume` |
//! | `claude: stream parser` | [`session::ClaudeSession`] `feed`, `finish` | S1's stream-json parser, the outcome of a phase |
//! | `claude: turn input` | [`session::ClaudeSession`] `send_turn`, `answer`, `cancel` | user turns, `control_response`, `interrupt` |
//! | `claude: replay` | [`session::ClaudeSession`] `note_written` | what the session wrote, for `replay_records` |
//! | `claude: version check` | [`version::read_version`], [`version::parse_version`] | `claude --version`, cached by path and modification time |
//! | `claude: smoke check` | [`version::smoke_check`] | the one-turn check for a version outside the tested range |
//! | `claude: settings canary` | [`version::canary_started`] | the no-op hook's `hook_started` event in the first turn |

use collide_core::adapter::{
    AgentAdapter, AgentSession, CommandSpec, ConfigPath, ConfigRoot, EnvStrip, LaunchError,
    LaunchRequest, LaunchSpec, SessionRef,
};
use collide_core::agent::{
    Capabilities, ContextOptions, McpServers, ModelSelect, PermissionHost, SessionResume, Support,
    SystemPrompt, UsageQuality, UsageReport, UsageScope,
};

pub mod launch;
pub mod session;
pub mod snapshot;
pub mod user_config;
pub mod version;

/// The program the adapter starts and the take-over command runs.
pub const PROGRAM: &str = "claude";

/// The variables stripped by exact name from everything Collide starts (0.3, What the adapter declares): the one
/// that makes a hook believe it runs inside Claude.
pub const STRIPPED_NAMES: [&str; 1] = ["CLAUDECODE"];

/// The variables stripped by prefix: every `CLAUDE_*` (S1 found the pid, effort, env-file and project-dir variables
/// beside the `CLAUDE_CODE_*` family). No `ANTHROPIC_*` variable is stripped, so a user's gateway or proxy stays.
pub const STRIPPED_PREFIXES: [&str; 1] = ["CLAUDE_"];

/// What the adapter strips, as core's [`EnvStrip`] takes it.
pub fn env_strip() -> EnvStrip {
    EnvStrip {
        names: STRIPPED_NAMES.map(String::from).to_vec(),
        prefixes: STRIPPED_PREFIXES.map(String::from).to_vec(),
    }
}

/// Claude's row of the capability table (0.3, Capabilities). `Yes` is what S1, S7, S9 or S6 measured. `Unknown` is what
/// none of them did, so a role that requires it is refused until the contract test (Stage 3b) says `Yes`:
///
/// - `background`: the tool running detached under a shim is the daemon's first real run (`ts-0070-mk`);
/// - `system_prompt`: S1 showed a definition's prompt takes effect, not whether it replaces or adds to Claude's own;
/// - `mcp_servers.sse`: no spike used an SSE server.
///
/// `limit_signal` is `Yes` on S1's reading of the binary's schemas and two recorded limit messages, with a live
/// `rate_limit_event` seen at `allowed`; a live `rejected` is unmeasured, and the contract test records one when it
/// happens (0.3, Usage and rate limits).
pub fn capabilities() -> Capabilities {
    Capabilities {
        headless_stream: Support::Yes,
        permission_host: PermissionHost {
            support: Support::Yes,
            deny_message: Support::Yes,
            input_at_request: Support::Yes,
            request_reason: Support::Yes,
        },
        session_resume: SessionResume::Silent,
        background: Support::Unknown,
        budget_native: Support::Yes,
        system_prompt: SystemPrompt::Unknown,
        tool_allowlist: Support::Yes,
        named_agent: Support::Yes,
        usage_report: UsageReport {
            support: Support::Yes,
            granularity: vec![
                UsageScope::Turn,
                UsageScope::Step,
                UsageScope::RunningTotal,
                UsageScope::Compaction,
                UsageScope::OtherCalls,
            ],
            cost: Support::Yes,
            cache_write_tiers: Support::Yes,
            context: Support::Yes,
            quality: Some(UsageQuality::Exact),
        },
        limit_signal: Support::Yes,
        compaction_events: Support::Yes,
        hook_events: Support::Yes,
        refusal_source: Support::Yes,
        strict_config: Support::Yes,
        context_options: ContextOptions {
            support: Support::Yes,
            compact_at: Support::Yes,
            bash_output_max: Support::Yes,
            mcp_output_max: Support::Yes,
        },
        model_select: ModelSelect::Launch,
        take_over: Support::Yes,
        mcp_servers: McpServers {
            support: Support::Yes,
            stdio: Support::Yes,
            http: Support::Yes,
            sse: Support::Unknown,
        },
    }
}

/// Claude Code as an [`AgentAdapter`]. It has no state, so one value serves every phase.
#[derive(Debug, Clone, Copy, Default)]
pub struct ClaudeAdapter;

impl AgentAdapter for ClaudeAdapter {
    fn capabilities(&self) -> Capabilities {
        capabilities()
    }

    fn launch(&self, request: &LaunchRequest) -> Result<LaunchSpec, LaunchError> {
        launch::launch_spec(request)
    }

    fn session(&self, request: &LaunchRequest) -> Box<dyn AgentSession> {
        Box::new(session::ClaudeSession::new(request.session.clone()))
    }

    /// `.claude/**`, `CLAUDE.md` and `.mcp.json` below a checkout, and auto memory below the user's home (0.3). A
    /// phase takes settings, agents and MCP config from the line's snapshot instead, so an edit to these cannot reach
    /// a later automatic phase.
    fn config_paths(&self) -> Vec<ConfigPath> {
        [
            (ConfigRoot::Repo, ".claude/**"),
            (ConfigRoot::Repo, "CLAUDE.md"),
            (ConfigRoot::Repo, ".mcp.json"),
            (
                ConfigRoot::Home,
                ".claude/projects/<main checkout key>/memory/**",
            ),
        ]
        .into_iter()
        .map(|(root, pattern)| ConfigPath {
            root,
            pattern: pattern.to_string(),
        })
        .collect()
    }

    /// `claude --resume <session id>` in the directory the session ran in (0.3, Run modes).
    fn interactive_command(&self, session: &SessionRef) -> Option<CommandSpec> {
        Some(CommandSpec {
            argv: vec![PROGRAM.into(), "--resume".into(), session.id.clone()],
            cwd: session.cwd.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn the_adapter_strips_the_claude_code_marker_by_name_and_the_claude_family_by_prefix() {
        let strip = env_strip();
        assert_eq!(strip.names, ["CLAUDECODE"]);
        assert_eq!(strip.prefixes, ["CLAUDE_"]);
    }

    #[test]
    fn the_adapter_strips_no_anthropic_variable() {
        let strip = env_strip();
        assert!(
            strip
                .names
                .iter()
                .chain(&strip.prefixes)
                .all(|name| !name.starts_with("ANTHROPIC"))
        );
    }

    #[test]
    fn the_config_paths_are_the_repo_files_and_the_auto_memory_directory() {
        let paths = ClaudeAdapter.config_paths();
        let repo: Vec<&str> = paths
            .iter()
            .filter(|path| path.root == ConfigRoot::Repo)
            .map(|path| path.pattern.as_str())
            .collect();
        assert_eq!(repo, [".claude/**", "CLAUDE.md", ".mcp.json"]);
        let home: Vec<&str> = paths
            .iter()
            .filter(|path| path.root == ConfigRoot::Home)
            .map(|path| path.pattern.as_str())
            .collect();
        assert_eq!(home, [".claude/projects/<main checkout key>/memory/**"]);
    }

    #[test]
    fn take_over_resumes_the_session_in_the_directory_it_ran_in() {
        let session = SessionRef {
            id: "7c1b".into(),
            cwd: PathBuf::from("/work/tree"),
        };
        let command = ClaudeAdapter.interactive_command(&session).unwrap();
        assert_eq!(command.argv, ["claude", "--resume", "7c1b"]);
        assert_eq!(command.cwd, PathBuf::from("/work/tree"));
    }
}
