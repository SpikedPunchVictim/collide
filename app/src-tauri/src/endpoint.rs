//! The listener of the UI-tools endpoint (0.2, The UI-tools endpoint; ts-0199-xz): a std [`TcpListener`] on 127.0.0.1,
//! one thread for each connection and one request for each connection. It reads the request with [`crate::http`] (the
//! reader the test bridge shares), hands it to the endpoint on the shell's thread ([`ShellHandle::http`]), and writes
//! what comes back. The guards, the tokens and the 10 s wait are the endpoint's (`host/src/ui_tools.rs`); this file
//! only carries bytes.
//!
//! ```text
//! client ── HTTP ──▶ EndpointServer ── ShellHandle::http ──▶ Shell ──▶ UiToolsEndpoint::handle
//!        ◀── HTTP ──                 ◀── one HttpResponse ──        (a tools/call waits for the window)
//! ```
//!
//! ## The port
//!
//! The port is picked once and kept in the user's settings, `ui_tools.port` (0.2). The first start binds port 0, so
//! the system picks, and writes the port it got through `config.set`. A later start binds that port. A port that is in
//! use fails the start with a message that names it, and the shell goes on without the endpoint. [`PortStore`] is
//! where the port is kept: [`DaemonPortStore`] in the app, which reads and writes through the daemon's `config.get`
//! and `config.set`, and [`MemoryPortStore`] for the bridge and for tests.
//!
//! ## Nothing here holds a token
//!
//! A request is passed on whole and its answer is written whole. No line this file logs holds a request, so none holds
//! the endpoint token that a client sent in its header or its path.

use crate::http::{self, ReadError, Request};
use crate::shell::{DaemonAccess, ShellHandle};
use collide_core::owner::{
    ConfigGetParams, ConfigGetResult, ConfigSetParams, ConfigSetResult, ConfigWriteLayer, Response,
    RpcError, code,
};
use collide_host::owner_client::ClientError;
use collide_host::ui_tools::{HttpRequest, HttpResponse, MAX_BODY_BYTES};
use serde_json::json;
use std::io::Read;
use std::net::{Ipv4Addr, Shutdown, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

/// The key of the port in the user's settings.
pub const PORT_KEY: &str = "ui_tools.port";

/// How long a connection may take to send its request, and to take its answer.
const IO_WAIT: Duration = Duration::from_secs(10);

/// The most a refused connection is read after its answer, in bytes.
const DRAIN_LIMIT: usize = 8 * 1024 * 1024;

/// How long the app waits for the daemon's `config.get` and `config.set`.
const SETTINGS_WAIT: Duration = Duration::from_secs(30);

/// Where a line goes: the app's is stderr.
pub type Log = Arc<dyn Fn(&str) + Send + Sync>;

/// The log of the app: one line on stderr.
pub fn stderr_log() -> Log {
    Arc::new(|line| eprintln!("{line}"))
}

// ---- where the port is kept -------------------------------------------------------------------------------------

/// Where the port is kept between starts.
pub trait PortStore {
    /// The kept port, or `None` when none has been kept.
    fn saved(&self) -> Result<Option<u16>, String>;
    /// Keeps `port`.
    fn save(&self, port: u16) -> Result<(), String>;
}

/// A port kept in memory: the bridge's, which picks a new one for each run, and a test's.
#[derive(Default)]
pub struct MemoryPortStore(Mutex<Option<u16>>);

impl MemoryPortStore {
    /// A store that holds `port`, as one does after a first start.
    pub fn holding(port: u16) -> Self {
        MemoryPortStore(Mutex::new(Some(port)))
    }
}

impl PortStore for MemoryPortStore {
    fn saved(&self) -> Result<Option<u16>, String> {
        Ok(*self.0.lock().unwrap_or_else(|e| e.into_inner()))
    }

    fn save(&self, port: u16) -> Result<(), String> {
        *self.0.lock().unwrap_or_else(|e| e.into_inner()) = Some(port);
        Ok(())
    }
}

/// The port in the user's settings, through the daemon: `config.get` and `config.set` of [`PORT_KEY`], in the user's
/// file. The daemon is started on demand, as for every call of the shell.
pub struct DaemonPortStore {
    access: Arc<DaemonAccess>,
}

impl DaemonPortStore {
    pub fn new(access: Arc<DaemonAccess>) -> Self {
        DaemonPortStore { access }
    }
}

fn why(error: ClientError) -> String {
    match error {
        ClientError::Rpc(error) => error.message,
        other => other.to_string(),
    }
}

impl PortStore for DaemonPortStore {
    fn saved(&self) -> Result<Option<u16>, String> {
        let mut client = self.access.connect().map_err(|error| error.message)?;
        let params = ConfigGetParams {
            board: None,
            key: Some(PORT_KEY.to_string()),
            profile: None,
            with_problems: true,
        };
        let result: ConfigGetResult = client
            .call("config.get", &params, Some(SETTINGS_WAIT))
            .map_err(why)?;
        let Some(value) = result
            .entries
            .into_iter()
            .find(|entry| entry.key == PORT_KEY)
            .and_then(|entry| entry.value)
        else {
            return Ok(None);
        };
        value
            .as_u64()
            .and_then(|number| u16::try_from(number).ok())
            .map(Some)
            .ok_or_else(|| format!("`{PORT_KEY}` is not a port number"))
    }

    fn save(&self, port: u16) -> Result<(), String> {
        let mut client = self.access.connect().map_err(|error| error.message)?;
        let params = ConfigSetParams {
            board: None,
            key: PORT_KEY.to_string(),
            value: json!(port),
            layer: ConfigWriteLayer::User,
            secret: false,
        };
        let _: ConfigSetResult = client
            .call("config.set", &params, Some(SETTINGS_WAIT))
            .map_err(why)?;
        Ok(())
    }
}

/// Binds the loopback port the store keeps, or, with none kept, a port the system picks, which is then kept.
fn bind(ports: &dyn PortStore) -> Result<(TcpListener, u16), String> {
    let saved = ports
        .saved()
        .map_err(|why| format!("the UI-tools endpoint could not read `{PORT_KEY}`: {why}"))?;
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, saved.unwrap_or(0))).map_err(|error| {
        match saved {
            Some(port) => format!(
                "the UI-tools endpoint cannot listen on port {port}: {error}. Something else may hold it. Stop that, \
                 or set `{PORT_KEY}` to a free port"
            ),
            None => format!("the UI-tools endpoint cannot listen on the loopback: {error}"),
        }
    })?;
    let port = listener
        .local_addr()
        .map_err(|error| format!("the UI-tools endpoint has no address: {error}"))?
        .port();
    if saved.is_none() {
        ports.save(port).map_err(|why| {
            format!("the UI-tools endpoint listens on port {port}, but could not keep it in `{PORT_KEY}`: {why}")
        })?;
    }
    Ok((listener, port))
}

// ---- the server -------------------------------------------------------------------------------------------------

/// A running listener. Dropping it stops it, and waits for the thread that accepts.
pub struct EndpointServer {
    port: u16,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl EndpointServer {
    /// Binds the port ([`PortStore`]), tells the endpoint which port it has, and starts accepting. The error says what
    /// failed, and names the port when it is in use.
    pub fn start(
        ports: &dyn PortStore,
        shell: Arc<ShellHandle>,
        log: Log,
    ) -> Result<EndpointServer, String> {
        let (listener, port) = bind(ports)?;
        shell.set_endpoint_port(port).map_err(|error| {
            format!(
                "the UI-tools endpoint could not be set up: {}",
                error.message
            )
        })?;
        let stop = Arc::new(AtomicBool::new(false));
        let accepting = {
            let stop = Arc::clone(&stop);
            thread::Builder::new()
                .name("collide-endpoint".into())
                .spawn(move || accept(&listener, &stop, &shell, &log))
                .map_err(|error| format!("the UI-tools endpoint could not start: {error}"))?
        };
        Ok(EndpointServer {
            port,
            stop,
            thread: Some(accepting),
        })
    }

    /// The port it listens on, on 127.0.0.1.
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Waits for the thread that accepts, which ends only when the server is dropped: the app calls it and never
    /// returns.
    pub fn wait(mut self) {
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for EndpointServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        // `accept` blocks, so a connection to ourselves wakes it to look at the flag.
        let _ = TcpStream::connect_timeout(
            &SocketAddr::from((Ipv4Addr::LOCALHOST, self.port)),
            IO_WAIT,
        );
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn accept(listener: &TcpListener, stop: &AtomicBool, shell: &Arc<ShellHandle>, log: &Log) {
    for stream in listener.incoming() {
        if stop.load(Ordering::SeqCst) {
            return;
        }
        let Ok(stream) = stream else { continue };
        let shell = Arc::clone(shell);
        let spawned = thread::Builder::new()
            .name("collide-endpoint-call".into())
            .spawn(move || serve(stream, &shell));
        if let Err(error) = spawned {
            log(&format!(
                "collide: the UI-tools endpoint could not serve a connection: {error}"
            ));
        }
    }
}

/// One connection: read the request, ask the endpoint, write the answer.
fn serve(mut stream: TcpStream, shell: &ShellHandle) {
    let _ = stream.set_read_timeout(Some(IO_WAIT));
    let _ = stream.set_write_timeout(Some(IO_WAIT));
    // One byte more than the endpoint allows, so a body just over its limit is read, and the endpoint says why it is
    // refused.
    let (answer, unread) = match http::read_request(&mut stream, MAX_BODY_BYTES + 1) {
        Ok(request) => (shell.http(endpoint_request(request)), false),
        Err(ReadError::TooLarge) => (refusal(413, "the body is over the 1 MB limit"), true),
        Err(ReadError::Malformed) => (
            refusal(400, "the request is not HTTP/1.1 that this endpoint reads"),
            true,
        ),
    };
    let _ = write_answer(&mut stream, &answer);
    if unread {
        drain(&mut stream);
    }
}

/// Reads and drops what the client still sends, up to [`DRAIN_LIMIT`]. A socket closed with unread data resets the
/// connection, and the reset can erase the answer before the client has read it.
fn drain(stream: &mut TcpStream) {
    let _ = stream.shutdown(Shutdown::Write);
    let mut sink = [0u8; 16 * 1024];
    let mut drained = 0;
    while drained < DRAIN_LIMIT {
        match stream.read(&mut sink) {
            Ok(0) | Err(_) => return,
            Ok(n) => drained += n,
        }
    }
}

fn endpoint_request(request: Request) -> HttpRequest {
    HttpRequest {
        method: request.method,
        path: request.path,
        headers: request.headers,
        body: request.body,
    }
}

/// An answer for a request the endpoint never saw, in the shape of its own refusals.
fn refusal(status: u16, message: &str) -> HttpResponse {
    let error = RpcError::new(code::INVALID_REQUEST, message);
    HttpResponse {
        status,
        headers: vec![("Content-Type".to_string(), "application/json".to_string())],
        body: serde_json::to_vec(&Response::failure(None, error)).expect("an error is JSON"),
    }
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        202 => "Accepted",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        413 => "Payload Too Large",
        415 => "Unsupported Media Type",
        503 => "Service Unavailable",
        _ => "Status",
    }
}

fn write_answer(stream: &mut TcpStream, answer: &HttpResponse) -> std::io::Result<()> {
    let content_type = answer
        .headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("content-type"))
        .map_or("application/json", |(_, value)| value.as_str());
    let extra: Vec<(&str, &str)> = answer
        .headers
        .iter()
        .filter(|(name, _)| !name.eq_ignore_ascii_case("content-type"))
        .map(|(name, value)| (name.as_str(), value.as_str()))
        .collect();
    http::write_response(
        stream,
        (answer.status, reason(answer.status)),
        content_type,
        &extra,
        &answer.body,
    )
}
