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
//! | tag | function | what it does |
//! |---|---|---|
//! | `shell: daemon connection` | [`SocketLink`] | the socket, the token file and the handshake, then each call |
//! | `shell: daemon events` | [`pump_daemon_events`] | the daemon's notices onto the event channel |
//! | `shell: ask token` | [`Shell`]'s ask forwarding | the ask's token and endpoint added to `ask.start` and `ask.test` |

use collide_core::ask::{AskCancelParams, AskMethod, AskStartParams, AskTestParams};
use collide_core::owner::{JsonRpc2, Notification, Request, RequestId, Response, RpcError, code};
use collide_core::ui_tools::REQUEST_EVENT;
use collide_core::ui_tools::UiToolsRequest;
use collide_host::ui_tools::{Completed, Rpc, UiToolsEndpoint, Window};
use serde::Serialize;
use serde_json::Value;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;

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

/// The real link: the daemon's socket, with the token read from its file (0.2; `host/src/daemon.rs`).
pub struct SocketLink;

impl DaemonLink for SocketLink {
    fn call(&mut self, request: &Request) -> Response {
        let _ = request;
        todo!("shell: daemon connection")
    }
}

/// Reads the daemon's notices from the link and puts each on the channel: the events of `daemon.subscribe`, and an
/// ask's `ask.event` and `ask.outcome`.
pub fn pump_daemon_events(link: &SocketLink, channel: &dyn EventChannel) {
    let _ = (link, channel);
    todo!("shell: daemon events")
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
