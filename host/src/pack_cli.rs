//! `collide pack add|link|enable|disable|list|update` (ts-0101-bd, ts-0100-pu; 0.7, Switching packs), through the
//! daemon's `config.pack_*`. `collide pack test` is a later stage's.
//!
//! - `add <source>`: shows what the pack runs (its review: source, commit, hooks, providers, observers, `requires`)
//!   and writes only on a yes (`--yes`, or `y` at the question). What is trusted is the tree the review showed.
//! - `link <name> [<folder>]`: a working copy over a pack, for you on this board.
//! - `enable <name>` and `disable <name>`: a pack of yours on or off for this board.
//! - `list`: the board's packs, then yours, one line each: name, origin, `on` or `off`, `dev` and `working-copy`
//!   marks, source.
//! - `update <name> [--to <ref>]`: fetches the pack's repository and prints the old and new commit and what changed
//!   (the manifest, the hooks and every other file). The lock is written only on a yes. What is written is the commit
//!   that was shown: the confirming call names it, so a remote that moves in between changes nothing.
//!
//! The first fetch from a host never fetched from is asked about, naming the host: `y` at the question or
//! `--allow-host <host>` records it; anything else (and an empty input) fetches nothing and exits [`exit::REFUSED`].
//! The daemon records only the host that fetch needs. Exit codes: [`exit::OK`] (also when nothing was to change or the
//! person said no), [`exit::USAGE`], [`exit::ID`] for a pack that is not there, [`exit::FAILED`] for a refusal or a
//! failure, [`exit::REFUSED`] for a host that was not approved and [`exit::NOT_BUILT`] for `test`.

use crate::config_cli::open_board;
use crate::daemon::{DaemonPaths, ROOT_ENV};
use crate::launch::ensure_running_from_cli;
use crate::owner_client::{ClientError, OwnerClient};
use crate::task_cli::exit;
use collide_core::owner::{
    ConfigPackAddParams, ConfigPackEnableParams, ConfigPackLinkParams, ConfigPackListParams,
    ConfigPackListResult, ConfigPackResult, ConfigPackUpdateParams, ConfigPackUpdateResult,
    PackEntry, PackFileChangeKind, PackOrigin, PackReviewData, code,
};
use collide_core::trust::{PackReview, ReviewEntry};
use std::io::{Read, Write};
use std::path::Path;
use std::time::Duration;

pub const USAGE: &str = "\
usage: collide pack add <source> [--yes] [--allow-host <host>]
usage: collide pack link <name> [<folder>]
usage: collide pack enable <name>
usage: collide pack disable <name>
usage: collide pack list
usage: collide pack update <name> [--to <ref>] [--yes] [--allow-host <host>]";

/// A fetch may take 120 s, and the call waits a little longer than that.
const WAIT: Duration = Duration::from_secs(150);

/// What `pack update` was asked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateArgs {
    pub name: String,
    pub to: Option<String>,
    pub yes: bool,
    pub allow_hosts: Vec<String>,
}

/// What `pack add` was asked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddArgs {
    pub source: String,
    pub yes: bool,
    pub allow_hosts: Vec<String>,
}

/// What `collide pack` was asked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Add(AddArgs),
    Link {
        name: String,
        folder: Option<String>,
    },
    Enable {
        name: String,
        enabled: bool,
    },
    List,
    Update(UpdateArgs),
    /// `collide pack test`: a later stage.
    Test,
}

impl Command {
    fn word(&self) -> &'static str {
        match self {
            Command::Add(_) => "add",
            Command::Link { .. } => "link",
            Command::Enable { enabled: true, .. } => "enable",
            Command::Enable { enabled: false, .. } => "disable",
            Command::List => "list",
            Command::Update(_) => "update",
            Command::Test => "test",
        }
    }
}

/// Reads the arguments after `pack`. The error is the message to print with the usage.
pub fn parse(args: &[String]) -> Result<Command, String> {
    let (command, rest) = args
        .split_first()
        .ok_or("name what to do: add, link, enable, disable, list or update")?;
    match command.as_str() {
        "add" => parse_add(rest),
        "link" => parse_link(rest),
        "enable" | "disable" => Ok(Command::Enable {
            name: one_name(rest, "pack")?,
            enabled: command == "enable",
        }),
        "list" => match rest.first() {
            None => Ok(Command::List),
            Some(extra) => Err(format!("`list` takes no arguments, and got `{extra}`")),
        },
        "update" => parse_update(rest),
        "test" => Ok(Command::Test),
        other => Err(format!("`{other}` is not a `collide pack` command")),
    }
}

fn one_name(rest: &[String], what: &str) -> Result<String, String> {
    match rest {
        [] => Err(format!("name the {what}")),
        [name] if name.is_empty() => Err(format!("the {what}'s name is empty")),
        [name] if name.starts_with('-') => Err(format!("unknown option `{name}`")),
        [name] => Ok(name.clone()),
        [_, extra, ..] => Err(format!("one {what} at a time: `{extra}` is extra")),
    }
}

fn parse_link(rest: &[String]) -> Result<Command, String> {
    let (name, folder) = match rest {
        [] => return Err("name the pack to link".into()),
        [name] => (name, None),
        [name, folder] => (name, Some(folder)),
        [_, _, extra, ..] => return Err(format!("`{extra}` is extra")),
    };
    if name.is_empty() || name.starts_with('-') {
        return Err(format!("`{name}` is not a pack's name"));
    }
    if folder.is_some_and(|folder| folder.is_empty()) {
        return Err("the folder is empty".into());
    }
    Ok(Command::Link {
        name: name.clone(),
        folder: folder.cloned(),
    })
}

fn parse_add(rest: &[String]) -> Result<Command, String> {
    let mut source: Option<String> = None;
    let mut yes = false;
    let mut allow_hosts = Vec::new();
    let mut rest = rest.iter();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--allow-host" => allow_hosts.push(host_after(&mut rest)?),
            "--yes" => yes = true,
            other if other.starts_with('-') => return Err(format!("unknown option `{other}`")),
            "" => return Err("the source is empty".into()),
            other => {
                if source.replace(other.to_string()).is_some() {
                    return Err("only one pack can be added at a time".into());
                }
            }
        }
    }
    let source = source.ok_or("name the source to add: `git:<url>` or `dev:<folder>`")?;
    Ok(Command::Add(AddArgs {
        source,
        yes,
        allow_hosts,
    }))
}

fn host_after(rest: &mut std::slice::Iter<String>) -> Result<String, String> {
    let host = rest.next().ok_or("--allow-host needs a host name")?;
    if host.is_empty() {
        return Err("the host after --allow-host is empty".into());
    }
    Ok(host.to_ascii_lowercase())
}

fn parse_update(rest: &[String]) -> Result<Command, String> {
    let mut name: Option<String> = None;
    let mut to: Option<String> = None;
    let mut yes = false;
    let mut allow_hosts = Vec::new();
    let mut rest = rest.iter();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--to" => {
                let reference = rest
                    .next()
                    .ok_or("--to needs a branch, a tag or a commit")?;
                if reference.is_empty() {
                    return Err("the ref after --to is empty".into());
                }
                if to.replace(reference.clone()).is_some() {
                    return Err("--to was given twice".into());
                }
            }
            "--allow-host" => allow_hosts.push(host_after(&mut rest)?),
            "--yes" => yes = true,
            other if other.starts_with('-') => return Err(format!("unknown option `{other}`")),
            "" => return Err("the pack's name is empty".into()),
            other => {
                if name.replace(other.to_string()).is_some() {
                    return Err("only one pack can be updated at a time".into());
                }
            }
        }
    }
    let name = name.ok_or("name the pack to update")?;
    Ok(Command::Update(UpdateArgs {
        name,
        to,
        yes,
        allow_hosts,
    }))
}

/// Runs the command with this process's own environment. `args` are the arguments after `pack`.
pub fn run(
    args: &[String],
    cwd: &Path,
    input: &mut dyn Read,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    match DaemonPaths::from_env(
        std::env::var(ROOT_ENV).ok().as_deref(),
        std::env::var("HOME").ok().as_deref(),
    ) {
        Ok(paths) => run_with(&paths, args, cwd, input, out, err),
        Err(message) => {
            let _ = writeln!(err, "collide pack: {message}");
            exit::FAILED
        }
    }
}

/// Reads one line from `input` a byte at a time, so nothing past it is taken from a stream a caller still reads.
fn read_line(input: &mut dyn Read) -> String {
    let mut line = Vec::new();
    let mut byte = [0u8; 1];
    while let Ok(1) = input.read(&mut byte) {
        if byte[0] == b'\n' {
            break;
        }
        line.push(byte[0]);
    }
    String::from_utf8_lossy(&line).trim().to_ascii_lowercase()
}

/// Asks `question`; only `y` or `yes` is a yes, and so an empty input is a no.
fn ask(input: &mut dyn Read, out: &mut dyn Write, question: &str) -> bool {
    let _ = write!(out, "{question} [y/N] ");
    let _ = out.flush();
    let answer = read_line(input);
    let _ = writeln!(out);
    answer == "y" || answer == "yes"
}

/// Runs the command against the daemon at `paths`, from the folder `cwd`.
pub fn run_with(
    paths: &DaemonPaths,
    args: &[String],
    cwd: &Path,
    input: &mut dyn Read,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let command = match parse(args) {
        Ok(command) => command,
        Err(message) => {
            let _ = writeln!(err, "collide pack: {message}\n{USAGE}");
            return exit::USAGE;
        }
    };
    let word = command.word();
    if command == Command::Test {
        let _ = writeln!(
            err,
            "collide pack test: a later stage builds it (`tsc --noEmit` over a pack)"
        );
        return exit::NOT_BUILT;
    }
    if let Err(message) = ensure_running_from_cli(paths) {
        let _ = writeln!(
            err,
            "collide pack {word}: the daemon is not running and did not start: {message}"
        );
        return exit::FAILED;
    }
    let mut client = match OwnerClient::connect(paths, "collide pack") {
        Ok(client) => client,
        Err(error) => {
            let _ = writeln!(err, "collide pack {word}: {error}");
            return exit::FAILED;
        }
    };
    // `add` is the user's and needs no board; every other word is for the board of the folder it runs in.
    let board = if matches!(command, Command::Add(_)) {
        String::new()
    } else {
        match open_board(&mut client, cwd) {
            Ok(Some(board)) => board,
            Ok(None) => {
                let _ = writeln!(
                    err,
                    "collide pack {word}: {} is not in a git checkout, and a pack is managed for a board",
                    cwd.display()
                );
                return exit::FAILED;
            }
            Err(error) => {
                let _ = writeln!(err, "collide pack {word}: {error}");
                return exit::FAILED;
            }
        }
    };
    let mut call = Caller {
        client: &mut client,
        word,
        err,
    };
    match command {
        Command::Add(args) => add(&mut call, &args, cwd, input, out),
        Command::Link { name, folder } => {
            let folder = folder.map(|folder| absolute(cwd, &folder));
            let params = ConfigPackLinkParams {
                board,
                name,
                folder,
            };
            match call.call::<_, ConfigPackResult>("config.pack_link", &params) {
                Ok(result) => print_change(out, &result, "linked", " as a working copy"),
                Err(code) => code,
            }
        }
        Command::Enable { name, enabled } => {
            let params = ConfigPackEnableParams {
                board,
                name,
                enabled,
            };
            match call.call::<_, ConfigPackResult>("config.pack_enable", &params) {
                Ok(result) => {
                    let done = if enabled { "enabled" } else { "disabled" };
                    print_change(out, &result, done, " for this board")
                }
                Err(code) => code,
            }
        }
        Command::List => {
            match call.call::<_, ConfigPackListResult>(
                "config.pack_list",
                &ConfigPackListParams { board },
            ) {
                Ok(result) => {
                    print_list(out, &result.packs);
                    exit::OK
                }
                Err(code) => code,
            }
        }
        Command::Update(args) => update(&mut call, &args, board, input, out),
        Command::Test => unreachable!("answered above"),
    }
}

/// A folder as the daemon wants it: from `/` or `~/`. A relative one is from the folder the person is in.
fn absolute(cwd: &Path, folder: &str) -> String {
    if folder.starts_with('/') || folder.starts_with("~/") {
        folder.to_string()
    } else {
        cwd.join(folder).display().to_string()
    }
}

/// The connection to the daemon, and where an error goes.
struct Caller<'a> {
    client: &'a mut OwnerClient,
    word: &'static str,
    err: &'a mut dyn Write,
}

impl Caller<'_> {
    /// One call. An error is printed and is the exit code.
    fn call<P: serde::Serialize, R: serde::de::DeserializeOwned>(
        &mut self,
        method: &str,
        params: &P,
    ) -> Result<R, u8> {
        match self.client.call::<_, R>(method, params, Some(WAIT)) {
            Ok(result) => Ok(result),
            Err(ClientError::Rpc(error)) => {
                let _ = writeln!(self.err, "collide pack {}: {}", self.word, error.message);
                Err(if error.code == code::NOT_FOUND {
                    exit::ID
                } else {
                    exit::FAILED
                })
            }
            Err(error) => {
                let _ = writeln!(self.err, "collide pack {}: {error}", self.word);
                Err(exit::FAILED)
            }
        }
    }
}

fn add(
    call: &mut Caller,
    args: &AddArgs,
    cwd: &Path,
    input: &mut dyn Read,
    out: &mut dyn Write,
) -> u8 {
    let source = match args.source.strip_prefix("dev:") {
        Some(folder) => format!("dev:{}", absolute(cwd, folder)),
        None => args.source.clone(),
    };
    let mut params = ConfigPackAddParams {
        source,
        confirm: false,
        approve_hosts: args.allow_hosts.clone(),
    };
    let mut result = match call.call::<_, ConfigPackResult>("config.pack_add", &params) {
        Ok(result) => result,
        Err(code) => return code,
    };
    if let Some(host) = result.needs_host.clone() {
        if !approve_host(call.word, &host, input, out, call.err) {
            return exit::REFUSED;
        }
        params.approve_hosts.push(host);
        result = match call.call::<_, ConfigPackResult>("config.pack_add", &params) {
            Ok(result) => result,
            Err(code) => return code,
        };
    }
    match &result.pack.review {
        Some(review) => {
            let _ = write!(out, "{}", render_review(review, &params.source));
        }
        None => {
            // Already added: nothing to review, and the note says so.
            if let Some(note) = &result.note {
                let _ = writeln!(out, "{note}");
            }
            return exit::OK;
        }
    }
    if !args.yes && !ask(input, out, "Trust this pack and add it?") {
        let _ = writeln!(out, "nothing was written");
        return exit::OK;
    }
    params.confirm = true;
    match call.call::<_, ConfigPackResult>("config.pack_add", &params) {
        Ok(done) if done.written => {
            let _ = writeln!(
                out,
                "added {} ({}): it is off until a board enables it with `collide pack enable {}`",
                done.pack.name, done.pack.source, done.pack.name
            );
            exit::OK
        }
        Ok(done) => {
            let _ = writeln!(
                out,
                "{}",
                done.note.unwrap_or_else(|| "nothing was written".into())
            );
            exit::OK
        }
        Err(code) => code,
    }
}

/// The first fetch from `host`: asks, and says what happens on a no.
fn approve_host(
    word: &str,
    host: &str,
    input: &mut dyn Read,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> bool {
    let _ = writeln!(
        out,
        "The first fetch from {host} needs your yes: Collide has not fetched from it before."
    );
    if ask(input, out, &format!("Fetch from {host}?")) {
        return true;
    }
    let _ = writeln!(
        err,
        "collide pack {word}: {host} was not approved, so nothing was fetched"
    );
    false
}

/// What the person is asked to trust, in the words of the trust prompt.
fn render_review(review: &PackReviewData, source: &str) -> String {
    let entries = |lines: &[collide_core::owner::PackReviewLine]| -> Vec<ReviewEntry> {
        lines
            .iter()
            .map(|line| ReviewEntry {
                key: line.key.clone(),
                script: line.script.clone(),
            })
            .collect()
    };
    let shown = PackReview {
        name: review.name.clone(),
        version: review.version.clone(),
        hooks: entries(&review.hooks),
        providers: entries(&review.providers),
        observers: entries(&review.observers),
        requires: review.requires.clone(),
    };
    shown.render(source, review.commit.as_deref())
}

fn print_change(out: &mut dyn Write, result: &ConfigPackResult, done: &str, suffix: &str) -> u8 {
    let name = &result.pack.name;
    match (&result.note, result.written) {
        (Some(note), false) => {
            let _ = writeln!(out, "{note}");
        }
        _ => {
            let _ = writeln!(out, "{done} {name}{suffix}");
        }
    }
    exit::OK
}

fn print_list(out: &mut dyn Write, packs: &[PackEntry]) {
    for pack in packs {
        let origin = match pack.origin {
            PackOrigin::Builtin => "builtin",
            PackOrigin::Repo => "repo",
            PackOrigin::User => "user",
            PackOrigin::Offered => "offered",
        };
        let state = if pack.enabled { "on" } else { "off" };
        let mut marks = Vec::new();
        if pack.dev {
            marks.push("dev");
        }
        if pack.working_copy {
            marks.push("working-copy");
        }
        let marks = if marks.is_empty() {
            "-".to_string()
        } else {
            marks.join(",")
        };
        let _ = writeln!(
            out,
            "{}  {origin}  {state}  {marks}  {}",
            pack.name, pack.source
        );
    }
}

fn update(
    call: &mut Caller,
    args: &UpdateArgs,
    board: String,
    input: &mut dyn Read,
    out: &mut dyn Write,
) -> u8 {
    let mut params = ConfigPackUpdateParams {
        board,
        name: args.name.clone(),
        to: args.to.clone(),
        confirm: false,
        approve_hosts: args.allow_hosts.clone(),
    };
    let mut result = match call.call::<_, ConfigPackUpdateResult>("config.pack_update", &params) {
        Ok(result) => result,
        Err(code) => return code,
    };
    if let Some(host) = result.needs_host.clone() {
        if !approve_host(call.word, &host, input, out, call.err) {
            return exit::REFUSED;
        }
        params.approve_hosts.push(host);
        result = match call.call::<_, ConfigPackUpdateResult>("config.pack_update", &params) {
            Ok(result) => result,
            Err(code) => return code,
        };
    }
    let Some(new) = result.new.clone() else {
        let _ = writeln!(call.err, "collide pack update: the daemon gave no new pin");
        return exit::FAILED;
    };
    print_preview(out, &result);
    if new == result.old {
        return exit::OK;
    }
    let lock = if result.pack.origin == PackOrigin::User {
        "your packs.lock"
    } else {
        "this checkout's .collide/packs.lock"
    };
    if !args.yes && !ask(input, out, &format!("Write the new pin to {lock}?")) {
        let _ = writeln!(out, "nothing was written");
        return exit::OK;
    }
    // The commit that was shown is the one written, whatever the remote has done since.
    params.confirm = true;
    params.to = Some(new.commit);
    match call.call::<_, ConfigPackUpdateResult>("config.pack_update", &params) {
        Ok(done) => {
            let file = done.lock_file.unwrap_or_default();
            let _ = writeln!(out, "wrote {file}");
            if result.pack.origin == PackOrigin::Repo {
                let _ = writeln!(out, "the change is not committed: commit it to share it");
            }
            exit::OK
        }
        Err(code) => code,
    }
}

fn short(id: &str) -> &str {
    id.get(..7).unwrap_or(id)
}

fn print_preview(out: &mut dyn Write, result: &ConfigPackUpdateResult) {
    let Some(new) = &result.new else {
        return;
    };
    let old = &result.old;
    let _ = writeln!(out, "{} ({})", result.pack.name, result.pack.source);
    let _ = writeln!(
        out,
        "  commit  {} -> {}",
        short(&old.commit),
        short(&new.commit)
    );
    let _ = writeln!(
        out,
        "  tree    {} -> {}",
        short(&old.tree),
        short(&new.tree)
    );
    if let Some(note) = &result.note {
        let _ = writeln!(out, "  {note}");
    }
    for change in &result.changes {
        let kind = match change.kind {
            PackFileChangeKind::Added => "added",
            PackFileChangeKind::Deleted => "deleted",
            PackFileChangeKind::Modified => "modified",
            PackFileChangeKind::TypeChanged => "type changed",
        };
        let _ = writeln!(out, "  {kind} {}", change.path);
        for line in &change.lines {
            let _ = writeln!(out, "    {line}");
        }
        if let Some(note) = &change.note {
            let _ = writeln!(out, "    ({note})");
        }
    }
}
