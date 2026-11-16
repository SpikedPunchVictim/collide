//! `collide config set <key> <value> [--layer user|repo] [--global]` and `collide config set <key> --secret`
//! (ts-0099-bw; 0.7, Settings and Secrets), through the daemon's `config.set`.
//!
//! - A plain value is written to the user's file. Inside a checkout that is the board's own user file, so a pack's key
//!   can be checked against the pack's schema; `--global` writes the user's own file instead, for Collide's own keys.
//!   `--layer repo` is refused by the daemon, which says to edit and commit `.collide/config.yaml`.
//! - `--secret` reads the value from standard input and refuses one on the command line: arguments are visible to
//!   other processes and kept in shell history. One trailing newline is trimmed. The value goes to the daemon in the
//!   request and is never printed, logged or put in an error here; the line printed is `key = <secret, in the
//!   keychain>  (user)`.
//!
//! A bad command line exits [`exit::USAGE`] before any daemon is started and before standard input is read. A
//! refusal exits [`exit::FAILED`] with the reason on standard error, and a board that is not there exits [`exit::ID`].

use crate::config_cli::{entry_line, open_board};
use crate::daemon::{DaemonPaths, ROOT_ENV};
use crate::launch::ensure_running_from_cli;
use crate::owner_client::{ClientError, OwnerClient};
use crate::task_cli::exit;
use collide_core::owner::{ConfigSetParams, ConfigSetResult, ConfigWriteLayer, code};
use std::io::{Read, Write};
use std::path::Path;

pub const USAGE: &str = "usage: collide config set <key> <value> [--layer user|repo] [--global]\n       \
                         collide config set <key> --secret    (the secret is read from standard input)";

/// The most standard input a secret may have, in bytes. A secret is a token or a password.
pub const MAX_SECRET_BYTES: usize = 64 * 1024;

/// What `config set` was asked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetArgs {
    pub key: String,
    /// The value on the command line. `None` with `--secret`, which reads standard input.
    pub value: Option<String>,
    pub secret: bool,
    /// `--layer repo`.
    pub repo: bool,
    /// `--global`: the user's own file, not the board's.
    pub global: bool,
}

/// Reads the arguments after `set`: a key, a value (not with `--secret`), and the options in any order. `--` ends the
/// options, so a value may start with a dash. The error is the message to print with the usage, and never holds a
/// value.
pub fn parse_set(args: &[String]) -> Result<SetArgs, String> {
    let mut positional: Vec<&String> = Vec::new();
    let (mut secret, mut global, mut layer) = (false, false, None::<&String>);
    let mut options_done = false;
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        if options_done || !arg.starts_with('-') || arg == "-" {
            positional.push(arg);
        } else if arg == "--" {
            options_done = true;
        } else if arg == "--secret" {
            if std::mem::replace(&mut secret, true) {
                return Err("--secret is given twice".into());
            }
        } else if arg == "--global" {
            if std::mem::replace(&mut global, true) {
                return Err("--global is given twice".into());
            }
        } else if arg == "--layer" {
            let name = rest.next().ok_or("--layer needs `user` or `repo`")?;
            if layer.replace(name).is_some() {
                return Err("--layer is given twice".into());
            }
        } else {
            return Err(format!(
                "unknown option `{arg}`; put `--` before a value that starts with a dash"
            ));
        }
    }
    let repo = match layer.map(String::as_str) {
        None | Some("user") => false,
        Some("repo") => true,
        Some(_) => return Err("--layer is `user` or `repo`".into()),
    };
    let mut positional = positional.into_iter();
    let key = positional.next().ok_or("name the setting to set")?;
    if key.is_empty() {
        return Err("the key is empty".into());
    }
    let value = positional.next();
    if positional.next().is_some() {
        return Err("only one value can be set at a time".into());
    }
    if secret && value.is_some() {
        return Err("--secret reads the value from standard input and takes none on the command line, which other \
                    processes and the shell's history can read"
            .into());
    }
    if !secret && value.is_none() {
        return Err(format!(
            "give `{key}` a value, or --secret to read it from standard input"
        ));
    }
    Ok(SetArgs {
        key: key.clone(),
        value: value.cloned(),
        secret,
        repo,
        global,
    })
}

/// The secret on `input`: all of it up to [`MAX_SECRET_BYTES`], as text, with one trailing newline (`\n` or `\r\n`)
/// removed. The error says what was wrong and never holds what was read.
fn read_secret(input: &mut dyn Read) -> Result<String, String> {
    let mut bytes = Vec::new();
    input
        .take(MAX_SECRET_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("cannot read standard input: {error}"))?;
    if bytes.len() > MAX_SECRET_BYTES {
        return Err(format!(
            "standard input is too long for a secret: the most is {MAX_SECRET_BYTES} bytes"
        ));
    }
    let mut text =
        String::from_utf8(bytes).map_err(|_| "standard input is not text (UTF-8)".to_string())?;
    if text.ends_with('\n') {
        text.pop();
        if text.ends_with('\r') {
            text.pop();
        }
    }
    if text.is_empty() {
        return Err("the secret on standard input is empty".into());
    }
    Ok(text)
}

/// Runs the command with this process's own environment. `args` are the arguments after `set`.
pub fn run_set(
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
        Ok(paths) => run_set_with(&paths, args, cwd, input, out, err),
        Err(message) => {
            let _ = writeln!(err, "collide config set: {message}");
            exit::FAILED
        }
    }
}

/// Runs the command against the daemon at `paths`, from the folder `cwd`, with `input` as standard input.
pub fn run_set_with(
    paths: &DaemonPaths,
    args: &[String],
    cwd: &Path,
    input: &mut dyn Read,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let parsed = match parse_set(args) {
        Ok(parsed) => parsed,
        Err(message) => {
            let _ = writeln!(err, "collide config set: {message}\n{USAGE}");
            return exit::USAGE;
        }
    };
    let value = match &parsed.value {
        Some(text) => serde_json::Value::String(text.clone()),
        None => match read_secret(input) {
            Ok(secret) => serde_json::Value::String(secret),
            Err(message) => {
                let _ = writeln!(err, "collide config set: {message}");
                return exit::FAILED;
            }
        },
    };
    if let Err(message) = ensure_running_from_cli(paths) {
        let _ = writeln!(
            err,
            "collide config set: the daemon is not running and did not start: {message}"
        );
        return exit::FAILED;
    }
    let mut client = match OwnerClient::connect(paths, "collide config") {
        Ok(client) => client,
        Err(error) => {
            let _ = writeln!(err, "collide config set: {error}");
            return exit::FAILED;
        }
    };
    let board = if parsed.global {
        None
    } else {
        match open_board(&mut client, cwd) {
            Ok(board) => board,
            Err(error) => {
                let _ = writeln!(err, "collide config set: {error}");
                return exit::FAILED;
            }
        }
    };
    let params = ConfigSetParams {
        board,
        key: parsed.key,
        value,
        layer: if parsed.repo {
            ConfigWriteLayer::Repo
        } else {
            ConfigWriteLayer::User
        },
        secret: parsed.secret,
    };
    match client.call::<_, ConfigSetResult>("config.set", &params, None) {
        Ok(result) => {
            let _ = writeln!(out, "{}", entry_line(&result.entry));
            exit::OK
        }
        Err(ClientError::Rpc(error)) => {
            let _ = writeln!(
                err,
                "collide config set: {}",
                error.message.replace('\n', "\n  ")
            );
            if error.code == code::NOT_FOUND {
                exit::ID
            } else {
                exit::FAILED
            }
        }
        Err(error) => {
            let _ = writeln!(err, "collide config set: {error}");
            exit::FAILED
        }
    }
}
