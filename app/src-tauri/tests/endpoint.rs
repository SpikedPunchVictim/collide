//! The listener of the UI-tools endpoint (0.2, The UI-tools endpoint; ts-0199-xz), over a real loopback socket with a
//! fake window: each guard, the answers of the shell alone, the three texts of a window that is closed or silent, a
//! call answered by the window's reply, the token across starts and rotations, the port kept in the user's settings,
//! the external call limit, and the endpoint token's absence from every log line.
//!
//! The rules: a socket binds 127.0.0.1 on a port the system picks (the one kept by a first start is bound again by the
//! next, never a fixed one). The clock is fake, so no test waits 10 s or 60 s. Tokens go through `MemorySecretStore`,
//! never the keychain. The port's settings are written to the temp root's own user config, by an in-process daemon.

mod common;

use collide_app::endpoint::{DaemonPortStore, EndpointServer, Log, MemoryPortStore, PortStore};
use collide_app::shell::{
    ChannelEvent, ChannelWindow, DaemonLink, EventChannel, RecordingOpener, Shell, ShellHandle,
};
use collide_core::owner::{Request, Response, RpcError, code};
use collide_core::ui_tools::{
    MAX_EXTERNAL_CALLS, NO_WINDOW_TEXT, REQUEST_EVENT, TIMEOUT_TEXT, ToolManifest,
};
use collide_host::secret_store::{MemorySecretStore, Secret, SecretStore, SecretStoreError};
use collide_host::ui_tools::{
    ENDPOINT_TOKEN_ITEM, MAX_BODY_BYTES, SecretStoreTokens, UiToolsEndpoint,
};
use common::{InProcessStarter, TempRoot, TestClocks, access, wait_until};
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

const WAIT: Duration = Duration::from_secs(20);
const SEEDED_TOKEN: &str = "seeded-endpoint-token-5f3a9c71d2e84b60";

// ---- fakes ------------------------------------------------------------------------------------------------------

/// A window that is open or closed as a test says, and keeps what it was sent.
#[derive(Clone)]
struct Gate {
    open: Arc<AtomicBool>,
    events: Arc<Mutex<Vec<ChannelEvent>>>,
}

impl Gate {
    fn new(open: bool) -> Self {
        Gate {
            open: Arc::new(AtomicBool::new(open)),
            events: Arc::default(),
        }
    }

    /// The `ui_tools.request` events the window was sent, in order.
    fn requests(&self) -> Vec<ChannelEvent> {
        self.events
            .lock()
            .unwrap()
            .iter()
            .filter(|event| event.method == REQUEST_EVENT)
            .cloned()
            .collect()
    }

    fn wait_for_requests(&self, count: usize) -> Vec<ChannelEvent> {
        wait_until("the window is sent the call", WAIT, || {
            self.requests().len() >= count
        });
        self.requests()
    }
}

impl EventChannel for Gate {
    fn is_open(&self) -> bool {
        self.open.load(Ordering::SeqCst)
    }

    fn emit(&self, event: &ChannelEvent) {
        self.events.lock().unwrap().push(event.clone());
    }
}

/// A link to a daemon that is not there: no test here forwards anything.
struct NoDaemon;

impl DaemonLink for NoDaemon {
    fn call(&mut self, request: &Request) -> Response {
        Response::failure(
            Some(request.id.clone()),
            RpcError::new(code::UNAVAILABLE, "no daemon in this test"),
        )
    }
}

/// A store in memory whose reads fail while a test says so.
#[derive(Default)]
struct FlakyStore {
    inner: MemorySecretStore,
    down: AtomicBool,
}

impl SecretStore for FlakyStore {
    fn set(&self, item: &str, secret: &Secret) -> Result<(), SecretStoreError> {
        self.inner.set(item, secret)
    }

    fn contains(&self, item: &str) -> Result<bool, SecretStoreError> {
        self.inner.contains(item)
    }

    fn get(&self, item: &str) -> Result<Secret, SecretStoreError> {
        if self.down.load(Ordering::SeqCst) {
            return Err(SecretStoreError::Unavailable {
                reason: "the keychain is down".into(),
            });
        }
        self.inner.get(item)
    }

    fn delete(&self, item: &str) -> Result<(), SecretStoreError> {
        self.inner.delete(item)
    }
}

/// Every line written to the log, by the tokens and by the listener.
#[derive(Clone, Default)]
struct Lines(Arc<Mutex<Vec<String>>>);

impl Lines {
    fn sink(&self) -> Log {
        let lines = Arc::clone(&self.0);
        Arc::new(move |line| lines.lock().unwrap().push(line.to_string()))
    }

    fn all(&self) -> Vec<String> {
        self.0.lock().unwrap().clone()
    }
}

fn manifest() -> ToolManifest {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/ui-tools/manifest.json");
    let text = std::fs::read_to_string(path).expect("the manifest fixture");
    ToolManifest::from_json(&text).expect("the fixture is a manifest")
}

// ---- the world --------------------------------------------------------------------------------------------------

/// A listener on a port of the system's choosing, over a shell with a fake window, a fake clock and a token store in
/// memory. The server is the first field, so it stops before the shell goes.
struct World {
    server: EndpointServer,
    shell: Arc<ShellHandle>,
    window: Gate,
    clocks: TestClocks,
    store: Arc<FlakyStore>,
    log: Lines,
}

fn shell_over(
    window: &Gate,
    clocks: &TestClocks,
    store: &Arc<FlakyStore>,
    log: &Lines,
) -> Arc<ShellHandle> {
    let (window, clocks, log) = (window.clone(), clocks.clone(), log.sink());
    let secrets: Arc<dyn SecretStore> = store.clone();
    Arc::new(ShellHandle::spawn(move || {
        let endpoint = UiToolsEndpoint::new(
            0,
            manifest(),
            Box::new(SecretStoreTokens::new(secrets, log)),
            Box::new(ChannelWindow::new(Arc::new(window))),
            Box::new(clocks),
        )
        .expect("the fixture manifest is valid");
        Shell::new(
            endpoint,
            Box::new(NoDaemon),
            Box::new(RecordingOpener::default()),
        )
    }))
}

fn world(open: bool) -> World {
    let store = Arc::new(FlakyStore::default());
    store
        .set(ENDPOINT_TOKEN_ITEM, &Secret::new(SEEDED_TOKEN))
        .unwrap();
    world_over(store, open)
}

fn world_over(store: Arc<FlakyStore>, open: bool) -> World {
    let window = Gate::new(open);
    let clocks = TestClocks::default();
    let log = Lines::default();
    let shell = shell_over(&window, &clocks, &store, &log);
    let server = EndpointServer::start(&MemoryPortStore::default(), Arc::clone(&shell), log.sink())
        .expect("the listener starts");
    World {
        server,
        shell,
        window,
        clocks,
        store,
        log,
    }
}

impl World {
    fn port(&self) -> u16 {
        self.server.port()
    }

    fn mcp(&self) -> Req {
        Req::mcp(self.port(), SEEDED_TOKEN)
    }

    /// Sends `req` and waits for the answer.
    fn send(&self, req: &Req) -> Reply {
        send(self.port(), req)
    }

    /// Sends `req` from a thread, for a call that waits for the window.
    fn send_later(&self, req: Req) -> JoinHandle<Reply> {
        let port = self.port();
        thread::spawn(move || send(port, &req))
    }

    /// The window answers the call it was sent, as `ui_tools.reply` does.
    fn reply(&self, request_id: &str, text: &str) {
        let params =
            json!({ "requestId": request_id, "content": [{ "type": "text", "text": text }] });
        self.shell
            .rpc("ui_tools.reply".into(), Some(params))
            .expect("the shell takes the reply");
    }

    fn assert_nothing_reached_the_window(&self) {
        assert!(
            self.window.requests().is_empty(),
            "a request reached the window"
        );
    }
}

// ---- a client, over a socket --------------------------------------------------------------------------------------

/// A request: raw, so a test can say anything about its headers.
#[derive(Clone)]
struct Req {
    method: String,
    path: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Req {
    /// A good `POST /mcp` for the port, with the bearer.
    fn mcp(port: u16, token: &str) -> Req {
        Req {
            method: "POST".into(),
            path: "/mcp".into(),
            headers: vec![
                ("Host".into(), format!("127.0.0.1:{port}")),
                ("Content-Type".into(), "application/json".into()),
                ("Authorization".into(), format!("Bearer {token}")),
            ],
            body: Vec::new(),
        }
    }

    fn rpc(mut self, method: &str, params: Value) -> Req {
        self.body = serde_json::to_vec(&json!({
            "jsonrpc": "2.0", "id": 1, "method": method, "params": params
        }))
        .unwrap();
        self
    }

    fn tool_call(self, tool: &str) -> Req {
        self.rpc(
            "tools/call",
            json!({ "name": tool, "arguments": { "surface": "field" } }),
        )
    }

    fn with(mut self, name: &str, value: &str) -> Req {
        self = self.without(name);
        self.headers.push((name.into(), value.into()));
        self
    }

    fn without(mut self, name: &str) -> Req {
        self.headers.retain(|(n, _)| !n.eq_ignore_ascii_case(name));
        self
    }

    fn method(mut self, method: &str) -> Req {
        self.method = method.into();
        self
    }

    fn path(mut self, path: &str) -> Req {
        self.path = path.into();
        self
    }

    fn body(mut self, body: Vec<u8>) -> Req {
        self.body = body;
        self
    }
}

struct Reply {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Reply {
    fn json(&self) -> Value {
        serde_json::from_slice(&self.body)
            .unwrap_or_else(|e| panic!("not JSON ({e}): {}", self.text()))
    }

    fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }

    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    /// The text of a tool result that failed (JSON-RPC success whose result has `isError`).
    fn tool_failure(&self) -> String {
        assert_eq!(self.status, 200, "{}", self.text());
        let doc = self.json();
        assert_eq!(doc["result"]["isError"], json!(true), "{doc}");
        doc["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .to_string()
    }
}

fn connect(port: u16) -> TcpStream {
    let stream = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).expect("the listener answers");
    stream
        .set_read_timeout(Some(Duration::from_secs(60)))
        .unwrap();
    stream
}

fn send(port: u16, req: &Req) -> Reply {
    let mut head = format!("{} {} HTTP/1.1\r\n", req.method, req.path);
    for (name, value) in &req.headers {
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    head.push_str(&format!("Content-Length: {}\r\n\r\n", req.body.len()));
    let mut stream = connect(port);
    // A refused request may be answered and closed before the whole body is sent, so a write error is not a failure.
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(&req.body);
    read_reply(stream)
}

fn read_reply(mut stream: TcpStream) -> Reply {
    let mut bytes = Vec::new();
    let _ = stream.read_to_end(&mut bytes);
    let split = bytes
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .unwrap_or_else(|| panic!("no answer: {:?}", String::from_utf8_lossy(&bytes)));
    let head = String::from_utf8_lossy(&bytes[..split]).into_owned();
    let mut lines = head.split("\r\n");
    let status = lines
        .next()
        .and_then(|line| line.split(' ').nth(1))
        .and_then(|code| code.parse().ok())
        .unwrap_or_else(|| panic!("no status in {head}"));
    let headers = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(name, value)| (name.trim().to_string(), value.trim().to_string()))
        .collect();
    Reply {
        status,
        headers,
        body: bytes[split + 4..].to_vec(),
    }
}

// ---- the guards: each refused, and nothing reaches the window -----------------------------------------------------

#[test]
fn a_wrong_token_is_refused_over_the_socket_and_nothing_reaches_the_window() {
    let world = world(true);
    let wrong = world
        .mcp()
        .with("Authorization", "Bearer not-the-token")
        .tool_call("open");
    let answer = world.send(&wrong);
    assert_eq!(answer.status, 401, "{}", answer.text());
    assert_eq!(answer.header("www-authenticate"), Some("Bearer"));
    let missing = world.send(&world.mcp().without("Authorization").tool_call("open"));
    assert_eq!(missing.status, 401);
    world.assert_nothing_reached_the_window();
}

#[test]
fn a_foreign_host_is_refused_over_the_socket_and_nothing_reaches_the_window() {
    let world = world(true);
    for host in ["evil.example", "127.0.0.1:1", "127.0.0.1", "localhost"] {
        let answer = world.send(&world.mcp().with("Host", host).tool_call("open"));
        assert_eq!(answer.status, 403, "{host}: {}", answer.text());
    }
    let none = world.send(&world.mcp().without("Host").tool_call("open"));
    assert_eq!(none.status, 403);
    world.assert_nothing_reached_the_window();
}

#[test]
fn both_loopback_names_of_this_port_are_accepted_as_the_host() {
    let world = world(false);
    for host in [
        format!("127.0.0.1:{}", world.port()),
        format!("localhost:{}", world.port()),
    ] {
        let answer = world.send(&world.mcp().with("Host", &host).rpc("ping", json!({})));
        assert_eq!(answer.status, 200, "{host}: {}", answer.text());
    }
}

#[test]
fn a_foreign_origin_is_refused_over_the_socket_and_nothing_reaches_the_window() {
    let world = world(true);
    for origin in [
        "http://evil.example",
        "null",
        "https://127.0.0.1",
        "http://127.0.0.1:1",
    ] {
        let answer = world.send(&world.mcp().with("Origin", origin).tool_call("open"));
        assert_eq!(answer.status, 403, "{origin}: {}", answer.text());
    }
    world.assert_nothing_reached_the_window();
}

#[test]
fn a_content_type_that_is_not_json_is_refused_over_the_socket_and_nothing_reaches_the_window() {
    let world = world(true);
    for kind in [
        "text/plain",
        "application/x-www-form-urlencoded",
        "multipart/form-data",
    ] {
        let answer = world.send(&world.mcp().with("Content-Type", kind).tool_call("open"));
        assert_eq!(answer.status, 415, "{kind}: {}", answer.text());
    }
    let none = world.send(&world.mcp().without("Content-Type").tool_call("open"));
    assert_eq!(none.status, 415);
    world.assert_nothing_reached_the_window();
}

#[test]
fn a_body_over_1_mb_is_refused_over_the_socket_and_nothing_reaches_the_window() {
    let world = world(true);
    let over = world.mcp().body(vec![b' '; MAX_BODY_BYTES + 1]);
    let answer = world.send(&over);
    assert_eq!(answer.status, 413, "{}", answer.text());
    let far_over = world.mcp().body(vec![b' '; MAX_BODY_BYTES * 2]);
    assert_eq!(world.send(&far_over).status, 413);
    world.assert_nothing_reached_the_window();
}

#[test]
fn a_body_of_exactly_1_mb_is_read_and_answered_by_the_endpoint() {
    let world = world(false);
    let ping = br#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#;
    let mut body = ping.to_vec();
    body.resize(MAX_BODY_BYTES, b' ');
    let answer = world.send(&world.mcp().body(body));
    assert_eq!(answer.status, 200, "{}", answer.text());
}

#[test]
fn a_request_that_is_not_http_is_a_400_and_nothing_reaches_the_window() {
    let world = world(true);
    let mut stream = connect(world.port());
    stream.write_all(b"this is not HTTP\r\n\r\n").unwrap();
    assert_eq!(read_reply(stream).status, 400);
    world.assert_nothing_reached_the_window();
}

#[test]
fn a_request_that_stops_part_way_does_not_hold_the_listener() {
    let world = world(false);
    let mut stuck = connect(world.port());
    stuck
        .write_all(b"POST /mcp HTTP/1.1\r\nHost: 127.0.0.1")
        .unwrap();
    let answer = world.send(&world.mcp().rpc("ping", json!({})));
    assert_eq!(answer.status, 200, "{}", answer.text());
}

// ---- answered by the shell alone ------------------------------------------------------------------------------------

#[test]
fn initialize_ping_and_tools_list_answer_over_the_socket_with_no_window() {
    let world = world(false);
    let init = world.send(&world.mcp().rpc(
        "initialize",
        json!({ "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "t", "version": "1" } }),
    ));
    assert_eq!(init.status, 200, "{}", init.text());
    assert_eq!(init.header("content-type"), Some("application/json"));
    assert_eq!(
        init.json()["result"]["serverInfo"]["name"],
        json!("collide")
    );
    let ping = world.send(&world.mcp().rpc("ping", json!({})));
    assert_eq!(
        (ping.status, ping.json()["result"].clone()),
        (200, json!({}))
    );
    let list = world.send(&world.mcp().rpc("tools/list", json!({})));
    assert_eq!(list.status, 200);
    let tools = list.json()["result"]["tools"].clone();
    assert_eq!(tools, serde_json::to_value(manifest()).unwrap());
    world.assert_nothing_reached_the_window();
}

#[test]
fn a_notification_gets_202_over_the_socket_and_no_body() {
    let world = world(false);
    let mut note = world.mcp();
    note.body = br#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#.to_vec();
    let answer = world.send(&note);
    assert_eq!(answer.status, 202);
    assert!(answer.body.is_empty(), "{}", answer.text());
}

#[test]
fn get_and_delete_get_405_over_the_socket() {
    let world = world(false);
    for method in ["GET", "DELETE"] {
        let answer = world.send(&world.mcp().method(method));
        assert_eq!(answer.status, 405, "{method}");
        assert_eq!(answer.header("allow"), Some("POST"));
    }
}

#[test]
fn the_token_may_be_the_last_path_segment_over_the_socket() {
    let world = world(false);
    let by_path = world
        .mcp()
        .without("Authorization")
        .path(&format!("/mcp/{SEEDED_TOKEN}"))
        .rpc("ping", json!({}));
    assert_eq!(world.send(&by_path).status, 200);
    let query = world.mcp().path("/mcp?x=1").rpc("ping", json!({}));
    assert_eq!(world.send(&query).status, 200);
    assert_eq!(world.send(&world.mcp().path("/other")).status, 404);
}

// ---- a call waits for the window ------------------------------------------------------------------------------------

#[test]
fn a_call_with_the_window_closed_gives_the_open_collide_text() {
    let world = world(false);
    let answer = world.send(&world.mcp().tool_call("open"));
    assert_eq!(answer.tool_failure(), NO_WINDOW_TEXT);
    world.assert_nothing_reached_the_window();
}

#[test]
fn a_call_with_the_window_closed_when_the_client_starts_gives_the_text_and_the_window_may_open_later()
 {
    let world = world(false);
    let client = world.send(&world.mcp().rpc("initialize", json!({})));
    assert_eq!(client.status, 200);
    assert_eq!(
        world.send(&world.mcp().tool_call("open")).tool_failure(),
        NO_WINDOW_TEXT
    );
    world.window.open.store(true, Ordering::SeqCst);
    let call = world.send_later(world.mcp().tool_call("open"));
    let events = world.window.wait_for_requests(1);
    world.reply(events[0].params["requestId"].as_str().unwrap(), "done");
    assert_eq!(call.join().unwrap().status, 200);
}

#[test]
fn a_window_that_closes_while_a_call_waits_gives_the_open_collide_text() {
    let world = world(true);
    let call = world.send_later(world.mcp().tool_call("open"));
    world.window.wait_for_requests(1);
    world.shell.window_gone();
    assert_eq!(call.join().unwrap().tool_failure(), NO_WINDOW_TEXT);
}

#[test]
fn a_window_that_stays_silent_for_10_s_gives_the_timeout_text_on_a_fake_clock() {
    let world = world(true);
    let call = world.send_later(world.mcp().tool_call("open"));
    world.window.wait_for_requests(1);
    world.clocks.advance(Duration::from_secs(9));
    assert!(!call.is_finished(), "the call was answered before its 10 s");
    world.clocks.advance(Duration::from_secs(1));
    assert_eq!(call.join().unwrap().tool_failure(), TIMEOUT_TEXT);
}

#[test]
fn a_call_is_sent_to_the_window_with_its_caller_and_a_reply_answers_it() {
    let world = world(true);
    let call = world.send_later(world.mcp().tool_call("open"));
    let events = world.window.wait_for_requests(1);
    let sent = &events[0].params;
    assert_eq!(sent["caller"], json!("external"));
    assert_eq!(sent["message"]["method"], json!("tools/call"));
    world.reply(sent["requestId"].as_str().unwrap(), "opened the field");
    let answer = call.join().unwrap();
    assert_eq!(answer.status, 200, "{}", answer.text());
    assert_eq!(
        answer.json()["result"]["content"][0]["text"],
        json!("opened the field")
    );
}

#[test]
fn two_calls_waiting_at_once_are_each_answered_by_their_own_reply() {
    let world = world(true);
    let first = world.send_later(world.mcp().tool_call("open"));
    world.window.wait_for_requests(1);
    let second = world.send_later(world.mcp().tool_call("open"));
    let events = world.window.wait_for_requests(2);
    // The window answers the second call first.
    world.reply(events[1].params["requestId"].as_str().unwrap(), "second");
    world.reply(events[0].params["requestId"].as_str().unwrap(), "first");
    let text = |reply: Reply| reply.json()["result"]["content"][0]["text"].clone();
    assert_eq!(text(second.join().unwrap()), json!("second"));
    assert_eq!(text(first.join().unwrap()), json!("first"));
}

/// A link that keeps every request it is sent and answers each with an empty result.
#[derive(Clone, Default)]
struct RecordingLink(Arc<Mutex<Vec<Request>>>);

impl DaemonLink for RecordingLink {
    fn call(&mut self, request: &Request) -> Response {
        self.0.lock().unwrap().push(request.clone());
        Response::success(request.id.clone(), json!({}))
    }
}

#[test]
fn a_window_that_goes_cancels_each_live_ask_at_the_daemon_once() {
    let window = Gate::new(true);
    let sent = RecordingLink::default();
    let shell = {
        let (window, link) = (window.clone(), sent.clone());
        ShellHandle::spawn(move || {
            let endpoint = UiToolsEndpoint::new(
                0,
                manifest(),
                Box::new(SecretStoreTokens::new(
                    Arc::new(MemorySecretStore::default()),
                    Lines::default().sink(),
                )),
                Box::new(ChannelWindow::new(Arc::new(window))),
                Box::new(TestClocks::default()),
            )
            .expect("the fixture manifest is valid");
            let mut shell = Shell::new(
                endpoint,
                Box::new(link),
                Box::new(RecordingOpener::default()),
            );
            shell
                .endpoint_mut()
                .register_ask("ask-1", "token-1")
                .unwrap();
            shell
                .endpoint_mut()
                .register_ask("ask-2", "token-2")
                .unwrap();
            shell
        })
    };
    shell.window_gone();
    shell.window_gone();
    // `window_gone` does not wait, so a call that does is queued behind it and the cancels are done by then.
    shell.rpc("ui_tools.token".into(), Some(json!({}))).unwrap();
    let methods: Vec<(String, Value)> = sent
        .0
        .lock()
        .unwrap()
        .iter()
        .map(|request| (request.method.clone(), request.params.clone()))
        .collect();
    assert_eq!(
        methods,
        [
            ("ask.cancel".to_string(), json!({ "ask": "ask-1" })),
            ("ask.cancel".to_string(), json!({ "ask": "ask-2" })),
        ],
        "each live ask is cancelled once, however often the window goes"
    );
}

// ---- the external limit ---------------------------------------------------------------------------------------------

#[test]
fn the_21st_screen_changing_call_in_a_minute_is_refused_over_the_socket_and_the_minute_is_on_the_fake_clock()
 {
    let world = world(true);
    for n in 1..=MAX_EXTERNAL_CALLS as usize {
        let call = world.send_later(world.mcp().tool_call("open"));
        let events = world.window.wait_for_requests(n);
        world.reply(events[n - 1].params["requestId"].as_str().unwrap(), "ok");
        assert_eq!(call.join().unwrap().status, 200);
    }
    let over = world.send(&world.mcp().tool_call("open"));
    assert_eq!(
        over.tool_failure(),
        "external clients may change the screen 20 times a minute; try again in 60 s"
    );
    assert_eq!(world.window.requests().len(), 20);
    world.clocks.advance(Duration::from_secs(60));
    let call = world.send_later(world.mcp().tool_call("open"));
    let events = world.window.wait_for_requests(21);
    world.reply(events[20].params["requestId"].as_str().unwrap(), "ok");
    assert_eq!(call.join().unwrap().status, 200);
}

// ---- the token --------------------------------------------------------------------------------------------

#[test]
fn after_rotate_the_old_token_is_refused_at_once_over_the_socket() {
    let world = world(false);
    let ping = |token: &str| world.send(&Req::mcp(world.port(), token).rpc("ping", json!({})));
    assert_eq!(ping(SEEDED_TOKEN).status, 200);
    let rotated = world
        .shell
        .rpc("ui_tools.rotate".into(), Some(json!({})))
        .unwrap();
    let new = rotated["token"].as_str().unwrap().to_string();
    assert_ne!(new, SEEDED_TOKEN);
    assert_eq!(ping(SEEDED_TOKEN).status, 401);
    assert_eq!(ping(&new).status, 200);
    let told = world
        .shell
        .rpc("ui_tools.token".into(), Some(json!({})))
        .unwrap();
    assert_eq!(told["token"], json!(new));
}

#[test]
fn the_token_is_made_at_first_use_and_is_the_same_at_the_next_start() {
    let store = Arc::new(FlakyStore::default());
    let first = world_over(Arc::clone(&store), false);
    assert_eq!(
        store.contains(ENDPOINT_TOKEN_ITEM),
        Ok(false),
        "made before first use"
    );
    let token = first
        .shell
        .rpc("ui_tools.token".into(), Some(json!({})))
        .unwrap()["token"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(token.len(), 64);
    drop(first);
    let next = world_over(store, false);
    let ping = next.send(&Req::mcp(next.port(), &token).rpc("ping", json!({})));
    assert_eq!(ping.status, 200, "{}", ping.text());
}

#[test]
fn a_store_that_fails_leaves_the_endpoint_refusing_every_call_over_the_socket() {
    let store = Arc::new(FlakyStore::default());
    store.down.store(true, Ordering::SeqCst);
    let world = world_over(store, true);
    for req in [
        world.mcp().tool_call("open"),
        world.mcp().rpc("ping", json!({})),
    ] {
        let answer = world.send(&req);
        assert_eq!(answer.status, 503, "{}", answer.text());
    }
    world.assert_nothing_reached_the_window();
    assert!(
        world
            .log
            .all()
            .iter()
            .any(|line| line.contains("the keychain is down"))
    );
}

#[test]
fn the_endpoint_token_is_in_no_log_line() {
    let world = world(true);
    // The token is used in a header and in a path, a wrong one is sent, the token rotates, and the store fails.
    let by_path = world
        .mcp()
        .without("Authorization")
        .path(&format!("/mcp/{SEEDED_TOKEN}"))
        .rpc("ping", json!({}));
    world.send(&by_path);
    world.send(&world.mcp().rpc("ping", json!({})));
    world.send(
        &world
            .mcp()
            .with("Authorization", &format!("Bearer {SEEDED_TOKEN}x"))
            .rpc("ping", json!({})),
    );
    world.send(
        &world
            .mcp()
            .with("Host", SEEDED_TOKEN)
            .rpc("ping", json!({})),
    );
    let rotated = world
        .shell
        .rpc("ui_tools.rotate".into(), Some(json!({})))
        .unwrap();
    let new = rotated["token"].as_str().unwrap().to_string();
    world.store.down.store(true, Ordering::SeqCst);
    world
        .shell
        .rpc("ui_tools.rotate".into(), Some(json!({})))
        .ok();
    // The cache serves the token while the store is down, so a new shell reads the store and fails.
    let broken = world_over(Arc::clone(&world.store), true);
    broken.send(&Req::mcp(broken.port(), SEEDED_TOKEN).rpc("ping", json!({})));
    broken.send(&Req::mcp(broken.port(), &new).rpc("ping", json!({})));
    let lines: Vec<String> = world
        .log
        .all()
        .into_iter()
        .chain(broken.log.all())
        .collect();
    assert!(!lines.is_empty(), "the failing store told nothing");
    for line in &lines {
        assert!(!line.contains(SEEDED_TOKEN), "{line}");
        assert!(!line.contains(&new), "{line}");
    }
}

// ---- the port, kept in the user's settings ----------------------------------------------------------------

fn user_config(root: &TempRoot) -> String {
    std::fs::read_to_string(root.path().join(".config/collide/config.yaml")).unwrap_or_default()
}

/// A listener whose port is kept in the temp root's user config, through an in-process daemon.
fn start_over_the_daemon(
    root: &TempRoot,
    starter: &InProcessStarter,
) -> Result<(EndpointServer, Arc<ShellHandle>), String> {
    let clocks = TestClocks::default();
    let store = Arc::new(FlakyStore::default());
    let window = Gate::new(false);
    let log = Lines::default();
    let shell = shell_over(&window, &clocks, &store, &log);
    let ports = DaemonPortStore::new(access(root, starter.clone(), &clocks));
    EndpointServer::start(&ports, Arc::clone(&shell), log.sink()).map(|server| (server, shell))
}

#[test]
fn the_first_start_writes_the_port_it_got_to_the_user_config_and_the_next_start_binds_it() {
    let root = TempRoot::new();
    let starter = InProcessStarter::new(&root);
    assert_eq!(user_config(&root), "");
    let (first, _shell) = start_over_the_daemon(&root, &starter).expect("the first start");
    let port = first.port();
    assert_ne!(port, 0);
    let written = user_config(&root);
    assert!(written.contains("ui_tools"), "{written}");
    assert!(written.contains(&format!("port: {port}")), "{written}");
    drop(first);
    let (second, _shell) = start_over_the_daemon(&root, &starter).expect("the next start");
    assert_eq!(
        second.port(),
        port,
        "the next start must bind the kept port"
    );
    assert!(user_config(&root).contains(&format!("port: {port}")));
}

#[test]
fn a_port_in_use_fails_the_start_and_the_message_names_it() {
    let root = TempRoot::new();
    let starter = InProcessStarter::new(&root);
    let (first, _shell) = start_over_the_daemon(&root, &starter).expect("the first start");
    let port = first.port();
    // The first listener still holds the port the settings keep.
    let error = start_over_the_daemon(&root, &starter)
        .err()
        .expect("the port is held");
    assert!(error.contains(&port.to_string()), "{error}");
    assert!(error.contains("ui_tools.port"), "{error}");
    assert_eq!(first.port(), port);
}

#[test]
fn a_port_held_by_another_program_fails_the_start_and_names_it() {
    let holder = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let port = holder.local_addr().unwrap().port();
    let clocks = TestClocks::default();
    let store = Arc::new(FlakyStore::default());
    let shell = shell_over(&Gate::new(false), &clocks, &store, &Lines::default());
    let error = EndpointServer::start(
        &MemoryPortStore::holding(port),
        shell,
        Lines::default().sink(),
    )
    .err()
    .expect("the port is held");
    assert!(error.contains(&format!("port {port}")), "{error}");
}

#[test]
fn a_start_with_no_kept_port_keeps_the_one_it_got() {
    let ports = MemoryPortStore::default();
    let clocks = TestClocks::default();
    let store = Arc::new(FlakyStore::default());
    let shell = shell_over(&Gate::new(false), &clocks, &store, &Lines::default());
    let server = EndpointServer::start(&ports, shell, Lines::default().sink()).unwrap();
    assert_eq!(ports.saved(), Ok(Some(server.port())));
}

#[test]
fn a_port_that_cannot_be_kept_fails_the_start_and_says_so() {
    struct Refusing;
    impl PortStore for Refusing {
        fn saved(&self) -> Result<Option<u16>, String> {
            Ok(None)
        }
        fn save(&self, _: u16) -> Result<(), String> {
            Err("the settings file has a comment".to_string())
        }
    }
    let clocks = TestClocks::default();
    let store = Arc::new(FlakyStore::default());
    let shell = shell_over(&Gate::new(false), &clocks, &store, &Lines::default());
    let error = EndpointServer::start(&Refusing, shell, Lines::default().sink())
        .err()
        .expect("the port is not kept");
    assert!(error.contains("ui_tools.port"), "{error}");
    assert!(error.contains("the settings file has a comment"), "{error}");
}

#[test]
fn the_listener_is_on_the_loopback_only() {
    let world = world(false);
    let local = TcpStream::connect((Ipv4Addr::LOCALHOST, world.port())).unwrap();
    assert_eq!(local.peer_addr().unwrap().ip(), Ipv4Addr::LOCALHOST);
    assert!(local.local_addr().unwrap().ip().is_loopback());
}

#[test]
fn a_stopped_server_frees_its_port() {
    let world = world(false);
    let port = world.port();
    drop(world);
    TcpListener::bind((Ipv4Addr::LOCALHOST, port))
        .expect("the port is free once the server is dropped");
}
