//! The observer protocol (0.4, Observers). An observer is a long-lived process. Collide writes one JSON object per
//! line to its stdin, and the observer writes one acknowledgement per line to its stdout. Framing is
//! newline-delimited JSON, as for providers (see [`crate::provider`]).
//!
//! Delivery is at least once: after a restart, delivery resumes after the last acknowledgement, so an observer
//! dedupes by `seq`. The pack's `events:` filter is a key of an observer entry in `pack.yaml`, which `ts-0012-ua`
//! adds to `PackManifest`.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// One line on an observer's stdin. `seq` counts up in the event log. `payload` is any JSON value until
/// `ts-0018-dd` types each event of the catalogue.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ObserverEvent {
    pub seq: u64,
    pub event: String,
    pub payload: Value,
}

/// One line on an observer's stdout: the event with this `seq` has been handled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ObserverAck {
    pub seq: u64,
}
