//! The run-state rules the engine will follow (0.2, 0.4, 0.5, 0.7), as three pure functions that read no clock and
//! touch nothing. The state machines are [`crate::transition`]'s (Stage 3a) and the engine is Stage 8's; these are the
//! rules their tests and code will use, with fixtures now.
//!
//! | rule | function |
//! |---|---|
//! | record a hook result into run state, and when to poll a `pending` one | [`record_hook_result`], [`poll_delay`] |
//! | what an `integrate` answer does to the run, and what it holds back | [`apply_integrate_answer`] |
//! | what a restart does with a step that has an intent and no outcome | [`recover`], [`settle`] |
//!
//! Time comes in as an uptime reading (0.2, the clock table: work timeouts use the uptime clock). It is a
//! [`Duration`] since any fixed point on that clock, so only the difference between two readings means anything.
//!
//! None of these types crosses a process boundary or is written to the event log, so none is in
//! [`crate::schema`]. [`settle`] returns the [`EventBody`] the engine writes, which is.

use crate::duration::Duration;
use crate::events::{Event, EventBody, StepKind, StepResult};
use crate::hook::{HookKind, HookResult, HookStatus, misplaced_pending_message};
use crate::non_empty::NonEmpty;
use crate::state::RunState;
use crate::transition::{RunVia, TransitionError, run_transition};
use crate::workflow::MIN_WAIT_INTERVAL;
use std::fmt;

/// An uptime reading, or the length of a span of uptime.
pub type Uptime = Duration;

/// What a run-state entry is keyed by (0.5, State): the hook as `pack/hook`, the phase, its round, and the
/// fan-out instance when there is one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryKey {
    pub hook: String,
    pub phase: String,
    pub round: u32,
    pub instance: Option<String>,
}

impl fmt::Display for EntryKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "hook {} in phase `{}`, round {}",
            self.hook, self.phase, self.round
        )?;
        if let Some(instance) = &self.instance {
            write!(f, ", instance `{instance}`")?;
        }
        Ok(())
    }
}

/// One entry of a run's hook state: what a hook answered for one [`EntryKey`].
#[derive(Debug, Clone, PartialEq)]
pub struct HookEntry {
    pub key: EntryKey,
    pub result: HookResult,
}

/// The timing of a `wait` phase: its `interval:` and `timeout:` (0.7), and the uptime reading when it started.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WaitTiming {
    pub interval: Duration,
    pub timeout: Duration,
    pub started: Uptime,
}

impl WaitTiming {
    fn deadline(&self) -> Uptime {
        Duration::from_secs(
            self.started
                .as_secs()
                .saturating_add(self.timeout.as_secs()),
        )
    }
}

/// When the engine asks a `pending` wait hook again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NextPoll {
    /// Nothing is polled: the result is not `pending`, or it is a pull request `integrate`'s, which a workflow polls
    /// with a `wait` phase (0.4).
    None,
    /// At this uptime reading.
    At(Uptime),
}

/// What [`record_hook_result`] decided.
#[derive(Debug, Clone, PartialEq)]
pub struct Recorded {
    /// The run's hook entries after this result.
    pub entries: Vec<HookEntry>,
    /// The result that counts. It is `result` as given, unless the rules turned it into an `error`.
    pub result: HookResult,
    pub next: NextPoll,
}

/// How long to wait before asking a `pending` hook again (0.5): the larger of its `retryAfter` hint and the phase's
/// `interval:`, and never under 1 s. With no hint, the interval.
pub fn poll_delay(interval: Duration, retry_after: Option<u32>) -> Duration {
    let hint = Duration::from_secs(u64::from(retry_after.unwrap_or(0)));
    interval.max(hint).max(MIN_WAIT_INTERVAL)
}

fn error_result(message: String) -> HookResult {
    HookResult {
        status: HookStatus::Error,
        message,
        outputs: Default::default(),
        retry_after: None,
    }
}

/// Applies the `pending` rules (0.5) to `result`: the result that counts, and the next poll.
fn settle_pending(
    key: &EntryKey,
    kind: HookKind,
    wait: Option<&WaitTiming>,
    now: Uptime,
    result: HookResult,
) -> (HookResult, NextPoll) {
    if result.status != HookStatus::Pending {
        return (result, NextPoll::None);
    }
    if !kind.may_answer_pending() {
        let message = format!("{key}: {}", misplaced_pending_message(kind));
        return (error_result(message), NextPoll::None);
    }
    if kind != HookKind::Wait {
        return (result, NextPoll::None);
    }
    let Some(wait) = wait else {
        let message =
            format!("{key} answered `pending` but its wait phase has no interval and timeout");
        return (error_result(message), NextPoll::None);
    };
    let deadline = wait.deadline();
    if now >= deadline {
        let last = if result.message.is_empty() {
            String::new()
        } else {
            format!(": {}", result.message)
        };
        let message = format!(
            "{key} was still pending at its {} timeout{last}",
            wait.timeout
        );
        return (error_result(message), NextPoll::None);
    }
    let delay = poll_delay(wait.interval, result.retry_after);
    let next = Duration::from_secs(now.as_secs().saturating_add(delay.as_secs())).min(deadline);
    (result, NextPoll::At(next))
}

/// Records a hook's `result` into the run's hook `entries` (0.5, State). `now` is the uptime reading when it
/// arrived, and `wait` is the timing of the phase when the hook is a `wait` phase's.
///
/// - A `pending` result replaces the pending entry of the same key, so a CI wait polled for an hour keeps one
///   entry. The result that ends the polling replaces it too. A `pending` never replaces a settled entry: a gate's
///   last non-pending result stays, and the new pending entry is added after it.
/// - `pending` from a hook that is neither `wait` nor a pull request `integrate` is an `error`.
/// - A `wait` whose `now` has reached `started + timeout` while it is still `pending` is an `error` that carries the
///   last message. Otherwise the next poll is [`poll_delay`] from now, at most at the timeout, so a wait ends at
///   its timeout and not up to an interval after it.
pub fn record_hook_result(
    entries: &[HookEntry],
    key: EntryKey,
    kind: HookKind,
    wait: Option<&WaitTiming>,
    now: Uptime,
    result: HookResult,
) -> Recorded {
    let (result, next) = settle_pending(&key, kind, wait, now, result);
    let mut entries = entries.to_vec();
    let entry = HookEntry {
        key,
        result: result.clone(),
    };
    let pending_slot = entries
        .iter()
        .position(|e| e.key == entry.key && e.result.status == HookStatus::Pending);
    match pending_slot {
        Some(slot) => entries[slot] = entry,
        None => entries.push(entry),
    }
    Recorded {
        entries,
        result,
        next,
    }
}

/// What an `integrate` hook answered (0.4, 0.5): `merged {commit}`, or for a pull request `pending {url, head}`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntegrateAnswer {
    Merged { commit: NonEmpty },
    Pending { url: NonEmpty, head: NonEmpty },
}

/// Why an `integrate` answer was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntegrateError {
    /// The result's status is neither `ok` (merged) nor `pending`.
    NotAnAnswer { status: HookStatus },
    /// An output the answer needs is missing, is not a string, or is empty.
    BadOutput { status: HookStatus, name: String },
    /// The run's state machine refused the move: only an `ended` run is accepted, and an `integrating` one keeps
    /// waiting for its merge ([`crate::transition::run_transition`]).
    RunRefused(TransitionError),
}

impl fmt::Display for IntegrateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IntegrateError::NotAnAnswer { status } => write!(
                f,
                "an integrate answers `ok` with a commit (merged) or `pending` with a url and a head, not `{}`",
                status_name(*status)
            ),
            IntegrateError::BadOutput { status, name } => write!(
                f,
                "an integrate result `{}` needs a non-empty string output `{name}`",
                status_name(*status)
            ),
            IntegrateError::RunRefused(error) => {
                write!(f, "an integrate answer was refused: {error}")
            }
        }
    }
}

impl std::error::Error for IntegrateError {}

fn status_name(status: HookStatus) -> &'static str {
    match status {
        HookStatus::Ok => "ok",
        HookStatus::Fail => "fail",
        HookStatus::Unverified => "unverified",
        HookStatus::Pending => "pending",
        HookStatus::Refused => "refused",
        HookStatus::Error => "error",
    }
}

impl IntegrateAnswer {
    /// Reads the answer from an `integrate` hook's result: `ok` with `outputs.commit`, or `pending` with
    /// `outputs.url` and `outputs.head`. Whether `pending` is allowed from this hook is
    /// [`crate::hook::effective_result`]'s rule, applied before this.
    pub fn from_result(result: &HookResult) -> Result<IntegrateAnswer, IntegrateError> {
        let status = result.status;
        let output = |name: &str| {
            result
                .outputs
                .get(name)
                .and_then(|value| value.as_str())
                .and_then(|text| NonEmpty::new(text).ok())
                .ok_or_else(|| IntegrateError::BadOutput {
                    status,
                    name: name.to_string(),
                })
        };
        match status {
            HookStatus::Ok => Ok(IntegrateAnswer::Merged {
                commit: output("commit")?,
            }),
            HookStatus::Pending => Ok(IntegrateAnswer::Pending {
                url: output("url")?,
                head: output("head")?,
            }),
            _ => Err(IntegrateError::NotAnAnswer { status }),
        }
    }
}

/// What waits for a merge (0.4): the run's `run.after_integrate` handlers, and then the source's operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AfterIntegrate {
    RunAfterIntegrate,
    SourceOps,
}

/// What follows a merge, in the lifecycle's order (0.4).
const AFTER_MERGE: [AfterIntegrate; 2] =
    [AfterIntegrate::RunAfterIntegrate, AfterIntegrate::SourceOps];

/// What an `integrate` answer did to the run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntegrateStep {
    /// The run's state after this answer, reached through the state machine ([`run_transition`]): `integrating` after
    /// `pending`, and `ended` with its outcome after `merged`.
    pub run: RunState,
    /// What may run now, in order.
    pub release: Vec<AfterIntegrate>,
    /// What waits for `merged`.
    pub held: Vec<AfterIntegrate>,
}

/// Applies an `integrate` answer to a run (0.4, `integrating` and `pending`):
/// - `merged {commit}` releases [`AfterIntegrate`]'s steps, `run.after_integrate` first, and takes the run out of
///   `integrating` back to `ended`;
/// - `pending {url, head}` puts the run in `integrating` and holds those steps back until `merged`. Answering
///   `pending` again keeps it so.
///
/// The run must be `ended` (accepting is an action on a finished run) or already `integrating`; the state machine
/// refuses any other state, and the error names it. `outcome` is what the run ended with. `integrating` does not keep
/// it, so the run record gives it again for the move back to `ended`.
pub fn apply_integrate_answer(
    run: &RunState,
    answer: &IntegrateAnswer,
    outcome: &NonEmpty,
) -> Result<IntegrateStep, IntegrateError> {
    let (after, release, held) = match answer {
        IntegrateAnswer::Merged { .. } => (
            RunState::Ended {
                outcome: outcome.clone(),
            },
            AFTER_MERGE.to_vec(),
            Vec::new(),
        ),
        IntegrateAnswer::Pending { .. } => {
            (RunState::Integrating {}, Vec::new(), AFTER_MERGE.to_vec())
        }
    };
    run_transition(run, &after, RunVia::IntegrateAnswer).map_err(IntegrateError::RunRefused)?;
    Ok(IntegrateStep {
        run: after,
        release,
        held,
    })
}

/// A side-effect step that wrote an intent and no outcome (0.2). After a restart the engine checks the world for
/// this step, with the step's own check (is the branch merged, does the PR exist, is the label set). It never
/// repeats the step before that.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckWorld {
    pub step: String,
    pub kind: StepKind,
    /// What the intent said the step would do.
    pub action: String,
    /// The `seq` of the intent event.
    pub intent_seq: u64,
}

/// The steps of an event log that have an intent and no outcome, in the order of their intents. `events` are in
/// log order. A step written again has one intent open, its newest. An outcome with no intent is ignored.
pub fn recover(events: &[Event]) -> Vec<CheckWorld> {
    let mut open: Vec<CheckWorld> = Vec::new();
    for event in events {
        match &event.body {
            EventBody::StepIntent { step, kind, action } => {
                open.retain(|check| !(check.step == *step && check.kind == *kind));
                open.push(CheckWorld {
                    step: step.clone(),
                    kind: *kind,
                    action: action.clone(),
                    intent_seq: event.seq,
                });
            }
            EventBody::StepOutcome { step, kind, .. } => {
                open.retain(|check| !(check.step == *step && check.kind == *kind));
            }
            _ => {}
        }
    }
    open
}

/// What the step's own check found in the world.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Finding {
    /// The step's effect is there: the branch is merged, the PR exists, the label is set. A merge that finds its
    /// change already at the target is this, not a conflict (0.2).
    Done { detail: String },
    /// The check is sure the effect is not there.
    NotDone,
    /// The check could not tell, for example because the remote did not answer.
    Inconclusive { reason: String },
}

/// The Queue item "outcome unknown": the run waits for a person to say whether the step happened (0.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutcomeUnknown {
    pub step: String,
    pub kind: StepKind,
    pub action: String,
    pub reason: String,
}

impl fmt::Display for OutcomeUnknown {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "outcome unknown: step {} ({:?}) intended to {}, and the check could not tell whether it happened: {}",
            self.step, self.kind, self.action, self.reason
        )
    }
}

/// What to do about an open step once its check has a [`Finding`].
#[derive(Debug, Clone, PartialEq)]
pub enum Settled {
    /// It happened: write this outcome, and do not run the step.
    Succeeded { outcome: Box<EventBody> },
    /// It did not happen, and the check was sure: the step runs again, with a new intent.
    RunStep,
    /// The check could not tell: nothing runs, and the run waits in the Queue.
    OutcomeUnknown(OutcomeUnknown),
}

/// Turns the check of an open step into what the engine does next (0.2). A step is never run again unless the
/// check found it was not done, and an inconclusive check never runs it.
pub fn settle(check: &CheckWorld, finding: Finding) -> Settled {
    match finding {
        Finding::Done { detail } => Settled::Succeeded {
            outcome: Box::new(EventBody::StepOutcome {
                step: check.step.clone(),
                kind: check.kind,
                result: StepResult::Succeeded,
                detail,
            }),
        },
        Finding::NotDone => Settled::RunStep,
        Finding::Inconclusive { reason } => Settled::OutcomeUnknown(OutcomeUnknown {
            step: check.step.clone(),
            kind: check.kind,
            action: check.action.clone(),
            reason,
        }),
    }
}
