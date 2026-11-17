//! The provider supervisor (0.4, Observers; 0.10, Protocol; Stage 4b). A provider is a long-lived process that speaks
//! JSON-RPC over stdio. [`crate::provider_client::ProviderClient`] is the client for one such process, one request at
//! a time. [`SupervisedProvider`] wraps it and keeps a child alive: it starts the child from a launch
//! ([`crate::provider_launch::provider_launch`]), sends `initialize`, and when a call finds the child gone it starts a
//! new one after a wait that grows with each crash ([`BackoffPolicy`]) and asks again. It runs the child on its own
//! reader thread, so a provider that hangs or crashes never stalls the daemon: a call waits no longer than the timeout
//! of its method ([`SupervisorPolicy`]).
//!
//! [`SupervisedProvider`] is a [`TaskProvider`] of `core`, so the engine and the corpus take it as one. Stage 4c moves
//! the planning slice's CLI behind it. The daemon methods that use it and the registry of sources are ts-0109 and
//! ts-0113.
//!
//! The supervisor that restarts providers restarts observers too (0.4): after repeated crashes it disables the
//! observer and a Queue item says so. That is Stage 12 and is outlined at the end ([`supervise_observer`], tag
//! `supervisor: observers`).

use crate::builtin_packs::ProviderLaunch;
use crate::provider_client::{CallTimeouts, ClientError, ClientOptions, ProviderClient};
use collide_core::ports::{ProviderError, TaskProvider};
use collide_core::provider::{
    ApplyParams, ApplyResult, Capabilities, GetParams, GetResult, InitializeParams,
    InitializeResult, ListParams, ListResult, PreviewResult, ProviderEvent, WatchParams,
    WatchResult, check_initialize,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, MutexGuard};
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
    /// The wait before the start that follows `crashes` crashes in a row (1 for the first), never over `max`: `initial`
    /// doubled for each crash after the first. No wait for no crash. `None` once `crashes` reaches `max_crashes`: the
    /// child is not started again.
    pub fn delay(&self, crashes: u32) -> Option<Duration> {
        if crashes >= self.max_crashes {
            return None;
        }
        let Some(doublings) = crashes.checked_sub(1) else {
            return Some(Duration::ZERO);
        };
        let factor = 1u32.checked_shl(doublings).unwrap_or(u32::MAX);
        Some(self.initial.saturating_mul(factor).min(self.max))
    }
}

/// Everything the supervisor of one provider is tuned by: how long each call may wait, how restarts back off, and how
/// long a stopping child has to end before it is killed. Tests give short values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SupervisorPolicy {
    pub timeouts: CallTimeouts,
    pub backoff: BackoffPolicy,
    pub stop_grace: Duration,
}

impl Default for SupervisorPolicy {
    fn default() -> Self {
        SupervisorPolicy {
            timeouts: CallTimeouts::default(),
            backoff: BackoffPolicy::default(),
            stop_grace: ClientOptions::default().stop_grace,
        }
    }
}

impl SupervisorPolicy {
    fn client_options(&self) -> ClientOptions {
        ClientOptions {
            timeouts: self.timeouts,
            stop_grace: self.stop_grace,
        }
    }
}

/// How the supervisor waits out a backoff. A test records the waits and does not make them.
pub trait Pause: Send + Sync {
    fn pause(&self, how_long: Duration);
}

/// The real wait.
pub struct ThreadSleep;

impl Pause for ThreadSleep {
    fn pause(&self, how_long: Duration) {
        std::thread::sleep(how_long);
    }
}

/// What a call needs and only a call touches: the child, and what it was told, to tell the next one the same. A call
/// holds the lock on it for as long as the call lasts, so calls go one at a time.
#[derive(Default)]
struct Calls {
    client: Option<ProviderClient>,
    init: Option<InitializeParams>,
    capabilities: Option<Capabilities>,
    watching: Option<WatchParams>,
}

/// What can be read while a call is in progress.
#[derive(Default)]
struct Status {
    crashes: u32,
    last_failure: Option<String>,
    given_up: bool,
    /// The current child's flag that its output closed.
    child_ended: Option<Arc<AtomicBool>>,
}

/// A provider kept alive. It starts in the pack's own folder with the environment its launch holds, tells the child the
/// pack's settings and the board's root, and restarts it when it is gone: after the wait of [`BackoffPolicy`], telling
/// the new child the same and asking again. A child that does not answer is killed at the timeout of the method. After
/// [`BackoffPolicy::max_crashes`] crashes in a row, with no call answered between them, it answers
/// [`ProviderError::Gone`] until [`SupervisedProvider::retry`].
///
/// A crash is a child that exits or closes its output before a call has its reply, or a timeout. A child that exits
/// while idle counts when the next call finds it.
///
/// A request that was in flight when the child ended is asked again of the next one, except `apply`: the provider may
/// have written before it died, and a second `apply` of a `create` would write twice. An `apply` that never reached the
/// child, because it was already gone, is asked again. A timeout is never asked again: the call fails, and the next
/// call starts a new child.
pub struct SupervisedProvider {
    name: String,
    launch: ProviderLaunch,
    policy: SupervisorPolicy,
    pause: Box<dyn Pause>,
    events: Sender<ProviderEvent>,
    events_receiver: Mutex<Option<Receiver<ProviderEvent>>>,
    calls: Mutex<Calls>,
    status: Mutex<Status>,
}

fn locked<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// What `start` makes: a child that answered `initialize`.
struct Started {
    client: ProviderClient,
    result: InitializeResult,
}

impl SupervisedProvider {
    /// A supervisor with no child yet: `initialize` starts it. `name` is how a message names the provider. The child
    /// starts in `launch.pack_dir` and never in a workspace.
    pub fn new(name: impl Into<String>, launch: ProviderLaunch, policy: SupervisorPolicy) -> Self {
        let (events, events_receiver) = mpsc::channel();
        Self {
            name: name.into(),
            launch,
            policy,
            pause: Box::new(ThreadSleep),
            events,
            events_receiver: Mutex::new(Some(events_receiver)),
            calls: Mutex::new(Calls::default()),
            status: Mutex::new(Status::default()),
        }
    }

    /// Waits out a backoff with `pause`.
    pub fn with_pause(mut self, pause: Box<dyn Pause>) -> Self {
        self.pause = pause;
        self
    }

    /// The notifications of every child this supervisor has started, in the order they were read. Given once. A
    /// restart does not change the channel.
    pub fn take_events(&self) -> Option<Receiver<ProviderEvent>> {
        locked(&self.events_receiver).take()
    }

    /// Whether a child is running now: started, and its output not closed. It does not wait for a call in progress.
    pub fn is_running(&self) -> bool {
        locked(&self.status)
            .child_ended
            .as_ref()
            .is_some_and(|ended| !ended.load(Ordering::SeqCst))
    }

    /// The crashes since the last call that was answered.
    pub fn crashes_in_a_row(&self) -> u32 {
        locked(&self.status).crashes
    }

    /// Why the last crash or failed start happened, naming the provider. `None` before one, and after a retry.
    pub fn last_failure(&self) -> Option<String> {
        locked(&self.status).last_failure.clone()
    }

    /// A person's explicit retry: starts a new child, tells it what the first one was told, and resets the count of
    /// crashes. For a provider that answers [`ProviderError::Gone`].
    pub fn retry(&self) -> Result<InitializeResult, ProviderError> {
        let mut calls = locked(&self.calls);
        let Some(params) = calls.init.clone() else {
            return Err(self.failed("it was never initialized, so there is nothing to start again"));
        };
        self.begin(&mut calls, params)
    }

    /// Closes the child's input, waits for it to end, then kills it. Calls after it need `initialize` again, so
    /// nothing starts a stopped provider.
    pub fn stop(&self) {
        let mut calls = locked(&self.calls);
        self.drop_child(&mut calls);
        *calls = Calls::default();
    }

    fn said(&self, what: impl std::fmt::Display) -> String {
        format!("provider {}: {what}", self.name)
    }

    fn failed(&self, what: &str) -> ProviderError {
        ProviderError::Failed {
            message: self.said(what),
        }
    }

    fn install_child(&self, calls: &mut Calls, client: ProviderClient) {
        locked(&self.status).child_ended = Some(client.ended_flag());
        calls.client = Some(client);
    }

    fn drop_child(&self, calls: &mut Calls) {
        locked(&self.status).child_ended = None;
        calls.client = None;
    }

    /// Notes one crash. After the last one the supervisor gives up.
    fn crashed(&self, why: String) {
        let mut status = locked(&self.status);
        status.crashes += 1;
        status.last_failure = Some(why);
        if self.policy.backoff.delay(status.crashes).is_none() {
            status.given_up = true;
        }
    }

    /// Starts a child and has it answer `initialize`, and `watch` again when the last child was watching.
    fn start(
        &self,
        params: &InitializeParams,
        watching: Option<&WatchParams>,
    ) -> Result<Started, String> {
        // A supervised provider never inherits the daemon's environment: its launch carries the base one (0.2).
        if self.launch.env.is_none() {
            return Err(self.said("its launch has no base environment"));
        }
        let mut client = ProviderClient::start_with(
            &self.launch,
            &self.launch.pack_dir,
            self.policy.client_options(),
            self.events.clone(),
        )
        .map_err(|error| self.said(error))?;
        let result = client
            .initialize(params)
            .map_err(|error| self.said(error))?;
        if let Err(refusals) = check_initialize(&result) {
            let reasons: Vec<String> = refusals.iter().map(ToString::to_string).collect();
            return Err(self.said(format!(
                "its `initialize` answer is refused: {}",
                reasons.join("; ")
            )));
        }
        if let Some(watch) = watching {
            client.watch(watch).map_err(|error| self.said(error))?;
        }
        Ok(Started { client, result })
    }

    /// The explicit start: the count of crashes is reset, and one child is started and initialized, with no restart.
    fn begin(
        &self,
        calls: &mut Calls,
        params: InitializeParams,
    ) -> Result<InitializeResult, ProviderError> {
        self.drop_child(calls);
        *locked(&self.status) = Status::default();
        match self.start(&params, calls.watching.as_ref()) {
            Ok(started) => {
                calls.capabilities = Some(started.result.capabilities.clone());
                calls.init = Some(params);
                self.install_child(calls, started.client);
                Ok(started.result)
            }
            Err(message) => {
                self.crashed(message.clone());
                Err(ProviderError::Failed { message })
            }
        }
    }

    /// Leaves a running child in `calls.client`: the one there, or a new one after the wait for the crashes so far.
    /// `Gone` when the supervisor has given up.
    fn ensure_child(&self, calls: &mut Calls) -> Result<(), ProviderError> {
        loop {
            if locked(&self.status).given_up {
                return Err(ProviderError::Gone);
            }
            match &calls.client {
                Some(client) if !client.has_ended() => return Ok(()),
                Some(_) => {
                    self.drop_child(calls);
                    self.crashed(self.said("the child ended while no call was waiting for it"));
                    continue;
                }
                None => {}
            }
            let crashes = locked(&self.status).crashes;
            if let Some(wait) = self.policy.backoff.delay(crashes)
                && !wait.is_zero()
            {
                self.pause.pause(wait);
            }
            let params = calls.init.clone().expect("a call comes after initialize");
            match self.start(&params, calls.watching.as_ref()) {
                Ok(started) => {
                    calls.capabilities = Some(started.result.capabilities);
                    self.install_child(calls, started.client);
                    return Ok(());
                }
                Err(message) => self.crashed(message),
            }
        }
    }

    /// One request through the current child, with the restarts and the retries of the type's rules.
    fn call<T>(
        &self,
        calls: &mut Calls,
        method: &str,
        ask: impl Fn(&mut ProviderClient) -> Result<T, ClientError>,
    ) -> Result<T, ProviderError> {
        if calls.init.is_none() {
            return Err(self.failed(&format!("`{method}` came before `initialize`")));
        }
        loop {
            self.ensure_child(calls)?;
            let client = calls.client.as_mut().expect("ensure_child leaves a child");
            let error = match ask(client) {
                Ok(value) => {
                    locked(&self.status).crashes = 0;
                    return Ok(value);
                }
                Err(error) => error,
            };
            if error.is_method_not_found() {
                return Err(ProviderError::Unsupported {
                    method: method.to_string(),
                });
            }
            if !error.ends_child() {
                return Err(ProviderError::Failed {
                    message: self.said(&error),
                });
            }
            self.drop_child(calls);
            let message = self.said(&error);
            self.crashed(message.clone());
            let in_flight_write = method == "apply" && error.may_have_run();
            let timed_out = matches!(error, ClientError::Timeout { .. });
            if in_flight_write {
                return Err(ProviderError::Failed {
                    message: format!(
                        "{message}; the write may have happened, so it is not asked again: \
                         check the task before you try again"
                    ),
                });
            }
            if timed_out {
                return Err(ProviderError::Failed { message });
            }
        }
    }

    /// `Unsupported` when the provider declared in `initialize` that it cannot do `method`.
    fn declared(
        &self,
        calls: &Calls,
        method: &str,
        can: impl Fn(&Capabilities) -> bool,
    ) -> Result<(), ProviderError> {
        match &calls.capabilities {
            Some(capabilities) if !can(capabilities) => Err(ProviderError::Unsupported {
                method: method.to_string(),
            }),
            _ => Ok(()),
        }
    }
}

impl TaskProvider for SupervisedProvider {
    /// The explicit start, as many times as it is called: each one starts a new child, resets the count of crashes
    /// and forgets any `watch`. A result that fails [`check_initialize`] is refused, naming the provider and why.
    fn initialize(&self, params: &InitializeParams) -> Result<InitializeResult, ProviderError> {
        let mut calls = locked(&self.calls);
        calls.watching = None;
        self.begin(&mut calls, params.clone())
    }

    fn list(&self, params: &ListParams) -> Result<ListResult, ProviderError> {
        let mut calls = locked(&self.calls);
        self.call(&mut calls, "list", |client| client.list(params))
    }

    fn get(&self, params: &GetParams) -> Result<GetResult, ProviderError> {
        let mut calls = locked(&self.calls);
        self.call(&mut calls, "get", |client| {
            client
                .get(&params.id, params.checkout.as_deref())
                .map(|task| GetResult { task })
        })
    }

    /// A provider that declared no `watch` answers `Unsupported`. The watch is asked again of each new child, so the
    /// events go on.
    fn watch(&self, params: &WatchParams) -> Result<WatchResult, ProviderError> {
        let mut calls = locked(&self.calls);
        self.declared(&calls, "watch", |capabilities| capabilities.watch)?;
        let result = self.call(&mut calls, "watch", |client| client.watch(params))?;
        calls.watching = Some(params.clone());
        Ok(result)
    }

    fn apply(&self, params: &ApplyParams) -> Result<ApplyResult, ProviderError> {
        let mut calls = locked(&self.calls);
        self.call(&mut calls, "apply", |client| client.apply(params))
    }

    /// A provider that declared no `preview`, or that answers "method not found", answers `Unsupported`.
    fn preview(&self, params: &ApplyParams) -> Result<PreviewResult, ProviderError> {
        let mut calls = locked(&self.calls);
        self.declared(&calls, "preview", |capabilities| capabilities.preview)?;
        self.call(&mut calls, "preview", |client| client.preview(params))
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

    fn seconds(list: &[u64]) -> Vec<Option<Duration>> {
        list.iter().map(|s| Some(Duration::from_secs(*s))).collect()
    }

    #[test]
    fn the_default_delays_for_the_first_four_crashes_double_from_a_second() {
        let policy = BackoffPolicy::default();
        let got: Vec<_> = (1..=4).map(|crashes| policy.delay(crashes)).collect();
        assert_eq!(got, seconds(&[1, 2, 4, 8]));
    }

    #[test]
    fn the_delay_is_none_once_crashes_reach_the_most() {
        let policy = BackoffPolicy::default();
        assert_eq!(policy.delay(5), None);
        assert_eq!(policy.delay(6), None);
        assert_eq!(policy.delay(u32::MAX), None);
    }

    #[test]
    fn the_delay_never_passes_the_cap() {
        let policy = BackoffPolicy {
            initial: Duration::from_secs(1),
            max: Duration::from_secs(60),
            max_crashes: 12,
        };
        let got: Vec<_> = (1..12).map(|crashes| policy.delay(crashes)).collect();
        assert_eq!(got, seconds(&[1, 2, 4, 8, 16, 32, 60, 60, 60, 60, 60]));
        assert_eq!(policy.delay(12), None);
    }

    #[test]
    fn a_long_run_of_crashes_does_not_overflow_the_doubling() {
        let policy = BackoffPolicy {
            initial: Duration::from_secs(1),
            max: Duration::from_secs(60),
            max_crashes: u32::MAX,
        };
        assert_eq!(policy.delay(200), Some(Duration::from_secs(60)));
    }

    #[test]
    fn no_crash_means_no_wait() {
        assert_eq!(BackoffPolicy::default().delay(0), Some(Duration::ZERO));
    }

    #[test]
    fn the_default_policy_has_the_owners_timeouts_and_backoff() {
        let policy = SupervisorPolicy::default();
        let timeouts = policy.timeouts;
        let got = [
            timeouts.initialize,
            timeouts.list,
            timeouts.get,
            timeouts.apply,
            timeouts.preview,
            timeouts.watch,
        ];
        assert_eq!(got.map(|d| d.as_secs()), [30, 60, 10, 120, 120, 10]);
        assert_eq!(policy.backoff.initial, Duration::from_secs(1));
        assert_eq!(policy.backoff.max, Duration::from_secs(60));
        assert_eq!(policy.backoff.max_crashes, 5);
    }

    #[test]
    fn each_method_has_the_timeout_of_its_own_field() {
        let timeouts = CallTimeouts::default();
        for (method, want) in [
            ("initialize", timeouts.initialize),
            ("list", timeouts.list),
            ("get", timeouts.get),
            ("apply", timeouts.apply),
            ("preview", timeouts.preview),
            ("watch", timeouts.watch),
        ] {
            assert_eq!(timeouts.of(method), want, "{method}");
        }
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
