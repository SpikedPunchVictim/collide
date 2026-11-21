//! The `queue.*` methods: every decision waiting on the user (0.11, Queue), its context view, and the answer. A
//! finished run to review is a Queue item too, so accept, discard and re-prompt are answers here and not methods of
//! their own.
//!
//! A person-only answer from a run's agent is refused with [`crate::owner::code::NOT_PERMITTED`] (Stage 3a's
//! enforcement; ts-0033-zj).

use crate::agent::{Money, PermissionReply, PermissionRequest};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// What a Queue item waits for: the list of 0.11, Queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueueKind {
    /// A permission request: a command, an edit or any other tool call.
    Permission,
    /// An `ask_human` of a run: its cause is in the title.
    AskHuman,
    /// A `human` phase with its `show:` list.
    HumanPhase,
    BudgetExceeded,
    /// A usage limit, with its reset time.
    UsageLimit,
    /// A prompt blocked by a user hook, with the hook's message.
    BlockedPrompt,
    /// A take-over or New session past its phase budget: a warning, and nothing is stopped.
    BudgetWarning,
    /// An `unverified` gate or `wait` result: retry, proceed or stop.
    Unverified,
    FailedGate,
    /// A write conflict, a merge conflict found before `integrate`, a queued write, a busy line root or unrecorded
    /// edits.
    Conflict,
    /// An integrate or take-over waiting because its range edits the agent tool's config, or a run waiting because an
    /// integrate changed a working-copy pack it loads.
    Acknowledge,
    ObserverDisabled,
    /// A finished run to review.
    Review,
    /// A step whose outcome is unknown after a restart (0.2): a person says.
    OutcomeUnknown,
    /// A run paused because free space is under the floor (0.2, Durability): free some space, then resume, or stop.
    DiskFull,
    /// An open question of the planning pack (`Decisions needed`, 0.11).
    Question,
    /// A phase the agent tool's setup stopped: a version outside the tested range whose smoke check failed, or settings
    /// the tool ignored (0.3). The title says which, in Collide's words.
    AgentSetup,
    /// A person-only write the CLI asked for: a person answers Accept or Dismiss.
    Confirmation,
}

/// What an answer is, as a tag. A Queue item lists the kinds it takes in `answers`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AnswerKind {
    Permission,
    Text,
    Proceed,
    Stop,
    Retry,
    RaiseCap,
    Accept,
    Discard,
    Reprompt,
    Acknowledge,
    Resume,
    Dismiss,
}

/// How long a live phase's prompt cache stays warm and what answering later costs (0.3, 0.11): "warm for 12 min,
/// then $1.30 to resume". It is a duration at the time of the answer, from the continuous clock (0.2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CacheWindow {
    pub warm_secs_left: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resume_cost: Option<Money>,
}

/// One decision waiting on the user. `title` is Collide's own words: task titles are not in it unless the setting
/// allows (0.9, 0.11). `since_ms` is the wall clock when it began blocking work, and the list is ordered by it,
/// oldest first. `permission` is set for a [`QueueKind::Permission`] item and carries the agent's request as it came;
/// `untrusted_input` says its arguments came from text Collide does not trust, and the UI flags them (0.9).
/// `reset_ms` is a usage limit's reset time on the wall clock. `show` is the phase's `show:` list. `answers` are the
/// kinds of answer the item takes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct QueueItem {
    pub id: String,
    pub kind: QueueKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phase: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,
    pub since_ms: u64,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permission: Option<PermissionRequest>,
    #[serde(default)]
    pub untrusted_input: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache: Option<CacheWindow>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reset_ms: Option<u64>,
    #[serde(default)]
    pub show: Vec<String>,
    #[serde(default)]
    pub answers: Vec<AnswerKind>,
}

/// `queue.list` params. With no `board` the items of every board, because a decision waits on the person and not on
/// a board. `kinds` keeps the listed kinds, or all of them when empty.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct QueueListParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub board: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run: Option<String>,
    #[serde(default)]
    pub kinds: Vec<QueueKind>,
}

/// `queue.list` result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct QueueListResult {
    pub items: Vec<QueueItem>,
}

/// `queue.context` params: Why is this asked? (`i`, 0.11). Its result is [`crate::context::ContextRecord`], built
/// from what the owner holds with no model call.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct QueueContextParams {
    pub item: String,
}

/// An answer to a Queue item. The keys `a`, `d` and `r` of 0.11's triage table are `permission` (allow or deny),
/// `proceed`, `stop`, `retry`, `raise_cap`, `accept`, `discard` and `reprompt`. `a` is never "always allow": the
/// permission replies are [`PermissionReply`], which has no such variant (0.3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum QueueAnswer {
    Permission {
        reply: PermissionReply,
    },
    /// The words that answer an `ask_human`, a `human` phase or a question.
    Text {
        text: String,
    },
    Proceed {},
    Stop {},
    Retry {},
    /// Raise the budget that was exceeded by `amount`.
    RaiseCap {
        amount: Money,
    },
    /// Accept a finished run: the integrate of 0.4 runs.
    Accept {},
    Discard {},
    Reprompt {
        prompt: String,
    },
    Acknowledge {},
    /// Resume a paused phase now, instead of at the limit's reset time.
    Resume {},
    /// Clear a warning.
    Dismiss {},
}

/// `queue.answer` params. The item takes the kinds its `answers` lists, and an answer of another kind is refused with
/// invalid params.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct QueueAnswerParams {
    pub item: String,
    pub answer: QueueAnswer,
}

/// What the answer did. An accept that cannot merge answers `queued` with the new item (a conflict goes to the Queue
/// and no checkout changes, 9a), `integrating` is a pull request that is not merged yet (0.4), and `refused` is an
/// answer the owner declined, in Collide's words. A discard that went through is `done`, or `discarded` when there is
/// something to tell the person (ts-0160-th: the repository was gone, so the branch went with it).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "outcome", rename_all = "snake_case", deny_unknown_fields)]
pub enum QueueAnswerResult {
    Done {},
    /// A discard that deleted the work and has a `note` for the person, in Collide's words.
    Discarded {
        note: String,
    },
    Merged {
        commit: String,
    },
    Integrating {
        url: String,
        head: String,
    },
    Queued {
        item: String,
    },
    Refused {
        message: String,
    },
}

/// The params of the `queue.changed` notification: the Queue's size after a change and the items that came and went,
/// so a badge or a menu needs no other message. A client lists the Queue with `queue.list` for the rest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct QueueChanged {
    pub count: u32,
    #[serde(default)]
    pub added: Vec<String>,
    #[serde(default)]
    pub removed: Vec<String>,
}
