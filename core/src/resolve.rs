//! Resolution of the names a workflow and a role use into the things a run will start (0.3, Who picks what; 0.4,
//! Implementations, chosen per workflow). Two kinds, both outlines:
//!
//! | what | function | filled by |
//! |---|---|---|
//! | a role's model alias, per user | [`resolve_model_alias`] | Stage 3b |
//! | a workflow's `workspace` and `integrate` | [`resolve_implementations`] | Stage 7 |
//!
//! Both are pure: they read parsed values and return a result or a refusal that names what failed.

use crate::agent::ModelRef;
use crate::pack::PackManifest;
use crate::workflow::Workflow;
use std::collections::BTreeMap;
use std::fmt;

/// A named implementation a run will use for one step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImplementationRef {
    /// The one Collide ships: for a workspace, `git worktree add` (0.4).
    Builtin,
    /// `pack/name`, as a pack's manifest declares it under `workspaces:`, `integrations:` or `syncs:`.
    Pack { pack: String, name: String },
}

/// The implementations a run resolved at launch, which the run records and the preview names (0.4, 0.8). A workflow
/// that names no workspace uses the built-in one. A workflow that names no `integrate` has none, so it cannot be
/// accepted, and review says why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedImplementations {
    pub workspace: ImplementationRef,
    pub integrate: Option<ImplementationRef>,
}

/// Why a name did not resolve.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolveError {
    /// The alias has no mapping for this tool in the user's `models.yaml`.
    UnmappedAlias { alias: String },
    /// The mapped `model.name` fails the pack's `requires.modelName` pattern.
    ModelNameFailsPattern {
        alias: String,
        name: String,
        pattern: String,
    },
    /// A workflow names an implementation that no loaded pack declares.
    UnknownImplementation { workflow: String, name: String },
}

impl fmt::Display for ResolveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ResolveError::UnmappedAlias { alias } => write!(
                f,
                "the model alias `{alias}` has no mapping in models.yaml for this tool"
            ),
            ResolveError::ModelNameFailsPattern {
                alias,
                name,
                pattern,
            } => write!(
                f,
                "the model `{name}` that alias `{alias}` maps to does not match the pack's `requires.modelName` pattern `{pattern}`"
            ),
            ResolveError::UnknownImplementation { workflow, name } => write!(
                f,
                "workflow `{workflow}` names the implementation `{name}`, which no loaded pack declares"
            ),
        }
    }
}

impl std::error::Error for ResolveError {}

/// Resolves a role's model alias (`fast`, `strong`) to the model of the tool's mapping (0.3, Who picks what). An alias
/// with no entry in `mapping` is refused with its name, and a mapped `name` that fails `required_pattern` (the pack's
/// `requires.modelName`, a regular expression) is refused with the pattern. The alias never switches the tool, and a
/// mapping's `id` passes through untouched.
pub fn resolve_model_alias(
    alias: &str,
    mapping: &BTreeMap<String, ModelRef>,
    required_pattern: Option<&str>,
) -> Result<ModelRef, ResolveError> {
    let _ = (alias, mapping, required_pattern);
    todo!("resolve: model alias")
}

/// Resolves the `workspace:` and `integrate:` of `workflow` (named `workflow_name` in messages) against the loaded packs (0.4). A name is `pack/name`
/// and must be declared by that pack; a workflow with no `workspace:` gets [`ImplementationRef::Builtin`]. An unknown
/// name is refused with the workflow's and the implementation's names.
pub fn resolve_implementations(
    workflow_name: &str,
    workflow: &Workflow,
    packs: &BTreeMap<String, PackManifest>,
) -> Result<ResolvedImplementations, ResolveError> {
    let _ = (workflow_name, workflow, packs);
    todo!("resolve: implementations")
}
