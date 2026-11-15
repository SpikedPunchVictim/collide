//! Spans (0.13, Model). A run is a trace, and a span is one thing inside it: a phase, an agent turn, a tool call, a
//! hook, a gate, a provisioning step, a subprocess or a wait. The shape follows OpenTelemetry's.
//!
//! The rules are in the type, so a span that breaks one does not parse:
//! - every span names its `kind`, which is the tag;
//! - only the root, `run`, has no parent. Every other kind requires `parent`;
//! - a wait on the user is its own kind, `user_wait`, and requires `queue_item`, the id of the Queue item it waited
//!   on (0.13, S6). No other kind accepts `queue_item`.
//!
//! Attributes and times are not here yet: the metrics stages add them. Whether a set of spans forms one tree (a
//! parent that exists, one root, no cycle) is a check over many spans, not over one.

use crate::non_empty::NonEmpty;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// One span. `id` is unique in its trace and `parent` is the `id` of the span that owns this one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Span {
    /// The root: the trace is the run. It is the only span with no parent.
    Run {
        id: NonEmpty,
    },
    Phase {
        id: NonEmpty,
        parent: NonEmpty,
    },
    AgentTurn {
        id: NonEmpty,
        parent: NonEmpty,
    },
    ToolCall {
        id: NonEmpty,
        parent: NonEmpty,
    },
    Hook {
        id: NonEmpty,
        parent: NonEmpty,
    },
    Gate {
        id: NonEmpty,
        parent: NonEmpty,
    },
    Provisioning {
        id: NonEmpty,
        parent: NonEmpty,
    },
    Subprocess {
        id: NonEmpty,
        parent: NonEmpty,
    },
    /// A wait that is not on the user: a `wait` phase, the concurrency cap, the line lock, a usage limit, machine
    /// sleep (0.13, Sources).
    Wait {
        id: NonEmpty,
        parent: NonEmpty,
    },
    /// A wait on the user. `queue_item` is the Queue item it waited on, so "where did this run wait" is answerable
    /// from the spans.
    UserWait {
        id: NonEmpty,
        parent: NonEmpty,
        queue_item: NonEmpty,
    },
}

impl Span {
    pub fn id(&self) -> &NonEmpty {
        match self {
            Span::Run { id }
            | Span::Phase { id, .. }
            | Span::AgentTurn { id, .. }
            | Span::ToolCall { id, .. }
            | Span::Hook { id, .. }
            | Span::Gate { id, .. }
            | Span::Provisioning { id, .. }
            | Span::Subprocess { id, .. }
            | Span::Wait { id, .. }
            | Span::UserWait { id, .. } => id,
        }
    }

    /// The owning span's id; `None` only for the root.
    pub fn parent(&self) -> Option<&NonEmpty> {
        match self {
            Span::Run { .. } => None,
            Span::Phase { parent, .. }
            | Span::AgentTurn { parent, .. }
            | Span::ToolCall { parent, .. }
            | Span::Hook { parent, .. }
            | Span::Gate { parent, .. }
            | Span::Provisioning { parent, .. }
            | Span::Subprocess { parent, .. }
            | Span::Wait { parent, .. }
            | Span::UserWait { parent, .. } => Some(parent),
        }
    }

    /// The Queue item a `user_wait` waited on.
    pub fn queue_item(&self) -> Option<&NonEmpty> {
        match self {
            Span::UserWait { queue_item, .. } => Some(queue_item),
            _ => None,
        }
    }
}
