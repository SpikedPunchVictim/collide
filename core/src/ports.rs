//! The two collaborators of the engine besides the agent traits (0.15, Dependency injection): [`HookRunner`], which
//! runs one hook, and [`TaskProvider`], which reads and writes one task source. Core logic takes them as traits, so a
//! test uses a fake and the engine runs with no process.
//!
//! Both are synchronous and take `&self`: the daemon shares one value across phases, and an implementation keeps its
//! own state behind it. Neither names a process, Tauri or SQLite type. The real runner is the hook executor of Stage 7
//! and the real providers are the supervisor's (Stage 4b), both in `host`.

use crate::hook::{HookInput, HookKind, HookResult};
use crate::pack::HookEntry;
use crate::provider::{
    ApplyParams, ApplyResult, GetParams, GetResult, InitializeParams, InitializeResult, ListParams,
    ListResult, PreviewResult, WatchParams, WatchResult,
};
use std::fmt;

/// One hook call: what kind of hook it is, the pack's entry for it (script, `timeout`, `grace` and the schema of each
/// output) and what it reads on stdin. The kind decides which results are valid (0.5; [`HookKind`]).
#[derive(Debug, Clone, PartialEq)]
pub struct HookRequest {
    pub kind: HookKind,
    pub entry: HookEntry,
    pub input: HookInput,
}

/// A hook that could not be run at all, which is not the hook's own `error` result: the executor could not start it
/// or the daemon was stopping.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HookRunError {
    /// The script could not be started. The message says why and where it looked.
    NotStarted { message: String },
    /// The run was stopped before the hook gave a result, and the hook's tree was stopped too (0.5, Stopping).
    Stopped,
}

impl fmt::Display for HookRunError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HookRunError::NotStarted { message } => write!(f, "hook did not start: {message}"),
            HookRunError::Stopped => f.write_str("hook was stopped before it gave a result"),
        }
    }
}

impl std::error::Error for HookRunError {}

/// Runs one hook and returns the result the engine acts on. The result is already the *effective* one
/// ([`crate::hook::effective_result`]): the executor applied the exit code, the result file and the rules for
/// `pending` and `unverified` before it answers, so the engine never sees a raw exit.
pub trait HookRunner {
    fn run(&self, request: &HookRequest) -> Result<HookResult, HookRunError>;
}

/// Why a provider call failed. A missing id is an answer (`GetResult { task: None }`), and a refusal is an
/// [`ApplyResult::Refused`], so neither is an error here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderError {
    /// The provider does not declare this method (`preview` when `capabilities.preview` is false).
    Unsupported { method: String },
    /// The provider answered with a JSON-RPC error, or its answer did not match the protocol.
    Failed { message: String },
    /// The provider process is gone. The supervisor restarts it with backoff (Stage 4b).
    Gone,
}

impl fmt::Display for ProviderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProviderError::Unsupported { method } => {
                write!(f, "the provider does not support `{method}`")
            }
            ProviderError::Failed { message } => write!(f, "the provider failed: {message}"),
            ProviderError::Gone => f.write_str("the provider process is gone"),
        }
    }
}

impl std::error::Error for ProviderError {}

/// A task source (0.10, Protocol): one method for each request of the provider protocol, over the types of
/// [`crate::provider`]. The notifications that `watch` starts (`task.changed`, `task.removed` and the others) reach the
/// host's supervisor as a stream, not through this trait.
pub trait TaskProvider {
    fn initialize(&self, params: &InitializeParams) -> Result<InitializeResult, ProviderError>;
    fn list(&self, params: &ListParams) -> Result<ListResult, ProviderError>;
    fn get(&self, params: &GetParams) -> Result<GetResult, ProviderError>;
    fn watch(&self, params: &WatchParams) -> Result<WatchResult, ProviderError>;
    fn apply(&self, params: &ApplyParams) -> Result<ApplyResult, ProviderError>;
    /// What `apply` would do, with nothing written. A provider that does not declare `preview` answers
    /// [`ProviderError::Unsupported`].
    fn preview(&self, params: &ApplyParams) -> Result<PreviewResult, ProviderError>;
}
