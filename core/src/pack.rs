//! `pack.yaml` in full (0.7, Pack manifest): one type, [`PackManifest`], with every key a pack may write, and the
//! pure checks a load runs over it in [`crate::pack_check`]. The shape of each key follows the reference pack's
//! manifest in the plan (0.7, the example) and the keys the plan adds (0.7, Keys added for planning).
//!
//! Every struct is closed (`deny_unknown_fields`), so a key that is not here fails at any depth. A required string
//! field also has `minLength: 1` in the schema. `workflows:` holds the types of [`crate::workflow`], `views:` and
//! `profiles:` those of [`crate::view`] and [`crate::profile`]. Settings (`config:`) and launch parameters
//! (`launch:`) hold one JSON Schema per key, which is data here, not a type.
//!
//! A path in a manifest is a [`String`]: relative to the pack directory, or `repo:<path>` for a path in the line's
//! snapshot (0.7, Paths in `pack.yaml`). [`crate::pack_check::parse_pack_path`] reads that form.

use crate::duration::Duration;
use crate::item::Category;
use crate::points::DerivedPoint;
use crate::profile::{Profile, RepoInstructions, RoleConfig};
use crate::view::View;
use crate::workflow::Workflow;
use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::de::value::{MapAccessDeserializer, SeqAccessDeserializer};
use serde::de::{MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fmt;

/// One JSON Schema, as a pack writes it for a setting (`config:`), a launch parameter (`launch:`) or a hook's
/// output. It is a mapping and its contents are the schema's own, so unknown keys are not refused inside it. A
/// setting may also carry `secret: true`, `overridable: true` and `repoOnly: true` beside its schema keywords (0.7,
/// Settings); `repoOnly` excludes the other two.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SchemaObject(pub Value);

impl JsonSchema for SchemaObject {
    fn schema_name() -> Cow<'static, str> {
        Cow::Borrowed("SchemaObject")
    }

    fn json_schema(_generator: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": "object",
            "description": concat!(
                "A JSON Schema, its keywords not checked here. A setting may add the booleans ",
                "`secret`, `overridable` and `repoOnly`."
            )
        })
    }
}

/// A budget in US dollars: a run's total (at the total, the run goes to `ask_human` with cause budget), or one
/// role's (0.7, `budget: { usd: 20 }`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RunBudget {
    pub usd: f64,
}

/// One line of a checklist template. `when:` is CEL over the item.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ChecklistTemplateItem {
    #[schemars(length(min = 1))]
    pub id: String,
    #[schemars(length(min = 1))]
    pub text: String,
    #[serde(default)]
    pub required: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<String>,
}

/// A checklist template. `gates` is the category whose status write the pack's gate blocks. The template's name
/// is its key in `checklists:`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ChecklistTemplate {
    pub version: u32,
    pub gates: Category,
    pub items: Vec<ChecklistTemplateItem>,
}

/// A tool the pack needs on `PATH` (0.7, `requires.tools`). `version` is a range such as `">=22.18"`; it is
/// optional, as in `- { name: pnpm }`. `hint` is the pack's install advice, shown when the tool is missing (0.7: "a
/// missing tool fails the load with the pack's install `hint:`"). Checking the tool is Stage 4a.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ToolRequirement {
    #[schemars(length(min = 1))]
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
}

/// `requires:` in a manifest (0.7). Whether the running Collide, its SDK, the tools and the agents satisfy it is
/// checked when a board loads the pack (Stage 4).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Requires {
    /// The range of `@collide/pack-sdk` the pack accepts (0.6), such as `">=0.1 <0.2"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sdk: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<ToolRequirement>,
    /// The agents a pack is bound to (`[claude]`, 0.3).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub agents: Vec<String>,
    /// A regular expression that a `model.name` must match (0.3), such as `"^[a-z]+$"`.
    #[serde(rename = "modelName", default, skip_serializing_if = "Option::is_none")]
    pub model_name: Option<String>,
}

/// One entry of `providers:`: the script that starts the provider, relative to the pack directory.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProviderEntry {
    #[schemars(length(min = 1))]
    pub script: String,
}

/// A hook (0.5): the script that runs, how long it may run, how long it gets to stop, and a JSON Schema for each
/// output it may return. It is the entry of `gates:`, `integrations:`, `syncs:`, `metrics:` and a workspace
/// implementation's sub-hooks. `timeout` and `grace` use [`Duration`] (`120m`, `60s`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HookEntry {
    #[schemars(length(min = 1))]
    pub script: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout: Option<Duration>,
    /// The time a stopped hook gets between SIGTERM and SIGKILL (0.5: default 60 s).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grace: Option<Duration>,
    /// A schema per output key. Outputs that fail it make the hook's result `error` (0.5).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub outputs: BTreeMap<String, SchemaObject>,
}

/// A workspace implementation (0.4): `demo/worktree`. Every hook returns `{ path, gitDir }` (0.1). `create` is
/// the one a workspace cannot be made without; `resume`, `remove`, `discard` and `cleanup` are the lifecycle's other
/// steps (0.4, Lifecycle order), and `sandbox`, `line` and `doctor` are the kinds and the check the reference pack
/// has (0.7, the example).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceImpl {
    pub create: HookEntry,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sandbox: Option<HookEntry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<HookEntry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resume: Option<HookEntry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub doctor: Option<HookEntry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remove: Option<HookEntry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub discard: Option<HookEntry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cleanup: Option<HookEntry>,
}

/// `record:` (0.7): what the pack's `record` hook may commit at the line root. `paths` are globs over repo paths
/// and an upper bound: only the files a write or a phase touched are committed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Record {
    pub paths: Vec<String>,
    #[schemars(length(min = 1))]
    pub script: String,
}

/// When a violation of `protectedPaths` is acted on (0.9). Only `phase-end` exists: the phase fails at its end.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ProtectedAt {
    PhaseEnd,
}

/// `protectedPaths:` (0.9): a list of repo globs, whose changes are flagged in review, or
/// `{ paths: [...], at: phase-end }`, which also fails the phase at its end. It is read by what it holds (a list
/// or a mapping), so that an unknown key in the mapping is refused by name, not as "no variant matched".
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(untagged)]
pub enum ProtectedPaths {
    Paths(Vec<String>),
    AtPhaseEnd(ProtectedPathsAt),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProtectedPathsAt {
    pub paths: Vec<String>,
    pub at: ProtectedAt,
}

struct ProtectedPathsVisitor;

impl<'de> Visitor<'de> for ProtectedPathsVisitor {
    type Value = ProtectedPaths;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a list of globs, or `{ paths: [...], at: phase-end }`")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, seq: A) -> Result<ProtectedPaths, A::Error> {
        Vec::<String>::deserialize(SeqAccessDeserializer::new(seq)).map(ProtectedPaths::Paths)
    }

    fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<ProtectedPaths, A::Error> {
        ProtectedPathsAt::deserialize(MapAccessDeserializer::new(map))
            .map(ProtectedPaths::AtPhaseEnd)
    }
}

impl<'de> Deserialize<'de> for ProtectedPaths {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<ProtectedPaths, D::Error> {
        deserializer.deserialize_any(ProtectedPathsVisitor)
    }
}

impl ProtectedPaths {
    pub fn paths(&self) -> &[String] {
        match self {
            ProtectedPaths::Paths(paths) => paths,
            ProtectedPaths::AtPhaseEnd(protected) => &protected.paths,
        }
    }
}

/// `metrics:` (0.7; used from Milestone C, 0.13): how a pack's own data becomes spans. `import` reads the pack's
/// timing lines; `processes` lists the processes a workspace started that sampling cannot attribute.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Metrics {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub import: Option<HookEntry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub processes: Option<HookEntry>,
}

/// `experiment:` (0.7; used from Milestone D, 0.14): per-arm isolation for state that the clone shares. `env`,
/// `appendOnly` and `unchanged` hold `{{ }}` templates over `arm.dir` and `git.commonDir` (0.5), so they are text
/// here and the experiment stage reads them.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Experiment {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prepare: Option<HookEntry>,
    /// The prefix of an arm's branches (default `collide-exp/`).
    #[serde(
        rename = "branchPrefix",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub branch_prefix: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub env: BTreeMap<String, String>,
    /// Paths that other work appends to during an experiment.
    #[serde(rename = "appendOnly", default, skip_serializing_if = "Vec::is_empty")]
    pub append_only: Vec<String>,
    /// Paths that must be byte-identical at the end.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unchanged: Vec<String>,
}

/// A role (0.3, 0.7): an agent, its definition, and the options that sit beside its model and budget.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Role {
    /// The agent kind, one of `requires.agents` (`claude`).
    #[schemars(length(min = 1))]
    pub agent: String,
    /// A path to the agent definition: in the pack, or `repo:` for one in the line's snapshot.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub definition: Option<String>,
    /// A model alias, resolved per user.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget: Option<RunBudget>,
    #[serde(
        rename = "readOnly",
        default,
        skip_serializing_if = "std::ops::Not::not"
    )]
    pub read_only: bool,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub env: BTreeMap<String, String>,
    /// Context options (0.3): the same four a profile may override.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compact_at: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config: Option<RoleConfig>,
    /// `worktree` (the default when absent) or `none`: whether the repository's own instructions load (0.3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repo_instructions: Option<RepoInstructions>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bash_output_max: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mcp_output_max: Option<u64>,
    /// The capabilities the role needs (0.3): each a name of [`crate::agent::CAPABILITY_NAMES`], or a name with a
    /// sub-flag (`permission_host.deny_message`). A tool that lacks one is refused at launch.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub require: Vec<String>,
    /// Tools the role asks for among the ones Collide disallows by default (0.3, Disallowed tools), as plain names.
    /// Which names an adapter disallows is the adapter's.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allow_tools: Vec<String>,
}

/// One entry of `bindings:` (0.4): a handler attached to a named point. It names either a `gate` of this pack,
/// which may bind only to a `before_*` point, or a `script`, a chain handler that may bind to any point.
/// `match` narrows a core point (a status category, an op name or a target source); `when:` is CEL over `task`,
/// `change` and this pack's `config`. `timeout` is the hook's own (0.5: default 30 s for task points, 120 s for
/// move points).
///
/// The plan describes bindings (0.4, Named points) and writes one as `task.before_status: review`; it does not
/// give the manifest key, so this shape is a decision for the owner. A list, because chain handlers run in the
/// order written.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    /// A core point (`task.before_status`), a derived point of this pack (a key of `points:`), or another pack's
    /// derived point as `pack/point`.
    #[schemars(length(min = 1))]
    pub point: String,
    #[serde(rename = "match", default, skip_serializing_if = "Option::is_none")]
    pub matches: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gate: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub script: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout: Option<Duration>,
}

/// An observer (0.4): a long-lived process that gets the events its `events:` patterns name, as JSONL on stdin
/// (see [`crate::observer`]). A pattern is an event type (`run.transition`), a family (`run.*`) or `*`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ObserverEntry {
    #[schemars(length(min = 1))]
    pub script: String,
    pub events: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PackManifest {
    /// `collide/v0`.
    #[serde(rename = "apiVersion")]
    #[schemars(length(min = 1))]
    pub api_version: String,
    #[schemars(length(min = 1))]
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requires: Option<Requires>,
    /// Launch parameters, asked in the run preview; they reach prompts and hooks as `launch` (0.7).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub launch: BTreeMap<String, SchemaObject>,
    /// Settings, one JSON Schema per key, with `secret`, `overridable` and `repoOnly` (0.7, Settings).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub config: BTreeMap<String, SchemaObject>,
    /// Named workspace implementations, as `pack/name` in a workflow's `workspace:` (0.4).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub workspaces: BTreeMap<String, WorkspaceImpl>,
    /// Named `integrate` implementations (0.4).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub integrations: BTreeMap<String, HookEntry>,
    /// Named `sync` implementations (0.4). The plan names the kind and not the key: a decision for the owner.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub syncs: BTreeMap<String, HookEntry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub record: Option<Record>,
    #[serde(
        rename = "protectedPaths",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub protected_paths: Option<ProtectedPaths>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metrics: Option<Metrics>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub experiment: Option<Experiment>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub roles: BTreeMap<String, Role>,
    /// Prompt name to the path of its template, in the pack.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub prompts: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub gates: BTreeMap<String, HookEntry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bindings: Vec<Binding>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub observers: BTreeMap<String, ObserverEntry>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub providers: BTreeMap<String, ProviderEntry>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub points: BTreeMap<String, DerivedPoint>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub checklists: BTreeMap<String, ChecklistTemplate>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub workflows: BTreeMap<String, Workflow>,
    /// Named views a board may adopt by name (0.7, Keys added for planning). A key is the view's name, and
    /// [`crate::pack_check::check_manifest`] refuses a view whose own `name:` differs from its key.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub views: BTreeMap<String, View>,
    /// Named profiles, which cover only this pack's own settings and workflows (0.7, Profiles).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub profiles: BTreeMap<String, Profile>,
}
