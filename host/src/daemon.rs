//! `collide-daemon`'s own parts (0.2; Stage 3a): the entry point, the socket and its security, and the adoption of
//! shims that outlived the last daemon. Starting the daemon is [`crate::launch`], its environment is
//! [`crate::base_env`], its storage is [`crate::store`], its clocks are [`crate::clock`] and its telemetry receiver
//! is [`crate::otlp`]. The binary's `main` is a placeholder until Stage 3a calls [`run`].
//!
//! ## What the daemon does at start, in this order
//!
//! 1. Raises its soft open-file limit ([`MIN_OPEN_FILES`]): launchd's default of 256 broke herdr at about 40 panes,
//!    and the daemon tails three files per child.
//! 2. Records its login session, and restarts in a new one. Ending at logout is recorded as host shutdown, not as a
//!    crash.
//! 3. Opens the store and **adopts existing shims** ([`adopt_shims`]) before it binds the socket, so no client sees a
//!    run as gone while its shim is alive.
//! 4. Checks the socket's path and directory ([`crate::auth::prepare_daemon_socket`]) and binds ([`bind_socket`]).
//!
//! ## The socket
//!
//! At a short fixed path, because an `AF_UNIX` path is limited to [`MAX_SOCKET_PATH_BYTES`] bytes on macOS (measured
//! S3), and a longer path fails fast with a message naming the path and the limit ([`SocketError::PathTooLong`]).
//!
//! - Its directory is mode `0700` and the daemon checks the owner and mode at start, refusing to bind in a directory
//!   that is not the user's or is open to others ([`crate::auth::check_socket_dir`]).
//! - A connecting peer's uid must equal the daemon's (`getpeereid`), so a second local user never reaches the token
//!   check ([`SocketError::ForeignUid`], [`crate::auth::admit`]).
//! - The CLI's token is in a file of mode `0600` in that directory, made at the daemon's first start
//!   ([`TOKEN_FILE_MODE`], [`crate::auth::create_token_file`]). It is never put in an environment variable.
//! - A shim's socket follows the same rules with no token, because only the daemon talks to it
//!   ([`crate::auth::prepare_shim_socket`]).
//!
//! The rules themselves, and the actor, are [`crate::auth`]'s: pure code over traits. This module binds, accepts and
//! serves, and calls them.
//!
//! ## Who is asking
//!
//! A peer's pid is read with `LOCAL_PEERPID`, and the daemon walks that process's parents looking for the markers:
//! `COLLIDE_RUN` makes a run's agent, the app's process makes the app, and anything else is the CLI from a terminal
//! ([`crate::auth::Caller`]). The result is the `actor` recorded on every event, and only the app does the person-only
//! operations ([`crate::auth::judge`]). This is a guardrail (0.1), not a boundary.
//!
//! | tag | function | what it does |
//! |---|---|---|
//! | `daemon: entry` | [`run`] | the start sequence above, then the serve loop |
//! | `daemon: socket server` | [`bind_socket`], [`serve`] | the bind, the handshake and the JSON-RPC loop, over [`crate::auth`]'s checks |
//! | `daemon: adopt shims` | [`adopt_shims`] | reattach by pid and start time, read `exit.json` for the ones that ended |

use crate::clock::Clocks;
use crate::store::Store;
use collide_core::shim::ExitRecord;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

/// The longest an `AF_UNIX` socket path may be on macOS, in bytes, without the terminating NUL (measured S3).
pub const MAX_SOCKET_PATH_BYTES: usize = 103;

/// The open-file soft limit the daemon raises itself to. The plan names launchd's 256 as too low and gives no target;
/// this is room for the three files of each of about a hundred children.
pub const MIN_OPEN_FILES: u64 = 4096;

/// The mode of the CLI's token file (0.2).
pub const TOKEN_FILE_MODE: u32 = 0o600;

/// The mode of the socket's directory (0.2).
pub const SOCKET_DIR_MODE: u32 = 0o700;

/// Where the daemon's files are.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DaemonPaths {
    /// The directory the socket and the token file are in, mode `0700`.
    pub socket_dir: PathBuf,
    pub socket: PathBuf,
    /// The CLI's token, mode `0600`.
    pub token_file: PathBuf,
    /// The lock file that serializes starts ([`crate::launch`]).
    pub lock_file: PathBuf,
    /// The directory that holds one run directory for each run, which the shims write into.
    pub runs_dir: PathBuf,
    /// The store's directory ([`Store::open`]).
    pub store_dir: PathBuf,
}

/// Why the daemon could not start or serve. Each message names the path or the limit.
#[derive(Debug)]
pub enum SocketError {
    /// The socket path is over [`MAX_SOCKET_PATH_BYTES`].
    PathTooLong {
        path: PathBuf,
        len: usize,
    },
    /// The directory is not the daemon's, or its mode lets others in (or is not `0700` at all).
    UnsafeDir {
        dir: PathBuf,
        mode: u32,
        owner_uid: u32,
        daemon_uid: u32,
    },
    /// Where the socket's directory should be there is a file, or a link.
    NotADirectory {
        path: PathBuf,
    },
    /// The token file's mode is not [`TOKEN_FILE_MODE`], so it is not read.
    UnsafeTokenFile {
        path: PathBuf,
        mode: u32,
    },
    /// A peer's uid is not the daemon's.
    ForeignUid {
        uid: u32,
        daemon_uid: u32,
    },
    Io {
        context: String,
        source: io::Error,
    },
}

impl fmt::Display for SocketError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SocketError::PathTooLong { path, len } => write!(
                f,
                "the socket path {} is {len} bytes, over the {MAX_SOCKET_PATH_BYTES}-byte limit of a macOS socket path",
                path.display()
            ),
            SocketError::UnsafeDir {
                dir,
                mode,
                owner_uid,
                daemon_uid,
            } => write!(
                f,
                "the socket directory {} is mode {mode:04o} and owned by uid {owner_uid}, and must be mode {SOCKET_DIR_MODE:04o} and owned by uid {daemon_uid}, the daemon's user",
                dir.display()
            ),
            SocketError::NotADirectory { path } => write!(
                f,
                "the socket directory {} is not a directory (a file or a link is there)",
                path.display()
            ),
            SocketError::UnsafeTokenFile { path, mode } => write!(
                f,
                "the token file {} is mode {mode:04o}, and must be a file of mode {TOKEN_FILE_MODE:04o}, so it was not read",
                path.display()
            ),
            SocketError::ForeignUid { uid, daemon_uid } => write!(
                f,
                "a connection from uid {uid} is refused: the daemon's user is uid {daemon_uid}"
            ),
            SocketError::Io { context, source } => write!(f, "{context}: {source}"),
        }
    }
}

impl std::error::Error for SocketError {}

/// An existing shim, as the daemon finds it at start: which run, which process, and either the exit record it left
/// or the fact that it is still alive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdoptedShim {
    pub run: String,
    pub pid: u32,
    pub state: AdoptedState,
}

/// What adoption found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdoptedState {
    /// The pid is alive and its start time matches the one recorded, so it is the same shim.
    Alive,
    /// The shim ended and left `exit.json`. The daemon takes the totals from it.
    Exited(ExitRecord),
    /// The shim is gone and left no `exit.json`: its phase is `lost` (0.2). When the child is still running, the
    /// daemon stops it as it marks the phase `lost`.
    Lost,
}

/// Runs the daemon: the start sequence in the module doc, then [`serve`] until it is told to stop.
pub fn run(paths: &DaemonPaths, clocks: &dyn Clocks) -> Result<(), SocketError> {
    let _ = (paths, clocks);
    todo!("daemon: entry")
}

/// Checks the socket's path and directory ([`crate::auth::prepare_daemon_socket`]), and binds it. The listener is the
/// std one's equivalent until Stage 3a picks the async runtime, which is why this returns the bound path and not a
/// type of a crate.
pub fn bind_socket(paths: &DaemonPaths) -> Result<PathBuf, SocketError> {
    let _ = paths;
    todo!("daemon: socket server")
}

/// Accepts connections on the bound socket: the version handshake, the per-client token, and the JSON-RPC loop. The
/// methods themselves are other items'.
pub fn serve(socket: &Path, store: &Store) -> Result<(), SocketError> {
    let _ = (socket, store);
    todo!("daemon: socket server")
}

/// Finds every shim of an earlier daemon under `runs_dir`: reattaches by pid plus start time, and reads `exit.json`
/// for the shims that ended while no daemon was running. It runs before the socket is bound.
pub fn adopt_shims(runs_dir: &Path) -> Result<Vec<AdoptedShim>, SocketError> {
    let _ = runs_dir;
    todo!("daemon: adopt shims")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_socket_path_error_names_the_path_and_the_limit() {
        let message = SocketError::PathTooLong {
            path: PathBuf::from("/very/long/path.sock"),
            len: 120,
        }
        .to_string();
        assert!(message.contains("/very/long/path.sock"), "{message}");
        assert!(message.contains("103"), "{message}");
    }

    #[test]
    fn an_unsafe_directory_error_names_the_directory_both_modes_and_both_owners() {
        let message = SocketError::UnsafeDir {
            dir: PathBuf::from("/tmp/collide"),
            mode: 0o755,
            owner_uid: 0,
            daemon_uid: 501,
        }
        .to_string();
        for part in ["/tmp/collide", "0755", "0700", "uid 0", "501"] {
            assert!(message.contains(part), "{part} in {message}");
        }
    }

    #[test]
    fn the_token_file_is_private_to_the_user() {
        assert_eq!(TOKEN_FILE_MODE, 0o600);
        assert_eq!(SOCKET_DIR_MODE, 0o700);
    }
}
