//! The daemon's socket server (ts-0056-ag), run inside the test process on a folder of its own under `/tmp`: the
//! handshake and its refusals, the token file, the socket's directory and path, the base environment at start, the
//! store that cannot be read, and a log that fails with `ENOSPC`. Nothing here touches the user's daemon files, a real
//! daemon, `launchctl` or the user's shell: every path is below the test's root, and the login shell is a script the
//! test writes. The daemon's own process (setsid, the lock, a terminal that closes) is `daemon_process.rs`.

mod common;

use collide_core::owner::{MAX_MESSAGE_BYTES, OWNER_PROTOCOL, code};
use collide_host::base_env::{CaptureOptions, EnvSource};
use collide_host::daemon::{Daemon, DaemonOptions, DaemonPaths, SocketError};
use common::TempRoot;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::fs;
use std::io::{self, BufRead, BufReader, Write};
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const PYTHON: &str = "/usr/bin/python3";
/// ENOSPC is 28 on macOS and on Linux.
const ENOSPC: i32 = 28;

/// A log the test can read back.
#[derive(Clone, Default)]
struct LogBuf(Arc<Mutex<Vec<u8>>>);

impl LogBuf {
    fn text(&self) -> String {
        String::from_utf8_lossy(&self.0.lock().unwrap()).into_owned()
    }
}

impl Write for LogBuf {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// A log that is full: every write fails with `ENOSPC`.
struct FullDisk;

impl Write for FullDisk {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        Err(io::Error::from_raw_os_error(ENOSPC))
    }

    fn flush(&mut self) -> io::Result<()> {
        Err(io::Error::from_raw_os_error(ENOSPC))
    }
}

/// A login shell that prints the environment, as the capture asks.
fn fake_shell(root: &TempRoot) -> String {
    root.script(
        "fake-shell",
        "#!/bin/sh\nif [ -f \"$0.broken\" ]; then exit 3; fi\nexport COLLIDE_FAKE=$(cat \"$0.gen\" 2>/dev/null)\neval \"$4\"\n",
    )
    .to_str()
    .unwrap()
    .to_string()
}

fn options(root: &TempRoot, log: Box<dyn Write + Send>) -> DaemonOptions {
    let mut options = DaemonOptions::new(DaemonPaths::under(root.path()), fake_shell(root), log);
    options.process_tree = common::test_process_tree();
    options.capture = CaptureOptions {
        timeout: Duration::from_secs(10),
        cap_bytes: 1024 * 1024,
        fallback: BTreeMap::from([("PATH".to_string(), "/usr/bin:/bin".to_string())]),
    };
    options
}

fn start(root: &TempRoot) -> (Daemon, LogBuf) {
    let log = LogBuf::default();
    let daemon = Daemon::start(options(root, Box::new(log.clone()))).expect("the daemon starts");
    (daemon, log)
}

fn token_of(paths: &DaemonPaths) -> String {
    fs::read_to_string(&paths.token_file)
        .unwrap()
        .trim()
        .to_string()
}

/// One client connection that speaks lines.
struct Client {
    stream: UnixStream,
    reader: BufReader<UnixStream>,
}

impl Client {
    fn connect(paths: &DaemonPaths) -> Client {
        let stream = UnixStream::connect(&paths.socket).expect("the daemon answers");
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        let reader = BufReader::new(stream.try_clone().unwrap());
        Client { stream, reader }
    }

    fn send(&mut self, value: &Value) {
        self.send_raw(&format!("{value}\n"));
    }

    fn send_raw(&mut self, text: &str) {
        self.stream.write_all(text.as_bytes()).unwrap();
    }

    /// The next line as JSON, or `None` when the daemon closed the connection.
    fn read(&mut self) -> Option<Value> {
        let mut line = String::new();
        match self.reader.read_line(&mut line) {
            Ok(0) => None,
            Ok(_) => Some(serde_json::from_str(&line).unwrap_or_else(|e| panic!("{e}: {line}"))),
            Err(e) if e.kind() == io::ErrorKind::ConnectionReset => None,
            Err(e) => panic!("reading from the daemon: {e}"),
        }
    }

    fn call(&mut self, id: u64, method: &str, params: Value) -> Value {
        self.send(&json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}));
        self.read().expect("the daemon answers")
    }

    fn initialize(&mut self, params: Value) -> Value {
        self.call(1, "initialize", params)
    }
}

fn error_of(response: &Value) -> (i64, String) {
    let error = &response["error"];
    assert!(error.is_object(), "not an error: {response}");
    (
        error["code"].as_i64().unwrap(),
        error["message"].as_str().unwrap().to_string(),
    )
}

fn connected(daemon: &Daemon) -> Client {
    let paths = daemon.paths();
    let mut client = Client::connect(paths);
    let response = client.initialize(json!({"protocol": OWNER_PROTOCOL, "token": token_of(paths)}));
    assert!(response["result"].is_object(), "{response}");
    client
}

fn wait_for<T>(what: &str, mut f: impl FnMut() -> Option<T>) -> T {
    let until = Instant::now() + Duration::from_secs(10);
    while Instant::now() < until {
        if let Some(found) = f() {
            return found;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("timed out waiting for {what}");
}

// ---- the handshake ----------------------------------------------------------------------------------------------

#[test]
fn a_client_with_the_token_and_the_version_is_let_in_and_told_its_actor() {
    let root = TempRoot::new();
    let (daemon, _) = start(&root);
    let mut client = Client::connect(daemon.paths());
    let response = client.initialize(json!({
        "protocol": OWNER_PROTOCOL,
        "token": token_of(daemon.paths()),
        "client": "the test"
    }));
    assert_eq!(response["id"], 1);
    assert_eq!(response["result"]["protocol"], OWNER_PROTOCOL);
    assert_eq!(response["result"]["actor"]["kind"], "person");
    assert!(response["result"]["daemon_version"].is_string());
}

#[test]
fn a_client_with_no_token_is_refused_and_the_connection_is_closed() {
    let root = TempRoot::new();
    let (daemon, _) = start(&root);
    let mut client = Client::connect(daemon.paths());
    let response = client.initialize(json!({"protocol": OWNER_PROTOCOL}));
    let (number, message) = error_of(&response);
    assert_eq!(number, code::TOKEN_REFUSED);
    assert!(message.contains("missing"), "{message}");
    assert!(client.read().is_none(), "the connection stayed open");
}

#[test]
fn a_client_with_a_wrong_token_is_refused() {
    let root = TempRoot::new();
    let (daemon, _) = start(&root);
    let mut client = Client::connect(daemon.paths());
    let response = client.initialize(json!({"protocol": OWNER_PROTOCOL, "token": "not-the-token"}));
    let (number, message) = error_of(&response);
    assert_eq!(number, code::TOKEN_REFUSED);
    assert!(message.contains("wrong"), "{message}");
    assert!(client.read().is_none());
}

#[test]
fn a_different_protocol_version_is_refused_with_both_versions_in_the_message() {
    let root = TempRoot::new();
    let (daemon, _) = start(&root);
    let mut client = Client::connect(daemon.paths());
    let response = client.initialize(json!({"protocol": 7, "token": token_of(daemon.paths())}));
    let (number, message) = error_of(&response);
    assert_eq!(number, code::VERSION_REFUSED);
    assert!(message.contains("protocol 7"), "{message}");
    assert!(
        message.contains(&format!("speaks {OWNER_PROTOCOL}")),
        "{message}"
    );
    assert_eq!(response["error"]["data"]["client"], 7);
    assert_eq!(response["error"]["data"]["daemon"], OWNER_PROTOCOL);
    assert!(client.read().is_none());
}

#[test]
fn a_wrong_token_with_a_wrong_version_learns_no_version() {
    let root = TempRoot::new();
    let (daemon, _) = start(&root);
    let mut client = Client::connect(daemon.paths());
    let response = client.initialize(json!({"protocol": 7, "token": "nope"}));
    let (number, message) = error_of(&response);
    assert_eq!(number, code::TOKEN_REFUSED);
    assert!(!message.contains("speaks"), "{message}");
}

#[test]
fn a_method_before_initialize_is_refused_and_the_connection_is_closed() {
    let root = TempRoot::new();
    let (daemon, _) = start(&root);
    let mut client = Client::connect(daemon.paths());
    let response = client.call(5, "daemon.status", json!({}));
    assert_eq!(error_of(&response).0, code::NOT_INITIALIZED);
    assert_eq!(response["id"], 5);
    assert!(client.read().is_none());
}

#[test]
fn a_second_initialize_is_refused_and_the_connection_goes_on() {
    let root = TempRoot::new();
    let (daemon, _) = start(&root);
    let mut client = connected(&daemon);
    let response = client.initialize(json!({"protocol": OWNER_PROTOCOL, "token": "x"}));
    assert_eq!(error_of(&response).0, code::ALREADY_INITIALIZED);
    assert!(client.call(2, "daemon.status", json!({}))["result"].is_object());
}

#[test]
fn text_that_is_not_json_before_initialize_is_answered_once_and_closed() {
    let root = TempRoot::new();
    let (daemon, _) = start(&root);
    let mut client = Client::connect(daemon.paths());
    client.send_raw("this is not json\n");
    let response = client.read().expect("an answer");
    assert_eq!(error_of(&response).0, code::PARSE_ERROR);
    assert!(response["id"].is_null());
    assert!(client.read().is_none());
}

#[test]
fn text_that_is_not_json_after_initialize_is_answered_and_the_connection_goes_on() {
    let root = TempRoot::new();
    let (daemon, _) = start(&root);
    let mut client = connected(&daemon);
    client.send_raw("{ broken\n");
    assert_eq!(error_of(&client.read().unwrap()).0, code::PARSE_ERROR);
    client.send_raw("{\"jsonrpc\":\"2.0\",\"id\":9}\n");
    assert_eq!(error_of(&client.read().unwrap()).0, code::INVALID_REQUEST);
    assert!(client.call(3, "daemon.status", json!({}))["result"].is_object());
}

#[test]
fn bytes_that_are_not_text_are_a_parse_error_and_not_a_crash() {
    let root = TempRoot::new();
    let (daemon, _) = start(&root);
    let mut client = connected(&daemon);
    client.stream.write_all(&[0xff, 0xfe, 0x80, b'\n']).unwrap();
    assert_eq!(error_of(&client.read().unwrap()).0, code::PARSE_ERROR);
    assert!(client.call(3, "daemon.status", json!({}))["result"].is_object());
}

#[test]
fn a_line_over_the_message_limit_is_refused_and_the_connection_is_closed() {
    let root = TempRoot::new();
    let (daemon, _) = start(&root);
    let mut client = connected(&daemon);
    let mut line = vec![b'a'; MAX_MESSAGE_BYTES + 1];
    line.push(b'\n');
    // The daemon stops reading at the limit and answers; whether the rest of the write lands does not matter.
    let _ = client.stream.write_all(&line);
    let response = client.read().expect("an answer before the close");
    let (number, message) = error_of(&response);
    assert_eq!(number, code::INVALID_REQUEST);
    assert!(
        message.contains(&MAX_MESSAGE_BYTES.to_string()),
        "{message}"
    );
    // A new connection is served: the daemon did not stop.
    let mut again = connected(&daemon);
    assert!(again.call(2, "daemon.status", json!({}))["result"].is_object());
}

#[test]
fn a_client_that_connects_and_says_nothing_is_closed_after_the_handshake_wait() {
    let root = TempRoot::new();
    let log = LogBuf::default();
    let mut opts = options(&root, Box::new(log.clone()));
    opts.handshake_wait = Duration::from_millis(300);
    let daemon = Daemon::start(opts).unwrap();
    let mut client = Client::connect(daemon.paths());
    let start = Instant::now();
    assert!(
        client.read().is_none(),
        "the daemon answered a silent client"
    );
    assert!(start.elapsed() < Duration::from_secs(5));
    wait_for("the log line", || {
        log.text().contains("said nothing").then_some(())
    });
}

#[test]
fn clients_that_drop_half_way_leave_the_daemon_serving() {
    let root = TempRoot::new();
    let (daemon, _) = start(&root);
    for _ in 0..20 {
        let mut stream = UnixStream::connect(&daemon.paths().socket).unwrap();
        let _ = stream.write_all(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"init");
    }
    for _ in 0..20 {
        drop(UnixStream::connect(&daemon.paths().socket).unwrap());
    }
    let mut client = connected(&daemon);
    assert!(client.call(2, "daemon.status", json!({}))["result"].is_object());
}

#[test]
fn a_request_with_an_actor_is_refused_because_the_daemon_decides_the_actor() {
    let root = TempRoot::new();
    let (daemon, _) = start(&root);
    let mut client = connected(&daemon);
    let response = client.call(2, "daemon.status", json!({"actor": {"kind": "person"}}));
    assert_eq!(error_of(&response).0, code::INVALID_PARAMS);
}

#[test]
fn a_method_that_another_item_serves_answers_method_not_found() {
    let root = TempRoot::new();
    let (daemon, _) = start(&root);
    let mut client = connected(&daemon);
    assert_eq!(
        error_of(&client.call(2, "run.list", json!({}))).0,
        code::METHOD_NOT_FOUND
    );
    assert_eq!(
        error_of(&client.call(3, "no.such.method", json!({}))).0,
        code::METHOD_NOT_FOUND
    );
}

#[test]
fn a_client_inside_a_run_is_an_agent_of_that_run() {
    let root = TempRoot::new();
    let (daemon, _) = start(&root);
    let script = "import socket, sys, json\n\
s = socket.socket(socket.AF_UNIX)\n\
s.connect(sys.argv[1])\n\
req = {'jsonrpc': '2.0', 'id': 1, 'method': 'initialize', 'params': {'protocol': 1, 'token': sys.argv[2]}}\n\
s.sendall((json.dumps(req) + '\\n').encode())\n\
print(s.makefile().readline())\n";
    let output = std::process::Command::new(PYTHON)
        .args(["-c", script])
        .arg(&daemon.paths().socket)
        .arg(token_of(daemon.paths()))
        .env("COLLIDE_RUN", "run-from-test")
        .output()
        .unwrap();
    let response: Value =
        serde_json::from_slice(&output.stdout).unwrap_or_else(|e| panic!("{e}: {output:?}"));
    assert_eq!(response["result"]["actor"]["kind"], "agent");
    assert_eq!(response["result"]["actor"]["run"], "run-from-test");
}

#[test]
fn status_names_the_daemon_its_protocol_and_a_store_that_is_well() {
    let root = TempRoot::new();
    let (daemon, _) = start(&root);
    let mut client = connected(&daemon);
    let status = client.call(2, "daemon.status", json!({}))["result"].clone();
    assert_eq!(status["pid"], std::process::id());
    assert_eq!(status["protocol"], OWNER_PROTOCOL);
    assert_eq!(status["store"]["state"], "ok");
    assert_eq!(status["head_seq"], 0);
    assert!(status["uptime_secs"].as_u64().unwrap() < 60);
}

// ---- the token file, the directory and the path -----------------------------------------------------------------

#[test]
fn the_token_file_is_made_at_the_first_start_with_mode_0600() {
    let root = TempRoot::new();
    let (daemon, _) = start(&root);
    let meta = fs::metadata(&daemon.paths().token_file).unwrap();
    assert_eq!(meta.permissions().mode() & 0o7777, 0o600);
    let token = token_of(daemon.paths());
    assert_eq!(token.len(), 64);
    assert!(token.bytes().all(|b| b.is_ascii_hexdigit()));
}

#[test]
fn the_socket_directory_is_0700_and_the_socket_is_private() {
    let root = TempRoot::new();
    let (daemon, _) = start(&root);
    let dir = fs::metadata(&daemon.paths().socket_dir).unwrap();
    assert_eq!(dir.permissions().mode() & 0o7777, 0o700);
    let socket = fs::metadata(&daemon.paths().socket).unwrap();
    assert_eq!(socket.permissions().mode() & 0o777, 0o600);
}

#[test]
fn a_restarted_daemon_keeps_the_token_so_the_clients_still_in_the_field_work() {
    let root = TempRoot::new();
    let (first, _) = start(&root);
    let token = token_of(first.paths());
    first.stop();
    let (second, _) = start(&root);
    assert_eq!(token_of(second.paths()), token);
    let mut client = connected(&second);
    assert!(client.call(2, "daemon.status", json!({}))["result"].is_object());
}

#[test]
fn a_token_file_that_others_can_read_stops_the_start() {
    let root = TempRoot::new();
    let paths = DaemonPaths::under(root.path());
    fs::DirBuilder::new()
        .mode(0o700)
        .create(&paths.socket_dir)
        .unwrap();
    fs::write(&paths.token_file, "secret").unwrap();
    fs::set_permissions(&paths.token_file, fs::Permissions::from_mode(0o644)).unwrap();
    let error = Daemon::start(options(&root, Box::new(LogBuf::default())))
        .err()
        .unwrap();
    assert!(
        matches!(error, SocketError::UnsafeTokenFile { mode: 0o644, .. }),
        "{error}"
    );
    assert!(
        error
            .to_string()
            .contains(paths.token_file.to_str().unwrap())
    );
}

#[test]
fn an_empty_token_file_stops_the_start() {
    let root = TempRoot::new();
    let paths = DaemonPaths::under(root.path());
    fs::DirBuilder::new()
        .mode(0o700)
        .create(&paths.socket_dir)
        .unwrap();
    fs::write(&paths.token_file, "").unwrap();
    fs::set_permissions(&paths.token_file, fs::Permissions::from_mode(0o600)).unwrap();
    let error = Daemon::start(options(&root, Box::new(LogBuf::default())))
        .err()
        .unwrap();
    assert!(error.to_string().contains("empty"), "{error}");
}

#[test]
fn a_socket_directory_open_to_others_stops_the_start_naming_the_directory() {
    let root = TempRoot::new();
    let paths = DaemonPaths::under(root.path());
    fs::DirBuilder::new()
        .mode(0o755)
        .create(&paths.socket_dir)
        .unwrap();
    fs::set_permissions(&paths.socket_dir, fs::Permissions::from_mode(0o755)).unwrap();
    let error = Daemon::start(options(&root, Box::new(LogBuf::default())))
        .err()
        .unwrap();
    assert!(
        matches!(error, SocketError::UnsafeDir { mode: 0o755, .. }),
        "{error}"
    );
    let message = error.to_string();
    assert!(
        message.contains(paths.socket_dir.to_str().unwrap()),
        "{message}"
    );
    assert!(
        message.contains("0755") && message.contains("0700"),
        "{message}"
    );
    assert!(
        !paths.socket.exists(),
        "a socket was made in an unsafe directory"
    );
}

#[test]
fn a_socket_path_over_103_bytes_fails_fast_naming_the_path_and_the_limit() {
    let root = TempRoot::new();
    let mut opts = options(&root, Box::new(LogBuf::default()));
    let long = root.path().join("x".repeat(100));
    opts.paths = DaemonPaths::under(&long);
    let paths = opts.paths.clone();
    let error = Daemon::start(opts).err().unwrap();
    assert!(matches!(error, SocketError::PathTooLong { .. }), "{error}");
    let message = error.to_string();
    assert!(
        message.contains(paths.socket.to_str().unwrap()),
        "{message}"
    );
    assert!(message.contains("103"), "{message}");
    // Fast means before anything is made: not even the folders.
    assert!(!paths.socket_dir.exists());
    assert!(!paths.store_dir.exists());
}

#[test]
fn a_socket_path_of_exactly_103_bytes_is_accepted() {
    let root = TempRoot::new();
    let base = root.path().to_str().unwrap().len();
    // root/<pad>/run/d.sock : the pad makes the whole socket path 103 bytes.
    let suffix = "/run/d.sock".len() + 1;
    let pad = 103 - base - suffix;
    let mut opts = options(&root, Box::new(LogBuf::default()));
    let nested = root.path().join("p".repeat(pad));
    fs::DirBuilder::new().mode(0o700).create(&nested).unwrap();
    opts.paths = DaemonPaths::under(&nested);
    assert_eq!(opts.paths.socket.as_os_str().len(), 103);
    let daemon = Daemon::start(opts).expect("103 bytes fit");
    assert!(UnixStream::connect(&daemon.paths().socket).is_ok());
}

#[test]
fn a_second_daemon_on_a_socket_that_answers_does_not_take_it() {
    let root = TempRoot::new();
    let (first, _) = start(&root);
    let error = Daemon::start(options(&root, Box::new(LogBuf::default())))
        .err()
        .unwrap();
    assert!(
        matches!(error, SocketError::AlreadyRunning { .. }),
        "{error}"
    );
    let mut client = connected(&first);
    assert!(client.call(2, "daemon.status", json!({}))["result"].is_object());
}

#[test]
fn a_socket_file_left_by_a_daemon_that_died_is_replaced() {
    let root = TempRoot::new();
    let paths = DaemonPaths::under(root.path());
    fs::DirBuilder::new()
        .mode(0o700)
        .create(&paths.socket_dir)
        .unwrap();
    drop(UnixListener::bind(&paths.socket).unwrap());
    assert!(paths.socket.exists());
    let (daemon, _) = start(&root);
    let mut client = connected(&daemon);
    assert!(client.call(2, "daemon.status", json!({}))["result"].is_object());
}

#[test]
fn a_file_where_the_socket_goes_is_not_replaced() {
    let root = TempRoot::new();
    let paths = DaemonPaths::under(root.path());
    fs::DirBuilder::new()
        .mode(0o700)
        .create(&paths.socket_dir)
        .unwrap();
    fs::write(&paths.socket, "precious").unwrap();
    let error = Daemon::start(options(&root, Box::new(LogBuf::default())))
        .err()
        .unwrap();
    assert!(error.to_string().contains("not a socket"), "{error}");
    assert_eq!(fs::read_to_string(&paths.socket).unwrap(), "precious");
}

#[test]
fn stopping_removes_the_socket_and_keeps_the_token() {
    let root = TempRoot::new();
    let (daemon, _) = start(&root);
    let paths = daemon.paths().clone();
    let mut client = connected(&daemon);
    daemon.stop();
    assert!(!paths.socket.exists());
    assert!(paths.token_file.exists());
    assert!(client.read().is_none(), "a connected client was left open");
}

// ---- the store ---------------------------------------------------------------------------------------------------

#[test]
fn a_store_that_cannot_be_read_is_moved_aside_and_reported_and_the_daemon_runs() {
    let root = TempRoot::new();
    let paths = DaemonPaths::under(root.path());
    fs::create_dir_all(&paths.store_dir).unwrap();
    fs::write(
        paths.store_dir.join("store.db"),
        b"this is not a database at all",
    )
    .unwrap();
    let (daemon, log) = start(&root);
    let mut client = connected(&daemon);
    let status = client.call(2, "daemon.status", json!({}))["result"].clone();
    assert_eq!(status["store"]["state"], "quarantined");
    let moved_to = status["store"]["moved_to"].as_str().unwrap();
    assert!(Path::new(moved_to).exists(), "{moved_to}");
    assert_eq!(
        fs::read(moved_to).unwrap(),
        b"this is not a database at all"
    );
    assert!(
        status["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w.as_str().unwrap().contains("moved to")),
        "{status}"
    );
    assert!(log.text().contains("moved to"), "{}", log.text());
    // The fresh store works: the daemon went on serving.
    assert_eq!(status["head_seq"], 0);
}

#[test]
fn a_store_from_a_newer_build_stops_the_start_and_is_left_alone() {
    let root = TempRoot::new();
    let paths = DaemonPaths::under(root.path());
    fs::create_dir_all(&paths.store_dir).unwrap();
    let db = paths.store_dir.join("store.db");
    set_user_version(&db, 9999);
    let error = Daemon::start(options(&root, Box::new(LogBuf::default())))
        .err()
        .unwrap();
    assert!(matches!(error, SocketError::Store(_)), "{error}");
    // Not moved aside and not downgraded: it is still there at the version a newer build gave it.
    let connection = rusqlite::Connection::open(&db).unwrap();
    let version: u32 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    assert_eq!(version, 9999);
    let moved: Vec<_> = fs::read_dir(&paths.store_dir).unwrap().flatten().collect();
    assert!(
        moved
            .iter()
            .all(|e| !e.file_name().to_string_lossy().contains("unreadable")),
        "{moved:?}"
    );
}

/// Makes an SQLite file whose `user_version` is `version`, as a build newer than this one would leave it.
fn set_user_version(db: &Path, version: u32) {
    let connection = rusqlite::Connection::open(db).unwrap();
    connection
        .pragma_update(None, "user_version", version)
        .unwrap();
}

// ---- the base environment ----------------------------------------------------------------------------------------

#[test]
fn the_socket_answers_while_the_base_environment_is_still_being_captured() {
    let root = TempRoot::new();
    // A shell that hangs: the capture ends at its timeout, and the daemon served meanwhile.
    let hung = root.script("hung-shell", "#!/bin/sh\nsleep 60\n");
    let log = LogBuf::default();
    let mut opts = options(&root, Box::new(log.clone()));
    opts.shell = hung.to_str().unwrap().to_string();
    opts.capture.timeout = Duration::from_secs(3);
    let daemon = Daemon::start(opts).unwrap();
    let began = Instant::now();
    let mut client = connected(&daemon);
    assert!(client.call(2, "daemon.status", json!({}))["result"].is_object());
    assert!(began.elapsed() < Duration::from_secs(2));
    assert!(
        daemon.base_environment().is_none(),
        "the capture ended early"
    );

    let env = wait_for("the fallback", || daemon.base_environment());
    let EnvSource::Minimal { shell, reason } = &env.source else {
        panic!("expected the minimal environment, got {:?}", env.source);
    };
    assert_eq!(shell.as_deref(), hung.to_str());
    assert!(reason.contains("did not finish"), "{reason}");
    assert_eq!(
        env.vars.get("PATH").map(String::as_str),
        Some("/usr/bin:/bin")
    );
    assert_eq!(
        env.vars.get("LANG").map(String::as_str),
        Some("en_US.UTF-8")
    );
    // The shell and the reason are in the log, and in the status.
    let text = log.text();
    assert!(
        text.contains(hung.to_str().unwrap()) && text.contains("did not finish"),
        "{text}"
    );
    let status = client.call(3, "daemon.status", json!({}))["result"].clone();
    assert!(
        status["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w.as_str().unwrap().contains("did not finish")),
        "{status}"
    );
}

#[test]
fn the_environment_is_captured_at_start_and_again_on_request_and_a_failed_capture_keeps_the_good_one()
 {
    let root = TempRoot::new();
    fs::write(root.join("fake-shell.gen"), "one").unwrap();
    let (daemon, _) = start(&root);
    let first = wait_for("the first capture", || daemon.base_environment());
    assert_eq!(
        first.vars.get("COLLIDE_FAKE").map(String::as_str),
        Some("one")
    );
    let running_phase = first.for_child(&Default::default(), &BTreeMap::new());

    // The app starts and asks again: the new capture is the daemon's newest.
    fs::write(root.join("fake-shell.gen"), "two").unwrap();
    let mut client = connected(&daemon);
    let result = client.call(2, "daemon.recapture_environment", json!({}))["result"].clone();
    assert_eq!(result["capture"]["outcome"], "captured");
    assert!(result["capture"]["variables"].as_u64().unwrap() > 0);
    let second = daemon.base_environment().unwrap();
    assert_eq!(
        second.vars.get("COLLIDE_FAKE").map(String::as_str),
        Some("two")
    );
    // A phase that copied the first capture keeps what it had.
    assert_eq!(
        running_phase.get("COLLIDE_FAKE").map(String::as_str),
        Some("one")
    );
    // No value of the environment is in the answer.
    assert!(!result.to_string().contains("two"), "{result}");

    // A capture that fails is reported, and the working one stays.
    fs::write(root.join("fake-shell.broken"), "").unwrap();
    let failed = client.call(3, "daemon.recapture_environment", json!({}))["result"].clone();
    assert_eq!(failed["capture"]["outcome"], "fell_back");
    assert!(
        failed["capture"]["reason"]
            .as_str()
            .unwrap()
            .contains("exit status 3")
    );
    let kept = daemon.base_environment().unwrap();
    assert!(matches!(kept.source, EnvSource::LoginShell { .. }));
    assert_eq!(
        kept.vars.get("COLLIDE_FAKE").map(String::as_str),
        Some("two")
    );
}

// ---- the log -----------------------------------------------------------------------------------------------------

#[test]
fn a_log_that_fails_with_enospc_does_not_crash_or_stop_the_daemon() {
    let root = TempRoot::new();
    let daemon = Daemon::start(options(&root, Box::new(FullDisk)))
        .expect("a full log is not a failed start");
    let mut client = connected(&daemon);
    for id in 2..12 {
        assert!(client.call(id, "daemon.status", json!({}))["result"].is_object());
    }
    // Refusals are logged too, and are still answered.
    let mut refused = Client::connect(daemon.paths());
    let response = refused.initialize(json!({"protocol": OWNER_PROTOCOL, "token": "nope"}));
    assert_eq!(error_of(&response).0, code::TOKEN_REFUSED);
    assert!(
        daemon.log_failures() > 0,
        "the failing writer was never used"
    );
    let mut again = connected(&daemon);
    assert!(again.call(2, "daemon.status", json!({}))["result"].is_object());
}

#[test]
fn a_log_that_works_has_no_failures_and_names_the_connections() {
    let root = TempRoot::new();
    let (daemon, log) = start(&root);
    let mut client = connected(&daemon);
    client.call(2, "daemon.status", json!({}));
    assert_eq!(daemon.log_failures(), 0);
    wait_for("the connection line", || {
        log.text().contains("connected as").then_some(())
    });
    assert!(log.text().contains("listening on"));
}

#[test]
fn two_requests_in_one_write_are_answered_in_order() {
    let root = TempRoot::new();
    let (daemon, _) = start(&root);
    let mut client = connected(&daemon);
    let one = json!({"jsonrpc": "2.0", "id": 10, "method": "daemon.status", "params": {}});
    let two = json!({"jsonrpc": "2.0", "id": 11, "method": "daemon.status"});
    client.send_raw(&format!("{one}\n{two}\n"));
    assert_eq!(client.read().unwrap()["id"], 10);
    assert_eq!(client.read().unwrap()["id"], 11);
}

/// A `launchctl` that only records what it is asked.
#[derive(Debug, Default)]
struct RecordingLaunchctl {
    calls: Mutex<Vec<String>>,
}

impl collide_host::launch::Launchctl for RecordingLaunchctl {
    fn bootstrap(&self, _domain: &str, _plist: &Path) -> io::Result<()> {
        Err(io::Error::other("a daemon never bootstraps"))
    }

    fn bootout(&self, target: &str) -> io::Result<()> {
        self.calls.lock().unwrap().push(format!("bootout {target}"));
        Ok(())
    }

    fn process_of(&self, _target: &str) -> io::Result<Option<u32>> {
        Ok(None)
    }
}

#[test]
fn stopping_the_daemon_boots_out_the_launchd_job_its_start_recorded() {
    let root = TempRoot::new();
    let paths = DaemonPaths::under(root.path());
    let launchctl = Arc::new(RecordingLaunchctl::default());
    let mut daemon_options = options(&root, Box::new(LogBuf::default()));
    daemon_options.launchctl = Some(launchctl.clone());
    let daemon = Daemon::start(daemon_options).expect("the daemon starts");
    let label_file = paths.lock_file.parent().unwrap().join("launchd.label");
    fs::write(&label_file, "dev.collide.test.daemon-1\n").unwrap();
    daemon.stop();
    // SAFETY: `getuid` takes no arguments and cannot fail.
    let uid = unsafe { libc::getuid() };
    assert_eq!(
        launchctl.calls.lock().unwrap().clone(),
        vec![format!("bootout gui/{uid}/dev.collide.test.daemon-1")]
    );
    assert!(!label_file.exists(), "the record goes with the job");
}

#[test]
fn a_daemon_with_no_launchctl_calls_none_and_keeps_the_record() {
    let root = TempRoot::new();
    let paths = DaemonPaths::under(root.path());
    let (daemon, _log) = start(&root);
    let label_file = paths.lock_file.parent().unwrap().join("launchd.label");
    fs::write(&label_file, "dev.collide.test.daemon-2\n").unwrap();
    daemon.stop();
    assert!(label_file.exists());
}
