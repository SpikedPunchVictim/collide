//! The provider supervisor (0.4, Observers; 0.10, Protocol; Stage 4b). A provider is a long-lived process that speaks
//! JSON-RPC over stdio. [`crate::provider_client::ProviderClient`] is the client for one such process, one request at
//! a time, and the CLI uses it directly until this module is real. The supervisor wraps it and does not replace it:
//! it owns the current client, and when a call finds the provider gone ([`ProviderError::Gone`]) it starts a new one
//! after a delay that grows with each failure ([`BackoffPolicy`]), and asks again. It is isolated from the process
//! owner, so a provider that hangs or crashes never stalls the daemon.
//!
//! [`SupervisedProvider`] is a [`TaskProvider`] of `core`, so the engine and the corpus take it as one. Stage 4c moves
//! the planning slice's CLI behind it.
//!
//! The supervisor that restarts providers restarts observers too (0.4): after repeated crashes it disables the
//! observer and a Queue item says so. That is Stage 12 and is outlined at the end ([`supervise_observer`]).
//!
//! | tag | where | what it does |
//! |---|---|---|
//! | `supervisor: provider start` | [`SupervisedProvider`] `initialize` | starts the child and sends `initialize` with the pack's settings |
//! | `supervisor: restart with backoff` | [`BackoffPolicy::delay`] | the wait before the next start |
//! | `supervisor: provider calls` | [`SupervisedProvider`] the other methods | one request through the current child, a restart if it is gone |
//! | `supervisor: observers` | [`supervise_observer`] | start, restart with backoff, disable after repeated crashes |

use crate::builtin_packs::ProviderLaunch;
use crate::provider_client::ProviderClient;
use collide_core::ports::{ProviderError, TaskProvider};
use collide_core::provider::{
    ApplyParams, ApplyResult, GetParams, GetResult, InitializeParams, InitializeResult, ListParams,
    ListResult, PreviewResult, WatchParams, WatchResult,
};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

/// The wait before the first restart. The plan fixes none of the values here; Stage 4b confirms them.
pub const INITIAL_DELAY: Duration = Duration::from_secs(1);

/// The longest wait between two starts.
pub const MAX_DELAY: Duration = Duration::from_secs(60);

/// The crashes in a row after which a child is given up on: a provider reports `Gone` to its callers, an observer is
/// disabled.
pub const MAX_CRASHES: u32 = 5;

/// How long a restart waits, and when it stops trying.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BackoffPolicy {
    pub initial: Duration,
    pub max: Duration,
    pub max_crashes: u32,
}

impl Default for BackoffPolicy {
    fn default() -> Self {
        BackoffPolicy {
            initial: INITIAL_DELAY,
            max: MAX_DELAY,
            max_crashes: MAX_CRASHES,
        }
    }
}

impl BackoffPolicy {
    /// The wait before the start that follows `crashes` crashes in a row (1 for the first), never over `max`. `None`
    /// once `crashes` reaches `max_crashes`: the child is not started again.
    pub fn delay(&self, crashes: u32) -> Option<Duration> {
        let _ = (self, crashes);
        todo!("supervisor: restart with backoff")
    }
}

/// A provider kept alive: its launch, the folder it starts in, the policy, and the client of the current child.
pub struct SupervisedProvider {
    pub launch: ProviderLaunch,
    pub cwd: PathBuf,
    pub policy: BackoffPolicy,
    client: Mutex<Option<ProviderClient>>,
}

impl SupervisedProvider {
    /// A supervisor with no child yet: `initialize` starts it.
    pub fn new(launch: ProviderLaunch, cwd: impl Into<PathBuf>, policy: BackoffPolicy) -> Self {
        Self {
            launch,
            cwd: cwd.into(),
            policy,
            client: Mutex::new(None),
        }
    }

    /// Whether a child is running now.
    pub fn is_running(&self) -> bool {
        self.client
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .is_some()
    }
}

impl TaskProvider for SupervisedProvider {
    fn initialize(&self, params: &InitializeParams) -> Result<InitializeResult, ProviderError> {
        let _ = params;
        todo!("supervisor: provider start")
    }

    fn list(&self, params: &ListParams) -> Result<ListResult, ProviderError> {
        let _ = params;
        todo!("supervisor: provider calls")
    }

    fn get(&self, params: &GetParams) -> Result<GetResult, ProviderError> {
        let _ = params;
        todo!("supervisor: provider calls")
    }

    fn watch(&self, params: &WatchParams) -> Result<WatchResult, ProviderError> {
        let _ = params;
        todo!("supervisor: provider calls")
    }

    fn apply(&self, params: &ApplyParams) -> Result<ApplyResult, ProviderError> {
        let _ = params;
        todo!("supervisor: provider calls")
    }

    fn preview(&self, params: &ApplyParams) -> Result<PreviewResult, ProviderError> {
        let _ = params;
        todo!("supervisor: provider calls")
    }
}

/// An observer to keep alive (0.4): a pack's long-lived process, which gets events as JSONL on stdin and acknowledges
/// each on stdout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObserverSpec {
    pub pack: String,
    pub argv: Vec<String>,
    /// The event patterns the observer asked for (`run.*`, `ask_human.*`).
    pub events: Vec<String>,
}

/// Where an observer is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObserverStatus {
    Running,
    /// Crashed `crashes` times in a row; the next start is after the policy's delay.
    Restarting {
        crashes: u32,
    },
    /// Crashed `max_crashes` times in a row and was given up on. A Queue item says so.
    Disabled {
        crashes: u32,
    },
}

/// Starts an observer and keeps it alive under `policy`, resuming delivery after its last acknowledgement. Returns when
/// it is running, or disabled.
pub fn supervise_observer(spec: &ObserverSpec, policy: &BackoffPolicy) -> ObserverStatus {
    let _ = (spec, policy);
    todo!("supervisor: observers")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn provider() -> SupervisedProvider {
        let launch = ProviderLaunch {
            pack_dir: PathBuf::from("/x"),
            program: PathBuf::from("node"),
            args: Vec::new(),
            data: None,
        };
        SupervisedProvider::new(launch, "/x", BackoffPolicy::default())
    }

    #[test]
    fn a_new_supervisor_has_no_child_until_initialize_starts_one() {
        assert!(!provider().is_running());
    }

    #[test]
    fn the_default_policy_waits_a_second_at_first_and_a_minute_at_most_and_gives_up_after_five_crashes()
     {
        let policy = BackoffPolicy::default();
        assert_eq!(policy.initial, Duration::from_secs(1));
        assert_eq!(policy.max, Duration::from_secs(60));
        assert_eq!(policy.max_crashes, 5);
    }

    #[test]
    #[should_panic(expected = "supervisor: restart with backoff")]
    fn the_delay_is_a_stub_tagged_for_stage_4b() {
        let _ = BackoffPolicy::default().delay(1);
    }

    #[test]
    #[should_panic(expected = "supervisor: observers")]
    fn supervising_an_observer_is_a_stub_tagged_for_stage_12() {
        let spec = ObserverSpec {
            pack: "demo".into(),
            argv: vec!["observer".into()],
            events: vec!["run.*".into()],
        };
        let _ = supervise_observer(&spec, &BackoffPolicy::default());
    }
}
