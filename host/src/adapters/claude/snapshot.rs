//! The phase's snapshot files (0.3, The tool's config paths in a repo): `settings.json`, `agents.json` and `mcp.json`.
//! The launch line names them (`--settings`, `--agents`, `--mcp-config`) and the adapter only renders their text from
//! what it is given. It opens no file: the daemon reads the role's definition and the line's MCP servers, and writes
//! what this returns into the phase's snapshot directory. The same input gives the same text.
//!
//! | file | what it holds |
//! |---|---|
//! | `settings.json` | `autoMemoryEnabled: false` (shared by every worktree of a repo, so one run's notes would reach another) and the canary: a `SessionStart` hook that writes a marker to stderr, whose absence from the first turn means `claude` ignored the file |
//! | `agents.json` | the role's definition, under `collide-<pack>-<role>` |
//! | `mcp.json` | the MCP servers the daemon passes, none by default |

use super::launch::{AGENTS_FILE, MCP_FILE, SETTINGS_FILE, agent_name};
use super::version::CANARY_MARKER;
use collide_core::adapter::{LaunchError, LaunchRequest};
use collide_core::yaml::load_yaml;
use serde_json::{Map, Value, json};

/// The text of the three files, each ending in a newline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotFiles {
    pub agents: String,
    pub settings: String,
    pub mcp: String,
}

impl SnapshotFiles {
    /// Each file's name (the launch line's) and its text, in the order of the line.
    pub fn named(&self) -> [(&'static str, &str); 3] {
        [
            (AGENTS_FILE, &self.agents),
            (SETTINGS_FILE, &self.settings),
            (MCP_FILE, &self.mcp),
        ]
    }
}

/// Renders the snapshot files for `request`'s role. `definition` is the text of the role's definition file (a Markdown
/// file with a YAML front matter, the form of a Claude agent), and `mcp_servers` the `mcpServers` of the line's MCP
/// config. A definition with no front matter, an unreadable one or one with no `description` is refused.
pub fn snapshot_files(
    request: &LaunchRequest,
    definition: &str,
    mcp_servers: &Map<String, Value>,
) -> Result<SnapshotFiles, LaunchError> {
    let name = agent_name(&request.role_name)?;
    let agents = Value::Object(Map::from_iter([(name, agent_of(definition)?)]));
    Ok(SnapshotFiles {
        agents: render(&agents),
        settings: render(&settings()),
        mcp: render(&json!({ "mcpServers": mcp_servers })),
    })
}

fn render(value: &Value) -> String {
    let mut text = serde_json::to_string_pretty(value).expect("a JSON value always renders");
    text.push('\n');
    text
}

/// Auto memory off, and the canary: a `SessionStart` hook that writes its marker to stderr and nothing to stdout (a
/// `SessionStart` hook's stdout becomes context; its stderr does not). The session reads the marker in the hook's
/// `hook_response` ([`super::version::canary_started`]).
fn settings() -> Value {
    json!({
        "autoMemoryEnabled": false,
        "hooks": {
            "SessionStart": [
                { "hooks": [{ "type": "command", "command": format!("echo {CANARY_MARKER} >&2") }] }
            ]
        }
    })
}

/// The agent object `--agents` takes: `description`, `prompt` and, when the definition lists them, `tools`. The
/// definition's own `model` is left out, because the launch line's `--model` decides it (measured S1), and its `name`
/// is replaced by the key.
fn agent_of(definition: &str) -> Result<Value, LaunchError> {
    let invalid = |message: String| LaunchError::Invalid {
        message: format!("the role's definition {message}"),
    };
    let text = definition.replace("\r\n", "\n");
    let (front, body) = text
        .strip_prefix("---\n")
        .and_then(|rest| {
            rest.split_once("\n---\n")
                .or_else(|| rest.strip_suffix("\n---").map(|f| (f, "")))
        })
        .ok_or_else(|| invalid("has no `---` front matter".into()))?;
    let front: Value = load_yaml(front.as_bytes(), "the definition's front matter")
        .map_err(|error| invalid(format!("is unreadable: {error}")))?;
    let description = front
        .get("description")
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
        .ok_or_else(|| invalid("has no `description`".into()))?;

    let mut agent = Map::new();
    agent.insert("description".into(), description.into());
    agent.insert("prompt".into(), body.trim().into());
    if let Some(tools) = tools_of(front.get("tools")).map_err(invalid)? {
        agent.insert("tools".into(), tools.into());
    }
    Ok(Value::Object(agent))
}

/// `tools:` written as `A, B` or as a YAML list. `None` when the key is absent.
fn tools_of(tools: Option<&Value>) -> Result<Option<Vec<String>>, String> {
    match tools {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(list)) => Ok(Some(
            list.split(',')
                .map(str::trim)
                .filter(|tool| !tool.is_empty())
                .map(String::from)
                .collect(),
        )),
        Some(Value::Array(items)) => items
            .iter()
            .map(|item| item.as_str().map(String::from))
            .collect::<Option<Vec<_>>>()
            .map(Some)
            .ok_or_else(|| "has a `tools` list with an entry that is not text".into()),
        Some(_) => Err("has `tools` that is neither a list nor text".into()),
    }
}
