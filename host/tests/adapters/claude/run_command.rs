//! `collide run` against a daemon in this process (ts-0069-bn): the command's own code, [`collide_host::run_cli`], is
//! called with an [`Env`] that points at the test's daemon, a pipe for standard input and buffers for the output, so
//! every line it prints and every exit code is checked. The daemon is the one of `file_run.rs` (a `Fixture`) or
//! `pause.rs` (a `Rig`, whose clocks a test moves), with fake Claude as `claude`. The command talks to the daemon over
//! its socket as a real one does.
//!
//! What no test here does: start a daemon process, read the owner's files, reach a network, load a launchd job or run
//! the real `claude`. The `git` that finds a task file's repository is run with a cleared environment
//! ([`clean_toplevel`]); the processes of `run_process.rs` have `env_clear()`, `HOME` and `COLLIDE_DAEMON_ROOT` in a temp
//! root. Every process a test starts is reaped by its fixture's `Drop`.

use super::common::{TempRoot, kill_child_of, wait_until};
use super::fake::recording;
use super::file_run::{Fixture, Setup, git};
use super::pause::{Rig, Settings};
use super::published::{BODY, Reply, Server, source};
use collide_core::agent::PermissionReply;
use collide_core::events::EventBody;
use collide_core::owner::{
    DaemonAgentVersionsParams, DaemonAgentVersionsResult, QueueAnswer, QueueAnswerParams,
    QueueAnswerResult, QueueKind, RunRecordParams, RunRecordResult, RunStopParams, RunStopResult,
};
use collide_core::state::RunState;
use collide_host::claude_versions::PublishedChoice;
use collide_host::daemon::DaemonPaths;
use collide_host::file_run::PHASE_NAME;
use collide_host::owner_client::OwnerClient;
use collide_host::run_cli::{Env, run_with};
use serde_json::{Value, json};
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

/// What the command printed and the code it exited with.
#[derive(Debug)]
pub(super) struct Outcome {
    pub(super) code: u8,
    pub(super) out: String,
    pub(super) err: String,
}

/// The top of the checkout that holds `dir`, asked of a `git` with no environment of the caller's.
fn clean_toplevel(dir: &Path) -> Option<PathBuf> {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["rev-parse", "--show-toplevel"])
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
    fs::canonicalize(text.trim_end_matches('\n')).ok()
}

/// What a command needs to find a test's daemon.
#[derive(Clone)]
pub(super) struct Cli {
    paths: DaemonPaths,
    home: PathBuf,
    cwd: PathBuf,
    started: Arc<AtomicUsize>,
}

impl Cli {
    pub(super) fn new(paths: DaemonPaths, home: PathBuf, cwd: PathBuf) -> Cli {
        Cli {
            paths,
            home,
            cwd,
            started: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub(super) fn of(fx: &Fixture) -> Cli {
        Cli::new(fx.paths(), fx.root.join("home"), fx.repo.clone())
    }

    pub(super) fn of_rig(rig: &Rig) -> Cli {
        Cli::new(
            DaemonPaths::under(rig.root.path()),
            rig.root.join("home"),
            rig.repo.clone(),
        )
    }

    fn env(&self) -> Env {
        let started = Arc::clone(&self.started);
        Env {
            paths: self.paths.clone(),
            home: Some(self.home.to_str().unwrap().to_string()),
            cwd: self.cwd.clone(),
            start_daemon: Box::new(move |_| {
                started.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }),
            poll: Duration::from_millis(20),
            toplevel: clean_toplevel,
        }
    }

    /// Runs `collide run <args>` with `stdin` as what the person types.
    pub(super) fn run(&self, args: &[&str], stdin: &str) -> Outcome {
        let args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
        let mut input = Cursor::new(stdin.as_bytes().to_vec());
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = run_with(&self.env(), &args, &mut input, &mut out, &mut err);
        Outcome {
            code,
            out: String::from_utf8(out).unwrap(),
            err: String::from_utf8(err).unwrap(),
        }
    }

    /// The same, on a thread of its own, so the test can act while the command follows the run.
    pub(super) fn spawn(&self, args: &[&str], stdin: &str) -> JoinHandle<Outcome> {
        let cli = self.clone();
        let args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
        let stdin = stdin.to_string();
        thread::spawn(move || {
            let args: Vec<&str> = args.iter().map(String::as_str).collect();
            cli.run(&args, &stdin)
        })
    }

    fn daemon_starts(&self) -> usize {
        self.started.load(Ordering::SeqCst)
    }
}

fn task_arg(fx: &Fixture) -> String {
    fx.task.to_str().unwrap().to_string()
}

/// The line of `out` that starts `prefix`.
fn line_with<'a>(out: &'a str, prefix: &str) -> &'a str {
    out.lines()
        .find(|line| line.trim_start().starts_with(prefix))
        .unwrap_or_else(|| panic!("no line starting {prefix:?} in:\n{out}"))
}

/// The run the daemon's event log saw first.
fn first_run(events: impl Fn() -> Vec<EventBody>) -> String {
    let until = Instant::now() + Duration::from_secs(30);
    loop {
        for body in events() {
            if let EventBody::RunTransition { run, .. } = body {
                return run;
            }
        }
        assert!(Instant::now() < until, "no run began");
        thread::sleep(Duration::from_millis(20));
    }
}

/// `run.record` of `run`, asked over a client of the test's own.
fn record_of(paths: &DaemonPaths, run: &str) -> RunRecordResult {
    let mut client = OwnerClient::connect(paths, "run-command-test").unwrap();
    client
        .call(
            "run.record",
            &RunRecordParams {
                run: run.to_string(),
            },
            Some(Duration::from_secs(10)),
        )
        .unwrap()
}

/// Waits until the daemon has fetched the published versions. The daemon makes its source once its start capture and
/// its first read of the `claude` version have ended, and `collide run` waits only a few seconds for it
/// (`print_versions`): on a machine that starts programs slowly a run can come first (ts-0166-rq).
fn wait_for_published_versions(fx: &mut Fixture) {
    wait_until(
        "the daemon has the published versions",
        Duration::from_secs(120),
        || {
            let result: DaemonAgentVersionsResult = fx
                .client
                .call(
                    "daemon.agent_versions",
                    &DaemonAgentVersionsParams {},
                    Some(Duration::from_secs(60)),
                )
                .expect("the call is answered");
            result.agents[0]
                .published
                .as_ref()
                .is_some_and(|published| published.latest.is_some())
        },
    );
}

// ---- the versions and the record of an ordinary run ---------------------------------------------------------------

#[test]
fn a_task_file_run_prints_both_versions_then_follows_the_phase_and_prints_its_record() {
    let server = Server::start(vec![Reply::Body(BODY.into())]);
    let published = PublishedChoice::Source(Arc::new(source(&server.url("/dist-tags"))));
    let mut fx = Fixture::new(
        &recording("report"),
        Setup {
            published: Some(published),
            ..Setup::default()
        },
    );
    wait_for_published_versions(&mut fx);
    let cli = Cli::of(&fx);
    let outcome = cli.run(&[&task_arg(&fx)], "");
    assert_eq!(outcome.code, 0, "{outcome:?}");
    assert_eq!(outcome.err, "");
    let out = &outcome.out;
    assert_eq!(cli.daemon_starts(), 1, "the daemon is made sure of once");

    let in_use = line_with(out, "claude in use: 2.1.325");
    assert!(in_use.contains("tested range"), "{in_use}");
    line_with(out, "claude published: latest 2.1.325, stable 2.1.311");
    let order = |needle: &str| {
        out.find(needle)
            .unwrap_or_else(|| panic!("{needle} in {out}"))
    };
    assert!(
        order("claude in use") < order("claude published")
            && order("claude published") < order("started"),
        "both versions come before the run starts: {out}"
    );

    line_with(out, "run ");
    assert!(out.contains(": ended\n"), "{out}");
    assert!(
        out.contains("report:\n  The notes start with the first line."),
        "{out}"
    );
    let turn = line_with(out, "turn 1:");
    assert!(turn.contains("model time 1.3 s"), "{turn}");
    assert!(turn.contains("input 4, output 40"), "{turn}");
    assert!(turn.contains("cache read 12,000"), "{turn}");
    assert!(turn.contains("cost $0.0041"), "{turn}");
    assert!(!turn.contains("wall not reported"), "{turn}");
    line_with(out, "total: wall ");

    let worktree = line_with(out, "worktree: ");
    let path = worktree
        .trim_start_matches("worktree: ")
        .split(" (branch")
        .next()
        .unwrap();
    assert!(Path::new(path).join("README").is_file(), "{worktree}");
}

#[test]
fn with_no_published_source_the_published_line_says_the_daemon_does_not_look_it_up() {
    let fx = Fixture::new(&recording("report"), Setup::default());
    let outcome = Cli::of(&fx).run(&[&task_arg(&fx)], "");
    assert_eq!(outcome.code, 0, "{outcome:?}");
    line_with(&outcome.out, "claude in use: 2.1.325");
    assert_eq!(
        line_with(&outcome.out, "claude published"),
        "claude published: not known (this daemon does not look it up)"
    );
}

#[test]
fn a_version_outside_the_tested_range_is_printed_with_its_smoke_check() {
    let fx = Fixture::new(
        &recording("report"),
        Setup {
            version: Some("2.1.999".to_string()),
            smoke: Some(recording("report")),
            ..Setup::default()
        },
    );
    let outcome = Cli::of(&fx).run(&[&task_arg(&fx)], "");
    let line = line_with(&outcome.out, "claude in use: 2.1.999");
    assert!(line.contains("outside the tested range"), "{line}");
    assert!(line.contains("smoke check"), "{line}");
}

// ---- model time: batched turns, and a turn the phase never finished ----------------------------------------------

#[test]
fn a_session_with_several_turns_prints_each_with_its_own_figures_and_not_reported_where_it_has_none()
 {
    let fx = Fixture::new(&recording("usage-one-send"), Setup::default());
    let outcome = Cli::of(&fx).run(&[&task_arg(&fx)], "");
    assert_eq!(outcome.code, 0, "{outcome:?}");
    let out = &outcome.out;
    let calls: Vec<&str> = out
        .lines()
        .map(str::trim_start)
        .filter(|line| {
            line.starts_with("turn ") || line.starts_with("compaction") || line.starts_with("other")
        })
        .collect();
    assert_eq!(calls.len(), 5, "{out}");
    assert!(calls[0].starts_with("turn 1: wall "), "{}", calls[0]);
    assert!(
        !calls[0].starts_with("turn 1: wall not reported"),
        "{}",
        calls[0]
    );
    assert!(calls[0].contains("model time 2.3 s"), "{}", calls[0]);
    assert!(calls[0].contains("cost $0.0084"), "{}", calls[0]);
    assert!(
        calls[1].starts_with("compaction: wall not reported, model time not reported"),
        "{}",
        calls[1]
    );
    assert!(calls[1].contains("cost $0.0034"), "{}", calls[1]);
    // A turn that did no model work has no model time, and its cost is the reported zero.
    assert!(
        calls[2].contains("wall not reported, model time not reported"),
        "{}",
        calls[2]
    );
    assert!(calls[3].contains("model time not reported"), "{}", calls[3]);
    assert!(calls[3].contains("cost $0.0000"), "{}", calls[3]);
    assert!(calls[4].contains("model time 8.0 s"), "{}", calls[4]);
    assert!(calls[4].contains("cost $0.0040"), "{}", calls[4]);
    // The total is the sum of what was reported.
    let total = line_with(out, "total:");
    assert!(total.contains("cost $0.0158"), "{total}");
}

/// A recording that reads the first turn, answers two responses and then holds on until it is signalled.
fn cut_recording(root: &TempRoot) -> PathBuf {
    let text = fs::read_to_string(recording("report")).unwrap();
    let mut steps: Vec<String> = text
        .lines()
        .filter(|line| !line.starts_with('#') && !line.is_empty())
        .take(4)
        .map(str::to_string)
        .collect();
    steps.push(json!({"sleep": 120}).to_string());
    let path = root.join("cut.jsonl");
    fs::write(&path, steps.join("\n")).unwrap();
    path
}

/// Waits until the run's record holds a model call, so the phase has spent tokens the test can then cut short.
fn wait_for_spent_tokens(paths: &DaemonPaths, run: &str) {
    wait_until(
        "the phase has spent tokens",
        Duration::from_secs(30),
        || !record_of(paths, run).calls.is_empty(),
    );
}

fn kill_the_phase(fx: &Fixture, run: &str) {
    kill_child_of(&fx.paths().runs_dir.join(run).join(PHASE_NAME));
}

#[test]
fn a_phase_killed_in_its_first_turn_keeps_the_tokens_it_spent_and_reports_the_cost_as_not_reported()
{
    let scratch = TempRoot::new();
    let fx = Fixture::new(&cut_recording(&scratch), Setup::default());
    let cli = Cli::of(&fx);
    let handle = cli.spawn(&[&task_arg(&fx)], "");
    let run = first_run(|| fx.events());
    wait_for_spent_tokens(&fx.paths(), &run);
    kill_the_phase(&fx, &run);
    let outcome = handle.join().unwrap();
    assert_eq!(outcome.code, 7, "{outcome:?}");
    let out = &outcome.out;
    assert!(out.contains(": failed\n"), "{out}");
    line_with(out, "failure: ");
    assert!(out.contains("report: not reported"), "{out}");
    let cut = line_with(out, "turn 1:");
    assert!(cut.contains("input at least 4"), "{cut}");
    assert!(cut.contains("cache read at least "), "{cut}");
    assert!(cut.contains("output not reported"), "{cut}");
    assert!(cut.contains("model time not reported"), "{cut}");
    assert!(cut.contains("cost not reported"), "{cut}");
    assert!(!cut.contains("$"), "{cut}");
    let total = line_with(out, "total:");
    assert!(total.contains("cost not reported"), "{total}");
}

#[test]
fn a_stopped_run_exits_8_and_prints_what_it_has() {
    let scratch = TempRoot::new();
    let fx = Fixture::new(&cut_recording(&scratch), Setup::default());
    let cli = Cli::of(&fx);
    let handle = cli.spawn(&[&task_arg(&fx)], "");
    let run = first_run(|| fx.events());
    wait_for_spent_tokens(&fx.paths(), &run);
    let mut client = OwnerClient::connect(&fx.paths(), "run-command-test").unwrap();
    let stopped: RunStopResult = client
        .call(
            "run.stop",
            &RunStopParams {
                run: run.clone(),
                grace_secs: Some(1),
            },
            Some(Duration::from_secs(60)),
        )
        .unwrap();
    assert_eq!(stopped.state, RunState::Stopped {});
    let outcome = handle.join().unwrap();
    assert_eq!(outcome.code, 8, "{outcome:?}");
    assert!(outcome.out.contains(": stopped\n"), "{}", outcome.out);
    line_with(&outcome.out, "turn 1:");
}

// ---- permission requests ------------------------------------------------------------------------------------------

/// `permission-request` with the answer it expects replaced by `expected`, and the request's input removed when
/// `without_input`.
fn permission_recording(root: &TempRoot, expected: Value, without_input: bool) -> PathBuf {
    let text = fs::read_to_string(recording("permission-request")).unwrap();
    let mut lines = Vec::new();
    for line in text.lines() {
        if line.starts_with("{\"expect\":{\"type\":\"control_response\"") {
            lines.push(
                json!({"expect": {"type": "control_response", "response": {"subtype": "success",
                "request_id": "req-01", "response": expected}}})
                .to_string(),
            );
        } else if without_input && line.contains("\"control_request\"") {
            let mut step: Value = serde_json::from_str(line).unwrap();
            step["say"]["request"]
                .as_object_mut()
                .unwrap()
                .remove("input");
            lines.push(step.to_string());
        } else {
            lines.push(line.to_string());
        }
    }
    let path = root.join(if without_input {
        "no-input.jsonl"
    } else {
        "ask.jsonl"
    });
    fs::write(&path, lines.join("\n")).unwrap();
    path
}

#[test]
fn a_permission_request_is_asked_on_the_terminal_and_a_y_allows_it_once() {
    let fx = Fixture::new(&recording("permission-request"), Setup::default());
    let outcome = Cli::of(&fx).run(&[&task_arg(&fx)], "y\n");
    assert_eq!(outcome.code, 0, "{outcome:?}");
    let out = &outcome.out;
    assert!(out.contains("The agent asks to use Bash"), "{out}");
    assert!(out.contains("touch out.txt"), "the input is shown: {out}");
    assert!(out.contains("Allow this once? [y/N]"), "{out}");
    assert!(out.contains("allowed once"), "{out}");
    assert!(!out.to_lowercase().contains("always"), "{out}");
    assert!(
        out.contains(": ended\n"),
        "the fake exits 97 on an answer it did not expect: {out}"
    );
}

#[test]
fn an_n_denies_the_request_and_the_deny_reaches_claude() {
    let scratch = TempRoot::new();
    let path = permission_recording(&scratch, json!({"behavior": "deny"}), false);
    let fx = Fixture::new(&path, Setup::default());
    let outcome = Cli::of(&fx).run(&[&task_arg(&fx)], "n\n");
    assert_eq!(outcome.code, 0, "{outcome:?}");
    assert!(outcome.out.contains("denied once"), "{}", outcome.out);
    assert!(
        !outcome.out.to_lowercase().contains("always"),
        "{}",
        outcome.out
    );
}

#[test]
fn a_request_whose_input_the_agent_did_not_give_says_it_is_not_available_until_approved() {
    let scratch = TempRoot::new();
    let path = permission_recording(&scratch, json!({"behavior": "allow"}), true);
    let fx = Fixture::new(&path, Setup::default());
    let outcome = Cli::of(&fx).run(&[&task_arg(&fx)], "y\n");
    assert_eq!(outcome.code, 0, "{outcome:?}");
    assert!(
        outcome.out.contains("input not available until approved"),
        "{}",
        outcome.out
    );
    assert!(!outcome.out.contains("touch out.txt"), "{}", outcome.out);
}

#[test]
fn input_that_ends_with_a_request_unanswered_leaves_it_in_the_queue_and_resume_asks_again() {
    let mut fx = Fixture::new(&recording("permission-request"), Setup::default());
    let cli = Cli::of(&fx);
    let first = cli.run(&[&task_arg(&fx)], "");
    assert_eq!(first.code, 9, "{first:?}");
    assert!(first.out.contains("stays pending"), "{}", first.out);
    let run = fx.queue_all()[0].run.clone().expect("a request waits");
    assert!(
        first.out.contains(&format!("collide run --resume {run}")),
        "{}",
        first.out
    );
    assert_eq!(fx.queue(&run).len(), 1, "the request is still in the Queue");

    let second = cli.run(&["--resume", &run], "y\n");
    assert_eq!(second.code, 0, "{second:?}");
    assert!(
        second.out.contains("The agent asks to use Bash"),
        "{}",
        second.out
    );
    assert!(second.out.contains(": ended\n"), "{}", second.out);
    assert!(fx.queue(&run).is_empty());
}

#[test]
fn an_answer_that_is_neither_y_nor_n_is_asked_again() {
    let fx = Fixture::new(&recording("permission-request"), Setup::default());
    let outcome = Cli::of(&fx).run(&[&task_arg(&fx)], "always\nmaybe\ny\n");
    assert_eq!(outcome.code, 0, "{outcome:?}");
    assert_eq!(
        outcome.out.matches("Allow this once? [y/N]").count(),
        3,
        "{}",
        outcome.out
    );
    assert!(
        outcome
            .out
            .contains("Answer y to allow this once, or n to deny it.")
    );
}

// ---- --resume ---------------------------------------------------------------------------------------------------

#[test]
fn resume_reattaches_to_a_live_run_started_elsewhere_and_follows_it_to_its_end() {
    let mut fx = Fixture::new(&recording("permission-request"), Setup::default());
    let run = fx.started();
    fx.queue_all();
    let outcome = Cli::of(&fx).run(&["--resume", &run], "y\n");
    assert_eq!(outcome.code, 0, "{outcome:?}");
    assert!(
        outcome.out.contains("The agent asks to use Bash"),
        "{}",
        outcome.out
    );
    assert!(outcome.out.contains(": ended\n"), "{}", outcome.out);
    assert!(outcome.out.contains("claude in use"), "{}", outcome.out);
}

#[test]
fn resume_of_a_run_that_is_over_prints_its_record_and_exits_with_its_end() {
    let mut fx = Fixture::new(&recording("report"), Setup::default());
    let run = fx.started();
    fx.wait_run(&run, |state| matches!(state, RunState::Ended { .. }));
    let outcome = Cli::of(&fx).run(&["--resume", &run], "");
    assert_eq!(outcome.code, 0, "{outcome:?}");
    assert!(
        outcome.out.contains("The notes start with the first line."),
        "{}",
        outcome.out
    );
    line_with(&outcome.out, "turn 1:");
    assert!(!outcome.out.contains("Nothing resumes"), "{}", outcome.out);
}

#[test]
fn resume_of_a_run_that_failed_says_nothing_resumes_it_and_exits_7() {
    let scratch = TempRoot::new();
    let path = scratch.join("no-result.jsonl");
    fs::write(
        &path,
        [
            r#"{"expect":{"type":"user"}}"#,
            r#"{"say":{"type":"system","subtype":"init","session_id":"7c1b0e52-9f3a-4d2e-8b61-0a5d3c9e4f17","cwd":"/work","tools":[],"mcp_servers":[],"model":"m","permissionMode":"default","apiKeySource":"none"}}"#,
            r#"{"exit":0}"#,
        ]
        .join("\n"),
    )
    .unwrap();
    let mut fx = Fixture::new(&path, Setup::default());
    let run = fx.started();
    fx.wait_run(&run, |state| matches!(state, RunState::Failed {}));
    wait_until("the failure is recorded", Duration::from_secs(30), || {
        record_of(&fx.paths(), &run).state == RunState::Failed {}
    });
    let outcome = Cli::of(&fx).run(&["--resume", &run], "");
    assert_eq!(outcome.code, 7, "{outcome:?}");
    assert!(outcome.out.contains(": failed\n"), "{}", outcome.out);
    assert!(
        outcome.out.contains("failure: no_result"),
        "{}",
        outcome.out
    );
    assert!(
        outcome.out.contains("Nothing resumes this run"),
        "{}",
        outcome.out
    );
}

#[test]
fn resume_of_a_run_the_daemon_does_not_know_exits_3() {
    let fx = Fixture::new(&recording("report"), Setup::default());
    let outcome = Cli::of(&fx).run(&["--resume", "no-such-run"], "");
    assert_eq!(outcome.code, 3, "{outcome:?}");
    assert!(outcome.err.contains("no run"), "{}", outcome.err);
}

#[test]
fn resume_answers_the_resume_item_of_a_run_paused_for_a_usage_limit() {
    let mut rig = Rig::new(
        &recording("limit"),
        &recording("report"),
        Settings::default(),
    );
    let run = rig.started(None);
    rig.wait_item(&run, QueueKind::UsageLimit);
    let outcome = Cli::of_rig(&rig).run(&["--resume", &run], "");
    assert_eq!(outcome.code, 0, "{outcome:?}");
    assert!(outcome.out.contains("resuming: "), "{}", outcome.out);
    assert!(outcome.out.contains(": ended\n"), "{}", outcome.out);
    assert_eq!(
        rig.launches().len(),
        2,
        "claude was started again, by session"
    );
    assert!(rig.queue(&run).is_empty());
}

#[test]
fn following_a_run_that_pauses_for_a_usage_limit_says_so_and_goes_on_to_its_end() {
    let mut rig = Rig::new(
        &recording("limit"),
        &recording("report"),
        Settings::default(),
    );
    let cli = Cli::of_rig(&rig);
    let task = rig.task.to_str().unwrap().to_string();
    let handle = cli.spawn(&[&task], "");
    let run = first_run(|| rig.events());
    rig.wait_item(&run, QueueKind::UsageLimit);
    // Give the command time to look at the Queue, then move the wall clock past the reset.
    thread::sleep(Duration::from_millis(300));
    rig.clocks.set_step(Duration::from_secs(2 * 3600));
    let outcome = handle.join().unwrap();
    assert_eq!(outcome.code, 0, "{outcome:?}");
    assert!(
        outcome.out.contains("run ") && outcome.out.contains("paused: usage limit"),
        "{}",
        outcome.out
    );
    assert!(
        outcome.out.contains("resumes by itself at the reset"),
        "{}",
        outcome.out
    );
    assert!(outcome.out.contains(": ended\n"), "{}", outcome.out);
}

#[test]
fn a_full_disk_leaves_the_run_for_the_owner_and_resume_continues_it() {
    use super::file_run::FakeFree;
    let mut fx = Fixture::new(
        &recording("report"),
        Setup {
            free: Some(FakeFree::new(&[0, u64::MAX])),
            ..Setup::default()
        },
    );
    let cli = Cli::of(&fx);
    let first = cli.run(&[&task_arg(&fx)], "");
    assert_eq!(first.code, 9, "{first:?}");
    assert!(first.out.contains("Free some space"), "{}", first.out);
    let run = fx.queue_all()[0].run.clone().unwrap();
    assert!(
        first.out.contains(&format!("collide run --resume {run}")),
        "{}",
        first.out
    );

    let second = cli.run(&["--resume", &run], "");
    assert_eq!(second.code, 0, "{second:?}");
    assert!(second.out.contains("resuming: "), "{}", second.out);
    assert!(second.out.contains(": ended\n"), "{}", second.out);
}

#[test]
fn a_budget_that_ran_out_leaves_the_run_and_says_how_to_stop_it() {
    let mut fx = Fixture::new(&recording("error-result"), Setup::default());
    let outcome = Cli::of(&fx).run(&[&task_arg(&fx), "--budget", "0.0001"], "");
    assert_eq!(outcome.code, 9, "{outcome:?}");
    let run = fx.queue_all()[0].run.clone().unwrap();
    assert!(outcome.out.contains("budget"), "{}", outcome.out);
    assert!(
        outcome.out.contains(&format!("collide stop {run}")),
        "{}",
        outcome.out
    );
}

// ---- the task file and the daemon's refusals ------------------------------------------------------------------

#[test]
fn an_alias_with_no_mapping_is_refused_and_the_message_shows_the_mapping_to_add() {
    let fx = Fixture::new(&recording("report"), Setup::default());
    let outcome = Cli::of(&fx).run(&[&task_arg(&fx), "--alias", "nosuch"], "");
    assert_eq!(outcome.code, 5, "{outcome:?}");
    assert!(outcome.err.contains("refused"), "{}", outcome.err);
    assert!(outcome.err.contains("nosuch"), "{}", outcome.err);
    assert!(
        outcome
            .err
            .contains("models.yaml does not map the alias `nosuch`"),
        "{}",
        outcome.err
    );
    assert!(
        outcome.err.contains("nosuch: { name: haiku }"),
        "{}",
        outcome.err
    );
}

#[test]
fn with_no_models_file_the_message_says_where_it_goes_and_maps_fast() {
    let fx = Fixture::new(&recording("report"), Setup::default());
    fs::remove_file(fx.root.join("home/.config/collide/models.yaml")).unwrap();
    let outcome = Cli::of(&fx).run(&[&task_arg(&fx)], "");
    assert_eq!(outcome.code, 5, "{outcome:?}");
    let home = fx.root.join("home");
    assert!(
        outcome
            .err
            .contains(&format!("{}/.config/collide/models.yaml", home.display())),
        "{}",
        outcome.err
    );
    assert!(
        outcome.err.contains("There is no models.yaml"),
        "{}",
        outcome.err
    );
    assert!(
        outcome.err.contains("fast: { name: haiku }"),
        "{}",
        outcome.err
    );
}

#[test]
fn a_models_file_that_does_not_load_is_named_once_and_no_mapping_is_offered() {
    let fx = Fixture::new(&recording("report"), Setup::default());
    let file = fx.root.join("home/.config/collide/models.yaml");
    fs::write(
        &file,
        "tools:\n  claude:\n    fast: { name: haiku }\nagents: {}\n",
    )
    .unwrap();
    let outcome = Cli::of(&fx).run(&[&task_arg(&fx)], "");
    assert_eq!(outcome.code, 5, "{outcome:?}");
    assert!(outcome.err.contains("refused"), "{}", outcome.err);
    assert!(
        outcome.err.contains(&file.display().to_string()),
        "{}",
        outcome.err
    );
    assert!(outcome.err.contains("agents"), "{}", outcome.err);
    assert_eq!(
        outcome.err.matches("unknown field").count(),
        1,
        "{}",
        outcome.err
    );
    assert_eq!(
        outcome.err.matches("models.yaml").count(),
        1,
        "{}",
        outcome.err
    );
    assert!(!outcome.err.contains("does not map"), "{}", outcome.err);
    assert!(!outcome.err.contains("{ name: haiku }"), "{}", outcome.err);
}

#[test]
fn a_models_path_that_cannot_be_read_is_named_and_no_mapping_is_offered() {
    let fx = Fixture::new(&recording("report"), Setup::default());
    let file = fx.root.join("home/.config/collide/models.yaml");
    fs::remove_file(&file).unwrap();
    fs::create_dir(&file).unwrap();
    let outcome = Cli::of(&fx).run(&[&task_arg(&fx)], "");
    assert_eq!(outcome.code, 5, "{outcome:?}");
    assert!(
        outcome.err.contains(&file.display().to_string()),
        "{}",
        outcome.err
    );
    assert!(!outcome.err.contains("does not map"), "{}", outcome.err);
    assert!(
        !outcome.err.contains("There is no models.yaml"),
        "{}",
        outcome.err
    );
}

#[test]
fn a_budget_that_is_not_above_zero_is_a_usage_error_and_no_daemon_is_asked() {
    let fx = Fixture::new(&recording("report"), Setup::default());
    let cli = Cli::of(&fx);
    for args in [
        vec![],
        vec!["a.md", "b.md"],
        vec!["a.md", "--budget", "0"],
        vec!["a.md", "--budget", "lots"],
        vec!["a.md", "--wat"],
        vec!["--resume", "r", "a.md"],
    ] {
        let outcome = cli.run(&args, "");
        assert_eq!(outcome.code, 2, "{args:?}: {outcome:?}");
        assert!(
            outcome.err.contains("usage: collide run"),
            "{}",
            outcome.err
        );
    }
    assert_eq!(cli.daemon_starts(), 0);
}

#[test]
fn a_task_file_that_is_not_there_exits_3_before_the_daemon_is_asked() {
    let fx = Fixture::new(&recording("report"), Setup::default());
    let cli = Cli::of(&fx);
    let outcome = cli.run(&["no-such-task.md"], "");
    assert_eq!(outcome.code, 3, "{outcome:?}");
    assert!(outcome.err.contains("no task file at"), "{}", outcome.err);
    assert_eq!(cli.daemon_starts(), 0);
}

#[test]
fn a_task_file_outside_any_git_checkout_exits_1_and_says_so() {
    let fx = Fixture::new(&recording("report"), Setup::default());
    let loose = fx.root.join("loose");
    fs::create_dir(&loose).unwrap();
    fs::write(loose.join("task.md"), "Do it.\n").unwrap();
    let cli = Cli::new(fx.paths(), fx.root.join("home"), loose);
    let outcome = cli.run(&["task.md"], "");
    assert_eq!(outcome.code, 1, "{outcome:?}");
    assert!(outcome.err.contains("git repository"), "{}", outcome.err);
    assert_eq!(cli.daemon_starts(), 0);
}

#[test]
fn a_daemon_that_does_not_start_exits_1_with_its_reason() {
    let fx = Fixture::new(&recording("report"), Setup::default());
    let mut env = Cli::of(&fx).env();
    env.start_daemon = Box::new(|_| Err("no collide-daemon beside collide".to_string()));
    let args = vec![task_arg(&fx)];
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = run_with(
        &env,
        &args,
        &mut Cursor::new(Vec::new()),
        &mut out,
        &mut err,
    );
    assert_eq!(code, 1);
    let err = String::from_utf8(err).unwrap();
    assert!(
        err.contains("did not start: no collide-daemon beside collide"),
        "{err}"
    );
    assert!(out.is_empty());
}

#[test]
fn an_item_of_the_planning_pack_is_a_task_file_and_the_repo_is_the_one_that_holds_it() {
    let fx = Fixture::new(&recording("report"), Setup::default());
    let tasks = fx.repo.join(".collide/planning/epics/ep-001-aa-demo/tasks");
    fs::create_dir_all(&tasks).unwrap();
    let item = tasks.join("ts-0001-ab-add-a-line.md");
    fs::write(
        &item,
        "---\nid: ts-0001-ab\nkind: task\ntitle: Add a line\nstatus: ready\n---\n\nAdd a line to README.\n",
    )
    .unwrap();
    git(&fx.repo, &["add", "."]);
    git(&fx.repo, &["commit", "-q", "-m", "an item"]);
    // The command runs from a folder that is no checkout, and names the item by a relative path.
    let cli = Cli::new(
        fx.paths(),
        fx.root.join("home"),
        fx.root.path().to_path_buf(),
    );
    let outcome = cli.run(
        &["repo/.collide/planning/epics/ep-001-aa-demo/tasks/ts-0001-ab-add-a-line.md"],
        "",
    );
    assert_eq!(outcome.code, 0, "{outcome:?}");
    let worktree = line_with(&outcome.out, "worktree: ");
    let path = worktree
        .trim_start_matches("worktree: ")
        .split(" (branch")
        .next()
        .unwrap();
    assert!(
        Path::new(path)
            .join(".collide/planning/epics/ep-001-aa-demo/tasks/ts-0001-ab-add-a-line.md")
            .is_file(),
        "the worktree is of the repo that holds the item: {worktree}"
    );
}

// ---- the answers the command sends, as the daemon sees them ---------------------------------------------------------

#[test]
fn the_command_never_sends_an_always_answer_and_the_wire_type_has_none() {
    // `PermissionReply` has no always variant; the only replies the command can build are these.
    for reply in [
        PermissionReply::Allow {},
        PermissionReply::Deny { message: None },
        PermissionReply::Cancel {},
    ] {
        let wire = serde_json::to_string(&QueueAnswerParams {
            item: "q1".to_string(),
            answer: QueueAnswer::Permission { reply },
        })
        .unwrap();
        assert!(!wire.to_lowercase().contains("always"), "{wire}");
    }
    let _ = QueueAnswerResult::Done {};
}
