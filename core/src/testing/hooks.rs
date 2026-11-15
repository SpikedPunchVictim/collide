//! The fake hook runner: a [`HookRunner`] that returns scripted results by hook name, so an engine test drives a
//! gate, a wait or a record with no script and no process (0.15, Dependency injection).
//!
//! A script is a list of results for one hook (`demo/ci` in `HookInput.hook`). Each call takes the next result and
//! the last one repeats, so one entry is "always this" and a list is "pending twice, then ok". A hook with no script
//! did not start, as a missing script would not.
//!
//! The runner returns the *effective* result, as the real executor does ([`crate::ports::HookRunner`]): a scripted
//! result goes through [`effective_result`], so a `pending` from a gate comes back as the `error` the engine would
//! get, and a test cannot script something no real hook could answer.

use crate::hook::{HOOK_PROTOCOL, HookInput, HookKind, HookResult, HookStatus, effective_result};
use crate::pack::HookEntry;
use crate::ports::{HookRequest, HookRunError, HookRunner};
use serde_json::json;
use std::collections::{BTreeMap, VecDeque};
use std::sync::Mutex;

type Scripted = Result<HookResult, HookRunError>;

#[derive(Default)]
struct State {
    scripts: BTreeMap<String, VecDeque<Scripted>>,
    calls: Vec<HookRequest>,
}

/// A hook runner whose answers are scripted. It takes `&self`, like the trait, and keeps its state behind a lock.
#[derive(Default)]
pub struct FakeHookRunner {
    state: Mutex<State>,
}

impl FakeHookRunner {
    pub fn new() -> Self {
        Self::default()
    }

    /// The results `hook` returns, in order, the last one repeating. A second script for the same hook replaces the
    /// first.
    pub fn on(self, hook: &str, results: impl IntoIterator<Item = Scripted>) -> Self {
        self.state
            .lock()
            .expect("the fake's lock is never held across a panic")
            .scripts
            .insert(hook.to_string(), results.into_iter().collect());
        self
    }

    /// Every request the runner has been given, in order.
    pub fn calls(&self) -> Vec<HookRequest> {
        self.state
            .lock()
            .expect("the fake's lock is never held across a panic")
            .calls
            .clone()
    }
}

impl HookRunner for FakeHookRunner {
    fn run(&self, request: &HookRequest) -> Result<HookResult, HookRunError> {
        let mut state = self
            .state
            .lock()
            .expect("the fake's lock is never held across a panic");
        state.calls.push(request.clone());
        let hook = &request.input.hook;
        let script = state
            .scripts
            .get_mut(hook)
            .filter(|script| !script.is_empty());
        let Some(script) = script else {
            return Err(HookRunError::NotStarted {
                message: format!("the fake hook runner has no script for hook `{hook}`"),
            });
        };
        let scripted = if script.len() > 1 {
            script
                .pop_front()
                .expect("a script of two or more has a first")
        } else {
            script.front().expect("a script of one has a first").clone()
        };
        scripted.map(|result| {
            let file = serde_json::to_vec(&result).expect("a hook result serializes");
            effective_result(request.kind, true, Some(&file))
        })
    }
}

/// A hook result with a status and a message, and no outputs.
pub fn hook_result(status: HookStatus, message: &str) -> HookResult {
    HookResult {
        status,
        message: message.into(),
        outputs: BTreeMap::new(),
        retry_after: None,
    }
}

/// A request to run the hook `hook` of kind `kind`, with an entry for a script of the same name and an input that
/// belongs to no run.
pub fn hook_request(hook: &str, kind: HookKind) -> HookRequest {
    let input = HookInput {
        protocol: HOOK_PROTOCOL,
        hook: hook.into(),
        pack: hook.split('/').next().unwrap_or(hook).into(),
        workflow: None,
        board: "board".into(),
        line: "main".into(),
        experiment: None,
        run: None,
        launch: BTreeMap::new(),
        config: BTreeMap::new(),
        tasks: Vec::new(),
        phase: None,
        workspace: None,
        history: Vec::new(),
        state: Default::default(),
        change: None,
        latest_run: None,
    };
    HookRequest {
        kind,
        entry: serde_json::from_value::<HookEntry>(json!({ "script": format!("{hook}.sh") }))
            .expect("an entry with a script"),
        input,
    }
}
