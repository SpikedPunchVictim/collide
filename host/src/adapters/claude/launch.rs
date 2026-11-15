//! The launch line (0.3, v0 ships one adapter, Claude, for runs; measured by S1 on 2.1.323). What is fixed about it is
//! declared here as constants; [`launch_spec`] builds the rest from a role and a phase.
//!
//! ```text
//! claude -p --input-format stream-json --output-format stream-json --verbose --include-hook-events
//!   --session-id <uuid>                       # `Assign`; a resumed phase passes --resume <uuid> instead
//!   --permission-prompts host --permission-prompt-tool stdio
//!   --model <model.name> --max-budget-usd <budget left for this phase>
//!   --agents <snapshot>/agents.json --agent collide-<pack>-<role>
//!   --setting-sources user --settings <snapshot>/settings.json
//!   --mcp-config <snapshot>/mcp.json --strict-mcp-config
//!   --disallowed-tools <scheduling, worktree and messaging tools>
//!   [--plugin-dir <snapshot>/plugins/<name> ...] [--permission-mode <mode>]
//! ```
//!
//! Prompt text never appears in argv or in `ps`: the definitions go in a file, the first turn on stdin.
//!
//! | tag | function | what it does |
//! |---|---|---|
//! | `claude: launch line` | [`launch_spec`] | argv, env, working directory and the variables to strip |

use super::env_strip;
use collide_core::adapter::{LaunchError, LaunchRequest, LaunchSpec};

/// The words of the line that never change, in order: `-p`, the stream formats, `--verbose` (without it `-p` with
/// stream-json output exits 1), `--include-hook-events` (without it a failed hook leaves no event), the host as the
/// permission prompter (`--permission-prompt-tool stdio` is the Agent SDK's own protocol: without it every prompt is
/// denied) and `--strict-mcp-config`.
pub const FIXED_ARGV: [&str; 12] = [
    "-p",
    "--input-format",
    "stream-json",
    "--output-format",
    "stream-json",
    "--verbose",
    "--include-hook-events",
    "--permission-prompts",
    "host",
    "--permission-prompt-tool",
    "stdio",
    "--strict-mcp-config",
];

/// The tools disallowed unless a role asks for one (owner, 2026-11-13; measured S1): they start work the owner cannot
/// see, or move the session out of its workspace. They are the value of `--disallowed-tools`.
pub const DISALLOWED_TOOLS: [&str; 8] = [
    "CronCreate",
    "ScheduleWakeup",
    "RemoteTrigger",
    "PushNotification",
    "EnterWorktree",
    "ExitWorktree",
    "Workflow",
    "SendMessage",
];

/// The files of the line's snapshot that the line names.
pub const AGENTS_FILE: &str = "agents.json";
pub const SETTINGS_FILE: &str = "settings.json";
pub const MCP_FILE: &str = "mcp.json";

/// The variable a role's `compact_at` sets (until the contract test shows `--autocompact` agrees with it). The smallest
/// window is [`COMPACT_AT_FLOOR`]; an unusable value is ignored with no warning, so a value under it is refused.
pub const COMPACT_AT_VAR: &str = "CLAUDE_CODE_AUTO_COMPACT_WINDOW";

/// The variable a role's `bash_output_max` sets, in characters (default 30,000).
pub const BASH_OUTPUT_MAX_VAR: &str = "BASH_MAX_OUTPUT_LENGTH";

/// The variable a role's `mcp_output_max` sets, in tokens (default 25,000).
pub const MCP_OUTPUT_MAX_VAR: &str = "MAX_MCP_OUTPUT_TOKENS";

/// The smallest `compact_at` (0.3, Context options). A role below it is refused with a message that names this floor.
pub const COMPACT_AT_FLOOR: u64 = 100_000;

/// The launch spec for a role and a phase: the argv above, the environment the context options set after the strip,
/// the variables to strip ([`env_strip`]) and the working directory. A capability the role requires and the adapter
/// cannot promise, a mapped model that fails the pack's pattern and a `compact_at` under [`COMPACT_AT_FLOOR`] are a
/// [`LaunchError`].
pub fn launch_spec(request: &LaunchRequest) -> Result<LaunchSpec, LaunchError> {
    let _ = (request, env_strip);
    todo!("claude: launch line")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fixed_line_asks_for_verbose_hook_events_and_host_permissions() {
        for word in ["--verbose", "--include-hook-events", "--strict-mcp-config"] {
            assert!(FIXED_ARGV.contains(&word), "{word}");
        }
        let at = FIXED_ARGV
            .iter()
            .position(|word| *word == "--permission-prompt-tool")
            .unwrap();
        assert_eq!(FIXED_ARGV[at + 1], "stdio");
    }

    #[test]
    fn the_scheduling_worktree_and_messaging_tools_are_disallowed() {
        for tool in ["CronCreate", "EnterWorktree", "SendMessage", "Workflow"] {
            assert!(DISALLOWED_TOOLS.contains(&tool), "{tool}");
        }
    }

    #[test]
    fn the_compact_window_floor_is_one_hundred_thousand_tokens() {
        assert_eq!(COMPACT_AT_FLOOR, 100_000);
    }
}
