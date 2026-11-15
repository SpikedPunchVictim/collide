//! The base environment (0.2, The base environment; measured S4). An app started from Finder or launchd gets a
//! minimal `PATH` and no `LANG`, so the daemon captures the login shell's environment, and everything Collide starts
//! gets that environment minus a list of variables.
//!
//! - **The capture** runs `$SHELL -l -i -c` and has it print `env -0` between nonce markers, with stdin from
//!   `/dev/null`, in a session of its own that is killed after reading, under [`CAPTURE_TIMEOUT`], with output capped
//!   at [`CAPTURE_CAP_BYTES`]. On failure the daemon falls back to the minimal environment and logs the shell and the
//!   reason ([`EnvSource::Minimal`]). `LANG` is set when missing ([`FALLBACK_LANG`]).
//! - **It is captured again when the app starts** (R5), and each new phase uses the newest capture. A phase already
//!   running keeps the environment it started with, which is why [`BaseEnvironment`] is a value a phase copies and not
//!   a view of the daemon's current one.
//! - **What a child does not get** is the adapter's [`EnvStrip`] (0.3), [`ALWAYS_STRIPPED`] and nothing else.
//!
//! | tag | function | what it does |
//! |---|---|---|
//! | `daemon: base environment` | [`BaseEnvironment::capture`], [`BaseEnvironment::for_child`] | the capture, and the environment of one child |

use crate::clock::{Clocks, Reading};
use collide_core::adapter::EnvStrip;
use std::collections::BTreeMap;
use std::time::Duration;

/// The most output the capture reads, in bytes (0.2: 1 MiB).
pub const CAPTURE_CAP_BYTES: usize = 1024 * 1024;

/// How long the capture may take before its session is killed. The plan says "under a timeout" and gives no number;
/// a login shell that loads a heavy profile takes seconds.
pub const CAPTURE_TIMEOUT: Duration = Duration::from_secs(10);

/// The variables removed from every child whatever the adapter says (0.2): terminal session ids, which name a
/// terminal the child is not in, and launchd's service markers.
pub const ALWAYS_STRIPPED: [&str; 4] = [
    "TERM_SESSION_ID",
    "ITERM_SESSION_ID",
    "XPC_SERVICE_NAME",
    "XPC_FLAGS",
];

/// The `LANG` set when the captured environment has none. The plan does not name a value.
pub const FALLBACK_LANG: &str = "en_US.UTF-8";

/// Where an environment came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnvSource {
    /// The login shell `shell` printed it.
    LoginShell { shell: String },
    /// The capture failed, so this is the minimal environment. `shell` is the shell that was tried, when there was
    /// one, and `reason` is why it failed, for the log.
    Minimal {
        shell: Option<String>,
        reason: String,
    },
}

/// One capture of the base environment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaseEnvironment {
    pub vars: BTreeMap<String, String>,
    pub source: EnvSource,
    /// The wall-clock reading when the capture finished, so the newest of two can be told.
    pub captured_at: Reading,
}

impl BaseEnvironment {
    /// Captures the environment of `shell`'s login session. It does not fail: a capture that fails returns
    /// [`EnvSource::Minimal`] with the reason.
    pub fn capture(shell: &str, clocks: &dyn Clocks) -> BaseEnvironment {
        let _ = (shell, clocks);
        todo!("daemon: base environment")
    }

    /// The environment of one child: this capture, minus `strip` and [`ALWAYS_STRIPPED`], plus `extra` (the adapter's
    /// own variables, and the run's `COLLIDE_RUN` and `COLLIDE_RUN_START`).
    pub fn for_child(
        &self,
        strip: &EnvStrip,
        extra: &BTreeMap<String, String>,
    ) -> BTreeMap<String, String> {
        let _ = (strip, extra);
        todo!("daemon: base environment")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_capture_is_capped_at_one_mebibyte() {
        assert_eq!(CAPTURE_CAP_BYTES, 1_048_576);
    }

    #[test]
    fn terminal_session_ids_and_xpc_markers_are_always_stripped() {
        for name in [
            "TERM_SESSION_ID",
            "ITERM_SESSION_ID",
            "XPC_SERVICE_NAME",
            "XPC_FLAGS",
        ] {
            assert!(ALWAYS_STRIPPED.contains(&name), "{name}");
        }
    }

    #[test]
    #[should_panic(expected = "daemon: base environment")]
    fn the_capture_is_a_stub_tagged_for_stage_3a() {
        struct Never;
        impl Clocks for Never {
            fn read(&self, _: crate::clock::Clock) -> Reading {
                unreachable!("the stub panics first")
            }
        }
        BaseEnvironment::capture("/bin/zsh", &Never);
    }
}
