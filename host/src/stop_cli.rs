//! `collide stop <run> [--grace <secs>]` (0.5, Stopping; Stage 3a; tag `cli: stop`): asks the daemon to stop a run
//! (`run.stop`, [`crate::phases::Phases::stop_run`]) and prints the report. The daemon is started first when it is not
//! running ([`crate::launch::ensure_daemon`]), because the shims outlive a daemon and a stop that finds no daemon
//! should still reach them: the new daemon adopts them before it answers.
//!
//! The report is one line for the state and the number of processes signalled, then one line for each survivor with
//! its pid and how it came to be listed (`found`, `recorded` or `suspect`). The exit code is `0` when the stop ran,
//! whether or not it left survivors: they are in the text. A run the daemon does not know exits [`exit::ID`].

use crate::daemon::{DaemonPaths, ROOT_ENV};
use crate::launch::{
    FORWARDED_ENV, LAUNCHCTL_ENV, StartRequest, ensure_daemon, launchctl_from_env,
};
use crate::owner_client::{ClientError, OwnerClient};
use crate::task_cli::exit;
use collide_core::owner::{RunStopParams, RunStopResult, code};
use std::io::Write;
use std::path::PathBuf;

pub const USAGE: &str = "usage: collide stop <run> [--grace <secs>]";

/// What the command line asked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StopArgs {
    pub run: String,
    pub grace_secs: Option<u32>,
}

/// Reads the arguments after `stop`. The error is the message to print with the usage.
pub fn parse(args: &[String]) -> Result<StopArgs, String> {
    let mut run: Option<String> = None;
    let mut grace_secs: Option<u32> = None;
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        if arg == "--grace" {
            let value = rest.next().ok_or("--grace needs a number of seconds")?;
            grace_secs = Some(
                value
                    .parse()
                    .map_err(|_| format!("--grace {value:?} is not a whole number of seconds"))?,
            );
        } else if arg.starts_with('-') {
            return Err(format!("unknown option `{arg}`"));
        } else if run.replace(arg.clone()).is_some() {
            return Err("only one run can be stopped at a time".to_string());
        }
    }
    let run = run.ok_or("name the run to stop")?;
    if run.is_empty() {
        return Err("the run's id is empty".to_string());
    }
    Ok(StopArgs { run, grace_secs })
}

/// The `collide-daemon` next to this binary, canonical.
fn daemon_exe() -> std::io::Result<PathBuf> {
    let exe = std::fs::canonicalize(std::env::current_exe()?)?;
    exe.parent()
        .map(|dir| dir.join("collide-daemon"))
        .ok_or_else(|| std::io::Error::other("the collide binary has no folder"))
}

/// Makes sure a daemon answers, as `collide-daemon start` does.
fn ensure_running(paths: &DaemonPaths) -> Result<(), String> {
    let exe = daemon_exe().map_err(|e| format!("cannot find collide-daemon: {e}"))?;
    let mut request = StartRequest::new(exe, paths);
    request.env = FORWARDED_ENV
        .into_iter()
        .chain([LAUNCHCTL_ENV])
        .filter_map(|name| Some((name.to_string(), std::env::var(name).ok()?)))
        .collect();
    request.launchctl = launchctl_from_env(
        std::env::var(ROOT_ENV).ok().as_deref(),
        std::env::var(LAUNCHCTL_ENV).ok().as_deref(),
    );
    ensure_daemon(&request)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// The report, as `collide stop` prints it.
pub fn report_text(run: &str, result: &RunStopResult) -> String {
    let state = serde_json::to_value(&result.state)
        .ok()
        .and_then(|v| v.get("state").and_then(|s| s.as_str().map(str::to_string)))
        .unwrap_or_else(|| "unknown".to_string());
    let mut text = format!(
        "run {run}: {state}; {} process(es) signalled\n",
        result.signalled
    );
    if result.survivors.is_empty() {
        text.push_str("survivors: none\n");
    } else {
        text.push_str(&format!("survivors: {}\n", result.survivors.len()));
        for survivor in &result.survivors {
            text.push_str(&format!("  {} {}\n", survivor.pid, survivor.command));
        }
    }
    text
}

/// Runs the command. `args` are the arguments after `stop`.
pub fn run(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let parsed = match parse(args) {
        Ok(parsed) => parsed,
        Err(message) => {
            let _ = writeln!(err, "collide stop: {message}\n{USAGE}");
            return exit::USAGE;
        }
    };
    let paths = match DaemonPaths::from_env(
        std::env::var(ROOT_ENV).ok().as_deref(),
        std::env::var("HOME").ok().as_deref(),
    ) {
        Ok(paths) => paths,
        Err(message) => {
            let _ = writeln!(err, "collide stop: {message}");
            return exit::FAILED;
        }
    };
    if let Err(message) = ensure_running(&paths) {
        let _ = writeln!(
            err,
            "collide stop: the daemon is not running and did not start: {message}"
        );
        return exit::FAILED;
    }
    let mut client = match OwnerClient::connect(&paths, "collide stop") {
        Ok(client) => client,
        Err(error) => {
            let _ = writeln!(err, "collide stop: {error}");
            return exit::FAILED;
        }
    };
    let params = RunStopParams {
        run: parsed.run.clone(),
        grace_secs: parsed.grace_secs,
    };
    match client.call::<_, RunStopResult>("run.stop", &params, None) {
        Ok(result) => {
            let _ = write!(out, "{}", report_text(&parsed.run, &result));
            exit::OK
        }
        Err(ClientError::Rpc(error)) if error.code == code::NOT_FOUND => {
            let _ = writeln!(err, "collide stop: {}", error.message);
            exit::ID
        }
        Err(error) => {
            let _ = writeln!(err, "collide stop: {error}");
            exit::FAILED
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use collide_core::owner::Survivor;
    use collide_core::state::RunState;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn a_run_and_a_grace_are_read() {
        assert_eq!(
            parse(&args(&["r-1", "--grace", "5"])).unwrap(),
            StopArgs {
                run: "r-1".to_string(),
                grace_secs: Some(5)
            }
        );
        assert_eq!(parse(&args(&["r-1"])).unwrap().grace_secs, None);
    }

    #[test]
    fn a_bad_command_line_is_refused() {
        for bad in [
            args(&[]),
            args(&[""]),
            args(&["a", "b"]),
            args(&["a", "--grace"]),
            args(&["a", "--grace", "x"]),
            args(&["a", "--grace", "-1"]),
            args(&["a", "--force"]),
        ] {
            assert!(parse(&bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn the_report_names_the_state_the_count_and_each_survivor() {
        let result = RunStopResult {
            state: RunState::Stopped {},
            signalled: 3,
            survivors: vec![Survivor {
                pid: 77,
                command: "/bin/sleep (suspect)".to_string(),
            }],
        };
        assert_eq!(
            report_text("r-1", &result),
            "run r-1: stopped; 3 process(es) signalled\nsurvivors: 1\n  77 /bin/sleep (suspect)\n"
        );
        let none = RunStopResult {
            survivors: vec![],
            ..result
        };
        assert!(report_text("r-1", &none).ends_with("survivors: none\n"));
    }
}
