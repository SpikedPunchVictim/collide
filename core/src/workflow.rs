//! A workflow as a declared transition table (0.7, A workflow is a declared transition table): the types of the
//! `workflows:` key of `pack.yaml`. The checks that need the whole table (a `then:` that names no phase, an
//! undeclared decision token, identical fan-out instances) are in [`crate::workflow_check`]. Running a workflow is
//! the workflow engine's (Stage 8).
//!
//! A phase is written without a `kind:`: its kind is the key that names what it does, as in the reference pack.
//!
//! | key | kind |
//! |---|---|
//! | `role:` | `agent`, or `fanout` when it has `fanout:` |
//! | `script:` | `script` |
//! | `wait:` | `wait` |
//! | `human:` | `human` |
//!
//! Every struct is closed, and a key that does not belong to the phase's kind is refused by name. A [`Phase`]
//! therefore reads its own keys in [`Deserialize`] below, and serializes as the one struct it holds.

use crate::duration::Duration;
use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::de::value::MapAccessDeserializer;
use serde::de::{self, MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fmt;
use std::marker::PhantomData;

/// The target that pauses the run in the Queue (0.7, `ask_human`). No phase may have this name.
pub const ASK_HUMAN: &str = "ask_human";

/// A `wait` phase's `interval:` when it has none (0.7).
pub const DEFAULT_WAIT_INTERVAL: Duration = Duration::from_secs(30);

/// The shortest `interval:` of a `wait` phase (0.5: a poll is never sooner than 1 s).
pub const MIN_WAIT_INTERVAL: Duration = Duration::from_secs(1);

/// Reads a scalar or a mapping, so that a value that is a name or a small object needs no buffering of the
/// document and keeps serde's own error for an unknown key inside the object.
enum NameOrMap<M> {
    Name(String),
    Map(M),
}

struct NameOrMapVisitor<M> {
    expecting: &'static str,
    map: PhantomData<M>,
}

impl<'de, M: Deserialize<'de>> Visitor<'de> for NameOrMapVisitor<M> {
    type Value = NameOrMap<M>;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.expecting)
    }

    fn visit_str<E: de::Error>(self, text: &str) -> Result<Self::Value, E> {
        Ok(NameOrMap::Name(text.to_string()))
    }

    fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
        M::deserialize(MapAccessDeserializer::new(map)).map(NameOrMap::Map)
    }
}

fn name_or_map<'de, D: Deserializer<'de>, M: Deserialize<'de>>(
    deserializer: D,
    expecting: &'static str,
) -> Result<NameOrMap<M>, D::Error> {
    deserializer.deserialize_any(NameOrMapVisitor {
        expecting,
        map: PhantomData,
    })
}

/// `{ end: <outcome> }`: the run ends with this outcome (0.7, Outcomes and terminal states).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EndTarget {
    #[schemars(length(min = 1))]
    pub end: String,
}

/// Where a transition goes: a phase name, `ask_human`, or `{ end: <outcome> }`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(untagged)]
pub enum Target {
    End(EndTarget),
    /// A phase name, or [`ASK_HUMAN`].
    Name(String),
}

impl Target {
    pub fn is_ask_human(&self) -> bool {
        matches!(self, Target::Name(name) if name == ASK_HUMAN)
    }
}

impl<'de> Deserialize<'de> for Target {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Target, D::Error> {
        Ok(
            match name_or_map::<_, EndTarget>(
                deserializer,
                "a phase name, `ask_human` or `{ end: <outcome> }`",
            )? {
                NameOrMap::Name(name) => Target::Name(name),
                NameOrMap::Map(end) => Target::End(end),
            },
        )
    }
}

/// A decision: a hook that returns one token per task, and the target each token goes to (0.7, `then:`).
/// `tokens:` is optional; when it is there it declares the tokens the script may return, and `on:` must map
/// exactly those. Without it, the keys of `on:` are the declared tokens.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Decision {
    #[schemars(length(min = 1))]
    pub script: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tokens: Vec<String>,
    pub on: BTreeMap<String, Target>,
    /// What the person reads when a token reaches `ask_human` (0.7, `show:`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub show: Vec<String>,
}

impl Decision {
    /// The target of a token the script returned, or `None` when the token is not declared (0.7: an undeclared
    /// token makes the decision `error`).
    pub fn target_for(&self, token: &str) -> Option<&Target> {
        self.on.get(token)
    }
}

/// What follows a phase (0.7, `then:`). Without it, the next phase in declaration order follows, and the last
/// phase ends the run with outcome `done`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(untagged)]
pub enum Then {
    Decision(Decision),
    Target(Target),
}

/// The keys of a `then:` that is a mapping: either an `end`, or a decision.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawThen {
    end: Option<String>,
    script: Option<String>,
    #[serde(default)]
    tokens: Vec<String>,
    #[serde(default)]
    on: BTreeMap<String, Target>,
    #[serde(default)]
    show: Vec<String>,
}

impl<'de> Deserialize<'de> for Then {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Then, D::Error> {
        let raw = match name_or_map::<_, RawThen>(
            deserializer,
            "a phase name, `ask_human`, `{ end: <outcome> }` or a decision `{ script, on }`",
        )? {
            NameOrMap::Name(name) => return Ok(Then::Target(Target::Name(name))),
            NameOrMap::Map(raw) => raw,
        };
        let decision_keys = raw.script.is_some()
            || !raw.tokens.is_empty()
            || !raw.on.is_empty()
            || !raw.show.is_empty();
        match (raw.end, raw.script) {
            (Some(end), None) if !decision_keys => Ok(Then::Target(Target::End(EndTarget { end }))),
            (Some(_), _) => Err(de::Error::custom(
                "`end` cannot be combined with the keys of a decision (`script`, `tokens`, `on`, `show`)",
            )),
            (None, Some(script)) => Ok(Then::Decision(Decision {
                script,
                tokens: raw.tokens,
                on: raw.on,
                show: raw.show,
            })),
            (None, None) if decision_keys => {
                Err(de::Error::custom("a decision needs `script:` and `on:`"))
            }
            (None, None) => Err(de::Error::custom(
                "an empty `then:`; name a phase, write `{ end: <outcome> }` or a decision",
            )),
        }
    }
}

/// A prompt: the name of one of the pack's prompts, or an earlier phase's artifact (`{ artifact: brief/x.md }`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(untagged)]
pub enum Prompt {
    Artifact(ArtifactPrompt),
    Name(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ArtifactPrompt {
    #[schemars(length(min = 1))]
    pub artifact: String,
}

impl<'de> Deserialize<'de> for Prompt {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Prompt, D::Error> {
        Ok(
            match name_or_map::<_, ArtifactPrompt>(
                deserializer,
                "a prompt name or `{ artifact: <path> }`",
            )? {
                NameOrMap::Name(name) => Prompt::Name(name),
                NameOrMap::Map(artifact) => Prompt::Artifact(artifact),
            },
        )
    }
}

/// `on_cold:` of an agent phase (0.3): what a phase does when its session's cache has expired.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum OnCold {
    /// Keep the session.
    #[default]
    Resume,
    /// Start a new session from a handoff note.
    Fresh,
}

/// The models whose phase instance gets a sandbox workspace (0.7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Sandbox {
    pub models: Vec<String>,
}

/// One instance of a fan-out (0.10, owner 2026-11-13): each names any of a model, an agent definition, a profile
/// and a prompt. What an instance leaves out comes from the phase's role.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Instance {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub definition: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<Prompt>,
}

/// `fanout:` of an agent phase: one role on several `models`, or on several `instances`, never both. `when:` is
/// CEL over the run (`round == 1`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Fanout {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub models: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub instances: Vec<Instance>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<String>,
}

/// An `agent` phase: one role. With `fanout:` it is a `fanout` phase.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AgentPhase {
    #[schemars(length(min = 1))]
    pub role: String,
    pub prompt: Prompt,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gates: Vec<String>,
    /// The files the phase writes to its directory.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub outputs: Vec<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub record: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sandbox: Option<Sandbox>,
    #[serde(default)]
    pub on_cold: OnCold,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fanout: Option<Fanout>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub then: Option<Then>,
}

/// A `script` phase: a hook in the workspace. `agentWork: true` marks one that runs agents itself (0.3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ScriptPhase {
    #[schemars(length(min = 1))]
    pub script: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout: Option<Duration>,
    #[serde(
        rename = "agentWork",
        default,
        skip_serializing_if = "std::ops::Not::not"
    )]
    pub agent_work: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gates: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub outputs: Vec<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub record: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub then: Option<Then>,
}

/// A `wait` phase: a hook polled every `interval` until it answers `ok` or `fail`, or `timeout` passes (0.7).
/// `timeout` is required.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WaitPhase {
    #[schemars(length(min = 1))]
    pub wait: String,
    #[serde(default = "default_interval")]
    pub interval: Duration,
    pub timeout: Duration,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub then: Option<Then>,
}

fn default_interval() -> Duration {
    DEFAULT_WAIT_INTERVAL
}

/// A `human` phase: a Queue item where a person picks one of the tokens of `on:`. `human` is the text the item
/// shows; `show` lists what the person should read (0.7, `show:`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HumanPhase {
    #[schemars(length(min = 1))]
    pub human: String,
    pub on: BTreeMap<String, Target>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub show: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PhaseKind {
    Agent,
    Fanout,
    Script,
    Wait,
    Human,
}

impl PhaseKind {
    pub fn name(self) -> &'static str {
        match self {
            PhaseKind::Agent => "agent",
            PhaseKind::Fanout => "fanout",
            PhaseKind::Script => "script",
            PhaseKind::Wait => "wait",
            PhaseKind::Human => "human",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(untagged)]
pub enum Phase {
    Agent(AgentPhase),
    Script(ScriptPhase),
    Wait(WaitPhase),
    Human(HumanPhase),
}

impl Phase {
    pub fn kind(&self) -> PhaseKind {
        match self {
            Phase::Agent(agent) if agent.fanout.is_some() => PhaseKind::Fanout,
            Phase::Agent(_) => PhaseKind::Agent,
            Phase::Script(_) => PhaseKind::Script,
            Phase::Wait(_) => PhaseKind::Wait,
            Phase::Human(_) => PhaseKind::Human,
        }
    }

    /// The phase's own `then:`. A human phase has none: its `on:` is its table.
    pub fn then(&self) -> Option<&Then> {
        match self {
            Phase::Agent(phase) => phase.then.as_ref(),
            Phase::Script(phase) => phase.then.as_ref(),
            Phase::Wait(phase) => phase.then.as_ref(),
            Phase::Human(_) => None,
        }
    }

    /// Every transition written on this phase, with where it was written (`then`, `then.on.fix`, `on.stop`).
    /// The next phase in declaration order, which a missing `then:` means, is not written and not listed.
    pub fn targets(&self) -> Vec<(String, &Target)> {
        let mut found = Vec::new();
        match self.then() {
            Some(Then::Target(target)) => found.push(("then".to_string(), target)),
            Some(Then::Decision(decision)) => {
                for (token, target) in &decision.on {
                    found.push((format!("then.on.{token}"), target));
                }
            }
            None => {}
        }
        if let Phase::Human(human) = self {
            for (token, target) in &human.on {
                found.push((format!("on.{token}"), target));
            }
        }
        found
    }
}

/// Every key a phase can have, as the file writes it. Which of them a phase may use depends on its kind.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPhase {
    role: Option<String>,
    prompt: Option<Prompt>,
    label: Option<String>,
    gates: Option<Vec<String>>,
    outputs: Option<Vec<String>>,
    record: Option<bool>,
    sandbox: Option<Sandbox>,
    on_cold: Option<OnCold>,
    fanout: Option<Fanout>,
    script: Option<String>,
    #[serde(rename = "agentWork")]
    agent_work: Option<bool>,
    timeout: Option<Duration>,
    wait: Option<String>,
    interval: Option<Duration>,
    human: Option<String>,
    on: Option<BTreeMap<String, Target>>,
    show: Option<Vec<String>>,
    then: Option<Then>,
}

const AGENT_KEYS: &[&str] = &[
    "role", "prompt", "label", "gates", "outputs", "record", "sandbox", "on_cold", "fanout", "then",
];
const SCRIPT_KEYS: &[&str] = &[
    "script",
    "timeout",
    "agentWork",
    "label",
    "gates",
    "outputs",
    "record",
    "then",
];
const WAIT_KEYS: &[&str] = &["wait", "interval", "timeout", "label", "then"];
const HUMAN_KEYS: &[&str] = &["human", "on", "show", "label"];

impl RawPhase {
    /// The keys the file wrote, in a fixed order.
    fn present(&self) -> Vec<&'static str> {
        let all = [
            ("role", self.role.is_some()),
            ("prompt", self.prompt.is_some()),
            ("label", self.label.is_some()),
            ("gates", self.gates.is_some()),
            ("outputs", self.outputs.is_some()),
            ("record", self.record.is_some()),
            ("sandbox", self.sandbox.is_some()),
            ("on_cold", self.on_cold.is_some()),
            ("fanout", self.fanout.is_some()),
            ("script", self.script.is_some()),
            ("agentWork", self.agent_work.is_some()),
            ("timeout", self.timeout.is_some()),
            ("wait", self.wait.is_some()),
            ("interval", self.interval.is_some()),
            ("human", self.human.is_some()),
            ("on", self.on.is_some()),
            ("show", self.show.is_some()),
            ("then", self.then.is_some()),
        ];
        all.into_iter()
            .filter(|(_, present)| *present)
            .map(|(key, _)| key)
            .collect()
    }

    fn into_phase(self) -> Result<Phase, String> {
        let present = self.present();
        let kind_keys: Vec<&str> = ["role", "script", "wait", "human"]
            .into_iter()
            .filter(|key| present.contains(key))
            .collect();
        let (kind, allowed) = match kind_keys.as_slice() {
            ["role"] => ("an agent", AGENT_KEYS),
            ["script"] => ("a script", SCRIPT_KEYS),
            ["wait"] => ("a wait", WAIT_KEYS),
            ["human"] => ("a human", HUMAN_KEYS),
            [] => {
                return Err(
                    "a phase needs `role:` (agent), `script:`, `wait:` or `human:` to say what it is"
                        .to_string(),
                );
            }
            several => {
                let named: Vec<String> = several.iter().map(|key| format!("`{key}`")).collect();
                return Err(format!(
                    "a phase is one kind, but this one has {}",
                    named.join(" and ")
                ));
            }
        };
        if let Some(key) = present.iter().find(|key| !allowed.contains(key)) {
            return Err(format!("`{key}` is not a key of {kind} phase"));
        }
        let missing = |key: &str| format!("{kind} phase needs `{key}:`");
        Ok(match kind_keys[0] {
            "role" => Phase::Agent(AgentPhase {
                role: self.role.ok_or_else(|| missing("role"))?,
                prompt: self.prompt.ok_or_else(|| missing("prompt"))?,
                label: self.label,
                gates: self.gates.unwrap_or_default(),
                outputs: self.outputs.unwrap_or_default(),
                record: self.record.unwrap_or_default(),
                sandbox: self.sandbox,
                on_cold: self.on_cold.unwrap_or_default(),
                fanout: self.fanout,
                then: self.then,
            }),
            "script" => Phase::Script(ScriptPhase {
                script: self.script.ok_or_else(|| missing("script"))?,
                timeout: self.timeout,
                agent_work: self.agent_work.unwrap_or_default(),
                label: self.label,
                gates: self.gates.unwrap_or_default(),
                outputs: self.outputs.unwrap_or_default(),
                record: self.record.unwrap_or_default(),
                then: self.then,
            }),
            "wait" => Phase::Wait(WaitPhase {
                wait: self.wait.ok_or_else(|| missing("wait"))?,
                interval: self.interval.unwrap_or(DEFAULT_WAIT_INTERVAL),
                timeout: self.timeout.ok_or_else(|| missing("timeout"))?,
                label: self.label,
                then: self.then,
            }),
            _ => Phase::Human(HumanPhase {
                human: self.human.ok_or_else(|| missing("human"))?,
                on: self.on.ok_or_else(|| missing("on"))?,
                show: self.show.unwrap_or_default(),
                label: self.label,
            }),
        })
    }
}

impl<'de> Deserialize<'de> for Phase {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Phase, D::Error> {
        RawPhase::deserialize(deserializer)?
            .into_phase()
            .map_err(de::Error::custom)
    }
}

/// The phases of a workflow, by name, in declaration order. Order is part of the table: a phase without `then:`
/// is followed by the next one written (0.7).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PhaseTable(Vec<(String, Phase)>);

impl PhaseTable {
    pub fn new(phases: Vec<(String, Phase)>) -> PhaseTable {
        PhaseTable(phases)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &Phase)> {
        self.0.iter().map(|(name, phase)| (name.as_str(), phase))
    }

    pub fn get(&self, name: &str) -> Option<&Phase> {
        self.0
            .iter()
            .find(|(phase_name, _)| phase_name == name)
            .map(|(_, phase)| phase)
    }

    pub fn contains(&self, name: &str) -> bool {
        self.get(name).is_some()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn first(&self) -> Option<&str> {
        self.0.first().map(|(name, _)| name.as_str())
    }

    /// The phase written after `name`, which follows it when it has no `then:`. `None` for the last phase.
    pub fn next_after(&self, name: &str) -> Option<&str> {
        let at = self
            .0
            .iter()
            .position(|(phase_name, _)| phase_name == name)?;
        self.0.get(at + 1).map(|(next, _)| next.as_str())
    }
}

impl Serialize for PhaseTable {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        for (name, phase) in &self.0 {
            map.serialize_entry(name, phase)?;
        }
        map.end()
    }
}

struct PhaseTableVisitor;

impl<'de> Visitor<'de> for PhaseTableVisitor {
    type Value = PhaseTable;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a map of phase names to phases")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<PhaseTable, A::Error> {
        let mut phases: Vec<(String, Phase)> = Vec::new();
        while let Some(name) = map.next_key::<String>()? {
            if phases.iter().any(|(seen, _)| *seen == name) {
                return Err(de::Error::custom(format!(
                    "phase `{name}` is declared twice"
                )));
            }
            phases.push((name, map.next_value()?));
        }
        Ok(PhaseTable(phases))
    }
}

impl<'de> Deserialize<'de> for PhaseTable {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<PhaseTable, D::Error> {
        deserializer.deserialize_map(PhaseTableVisitor)
    }
}

impl JsonSchema for PhaseTable {
    fn schema_name() -> Cow<'static, str> {
        Cow::Borrowed("PhaseTable")
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": "object",
            "description": "Phase names to phases, in declaration order.",
            "additionalProperties": generator.subschema_for::<Phase>()
        })
    }
}

/// Which tasks a workflow can take (0.7, `accepts`): the sources, and the template fields it needs.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Accepts {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sources: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<String>,
}

/// An op of `on_start` or `on_end` (0.4, Lifecycle order). It addresses status categories (`setStatus
/// { category: active }`). `when:` is CEL; in `on_end` it reads `outcome`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkflowOp {
    #[schemars(length(min = 1))]
    pub op: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub args: BTreeMap<String, Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<String>,
}

/// One entry of `workflows:`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Workflow {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accepts: Option<Accepts>,
    /// A `pack/name` workspace implementation. Without it the built-in one is used (0.4).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<String>,
    /// A `pack/name` integrate implementation. A workflow without one cannot be accepted (0.4).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub integrate: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub on_start: Vec<WorkflowOp>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub on_end: Vec<WorkflowOp>,
    /// The first phase. The first one declared when this is missing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<String>,
    pub phases: PhaseTable,
}

impl Workflow {
    /// The phase a run begins at, when the table has one.
    pub fn start_phase(&self) -> Option<&str> {
        self.start.as_deref().or_else(|| self.phases.first())
    }
}

/// A run that starts at a named phase and round, in an existing workspace, so that a chain already in flight
/// moves into Collide without starting over (0.7, Adopting).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Adoption {
    pub phase: String,
    /// Starts at 1.
    pub round: u32,
}
