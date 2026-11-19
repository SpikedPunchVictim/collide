//! Every `collide tasks`, `collide task` and `collide sections` command, through the daemon (ts-0120-wm, ts-0136-rr):
//!
//! - `collide tasks [--view <name>] [--ready] [--kind <kind>] [--checkout <path>]`: `task.list`, through the named view
//!   and limited to the copies in one checkout. `--ready` alone is the built-in `ready` view, whose cycle and unknown
//!   blockers are said on stderr.
//! - `collide task <id> [--json]`: `task.get`, which has the item, its links one hop out, its copies and its latest
//!   run. The text names an incoming link by its inverse. A bare or short id is resolved here, by `short_id::resolve`
//!   over the board's listing.
//! - `collide task new <prompt>`: a scratch task for the board of the repository, by `task.apply` of the scratch
//!   source's `create`. It prints the key.
//! - `collide task <id> <op> ...`: the write by `task.apply`. A write to a busy checkout is queued by the daemon
//!   (0.4, Root busy); the command says why, prints the Queue item's id, and waits for it with `Retry` until the write
//!   has applied, or the wait has run out ([`wait_for_queued`]). A person-only op (0.10) is not written: the daemon
//!   keeps it with a Queue confirmation that a person answers in the app, and the command exits 12
//!   ([`confirmation_waits`]).
//! - `collide task create <title> ...` and `collide task reid <id>`: `task.create` and `task.reid`, queued and waited
//!   for as any write is.
//! - `collide sections`: `task.sections`.
//!
//! The daemon is started when none answers, and the command says so once ([`start_daemon`]); when it cannot start, the
//! command fails and reads nothing itself. A command line that is wrong, and a `--checkout` that is not an entry of
//! `git worktree list`, are refused before that.
//!
//! The board is the one of the repository's main checkout ([`crate::checkout::main_checkout`]), whatever worktree the
//! command runs in, so a linked worktree and the main checkout name one board.

use crate::checkout::main_checkout;
use crate::config_cli::open_board;
use crate::daemon::{DaemonPaths, ROOT_ENV};
use crate::launch::{StartKind, ensure_running_from_cli_kind};
use crate::owner_client::{ClientError, OwnerClient};
use crate::scratch;
use crate::section_view::sections_text;
use crate::sources::PLANNING_SOURCE;
use crate::task_args::{describe, map_args};
use crate::task_cli::{
    CliError, Command, CreateOptions, NewOptions, ReidOptions, RenameSectionOptions,
    SectionsOptions, TaskOptions, TasksOptions, Wait, WriteOptions, ambiguous_message,
    checkout_named, commit_reported, emit, exit, ok_text,
};
use crate::task_view::{heading, show_text};
use collide_core::item::OpDecl;
use collide_core::owner::{
    QueueAnswer, QueueAnswerParams, QueueAnswerResult, QueueKind, QueueListParams, QueueListResult,
    TaskApplyParams, TaskApplyResult, TaskCreateParams, TaskGetParams, TaskGetResult,
    TaskListParams, TaskListResult, TaskReidParams, TaskReidResult, TaskSectionsParams, ViewRef,
    ViewReport, WriteQueued, code,
};
use collide_core::provider::{ApplyParams, ApplyResult, Task};
use collide_core::section::Sections;
use collide_core::short_id::{Lookup, resolve};
use collide_core::view_builtin::READY;
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

/// Runs a command against the daemon this process's environment names.
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
        Command::Create(options) => options.write.checkout.as_deref(),
        Command::Reid(options) => options.write.checkout.as_deref(),
        Command::New(_) | Command::Sections(_) => None,
        Command::RenameSection(options) => options.write.checkout.as_deref(),
    };
    let result = open(paths, checkout, cwd, err).and_then(|mut session| match command {
        Command::New(options) => new_task(&mut session, options, out, err),
        Command::Tasks(options) => tasks(&mut session, options, out, err),
        Command::Task(options) => task(&mut session, options, out, err),
        Command::Create(options) => create(&mut session, options, out, err),
        Command::Reid(options) => reid(&mut session, options, out, err),
        Command::Sections(options) => sections(&mut session, options, out, err),
        Command::RenameSection(options) => rename_section(&mut session, options, out, err),
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
            if started.elapsed() >= SETTLE {
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

/// An error of a call: a thing the daemon does not have is an id that names nothing (3), a `--kind` the board's
/// sources do not declare is a wrong command line (2, the refusal carries the kind in its data), anything else
/// failed (1).
fn rpc_failed(error: ClientError) -> CliError {
    match error {
        ClientError::Rpc(rpc)
            if rpc.code == code::INVALID_PARAMS
                && rpc
                    .data
                    .as_ref()
                    .is_some_and(|data| data.get("unknown_kind").is_some()) =>
        {
            CliError::usage(rpc.message)
        }
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
/// fails and says why, and reads nothing itself.
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
            actor: None,
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
    // `--ready` alone is the built-in view of that name, whose report says what kept an item from being ready.
    params.view = match (&options.view, options.ready) {
        (Some(name), _) => Some(ViewRef::Named { name: name.clone() }),
        (None, true) => Some(ViewRef::Named {
            name: READY.to_string(),
        }),
        (None, false) => None,
    };
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
    // Only the built-in view's cycle is a `blocked-by` one, and the only one that keeps tasks from being ready.
    if report.name == READY && !report.cycle.is_empty() {
        let _ = writeln!(
            err,
            "collide: blocked-by cycle, none is ready: {}",
            report.cycle.join(", ")
        );
    }
    for unknown in &report.unknown_blockers {
        let _ = writeln!(
            err,
            "collide: {} is blocked by {}, which is not an item; it is not ready",
            unknown.from, unknown.to
        );
    }
    if report.part_problems > 0 {
        let _ = writeln!(
            err,
            "collide: view `{}` has {} problem(s) in the parts of its items",
            report.name, report.part_problems
        );
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

// ---- task <id>, task <id> <op> -------------------------------------------------------------------------------------

fn task(
    session: &mut Session,
    options: &TaskOptions,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<u8, CliError> {
    let Some((got, current)) = read_one(session, &options.id, options.json, out, err)? else {
        return Ok(exit::ID);
    };
    let Some(op_name) = &options.op else {
        if options.json {
            emit(out, &document(&got))?;
        } else {
            emit(out, &show_text(&got, &current))?;
        }
        return Ok(exit::OK);
    };
    write_op(session, options, op_name, &current, &got.ops, out, err)
}

/// The item `id` names, as `task.get` gives it, or `None` when `id` names several: the command has then said which (and
/// printed the answer, under `--json`) and ends with `ID`. An id that names none is an error.
fn read_one(
    session: &mut Session,
    id: &str,
    json: bool,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<Option<(TaskGetResult, Task)>, CliError> {
    let key = match key_of(session, id)? {
        Keyed::One(key) => key,
        Keyed::Several(keys, message) => {
            let _ = writeln!(err, "collide: {message}");
            if json {
                emit(
                    out,
                    &document(&TaskGetResult {
                        ambiguous: keys,
                        ..empty_get()
                    }),
                )?;
            }
            return Ok(None);
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
        if json {
            emit(out, &document(&got))?;
        }
        return Ok(None);
    };
    Ok(Some((got, current)))
}

/// What an id names: one key, or several (the message says which).
enum Keyed {
    One(String),
    Several(Vec<String>, String),
}

/// What a bare id names among the board's tasks.
enum Named {
    One(Box<Task>),
    Several(Vec<String>, String),
}

/// The task a bare id names among the tasks the board lists, by [`resolve`]: a full id, or a short form that must fit
/// one item. The listing is the board's own, whichever checkout the copies are in. `kind` keeps only items of that kind
/// from the candidates.
fn name_in_board(session: &mut Session, id: &str, kind: Option<&str>) -> Result<Named, CliError> {
    let mut params = session.list_params();
    params.checkout = None;
    let listed = session.settled_list(&params)?;
    let candidates: Vec<Task> = listed
        .tasks
        .into_iter()
        .filter(|task| kind.is_none_or(|kind| task.kind.as_deref() == Some(kind)))
        .collect();
    match resolve(&candidates, id) {
        Lookup::One(task) => Ok(Named::One(Box::new(task.clone()))),
        Lookup::Many(matches) => Ok(Named::Several(
            matches.iter().map(|task| task.key.clone()).collect(),
            ambiguous_message(id, &matches),
        )),
        Lookup::None => Err(CliError::new(
            exit::ID,
            format!(
                "no item matches `{id}`: looked for the id `{id}` and for items with that number (and suffix, if \
                 given) among the {} items of the board",
                candidates.len()
            ),
        )),
        Lookup::Malformed => Err(CliError::usage(format!(
            "`{id}` is not an item id; use the full id (ts-0012-ua), a short form (12, 12-ua, ts-12) \
             or a key (plan:ts-0012-ua)"
        ))),
    }
}

/// The key of the item `id` names. A key (it has a source in front of it) is its own. A bare id is a full id, or a
/// short form that must fit one item, among the tasks the board lists.
fn key_of(session: &mut Session, id: &str) -> Result<Keyed, CliError> {
    if id.contains(':') {
        // The board's first listing has to be done before the daemon is asked for a key it may not hold yet.
        let mut params = session.list_params();
        params.checkout = None;
        params.limit = Some(1);
        session.settled_list(&params)?;
        return Ok(Keyed::One(id.to_string()));
    }
    Ok(match name_in_board(session, id, None)? {
        Named::One(task) => Keyed::One(task.key),
        Named::Several(keys, message) => Keyed::Several(keys, message),
    })
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
        link_types: Vec::new(),
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
            actor: None,
            op: op.name.clone(),
            id: Some(current.key.clone()),
            args,
            version: Some(current.version.clone()),
            checkout: session.checkout_arg(),
        },
        commit: false,
    };
    let answer: TaskApplyResult = session.call("task.apply", &params)?;
    let waiting = Waiting {
        label: &current.key,
        current: Some(current),
    };
    let result = match settled_write(
        session,
        answer,
        &waiting,
        &options.write,
        options.json,
        out,
        err,
    )? {
        Written::Done(result) => *result,
        Written::Left(code) => return Ok(code),
    };
    if options.json {
        emit(out, &format!("{}\n", result_json(&result)))?;
    }
    let subject = |task: &Task| format!("task {} {}: {}", task.id, op.name, task.title);
    Ok(finish(
        session.written_in(),
        &result,
        &options.write,
        options.json,
        &subject,
        |task, files| ok_text(current, task, files),
        out,
        err,
    ))
}

/// What a write came to once the daemon had answered it.
enum Written {
    /// The provider's result: applied, conflicting or refused.
    Done(Box<ApplyResult>),
    /// The write was queued and not applied, or a person must confirm it: the command ends with this code.
    Left(u8),
}

/// Follows the daemon's answer to a write: waits for a queued one (see [`wait_for_queued`]), reports a confirmation
/// that waits, and otherwise hands the provider's result back.
fn settled_write(
    session: &mut Session,
    answer: TaskApplyResult,
    waiting: &Waiting<'_>,
    write: &WriteOptions,
    json: bool,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<Written, CliError> {
    if let Some(queued) = &answer.queued {
        return wait_for_queued(session, queued, waiting, write, json, out, err).map(Written::Left);
    }
    if let Some(asked) = &answer.confirmation {
        return confirmation_waits(asked, write, json, out, err).map(Written::Left);
    }
    answer
        .result
        .map(|result| Written::Done(Box::new(result)))
        .ok_or_else(|| {
            CliError::failed(
                "the daemon answered with neither a result, a queued write nor a confirmation",
            )
        })
}

/// Ends a write whose result is in: the text of an applied one (and its `--commit`), or the conflict or the refusal,
/// with the exit code each has. `subject` is the commit message.
#[allow(clippy::too_many_arguments)]
fn finish(
    written_in: &Path,
    result: &ApplyResult,
    write: &WriteOptions,
    json: bool,
    subject: &dyn Fn(&Task) -> String,
    text: impl FnOnce(&Task, &[String]) -> String,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    match result {
        ApplyResult::Ok { task, files, .. } => {
            if !json {
                let _ = out.write_all(text(task, files).as_bytes());
            }
            let commit = write.commit.then_some(written_in);
            commit_reported(commit, task, files, &subject(task), json, out, err)
        }
        ApplyResult::Conflict { current: latest } => {
            if !json {
                let _ = writeln!(
                    err,
                    "collide: conflict: {} changed after it was read; nothing was written. It is now {}, \
                     at version {}",
                    latest.key, latest.state.status, latest.version
                );
            }
            exit::CONFLICT
        }
        ApplyResult::Refused { message } => {
            if !json {
                let _ = writeln!(err, "collide: refused: {message}");
            }
            exit::REFUSED
        }
    }
}

// ---- task create, task reid, task renameSection --------------------------------------------------------------------

/// `collide task create <title> ...`: `task.create`. An `epic` that is a short form is resolved here, to the full id.
fn create(
    session: &mut Session,
    options: &CreateOptions,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<u8, CliError> {
    let epic = options
        .epic
        .as_deref()
        .map(|given| resolve_epic(session, given))
        .transpose()?;
    let answer: TaskApplyResult = session.call(
        "task.create",
        &TaskCreateParams {
            board: session.board.clone(),
            title: options.title.clone(),
            kind: options.kind.clone(),
            epic,
            seed: options.seed,
            root: options.root.clone(),
            checkout: session.checkout_arg(),
        },
    )?;
    let waiting = Waiting {
        label: "the new item",
        current: None,
    };
    let result = match settled_write(
        session,
        answer,
        &waiting,
        &options.write,
        options.json,
        out,
        err,
    )? {
        Written::Done(result) => *result,
        Written::Left(code) => return Ok(code),
    };
    if options.json {
        emit(out, &format!("{}\n", result_json(&result)))?;
    }
    let subject = |task: &Task| format!("task {} create: {}", task.id, task.title);
    Ok(finish(
        session.written_in(),
        &result,
        &options.write,
        options.json,
        &subject,
        |task, files| {
            let mut text = format!("ok  created {}\n  title   {}\n", heading(task), task.title);
            for file in files {
                text.push_str(&format!("  file    {file}\n"));
            }
            text
        },
        out,
        err,
    ))
}

/// `collide task reid <id>`: `task.reid`. It prints the old and the new key, each other item whose links were
/// rewritten, and each file touched.
fn reid(
    session: &mut Session,
    options: &ReidOptions,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<u8, CliError> {
    let Some((_, current)) = read_one(session, &options.id, options.json, out, err)? else {
        return Ok(exit::ID);
    };
    let answer: TaskReidResult = session.call(
        "task.reid",
        &TaskReidParams {
            board: session.board.clone(),
            id: current.key.clone(),
            seed: options.seed,
            checkout: session.checkout_arg(),
        },
    )?;
    if let Some(queued) = &answer.queued {
        // A reid moves the item to a new key, so its old key never shows a new version: it waits as a create does.
        let waiting = Waiting {
            label: &current.key,
            current: None,
        };
        return wait_for_queued(
            session,
            queued,
            &waiting,
            &options.write,
            options.json,
            out,
            err,
        );
    }
    if options.json {
        let document = serde_json::to_string(&answer).expect("a result is JSON");
        emit(out, &format!("{document}\n"))?;
    }
    let Some(result) = &answer.result else {
        return Err(CliError::failed(
            "the daemon answered with neither a result nor a queued write",
        ));
    };
    let from = answer.from.as_str();
    let old_id = from.split_once(':').map_or(from, |(_, id)| id);
    let subject = |task: &Task| format!("task {} reid (was {old_id}): {}", task.id, task.title);
    let written_in = session.written_in().to_path_buf();
    Ok(finish(
        &written_in,
        result,
        &options.write,
        options.json,
        &subject,
        |task, files| {
            let mut text = format!("ok  {}\n  id      {from} -> {}\n", heading(task), task.key);
            for key in &answer.rewritten {
                text.push_str(&format!("  links   rewritten in {key}\n"));
            }
            for file in files {
                let label = if written_in.join(file).exists() {
                    "file   "
                } else {
                    "removed"
                };
                text.push_str(&format!("  {label} {file}\n"));
            }
            if !options.write.commit {
                text.push_str(
                    "nothing is committed; review the changes with git diff and git status\n",
                );
            }
            text
        },
        out,
        err,
    ))
}

/// `collide task renameSection <from> <to>`: the planning source's `renameSection`, which names no item and takes no
/// version: `task.apply` with the source named and no id.
fn rename_section(
    session: &mut Session,
    options: &RenameSectionOptions,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<u8, CliError> {
    let args = BTreeMap::from([
        ("from".to_string(), json!(options.from)),
        ("to".to_string(), json!(options.to)),
    ]);
    let answer: TaskApplyResult = session.call(
        "task.apply",
        &TaskApplyParams {
            board: session.board.clone(),
            source: Some(PLANNING_SOURCE.to_string()),
            apply: ApplyParams {
                actor: None,
                op: "renameSection".to_string(),
                id: None,
                args,
                version: None,
                checkout: session.checkout_arg(),
            },
            commit: false,
        },
    )?;
    let waiting = Waiting {
        label: "renameSection",
        current: None,
    };
    let result = match settled_write(
        session,
        answer,
        &waiting,
        &options.write,
        options.json,
        out,
        err,
    )? {
        Written::Done(result) => *result,
        Written::Left(code) => return Ok(code),
    };
    if options.json {
        emit(out, &format!("{}\n", result_json(&result)))?;
    }
    let subject = |task: &Task| {
        format!(
            "renameSection {} to {} (first: {})",
            options.from, options.to, task.id
        )
    };
    Ok(finish(
        session.written_in(),
        &result,
        &options.write,
        options.json,
        &subject,
        |_, files| {
            let mut text = format!(
                "ok  renamed section `{}` to `{}`\n",
                options.from, options.to
            );
            if let ApplyResult::Ok { items, .. } = &result {
                for item in items {
                    text.push_str(&format!("  item    {}\n", item.key));
                }
            }
            for file in files {
                text.push_str(&format!("  file    {file}\n"));
            }
            text
        },
        out,
        err,
    ))
}

/// The id of the epic that `given` names: a full id or a short form, which must fit one epic. A short form is read
/// among the epics, so `2` is `ep-002-..` and not `ts-0002-..`. What is no epic, or is none of them, is told apart: an
/// item that is not an epic is a usage error, and one that is no item at all is an unknown id.
fn resolve_epic(session: &mut Session, given: &str) -> Result<String, CliError> {
    match name_in_board(session, given, Some("epic")) {
        Ok(Named::One(epic)) => Ok(epic.id),
        Ok(Named::Several(_, message)) => Err(CliError::new(exit::ID, message)),
        Err(missing) if missing.code == exit::ID => match name_in_board(session, given, None) {
            Ok(Named::One(other)) => Err(CliError::usage(format!(
                "`{given}` names {}, which is a {}, not an epic",
                other.key,
                other.kind.as_deref().unwrap_or("item")
            ))),
            _ => Err(missing),
        },
        Err(other) => Err(other),
    }
}

// ---- sections ------------------------------------------------------------------------------------------------------

/// `collide sections`: `task.sections`.
fn sections(
    session: &mut Session,
    options: &SectionsOptions,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<u8, CliError> {
    // The board's first listing has to be done before the daemon counts its features. An item it could not read is
    // said on stderr, as `collide tasks` does.
    let mut params = session.list_params();
    params.checkout = None;
    params.limit = Some(1);
    let listed = session.settled_list(&params)?;
    for item in &listed.invalid {
        let _ = writeln!(err, "collide: skipped {}: {}", item.path, item.reason);
    }
    let found: Sections = session.call(
        "task.sections",
        &TaskSectionsParams {
            board: session.board.clone(),
        },
    )?;
    if options.json {
        let document = serde_json::to_string_pretty(&found).expect("sections are JSON");
        emit(out, &format!("{document}\n"))?;
    } else {
        emit(out, &sections_text(&found))?;
    }
    Ok(exit::OK)
}

// ---- a write that waits for a person -----------------------------------------------------------------------------

/// A person-only op (0.10): the daemon wrote nothing and put a confirmation in the Queue, which a person answers in
/// the app. Says so, names the Queue item, and exits `CONFIRMATION`. The command does not wait: the person may be away.
fn confirmation_waits(
    asked: &WriteQueued,
    write: &WriteOptions,
    json: bool,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<u8, CliError> {
    if json {
        let document = json!({ "confirmation": { "item": asked.item, "reason": asked.reason } });
        emit(out, &format!("{document}\n"))?;
    } else {
        let _ = writeln!(
            err,
            "collide: a confirmation waits in the app: {}; queue item {}; nothing was written",
            asked.reason, asked.item
        );
    }
    if write.commit {
        let _ = writeln!(
            err,
            "collide: --commit does not apply: nothing was written, so there is nothing to commit"
        );
    }
    Ok(exit::CONFIRMATION)
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

/// What a command waits for: the item the write is to change (`current`, read before the write), when it has one, and
/// how the command names it. A write that makes an item, or gives it a new id, has none, so it cannot see the version
/// change: a write that left the Queue with no Queue item that says it was undone is taken as applied.
struct Waiting<'a> {
    label: &'a str,
    current: Option<&'a Task>,
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
    waiting: &Waiting<'_>,
    write: &WriteOptions,
    json: bool,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<u8, CliError> {
    let _ = writeln!(
        err,
        "collide: the write to {} is queued: {}; queue item {}",
        waiting.label, queued.reason, queued.item
    );
    if write.commit {
        let _ = writeln!(
            err,
            "collide: --commit does not apply to a queued write: commit it when it has landed"
        );
    }
    let settled = match write.wait {
        Wait::Never => Settled::Queued,
        wait => retry_until(session, &queued.item, wait.limit())?,
    };
    // Retry answers `Done` when the item is gone, which is also so when a person dismissed it or another drain of the
    // checkout took it, and that drain may still be applying it. The command says applied only when it sees the
    // version of the task change.
    let settled = match settled {
        Settled::Applied => settle(session, queued, waiting.current)?,
        other => other,
    };
    if json {
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
            if !json {
                emit(
                    out,
                    &format!("ok  {}: the queued write was applied\n", waiting.label),
                )?;
            }
            Ok(exit::OK)
        }
        Settled::Unchanged => {
            let _ = writeln!(
                err,
                "collide: the queued write is no longer queued and {} did not change: it was dismissed in the Queue, \
                 or it changed nothing",
                waiting.label
            );
            Ok(exit::REFUSED)
        }
        Settled::NotApplied => {
            let title = undone_of(
                session,
                queued,
                waiting.current.map(|task| task.key.as_str()),
            )?
            .unwrap_or_default();
            let _ = writeln!(err, "collide: the queued write was not applied: {title}");
            Ok(exit::REFUSED)
        }
        Settled::Queued => {
            let how = match write.wait {
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
    current: Option<&Task>,
) -> Result<Settled, CliError> {
    let key = current.map(|task| task.key.as_str());
    let started = Instant::now();
    loop {
        if undone_of(session, queued, key)?.is_some() {
            return Ok(Settled::NotApplied);
        }
        let Some(current) = current else {
            return Ok(Settled::Applied);
        };
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

/// The title of the Queue item that says a write of the task `key` (of any task, when the write names none) was taken
/// and not applied, made after `queued` was. Its id is `write-<ms>-<n>-undone`, and the queued item's `write-<ms>-<n>`.
fn undone_of(
    session: &mut Session,
    queued: &WriteQueued,
    key: Option<&str>,
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
                && key.is_none_or(|key| item.task.as_deref() == Some(key))
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn task(status: &str, version: &str) -> Task {
        serde_json::from_value(json!({
            "key": "plan:ts-0001-aa",
            "id": "ts-0001-aa",
            "title": "The only item",
            "state": { "status": status, "category": status },
            "version": version,
            "kind": "task",
            "links": [{ "type": "part-of", "to": "ep-001-aa" }],
            "owners": [],
        }))
        .expect("a task")
    }

    /// What `finish` printed and returned for a result, with the options of a plain write.
    fn finished(result: &ApplyResult, json: bool) -> (u8, String, String) {
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = finish(
            Path::new("/nowhere"),
            result,
            &WriteOptions::default(),
            json,
            &|task| task.title.clone(),
            |_, _| "ok\n".to_string(),
            &mut out,
            &mut err,
        );
        (
            code,
            String::from_utf8(out).unwrap(),
            String::from_utf8(err).unwrap(),
        )
    }

    #[test]
    fn a_conflict_says_nothing_was_written_names_the_state_now_and_exits_4() {
        let result = ApplyResult::Conflict {
            current: task("active", "v2"),
        };
        let (code, out, err) = finished(&result, false);
        assert_eq!(code, exit::CONFLICT);
        assert!(out.is_empty(), "{out}");
        assert_eq!(
            err,
            "collide: conflict: plan:ts-0001-aa changed after it was read; nothing was written. \
             It is now active, at version v2\n"
        );
    }

    #[test]
    fn a_conflict_under_json_prints_nothing_of_its_own_and_exits_4() {
        let result = ApplyResult::Conflict {
            current: task("active", "v2"),
        };
        let (code, out, err) = finished(&result, true);
        assert_eq!((code, out.as_str(), err.as_str()), (exit::CONFLICT, "", ""));
        let document: Value = serde_json::from_str(&result_json(&result)).unwrap();
        assert_eq!(document["outcome"], "conflict");
        assert_eq!(document["current"]["version"], "v2");
    }

    #[test]
    fn a_refusal_prints_the_providers_message_and_exits_5() {
        let result = ApplyResult::Refused {
            message: "no".to_string(),
        };
        let (code, _, err) = finished(&result, false);
        assert_eq!(
            (code, err.as_str()),
            (exit::REFUSED, "collide: refused: no\n")
        );
    }
}
