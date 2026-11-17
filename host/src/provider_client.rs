//! A client for a provider over its stdio (0.10, Protocol): newline-delimited JSON-RPC 2.0, one request at a time.
//! The supervisor ([`crate::supervisor`]) keeps one of these for each provider; the CLI uses one directly until
//! Stage 4c. The provider's stderr goes to this process's own stderr, so its log lines reach the person at the
//! terminal.
//!
//! **One reader thread for each child** reads its output line by line, so nothing waits on the provider's output but
//! that thread. A line with an `id` is a reply and goes to the caller's thread, which waits for it with
//! `recv_timeout`: a provider that does not answer never leaves the caller waiting past the timeout of the method
//! ([`CallTimeouts`]), and the child is killed. A line with no `id` is a notification, read into a
//! [`ProviderEvent`] and sent on the events channel, so none is lost while the caller is busy and none comes between
//! a request and its reply. There is no async runtime.

use crate::builtin_packs::ProviderLaunch;
use collide_core::provider::{
    ApplyParams, ApplyResult, GetParams, GetResult, InitializeParams, InitializeResult, ListParams,
    ListResult, PreviewResult, ProviderEvent, Task, TaskInvalid, WatchParams, WatchResult,
};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::fmt;
use std::io::{self, BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

/// The JSON-RPC code for a method the provider does not have.
const METHOD_NOT_FOUND: i64 = -32601;

/// How far apart the checks of a child that is being stopped are.
const STOP_POLL: Duration = Duration::from_millis(10);

/// How long each call may wait for its reply. The plan fixes none of the values ("Stage 4b confirms them").
/// `list` is slow because a provider parses its files itself (the plan CLI's JSON alone takes about 1 s for 1,567
/// tasks), and `apply` and `preview` can wait on a lock for up to 15 s (0.10).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CallTimeouts {
    pub initialize: Duration,
    pub list: Duration,
    pub get: Duration,
    pub apply: Duration,
    pub preview: Duration,
    pub watch: Duration,
}

impl Default for CallTimeouts {
    fn default() -> Self {
        CallTimeouts {
            initialize: Duration::from_secs(30),
            list: Duration::from_secs(60),
            get: Duration::from_secs(10),
            apply: Duration::from_secs(120),
            preview: Duration::from_secs(120),
            watch: Duration::from_secs(10),
        }
    }
}

impl CallTimeouts {
    /// The timeout of `method`. A method the protocol does not have gets `apply`'s, the longest.
    pub fn of(&self, method: &str) -> Duration {
        match method {
            "initialize" => self.initialize,
            "list" => self.list,
            "get" => self.get,
            "preview" => self.preview,
            "watch" => self.watch,
            _ => self.apply,
        }
    }
}

/// What a client is started with besides the launch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClientOptions {
    pub timeouts: CallTimeouts,
    /// How long a child gets, after its input is closed, to end by itself before it is killed.
    pub stop_grace: Duration,
}

impl Default for ClientOptions {
    fn default() -> Self {
        ClientOptions {
            timeouts: CallTimeouts::default(),
            stop_grace: Duration::from_secs(5),
        }
    }
}

#[derive(Debug)]
pub enum ClientError {
    /// The provider process did not start.
    Spawn { program: String, error: io::Error },
    /// Writing a request failed: the provider is not reading, so it never had the request.
    Io { method: String, error: io::Error },
    /// The provider closed its output before it answered, so it is gone.
    Closed {
        method: String,
        status: Option<String>,
    },
    /// The provider did not answer within `after`. It was killed.
    Timeout { method: String, after: Duration },
    /// A line of the provider's output is not JSON, or a reply is not what the method returns.
    Malformed { method: String, detail: String },
    /// The provider answered with a JSON-RPC error.
    Rpc {
        method: String,
        code: i64,
        message: String,
    },
}

impl ClientError {
    /// Whether the child is gone, or was killed, so the next request needs a new one.
    pub fn ends_child(&self) -> bool {
        matches!(
            self,
            Self::Io { .. } | Self::Closed { .. } | Self::Timeout { .. }
        )
    }

    /// Whether the provider may have acted on the request: it had the request, and did not answer.
    pub fn may_have_run(&self) -> bool {
        matches!(self, Self::Closed { .. } | Self::Timeout { .. })
    }

    /// Whether the provider says it has no such method.
    pub fn is_method_not_found(&self) -> bool {
        matches!(
            self,
            Self::Rpc {
                code: METHOD_NOT_FOUND,
                ..
            }
        )
    }
}

impl fmt::Display for ClientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Spawn { program, error } => {
                write!(f, "could not start the provider `{program}`: {error}")
            }
            Self::Io { method, error } => {
                write!(f, "talking to the provider ({method}) failed: {error}")
            }
            Self::Closed { method, status } => {
                let status = status
                    .as_ref()
                    .map(|status| format!(" ({status})"))
                    .unwrap_or_default();
                write!(
                    f,
                    "the provider exited{status} before it answered `{method}`; see its messages above"
                )
            }
            Self::Timeout { method, after } => write!(
                f,
                "the provider did not answer `{method}` within {} s and was stopped",
                after.as_secs_f64()
            ),
            Self::Malformed { method, detail } => {
                write!(
                    f,
                    "the provider's answer to `{method}` is not valid: {detail}"
                )
            }
            Self::Rpc {
                method,
                code,
                message,
            } => write!(
                f,
                "the provider refused `{method}` (error {code}): {message}"
            ),
        }
    }
}

impl std::error::Error for ClientError {}

/// What the reader thread hands the caller's thread.
enum Incoming {
    /// A line with an `id`.
    Reply(Value),
    /// A line that is not JSON.
    Bad(String),
    /// The output closed.
    Closed,
}

/// A running provider. Dropping it closes the provider's input, which ends it, and kills it when it does not end
/// within the grace.
pub struct ProviderClient {
    child: Child,
    input: Option<ChildStdin>,
    replies: Receiver<Incoming>,
    /// Set by the reader thread when the provider's output closes.
    ended: Arc<AtomicBool>,
    /// The events of a client started with [`ProviderClient::start`], which has a channel of its own.
    own_events: Option<Receiver<ProviderEvent>>,
    next_id: u64,
    options: ClientOptions,
}

impl ProviderClient {
    /// Starts the provider with `cwd` as its working directory, with the default options and a channel of its own
    /// for its events ([`ProviderClient::events`]). It never reads the root from `cwd` (`initialize` carries it); the
    /// directory is only somewhere that exists.
    pub fn start(launch: &ProviderLaunch, cwd: &std::path::Path) -> Result<Self, ClientError> {
        let (send, receive) = mpsc::channel();
        let mut client = Self::start_with(launch, cwd, ClientOptions::default(), send)?;
        client.own_events = Some(receive);
        Ok(client)
    }

    /// [`ProviderClient::start`] with the options given, and the events sent to `events`: the supervisor gives each
    /// of its children the same sender, so a restart does not change the channel.
    pub fn start_with(
        launch: &ProviderLaunch,
        cwd: &std::path::Path,
        options: ClientOptions,
        events: Sender<ProviderEvent>,
    ) -> Result<Self, ClientError> {
        let mut child = launch
            .command()
            .current_dir(cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|error| ClientError::Spawn {
                program: launch.program.display().to_string(),
                error,
            })?;
        let input = child.stdin.take();
        let output = BufReader::new(child.stdout.take().expect("stdout is piped"));
        let (send, replies) = mpsc::channel();
        let ended = Arc::new(AtomicBool::new(false));
        let flag = ended.clone();
        std::thread::spawn(move || read_output(output, &send, &events, &flag));
        Ok(Self {
            child,
            input,
            replies,
            ended,
            own_events: None,
            next_id: 1,
            options,
        })
    }

    /// The events of a client started with [`ProviderClient::start`].
    pub fn events(&self) -> Option<&Receiver<ProviderEvent>> {
        self.own_events.as_ref()
    }

    /// Whether the provider's output has closed: it is gone, or it will be. A request to it would not be answered.
    pub fn has_ended(&self) -> bool {
        self.ended.load(Ordering::SeqCst)
    }

    /// The flag [`ProviderClient::has_ended`] reads, for a reader that cannot borrow the client.
    pub fn ended_flag(&self) -> Arc<AtomicBool> {
        self.ended.clone()
    }

    /// Closes the provider's input, gives it the grace to end by itself, then kills it. The exit status when it ended
    /// by itself.
    pub fn stop(&mut self) -> Option<String> {
        self.input = None;
        let until = Instant::now() + self.options.stop_grace;
        loop {
            match self.child.try_wait() {
                Ok(Some(status)) => return Some(status.to_string()),
                Ok(None) => {}
                Err(_) => break,
            }
            if Instant::now() >= until {
                break;
            }
            std::thread::sleep(STOP_POLL);
        }
        self.kill();
        None
    }

    fn kill(&mut self) {
        self.input = None;
        let _ = self.child.kill();
        let _ = self.child.wait();
    }

    /// Sends one request and returns its `result`, waiting for the reply no longer than the method's timeout.
    fn request(&mut self, method: &str, params: Value) -> Result<Value, ClientError> {
        let id = self.next_id;
        self.next_id += 1;
        let request = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params });
        let input = self.input.as_mut().ok_or_else(|| ClientError::Closed {
            method: method.to_string(),
            status: None,
        })?;
        writeln!(input, "{request}")
            .and_then(|()| input.flush())
            .map_err(|error| ClientError::Io {
                method: method.to_string(),
                error,
            })?;
        let timeout = self.options.timeouts.of(method);
        match self.replies.recv_timeout(timeout) {
            Ok(Incoming::Reply(reply)) => read_reply(method, id, &reply),
            Ok(Incoming::Bad(detail)) => Err(ClientError::Malformed {
                method: method.to_string(),
                detail,
            }),
            Ok(Incoming::Closed) | Err(RecvTimeoutError::Disconnected) => {
                Err(ClientError::Closed {
                    method: method.to_string(),
                    status: self.stop(),
                })
            }
            Err(RecvTimeoutError::Timeout) => {
                self.kill();
                Err(ClientError::Timeout {
                    method: method.to_string(),
                    after: timeout,
                })
            }
        }
    }

    /// Calls `method` and reads the reply as the method's result type.
    fn call<R: DeserializeOwned>(
        &mut self,
        method: &str,
        params: &impl Serialize,
    ) -> Result<R, ClientError> {
        let params = serde_json::to_value(params).expect("protocol params are JSON");
        let result = self.request(method, params)?;
        serde_json::from_value(result).map_err(|error| malformed(method, error))
    }

    pub fn initialize(
        &mut self,
        params: &InitializeParams,
    ) -> Result<InitializeResult, ClientError> {
        self.call("initialize", params)
    }

    /// One page of tasks.
    pub fn list(&mut self, params: &ListParams) -> Result<ListResult, ClientError> {
        self.call("list", params)
    }

    /// Every task, following `nextCursor` to the end, and every item the source could not read. `checkout` names
    /// the checkout to read; none means the main checkout.
    pub fn list_all(
        &mut self,
        checkout: Option<&str>,
    ) -> Result<(Vec<Task>, Vec<TaskInvalid>), ClientError> {
        let mut params = ListParams {
            checkout: checkout.map(str::to_string),
            cursor: None,
        };
        let (mut tasks, mut invalid) = (Vec::new(), Vec::new());
        loop {
            let page = self.list(&params)?;
            tasks.extend(page.tasks);
            invalid.extend(page.invalid);
            match page.next_cursor {
                Some(cursor) => params.cursor = Some(cursor),
                None => return Ok((tasks, invalid)),
            }
        }
    }

    pub fn get(&mut self, id: &str, checkout: Option<&str>) -> Result<Option<Task>, ClientError> {
        let params = GetParams {
            id: id.to_string(),
            checkout: checkout.map(str::to_string),
        };
        let result: GetResult = self.call("get", &params)?;
        Ok(result.task)
    }

    pub fn watch(&mut self, params: &WatchParams) -> Result<WatchResult, ClientError> {
        self.call("watch", params)
    }

    pub fn apply(&mut self, params: &ApplyParams) -> Result<ApplyResult, ClientError> {
        self.call("apply", params)
    }

    pub fn preview(&mut self, params: &ApplyParams) -> Result<PreviewResult, ClientError> {
        self.call("preview", params)
    }
}

/// The `result` of a reply, or the error it carries.
fn read_reply(method: &str, id: u64, reply: &Value) -> Result<Value, ClientError> {
    if reply["id"] != json!(id) {
        return Err(ClientError::Malformed {
            method: method.to_string(),
            detail: format!(
                "a reply to request {} arrived while waiting for {id}",
                reply["id"]
            ),
        });
    }
    if let Some(error) = reply.get("error") {
        return Err(ClientError::Rpc {
            method: method.to_string(),
            code: error["code"].as_i64().unwrap_or(0),
            message: error["message"]
                .as_str()
                .unwrap_or("no message")
                .to_string(),
        });
    }
    reply
        .get("result")
        .cloned()
        .ok_or_else(|| ClientError::Malformed {
            method: method.to_string(),
            detail: "the reply has neither result nor error".to_string(),
        })
}

fn malformed(method: &str, error: serde_json::Error) -> ClientError {
    ClientError::Malformed {
        method: method.to_string(),
        detail: error.to_string(),
    }
}

/// The reader thread of one child: replies go to `replies`, notifications to `events`, and the end of the output is
/// told to the caller and noted in `ended`. It ends with the output, which a stopped child closes.
fn read_output(
    mut output: BufReader<ChildStdout>,
    replies: &Sender<Incoming>,
    events: &Sender<ProviderEvent>,
    ended: &AtomicBool,
) {
    let mut bytes = Vec::new();
    loop {
        bytes.clear();
        match output.read_until(b'\n', &mut bytes) {
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }
        let line = String::from_utf8_lossy(&bytes);
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        match serde_json::from_str::<Value>(line) {
            Err(error) => {
                let _ = replies.send(Incoming::Bad(format!("`{line}` is not JSON: {error}")));
            }
            Ok(message) if message.get("id").is_none_or(Value::is_null) => {
                let method = message["method"].as_str().unwrap_or_default();
                let params = message.get("params").cloned().unwrap_or_else(|| json!({}));
                let _ = events.send(ProviderEvent::from_notification(method, params));
            }
            Ok(message) => {
                let _ = replies.send(Incoming::Reply(message));
            }
        }
    }
    ended.store(true, Ordering::SeqCst);
    let _ = replies.send(Incoming::Closed);
}

impl Drop for ProviderClient {
    fn drop(&mut self) {
        self.stop();
    }
}
