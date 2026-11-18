//! The forms of `collide tasks` and `collide task` that go through the daemon (ts-0120-wm; Stage 4b, CLI):
//!
//! - `collide task new <prompt>`: a scratch task for the board of the repository, by `task.apply` of the scratch
//!   source's `create`. It prints the key.
//! - `collide tasks --view <name> [--checkout <path>]` and `collide tasks --checkout <path>`: `task.list`, through the
//!   named view and limited to the copies in one checkout. `--ready` and `--kind` apply on top, in the daemon.
//! - `collide task <id> --json`: `task.get`, which has the item, its links one hop out, its copies and its latest run.
//! - `collide task <source>:<id> <op> ...`: the write by `task.apply`. A write to a busy checkout is queued by the
//!   daemon (0.4, Root busy); the command says why, prints the Queue item's id, and waits for it with `Retry` until
//!   the write has applied, or the wait has run out ([`wait_for_queued`]).
//!
//! The daemon is started when none answers, and the command says so once ([`start_daemon`]). A command line that is
//! wrong, and a `--checkout` that is not an entry of `git worktree list`, are refused before that. What stays direct
//! until Stage 4c is in [`crate::task_cli`]: `tasks`, `task <id>`, `task create`, `task reid` and `sections`.
//!
//! The board is the one of the repository's main checkout ([`crate::checkout::main_checkout`]), whatever worktree the
//! command runs in, so a linked worktree and the main checkout name one board.

use crate::checkout::main_checkout;
use crate::config_cli::open_board;
use crate::daemon::{DaemonPaths, ROOT_ENV};
use crate::launch::{StartKind, ensure_running_from_cli_kind};
use crate::owner_client::{ClientError, OwnerClient};
use crate::scratch;
use crate::task_args::{describe, map_args};
use crate::task_cli::{
    CliError, Command, NewOptions, TaskOptions, TasksOptions, Wait, ambiguous_message,
    checkout_named, commit_reported, emit, exit, ok_text,
};
use collide_core::item::OpDecl;
use collide_core::owner::{
    QueueAnswer, QueueAnswerParams, QueueAnswerResult, QueueKind, QueueListParams, QueueListResult,
    TaskApplyParams, TaskApplyResult, TaskGetParams, TaskGetResult, TaskListParams, TaskListResult,
    ViewRef, ViewReport, WriteQueued, code,
};
use collide_core::provider::{ApplyParams, ApplyResult, Task};
use collide_core::short_id::{Lookup, resolve};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::json;
use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// How long a command waits for the first listing of a board's sources: it asks again until the daemon says the
/// listing is done.
const SETTLE: Duration = Duration::from_secs(60);

/// How often it asks again.
const SETTLE_POLL: Duration = Duration::from_millis(100);

/// How often a queued write is looked at again while the command waits for it.
const QUEUE_POLL: Duration = Duration::from_millis(500);
/// How often, and for how long, the command looks for the outcome of a write that left the Queue.
const OUTCOME_POLL: Duration = Duration::from_millis(250);
const OUTCOME_WAIT: Duration = Duration::from_secs(10);

/// Whether `command` goes through the daemon.
pub fn wants_daemon(command: &Command) -> bool {
    match command {
        Command::New(_) => true,
        Command::Tasks(options) => options.view.is_some() || options.checkout.is_some(),
        // An id with a source in front of it (`scratch:3`) is a key only the daemon knows; the text of `task <id>`
        // stays direct, and a bare id's write is the plan's, which stays direct.
        Command::Task(options) => match options.op {
            None => options.json,
            Some(_) => options.id.contains(':'),
        },
        Command::Create(_) | Command::Reid(_) | Command::Sections(_) => false,
    }
}

/// Runs a command that [`wants_daemon`], against the daemon this process's environment names.
pub fn run(command: &Command, cwd: &Path, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    match DaemonPaths::from_env(
        std::env::var(ROOT_ENV).ok().as_deref(),
        std::env::var("HOME").ok().as_deref(),
    ) {
        Ok(paths) => run_with(&paths, command, cwd, out, err),
        Err(message) => {
            let _ = writeln!(err, "collide: {message}");
            exit::FAILED
        }
    }
}

/// Runs the command against the daemon at `paths`, from the folder `cwd`. Returns the exit code.
pub fn run_with(
    paths: &DaemonPaths,
    command: &Command,
    cwd: &Path,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let checkout = match command {
        Command::Tasks(options) => options.checkout.as_deref(),
        Command::Task(options) => options.write.checkout.as_deref(),
        _ => None,
    };
    let result = open(paths, checkout, cwd, err).and_then(|mut session| match command {
        Command::New(options) => new_task(&mut session, options, out, err),
        Command::Tasks(options) => tasks(&mut session, options, out, err),
        Command::Task(options) => task(&mut session, options, out, err),
        Command::Create(_) | Command::Reid(_) | Command::Sections(_) => Err(CliError::failed(
            "this command does not go through the daemon",
        )),
    });
    match result {
        Ok(code) => code,
        Err(error) => {
            let _ = writeln!(err, "collide: {}", error.message);
            error.code
        }
    }
}

/// A connection to the daemon for one board.
struct Session {
    client: OwnerClient,
    board: String,
    /// The repository's main checkout.
    root: PathBuf,
    /// The checkout that `--checkout` named, as git lists it.
    checkout: Option<PathBuf>,
}

impl Session {
    fn call<P: Serialize, R: DeserializeOwned>(
        &mut self,
        method: &str,
        params: &P,
    ) -> Result<R, CliError> {
        self.client.call(method, params, None).map_err(rpc_failed)
    }

    fn checkout_arg(&self) -> Option<String> {
        self.checkout
            .as_ref()
            .map(|path| path.display().to_string())
    }

    /// The folder that `--commit` commits in.
    fn written_in(&self) -> &Path {
        self.checkout.as_deref().unwrap_or(&self.root)
    }

    /// `task.list`, asked again until the daemon has finished the first listing of the board's sources: before that the
    /// answer is what the last run cached, or less.
    fn settled_list(&mut self, params: &TaskListParams) -> Result<TaskListResult, CliError> {
        let started = Instant::now();
        loop {
            let listed: TaskListResult = self.call("task.list", params)?;
            if !listed.refreshing {
                return Ok(listed);
            }
            if started.elapsed() >= OUTCOME_WAIT {
                return Err(CliError::failed(format!(
                    "the board's first listing did not finish in {} seconds",
                    SETTLE.as_secs()
                )));
            }
            std::thread::sleep(SETTLE_POLL);
        }
    }

    fn list_params(&self) -> TaskListParams {
        TaskListParams {
            board: self.board.clone(),
            view: None,
            ready: false,
            kind: None,
            checkout: self.checkout_arg(),
            limit: None,
            cursor: None,
        }
    }
}

/// An error of a call: a thing the daemon does not have is an id that names nothing (3), anything else failed (1).
fn rpc_failed(error: ClientError) -> CliError {
    match error {
        ClientError::Rpc(rpc) if rpc.code == code::NOT_FOUND => {
            CliError::new(exit::ID, rpc.message)
        }
        other => CliError::failed(other),
    }
}

/// Resolves `--checkout` (a path that is no checkout of the repository starts nothing), starts the daemon when none
/// answers, and opens the board of the repository.
fn open(
    paths: &DaemonPaths,
    checkout: Option<&str>,
    cwd: &Path,
    err: &mut dyn Write,
) -> Result<Session, CliError> {
    let root = main_checkout(cwd).map_err(CliError::failed)?;
    let checkout = checkout
        .map(|given| checkout_named(&root, cwd, given))
        .transpose()?;
    start_daemon(paths, err)?;
    let mut client = OwnerClient::connect(paths, "collide task").map_err(CliError::failed)?;
    let board = open_board(&mut client, &root)
        .map_err(CliError::failed)?
        .ok_or_else(|| CliError::failed(format!("{} is not a board", root.display())))?;
    Ok(Session {
        client,
        board,
        root,
        checkout,
    })
}

/// Makes sure a daemon answers. When this command started it, it says so, once. When it cannot start, the command
/// fails and says why: the direct commands do not need it.
fn start_daemon(paths: &DaemonPaths, err: &mut dyn Write) -> Result<(), CliError> {
    match ensure_running_from_cli_kind(paths) {
        Ok(StartKind::AlreadyRunning) => {}
        Ok(StartKind::Launchd) => {
            let _ = writeln!(err, "collide: the daemon was not running; started it");
        }
        Ok(StartKind::SetsidFallback { warning }) => {
            let _ = writeln!(
                err,
                "collide: the daemon was not running; started it\ncollide: {warning}"
            );
        }
        Err(message) => {
            return Err(CliError::failed(format!(
                "the daemon is not running and did not start: {message}"
            )));
        }
    }
    Ok(())
}

// ---- task new ------------------------------------------------------------------------------------------------------

fn new_task(
    session: &mut Session,
    options: &NewOptions,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<u8, CliError> {
    let params = TaskApplyParams {
        board: session.board.clone(),
        source: Some(scratch::SOURCE.to_string()),
        apply: ApplyParams {
            op: "create".to_string(),
            id: None,
            args: BTreeMap::from([("prompt".to_string(), json!(options.prompt))]),
            version: None,
            checkout: None,
        },
        commit: false,
    };
    let answer: TaskApplyResult = session.call("task.apply", &params)?;
    let Some(result) = answer.result else {
        return Err(CliError::failed(
            "the daemon queued a scratch task, which it has no reason to",
        ));
    };
    if options.json {
        emit(out, &format!("{}\n", result_json(&result)))?;
    }
    match result {
        ApplyResult::Ok { task, .. } => {
            if !options.json {
                emit(out, &format!("{}\n", task.key))?;
            }
            Ok(exit::OK)
        }
        ApplyResult::Conflict { current } => {
            let _ = writeln!(
                err,
                "collide: conflict: {} changed; nothing was written",
                current.key
            );
            Ok(exit::CONFLICT)
        }
        ApplyResult::Refused { message } => {
            let _ = writeln!(err, "collide: refused: {message}");
            Ok(exit::REFUSED)
        }
    }
}

fn result_json(result: &ApplyResult) -> String {
    serde_json::to_string(result).expect("a result is JSON")
}

// ---- tasks ---------------------------------------------------------------------------------------------------------

fn tasks(
    session: &mut Session,
    options: &TasksOptions,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<u8, CliError> {
    let mut params = session.list_params();
    params.view = options
        .view
        .as_ref()
        .map(|name| ViewRef::Named { name: name.clone() });
    params.ready = options.ready;
    params.kind = options.kind.clone();
    let listed = session.settled_list(&params)?;
    for item in &listed.invalid {
        let _ = writeln!(err, "collide: skipped {}: {}", item.path, item.reason);
    }
    for stale in &listed.stale_sources {
        let _ = writeln!(
            err,
            "collide: source {} may be out of date: {}",
            stale.source, stale.reason
        );
    }
    let failed = listed.view.as_ref().filter(|report| report.error.is_some());
    if options.json {
        let document = serde_json::to_string(&listed).expect("a result is JSON");
        emit(out, &format!("{document}\n"))?;
    } else if failed.is_none() {
        emit(out, &list_text(&listed.tasks))?;
    }
    let Some(report) = &listed.view else {
        return Ok(exit::OK);
    };
    if let Some(error) = &report.error {
        let _ = writeln!(err, "collide: view `{}` has an error: {error}", report.name);
        return Ok(exit::VIEW_FAILED);
    }
    if let Some(line) = errors_line(report) {
        let _ = writeln!(err, "collide: {line}");
    }
    Ok(exit::OK)
}

/// One line per task: its key, status, kind (`-` for a task with none) and title, the first three in columns.
fn list_text(tasks: &[Task]) -> String {
    let rows: Vec<[&str; 3]> = tasks
        .iter()
        .map(|task| {
            [
                task.key.as_str(),
                task.state.status.as_str(),
                task.kind.as_deref().unwrap_or("-"),
            ]
        })
        .collect();
    let width = |column: usize| rows.iter().map(|row| row[column].len()).max().unwrap_or(0);
    let widths = [width(0), width(1), width(2)];
    let mut text = String::new();
    for (row, task) in rows.iter().zip(tasks) {
        text.push_str(&format!(
            "{:<w0$}  {:<w1$}  {:<w2$}  {}\n",
            row[0],
            row[1],
            row[2],
            task.title,
            w0 = widths[0],
            w1 = widths[1],
            w2 = widths[2],
        ));
    }
    text
}

/// What a view could not check, as one line, or `None` when it checked every task: the board's invalid items, the tasks
/// whose `where` raised an error, and those that ran out of steps.
fn errors_line(report: &ViewReport) -> Option<String> {
    let mut parts = Vec::new();
    if report.invalid_items > 0 {
        parts.push(format!("{} invalid item(s)", report.invalid_items));
    }
    for (reason, count) in &report.errors {
        parts.push(format!("{count} with an error: {reason}"));
    }
    if report.over_task_budget > 0 {
        parts.push(format!("{} over the step budget", report.over_task_budget));
    }
    if parts.is_empty() {
        return None;
    }
    let count = u64::from(report.invalid_items)
        + report.errors.values().sum::<u64>()
        + report.over_task_budget;
    Some(format!(
        "view `{}` could not check {count} task(s): {}",
        report.name,
        parts.join(", ")
    ))
}

// ---- task <id> --json, task <source>:<id> <op> ---------------------------------------------------------------------

fn task(
    session: &mut Session,
    options: &TaskOptions,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<u8, CliError> {
    let key = match key_of(session, &options.id)? {
        Keyed::One(key) => key,
        Keyed::Several(keys, message) => {
            let _ = writeln!(err, "collide: {message}");
            if options.json {
                emit(
                    out,
                    &document(&TaskGetResult {
                        ambiguous: keys,
                        ..empty_get()
                    }),
                )?;
            }
            return Ok(exit::ID);
        }
    };
    let got: TaskGetResult = session.call(
        "task.get",
        &TaskGetParams {
            board: session.board.clone(),
            id: key.clone(),
            checkout: session.checkout_arg(),
        },
    )?;
    let Some(current) = got.task.clone() else {
        if got.ambiguous.is_empty() {
            return Err(CliError::new(
                exit::ID,
                format!("no item has the key `{key}`"),
            ));
        }
        let _ = writeln!(
            err,
            "collide: `{key}` is ambiguous: {}",
            collision_text(&got)
        );
        if options.json {
            emit(out, &document(&got))?;
        }
        return Ok(exit::ID);
    };
    let Some(op_name) = &options.op else {
        emit(out, &document(&got))?;
        return Ok(exit::OK);
    };
    write_op(session, options, op_name, &current, &got.ops, out, err)
}

/// What an id names: one key, or several (the message says which).
enum Keyed {
    One(String),
    Several(Vec<String>, String),
}

/// The key of the item `id` names. A key (it has a source in front of it) is its own. A bare id is read as the direct
/// commands read it: a full id, or a short form that must fit one item, among the tasks the board lists.
fn key_of(session: &mut Session, id: &str) -> Result<Keyed, CliError> {
    let mut params = session.list_params();
    // The board's own listing, whichever checkout the copies are in: `task.get` takes the checkout itself.
    params.checkout = None;
    if id.contains(':') {
        params.limit = Some(1);
        session.settled_list(&params)?;
        return Ok(Keyed::One(id.to_string()));
    }
    let listed = session.settled_list(&params)?;
    match resolve(&listed.tasks, id) {
        Lookup::One(task) => Ok(Keyed::One(task.key.clone())),
        Lookup::Many(matches) => Ok(Keyed::Several(
            matches.iter().map(|task| task.key.clone()).collect(),
            ambiguous_message(id, &matches),
        )),
        Lookup::None => Err(CliError::new(
            exit::ID,
            format!(
                "no item matches `{id}`: looked for the id `{id}` and for items with that number (and suffix, if \
                 given) among the {} items of the board",
                listed.tasks.len()
            ),
        )),
        Lookup::Malformed => Err(CliError::usage(format!(
            "`{id}` is not an item id; use the full id (ts-0012-ua), a short form (12, 12-ua, ts-12) \
             or a key (plan:ts-0012-ua)"
        ))),
    }
}

fn empty_get() -> TaskGetResult {
    TaskGetResult {
        task: None,
        ambiguous: Vec::new(),
        copies: Vec::new(),
        collisions: Vec::new(),
        runs: Vec::new(),
        latest_run: None,
        links: Vec::new(),
        ops: Vec::new(),
    }
}

/// The document `task <id> --json` prints: the daemon's answer without the ops, which are for the command's own use.
fn document(got: &TaskGetResult) -> String {
    let mut value = serde_json::to_value(got).expect("a result is JSON");
    if let Some(fields) = value.as_object_mut() {
        fields.remove("ops");
    }
    format!("{value}\n")
}

/// Where an ambiguous key is: the keys it fits, and each place a collided key is held.
fn collision_text(got: &TaskGetResult) -> String {
    let mut text = got.ambiguous.join(", ");
    for collision in &got.collisions {
        let places: Vec<String> = collision
            .locations
            .iter()
            .map(|location| format!("{} in {}", location.path, location.checkout))
            .collect();
        text.push_str(&format!(
            "; short id {} is held at different paths: {}",
            collision.short_id,
            places.join(", ")
        ));
    }
    text
}

/// `collide task <key> <op> ...`: the write, at the version `task.get` gave.
fn write_op(
    session: &mut Session,
    options: &TaskOptions,
    op_name: &str,
    current: &Task,
    declared: &[OpDecl],
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<u8, CliError> {
    if op_name == "create" {
        return Err(CliError::usage(
            "`create` makes a new item and takes no id: collide task new <prompt> makes a scratch task",
        ));
    }
    let ops: Vec<&OpDecl> = declared.iter().filter(|op| op.name != "create").collect();
    let Some(op) = ops.iter().find(|op| op.name == op_name) else {
        let mut message = format!(
            "unknown op `{op_name}` for {} ({}); its source declares:",
            current.key,
            current.kind.as_deref().unwrap_or("item")
        );
        for op in &ops {
            message.push_str(&format!("\n  {}", describe(op)));
        }
        return Err(CliError::usage(message));
    };
    let args = map_args(op, &options.words).map_err(CliError::usage)?;
    let params = TaskApplyParams {
        board: session.board.clone(),
        source: None,
        apply: ApplyParams {
            op: op.name.clone(),
            id: Some(current.key.clone()),
            args,
            version: Some(current.version.clone()),
            checkout: session.checkout_arg(),
        },
        commit: false,
    };
    let answer: TaskApplyResult = session.call("task.apply", &params)?;
    if let Some(queued) = &answer.queued {
        return wait_for_queued(session, queued, current, options, out, err);
    }
    let Some(result) = answer.result else {
        return Err(CliError::failed(
            "the daemon answered with neither a result nor a queued write",
        ));
    };
    if options.json {
        emit(out, &format!("{}\n", result_json(&result)))?;
    }
    match &result {
        ApplyResult::Ok { task, files, .. } => {
            if !options.json {
                emit(out, &ok_text(current, task, files))?;
            }
            let commit = options.write.commit.then(|| session.written_in());
            Ok(commit_reported(
                commit,
                task,
                files,
                &format!("task {} {}: {}", task.id, op.name, task.title),
                options.json,
                out,
                err,
            ))
        }
        ApplyResult::Conflict { current: latest } => {
            if !options.json {
                let _ = writeln!(
                    err,
                    "collide: conflict: {} changed after it was read; nothing was written. It is now {}, \
                     at version {}",
                    latest.key, latest.state.status, latest.version
                );
            }
            Ok(exit::CONFLICT)
        }
        ApplyResult::Refused { message } => {
            if !options.json {
                let _ = writeln!(err, "collide: refused: {message}");
            }
            Ok(exit::REFUSED)
        }
    }
}

// ---- a write that is queued ----------------------------------------------------------------------------------------

/// How a queued write ended, as far as the command could tell.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Settled {
    /// It was taken out of the Queue and the task's version changed: the write landed. Before [`settle`] has looked,
    /// it is only "the item is gone".
    Applied,
    /// It left the Queue, no Queue item says it was undone, and the task did not change: it was dismissed by a
    /// person, or it changed nothing.
    Unchanged,
    /// It was taken out of the Queue and the daemon could not apply it: a new Queue item says why.
    NotApplied,
    /// It is in the Queue still.
    Queued,
}

/// Says why the write is queued and which Queue item holds it, then waits for it as `--wait` and `--no-wait` say. A
/// `Retry` asks the daemon to look at the checkout now, and answers when it has: the write is applied, or it stays. The
/// outcome is `OK` (applied), `REFUSED` (it was taken and could not be applied) or `QUEUED` (still in the Queue).
///
/// A write that is queued is not committed: the files it touched are not known here, so `--commit` is said to be
/// ignored and the person commits it.
fn wait_for_queued(
    session: &mut Session,
    queued: &WriteQueued,
    current: &Task,
    options: &TaskOptions,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<u8, CliError> {
    let _ = writeln!(
        err,
        "collide: the write to {} is queued: {}; queue item {}",
        current.key, queued.reason, queued.item
    );
    if options.write.commit {
        let _ = writeln!(
            err,
            "collide: --commit does not apply to a queued write: commit it when it has landed"
        );
    }
    let settled = match options.write.wait {
        Wait::Never => Settled::Queued,
        wait => retry_until(session, &queued.item, wait.limit())?,
    };
    // Retry answers `Done` when the item is gone, which is also so when a person dismissed it or another drain of the
    // checkout took it, and that drain may still be applying it. The command says applied only when it sees the
    // version of the task change.
    let settled = match settled {
        Settled::Applied => settle(session, queued, current)?,
        other => other,
    };
    if options.json {
        let word = match settled {
            Settled::Applied => "applied",
            Settled::NotApplied | Settled::Unchanged => "not_applied",
            Settled::Queued => "queued",
        };
        let document =
            json!({ "queued": { "item": queued.item, "reason": queued.reason }, "settled": word });
        emit(out, &format!("{document}\n"))?;
    }
    match settled {
        Settled::Applied => {
            if !options.json {
                emit(
                    out,
                    &format!("ok  {}: the queued write was applied\n", current.key),
                )?;
            }
            Ok(exit::OK)
        }
        Settled::Unchanged => {
            let _ = writeln!(
                err,
                "collide: the queued write is no longer queued and {} did not change: it was dismissed in the Queue, \
                 or it changed nothing",
                current.key
            );
            Ok(exit::REFUSED)
        }
        Settled::NotApplied => {
            let title = undone_of(session, queued, current)?.unwrap_or_default();
            let _ = writeln!(err, "collide: the queued write was not applied: {title}");
            Ok(exit::REFUSED)
        }
        Settled::Queued => {
            let how = match options.write.wait {
                Wait::Never => "--no-wait".to_string(),
                wait => format!("it did not apply in {} seconds", wait.limit().as_secs()),
            };
            let _ = writeln!(
                err,
                "collide: the write stays queued ({how}); it is applied when the checkout is free"
            );
            Ok(exit::QUEUED)
        }
    }
}

/// After the queue item is gone: waits up to [`OUTCOME_WAIT`] to see what became of the write. A Queue item that says
/// it was undone is not applied; a version of the task that differs from the one the write was sent at is applied;
/// the deadline with neither is `Unchanged`.
fn settle(
    session: &mut Session,
    queued: &WriteQueued,
    current: &Task,
) -> Result<Settled, CliError> {
    let started = Instant::now();
    loop {
        if undone_of(session, queued, current)?.is_some() {
            return Ok(Settled::NotApplied);
        }
        let got: TaskGetResult = session.call(
            "task.get",
            &TaskGetParams {
                board: session.board.clone(),
                id: current.key.clone(),
                checkout: session.checkout_arg(),
            },
        )?;
        if got.task.is_some_and(|task| task.version != current.version) {
            return Ok(Settled::Applied);
        }
        if started.elapsed() >= OUTCOME_WAIT {
            return Ok(Settled::Unchanged);
        }
        std::thread::sleep(OUTCOME_POLL);
    }
}

/// Asks the daemon to retry the queued write until it leaves the Queue or `limit` has passed. `Applied` here only
/// means that the item is gone: [`settle`] says what became of the write.
fn retry_until(session: &mut Session, item: &str, limit: Duration) -> Result<Settled, CliError> {
    let started = Instant::now();
    loop {
        let params = QueueAnswerParams {
            item: item.to_string(),
            answer: QueueAnswer::Retry {},
        };
        let answer: QueueAnswerResult = match session.client.call("queue.answer", &params, None) {
            Ok(answer) => answer,
            // The item is gone: a person dismissed it, or another drain of the checkout took it.
            Err(ClientError::Rpc(rpc)) if rpc.code == code::NOT_FOUND => {
                return Ok(Settled::Applied);
            }
            Err(other) => return Err(rpc_failed(other)),
        };
        match answer {
            QueueAnswerResult::Queued { .. } => {}
            QueueAnswerResult::Refused { message } => return Err(CliError::failed(message)),
            _ => return Ok(Settled::Applied),
        }
        if started.elapsed() >= limit {
            return Ok(Settled::Queued);
        }
        std::thread::sleep(QUEUE_POLL);
    }
}

/// The title of the Queue item that says a write of this task was taken and not applied, made after `queued` was.
/// Its id is `write-<ms>-<n>-undone`, and the queued item's `write-<ms>-<n>`.
fn undone_of(
    session: &mut Session,
    queued: &WriteQueued,
    current: &Task,
) -> Result<Option<String>, CliError> {
    let since = queued_at(&queued.item);
    let listed: QueueListResult = session.call(
        "queue.list",
        &QueueListParams {
            board: None,
            run: None,
            kinds: vec![QueueKind::Conflict],
        },
    )?;
    Ok(listed
        .items
        .into_iter()
        .find(|item| {
            item.id.ends_with("-undone")
                && item.task.as_deref() == Some(current.key.as_str())
                && queued_at(&item.id) >= since
        })
        .map(|item| item.title))
}

/// The millisecond in a write's Queue id, `write-<ms>-<n>`; 0 for an id that is not one.
fn queued_at(id: &str) -> u64 {
    id.split('-')
        .nth(1)
        .and_then(|ms| ms.parse().ok())
        .unwrap_or(0)
}
