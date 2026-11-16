//! The agent entries through the layers (ts-0099-bw; 0.7, Agent entries): for each agent id and each field, the value
//! and the layer it came from. A pure function over the loaded files; nothing is read here.
//!
//! - The first layer holds the built-in entry ([`builtin_agents`]). The user's file, then the board's own user file,
//!   change it field by field and add entries. The repo's file cannot set a field of an entry (the load refuses it,
//!   [`crate::settings::repo_file_refusals`]), so none is applied.
//! - `ask.budget_usd` and `ask.timeout_s` are defaults for every ask, not per id. The repo's `ask.budget_usd` wins
//!   over an entry's own `budget_usd`, so a team's cap holds (owner, 2026-11-16). An entry's own `timeout_s` wins over
//!   the repo's default, and so does the user's `ask.timeout_s`, which is `overridable`.
//! - A default budget that reaches an agent that cannot enforce one (ACP, external) is not applied, and a notice names
//!   the agent. It is no error: the repo's file cannot know which agent a developer picked.
//! - `ask.agent` names the entry the ask box uses. It is the user's choice, so only the user's files are read for it,
//!   and with none it is [`BUILTIN_AGENT_ID`]. One that names no entry is a problem.
//!
//! A refusal that needs one file only (a timeout out of range, a budget on an ACP entry, a literal secret) is
//! [`crate::settings::check_agents`]'s.

use crate::owner::{ConfigEntry, SettingLayer};
use crate::settings::{
    AGENT_KEYS, AgentEntry, AgentKind, BUILTIN_AGENT_ID, ConfigFile, DEFAULT_TIMEOUT_S,
    builtin_agents,
};
use crate::settings_layers::SettingProblem;
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};

/// The three files the entries are read from.
#[derive(Debug, Clone, Copy)]
pub struct AgentInputs<'a> {
    /// `~/.config/collide/config.yaml`.
    pub user: &'a ConfigFile,
    /// `~/.config/collide/boards/<board-id>/config.yaml`: wins over `user`.
    pub user_board: &'a ConfigFile,
    /// The line snapshot's `.collide/config.yaml`.
    pub repo: &'a ConfigFile,
}

/// One agent entry as the layers resolve it.
#[derive(Debug, Clone, PartialEq)]
pub struct AgentView {
    pub id: String,
    /// Every field resolved. A field no layer gives is `None`, except `timeout_s`, which every agent has.
    pub entry: AgentEntry,
    layers: BTreeMap<&'static str, SettingLayer>,
}

impl AgentView {
    /// The layer `field` (one of [`AGENT_KEYS`]) came from: [`SettingLayer::Default`] when no file sets it.
    pub fn layer(&self, field: &str) -> SettingLayer {
        self.layers
            .get(field)
            .copied()
            .unwrap_or(SettingLayer::Default)
    }

    pub fn kind(&self) -> Option<AgentKind> {
        self.entry.kind
    }

    /// One [`ConfigEntry`] per field in the order of [`AGENT_KEYS`], keyed `agents.<id>.<field>`. A field with no value
    /// has none. `secrets` holds item names, never values, so no entry is a secret.
    pub fn config_entries(&self) -> Vec<ConfigEntry> {
        let fields = fields_of(&self.entry);
        AGENT_KEYS
            .iter()
            .map(|field| ConfigEntry {
                key: format!("agents.{}.{field}", self.id),
                value: fields.get(*field).cloned(),
                layer: self.layer(field),
                secret: false,
            })
            .collect()
    }
}

/// The agent entries, the entry `ask.agent` names, and what is wrong or worth saying.
#[derive(Debug, Clone, PartialEq)]
pub struct AgentLayers {
    /// The built-in entry first, then the others in the order of their ids.
    pub agents: Vec<AgentView>,
    /// The id of the entry the ask box uses.
    pub ask_agent: String,
    pub ask_agent_layer: SettingLayer,
    /// A default budget that was not applied, one line naming the agent.
    pub notices: Vec<String>,
    pub problems: Vec<SettingProblem>,
}

/// The fields an entry sets, by name. The entry's own serialization, which leaves out what it does not set.
fn fields_of(entry: &AgentEntry) -> Map<String, Value> {
    match serde_json::to_value(entry) {
        Ok(Value::Object(fields)) => fields,
        _ => unreachable!("an agent entry serializes as a map"),
    }
}

/// The value and the layer of the default (`ask.timeout_s`, `ask.budget_usd`) a file sets, user files first.
fn default_of(
    inputs: &AgentInputs,
    pick: impl Fn(&ConfigFile) -> Option<Value>,
) -> (Option<Value>, Option<Value>) {
    let user = pick(inputs.user_board).or_else(|| pick(inputs.user));
    (user, pick(inputs.repo))
}

fn merge_entry(
    id: &str,
    inputs: &AgentInputs,
) -> (Map<String, Value>, BTreeMap<&'static str, SettingLayer>) {
    let mut fields = Map::new();
    let mut layers = BTreeMap::new();
    let builtin = builtin_agents();
    let sources = [
        (builtin.get(id), SettingLayer::Default),
        (inputs.user.agents.get(id), SettingLayer::User),
        (inputs.user_board.agents.get(id), SettingLayer::User),
    ];
    for (entry, layer) in sources {
        let Some(entry) = entry else { continue };
        for (name, value) in fields_of(entry) {
            if let Some(field) = AGENT_KEYS.iter().find(|field| **field == name) {
                fields.insert(name, value);
                layers.insert(*field, layer);
            }
        }
    }
    (fields, layers)
}

/// The timeout an entry has: its own from a user file, else the default every ask has (the user's over the repo's),
/// else what the built-in entry says, else [`DEFAULT_TIMEOUT_S`].
fn resolve_timeout(
    fields: &mut Map<String, Value>,
    layers: &mut BTreeMap<&'static str, SettingLayer>,
    inputs: &AgentInputs,
) {
    if layers.get("timeout_s") == Some(&SettingLayer::User) {
        return;
    }
    let (user, repo) = default_of(inputs, |file| file.ask.timeout_s.map(Value::from));
    let (value, layer) = match (user, repo) {
        (Some(value), _) => (Some(value), SettingLayer::User),
        (None, Some(value)) => (Some(value), SettingLayer::Repo),
        (None, None) => (None, SettingLayer::Default),
    };
    if let Some(value) = value {
        fields.insert("timeout_s".into(), value);
        layers.insert("timeout_s", layer);
    } else if !fields.contains_key("timeout_s") {
        fields.insert("timeout_s".into(), Value::from(DEFAULT_TIMEOUT_S));
    }
}

/// The budget an entry has. The repo's default wins over the entry's own, which wins over the user's default. A
/// default that the agent cannot enforce is not applied, and the notice says so.
fn resolve_budget(
    id: &str,
    fields: &mut Map<String, Value>,
    layers: &mut BTreeMap<&'static str, SettingLayer>,
    inputs: &AgentInputs,
    notices: &mut Vec<String>,
) {
    let kind = fields
        .get("kind")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let can_enforce = kind == AgentKind::ClaudeCode.as_str();
    let (user, repo) = default_of(inputs, |file| file.ask.budget_usd.map(Value::from));
    let own = layers.get("budget_usd") == Some(&SettingLayer::User);
    let (chosen, layer) = match (repo, own, user) {
        (Some(value), _, _) => (Some(value), SettingLayer::Repo),
        (None, true, _) => return,
        (None, false, Some(value)) => (Some(value), SettingLayer::User),
        (None, false, None) => return,
    };
    if !can_enforce {
        // An entry with no kind is refused where its file is checked, and has nothing to say here.
        if kind.is_empty() {
            return;
        }
        let whose = if layer == SettingLayer::Repo {
            "repo's"
        } else {
            "user's"
        };
        notices.push(format!(
            "`agents.{id}.budget_usd`: the {whose} `ask.budget_usd` is not applied to `{id}`: a `{kind}` agent cannot \
             enforce a budget, so it gets a timeout only"
        ));
        return;
    }
    if let Some(value) = chosen {
        fields.insert("budget_usd".into(), value);
        layers.insert("budget_usd", layer);
    }
}

/// Resolves the entries. See the module docs.
pub fn agent_layers(inputs: &AgentInputs) -> AgentLayers {
    let others: BTreeSet<&String> = [inputs.user, inputs.user_board]
        .into_iter()
        .flat_map(|file| file.agents.keys())
        .filter(|id| id.as_str() != BUILTIN_AGENT_ID)
        .collect();
    let ids = std::iter::once(BUILTIN_AGENT_ID.to_string()).chain(others.into_iter().cloned());

    let mut notices = Vec::new();
    let agents: Vec<AgentView> = ids
        .map(|id| {
            let (mut fields, mut layers) = merge_entry(&id, inputs);
            resolve_timeout(&mut fields, &mut layers, inputs);
            resolve_budget(&id, &mut fields, &mut layers, inputs, &mut notices);
            let entry = serde_json::from_value(Value::Object(fields))
                .expect("fields that came from agent entries read back as one");
            AgentView { id, entry, layers }
        })
        .collect();

    let named = [
        (inputs.user_board.ask.agent.as_ref(), SettingLayer::User),
        (inputs.user.ask.agent.as_ref(), SettingLayer::User),
    ]
    .into_iter()
    .find_map(|(agent, layer)| agent.map(|agent| (agent.clone(), layer)));
    let (ask_agent, ask_agent_layer) =
        named.unwrap_or_else(|| (BUILTIN_AGENT_ID.to_string(), SettingLayer::Default));

    let mut problems = Vec::new();
    if !agents.iter().any(|agent| agent.id == ask_agent) {
        let ids: Vec<String> = agents
            .iter()
            .map(|agent| format!("`{}`", agent.id))
            .collect();
        problems.push(SettingProblem {
            key: "ask.agent".into(),
            message: format!(
                "`ask.agent` names `{ask_agent}`, and no agent entry has that id; the entries are {}",
                ids.join(", ")
            ),
        });
    }
    AgentLayers {
        agents,
        ask_agent,
        ask_agent_layer,
        notices,
        problems,
    }
}
