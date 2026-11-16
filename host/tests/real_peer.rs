//! The daemon's connection rules over real sockets and real processes (ts-0052-xt): the peer's pid and uid from the
//! connection, the daemon's own uid, and the process tree read from the system. The children are python3 (a process
//! that is not a platform binary, so its environment is readable) and `/bin/bash` (a platform binary, whose environment
//! macOS hides, so it is classified through its parents, 0.2). The fake-tree table of ts-0033-zj stays in
//! `auth.rs`, and so does the refusal of a foreign uid over a fake peer: a connection from another uid cannot be made
//! without root.
//!
//! Each test makes its socket in a short folder of its own under `/tmp` (a socket path is limited to 103 bytes, and
//! `target/tmp` is longer than that), and removes exactly what it made.

mod common;

use collide_core::owner::{OwnerInitializeParams, code};
use collide_host::auth::{
    self, Caller, PeerCredentials, PersonOnlyItem, PersonOnlyOp, PersonOnlyRequest, ProcessTree,
    RUN_MARKER, SystemProcessTree, UnixPeer, Verdict,
};
use collide_host::daemon::{DaemonPaths, SOCKET_DIR_MODE, SocketError};
use std::cell::Cell;
use std::fs;
use std::io;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

const PYTHON: &str = "/usr/bin/python3";
/// The platform shell the tests use as a platform binary in the tree, and as the app's stand-in. It is `/bin/bash`
/// and not `/bin/sh`: `/bin/sh` starts as one executable and the system reports it as `/bin/bash` a moment later.
const SH: &str = "/bin/bash";

/// Connects to the socket in `argv[1]`, says one byte, then waits for the daemon's side to close.
const CLIENT_PY: &str = "import socket, sys\n\
s = socket.socket(socket.AF_UNIX)\n\
s.connect(sys.argv[1])\n\
s.sendall(b'x')\n\
s.recv(1)\n";

/// A python3 that has `COLLIDE_RUN` in its own environment and starts `/bin/bash`, without the marker, to start the
/// client: the marker is in the tree only above a platform binary.
const MARKED_PARENT_PY: &str = "import os, subprocess, sys\n\
env = {k: v for k, v in os.environ.items() if k != 'COLLIDE_RUN'}\n\
cmd = '/usr/bin/python3 \"$0\" \"$1\"; rc=$?; exit $rc'\n\
sys.exit(subprocess.call(['/bin/bash', '-c', cmd, sys.argv[1], sys.argv[2]], env=env))\n";

/// The shell line that starts the client under `/bin/bash` and keeps the shell alive until it ends.
const SH_LINE: &str = "/usr/bin/python3 \"$0\" \"$1\"; rc=$?; exit $rc";

static COUNTER: AtomicU32 = AtomicU32::new(0);

// ---- a short folder, a socket and children ----------------------------------------------------------------------

/// A folder `/tmp/clp-<pid>-<n>` made for one test. `Drop` removes the files the test made in it, by name, and then
/// the folder itself (never a recursive delete).
struct Short {
    dir: PathBuf,
    files: Vec<PathBuf>,
}

impl Short {
    fn new() -> Short {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = PathBuf::from(format!("/tmp/clp-{}-{n}", std::process::id()));
        fs::create_dir(&dir).unwrap();
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).unwrap();
        Short {
            dir,
            files: Vec::new(),
        }
    }

    fn path(&mut self, name: &str) -> PathBuf {
        let path = self.dir.join(name);
        self.files.push(path.clone());
        path
    }

    fn file(&mut self, name: &str, text: &str) -> PathBuf {
        let path = self.path(name);
        fs::write(&path, text).unwrap();
        path
    }
}

impl Drop for Short {
    fn drop(&mut self) {
        for file in &self.files {
            if fs::remove_file(file).is_err() {
                let _ = fs::remove_dir(file);
            }
        }
        let _ = fs::remove_dir(&self.dir);
    }
}

/// A child this test started. It is killed and reaped when the guard goes, so a failing test leaves nothing behind.
struct Spawned(Child);

impl Spawned {
    fn pid(&self) -> u32 {
        self.0.id()
    }

    /// Waits for the child to end on its own (its connection was closed), then reaps it.
    fn finish(mut self) {
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            if self.0.try_wait().unwrap().is_some() {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("the child did not end after its connection was closed");
    }
}

impl Drop for Spawned {
    fn drop(&mut self) {
        if self.0.try_wait().unwrap().is_none() {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}

fn accept(listener: &UnixListener) -> UnixStream {
    listener.set_nonblocking(true).unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                stream.set_nonblocking(false).unwrap();
                return stream;
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock && Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(e) => panic!("no connection from the child: {e}"),
        }
    }
}

/// A command with the marker removed, so a test run inside a run of Collide starts from a clean environment.
fn clean(program: &str) -> Command {
    let mut command = Command::new(program);
    command.env_remove(RUN_MARKER).stdin(Stdio::null());
    command
}

/// Stops the walk at this test process and does not read it: what is at or above it (cargo, a shell, a phase of a
/// Collide run, whose `COLLIDE_RUN` is in the environment this process was started with and cannot be changed) is not
/// the test's. A child's own marker is still read.
struct CutAtThisProcess<'a> {
    inner: &'a SystemProcessTree,
}

impl ProcessTree for CutAtThisProcess<'_> {
    fn parent(&self, pid: u32) -> Option<u32> {
        if pid == std::process::id() {
            None
        } else {
            self.inner.parent(pid)
        }
    }

    fn run_marker(&self, pid: u32) -> Option<String> {
        if pid == std::process::id() {
            None
        } else {
            self.inner.run_marker(pid)
        }
    }

    fn is_app(&self, pid: u32) -> bool {
        self.inner.is_app(pid)
    }
}

/// Everything a connection test holds: the folder (for its files), the listener and the stream the child connected
/// with. Dropping the stream lets the child end.
struct Connection {
    _short: Short,
    stream: UnixStream,
    child: Spawned,
}

impl Connection {
    fn finish(self) {
        drop(self.stream);
        self.child.finish();
    }
}

/// Makes a socket and starts the child `start(socket, client_script, short)` makes, and accepts its connection.
fn connect(start: impl FnOnce(&Path, &Path, &mut Short) -> Command) -> Connection {
    let mut short = Short::new();
    let socket = short.path("s");
    let client = short.file("client.py", CLIENT_PY);
    let listener = UnixListener::bind(&socket).unwrap();
    let child = Spawned(start(&socket, &client, &mut short).spawn().unwrap());
    let stream = accept(&listener);
    drop(listener);
    Connection {
        _short: short,
        stream,
        child,
    }
}

fn python_client(socket: &Path, client: &Path) -> Command {
    let mut command = clean(PYTHON);
    command.arg(client).arg(socket);
    command
}

fn sh_then_python_client(socket: &Path, client: &Path) -> Command {
    let mut command = clean(SH);
    command.args(["-c", SH_LINE]).arg(client).arg(socket);
    command
}

fn caller_of(connection: &Connection, tree: &SystemProcessTree) -> Caller {
    let peer = UnixPeer {
        stream: &connection.stream,
    };
    let cut = CutAtThisProcess { inner: tree };
    auth::admit(auth::daemon_uid(), &peer, &cut)
        .unwrap()
        .caller()
        .clone()
}

fn no_app() -> SystemProcessTree {
    SystemProcessTree::new(None)
}

// ---- the connection's own credentials ---------------------------------------------------------------------------

#[test]
fn the_peers_pid_is_the_connecting_childs_and_its_uid_is_this_users() {
    let connection = connect(|s, c, _| python_client(s, c));
    let peer = UnixPeer {
        stream: &connection.stream,
    };
    assert_eq!(peer.pid().unwrap(), connection.child.pid());
    assert_eq!(peer.uid().unwrap(), auth::daemon_uid());
    connection.finish();
}

#[test]
fn the_daemons_uid_is_the_owner_of_a_file_this_process_makes() {
    let mut short = Short::new();
    let file = short.file("owned", "x");
    assert_eq!(auth::daemon_uid(), fs::metadata(&file).unwrap().uid());
}

// ---- who is asking, over real processes --------------------------------------------------------------------------

#[test]
fn a_python_child_with_the_run_marker_is_a_run_agent() {
    let connection = connect(|s, c, _| {
        let mut command = python_client(s, c);
        command.env(RUN_MARKER, "run-0007-ab");
        command
    });
    assert_eq!(
        caller_of(&connection, &no_app()),
        Caller::RunAgent {
            run: "run-0007-ab".to_string()
        }
    );
    connection.finish();
}

#[test]
fn a_person_only_operation_from_a_run_agent_is_refused() {
    let connection = connect(|s, c, _| {
        let mut command = python_client(s, c);
        command.env(RUN_MARKER, "run-0007-ab");
        command
    });
    let peer = UnixPeer {
        stream: &connection.stream,
    };
    let tree = no_app();
    let admitted = auth::admit(
        auth::daemon_uid(),
        &peer,
        &CutAtThisProcess { inner: &tree },
    )
    .unwrap();
    let session = admitted
        .initialize(
            || OwnerInitializeParams {
                protocol: collide_core::owner::OWNER_PROTOCOL,
                token: Some("secret".to_string()),
                client: None,
            },
            "secret",
        )
        .unwrap();
    let request = PersonOnlyRequest {
        op: PersonOnlyOp::Accept,
        item_kind: PersonOnlyItem::Proposal,
        item: "pr-0001-xx".to_string(),
    };
    match session.judge(&request) {
        Verdict::Refuse(error) => assert_eq!(error.code, code::NOT_PERMITTED),
        other => panic!("a run's agent was not refused: {other:?}"),
    }
    connection.finish();
}

#[test]
fn a_python_child_with_no_marker_in_its_tree_is_the_cli() {
    let connection = connect(|s, c, _| python_client(s, c));
    assert_eq!(caller_of(&connection, &no_app()), Caller::Cli);
    connection.finish();
}

#[test]
fn a_person_only_operation_from_the_cli_becomes_a_queue_confirmation() {
    let connection = connect(|s, c, _| python_client(s, c));
    let caller = caller_of(&connection, &no_app());
    let request = PersonOnlyRequest {
        op: PersonOnlyOp::Reject,
        item_kind: PersonOnlyItem::Proposal,
        item: "pr-0001-xx".to_string(),
    };
    assert!(matches!(
        auth::judge(&caller, &request),
        Verdict::Confirm(_)
    ));
    connection.finish();
}

#[test]
fn a_process_whose_tree_holds_the_apps_process_is_the_app() {
    // `/bin/bash` stands in for the app's process: the daemon is told its executable, and the client is its child.
    let connection = connect(|s, c, _| sh_then_python_client(s, c));
    let app = SystemProcessTree::new(Some(PathBuf::from(SH)));
    assert_eq!(caller_of(&connection, &app), Caller::App);
    connection.finish();
}

#[test]
fn the_same_tree_is_the_cli_when_the_daemon_knows_no_app() {
    let connection = connect(|s, c, _| sh_then_python_client(s, c));
    assert_eq!(caller_of(&connection, &no_app()), Caller::Cli);
    connection.finish();
}

#[test]
fn a_process_that_is_not_the_apps_executable_is_not_the_app() {
    let connection = connect(|s, c, _| sh_then_python_client(s, c));
    let other = SystemProcessTree::new(Some(PathBuf::from("/usr/bin/true")));
    assert_eq!(caller_of(&connection, &other), Caller::Cli);
    connection.finish();
}

// ---- a platform binary in the tree ------------------------------------------------------------------------------

#[test]
fn macos_hides_the_environment_of_a_platform_binary() {
    // The premise of the next test, checked here so a macOS that stops hiding it is noticed: a `/bin/bash` that has
    // the marker in its environment shows none of it.
    let mut shell = Spawned(
        clean(SH)
            .env(RUN_MARKER, "run-0007-ab")
            .args(["-c", "read line"])
            .stdin(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let pid = shell.pid();
    assert_eq!(SystemProcessTree::new(None).run_marker(pid), None);
    drop(shell.0.stdin.take());
    shell.finish();
}

#[test]
fn a_marker_above_a_platform_binary_is_found_through_the_parents() {
    // client (no marker) <- /bin/bash (hidden, and no marker) <- python3 with COLLIDE_RUN <- this test.
    let connection = connect(|s, c, short| {
        let parent = short.file("parent.py", MARKED_PARENT_PY);
        let mut command = clean(PYTHON);
        command
            .env(RUN_MARKER, "run-0009-cd")
            .arg(parent)
            .arg(c)
            .arg(s);
        command
    });
    let tree = no_app();
    let peer = UnixPeer {
        stream: &connection.stream,
    };
    let shell = tree.parent(peer.pid().unwrap()).unwrap();
    assert_eq!(tree.run_marker(peer.pid().unwrap()), None, "the client");
    assert_eq!(tree.run_marker(shell), None, "the platform binary");
    assert_eq!(
        caller_of(&connection, &tree),
        Caller::RunAgent {
            run: "run-0009-cd".to_string()
        }
    );
    connection.finish();
}

#[test]
fn a_run_marker_wins_over_the_app_below_it() {
    // client <- /bin/bash (the app's stand-in) <- python3 with COLLIDE_RUN: the least power wins.
    let connection = connect(|s, c, short| {
        let parent = short.file("parent.py", MARKED_PARENT_PY);
        let mut command = clean(PYTHON);
        command
            .env(RUN_MARKER, "run-0009-cd")
            .arg(parent)
            .arg(c)
            .arg(s);
        command
    });
    let app = SystemProcessTree::new(Some(PathBuf::from(SH)));
    assert_eq!(
        caller_of(&connection, &app),
        Caller::RunAgent {
            run: "run-0009-cd".to_string()
        }
    );
    connection.finish();
}

// ---- the readers on their own ----------------------------------------------------------------------------------

#[test]
fn the_parent_of_this_process_is_the_one_the_system_names() {
    let tree = no_app();
    assert_eq!(
        tree.parent(std::process::id()),
        Some(std::os::unix::process::parent_id())
    );
}

#[test]
fn a_child_started_here_has_this_process_as_its_parent() {
    let mut shell = Spawned(
        clean(SH)
            .args(["-c", "read line"])
            .stdin(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    assert_eq!(no_app().parent(shell.pid()), Some(std::process::id()));
    drop(shell.0.stdin.take());
    shell.finish();
}

#[test]
fn the_top_of_the_tree_has_no_parent() {
    assert_eq!(no_app().parent(1), None);
}

#[test]
fn a_pid_that_cannot_be_looked_up_has_no_parent_marker_or_app() {
    let tree = SystemProcessTree::new(Some(PathBuf::from(SH)));
    for pid in [u32::MAX, i32::MAX as u32] {
        assert_eq!(tree.parent(pid), None, "pid {pid}");
        assert_eq!(tree.run_marker(pid), None, "pid {pid}");
        assert!(!tree.is_app(pid), "pid {pid}");
    }
}

#[test]
fn this_process_is_the_app_when_the_daemon_is_told_its_executable() {
    let exe = std::env::current_exe().unwrap();
    assert!(SystemProcessTree::new(Some(exe)).is_app(std::process::id()));
    assert!(!no_app().is_app(std::process::id()));
}

#[test]
fn an_app_path_given_through_a_symlink_names_the_same_executable() {
    let mut short = Short::new();
    let link = short.path("app-link");
    std::os::unix::fs::symlink(std::env::current_exe().unwrap(), &link).unwrap();
    assert!(SystemProcessTree::new(Some(link)).is_app(std::process::id()));
}

// ---- another uid, and the socket directory, with the real readers -----------------------------------------------

/// A peer of another uid. One cannot be made without root, so the refusal is tested through the trait, over the
/// real tree.
struct ForeignPeer {
    uid: u32,
    pid_reads: Cell<u32>,
}

impl PeerCredentials for ForeignPeer {
    fn uid(&self) -> io::Result<u32> {
        Ok(self.uid)
    }

    fn pid(&self) -> io::Result<u32> {
        self.pid_reads.set(self.pid_reads.get() + 1);
        Ok(std::process::id())
    }
}

#[test]
fn a_connection_from_another_uid_is_refused_and_its_pid_is_never_read() {
    let mine = auth::daemon_uid();
    let peer = ForeignPeer {
        uid: mine.wrapping_add(1),
        pid_reads: Cell::new(0),
    };
    let error = auth::admit(mine, &peer, &no_app()).unwrap_err();
    assert!(
        matches!(error, SocketError::ForeignUid { uid, daemon_uid } if uid == mine.wrapping_add(1) && daemon_uid == mine),
        "{error}"
    );
    assert_eq!(peer.pid_reads.get(), 0);
    assert!(matches!(
        auth::admit_shim(mine, &peer),
        Err(SocketError::ForeignUid { .. })
    ));
}

fn paths_in(short: &mut Short) -> DaemonPaths {
    let socket_dir = short.path("d");
    DaemonPaths {
        socket: socket_dir.join("s"),
        token_file: socket_dir.join("t"),
        lock_file: socket_dir.join("l"),
        runs_dir: socket_dir.join("runs"),
        store_dir: socket_dir.join("store"),
        log_file: socket_dir.join("log"),
        socket_dir,
    }
}

#[test]
fn the_socket_directory_checks_pass_against_the_real_uid() {
    let mut short = Short::new();
    let paths = paths_in(&mut short);
    auth::prepare_daemon_socket(&auth::StdFs, auth::daemon_uid(), &paths).unwrap();
    let mode = fs::metadata(&paths.socket_dir).unwrap().mode() & 0o7777;
    assert_eq!(mode, SOCKET_DIR_MODE);
    // A second time the directory is there, and still passes.
    auth::prepare_daemon_socket(&auth::StdFs, auth::daemon_uid(), &paths).unwrap();
}

#[test]
fn a_socket_directory_open_to_the_group_is_refused_against_the_real_uid() {
    let mut short = Short::new();
    let paths = paths_in(&mut short);
    auth::prepare_daemon_socket(&auth::StdFs, auth::daemon_uid(), &paths).unwrap();
    fs::set_permissions(&paths.socket_dir, fs::Permissions::from_mode(0o750)).unwrap();
    let error = auth::prepare_daemon_socket(&auth::StdFs, auth::daemon_uid(), &paths).unwrap_err();
    assert!(
        matches!(error, SocketError::UnsafeDir { mode: 0o750, .. }),
        "{error}"
    );
}

#[test]
fn a_socket_directory_of_another_owner_is_refused_against_the_real_uid() {
    let mut short = Short::new();
    let paths = paths_in(&mut short);
    auth::prepare_daemon_socket(&auth::StdFs, auth::daemon_uid(), &paths).unwrap();
    let error =
        auth::prepare_daemon_socket(&auth::StdFs, auth::daemon_uid().wrapping_add(1), &paths)
            .unwrap_err();
    assert!(matches!(error, SocketError::UnsafeDir { .. }), "{error}");
}

#[test]
fn a_symlink_in_place_of_the_socket_directory_is_refused_against_the_real_uid() {
    let mut short = Short::new();
    let real = short.path("real");
    fs::create_dir(&real).unwrap();
    fs::set_permissions(&real, fs::Permissions::from_mode(0o700)).unwrap();
    let paths = paths_in(&mut short);
    std::os::unix::fs::symlink(&real, &paths.socket_dir).unwrap();
    let error = auth::prepare_daemon_socket(&auth::StdFs, auth::daemon_uid(), &paths).unwrap_err();
    assert!(
        matches!(error, SocketError::NotADirectory { .. }),
        "{error}"
    );
}
