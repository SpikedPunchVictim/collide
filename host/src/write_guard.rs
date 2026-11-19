//! The guards on a write to a checkout (ts-0114-hr; 0.4, Line lock and Root busy; 0.10, Ref-scoped sources).
//!
//! Three things stand between a task write and the checkout it names:
//!
//! 1. **The line lock** ([`LineLocks`]). One holder at a time for each checkout, held for a write and what follows it
//!    (its events). Writes to two checkouts do not wait for each other. It lives in the daemon's process: the CLI's
//!    direct path takes the provider's own lock file, and the two are not tied together, so a CLI write and a daemon
//!    write to one root are not serialized by this lock. `record` and `integrate` take it in Stage 7.
//! 2. **A busy checkout.** A rebase, merge, cherry-pick, revert or bisect in progress there
//!    ([`crate::checkout::root_busy`]), or a phase running in it ([`LivePhases`]). The write is not applied.
//! 3. **The queue.** A write that cannot be applied is stored with a `Conflict` Queue item that says why, and is
//!    applied when the checkout is free, in the order the writes were made ([`WriteGuard::enqueue`],
//!    [`WriteGuard::drain`]). The item and the write are one row of the store, so both survive a restart.
//!
//! A phase that starts at the root of a line is the exception (0.4, revision 22): Collide's writes there do not wait
//! for it, so a registration says whether its phase started at the root ([`LivePhases::started`]). The git states still
//! hold the write at such a root.
//!
//! A person-only write the CLI asked for (0.10) is stored the same way, with a `Confirmation` Queue item that takes
//! Accept and Dismiss ([`WriteGuard::confirm`]). It is never applied by [`WriteGuard::drain`]: only a person's Accept
//! takes it out ([`WriteGuard::take_confirmation`]), and the daemon then writes it as a person.
//!
//! The daemon owns what a write *is* (the source's `apply`, the events). This module owns when it may run: the caller
//! gives [`WriteGuard::drain`] the function that applies a stored write.

use crate::auth::QueueConfirmation;
use crate::checkout::root_busy;
use crate::queue::Queue;
use collide_core::change::Actor;
use collide_core::owner::{AnswerKind, QueueItem, QueueKind, WriteQueued};
use collide_core::provider::ApplyParams;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

fn locked<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The path a checkout is kept under: symbolic links resolved when the folder exists, as given when it does not.
pub fn canonical(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

// ---- the line lock ------------------------------------------------------------------------------------------------

/// One lock for each checkout, made when it is first asked for and kept for the daemon's life.
#[derive(Default)]
pub struct LineLocks {
    held: Mutex<HashMap<PathBuf, Arc<Mutex<()>>>>,
}

impl LineLocks {
    pub fn new() -> LineLocks {
        LineLocks::default()
    }

    /// Runs `body` while holding the lock of `root`, which waits for the holder before it. The lock is not re-entrant:
    /// a `body` that asks for the same root again waits for itself for good.
    pub fn hold<T>(&self, root: &Path, body: impl FnOnce() -> T) -> T {
        let lock = locked(&self.held)
            .entry(canonical(root))
            .or_default()
            .clone();
        let _held = locked(&lock);
        body()
    }
}

// ---- the phases that are live -------------------------------------------------------------------------------------

/// One live phase: the run, and the checkout it works in.
struct LivePhase {
    run: String,
    checkout: PathBuf,
    at_root: bool,
}

/// The checkouts where a phase is live. [`crate::file_run`] registers a run's phase when it starts and takes it out
/// when it ends, and registers again the runs a restarted daemon finds in the store. A paused phase stays registered:
/// its agent comes back to the same checkout.
#[derive(Default)]
pub struct LivePhases {
    phases: Mutex<Vec<LivePhase>>,
    released: AtomicBool,
}

impl LivePhases {
    pub fn new() -> LivePhases {
        LivePhases::default()
    }

    /// Registers the phase of `run` as live in `checkout`. `at_root` is for a phase that started at the root of a line,
    /// which a write there does not wait for. A run registered again is moved, not added.
    pub fn started(&self, run: &str, checkout: &Path, at_root: bool) {
        let mut phases = locked(&self.phases);
        phases.retain(|phase| phase.run != run);
        phases.push(LivePhase {
            run: run.to_string(),
            checkout: canonical(checkout),
            at_root,
        });
    }

    /// Takes the phase of `run` out, and says so to whoever waits on a release ([`LivePhases::take_released`]). A run
    /// that is not registered changes nothing.
    pub fn ended(&self, run: &str) {
        let mut phases = locked(&self.phases);
        let before = phases.len();
        phases.retain(|phase| phase.run != run);
        if phases.len() != before {
            self.released.store(true, Ordering::SeqCst);
        }
    }

    /// The run of a live phase in `checkout` that a write must wait for: one that did not start at the root.
    pub fn blocking(&self, checkout: &Path) -> Option<String> {
        let checkout = canonical(checkout);
        locked(&self.phases)
            .iter()
            .find(|phase| phase.checkout == checkout && !phase.at_root)
            .map(|phase| phase.run.clone())
    }

    /// Whether a phase was taken out since the last call, and forgets it: the daemon's writer looks again then, at
    /// once, and not at its next poll.
    pub fn take_released(&self) -> bool {
        self.released.swap(false, Ordering::SeqCst)
    }
}

// ---- the queued write ---------------------------------------------------------------------------------------------

/// A write the daemon stored, with all it needs to be applied later as the person who asked would have applied it. The
/// store keeps it as the payload of its Queue item. It is the daemon's own text, not a wire type.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QueuedWrite {
    pub board: String,
    pub source: String,
    pub apply: ApplyParams,
    /// The checkout the write goes to, as [`canonical`] gives it: the key the line lock and the queue use.
    pub checkout: PathBuf,
    /// Who asked, so the event of the write names them when it is applied.
    pub actor: Actor,
}

/// How a stored write ended when it was applied.
#[derive(Debug, PartialEq, Eq)]
pub enum Applied {
    /// It was written, and its events are recorded.
    Done,
    /// It was not written: the source said conflict or refused, or could not be asked. The words are Collide's.
    Problem(String),
}

// ---- the guard ----------------------------------------------------------------------------------------------------

/// What a [`WriteGuard`] is made of.
pub struct WriteGuardConfig {
    pub queue: Arc<Queue>,
    pub live: Arc<LivePhases>,
    /// Added to the environment of git, as for the daemon's other git commands.
    pub git_env: Vec<(String, String)>,
    /// The wall clock in milliseconds, for the time an item began waiting.
    pub now_ms: Arc<dyn Fn() -> u64 + Send + Sync>,
    pub log: Arc<dyn Fn(&str) + Send + Sync>,
}

/// The locks, the live phases and the queued writes of one daemon.
pub struct WriteGuard {
    config: WriteGuardConfig,
    locks: LineLocks,
    counter: AtomicU64,
}

impl WriteGuard {
    pub fn new(config: WriteGuardConfig) -> WriteGuard {
        // The numbers go on after the ones the store has: a restart under a stopped clock makes no id twice.
        let next = config
            .queue
            .list(None)
            .iter()
            .filter_map(|item| {
                item.id
                    .strip_prefix("write-")
                    .or_else(|| item.id.strip_prefix("confirm-"))?
                    .split('-')
                    .nth(1)?
                    .parse::<u64>()
                    .ok()
            })
            .max()
            .map_or(0, |last| last + 1);
        WriteGuard {
            config,
            locks: LineLocks::new(),
            counter: AtomicU64::new(next),
        }
    }

    pub fn live(&self) -> &Arc<LivePhases> {
        &self.config.live
    }

    /// Runs `body` holding the line lock of `checkout` ([`LineLocks::hold`]).
    pub fn hold<T>(&self, checkout: &Path, body: impl FnOnce() -> T) -> T {
        self.locks.hold(checkout, body)
    }

    /// Why a write to `checkout` cannot be applied now, in Collide's words, or `None` when it can. An `Err` is git
    /// failing to say, which is not the same as the checkout being free.
    pub fn blocked(&self, checkout: &Path) -> Result<Option<String>, String> {
        let shown = checkout.display();
        match root_busy(&self.config.git_env, checkout) {
            Ok(Some(what)) => return Ok(Some(format!("{what} is in progress in {shown}"))),
            Ok(None) => {}
            Err(error) => {
                return Err(format!(
                    "git could not say whether {shown} is busy, so the write was not made: {error}"
                ));
            }
        }
        Ok(self
            .config
            .live
            .blocking(checkout)
            .map(|run| format!("a phase of run {run} is running in {shown}")))
    }

    /// Stores `write`, with a `Conflict` Queue item that says `reason`, and returns the item. The caller holds the line
    /// lock. The item takes Retry and Dismiss.
    pub fn enqueue(&self, write: &QueuedWrite, reason: &str) -> Result<WriteQueued, String> {
        let n = self.counter.fetch_add(1, Ordering::SeqCst);
        let since_ms = (self.config.now_ms)();
        let id = format!("write-{since_ms}-{n}");
        let payload = serde_json::to_string(write)
            .map_err(|error| format!("the write could not be kept: {error}"))?;
        let item = QueueItem {
            id: id.clone(),
            kind: QueueKind::Conflict,
            run: None,
            phase: None,
            task: task_of(write),
            since_ms,
            title: waiting_title(write, reason),
            permission: None,
            untrusted_input: false,
            cache: None,
            reset_ms: None,
            show: Vec::new(),
            answers: vec![AnswerKind::Retry, AnswerKind::Dismiss],
        };
        self.config
            .queue
            .push_with_payload(item, &payload)
            .map_err(|error| format!("the write was not queued: {error}"))?;
        Ok(WriteQueued {
            item: id,
            reason: reason.to_string(),
        })
    }

    /// Stores `write` with a `Confirmation` Queue item, and returns the item. Nothing is written until a person
    /// answers Accept. The title is the confirmation's own, in Collide's words. The caller holds no lock: the write is
    /// only kept here.
    pub fn confirm(
        &self,
        write: &QueuedWrite,
        confirmation: &QueueConfirmation,
    ) -> Result<WriteQueued, String> {
        let n = self.counter.fetch_add(1, Ordering::SeqCst);
        let since_ms = (self.config.now_ms)();
        let id = format!("confirm-{since_ms}-{n}");
        let payload = serde_json::to_string(write)
            .map_err(|error| format!("the write could not be kept: {error}"))?;
        let item = QueueItem {
            id: id.clone(),
            kind: QueueKind::Confirmation,
            run: None,
            phase: None,
            task: task_of(write),
            since_ms,
            title: confirmation.title.clone(),
            permission: None,
            untrusted_input: false,
            cache: None,
            reset_ms: None,
            show: Vec::new(),
            answers: vec![AnswerKind::Accept, AnswerKind::Dismiss],
        };
        self.config
            .queue
            .push_with_payload(item, &payload)
            .map_err(|error| format!("the confirmation was not kept: {error}"))?;
        Ok(WriteQueued {
            item: id,
            reason: confirmation.title.clone(),
        })
    }

    /// Whether `item` is a stored write that waits for its checkout.
    pub fn is_queued_write(&self, item: &str) -> bool {
        !self.is_confirmation(item) && self.config.queue.payload(item).is_some()
    }

    /// Whether `item` is a stored write that waits for a person's confirmation.
    pub fn is_confirmation(&self, item: &str) -> bool {
        self.config
            .queue
            .get(item)
            .is_some_and(|held| held.kind == QueueKind::Confirmation)
            && self.config.queue.payload(item).is_some()
    }

    /// Takes the confirmed write out of the Queue, with its item, for a person's Accept. `None` when it was gone
    /// already (another answer took it first), so two answers never both write. A payload that cannot be read is
    /// logged and dropped with its item, as in [`WriteGuard::waiting`].
    pub fn take_confirmation(&self, item: &str) -> Option<QueuedWrite> {
        if !self.is_confirmation(item) {
            return None;
        }
        let payload = self.config.queue.payload(item)?;
        self.config.queue.take(item)?;
        match serde_json::from_str::<QueuedWrite>(&payload) {
            Ok(write) => Some(write),
            Err(error) => {
                (self.config.log)(&format!(
                    "queue item {item}: its stored write is not readable, so it was dropped: {error}"
                ));
                None
            }
        }
    }

    /// The checkout the stored write behind `item` goes to.
    pub fn checkout_of(&self, item: &str) -> Option<PathBuf> {
        self.waiting()
            .into_iter()
            .find(|(id, _)| id == item)
            .map(|(_, write)| write.checkout)
    }

    /// The checkouts that have a stored write waiting, in the order of their oldest write.
    pub fn checkouts_waiting(&self) -> Vec<PathBuf> {
        let mut found: Vec<PathBuf> = Vec::new();
        for (_, write) in self.waiting() {
            if !found.contains(&write.checkout) {
                found.push(write.checkout);
            }
        }
        found
    }

    /// Drops the stored write behind `item` with its Queue item. `false` when it was gone already: another answer or
    /// the writer took it first.
    pub fn dismiss(&self, item: &str) -> bool {
        self.config.queue.take(item).is_some()
    }

    /// The stored writes, oldest first. A payload that cannot be read is logged and its item is taken out: nothing
    /// could ever apply it.
    fn waiting(&self) -> Vec<(String, QueuedWrite)> {
        let mut writes = Vec::new();
        for (id, payload) in self.config.queue.payloads() {
            if self
                .config
                .queue
                .get(&id)
                .is_some_and(|held| held.kind == QueueKind::Confirmation)
            {
                continue;
            }
            match serde_json::from_str::<QueuedWrite>(&payload) {
                Ok(write) => writes.push((id, write)),
                Err(error) => {
                    (self.config.log)(&format!(
                        "queue item {id}: its stored write is not readable, so it was dropped: {error}"
                    ));
                    self.config.queue.take(&id);
                }
            }
        }
        writes
    }

    /// Applies the stored writes of `checkout`, oldest first, while it is free; stops at the first it must still wait
    /// for and tells that item why. The caller holds the line lock of `checkout`. Each write is taken out of the Queue
    /// before `apply` runs, so it is never applied twice; one that comes back as a [`Applied::Problem`] becomes a
    /// `Conflict` item that can only be dismissed.
    pub fn drain(&self, checkout: &Path, apply: &mut dyn FnMut(QueuedWrite) -> Applied) {
        let checkout = canonical(checkout);
        loop {
            let next = self
                .waiting()
                .into_iter()
                .find(|(_, write)| write.checkout == checkout);
            let Some((id, write)) = next else {
                return;
            };
            match self.blocked(&checkout) {
                Err(message) => {
                    (self.config.log)(&message);
                    return;
                }
                Ok(Some(reason)) => {
                    self.say_why(&id, &write, &reason);
                    return;
                }
                Ok(None) => {}
            }
            if self.config.queue.take(&id).is_none() {
                continue;
            }
            if let Applied::Problem(message) = apply(write.clone()) {
                self.give_up(&write, &message);
            }
        }
    }

    /// Keeps the waiting item's title true: the reason can change while it waits (the rebase ended, a phase runs).
    fn say_why(&self, id: &str, write: &QueuedWrite, reason: &str) {
        let Some(mut item) = self.config.queue.get(id) else {
            return;
        };
        let title = waiting_title(write, reason);
        if item.title != title {
            item.title = title;
            self.config.queue.push(item);
        }
    }

    /// A stored write that could not be applied: the person is told in a `Conflict` item, and the write is gone.
    pub fn give_up(&self, write: &QueuedWrite, message: &str) {
        let n = self.counter.fetch_add(1, Ordering::SeqCst);
        let since_ms = (self.config.now_ms)();
        self.config.queue.push(QueueItem {
            id: format!("write-{since_ms}-{n}-undone"),
            kind: QueueKind::Conflict,
            run: None,
            phase: None,
            task: task_of(write),
            since_ms,
            title: format!("A {} was not applied: {message}", describe(write)),
            permission: None,
            untrusted_input: false,
            cache: None,
            reset_ms: None,
            show: Vec::new(),
            answers: vec![AnswerKind::Dismiss],
        });
    }
}

/// The task a write names, as its key: `source:id`.
fn task_of(write: &QueuedWrite) -> Option<String> {
    write
        .apply
        .id
        .as_ref()
        .map(|id| format!("{}:{id}", write.source))
}

/// What the write is, in words that carry no task text: `setStatus of plan:ts-0001`, or `create in source plan`.
fn describe(write: &QueuedWrite) -> String {
    match task_of(write) {
        Some(key) => format!("`{}` of {key}", write.apply.op),
        None => format!("`{}` in source {}", write.apply.op, write.source),
    }
}

fn waiting_title(write: &QueuedWrite, reason: &str) -> String {
    format!("A {} waits: {reason}", describe(write))
}
