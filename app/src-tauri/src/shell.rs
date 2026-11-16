//! The shell's `rpc` command and event channel (0.2, The UI speaks the owner's protocol; ts-0017-qb), as plain code with
//! no Tauri type, so a test needs no runtime. [`crate::run`] wires it to the webview.
//!
//! ```text
//! webview ── rpc(method, params) ──▶ Shell ─ ui_tools.* ─▶ UiToolsEndpoint::rpc   (answered here, never forwarded)
//!                                          └ the rest ────▶ DaemonLink::call       (adds the per-client token)
//! webview ◀── one event channel ──── EventChannel ◀─ daemon notices, ui_tools.request
//! ```
//!
//! ## What the shell decides
//!
//! 1. `initialize` is the shell's: it carries the per-client token, so the window cannot send it. The link makes the
//!    handshake when it connects, with the token it read itself.
//! 2. A `ui_tools.*` method goes to [`UiToolsEndpoint::rpc`] and is never forwarded, whatever it answers.
//! 3. `ask.start` and `ask.test` are read in the window's shape ([`collide_core::ask::AskStartParams`]), which has no
//!    token field, so a window that sends one is refused. The shell then makes the daemon's shape, with the endpoint and
//!    the ask's token, which is Stage 6b's (`shell: ask token`). `ask.cancel` has one shape and goes on as it is.
//! 4. Everything else is forwarded, and the daemon says if the method is unknown.
//!
//! ## One thread owns the shell
//!
//! The endpoint holds boxed traits that are not `Send`, so [`ShellHandle`] runs the [`Shell`] on a thread of its own and
//! takes calls over a channel. A call from the webview waits for its answer.
//!
//! ## The way to the daemon
//!
//! [`DaemonAccess`] holds the paths, the start on demand and the rule that ends it. It makes each connection: it tells
//! the daemon where the app is, starts the daemon when nothing answers, and does the handshake with the token it read
//! from the token file. [`SocketLink`] keeps one such connection for the window's calls. [`pump_daemon_events`] has a
//! connection of its own for `daemon.subscribe`, so a call never waits behind a stream.
//!
//! One tag is still a stub: `shell: ask token` ([`Shell`]'s ask forwarding), the ask's token and endpoint added to
//! `ask.start` and `ask.test` (Stage 6b).

use collide_core::ask::{AskCancelParams, AskMethod, AskStartParams, AskTestParams};
use collide_core::owner::{
    DaemonSubscribeParams, DaemonSubscribeResult, JsonRpc2, Notification, Request, RequestId,
    Response, RpcError, code,
};
use collide_core::ui_tools::REQUEST_EVENT;
use collide_core::ui_tools::UiToolsRequest;
use collide_host::auth;
use collide_host::clock::{Clock, Clocks};
use collide_host::daemon::DaemonPaths;
use collide_host::launch;
use collide_host::owner_client::{ClientError, Incoming, OwnerClient};
use collide_host::ui_tools::{Completed, Rpc, UiToolsEndpoint, Window};
use serde::Serialize;
use serde_json::Value;
use std::io;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

/// What the webview hears on the one event channel: a notice's `method` and `params`, the shape of `OwnerNotice` and
/// `AskNotice` in the generated types, plus the shell's own `ui_tools.request`. It has no envelope and no token.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ChannelEvent {
    pub method: String,
    pub params: Value,
}

impl From<Notification> for ChannelEvent {
    fn from(notification: Notification) -> Self {
        ChannelEvent {
            method: notification.method,
            params: notification.params,
        }
    }
}

/// The event channel, as the shell sees it. The real one is the webview's ([`crate::run`]).
pub trait EventChannel: Send + Sync {
    /// Whether a window is there to hear.
    fn is_open(&self) -> bool;
    fn emit(&self, event: &ChannelEvent);
}

/// The window of the UI-tools endpoint, over the event channel: an agent's `tools/call` is a `ui_tools.request` event.
pub struct ChannelWindow {
    channel: Arc<dyn EventChannel>,
}

impl ChannelWindow {
    pub fn new(channel: Arc<dyn EventChannel>) -> Self {
        ChannelWindow { channel }
    }
}

impl Window for ChannelWindow {
    fn is_open(&self) -> bool {
        self.channel.is_open()
    }

    fn send(&mut self, event: UiToolsRequest) {
        let params = serde_json::to_value(event).expect("a ui_tools.request is JSON");
        self.channel.emit(&ChannelEvent {
            method: REQUEST_EVENT.to_string(),
            params,
        });
    }
}

/// The way to the daemon. It owns the per-client token: a request goes out with the connection's handshake already
/// made, and nothing in a request carries the token.
pub trait DaemonLink {
    fn call(&mut self, request: &Request) -> Response;
}

/// Starts the daemon when its socket does not answer (0.2, Starting the daemon).
pub trait DaemonStarter: Send + Sync {
    /// Makes sure a daemon answers on the socket. The error says why it could not.
    fn ensure_running(&self) -> Result<(), String>;
}

/// The real start: the `collide-daemon` beside this executable, started as the CLI starts it, so the app and the CLI
/// start and connect the same way ([`launch::ensure_running_from_cli`]).
pub struct LaunchStarter {
    paths: DaemonPaths,
}

impl LaunchStarter {
    pub fn new(paths: DaemonPaths) -> Self {
        LaunchStarter { paths }
    }
}

impl DaemonStarter for LaunchStarter {
    fn ensure_running(&self) -> Result<(), String> {
        launch::ensure_running_from_cli(&self.paths)
    }
}

/// The name a connection of the app gives the daemon, for its log.
const CLIENT_NAME: &str = "collide-app";

/// How long a call waits for the daemon's answer before the connection is closed. A stop with a long grace period is
/// the
/// longest call, and it is under this. Without a limit a daemon that hangs would hang every call, because one thread
/// makes them all.
const CALL_WAIT: Duration = Duration::from_secs(120);

/// A failed start of the daemon: when (on the continuous clock) and why.
struct FailedStart {
    at_nanos: u64,
    reason: String,
}

/// How the shell reaches the daemon, for the calls of the window and for the event pump: the files, the start on demand
/// and the rule that ends it. Each connection it makes is its own, so the two never share one.
pub struct DaemonAccess {
    paths: DaemonPaths,
    starter: Box<dyn DaemonStarter>,
    clocks: Arc<dyn Clocks + Send + Sync>,
    /// The path of the app's executable, which the daemon is told so it can take a connection from here as the app.
    app_executable: PathBuf,
    policy: launch::RestartPolicy,
    failed_starts: Mutex<Vec<FailedStart>>,
}

impl DaemonAccess {
    pub fn new(
        paths: DaemonPaths,
        starter: Box<dyn DaemonStarter>,
        clocks: Arc<dyn Clocks + Send + Sync>,
        app_executable: PathBuf,
    ) -> Self {
        DaemonAccess {
            paths,
            starter,
            clocks,
            app_executable,
            policy: launch::RestartPolicy::default(),
            failed_starts: Mutex::new(Vec::new()),
        }
    }

    /// A new connection that has done `initialize`, with the token read from the token file (which is refused unless it
    /// is
    /// mode `0600`). It tells the daemon where the app is first, then makes sure a daemon answers, and starts one when
    /// none
    /// does, unless [`launch::RestartPolicy`] says the app has failed to start one too often lately: then it only
    /// connects, so a daemon someone else started is still used.
    pub fn connect(&self) -> Result<OwnerClient, RpcError> {
        auth::write_app_executable(&self.paths, &self.app_executable).map_err(|error| {
            unavailable(format!(
                "could not tell the daemon where the app is: {error}"
            ))
        })?;
        if self.stopped_trying().is_none() {
            self.starter
                .ensure_running()
                .map_err(|reason| self.start_failed(reason))?;
        }
        OwnerClient::connect(&self.paths, CLIENT_NAME).map_err(|error| {
            unavailable(
                self.stopped_trying()
                    .unwrap_or_else(|| format!("could not connect to the daemon: {error}")),
            )
        })
    }

    /// The failed starts of the last [`launch::RestartPolicy::window`], with the older ones dropped.
    fn recent_failures(&self) -> std::sync::MutexGuard<'_, Vec<FailedStart>> {
        let now = self.clocks.read(Clock::Continuous).nanos;
        let window = self.policy.window.as_nanos() as u64;
        let mut failed = self.failed_starts.lock().unwrap_or_else(|e| e.into_inner());
        failed.retain(|start| now.saturating_sub(start.at_nanos) < window);
        failed
    }

    /// What to say when the app has stopped trying to start the daemon, and `None` while it still tries.
    fn stopped_trying(&self) -> Option<String> {
        let failed = self.recent_failures();
        let last = failed.last()?;
        (failed.len() as u32 >= self.policy.max_failed_starts).then(|| {
            format!(
                "the daemon did not start {} times in {} minutes, so the app has stopped trying. The last error: {}. \
                 The daemon's log is {}",
                self.policy.max_failed_starts,
                self.policy.window.as_secs() / 60,
                last.reason,
                self.paths.log_file.display()
            )
        })
    }

    fn start_failed(&self, reason: String) -> RpcError {
        let at_nanos = self.clocks.read(Clock::Continuous).nanos;
        self.recent_failures().push(FailedStart {
            at_nanos,
            reason: reason.clone(),
        });
        unavailable(
            self.stopped_trying()
                .unwrap_or_else(|| format!("the daemon could not be started: {reason}")),
        )
    }
}

fn unavailable(message: String) -> RpcError {
    RpcError::new(code::UNAVAILABLE, message)
}

/// What a call says when the connection failed under it. The request may have been sent, so it is never sent again.
fn dropped(what: &str, error: &ClientError, wait: Duration) -> RpcError {
    let timed_out = matches!(
        error,
        ClientError::Io { source, .. }
            if matches!(source.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut)
    );
    unavailable(if timed_out {
        format!(
            "the daemon did not answer `{what}` within {} s, so the connection was closed. The call was not sent \
             again, and it may still take effect",
            wait.as_secs_f64()
        )
    } else {
        format!(
            "the connection to the daemon dropped during `{what}` ({error}). The call was not sent again, and it \
             may or may not have taken effect"
        )
    })
}

/// The real link: the daemon's socket, with the token read from its file (0.2; `host/src/daemon.rs`). It keeps one
/// connection. A connection the daemon has closed is made again before the next call is sent. One that fails under a
/// call is dropped, and the call fails and says so: it is not sent again, because it may have taken effect.
pub struct SocketLink {
    access: Arc<DaemonAccess>,
    client: Option<OwnerClient>,
    call_wait: Duration,
}

impl SocketLink {
    pub fn new(access: Arc<DaemonAccess>) -> Self {
        SocketLink {
            access,
            client: None,
            call_wait: CALL_WAIT,
        }
    }

    /// How long a call waits for its answer ([`CALL_WAIT`] unless a test sets it).
    pub fn with_call_wait(mut self, wait: Duration) -> Self {
        self.call_wait = wait;
        self
    }

    fn call_value(&mut self, request: &Request) -> Result<Value, RpcError> {
        if self.client.as_mut().is_some_and(OwnerClient::is_closed) {
            self.client = None;
        }
        if self.client.is_none() {
            self.client = Some(self.access.connect()?);
        }
        let client = self.client.as_mut().expect("the connection was just made");
        match client.call::<Value, Value>(&request.method, &request.params, Some(self.call_wait)) {
            Ok(result) => Ok(result),
            Err(ClientError::Rpc(error)) => Err(error),
            Err(error) => {
                self.client = None;
                Err(dropped(&request.method, &error, self.call_wait))
            }
        }
    }
}

impl DaemonLink for SocketLink {
    fn call(&mut self, request: &Request) -> Response {
        match self.call_value(request) {
            Ok(result) => Response::success(request.id.clone(), result),
            Err(error) => Response::failure(Some(request.id.clone()), error),
        }
    }
}

/// How long the pump waits for one notice before it looks at the stop flag.
const PUMP_TICK: Duration = Duration::from_millis(200);

/// How long the pump waits for the answer to `daemon.recapture_environment`, which runs the login shell, and to
/// `daemon.subscribe`.
const PUMP_CALL_WAIT: Duration = Duration::from_secs(60);

/// What the pump has done so far, which outlives each stream.
#[derive(Default)]
struct PumpProgress {
    /// The `seq` of the last event put on the channel, or the head of the log when none has been yet.
    last_seq: Option<u64>,
    asked_for_environment: bool,
}

/// Reads the daemon's notices on a connection of its own and puts each on the channel: the `daemon.event` and
/// `queue.changed` notices of `daemon.subscribe`. The first connection of the app also asks the daemon to capture the
/// login-shell environment again (0.2, The base environment). When the stream ends, because the daemon stopped or
/// because this reader fell behind, the pump waits `retry_wait` and subscribes again from the last `seq` it forwarded,
/// so
/// no event is missing and none is sent twice. It starts the daemon when none answers, as a call does, and returns when
/// `stop` is set.
pub fn pump_daemon_events(
    access: &DaemonAccess,
    channel: &dyn EventChannel,
    stop: &AtomicBool,
    retry_wait: Duration,
) {
    let mut progress = PumpProgress::default();
    let mut told = String::new();
    while !stop.load(Ordering::SeqCst) {
        match follow_daemon(access, channel, stop, &mut progress) {
            Ok(()) => told.clear(),
            // The same trouble is said once, not at each try.
            Err(error) if error.message != told => {
                eprintln!("collide: the event stream: {}", error.message);
                told = error.message;
            }
            Err(_) => {}
        }
        let until = Instant::now() + retry_wait;
        while Instant::now() < until && !stop.load(Ordering::SeqCst) {
            thread::sleep(Duration::from_millis(20).min(retry_wait));
        }
    }
}

/// One connection, from its start to the end of its stream.
fn follow_daemon(
    access: &DaemonAccess,
    channel: &dyn EventChannel,
    stop: &AtomicBool,
    progress: &mut PumpProgress,
) -> Result<(), RpcError> {
    let mut client = access.connect()?;
    if !progress.asked_for_environment {
        progress.asked_for_environment = true;
        let method = "daemon.recapture_environment";
        match client.call::<_, Value>(method, &serde_json::json!({}), Some(PUMP_CALL_WAIT)) {
            Ok(_) => {}
            // The daemon refused. The events do not wait on it.
            Err(ClientError::Rpc(error)) => eprintln!("collide: {method}: {}", error.message),
            Err(error) => return Err(dropped(method, &error, PUMP_CALL_WAIT)),
        }
    }
    let subscribe = DaemonSubscribeParams {
        since: progress.last_seq,
    };
    let method = "daemon.subscribe";
    let result: DaemonSubscribeResult = client
        .call(method, &subscribe, Some(PUMP_CALL_WAIT))
        .map_err(|error| match error {
            ClientError::Rpc(error) => error,
            other => dropped(method, &other, PUMP_CALL_WAIT),
        })?;
    match progress.last_seq {
        None => progress.last_seq = Some(result.head_seq),
        Some(last) if last + 1 < result.oldest_seq => eprintln!(
            "collide: the daemon's log no longer holds events {} to {}; the window's state is behind",
            last + 1,
            result.oldest_seq - 1
        ),
        Some(_) => {}
    }
    // After the daemon has shut the stream down macOS refuses to set an option. Then the reads below give what was
    // sent and the end of the stream, and need no wait.
    let _ = client.set_read_wait(Some(PUMP_TICK));
    while !stop.load(Ordering::SeqCst) {
        match client.next_notice() {
            Ok(Incoming::Notice(notice)) => forward(notice, &mut progress.last_seq, channel),
            Ok(Incoming::Quiet) => {}
            Ok(Incoming::Closed) => return Ok(()),
            Err(error) => return Err(dropped("the event stream", &error, PUMP_TICK)),
        }
    }
    Ok(())
}

/// Puts one notice on the channel as it came. An event that is not newer than the last one forwarded was forwarded
/// already, and is not sent again.
fn forward(notice: Notification, last_seq: &mut Option<u64>, channel: &dyn EventChannel) {
    if notice.method == "daemon.event" {
        match notice.params.get("seq").and_then(Value::as_u64) {
            Some(seq) if last_seq.is_some_and(|last| seq <= last) => return,
            Some(seq) => *last_seq = Some(seq),
            None => eprintln!("collide: a `daemon.event` with no `seq` was forwarded as it came"),
        }
    }
    channel.emit(&ChannelEvent::from(notice));
}

/// The shell. See the module doc.
pub struct Shell {
    endpoint: UiToolsEndpoint,
    link: Box<dyn DaemonLink>,
    next_id: i64,
    completed: Vec<Completed>,
}

impl Shell {
    pub fn new(endpoint: UiToolsEndpoint, link: Box<dyn DaemonLink>) -> Self {
        Shell {
            endpoint,
            link,
            next_id: 0,
            completed: Vec::new(),
        }
    }

    /// The UI-tools endpoint, for the listener of Stage 6b to feed with HTTP requests.
    pub fn endpoint_mut(&mut self) -> &mut UiToolsEndpoint {
        &mut self.endpoint
    }

    /// The agent calls the window's `ui_tools.reply` finished since the last call, for the listener to answer.
    pub fn take_completed(&mut self) -> Vec<Completed> {
        std::mem::take(&mut self.completed)
    }

    /// One call of the webview's `rpc` command.
    pub fn rpc(&mut self, method: &str, params: Option<Value>) -> Result<Value, RpcError> {
        if method == "initialize" {
            return Err(RpcError::method_not_found(method).with_data(serde_json::json!({
                "reason": "the shell makes the handshake; the window does not send `initialize`",
            })));
        }
        self.next_id += 1;
        let request = Request {
            jsonrpc: JsonRpc2,
            id: RequestId::Number(self.next_id),
            method: method.to_string(),
            params: params.unwrap_or(Value::Null),
        };
        match self.endpoint.rpc(&request) {
            Rpc::Handled(handled) => {
                self.completed.extend(handled.completed);
                answer(handled.response)
            }
            Rpc::Forward => {
                let request = self.for_the_daemon(request)?;
                answer(self.link.call(&request))
            }
        }
    }

    /// Checks a request that goes on to the daemon, and makes the daemon's shape of an ask's.
    fn for_the_daemon(&mut self, request: Request) -> Result<Request, RpcError> {
        match AskMethod::from_name(&request.method) {
            None => Ok(request),
            Some(AskMethod::Cancel) => {
                read::<AskCancelParams>(&request)?;
                Ok(request)
            }
            Some(AskMethod::Start) => {
                read::<AskStartParams>(&request)?;
                Ok(self.with_ask_token(request))
            }
            Some(AskMethod::Test) => {
                read::<AskTestParams>(&request)?;
                Ok(self.with_ask_token(request))
            }
        }
    }

    /// Registers the ask with the endpoint, and adds the endpoint's URL and the ask's token to its params.
    fn with_ask_token(&mut self, request: Request) -> Request {
        let _ = (&mut self.endpoint, request);
        todo!("shell: ask token")
    }
}

/// Reads a request's params as the window's type, which refuses a key it does not list.
fn read<T: serde::de::DeserializeOwned>(request: &Request) -> Result<T, RpcError> {
    serde_json::from_value(request.params.clone()).map_err(|error| {
        RpcError::new(
            code::INVALID_PARAMS,
            format!("invalid params for `{}`: {error}", request.method),
        )
    })
}

fn answer(response: Response) -> Result<Value, RpcError> {
    match response {
        Response::Success { result, .. } => Ok(result),
        Response::Failure { error, .. } => Err(error),
    }
}

struct Job {
    method: String,
    params: Option<Value>,
    reply: mpsc::Sender<Result<Value, RpcError>>,
}

/// A [`Shell`] on a thread of its own, callable from any thread.
pub struct ShellHandle {
    jobs: Mutex<mpsc::Sender<Job>>,
}

impl ShellHandle {
    /// Starts the thread, which builds the shell with `make` (so the shell never crosses threads) and answers calls
    /// until the handle is dropped.
    pub fn spawn(make: impl FnOnce() -> Shell + Send + 'static) -> ShellHandle {
        let (jobs, inbox) = mpsc::channel::<Job>();
        thread::Builder::new()
            .name("collide-shell".into())
            .spawn(move || {
                let mut shell = make();
                for job in inbox {
                    let answer = shell.rpc(&job.method, job.params);
                    let _ = job.reply.send(answer);
                }
            })
            .expect("start the shell's thread");
        ShellHandle {
            jobs: Mutex::new(jobs),
        }
    }

    /// Makes one call and waits for its answer. A shell thread that has stopped (a panic in its link) fails the call.
    pub fn rpc(&self, method: String, params: Option<Value>) -> Result<Value, RpcError> {
        let (reply, answer) = mpsc::channel();
        let sent = self
            .jobs
            .lock()
            .expect("the shell's job queue lock")
            .send(Job {
                method,
                params,
                reply,
            });
        let stopped = || RpcError::new(code::INTERNAL_ERROR, "the shell's rpc thread has stopped");
        sent.map_err(|_| stopped())?;
        answer.recv().map_err(|_| stopped())?
    }
}
