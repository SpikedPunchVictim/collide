//! The state of a run, a phase and a workspace (0.8, Run, Phase, Workspace). These are the types only. The
//! transitions between them are [`crate::transition`]'s, and `integrating` is entered by the `integrate` answer
//! `pending {url, head}` (0.4, [`crate::run_rules::apply_integrate_answer`]).
//!
//! A state that carries data is a tagged object with a `state` key in `snake_case`, such as
//! `{"state":"paused","cause":"disk"}`. A workspace state carries none, so it is a plain string. A state with no
//! fields is written `Running {}`, never `Running` (0.6, see [`crate::schema`]).
//!
//! The phase *kinds* (`agent`, `fanout`, `script`, `wait`, `human`) are [`crate::workflow::PhaseKind`], not
//! repeated here.

use crate::non_empty::NonEmpty;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Why a run or a phase is paused (0.2, 0.3, 0.8). `ask_human` waits in the Queue, `usage_limit` waits for the
/// account's reset time, and `disk` is the free-space floor: nothing is started until space comes back (0.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PauseCause {
    AskHuman,
    UsageLimit,
    Disk,
}

/// Why a phase failed when the cause is known. A phase that ends with no `result` on a full volume fails with cause
/// `disk`, not `lost` (0.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FailCause {
    Disk,
}

/// Where a run is (0.8, Run). It ends `ended(<outcome>)`, `failed` or `stopped`; `outcome` is the `end` token the run
/// reached (0.7, Outcomes and terminal states). `integrating` is a run whose `integrate` answered `pending`, so the
/// pull request is not merged yet (0.4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum RunState {
    Running {},
    Paused { cause: PauseCause },
    Integrating {},
    Ended { outcome: NonEmpty },
    Failed {},
    Stopped {},
}

/// Where a phase is (0.8, Phase). A shim that dies without an `exit.json` makes its phase `lost`, whatever the agent's
/// stream said (0.2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum PhaseState {
    Pending {},
    Running {},
    Paused {
        cause: PauseCause,
    },
    Ended {},
    Failed {
        #[serde(default)]
        cause: Option<FailCause>,
    },
    Stopped {},
    Lost {},
}

/// Where a workspace is (0.8, Workspace): `provisioning → ready → in_use → idle → removable → removed`, plus
/// `failed`, which resumes through the `resume` hook.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceState {
    Provisioning,
    Ready,
    InUse,
    Idle,
    Removable,
    Removed,
    Failed,
}
