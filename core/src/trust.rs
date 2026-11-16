//! Trust, the parts that need no disk and no git (ts-0097-kw; 0.9, Trust).
//!
//! What a person has trusted is kept outside the repo, in `~/.config/collide/trust.yaml`. This module is that file's
//! types ([`TrustFile`]), the list of named git ids a line's trust hash covers ([`TrustInput`]) and its text
//! ([`canonical_text`]), the difference of two such lists ([`diff_inputs`]) and the shapes the prompt shows
//! ([`InputDiff`], [`PackReview`]). Reading the repo, hashing and writing the file are in `host`.

use crate::pack::{HookEntry, PackManifest};
use crate::workflow::{Phase, Then};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// One thing the trust hash covers: a name (`board.yaml`, `pack repo:.collide/packs/ferry`, `protected src/gen`) and
/// the git id of what it was at the line's commit.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrustInput {
    pub name: String,
    pub id: String,
}

impl TrustInput {
    pub fn new(name: impl Into<String>, id: impl Into<String>) -> TrustInput {
        TrustInput {
            name: name.into(),
            id: id.into(),
        }
    }
}

/// Why a list of inputs has no text to hash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputError {
    /// A name or an id that is empty or has a NUL or a newline, which the text uses to tell its parts apart.
    BadPart { name: String },
    /// Two inputs with one name and different ids.
    DuplicateName { name: String },
}

impl fmt::Display for InputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InputError::BadPart { name } => write!(
                f,
                "the trust input {name:?} has an empty part, or a NUL or a newline, so it cannot be hashed"
            ),
            InputError::DuplicateName { name } => {
                write!(
                    f,
                    "the trust input {name:?} is listed twice with different ids"
                )
            }
        }
    }
}

impl std::error::Error for InputError {}

/// The text the trust hash is of: each input as `name NUL id LF`, sorted by name. The same inputs in any order give
/// the same text. An input listed twice with the same id is written once.
pub fn canonical_text(inputs: &[TrustInput]) -> Result<String, InputError> {
    let mut sorted: Vec<&TrustInput> = inputs.iter().collect();
    sorted.sort();
    sorted.dedup();
    let mut text = String::new();
    for (index, input) in sorted.iter().enumerate() {
        let bad = |part: &str| part.is_empty() || part.contains(['\0', '\n']);
        if bad(&input.name) || bad(&input.id) {
            return Err(InputError::BadPart {
                name: input.name.clone(),
            });
        }
        if index > 0 && sorted[index - 1].name == input.name {
            return Err(InputError::DuplicateName {
                name: input.name.clone(),
            });
        }
        text.push_str(&input.name);
        text.push('\0');
        text.push_str(&input.id);
        text.push('\n');
    }
    Ok(text)
}

/// How one input differs between two lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeKind {
    Added,
    Removed,
    Changed,
}

/// One input that was added, removed or changed. `old` is the id before (not for an added input) and `new` the id
/// after (not for a removed one).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputChange {
    pub name: String,
    pub kind: ChangeKind,
    pub old: Option<String>,
    pub new: Option<String>,
}

/// The inputs that differ between `old` and `new`, in name order. An input with the same id in both is not listed.
pub fn diff_inputs(old: &[TrustInput], new: &[TrustInput]) -> Vec<InputChange> {
    let ids = |inputs: &[TrustInput]| -> BTreeMap<String, String> {
        inputs
            .iter()
            .map(|input| (input.name.clone(), input.id.clone()))
            .collect()
    };
    let (old, new) = (ids(old), ids(new));
    let names: BTreeSet<&String> = old.keys().chain(new.keys()).collect();
    let mut changes = Vec::new();
    for name in names {
        let (before, after) = (old.get(name), new.get(name));
        let kind = match (before, after) {
            (Some(before), Some(after)) if before == after => continue,
            (Some(_), Some(_)) => ChangeKind::Changed,
            (None, _) => ChangeKind::Added,
            (_, None) => ChangeKind::Removed,
        };
        changes.push(InputChange {
            name: name.clone(),
            kind,
            old: before.cloned(),
            new: after.cloned(),
        });
    }
    changes
}

/// What is known of one line the person trusted: the hash, and the inputs it was made from, because a diff needs the
/// old ids.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LineTrust {
    pub hash: String,
    pub inputs: Vec<TrustInput>,
}

/// A user pack the person trusted when it was added: the tree id they saw. Its source is the key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackTrust {
    pub tree: String,
}

/// `~/.config/collide/trust.yaml`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrustFile {
    /// By the board's root path, then by the line's ref.
    #[serde(default)]
    pub boards: BTreeMap<String, BTreeMap<String, LineTrust>>,
    /// User packs, by source.
    #[serde(default)]
    pub packs: BTreeMap<String, PackTrust>,
    /// The paths of the `dev:` folders the person trusted, sorted.
    #[serde(default)]
    pub dev: Vec<String>,
}

impl TrustFile {
    pub fn line(&self, root: &str, ref_name: &str) -> Option<&LineTrust> {
        self.boards.get(root).and_then(|lines| lines.get(ref_name))
    }

    pub fn set_line(&mut self, root: &str, ref_name: &str, trust: LineTrust) {
        self.boards
            .entry(root.to_string())
            .or_default()
            .insert(ref_name.to_string(), trust);
    }

    pub fn set_pack(&mut self, source: &str, tree: &str) {
        self.packs.insert(
            source.to_string(),
            PackTrust {
                tree: tree.to_string(),
            },
        );
    }

    pub fn add_dev(&mut self, folder: &str) {
        if let Err(at) = self.dev.binary_search_by(|have| have.as_str().cmp(folder)) {
            self.dev.insert(at, folder.to_string());
        }
    }
}

/// Where a protected glob is hashed: its folder up to the first wildcard (`src/gen/**/*.rs` is `src/gen`), or the
/// path itself when there is no wildcard. An empty string is the repo's root, for a glob that starts with a wildcard.
/// A `.` part and an empty part are dropped.
pub fn protected_root(glob: &str) -> String {
    let literal = match glob.find(['*', '?', '[', '{', '\\']) {
        None => glob,
        // The part of the path that holds the wildcard is not a folder yet: keep the folders before it.
        Some(at) => glob[..at].rfind('/').map_or("", |slash| &glob[..slash]),
    };
    let parts: Vec<&str> = literal
        .split('/')
        .filter(|part| !part.is_empty() && *part != ".")
        .collect();
    parts.join("/")
}

/// How a file of a pack tree differs between two trees.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileChangeKind {
    Added,
    Deleted,
    Modified,
    TypeChanged,
}

/// The changed lines of a file's diff: `@@` lines, `-` lines and `+` lines, none of the context.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LineDiff {
    pub lines: Vec<String>,
    /// How many changed lines came after the cap and are not in `lines`.
    pub omitted: usize,
    pub binary: bool,
}

/// A file of a changed pack tree. `lines` is its diff when it is a text file whose diff was read, and `note` says why
/// it is not otherwise.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileDiff {
    pub kind: FileChangeKind,
    pub path: String,
    pub lines: Option<LineDiff>,
    pub note: Option<String>,
}

/// What a changed input shows beyond its name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffDetail {
    None,
    /// A changed folder: the files that differ.
    Files(Vec<FileDiff>),
    /// A changed or added file: its changed lines.
    Lines(LineDiff),
    /// The old objects are not in the repository (a pruned clone), so there is nothing more to show. Says so.
    Unavailable(String),
}

/// One line of the diff the prompt shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputDiff {
    pub change: InputChange,
    pub detail: DiffDetail,
}

/// One script or provider a pack runs, with the manifest key that names it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewEntry {
    pub key: String,
    pub script: String,
}

/// What the prompt shows of a pack a person is asked to trust (0.9, Per pack source): the hooks, providers and
/// observers it runs, and what it requires.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackReview {
    pub name: String,
    pub version: Option<String>,
    pub hooks: Vec<ReviewEntry>,
    pub providers: Vec<ReviewEntry>,
    /// An observer's key holds the events it gets.
    pub observers: Vec<ReviewEntry>,
    pub requires: Vec<String>,
}

pub fn pack_review(manifest: &PackManifest) -> PackReview {
    PackReview {
        name: manifest.name.clone(),
        version: manifest.version.clone(),
        hooks: hooks_of(manifest),
        providers: manifest
            .providers
            .iter()
            .map(|(name, provider)| entry(format!("providers.{name}"), &provider.script))
            .collect(),
        observers: manifest
            .observers
            .iter()
            .map(|(name, observer)| {
                let events = observer.events.join(", ");
                entry(format!("observers.{name} ({events})"), &observer.script)
            })
            .collect(),
        requires: requires_of(manifest),
    }
}

fn entry(key: String, script: &str) -> ReviewEntry {
    ReviewEntry {
        key,
        script: script.to_string(),
    }
}

/// Every script a pack runs besides its providers and observers: the hooks of the manifest's tables and bindings, and
/// the scripts of its workflows' phases and decisions.
fn hooks_of(manifest: &PackManifest) -> Vec<ReviewEntry> {
    let mut hooks = Vec::new();
    let mut hook = |key: String, entry_hook: &HookEntry| hooks.push(entry(key, &entry_hook.script));
    for (name, workspace) in &manifest.workspaces {
        let steps = [
            ("create", Some(&workspace.create)),
            ("sandbox", workspace.sandbox.as_ref()),
            ("line", workspace.line.as_ref()),
            ("resume", workspace.resume.as_ref()),
            ("doctor", workspace.doctor.as_ref()),
            ("remove", workspace.remove.as_ref()),
            ("discard", workspace.discard.as_ref()),
            ("cleanup", workspace.cleanup.as_ref()),
        ];
        for (step, found) in steps {
            if let Some(found) = found {
                hook(format!("workspaces.{name}.{step}"), found);
            }
        }
    }
    for (name, found) in &manifest.integrations {
        hook(format!("integrations.{name}"), found);
    }
    for (name, found) in &manifest.syncs {
        hook(format!("syncs.{name}"), found);
    }
    for (name, found) in &manifest.gates {
        hook(format!("gates.{name}"), found);
    }
    if let Some(metrics) = &manifest.metrics {
        for (step, found) in [
            ("import", &metrics.import),
            ("processes", &metrics.processes),
        ] {
            if let Some(found) = found {
                hook(format!("metrics.{step}"), found);
            }
        }
    }
    if let Some(prepare) = manifest
        .experiment
        .as_ref()
        .and_then(|experiment| experiment.prepare.as_ref())
    {
        hook("experiment.prepare".to_string(), prepare);
    }
    if let Some(record) = &manifest.record {
        hooks.push(entry("record".to_string(), &record.script));
    }
    for (index, binding) in manifest.bindings.iter().enumerate() {
        if let Some(script) = &binding.script {
            hooks.push(entry(format!("bindings[{index}]"), script));
        }
    }
    for (name, workflow) in &manifest.workflows {
        for (phase_name, phase) in workflow.phases.iter() {
            let at = format!("workflows.{name}.phases.{phase_name}");
            let then = match phase {
                Phase::Script(script) => {
                    hooks.push(entry(at.clone(), &script.script));
                    script.then.as_ref()
                }
                Phase::Wait(wait) => {
                    hooks.push(entry(at.clone(), &wait.wait));
                    wait.then.as_ref()
                }
                Phase::Agent(agent) => agent.then.as_ref(),
                Phase::Human(_) => None,
            };
            if let Some(Then::Decision(decision)) = then {
                hooks.push(entry(format!("{at}.then"), &decision.script));
            }
        }
    }
    hooks
}

fn requires_of(manifest: &PackManifest) -> Vec<String> {
    let Some(requires) = &manifest.requires else {
        return Vec::new();
    };
    let mut lines = Vec::new();
    if let Some(sdk) = &requires.sdk {
        lines.push(format!("sdk {sdk}"));
    }
    for tool in &requires.tools {
        let version = tool
            .version
            .as_ref()
            .map(|version| format!(" {version}"))
            .unwrap_or_default();
        let hint = tool
            .hint
            .as_ref()
            .map(|hint| format!(" ({hint})"))
            .unwrap_or_default();
        lines.push(format!("tool {}{version}{hint}", tool.name));
    }
    for agent in &requires.agents {
        lines.push(format!("agent {agent}"));
    }
    if let Some(pattern) = &requires.model_name {
        lines.push(format!("model name matching {pattern}"));
    }
    lines
}

impl PackReview {
    /// The prompt's text for a pack: its source and commit, then what it runs and requires.
    pub fn render(&self, source: &str, commit: Option<&str>) -> String {
        let version = self
            .version
            .as_ref()
            .map(|version| format!(" {version}"))
            .unwrap_or_default();
        let mut text = format!("Pack {}{version}\nSource: {source}\n", self.name);
        if let Some(commit) = commit {
            text.push_str(&format!("Commit: {commit}\n"));
        }
        let list = |text: &mut String, title: &str, lines: Vec<String>| {
            if lines.is_empty() {
                text.push_str(&format!("{title}: none\n"));
            } else {
                text.push_str(&format!("{title}:\n"));
                for line in lines {
                    text.push_str(&format!("  {line}\n"));
                }
            }
        };
        let scripts = |entries: &[ReviewEntry]| -> Vec<String> {
            entries
                .iter()
                .map(|entry| format!("{}: {}", entry.key, entry.script))
                .collect()
        };
        list(&mut text, "Hooks it runs", scripts(&self.hooks));
        list(&mut text, "Providers it runs", scripts(&self.providers));
        list(&mut text, "Observers it runs", scripts(&self.observers));
        list(&mut text, "Requires", self.requires.clone());
        text
    }
}
