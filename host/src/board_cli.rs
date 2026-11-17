//! `collide init` and `collide trust` (ts-0102-vu; 0.11 Share with team, 0.9 Trust), through the daemon's
//! `board.init`, `board.trust_review` and `board.trust`.
//!
//! - `init [--dry-run]`: writes `.collide/board.yaml` (a short comment and `packs: []`) in the checkout you are in and
//!   nothing else. It never commits and never edits `.gitignore`. It refuses when `.collide` exists. `--dry-run` says
//!   what would be written and writes nothing.
//! - `trust [--line <ref>] [--yes] [--allow-host <host>] [--allow-dev <folder>]`: shows the line's trust hash, each
//!   change since it was trusted with its diff, and the review of each pack its repo supplies. Then it asks
//!   `Trust this? [y/N]`, and what is recorded is the hash that was shown. With no terminal and no `--yes` it
//!   refuses. The line is the checked-out branch unless `--line` names a ref.
//!
//! A line that cannot load until a host or a `dev:` folder is approved is asked about first: the host a `git:` pack
//! is fetched from for the first time, and a `dev:` folder never trusted. A `y` records it, the line is loaded and
//! reviewed again, and only then is the hash asked about. With no terminal these are `--allow-host` and `--allow-dev`
//! (`--yes` is for the hash alone).
//!
//! Exit codes: [`exit::OK`] (also for a line already trusted and for a no at the question), [`exit::USAGE`],
//! [`exit::ID`] for a ref that is no line, [`exit::FAILED`], and [`exit::REFUSED`] for a question that no person
//! answered or that was answered no where the load cannot go on without it.

use crate::config_cli::open_board_summary;
use crate::daemon::{DaemonPaths, ROOT_ENV};
use crate::launch::ensure_running_from_cli;
use crate::owner_client::{ClientError, OwnerClient};
use crate::pack_cli::{ask, render_review};
use crate::task_cli::exit;
use collide_core::owner::{
    BoardInitParams, BoardInitResult, BoardTrustParams, BoardTrustResult, BoardTrustReviewParams,
    BoardTrustReviewResult, PackFileChangeKind, TrustChange, TrustChangeKind, TrustState, code,
};
use std::io::{IsTerminal, Read, Write};
use std::path::Path;
use std::time::Duration;

pub const USAGE: &str = "\
usage: collide init [--dry-run]
usage: collide trust [--line <ref>] [--yes] [--allow-host <host>] [--allow-dev <folder>]";

/// A fetch may take 120 s, and the call waits a little longer than that.
const WAIT: Duration = Duration::from_secs(150);

/// How many times a line is loaded again after an approval. Each round approves what the last one needed.
const ROUNDS: usize = 8;

/// What `collide trust` was asked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustArgs {
    pub line: Option<String>,
    pub yes: bool,
    pub allow_hosts: Vec<String>,
    pub allow_dev: Vec<String>,
}

/// Reads the arguments after `init`: whether it is a dry run. The error is the message to print with the usage.
pub fn parse_init(args: &[String]) -> Result<bool, String> {
    match args {
        [] => Ok(false),
        [flag] if flag == "--dry-run" => Ok(true),
        [flag, ..] if flag == "--dry-run" => Err("`--dry-run` is given once".into()),
        [other, ..] if other.starts_with('-') => Err(format!("unknown option `{other}`")),
        [other, ..] => Err(format!("`init` takes no arguments, and got `{other}`")),
    }
}

/// Reads the arguments after `trust`. The error is the message to print with the usage.
pub fn parse_trust(args: &[String]) -> Result<TrustArgs, String> {
    let mut parsed = TrustArgs {
        line: None,
        yes: false,
        allow_hosts: Vec::new(),
        allow_dev: Vec::new(),
    };
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        let mut value = |what: &str| match rest.next() {
            Some(value) if !value.is_empty() => Ok(value.clone()),
            Some(_) => Err(format!("`{arg}` has an empty {what}")),
            None => Err(format!("`{arg}` needs a {what}")),
        };
        match arg.as_str() {
            "--yes" => {
                if std::mem::replace(&mut parsed.yes, true) {
                    return Err("`--yes` is given once".into());
                }
            }
            "--line" => {
                let line = value("ref")?;
                if parsed.line.replace(line).is_some() {
                    return Err("`--line` is given once".into());
                }
            }
            "--allow-host" => parsed.allow_hosts.push(value("host")?),
            "--allow-dev" => parsed.allow_dev.push(value("folder")?),
            other if other.starts_with('-') => return Err(format!("unknown option `{other}`")),
            other => return Err(format!("`trust` takes no arguments, and got `{other}`")),
        }
    }
    Ok(parsed)
}

fn daemon_paths(word: &str, err: &mut dyn Write) -> Option<DaemonPaths> {
    match DaemonPaths::from_env(
        std::env::var(ROOT_ENV).ok().as_deref(),
        std::env::var("HOME").ok().as_deref(),
    ) {
        Ok(paths) => Some(paths),
        Err(message) => {
            let _ = writeln!(err, "collide {word}: {message}");
            None
        }
    }
}

/// `collide init` with this process's own environment. `args` are the arguments after `init`.
pub fn run_init(args: &[String], cwd: &Path, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    match daemon_paths("init", err) {
        Some(paths) => init_with(&paths, args, cwd, out, err),
        None => exit::FAILED,
    }
}

/// `collide trust` with this process's own environment. `args` are the arguments after `trust`.
pub fn run_trust(
    args: &[String],
    cwd: &Path,
    input: &mut dyn Read,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    match daemon_paths("trust", err) {
        Some(paths) => trust_with(
            &paths,
            args,
            cwd,
            std::io::stdin().is_terminal(),
            input,
            out,
            err,
        ),
        None => exit::FAILED,
    }
}

/// The connection to the daemon, the board of the folder the command runs in, and where an error goes.
struct Session<'a> {
    client: OwnerClient,
    word: &'static str,
    err: &'a mut dyn Write,
}

impl Session<'_> {
    fn say(&mut self, message: &str) {
        let _ = writeln!(self.err, "collide {}: {message}", self.word);
    }

    /// One call. An error is printed and is the exit code.
    fn call<P: serde::Serialize, R: serde::de::DeserializeOwned>(
        &mut self,
        method: &str,
        params: &P,
    ) -> Result<R, u8> {
        match self.client.call::<_, R>(method, params, Some(WAIT)) {
            Ok(result) => Ok(result),
            Err(ClientError::Rpc(error)) => {
                self.say(&error.message);
                Err(if error.code == code::NOT_FOUND {
                    exit::ID
                } else {
                    exit::FAILED
                })
            }
            Err(error) => {
                self.say(&error.to_string());
                Err(exit::FAILED)
            }
        }
    }
}

/// Starts the daemon if need be and connects; the board of `cwd` with its lines. The error is the exit code.
fn open<'a>(
    paths: &DaemonPaths,
    word: &'static str,
    cwd: &Path,
    err: &'a mut dyn Write,
) -> Result<(Session<'a>, collide_core::owner::BoardSummary), u8> {
    if let Err(message) = ensure_running_from_cli(paths) {
        let _ = writeln!(
            err,
            "collide {word}: the daemon is not running and did not start: {message}"
        );
        return Err(exit::FAILED);
    }
    let mut client = match OwnerClient::connect(paths, &format!("collide {word}")) {
        Ok(client) => client,
        Err(error) => {
            let _ = writeln!(err, "collide {word}: {error}");
            return Err(exit::FAILED);
        }
    };
    match open_board_summary(&mut client, cwd) {
        Ok(Some(board)) => Ok((Session { client, word, err }, board)),
        Ok(None) => {
            let _ = writeln!(
                err,
                "collide {word}: {} is not in a git checkout, and a board is a checkout",
                cwd.display()
            );
            Err(exit::FAILED)
        }
        Err(error) => {
            let _ = writeln!(err, "collide {word}: {error}");
            Err(exit::FAILED)
        }
    }
}

/// Runs `collide init` against the daemon at `paths`, from the folder `cwd`.
pub fn init_with(
    paths: &DaemonPaths,
    args: &[String],
    cwd: &Path,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let dry_run = match parse_init(args) {
        Ok(dry_run) => dry_run,
        Err(message) => {
            let _ = writeln!(err, "collide init: {message}\n{USAGE}");
            return exit::USAGE;
        }
    };
    let (mut session, board) = match open(paths, "init", cwd, err) {
        Ok(opened) => opened,
        Err(code) => return code,
    };
    let params = BoardInitParams {
        board: board.id,
        dry_run,
    };
    let result: BoardInitResult = match session.call("board.init", &params) {
        Ok(result) => result,
        Err(code) => return code,
    };
    for file in &result.files {
        let _ = if result.written {
            writeln!(out, "Wrote {file}")
        } else {
            writeln!(out, "Dry run: would write {file}")
        };
    }
    if result.written {
        let _ = writeln!(
            out,
            "Nothing is committed. Add packs with `collide pack add`, commit it, then `collide trust`."
        );
    }
    exit::OK
}

fn kind_word(kind: TrustChangeKind) -> &'static str {
    match kind {
        TrustChangeKind::Added => "added",
        TrustChangeKind::Removed => "removed",
        TrustChangeKind::Changed => "changed",
    }
}

fn file_word(kind: PackFileChangeKind) -> &'static str {
    match kind {
        PackFileChangeKind::Added => "added",
        PackFileChangeKind::Deleted => "deleted",
        PackFileChangeKind::Modified => "modified",
        PackFileChangeKind::TypeChanged => "type changed",
    }
}

fn print_change(out: &mut dyn Write, change: &TrustChange) {
    let _ = writeln!(out, "  {} {}", kind_word(change.kind), change.name);
    for file in &change.files {
        let _ = writeln!(out, "    {} {}", file_word(file.kind), file.path);
        for line in &file.lines {
            let _ = writeln!(out, "      {line}");
        }
        if let Some(note) = &file.note {
            let _ = writeln!(out, "      ({note})");
        }
    }
    for line in &change.lines {
        let _ = writeln!(out, "    {line}");
    }
    if let Some(note) = &change.note {
        let _ = writeln!(out, "    ({note})");
    }
}

fn hash_of(state: &TrustState) -> &str {
    match state {
        TrustState::Untrusted { hash } | TrustState::Trusted { hash } => hash,
    }
}

/// What the load of a line needs approved before it can go on, from a review of it.
struct Needs {
    hosts: Vec<String>,
    dev_folders: Vec<String>,
}

/// The approvals a person gives, by flag or at the question. `None` when something was not approved: the exit code.
fn approvals(
    session: &mut Session,
    args: &TrustArgs,
    needs: &Needs,
    tty: bool,
    input: &mut dyn Read,
    out: &mut dyn Write,
) -> Result<(), u8> {
    let mut missing = Vec::new();
    let mut declined = false;
    for host in &needs.hosts {
        if args.allow_hosts.contains(host) {
            continue;
        }
        if tty {
            declined |= !ask(input, out, &format!("Fetch packs from {host}?"));
        } else {
            missing.push(format!("--allow-host {host}"));
        }
    }
    for folder in &needs.dev_folders {
        if args.allow_dev.contains(folder) {
            continue;
        }
        if tty {
            declined |= !ask(input, out, &format!("Trust the dev: folder {folder}?"));
        } else {
            missing.push(format!("--allow-dev {folder}"));
        }
    }
    if !missing.is_empty() {
        session.say(&format!(
            "the line cannot load until this is approved, and there is no terminal to ask at: pass {}",
            missing.join(" and ")
        ));
        return Err(exit::REFUSED);
    }
    if declined {
        session.say("not approved, so nothing was fetched or trusted");
        return Err(exit::REFUSED);
    }
    Ok(())
}

/// Loads and reviews the line, approving what it needs on the way, until there is a hash to ask about.
fn reviewed(
    session: &mut Session,
    args: &TrustArgs,
    board: &str,
    line: &str,
    tty: bool,
    input: &mut dyn Read,
    out: &mut dyn Write,
) -> Result<BoardTrustReviewResult, u8> {
    for _ in 0..ROUNDS {
        let params = BoardTrustReviewParams {
            board: board.to_string(),
            line: line.to_string(),
        };
        let review: BoardTrustReviewResult = session.call("board.trust_review", &params)?;
        if !hash_of(&review.trust).is_empty() {
            return Ok(review);
        }
        if review.hosts.is_empty() && review.dev_folders.is_empty() {
            for problem in &review.problems {
                session.say(problem);
            }
            return Err(exit::FAILED);
        }
        for problem in &review.problems {
            let _ = writeln!(out, "{problem}");
        }
        let needs = Needs {
            hosts: review.hosts.clone(),
            dev_folders: review.dev_folders.clone(),
        };
        approvals(session, args, &needs, tty, input, out)?;
        let params = BoardTrustParams {
            board: board.to_string(),
            line: line.to_string(),
            hash: String::new(),
            hosts: needs.hosts,
            dev_folders: needs.dev_folders,
        };
        let _: BoardTrustResult = session.call("board.trust", &params)?;
    }
    session.say("the line keeps needing more approvals; run `collide trust` again");
    Err(exit::FAILED)
}

/// Runs `collide trust` against the daemon at `paths`, from the folder `cwd`. `tty` is whether `input` is a person at
/// a terminal.
pub fn trust_with(
    paths: &DaemonPaths,
    args: &[String],
    cwd: &Path,
    tty: bool,
    input: &mut dyn Read,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let args = match parse_trust(args) {
        Ok(args) => args,
        Err(message) => {
            let _ = writeln!(err, "collide trust: {message}\n{USAGE}");
            return exit::USAGE;
        }
    };
    let (mut session, board) = match open(paths, "trust", cwd, err) {
        Ok(opened) => opened,
        Err(code) => return code,
    };
    let Some(line) = args
        .line
        .clone()
        .or_else(|| board.lines.first().map(|line| line.ref_name.clone()))
    else {
        session.say("the checkout has no commit yet, so it has no line to trust: commit first");
        return exit::FAILED;
    };
    let review = match reviewed(&mut session, &args, &board.id, &line, tty, input, out) {
        Ok(review) => review,
        Err(code) => return code,
    };
    let hash = hash_of(&review.trust).to_string();
    if matches!(review.trust, TrustState::Trusted { .. }) {
        let _ = writeln!(out, "Line {line} is already trusted at {hash}.");
        return exit::OK;
    }
    let _ = writeln!(out, "Line {line}\nTrust hash {hash}");
    if !review.changes.is_empty() {
        let _ = writeln!(out, "Changes:");
        for change in &review.changes {
            print_change(out, change);
        }
    }
    for pack in &review.packs {
        let _ = writeln!(out, "\n{}", render_review(&pack.review, &pack.source));
    }
    if !args.yes {
        if !tty {
            session.say("there is no terminal to ask at: run it at a terminal, or pass --yes to trust the hash above");
            return exit::REFUSED;
        }
        if !ask(input, out, "Trust this?") {
            let _ = writeln!(out, "Nothing was trusted.");
            return exit::OK;
        }
    }
    let params = BoardTrustParams {
        board: board.id,
        line: line.clone(),
        hash,
        hosts: Vec::new(),
        dev_folders: Vec::new(),
    };
    let result: BoardTrustResult = match session.call("board.trust", &params) {
        Ok(result) => result,
        Err(code) => return code,
    };
    match result.trust {
        TrustState::Trusted { hash } => {
            let _ = writeln!(out, "Trusted line {line} at {hash}.");
            exit::OK
        }
        TrustState::Untrusted { .. } => {
            session.say("the line is still not trusted");
            exit::FAILED
        }
    }
}
