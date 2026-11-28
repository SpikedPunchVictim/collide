//! The shell's `rpc` command and event channel (0.2, The UI speaks the owner's protocol; ts-0017-qb), over a fake
//! daemon link and a fake event channel: no Tauri runtime and no socket. The last tests put the real link, over a
//! daemon
//! in this process under a root of the test's own (`tests/common`), behind the shell, to see what the webview gets.

mod common;

use collide_app::shell::{
    AskConnector, AskRelay, ChannelEvent, ChannelWindow, DaemonLink, EventChannel, RecordingOpener,
    RecordingPicker, Shell, ShellHandle,
};
use collide_core::agent::{AgentEvent, UsageQuality};
use collide_core::ask::{AskCost, AskEnd, AskEventNotice, AskNotice, AskOutcome, AskOutcomeNotice};
use collide_core::non_empty::NonEmpty;
use collide_core::owner::{Notification, Request, RequestId, Response, RpcError, code};
use collide_core::ui_tools::ToolManifest;
use collide_host::clock::{Clock, Clocks, Reading};
use collide_host::ui_tools::{EndpointTokens, Handled, HttpRequest, TokenFailure, UiToolsEndpoint};
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
    modes: Arc<Mutex<Vec<bool>>>,
    result: Value,
}

impl FakeLink {
    fn answering(result: Value) -> Self {
        FakeLink {
            calls: Arc::default(),
            modes: Arc::default(),
            result,
        }
    }

    /// The modes the shell told the link, in order.
    fn modes(&self) -> Vec<bool> {
        self.modes.lock().unwrap().clone()
    }

    fn calls(&self) -> Vec<Request> {
        self.calls.lock().unwrap().clone()
    }
}

impl DaemonLink for FakeLink {
    fn set_defaults(&mut self, defaults: bool) {
        self.modes.lock().unwrap().push(defaults);
    }

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
    fn current(&self) -> Result<String, TokenFailure> {
        Ok(ENDPOINT_TOKEN.to_string())
    }

    fn rotate(&mut self) -> Result<String, TokenFailure> {
        Ok("rotated-token-0001".to_string())
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

/// A shell over a fake link (answering `{"from":"daemon"}`), a fake open channel and an opener that opens nothing.
fn shell() -> (Shell, FakeLink, FakeChannel) {
    shell_with_opener(RecordingOpener::default())
}

fn shell_with_opener(opener: RecordingOpener) -> (Shell, FakeLink, FakeChannel) {
    let link = FakeLink::answering(json!({ "from": "daemon" }));
    let channel = FakeChannel::open();
    let shell = Shell::new(endpoint(&channel), Box::new(link.clone()), Box::new(opener));
    (shell, link, channel)
}

/// A shell whose folder picker is `picker`, which opens no window.
fn shell_with_picker(picker: RecordingPicker) -> (Shell, FakeLink) {
    let (shell, link, _) = shell();
    (shell.with_picker(Box::new(picker)), link)
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

// ---- shell.* is the shell's own and never goes to the daemon ----------------------------------------------------

#[test]
fn shell_open_external_is_answered_locally_and_the_forward_path_is_not_called() {
    let opener = RecordingOpener::default();
    let (mut shell, link, _) = shell_with_opener(opener.clone());
    let result = shell.rpc(
        "shell.open_external",
        Some(json!({ "url": "https://example.com/docs" })),
    );
    assert_eq!(result, Ok(json!({})));
    assert!(link.calls().is_empty(), "{:?}", link.calls());
    assert_eq!(
        opener.opened(),
        vec!["https://example.com/docs".to_string()]
    );
}

#[test]
fn shell_open_external_refuses_a_scheme_it_does_not_open_and_does_not_forward_it() {
    let opener = RecordingOpener::default();
    let (mut shell, link, _) = shell_with_opener(opener.clone());
    for url in [
        "javascript:alert(1)",
        "file:///etc/passwd",
        "data:text/plain,x",
        "https://a.example/\nb",
    ] {
        let error = failure(shell.rpc("shell.open_external", Some(json!({ "url": url }))));
        assert_eq!(error.code, code::INVALID_PARAMS, "{url}");
    }
    assert!(link.calls().is_empty(), "{:?}", link.calls());
    assert!(opener.opened().is_empty(), "{:?}", opener.opened());
}

#[test]
fn an_unknown_shell_method_is_refused_here_and_is_not_forwarded() {
    let (mut shell, link, _) = shell();
    let error = failure(shell.rpc("shell.nothing", None));
    assert_eq!(error.code, code::METHOD_NOT_FOUND);
    assert!(link.calls().is_empty(), "{:?}", link.calls());
}

#[test]
fn shell_pick_folder_answers_the_folder_the_person_chose_and_does_not_forward_it() {
    let picker = RecordingPicker::choosing("/work/chosen");
    let (mut shell, link) = shell_with_picker(picker.clone());
    let result = shell.rpc("shell.pick_folder", None);
    assert_eq!(result, Ok(json!({ "path": "/work/chosen" })));
    assert_eq!(picker.asked(), 1);
    assert!(link.calls().is_empty(), "{:?}", link.calls());
}

#[test]
fn shell_pick_folder_answers_no_path_when_the_person_cancels() {
    let picker = RecordingPicker::cancelling();
    let (mut shell, link) = shell_with_picker(picker.clone());
    assert_eq!(
        shell.rpc("shell.pick_folder", Some(json!({}))),
        Ok(json!({ "path": null }))
    );
    assert_eq!(picker.asked(), 1);
    assert!(link.calls().is_empty(), "{:?}", link.calls());
}

#[test]
fn shell_pick_folder_refuses_params_it_does_not_have_and_does_not_ask_the_person() {
    let picker = RecordingPicker::choosing("/work/chosen");
    let (mut shell, link) = shell_with_picker(picker.clone());
    let error = failure(shell.rpc("shell.pick_folder", Some(json!({ "start": "/etc" }))));
    assert_eq!(error.code, code::INVALID_PARAMS);
    assert_eq!(picker.asked(), 0);
    assert!(link.calls().is_empty(), "{:?}", link.calls());
}

#[test]
fn shell_pick_folder_says_why_when_the_picker_fails() {
    let (mut shell, link) = shell_with_picker(RecordingPicker::failing("no window to ask in"));
    let error = failure(shell.rpc("shell.pick_folder", None));
    assert_eq!(error.code, code::INTERNAL_ERROR);
    assert!(
        error.message.contains("no window to ask in"),
        "{}",
        error.message
    );
    assert!(link.calls().is_empty(), "{:?}", link.calls());
}

#[test]
fn a_shell_with_no_picker_says_so_and_does_not_forward_the_call() {
    let (mut shell, link, _) = shell();
    let error = failure(shell.rpc("shell.pick_folder", None));
    assert_eq!(error.code, code::INTERNAL_ERROR);
    assert!(
        error.message.contains("no folder picker"),
        "{}",
        error.message
    );
    assert!(link.calls().is_empty(), "{:?}", link.calls());
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
    let mut shell = Shell::new(
        endpoint(&channel),
        Box::new(Refusing),
        Box::new(RecordingOpener::default()),
    );
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

// ---- the ask's token and its notices ----------------------------------------------------------------------------

/// A connector that keeps what it was asked and the relays it was given, so a test plays the daemon's notices.
#[derive(Clone, Default)]
struct FakeConnector {
    started: Arc<Mutex<Vec<Request>>>,
    relays: Arc<Mutex<Vec<AskRelay>>>,
    refusal: Arc<Mutex<Option<RpcError>>>,
}

impl FakeConnector {
    fn started(&self) -> Vec<Request> {
        self.started.lock().unwrap().clone()
    }

    fn relay(&self, index: usize) -> AskRelay {
        self.relays.lock().unwrap()[index].clone()
    }

    fn refusing(&self, error: RpcError) {
        *self.refusal.lock().unwrap() = Some(error);
    }
}

impl AskConnector for FakeConnector {
    fn start(&mut self, request: &Request, relay: AskRelay) -> Result<(), RpcError> {
        if let Some(error) = self.refusal.lock().unwrap().clone() {
            return Err(error);
        }
        self.started.lock().unwrap().push(request.clone());
        self.relays.lock().unwrap().push(relay);
        Ok(())
    }
}

/// A shell whose asks go through a fake connector, with the fake link for everything else.
fn shell_with_asks() -> (Shell, FakeLink, FakeChannel, FakeConnector) {
    let (shell, link, channel) = shell();
    let connector = FakeConnector::default();
    let shell = shell.with_asks(Box::new(connector.clone()), Arc::new(channel.clone()));
    (shell, link, channel, connector)
}

fn ask_start(shell: &mut Shell, ask: &str) -> Result<Value, RpcError> {
    shell.rpc(
        "ask.start",
        Some(json!({ "ask": ask, "agent": "claude", "request": "show the queue" })),
    )
}

/// The token the shell put in the `ask.start` the connector got.
fn token_of(request: &Request) -> String {
    request.params["token"]
        .as_str()
        .expect("a token")
        .to_string()
}

fn ask_name(ask: &str) -> NonEmpty {
    NonEmpty::new(ask).unwrap()
}

fn event_notice(ask: &str, text: &str) -> Notification {
    AskNotice::Event(Box::new(AskEventNotice {
        ask: ask_name(ask),
        event: AgentEvent::Message { text: text.into() },
    }))
    .to_notification()
}

fn outcome_notice(ask: &str, end: AskEnd) -> Notification {
    AskNotice::Outcome(Box::new(AskOutcomeNotice {
        ask: ask_name(ask),
        outcome: AskOutcome {
            end,
            cost: AskCost::NotReported {},
            quality: UsageQuality::Unreported,
        },
    }))
    .to_notification()
}

fn tools_call_with(token: &str) -> HttpRequest {
    let mut request = tools_call();
    for (name, value) in &mut request.headers {
        if name == "Authorization" {
            *value = format!("Bearer {token}");
        }
    }
    request
}

#[test]
fn a_valid_ask_start_goes_to_the_daemon_with_the_endpoint_and_a_token_of_its_own() {
    let (mut shell, link, _, connector) = shell_with_asks();
    assert_eq!(ask_start(&mut shell, "a1"), Ok(json!({})));
    let started = connector.started();
    assert_eq!(started.len(), 1);
    assert_eq!(started[0].method, "ask.start");
    let params = &started[0].params;
    assert_eq!(
        params["endpoint"],
        json!(format!("http://127.0.0.1:{PORT}/mcp"))
    );
    assert_eq!(
        (&params["ask"], &params["agent"], &params["request"]),
        (&json!("a1"), &json!("claude"), &json!("show the queue"))
    );
    let token = token_of(&started[0]);
    assert_eq!(token.len(), 64);
    assert!(token.bytes().all(|b| b.is_ascii_hexdigit()), "{token}");
    assert_ne!(token, ENDPOINT_TOKEN);
    assert!(
        link.calls().is_empty(),
        "the ask has a connection of its own: {:?}",
        link.calls()
    );
}

#[test]
fn an_ask_start_with_an_empty_request_is_refused_and_registers_nothing() {
    let (mut shell, link, _, connector) = shell_with_asks();
    let error = failure(shell.rpc(
        "ask.start",
        Some(json!({ "ask": "a1", "agent": "claude", "request": "" })),
    ));
    assert_eq!(error.code, code::INVALID_PARAMS);
    assert!(connector.started().is_empty());
    assert!(link.calls().is_empty());
    assert!(shell.endpoint_mut().live_asks().is_empty());
}

#[test]
fn each_ask_gets_a_token_of_its_own() {
    let (mut shell, _, _, connector) = shell_with_asks();
    ask_start(&mut shell, "a1").unwrap();
    ask_start(&mut shell, "a2").unwrap();
    let started = connector.started();
    assert_ne!(token_of(&started[0]), token_of(&started[1]));
}

#[test]
fn the_ask_token_is_registered_with_the_endpoint() {
    let (mut shell, _, channel, connector) = shell_with_asks();
    ask_start(&mut shell, "a1").unwrap();
    assert_eq!(shell.endpoint_mut().live_asks(), vec!["a1".to_string()]);
    let token = token_of(&connector.started()[0]);
    let Handled::Pending { .. } = shell.endpoint_mut().handle(&tools_call_with(&token)) else {
        panic!("a call with the ask's token should reach the window");
    };
    assert_eq!(channel.events()[0].params["caller"], json!("a1"));
}

#[test]
fn an_ask_start_with_an_id_in_use_is_refused_and_does_not_reach_the_daemon() {
    let (mut shell, _, _, connector) = shell_with_asks();
    ask_start(&mut shell, "a1").unwrap();
    let error = failure(ask_start(&mut shell, "a1"));
    assert_eq!(error.code, code::CONFLICT);
    assert_eq!(connector.started().len(), 1);
}

#[test]
fn an_ask_start_the_daemon_refuses_comes_back_as_its_error_and_ends_the_token() {
    let (mut shell, _, _, connector) = shell_with_asks();
    connector.refusing(RpcError::new(
        code::NOT_FOUND,
        "no agent entry has the id `claude`",
    ));
    let error = failure(ask_start(&mut shell, "a1"));
    assert_eq!(error.code, code::NOT_FOUND);
    assert!(
        error.message.contains("no agent entry"),
        "{}",
        error.message
    );
    assert!(shell.endpoint_mut().live_asks().is_empty());
}

#[test]
fn an_ask_start_in_a_shell_with_no_connector_is_refused_and_ends_the_token() {
    let (mut shell, _, _) = shell();
    let error = failure(ask_start(&mut shell, "a1"));
    assert_eq!(error.code, code::UNAVAILABLE);
    assert!(shell.endpoint_mut().live_asks().is_empty());
}

#[test]
fn the_asks_notices_are_emitted_to_the_window_as_the_daemon_sent_them() {
    let (mut shell, _, channel, connector) = shell_with_asks();
    ask_start(&mut shell, "a1").unwrap();
    let relay = connector.relay(0);
    relay.notice(event_notice("a1", "looking"));
    relay.notice(outcome_notice("a1", AskEnd::Finished {}));
    let events = channel.events();
    let methods: Vec<&str> = events.iter().map(|e| e.method.as_str()).collect();
    assert_eq!(methods, ["ask.event", "ask.outcome"]);
    assert_eq!(events[0].params, event_notice("a1", "looking").params);
    assert_eq!(
        events[1].params["outcome"]["end"],
        json!({ "type": "finished" })
    );
}

#[test]
fn a_notice_that_is_not_an_asks_is_not_emitted() {
    let (mut shell, _, channel, connector) = shell_with_asks();
    ask_start(&mut shell, "a1").unwrap();
    connector.relay(0).notice(Notification {
        jsonrpc: Default::default(),
        method: "daemon.event".into(),
        params: json!({}),
    });
    assert!(channel.events().is_empty(), "{:?}", channel.events());
}

#[test]
fn the_ask_token_is_never_sent_to_the_window() {
    let (mut shell, _, channel, connector) = shell_with_asks();
    ask_start(&mut shell, "a1").unwrap();
    let token = token_of(&connector.started()[0]);
    let relay = connector.relay(0);
    relay.notice(event_notice("a1", "looking"));
    relay.notice(outcome_notice("a1", AskEnd::Finished {}));
    shell.tick();
    shell.endpoint_mut().handle(&tools_call_with(&token));
    let seen = serde_json::to_string(&channel.events()).unwrap();
    assert!(
        !seen.contains(&token),
        "the token reached the window: {seen}"
    );
    assert!(
        !seen.contains(ENDPOINT_TOKEN),
        "the endpoint token reached the window: {seen}"
    );
}

#[test]
fn the_outcome_ends_the_ask_and_its_token_stops_working() {
    let (mut shell, _, _, connector) = shell_with_asks();
    ask_start(&mut shell, "a1").unwrap();
    let token = token_of(&connector.started()[0]);
    connector
        .relay(0)
        .notice(outcome_notice("a1", AskEnd::Finished {}));
    shell.tick();
    assert!(shell.endpoint_mut().live_asks().is_empty());
    match shell.endpoint_mut().handle(&tools_call_with(&token)) {
        Handled::Response(response) => assert_eq!(response.status, 401),
        other => panic!("a dead token should be refused, got {other:?}"),
    }
}

#[test]
fn an_ask_event_alone_does_not_end_the_ask() {
    let (mut shell, _, _, connector) = shell_with_asks();
    ask_start(&mut shell, "a1").unwrap();
    connector.relay(0).notice(event_notice("a1", "looking"));
    shell.tick();
    assert_eq!(shell.endpoint_mut().live_asks(), vec!["a1".to_string()]);
}

#[test]
fn an_id_can_be_used_again_once_its_ask_has_ended() {
    let (mut shell, _, _, connector) = shell_with_asks();
    ask_start(&mut shell, "a1").unwrap();
    connector
        .relay(0)
        .notice(outcome_notice("a1", AskEnd::Finished {}));
    assert_eq!(ask_start(&mut shell, "a1"), Ok(json!({})));
}

#[test]
fn a_connection_that_ends_with_no_outcome_tells_the_window_the_ask_was_lost() {
    let (mut shell, _, channel, connector) = shell_with_asks();
    ask_start(&mut shell, "a1").unwrap();
    connector.relay(0).ended();
    shell.tick();
    let events = channel.events();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].method, "ask.outcome");
    assert_eq!(
        events[0].params["outcome"]["end"],
        json!({ "type": "lost" })
    );
    assert!(shell.endpoint_mut().live_asks().is_empty());
}

#[test]
fn a_connection_that_ends_after_the_outcome_says_nothing_more() {
    let (mut shell, _, channel, connector) = shell_with_asks();
    ask_start(&mut shell, "a1").unwrap();
    let relay = connector.relay(0);
    relay.notice(outcome_notice("a1", AskEnd::Finished {}));
    relay.ended();
    assert_eq!(channel.events().len(), 1);
}

#[test]
fn a_window_close_sends_ask_cancel_for_each_live_ask() {
    let (mut shell, link, _, _) = shell_with_asks();
    ask_start(&mut shell, "a1").unwrap();
    ask_start(&mut shell, "a2").unwrap();
    shell.window_gone();
    let calls = link.calls();
    let sent: Vec<(&str, &Value)> = calls
        .iter()
        .map(|c| (c.method.as_str(), &c.params))
        .collect();
    assert_eq!(
        sent,
        [
            ("ask.cancel", &json!({ "ask": "a1" })),
            ("ask.cancel", &json!({ "ask": "a2" }))
        ]
    );
}

#[test]
fn a_window_close_does_not_cancel_an_ask_that_has_ended() {
    let (mut shell, link, _, connector) = shell_with_asks();
    ask_start(&mut shell, "a1").unwrap();
    connector
        .relay(0)
        .notice(outcome_notice("a1", AskEnd::Finished {}));
    shell.window_gone();
    assert!(link.calls().is_empty(), "{:?}", link.calls());
}

#[test]
fn an_ask_test_goes_to_the_daemon_with_a_token_and_the_token_ends_with_the_answer() {
    let (mut shell, link, _, _) = shell_with_asks();
    let result = shell.rpc("ask.test", Some(json!({ "ask": "t1", "agent": "claude" })));
    assert_eq!(result, Ok(json!({ "from": "daemon" })));
    let calls = link.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].method, "ask.test");
    assert_eq!(
        calls[0].params["endpoint"],
        json!(format!("http://127.0.0.1:{PORT}/mcp"))
    );
    assert_eq!(calls[0].params["token"].as_str().map(str::len), Some(64));
    assert!(shell.endpoint_mut().live_asks().is_empty());
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
        Shell::new(
            endpoint(&channel),
            Box::new(link),
            Box::new(RecordingOpener::default()),
        )
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
        Shell::new(
            endpoint(&channel),
            Box::new(Panicking),
            Box::new(RecordingOpener::default()),
        )
    });
    let error = failure(handle.rpc("daemon.status".into(), None));
    assert_eq!(error.code, code::INTERNAL_ERROR);
}

// ---- the token never reaches the webview ------------------------------------------------------------------------

/// Everything the webview would receive from `calls`: each result and each error, as the command rejects or resolves
/// it.
fn what_the_webview_gets(shell: &mut Shell, calls: &[(&str, Option<Value>)]) -> String {
    calls
        .iter()
        .map(|(method, params)| match shell.rpc(method, params.clone()) {
            Ok(value) => serde_json::to_string(&value).unwrap(),
            Err(error) => serde_json::to_string(&error).unwrap(),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn real_shell(root: &common::TempRoot, channel: &FakeChannel) -> Shell {
    let access = common::access(root, common::AlreadyThere, &common::TestClocks::default());
    Shell::new(
        endpoint(channel),
        Box::new(collide_app::shell::SocketLink::new(access)),
        Box::new(RecordingOpener::default()),
    )
}

#[test]
fn nothing_the_webview_receives_holds_the_token() {
    let root = common::TempRoot::new();
    let daemon = common::start_daemon(&root, Box::new(std::io::sink()), None);
    let token = std::fs::read_to_string(&root.paths().token_file).unwrap();
    let token = token.trim();
    assert!(
        token.len() >= 32,
        "a real token, or the check means nothing"
    );

    // The pump's channel: the webview's event channel, with the daemon's notices on it.
    let events = common::Channel::default();
    let pump = common::PumpRun::start(
        common::access(&root, common::AlreadyThere, &common::TestClocks::default()),
        events.clone(),
        std::time::Duration::from_millis(50),
    );
    common::wait_until(
        "the pump is subscribed",
        std::time::Duration::from_secs(10),
        || {
            common::append(&daemon, 1);
            !events.seqs().is_empty()
        },
    );
    common::wait_until(
        "the Queue and the log are read",
        std::time::Duration::from_secs(10),
        || {
            events.seqs().len() >= 2 || {
                common::append(&daemon, 1);
                false
            }
        },
    );
    pump.stop();

    let channel = FakeChannel::open();
    let mut shell = real_shell(&root, &channel);
    let seen = what_the_webview_gets(
        &mut shell,
        &[
            ("daemon.status", None),
            ("daemon.status", Some(json!({ "extra": token }))),
            ("nothing.here", None),
            ("run.get", Some(json!({ "run": "none" }))),
            (
                "initialize",
                Some(json!({ "protocol": 1, "token": "guess" })),
            ),
        ],
    );
    let notices = serde_json::to_string(&events.events()).unwrap();
    assert!(
        !seen.contains(token),
        "a result or an error holds the token: {seen}"
    );
    assert!(!notices.contains(token), "an event holds the token");
    assert!(seen.contains("pid"), "the calls were answered: {seen}");
}

#[test]
fn a_token_the_daemon_refuses_is_not_echoed_to_the_webview() {
    let root = common::TempRoot::new();
    let _daemon = common::start_daemon(&root, Box::new(std::io::sink()), None);
    let paths = root.paths();
    // The file now holds another token, as a daemon that was replaced would leave it.
    let wrong = "a-token-the-daemon-does-not-know-0123456789";
    std::fs::remove_file(&paths.token_file).unwrap();
    collide_host::auth::create_token_file(&paths.token_file, wrong).unwrap();
    let channel = FakeChannel::open();
    let mut shell = real_shell(&root, &channel);
    let seen = what_the_webview_gets(&mut shell, &[("daemon.status", None)]);
    assert!(
        seen.contains("-32004") || seen.contains("token"),
        "refused: {seen}"
    );
    assert!(!seen.contains(wrong), "the token is in the error: {seen}");
}

// ---- shell.set_defaults (ts-0215-gt) -------------------------------------------------------------------------

#[test]
fn shell_set_defaults_tells_the_link_the_mode_and_does_not_forward_it() {
    let (mut shell, link, _) = shell();
    assert_eq!(
        shell.rpc("shell.set_defaults", Some(json!({ "defaults": true }))),
        Ok(json!({}))
    );
    assert_eq!(
        shell.rpc("shell.set_defaults", Some(json!({ "defaults": false }))),
        Ok(json!({}))
    );
    assert_eq!(link.modes(), [true, false]);
    assert!(link.calls().is_empty(), "{:?}", link.calls());
}

#[test]
fn shell_set_defaults_refuses_params_it_does_not_have_and_changes_nothing() {
    let (mut shell, link, _) = shell();
    for params in [
        None,
        Some(json!({})),
        Some(json!({ "defaults": "yes" })),
        Some(json!({ "defaults": true, "token": "x" })),
    ] {
        let error = failure(shell.rpc("shell.set_defaults", params.clone()));
        assert_eq!(error.code, code::INVALID_PARAMS, "{params:?}");
    }
    assert!(link.modes().is_empty(), "{:?}", link.modes());
    assert!(link.calls().is_empty(), "{:?}", link.calls());
}
