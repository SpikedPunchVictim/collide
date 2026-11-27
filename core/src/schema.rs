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
    AgentEvent, Capabilities, PermissionReply, PermissionRequest, SessionStart, Usage, UserModels,
};
use crate::ask;
use crate::board::{Board, CollidePointer};
use crate::change::{Change, Rename, TaskPointPayload};
use crate::context::ContextRecord;
use crate::events::Event;
use crate::hook::{HookInput, HookResult};
use crate::item::{
    Assumption, Claim, Coverage, Decision, Evidence, Feature, FeatureProblem, Finding, KindDecl,
    LinkTypeDecl, PartProblem, PlanningFields, Preregistration,
};
use crate::lock::{PackLock, UserPacks};
use crate::observer::{ObserverAck, ObserverEvent};
use crate::owner;
use crate::pack::{PackManifest, RunBudget};
use crate::points::Point;
use crate::profile::Profile;
use crate::provider::{
    ApplyParams, ApplyResult, CollectionsChanged, GetParams, GetResult, InitializeParams,
    InitializeResult, ListParams, ListResult, PreviewResult, SourceStale, Task, TaskChanged,
    TaskCollision, TaskInvalid, TaskRemoved, WatchParams, WatchResult,
};
use crate::report::ReportLine;
use crate::section::{NearDuplicateGroup, SectionUse, Sections};
use crate::settings::ConfigFile;
use crate::shim::{DaemonMessage, ExitRecord, ShimMessage};
use crate::span::Span;
use crate::stage::Stage;
use crate::state::{PhaseState, RunState, WorkspaceState};
use crate::ui_tools;
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
    registry.add::<Assumption>();
    registry.add::<Board>();
    registry.add::<Capabilities>();
    registry.add::<Change>();
    registry.add::<Claim>();
    registry.add::<CollectionsChanged>();
    registry.add::<CollidePointer>();
    registry.add::<ConfigFile>();
    registry.add::<ContextRecord>();
    registry.add::<Coverage>();
    registry.add::<DaemonMessage>();
    registry.add::<Decision>();
    registry.add::<Evidence>();
    registry.add::<Event>();
    registry.add::<ExitRecord>();
    registry.add::<Feature>();
    registry.add::<FeatureProblem>();
    registry.add::<Finding>();
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
    registry.add::<PartProblem>();
    registry.add::<PermissionReply>();
    registry.add::<PermissionRequest>();
    registry.add::<PhaseState>();
    registry.add::<Point>();
    registry.add::<PlanningFields>();
    registry.add::<Preregistration>();
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
    registry.add::<ShimMessage>();
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
    registry.add::<UserModels>();
    registry.add::<UserPacks>();
    registry.add::<View>();
    registry.add::<WatchParams>();
    registry.add::<WatchResult>();
    registry.add::<Workflow>();
    registry.add::<WorkspaceState>();
    // The owner's JSON-RPC (ts-0032-pp): the envelope, the notices, and each method's request and result.
    registry.add::<owner::Request>();
    registry.add::<owner::Response>();
    registry.add::<owner::Notification>();
    registry.add::<owner::Notice>();
    registry.add::<owner::RpcError>();
    registry.add::<owner::Ack>();
    registry.add::<owner::BoardChecksParams>();
    registry.add::<owner::BoardChecksResult>();
    registry.add::<owner::BoardInitParams>();
    registry.add::<owner::BoardInitResult>();
    registry.add::<owner::BoardListParams>();
    registry.add::<owner::BoardListResult>();
    registry.add::<owner::BoardLocateParams>();
    registry.add::<owner::BoardLocateResult>();
    registry.add::<owner::BoardOpenParams>();
    registry.add::<owner::BoardOpenResult>();
    registry.add::<owner::BoardPickParams>();
    registry.add::<owner::BoardTrustParams>();
    registry.add::<owner::BoardTrustResult>();
    registry.add::<owner::BoardTrustReviewParams>();
    registry.add::<owner::BoardTrustReviewResult>();
    registry.add::<owner::ConfigGetParams>();
    registry.add::<owner::ConfigGetResult>();
    registry.add::<owner::ConfigPackAddParams>();
    registry.add::<owner::ConfigPackEnableParams>();
    registry.add::<owner::ConfigPackLinkParams>();
    registry.add::<owner::ConfigPackListParams>();
    registry.add::<owner::ConfigPackListResult>();
    registry.add::<owner::ConfigPackResult>();
    registry.add::<owner::ConfigPackUpdateParams>();
    registry.add::<owner::ConfigPackUpdateResult>();
    registry.add::<owner::ConfigResetParams>();
    registry.add::<owner::ConfigResetResult>();
    registry.add::<owner::PackPin>();
    registry.add::<owner::PackFileChange>();
    registry.add::<owner::PackFileChangeKind>();
    registry.add::<owner::ConfigSetParams>();
    registry.add::<owner::ConfigSetResult>();
    registry.add::<owner::DaemonAgentVersionsParams>();
    registry.add::<owner::DaemonAgentVersionsResult>();
    registry.add::<owner::DaemonRecaptureEnvironmentParams>();
    registry.add::<owner::DaemonRecaptureEnvironmentResult>();
    registry.add::<owner::DaemonStatusParams>();
    registry.add::<owner::DaemonStatusResult>();
    registry.add::<owner::DaemonSubscribeParams>();
    registry.add::<owner::DaemonSubscribeResult>();
    registry.add::<owner::OwnerInitializeParams>();
    registry.add::<owner::OwnerInitializeResult>();
    registry.add::<owner::QueueAnswerParams>();
    registry.add::<owner::QueueAnswerResult>();
    registry.add::<owner::QueueContextParams>();
    registry.add::<owner::QueueListParams>();
    registry.add::<owner::QueueListResult>();
    registry.add::<owner::RunDetachParams>();
    registry.add::<owner::RunDiffParams>();
    registry.add::<owner::RunDiffResult>();
    registry.add::<owner::RunEndTakeOverParams>();
    registry.add::<owner::RunGetParams>();
    registry.add::<owner::RunGetResult>();
    registry.add::<owner::RunIntervenParams>();
    registry.add::<owner::RunListParams>();
    registry.add::<owner::RunListResult>();
    registry.add::<owner::RunOutputParams>();
    registry.add::<owner::RunOutputResult>();
    registry.add::<owner::RunPreviewResult>();
    registry.add::<owner::RunRecordParams>();
    registry.add::<owner::RunRecordResult>();
    registry.add::<owner::RunStartFileParams>();
    registry.add::<owner::RunStartParams>();
    registry.add::<owner::RunStartResult>();
    registry.add::<owner::RunStopParams>();
    registry.add::<owner::RunStopResult>();
    registry.add::<owner::RunTakeOverParams>();
    registry.add::<owner::RunTakeOverResult>();
    registry.add::<owner::RunTerminalInputParams>();
    registry.add::<owner::RunTerminalResizeParams>();
    registry.add::<owner::TaskApplyParams>();
    registry.add::<owner::TaskApplyResult>();
    registry.add::<owner::TaskChangesParams>();
    registry.add::<owner::TaskChangesResult>();
    registry.add::<owner::TaskCreateParams>();
    registry.add::<owner::TaskDiffParams>();
    registry.add::<owner::TaskDiffResult>();
    registry.add::<owner::TaskGetParams>();
    registry.add::<owner::TaskGetResult>();
    registry.add::<owner::TaskListParams>();
    registry.add::<owner::TaskListResult>();
    registry.add::<owner::TaskPreviewParams>();
    registry.add::<owner::TaskReidParams>();
    registry.add::<owner::TaskReidResult>();
    registry.add::<owner::TaskSaveViewParams>();
    registry.add::<owner::TaskSaveViewResult>();
    registry.add::<owner::TaskSectionsParams>();
    registry.add::<owner::TaskViewsParams>();
    registry.add::<owner::TaskViewsResult>();
    registry.add::<owner::WorkspaceListParams>();
    registry.add::<owner::WorkspaceListResult>();
    registry.add::<owner::WorkspaceRemoveParams>();
    registry.add::<owner::WorkspaceRemoveResult>();
    registry.add::<owner::WorkspaceResumeParams>();
    registry.add::<owner::WorkspaceResumeResult>();
    // The ask contract (ts-0034-hh): the window's and the daemon's shape of each method, the events and the outcome.
    registry.add::<ask::AskStartParams>();
    registry.add::<ask::AskStartDaemonParams>();
    registry.add::<ask::AskCancelParams>();
    registry.add::<ask::AskTestParams>();
    registry.add::<ask::AskTestDaemonParams>();
    registry.add::<ask::AskTestResult>();
    registry.add::<ask::AskOutcome>();
    registry.add::<ask::AskNotice>();
    // The shell's `ui_tools.*` contract (ts-0035-re): the window's three methods, the request event and the manifest.
    registry.add::<ui_tools::UiToolsReplyParams>();
    registry.add::<ui_tools::UiToolsTokenParams>();
    registry.add::<ui_tools::UiToolsTokenResult>();
    registry.add::<ui_tools::UiToolsRotateParams>();
    registry.add::<ui_tools::UiToolsRotateResult>();
    registry.add::<ui_tools::UiToolsRequest>();
    registry.add::<ui_tools::ToolDefinition>();
    registry.add::<ui_tools::ToolManifest>();
    registry
}
