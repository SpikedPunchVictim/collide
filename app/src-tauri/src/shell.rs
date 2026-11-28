//! The shell's `rpc` command and event channel (0.2, The UI speaks the owner's protocol; ts-0017-qb), as plain code with
//! no Tauri type, so a test needs no runtime. [`crate::run`] wires it to the webview.
//!
//! ```text
//! webview ── rpc(method, params) ──▶ Shell ─ ui_tools.* ─▶ UiToolsEndpoint::rpc   (answered here, never forwarded)
//!                                          ├ shell.* ─────▶ the shell's own methods (answered here, never forwarded)
//!                                          └ the rest ────▶ DaemonLink::call       (adds the per-client token)
//! webview ◀── one event channel ──── EventChannel ◀─ daemon notices, ui_tools.request
//! ```
//!
//! ## What the shell decides
//!
//! 1. `initialize` is the shell's: it carries the per-client token, so the window cannot send it. The link makes the
//!    handshake when it connects, with the token it read itself.
//! 2. A `ui_tools.*` method goes to [`UiToolsEndpoint::rpc`] and is never forwarded, whatever it answers.
//! 3. A `shell.*` method is the shell's own and is never forwarded, whatever it answers. There are three.
//!    [`OPEN_EXTERNAL`] opens a link in the person's browser after the shell has checked its scheme
//!    ([`open_external`]). The opener is given to [`Shell::new`], so a test never opens a real link. [`PICK_FOLDER`]
//!    asks the person for a folder ([`pick_folder`]). The picker is given to [`Shell::with_picker`], so a test never
//!    opens a real window. [`SET_DEFAULTS`] tells the link that the window started with defaults (ts-0215-gt), so its
//!    connection to the daemon is made with the flag on `initialize` ([`set_defaults`]).
//! 4. `ask.start` and `ask.test` are read in the window's shape ([`collide_core::ask::AskStartParams`]), which has no
//!    token field, so a window that sends one is refused. The shell then makes the daemon's shape: a token of 32 random
//!    bytes for the ask, registered with the endpoint, and the endpoint's URL (`with_ask_token`). `ask.cancel`
//!    has one shape and goes on as it is. See "An ask" below.
//! 5. Everything else is forwarded, and the daemon says if the method is unknown.
//!
//! ## One thread owns the shell
//!
//! The endpoint holds boxed traits that are not `Send`, so [`ShellHandle`] runs the [`Shell`] on a thread of its own and
//! takes calls over a channel. A call from the webview waits for its answer.
//!
//! The listener of the UI-tools endpoint ([`crate::endpoint`]) reaches the endpoint the same way: [`ShellHandle::http`]
//! hands the thread one request and waits. A `tools/call` waits there for the window's reply, and the thread looks at
//! the clock between jobs ([`EXPIRE_TICK`]), so a window that stays silent for 10 s is answered ([`Shell::tick`]).
//! The window closing or reloading is [`ShellHandle::window_gone`].
//!
//! ## The way to the daemon
//!
//! [`DaemonAccess`] holds the paths, the start on demand and the rule that ends it. It makes each connection: it tells
//! the daemon where the app is, starts the daemon when nothing answers, and does the handshake with the token it read
//! from the token file. [`SocketLink`] keeps one such connection for the window's calls. [`pump_daemon_events`] has a
//! connection of its own for `daemon.subscribe`, so a call never waits behind a stream.
//!
//! ## An ask
//!
//! `ask.start` goes out on a connection of its own, made by an [`AskConnector`] ([`SocketAskConnector`] in the app),
//! not on the one the window's calls share: the daemon ties an ask to the connection that started it, so closing that
//! connection is the cancel, and one dropped socket cancels one ask. A thread of the connector reads the connection and
//! gives each `ask.event` and `ask.outcome` to the ask's [`AskRelay`], which puts it on the event channel. The outcome
//! also queues the ask's token to end, and the shell's thread ends it ([`Shell::tick`], and before any call, so the
//! token is dead by the time the window acts on the outcome). A connection that ends with no outcome is told to the
//! window as a lost ask. When the window closes or reloads, [`Shell::window_gone`] sends `ask.cancel` for each live
//! ask on the call connection. `ask.test` answers once, on the call connection, and its token ends with the answer.

use collide_core::agent::UsageQuality;
use collide_core::ask::{
    AskCancelParams, AskCost, AskEnd, AskMethod, AskNotice, AskOutcome, AskOutcomeNotice,
    AskStartParams, AskTestParams,
};
use collide_core::non_empty::NonEmpty;
use collide_core::owner::{
    DaemonSubscribeParams, DaemonSubscribeResult, JsonRpc2, Notification, Request, RequestId,
    Response, RpcError, code,
};
use collide_core::ui_tools::REQUEST_EVENT;
use collide_core::ui_tools::UiToolsRequest;
use collide_host::auth;
use collide_host::clock::{Clock, Clocks};
use collide_host::daemon::{DaemonPaths, check_home};
use collide_host::launch;
use collide_host::owner_client::{ClientError, Incoming, OwnerClient};
use collide_host::ui_tools::{
    AskRegistration, Completed, ENDPOINT_PATH, Handled, HttpRequest, HttpResponse, Rpc,
    UiToolsEndpoint, Window, random_token,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::io;
use std::path::PathBuf;
use std::process::{Command, Stdio};
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

/// The method of the shell's own notice that the daemon's event stream ended or came back (ts-0081-wm). Its params are
/// `{"state": "lost"}` or `{"state": "back"}`, and nothing else. The window reloads its state on `back`, because the
/// daemon's log may no longer hold what came between. `app/src/owner/client.ts` has the same name; a test keeps the two
/// equal.
pub const CONNECTION_EVENT: &str = "shell.connection";

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

    /// Tells the link that the window started with defaults, or came back from it. The next call goes over a
    /// connection made in that mode. A link that has no modes ignores it.
    fn set_defaults(&mut self, _defaults: bool) {}
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
    /// The app's `HOME`, when it has one. A `HOME` that names no folder is refused ([`check_home`]).
    home: Option<String>,
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
            home: None,
        }
    }

    /// The app's `HOME`, which every connection checks before it makes or starts anything.
    pub fn with_home(mut self, home: Option<String>) -> Self {
        self.home = home;
        self
    }

    /// A new connection that has done `initialize`, with the token read from the token file (which is refused unless it
    /// is
    /// mode `0600`). It tells the daemon where the app is first, then makes sure a daemon answers, and starts one when
    /// none
    /// does, unless [`launch::RestartPolicy`] says the app has failed to start one too often lately: then it only
    /// connects, so a daemon someone else started is still used.
    pub fn connect(&self) -> Result<OwnerClient, RpcError> {
        self.connect_as(false)
    }

    /// [`DaemonAccess::connect`] with the defaults flag on `initialize` when `defaults` is set (ts-0215-gt): the daemon
    /// reads only the Default layer and the built-in packs and views for that connection, and refuses its writes.
    pub fn connect_as(&self, defaults: bool) -> Result<OwnerClient, RpcError> {
        // First, before the app's file below the run folder is written: that folder can be below the home.
        check_home(self.home.as_deref())
            .map_err(|reason| unavailable(format!("the daemon could not be started: {reason}")))?;
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
        OwnerClient::connect_as(&self.paths, CLIENT_NAME, defaults).map_err(|error| {
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
    /// Whether the connection is made with the defaults flag.
    defaults: bool,
    call_wait: Duration,
}

impl SocketLink {
    pub fn new(access: Arc<DaemonAccess>) -> Self {
        SocketLink {
            access,
            client: None,
            defaults: false,
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
            self.client = Some(self.access.connect_as(self.defaults)?);
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
    fn set_defaults(&mut self, defaults: bool) {
        if self.defaults != defaults {
            // The mode rides the connection, so the old one is dropped and the next call makes a new one.
            self.defaults = defaults;
            self.client = None;
        }
    }

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

/// Whether the window has been told that the stream is down.
#[derive(Default, PartialEq)]
enum StreamState {
    /// Nothing has ended yet, so the window has heard nothing.
    #[default]
    Unknown,
    Up,
    /// The window was told `lost` and has not been told `back`.
    Down,
}

/// Tells the window about the stream, on the one event channel.
fn tell_connection(channel: &dyn EventChannel, state: &str) {
    channel.emit(&ChannelEvent {
        method: CONNECTION_EVENT.to_string(),
        params: serde_json::json!({ "state": state }),
    });
}

/// What the pump has done so far, which outlives each stream.
#[derive(Default)]
struct PumpProgress {
    stream: StreamState,
    /// The `seq` of the last event put on the channel, or the head of the log when none has been yet.
    last_seq: Option<u64>,
    asked_for_environment: bool,
}

/// Reads the daemon's notices on a connection of its own and puts each on the channel: the `daemon.event` and
/// `queue.changed` notices of `daemon.subscribe`. The first connection of the app also asks the daemon to capture the
/// login-shell environment again (0.2, The base environment).
///
/// The window is told, with a [`CONNECTION_EVENT`] notice, when the stream ends and when it comes back. The first
/// connection is not a return and says nothing, unless it had to be tried again.
///
/// When the stream ends, because the daemon stopped or because this reader fell behind, the pump waits `retry_wait` and
/// subscribes again from the last `seq` it forwarded, so no event is missing and none is sent twice. It starts the
/// daemon when none answers, as a call does, and returns when `stop` is set.
pub fn pump_daemon_events(
    access: &DaemonAccess,
    channel: &dyn EventChannel,
    stop: &AtomicBool,
    retry_wait: Duration,
) {
    let mut progress = PumpProgress::default();
    let mut told = String::new();
    while !stop.load(Ordering::SeqCst) {
        let followed = follow_daemon(access, channel, stop, &mut progress);
        // A stream that ends, or a connection that cannot be made, is told once, until it comes back. A pump that is
        // told to stop is not a stream that ended.
        if !stop.load(Ordering::SeqCst) && progress.stream != StreamState::Down {
            progress.stream = StreamState::Down;
            tell_connection(channel, "lost");
        }
        match followed {
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
            "collide: the daemon's log no longer holds events {} to {}; the window reloads instead of replaying",
            last + 1,
            result.oldest_seq - 1
        ),
        Some(_) => {}
    }
    // The window hears of the return before the replay, so it reloads first and the replayed events find it current.
    if progress.stream == StreamState::Down {
        tell_connection(channel, "back");
    }
    progress.stream = StreamState::Up;
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

/// The method that tells the shell the window started with defaults, or left that mode (ts-0215-gt). The window cannot
/// send `initialize`, so this is how the flag reaches the connection. `app/src/owner/` has the same name
/// (`SHELL_METHODS`); a test keeps the two equal.
pub const SET_DEFAULTS: &str = "shell.set_defaults";

/// The prefix of the methods the shell answers itself. No method of the daemon starts with it.
const SHELL_PREFIX: &str = "shell.";

/// The method that opens a link in the person's browser, after the window has asked them (R5; 0.9). `app/src/owner/`
/// has the same name (`SHELL_METHODS`); a test keeps the two equal.
pub const OPEN_EXTERNAL: &str = "shell.open_external";

/// Opens a link outside the app. The app's wiring gives [`UsrBinOpen`]; a test, and the test bridge, give
/// [`RecordingOpener`], so nothing a test does leaves the machine.
pub trait Opener {
    /// Opens `url`. The caller has checked its scheme.
    fn open(&mut self, url: &str) -> Result<(), String>;
}

/// The program that opens a link in the person's default browser (or mail program) on macOS.
const OPEN_PROGRAM: &str = "/usr/bin/open";

/// The command that opens `url`: [`OPEN_PROGRAM`] with the URL as its one argument, and nothing else. It is made, not
/// run, so a test can read it. Its caller has checked that `url` starts with a scheme, so `open` cannot take it for a
/// flag or a path.
pub fn open_command(url: &str) -> Command {
    let mut command = Command::new(OPEN_PROGRAM);
    command.arg(url);
    command
}

/// The real opener: runs [`open_command`]. It does not wait for the program, so a slow start never holds the shell's
/// thread, which answers every call; a thread of its own waits, so no child is left behind.
pub struct UsrBinOpen;

impl Opener for UsrBinOpen {
    fn open(&mut self, url: &str) -> Result<(), String> {
        let mut child = open_command(url)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| format!("could not run {OPEN_PROGRAM}: {error}"))?;
        thread::Builder::new()
            .name("collide-open".into())
            .spawn(move || match child.wait() {
                Ok(status) if status.success() => {}
                Ok(status) => eprintln!("collide: {OPEN_PROGRAM} ended with {status}"),
                Err(error) => eprintln!("collide: could not wait for {OPEN_PROGRAM}: {error}"),
            })
            .map(|_| ())
            .map_err(|error| format!("could not wait for {OPEN_PROGRAM}: {error}"))
    }
}

/// An opener that keeps what it was asked to open and opens nothing. Clones share the list.
#[derive(Clone, Default)]
pub struct RecordingOpener {
    opened: Arc<Mutex<Vec<String>>>,
}

impl RecordingOpener {
    /// The links it was asked to open, in order.
    pub fn opened(&self) -> Vec<String> {
        self.opened
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
}

impl Opener for RecordingOpener {
    fn open(&mut self, url: &str) -> Result<(), String> {
        self.opened
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(url.to_string());
        Ok(())
    }
}

/// The method that asks the person for a folder, for `board.locate` and the board menu's typed path (ts-0191-nv). It
/// answers `{ "path": "<absolute path>" }`, or `{ "path": null }` when they cancel. `app/src/owner/` has the same name
/// (`SHELL_METHODS`); a test keeps the two equal.
pub const PICK_FOLDER: &str = "shell.pick_folder";

/// Asks the person for a folder. The app's wiring gives the system's picker (`DialogPicker` in `lib.rs`); a test gives
/// [`RecordingPicker`], so nothing a test does opens a window. The picker is the shell's and the webview has no way to
/// it but [`PICK_FOLDER`]: the dialog plugin has no permission in the window's capability.
pub trait FolderPicker {
    /// Asks for one folder and waits. `Ok(None)` when the person cancels. The path is absolute.
    fn pick(&mut self) -> Result<Option<PathBuf>, String>;
}

/// The picker of a shell that was given none, as the test bridge's: it says so and asks nothing.
pub struct NoPicker;

impl FolderPicker for NoPicker {
    fn pick(&mut self) -> Result<Option<PathBuf>, String> {
        Err("this shell has no folder picker".to_string())
    }
}

/// What a [`RecordingPicker`] does when it is asked.
#[derive(Clone)]
enum Choice {
    Folder(PathBuf),
    Cancel,
    Fail(String),
}

/// A picker that counts how often it was asked and opens nothing. Clones share the count.
#[derive(Clone)]
pub struct RecordingPicker {
    choice: Choice,
    asked: Arc<Mutex<usize>>,
}

impl RecordingPicker {
    fn with(choice: Choice) -> Self {
        RecordingPicker {
            choice,
            asked: Arc::new(Mutex::new(0)),
        }
    }

    /// A person who chooses `folder`.
    pub fn choosing(folder: &str) -> Self {
        Self::with(Choice::Folder(PathBuf::from(folder)))
    }

    /// A person who cancels.
    pub fn cancelling() -> Self {
        Self::with(Choice::Cancel)
    }

    /// A picker that cannot ask, for `reason`.
    pub fn failing(reason: &str) -> Self {
        Self::with(Choice::Fail(reason.to_string()))
    }

    /// How many times it was asked.
    pub fn asked(&self) -> usize {
        *self.asked.lock().unwrap_or_else(|e| e.into_inner())
    }
}

impl FolderPicker for RecordingPicker {
    fn pick(&mut self) -> Result<Option<PathBuf>, String> {
        *self.asked.lock().unwrap_or_else(|e| e.into_inner()) += 1;
        match &self.choice {
            Choice::Folder(folder) => Ok(Some(folder.clone())),
            Choice::Cancel => Ok(None),
            Choice::Fail(reason) => Err(reason.clone()),
        }
    }
}

/// The most a link may be, in bytes.
const MAX_LINK: usize = 2048;

/// The ways a link may start, in lower case: the web, and a mail address.
const LINK_STARTS: [&str; 3] = ["http://", "https://", "mailto:"];

/// Whether `url` may be opened: it starts with one of [`LINK_STARTS`] (in any case), says something after it, is no
/// longer than [`MAX_LINK`], and holds no space and no control character, so a newline cannot split it into two
/// arguments or two lines. The error says why, without repeating the link.
fn check_link(url: &str) -> Result<(), String> {
    if url.len() > MAX_LINK {
        return Err(format!(
            "it is {} bytes and the most is {MAX_LINK}",
            url.len()
        ));
    }
    if url.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return Err("it holds a space or a control character".to_string());
    }
    let lower = url.to_ascii_lowercase();
    let after = LINK_STARTS
        .iter()
        .find_map(|start| lower.strip_prefix(start));
    match after {
        Some(rest) if !rest.is_empty() => Ok(()),
        _ => Err("only http, https and mailto links open".to_string()),
    }
}

/// `shell.open_external`'s params: the one link.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OpenExternalParams {
    url: String,
}

/// `shell.open_external`: opens the link with `opener` after [`check_link`]. A link that fails the check is refused
/// with `invalid params` and `opener` is not called. It never reaches the daemon.
pub fn open_external(opener: &mut dyn Opener, params: Option<Value>) -> Result<Value, RpcError> {
    let params: OpenExternalParams = serde_json::from_value(params.unwrap_or(Value::Null))
        .map_err(|error| {
            RpcError::new(
                code::INVALID_PARAMS,
                format!("invalid params for `{OPEN_EXTERNAL}`: {error}"),
            )
        })?;
    check_link(&params.url).map_err(|why| {
        RpcError::new(
            code::INVALID_PARAMS,
            format!("`{OPEN_EXTERNAL}` refused the link: {why}"),
        )
    })?;
    opener.open(&params.url).map_err(|why| {
        RpcError::new(
            code::INTERNAL_ERROR,
            format!("could not open the link: {why}"),
        )
    })?;
    Ok(serde_json::json!({}))
}

/// `shell.pick_folder`'s params: none. A window that sends a field is refused, so nothing it adds reaches the picker.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PickFolderParams {}

/// `shell.pick_folder`: asks the person for a folder with `picker`. It never reaches the daemon.
pub fn pick_folder(
    picker: &mut dyn FolderPicker,
    params: Option<Value>,
) -> Result<Value, RpcError> {
    let params = match params {
        None | Some(Value::Null) => Value::Object(serde_json::Map::new()),
        Some(other) => other,
    };
    serde_json::from_value::<PickFolderParams>(params).map_err(|error| {
        RpcError::new(
            code::INVALID_PARAMS,
            format!("invalid params for `{PICK_FOLDER}`: {error}"),
        )
    })?;
    let picked = picker.pick().map_err(|why| {
        RpcError::new(
            code::INTERNAL_ERROR,
            format!("could not ask for a folder: {why}"),
        )
    })?;
    let Some(folder) = picked else {
        return Ok(serde_json::json!({ "path": null }));
    };
    let path = folder.to_str().ok_or_else(|| {
        RpcError::new(
            code::INTERNAL_ERROR,
            "the folder's path is not text, so the window cannot be given it",
        )
    })?;
    Ok(serde_json::json!({ "path": path }))
}

/// `shell.set_defaults`'s params: the mode. A window that sends another field is refused.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SetDefaultsParams {
    defaults: bool,
}

/// `shell.set_defaults`: tells `link` the mode. It never reaches the daemon as a call.
pub fn set_defaults(link: &mut dyn DaemonLink, params: Option<Value>) -> Result<Value, RpcError> {
    let params: SetDefaultsParams =
        serde_json::from_value(params.unwrap_or(Value::Null)).map_err(|error| {
            RpcError::new(
                code::INVALID_PARAMS,
                format!("invalid params for `{SET_DEFAULTS}`: {error}"),
            )
        })?;
    link.set_defaults(params.defaults);
    Ok(serde_json::json!({}))
}

/// The notices of one ask, from its connection to the window. The connector hands each notice it reads to
/// [`AskRelay::notice`], and calls [`AskRelay::ended`] when the connection ends. A relay is made per ask by
/// [`Shell::rpc`], and it can be used from any thread.
#[derive(Clone)]
pub struct AskRelay {
    ask: String,
    channel: Arc<dyn EventChannel>,
    done: mpsc::Sender<String>,
    outcome_seen: Arc<AtomicBool>,
}

impl AskRelay {
    /// Puts one notice of the ask on the event channel. The ask's outcome also tells the shell the ask is over, so its
    /// token is ended ([`Shell::tick`]). Only `ask.event` and `ask.outcome` pass.
    pub fn notice(&self, notification: Notification) {
        let outcome = notification.method == AskNotice::OUTCOME_METHOD;
        if !outcome && notification.method != AskNotice::EVENT_METHOD {
            return;
        }
        if outcome {
            self.outcome_seen.store(true, Ordering::SeqCst);
            // Before the window hears it, so the token is already queued to end when the window acts on the outcome.
            let _ = self.done.send(self.ask.clone());
        }
        self.channel.emit(&ChannelEvent::from(notification));
    }

    /// The connection ended. An ask that ended with no outcome (the daemon went away) is lost, and the window is told
    /// so in the shape of an outcome, so its box never waits for one that cannot come.
    pub fn ended(&self) {
        if self.outcome_seen.load(Ordering::SeqCst) {
            return;
        }
        let ask = NonEmpty::new(self.ask.as_str()).expect("register_ask refuses an empty id");
        let lost = AskNotice::Outcome(Box::new(AskOutcomeNotice {
            ask,
            outcome: AskOutcome {
                end: AskEnd::Lost {},
                cost: AskCost::NotReported {},
                quality: UsageQuality::Unreported,
            },
        }));
        self.notice(lost.to_notification());
    }
}

/// The way an ask's notices come from the daemon: one connection of its own per live ask, so closing it is the cancel
/// the daemon's rule expects (0.2, The connection rule).
pub trait AskConnector {
    /// Opens a connection, sends `request` (an `ask.start`) on it and waits for the daemon's answer. When the daemon
    /// accepted, the connection is read on a thread of the connector's, which gives each notice to `relay` and calls
    /// [`AskRelay::ended`] when the connection ends. A refusal is the error, and nothing is left running.
    fn start(&mut self, request: &Request, relay: AskRelay) -> Result<(), RpcError>;
}

/// The channel of a shell that was given none: no window is open, and nothing is emitted.
struct NoChannel;

impl EventChannel for NoChannel {
    fn is_open(&self) -> bool {
        false
    }

    fn emit(&self, _event: &ChannelEvent) {}
}

/// The connector of a shell that was given none: an ask cannot start.
struct NoAsks;

impl AskConnector for NoAsks {
    fn start(&mut self, _request: &Request, _relay: AskRelay) -> Result<(), RpcError> {
        Err(unavailable(
            "this shell has no way to the daemon for an ask".to_string(),
        ))
    }
}

/// The real connector: a connection to the daemon per ask ([`DaemonAccess::connect`]).
pub struct SocketAskConnector {
    access: Arc<DaemonAccess>,
}

impl SocketAskConnector {
    pub fn new(access: Arc<DaemonAccess>) -> Self {
        SocketAskConnector { access }
    }
}

impl AskConnector for SocketAskConnector {
    fn start(&mut self, request: &Request, relay: AskRelay) -> Result<(), RpcError> {
        let mut client = self.access.connect()?;
        match client.call::<Value, Value>(&request.method, &request.params, Some(CALL_WAIT)) {
            Ok(_) => {}
            Err(ClientError::Rpc(error)) => return Err(error),
            Err(error) => return Err(dropped(&request.method, &error, CALL_WAIT)),
        }
        client.set_read_wait(Some(ASK_READ_TICK)).map_err(|error| {
            unavailable(format!("could not read the ask's connection: {error}"))
        })?;
        thread::Builder::new()
            .name("collide-ask-notices".into())
            .spawn(move || {
                loop {
                    match client.next_notice() {
                        Ok(Incoming::Notice(notification)) => {
                            let outcome = notification.method == AskNotice::OUTCOME_METHOD;
                            relay.notice(notification);
                            if outcome {
                                break;
                            }
                        }
                        Ok(Incoming::Quiet) => {}
                        Ok(Incoming::Closed) | Err(_) => break,
                    }
                }
                relay.ended();
            })
            .map_err(|error| unavailable(format!("could not start the ask's reader: {error}")))?;
        Ok(())
    }
}

/// How long the reader of an ask's connection waits for a notice before it reads again.
const ASK_READ_TICK: Duration = Duration::from_millis(500);

/// The shell. See the module doc.
pub struct Shell {
    endpoint: UiToolsEndpoint,
    link: Box<dyn DaemonLink>,
    opener: Box<dyn Opener>,
    picker: Box<dyn FolderPicker>,
    next_id: i64,
    completed: Vec<Completed>,
    /// The HTTP clients whose `tools/call` is waiting on the window, by `requestId`.
    waiting: BTreeMap<String, mpsc::Sender<HttpResponse>>,
    asks: Box<dyn AskConnector>,
    /// Where an ask's notices go. The same channel as the endpoint's window, when the app gave it.
    ask_channel: Arc<dyn EventChannel>,
    /// The asks whose outcome came, from the readers of their connections, for [`Shell::end_finished_asks`].
    finished_asks: mpsc::Receiver<String>,
    finished_sender: mpsc::Sender<String>,
}

impl Shell {
    pub fn new(
        endpoint: UiToolsEndpoint,
        link: Box<dyn DaemonLink>,
        opener: Box<dyn Opener>,
    ) -> Self {
        let (finished_sender, finished_asks) = mpsc::channel();
        Shell {
            endpoint,
            link,
            opener,
            picker: Box::new(NoPicker),
            next_id: 0,
            completed: Vec::new(),
            waiting: BTreeMap::new(),
            asks: Box::new(NoAsks),
            ask_channel: Arc::new(NoChannel),
            finished_asks,
            finished_sender,
        }
    }

    /// The shell with `connector` to start asks, and `channel` for their notices. Without one, `ask.start` says it
    /// cannot.
    pub fn with_asks(
        mut self,
        connector: Box<dyn AskConnector>,
        channel: Arc<dyn EventChannel>,
    ) -> Self {
        self.asks = connector;
        self.ask_channel = channel;
        self
    }

    /// The shell with `picker` to ask the person for a folder. Without one, `shell.pick_folder` says there is none.
    pub fn with_picker(mut self, picker: Box<dyn FolderPicker>) -> Self {
        self.picker = picker;
        self
    }

    /// The UI-tools endpoint, for `ask.start` to register an ask and its token (ts-0201-us).
    pub fn endpoint_mut(&mut self) -> &mut UiToolsEndpoint {
        &mut self.endpoint
    }

    /// The agent calls the window's `ui_tools.reply` finished since the last call, for the shell to hand to the
    /// clients that wait for them.
    pub fn take_completed(&mut self) -> Vec<Completed> {
        std::mem::take(&mut self.completed)
    }

    /// Tells the endpoint the port its listener has, which is the one `Host` may name.
    pub fn set_endpoint_port(&mut self, port: u16) {
        self.endpoint.set_port(port);
    }

    /// One HTTP request of the UI-tools endpoint. The answer goes to `reply`: now, or, for a `tools/call` that went to
    /// the window, when the window replies ([`Shell::rpc`]), when its deadline comes ([`Shell::tick`]) or when the
    /// window goes ([`Shell::window_gone`]).
    pub fn http(&mut self, request: &HttpRequest, reply: mpsc::Sender<HttpResponse>) {
        self.end_finished_asks();
        match self.endpoint.handle(request) {
            Handled::Response(response) => {
                let _ = reply.send(response);
            }
            Handled::Pending { request_id } => {
                self.waiting.insert(request_id, reply);
            }
        }
    }

    /// Answers the calls whose deadline has come, and every call that finished since the last look.
    pub fn tick(&mut self) {
        self.end_finished_asks();
        let expired = self.endpoint.expire();
        self.completed.extend(expired);
        self.deliver();
    }

    /// The window closed or reloaded: the calls waiting on it are answered, and each live ask is cancelled at the
    /// daemon (0.2, Asks are cancelled when the window closes or reloads).
    pub fn window_gone(&mut self) {
        self.end_finished_asks();
        let gone = self.endpoint.window_gone();
        self.completed.extend(gone.completed);
        for cancel in gone.cancel {
            self.next_id += 1;
            let request = Request {
                jsonrpc: JsonRpc2,
                id: RequestId::Number(self.next_id),
                method: AskMethod::Cancel.name().to_string(),
                params: serde_json::to_value(&cancel).expect("an ask.cancel is JSON"),
            };
            if let Response::Failure { error, .. } = self.link.call(&request) {
                eprintln!(
                    "collide: could not cancel the ask `{}`: {}",
                    cancel.ask.as_str(),
                    error.message
                );
            }
        }
        self.deliver();
    }

    /// Sends each finished call to the client that waits for it. A call with no client (it hung up) is dropped.
    fn deliver(&mut self) {
        for done in std::mem::take(&mut self.completed) {
            if let Some(reply) = self.waiting.remove(&done.request_id) {
                let _ = reply.send(done.response);
            }
        }
    }

    /// One call of the webview's `rpc` command.
    pub fn rpc(&mut self, method: &str, params: Option<Value>) -> Result<Value, RpcError> {
        self.end_finished_asks();
        if method == "initialize" {
            return Err(RpcError::method_not_found(method).with_data(serde_json::json!({
                "reason": "the shell makes the handshake; the window does not send `initialize`",
            })));
        }
        match method {
            OPEN_EXTERNAL => return open_external(self.opener.as_mut(), params),
            PICK_FOLDER => return pick_folder(self.picker.as_mut(), params),
            SET_DEFAULTS => return set_defaults(self.link.as_mut(), params),
            other if other.starts_with(SHELL_PREFIX) => {
                return Err(RpcError::method_not_found(other));
            }
            _ => {}
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
            Rpc::Forward => match AskMethod::from_name(&request.method) {
                None => answer(self.link.call(&request)),
                Some(AskMethod::Cancel) => {
                    read::<AskCancelParams>(&request)?;
                    answer(self.link.call(&request))
                }
                Some(AskMethod::Start) => self.ask_start(request),
                Some(AskMethod::Test) => self.ask_test(request),
            },
        }
    }

    /// `ask.start`: the daemon's shape of the request goes out on a connection of its own, which stays open and is read
    /// for the ask's notices. A refusal ends the token the ask was given.
    fn ask_start(&mut self, request: Request) -> Result<Value, RpcError> {
        let window: AskStartParams = read(&request)?;
        let ask = window.ask.as_str().to_string();
        let request = self.with_ask_token(&ask, request)?;
        let relay = AskRelay {
            ask: ask.clone(),
            channel: Arc::clone(&self.ask_channel),
            done: self.finished_sender.clone(),
            outcome_seen: Arc::default(),
        };
        match self.asks.start(&request, relay) {
            Ok(()) => Ok(serde_json::json!({})),
            Err(error) => {
                self.endpoint.end_ask(&ask);
                Err(error)
            }
        }
    }

    /// `ask.test`: the daemon answers once, and the test's token ends with the answer.
    fn ask_test(&mut self, request: Request) -> Result<Value, RpcError> {
        let window: AskTestParams = read(&request)?;
        let ask = window.ask.as_str().to_string();
        let request = self.with_ask_token(&ask, request)?;
        let response = self.link.call(&request);
        self.endpoint.end_ask(&ask);
        answer(response)
    }

    /// Makes the ask's token, registers it with the endpoint, and adds the endpoint's URL and the token to the params.
    fn with_ask_token(&mut self, ask: &str, request: Request) -> Result<Request, RpcError> {
        let token = random_token().map_err(|error| {
            RpcError::new(
                code::INTERNAL_ERROR,
                format!("could not make a token for the ask: {error}"),
            )
        })?;
        self.endpoint.register_ask(ask, &token).map_err(|problem| {
            let code = match problem {
                AskRegistration::DuplicateId(_) => code::CONFLICT,
                _ => code::INVALID_PARAMS,
            };
            RpcError::new(code, problem.to_string())
        })?;
        let mut params = request.params;
        params["endpoint"] = Value::String(format!(
            "http://127.0.0.1:{}{ENDPOINT_PATH}",
            self.endpoint.port()
        ));
        params["token"] = Value::String(token);
        Ok(Request { params, ..request })
    }

    /// Ends the token of each ask whose outcome came since the last look.
    fn end_finished_asks(&mut self) {
        while let Ok(ask) = self.finished_asks.try_recv() {
            self.endpoint.end_ask(&ask);
        }
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

/// What the shell's thread is asked to do.
enum Job {
    /// One call of the webview's `rpc` command, answered on `reply`.
    Rpc {
        method: String,
        params: Option<Value>,
        reply: mpsc::Sender<Result<Value, RpcError>>,
    },
    /// Something to run on the shell.
    Run(Box<dyn FnOnce(&mut Shell) + Send>),
}

/// How often the shell's thread looks at the clock when it has no job: a `tools/call` whose window is silent is
/// answered within this much of its 10 s.
pub const EXPIRE_TICK: Duration = Duration::from_millis(250);

/// How long [`ShellHandle::http`] waits for the shell's thread, which is longer than any `tools/call` waits for the
/// window (10 s), so only a shell that is stuck or gone fails a request.
const HTTP_WAIT: Duration = Duration::from_secs(30);

/// A [`Shell`] on a thread of its own, callable from any thread.
pub struct ShellHandle {
    jobs: Mutex<mpsc::Sender<Job>>,
}

impl ShellHandle {
    /// Starts the thread, which builds the shell with `make` (so the shell never crosses threads) and answers calls
    /// until the handle is dropped. Between jobs it runs [`Shell::tick`] every [`EXPIRE_TICK`].
    pub fn spawn(make: impl FnOnce() -> Shell + Send + 'static) -> ShellHandle {
        let (jobs, inbox) = mpsc::channel::<Job>();
        thread::Builder::new()
            .name("collide-shell".into())
            .spawn(move || {
                let mut shell = make();
                loop {
                    match inbox.recv_timeout(EXPIRE_TICK) {
                        Ok(Job::Rpc {
                            method,
                            params,
                            reply,
                        }) => {
                            let _ = reply.send(shell.rpc(&method, params));
                        }
                        Ok(Job::Run(job)) => job(&mut shell),
                        Err(mpsc::RecvTimeoutError::Timeout) => {}
                        Err(mpsc::RecvTimeoutError::Disconnected) => return,
                    }
                    shell.tick();
                }
            })
            .expect("start the shell's thread");
        ShellHandle {
            jobs: Mutex::new(jobs),
        }
    }

    fn send(&self, job: Job) -> Result<(), RpcError> {
        let sent = self
            .jobs
            .lock()
            .expect("the shell's job queue lock")
            .send(job);
        sent.map_err(|_| RpcError::new(code::INTERNAL_ERROR, "the shell's rpc thread has stopped"))
    }

    /// Makes one call and waits for its answer. A shell thread that has stopped (a panic in its link) fails the call.
    pub fn rpc(&self, method: String, params: Option<Value>) -> Result<Value, RpcError> {
        let (reply, answer) = mpsc::channel();
        self.send(Job::Rpc {
            method,
            params,
            reply,
        })?;
        answer.recv().map_err(|_| {
            RpcError::new(code::INTERNAL_ERROR, "the shell's rpc thread has stopped")
        })?
    }

    /// Hands the UI-tools endpoint one HTTP request and waits for its answer ([`Shell::http`]). A `tools/call` waits
    /// for the window. A shell that is gone or stuck gives a 503.
    pub fn http(&self, request: HttpRequest) -> HttpResponse {
        let (reply, answer) = mpsc::channel();
        let sent = self.send(Job::Run(Box::new(move |shell| shell.http(&request, reply))));
        match sent.map(|()| answer.recv_timeout(HTTP_WAIT)) {
            Ok(Ok(response)) => response,
            _ => unavailable_response(),
        }
    }

    /// Tells the shell the window closed or reloaded ([`Shell::window_gone`]). It does not wait: the cancel of an ask
    /// is a call to the daemon.
    pub fn window_gone(&self) {
        let _ = self.send(Job::Run(Box::new(Shell::window_gone)));
    }

    /// Tells the endpoint the port its listener has, and waits until it knows ([`Shell::set_endpoint_port`]).
    pub fn set_endpoint_port(&self, port: u16) -> Result<(), RpcError> {
        let (done, told) = mpsc::channel();
        self.send(Job::Run(Box::new(move |shell| {
            shell.set_endpoint_port(port);
            let _ = done.send(());
        })))?;
        told.recv()
            .map_err(|_| RpcError::new(code::INTERNAL_ERROR, "the shell's rpc thread has stopped"))
    }
}

/// What a client gets when the shell's thread does not answer.
fn unavailable_response() -> HttpResponse {
    let error = RpcError::new(
        code::UNAVAILABLE,
        "Collide did not answer; try again in a moment",
    );
    HttpResponse {
        status: 503,
        headers: vec![("Content-Type".to_string(), "application/json".to_string())],
        body: serde_json::to_vec(&Response::failure(None, error)).expect("an error is JSON"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn open(url: &str) -> (Result<Value, RpcError>, Vec<String>) {
        let mut opener = RecordingOpener::default();
        let answer = open_external(&mut opener, Some(json!({ "url": url })));
        (answer, opener.opened())
    }

    #[test]
    fn an_accepted_link_is_passed_to_the_opener_as_one_argument() {
        for url in [
            "https://example.com/docs?a=1&b=2#top",
            "http://example.com",
            "HTTPS://EXAMPLE.COM/Path",
            "mailto:person@example.com?subject=hello%20there",
        ] {
            let (answer, opened) = open(url);
            assert_eq!(answer, Ok(json!({})), "{url}");
            assert_eq!(opened, vec![url.to_string()], "{url}");
        }
    }

    #[test]
    fn a_link_with_another_scheme_is_refused_and_nothing_is_opened() {
        for url in [
            "javascript:alert(1)",
            "JavaScript:alert(1)",
            "file:///etc/passwd",
            "data:text/html,<script>1</script>",
            "ftp://example.com/file",
            "vbscript:x",
            "tel:+15550100",
            "java\tscript:alert(1)",
            "//example.com/path",
            "example.com",
            "/Applications/Calculator.app",
            "-a Calculator",
            "https:",
            "https://",
            "mailto:",
            "",
        ] {
            let (answer, opened) = open(url);
            let error = answer.expect_err(url);
            assert_eq!(error.code, code::INVALID_PARAMS, "{url}");
            assert!(opened.is_empty(), "{url} was opened");
        }
    }

    #[test]
    fn a_link_with_a_newline_a_space_or_a_control_character_is_refused() {
        for url in [
            "https://example.com/a\nhttps://other.example/",
            "https://example.com/\n",
            "https://example.com/\r\n",
            "https://example.com/a b",
            " https://example.com/",
            "https://example.com/\u{0}",
            "https://example.com/\u{7f}",
            "https://example.com/\u{85}",
            "https://example.com/\u{2028}",
        ] {
            let (answer, opened) = open(url);
            let error = answer.expect_err(url);
            assert_eq!(error.code, code::INVALID_PARAMS, "{url:?}");
            assert!(opened.is_empty(), "{url:?} was opened");
        }
    }

    #[test]
    fn a_link_over_the_limit_is_refused_and_one_at_it_is_not() {
        let at = format!(
            "https://example.com/{}",
            "a".repeat(MAX_LINK - "https://example.com/".len())
        );
        assert_eq!(at.len(), MAX_LINK);
        assert_eq!(open(&at).0, Ok(json!({})));
        let (answer, opened) = open(&format!("{at}a"));
        assert_eq!(answer.expect_err("too long").code, code::INVALID_PARAMS);
        assert!(opened.is_empty());
    }

    #[test]
    fn a_refusal_does_not_repeat_the_link() {
        let (answer, _) = open("javascript:steal(secret-0123)");
        assert!(!answer.expect_err("refused").message.contains("secret-0123"));
    }

    #[test]
    fn params_that_are_not_one_url_are_refused() {
        for params in [
            None,
            Some(json!({})),
            Some(json!({ "url": 7 })),
            Some(json!({ "url": "https://example.com/", "also": true })),
        ] {
            let mut opener = RecordingOpener::default();
            let error = open_external(&mut opener, params.clone()).expect_err("refused");
            assert_eq!(error.code, code::INVALID_PARAMS, "{params:?}");
            assert!(opener.opened().is_empty(), "{params:?}");
        }
    }

    #[test]
    fn an_opener_that_fails_gives_an_error_that_says_why() {
        struct Failing;
        impl Opener for Failing {
            fn open(&mut self, _: &str) -> Result<(), String> {
                Err("no browser".to_string())
            }
        }
        let error = open_external(&mut Failing, Some(json!({ "url": "https://example.com/" })))
            .expect_err("the opener failed");
        assert_eq!(error.code, code::INTERNAL_ERROR);
        assert!(error.message.contains("no browser"), "{}", error.message);
    }

    #[test]
    fn the_real_opener_runs_usr_bin_open_with_the_link_as_its_one_argument() {
        // The command is made and read, never run: a test opens nothing.
        let url = "https://example.com/a?b=1&c=2";
        let command = open_command(url);
        assert_eq!(command.get_program(), "/usr/bin/open");
        let args: Vec<_> = command.get_args().collect();
        assert_eq!(args, [url]);
    }
}
