//! The ask's launch spec (ts-0200-gs; 0.3, The ask agent is a separate role; Claude Code's launch line). The ask is not
//! a run: it has no role or phase, so [`ask_launch_spec`] builds the line from an [`AskProfile`] and the adapter's
//! [`super::ClaudeAdapter`] and core's `LaunchRequest` stay as they are. No process starts here.
//!
//! ```text
//! claude -p --output-format stream-json --verbose
//!   --model <model>
//!   --setting-sources ""                      # `config: user` passes `user` instead
//!   --settings <ask dir>/settings.json
//!   --mcp-config <ask dir>/mcp.json --strict-mcp-config
//!   [--add-dir <folder>]...                   # one per readable folder after the first
//!   --permission-prompts none
//!   --restricted                              # ts-0198-tf: reads stay in the folders, and `--add-dir` takes effect
//!   --tools Read,Grep,Glob
//!   --allowedTools Read,Grep,Glob,mcp__collide__<each UI tool>
//!   --no-session-persistence
//!   --append-system-prompt <ASK_PROMPT>
//!   [--max-budget-usd <budget>]
//! ```
//!
//! The request goes on stdin, so it is never in argv. The ask token is in `mcp.json` (mode 0600, [`write_ask_folder`])
//! and nowhere else: not in argv, not in the environment. The program is not part of the spec, as for a run's line.

use super::launch::{MCP_FILE, SETTINGS_FILE, invalid, safe};
use super::{PROGRAM, env_strip};
use crate::atomic_write::{AtomicError, write_atomically};
use crate::file_run::find_on_path;
use collide_core::adapter::{LaunchError, LaunchSpec, refuse_unmet_capabilities};
use collide_core::agent::{Capabilities, Support};
use collide_core::profile::RoleConfig;
use serde_json::json;
use std::collections::BTreeMap;
use std::fmt;
use std::fs::{self, DirBuilder};
use std::io;
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::path::{Path, PathBuf};

/// The tools the ask can use on files, and the whole of `--tools`: Bash, Edit and Write are not there.
pub const READ_TOOLS: [&str; 3] = ["Read", "Grep", "Glob"];

/// The UI tools of Collide's own MCP server (0.2) that the ask may call, in the order of the pinned line.
pub const UI_TOOLS: [&str; 5] = ["query", "set_view", "show_groups", "highlight", "open"];

/// The name the ask's MCP server has in `mcp.json`, so a tool is `mcp__collide__<name>`.
pub const SERVER_NAME: &str = "collide";

/// The capability the ask role requires of its tool (0.3).
const REQUIRED_CAPABILITY: &str = "mcp_servers.http";

/// The words before `--model`, which never change.
const HEAD: [&str; 4] = ["-p", "--output-format", "stream-json", "--verbose"];

/// The words after the `--add-dir` ones, up to the tools: a request the line does not allow is denied and never asked,
/// and reads stay in the folders (`--restricted`, ts-0198-tf).
const CONFINEMENT: [&str; 3] = ["--permission-prompts", "none", "--restricted"];

/// The mode of the ask folder's files, and of the folder: only the daemon's user reads the token.
const FILE_MODE: u32 = 0o600;
const DIR_MODE: u32 = 0o700;

/// Collide's fixed ask prompt, from S6's prompt v2: a small model followed "when the request is about code" never, so
/// v2 says it must read the folder before it groups or ranks, say which tasks it left out, and never guess an id.
pub const ASK_PROMPT: &str = concat!(
    "You operate the screen of Collide, a task-oriented agent IDE, for the user, through the \"collide\" tools. The \
     screen only presents what you tell it to; it does not know why. You do the analysis.\n",
    "Use the collide query tool to read tasks and runs, and Read, Grep and Glob to read the code folder you are in \
     (read only).\n",
    "When the request is about code, files or what a change would touch, you must read the code folder (Glob its \
     layout, then Read or Grep the files a task would change) before you group or rank anything. Task text rarely \
     names files; work out where each change would land from the code. Only use tasks that are about the code in \
     this folder, and say which tasks you left out. Tasks that are ready are the ones that can still be launched.\n",
    "Then change the screen: set_view when the answer is a filter, sort or grouping of data the screen already has; \
     show_groups when the answer is your own judgement (named groups of ids, each with a reason); highlight to \
     point at ids; open to open a surface.\n",
    "You cannot launch, accept, edit or delete anything. Never ask the user a question: make the best change, then \
     say in one or two sentences what you did and what you were unsure about.\n",
    "Take ids from query, never guess them. Keep the number of calls small and finish by calling a screen-changing \
     tool.",
);

/// What an ask needs from its entry (0.7) and from the daemon. The caller has resolved the entry: `<board root>` in
/// the folders is a path, an absent `config` is `none` (the ask's own default, stricter than a run's), and the model is
/// the entry's, `haiku` by default.
#[derive(Debug, Clone, PartialEq)]
pub struct AskProfile {
    /// The model name, a safe token (0.5), so it cannot become a flag.
    pub model: String,
    /// The folders the ask may read, as absolute paths. The first is its working directory; each later one is passed
    /// with `--add-dir`. At least one.
    pub read: Vec<PathBuf>,
    /// `none` passes `--setting-sources ""` and `user` passes `user`: the only difference between the two lines.
    pub config: RoleConfig,
    /// The cap on this ask's spend, in dollars. `None` leaves `--max-budget-usd` off.
    pub budget_usd: Option<f64>,
    /// The ask's own folder, an absolute path: [`write_ask_folder`] makes it and `--settings` and `--mcp-config` name
    /// its files.
    pub dir: PathBuf,
}

/// The launch spec for an ask: [`ASK_PROMPT`] and the line above, the adapter's [`env_strip`] and no variable to add,
/// and the first readable folder as the working directory. A profile that cannot make a line is a
/// [`LaunchError::Invalid`] that says why: no folder, a folder or the ask folder that is not an absolute UTF-8 path, a
/// model that is not a safe token, and a budget that is not a positive amount.
pub fn ask_launch_spec(profile: &AskProfile) -> Result<LaunchSpec, LaunchError> {
    let Some((first, extra)) = profile.read.split_first() else {
        return invalid(
            "the ask has no readable folder, so it has no working directory (give the entry at least one in `read`)"
                .into(),
        );
    };
    let ask_file = |name: &str| absolute("the ask folder", &profile.dir.join(name));

    let mut argv: Vec<String> = HEAD.map(String::from).to_vec();
    argv.extend(["--model".into(), safe("the model name", &profile.model)?]);
    argv.extend([
        "--setting-sources".into(),
        setting_sources(profile.config).into(),
        "--settings".into(),
        ask_file(SETTINGS_FILE)?,
        "--mcp-config".into(),
        ask_file(MCP_FILE)?,
        "--strict-mcp-config".into(),
    ]);
    absolute("the first readable folder", first)?;
    for folder in extra {
        argv.extend(["--add-dir".into(), absolute("a readable folder", folder)?]);
    }
    argv.extend(CONFINEMENT.map(String::from));
    argv.extend([
        "--tools".into(),
        READ_TOOLS.join(","),
        "--allowedTools".into(),
        allowed_tools().join(","),
        "--no-session-persistence".into(),
        "--append-system-prompt".into(),
        ASK_PROMPT.into(),
    ]);
    if let Some(budget) = profile.budget_usd {
        if !budget.is_finite() || budget <= 0.0 {
            return invalid(format!(
                "the ask's budget ({budget}) is not a positive amount of dollars to pass as --max-budget-usd"
            ));
        }
        argv.extend(["--max-budget-usd".into(), budget.to_string()]);
    }
    Ok(LaunchSpec {
        argv,
        env: BTreeMap::new(),
        env_strip: env_strip(),
        cwd: first.clone(),
    })
}

/// The checks before an ask launches, in this order, each with its plain text. The ask role requires
/// `mcp_servers: http` (0.3): `agent` is the entry's id and `capabilities` its adapter's table, and a `No` or
/// `Unknown` is refused as "this agent cannot use HTTP MCP servers". Then `claude` is looked up on `path`, the `PATH`
/// of the base environment and no other, and its path is returned: a miss is "claude was not found on PATH".
pub fn check_before_launch(
    agent: &str,
    capabilities: &Capabilities,
    path: Option<&str>,
) -> Result<PathBuf, LaunchError> {
    let required = [REQUIRED_CAPABILITY.to_string()];
    if let Err(refusals) = refuse_unmet_capabilities(agent, capabilities, &required) {
        let said = match refusals[0].support {
            Support::No => "no",
            Support::Unknown | Support::Yes => "unknown",
        };
        return invalid(format!(
            "this agent cannot use HTTP MCP servers (`{agent}`: the adapter says {said})"
        ));
    }
    find_on_path(PROGRAM, path)
        .map_or_else(|| invalid(format!("{PROGRAM} was not found on PATH")), Ok)
}

/// Why the ask folder could not be written.
#[derive(Debug)]
pub enum AskFolderError {
    /// The endpoint or the token is empty: a file with no endpoint, or a header that sends no token, would only fail
    /// later and less plainly.
    Empty { what: &'static str },
    /// The folder could not be made, or closed to other users.
    Folder { path: PathBuf, source: io::Error },
    /// A file could not be written. [`AtomicError`] says which step and on which path.
    File(AtomicError),
}

impl fmt::Display for AskFolderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AskFolderError::Empty { what } => write!(f, "the ask's {what} is empty"),
            AskFolderError::Folder { path, source } => {
                write!(f, "making the ask folder {}: {source}", path.display())
            }
            AskFolderError::File(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for AskFolderError {}

/// Makes the ask folder (mode 0700) and writes its two files, each with `write_atomically` and mode 0600:
/// `settings.json` (`{"autoMemoryEnabled":false}`) and `mcp.json`, which names Collide's server by `endpoint` and sends
/// `token` as an `Authorization: Bearer` header. The token is in this one file. A failure part way leaves what was
/// written: the caller removes the folder with [`remove_ask_folder`].
pub fn write_ask_folder(dir: &Path, endpoint: &str, token: &str) -> Result<(), AskFolderError> {
    if endpoint.is_empty() {
        return Err(AskFolderError::Empty {
            what: "endpoint URL",
        });
    }
    if token.is_empty() {
        return Err(AskFolderError::Empty { what: "token" });
    }
    let folder = |source| AskFolderError::Folder {
        path: dir.to_path_buf(),
        source,
    };
    DirBuilder::new()
        .recursive(true)
        .mode(DIR_MODE)
        .create(dir)
        .and_then(|()| fs::set_permissions(dir, fs::Permissions::from_mode(DIR_MODE)))
        .map_err(folder)?;
    let settings = json!({ "autoMemoryEnabled": false });
    let mcp = json!({
        "mcpServers": {
            SERVER_NAME: {
                "type": "http",
                "url": endpoint,
                "headers": { "Authorization": format!("Bearer {token}") }
            }
        }
    });
    for (name, value) in [(SETTINGS_FILE, settings), (MCP_FILE, mcp)] {
        let bytes = serde_json::to_vec(&value).expect("a JSON value always serializes");
        write_atomically(dir, name, &bytes, FILE_MODE, |file| file)
            .map_err(AskFolderError::File)?;
    }
    Ok(())
}

/// Removes the ask folder at `dir` and what is in it, the token with it, and nothing else. `dir` must be an absolute
/// path that is a folder: a relative path, the root, a link and a file are each refused and nothing is removed. A
/// folder that is already gone is not an error, so a second call, or a cleanup after a failed write, is harmless.
pub fn remove_ask_folder(dir: &Path) -> io::Result<()> {
    let refuse = |why: &str| {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("not removing {}: {why}", dir.display()),
        ))
    };
    if !dir.is_absolute() || dir.file_name().is_none() {
        return refuse("it is not an absolute path to a folder");
    }
    match fs::symlink_metadata(dir) {
        Ok(meta) if meta.file_type().is_dir() => fs::remove_dir_all(dir),
        Ok(_) => refuse("it is not a folder (a link is not followed)"),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

/// Removes `mcp.json`, the one file of the ask folder that holds the token, and leaves the rest of the folder: the
/// ask's record lives there (0.3, The ask agent is a separate role). A file that is already gone is not an error. A
/// link is removed as a link and not followed.
pub fn remove_mcp_file(dir: &Path) -> io::Result<()> {
    match fs::remove_file(dir.join(MCP_FILE)) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}

/// `""` or `user` (0.3, Claude Code's launch line).
fn setting_sources(config: RoleConfig) -> &'static str {
    match config {
        RoleConfig::None => "",
        RoleConfig::User => "user",
    }
}

/// `--allowedTools`: the read tools, then each UI tool as Claude names an MCP tool.
fn allowed_tools() -> Vec<String> {
    READ_TOOLS
        .iter()
        .map(|tool| tool.to_string())
        .chain(
            UI_TOOLS
                .iter()
                .map(|tool| format!("mcp__{SERVER_NAME}__{tool}")),
        )
        .collect()
}

/// `path` as an argument: absolute, so it does not depend on the directory the child starts in and cannot read as a
/// flag, and UTF-8, since argv here is text.
fn absolute(what: &str, path: &Path) -> Result<String, LaunchError> {
    match path.to_str() {
        Some(text) if path.is_absolute() => Ok(text.to_string()),
        _ => invalid(format!(
            "{what}, `{}`, is not an absolute UTF-8 path",
            path.display()
        )),
    }
}
