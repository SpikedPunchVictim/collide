//! `collide tasks`, `collide task <id>` and `collide task <id> <op> ...` (ts-0020). Until the daemon exists the CLI
//! starts the built-in `collide/plan` provider itself (through the loader of ts-0029), asks it for the items, and
//! applies the ready rule and short-id lookup of `core` to what it lists. The corpus stage moves this behind the
//! provider supervisor.

use crate::builtin_packs::{BuiltinPacks, Environment};
use crate::checkout::main_checkout;
use crate::pack_location::{self, locate_bundle};
use crate::provider_client::ProviderClient;
use crate::task_args::{describe, map_args};
use crate::task_view::View;
use collide_core::item::OpDecl;
use collide_core::provider::{ApplyParams, ApplyResult, InitializeParams, InitializeResult, Task};
use collide_core::ready::readiness;
use collide_core::short_id::{Lookup, resolve};
use serde_json::json;
use std::collections::BTreeMap;
use std::io::Write;
use std::path::Path;

/// Exit codes. An op's outcome has its own: `ok` is 0, `conflict` 4 and `refused` 5.
pub mod exit {
    pub const OK: u8 = 0;
    /// Something failed: no repository, no built-in packs, the provider did not start or answered badly.
    pub const FAILED: u8 = 1;
    /// The command line is wrong: an option, an op, an argument.
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
  collide task <id> <op> [<value>] [name=value ...] [--json]";

/// What `collide help` prints.
pub const HELP: &str = "\
collide tasks: list the items under .collide/planning/epics/ (--ready: only the items that can be started now;
  --kind: only items of that kind; --json: machine-readable)
collide task <id>: show one item
collide task <id> <op> ...: change one item with an op the provider declares (an unknown op lists them)

<id> is the full id (ts-0012-ua) or a short form: 12, 0012, 12-ua, ts-12. A short form must match exactly one item;
if it matches several they are listed with their epics, and you add the suffix.

An op's words are name=value, or one bare value that sets the op's main property (setStatus active sets status).
Lengths and patterns are checked by the provider.

An item is ready when its status is ready and each blocked-by target is done or dropped. A blocked-by cycle, and
a blocked-by target that is no item, keep their items from being ready and are reported on stderr.

exit codes: 0 ok, 1 failed, 2 wrong command line, 3 id not found or ambiguous, 4 conflict (nothing was written),
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
    pub json: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Tasks(TasksOptions),
    Task(TaskOptions),
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
    let mut plain = Vec::new();
    for word in args {
        match word.as_str() {
            "--json" => json = true,
            other if other.starts_with("--") => {
                return Err(CliError::usage(format!("unknown option `{other}`")));
            }
            _ => plain.push(word.clone()),
        }
    }
    let mut plain = plain.into_iter();
    let Some(id) = plain.next() else {
        return Err(CliError::usage(
            "`collide task` needs an id, such as 12 or ts-0012-ua",
        ));
    };
    let op = plain.next();
    Ok(Command::Task(TaskOptions {
        id,
        op,
        words: plain.collect(),
        json,
    }))
}

/// A started provider, initialized with the main checkout as its root.
struct Session {
    client: ProviderClient,
    init: InitializeResult,
}

fn open(cwd: &Path) -> Result<Session, CliError> {
    let root = main_checkout(cwd).map_err(CliError::failed)?;
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
    Ok(Session { client, init })
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
    let mut session = open(cwd)?;
    if let Some(kind) = &options.kind {
        let kinds: Vec<&str> = session.init.kinds.iter().map(|k| k.name.as_str()).collect();
        if !kinds.is_empty() && !kinds.contains(&kind.as_str()) {
            return Err(CliError::usage(format!(
                "unknown kind `{kind}`; the provider declares: {}",
                kinds.join(", ")
            )));
        }
    }
    let (tasks, invalid) = session.client.list_all().map_err(CliError::failed)?;
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

fn run_task(
    options: &TaskOptions,
    cwd: &Path,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<u8, CliError> {
    let mut session = open(cwd)?;
    let (tasks, _invalid) = session.client.list_all().map_err(CliError::failed)?;
    let query = &options.id;
    let item = match resolve(&tasks, query) {
        Lookup::One(item) => item,
        Lookup::Many(matches) => {
            return Err(CliError::new(exit::ID, ambiguous_message(query, &matches)));
        }
        Lookup::None => {
            return Err(CliError::new(
                exit::ID,
                format!(
                    "no item matches `{query}`: looked for the id `{query}` and for items with that number \
                     (and suffix, if given) among the {} items under .collide/planning/epics/",
                    tasks.len()
                ),
            ));
        }
        Lookup::Malformed => {
            return Err(CliError::usage(format!(
                "`{query}` is not an item id; use the full id (ts-0012-ua) or a short form (12, 12-ua, ts-12)"
            )));
        }
    };
    let found = readiness(&tasks);
    let view = View::new(&tasks, &found);
    let current = session
        .client
        .get(&item.id)
        .map_err(CliError::failed)?
        .ok_or_else(|| {
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

    let ops = declared_ops(&session.init, current.kind.as_deref());
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
            checkout: None,
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
    match result {
        ApplyResult::Ok { task, files, .. } => {
            if !options.json {
                emit(out, &ok_text(&current, &task, &files))?;
            }
            Ok(exit::OK)
        }
        ApplyResult::Conflict { current: latest } => {
            if !options.json {
                let _ = writeln!(
                    err,
                    "collide: conflict: {} changed after it was read; nothing was written. It is now:\n{}",
                    View::heading(&latest),
                    view.show_text(&latest).trim_end()
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
