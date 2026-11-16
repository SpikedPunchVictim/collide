//! `collide config get [<key>] [--profile <name>]` (ts-0098-nx; 0.7, Settings): the settings with the layer each value
//! came from, through the daemon's `config.get`. The folder the command runs in picks the board (`board.open`, which
//! finds the checkout the folder is in and gives the same board each time); outside a checkout there is no board, and
//! the answer is the user's layers over Collide's own keys. The daemon is started when it is not running, as `collide
//! stop` does ([`crate::launch::ensure_running_from_cli`]).
//!
//! A line is `key = value  (layer)`. A value is written as JSON (so a string is quoted and a number or a boolean is
//! not), a setting with no value says `<not set>`, and a secret never has a value: `key = <secret, in the keychain>
//! (user)`. A refusal prints every problem on standard error and nothing on standard output, and exits
//! [`exit::FAILED`]; a key, a profile or a board that is not there exits [`exit::ID`].
//!
//! `collide config set` is the keychain's and the writers' (`ts-0099-bw`), so it is still the tagged stub of
//! [`crate::commands`].

use crate::daemon::{DaemonPaths, ROOT_ENV};
use crate::launch::ensure_running_from_cli;
use crate::owner_client::{ClientError, OwnerClient};
use crate::task_cli::exit;
use collide_core::owner::{
    BoardOpenParams, BoardOpenResult, ConfigEntry, ConfigGetParams, ConfigGetResult, SettingLayer,
    code,
};
use std::io::Write;
use std::path::Path;

pub const USAGE: &str = "usage: collide config get [<key>] [--profile <name>]";

/// What `config get` was asked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GetArgs {
    pub key: Option<String>,
    pub profile: Option<String>,
}

/// Reads the arguments after `config`: `get`, then an optional key and an optional `--profile <name>` in either order.
/// The error is the message to print with the usage.
pub fn parse(args: &[String]) -> Result<GetArgs, String> {
    let (command, rest) = args
        .split_first()
        .ok_or("name what to do: `collide config get`")?;
    if command != "get" {
        return Err(format!(
            "`{command}` is not a `collide config` command that works yet; `get` does"
        ));
    }
    let mut key: Option<String> = None;
    let mut profile: Option<String> = None;
    let mut rest = rest.iter();
    while let Some(arg) = rest.next() {
        if arg == "--profile" {
            let name = rest.next().ok_or("--profile needs a profile's name")?;
            if name.is_empty() {
                return Err("the profile's name is empty".into());
            }
            if profile.replace(name.clone()).is_some() {
                return Err("only one profile can be applied at a time".into());
            }
        } else if arg.starts_with('-') {
            return Err(format!("unknown option `{arg}`"));
        } else if arg.is_empty() {
            return Err("the key is empty".into());
        } else if key.replace(arg.clone()).is_some() {
            return Err("only one key can be read at a time".into());
        }
    }
    Ok(GetArgs { key, profile })
}

fn layer_name(layer: SettingLayer) -> &'static str {
    match layer {
        SettingLayer::Default => "default",
        SettingLayer::User => "user",
        SettingLayer::Repo => "repo",
        SettingLayer::Profile => "profile",
    }
}

/// One entry, as `collide config get` prints it.
pub fn entry_line(entry: &ConfigEntry) -> String {
    let value = if entry.secret {
        "<secret, in the keychain>".to_string()
    } else {
        entry
            .value
            .as_ref()
            .map_or_else(|| "<not set>".to_string(), ToString::to_string)
    };
    format!("{} = {value}  ({})", entry.key, layer_name(entry.layer))
}

/// Runs the command with this process's own environment. `args` are the arguments after `config`.
pub fn run(args: &[String], cwd: &Path, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    match DaemonPaths::from_env(
        std::env::var(ROOT_ENV).ok().as_deref(),
        std::env::var("HOME").ok().as_deref(),
    ) {
        Ok(paths) => run_with(&paths, args, cwd, out, err),
        Err(message) => {
            let _ = writeln!(err, "collide config get: {message}");
            exit::FAILED
        }
    }
}

/// Runs the command against the daemon at `paths`, from the folder `cwd`.
pub fn run_with(
    paths: &DaemonPaths,
    args: &[String],
    cwd: &Path,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let parsed = match parse(args) {
        Ok(parsed) => parsed,
        Err(message) => {
            let _ = writeln!(err, "collide config get: {message}\n{USAGE}");
            return exit::USAGE;
        }
    };
    if let Err(message) = ensure_running_from_cli(paths) {
        let _ = writeln!(
            err,
            "collide config get: the daemon is not running and did not start: {message}"
        );
        return exit::FAILED;
    }
    let mut client = match OwnerClient::connect(paths, "collide config") {
        Ok(client) => client,
        Err(error) => {
            let _ = writeln!(err, "collide config get: {error}");
            return exit::FAILED;
        }
    };
    let board = match open_board(&mut client, cwd) {
        Ok(board) => board,
        Err(error) => {
            let _ = writeln!(err, "collide config get: {error}");
            return exit::FAILED;
        }
    };
    if board.is_none() {
        let _ = writeln!(
            err,
            "collide config get: {} is not in a git checkout, so this shows the user's layers only",
            cwd.display()
        );
    }
    let params = ConfigGetParams {
        board,
        key: parsed.key,
        profile: parsed.profile,
    };
    match client.call::<_, ConfigGetResult>("config.get", &params, None) {
        Ok(result) => {
            for entry in &result.entries {
                let _ = writeln!(out, "{}", entry_line(entry));
            }
            exit::OK
        }
        Err(ClientError::Rpc(error)) if error.code == code::NOT_FOUND => {
            let _ = writeln!(err, "collide config get: {}", error.message);
            exit::ID
        }
        Err(ClientError::Rpc(error)) if error.code == code::CONFLICT => {
            let _ = writeln!(err, "collide config get: the settings have problems:");
            for line in error.message.lines() {
                let _ = writeln!(err, "  {line}");
            }
            exit::FAILED
        }
        Err(error) => {
            let _ = writeln!(err, "collide config get: {error}");
            exit::FAILED
        }
    }
}

/// The id of the board for the checkout `cwd` is in, or `None` when it is in none.
fn open_board(client: &mut OwnerClient, cwd: &Path) -> Result<Option<String>, ClientError> {
    let params = BoardOpenParams {
        root: cwd.display().to_string(),
        init_git: false,
    };
    let opened: BoardOpenResult = client.call("board.open", &params, None)?;
    Ok(opened.board.map(|board| board.id))
}
