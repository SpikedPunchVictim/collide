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
//!   --disallowed-tools <scheduling, worktree and messaging tools, but the ones the role allows>
//!   [--add-dir <the folder the phase starts in> [--plugin-dir <snapshot>/plugins/repo-skills]]   # worktree
//!   [--permission-mode <mode>]
//! ```
//!
//! The `--add-dir` part is the role's `repo_instructions` (`worktree`, the default; `none` leaves it off). It comes
//! with one variable ([`super::instructions::ADD_DIR_VAR`]), and the plugin only when the snapshot holds it and
//! [`PLUGIN_ON_THE_LINE`] is on.
//!
//! Prompt text never appears in argv or in `ps`: the definitions go in a file, the first turn on stdin.
//!
//! | tag | function | what it does |
//! |---|---|---|
//! | `claude: launch line` | [`launch_spec`] | argv, env, working directory and the variables to strip |

use super::env_strip;
use super::instructions::{
    ADD_DIR_VAR, PLUGIN_NAME, PLUGIN_ON_THE_LINE, PLUGINS_DIR, skills_plugin_dir,
};
use collide_core::adapter::{LaunchError, LaunchRequest, LaunchSpec};
use collide_core::agent::SessionStart;
use collide_core::pack::Role;
use collide_core::profile::{RepoInstructions, RoleConfig};
use collide_core::resolve::is_safe_token;
use std::collections::BTreeMap;

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
/// the variables to strip ([`env_strip`]) and the working directory. A `compact_at` under [`COMPACT_AT_FLOOR`], an
/// output cap of zero, a budget that is spent, a session the tool would name itself and a value that is not a safe
/// token (0.5) where one becomes an argument are each a [`LaunchError::Invalid`] that says why.
///
/// The role's required capabilities are not checked here: the caller refuses them with
/// [`collide_core::adapter::refuse_unmet_capabilities`] over [`super::ClaudeAdapter`]'s table, before it builds the
/// request. The model is the one the alias already resolved to (`collide_core::resolve::resolve_model_alias`), and only
/// its `name` is used.
///
/// The first turn is not in the spec. It goes on stdin as a `user` message, written by the session.
pub fn launch_spec(request: &LaunchRequest) -> Result<LaunchSpec, LaunchError> {
    let role = &request.role;
    let mut env = context_env(role)?;

    let mut argv: Vec<String> = FIXED_ARGV.map(String::from).to_vec();
    argv.extend(session_args(&request.session)?);
    if let Some(model) = &request.model {
        argv.extend(["--model".into(), safe("the model name", &model.name)?]);
    }
    if let Some(left) = request.budget_left_usd {
        if !left.is_finite() || left <= 0.0 {
            return invalid(format!(
                "the phase has no budget left to pass as --max-budget-usd ({left}); \
                 a spent phase is paused, not launched"
            ));
        }
        argv.extend(["--max-budget-usd".into(), left.to_string()]);
    }
    let snapshot = |file: &str| -> Result<String, LaunchError> {
        match request.snapshot_dir.join(file).to_str() {
            Some(path) => Ok(path.to_string()),
            None => invalid(format!(
                "the snapshot directory {} is not valid UTF-8",
                request.snapshot_dir.display()
            )),
        }
    };
    argv.extend([
        "--agents".into(),
        snapshot(AGENTS_FILE)?,
        "--agent".into(),
        agent_name(&request.role_name)?,
        "--setting-sources".into(),
        setting_sources(role.config).into(),
        "--settings".into(),
        snapshot(SETTINGS_FILE)?,
        "--mcp-config".into(),
        snapshot(MCP_FILE)?,
    ]);
    check_allowed_tools(&request.role_name, &role.allow_tools)?;
    let disallowed = disallowed_tools_without(&role.allow_tools);
    if !disallowed.is_empty() {
        argv.push("--disallowed-tools".into());
        argv.extend(disallowed.into_iter().map(String::from));
    }
    if role.repo_instructions.unwrap_or(RepoInstructions::Worktree) == RepoInstructions::Worktree {
        let Some(start) = request.cwd.to_str() else {
            return invalid(format!(
                "the folder the phase starts in, {}, is not valid UTF-8, so the repository's instructions cannot \
                 be added to the line (set `repo_instructions: none` to leave them off)",
                request.cwd.display()
            ));
        };
        argv.extend(["--add-dir".into(), start.to_string()]);
        let plugin = skills_plugin_dir(&request.snapshot_dir);
        if PLUGIN_ON_THE_LINE && plugin.is_dir() {
            argv.extend([
                "--plugin-dir".into(),
                snapshot(&format!("{PLUGINS_DIR}/{PLUGIN_NAME}"))?,
            ]);
        }
        env.insert(ADD_DIR_VAR.to_string(), "1".to_string());
    }

    Ok(LaunchSpec {
        argv,
        env,
        env_strip: env_strip(),
        cwd: request.cwd.clone(),
    })
}

fn invalid<T>(message: String) -> Result<T, LaunchError> {
    Err(LaunchError::Invalid { message })
}

/// `value` when it is a safe token (0.5), so it cannot become a flag or climb out of a directory.
fn safe(what: &str, value: &str) -> Result<String, LaunchError> {
    if is_safe_token(value) {
        Ok(value.to_string())
    } else {
        invalid(format!(
            "{what} `{value}` is not a safe token (1 to 64 of A-Z a-z 0-9 . _ -, not starting with - or .)"
        ))
    }
}

/// `--session-id <uuid>` for `Assign` and `--resume <id>` for `Resume`. A tool that names its own session is ACP's
/// shape, so `AgentAssigned` is not a Claude start.
fn session_args(start: &SessionStart) -> Result<[String; 2], LaunchError> {
    match start {
        SessionStart::Assign { uuid } => Ok(["--session-id".into(), safe("the session id", uuid)?]),
        SessionStart::Resume { id } => Ok(["--resume".into(), safe("the session id", id)?]),
        SessionStart::AgentAssigned {} => invalid(
            "Claude cannot name its own session: a phase starts with Assign (--session-id) or Resume (--resume)"
                .into(),
        ),
    }
}

/// `collide-<pack>-<role>` from a role as a workflow names it, `pack/role`. Both parts are safe tokens, because the
/// name reaches argv and a definition's key.
pub fn agent_name(role_name: &str) -> Result<String, LaunchError> {
    match role_name.split_once('/') {
        Some((pack, role)) if !role.contains('/') => Ok(format!(
            "collide-{}-{}",
            safe("the pack name", pack)?,
            safe("the role name", role)?
        )),
        _ => invalid(format!(
            "the role `{role_name}` is not written as `pack/role`"
        )),
    }
}

/// `user` or `""` (0.3, Context options). Both lines keep `--strict-mcp-config`, which is fixed.
fn setting_sources(config: Option<RoleConfig>) -> &'static str {
    match config {
        Some(RoleConfig::None) => "",
        Some(RoleConfig::User) | None => "user",
    }
}

/// [`DISALLOWED_TOOLS`] but the ones a role asks for with `allow_tools`.
pub fn disallowed_tools_without(asked_for: &[String]) -> Vec<&'static str> {
    DISALLOWED_TOOLS
        .into_iter()
        .filter(|tool| !asked_for.iter().any(|asked| asked == tool))
        .collect()
}

/// A name in `allow_tools` that is not one of [`DISALLOWED_TOOLS`] is refused, naming the tool and the role: allowing
/// a tool that was never disallowed does nothing, and a typo must not look as if it worked.
fn check_allowed_tools(role_name: &str, allow_tools: &[String]) -> Result<(), LaunchError> {
    match allow_tools
        .iter()
        .find(|tool| !DISALLOWED_TOOLS.contains(&tool.as_str()))
    {
        None => Ok(()),
        Some(tool) => invalid(format!(
            "the role `{role_name}` allows the tool `{tool}`, which is not one of the tools the line disallows ({}), \
             so there is nothing to allow",
            DISALLOWED_TOOLS.join(", ")
        )),
    }
}

/// The variables the role's context options set, after the strip, so they beat an inherited value. An unusable value
/// is refused here because `claude` ignores one without a warning.
fn context_env(role: &Role) -> Result<BTreeMap<String, String>, LaunchError> {
    let mut env = BTreeMap::new();
    if let Some(window) = role.compact_at {
        if window < COMPACT_AT_FLOOR {
            return invalid(format!(
                "`compact_at` {} is under the floor of {} tokens: claude ignores a smaller window without a warning",
                with_commas(window),
                with_commas(COMPACT_AT_FLOOR)
            ));
        }
        env.insert(COMPACT_AT_VAR.to_string(), window.to_string());
    }
    for (option, value, variable) in [
        ("bash_output_max", role.bash_output_max, BASH_OUTPUT_MAX_VAR),
        ("mcp_output_max", role.mcp_output_max, MCP_OUTPUT_MAX_VAR),
    ] {
        match value {
            Some(0) => {
                return invalid(format!(
                    "`{option}` is 0: claude would ignore it without a warning, so give it at least 1"
                ));
            }
            Some(value) => {
                env.insert(variable.to_string(), value.to_string());
            }
            None => {}
        }
    }
    Ok(env)
}

/// 100000 as `100,000`.
fn with_commas(number: u64) -> String {
    let digits = number.to_string();
    let mut text = String::new();
    for (at, digit) in digits.chars().enumerate() {
        if at > 0 && (digits.len() - at).is_multiple_of(3) {
            text.push(',');
        }
        text.push(digit);
    }
    text
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
    fn numbers_in_messages_have_thousands_separators() {
        let shown: Vec<String> = [0, 999, 1_000, 99_999, 100_000, 1_234_567]
            .into_iter()
            .map(with_commas)
            .collect();
        assert_eq!(
            shown,
            ["0", "999", "1,000", "99,999", "100,000", "1,234,567"]
        );
    }

    #[test]
    fn the_compact_window_floor_is_one_hundred_thousand_tokens() {
        assert_eq!(COMPACT_AT_FLOOR, 100_000);
    }
}
