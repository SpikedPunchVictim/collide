//! Phases under a daemon that the real launchd started (ts-0070-mk). Two opt-in tests, skipped with the reason printed
//! unless `COLLIDE_REAL_LAUNCHD=1` is set (and, for the second, `COLLIDE_REAL_CLAUDE=1`):
//!
//! * a fake-Claude phase outlives a daemon that is killed, and the daemon launchd starts next adopts it
//!   (`AbandonProcessGroup` in the job's plist is what keeps the shim alive), and
//! * a real `claude -p` phase, on haiku with a budget cap of $0.10, through `collide run` against a launchd daemon.
//!
//! SAFETY. Every process has `env_clear()` or, for a launchd job, only the variables the test gives its plist, with `HOME`
//! and `COLLIDE_DAEMON_ROOT` in the test's own folder under `/tmp`. The job's label starts with
//! `dev.collide.test.<pid>.<n>`. [`Jobs`] boots out exactly the labels the starts recorded under that prefix, and its
//! `Drop` ends the daemon by the pid its own socket reported. The real `claude` of the second test is behind a wrapper
//! script in the test's folder (see [`wrapper`]): it is the only place where the real home and the login are used.

use super::common::{TempRoot, daemon_status, pid_alive};
use super::fake::recording;
use super::real;
use super::run_process::{Proc, lines_of, slowed_report, task_of, text};
use collide_host::daemon::DaemonPaths;
use collide_host::file_run::PHASE_NAME;
use collide_host::launch::{
    LABEL_FILE, Launchctl, StartKind, StartRequest, SystemLaunchctl, ensure_daemon,
};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

const DAEMON: &str = env!("CARGO_BIN_EXE_collide-daemon");
const REAL_LAUNCHCTL: &str = "/bin/launchctl";
const USER: &str = "collide-test";

static JOBS: AtomicU32 = AtomicU32::new(0);

fn uid() -> u32 {
    // SAFETY: `getuid` takes no arguments and cannot fail.
    unsafe { libc::getuid() }
}

/// Why a launchd test cannot run here, or `None` when it can.
fn unavailable() -> Option<String> {
    if std::env::var("COLLIDE_REAL_LAUNCHD").as_deref() != Ok("1") {
        return Some("COLLIDE_REAL_LAUNCHD=1 is not set".to_string());
    }
    if !cfg!(target_os = "macos") {
        return Some("this is not macOS".to_string());
    }
    // A read-only question: does the user's GUI domain exist?
    let gui = Command::new(REAL_LAUNCHCTL)
        .args(["print", &format!("gui/{}", uid())])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    match gui {
        Ok(status) if status.success() => None,
        other => Some(format!("no GUI session for uid {} ({other:?})", uid())),
    }
}

/// The jobs one test started, each with the prefix of this test. It boots them out when the test ends.
struct Jobs {
    paths: DaemonPaths,
    prefix: String,
    labels: Vec<String>,
}

impl Jobs {
    fn new(paths: &DaemonPaths) -> Jobs {
        let n = JOBS.fetch_add(1, Ordering::SeqCst);
        Jobs {
            paths: paths.clone(),
            prefix: format!("dev.collide.test.{}.{n}", std::process::id()),
            labels: Vec::new(),
        }
    }

    /// Starts the daemon through the real launchd with exactly `env` (plus `USER`), and returns its pid.
    fn start_daemon(&mut self, proc: &Proc) -> i32 {
        let mut request = StartRequest::new(PathBuf::from(DAEMON), &self.paths);
        request.label_prefix = self.prefix.clone();
        request.launchctl = Some(Arc::new(SystemLaunchctl::new(REAL_LAUNCHCTL)));
        for (name, value) in [
            ("USER", USER),
            ("HOME", proc.root.join("home").to_str().unwrap()),
            ("PATH", "/usr/bin:/bin"),
            ("SHELL", proc.shell.to_str().unwrap()),
            ("COLLIDE_DAEMON_ROOT", proc.root.path().to_str().unwrap()),
        ] {
            request.env.insert(name.to_string(), value.to_string());
        }
        let started = ensure_daemon(&request).expect("the launchd start");
        assert_eq!(started, StartKind::Launchd);
        let label = fs::read_to_string(self.paths.socket_dir.join(LABEL_FILE))
            .expect("the start recorded its label")
            .trim()
            .to_string();
        assert!(label.starts_with(&self.prefix), "{label}");
        self.labels.push(label);
        daemon_status(&self.paths).expect("the daemon answers")["pid"]
            .as_i64()
            .expect("a pid") as i32
    }
}

impl Drop for Jobs {
    fn drop(&mut self) {
        for label in &self.labels {
            if label.starts_with(&self.prefix) {
                let _ =
                    SystemLaunchctl::new(REAL_LAUNCHCTL).bootout(&format!("gui/{}/{label}", uid()));
            }
        }
    }
}

/// Waits for a `collide` command that the test started, up to `within`, with its output in files of the test's folder.
fn finish(mut child: Child, within: Duration) -> Option<i32> {
    let until = Instant::now() + within;
    loop {
        if let Some(status) = child.try_wait().expect("collide is waited for") {
            return status.code();
        }
        if Instant::now() > until {
            // The test's own child.
            let _ = child.kill();
            let _ = child.wait();
            return None;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// `collide <args>` with its output kept in `<root>/<name>.out` and `.err`, ended after `within`.
fn collide_logged(
    proc: &Proc,
    name: &str,
    args: &[&str],
    within: Duration,
) -> (Option<i32>, String, String) {
    let out = proc.root.join(&format!("{name}.out"));
    let err = proc.root.join(&format!("{name}.err"));
    let child = proc
        .command(args, &proc.repo)
        .stdout(fs::File::create(&out).unwrap())
        .stderr(fs::File::create(&err).unwrap())
        .spawn()
        .expect("collide runs");
    let code = finish(child, within);
    let read = |path: &Path| text(&fs::read(path).unwrap_or_default());
    (code, read(&out), read(&err))
}

#[test]
fn a_phase_outlives_a_daemon_that_is_killed_and_the_next_daemon_adopts_it() {
    if let Some(reason) = unavailable() {
        println!("skipped: {reason}");
        return;
    }
    let scratch = TempRoot::new();
    let slow = slowed_report(&scratch, 10);
    let proc = Proc::new(&slow);
    let paths = DaemonPaths::under(proc.root.path());
    let mut jobs = Jobs::new(&paths);
    let first_daemon = jobs.start_daemon(&proc);

    // A run begins, and the command that started it goes away, as a closed terminal does.
    let mut first = proc
        .command(&["run", &task_of(&proc)], &proc.repo)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("collide runs");
    let lines = lines_of(&mut first);
    let mut run = None;
    while run.is_none() {
        let line = lines
            .recv_timeout(Duration::from_secs(60))
            .unwrap_or_else(|_| {
                let _ = first.kill();
                panic!("the command never said it started")
            });
        if let Some(rest) = line.strip_prefix("run ")
            && let Some((id, _)) = rest.split_once(" started on ")
        {
            run = Some(id.to_string());
        }
    }
    let run = run.unwrap();
    first.kill().expect("the command is killed");
    first.wait().expect("the command is reaped");

    // The phase's shim, by the identity the shim wrote.
    let shim_file = paths.runs_dir.join(&run).join(PHASE_NAME).join("shim.json");
    common_wait("the shim's identity file", || shim_file.exists());
    let identity: Value = serde_json::from_str(&fs::read_to_string(&shim_file).unwrap()).unwrap();
    let child =
        collide_host::shim::ChildIdentity::from_json(&identity).expect("the child's identity");
    let shim_pid = child.pid;
    assert!(pid_alive(shim_pid), "the phase's child runs");

    // The daemon is killed hard, as a crash would: launchd ends the job, and `AbandonProcessGroup` leaves the shim.
    // SAFETY: `kill` takes two integers; the pid is the daemon this test started, from its own socket.
    unsafe { libc::kill(first_daemon, libc::SIGKILL) };
    common_wait("the daemon is gone", || daemon_status(&paths).is_none());
    std::thread::sleep(Duration::from_secs(1));
    assert!(
        pid_alive(shim_pid),
        "launchd took the phase with the daemon: AbandonProcessGroup is not doing its work"
    );

    // The next daemon, started by launchd under a new job, adopts the phase, and `--resume` follows it to its end.
    let second_daemon = jobs.start_daemon(&proc);
    assert_ne!(first_daemon, second_daemon);
    let (code, out, err) = collide_logged(
        &proc,
        "resume",
        &["run", "--resume", &run],
        Duration::from_secs(120),
    );
    assert_eq!(code, Some(0), "{out}\n{err}");
    assert!(out.contains(": ended\n"), "{out}");
    assert!(
        out.contains("The notes start with the first line."),
        "{out}"
    );
    println!("adopted: {out}");

    // Nothing of the test's own is left loaded under its prefix once the jobs are booted out.
    let labels = jobs.labels.clone();
    drop(jobs);
    common_wait("the jobs are no longer loaded", || {
        let listing = Command::new(REAL_LAUNCHCTL)
            .arg("list")
            .output()
            .expect("launchctl list");
        let listing = text(&listing.stdout);
        labels.iter().all(|label| !listing.contains(label))
    });
}

// ---- a real `claude -p` under a launchd daemon -------------------------------------------------------------------------

/// The `claude` the daemon's login shell finds: a script that logs what it was called with and what it wrote, gives the
/// real binary the real login (`HOME` and `USER`, for that process only), turns the user settings off (the built-in
/// builder role has no `config`, so the line says `--setting-sources user`; the wrapper makes that `""`, config none) and
/// keeps the auto-updater off.
fn wrapper(proc: &Proc) -> PathBuf {
    let logs = proc.root.join("claude-logs");
    fs::create_dir(&logs).unwrap();
    let body = format!(
        r#"#!/bin/bash
args=()
prev=""
for a in "$@"; do
  was="$a"
  if [ "$prev" = "--setting-sources" ] && [ "$a" = "user" ]; then a=""; fi
  args+=("$a")
  prev="$was"
done
printf '%s\n' "${{args[@]}}" > '{logs}'/$$.args
export HOME='{home}' USER='{user}' DISABLE_AUTOUPDATER=1
set -o pipefail
'{claude}' "${{args[@]}}" | tee '{logs}'/$$.out
"#,
        logs = logs.display(),
        home = std::env::var("HOME").unwrap_or_default(),
        user = std::env::var("USER").unwrap_or_default(),
        claude = real::binary().display(),
    );
    proc.root.script("bin/claude", &body)
}

/// What the wrapper logged: for each call, its arguments and the cost its output reported.
fn calls_of(proc: &Proc) -> Vec<(Vec<String>, f64)> {
    let logs = proc.root.join("claude-logs");
    let mut calls = Vec::new();
    for entry in fs::read_dir(&logs).into_iter().flatten().flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "args") {
            let args: Vec<String> = fs::read_to_string(&path)
                .unwrap()
                .lines()
                .map(String::from)
                .collect();
            let out = fs::read_to_string(path.with_extension("out")).unwrap_or_default();
            let cost = out
                .lines()
                .filter_map(|l| serde_json::from_str::<Value>(l).ok())
                .filter_map(|v| v["total_cost_usd"].as_f64())
                .next_back()
                .unwrap_or(0.0);
            calls.push((args, cost));
        }
    }
    calls
}

#[test]
fn a_real_claude_phase_runs_under_a_launchd_daemon() {
    if let Some(reason) = unavailable() {
        println!("skipped: {reason}");
        return;
    }
    if let Some(reason) = real::unavailable() {
        println!("skipped: {reason}");
        return;
    }
    const PHASE_CAP: f64 = 0.10;
    real::check_ledger("launchd-phase", PHASE_CAP);
    let proc = Proc::new(&recording("report"));
    wrapper(&proc);
    fs::write(
        &proc.task,
        "Reply with the single word ok. Do not use any tool.\n",
    )
    .unwrap();
    let paths = DaemonPaths::under(proc.root.path());
    let mut jobs = Jobs::new(&paths);
    jobs.start_daemon(&proc);

    let began = Instant::now();
    let (code, out, err) = collide_logged(
        &proc,
        "phase",
        &["run", &task_of(&proc), "--budget", "0.10"],
        Duration::from_secs(180),
    );
    let took = began.elapsed();
    let calls = calls_of(&proc);
    // The daemon runs no smoke check for a version inside the tested range, so the one call with `--input-format` is
    // the phase, and its cost is what the ledger takes.
    let phase_cost: f64 = calls
        .iter()
        .filter(|(args, _)| !args.iter().any(|a| a == "--version"))
        .map(|(_, cost)| cost)
        .sum();
    real::add_to_ledger("launchd-phase", PHASE_CAP, phase_cost);
    println!("collide run: code {code:?} in {took:?}\n{out}\n{err}");
    println!("claude calls: {calls:?}");

    assert_eq!(code, Some(0), "{out}\n{err}");
    assert!(out.contains("claude in use: 2."), "{out}");
    assert!(out.contains(": ended\n"), "{out}");
    assert!(out.contains("turn 1: wall "), "{out}");

    let phase = calls
        .iter()
        .find(|(args, _)| args.iter().any(|a| a == "--input-format"))
        .expect("the phase's call");
    let args = &phase.0;
    let at = |flag: &str| {
        args.iter()
            .position(|a| a == flag)
            .unwrap_or_else(|| panic!("{flag} in {args:?}"))
    };
    assert_eq!(
        args[at("--setting-sources") + 1],
        "",
        "config none: {args:?}"
    );
    assert_eq!(args[at("--permission-prompt-tool") + 1], "stdio");
    let cap: f64 = args[at("--max-budget-usd") + 1].parse().expect("a cap");
    assert!(cap <= PHASE_CAP + 1e-9, "the cap on the line is {cap}");
    assert!(phase_cost <= PHASE_CAP, "{phase_cost}");
}

fn common_wait(what: &str, done: impl FnMut() -> bool) {
    super::common::wait_until(what, Duration::from_secs(30), done);
}
