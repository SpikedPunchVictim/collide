//! The checks a pack load runs over a manifest (0.7, Pack manifest), as pure functions over [`PackManifest`].
//! [`check_manifest`] returns every refusal in one pass, so an author fixes a manifest once. Reading the file
//! (an unknown key at any depth, a missing required key, the YAML hazards) is [`crate::yaml`] with the closed types
//! of [`crate::pack`]; this module holds what a type cannot say:
//!
//! - the identity strings are not empty, and a name used in `pack/name` is one word;
//! - a path is relative to the pack directory, or `repo:<path>` for the line's snapshot ([`parse_pack_path`]);
//! - a gate bound to an `after_*` point is refused, naming the point (0.4: gates bind only to `before_*`);
//! - a view's key is its name, and a pack's profile covers only that pack (0.7, Profiles);
//! - every `workflows:` entry passes [`crate::workflow_check::validate`], and every view, profile and derived
//!   point passes its own check.
//!
//! What this does not do: run a hook, a gate or an observer, read a path on disk, resolve a name against another
//! pack or against this pack's roles, prompts and implementations, parse CEL, or layer settings. Those belong to
//! the load of a board (Stage 4), the workflow engine (Stage 8) and the settings stage.

use crate::duration::Duration;
use crate::pack::{Binding, HookEntry, PackManifest, SchemaObject};
use crate::points::{Point, PointError, check_derived_points, side};
use crate::profile::{MIN_COMPACT_AT, ProfileError, check_profile};
use crate::view::{MAX_CEL_SOURCE_BYTES, ViewError, check_view};
use crate::workflow::{Phase, Prompt, Then, Workflow};
use crate::workflow_check::{self, WorkflowError};
use std::fmt;

/// Why a path in a manifest is refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathProblem {
    Empty,
    /// It starts with `/` or `\`: a path is relative to the pack directory, or to the line's snapshot.
    Absolute,
    /// It has a `..` segment.
    Climbs,
    /// Its first segment holds a `:` and is not `repo:`, so it reads as another scheme (`dev:`, `C:`).
    Scheme {
        scheme: String,
    },
}

impl fmt::Display for PathProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PathProblem::Empty => write!(f, "is empty"),
            PathProblem::Absolute => write!(
                f,
                "is absolute; write it relative to the pack directory, or `repo:<path>` for the line's snapshot"
            ),
            PathProblem::Climbs => write!(f, "climbs out with `..`"),
            PathProblem::Scheme { scheme } => write!(
                f,
                "starts with `{scheme}:`; only `repo:` marks a path in the line's snapshot"
            ),
        }
    }
}

/// A path in `pack.yaml` (0.7, Paths in `pack.yaml`): where it is read from, and the relative path in it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackPath<'a> {
    /// Relative to the pack directory.
    InPack(&'a str),
    /// `repo:<path>`: a path in the line's snapshot, which later stages resolve.
    InRepo(&'a str),
}

/// A relative path or glob: not empty, not absolute, no `..` segment, no other scheme. Used for `repo:` paths
/// after the marker, and for the globs of `record.paths` and `protectedPaths`.
fn check_relative(text: &str) -> Result<(), PathProblem> {
    if text.trim().is_empty() {
        return Err(PathProblem::Empty);
    }
    if text.starts_with('/') || text.starts_with('\\') {
        return Err(PathProblem::Absolute);
    }
    if text.split(['/', '\\']).any(|segment| segment == "..") {
        return Err(PathProblem::Climbs);
    }
    let first = text.split(['/', '\\']).next().unwrap_or(text);
    if first.contains(':') {
        return Err(PathProblem::Scheme {
            scheme: first.split(':').next().unwrap_or(first).to_string(),
        });
    }
    Ok(())
}

/// Reads a `script`, `wait`, hook, prompt or definition path: `hooks/x.ts` is in the pack and `repo:.claude/x.md`
/// is in the line's snapshot. Whether the file exists is not checked here.
pub fn parse_pack_path(text: &str) -> Result<PackPath<'_>, PathProblem> {
    match text.strip_prefix("repo:") {
        Some(rest) => check_relative(rest).map(|()| PackPath::InRepo(rest)),
        None => check_relative(text).map(|()| PackPath::InPack(text)),
    }
}

/// Why a binding is refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BindingProblem {
    /// A point that is not a core point, a derived point of this pack, or `pack/point`.
    UnknownPoint {
        point: String,
    },
    NoTarget,
    BothTargets,
    /// A gate on an `after_*` point: the action has already happened (0.4).
    GateOnAfterPoint {
        point: String,
        gate: String,
    },
    /// `match:` with no entry, or an empty one.
    EmptyMatch,
}

impl fmt::Display for BindingProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BindingProblem::UnknownPoint { point } => write!(
                f,
                "`{point}` is not a core point, a derived point of this pack, or another pack's `pack/point`"
            ),
            BindingProblem::NoTarget => write!(f, "names neither a `gate:` nor a `script:`"),
            BindingProblem::BothTargets => write!(f, "names both a `gate:` and a `script:`"),
            BindingProblem::GateOnAfterPoint { point, gate } => write!(
                f,
                "gate `{gate}` is bound to `{point}`, an `after_*` point; a gate binds only to a `before_*` point, \
                 because the action has already happened"
            ),
            BindingProblem::EmptyMatch => write!(f, "`match:` has no entry, or an empty one"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ManifestError {
    /// A required string, or an entry of a list that may not hold an empty one. `at` says where it was written.
    Empty {
        at: String,
    },
    /// A key that names a thing of this pack and is not one word (it is empty, has white space, or has a `/`).
    BadName {
        kind: &'static str,
        name: String,
    },
    BadPath {
        at: String,
        path: String,
        problem: PathProblem,
    },
    ZeroTimeout {
        at: String,
    },
    /// A `config:`, `launch:` or `outputs:` entry that is not a mapping.
    NotASchema {
        at: String,
    },
    /// A setting's `secret` or `overridable` that is not a boolean.
    NotABoolean {
        at: String,
        flag: &'static str,
    },
    CompactAtBelowFloor {
        role: String,
        value: u64,
    },
    Binding {
        index: usize,
        problem: BindingProblem,
    },
    /// A `when:` that is empty, or over the 1 KB CEL source cap (0.9).
    BadWhen {
        at: String,
        bytes: usize,
    },
    Point(PointError),
    Workflow {
        name: String,
        error: WorkflowError,
    },
    View {
        key: String,
        error: ViewError,
    },
    /// A view whose key and `name:` differ.
    ViewNameMismatch {
        key: String,
        name: String,
    },
    Profile {
        key: String,
        error: ProfileError,
    },
    /// A pack's profile that reaches past the pack: `packs:`, or a workflow or settings of another pack.
    ProfileScope {
        profile: String,
        key: String,
    },
}

impl fmt::Display for ManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ManifestError::Empty { at } => write!(f, "`{at}` is empty"),
            ManifestError::BadName { kind, name } => write!(
                f,
                "{kind} `{name}` is not a usable name: it must be one word, with no `/` and no white space"
            ),
            ManifestError::BadPath { at, path, problem } => {
                write!(f, "`{at}`: the path `{path}` {problem}")
            }
            ManifestError::ZeroTimeout { at } => {
                write!(f, "`{at}`: `timeout:` is zero, so the hook ends at once")
            }
            ManifestError::NotASchema { at } => {
                write!(f, "`{at}` is not a mapping; write a JSON Schema")
            }
            ManifestError::NotABoolean { at, flag } => {
                write!(f, "`{at}`: `{flag}` must be true or false")
            }
            ManifestError::CompactAtBelowFloor { role, value } => write!(
                f,
                "role `{role}`: `compact_at: {value}` is under the floor of {MIN_COMPACT_AT}"
            ),
            ManifestError::Binding { index, problem } => {
                write!(f, "`bindings[{index}]` {problem}")
            }
            ManifestError::BadWhen { at, bytes } => {
                if *bytes == 0 {
                    write!(f, "`{at}`: `when:` is empty")
                } else {
                    write!(
                        f,
                        "`{at}`: `when:` is {bytes} bytes; a CEL source is capped at {MAX_CEL_SOURCE_BYTES}"
                    )
                }
            }
            ManifestError::Point(error) => write!(f, "`points`: {error}"),
            ManifestError::Workflow { name, error } => write!(f, "workflow `{name}`: {error}"),
            ManifestError::View { key, error } => write!(f, "view `{key}`: {error}"),
            ManifestError::ViewNameMismatch { key, name } => write!(
                f,
                "view `{key}` has `name: {name}`; the key of a pack's view is its name, so write `{key}` in both"
            ),
            ManifestError::Profile { key, error } => write!(f, "profile `{key}`: {error}"),
            ManifestError::ProfileScope { profile, key } => write!(
                f,
                "profile `{profile}`: `{key}` reaches past this pack; a pack's profile covers only its own \
                 settings and workflows"
            ),
        }
    }
}

impl std::error::Error for ManifestError {}

struct Check {
    errors: Vec<ManifestError>,
}

impl Check {
    fn empty(&mut self, at: &str, text: &str) {
        if text.trim().is_empty() {
            self.errors
                .push(ManifestError::Empty { at: at.to_string() });
        }
    }

    fn name(&mut self, kind: &'static str, name: &str) {
        let one_word = !name.trim().is_empty()
            && !name.contains('/')
            && !name.chars().any(char::is_whitespace);
        if !one_word {
            self.errors.push(ManifestError::BadName {
                kind,
                name: name.to_string(),
            });
        }
    }

    fn path(&mut self, at: &str, text: &str) {
        if let Err(problem) = parse_pack_path(text) {
            self.errors.push(ManifestError::BadPath {
                at: at.to_string(),
                path: text.to_string(),
                problem,
            });
        }
    }

    /// A glob over repo paths (`record.paths`, `protectedPaths`).
    fn glob(&mut self, at: &str, text: &str) {
        if let Err(problem) = check_relative(text) {
            self.errors.push(ManifestError::BadPath {
                at: at.to_string(),
                path: text.to_string(),
                problem,
            });
        }
    }

    fn timeout(&mut self, at: &str, timeout: Option<Duration>) {
        if timeout.is_some_and(|timeout| timeout.as_secs() == 0) {
            self.errors
                .push(ManifestError::ZeroTimeout { at: at.to_string() });
        }
    }

    fn schema(&mut self, at: &str, schema: &SchemaObject) {
        if !schema.0.is_object() {
            self.errors
                .push(ManifestError::NotASchema { at: at.to_string() });
        }
    }

    fn when(&mut self, at: &str, when: &Option<String>) {
        if let Some(source) = when
            && (source.trim().is_empty() || source.len() > MAX_CEL_SOURCE_BYTES)
        {
            self.errors.push(ManifestError::BadWhen {
                at: at.to_string(),
                bytes: if source.trim().is_empty() {
                    0
                } else {
                    source.len()
                },
            });
        }
    }

    fn hook(&mut self, at: &str, hook: &HookEntry) {
        self.path(&format!("{at}.script"), &hook.script);
        self.timeout(&format!("{at}.timeout"), hook.timeout);
        for (key, schema) in &hook.outputs {
            self.schema(&format!("{at}.outputs.{key}"), schema);
        }
    }

    fn setting(&mut self, key: &str, schema: &SchemaObject) {
        let at = format!("config.{key}");
        self.schema(&at, schema);
        for flag in ["secret", "overridable"] {
            if schema.0.get(flag).is_some_and(|value| !value.is_boolean()) {
                self.errors.push(ManifestError::NotABoolean {
                    at: at.clone(),
                    flag,
                });
            }
        }
    }

    fn binding(&mut self, index: usize, binding: &Binding, manifest: &PackManifest) {
        let mut problems = Vec::new();
        let point_side = side(&binding.point);
        let own = manifest.points.contains_key(&binding.point);
        let core = Point::parse(&binding.point).is_ok();
        let other_pack = binding.point.contains('/') && point_side.is_some();
        if !(own || core || other_pack) {
            problems.push(BindingProblem::UnknownPoint {
                point: binding.point.clone(),
            });
        }
        match (&binding.gate, &binding.script) {
            (None, None) => problems.push(BindingProblem::NoTarget),
            (Some(_), Some(_)) => problems.push(BindingProblem::BothTargets),
            (Some(gate), None) if point_side == Some("after") => {
                problems.push(BindingProblem::GateOnAfterPoint {
                    point: binding.point.clone(),
                    gate: gate.clone(),
                });
            }
            _ => {}
        }
        if binding
            .matches
            .as_ref()
            .is_some_and(|names| names.is_empty() || names.iter().any(|n| n.trim().is_empty()))
        {
            problems.push(BindingProblem::EmptyMatch);
        }
        self.errors.extend(
            problems
                .into_iter()
                .map(|problem| ManifestError::Binding { index, problem }),
        );
        let at = format!("bindings[{index}]");
        self.empty(&format!("{at}.point"), &binding.point);
        if let Some(gate) = &binding.gate {
            self.empty(&format!("{at}.gate"), gate);
        }
        if let Some(script) = &binding.script {
            self.path(&format!("{at}.script"), script);
        }
        self.when(&format!("{at}.when"), &binding.when);
        self.timeout(&format!("{at}.timeout"), binding.timeout);
    }

    /// The strings of one workflow that a type cannot keep empty, and its script paths.
    fn workflow_fields(&mut self, name: &str, workflow: &Workflow) {
        let at = format!("workflows.{name}");
        for (list, ops) in [
            ("on_start", &workflow.on_start),
            ("on_end", &workflow.on_end),
        ] {
            for (index, op) in ops.iter().enumerate() {
                self.empty(&format!("{at}.{list}[{index}].op"), &op.op);
            }
        }
        for (phase_name, phase) in workflow.phases.iter() {
            let at = format!("{at}.phases.{phase_name}");
            match phase {
                Phase::Agent(agent) => {
                    self.empty(&format!("{at}.role"), &agent.role);
                    match &agent.prompt {
                        Prompt::Name(prompt) => self.empty(&format!("{at}.prompt"), prompt),
                        Prompt::Artifact(artifact) => {
                            self.empty(&format!("{at}.prompt.artifact"), &artifact.artifact);
                        }
                    }
                }
                Phase::Script(script) => self.path(&format!("{at}.script"), &script.script),
                Phase::Wait(wait) => self.path(&format!("{at}.wait"), &wait.wait),
                Phase::Human(human) => self.empty(&format!("{at}.human"), &human.human),
            }
            if let Some(Then::Decision(decision)) = phase.then() {
                self.path(&format!("{at}.then.script"), &decision.script);
            }
        }
    }
}

/// Whether `reference` is `<pack>/<name>` for this pack. A pack's own name may hold a slash (`collide/plan`).
fn own_reference(pack: &str, reference: &str) -> bool {
    reference
        .strip_prefix(pack)
        .and_then(|rest| rest.strip_prefix('/'))
        .is_some_and(|local| !local.is_empty() && !local.contains('/'))
}

/// Checks a manifest (0.7): identity, names, paths, hooks, bindings, observers, derived points, checklists, and
/// every workflow, view and profile it holds. Returns every refusal, in the order of the manifest's keys.
pub fn check_manifest(manifest: &PackManifest) -> Result<(), Vec<ManifestError>> {
    let mut check = Check { errors: Vec::new() };

    check.empty("apiVersion", &manifest.api_version);
    check.empty("name", &manifest.name);
    if manifest.name.chars().any(char::is_whitespace) {
        check.errors.push(ManifestError::BadName {
            kind: "pack",
            name: manifest.name.clone(),
        });
    }
    if let Some(version) = &manifest.version {
        check.empty("version", version);
    }
    if let Some(requires) = &manifest.requires {
        for (key, value) in [("sdk", &requires.sdk), ("modelName", &requires.model_name)] {
            if let Some(value) = value {
                check.empty(&format!("requires.{key}"), value);
            }
        }
        for (index, tool) in requires.tools.iter().enumerate() {
            check.empty(&format!("requires.tools[{index}].name"), &tool.name);
            if let Some(version) = &tool.version {
                check.empty(&format!("requires.tools[{index}].version"), version);
            }
        }
        for (index, agent) in requires.agents.iter().enumerate() {
            check.empty(&format!("requires.agents[{index}]"), agent);
        }
    }

    for key in manifest.launch.keys() {
        check.name("launch parameter", key);
    }
    for (key, schema) in &manifest.launch {
        check.schema(&format!("launch.{key}"), schema);
    }
    for key in manifest.config.keys() {
        check.name("setting", key);
    }
    for (key, schema) in &manifest.config {
        check.setting(key, schema);
    }

    for (name, workspace) in &manifest.workspaces {
        check.name("workspace implementation", name);
        let at = format!("workspaces.{name}");
        check.hook(&format!("{at}.create"), &workspace.create);
        for (hook_name, hook) in [
            ("sandbox", &workspace.sandbox),
            ("line", &workspace.line),
            ("resume", &workspace.resume),
            ("doctor", &workspace.doctor),
            ("remove", &workspace.remove),
            ("discard", &workspace.discard),
            ("cleanup", &workspace.cleanup),
        ] {
            if let Some(hook) = hook {
                check.hook(&format!("{at}.{hook_name}"), hook);
            }
        }
    }
    for (name, hook) in &manifest.integrations {
        check.name("integrate implementation", name);
        check.hook(&format!("integrations.{name}"), hook);
    }
    for (name, hook) in &manifest.syncs {
        check.name("sync implementation", name);
        check.hook(&format!("syncs.{name}"), hook);
    }
    if let Some(record) = &manifest.record {
        check.path("record.script", &record.script);
        for (index, glob) in record.paths.iter().enumerate() {
            check.glob(&format!("record.paths[{index}]"), glob);
        }
    }
    if let Some(protected) = &manifest.protected_paths {
        for (index, glob) in protected.paths().iter().enumerate() {
            check.glob(&format!("protectedPaths[{index}]"), glob);
        }
    }
    if let Some(metrics) = &manifest.metrics {
        for (name, hook) in [
            ("import", &metrics.import),
            ("processes", &metrics.processes),
        ] {
            if let Some(hook) = hook {
                check.hook(&format!("metrics.{name}"), hook);
            }
        }
    }
    if let Some(prepare) = manifest
        .experiment
        .as_ref()
        .and_then(|e| e.prepare.as_ref())
    {
        check.hook("experiment.prepare", prepare);
    }

    for (name, role) in &manifest.roles {
        check.name("role", name);
        check.empty(&format!("roles.{name}.agent"), &role.agent);
        if let Some(definition) = &role.definition {
            check.path(&format!("roles.{name}.definition"), definition);
        }
        if let Some(value) = role.compact_at
            && value < MIN_COMPACT_AT
        {
            check.errors.push(ManifestError::CompactAtBelowFloor {
                role: name.clone(),
                value,
            });
        }
    }
    for (name, path) in &manifest.prompts {
        check.name("prompt", name);
        check.path(&format!("prompts.{name}"), path);
    }
    for (name, gate) in &manifest.gates {
        check.name("gate", name);
        check.hook(&format!("gates.{name}"), gate);
    }
    for (index, binding) in manifest.bindings.iter().enumerate() {
        check.binding(index, binding, manifest);
    }
    for (name, observer) in &manifest.observers {
        check.name("observer", name);
        check.path(&format!("observers.{name}.script"), &observer.script);
        if observer.events.is_empty() {
            check.errors.push(ManifestError::Empty {
                at: format!("observers.{name}.events"),
            });
        }
        for (index, pattern) in observer.events.iter().enumerate() {
            let at = format!("observers.{name}.events[{index}]");
            if pattern.chars().any(char::is_whitespace) {
                check.errors.push(ManifestError::BadName {
                    kind: "event pattern",
                    name: pattern.clone(),
                });
            }
            check.empty(&at, pattern);
        }
    }
    for (name, provider) in &manifest.providers {
        check.name("provider", name);
        check.path(&format!("providers.{name}.script"), &provider.script);
    }

    if let Err(errors) = check_derived_points(&manifest.points, &Point::ALL) {
        check
            .errors
            .extend(errors.into_iter().map(ManifestError::Point));
    }
    for (name, point) in &manifest.points {
        check.when(&format!("points.{name}.when"), &point.when);
    }
    for (name, template) in &manifest.checklists {
        check.name("checklist", name);
        for (index, item) in template.items.iter().enumerate() {
            let at = format!("checklists.{name}.items[{index}]");
            check.empty(&format!("{at}.id"), &item.id);
            check.empty(&format!("{at}.text"), &item.text);
            check.when(&format!("{at}.when"), &item.when);
        }
    }

    for (name, workflow) in &manifest.workflows {
        check.name("workflow", name);
        if let Err(errors) = workflow_check::validate(workflow) {
            check
                .errors
                .extend(errors.into_iter().map(|error| ManifestError::Workflow {
                    name: name.clone(),
                    error,
                }));
        }
        check.workflow_fields(name, workflow);
    }

    for (key, view) in &manifest.views {
        check.name("view", key);
        if key != &view.name {
            check.errors.push(ManifestError::ViewNameMismatch {
                key: key.clone(),
                name: view.name.clone(),
            });
        }
        if let Err(errors) = check_view(view) {
            check
                .errors
                .extend(errors.into_iter().map(|error| ManifestError::View {
                    key: key.clone(),
                    error,
                }));
        }
    }

    for (key, profile) in &manifest.profiles {
        check.name("profile", key);
        if let Err(errors) = check_profile(profile) {
            check
                .errors
                .extend(errors.into_iter().map(|error| ManifestError::Profile {
                    key: key.clone(),
                    error,
                }));
        }
        let mut reaches = |what: String| {
            check.errors.push(ManifestError::ProfileScope {
                profile: key.clone(),
                key: what,
            });
        };
        if !profile.packs.on.is_empty() || !profile.packs.off.is_empty() {
            reaches("packs".to_string());
        }
        if let Some(workflow) = &profile.workflow
            && !own_reference(&manifest.name, workflow)
        {
            reaches(format!("workflow: {workflow}"));
        }
        for pack in profile.config.keys().filter(|pack| **pack != manifest.name) {
            reaches(format!("config.{pack}"));
        }
    }

    if check.errors.is_empty() {
        Ok(())
    } else {
        Err(check.errors)
    }
}
