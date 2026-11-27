//! The UI-tools request handler (0.2, The UI-tools endpoint; 0.9, an agent's tool inputs are untrusted; ts-0035-re),
//! as pure code over small traits, so a test uses a fake clock and a fake window and no real HTTP listener. The
//! listener is the shell's (`app/src-tauri/src/endpoint.rs`, ts-0199-xz): it reads a request into an
//! [`HttpRequest`], calls [`UiToolsEndpoint::handle`], and writes the [`HttpResponse`] that comes back. The endpoint
//! token lives in the OS keychain ([`SecretStoreTokens`]).
//!
//! ## What the handler decides
//!
//! The shell answers the protocol and the window answers the tools. In this order, [`UiToolsEndpoint::handle`]:
//!
//! 1. refuses a repeated `Host`, `Origin`, `Content-Type` or `Authorization` header (400), a `Host` that is not
//!    `127.0.0.1:<port>` or `localhost:<port>` (403) and an `Origin`, when there is one, that is not
//!    `http://127.0.0.1:<port>` or `http://localhost:<port>` (403);
//! 2. refuses a path other than `/mcp` and `/mcp/<token>` (404), a token that is missing or wrong (401), and every
//!    call when the endpoint token cannot be read or made (503, with the reason in the log). The
//!    token is `Authorization: Bearer <token>`, or the last path segment when there is no header; a header that is
//!    wrong is not rescued by a good path. A token is the endpoint token (a client of 0.7, the caller `external`) or
//!    an ask's ([`UiToolsEndpoint::register_ask`], the caller is the ask). Every candidate is compared with
//!    [`collide_core::owner::tokens_match`] and none ends the comparison early;
//! 3. answers `405` to any method but `POST` (so `GET` and `DELETE`), a content type that is not JSON with `415`, and
//!    a body over [`MAX_BODY_BYTES`] with `413`;
//! 4. reads the body as one JSON-RPC message (a batch is refused), then by what it is: a notification or a response
//!    gets `202` and no body, `initialize`, `ping` and `tools/list` are answered here from the manifest, an unknown
//!    method is a JSON-RPC error, and only `tools/call` goes on, as one [`UiToolsRequest`] to the [`Window`].
//!
//! A refusal is a JSON-RPC error body that says what to change and never holds a token.
//!
//! ## A call waits for the window
//!
//! [`UiToolsEndpoint::handle`] cannot hold a thread for 10 s, so a forwarded `tools/call` returns
//! [`Handled::Pending`] with its `requestId`, and the answer comes later as a [`Completed`]: from the window's
//! `ui_tools.reply` ([`UiToolsEndpoint::rpc`]), from [`UiToolsEndpoint::expire`] when the deadline has passed
//! ([`TIMEOUT_TEXT`]), or from [`UiToolsEndpoint::window_gone`] ([`NO_WINDOW_TEXT`]). A reply to a request that was
//! answered, expired or never made is ignored, and so is a reply that comes at or after the deadline, whether or not
//! `expire` has run. A failure is a tool result with `isError`, as MCP says, so the agent reads the text.
//!
//! The deadline is measured on the uptime clock (0.2, Clocks: a work timeout). The `deadline` the window is told is
//! the wall clock's, in Unix milliseconds, because the window cannot read the uptime clock.
//!
//! ## The cap
//!
//! An ask makes at most [`MAX_SCREEN_CHANGING_CALLS`] screen-changing calls (0.9). A call is screen-changing unless
//! the manifest marks its tool read-only ([`ToolManifest::is_read_only`]), so a tool the manifest does not hold counts.
//! The count lives here, not in the window, so a reload does not reset it, and a call that does not reach a window
//! is not counted.
//!
//! A client of the endpoint token has no ask and no per-ask cap. Its limit is shared by every such client (0.7): at
//! most [`MAX_EXTERNAL_CALLS`] screen-changing calls in a rolling [`EXTERNAL_CALL_WINDOW_SECS`], counted here on the
//! uptime clock. The call over the limit fails with a text that says how long to wait, and is not counted. An ask's
//! calls are never counted against it.
//!
//! ## Asks and the window
//!
//! The shell registers an ask with its token when it forwards `ask.start` ([`UiToolsEndpoint::register_ask`]) and
//! ends it when the ask's outcome arrives ([`UiToolsEndpoint::end_ask`]), which is when its token dies (0.2). When the
//! window closes or reloads the shell calls [`UiToolsEndpoint::window_gone`], which names every live ask for the
//! `ask.cancel` the shell sends; an ask is named once, and [`UiToolsEndpoint::live_asks`] is empty for the new
//! window. A cancelled ask keeps its token and its count until `end_ask`, because the daemon's cancel takes a moment.
//!
//! ## The window's methods
//!
//! [`UiToolsEndpoint::rpc`] takes a request the window sent over `rpc`. Every `ui_tools.*` method is answered here
//! ([`collide_core::ui_tools::UiToolsMethod`], and an unknown one is `METHOD_NOT_FOUND`) and none is forwarded to the
//! daemon; anything else is [`Rpc::Forward`].
//!
//! The tag `ui_tools: endpoint token store` is filled by [`SecretStoreTokens`] (ts-0199-xz): the endpoint token in the
//! keychain, made at first use, read, and rotated.

use crate::clock::{Clock, Clocks, Reading};
use crate::secret_store::{Secret, SecretStore, SecretStoreError};
use collide_core::ask::AskCancelParams;
use collide_core::non_empty::NonEmpty;
use collide_core::owner::{Request, RequestId, Response, RpcError, code, tokens_match};
use collide_core::ui_tools::{
    EXTERNAL_CALL_WINDOW_SECS, EXTERNAL_CALLER, MAX_EXTERNAL_CALLS, MAX_SCREEN_CHANGING_CALLS,
    ManifestProblem, NO_WINDOW_TEXT, REQUEST_DEADLINE_SECS, TIMEOUT_TEXT, ToolManifest,
    UiToolsMethod, UiToolsReplyParams, UiToolsRequest, UiToolsRotateParams, UiToolsRotateResult,
    UiToolsTokenParams, UiToolsTokenResult,
};
use serde_json::{Value, json};
use std::cell::RefCell;
use std::collections::{BTreeMap, VecDeque};
use std::fmt;
use std::io::{self, Read};
use std::sync::Arc;

/// The largest body the handler reads, in bytes: 1 MB (0.2).
pub const MAX_BODY_BYTES: usize = 1024 * 1024;

/// The MCP protocol versions `initialize` answers, newest first. A client that asks for one of them gets it; any other
/// gets the first.
pub const SUPPORTED_PROTOCOLS: [&str; 3] = ["2025-06-18", "2025-03-26", "2024-11-05"];

/// The path of the endpoint. A client that cannot send headers puts its token after it: `/mcp/<token>`.
pub const ENDPOINT_PATH: &str = "/mcp";

const RESULT_IS_JSON: &str = "a result of this protocol is JSON";
const NANOS_PER_SEC: u64 = 1_000_000_000;
const NANOS_PER_MILLI: u64 = 1_000_000;

// ---- the values -------------------------------------------------------------------------------------------------

/// An HTTP request as the listener read it. `path` holds the query when there is one, which the handler ignores.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpRequest {
    pub method: String,
    pub path: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

/// An HTTP answer for the listener to write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl HttpResponse {
    fn empty(status: u16) -> Self {
        HttpResponse {
            status,
            headers: Vec::new(),
            body: Vec::new(),
        }
    }

    fn json(status: u16, message: &Response) -> Self {
        HttpResponse {
            status,
            headers: vec![("Content-Type".to_string(), "application/json".to_string())],
            body: serde_json::to_vec(message).expect("a JSON-RPC answer is JSON"),
        }
    }
}

/// What [`UiToolsEndpoint::handle`] found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Handled {
    /// The answer, now.
    Response(HttpResponse),
    /// A `tools/call` that went to the window. Its answer comes later as a [`Completed`] with this `requestId`.
    Pending { request_id: String },
}

/// The answer to a call that was pending.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Completed {
    pub request_id: String,
    pub response: HttpResponse,
}

/// What [`UiToolsEndpoint::window_gone`] found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowGone {
    /// The live asks, in id order, to cancel: the params of `ask.cancel` (ts-0034-hh).
    pub cancel: Vec<AskCancelParams>,
    /// The calls that were waiting on the window, answered with [`NO_WINDOW_TEXT`].
    pub completed: Vec<Completed>,
}

/// What [`UiToolsEndpoint::rpc`] did with a request of the window's.
#[derive(Debug, Clone, PartialEq)]
pub enum Rpc {
    /// Not a `ui_tools.*` method: the shell sends it on to the daemon.
    Forward,
    /// A `ui_tools.*` method, answered here.
    Handled(RpcHandled),
}

/// The answer to a `ui_tools.*` request, and the calls it finished (a reply finishes one).
#[derive(Debug, Clone, PartialEq)]
pub struct RpcHandled {
    pub response: Response,
    pub completed: Vec<Completed>,
}

/// An ask that cannot be registered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AskRegistration {
    EmptyId,
    ReservedId,
    DuplicateId(String),
    EmptyToken,
    TokenInUse,
    /// The endpoint token cannot be read, so it cannot be told apart from the ask's.
    TokenStoreFailed,
}

impl fmt::Display for AskRegistration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AskRegistration::EmptyId => write!(f, "an ask needs an id"),
            AskRegistration::ReservedId => {
                write!(
                    f,
                    "`{EXTERNAL_CALLER}` is the caller of the endpoint token and cannot be an ask's id"
                )
            }
            AskRegistration::DuplicateId(id) => write!(f, "the ask `{id}` is already registered"),
            AskRegistration::EmptyToken => write!(f, "an ask needs a token"),
            AskRegistration::TokenInUse => {
                write!(
                    f,
                    "the ask's token is already the endpoint's or another ask's"
                )
            }
            AskRegistration::TokenStoreFailed => {
                write!(
                    f,
                    "the endpoint token cannot be read, so no ask is registered"
                )
            }
        }
    }
}

impl std::error::Error for AskRegistration {}

// ---- the traits -------------------------------------------------------------------------------------------------

/// The window, as the handler sees it. The real one is the shell's webview (Stage 6b).
pub trait Window {
    /// Whether a window is open to answer.
    fn is_open(&self) -> bool;
    /// Sends the window one request. The window answers with `ui_tools.reply`.
    fn send(&mut self, event: UiToolsRequest);
}

/// Why the endpoint token cannot be read or made. The words say what failed and never hold a token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenFailure(pub String);

impl fmt::Display for TokenFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for TokenFailure {}

/// The endpoint token (0.2): 32 random bytes in the OS keychain, made at first use. The handler asks for it on every
/// request, so a rotation ends the old token at once; the real store caches it. A store that fails is an error, and
/// the handler then refuses every call.
pub trait EndpointTokens {
    fn current(&self) -> Result<String, TokenFailure>;
    /// Makes a new token, keeps it in place of the old and returns it. A failure keeps the old one.
    fn rotate(&mut self) -> Result<String, TokenFailure>;
}

/// The keychain item the endpoint token is kept under.
pub const ENDPOINT_TOKEN_ITEM: &str = "ui_tools.endpoint_token";

/// How many random bytes the endpoint token has (0.2).
const TOKEN_BYTES: usize = 32;

/// A token of [`TOKEN_BYTES`] random bytes from the system, as lower-case hex.
pub fn random_token() -> io::Result<String> {
    let mut bytes = [0u8; TOKEN_BYTES];
    std::fs::File::open("/dev/urandom").and_then(|mut random| random.read_exact(&mut bytes))?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

/// The real [`EndpointTokens`], over a [`SecretStore`] (the keychain in the app, memory in a test). The token is made
/// the first time it is asked for and kept under [`ENDPOINT_TOKEN_ITEM`], so the next start reads the same one. A
/// failure of the store is told once to `log`, as a plain line that holds no token.
pub struct SecretStoreTokens {
    store: Arc<dyn SecretStore>,
    log: Arc<dyn Fn(&str) + Send + Sync>,
    cached: RefCell<Option<String>>,
}

impl SecretStoreTokens {
    pub fn new(store: Arc<dyn SecretStore>, log: Arc<dyn Fn(&str) + Send + Sync>) -> Self {
        SecretStoreTokens {
            store,
            log,
            cached: RefCell::new(None),
        }
    }

    /// The token in the store, or a new one that is kept there when the store has none.
    fn read_or_make(&self) -> Result<String, TokenFailure> {
        match self.store.get(ENDPOINT_TOKEN_ITEM) {
            Ok(secret) if !secret.expose().is_empty() => Ok(secret.expose().to_string()),
            Ok(_) | Err(SecretStoreError::NotFound { .. }) => self.make_and_keep(),
            Err(error) => Err(TokenFailure(error.to_string())),
        }
    }

    fn make_and_keep(&self) -> Result<String, TokenFailure> {
        let token = random_token()
            .map_err(|error| TokenFailure(format!("could not read random bytes: {error}")))?;
        self.store
            .set(ENDPOINT_TOKEN_ITEM, &Secret::new(token.clone()))
            .map_err(|error| TokenFailure(error.to_string()))?;
        Ok(token)
    }

    fn told(&self, result: Result<String, TokenFailure>) -> Result<String, TokenFailure> {
        if let Err(failure) = &result {
            (self.log)(&format!(
                "collide: the UI-tools endpoint token is unavailable, so the endpoint refuses every call: {failure}"
            ));
        }
        result
    }
}

impl EndpointTokens for SecretStoreTokens {
    fn current(&self) -> Result<String, TokenFailure> {
        if let Some(token) = self.cached.borrow().as_ref() {
            return Ok(token.clone());
        }
        let token = self.told(self.read_or_make())?;
        *self.cached.borrow_mut() = Some(token.clone());
        Ok(token)
    }

    fn rotate(&mut self) -> Result<String, TokenFailure> {
        let token = self.told(self.make_and_keep())?;
        *self.cached.borrow_mut() = Some(token.clone());
        Ok(token)
    }
}

// ---- the endpoint -----------------------------------------------------------------------------------------------

/// Who a token belongs to.
enum Client {
    External,
    Ask(String),
}

struct AskState {
    token: String,
    calls: u32,
    cancelled: bool,
}

struct PendingCall {
    request_id: String,
    rpc_id: RequestId,
    deadline: Reading,
}

/// The handler. See the module doc.
pub struct UiToolsEndpoint {
    port: u16,
    manifest: ToolManifest,
    tokens: Box<dyn EndpointTokens>,
    window: Box<dyn Window>,
    clocks: Box<dyn Clocks>,
    asks: BTreeMap<String, AskState>,
    pending: Vec<PendingCall>,
    next_request: u64,
    /// The uptime, in nanoseconds, of each screen-changing call of the endpoint token in the last
    /// [`EXTERNAL_CALL_WINDOW_SECS`], oldest first.
    external_calls: VecDeque<u64>,
}

impl UiToolsEndpoint {
    /// A handler for the endpoint on `port`, answering `tools/list` from `manifest`. A manifest that fails
    /// [`ToolManifest::check`] is refused here, not at the first `tools/list`.
    pub fn new(
        port: u16,
        manifest: ToolManifest,
        tokens: Box<dyn EndpointTokens>,
        window: Box<dyn Window>,
        clocks: Box<dyn Clocks>,
    ) -> Result<UiToolsEndpoint, ManifestProblem> {
        manifest.check()?;
        Ok(UiToolsEndpoint {
            port,
            manifest,
            tokens,
            window,
            clocks,
            asks: BTreeMap::new(),
            pending: Vec::new(),
            next_request: 0,
            external_calls: VecDeque::new(),
        })
    }

    /// The port the listener got, which `Host` and `Origin` must name. The shell tells it once the listener is bound,
    /// because a first start lets the system pick the port.
    pub fn set_port(&mut self, port: u16) {
        self.port = port;
    }

    /// Registers an ask and its token, which the shell made when it forwarded `ask.start`.
    pub fn register_ask(&mut self, ask: &str, token: &str) -> Result<(), AskRegistration> {
        if ask.is_empty() {
            return Err(AskRegistration::EmptyId);
        }
        if ask == EXTERNAL_CALLER {
            return Err(AskRegistration::ReservedId);
        }
        if self.asks.contains_key(ask) {
            return Err(AskRegistration::DuplicateId(ask.to_string()));
        }
        if token.is_empty() {
            return Err(AskRegistration::EmptyToken);
        }
        let endpoint_token = self
            .tokens
            .current()
            .map_err(|_| AskRegistration::TokenStoreFailed)?;
        let in_use = tokens_match(token, &endpoint_token)
            || self
                .asks
                .values()
                .any(|state| tokens_match(token, &state.token));
        if in_use {
            return Err(AskRegistration::TokenInUse);
        }
        self.asks.insert(
            ask.to_string(),
            AskState {
                token: token.to_string(),
                calls: 0,
                cancelled: false,
            },
        );
        Ok(())
    }

    /// Ends an ask: its token is dead and its count is dropped. An ask that is not registered is ignored.
    pub fn end_ask(&mut self, ask: &str) {
        self.asks.remove(ask);
    }

    /// The asks not yet cancelled, in id order.
    pub fn live_asks(&self) -> Vec<String> {
        self.asks
            .iter()
            .filter(|(_, state)| !state.cancelled)
            .map(|(ask, _)| ask.clone())
            .collect()
    }

    /// The screen-changing calls an ask has made, or `None` for an ask that is not registered.
    pub fn calls_made(&self, ask: &str) -> Option<u32> {
        self.asks.get(ask).map(|state| state.calls)
    }

    /// The window closed or reloaded: the page that was going to answer is gone. Names each live ask to cancel, once,
    /// and answers the calls that were waiting. Counts and tokens stay until [`UiToolsEndpoint::end_ask`].
    pub fn window_gone(&mut self) -> WindowGone {
        let mut cancel = Vec::new();
        for (ask, state) in &mut self.asks {
            if !state.cancelled {
                state.cancelled = true;
                cancel.push(AskCancelParams {
                    ask: NonEmpty::new(ask.as_str()).expect("register_ask refuses an empty id"),
                });
            }
        }
        let completed = self
            .pending
            .drain(..)
            .map(|call| complete_with_text(call, NO_WINDOW_TEXT))
            .collect();
        WindowGone { cancel, completed }
    }

    /// Answers every pending call whose deadline has come with [`TIMEOUT_TEXT`]. The shell calls it on a timer.
    pub fn expire(&mut self) -> Vec<Completed> {
        let now = self.uptime();
        let (expired, waiting): (Vec<PendingCall>, Vec<PendingCall>) = self
            .pending
            .drain(..)
            .partition(|call| now.nanos >= call.deadline.nanos);
        self.pending = waiting;
        expired
            .into_iter()
            .map(|call| complete_with_text(call, TIMEOUT_TEXT))
            .collect()
    }

    /// A request the window sent over `rpc`. See the module doc, The window's methods.
    pub fn rpc(&mut self, request: &Request) -> Rpc {
        if !request.method.starts_with("ui_tools.") {
            return Rpc::Forward;
        }
        let id = request.id.clone();
        let handled = match UiToolsMethod::from_name(&request.method) {
            None => RpcHandled::failed(id, RpcError::method_not_found(&request.method)),
            Some(method) => self.run_method(method, request),
        };
        Rpc::Handled(handled)
    }

    /// One HTTP request. See the module doc.
    pub fn handle(&mut self, request: &HttpRequest) -> Handled {
        match self.handle_request(request) {
            Ok(handled) => handled,
            Err(refusal) => Handled::Response(refusal.into_response()),
        }
    }

    // ---- the window's methods ----

    fn run_method(&mut self, method: UiToolsMethod, request: &Request) -> RpcHandled {
        let id = request.id.clone();
        let params = params_or_empty(request);
        let invalid = |error: serde_json::Error| {
            RpcHandled::failed(
                id.clone(),
                RpcError::new(
                    code::INVALID_PARAMS,
                    format!("invalid params for `{}`: {error}", request.method),
                ),
            )
        };
        match method {
            UiToolsMethod::Reply => match serde_json::from_value::<UiToolsReplyParams>(params) {
                Err(error) => invalid(error),
                Ok(params) => RpcHandled {
                    completed: self.complete_reply(params),
                    response: Response::success(id, json!({})),
                },
            },
            UiToolsMethod::Token => match serde_json::from_value::<UiToolsTokenParams>(params) {
                Err(error) => invalid(error),
                Ok(UiToolsTokenParams {}) => match self.tokens.current() {
                    Err(failure) => token_unavailable(id, &failure),
                    Ok(token) => {
                        let result = UiToolsTokenResult { token };
                        RpcHandled::answered(
                            id,
                            serde_json::to_value(result).expect(RESULT_IS_JSON),
                        )
                    }
                },
            },
            UiToolsMethod::Rotate => match serde_json::from_value::<UiToolsRotateParams>(params) {
                Err(error) => invalid(error),
                Ok(UiToolsRotateParams {}) => match self.tokens.rotate() {
                    Err(failure) => token_unavailable(id, &failure),
                    Ok(token) => {
                        let result = UiToolsRotateResult { token };
                        RpcHandled::answered(
                            id,
                            serde_json::to_value(result).expect(RESULT_IS_JSON),
                        )
                    }
                },
            },
        }
    }

    /// Finishes the call a reply names, when it is still waiting and its deadline has not come.
    fn complete_reply(&mut self, params: UiToolsReplyParams) -> Vec<Completed> {
        let now = self.uptime();
        let Some(at) = self.pending.iter().position(|call| {
            call.request_id == params.request_id && now.nanos < call.deadline.nanos
        }) else {
            return Vec::new();
        };
        let call = self.pending.remove(at);
        let result = json!({ "content": params.content, "isError": params.is_error });
        vec![Completed {
            request_id: call.request_id,
            response: HttpResponse::json(200, &Response::success(call.rpc_id, result)),
        }]
    }

    // ---- HTTP ----

    fn handle_request(&mut self, request: &HttpRequest) -> Result<Handled, Refusal> {
        self.check_host_and_origin(request)?;
        let path_token = endpoint_path_token(&request.path)?;
        let client = self.authenticate(request, path_token)?;
        if request.method != "POST" {
            return Err(Refusal::new(
                405,
                code::INVALID_REQUEST,
                format!(
                    "the endpoint takes POST only, and `{}` is refused: it has no stream and no session to end",
                    request.method
                ),
            )
            .with_header("Allow", "POST"));
        }
        check_content_type(request)?;
        if request.body.len() > MAX_BODY_BYTES {
            return Err(Refusal::new(
                413,
                code::INVALID_REQUEST,
                format!(
                    "the body is {} bytes, over the 1 MB limit",
                    request.body.len()
                ),
            ));
        }
        let message = read_message(&request.body)?;
        Ok(self.dispatch(message, client))
    }

    fn check_host_and_origin(&self, request: &HttpRequest) -> Result<(), Refusal> {
        let host = single_header(request, "Host")?.map(str::to_ascii_lowercase);
        let hosts = [
            format!("127.0.0.1:{}", self.port),
            format!("localhost:{}", self.port),
        ];
        if !host.as_ref().is_some_and(|host| hosts.contains(host)) {
            return Err(Refusal::new(
                403,
                code::INVALID_REQUEST,
                format!("the Host header must be {} or {}", hosts[0], hosts[1]),
            ));
        }
        if let Some(origin) = single_header(request, "Origin")?.map(str::to_ascii_lowercase) {
            let origins = [
                format!("http://{}", hosts[0]),
                format!("http://{}", hosts[1]),
            ];
            if !origins.contains(&origin) {
                return Err(Refusal::new(
                    403,
                    code::INVALID_REQUEST,
                    "the Origin is not this endpoint: a web page cannot call it",
                ));
            }
        }
        Ok(())
    }

    /// Who the request's token belongs to. Every token is compared and the comparison never ends early.
    fn authenticate(
        &self,
        request: &HttpRequest,
        path_token: Option<&str>,
    ) -> Result<Client, Refusal> {
        let refused = || {
            Refusal::new(
                401,
                code::TOKEN_REFUSED,
                "the endpoint token is missing or wrong: send it as `Authorization: Bearer <token>`, or as the last path segment (/mcp/<token>)",
            )
            .with_header("WWW-Authenticate", "Bearer")
        };
        let given = match single_header(request, "Authorization")? {
            Some(value) => match value.split_once(' ') {
                Some((scheme, token)) if scheme.eq_ignore_ascii_case("bearer") => token.trim(),
                _ => return Err(refused()),
            },
            None => path_token.unwrap_or(""),
        };
        if given.is_empty() {
            return Err(refused());
        }
        let endpoint_token = self.tokens.current().map_err(|_| {
            Refusal::new(
                503,
                code::UNAVAILABLE,
                "the endpoint token store is not working, so the endpoint refuses every call; \
                 the reason is in Collide's log",
            )
        })?;
        let mut client = tokens_match(given, &endpoint_token).then_some(Client::External);
        for (ask, state) in &self.asks {
            if tokens_match(given, &state.token) {
                client = Some(Client::Ask(ask.clone()));
            }
        }
        client.ok_or_else(refused)
    }

    fn dispatch(&mut self, message: Message, client: Client) -> Handled {
        let Message::Request {
            id,
            method,
            params,
            raw,
        } = message
        else {
            return Handled::Response(HttpResponse::empty(202));
        };
        let answered = |result: Value| {
            Handled::Response(HttpResponse::json(
                200,
                &Response::success(id.clone(), result),
            ))
        };
        match method.as_str() {
            "initialize" => {
                let asked = params.get("protocolVersion").and_then(Value::as_str);
                let version = SUPPORTED_PROTOCOLS
                    .iter()
                    .find(|supported| Some(**supported) == asked)
                    .unwrap_or(&SUPPORTED_PROTOCOLS[0]);
                answered(json!({
                    "protocolVersion": version,
                    "capabilities": { "tools": {} },
                    "serverInfo": { "name": "collide", "version": env!("CARGO_PKG_VERSION") },
                }))
            }
            "ping" => answered(json!({})),
            "tools/list" => answered(json!({ "tools": self.manifest })),
            "tools/call" => self.tool_call(id, &params, raw, client),
            other => Handled::Response(HttpResponse::json(
                200,
                &Response::failure(Some(id), RpcError::method_not_found(other)),
            )),
        }
    }

    fn tool_call(&mut self, id: RequestId, params: &Value, raw: Value, client: Client) -> Handled {
        let Some(tool) = params.get("name").and_then(Value::as_str) else {
            return Handled::Response(HttpResponse::json(
                200,
                &Response::failure(
                    Some(id),
                    RpcError::new(
                        code::INVALID_PARAMS,
                        "tools/call needs params.name, the name of a tool from tools/list",
                    ),
                ),
            ));
        };
        if !self.window.is_open() {
            return fail_with_text(id, NO_WINDOW_TEXT);
        }
        let caller = match &client {
            Client::External => EXTERNAL_CALLER.to_string(),
            Client::Ask(ask) => ask.clone(),
        };
        if let Client::Ask(ask) = &client
            && !self.manifest.is_read_only(tool)
            && let Some(state) = self.asks.get_mut(ask)
        {
            if state.calls >= MAX_SCREEN_CHANGING_CALLS {
                return fail_with_text(
                    id,
                    &format!(
                        "this ask has made {MAX_SCREEN_CHANGING_CALLS} screen-changing calls, the most it may make; nothing was shown. Answer in words, or use query, which changes nothing"
                    ),
                );
            }
            state.calls += 1;
        }
        if matches!(client, Client::External)
            && !self.manifest.is_read_only(tool)
            && let Some(wait) = self.external_wait()
        {
            return fail_with_text(
                id,
                &format!(
                    "external clients may change the screen {MAX_EXTERNAL_CALLS} times a minute; try again in {wait} s"
                ),
            );
        }
        self.next_request += 1;
        let request_id = format!("ui-{}", self.next_request);
        let uptime = self.uptime();
        let deadline = Reading {
            clock: Clock::Uptime,
            nanos: uptime
                .nanos
                .saturating_add(REQUEST_DEADLINE_SECS * NANOS_PER_SEC),
        };
        let wall = self.clocks.read(Clock::Wall);
        self.window.send(UiToolsRequest {
            request_id: request_id.clone(),
            caller,
            deadline: wall.nanos / NANOS_PER_MILLI + REQUEST_DEADLINE_SECS * 1000,
            message: raw,
        });
        self.pending.push(PendingCall {
            request_id: request_id.clone(),
            rpc_id: id,
            deadline,
        });
        Handled::Pending { request_id }
    }

    /// Counts one screen-changing call of the endpoint token. `None` when it is within the limit, and counted;
    /// otherwise the whole seconds, at least 1, until the oldest counted call leaves the window. A refused call is not
    /// counted.
    fn external_wait(&mut self) -> Option<u64> {
        let now = self.uptime().nanos;
        let window = EXTERNAL_CALL_WINDOW_SECS * NANOS_PER_SEC;
        while self
            .external_calls
            .front()
            .is_some_and(|at| now.saturating_sub(*at) >= window)
        {
            self.external_calls.pop_front();
        }
        if self.external_calls.len() >= MAX_EXTERNAL_CALLS as usize {
            let oldest = self.external_calls.front().copied().unwrap_or(now);
            let left = (oldest + window).saturating_sub(now);
            return Some(left.div_ceil(NANOS_PER_SEC).max(1));
        }
        self.external_calls.push_back(now);
        None
    }

    fn uptime(&self) -> Reading {
        self.clocks.read(Clock::Uptime)
    }
}

impl RpcHandled {
    fn failed(id: RequestId, error: RpcError) -> Self {
        RpcHandled {
            response: Response::failure(Some(id), error),
            completed: Vec::new(),
        }
    }

    fn answered(id: RequestId, result: Value) -> Self {
        RpcHandled {
            response: Response::success(id, result),
            completed: Vec::new(),
        }
    }
}

/// The window asked for the token and the store failed: it is told why.
fn token_unavailable(id: RequestId, failure: &TokenFailure) -> RpcHandled {
    RpcHandled::failed(
        id,
        RpcError::new(
            code::INTERNAL_ERROR,
            format!("the endpoint token is unavailable: {failure}"),
        ),
    )
}

/// A request's params, with null read as `{}`, the way the owner's methods do.
fn params_or_empty(request: &Request) -> Value {
    if request.params.is_null() {
        json!({})
    } else {
        request.params.clone()
    }
}

/// A tool result that failed: the agent reads `text` and decides what to do.
fn fail_with_text(id: RequestId, text: &str) -> Handled {
    Handled::Response(text_failure(id, text))
}

fn text_failure(id: RequestId, text: &str) -> HttpResponse {
    HttpResponse::json(
        200,
        &Response::success(
            id,
            json!({ "content": [{ "type": "text", "text": text }], "isError": true }),
        ),
    )
}

fn complete_with_text(call: PendingCall, text: &str) -> Completed {
    Completed {
        request_id: call.request_id,
        response: text_failure(call.rpc_id, text),
    }
}

// ---- reading a request ------------------------------------------------------------------------------------------

/// A request that is refused: the status, and a JSON-RPC error body that says what to change.
struct Refusal {
    status: u16,
    error: RpcError,
    headers: Vec<(String, String)>,
}

impl Refusal {
    fn new(status: u16, code: i64, message: impl Into<String>) -> Self {
        Refusal {
            status,
            error: RpcError::new(code, message),
            headers: Vec::new(),
        }
    }

    fn with_header(mut self, name: &str, value: &str) -> Self {
        self.headers.push((name.to_string(), value.to_string()));
        self
    }

    fn into_response(self) -> HttpResponse {
        let mut response = HttpResponse::json(self.status, &Response::failure(None, self.error));
        response.headers.extend(self.headers);
        response
    }
}

/// The one value of a header. A header sent twice is refused: which one a proxy or a client means is a guess.
fn single_header<'a>(request: &'a HttpRequest, name: &str) -> Result<Option<&'a str>, Refusal> {
    let mut found = request
        .headers
        .iter()
        .filter(|(header, _)| header.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str());
    let first = found.next();
    if found.next().is_some() {
        return Err(Refusal::new(
            400,
            code::INVALID_REQUEST,
            format!("the {name} header was sent more than once"),
        ));
    }
    Ok(first)
}

/// The token in the path (`/mcp/<token>`), `None` for `/mcp`; a path that is neither is a 404. A query is ignored.
fn endpoint_path_token(path: &str) -> Result<Option<&str>, Refusal> {
    let path = path.split('?').next().unwrap_or("");
    let not_found = || {
        Refusal::new(
            404,
            code::INVALID_REQUEST,
            format!("the endpoint is {ENDPOINT_PATH}"),
        )
    };
    let Some(rest) = path.strip_prefix(ENDPOINT_PATH) else {
        return Err(not_found());
    };
    match rest {
        "" => Ok(None),
        _ => match rest.strip_prefix('/') {
            Some(token) if !token.contains('/') => Ok(Some(token)),
            _ => Err(not_found()),
        },
    }
}

fn check_content_type(request: &HttpRequest) -> Result<(), Refusal> {
    let media = single_header(request, "Content-Type")?
        .and_then(|value| value.split(';').next())
        .map(|media| media.trim().to_ascii_lowercase());
    if media.as_deref() == Some("application/json") {
        Ok(())
    } else {
        Err(Refusal::new(
            415,
            code::INVALID_REQUEST,
            "the content type must be application/json",
        ))
    }
}

/// One JSON-RPC message of a body.
enum Message {
    Request {
        id: RequestId,
        method: String,
        params: Value,
        raw: Value,
    },
    /// A notification, or a client's answer to a request: no answer is sent.
    NoAnswer,
}

fn read_message(body: &[u8]) -> Result<Message, Refusal> {
    let invalid = |why: &str| {
        Refusal::new(
            400,
            code::INVALID_REQUEST,
            format!("not a JSON-RPC message: {why}"),
        )
    };
    let value: Value = serde_json::from_slice(body)
        .map_err(|error| Refusal::new(400, code::PARSE_ERROR, format!("not JSON: {error}")))?;
    let Some(object) = value.as_object() else {
        return Err(invalid("send one message, not a batch or a bare value"));
    };
    if object.get("jsonrpc") != Some(&json!("2.0")) {
        return Err(invalid("`jsonrpc` must be \"2.0\""));
    }
    let Some(method) = object.get("method") else {
        let is_answer = object.contains_key("id")
            && (object.contains_key("result") || object.contains_key("error"));
        return if is_answer {
            Ok(Message::NoAnswer)
        } else {
            Err(invalid("it has no `method`"))
        };
    };
    let Some(method) = method.as_str() else {
        return Err(invalid("`method` must be text"));
    };
    let Some(id) = object.get("id") else {
        return Ok(Message::NoAnswer);
    };
    let id = serde_json::from_value::<RequestId>(id.clone())
        .map_err(|_| invalid("`id` must be a number or text"))?;
    let params = object.get("params").cloned().unwrap_or(Value::Null);
    Ok(Message::Request {
        id,
        method: method.to_string(),
        params,
        raw: value,
    })
}
