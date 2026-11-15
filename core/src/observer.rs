//! The observer protocol (0.4, Observers). An observer is a long-lived process. Collide writes one JSON object per
//! line to its stdin, and the observer writes one acknowledgement per line to its stdout. Framing is
//! newline-delimited JSON, as for providers (see [`crate::provider`]).
//!
//! Delivery is at least once: after a restart, delivery resumes after the last acknowledgement, so an observer
//! dedupes by `seq`. The pack's `events:` filter is a key of an observer entry in `pack.yaml`, which `ts-0012-ua`
//! adds to `PackManifest`.

use crate::events::Event;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// One line on an observer's stdin: an [`Event`] of the catalogue, as it is in the event log. Its `seq` counts up
/// in the event log. It is a newtype so that the registry keeps the name `ObserverEvent`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct ObserverEvent(pub Event);

/// One line on an observer's stdout: the event with this `seq` has been handled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ObserverAck {
    pub seq: u64,
}
