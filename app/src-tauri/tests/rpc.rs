//! The shell's `rpc` command and event channel (0.2, The UI speaks the owner's protocol; ts-0017-qb), over a fake
//! daemon link and a fake event channel: no Tauri runtime and no socket. The real link is a tagged stub (Stage 3c).

use collide_app::shell::{
    ChannelEvent, ChannelWindow, DaemonLink, EventChannel, Shell, ShellHandle,
};
use collide_core::owner::{Request, RequestId, Response, RpcError, code};
use collide_core::ui_tools::ToolManifest;
use collide_host::clock::{Clock, Clocks, Reading};
use collide_host::ui_tools::{EndpointTokens, Handled, HttpRequest, UiToolsEndpoint};
use serde_json::{Value, json};
use std::cell::Cell;
use std::path::Path;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

const PORT: u16 = 47_821;
const ENDPOINT_TOKEN: &str = "endpoint-token-0001";

// ---- fakes ----------------------------------------------------------------------------------------------------

/// A daemon link that records what it was asked and answers each call with `result`.
#[derive(Clone)]
struct FakeLink {
    calls: Arc<Mutex<Vec<Request>>>,
    result: Value,
}

impl FakeLink {
    fn answering(result: Value) -> Self {
        FakeLink {
            calls: Arc::default(),
            result,
        }
    }

    fn calls(&self) -> Vec<Request> {
        self.calls.lock().unwrap().clone()
    }
}

impl DaemonLink for FakeLink {
    fn call(&mut self, request: &Request) -> Response {
        self.calls.lock().unwrap().push(request.clone());
        Response::success(request.id.clone(), self.result.clone())
    }
}

/// An event channel that keeps what was emitted.
#[derive(Clone)]
struct FakeChannel {
    open: bool,
    events: Arc<Mutex<Vec<ChannelEvent>>>,
}

impl FakeChannel {
    fn open() -> Self {
        FakeChannel {
            open: true,
            events: Arc::default(),
        }
    }

    fn events(&self) -> Vec<ChannelEvent> {
        self.events.lock().unwrap().clone()
    }
}

impl EventChannel for FakeChannel {
    fn is_open(&self) -> bool {
        self.open
    }

    fn emit(&self, event: &ChannelEvent) {
        self.events.lock().unwrap().push(event.clone());
    }
}

#[derive(Clone, Default)]
struct FakeClocks(Rc<Cell<u64>>);

impl Clocks for FakeClocks {
    fn read(&self, clock: Clock) -> Reading {
        Reading {
            clock,
            nanos: 1_700_000_000_000_000_000 + self.0.get(),
        }
    }
}

struct FakeTokens;

impl EndpointTokens for FakeTokens {
    fn current(&self) -> String {
        ENDPOINT_TOKEN.to_string()
    }

    fn rotate(&mut self) -> String {
        "rotated-token-0001".to_string()
    }
}

fn manifest() -> ToolManifest {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/ui-tools/manifest.json");
    let text = std::fs::read_to_string(path).expect("the manifest fixture");
    ToolManifest::from_json(&text).expect("the fixture is a manifest")
}

fn endpoint(channel: &FakeChannel) -> UiToolsEndpoint {
    UiToolsEndpoint::new(
        PORT,
        manifest(),
        Box::new(FakeTokens),
        Box::new(ChannelWindow::new(Arc::new(channel.clone()))),
        Box::new(FakeClocks::default()),
    )
    .expect("the fixture manifest is valid")
}

/// A shell over a fake link (answering `{"from":"daemon"}`) and a fake open channel.
fn shell() -> (Shell, FakeLink, FakeChannel) {
    let link = FakeLink::answering(json!({ "from": "daemon" }));
    let channel = FakeChannel::open();
    let shell = Shell::new(endpoint(&channel), Box::new(link.clone()));
    (shell, link, channel)
}

fn failure(result: Result<Value, RpcError>) -> RpcError {
    result.expect_err("the call should fail")
}

// ---- ui_tools.* never goes to the daemon -----------------------------------------------------------------------

#[test]
fn a_ui_tools_reply_is_answered_locally_and_the_forward_path_is_not_called() {
    let (mut shell, link, _) = shell();
    let result = shell.rpc(
        "ui_tools.reply",
        Some(json!({ "requestId": "none", "content": [{ "type": "text", "text": "ok" }] })),
    );
    assert_eq!(result, Ok(json!({})));
    assert!(link.calls().is_empty(), "{:?}", link.calls());
}

#[test]
fn a_ui_tools_token_is_answered_locally_and_the_forward_path_is_not_called() {
    let (mut shell, link, _) = shell();
    let result = shell.rpc("ui_tools.token", None);
    assert_eq!(result, Ok(json!({ "token": ENDPOINT_TOKEN })));
    assert!(link.calls().is_empty());
}

#[test]
fn an_unknown_ui_tools_method_is_refused_here_and_is_not_forwarded() {
    let (mut shell, link, _) = shell();
    let error = failure(shell.rpc("ui_tools.nothing", None));
    assert_eq!(error.code, code::METHOD_NOT_FOUND);
    assert!(link.calls().is_empty());
}

// ---- everything else is forwarded ------------------------------------------------------------------------------

#[test]
fn another_method_is_forwarded_whole_and_its_result_comes_back() {
    let (mut shell, link, _) = shell();
    let params = json!({ "board": "b", "ready": true });
    let result = shell.rpc("task.list", Some(params.clone()));
    assert_eq!(result, Ok(json!({ "from": "daemon" })));
    let calls = link.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].method, "task.list");
    assert_eq!(calls[0].params, params);
}

#[test]
fn a_failure_of_the_daemon_comes_back_as_its_error() {
    struct Refusing;
    impl DaemonLink for Refusing {
        fn call(&mut self, request: &Request) -> Response {
            Response::failure(
                Some(request.id.clone()),
                RpcError::new(code::NOT_FOUND, "no such run"),
            )
        }
    }
    let channel = FakeChannel::open();
    let mut shell = Shell::new(endpoint(&channel), Box::new(Refusing));
    let error = failure(shell.rpc("run.get", Some(json!({ "run": "r-1" }))));
    assert_eq!(
        (error.code, error.message.as_str()),
        (code::NOT_FOUND, "no such run")
    );
}

#[test]
fn each_forwarded_call_gets_its_own_request_id() {
    let (mut shell, link, _) = shell();
    shell.rpc("daemon.status", None).unwrap();
    shell.rpc("daemon.status", None).unwrap();
    let ids: Vec<RequestId> = link.calls().into_iter().map(|c| c.id).collect();
    assert_eq!(ids.len(), 2);
    assert_ne!(ids[0], ids[1]);
}

// ---- the handshake and the ask token are the shell's -----------------------------------------------------------

#[test]
fn the_window_cannot_send_initialize_and_it_is_not_forwarded() {
    let (mut shell, link, _) = shell();
    let error = failure(shell.rpc(
        "initialize",
        Some(json!({ "protocol": 1, "token": "guess" })),
    ));
    assert_eq!(error.code, code::METHOD_NOT_FOUND);
    assert!(!error.message.contains("guess"));
    assert!(link.calls().is_empty());
}

#[test]
fn an_ask_start_that_holds_a_token_is_refused_and_is_not_forwarded() {
    let (mut shell, link, _) = shell();
    let error = failure(shell.rpc(
        "ask.start",
        Some(json!({ "ask": "a1", "agent": "claude", "request": "show the queue", "token": "t" })),
    ));
    assert_eq!(error.code, code::INVALID_PARAMS);
    assert!(link.calls().is_empty());
}

#[test]
fn an_ask_cancel_is_forwarded_in_the_window_shape() {
    let (mut shell, link, _) = shell();
    shell
        .rpc("ask.cancel", Some(json!({ "ask": "a1" })))
        .unwrap();
    let calls = link.calls();
    assert_eq!(
        (calls[0].method.as_str(), &calls[0].params),
        ("ask.cancel", &json!({ "ask": "a1" }))
    );
}

#[test]
#[should_panic(expected = "shell: ask token")]
fn a_valid_ask_start_waits_for_the_ask_token_of_stage_6b() {
    let (mut shell, _, _) = shell();
    let _ = shell.rpc(
        "ask.start",
        Some(json!({ "ask": "a1", "agent": "claude", "request": "show the queue" })),
    );
}

// ---- the event channel -----------------------------------------------------------------------------------------

fn tools_call() -> HttpRequest {
    let body = json!({
        "jsonrpc": "2.0", "id": 1, "method": "tools/call",
        "params": { "name": "query", "arguments": { "surface": "field" } }
    });
    HttpRequest {
        method: "POST".into(),
        path: "/mcp".into(),
        headers: vec![
            ("Host".into(), format!("127.0.0.1:{PORT}")),
            ("Content-Type".into(), "application/json".into()),
            ("Authorization".into(), format!("Bearer {ENDPOINT_TOKEN}")),
        ],
        body: serde_json::to_vec(&body).unwrap(),
    }
}

#[test]
fn an_agent_tool_call_reaches_the_window_as_a_ui_tools_request_event() {
    let (mut shell, _, channel) = shell();
    let Handled::Pending { request_id } = shell.endpoint_mut().handle(&tools_call()) else {
        panic!("a tools/call with an open window should wait for the window");
    };
    let events = channel.events();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].method, "ui_tools.request");
    assert_eq!(events[0].params["requestId"], json!(request_id));
    assert_eq!(events[0].params["caller"], json!("external"));
}

#[test]
fn the_windows_reply_over_rpc_finishes_the_call_and_is_not_forwarded() {
    let (mut shell, link, _) = shell();
    let Handled::Pending { request_id } = shell.endpoint_mut().handle(&tools_call()) else {
        panic!("expected a pending call");
    };
    shell
        .rpc(
            "ui_tools.reply",
            Some(
                json!({ "requestId": request_id, "content": [{ "type": "text", "text": "done" }] }),
            ),
        )
        .unwrap();
    let completed = shell.take_completed();
    assert_eq!(completed.len(), 1);
    assert_eq!(completed[0].request_id, request_id);
    assert!(link.calls().is_empty());
}

#[test]
fn a_notification_of_the_daemon_is_emitted_as_the_same_method_and_params_with_no_envelope() {
    let channel = FakeChannel::open();
    let notification = collide_core::owner::Notification {
        jsonrpc: collide_core::owner::JsonRpc2,
        method: "queue.changed".into(),
        params: json!({ "count": 2 }),
    };
    channel.emit(&ChannelEvent::from(notification));
    let sent = serde_json::to_value(&channel.events()[0]).unwrap();
    assert_eq!(
        sent,
        json!({ "method": "queue.changed", "params": { "count": 2 } })
    );
}

#[test]
fn a_closed_channel_is_a_window_that_is_not_open() {
    use collide_host::ui_tools::Window;
    let channel = FakeChannel {
        open: false,
        events: Arc::default(),
    };
    assert!(!ChannelWindow::new(Arc::new(channel)).is_open());
}

// ---- the shell runs on its own thread --------------------------------------------------------------------------

#[test]
fn the_shell_handle_answers_calls_from_any_thread() {
    let link = FakeLink::answering(json!({ "from": "daemon" }));
    let seen = link.clone();
    let handle = Arc::new(ShellHandle::spawn(move || {
        let channel = FakeChannel::open();
        Shell::new(endpoint(&channel), Box::new(link))
    }));
    let remote = Arc::clone(&handle);
    let result = std::thread::spawn(move || remote.rpc("daemon.status".into(), None))
        .join()
        .unwrap();
    assert_eq!(result, Ok(json!({ "from": "daemon" })));
    assert_eq!(seen.calls().len(), 1);
}

#[test]
fn a_shell_thread_that_panicked_fails_the_call_instead_of_hanging() {
    struct Panicking;
    impl DaemonLink for Panicking {
        fn call(&mut self, _: &Request) -> Response {
            panic!("the link broke");
        }
    }
    let handle = ShellHandle::spawn(|| {
        let channel = FakeChannel::open();
        Shell::new(endpoint(&channel), Box::new(Panicking))
    });
    let error = failure(handle.rpc("daemon.status".into(), None));
    assert_eq!(error.code, code::INTERNAL_ERROR);
}
