//! What the shell's tests share (ts-0077-ws): a root of their own below `/tmp`, a daemon that runs in the test process,
//! a fake daemon that a test scripts, and fakes for the starter, the clocks and the event channel.
//!
//! The test rule: no test touches the real home, the real launchd or a real daemon. Every path is below the test's root
//! and is given to the code under test, which reads no environment variable here. The daemon is `Daemon::start` in the
//! test process, with no shim, so it starts no `claude` and loads no launchd job. The real starter
//! (`LaunchStarter`) is never used by a test: it starts a `collide-daemon` process and may load a launchd job.

#![allow(dead_code)]

use collide_app::shell::{
    ChannelEvent, DaemonAccess, DaemonStarter, EventChannel, pump_daemon_events,
};
use collide_core::change::{Actor, ActorKind};
use collide_core::events::{EventBody, StepKind};
use collide_core::owner::{
    OWNER_PROTOCOL, OwnerInitializeResult, Request, Response, RpcError, encode_line,
};
use collide_host::auth::{self, Caller, ProcessTree, SystemProcessTree};
use collide_host::base_env::CaptureOptions;
use collide_host::clock::{Clock, Clocks, Reading};
use collide_host::daemon::{Daemon, DaemonOptions, DaemonPaths, system_process_tree};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

// ---- a root of the test's own -----------------------------------------------------------------------------------

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// A folder `/tmp/cda-<pid>-<n>` that one test makes and owns, short enough for a socket path (103 bytes). `Drop`
/// removes
/// it by its own path.
pub struct TempRoot(PathBuf);

impl TempRoot {
    pub fn new() -> TempRoot {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = PathBuf::from(format!("/tmp/cda-{}-{n}", std::process::id()));
        fs::create_dir(&dir).unwrap();
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).unwrap();
        TempRoot(dir)
    }

    pub fn path(&self) -> &Path {
        &self.0
    }

    pub fn paths(&self) -> DaemonPaths {
        DaemonPaths::under(&self.0)
    }

    /// Writes an executable script (mode 0755) and returns its path.
    pub fn script(&self, name: &str, body: &str) -> PathBuf {
        let path = self.0.join(name);
        fs::write(&path, body).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        path
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        assert!(self.0.to_str().unwrap().starts_with("/tmp/cda-"));
        let _ = fs::remove_dir_all(&self.0);
    }
}

pub fn wait_until(what: &str, within: Duration, mut done: impl FnMut() -> bool) {
    let until = Instant::now() + within;
    while Instant::now() < until {
        if done() {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(done(), "timed out after {within:?} waiting for: {what}");
}

// ---- the daemon, in this process --------------------------------------------------------------------------------

/// Who is asking, as far as a test is concerned: the daemon's own default tree ([`system_process_tree`]), except that
/// the
/// walk stops at this test process and the process itself is not read for a run marker. What is above it (cargo, a
/// shell,
/// a phase of a Collide run with `COLLIDE_RUN` in its environment) is not the test's, and a test cannot change it.
struct StopsAtThisTest(SystemProcessTree);

impl ProcessTree for StopsAtThisTest {
    fn parent(&self, pid: u32) -> Option<u32> {
        (pid != std::process::id())
            .then(|| self.0.parent(pid))
            .flatten()
    }

    fn run_marker(&self, pid: u32) -> Option<String> {
        (pid != std::process::id())
            .then(|| self.0.run_marker(pid))
            .flatten()
    }

    fn is_app(&self, pid: u32) -> bool {
        self.0.is_app(pid)
    }
}

/// A log that a test can read.
#[derive(Clone, Default)]
pub struct SharedLog(Arc<Mutex<Vec<u8>>>);

impl SharedLog {
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.0.lock().unwrap()).into_owned()
    }
}

impl Write for SharedLog {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Starts a daemon on `root` in this process. The login shell is a script that prints its environment and, when
/// `shell_runs` is given, appends a line to that file each time it runs.
pub fn start_daemon(
    root: &TempRoot,
    log: Box<dyn Write + Send>,
    shell_runs: Option<&Path>,
) -> Daemon {
    let body = match shell_runs {
        Some(file) => format!("#!/bin/sh\necho run >> '{}'\neval \"$4\"\n", file.display()),
        None => "#!/bin/sh\neval \"$4\"\n".to_string(),
    };
    let shell = root
        .script("fake-shell", &body)
        .to_str()
        .unwrap()
        .to_string();
    let paths = root.paths();
    let mut options = DaemonOptions::new(paths.clone(), shell, log);
    options.process_tree = Some(Arc::new(StopsAtThisTest(system_process_tree(&paths, None))));
    options.home = Some(root.path().display().to_string());
    options.capture = CaptureOptions {
        timeout: Duration::from_secs(10),
        cap_bytes: 1024 * 1024,
        fallback: BTreeMap::from([("PATH".to_string(), "/usr/bin:/bin".to_string())]),
    };
    Daemon::start(options).expect("the daemon starts")
}

/// Appends `count` events through the daemon's own door for events.
pub fn append(daemon: &Daemon, count: usize) {
    for n in 0..count {
        daemon
            .phases()
            .append_event(
                Actor {
                    kind: ActorKind::Workflow,
                    run: None,
                    phase: None,
                },
                EventBody::StepIntent {
                    step: format!("step-{n}"),
                    kind: StepKind::Integrate,
                    action: "x".to_string(),
                },
            )
            .expect("the event is stored");
    }
}

/// A starter that runs the daemon in this process, as the real one starts a process: it starts one when no daemon
/// answers
/// on the socket and does nothing when one does. A test can end the daemon and the next start makes a new one on the
/// same
/// root, with the same store and token. Clones share the daemon.
#[derive(Clone)]
pub struct InProcessStarter(Arc<InProcess>);

struct InProcess {
    root: PathBuf,
    daemon: Mutex<Option<Daemon>>,
    starts: AtomicUsize,
}

impl InProcessStarter {
    pub fn new(root: &TempRoot) -> InProcessStarter {
        InProcessStarter(Arc::new(InProcess {
            root: root.path().to_path_buf(),
            daemon: Mutex::new(None),
            starts: AtomicUsize::new(0),
        }))
    }

    /// How many daemons this starter has started.
    pub fn starts(&self) -> usize {
        self.0.starts.load(Ordering::SeqCst)
    }

    pub fn stop_daemon(&self) {
        let daemon = self.0.daemon.lock().unwrap().take();
        drop(daemon);
    }
}

impl DaemonStarter for InProcessStarter {
    fn ensure_running(&self) -> Result<(), String> {
        let mut slot = self.0.daemon.lock().unwrap();
        let paths = DaemonPaths::under(&self.0.root);
        if UnixStream::connect(&paths.socket).is_ok() {
            return Ok(());
        }
        self.0.starts.fetch_add(1, Ordering::SeqCst);
        let shell = format!("{}/fake-shell", self.0.root.display());
        fs::write(&shell, "#!/bin/sh\neval \"$4\"\n").map_err(|e| e.to_string())?;
        fs::set_permissions(&shell, fs::Permissions::from_mode(0o755))
            .map_err(|e| e.to_string())?;
        let mut options = DaemonOptions::new(paths.clone(), shell, Box::new(std::io::sink()));
        options.process_tree = Some(Arc::new(StopsAtThisTest(system_process_tree(&paths, None))));
        options.home = Some(self.0.root.display().to_string());
        options.capture = CaptureOptions {
            timeout: Duration::from_secs(10),
            cap_bytes: 1024 * 1024,
            fallback: BTreeMap::from([("PATH".to_string(), "/usr/bin:/bin".to_string())]),
        };
        *slot = Some(Daemon::start(options).map_err(|e| e.to_string())?);
        Ok(())
    }
}

/// A starter that never starts a daemon, and counts how often it was asked.
#[derive(Clone, Default)]
pub struct FailingStarter(pub Arc<AtomicUsize>);

impl FailingStarter {
    pub fn asked(&self) -> usize {
        self.0.load(Ordering::SeqCst)
    }
}

impl DaemonStarter for FailingStarter {
    fn ensure_running(&self) -> Result<(), String> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Err("no collide-daemon next to the app".to_string())
    }
}

/// A starter for a daemon that is already there (the fake server): it is never asked to start anything.
pub struct AlreadyThere;

impl DaemonStarter for AlreadyThere {
    fn ensure_running(&self) -> Result<(), String> {
        Ok(())
    }
}

// ---- clocks, the access and the channel -------------------------------------------------------------------------

/// Clocks that a test moves.
#[derive(Clone, Default)]
pub struct TestClocks(Arc<AtomicU64>);

impl TestClocks {
    pub fn advance(&self, by: Duration) {
        self.0.fetch_add(by.as_nanos() as u64, Ordering::SeqCst);
    }
}

impl Clocks for TestClocks {
    fn read(&self, clock: Clock) -> Reading {
        Reading {
            clock,
            nanos: 1_000_000_000_000 + self.0.load(Ordering::SeqCst),
        }
    }
}

/// The path the tests give as the app's executable: this test's own binary, which is what the daemon's check finds when
/// this process connects.
pub fn this_executable() -> PathBuf {
    fs::canonicalize(std::env::current_exe().unwrap()).unwrap()
}

pub fn access(
    root: &TempRoot,
    starter: impl DaemonStarter + 'static,
    clocks: &TestClocks,
) -> Arc<DaemonAccess> {
    Arc::new(DaemonAccess::new(
        root.paths(),
        Box::new(starter),
        Arc::new(clocks.clone()),
        this_executable(),
    ))
}

/// An open event channel that keeps what was emitted.
#[derive(Clone, Default)]
pub struct Channel(pub Arc<Mutex<Vec<ChannelEvent>>>);

impl Channel {
    pub fn events(&self) -> Vec<ChannelEvent> {
        self.0.lock().unwrap().clone()
    }

    /// The `seq` of each `daemon.event` emitted so far, in order.
    pub fn seqs(&self) -> Vec<u64> {
        self.events()
            .iter()
            .filter(|event| event.method == "daemon.event")
            .map(|event| event.params["seq"].as_u64().unwrap())
            .collect()
    }
}

impl Channel {
    /// The `state` of each `shell.connection` notice emitted so far, in order.
    pub fn connection_states(&self) -> Vec<String> {
        self.events()
            .iter()
            .filter(|event| event.method == collide_app::shell::CONNECTION_EVENT)
            .map(|event| event.params["state"].as_str().unwrap().to_string())
            .collect()
    }
}

impl EventChannel for Channel {
    fn is_open(&self) -> bool {
        true
    }

    fn emit(&self, event: &ChannelEvent) {
        self.0.lock().unwrap().push(event.clone());
    }
}

pub fn request(id: i64, method: &str, params: Value) -> Request {
    Request {
        jsonrpc: collide_core::owner::JsonRpc2,
        id: collide_core::owner::RequestId::Number(id),
        method: method.to_string(),
        params,
    }
}

pub fn result_of(response: Response) -> Result<Value, RpcError> {
    match response {
        Response::Success { result, .. } => Ok(result),
        Response::Failure { error, .. } => Err(error),
    }
}

// ---- a fake daemon that a test scripts --------------------------------------------------------------------------

/// One request the fake daemon read: on which connection (counting from 0), and what.
#[derive(Debug, Clone, PartialEq)]
pub struct Seen {
    pub connection: usize,
    pub method: String,
    pub params: Value,
}

/// One connection of the fake daemon, for a script.
pub struct Conn {
    stream: UnixStream,
    reader: BufReader<UnixStream>,
    seen: Arc<Mutex<Vec<Seen>>>,
    pub index: usize,
}

impl Conn {
    /// Reads the next request and records it. `None` when the client closed.
    pub fn read(&mut self) -> Option<Request> {
        let mut line = String::new();
        if self.reader.read_line(&mut line).ok()? == 0 {
            return None;
        }
        let request: Request = serde_json::from_str(&line).expect("the client sends requests");
        self.seen.lock().unwrap().push(Seen {
            connection: self.index,
            method: request.method.clone(),
            params: request.params.clone(),
        });
        Some(request)
    }

    pub fn send(&mut self, value: Value) {
        self.send_raw(&format!("{value}\n"));
    }

    pub fn send_raw(&mut self, text: &str) {
        let _ = self.stream.write_all(text.as_bytes());
    }

    pub fn answer(&mut self, request: &Request, result: Value) {
        self.send_raw(&encode_line(&Response::success(request.id.clone(), result)));
    }

    /// Reads `initialize` and answers it as the daemon does.
    pub fn initialize(&mut self) {
        let request = self.read().expect("the client initializes");
        assert_eq!(request.method, "initialize");
        let result = OwnerInitializeResult {
            protocol: OWNER_PROTOCOL,
            daemon_version: "fake".to_string(),
            actor: Caller::App.actor(),
        };
        self.answer(&request, serde_json::to_value(result).unwrap());
    }

    /// Reads `daemon.recapture_environment` and answers it as the daemon does.
    pub fn answer_recapture(&mut self) {
        let request = self.read().expect("the environment is asked for");
        assert_eq!(request.method, "daemon.recapture_environment");
        self.answer(
            &request,
            json!({"capture": {"outcome": "captured", "shell": "s", "variables": 1}}),
        );
    }

    /// Answers `daemon.subscribe` with a log whose head is `head`.
    pub fn answer_subscribe(&mut self, head: u64) -> Option<Request> {
        let request = self.read()?;
        assert_eq!(request.method, "daemon.subscribe");
        self.answer(
            &request,
            json!({"head_seq": head, "oldest_seq": 1, "replayed": 0}),
        );
        Some(request)
    }

    /// A `daemon.event` notice with this `seq`.
    pub fn event(&mut self, seq: u64) {
        self.send(event_notice(seq));
    }
}

pub fn event_notice(seq: u64) -> Value {
    json!({"jsonrpc": "2.0", "method": "daemon.event", "params": {"seq": seq, "marker": "fake"}})
}

/// A daemon made of a listener and a script that handles each connection, one at a time, in the order they arrive. It
/// takes any token. A script that returns closes its connection.
pub struct FakeDaemon {
    pub seen: Arc<Mutex<Vec<Seen>>>,
    /// How many connections the script has finished with, which closes each of them.
    pub finished: Arc<AtomicUsize>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
    socket: PathBuf,
}

impl FakeDaemon {
    pub fn start(root: &TempRoot, script: impl Fn(&mut Conn) + Send + 'static) -> FakeDaemon {
        let paths = root.paths();
        // The folder and the token file are made as the daemon makes them.
        let prepared = auth::prepare_daemon_socket(&auth::StdFs, auth::daemon_uid(), &paths);
        prepared.expect("the run folder");
        if !paths.token_file.exists() {
            auth::create_token_file(&paths.token_file, "fake-daemon-token")
                .expect("the token file");
        }
        let _ = fs::remove_file(&paths.socket);
        let listener = UnixListener::bind(&paths.socket).expect("bind the fake daemon's socket");
        listener.set_nonblocking(true).unwrap();
        let seen: Arc<Mutex<Vec<Seen>>> = Arc::default();
        let stop = Arc::new(AtomicBool::new(false));
        let finished = Arc::new(AtomicUsize::new(0));
        let thread = {
            let (seen, stop, finished) = (seen.clone(), stop.clone(), finished.clone());
            std::thread::spawn(move || {
                let mut index = 0;
                while !stop.load(Ordering::SeqCst) {
                    match listener.accept() {
                        Ok((stream, _)) => {
                            stream.set_nonblocking(false).unwrap();
                            stream
                                .set_read_timeout(Some(Duration::from_secs(10)))
                                .unwrap();
                            let reader = BufReader::new(stream.try_clone().unwrap());
                            let mut conn = Conn {
                                stream,
                                reader,
                                seen: seen.clone(),
                                index,
                            };
                            index += 1;
                            script(&mut conn);
                            drop(conn);
                            finished.fetch_add(1, Ordering::SeqCst);
                        }
                        Err(_) => std::thread::sleep(Duration::from_millis(10)),
                    }
                }
            })
        };
        FakeDaemon {
            seen,
            finished,
            stop,
            thread: Some(thread),
            socket: paths.socket,
        }
    }

    /// Every request of one method, in the order they came.
    pub fn requests(&self, method: &str) -> Vec<Seen> {
        self.seen
            .lock()
            .unwrap()
            .iter()
            .filter(|seen| seen.method == method)
            .cloned()
            .collect()
    }

    pub fn methods(&self) -> Vec<(usize, String)> {
        self.seen
            .lock()
            .unwrap()
            .iter()
            .map(|seen| (seen.connection, seen.method.clone()))
            .collect()
    }
}

impl Drop for FakeDaemon {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        let _ = fs::remove_file(&self.socket);
    }
}

// ---- the pump, on a thread of its own ---------------------------------------------------------------------------

/// `pump_daemon_events` on a thread. `Drop` tells it to stop and waits for it, so a test that fails does not leave it
/// running.
pub struct PumpRun {
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl PumpRun {
    pub fn start(access: Arc<DaemonAccess>, channel: Channel, retry_wait: Duration) -> PumpRun {
        let stop = Arc::new(AtomicBool::new(false));
        let thread = {
            let stop = stop.clone();
            std::thread::spawn(move || pump_daemon_events(&access, &channel, &stop, retry_wait))
        };
        PumpRun {
            stop,
            thread: Some(thread),
        }
    }

    /// Tells the pump to stop and waits for it.
    pub fn stop(mut self) {
        self.end();
    }

    fn end(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            thread.join().expect("the pump does not panic");
        }
    }
}

impl Drop for PumpRun {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
