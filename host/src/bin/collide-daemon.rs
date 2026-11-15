//! `collide-daemon` (0.2; Stage 3a).
//!
//! - `collide-daemon` runs the daemon in the foreground until `SIGTERM` or `SIGINT`. The start (`setsid`, and later the
//!   launchd job) runs this, with its output going to the daemon's log file.
//! - `collide-daemon start` makes sure a daemon answers: it takes the lock file, starts a daemon when the socket does
//!   not answer, and waits for it. It prints `already running` or `started`, and a warning when the start may not
//!   outlive its launcher.
//!
//! Every path comes from `COLLIDE_DAEMON_ROOT` when it is set, and from the user's home otherwise
//! (`collide_host::daemon::DaemonPaths`).

use collide_host::daemon::{DaemonOptions, DaemonPaths, ROOT_ENV, run};
use collide_host::launch::{StartKind, StartRequest, ensure_daemon};
use collide_host::shim::DaemonStart;
use std::process::ExitCode;

fn fail(message: &str) -> ExitCode {
    eprintln!("collide-daemon: {message}");
    ExitCode::from(1)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // An argument it does not know is refused before anything is read or made.
    if let Some(other) = args.first().filter(|a| a.as_str() != "start") {
        return fail(&format!(
            "unknown argument `{other}`: run it with no argument to serve, or with `start`"
        ));
    }
    let paths = match DaemonPaths::from_env(
        std::env::var(ROOT_ENV).ok().as_deref(),
        std::env::var("HOME").ok().as_deref(),
    ) {
        Ok(paths) => paths,
        Err(message) => return fail(&message),
    };
    match args.first().map(String::as_str) {
        None => {
            // A login shell's path when the environment names none: macOS's default shell.
            let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());
            let mut options = DaemonOptions::new(paths, shell, Box::new(std::io::stderr()));
            let exe = std::env::current_exe().and_then(std::fs::canonicalize);
            // The shim is `collide shim`, the binary next to this one; a shim that finds no daemon starts this one,
            // with the same root and home, so it finds the same files.
            if let Ok(exe) = exe {
                options.shim_exe = exe.parent().map(|dir| dir.join("collide"));
                let env = ["COLLIDE_DAEMON_ROOT", "HOME", "SHELL"]
                    .into_iter()
                    .filter_map(|name| Some((name.to_string(), std::env::var(name).ok()?)))
                    .collect();
                options.daemon_start = Some(DaemonStart { exe, env });
            }
            match run(options) {
                Ok(()) => ExitCode::SUCCESS,
                Err(error) => fail(&error.to_string()),
            }
        }
        Some(_) => start(&paths),
    }
}

fn start(paths: &DaemonPaths) -> ExitCode {
    let exe = match std::env::current_exe().and_then(std::fs::canonicalize) {
        Ok(exe) => exe,
        Err(error) => return fail(&format!("cannot find its own executable: {error}")),
    };
    match ensure_daemon(&StartRequest::new(exe, paths)) {
        Ok(StartKind::AlreadyRunning) => {
            println!("already running");
            ExitCode::SUCCESS
        }
        Ok(StartKind::Launchd) => {
            println!("started");
            ExitCode::SUCCESS
        }
        Ok(StartKind::SetsidFallback { warning }) => {
            println!("started");
            eprintln!("collide-daemon: warning: {warning}");
            ExitCode::SUCCESS
        }
        Err(error) => fail(&error.to_string()),
    }
}
