//! The first-run checks for `claude` (ts-0192-es; 0.11, First run works with no pack and no tasks; Stage 6a, No-setup
//! open). `board.open` and `board.checks` answer the `git_repo` check ([`crate::boards`]) and these two:
//!
//! - `claude_installed`: `claude` is found on the `PATH` of the base environment, with its version when it can be read.
//!   Not required. Missing, the fix is the install command, for a Copy button.
//! - `claude_logged_in`: the probe of [`crate::adapters::claude::auth_status`]. Not required. Logged out, the fix is to
//!   run `claude` and log in. A probe that did not answer in time, or whose answer made no sense, is "could not check":
//!   not a failure. Left out when `claude` is missing.
//!
//! Neither check is required, because a board opens without `claude`: only runs need it. The two reads that start
//! `claude` (its version, and the probe) run side by side, each for at most [`CHECK_TIMEOUT`], so the checks add at
//! most that to an open once the base environment is captured. The probe's output is never kept; see
//! [`crate::adapters::claude::auth_status`].

use crate::adapters::claude::PROGRAM;
use crate::adapters::claude::auth_status::{Login, LoginProbe};
use crate::claude_versions::ClaudeVersions;
use crate::file_run::find_on_path;
use collide_core::owner::{CheckId, FirstRunCheck};
use std::collections::BTreeMap;
use std::time::Duration;

/// The longest the checks wait on one `claude` process (its `--version`, or `auth status`). `claude` starts a runtime
/// and answers in about a second; a first start on a machine that checks each new program can take far longer, and
/// that says "could not check", not "logged out", so the person is never told something false. Five seconds is the most
/// an open waits for them; Check again asks once more.
pub const CHECK_TIMEOUT: Duration = Duration::from_secs(5);

/// What the install fix tells a person to run: Claude Code's own installer for macOS (the owner's choice, 2026-11-25).
/// Its setup page also lists npm, as an alternative.
pub const INSTALL_COMMAND: &str = "curl -fsSL https://claude.ai/install.sh | bash";

/// What the checks ask of `claude`: the daemon's cache of versions, its login probe, and the wait on each.
pub struct ClaudeProbes<'a> {
    pub versions: &'a ClaudeVersions,
    pub login: &'a dyn LoginProbe,
    pub timeout: Duration,
}

/// The `claude_installed` check and, when `claude` is found, the `claude_logged_in` check. `env` is the environment a
/// child of the daemon gets ([`crate::base_env::BaseEnvironment::for_child`]); `None` is a daemon with no captured
/// environment to look in.
pub fn claude_checks(
    env: Option<&BTreeMap<String, String>>,
    probes: &ClaudeProbes<'_>,
) -> Vec<FirstRunCheck> {
    let Some(env) = env else {
        return vec![
            installed_unchecked("the daemon has no environment to look for it in yet"),
            login_unchecked("the daemon has no environment to look for it in yet"),
        ];
    };
    let Some(binary) = find_on_path(PROGRAM, env.get("PATH").map(String::as_str)) else {
        return vec![installed_missing()];
    };
    let (version, login) = std::thread::scope(|scope| {
        let version = scope.spawn(|| probes.versions.read_within(&binary, env, probes.timeout));
        let login = scope.spawn(|| probes.login.probe(&binary, env, probes.timeout));
        (version.join(), login.join())
    });
    let version = version.ok().and_then(Result::ok);
    let login =
        login.unwrap_or_else(|_| Login::CouldNotCheck("the check itself failed".to_string()));
    vec![
        installed(version.map(|version| version.to_string())),
        logged_in(login),
    ]
}

fn check(
    id: CheckId,
    ok: bool,
    message: String,
    copy: Option<&str>,
    could_not_check: bool,
) -> FirstRunCheck {
    FirstRunCheck {
        id,
        ok,
        required: false,
        message,
        copy: copy.map(str::to_string),
        could_not_check,
    }
}

fn installed(version: Option<String>) -> FirstRunCheck {
    let message = match version {
        Some(version) => format!("Claude Code is installed, version {version}."),
        None => "Claude Code is installed. Its version could not be read.".to_string(),
    };
    check(CheckId::ClaudeInstalled, true, message, None, false)
}

fn installed_missing() -> FirstRunCheck {
    check(
        CheckId::ClaudeInstalled,
        false,
        format!("Claude Code is not installed. Install it, then check again: `{INSTALL_COMMAND}`"),
        Some(INSTALL_COMMAND),
        false,
    )
}

fn installed_unchecked(reason: &str) -> FirstRunCheck {
    check(
        CheckId::ClaudeInstalled,
        true,
        format!(
            "Could not check whether Claude Code is installed: {reason}. Check again in a moment."
        ),
        None,
        true,
    )
}

fn logged_in(login: Login) -> FirstRunCheck {
    match login {
        Login::LoggedIn => check(CheckId::ClaudeLoggedIn, true, "Claude Code is logged in.".to_string(), None, false),
        Login::LoggedOut => check(
            CheckId::ClaudeLoggedIn,
            false,
            "Claude Code is not logged in. Run `claude` in a terminal and log in. You can open the board now; \
             runs need it."
                .to_string(),
            Some(PROGRAM),
            false,
        ),
        Login::CouldNotCheck(reason) => login_unchecked(&reason),
    }
}

fn login_unchecked(reason: &str) -> FirstRunCheck {
    check(
        CheckId::ClaudeLoggedIn,
        true,
        format!(
            "Could not check whether Claude Code is logged in: {reason}. Check again, or run `{PROGRAM}` in a \
             terminal to see. You can open the board now; runs need it."
        ),
        None,
        true,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::claude::auth_status::NotChecked;
    use crate::claude_versions::VersionsOptions;
    use std::sync::Arc;

    #[test]
    fn with_no_environment_to_look_in_both_checks_say_could_not_check() {
        let versions = ClaudeVersions::new(VersionsOptions::new(None, Arc::new(|_| {})));
        let probes = ClaudeProbes {
            versions: &versions,
            login: &NotChecked,
            timeout: CHECK_TIMEOUT,
        };

        let checks = claude_checks(None, &probes);

        let ids: Vec<CheckId> = checks.iter().map(|check| check.id).collect();
        assert_eq!(ids, [CheckId::ClaudeInstalled, CheckId::ClaudeLoggedIn]);
        for check in &checks {
            assert!(
                check.ok && check.could_not_check && !check.required,
                "{check:?}"
            );
        }
    }

    #[test]
    fn an_environment_with_no_path_finds_no_claude() {
        let versions = ClaudeVersions::new(VersionsOptions::new(None, Arc::new(|_| {})));
        let probes = ClaudeProbes {
            versions: &versions,
            login: &NotChecked,
            timeout: CHECK_TIMEOUT,
        };

        let checks = claude_checks(Some(&BTreeMap::new()), &probes);

        assert_eq!(checks.len(), 1, "{checks:?}");
        assert_eq!(checks[0].id, CheckId::ClaudeInstalled);
        assert!(!checks[0].ok);
        assert_eq!(checks[0].copy.as_deref(), Some(INSTALL_COMMAND));
    }
}
