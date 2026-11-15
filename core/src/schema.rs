//! The contract schemas. Every contract type is written once, in Rust, with serde and `schemars` derives. This
//! module turns the types into JSON Schema, and `pnpm gen` writes the schemas to `schemas/` and then makes the
//! TypeScript in `packages/pack-sdk` from them (0.6).
//!
//! Rules for a contract type, so that the schema rejects what serde rejects:
//! - derive `Serialize`, `Deserialize` and `JsonSchema`;
//! - put `#[serde(deny_unknown_fields)]` on every struct and on every tagged enum, so each one gets
//!   `additionalProperties: false`;
//! - write a tagged variant with no fields as `Wait {}`, never `Wait`: serde does not enforce
//!   `deny_unknown_fields` on a unit variant of an internally tagged enum;
//! - to add a type, add one `registry.add::<T>()` line to [`registry`].

use crate::agent::{
    AgentEvent, Capabilities, PermissionReply, PermissionRequest, SessionStart, Usage,
};
use crate::board::{Board, CollidePointer};
use crate::change::{Change, Rename, TaskPointPayload};
use crate::context::ContextRecord;
use crate::events::Event;
use crate::hook::{HookInput, HookResult};
use crate::item::{Feature, FeatureProblem, KindDecl, LinkTypeDecl, PlanningFields};
use crate::lock::{PackLock, UserPacks};
use crate::observer::{ObserverAck, ObserverEvent};
use crate::pack::{PackManifest, RunBudget};
use crate::points::Point;
use crate::profile::Profile;
use crate::provider::{
    ApplyParams, ApplyResult, GetParams, GetResult, InitializeParams, InitializeResult, ListParams,
    ListResult, PreviewResult, SourceStale, Task, TaskChanged, TaskCollision, TaskInvalid,
    TaskRemoved, WatchParams, WatchResult,
};
use crate::report::ReportLine;
use crate::section::{NearDuplicateGroup, SectionUse, Sections};
use crate::settings::ConfigFile;
use crate::span::Span;
use crate::stage::Stage;
use crate::state::{PhaseState, RunState, WorkspaceState};
use crate::view::View;
use crate::workflow::{Adoption, Workflow};
use schemars::JsonSchema;
use schemars::generate::SchemaSettings;
use serde_json::Value;
use std::collections::BTreeMap;

/// JSON Schema files, by file name. Each type added gets two: `<Name>.input.json` for what a user writes
/// (`Deserialize`, so defaulted fields are optional) and `<Name>.output.json` for what Collide emits
/// (`Serialize`, so every field is present).
#[derive(Debug, Default)]
pub struct Registry {
    files: BTreeMap<String, Value>,
}

impl Registry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds the input and output schema of `T`, named after `T`. Panics if `T`'s name is already there.
    pub fn add<T: JsonSchema>(&mut self) {
        let name = T::schema_name();
        self.insert(
            format!("{name}.input.json"),
            schema_for::<T>(SchemaSettings::draft2020_12().for_deserialize()),
        );
        self.insert(
            format!("{name}.output.json"),
            schema_for::<T>(SchemaSettings::draft2020_12().for_serialize()),
        );
    }

    fn insert(&mut self, file: String, schema: Value) {
        let previous = self.files.insert(file.clone(), schema);
        assert!(previous.is_none(), "{file} is already in the registry");
    }

    /// The text of each schema file, by file name: pretty-printed JSON with a final newline. Names come out in
    /// sorted order, and so do the keys inside each schema, so the text is the same on every run.
    pub fn render(&self) -> BTreeMap<String, String> {
        let pretty = |schema: &Value| {
            let mut text = serde_json::to_string_pretty(schema).expect("a schema is JSON");
            text.push('\n');
            text
        };
        self.files
            .iter()
            .map(|(file, schema)| (file.clone(), pretty(schema)))
            .collect()
    }
}

fn schema_for<T: JsonSchema>(settings: SchemaSettings) -> Value {
    let schema = settings.into_generator().into_root_schema_for::<T>();
    serde_json::to_value(schema).expect("a schema is JSON")
}

/// Every contract type, which `pnpm gen` writes to `schemas/`. Later items add their types here.
pub fn registry() -> Registry {
    let mut registry = Registry::new();
    registry.add::<Adoption>();
    registry.add::<AgentEvent>();
    registry.add::<ApplyParams>();
    registry.add::<ApplyResult>();
    registry.add::<Board>();
    registry.add::<Capabilities>();
    registry.add::<Change>();
    registry.add::<CollidePointer>();
    registry.add::<ConfigFile>();
    registry.add::<ContextRecord>();
    registry.add::<Event>();
    registry.add::<Feature>();
    registry.add::<FeatureProblem>();
    registry.add::<GetParams>();
    registry.add::<GetResult>();
    registry.add::<HookInput>();
    registry.add::<HookResult>();
    registry.add::<InitializeParams>();
    registry.add::<InitializeResult>();
    registry.add::<KindDecl>();
    registry.add::<LinkTypeDecl>();
    registry.add::<ListParams>();
    registry.add::<ListResult>();
    registry.add::<NearDuplicateGroup>();
    registry.add::<ObserverAck>();
    registry.add::<ObserverEvent>();
    registry.add::<PackLock>();
    registry.add::<PackManifest>();
    registry.add::<PermissionReply>();
    registry.add::<PermissionRequest>();
    registry.add::<PhaseState>();
    registry.add::<Point>();
    registry.add::<PlanningFields>();
    registry.add::<PreviewResult>();
    registry.add::<Profile>();
    registry.add::<ReportLine>();
    // `Rename` is also the body of the provider's `task.renamed` notification.
    registry.add::<Rename>();
    registry.add::<RunBudget>();
    registry.add::<RunState>();
    registry.add::<SectionUse>();
    registry.add::<Sections>();
    registry.add::<SessionStart>();
    registry.add::<SourceStale>();
    registry.add::<Span>();
    registry.add::<Stage>();
    registry.add::<Task>();
    registry.add::<TaskChanged>();
    registry.add::<TaskCollision>();
    registry.add::<TaskInvalid>();
    registry.add::<TaskPointPayload>();
    registry.add::<TaskRemoved>();
    registry.add::<Usage>();
    registry.add::<UserPacks>();
    registry.add::<View>();
    registry.add::<WatchParams>();
    registry.add::<WatchResult>();
    registry.add::<Workflow>();
    registry.add::<WorkspaceState>();
    registry
}
