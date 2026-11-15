//! `collide tasks`, `collide task <id>` and `collide task <id> <op> ...` (ts-0020). Until the daemon exists the CLI
//! starts the built-in `collide/plan` provider itself (through the loader of ts-0029), asks it for the items, and
//! applies the ready rule and short-id lookup of `core` to what it lists. The corpus stage moves this behind the
//! provider supervisor.

use crate::builtin_packs::{BuiltinPacks, Environment};
use crate::checkout::{CheckoutError, main_checkout, resolve_checkout};
use crate::commit::commit_files;
use crate::pack_location::{self, locate_bundle};
use crate::provider_client::ProviderClient;
use crate::task_args::{describe, map_args};
use crate::task_view::View;
use collide_core::item::OpDecl;
use collide_core::provider::{
    ApplyParams, ApplyResult, InitializeParams, InitializeResult, Task, TaskInvalid,
};
use collide_core::ready::readiness;
use collide_core::short_id::{Lookup, resolve};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Exit codes. An op's outcome has its own: `ok` is 0, `conflict` 4 and `refused` 5.
pub mod exit {
    pub const OK: u8 = 0;
    /// Something failed: no repository, no built-in packs, the provider did not start or answered badly, or the
    /// commit that `--commit` asked for failed (the write itself stands).
    pub const FAILED: u8 = 1;
    /// The command line is wrong: an option, an op, an argument, a `--checkout` that is no checkout of the repository.
    pub const USAGE: u8 = 2;
    /// An id names no item, or several.
    pub const ID: u8 = 3;
    /// The item changed since it was read, and nothing was written.
    pub const CONFLICT: u8 = 4;
    /// The provider refused the op, and said why.
    pub const REFUSED: u8 = 5;
}

pub const USAGE: &str = "\
usage:
  collide tasks [--ready] [--kind <kind>] [--json]
  collide task <id> [--json]
  collide task <id> <op> [<value>] [name=value ...] [--checkout <path>] [--commit] [--json]
  collide task create <title> [epic=<epic>] [kind=task|epic] [seed=<number>] [--checkout <path>] [--commit] [--json]
  collide task reid <id> [seed=<number>] [--checkout <path>] [--commit] [--json]";

/// What `collide help` prints.
pub const HELP: &str = "\
collide tasks: list the items under .collide/planning/epics/ (--ready: only the items that can be started now;
  --kind: only items of that kind; --json: machine-readable)
collide task <id>: show one item
collide task <id> <op> ...: change one item with an op the provider declares (an unknown op lists them)

<id> is the full id (ts-0012-ua) or a short form: 12, 0012, 12-ua, ts-12. A short form must match exactly one item;
if it matches several they are listed with their epics, and you add the suffix.

collide task create <title>: make an item in an epic (epic=ep-002-va, or a short form such as ep-2; kind=epic makes an
  epic, which takes no epic=). The id is the next free number for the kind across every local branch, every fetched
  remote branch and the working tree, with a two-letter suffix drawn from a seeded generator (seed=, random when left
  out). The new file is written; without --commit nothing is committed.
collide task reid <id>: give an item a new suffix and rewrite every link that names the old id, in every item file.
  Where two files share one id, the second (the one `collide tasks` reports as skipped) is the one renamed; links meant
  for the other file are rewritten too. Each file touched is printed. Without --commit nothing is committed: review with
  git diff.

Where a write goes: the main checkout's .collide/planning/, also when you run collide inside a linked worktree (which
is left untouched). --checkout <path> names another checkout instead: <path> must be an entry of `git worktree list`
for this repository (relative paths are taken from the working directory), or nothing is written and the exit code is
2. The item is then read from that checkout too, so a version check is against the file that is written. A write
leaves the file modified (or untracked, for a new one) and commits nothing.
--commit commits the write, in the checkout it went to: exactly the files the write reported (for reid, the new and
the removed paths too), with a message naming the item and the op, such as: task ts-0012-ua setStatus: <title>. Another
modified, staged or untracked file is not touched, and what you have staged stays staged. Hooks run (there is no
--no-verify). An op that changes nothing, and a conflict or a refusal, commit nothing. If the commit fails (a hook,
git's own refusal) git's message is printed, the files stay written and uncommitted, and the exit code is 1.

An op's words are name=value, or one bare value that sets the op's main property (setStatus active sets status).
Lengths and patterns are checked by the provider.

An item is ready when its status is ready and each blocked-by target is done or dropped. A blocked-by cycle, and
a blocked-by target that is no item, keep their items from being ready and are reported on stderr.

exit codes: 0 ok, 1 failed (also: the write was made but --commit's commit failed), 2 wrong command line (also: a
--checkout that is not a checkout of this repository), 3 id not found or ambiguous, 4 conflict (nothing was written),
5 refused by the provider

The provider is found through COLLIDE_BUILTIN_PACKS, else the staged bundle above the executable (pnpm build:packs),
else the bundle beside it. `node` is taken from PATH.";

/// A failure, with the exit code it ends with.
#[derive(Debug)]
pub struct CliError {
    pub code: u8,
    pub message: String,
}

impl CliError {
    fn new(code: u8, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    fn failed(error: impl std::fmt::Display) -> Self {
        Self::new(exit::FAILED, error.to_string())
    }

    fn usage(message: impl Into<String>) -> Self {
        Self::new(exit::USAGE, message)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct TasksOptions {
    pub ready: bool,
    pub kind: Option<String>,
    pub json: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub struct TaskOptions {
    pub id: String,
    pub op: Option<String>,
    pub words: Vec<String>,
    pub write: WriteOptions,
    pub json: bool,
}

/// Where a write goes and whether it is committed (ts-0031).
#[derive(Debug, Default, PartialEq, Eq)]
pub struct WriteOptions {
    /// `--checkout <path>`: write in this checkout instead of the main one.
    pub checkout: Option<String>,
    /// `--commit`: commit the files the write reported.
    pub commit: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub struct CreateOptions {
    pub words: Vec<String>,
    pub write: WriteOptions,
    pub json: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub struct ReidOptions {
    pub id: String,
    pub words: Vec<String>,
    pub write: WriteOptions,
    pub json: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Tasks(TasksOptions),
    Task(TaskOptions),
    Create(CreateOptions),
    Reid(ReidOptions),
}

/// Reads the words after `collide tasks`.
pub fn parse_tasks(args: &[String]) -> Result<Command, CliError> {
    let mut options = TasksOptions {
        ready: false,
        kind: None,
        json: false,
    };
    let mut words = args.iter();
    while let Some(word) = words.next() {
        match word.as_str() {
            "--ready" => options.ready = true,
            "--json" => options.json = true,
            "--kind" => match words.next() {
                Some(kind) => options.kind = Some(kind.clone()),
                None => return Err(CliError::usage("--kind needs a kind, such as --kind task")),
            },
            other => match other.strip_prefix("--kind=") {
                Some(kind) if !kind.is_empty() => options.kind = Some(kind.to_string()),
                _ if other.starts_with("--") => {
                    return Err(CliError::usage(format!("unknown option `{other}`")));
                }
                _ => {
                    return Err(CliError::usage(format!(
                        "`collide tasks` takes no argument; got `{other}`"
                    )));
                }
            },
        }
    }
    Ok(Command::Tasks(options))
}

/// Reads the words after `collide task`: an id, then an op and its words.
pub fn parse_task(args: &[String]) -> Result<Command, CliError> {
    let mut json = false;
    let mut write = WriteOptions::default();
    let mut plain = Vec::new();
    let mut words = args.iter();
    while let Some(word) = words.next() {
        let given = match word.as_str() {
            "--json" => {
                json = true;
                continue;
            }
            "--commit" => {
                write.commit = true;
                continue;
            }
            "--checkout" => match words.next() {
                Some(path) => path.clone(),
                None => {
                    return Err(CliError::usage(
                        "--checkout needs a path, such as --checkout ../other-worktree",
                    ));
                }
            },
            other => match other.strip_prefix("--checkout=") {
                Some(path) => path.to_string(),
                None if other.starts_with("--") => {
                    return Err(CliError::usage(format!("unknown option `{other}`")));
                }
                None => {
                    plain.push(word.clone());
                    continue;
                }
            },
        };
        if given.is_empty() {
            return Err(CliError::usage(
                "--checkout needs a path, such as --checkout ../other-worktree",
            ));
        }
        if write.checkout.replace(given).is_some() {
            return Err(CliError::usage("--checkout is given twice"));
        }
    }
    let mut plain = plain.into_iter();
    let Some(id) = plain.next() else {
        return Err(CliError::usage(
            "`collide task` needs an id, such as 12 or ts-0012-ua",
        ));
    };
    // `create` and `reid` are words of their own: neither is an id form, so they cannot be taken for one.
    if id == "create" {
        return Ok(Command::Create(CreateOptions {
            words: plain.collect(),
            write,
            json,
        }));
    }
    if id == "reid" {
        let Some(id) = plain.next() else {
            return Err(CliError::usage(
                "`collide task reid` needs an id, such as 12 or ts-0012-ua",
            ));
        };
        return Ok(Command::Reid(ReidOptions {
            id,
            words: plain.collect(),
            write,
            json,
        }));
    }
    let op = plain.next();
    // `collide task <id> reid` is `collide task reid <id>`: it is not a write against one version of one file.
    if op.as_deref() == Some("reid") {
        return Ok(Command::Reid(ReidOptions {
            id,
            words: plain.collect(),
            write,
            json,
        }));
    }
    if op.is_none() && write != WriteOptions::default() {
        return Err(CliError::usage(
            "--checkout and --commit are for a write: give an op after the id, such as setStatus active",
        ));
    }
    Ok(Command::Task(TaskOptions {
        id,
        op,
        words: plain.collect(),
        write,
        json,
    }))
}

/// A started provider, initialized with the main checkout as its root.
struct Session {
    client: ProviderClient,
    init: InitializeResult,
    root: PathBuf,
    /// The checkout that `--checkout` named, as git lists it, when there was one.
    checkout: Option<PathBuf>,
}

impl Session {
    /// The checkout the provider is told to read and write in: none means the main checkout.
    fn checkout_arg(&self) -> Option<String> {
        self.checkout
            .as_ref()
            .map(|path| path.display().to_string())
    }

    /// The folder that the files of a write are in, and that `--commit` commits in.
    fn written_in(&self) -> &Path {
        self.checkout.as_deref().unwrap_or(&self.root)
    }

    fn list(&mut self) -> Result<(Vec<Task>, Vec<TaskInvalid>), CliError> {
        let checkout = self.checkout_arg();
        self.client
            .list_all(checkout.as_deref())
            .map_err(CliError::failed)
    }

    fn get(&mut self, id: &str) -> Result<Option<Task>, CliError> {
        let checkout = self.checkout_arg();
        self.client
            .get(id, checkout.as_deref())
            .map_err(CliError::failed)
    }
}

/// Starts the provider on the main checkout of the repository that `cwd` is in. `checkout` is `--checkout`'s
/// path: checked here, before the provider starts, so that a path that is no checkout of the repository writes and
/// reads nothing. (The provider checks it again: it is the one that writes.)
fn open(cwd: &Path, checkout: Option<&str>) -> Result<Session, CliError> {
    let root = main_checkout(cwd).map_err(CliError::failed)?;
    let checkout = checkout
        .map(|given| {
            resolve_checkout(&root, cwd, given).map_err(|error| match error {
                CheckoutError::NotACheckout { .. } | CheckoutError::UnreadableCheckout { .. } => {
                    CliError::usage(format!("--checkout: {error}"))
                }
                other => CliError::failed(other),
            })
        })
        .transpose()?;
    let bundle = locate_bundle(&pack_location::Inputs::from_process()).map_err(CliError::failed)?;
    let launch = BuiltinPacks::new(bundle)
        .provider("collide/plan", "plan", &Environment::from_process())
        .map_err(CliError::failed)?;
    let mut client = ProviderClient::start(&launch, &root).map_err(CliError::failed)?;
    let init = client
        .initialize(&InitializeParams {
            config: BTreeMap::new(),
            root: root.display().to_string(),
        })
        .map_err(CliError::failed)?;
    Ok(Session {
        client,
        init,
        root,
        checkout,
    })
}

fn emit(out: &mut dyn Write, text: &str) -> Result<(), CliError> {
    out.write_all(text.as_bytes())
        .map_err(|error| CliError::failed(format!("could not write the output: {error}")))
}

/// Runs a command; returns the exit code. Errors go to `err`.
pub fn run(command: &Command, cwd: &Path, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let result = match command {
        Command::Tasks(options) => run_tasks(options, cwd, out, err),
        Command::Task(options) => run_task(options, cwd, out, err),
        Command::Create(options) => run_create(options, cwd, out, err),
        Command::Reid(options) => run_reid(options, cwd, out, err),
    };
    match result {
        Ok(code) => code,
        Err(error) => {
            let _ = writeln!(err, "collide: {}", error.message);
            error.code
        }
    }
}

fn run_tasks(
    options: &TasksOptions,
    cwd: &Path,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<u8, CliError> {
    let mut session = open(cwd, None)?;
    if let Some(kind) = &options.kind {
        let kinds: Vec<&str> = session.init.kinds.iter().map(|k| k.name.as_str()).collect();
        if !kinds.is_empty() && !kinds.contains(&kind.as_str()) {
            return Err(CliError::usage(format!(
                "unknown kind `{kind}`; the provider declares: {}",
                kinds.join(", ")
            )));
        }
    }
    let (tasks, invalid) = session.list()?;
    let found = readiness(&tasks);
    for item in &invalid {
        let _ = writeln!(err, "collide: skipped {}: {}", item.path, item.reason);
    }
    for cycle in &found.cycles {
        let _ = writeln!(
            err,
            "collide: blocked-by cycle, none is ready: {}",
            cycle.join(", ")
        );
    }
    for unknown in &found.unknown_targets {
        let _ = writeln!(
            err,
            "collide: {} is blocked by {}, which is not an item; it is not ready",
            unknown.from, unknown.to
        );
    }
    let view = View::new(&tasks, &found);
    let shown: Vec<&Task> = tasks
        .iter()
        .filter(|task| {
            options
                .kind
                .as_ref()
                .is_none_or(|kind| task.kind.as_ref() == Some(kind))
        })
        .filter(|task| !options.ready || view.is_ready(task))
        .collect();
    if options.json {
        let document = json!({
            "tasks": shown.iter().map(|task| view.task_json(task)).collect::<Vec<_>>(),
            "invalid": invalid,
            "cycles": found.cycles,
            "unknownTargets": found
                .unknown_targets
                .iter()
                .map(|u| json!({ "from": u.from, "to": u.to }))
                .collect::<Vec<_>>(),
        });
        emit(out, &format!("{document}\n"))?;
    } else {
        emit(out, &view.list_text(&shown))?;
    }
    Ok(exit::OK)
}

/// The text for an id that matched several items: each with its epic, and what to add.
fn ambiguous_message(query: &str, matches: &[&Task]) -> String {
    let mut text = format!("`{query}` matches {} items:\n", matches.len());
    for task in matches {
        text.push_str(&format!(
            "  {}  {}  {}\n",
            View::heading(task),
            task.kind.as_deref().unwrap_or(""),
            task.title
        ));
    }
    let example = matches
        .first()
        .and_then(|task| {
            let (prefix, rest) = task.id.split_once('-')?;
            let (number, suffix) = rest.split_once('-')?;
            Some(format!(", such as {number}-{suffix} or {prefix}-{number}"))
        })
        .unwrap_or_default();
    text.push_str(&format!(
        "add the suffix or the prefix{example}, or use the full id"
    ));
    text
}

/// The one item that `query` names, or the error that says why there is not one.
fn lookup<'a>(tasks: &'a [Task], query: &str) -> Result<&'a Task, CliError> {
    match resolve(tasks, query) {
        Lookup::One(item) => Ok(item),
        Lookup::Many(matches) => Err(CliError::new(exit::ID, ambiguous_message(query, &matches))),
        Lookup::None => Err(CliError::new(
            exit::ID,
            format!(
                "no item matches `{query}`: looked for the id `{query}` and for items with that number \
                 (and suffix, if given) among the {} items under .collide/planning/epics/",
                tasks.len()
            ),
        )),
        Lookup::Malformed => Err(CliError::usage(format!(
            "`{query}` is not an item id; use the full id (ts-0012-ua) or a short form (12, 12-ua, ts-12)"
        ))),
    }
}

fn run_task(
    options: &TaskOptions,
    cwd: &Path,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<u8, CliError> {
    let mut session = open(cwd, options.write.checkout.as_deref())?;
    let (tasks, _invalid) = session.list()?;
    let item = lookup(&tasks, &options.id)?;
    let found = readiness(&tasks);
    let view = View::new(&tasks, &found);
    let current = session.get(&item.id)?.ok_or_else(|| {
        CliError::failed(format!(
            "{} was listed, then was not there to read",
            item.id
        ))
    })?;

    let Some(op_name) = &options.op else {
        if options.json {
            emit(out, &format!("{}\n", view.task_json(&current)))?;
        } else {
            emit(out, &view.show_text(&current))?;
        }
        return Ok(exit::OK);
    };

    if op_name == "create" {
        return Err(CliError::usage(
            "`create` makes a new item and takes no id: collide task create <title> epic=<epic>",
        ));
    }
    let ops: Vec<OpDecl> = declared_ops(&session.init, current.kind.as_deref())
        .into_iter()
        .filter(|op| op.name != "create")
        .collect();
    let Some(op) = ops.iter().find(|op| &op.name == op_name) else {
        let mut message = format!(
            "unknown op `{op_name}` for {} ({}); the provider declares:",
            current.id,
            current.kind.as_deref().unwrap_or("item")
        );
        for op in &ops {
            message.push_str(&format!("\n  {}", describe(op)));
        }
        return Err(CliError::usage(message));
    };
    let args = map_args(op, &options.words).map_err(CliError::usage)?;
    let result = session
        .client
        .apply(&ApplyParams {
            op: op.name.clone(),
            id: Some(current.id.clone()),
            args,
            version: Some(current.version.clone()),
            checkout: session.checkout_arg(),
        })
        .map_err(CliError::failed)?;
    if options.json {
        emit(
            out,
            &format!(
                "{}\n",
                serde_json::to_string(&result).expect("a result is JSON")
            ),
        )?;
    }
    match &result {
        ApplyResult::Ok { task, files, .. } => {
            if !options.json {
                emit(out, &ok_text(&current, task, files))?;
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
                    "collide: conflict: {} changed after it was read; nothing was written. It is now:\n{}",
                    View::heading(latest),
                    view.show_text(latest).trim_end()
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

/// The ops that apply to an item of `kind`: the provider's own, then the kind's (a kind's op of one name replaces
/// the provider's).
fn declared_ops(init: &InitializeResult, kind: Option<&str>) -> Vec<OpDecl> {
    let mut ops = init.ops.clone();
    if let Some(decl) = init
        .kinds
        .iter()
        .find(|decl| Some(decl.name.as_str()) == kind)
    {
        for op in &decl.ops {
            ops.retain(|existing| existing.name != op.name);
            ops.push(op.clone());
        }
    }
    ops
}

/// What an `ok` changed, by comparing the item as it was read with what the write returned.
fn ok_text(before: &Task, after: &Task, files: &[String]) -> String {
    let mut changes = Vec::new();
    if before.state.status != after.state.status {
        changes.push(format!(
            "  status  {} -> {}\n",
            before.state.status, after.state.status
        ));
    }
    if before.owners != after.owners {
        let shown = |owners: &[String]| {
            if owners.is_empty() {
                "(none)".to_string()
            } else {
                owners.join(", ")
            }
        };
        changes.push(format!(
            "  owners  {} -> {}\n",
            shown(&before.owners),
            shown(&after.owners)
        ));
    }
    let link_names = |task: &Task| -> Vec<String> {
        task.links
            .iter()
            .filter(|link| link.link_type != "part-of")
            .map(|link| format!("{} {}", link.link_type, link.to))
            .collect()
    };
    let (was, now) = (link_names(before), link_names(after));
    for added in now.iter().filter(|link| !was.contains(link)) {
        changes.push(format!("  links   +{added}\n"));
    }
    for removed in was.iter().filter(|link| !now.contains(link)) {
        changes.push(format!("  links   -{removed}\n"));
    }
    let heading = View::heading(after);
    if changes.is_empty() && files.is_empty() {
        return format!("ok  {heading}: nothing to change\n");
    }
    let mut text = format!("ok  {heading}\n{}", changes.concat());
    for file in files {
        text.push_str(&format!("  file    {file}\n"));
    }
    text
}

/// An op the provider declares for the whole repository (one that does not start from an item), or a failure.
fn declared_op(session: &Session, name: &str) -> Result<OpDecl, CliError> {
    declared_ops(&session.init, None)
        .into_iter()
        .find(|op| op.name == name)
        .ok_or_else(|| CliError::failed(format!("the provider does not declare the op `{name}`")))
}

/// The full id of the epic that `query` names: a full id, or a short form (`2` and `ep-2` both mean `ep-002-xx`).
/// A bare number means an epic here, since `epic=` can only name one.
fn resolve_epic(tasks: &[Task], query: &str) -> Result<String, CliError> {
    let query = if !query.is_empty() && query.chars().all(|c| c.is_ascii_digit()) {
        format!("ep-{query}")
    } else {
        query.to_string()
    };
    let item = lookup(tasks, &query)?;
    if item.kind.as_deref() != Some("epic") {
        return Err(CliError::usage(format!(
            "`{query}` is {}, which is not an epic; give an epic, such as ep-002-va or ep-2",
            item.id
        )));
    }
    Ok(item.id.clone())
}

/// Commits what an `ok` reported, when `--commit` was given (`commit` is the checkout to commit in), and returns the
/// exit code: 0, or 1 when the commit failed (the files are then left written). Under `--json` the one document on
/// stdout stays the provider's result, so the new commit is not printed.
fn commit_reported(
    commit: Option<&Path>,
    task: &Task,
    files: &[String],
    subject: &str,
    json: bool,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let Some(dir) = commit.filter(|_| !files.is_empty()) else {
        return exit::OK;
    };
    match commit_files(dir, files, subject) {
        Ok(short) => {
            if !json {
                let _ = writeln!(out, "  commit  {short}  {subject}");
            }
            exit::OK
        }
        Err(message) => {
            let _ = writeln!(
                err,
                "collide: {} was written, but git commit failed; the files are left written and uncommitted:\n{message}",
                task.id
            );
            exit::FAILED
        }
    }
}

/// Prints the outcome of an op that is not a write to one version of one item (`create`, `reid`), then commits it
/// when `commit` names a checkout (`--commit`). `subject` is the commit message, from the new item.
fn finish(
    result: ApplyResult,
    commit: Option<&Path>,
    subject: impl FnOnce(&Task) -> String,
    json: bool,
    out: &mut dyn Write,
    err: &mut dyn Write,
    ok_text: impl FnOnce(&Task, &[String]) -> String,
) -> Result<u8, CliError> {
    if json {
        emit(
            out,
            &format!(
                "{}\n",
                serde_json::to_string(&result).expect("a result is JSON")
            ),
        )?;
    }
    match result {
        ApplyResult::Ok { task, files, .. } => {
            if !json {
                emit(out, &ok_text(&task, &files))?;
            }
            Ok(commit_reported(
                commit,
                &task,
                &files,
                &subject(&task),
                json,
                out,
                err,
            ))
        }
        ApplyResult::Conflict { current } => {
            if !json {
                let _ = writeln!(
                    err,
                    "collide: conflict: {} changed; nothing was written",
                    View::heading(&current)
                );
            }
            Ok(exit::CONFLICT)
        }
        ApplyResult::Refused { message } => {
            if !json {
                let _ = writeln!(err, "collide: refused: {message}");
            }
            Ok(exit::REFUSED)
        }
    }
}

fn run_create(
    options: &CreateOptions,
    cwd: &Path,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<u8, CliError> {
    let mut session = open(cwd, options.write.checkout.as_deref())?;
    let op = declared_op(&session, "create")?;
    let mut args = map_args(&op, &options.words).map_err(CliError::usage)?;
    if let Some(Value::String(epic)) = args.get("epic").cloned() {
        let (tasks, _invalid) = session.list()?;
        args.insert(
            "epic".to_string(),
            Value::String(resolve_epic(&tasks, &epic)?),
        );
    }
    let result = session
        .client
        .apply(&ApplyParams {
            op: op.name.clone(),
            id: None,
            args,
            version: None,
            checkout: session.checkout_arg(),
        })
        .map_err(CliError::failed)?;
    let commit = options.write.commit.then(|| session.written_in());
    let subject = |task: &Task| format!("task {} create: {}", task.id, task.title);
    finish(
        result,
        commit,
        subject,
        options.json,
        out,
        err,
        |task, files| {
            let mut text = format!(
                "ok  created {}\n  title   {}\n",
                View::heading(task),
                task.title
            );
            for file in files {
                text.push_str(&format!("  file    {file}\n"));
            }
            text
        },
    )
}

fn run_reid(
    options: &ReidOptions,
    cwd: &Path,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<u8, CliError> {
    let mut session = open(cwd, options.write.checkout.as_deref())?;
    let op = declared_op(&session, "reid")?;
    let (tasks, _invalid) = session.list()?;
    let item = lookup(&tasks, &options.id)?;
    let old_id = item.id.clone();
    let args = map_args(&op, &options.words).map_err(CliError::usage)?;
    // No version: the provider reads every file it rewrites under its lock. Where two files share the id, it
    // renames the second, which `lookup` cannot name (the list holds only the first).
    let result = session
        .client
        .apply(&ApplyParams {
            op: op.name.clone(),
            id: Some(old_id.clone()),
            args,
            version: None,
            checkout: session.checkout_arg(),
        })
        .map_err(CliError::failed)?;
    let written_in = session.written_in();
    let commit = options.write.commit.then_some(written_in);
    let subject = |task: &Task| format!("task {} reid (was {old_id}): {}", task.id, task.title);
    finish(
        result,
        commit,
        subject,
        options.json,
        out,
        err,
        |task, files| {
            let mut text = format!(
                "ok  {}\n  id      {old_id} -> {}\n",
                View::heading(task),
                task.id
            );
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
    )
}
