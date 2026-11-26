//! Whether `claude` is logged in (ts-0192-es; 0.11, First run works with no pack and no tasks): the probe the owner
//! recorded in `ts-0185-ge`. `claude auth status` sends no prompt and spends nothing. Logged in, it exits 0 and prints
//! JSON with `"loggedIn": true`; logged out, it exits 1 with `"loggedIn": false`.
//!
//! The JSON also names the account (an email address and an organization). Only the `loggedIn` field is read, and
//! nothing of the output is kept: not in the answer, a message or a log. An answer that does not parse, or an exit code
//! that disagrees with the field, is [`Login::CouldNotCheck`], never "logged out".
//!
//! The probe is a trait so a daemon can be given another: [`AuthStatus`] runs the command, [`NotChecked`] never starts
//! `claude` (what a test daemon has, so no test asks a real `claude` anything).

use crate::bounded_run::{cut_message, run_bounded_in_group};
use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

/// What the probe found. `CouldNotCheck` carries a reason that is safe to show: it never holds the probe's output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Login {
    LoggedIn,
    LoggedOut,
    CouldNotCheck(String),
}

/// Asks whether `binary` is logged in, with exactly `env` and no longer than `timeout`.
pub trait LoginProbe: Send + Sync {
    fn probe(&self, binary: &Path, env: &BTreeMap<String, String>, timeout: Duration) -> Login;
}

/// `claude auth status`, in a process group of its own so a timeout ends the runtime it started too.
pub struct AuthStatus;

impl LoginProbe for AuthStatus {
    fn probe(&self, binary: &Path, env: &BTreeMap<String, String>, timeout: Duration) -> Login {
        let mut command = Command::new(binary);
        command.args(["auth", "status"]).env_clear().envs(env);
        match run_bounded_in_group(command, None, timeout) {
            Err(reason) => {
                Login::CouldNotCheck(format!("`claude auth status` did not answer: {reason}"))
            }
            Ok(run) if run.stdout_cut => Login::CouldNotCheck(cut_message("stdout")),
            Ok(run) => parse_status(run.code, &run.stdout),
        }
    }
}

/// The probe that never runs `claude`: it says "could not check". [`crate::daemon::DaemonOptions::new`] gives it, and
/// only the daemon's own binary gives [`AuthStatus`] ([`probe_from_env`]).
pub struct NotChecked;

impl LoginProbe for NotChecked {
    fn probe(&self, _: &Path, _: &BTreeMap<String, String>, _: Duration) -> Login {
        Login::CouldNotCheck(
            "this daemon does not ask `claude` whether it is logged in".to_string(),
        )
    }
}

/// What `collide-daemon` uses, from `COLLIDE_DAEMON_ROOT` as it has it: [`AuthStatus`] for a daemon of the user's own,
/// and [`NotChecked`] for one under an overridden root, which is a test's or a developer's and asks no real `claude`
/// (the same rule as [`crate::claude_versions::published_from_env`]).
pub fn probe_from_env(root: Option<&str>) -> std::sync::Arc<dyn LoginProbe> {
    match root {
        Some(_) => std::sync::Arc::new(NotChecked),
        None => std::sync::Arc::new(AuthStatus),
    }
}

/// Reads the probe's exit code and output. Logged in is exit 0 with `true`, and logged out is exit 1 with `false`:
/// any other pair, and any output that is not a JSON object with a boolean `loggedIn`, is not an answer. Nothing but
/// that one field is read out of the object (the account's email address and organization are in it too).
pub fn parse_status(code: Option<i32>, stdout: &[u8]) -> Login {
    let logged_in = serde_json::from_slice::<serde_json::Value>(stdout)
        .ok()
        .and_then(|answer| answer.get("loggedIn")?.as_bool());
    let Some(logged_in) = logged_in else {
        return Login::CouldNotCheck(
            "its answer was not the JSON with a `loggedIn` field it gives".to_string(),
        );
    };
    match (code, logged_in) {
        (Some(0), true) => Login::LoggedIn,
        (Some(1), false) => Login::LoggedOut,
        _ => Login::CouldNotCheck("its exit code and its `loggedIn` field disagree".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn could_not_check(login: &Login) -> bool {
        matches!(login, Login::CouldNotCheck(_))
    }

    #[test]
    fn exit_0_with_logged_in_true_is_logged_in() {
        let output =
            br#"{"loggedIn": true, "email": "tester@example.invalid", "orgName": "Example Org"}"#;
        assert_eq!(parse_status(Some(0), output), Login::LoggedIn);
    }

    #[test]
    fn exit_1_with_logged_in_false_is_logged_out() {
        assert_eq!(
            parse_status(Some(1), br#"{"loggedIn": false}"#),
            Login::LoggedOut
        );
    }

    #[test]
    fn a_disagreeing_exit_code_is_could_not_check() {
        for (code, output) in [
            (Some(0), &br#"{"loggedIn": false}"#[..]),
            (Some(1), &br#"{"loggedIn": true}"#[..]),
            (Some(2), &br#"{"loggedIn": true}"#[..]),
            (None, &br#"{"loggedIn": false}"#[..]),
        ] {
            assert!(
                could_not_check(&parse_status(code, output)),
                "{code:?} {output:?}"
            );
        }
    }

    #[test]
    fn output_that_is_not_the_json_is_could_not_check() {
        for output in [
            &b""[..],
            b"logged in as tester@example.invalid",
            b"{}",
            br#"{"loggedIn": "yes"}"#,
            br#"{"loggedIn": null}"#,
            b"[true]",
            br#"{"loggedIn": true"#,
        ] {
            assert!(
                could_not_check(&parse_status(Some(0), output)),
                "{output:?}"
            );
        }
    }

    #[test]
    fn a_reason_never_holds_the_output() {
        let output =
            br#"{"loggedIn": false, "email": "tester@example.invalid", "orgName": "Example Org"}"#;
        let Login::CouldNotCheck(reason) = parse_status(Some(0), output) else {
            panic!("a disagreeing answer is not an answer");
        };
        assert!(!reason.contains("example"), "{reason}");
        let Login::CouldNotCheck(reason) = parse_status(Some(0), b"tester@example.invalid") else {
            panic!("text is not an answer");
        };
        assert!(!reason.contains("example"), "{reason}");
    }

    #[test]
    fn a_daemon_under_an_overridden_root_never_asks_claude() {
        let probe = probe_from_env(Some("/tmp/x"));
        let found = probe.probe(
            Path::new("/nonexistent/claude"),
            &BTreeMap::new(),
            Duration::from_secs(1),
        );
        let Login::CouldNotCheck(reason) = found else {
            panic!("a daemon that does not ask gives no answer");
        };
        assert!(reason.contains("does not ask"), "{reason}");
    }

    #[test]
    fn a_daemon_with_no_root_override_runs_auth_status() {
        // The real probe on a program that is not there: it tries to start it, and cannot.
        let probe = probe_from_env(None);
        let Login::CouldNotCheck(reason) = probe.probe(
            Path::new("/nonexistent/claude"),
            &BTreeMap::new(),
            Duration::from_secs(1),
        ) else {
            panic!("a program that is not there gives no answer");
        };
        assert!(reason.contains("auth status"), "{reason}");
    }
}
