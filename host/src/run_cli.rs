//! `collide run` (Stage 3b; ts-0069-bn; tag `cli: run`): one Claude phase on a task given as a file, in a new git
//! worktree, through the daemon.
//!
//! ```text
//! collide run <task-file> [--budget <usd>] [--alias <name>]
//! collide run --resume <run>
//! ```
//!
//! **What it does**, in order: reads its command line (a wrong one starts no daemon), finds the task file and the
//! repository it is in, starts the daemon when none answers ([`crate::launch::ensure_running_from_cli`]; under
//! `COLLIDE_DAEMON_ROOT` no launchd job is loaded), prints the `claude` version in use and the published one
//! (`daemon.agent_versions`), starts the run (`run.start_file`), and follows it in the foreground: it polls
//! `run.record` for the run's state and `queue.list` for what waits on a person.
//!
//! **A permission request is asked on the terminal** and answered with `queue.answer`. The prompt offers allow once and
//! deny and nothing else: no "always" is offered, whatever options the agent listed (0.3, Permissions). A request whose
//! input the agent did not give says "input not available until approved". The answer is read from standard input, so
//! no terminal is needed; when the input ends with a request unanswered, the request stays in the daemon's Queue and
//! the command exits [`exit::WAITING`]: `collide run --resume <run>` asks again.
//!
//! **`--resume <run>`** is the one way a person comes back to a run (there is no `run.resume` method; ts-0066-zb,
//! decision 2). A live run is reattached to and followed. A paused run whose Queue item takes `Resume` (a usage limit
//! before its reset, a full disk the owner has freed) is resumed with that answer. A run that is over shows its record.
//! A phase that is lost has no Queue item and nothing here can resume it; the record says so.
//!
//! **At the end** it prints the record ([`record_text`]): the state, the report, one line for each model call with its
//! wall time, model time, tokens and cost, and a total. A figure the agent did not give is "not reported", never zero.
//!
//! **Exit codes** (`EXIT_CODES`): 0 the phase ended; 1 the command could not do its work (no daemon, no repository,
//! an error from the daemon); 2 a wrong command line; 3 the task file or the run does not exist; 5 the daemon refused
//! to start the run; 7 the run failed; 8 the run was stopped; 9 the run is waiting for a person and the command left it.

use crate::builtin_roles::BUILDER_MODEL_ALIAS;
use crate::daemon::{DaemonPaths, ROOT_ENV};
use crate::launch::ensure_running_from_cli;
use crate::models::{load_user_models, models_path};
use crate::owner_client::{ClientError, OwnerClient};
use crate::task_cli::exit;
use collide_core::agent::{Money, PermissionReply, PermissionRequest, UsageQuality, UsageScope};
use collide_core::owner::{
    AgentVersions, AnswerKind, DaemonAgentVersionsParams, DaemonAgentVersionsResult, QueueAnswer,
    QueueAnswerParams, QueueAnswerResult, QueueItem, QueueKind, QueueListParams, QueueListResult,
    RecordedCall, RunRecordParams, RunRecordResult, RunStartFileParams, RunStartResult, code,
};
use collide_core::state::{PauseCause, RunState};
use std::collections::HashSet;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

pub const USAGE: &str = "usage: collide run <task-file> [--budget <usd>] [--alias <name>]\n       collide run --resume <run>";

/// What `collide --help` prints about `collide run`.
pub const HELP: &str = "\
collide run <task-file>: run one Claude phase on a task file (a markdown file; an item of .collide/planning works too)
  in a new git worktree of the repository the file is in. It starts the daemon when none answers, prints the claude
  version in use and the published one (only the one in use when claude.check_published is false), follows the
  phase in the foreground, asks each permission request on the terminal (y allows it once, n denies it; there is no
  always), and prints the record at the end: the outcome and report, then wall time, model time, tokens and cost for
  each turn, with \"not reported\" for a value the agent did not give. Closing it leaves the run going.
  --budget <usd>: stop the phase when its cost passes this many US dollars
  --alias <name>: the model alias (the built-in builder role's, fast, when left out), mapped in ~/.config/collide/models.yaml
collide run --resume <run>: come back to a run. A live run is followed again, a paused one is resumed with the Queue's
  Resume answer first, and a run that is over shows its record.
exit codes of collide run: 0 the phase ended, 1 failed to do its work (no daemon, no repository, the daemon's own
  error), 2 wrong command line, 3 no such task file or run, 5 the daemon refused to start the run, 7 the run failed,
  8 the run was stopped, 9 the run is waiting for a person (a permission request with no answer, a full disk, a
  budget that ran out) and the command left it";

/// How long one call may take: a start waits for the first capture of the login shell's environment and the first
/// phase's version check.
const START_WAIT: Duration = Duration::from_secs(300);
const CALL_WAIT: Duration = Duration::from_secs(60);

/// How long the version lines wait for the published version, which the daemon fetches on its own thread at start.
const PUBLISHED_WAIT: Duration = Duration::from_secs(8);

/// A figure the agent did not give.
const NOT_REPORTED: &str = "not reported";

/// What the command line asked.
#[derive(Debug, Clone, PartialEq)]
pub enum Target {
    /// A new run of this task file.
    Task(String),
    /// Come back to this run.
    Resume(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct RunArgs {
    pub target: Target,
    pub budget_usd: Option<f64>,
    pub alias: Option<String>,
}

/// Reads the arguments after `run`. The error is the message to print with the usage.
pub fn parse(args: &[String]) -> Result<RunArgs, String> {
    let mut task: Option<String> = None;
    let mut resume: Option<String> = None;
    let mut budget_usd: Option<f64> = None;
    let mut alias: Option<String> = None;
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--budget" => {
                let value = rest
                    .next()
                    .ok_or("--budget needs an amount in US dollars")?;
                let usd: f64 = value
                    .parse()
                    .map_err(|_| format!("--budget {value:?} is not a number of US dollars"))?;
                if !usd.is_finite() || usd <= 0.0 {
                    return Err(format!("--budget {value} must be above zero"));
                }
                budget_usd = Some(usd);
            }
            "--alias" => {
                let value = rest.next().ok_or("--alias needs a model alias")?;
                if value.is_empty() {
                    return Err("--alias is empty".to_string());
                }
                alias = Some(value.clone());
            }
            "--resume" => {
                let value = rest.next().ok_or("--resume needs a run")?;
                if value.is_empty() {
                    return Err("the run's id is empty".to_string());
                }
                if resume.replace(value.clone()).is_some() {
                    return Err("--resume names one run".to_string());
                }
            }
            other if other.starts_with('-') => return Err(format!("unknown option `{other}`")),
            other => {
                if task.replace(other.to_string()).is_some() {
                    return Err("only one task file can be run at a time".to_string());
                }
            }
        }
    }
    let target = match (task, resume) {
        (Some(_), Some(_)) => {
            return Err("name a task file or --resume <run>, not both".to_string());
        }
        (Some(task), None) => Target::Task(task),
        (None, Some(run)) => {
            if budget_usd.is_some() || alias.is_some() {
                return Err(
                    "--resume takes no --budget or --alias: the run has its own".to_string()
                );
            }
            Target::Resume(run)
        }
        (None, None) => return Err("name the task file to run".to_string()),
    };
    Ok(RunArgs {
        target,
        budget_usd,
        alias,
    })
}

/// What makes sure a daemon answers on the paths: its error is the reason it did not start.
pub type StartDaemon = Box<dyn Fn(&DaemonPaths) -> Result<(), String>>;

/// What the command works with. `run` makes it from the process; a test makes its own, so a daemon it started
/// in-process is the one the command talks to.
pub struct Env {
    pub paths: DaemonPaths,
    /// `$HOME`, where `models.yaml` is looked for to explain a refused alias.
    pub home: Option<String>,
    pub cwd: PathBuf,
    /// Makes sure a daemon answers on `paths`.
    pub start_daemon: StartDaemon,
    /// How often the command looks at the run.
    pub poll: Duration,
    /// The top of the git checkout that holds a folder, if one does. `run` gives [`toplevel`], which asks the `git` on
    /// the command's own PATH with its own environment; a test gives one with a cleared environment.
    pub toplevel: fn(&Path) -> Option<PathBuf>,
}

/// Runs the command. `args` are the arguments after `run`; permission answers are read from `input`.
pub fn run(
    args: &[String],
    input: &mut dyn BufRead,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let parsed = match parse(args) {
        Ok(parsed) => parsed,
        Err(message) => {
            let _ = writeln!(err, "collide run: {message}\n{USAGE}");
            return exit::USAGE;
        }
    };
    let home = std::env::var("HOME").ok();
    let paths =
        match DaemonPaths::from_env(std::env::var(ROOT_ENV).ok().as_deref(), home.as_deref()) {
            Ok(paths) => paths,
            Err(message) => {
                let _ = writeln!(err, "collide run: {message}");
                return exit::FAILED;
            }
        };
    let cwd = match std::env::current_dir() {
        Ok(cwd) => cwd,
        Err(error) => {
            let _ = writeln!(
                err,
                "collide run: cannot read the working directory: {error}"
            );
            return exit::FAILED;
        }
    };
    let env = Env {
        paths,
        home,
        cwd,
        start_daemon: Box::new(ensure_running_from_cli),
        poll: Duration::from_millis(250),
        toplevel,
    };
    run_in(&env, &parsed, input, out, err)
}

/// [`parse`] and [`run_in`] together.
pub fn run_with(
    env: &Env,
    args: &[String],
    input: &mut dyn BufRead,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    match parse(args) {
        Ok(parsed) => run_in(env, &parsed, input, out, err),
        Err(message) => {
            let _ = writeln!(err, "collide run: {message}\n{USAGE}");
            exit::USAGE
        }
    }
}

/// The task file as an absolute path, and the repository it is in: the checkout that holds the file, else the one the
/// command was run in.
fn locate(env: &Env, given: &str) -> Result<(PathBuf, PathBuf), (u8, String)> {
    let cwd = env.cwd.as_path();
    let file = std::fs::canonicalize(cwd.join(given))
        .ok()
        .filter(|path| path.is_file())
        .ok_or_else(|| (exit::ID, format!("no task file at {given:?}")))?;
    let dir = file.parent().unwrap_or(cwd);
    let repo = (env.toplevel)(dir)
        .or_else(|| (env.toplevel)(cwd))
        .ok_or_else(|| {
            (
                exit::FAILED,
                format!(
                    "neither {} nor the working directory is inside a git repository, so there is nothing to make a worktree of",
                    dir.display()
                ),
            )
        })?;
    Ok((file, repo))
}

pub fn toplevel(dir: &Path) -> Option<PathBuf> {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
    std::fs::canonicalize(text.trim_end_matches('\n')).ok()
}

fn is_final(state: &RunState) -> bool {
    matches!(
        state,
        RunState::Ended { .. } | RunState::Failed {} | RunState::Stopped {}
    )
}

/// The exit code of a run that is over.
fn final_code(state: &RunState) -> u8 {
    match state {
        RunState::Ended { .. } => exit::OK,
        RunState::Stopped {} => exit::RUN_STOPPED,
        _ => exit::RUN_FAILED,
    }
}

fn state_words(state: &RunState) -> String {
    match state {
        RunState::Running {} => "running".to_string(),
        RunState::Paused { cause } => format!(
            "paused: {}",
            match cause {
                PauseCause::UsageLimit => "usage limit",
                PauseCause::Disk => "disk full",
                PauseCause::AskHuman => "waiting for a person",
            }
        ),
        RunState::Integrating {} => "integrating".to_string(),
        RunState::Ended { .. } => "ended".to_string(),
        RunState::Failed {} => "failed".to_string(),
        RunState::Stopped {} => "stopped".to_string(),
    }
}

// ---- the lines it prints ---------------------------------------------------------------------------------------

/// `1234567` as `1,234,567`.
fn with_commas(n: u64) -> String {
    let digits = n.to_string();
    let mut text = String::new();
    for (at, digit) in digits.chars().enumerate() {
        if at > 0 && (digits.len() - at).is_multiple_of(3) {
            text.push(',');
        }
        text.push(digit);
    }
    text
}

/// A length in milliseconds, or "not reported".
fn time_text(ms: Option<u64>) -> String {
    match ms {
        None => NOT_REPORTED.to_string(),
        Some(ms) if ms < 1000 => format!("{ms} ms"),
        Some(ms) => format!("{:.1} s", ms as f64 / 1000.0),
    }
}

fn money_text(cost: &Money) -> String {
    if cost.currency == "USD" {
        format!("${:.4}", cost.amount)
    } else {
        format!("{:.4} {}", cost.amount, cost.currency)
    }
}

/// A token count, or "not reported". `at_least` says the count came from a partial figure.
fn count_text(n: Option<u64>, at_least: bool) -> String {
    match n {
        None => NOT_REPORTED.to_string(),
        Some(n) if at_least => format!("at least {}", with_commas(n)),
        Some(n) => with_commas(n),
    }
}

fn call_label(scope: UsageScope, turn_number: usize) -> String {
    match scope {
        UsageScope::Turn => format!("turn {turn_number}"),
        UsageScope::Step => format!("turn {turn_number} (cut before its result)"),
        UsageScope::Compaction => "compaction".to_string(),
        UsageScope::OtherCalls => "other model calls".to_string(),
        UsageScope::RunningTotal => "running total".to_string(),
    }
}

/// One model call: its wall time, model time, tokens and cost, each "not reported" when the agent gave none.
pub fn call_text(call: &RecordedCall, turn_number: usize) -> String {
    let partial = call.quality == UsageQuality::Partial;
    format!(
        "{}: wall {}, model time {}, input {}, output {}, cache read {}, cache write {}, cost {}",
        call_label(call.scope, turn_number),
        time_text(call.wall_ms),
        time_text(call.model_ms),
        count_text(call.input, partial),
        count_text(call.output, partial),
        count_text(call.cache_read, partial),
        count_text(call.cache_write, partial),
        call.cost
            .as_ref()
            .map_or(NOT_REPORTED.to_string(), money_text),
    )
}

/// A figure summed over the calls. "At least" when a call lacked it or gave only a partial figure; "not reported" when
/// no call gave it.
fn total_of(calls: &[RecordedCall], figure: impl Fn(&RecordedCall) -> Option<u64>) -> String {
    let given: Vec<u64> = calls.iter().filter_map(&figure).collect();
    if given.is_empty() {
        return NOT_REPORTED.to_string();
    }
    let floor = given.len() < calls.len()
        || calls
            .iter()
            .any(|call| call.quality == UsageQuality::Partial && figure(call).is_some());
    count_text(Some(given.iter().sum()), floor)
}

fn total_cost(calls: &[RecordedCall]) -> String {
    let usd: Vec<f64> = calls
        .iter()
        .filter_map(|call| call.cost.as_ref())
        .filter(|cost| cost.currency == "USD")
        .map(|cost| cost.amount)
        .collect();
    if usd.is_empty() {
        return NOT_REPORTED.to_string();
    }
    let sum = format!("${:.4}", usd.iter().sum::<f64>());
    if usd.len() < calls.len() {
        format!("at least {sum}")
    } else {
        sum
    }
}

/// A line of the agent's own words with its control characters dropped, so a terminal is not driven by them.
fn printable(line: &str) -> String {
    line.chars().filter(|c| !c.is_control()).collect()
}

/// The record of a run, as it is printed at the end and when the command reattaches to a run that is over.
pub fn record_text(record: &RunRecordResult) -> String {
    let mut text = format!("run {}: {}\n", record.run, state_words(&record.state));
    if let Some(workspace) = &record.workspace {
        text.push_str(&format!(
            "worktree: {workspace} (branch collide/{})\n",
            record.run
        ));
    }
    if let Some(failure) = &record.failure {
        text.push_str(&format!("failure: {failure}\n"));
        if let Some(evidence) = &record.failure_evidence {
            text.push_str("evidence:\n");
            for line in evidence.lines() {
                text.push_str(&format!("  {}\n", printable(line)));
            }
        }
    }
    match &record.report {
        Some(report) => {
            text.push_str("report:\n");
            for line in report.lines() {
                text.push_str(&format!("  {line}\n"));
            }
        }
        None => text.push_str(&format!("report: {NOT_REPORTED}\n")),
    }
    text.push_str("model calls:\n");
    if record.calls.is_empty() {
        text.push_str(&format!("  {NOT_REPORTED}\n"));
    }
    let mut turn_number = 0;
    for call in &record.calls {
        if matches!(call.scope, UsageScope::Turn | UsageScope::Step) {
            turn_number += 1;
        }
        text.push_str(&format!("  {}\n", call_text(call, turn_number)));
    }
    text.push_str(&format!(
        "total: wall {}, input {}, output {}, cache read {}, cache write {}, cost {}\n",
        time_text(record.wall_ms),
        total_of(&record.calls, |c| c.input),
        total_of(&record.calls, |c| c.output),
        total_of(&record.calls, |c| c.cache_read),
        total_of(&record.calls, |c| c.cache_write),
        total_cost(&record.calls),
    ));
    text
}

fn published_chip(agent: &AgentVersions) -> Option<String> {
    let published = agent.published.as_ref()?;
    Some(match (&published.latest, &published.stable) {
        (None, None) => match &published.error {
            Some(error) => format!("claude published: not known ({error})"),
            None => "claude published: not known yet".to_string(),
        },
        (latest, stable) => format!(
            "claude published: latest {}, stable {}",
            latest.as_deref().unwrap_or("not known"),
            stable.as_deref().unwrap_or("not known")
        ),
    })
}

/// The lines printed before a run starts: the version in use, and the published one unless the daemon is not asked to
/// fetch it (`claude.check_published: false`).
pub fn version_lines(agent: &AgentVersions) -> Vec<String> {
    let mut lines = Vec::new();
    lines.push(match (&agent.in_use, &agent.in_use_error) {
        (Some(version), _) => {
            let range = format!("{} through {}", agent.tested.from, agent.tested.through);
            match agent.in_tested_range {
                Some(false) => {
                    let smoke = match &agent.smoke {
                        Some(smoke) if smoke.passed => "its smoke check passed".to_string(),
                        Some(smoke) => format!(
                            "its smoke check failed: {}",
                            smoke.reason.as_deref().unwrap_or("no reason given")
                        ),
                        None => "its smoke check has not run yet".to_string(),
                    };
                    format!("claude in use: {version} (outside the tested range {range}; {smoke})")
                }
                _ => format!("claude in use: {version} (tested range {range})"),
            }
        }
        (None, Some(error)) => format!("claude in use: not known ({error})"),
        (None, None) => "claude in use: not known".to_string(),
    });
    lines.extend(published_chip(agent));
    lines
}

/// What to tell a person whose alias has no mapping: where `models.yaml` goes and a mapping for the alias.
pub fn models_hint(home: Option<&str>, alias: &str, file_exists: bool) -> String {
    let path = models_path(home)
        .map(|path| path.display().to_string())
        .unwrap_or_else(|_| "~/.config/collide/models.yaml".to_string());
    let lead = if file_exists {
        format!("{path} does not map the alias `{alias}`. Add it under tools.claude:")
    } else {
        format!("There is no models.yaml. It goes at {path}. A mapping for the alias `{alias}`:")
    };
    format!(
        "{lead}\n  tools:\n    claude:\n      {alias}: {{ name: haiku }}\n(`name` is the model Claude is asked for: `haiku`, `sonnet` or `opus`, or a full model name.)"
    )
}

// ---- talking to the daemon ---------------------------------------------------------------------------------------

fn call_error(err: &mut dyn Write, error: &ClientError) -> u8 {
    let _ = writeln!(err, "collide run: {error}");
    exit::FAILED
}

/// The version lines, waiting a little for the published version when the daemon has not fetched it yet.
fn print_versions(client: &mut OwnerClient, poll: Duration, out: &mut dyn Write) {
    let started = std::time::Instant::now();
    loop {
        let versions: Result<DaemonAgentVersionsResult, ClientError> = client.call(
            "daemon.agent_versions",
            &DaemonAgentVersionsParams {},
            Some(CALL_WAIT),
        );
        match versions {
            Err(error) => {
                let _ = writeln!(out, "claude in use: not known ({error})");
                return;
            }
            Ok(result) => {
                let Some(agent) = result.agents.iter().find(|agent| agent.agent == "claude") else {
                    return;
                };
                let pending = agent.published.as_ref().is_some_and(|published| {
                    published.latest.is_none()
                        && published.stable.is_none()
                        && published.error.is_none()
                });
                if pending && started.elapsed() < PUBLISHED_WAIT {
                    std::thread::sleep(poll.min(Duration::from_millis(100)));
                    continue;
                }
                for line in version_lines(agent) {
                    let _ = writeln!(out, "{line}");
                }
                return;
            }
        }
    }
}

fn record_of(client: &mut OwnerClient, run: &str) -> Result<RunRecordResult, ClientError> {
    client.call(
        "run.record",
        &RunRecordParams {
            run: run.to_string(),
        },
        Some(CALL_WAIT),
    )
}

fn queue_of(client: &mut OwnerClient, run: &str) -> Result<Vec<QueueItem>, ClientError> {
    let result: QueueListResult = client.call(
        "queue.list",
        &QueueListParams {
            board: None,
            run: Some(run.to_string()),
            kinds: Vec::new(),
        },
        Some(CALL_WAIT),
    )?;
    Ok(result.items)
}

fn answer(
    client: &mut OwnerClient,
    item: &str,
    answer: QueueAnswer,
) -> Result<QueueAnswerResult, ClientError> {
    client.call(
        "queue.answer",
        &QueueAnswerParams {
            item: item.to_string(),
            answer,
        },
        Some(CALL_WAIT),
    )
}

/// The agent's own words for what it asks, as short as they can be: the tool, what kind of call it is, why, and the
/// input when the agent gave one. An input the agent has not given is said so: Claude shows a request's input only once
/// it is approved.
fn request_lines(request: &PermissionRequest) -> Vec<String> {
    let kind = serde_json::to_value(request.kind)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_else(|| "other".to_string());
    let mut lines = vec![format!(
        "The agent asks to use {} ({kind}).",
        request.tool.as_deref().unwrap_or("a tool")
    )];
    if let Some(reason) = &request.reason {
        lines.push(format!("  reason: {reason}"));
    }
    match &request.input {
        Some(input) => {
            let mut text = input.to_string();
            if text.chars().count() > 600 {
                text = text.chars().take(600).collect::<String>() + "...";
            }
            lines.push(format!("  input: {text}"));
        }
        None => lines.push("  input not available until approved".to_string()),
    }
    lines
}

/// What the person said to a permission request.
#[derive(Debug, PartialEq, Eq)]
enum Reply {
    Allow,
    Deny,
    /// The input ended before an answer.
    None,
}

/// Reads allow or deny from `input`, asking again for anything else. Only `y` and `n` are offered: the prompt has no
/// option that outlasts this one call.
fn ask(input: &mut dyn BufRead, out: &mut dyn Write) -> Reply {
    loop {
        let _ = write!(out, "Allow this once? [y/N] ");
        let _ = out.flush();
        let mut line = String::new();
        match input.read_line(&mut line) {
            Ok(0) | Err(_) => {
                let _ = writeln!(out);
                return Reply::None;
            }
            Ok(_) => {}
        }
        match line.trim().to_ascii_lowercase().as_str() {
            "y" | "yes" => return Reply::Allow,
            "n" | "no" | "" => return Reply::Deny,
            _ => {
                let _ = writeln!(out, "Answer y to allow this once, or n to deny it.");
            }
        }
    }
}

/// What the loop does after it handled one Queue item.
enum Handled {
    Go,
    /// The run waits for a person and the command leaves it, with this message.
    Leave(String),
}

fn reset_in(reset_ms: Option<u64>) -> String {
    let Some(reset_ms) = reset_ms else {
        return String::new();
    };
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64);
    let minutes = reset_ms.saturating_sub(now_ms) / 60_000;
    format!(" (resets in about {minutes} min)")
}

fn handle_item(
    client: &mut OwnerClient,
    run: &str,
    item: &QueueItem,
    input: &mut dyn BufRead,
    out: &mut dyn Write,
) -> Result<Handled, ClientError> {
    match (&item.kind, &item.permission) {
        (QueueKind::Permission, Some(request)) => {
            for line in request_lines(request) {
                let _ = writeln!(out, "{line}");
            }
            let reply = match ask(input, out) {
                Reply::Allow => PermissionReply::Allow {},
                Reply::Deny => PermissionReply::Deny { message: None },
                Reply::None => {
                    return Ok(Handled::Leave(format!(
                        "No answer was given, so the request stays pending. Answer it with `collide run --resume {run}`."
                    )));
                }
            };
            let said = match reply {
                PermissionReply::Allow {} => "allowed",
                _ => "denied",
            };
            match answer(client, &item.id, QueueAnswer::Permission { reply })? {
                QueueAnswerResult::Refused { message } => {
                    let _ = writeln!(out, "The daemon did not take the answer: {message}");
                }
                _ => {
                    let _ = writeln!(out, "{said} once");
                }
            }
            Ok(Handled::Go)
        }
        (QueueKind::UsageLimit, _) => {
            let _ = writeln!(
                out,
                "{}{}. It resumes by itself at the reset; `collide run --resume {run}` resumes it now.",
                item.title,
                reset_in(item.reset_ms)
            );
            Ok(Handled::Go)
        }
        (QueueKind::DiskFull, _) => Ok(Handled::Leave(format!(
            "{}. Free some space, then `collide run --resume {run}` resumes it, or `collide stop {run}` stops it.",
            item.title
        ))),
        (QueueKind::BudgetExceeded, _) => Ok(Handled::Leave(format!(
            "{}. Raising the budget is not possible yet; `collide stop {run}` stops the run.",
            item.title
        ))),
        _ => {
            let _ = writeln!(out, "{}", item.title);
            Ok(Handled::Go)
        }
    }
}

/// How following ended.
enum Followed {
    Over(RunRecordResult),
    Left(String),
}

/// Follows the run until it is over, asking for each permission request, or until it waits for a person.
fn follow(
    client: &mut OwnerClient,
    env: &Env,
    run: &str,
    input: &mut dyn BufRead,
    out: &mut dyn Write,
) -> Result<Followed, ClientError> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut shown: Option<String> = None;
    loop {
        let record = record_of(client, run)?;
        if is_final(&record.state) {
            return Ok(Followed::Over(record));
        }
        let words = state_words(&record.state);
        if shown.as_deref() != Some(words.as_str()) {
            let _ = writeln!(out, "run {run}: {words}");
            shown = Some(words);
        }
        for item in queue_of(client, run)? {
            if !seen.insert(item.id.clone()) {
                continue;
            }
            if let Handled::Leave(message) = handle_item(client, run, &item, input, out)? {
                return Ok(Followed::Left(message));
            }
        }
        std::thread::sleep(env.poll);
    }
}

/// Resumes the paused run with the answer its Queue item takes: the one way back (ts-0066-zb, decision 2).
fn resume_paused(
    client: &mut OwnerClient,
    run: &str,
    out: &mut dyn Write,
) -> Result<(), ClientError> {
    for item in queue_of(client, run)? {
        if !item.answers.contains(&AnswerKind::Resume) {
            continue;
        }
        match answer(client, &item.id, QueueAnswer::Resume {})? {
            QueueAnswerResult::Refused { message } => {
                let _ = writeln!(out, "The daemon did not resume the run: {message}");
            }
            _ => {
                let _ = writeln!(out, "resuming: {}", item.title);
            }
        }
    }
    Ok(())
}

fn finish(followed: Followed, out: &mut dyn Write) -> u8 {
    match followed {
        Followed::Over(record) => {
            let _ = write!(out, "{}", record_text(&record));
            final_code(&record.state)
        }
        Followed::Left(message) => {
            let _ = writeln!(out, "{message}");
            exit::WAITING
        }
    }
}

/// Runs the command for parsed arguments.
pub fn run_in(
    env: &Env,
    args: &RunArgs,
    input: &mut dyn BufRead,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let located = match &args.target {
        Target::Task(given) => match locate(env, given) {
            Ok(located) => Some(located),
            Err((code, message)) => {
                let _ = writeln!(err, "collide run: {message}");
                return code;
            }
        },
        Target::Resume(_) => None,
    };
    if let Err(message) = (env.start_daemon)(&env.paths) {
        let _ = writeln!(
            err,
            "collide run: the daemon is not running and did not start: {message}"
        );
        return exit::FAILED;
    }
    let mut client = match OwnerClient::connect(&env.paths, "collide run") {
        Ok(client) => client,
        Err(error) => return call_error(err, &error),
    };
    let run = match (&args.target, located) {
        (Target::Task(_), Some((task_file, repo))) => {
            print_versions(&mut client, env.poll, out);
            let params = RunStartFileParams {
                repo: repo.to_string_lossy().into_owned(),
                task_file: task_file.to_string_lossy().into_owned(),
                alias: args.alias.clone(),
                budget_usd: args.budget_usd,
            };
            let started: Result<RunStartResult, ClientError> =
                client.call("run.start_file", &params, Some(START_WAIT));
            match started {
                Ok(RunStartResult::Started { run }) => {
                    let _ = writeln!(out, "run {run} started on {}", task_file.display());
                    let _ = writeln!(
                        out,
                        "Closing this command leaves the run going: `collide run --resume {run}` comes back to it, `collide stop {run}` stops it."
                    );
                    run
                }
                Ok(RunStartResult::Refused { problems }) => {
                    for problem in &problems {
                        let _ = writeln!(err, "collide run: refused: {}", problem.message);
                    }
                    let alias = args.alias.as_deref().unwrap_or(BUILDER_MODEL_ALIAS);
                    let mapped = load_user_models(env.home.as_deref())
                        .ok()
                        .is_some_and(|models| models.mapping_for("claude").contains_key(alias));
                    if !mapped {
                        let exists =
                            models_path(env.home.as_deref()).is_ok_and(|path| path.is_file());
                        let _ =
                            writeln!(err, "{}", models_hint(env.home.as_deref(), alias, exists));
                    }
                    return exit::REFUSED;
                }
                Err(error) => return call_error(err, &error),
            }
        }
        (Target::Resume(run), _) => {
            print_versions(&mut client, env.poll, out);
            let record = match record_of(&mut client, run) {
                Ok(record) => record,
                Err(ClientError::Rpc(error)) if error.code == code::NOT_FOUND => {
                    let _ = writeln!(err, "collide run: {}", error.message);
                    return exit::ID;
                }
                Err(error) => return call_error(err, &error),
            };
            match &record.state {
                state if is_final(state) => {
                    let _ = write!(out, "{}", record_text(&record));
                    if record.failure.is_some() || matches!(state, RunState::Failed {}) {
                        let _ = writeln!(
                            out,
                            "Nothing resumes this run: a phase that failed or was lost has no Queue item to answer, so start the task again."
                        );
                    }
                    return final_code(state);
                }
                RunState::Paused { .. } => {
                    if let Err(error) = resume_paused(&mut client, run, out) {
                        return call_error(err, &error);
                    }
                }
                _ => {}
            }
            run.clone()
        }
        (Target::Task(_), None) => unreachable!("a task target is located first"),
    };
    match follow(&mut client, env, &run, input, out) {
        Ok(followed) => finish(followed, out),
        Err(error) => call_error(err, &error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use collide_core::owner::{PublishedVersions, SmokeCheckResult, TestedVersions};

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    fn agent(in_use: Option<&str>, published: Option<PublishedVersions>) -> AgentVersions {
        AgentVersions {
            agent: "claude".to_string(),
            in_use: in_use.map(str::to_string),
            in_use_error: None,
            tested: TestedVersions {
                from: "2.1.300".to_string(),
                through: "2.1.330".to_string(),
            },
            in_tested_range: in_use.map(|_| true),
            smoke: None,
            published,
        }
    }

    fn call(scope: UsageScope, quality: UsageQuality) -> RecordedCall {
        RecordedCall {
            scope,
            quality,
            wall_ms: None,
            model_ms: None,
            input: None,
            output: None,
            cache_read: None,
            cache_write: None,
            cost: None,
        }
    }

    #[test]
    fn a_task_a_budget_and_an_alias_are_read() {
        assert_eq!(
            parse(&args(&["t.md", "--budget", "1.5", "--alias", "strong"])).unwrap(),
            RunArgs {
                target: Target::Task("t.md".to_string()),
                budget_usd: Some(1.5),
                alias: Some("strong".to_string()),
            }
        );
        assert_eq!(
            parse(&args(&["--resume", "r-1"])).unwrap().target,
            Target::Resume("r-1".to_string())
        );
    }

    #[test]
    fn a_bad_command_line_is_refused() {
        for bad in [
            args(&[]),
            args(&["a", "b"]),
            args(&["a", "--budget"]),
            args(&["a", "--budget", "x"]),
            args(&["a", "--budget", "0"]),
            args(&["a", "--budget", "-1"]),
            args(&["a", "--budget", "inf"]),
            args(&["a", "--alias"]),
            args(&["a", "--force"]),
            args(&["--resume"]),
            args(&["--resume", "r", "a"]),
            args(&["--resume", "r", "--budget", "1"]),
            args(&["--resume", "r", "--alias", "x"]),
            args(&["--resume", "r", "--resume", "q"]),
        ] {
            assert!(parse(&bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn a_figure_the_agent_did_not_give_is_not_reported_and_never_zero() {
        let text = call_text(&call(UsageScope::Turn, UsageQuality::Unreported), 1);
        assert_eq!(
            text,
            "turn 1: wall not reported, model time not reported, input not reported, output not reported, cache read not reported, cache write not reported, cost not reported"
        );
    }

    #[test]
    fn a_turns_line_shows_its_times_tokens_and_cost() {
        let line = RecordedCall {
            wall_ms: Some(3100),
            model_ms: Some(2300),
            input: Some(4),
            output: Some(270),
            cache_read: Some(46_000),
            cache_write: Some(38_000),
            cost: Some(Money {
                amount: 0.0084,
                currency: "USD".to_string(),
            }),
            ..call(UsageScope::Turn, UsageQuality::Exact)
        };
        assert_eq!(
            call_text(&line, 2),
            "turn 2: wall 3.1 s, model time 2.3 s, input 4, output 270, cache read 46,000, cache write 38,000, cost $0.0084"
        );
    }

    #[test]
    fn a_partial_line_says_at_least_and_a_zero_cost_is_a_reported_zero() {
        let cut = RecordedCall {
            input: Some(4),
            cache_read: Some(46_000),
            ..call(UsageScope::Step, UsageQuality::Partial)
        };
        let text = call_text(&cut, 1);
        assert!(
            text.starts_with("turn 1 (cut before its result): "),
            "{text}"
        );
        assert!(text.contains("input at least 4, output not reported, cache read at least 46,000"));
        assert!(text.ends_with("cost not reported"));
        let free = RecordedCall {
            cost: Some(Money {
                amount: 0.0,
                currency: "USD".to_string(),
            }),
            ..call(UsageScope::Turn, UsageQuality::Exact)
        };
        assert!(call_text(&free, 1).ends_with("cost $0.0000"));
    }

    #[test]
    fn totals_are_at_least_when_a_call_lacks_the_figure_and_not_reported_when_all_do() {
        let with_cost = RecordedCall {
            cost: Some(Money {
                amount: 0.5,
                currency: "USD".to_string(),
            }),
            input: Some(10),
            ..call(UsageScope::Turn, UsageQuality::Exact)
        };
        let without = call(UsageScope::Turn, UsageQuality::Exact);
        assert_eq!(
            total_cost(&[with_cost.clone(), with_cost.clone()]),
            "$1.0000"
        );
        assert_eq!(
            total_cost(&[with_cost.clone(), without.clone()]),
            "at least $0.5000"
        );
        assert_eq!(total_cost(std::slice::from_ref(&without)), NOT_REPORTED);
        assert_eq!(total_cost(&[]), NOT_REPORTED);
        assert_eq!(
            total_of(&[with_cost.clone(), without], |c| c.input),
            "at least 10"
        );
        assert_eq!(total_of(&[with_cost], |c| c.input), "10");
    }

    #[test]
    fn both_versions_are_printed_when_the_daemon_fetches_the_published_one() {
        let lines = version_lines(&agent(
            Some("2.1.325"),
            Some(PublishedVersions {
                latest: Some("2.1.400".to_string()),
                stable: Some("2.1.390".to_string()),
                fetched_ms: Some(1),
                error: None,
            }),
        ));
        assert_eq!(
            lines,
            [
                "claude in use: 2.1.325 (tested range 2.1.300 through 2.1.330)",
                "claude published: latest 2.1.400, stable 2.1.390"
            ]
        );
    }

    #[test]
    fn with_the_published_check_off_only_the_version_in_use_is_printed() {
        let lines = version_lines(&agent(Some("2.1.325"), None));
        assert_eq!(lines.len(), 1);
        assert!(lines[0].starts_with("claude in use: 2.1.325"));
    }

    #[test]
    fn a_failed_fetch_and_a_version_outside_the_range_are_said() {
        let mut outside = agent(
            Some("2.2.0"),
            Some(PublishedVersions {
                latest: None,
                stable: None,
                fetched_ms: None,
                error: Some("offline".to_string()),
            }),
        );
        outside.in_tested_range = Some(false);
        outside.smoke = Some(SmokeCheckResult {
            passed: true,
            at_ms: 1,
            reason: None,
        });
        let lines = version_lines(&outside);
        assert!(lines[0].contains("outside the tested range 2.1.300 through 2.1.330"));
        assert!(lines[0].contains("its smoke check passed"));
        assert_eq!(lines[1], "claude published: not known (offline)");
        let mut unreadable = agent(None, None);
        unreadable.in_use_error = Some("no claude on PATH".to_string());
        assert_eq!(
            version_lines(&unreadable),
            ["claude in use: not known (no claude on PATH)"]
        );
    }

    #[test]
    fn the_models_hint_says_where_the_file_goes_and_maps_the_alias() {
        let hint = models_hint(Some("/home/u"), "fast", false);
        assert!(
            hint.contains("/home/u/.config/collide/models.yaml"),
            "{hint}"
        );
        assert!(hint.contains("fast: { name: haiku }"), "{hint}");
        assert!(hint.contains("There is no models.yaml"));
        assert!(
            models_hint(Some("/home/u"), "fast", true).contains("does not map the alias `fast`")
        );
    }

    #[test]
    fn a_permission_request_with_no_input_says_it_is_not_available_until_approved() {
        let request = PermissionRequest {
            id: "p1".to_string(),
            tool_call_id: "t1".to_string(),
            tool: Some("Bash".to_string()),
            kind: collide_core::agent::ToolKind::Execute,
            input: None,
            reason: None,
            options: Vec::new(),
        };
        let lines = request_lines(&request);
        assert!(lines.contains(&"  input not available until approved".to_string()));
        assert!(lines[0].contains("Bash (execute)"));
    }

    #[test]
    fn the_prompt_offers_no_always_whatever_options_the_agent_listed() {
        let mut out = Vec::new();
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        assert_eq!(ask(&mut input, &mut out), Reply::Allow);
        let printed = String::from_utf8(out).unwrap().to_lowercase();
        assert!(!printed.contains("always"), "{printed}");
    }

    #[test]
    fn the_answer_is_read_from_a_line_and_anything_else_is_asked_again() {
        let mut out = Vec::new();
        let mut yes = std::io::Cursor::new(b"YES\n".to_vec());
        assert_eq!(ask(&mut yes, &mut out), Reply::Allow);
        let mut enter = std::io::Cursor::new(b"\n".to_vec());
        assert_eq!(ask(&mut enter, &mut out), Reply::Deny);
        let mut again = std::io::Cursor::new(b"maybe\nn\n".to_vec());
        assert_eq!(ask(&mut again, &mut out), Reply::Deny);
        let mut closed = std::io::Cursor::new(Vec::new());
        assert_eq!(ask(&mut closed, &mut out), Reply::None);
    }

    #[test]
    fn numbers_get_thousands_separators() {
        assert_eq!(with_commas(0), "0");
        assert_eq!(with_commas(999), "999");
        assert_eq!(with_commas(1000), "1,000");
        assert_eq!(with_commas(1_234_567), "1,234,567");
    }

    #[test]
    fn the_exit_code_follows_the_runs_end() {
        assert_eq!(
            final_code(&RunState::Ended {
                outcome: collide_core::non_empty::NonEmpty::new("x").unwrap()
            }),
            exit::OK
        );
        assert_eq!(final_code(&RunState::Failed {}), exit::RUN_FAILED);
        assert_eq!(final_code(&RunState::Stopped {}), exit::RUN_STOPPED);
    }

    // ---- an ACP phase (ts-0091-pz) -------------------------------------------------------------------------------

    use crate::adapters::acp::options::AcpOptions;
    use crate::adapters::acp::session::AcpSession;
    use crate::metrics_store::UsageRow;
    use collide_core::adapter::AgentSession;
    use collide_core::agent::AgentEvent;
    use collide_core::testing::acp::{initialize_response, new_session_response, prompt_response};
    use serde_json::json;

    /// The record of a run whose one phase was an ACP turn that ended with `usage` in its response (`None`: no
    /// `usage`), as the daemon stores and `run.record` gives it, for a profile that claims `quality`.
    fn acp_record(quality: UsageQuality, usage: Option<serde_json::Value>) -> RunRecordResult {
        let mut session =
            AcpSession::new(AcpOptions::new("/work", "go").with_usage(quality, false));
        session.start();
        session.feed(&initialize_response(0), 1);
        session.feed(&new_session_response(1, "ses-stand-in-1"), 2);
        let step = session.feed(&prompt_response(2, "end_turn", usage), 3);
        let rows: Vec<UsageRow> = step
            .events
            .into_iter()
            .filter_map(|stamped| match stamped.event {
                AgentEvent::Usage { usage } => Some(UsageRow {
                    phase: "build".to_string(),
                    turn: None,
                    usage,
                }),
                _ => None,
            })
            .collect();
        RunRecordResult {
            run: "r".to_string(),
            state: RunState::Failed {},
            failure: None,
            failure_evidence: None,
            report: None,
            workspace: None,
            wall_ms: None,
            calls: crate::run_record::recorded_calls(&[], &rows, 0),
        }
    }

    #[test]
    fn an_acp_phase_with_partial_usage_prints_at_least_and_never_a_zero_for_what_it_lacks() {
        let usage = json!({ "inputTokens": 34, "outputTokens": 49, "cachedReadTokens": 9331 });
        let text = record_text(&acp_record(UsageQuality::Partial, Some(usage)));
        assert!(
            text.contains(
                "turn 1: wall not reported, model time not reported, input at least 34, output at least 49, \
                 cache read at least 9,331, cache write not reported, cost not reported"
            ),
            "{text}"
        );
        assert!(
            text.contains(
                "total: wall not reported, input at least 34, output at least 49, cache read at least 9,331, \
                 cache write not reported, cost not reported"
            ),
            "{text}"
        );
        assert!(!text.contains("cache write 0"), "{text}");
    }

    #[test]
    fn an_acp_phase_with_no_usage_prints_not_reported_in_every_figure_and_never_a_zero() {
        let text = record_text(&acp_record(UsageQuality::Partial, None));
        assert!(
            text.contains(
                "turn 1: wall not reported, model time not reported, input not reported, output not reported, \
                 cache read not reported, cache write not reported, cost not reported"
            ),
            "{text}"
        );
        assert!(
            text.contains(
                "total: wall not reported, input not reported, output not reported, cache read not reported, \
                 cache write not reported, cost not reported"
            ),
            "{text}"
        );
    }

    #[test]
    fn a_failure_with_evidence_prints_the_evidence_after_it_and_a_failure_without_prints_none() {
        let mut record = acp_record(UsageQuality::Partial, None);
        record.failure = Some("empty_turn".to_string());
        assert!(!record_text(&record).contains("evidence"));
        record.failure_evidence = Some("AI_APICallError: model not found\nsecond line".to_string());
        let text = record_text(&record);
        assert!(
            text.contains(
                "failure: empty_turn\nevidence:\n  AI_APICallError: model not found\n  second line\n"
            ),
            "{text}"
        );
    }

    #[test]
    fn evidence_is_untrusted_so_its_control_characters_are_not_printed() {
        let mut record = acp_record(UsageQuality::Partial, None);
        record.failure = Some("empty_turn".to_string());
        record.failure_evidence =
            Some("red \u{1b}[31mtext\u{7} bell\r\n\u{1b}]0;title".to_string());
        let text = record_text(&record);
        assert!(
            !text.contains('\u{1b}') && !text.contains('\u{7}') && !text.contains('\r'),
            "{text:?}"
        );
        assert!(text.contains("  red [31mtext bell\n"), "{text:?}");
    }
}
