//! The `daemon.*` methods: is the daemon there and well, the login-shell environment, and the event channel.

use crate::events::Event;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// `daemon.status` params: none.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DaemonStatusParams {}

/// Whether the store opened. An unreadable store is moved aside and reported, never overwritten (0.2, Durability).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum StoreHealth {
    Ok {},
    /// The store could not be read. It was moved to `moved_to` and a new one was made.
    Quarantined {
        moved_to: String,
        reason: String,
    },
}

/// `daemon.status` result. `uptime_secs` is on the uptime clock, which stops in sleep (0.2, Clocks). `warnings` are
/// the daemon's own words, such as the `setsid` start's warning that it may end with its launcher (0.2). `head_seq`
/// is the `seq` of the newest event in the log.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DaemonStatusResult {
    pub daemon_version: String,
    pub protocol: u32,
    pub pid: u32,
    pub started_ms: u64,
    pub uptime_secs: u64,
    pub head_seq: u64,
    pub live_runs: u32,
    pub store: StoreHealth,
    #[serde(default)]
    pub warnings: Vec<String>,
}

/// `daemon.recapture_environment` params: none. The app asks for it when it starts (0.2, The base environment).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DaemonRecaptureEnvironmentParams {}

/// What a capture of the login-shell environment did. No value of the environment is ever returned.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "outcome", rename_all = "snake_case", deny_unknown_fields)]
pub enum EnvCapture {
    Captured {
        shell: String,
        variables: u32,
    },
    /// The capture failed and the daemon uses the minimal environment (0.2).
    FellBack {
        shell: String,
        reason: String,
    },
}

/// `daemon.recapture_environment` result. A phase already running keeps the environment it started with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DaemonRecaptureEnvironmentResult {
    pub capture: EnvCapture,
}

/// `daemon.subscribe` params. `since` is the `seq` of the last event the client has. The daemon sends one
/// `daemon.event` notification for each later event, in order and with none missing, then the live ones, on this
/// connection only. With no `since` it sends only the live ones. Subscribing again replaces the earlier
/// subscription, and a closed connection ends it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DaemonSubscribeParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub since: Option<u64>,
}

/// `daemon.subscribe` result. `oldest_seq` is the oldest event the log still holds: a `since` below `oldest_seq - 1`
/// has a gap, and the client reloads its state instead of trusting the replay. `replayed` counts the notifications
/// sent before this answer. The daemon answers first and sends the replay after it, so a client reads the answer
/// before any `daemon.event` of it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DaemonSubscribeResult {
    pub head_seq: u64,
    pub oldest_seq: u64,
    pub replayed: u64,
}

/// The params of the `daemon.event` notification are an [`Event`], as the log holds it: `seq`, and the `actor` the
/// daemon recorded.
pub type DaemonEvent = Event;
