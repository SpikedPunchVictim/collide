//! `collide pack update <name> [--to <ref>] [--yes] [--allow-host <host>]` (ts-0100-pu; 0.7, Switching packs), through
//! the daemon's `config.pack_update`. The rest of `collide pack` is `ts-0101-bd`'s and is still the `cli: pack` stub.
//!
//! The command fetches the pack's repository, prints the old and new commit and what changed (the manifest, the hooks
//! and every other file), and writes the lock only on a yes: `--yes`, or `y` at the question. What is written is the
//! commit that was shown: the confirming call names it, so a remote that moves in between changes nothing.
//!
//! The first fetch from a host never fetched from is asked about, naming the host: `y` at the question or
//! `--allow-host <host>` records it; anything else (and an empty input) fetches nothing and exits [`exit::REFUSED`].
//! The daemon records only the host that fetch needs. Exit codes: [`exit::OK`] (also when nothing was to change or the
//! person said no), [`exit::USAGE`], [`exit::ID`] for a pack that is not there, [`exit::FAILED`] for a refusal or a
//! failure, and [`exit::REFUSED`] for a host that was not approved.

use crate::config_cli::open_board;
use crate::daemon::{DaemonPaths, ROOT_ENV};
use crate::launch::ensure_running_from_cli;
use crate::owner_client::{ClientError, OwnerClient};
use crate::task_cli::exit;
use collide_core::owner::{
    ConfigPackUpdateParams, ConfigPackUpdateResult, PackFileChangeKind, PackOrigin, code,
};
use std::io::{Read, Write};
use std::path::Path;
use std::time::Duration;

pub const USAGE: &str =
    "usage: collide pack update <name> [--to <ref>] [--yes] [--allow-host <host>]";

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

/// Reads the arguments after `pack`. The error is the message to print with the usage.
pub fn parse(args: &[String]) -> Result<UpdateArgs, String> {
    let (command, rest) = args
        .split_first()
        .ok_or("name what to do: `collide pack update <name>`")?;
    if command != "update" {
        return Err(format!(
            "`{command}` is not built yet; `update` is the only `collide pack` command here"
        ));
    }
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
            "--allow-host" => {
                let host = rest.next().ok_or("--allow-host needs a host name")?;
                if host.is_empty() {
                    return Err("the host after --allow-host is empty".into());
                }
                allow_hosts.push(host.to_ascii_lowercase());
            }
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
    Ok(UpdateArgs {
        name,
        to,
        yes,
        allow_hosts,
    })
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
            let _ = writeln!(err, "collide pack update: {message}");
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
    let parsed = match parse(args) {
        Ok(parsed) => parsed,
        Err(message) => {
            let _ = writeln!(err, "collide pack update: {message}\n{USAGE}");
            return exit::USAGE;
        }
    };
    if let Err(message) = ensure_running_from_cli(paths) {
        let _ = writeln!(
            err,
            "collide pack update: the daemon is not running and did not start: {message}"
        );
        return exit::FAILED;
    }
    let mut client = match OwnerClient::connect(paths, "collide pack") {
        Ok(client) => client,
        Err(error) => {
            let _ = writeln!(err, "collide pack update: {error}");
            return exit::FAILED;
        }
    };
    let board = match open_board(&mut client, cwd) {
        Ok(Some(board)) => board,
        Ok(None) => {
            let _ = writeln!(
                err,
                "collide pack update: {} is not in a git checkout, and a pack is updated for a board",
                cwd.display()
            );
            return exit::FAILED;
        }
        Err(error) => {
            let _ = writeln!(err, "collide pack update: {error}");
            return exit::FAILED;
        }
    };
    let mut params = ConfigPackUpdateParams {
        board,
        name: parsed.name.clone(),
        to: parsed.to.clone(),
        confirm: false,
        approve_hosts: parsed.allow_hosts.clone(),
    };
    let mut result = match call(&mut client, &params, err) {
        Ok(result) => result,
        Err(code) => return code,
    };
    if let Some(host) = result.needs_host.clone() {
        let _ = writeln!(
            out,
            "The first fetch from {host} needs your yes: Collide has not fetched from it before."
        );
        if !ask(input, out, &format!("Fetch from {host}?")) {
            let _ = writeln!(
                err,
                "collide pack update: {host} was not approved, so nothing was fetched"
            );
            return exit::REFUSED;
        }
        params.approve_hosts.push(host);
        result = match call(&mut client, &params, err) {
            Ok(result) => result,
            Err(code) => return code,
        };
    }
    let Some(new) = result.new.clone() else {
        let _ = writeln!(err, "collide pack update: the daemon gave no new pin");
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
    if !parsed.yes && !ask(input, out, &format!("Write the new pin to {lock}?")) {
        let _ = writeln!(out, "nothing was written");
        return exit::OK;
    }
    // The commit that was shown is the one written, whatever the remote has done since.
    params.confirm = true;
    params.to = Some(new.commit);
    match call(&mut client, &params, err) {
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

fn call(
    client: &mut OwnerClient,
    params: &ConfigPackUpdateParams,
    err: &mut dyn Write,
) -> Result<ConfigPackUpdateResult, u8> {
    match client.call::<_, ConfigPackUpdateResult>("config.pack_update", params, Some(WAIT)) {
        Ok(result) => Ok(result),
        Err(ClientError::Rpc(error)) => {
            let _ = writeln!(err, "collide pack update: {}", error.message);
            Err(if error.code == code::NOT_FOUND {
                exit::ID
            } else {
                exit::FAILED
            })
        }
        Err(error) => {
            let _ = writeln!(err, "collide pack update: {error}");
            Err(exit::FAILED)
        }
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
