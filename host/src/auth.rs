//! The daemon's connection rules (0.2, The socket and Who is asking; 0.10, the person-only operations; 0.1, the
//! same-user rule), as pure code over small traits, so a test uses fakes and touches no real socket. Two things are
//! decided here and nowhere else: **who is asking**, and **whether a socket may be used**. Binding, accepting and
//! the JSON-RPC loop are [`crate::daemon`]'s.
//!
//! ## Order of checks on a connection
//!
//! 1. **The peer's uid** ([`admit`]) must be the daemon's. A connection from another uid is refused here, before the
//!    peer's pid is read, before its process tree is walked and before any byte of its `initialize` (which carries
//!    the token) is read: [`Admitted::initialize`] is a method of a connection that passed, so a refused one has no
//!    way to reach the token check.
//! 2. **The actor** is read from the peer's process tree ([`classify_peer`]).
//! 3. **The token** is checked by `initialize` ([`collide_core::owner::check_initialize`]).
//! 4. **A person-only operation** is judged against the actor ([`judge`]).
//!
//! A shim's socket follows the same rules without 2 to 4, because only the daemon talks to it ([`admit_shim`],
//! [`prepare_shim_socket`]).
//!
//! ## Who is asking
//!
//! [`classify_peer`] walks the peer's process and its parents, at most [`MAX_TREE_DEPTH`] of them, and looks for the
//! markers: `COLLIDE_RUN` in a process's environment makes a run's agent, a process that is the app makes the app,
//! and anything else is the CLI from a terminal ([`Caller`]). When both markers are in one tree the run wins, so the
//! result is the one with the least power. A process that cannot be looked up ends the walk and the answer is the
//! CLI, which can only ask for a Queue confirmation, never do a person-only operation. A process that left its
//! parent (a daemonized child of a run) is not found by its tree; that is the same-user rule (0.1), and this is a
//! guardrail, not a boundary.
//!
//! ## Person-only operations
//!
//! Accept, defer, reject and resolve, of proposals and of questions, a status write to `accepted`, `deferred` or
//! `rejected` on a proposal, and the return of a deferred proposal to `proposed` ([`person_only_request`]).
//! [`judge`] allows the app, turns the CLI's into a [`QueueConfirmation`] that a person answers in the app, and
//! refuses a run agent with [`collide_core::owner::code::NOT_PERMITTED`].
//!
//! ## The socket's files
//!
//! The path is at most [`crate::daemon::MAX_SOCKET_PATH_BYTES`] bytes ([`check_socket_path`]); its directory is the
//! daemon's own and mode `0700` exactly ([`check_socket_dir`], made when missing); the CLI's token is in a file of
//! mode `0600` in that directory ([`create_token_file`], [`read_token_file`]) and is never put in an environment
//! variable (a scan in `host/tests/token_not_in_env.rs` keeps that so). File metadata is behind [`DirFs`], with
//! [`StdFs`] over `std` for the real one.
//!
//! ## The real readers
//!
//! [`UnixPeer`] and [`daemon_uid`] (`getpeereid`, `LOCAL_PEERPID`, `geteuid`) and [`SystemProcessTree`] (a process's
//! parent from libproc, its `COLLIDE_RUN` from `KERN_PROCARGS2`, and the app by its executable) are over
//! [`crate::sysproc`], which holds every call into the system. `host/tests/real_peer.rs` runs them over real sockets
//! and python3 children.

use crate::daemon::{
    DaemonPaths, MAX_SOCKET_PATH_BYTES, SOCKET_DIR_MODE, SocketError, TOKEN_FILE_MODE,
};
use crate::shim;
use crate::sysproc;
use collide_core::change::{Actor, ActorKind};
use collide_core::owner::{OwnerInitializeParams, RpcError, check_initialize, code};
use std::collections::HashSet;
use std::fs;
use std::io::{self, Read, Write};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};

/// The environment variable that marks a run's agent and everything it starts (0.2, Who is asking).
pub const RUN_MARKER: &str = "COLLIDE_RUN";

/// How many processes of a tree [`classify_peer`] looks at, the peer's own included. A real tree is a dozen deep;
/// the limit is for a table that loops or a walk that never ends.
pub const MAX_TREE_DEPTH: usize = 64;

/// The mode of the file the app writes its executable path to: the user's alone, as the token file is.
const APP_FILE_MODE: u32 = 0o600;

/// The bytes of randomness in a token ([`mint_token`]): 256 bits.
pub const TOKEN_BYTES: usize = 32;

/// The permission bits of a mode that matter to a check: the nine of owner, group and other, and setuid, setgid and
/// sticky.
const MODE_MASK: u32 = 0o7777;

// ---- who is asking ----------------------------------------------------------------------------------------------

/// Who is on the other end of a connection (0.2, Who is asking).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Caller {
    /// The app: its process is among the peer's parents.
    App,
    /// The CLI from a terminal: no marker was found.
    Cli,
    /// A run's agent: `COLLIDE_RUN` is in the peer's tree, and `run` is its value.
    RunAgent { run: String },
}

impl Caller {
    /// The `actor` the daemon records on every event this connection causes. The app and the CLI are a person (the
    /// CLI's person-only operations wait for a confirmation first); a run's agent is an agent of that run.
    pub fn actor(&self) -> Actor {
        match self {
            Caller::App | Caller::Cli => Actor {
                kind: ActorKind::Person,
                run: None,
                phase: None,
            },
            Caller::RunAgent { run } => Actor {
                kind: ActorKind::Agent,
                run: Some(run.clone()),
                phase: None,
            },
        }
    }
}

/// What the system says about a process. The real one ([`SystemProcessTree`]) reads it with `sysctl` and
/// `proc_pidinfo`; a test gives a table.
pub trait ProcessTree {
    /// The parent of `pid`, or `None` at the top of the tree or when `pid` cannot be looked up.
    fn parent(&self, pid: u32) -> Option<u32>;
    /// The value of `COLLIDE_RUN` in `pid`'s own environment. `None` when it is not there or macOS hides the
    /// environment, in which case the walk finds the marker through the parents.
    fn run_marker(&self, pid: u32) -> Option<String>;
    /// Whether `pid` is the app's process.
    fn is_app(&self, pid: u32) -> bool;
}

/// Reads who is asking from `peer_pid` by the walk of its parents (see the module doc).
pub fn classify_peer(tree: &dyn ProcessTree, peer_pid: u32) -> Caller {
    let mut seen = HashSet::new();
    let mut found_app = false;
    let mut next = Some(peer_pid);
    while let Some(pid) = next {
        if seen.len() >= MAX_TREE_DEPTH || !seen.insert(pid) {
            break;
        }
        if let Some(run) = tree.run_marker(pid) {
            return Caller::RunAgent { run };
        }
        found_app |= tree.is_app(pid);
        next = tree.parent(pid);
    }
    if found_app { Caller::App } else { Caller::Cli }
}

// ---- the peer and the checks on a connection --------------------------------------------------------------------

/// What the kernel says about the other end of a connected socket. The real one ([`UnixPeer`]) is `getpeereid` for the
/// uid and `LOCAL_PEERPID` for the pid.
pub trait PeerCredentials {
    fn uid(&self) -> io::Result<u32>;
    fn pid(&self) -> io::Result<u32>;
}

fn peer_uid_checked(daemon_uid: u32, peer: &dyn PeerCredentials) -> Result<(), SocketError> {
    let uid = peer.uid().map_err(|source| SocketError::Io {
        context: "could not read the uid of the connecting peer (getpeereid)".to_string(),
        source,
    })?;
    if uid == daemon_uid {
        Ok(())
    } else {
        Err(SocketError::ForeignUid { uid, daemon_uid })
    }
}

/// A connection that passed the uid check, with the actor read from its process tree. It is the only way to reach
/// the token check, which is how the uid check comes first.
#[derive(Debug)]
pub struct Admitted {
    caller: Caller,
}

/// Checks the peer's uid against `daemon_uid`, then classifies it. A foreign uid is [`SocketError::ForeignUid`] and
/// nothing else of the peer is read. A peer that closed before its pid was read (`ENOTCONN`, a probe) is
/// [`SocketError::PeerClosed`]: the uid was the daemon's, so nothing was refused.
pub fn admit(
    daemon_uid: u32,
    peer: &dyn PeerCredentials,
    tree: &dyn ProcessTree,
) -> Result<Admitted, SocketError> {
    peer_uid_checked(daemon_uid, peer)?;
    let pid = peer.pid().map_err(|source| match source.kind() {
        io::ErrorKind::NotConnected => SocketError::PeerClosed,
        _ => SocketError::Io {
            context: "could not read the pid of the connecting peer (LOCAL_PEERPID)".to_string(),
            source,
        },
    })?;
    Ok(Admitted {
        caller: classify_peer(tree, pid),
    })
}

/// A shim's peer: the same uid check, and nothing else. There is no token and no actor, because only the daemon
/// talks to a shim.
pub fn admit_shim(daemon_uid: u32, peer: &dyn PeerCredentials) -> Result<(), SocketError> {
    peer_uid_checked(daemon_uid, peer)
}

impl Admitted {
    pub fn caller(&self) -> &Caller {
        &self.caller
    }

    /// Reads the connection's `initialize` with `read` and checks its token against `expected_token`, then its
    /// version. `read` is called here and not before, so a connection refused by [`admit`] never has its token read.
    pub fn initialize(
        self,
        read: impl FnOnce() -> OwnerInitializeParams,
        expected_token: &str,
    ) -> Result<Session, RpcError> {
        let params = read();
        check_initialize(&params, expected_token)?;
        Ok(Session {
            caller: self.caller,
            defaults: params.defaults,
        })
    }
}

/// A connection that has initialized: its caller is fixed for the connection's life.
#[derive(Debug)]
pub struct Session {
    caller: Caller,
    defaults: bool,
}

impl Session {
    pub fn caller(&self) -> &Caller {
        &self.caller
    }

    /// The connection asked for Start with defaults at `initialize`: it reads only the Default layer, the built-in
    /// packs and views, and its writes are refused. Fixed for the connection's life, as the caller is.
    pub fn defaults(&self) -> bool {
        self.defaults
    }

    /// The `actor` for the events this connection causes, which no request can change (0.10).
    pub fn actor(&self) -> Actor {
        self.caller.actor()
    }

    pub fn judge(&self, request: &PersonOnlyRequest) -> Verdict {
        judge(&self.caller, request)
    }
}

// ---- person-only operations -------------------------------------------------------------------------------------

/// The operations only a person does (0.10).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PersonOnlyOp {
    Accept,
    Defer,
    Reject,
    Resolve,
    /// Returning a deferred proposal to `proposed`: deferred holds "until a person returns it" (0.10).
    Reopen,
}

impl PersonOnlyOp {
    pub fn name(self) -> &'static str {
        match self {
            PersonOnlyOp::Accept => "accept",
            PersonOnlyOp::Defer => "defer",
            PersonOnlyOp::Reject => "reject",
            PersonOnlyOp::Resolve => "resolve",
            PersonOnlyOp::Reopen => "reopen",
        }
    }
}

/// The kinds of item a person-only operation is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PersonOnlyItem {
    Proposal,
    Question,
}

impl PersonOnlyItem {
    pub fn name(self) -> &'static str {
        match self {
            PersonOnlyItem::Proposal => "proposal",
            PersonOnlyItem::Question => "question",
        }
    }
}

/// One person-only operation asked of one item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersonOnlyRequest {
    pub op: PersonOnlyOp,
    pub item_kind: PersonOnlyItem,
    pub item: String,
}

impl PersonOnlyRequest {
    fn describe(&self) -> String {
        format!("{} {} {}", self.op.name(), self.item_kind.name(), self.item)
    }
}

/// The Queue confirmation a CLI's person-only operation becomes: the request, and a title in Collide's own words (no
/// task title in it, 0.9). The app lists it, and a person's answer is the one that does the operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueueConfirmation {
    pub request: PersonOnlyRequest,
    pub title: String,
}

/// What [`judge`] decides.
#[derive(Debug, Clone, PartialEq)]
pub enum Verdict {
    /// Do it: the app is a person.
    Allow,
    /// Do not do it now: ask a person in the app, through a Queue item.
    Confirm(QueueConfirmation),
    /// Refuse: a run's agent may not. The error is the wire's [`code::NOT_PERMITTED`].
    Refuse(RpcError),
}

/// Reads whether `op` on an item of `item_kind` is person-only. `from_status` is the item's status now and
/// `to_status` the status a status write asks for. `None` for everything else, which any actor may do.
pub fn person_only_request(
    op: &str,
    item_kind: &str,
    item: &str,
    from_status: Option<&str>,
    to_status: Option<&str>,
) -> Option<PersonOnlyRequest> {
    let item_kind = match item_kind {
        "proposal" => PersonOnlyItem::Proposal,
        "question" => PersonOnlyItem::Question,
        _ => return None,
    };
    let op = match (op, item_kind, from_status, to_status) {
        ("accept", ..) => PersonOnlyOp::Accept,
        ("defer", ..) => PersonOnlyOp::Defer,
        ("reject", ..) => PersonOnlyOp::Reject,
        ("resolve", ..) => PersonOnlyOp::Resolve,
        ("setStatus", PersonOnlyItem::Proposal, _, Some("accepted")) => PersonOnlyOp::Accept,
        ("setStatus", PersonOnlyItem::Proposal, _, Some("deferred")) => PersonOnlyOp::Defer,
        ("setStatus", PersonOnlyItem::Proposal, _, Some("rejected")) => PersonOnlyOp::Reject,
        ("setStatus", PersonOnlyItem::Proposal, Some("deferred"), Some("proposed")) => {
            PersonOnlyOp::Reopen
        }
        _ => return None,
    };
    Some(PersonOnlyRequest {
        op,
        item_kind,
        item: item.to_string(),
    })
}

/// Allows the app, turns the CLI's request into a Queue confirmation, and refuses a run's agent (0.10).
pub fn judge(caller: &Caller, request: &PersonOnlyRequest) -> Verdict {
    match caller {
        Caller::App => Verdict::Allow,
        Caller::Cli => Verdict::Confirm(QueueConfirmation {
            request: request.clone(),
            title: format!("Confirm: {}", request.describe()),
        }),
        Caller::RunAgent { run } => Verdict::Refuse(
            RpcError::new(
                code::NOT_PERMITTED,
                format!(
                    "run {run} may not {}: only a person does that, in the app",
                    request.describe()
                ),
            )
            .with_data(serde_json::json!({
                "run": run,
                "op": request.op.name(),
                "item": request.item,
            })),
        ),
    }
}

// ---- the socket's path and directory ----------------------------------------------------------------------------

/// What a check needs to know of a directory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DirStat {
    pub is_dir: bool,
    pub uid: u32,
    /// The permission bits, as `st_mode` has them below the file type.
    pub mode: u32,
}

/// The file system as the socket checks see it. A test gives a table; [`StdFs`] is the real one.
pub trait DirFs {
    /// The stat of `path` itself (a symlink is not followed), or `None` when nothing is there.
    fn stat(&self, path: &Path) -> io::Result<Option<DirStat>>;
    /// Makes `path` and any missing parent with mode `0700`.
    fn create_private_dir(&self, path: &Path) -> io::Result<()>;
}

/// [`DirFs`] over `std`.
pub struct StdFs;

impl DirFs for StdFs {
    fn stat(&self, path: &Path) -> io::Result<Option<DirStat>> {
        match fs::symlink_metadata(path) {
            Ok(meta) => Ok(Some(DirStat {
                is_dir: meta.is_dir(),
                uid: meta.uid(),
                mode: meta.mode() & MODE_MASK,
            })),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }

    fn create_private_dir(&self, path: &Path) -> io::Result<()> {
        fs::DirBuilder::new()
            .recursive(true)
            .mode(SOCKET_DIR_MODE)
            .create(path)
    }
}

/// Refuses a socket path over [`MAX_SOCKET_PATH_BYTES`] bytes, naming the path and the limit.
pub fn check_socket_path(socket: &Path) -> Result<(), SocketError> {
    let len = socket.as_os_str().as_bytes().len();
    if len > MAX_SOCKET_PATH_BYTES {
        return Err(SocketError::PathTooLong {
            path: socket.to_path_buf(),
            len,
        });
    }
    Ok(())
}

fn stat_io(path: &Path, source: io::Error) -> SocketError {
    SocketError::Io {
        context: format!("could not read the owner and mode of {}", path.display()),
        source,
    }
}

/// Refuses a socket directory that is not the daemon's own or whose mode is not exactly [`SOCKET_DIR_MODE`], and
/// makes the directory when it is not there. A directory made with a looser mode (a umask cannot loosen `0700`, but a
/// file system can) is refused after it is made, with its real mode in the message.
pub fn check_socket_dir(fs: &dyn DirFs, daemon_uid: u32, dir: &Path) -> Result<(), SocketError> {
    let mut stat = fs.stat(dir).map_err(|e| stat_io(dir, e))?;
    if stat.is_none() {
        fs.create_private_dir(dir)
            .map_err(|source| SocketError::Io {
                context: format!("could not make the socket directory {}", dir.display()),
                source,
            })?;
        stat = fs.stat(dir).map_err(|e| stat_io(dir, e))?;
    }
    let stat = stat.ok_or_else(|| SocketError::Io {
        context: format!(
            "the socket directory {} is not there after it was made",
            dir.display()
        ),
        source: io::Error::from(io::ErrorKind::NotFound),
    })?;
    if !stat.is_dir {
        return Err(SocketError::NotADirectory {
            path: dir.to_path_buf(),
        });
    }
    if stat.uid != daemon_uid || stat.mode != SOCKET_DIR_MODE {
        return Err(SocketError::UnsafeDir {
            dir: dir.to_path_buf(),
            mode: stat.mode,
            owner_uid: stat.uid,
            daemon_uid,
        });
    }
    Ok(())
}

/// The daemon's socket: the path's length first (it needs no file), then the directory.
pub fn prepare_daemon_socket(
    fs: &dyn DirFs,
    daemon_uid: u32,
    paths: &DaemonPaths,
) -> Result<(), SocketError> {
    check_socket_path(&paths.socket)?;
    check_socket_dir(fs, daemon_uid, &paths.socket_dir)
}

/// A shim's socket, under the same rules and without a token: the path inside the run directory's `shim` directory,
/// its length, and that directory. Returns the socket's path.
pub fn prepare_shim_socket(
    fs: &dyn DirFs,
    daemon_uid: u32,
    run_dir: &Path,
) -> Result<PathBuf, SocketError> {
    let dir = run_dir.join(shim::files::SOCKET_DIR);
    let socket = dir.join(shim::files::SOCKET);
    check_socket_path(&socket)?;
    check_socket_dir(fs, daemon_uid, &dir)?;
    Ok(socket)
}

// ---- the token file ---------------------------------------------------------------------------------------------

fn token_io(path: &Path, what: &str, source: io::Error) -> SocketError {
    SocketError::Io {
        context: format!("could not {what} the token file {}", path.display()),
        source,
    }
}

/// Makes the token file at `path` with mode `0600` and writes `token` to it. It is made once, at the daemon's first
/// start, and an existing file is an error and is never replaced. The mode is set when the file is made, so no
/// reader can see it open, and set again to be exact whatever the umask. The token goes nowhere else.
pub fn create_token_file(path: &Path, token: &str) -> Result<(), SocketError> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(TOKEN_FILE_MODE)
        .open(path)
        .map_err(|e| token_io(path, "create", e))?;
    file.set_permissions(fs::Permissions::from_mode(TOKEN_FILE_MODE))
        .map_err(|e| token_io(path, "set the mode of", e))?;
    file.write_all(token.as_bytes())
        .map_err(|e| token_io(path, "write", e))
}

/// Reads the token file, after checking that its mode is [`TOKEN_FILE_MODE`]: a file others can read is refused
/// unread, naming the path and the mode.
pub fn read_token_file(path: &Path) -> Result<String, SocketError> {
    let meta = fs::symlink_metadata(path).map_err(|e| token_io(path, "read", e))?;
    let mode = meta.mode() & MODE_MASK;
    if !meta.is_file() || mode != TOKEN_FILE_MODE {
        return Err(SocketError::UnsafeTokenFile {
            path: path.to_path_buf(),
            mode,
        });
    }
    fs::read_to_string(path).map_err(|e| token_io(path, "read", e))
}

/// A new token: [`TOKEN_BYTES`] bytes from the system's random source, as hex.
pub fn mint_token() -> Result<String, SocketError> {
    let mut bytes = [0u8; TOKEN_BYTES];
    let source = Path::new("/dev/urandom");
    fs::File::open(source)
        .and_then(|mut f| f.read_exact(&mut bytes))
        .map_err(|e| SocketError::Io {
            context: format!("could not read random bytes from {}", source.display()),
            source: e,
        })?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

// ---- the real peer and process lookups --------------------------------------------------------------------------

/// The real [`PeerCredentials`] of a connected socket: the kernel's answer for the other end, as it was when the peer
/// connected ([`crate::sysproc`]).
pub struct UnixPeer<'a> {
    pub stream: &'a UnixStream,
}

impl PeerCredentials for UnixPeer<'_> {
    fn uid(&self) -> io::Result<u32> {
        sysproc::peer_uid(self.stream)
    }

    fn pid(&self) -> io::Result<u32> {
        sysproc::peer_pid(self.stream)
    }
}

/// The daemon's own uid, for [`admit`] and [`check_socket_dir`]. Off macOS it is [`sysproc::NO_UID`], which no peer
/// has, so a connection is refused there and not let in.
pub fn daemon_uid() -> u32 {
    sysproc::daemon_uid()
}

/// The real [`ProcessTree`], from the system. The app is known by its executable: the daemon is told the path of
/// the app's binary, and a process is the app when the system says it runs that file. A process cannot take the
/// app's path without writing to the app's files, which is stronger than a marker in an environment that any child
/// can copy; it is still a guardrail, not a boundary (0.1). The path comes from the daemon's options, or from the file
/// the app writes ([`write_app_executable`]).
pub struct SystemProcessTree {
    app_executable: Option<PathBuf>,
    app_file: Option<DaemonPaths>,
}

impl SystemProcessTree {
    /// `app_executable` is the path of the app's binary, or `None` when the daemon does not know it (then no process
    /// is the app, and a person-only operation waits for a Queue confirmation). The path's links are resolved, as
    /// the system's answer for a process is.
    pub fn new(app_executable: Option<PathBuf>) -> SystemProcessTree {
        SystemProcessTree {
            app_executable: app_executable.map(|path| fs::canonicalize(&path).unwrap_or(path)),
            app_file: None,
        }
    }

    /// Also takes the app to be the executable that the app wrote to its file in `paths`' run folder
    /// ([`write_app_executable`]), when no path was given to [`SystemProcessTree::new`].
    pub fn with_app_file(mut self, paths: &DaemonPaths) -> SystemProcessTree {
        self.app_file = Some(paths.clone());
        self
    }
}

/// Writes the app's executable path to its file in the run folder, for the daemon to read when it checks a peer. The
/// folder is made `0700` when it is missing and refused when it is not the user's own or not `0700`, as for the socket
/// ([`check_socket_dir`]), so only the user can put a path there. The file is written beside itself and renamed over,
/// so
/// a daemon never reads half a path, and it is mode `0600`.
pub fn write_app_executable(paths: &DaemonPaths, executable: &Path) -> Result<(), SocketError> {
    check_socket_dir(&StdFs, daemon_uid(), &paths.socket_dir)?;
    let file = paths.app_executable_file();
    let temporary = file.with_extension("tmp");
    let io_error = |what: &str, path: &Path| {
        let context = format!("could not {what} {}", path.display());
        move |source| SocketError::Io { context, source }
    };
    let mut open = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(APP_FILE_MODE)
        .open(&temporary)
        .map_err(io_error("write", &temporary))?;
    open.set_permissions(fs::Permissions::from_mode(APP_FILE_MODE))
        .and_then(|()| open.write_all(executable.as_os_str().as_bytes()))
        .map_err(io_error("write", &temporary))?;
    fs::rename(&temporary, &file).map_err(io_error("replace", &file))
}

/// Reads the app's executable path from its file. The file is trusted only when it is a plain file (not a link) of
/// mode `0600` that `daemon_uid` owns, and holds a path; anything else names no app, which is the safe side (a
/// person-only operation then waits for a Queue confirmation).
pub fn read_app_executable(paths: &DaemonPaths, daemon_uid: u32) -> Option<PathBuf> {
    let file = paths.app_executable_file();
    let meta = fs::symlink_metadata(&file).ok()?;
    if !meta.is_file() || meta.mode() & MODE_MASK != APP_FILE_MODE || meta.uid() != daemon_uid {
        return None;
    }
    let bytes = fs::read(&file).ok()?;
    (!bytes.is_empty()).then(|| PathBuf::from(std::ffi::OsStr::from_bytes(&bytes)))
}

impl ProcessTree for SystemProcessTree {
    fn parent(&self, pid: u32) -> Option<u32> {
        sysproc::parent_pid(pid)
    }

    fn run_marker(&self, pid: u32) -> Option<String> {
        sysproc::env_var_of(pid, RUN_MARKER)
    }

    fn is_app(&self, pid: u32) -> bool {
        let app = self.app_executable.clone().or_else(|| {
            // Read at each check, so a daemon the CLI started first knows an app that came later.
            let paths = self.app_file.as_ref()?;
            let path = read_app_executable(paths, daemon_uid())?;
            Some(fs::canonicalize(&path).unwrap_or(path))
        });
        match app {
            Some(app) => sysproc::executable_of(pid).is_some_and(|exe| exe == app),
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_run_marker_is_the_variable_the_plan_names() {
        assert_eq!(RUN_MARKER, "COLLIDE_RUN");
    }
}
