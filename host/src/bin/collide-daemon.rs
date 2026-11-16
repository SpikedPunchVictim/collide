//! `collide-daemon` (0.2; Stage 3a).
//!
//! - `collide-daemon` runs the daemon in the foreground until `SIGTERM` or `SIGINT`. The start (a one-shot launchd job
//!   on macOS, `setsid` otherwise or when launchd refuses) runs this, with its output going to the daemon's log file.
//! - `collide-daemon start` makes sure a daemon answers: it takes the lock file, starts a daemon when the socket does
//!   not answer, and waits for it. It prints `already running` or `started`, and a warning when the start may not
//!   outlive its launcher.
//!
//! Every path comes from `COLLIDE_DAEMON_ROOT` when it is set, and from the user's home otherwise
//! (`collide_host::daemon::DaemonPaths`). A start under `COLLIDE_DAEMON_ROOT` never loads a launchd job unless
//! `COLLIDE_LAUNCHCTL` names the program to load it with (`collide_host::launch::launchctl_from_env`).

use collide_host::daemon::{DaemonOptions, DaemonPaths, ROOT_ENV, run};
use collide_host::launch::{
    FORWARDED_ENV, LAUNCHCTL_ENV, StartKind, StartRequest, ensure_daemon, launchctl_from_env,
};
use collide_host::log_cap::{CappedLog, DEFAULT_LOG_CAP_BYTES, fd_is_file};
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
            // The log is written through a file of its own with a size cap and one rotated file. A log that cannot be
            // opened leaves the daemon on standard error, which a launch has pointed at the same file.
            // Standard error follows each rotation when a launch has pointed it at the log.
            let follow = fd_is_file(2, &paths.log_file).then_some(2);
            let log: Box<dyn std::io::Write + Send> =
                match CappedLog::open_following(&paths.log_file, DEFAULT_LOG_CAP_BYTES, follow) {
                    Ok(log) => Box::new(log),
                    Err(error) => {
                        eprintln!("collide-daemon: the log file cannot be opened: {error}");
                        Box::new(std::io::stderr())
                    }
                };
            let mut options = DaemonOptions::new(paths, shell, log);
            // A daemon under an overridden root boots out no job unless `COLLIDE_LAUNCHCTL` names the program.
            options.launchctl = launchctl_from_env(
                std::env::var(ROOT_ENV).ok().as_deref(),
                std::env::var(LAUNCHCTL_ENV).ok().as_deref(),
            );
            // The published `claude` version is fetched only by a daemon that is not under an overridden root.
            options.claude_published = collide_host::claude_versions::published_from_env(
                std::env::var(ROOT_ENV).ok().as_deref(),
            );
            // Secrets are kept in the OS keychain only when the daemon is not under an overridden root.
            options.keychain =
                collide_host::keychain::keychain_for_root(std::env::var(ROOT_ENV).ok().as_deref());
            let exe = std::env::current_exe().and_then(std::fs::canonicalize);
            // The shim is `collide shim`, the binary next to this one; a shim that finds no daemon starts this one,
            // with the same root and home, so it finds the same files.
            if let Ok(exe) = exe {
                options.shim_exe = exe.parent().map(|dir| dir.join("collide"));
                let env = FORWARDED_ENV
                    .into_iter()
                    .chain([LAUNCHCTL_ENV])
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
    let mut request = StartRequest::new(exe, paths);
    // launchd's environment is bare, so what the daemon needs of this one goes in the job's plist.
    request.env = FORWARDED_ENV
        .into_iter()
        .chain([LAUNCHCTL_ENV])
        .filter_map(|name| Some((name.to_string(), std::env::var(name).ok()?)))
        .collect();
    request.launchctl = launchctl_from_env(
        std::env::var(ROOT_ENV).ok().as_deref(),
        std::env::var(LAUNCHCTL_ENV).ok().as_deref(),
    );
    match ensure_daemon(&request) {
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
