//! The Claude adapter's declarations, launch line, snapshot files and user-config reading (ts-0061-bg; 0.3), and fake
//! Claude with the Claude session (ts-0062-rj). These tests name `CLAUDECODE`, `CLAUDE_` and `ANTHROPIC_`, and start
//! fake Claude, so they live here (the names check). None reads the owner's own `~/.claude/settings.json`: the settings
//! are the fixture below, copied into a temp root that stands in for `HOME`. Every process a test starts has an
//! environment of its own (`env_clear()`, then `HOME` in a temp root), and is reaped.

use collide_core::adapter::LaunchRequest;
use collide_core::agent::{ModelRef, SessionStart};
use collide_core::pack::Role;
use collide_core::testing::sample_launch_request;
use serde_json::{Value, json};

#[path = "../../common/mod.rs"]
mod common;

mod accept;
mod agent_setup;
mod canary;
mod capabilities;
mod contract;
mod discard;
mod event_stream;
mod fake;
mod fake_claude;
mod file_run;
mod hooks;
mod instruction_warnings;
mod instructions;
mod launch;
mod launchd_run;
mod limits;
mod metrics;
mod pause;
mod published;
mod quick_run;
mod real;
mod real_claude;
mod replay;
mod repo_instructions;
mod repo_instructions_live;
mod reprompt;
mod results;
mod run_command;
mod run_env;
mod run_options;
mod run_process;
mod session;
mod settings_schema;
mod snapshot;
mod tool_results;
mod usage;
mod user_config;
mod version;

pub const ASSIGNED: &str = "7c1b0e52-9f3a-4d2e-8b61-0a5d3c9e4f17";

/// A request for the role `demo/builder`, whose role is `role` (a JSON object with at least `agent`), in `/work`, with
/// a snapshot at `/snap`.
pub fn request(role: Value) -> LaunchRequest {
    let mut request = sample_launch_request();
    request.role = serde_json::from_value::<Role>(role).expect("a role");
    request.session = SessionStart::Assign {
        uuid: ASSIGNED.to_string(),
    };
    request.model = Some(ModelRef {
        name: "sonnet".into(),
        id: None,
    });
    request
}

/// A role with no options but `repo_instructions: none`, which is the launch line before ts-0104-gx: the tests that
/// pin that line use it. `default_role` has no `repo_instructions` key, so it gets the default (`worktree`).
pub fn plain_role() -> Value {
    json!({ "agent": "claude", "repo_instructions": "none" })
}

pub fn default_role() -> Value {
    json!({ "agent": "claude" })
}

/// The argument that follows `flag` in `argv`.
pub fn value_of<'a>(argv: &'a [String], flag: &str) -> Option<&'a str> {
    let at = argv.iter().position(|word| word == flag)?;
    argv.get(at + 1).map(String::as_str)
}
