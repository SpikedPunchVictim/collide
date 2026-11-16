//! `collide-bridge` (ts-0082-fp), the test-only bridge between a browser and a daemon.
//!
//! ```text
//! browser ── POST /rpc {method, params} ──▶ Shell ─────▶ SocketLink ──▶ collide-daemon (a child, in the root)
//! browser ◀── GET /events (server-sent) ─── Hub ◀── pump_daemon_events ◀─┘
//! browser ◀── GET / and the page's files ─── the built page (`bridge.html`)
//! ```
//!
//! It hosts the shell's own [`Shell`], [`SocketLink`] and event pump (`app/src-tauri/src/shell.rs`), so a call from the
//! page takes the path a call from the webview takes, and the daemon is started on demand as the app's is. It is built
//! only with the feature `test-bridge`, which only the browser tests' build turns on, and nothing of it is in the app.
//!
//! It listens on the loopback only, on a port the system picks. It refuses a foreign `Host` or `Origin`, a call
//! without the token it made for this run, and a body that is not JSON (`http.rs`). It prints one line when it listens,
//! `listening <port> <token>`, and ends on `POST /shutdown` or when its `--parent` process is gone: it stops the runs
//! that are live, then the daemon, so no process of the test is left behind.
//!
//! ```text
//! collide-bridge --root <dir> --shell <login shell> --static <built page> [--parent <pid>]
//! ```

mod daemon;
mod http;

use collide_app::shell::{
    ChannelEvent, ChannelWindow, DaemonAccess, EventChannel, Shell, ShellHandle, SocketLink,
    pump_daemon_events,
};
use collide_core::ui_tools::ToolManifest;
use collide_host::clock::SystemClocks;
use collide_host::daemon::DaemonPaths;
use collide_host::ui_tools::{EndpointTokens, UiToolsEndpoint};
use daemon::ChildDaemon;
use http::{Policy, Refusal, Request};
use serde::Deserialize;
use serde_json::{Value, json};
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Command, ExitCode};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

/// How long the event pump waits before it connects again when the stream ends.
const EVENT_RETRY: Duration = Duration::from_millis(200);

/// How often a quiet event stream writes a comment, so that a closed browser is noticed.
const KEEP_ALIVE: Duration = Duration::from_secs(2);

/// How often the bridge looks for its parent.
const PARENT_CHECK: Duration = Duration::from_millis(500);

/// How long a connection may take to send its request.
const READ_WAIT: Duration = Duration::from_secs(10);

fn fail(message: &str) -> ExitCode {
    eprintln!("collide-bridge: {message}");
    ExitCode::from(1)
}

struct Args {
    root: PathBuf,
    shell: PathBuf,
    page: PathBuf,
    parent: Option<String>,
}

fn parse_args() -> Result<Args, String> {
    let mut values = std::collections::BTreeMap::new();
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        let name = flag
            .strip_prefix("--")
            .ok_or_else(|| format!("unknown argument `{flag}`"))?
            .to_string();
        let value = args
            .next()
            .ok_or_else(|| format!("`{flag}` needs a value"))?;
        values.insert(name, value);
    }
    let mut take = |name: &str| {
        values
            .remove(name)
            .ok_or_else(|| format!("`--{name}` is needed"))
    };
    let args = Args {
        root: take("root")?.into(),
        shell: take("shell")?.into(),
        page: take("static")?.into(),
        parent: take("parent").ok(),
    };
    if let Some(other) = values.keys().next() {
        return Err(format!("unknown argument `--{other}`"));
    }
    Ok(args)
}

/// 16 random bytes as hex, from the system.
fn make_token() -> Result<String, String> {
    let mut bytes = [0u8; 16];
    fs::File::open("/dev/urandom")
        .and_then(|mut random| random.read_exact(&mut bytes))
        .map_err(|error| format!("could not read a random token: {error}"))?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

/// The endpoint token of the UI-tools endpoint, kept in memory. The bridge never touches the keychain.
struct MemoryToken(String);

impl EndpointTokens for MemoryToken {
    fn current(&self) -> String {
        self.0.clone()
    }

    fn rotate(&mut self) -> String {
        self.0.clone()
    }
}

/// The event channel of the bridge: what the shell and the pump emit goes to each page that listens, as a line of JSON.
#[derive(Default)]
struct Hub {
    listeners: Mutex<Vec<mpsc::Sender<String>>>,
}

impl Hub {
    fn listen(&self) -> mpsc::Receiver<String> {
        let (send, receive) = mpsc::channel();
        self.listeners
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(send);
        receive
    }
}

impl EventChannel for Hub {
    fn is_open(&self) -> bool {
        true
    }

    fn emit(&self, event: &ChannelEvent) {
        let line = serde_json::to_string(event).expect("an event is JSON");
        let mut listeners = self.listeners.lock().unwrap_or_else(|e| e.into_inner());
        listeners.retain(|listener| listener.send(line.clone()).is_ok());
    }
}

struct Bridge {
    shell: ShellHandle,
    hub: Arc<Hub>,
    policy: Policy,
    page: PathBuf,
    daemon: Arc<ChildDaemon>,
    stop_pump: Arc<AtomicBool>,
}

/// What a request comes to.
enum Outcome {
    Reply(Vec<u8>, &'static str),
    Events,
    Shutdown,
}

#[derive(Deserialize)]
struct RpcBody {
    method: String,
    #[serde(default)]
    params: Option<Value>,
}

impl Bridge {
    fn route(&self, request: &Request) -> Result<Outcome, Refusal> {
        self.policy.check_origin(request)?;
        match (request.method.as_str(), request.path.as_str()) {
            ("OPTIONS", _) => Err(Refusal::Forbidden),
            ("GET", "/events") => {
                self.policy.check_token(request)?;
                Ok(Outcome::Events)
            }
            ("POST", "/rpc") => {
                self.policy.check_token(request)?;
                http::check_json(request)?;
                let body: RpcBody =
                    serde_json::from_slice(&request.body).map_err(|_| Refusal::BadRequest)?;
                let answer = match self.shell.rpc(body.method, body.params) {
                    Ok(result) => json!({ "result": result }),
                    Err(error) => json!({ "error": error }),
                };
                Ok(Outcome::Reply(
                    answer.to_string().into_bytes(),
                    "application/json",
                ))
            }
            ("POST", "/shutdown") => {
                self.policy.check_token(request)?;
                http::check_json(request)?;
                Ok(Outcome::Shutdown)
            }
            ("GET", path) => {
                let file = http::file_for(&self.page, path).ok_or(Refusal::NotFound)?;
                let bytes = fs::read(&file).map_err(|_| Refusal::NotFound)?;
                Ok(Outcome::Reply(bytes, http::content_type_of(&file)))
            }
            _ => Err(Refusal::MethodNotAllowed),
        }
    }

    fn serve(&self, mut stream: TcpStream) {
        let _ = stream.set_read_timeout(Some(READ_WAIT));
        let outcome = http::read_request(&mut stream).and_then(|request| self.route(&request));
        match outcome {
            Ok(Outcome::Reply(body, content_type)) => {
                let _ = http::write_response(&mut stream, (200, "OK"), content_type, &body);
            }
            Ok(Outcome::Events) => self.stream_events(stream),
            Ok(Outcome::Shutdown) => {
                let _ = http::write_response(&mut stream, (200, "OK"), "application/json", b"{}");
                drop(stream);
                self.finish();
            }
            Err(refusal) => {
                let status = refusal.status();
                let _ =
                    http::write_response(&mut stream, status, "text/plain", status.1.as_bytes());
            }
        }
    }

    /// Server-sent events: one `data:` line for each event, until the page goes away. The first bytes are written
    /// after the page is a listener, so a page that has its headers misses nothing that comes after.
    fn stream_events(&self, mut stream: TcpStream) {
        let events = self.hub.listen();
        let head = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-store\r\n\
                    Connection: close\r\n\r\n: open\n\n";
        if stream
            .write_all(head.as_bytes())
            .and_then(|()| stream.flush())
            .is_err()
        {
            return;
        }
        loop {
            let chunk = match events.recv_timeout(KEEP_ALIVE) {
                Ok(line) => format!("data: {line}\n\n"),
                Err(RecvTimeoutError::Timeout) => ": keep-alive\n\n".to_string(),
                Err(RecvTimeoutError::Disconnected) => return,
            };
            if stream
                .write_all(chunk.as_bytes())
                .and_then(|()| stream.flush())
                .is_err()
            {
                return;
            }
        }
    }

    /// Stops the runs that are live (their shims and agents outlive a daemon that is killed), then the daemon, and
    /// ends the process.
    fn finish(&self) -> ! {
        self.stop_pump.store(true, Ordering::SeqCst);
        if self.daemon.answers() {
            self.stop_live_runs();
        }
        self.daemon.retire();
        self.daemon.end();
        std::process::exit(0);
    }

    fn stop_live_runs(&self) {
        let Ok(listed) = self
            .shell
            .rpc("run.list".into(), Some(json!({ "scope": "live" })))
        else {
            return;
        };
        let runs = listed["runs"].as_array().cloned().unwrap_or_default();
        for run in runs.iter().filter_map(|run| run["id"].as_str()) {
            let params = json!({ "run": run, "grace_secs": 1 });
            if let Err(error) = self.shell.rpc("run.stop".into(), Some(params)) {
                eprintln!(
                    "collide-bridge: could not stop run {run}: {}",
                    error.message
                );
            }
        }
    }
}

fn parent_alive(pid: &str) -> bool {
    Command::new("/bin/kill")
        .args(["-0", pid])
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(args) => args,
        Err(message) => return fail(&message),
    };
    let token = match make_token() {
        Ok(token) => token,
        Err(message) => return fail(&message),
    };
    let listener = match TcpListener::bind("127.0.0.1:0") {
        Ok(listener) => listener,
        Err(error) => return fail(&format!("could not listen on the loopback: {error}")),
    };
    let port = listener.local_addr().map(|addr| addr.port()).unwrap_or(0);

    let exe = match std::env::current_exe().and_then(fs::canonicalize) {
        Ok(exe) => exe,
        Err(error) => return fail(&format!("could not find its own executable: {error}")),
    };
    let Some(beside) = exe.parent() else {
        return fail("its executable has no folder");
    };
    let daemon = Arc::new(ChildDaemon::new(
        beside.join("collide-daemon"),
        &args.root,
        args.shell,
    ));
    let access = Arc::new(DaemonAccess::new(
        DaemonPaths::under(&args.root),
        Box::new(Starter(Arc::clone(&daemon))),
        Arc::new(SystemClocks),
        exe,
    ));
    let hub = Arc::new(Hub::default());
    let stop_pump = Arc::new(AtomicBool::new(false));

    let shell_access = Arc::clone(&access);
    let shell_hub = Arc::clone(&hub);
    let shell_token = token.clone();
    let shell = ShellHandle::spawn(move || {
        // The manifest is empty, as the app's is until Stage 6 writes the real one.
        let endpoint = UiToolsEndpoint::new(
            0,
            ToolManifest(Vec::new()),
            Box::new(MemoryToken(shell_token)),
            Box::new(ChannelWindow::new(shell_hub)),
            Box::new(SystemClocks),
        )
        .expect("an empty manifest is valid");
        Shell::new(endpoint, Box::new(SocketLink::new(shell_access)))
    });

    let pump_hub = Arc::clone(&hub);
    let pump_stop = Arc::clone(&stop_pump);
    let spawned = thread::Builder::new()
        .name("collide-events".into())
        .spawn(move || pump_daemon_events(&access, pump_hub.as_ref(), &pump_stop, EVENT_RETRY));
    if let Err(error) = spawned {
        return fail(&format!("could not start the event pump: {error}"));
    }

    let bridge = Arc::new(Bridge {
        shell,
        hub,
        policy: Policy::new(port, token.clone()),
        page: args.page,
        daemon,
        stop_pump,
    });
    if let Some(parent) = args.parent {
        let bridge = Arc::clone(&bridge);
        thread::spawn(move || {
            while parent_alive(&parent) {
                thread::sleep(PARENT_CHECK);
            }
            bridge.finish();
        });
    }

    println!("listening {port} {token}");
    let _ = std::io::stdout().flush();
    for stream in listener.incoming().flatten() {
        let bridge = Arc::clone(&bridge);
        thread::spawn(move || bridge.serve(stream));
    }
    ExitCode::SUCCESS
}

/// The starter the shell is given: the bridge's own child daemon.
struct Starter(Arc<ChildDaemon>);

impl collide_app::shell::DaemonStarter for Starter {
    fn ensure_running(&self) -> Result<(), String> {
        self.0.ensure_running()
    }
}
