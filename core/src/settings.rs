//! Collide's settings files and the agent entries (0.7, Settings and Agent entries; 0.9; Stage 2, Contracts).
//!
//! [`ConfigFile`] is `.collide/config.yaml` (committed, in the trust hash) and, the same shape, the user's
//! `~/.config/collide/config.yaml`. It holds Collide's own settings, `agents` and `ask`, and the pack settings
//! (`config:`, pack name, then key, then value, as a profile writes them).
//!
//! The checks are pure functions over a parsed file. Reading the layers (the schema defaults, the user's file, the
//! repo's file, the run's profile), merging them, checking a pack's values against its schema, and the keychain
//! are Stage 4a's. What lives here is each rule that needs only one file:
//! - [`check_config_file`]: the range of `timeout_s`, which agent kinds need a `command`, a budget on a kind that
//!   cannot enforce one, a literal secret in `env`, and an ACP agent's refused flags;
//! - [`repo_file_refusals`]: which keys a repo `config.yaml` may not set (0.7, Layers and sources), as data
//!   ([`REPO_FILE_AGENT_ASK_KEYS`]) and as a function over a parsed file;
//! - [`secrets_in_file`]: a `secret: true` pack key written in either file.
//!
//! A tool kind other than `read` and `search` does not parse: [`ToolKind`] has two variants, so the load error
//! names the key.

use crate::profile::RoleConfig;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::net::IpAddr;

/// The shortest timeout an agent may have, in seconds (0.7: "every agent has one: 5 to 1800").
pub const MIN_TIMEOUT_S: u64 = 5;

/// The longest timeout an agent may have, in seconds.
pub const MAX_TIMEOUT_S: u64 = 1800;

/// The id of the one built-in agent entry: Claude Code with haiku (0.7, Agent entries).
pub const BUILTIN_AGENT_ID: &str = "claude-haiku";

/// The only keys of `agents` and `ask` a repo `config.yaml` may set (0.7, Layers and sources): the two defaults
/// for every ask. A team has a say in cost and not in what runs on a developer's machine.
pub const REPO_FILE_AGENT_ASK_KEYS: [&str; 2] = ["ask.budget_usd", "ask.timeout_s"];

/// The keys of Collide's own settings that are `overridable` (0.7: a slow local model is a fact about one
/// machine): the user's value wins over the repo's. The others a repo file sets win over the user's.
pub const COLLIDE_OVERRIDABLE_KEYS: [&str; 1] = ["ask.timeout_s"];

/// A name that, at the end of an `env` variable's name, marks it as a secret (0.7, Secrets are names).
pub const SECRET_NAME_ENDINGS: [&str; 4] = ["KEY", "TOKEN", "SECRET", "PASSWORD"];

/// What an agent runs as. The variants carry plain comments, not doc comments, so the schema stays a bare enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum AgentKind {
    // Claude Code, launched by Collide. The only kind that enforces a budget.
    ClaudeCode,
    // An ACP agent such as opencode, launched by Collide from `command` and `args`.
    Acp,
    // An agent the user runs. Collide launches nothing.
    External,
}

impl AgentKind {
    pub fn as_str(self) -> &'static str {
        match self {
            AgentKind::ClaudeCode => "claude-code",
            AgentKind::Acp => "acp",
            AgentKind::External => "external",
        }
    }
}

/// A tool kind an agent may use. v0 accepts only these two: any other kind is a load error that names the key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ToolKind {
    Read,
    Search,
}

/// One entry of `agents:`. Every key is optional, because a layer may change the built-in entry field by field
/// (0.7); a file's entry for any other id must name its `kind` ([`SettingsError::MissingKind`]).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AgentEntry {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<AgentKind>,
    /// The program an ACP agent starts. Required for `kind: acp`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    /// An ACP agent's arguments. `--mdns`, `--cors` and a `--hostname` that is not loopback are refused.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub args: Option<Vec<String>>,
    /// A model name for Claude Code (`haiku`), or an opaque id for ACP (`ollama/qwen2.5:7b`, 0.3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Which Claude settings the agent tool reads: `none`, or `user` for "use my Claude config" (0.3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config: Option<RoleConfig>,
    /// Folders it may read. `<board root>` stands for the board's default line root.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub read: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<ToolKind>>,
    /// Collide's timeout for one ask, in seconds: 5 to 1800.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 5, max = 1800))]
    pub timeout_s: Option<u64>,
    /// An optional cap on the spend of one ask. Claude Code only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget_usd: Option<f64>,
    /// Environment for the child. A literal value under a name that ends in `KEY`, `TOKEN`, `SECRET` or
    /// `PASSWORD` is refused: use `secrets`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub env: Option<BTreeMap<String, String>>,
    /// Environment variable to the name of an item in the OS keychain. A reference, never a value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secrets: Option<BTreeMap<String, String>>,
}

/// A key of an [`AgentEntry`], as the file writes it.
pub const AGENT_KEYS: [&str; 11] = [
    "kind",
    "command",
    "args",
    "model",
    "config",
    "read",
    "tools",
    "timeout_s",
    "budget_usd",
    "env",
    "secrets",
];

impl AgentEntry {
    /// The keys this entry sets, in the order of [`AGENT_KEYS`].
    pub fn keys_set(&self) -> Vec<&'static str> {
        let set = [
            self.kind.is_some(),
            self.command.is_some(),
            self.args.is_some(),
            self.model.is_some(),
            self.config.is_some(),
            self.read.is_some(),
            self.tools.is_some(),
            self.timeout_s.is_some(),
            self.budget_usd.is_some(),
            self.env.is_some(),
            self.secrets.is_some(),
        ];
        AGENT_KEYS
            .into_iter()
            .zip(set)
            .filter_map(|(key, is_set)| is_set.then_some(key))
            .collect()
    }
}

/// `ask:`. `agent` names the entry the ask box uses. `budget_usd` and `timeout_s` are the defaults for every
/// ask, and the only keys a repo file may set here.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Ask {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget_usd: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 5, max = 1800))]
    pub timeout_s: Option<u64>,
}

impl Ask {
    fn is_empty(&self) -> bool {
        *self == Ask::default()
    }

    /// The keys this block sets, as `ask.<key>`.
    pub fn keys_set(&self) -> Vec<&'static str> {
        let mut keys = Vec::new();
        if self.agent.is_some() {
            keys.push("ask.agent");
        }
        if self.budget_usd.is_some() {
            keys.push("ask.budget_usd");
        }
        if self.timeout_s.is_some() {
            keys.push("ask.timeout_s");
        }
        keys
    }
}

/// `.collide/config.yaml` and `~/.config/collide/config.yaml`.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConfigFile {
    /// One entry per agent id. Only the user's file may hold these (0.7).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub agents: BTreeMap<String, AgentEntry>,
    #[serde(default, skip_serializing_if = "Ask::is_empty")]
    pub ask: Ask,
    /// Pack settings: pack name, then key, then value. A pack's schema checks each value (Stage 4a), and a key
    /// the pack marks `secret: true` is refused here ([`secrets_in_file`]).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub config: BTreeMap<String, BTreeMap<String, Value>>,
}

/// The built-in entry, written as 0.7 shows it with every field it has. The first settings layer holds it, and the
/// user's file may change it field by field.
pub fn builtin_agents() -> BTreeMap<String, AgentEntry> {
    BTreeMap::from([(
        BUILTIN_AGENT_ID.to_string(),
        AgentEntry {
            kind: Some(AgentKind::ClaudeCode),
            model: Some("haiku".into()),
            config: Some(RoleConfig::None),
            read: Some(vec!["<board root>".into()]),
            tools: Some(vec![ToolKind::Read, ToolKind::Search]),
            timeout_s: Some(180),
            ..AgentEntry::default()
        },
    )])
}

/// Why a settings file is refused. The load names the key.
#[derive(Debug, Clone, PartialEq)]
pub enum SettingsError {
    /// An `agents` key that is empty.
    EmptyAgentId,
    /// An entry other than the built-in one with no `kind`: nothing below it supplies one.
    MissingKind { id: String },
    /// `kind: acp` with no `command`.
    AcpWithoutCommand { id: String },
    /// A `timeout_s` outside [`MIN_TIMEOUT_S`] to [`MAX_TIMEOUT_S`]. `key` is `agents.<id>.timeout_s` or
    /// `ask.timeout_s`.
    TimeoutOutOfRange { key: String, value: u64 },
    /// A `budget_usd` on an entry whose kind cannot enforce one: only Claude Code can.
    BudgetNotEnforceable { id: String, kind: AgentKind },
    /// An `env` line with a secret-looking name and a literal value.
    LiteralSecretInEnv { id: String, name: String },
    /// An ACP argument that is refused. `reason` says why.
    RefusedAcpArg {
        id: String,
        arg: String,
        reason: &'static str,
    },
    /// `ask.agent` that is empty.
    EmptyAskAgent,
    /// A pack name in `config:` that is empty.
    EmptyConfigPack,
}

impl fmt::Display for SettingsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SettingsError::EmptyAgentId => write!(f, "`agents` has an entry with no id"),
            SettingsError::MissingKind { id } => write!(
                f,
                "`agents.{id}.kind` is missing; only `{BUILTIN_AGENT_ID}` has a built-in kind"
            ),
            SettingsError::AcpWithoutCommand { id } => {
                write!(f, "`agents.{id}.command` is missing; `kind: acp` needs one")
            }
            SettingsError::TimeoutOutOfRange { key, value } => write!(
                f,
                "`{key}: {value}` is outside {MIN_TIMEOUT_S} to {MAX_TIMEOUT_S} seconds"
            ),
            SettingsError::BudgetNotEnforceable { id, kind } => write!(
                f,
                "`agents.{id}.budget_usd`: a `{}` agent cannot enforce a budget; only Claude Code can, \
                 and the others get a timeout only",
                kind.as_str()
            ),
            SettingsError::LiteralSecretInEnv { id, name } => write!(
                f,
                "`agents.{id}.env.{name}` holds a literal value under a secret-looking name; \
                 use `secrets` and a keychain item"
            ),
            SettingsError::RefusedAcpArg { id, arg, reason } => {
                write!(f, "`agents.{id}.args`: `{arg}` is refused: {reason}")
            }
            SettingsError::EmptyAskAgent => write!(f, "`ask.agent` is empty"),
            SettingsError::EmptyConfigPack => write!(f, "`config:` has an entry with no pack name"),
        }
    }
}

impl std::error::Error for SettingsError {}

fn is_loopback_host(host: &str) -> bool {
    let host = host.trim_start_matches('[').trim_end_matches(']');
    host.eq_ignore_ascii_case("localhost")
        || host.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback())
}

/// The arguments of an ACP agent that Collide refuses (0.3, An ACP agent's listener): `--mdns`, any `--cors`, and
/// a `--hostname` whose value is not a loopback address. Both `--flag value` and `--flag=value` are read. Returns
/// each refused argument with the reason, in order.
pub fn refused_acp_args(args: &[String]) -> Vec<(String, &'static str)> {
    const MDNS: &str = "it announces the agent on the local network";
    const CORS: &str = "it opens the agent's listener to web pages";
    const HOST: &str = "the hostname must be a loopback address";
    let mut refused = Vec::new();
    let mut index = 0;
    while index < args.len() {
        let arg = args[index].as_str();
        if arg == "--mdns" || arg.starts_with("--mdns=") {
            refused.push((arg.to_string(), MDNS));
        } else if arg == "--cors" || arg.starts_with("--cors=") {
            refused.push((arg.to_string(), CORS));
        } else if arg == "--hostname" {
            let value = args.get(index + 1).map(String::as_str);
            if !value.is_some_and(is_loopback_host) {
                refused.push((
                    format!("{arg} {}", value.unwrap_or("")).trim().to_string(),
                    HOST,
                ));
            }
            index += 1;
        } else if let Some(value) = arg.strip_prefix("--hostname=")
            && !is_loopback_host(value)
        {
            refused.push((arg.to_string(), HOST));
        }
        index += 1;
    }
    refused
}

fn check_timeout(key: String, value: Option<u64>, errors: &mut Vec<SettingsError>) {
    if let Some(value) = value
        && !(MIN_TIMEOUT_S..=MAX_TIMEOUT_S).contains(&value)
    {
        errors.push(SettingsError::TimeoutOutOfRange { key, value });
    }
}

fn check_agent(id: &str, entry: &AgentEntry, errors: &mut Vec<SettingsError>) {
    if id.trim().is_empty() {
        errors.push(SettingsError::EmptyAgentId);
    }
    if entry.kind.is_none() && id != BUILTIN_AGENT_ID {
        errors.push(SettingsError::MissingKind { id: id.into() });
    }
    if entry.kind == Some(AgentKind::Acp)
        && entry
            .command
            .as_deref()
            .is_none_or(|command| command.trim().is_empty())
    {
        errors.push(SettingsError::AcpWithoutCommand { id: id.into() });
    }
    check_timeout(format!("agents.{id}.timeout_s"), entry.timeout_s, errors);
    if let Some(kind) = entry.kind
        && kind != AgentKind::ClaudeCode
        && entry.budget_usd.is_some()
    {
        errors.push(SettingsError::BudgetNotEnforceable {
            id: id.into(),
            kind,
        });
    }
    for (name, value) in entry.env.iter().flatten() {
        let upper = name.to_ascii_uppercase();
        if !value.is_empty() && SECRET_NAME_ENDINGS.iter().any(|end| upper.ends_with(end)) {
            errors.push(SettingsError::LiteralSecretInEnv {
                id: id.into(),
                name: name.clone(),
            });
        }
    }
    if entry.kind == Some(AgentKind::Acp) {
        for (arg, reason) in refused_acp_args(entry.args.as_deref().unwrap_or_default()) {
            errors.push(SettingsError::RefusedAcpArg {
                id: id.into(),
                arg,
                reason,
            });
        }
    }
}

/// Checks one settings file on its own, whichever file it is. Returns every refusal: the entries in the order of
/// their ids, then `ask`, then `config:`. Which keys the file's place allows is [`repo_file_refusals`]; whether
/// `ask.agent` names an entry, and every merged value, is Stage 4a's.
pub fn check_config_file(file: &ConfigFile) -> Result<(), Vec<SettingsError>> {
    let mut errors = Vec::new();
    for (id, entry) in &file.agents {
        check_agent(id, entry, &mut errors);
    }
    if file
        .ask
        .agent
        .as_deref()
        .is_some_and(|agent| agent.trim().is_empty())
    {
        errors.push(SettingsError::EmptyAskAgent);
    }
    check_timeout("ask.timeout_s".into(), file.ask.timeout_s, &mut errors);
    if file.config.keys().any(|pack| pack.trim().is_empty()) {
        errors.push(SettingsError::EmptyConfigPack);
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// A key a repo `config.yaml` set and may not: the load refuses the file and names the key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoFileRefusal {
    /// `agents.<id>.<field>` (`agents.<id>` for an entry that sets nothing) or `ask.agent`.
    pub key: String,
}

impl fmt::Display for RepoFileRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "the repo's config may not set `{}`: only {} are a team's to set, because a committed \
             agent would be code that Collide runs on the strength of a trust prompt",
            self.key,
            REPO_FILE_AGENT_ASK_KEYS
                .map(|key| format!("`{key}`"))
                .join(" and ")
        )
    }
}

impl std::error::Error for RepoFileRefusal {}

/// The `agents` and `ask` keys of a repo `config.yaml` that are not in [`REPO_FILE_AGENT_ASK_KEYS`], one refusal
/// each: every field of every `agents` entry, and `ask.agent`. A budget that later reaches an agent which cannot
/// enforce one is no load error (0.7), and is not looked at here.
pub fn repo_file_refusals(file: &ConfigFile) -> Vec<RepoFileRefusal> {
    let mut refusals = Vec::new();
    for (id, entry) in &file.agents {
        let fields = entry.keys_set();
        if fields.is_empty() {
            refusals.push(RepoFileRefusal {
                key: format!("agents.{id}"),
            });
        }
        for field in fields {
            refusals.push(RepoFileRefusal {
                key: format!("agents.{id}.{field}"),
            });
        }
    }
    refusals.extend(
        file.ask
            .keys_set()
            .into_iter()
            .filter(|key| !REPO_FILE_AGENT_ASK_KEYS.contains(key))
            .map(|key| RepoFileRefusal { key: key.into() }),
    );
    refusals
}

/// A `secret: true` pack key found in a settings file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretInFile {
    pub pack: String,
    pub key: String,
}

impl fmt::Display for SecretInFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "`config.{}.{}` is a secret and may not be in a settings file; \
             set it with `collide config set --secret`",
            self.pack, self.key
        )
    }
}

impl std::error::Error for SecretInFile {}

/// The keys of `file.config` that a pack marked `secret: true` (0.7, Secrets). `secret_keys` holds each secret
/// key as `(pack, key)`: reading them from the packs' `config:` schemas is Stage 4a's. Both files are checked the
/// same way, and a secret in either is a load error.
pub fn secrets_in_file(
    file: &ConfigFile,
    secret_keys: &BTreeSet<(String, String)>,
) -> Vec<SecretInFile> {
    file.config
        .iter()
        .flat_map(|(pack, keys)| keys.keys().map(move |key| (pack, key)))
        .filter(|(pack, key)| secret_keys.contains(&((*pack).clone(), (*key).clone())))
        .map(|(pack, key)| SecretInFile {
            pack: pack.clone(),
            key: key.clone(),
        })
        .collect()
}
