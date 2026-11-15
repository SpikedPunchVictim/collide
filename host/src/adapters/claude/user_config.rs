//! The user's own config a phase loads (0.3, The user's own tooling; revision 15). Under `config: user` the launch line
//! passes `--setting-sources user`, so the hooks and the `env` of `$HOME/.claude/settings.json` reach the phase. The run
//! preview lists them, flags the ones that can change what the model sees, and the fingerprint (0.13) records the host
//! of the model endpoint. Under `config: none` nothing of it loads, so nothing is read.
//!
//! What is kept is a name and a flag. A hook's command and a settings value are the user's own text and may hold a key,
//! so neither is kept; the endpoint is kept as its host alone, never the path, the query, the port or a key in the user
//! part of the URL. The home directory is given and never read from the environment here, so a test can point it at a
//! temp root.

use super::launch::{BASH_OUTPUT_MAX_VAR, COMPACT_AT_VAR, MCP_OUTPUT_MAX_VAR};
use collide_core::profile::RoleConfig;
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::fmt;
use std::io::ErrorKind;
use std::path::PathBuf;

/// The variable that names the model endpoint. Collide never sets or strips it (0.3).
pub const ENDPOINT_VAR: &str = "ANTHROPIC_BASE_URL";

/// Where the user's settings are, below the home directory.
const SETTINGS_BELOW_HOME: &str = ".claude/settings.json";

/// The context variables a settings `env` may set, and so override a role's option (0.3, Context options).
const CONTEXT_VARS: [&str; 3] = [COMPACT_AT_VAR, BASH_OUTPUT_MAX_VAR, MCP_OUTPUT_MAX_VAR];

/// A hook event that can change what the model sees, so the run preview flags it (0.3). A hook's output is unknown
/// until it runs, so the event is all the preview can go by.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookFlag {
    PreToolUse,
    UserPromptSubmit,
    SessionStart,
    PostToolUse,
}

impl HookFlag {
    /// The flag for a hook event, or `None` for an event that cannot change what the model sees.
    pub fn of_event(event: &str) -> Option<HookFlag> {
        match event {
            "PreToolUse" => Some(HookFlag::PreToolUse),
            "UserPromptSubmit" => Some(HookFlag::UserPromptSubmit),
            "SessionStart" => Some(HookFlag::SessionStart),
            "PostToolUse" => Some(HookFlag::PostToolUse),
            _ => None,
        }
    }

    /// The flag's name in the fingerprint.
    pub fn wire_name(self) -> &'static str {
        match self {
            HookFlag::PreToolUse => "pre_tool_use",
            HookFlag::UserPromptSubmit => "user_prompt_submit",
            HookFlag::SessionStart => "session_start",
            HookFlag::PostToolUse => "post_tool_use",
        }
    }

    /// What a hook of this event can do, for the preview.
    pub fn can_change(&self) -> &'static str {
        match self {
            HookFlag::PreToolUse => "can rewrite a tool call",
            HookFlag::UserPromptSubmit => "can block a prompt or add context",
            HookFlag::SessionStart => "can add context",
            HookFlag::PostToolUse => "can replace a tool's result",
        }
    }
}

/// One hook of the user's settings. `name` is the event, and `event:matcher` when the hook has a matcher, the way the
/// stream names a hook call. It never holds the command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserHook {
    pub name: String,
    pub event: String,
    pub flag: Option<HookFlag>,
}

/// Why a settings `env` entry is flagged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnvFlagKind {
    /// It sets one of the three context variables, and so overrides the role's option.
    ContextVariable,
    /// It sets the model endpoint.
    Endpoint,
}

/// A settings `env` entry that the preview flags. Only the name: the value is the user's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvFlag {
    pub name: String,
    pub kind: EnvFlagKind,
}

/// Where the endpoint variable was set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndpointSource {
    /// The captured environment of the daemon.
    Environment,
    /// The `env` of a settings file, which `claude` applies itself after the strip.
    SettingsEnv,
}

/// The model endpoint a phase will reach, as the fingerprint records it. `host` is `None` when the value is not a URL
/// with a plain host: the variable is set, and nothing of it is kept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoint {
    pub source: EndpointSource,
    pub host: Option<String>,
}

/// What the user's settings add to a phase.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct UserConfig {
    /// Sorted by name, so two reads of one file agree.
    pub hooks: Vec<UserHook>,
    /// The `env` entries to flag, sorted by name. The endpoint is here too, and its host is in `endpoint`.
    pub env_flags: Vec<EnvFlag>,
    pub endpoint: Option<Endpoint>,
}

/// Why the user's settings could not be read.
#[derive(Debug)]
pub enum UserConfigError {
    /// `HOME` is not set (or is empty), so the file cannot be found.
    NoHome,
    /// The file exists and cannot be read.
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    /// The file is not JSON. `message` carries the place.
    Json { path: PathBuf, message: String },
    /// The file is JSON of a shape the adapter cannot read.
    Shape { path: PathBuf, message: String },
}

impl fmt::Display for UserConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UserConfigError::NoHome => {
                f.write_str("HOME is not set, so the user's Claude settings cannot be found")
            }
            UserConfigError::Read { path, source } => {
                write!(f, "{}: cannot be read: {source}", path.display())
            }
            UserConfigError::Json { path, message } => {
                write!(f, "{}: is not valid JSON: {message}", path.display())
            }
            UserConfigError::Shape { path, message } => {
                write!(f, "{}: {message}", path.display())
            }
        }
    }
}

impl std::error::Error for UserConfigError {}

/// Reads the hooks and the `env` the user's settings give a phase. `config` is the role's: `None` is the launch line's
/// default, which is `user`; under `config: none` nothing loads, so the file is not read. A missing file is no config.
/// A file that cannot be read, is not JSON or is of the wrong shape is an error with its path.
pub fn read_user_config(
    home: Option<&str>,
    config: Option<RoleConfig>,
) -> Result<UserConfig, UserConfigError> {
    if config == Some(RoleConfig::None) {
        return Ok(UserConfig::default());
    }
    let home = match home {
        Some(home) if !home.is_empty() => home,
        _ => return Err(UserConfigError::NoHome),
    };
    let path = PathBuf::from(home).join(SETTINGS_BELOW_HOME);
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(UserConfig::default()),
        Err(source) => return Err(UserConfigError::Read { path, source }),
    };
    let settings: Value =
        serde_json::from_slice(&bytes).map_err(|error| UserConfigError::Json {
            path: path.clone(),
            message: error.to_string(),
        })?;
    let shape = |message: String| UserConfigError::Shape {
        path: path.clone(),
        message,
    };
    let Value::Object(settings) = settings else {
        return Err(shape("the settings are not a JSON object".into()));
    };
    let mut user = UserConfig {
        hooks: read_hooks(&settings).map_err(shape)?,
        ..UserConfig::default()
    };
    read_env(&settings, &mut user).map_err(shape)?;
    Ok(user)
}

/// The object under `key`, `None` when the key is absent.
fn object_under<'a>(
    settings: &'a Map<String, Value>,
    key: &str,
) -> Result<Option<&'a Map<String, Value>>, String> {
    match settings.get(key) {
        None => Ok(None),
        Some(Value::Object(object)) => Ok(Some(object)),
        Some(_) => Err(format!("`{key}` is not a JSON object")),
    }
}

fn read_hooks(settings: &Map<String, Value>) -> Result<Vec<UserHook>, String> {
    let mut hooks = Vec::new();
    let Some(events) = object_under(settings, "hooks")? else {
        return Ok(hooks);
    };
    for (event, groups) in events {
        let Value::Array(groups) = groups else {
            return Err(format!("`hooks.{event}` is not a list"));
        };
        for group in groups {
            let matcher = group.get("matcher").and_then(Value::as_str);
            let commands = group.get("hooks").and_then(Value::as_array);
            let name = match matcher {
                Some(matcher) if !matcher.is_empty() => format!("{event}:{matcher}"),
                _ => event.clone(),
            };
            for _ in commands.into_iter().flatten() {
                hooks.push(UserHook {
                    name: name.clone(),
                    event: event.clone(),
                    flag: HookFlag::of_event(event),
                });
            }
        }
    }
    hooks.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(hooks)
}

fn read_env(settings: &Map<String, Value>, user: &mut UserConfig) -> Result<(), String> {
    let Some(env) = object_under(settings, "env")? else {
        return Ok(());
    };
    for (name, value) in env {
        if name == ENDPOINT_VAR {
            user.env_flags.push(EnvFlag {
                name: name.clone(),
                kind: EnvFlagKind::Endpoint,
            });
            user.endpoint = Some(Endpoint {
                source: EndpointSource::SettingsEnv,
                host: value.as_str().and_then(endpoint_host),
            });
        } else if CONTEXT_VARS.contains(&name.as_str()) {
            user.env_flags.push(EnvFlag {
                name: name.clone(),
                kind: EnvFlagKind::ContextVariable,
            });
        }
    }
    user.env_flags.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(())
}

/// The fingerprint's user-config part (0.13): the hooks by name, event and flag, the flagged `env` names, and the
/// endpoint's host. It is the same names and flags the run preview shows; a command, a value, a key and a path are not
/// in it, because [`UserConfig`] never held them.
pub fn user_config_fingerprint(config: &UserConfig) -> Value {
    let hooks: Vec<Value> = config
        .hooks
        .iter()
        .map(|hook| {
            serde_json::json!({
                "name": hook.name,
                "event": hook.event,
                "flag": hook.flag.map(HookFlag::wire_name),
            })
        })
        .collect();
    let env_flags: Vec<Value> = config
        .env_flags
        .iter()
        .map(|flag| {
            let kind = match flag.kind {
                EnvFlagKind::ContextVariable => "context_variable",
                EnvFlagKind::Endpoint => "endpoint",
            };
            serde_json::json!({"name": flag.name, "kind": kind})
        })
        .collect();
    let endpoint = config.endpoint.as_ref().map(|endpoint| {
        let source = match endpoint.source {
            EndpointSource::Environment => "environment",
            EndpointSource::SettingsEnv => "settings_env",
        };
        serde_json::json!({"source": source, "host": endpoint.host})
    });
    serde_json::json!({"hooks": hooks, "env_flags": env_flags, "endpoint": endpoint})
}

/// The endpoint variable of the captured environment, recorded the way a settings `env` one is.
pub fn endpoint_in_environment(env: &BTreeMap<String, String>) -> Option<Endpoint> {
    env.get(ENDPOINT_VAR).map(|value| Endpoint {
        source: EndpointSource::Environment,
        host: endpoint_host(value),
    })
}

/// The host of an endpoint URL, lowercased, and nothing else: no scheme, port, key in the user part, path, query or
/// fragment. A value that is not `scheme://host...` with a plain host (letters, digits, `.`, `-`, `_`, or a bracketed
/// IPv6 address) gives `None`, so a guess never carries a key into the record.
pub fn endpoint_host(url: &str) -> Option<String> {
    let after_scheme = url.trim().split_once("://")?.1;
    let authority = after_scheme.split(['/', '?', '#']).next()?;
    let host_and_port = authority.rsplit('@').next()?;
    let host = if host_and_port.starts_with('[') {
        let end = host_and_port.find(']')?;
        let bracketed = &host_and_port[..=end];
        let plain_v6 = bracketed[1..end]
            .bytes()
            .all(|b| b.is_ascii_hexdigit() || b == b':' || b == b'.');
        plain_v6.then_some(bracketed)?
    } else {
        let host = host_and_port.split(':').next()?;
        let plain = host
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'));
        plain.then_some(host)?
    };
    (!host.is_empty() && host != "[]").then(|| host.to_ascii_lowercase())
}
