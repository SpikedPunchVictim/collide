//! The `ui_tools` request handler (0.2, The UI-tools endpoint; ts-0035-re), over a fake clock and a fake window: no
//! listener and no real window. A request is a value and so is an answer, so each success criterion of the item is a
//! test below.

use collide_core::owner::{Request, RequestId, Response, code};
use collide_core::ui_tools::{
    MAX_SCREEN_CHANGING_CALLS, NO_WINDOW_TEXT, TIMEOUT_TEXT, ToolManifest, UiToolsRequest,
};
use collide_host::clock::{Clock, Clocks, Reading};
use collide_host::ui_tools::{
    EndpointTokens, Handled, HttpRequest, HttpResponse, KeychainTokens, MAX_BODY_BYTES, Rpc,
    UiToolsEndpoint, Window,
};
use serde_json::{Value, json};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

mod common;

const PORT: u16 = 47_821;
const TOKEN: &str = "endpoint-token-0001";
const WALL_BASE_MS: u64 = 1_700_000_000_000;

// ---- fakes ----------------------------------------------------------------------------------------------------

/// Uptime nanoseconds, moved by a test. The wall clock is the same count from a fixed base.
#[derive(Clone, Default)]
struct FakeClocks(Rc<Cell<u64>>);

impl FakeClocks {
    fn advance_ms(&self, ms: u64) {
        self.0.set(self.0.get() + ms * 1_000_000);
    }
}

impl Clocks for FakeClocks {
    fn read(&self, clock: Clock) -> Reading {
        let up = self.0.get();
        let nanos = match clock {
            Clock::Wall => WALL_BASE_MS * 1_000_000 + up,
            _ => up,
        };
        Reading { clock, nanos }
    }
}

#[derive(Default)]
struct WindowState {
    open: bool,
    sent: Vec<UiToolsRequest>,
}

#[derive(Clone, Default)]
struct FakeWindow(Rc<RefCell<WindowState>>);

impl FakeWindow {
    fn open() -> Self {
        let window = FakeWindow::default();
        window.0.borrow_mut().open = true;
        window
    }
    fn set_open(&self, open: bool) {
        self.0.borrow_mut().open = open;
    }
    fn sent(&self) -> Vec<UiToolsRequest> {
        self.0.borrow().sent.clone()
    }
}

impl Window for FakeWindow {
    fn is_open(&self) -> bool {
        self.0.borrow().open
    }
    fn send(&mut self, event: UiToolsRequest) {
        self.0.borrow_mut().sent.push(event);
    }
}

#[derive(Clone)]
struct FakeTokens(Rc<RefCell<(String, u32)>>);

impl FakeTokens {
    fn new() -> Self {
        FakeTokens(Rc::new(RefCell::new((TOKEN.to_string(), 0))))
    }
}

impl EndpointTokens for FakeTokens {
    fn current(&self) -> String {
        self.0.borrow().0.clone()
    }
    fn rotate(&mut self) -> String {
        let mut state = self.0.borrow_mut();
        state.1 += 1;
        state.0 = format!("rotated-token-{:04}", state.1);
        state.0.clone()
    }
}

struct Rig {
    endpoint: UiToolsEndpoint,
    clocks: FakeClocks,
    window: FakeWindow,
}

fn manifest() -> ToolManifest {
    let text = std::fs::read_to_string(common::repo_root().join("fixtures/ui-tools/manifest.json"))
        .expect("the manifest fixture");
    ToolManifest::from_json(&text).expect("the fixture is a manifest")
}

fn rig_with(window: FakeWindow) -> Rig {
    let clocks = FakeClocks::default();
    let endpoint = UiToolsEndpoint::new(
        PORT,
        manifest(),
        Box::new(FakeTokens::new()),
        Box::new(window.clone()),
        Box::new(clocks.clone()),
    )
    .expect("the fixture manifest is valid");
    Rig {
        endpoint,
        clocks,
        window,
    }
}

fn rig() -> Rig {
    rig_with(FakeWindow::open())
}

// ---- requests ---------------------------------------------------------------------------------------------------

fn good_headers() -> Vec<(String, String)> {
    vec![
        ("Host".into(), format!("127.0.0.1:{PORT}")),
        ("Content-Type".into(), "application/json".into()),
        ("Authorization".into(), format!("Bearer {TOKEN}")),
    ]
}

fn post(body: &Value) -> HttpRequest {
    HttpRequest {
        method: "POST".into(),
        path: "/mcp".into(),
        headers: good_headers(),
        body: serde_json::to_vec(body).unwrap(),
    }
}

fn with_header(mut request: HttpRequest, name: &str, value: &str) -> HttpRequest {
    request
        .headers
        .retain(|(n, _)| !n.eq_ignore_ascii_case(name));
    request.headers.push((name.into(), value.into()));
    request
}

fn without_header(mut request: HttpRequest, name: &str) -> HttpRequest {
    request
        .headers
        .retain(|(n, _)| !n.eq_ignore_ascii_case(name));
    request
}

fn rpc(method: &str, id: i64, params: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params })
}

fn call(tool: &str, id: i64) -> Value {
    rpc(
        "tools/call",
        id,
        json!({ "name": tool, "arguments": { "surface": "field" } }),
    )
}

fn response(handled: Handled) -> HttpResponse {
    match handled {
        Handled::Response(response) => response,
        Handled::Pending { request_id } => panic!("expected an answer, got a pending {request_id}"),
    }
}

fn pending(handled: Handled) -> String {
    match handled {
        Handled::Pending { request_id } => request_id,
        Handled::Response(r) => panic!("expected a pending call, got {} {:?}", r.status, text(&r)),
    }
}

fn text(response: &HttpResponse) -> String {
    String::from_utf8_lossy(&response.body).into_owned()
}

fn body(response: &HttpResponse) -> Value {
    serde_json::from_slice(&response.body)
        .unwrap_or_else(|e| panic!("not JSON ({e}): {}", text(response)))
}

fn header<'a>(response: &'a HttpResponse, name: &str) -> Option<&'a str> {
    response
        .headers
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.as_str())
}

/// The text of a tool result that failed: JSON-RPC success whose result has `isError`.
fn tool_failure_text(response: &HttpResponse) -> String {
    assert_eq!(response.status, 200, "{}", text(response));
    let doc = body(response);
    assert_eq!(doc["result"]["isError"], json!(true), "{doc}");
    doc["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_else(|| panic!("no text in {doc}"))
        .to_string()
}

fn owner_request(method: &str, params: Value) -> Request {
    Request {
        jsonrpc: Default::default(),
        id: RequestId::Number(9),
        method: method.into(),
        params,
    }
}

fn reply(rig: &mut Rig, request_id: &str, text: &str) -> Vec<collide_host::ui_tools::Completed> {
    let handled = match rig.endpoint.rpc(&owner_request(
        "ui_tools.reply",
        json!({ "requestId": request_id, "content": [{ "type": "text", "text": text }] }),
    )) {
        Rpc::Handled(handled) => handled,
        Rpc::Forward => panic!("ui_tools.reply was forwarded"),
    };
    assert!(
        matches!(handled.response, Response::Success { .. }),
        "{:?}",
        handled.response
    );
    handled.completed
}

fn start_ask(rig: &mut Rig, ask: &str, token: &str) {
    rig.endpoint.register_ask(ask, token).expect("registered");
}

fn ask_call(token: &str, tool: &str, id: i64) -> HttpRequest {
    with_header(
        post(&call(tool, id)),
        "Authorization",
        &format!("Bearer {token}"),
    )
}

// ---- answered by the shell alone ------------------------------------------------------------------------------

#[test]
fn initialize_ping_and_tools_list_answer_with_no_window_connected() {
    let mut rig = rig_with(FakeWindow::default());
    let init = response(rig.endpoint.handle(&post(&rpc(
        "initialize",
        1,
        json!({ "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "t", "version": "1" } }),
    ))));
    assert_eq!(init.status, 200);
    assert_eq!(header(&init, "content-type"), Some("application/json"));
    let doc = body(&init);
    assert_eq!(doc["id"], json!(1));
    assert_eq!(doc["result"]["protocolVersion"], json!("2025-06-18"));
    assert_eq!(doc["result"]["serverInfo"]["name"], json!("collide"));
    assert_eq!(doc["result"]["capabilities"], json!({ "tools": {} }));

    let ping = response(rig.endpoint.handle(&post(&rpc("ping", 2, json!({})))));
    assert_eq!(
        (ping.status, body(&ping)["result"].clone()),
        (200, json!({}))
    );

    let list = response(rig.endpoint.handle(&post(&rpc("tools/list", 3, json!({})))));
    assert_eq!(list.status, 200);
    let tools = body(&list)["result"]["tools"].clone();
    let expected: Value = serde_json::from_str(
        &std::fs::read_to_string(common::repo_root().join("fixtures/ui-tools/manifest.json"))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(tools, expected, "tools/list is the manifest");
    assert!(rig.window.sent().is_empty());
}

#[test]
fn initialize_answers_the_newest_version_it_speaks_to_a_client_that_asks_for_another() {
    let mut rig = rig();
    let init = response(rig.endpoint.handle(&post(&rpc(
        "initialize",
        1,
        json!({ "protocolVersion": "1999-01-01" }),
    ))));
    assert_eq!(
        body(&init)["result"]["protocolVersion"],
        json!("2025-06-18")
    );
}

#[test]
fn ids_are_answered_as_sent_text_or_number() {
    let mut rig = rig();
    let text_id = json!({ "jsonrpc": "2.0", "id": "abc", "method": "ping" });
    let answer = response(rig.endpoint.handle(&post(&text_id)));
    assert_eq!(body(&answer)["id"], json!("abc"));
}

#[test]
fn an_unknown_method_is_a_json_rpc_method_not_found() {
    let mut rig = rig();
    let answer = response(
        rig.endpoint
            .handle(&post(&rpc("resources/list", 4, json!({})))),
    );
    assert_eq!(answer.status, 200);
    assert_eq!(
        body(&answer)["error"]["code"],
        json!(code::METHOD_NOT_FOUND)
    );
}

#[test]
fn a_notification_gets_202_and_no_body() {
    let mut rig = rig();
    let note = json!({ "jsonrpc": "2.0", "method": "notifications/initialized" });
    let answer = response(rig.endpoint.handle(&post(&note)));
    assert_eq!(answer.status, 202);
    assert!(answer.body.is_empty());
    assert!(rig.window.sent().is_empty());
}

#[test]
fn a_notification_that_looks_like_a_tool_call_is_not_forwarded() {
    let mut rig = rig();
    let note = json!({ "jsonrpc": "2.0", "method": "tools/call", "params": { "name": "open" } });
    let answer = response(rig.endpoint.handle(&post(&note)));
    assert_eq!(answer.status, 202);
    assert!(rig.window.sent().is_empty());
}

#[test]
fn get_and_delete_get_405() {
    let mut rig = rig();
    for method in ["GET", "DELETE", "PUT", "OPTIONS"] {
        let mut request = post(&json!({}));
        request.method = method.into();
        request.body.clear();
        let answer = response(rig.endpoint.handle(&request));
        assert_eq!(answer.status, 405, "{method}");
        assert_eq!(header(&answer, "allow"), Some("POST"), "{method}");
    }
}

#[test]
fn a_path_that_is_not_the_endpoint_is_404() {
    let mut rig = rig();
    let mut request = post(&rpc("ping", 1, json!({})));
    request.path = "/other".into();
    assert_eq!(response(rig.endpoint.handle(&request)).status, 404);
}

#[test]
fn text_that_is_not_json_and_a_batch_are_refused_with_400() {
    let mut rig = rig();
    let mut request = post(&json!({}));
    request.body = b"{not json".to_vec();
    let answer = response(rig.endpoint.handle(&request));
    assert_eq!(answer.status, 400);
    assert_eq!(body(&answer)["error"]["code"], json!(code::PARSE_ERROR));

    let batch = response(
        rig.endpoint
            .handle(&post(&json!([rpc("ping", 1, json!({}))]))),
    );
    assert_eq!(batch.status, 400);
    assert_eq!(body(&batch)["error"]["code"], json!(code::INVALID_REQUEST));

    let no_version = response(
        rig.endpoint
            .handle(&post(&json!({ "id": 1, "method": "ping" }))),
    );
    assert_eq!(no_version.status, 400);
}

// ---- tools/call and its answer --------------------------------------------------------------------------------

#[test]
fn a_tools_call_becomes_one_request_event_and_a_reply_completes_it() {
    let mut rig = rig();
    let message = call("set_view", 11);
    let id = pending(rig.endpoint.handle(&post(&message)));
    let sent = rig.window.sent();
    assert_eq!(sent.len(), 1);
    let event = &sent[0];
    assert_eq!(event.request_id, id);
    assert_eq!(event.caller, "external");
    assert_eq!(event.deadline, WALL_BASE_MS + 10_000);
    assert_eq!(event.message, message);

    let completed = reply(&mut rig, &id, "set the field view");
    assert_eq!(completed.len(), 1);
    assert_eq!(completed[0].request_id, id);
    let answer = &completed[0].response;
    assert_eq!(answer.status, 200);
    let doc = body(answer);
    assert_eq!(doc["id"], json!(11));
    assert_eq!(
        doc["result"]["content"],
        json!([{ "type": "text", "text": "set the field view" }])
    );
    assert_eq!(doc["result"]["isError"], json!(false));
}

#[test]
fn a_reply_marked_as_an_error_is_a_tool_error_result() {
    let mut rig = rig();
    let id = pending(rig.endpoint.handle(&post(&call("open", 1))));
    let handled = rig.endpoint.rpc(&owner_request(
        "ui_tools.reply",
        json!({ "requestId": id, "content": [{ "type": "text", "text": "unknown surface" }], "isError": true }),
    ));
    let Rpc::Handled(handled) = handled else {
        panic!("forwarded")
    };
    assert_eq!(
        tool_failure_text(&handled.completed[0].response),
        "unknown surface"
    );
}

#[test]
fn a_reply_completes_only_the_call_it_names() {
    let mut rig = rig();
    let first = pending(rig.endpoint.handle(&post(&call("open", 1))));
    let second = pending(rig.endpoint.handle(&post(&call("open", 2))));
    assert_ne!(first, second);
    let completed = reply(&mut rig, &second, "two");
    assert_eq!(completed.len(), 1);
    assert_eq!(body(&completed[0].response)["id"], json!(2));
    assert_eq!(reply(&mut rig, &first, "one").len(), 1);
}

#[test]
fn a_late_reply_is_ignored() {
    let mut rig = rig();
    let id = pending(rig.endpoint.handle(&post(&call("open", 1))));
    rig.clocks.advance_ms(10_000);
    let expired = rig.endpoint.expire();
    assert_eq!(expired.len(), 1);
    assert_eq!(tool_failure_text(&expired[0].response), TIMEOUT_TEXT);
    assert!(reply(&mut rig, &id, "too late").is_empty());
    assert!(rig.endpoint.expire().is_empty(), "nothing left to expire");
}

#[test]
fn a_reply_after_the_deadline_is_ignored_even_before_expire_runs() {
    let mut rig = rig();
    let id = pending(rig.endpoint.handle(&post(&call("open", 1))));
    rig.clocks.advance_ms(10_001);
    assert!(reply(&mut rig, &id, "late").is_empty());
    let expired = rig.endpoint.expire();
    assert_eq!(tool_failure_text(&expired[0].response), TIMEOUT_TEXT);
}

#[test]
fn a_reply_to_a_request_that_never_existed_is_ignored() {
    let mut rig = rig();
    assert!(reply(&mut rig, "ui-999", "nobody asked").is_empty());
}

#[test]
fn a_reply_is_answered_once() {
    let mut rig = rig();
    let id = pending(rig.endpoint.handle(&post(&call("open", 1))));
    assert_eq!(reply(&mut rig, &id, "a").len(), 1);
    assert!(reply(&mut rig, &id, "b").is_empty());
}

#[test]
fn with_no_window_a_tools_call_fails_with_the_open_collide_text() {
    let mut rig = rig_with(FakeWindow::default());
    let answer = response(rig.endpoint.handle(&post(&call("set_view", 5))));
    assert_eq!(
        tool_failure_text(&answer),
        "Collide has no window open; open Collide and ask again"
    );
    assert_eq!(tool_failure_text(&answer), NO_WINDOW_TEXT);
    assert_eq!(body(&answer)["id"], json!(5));
    assert!(rig.window.sent().is_empty());
}

#[test]
fn a_silent_window_gives_the_10_s_text_and_not_before() {
    let mut rig = rig();
    pending(rig.endpoint.handle(&post(&call("open", 1))));
    rig.clocks.advance_ms(9_999);
    assert!(rig.endpoint.expire().is_empty(), "9.999 s is not yet 10 s");
    rig.clocks.advance_ms(1);
    let expired = rig.endpoint.expire();
    assert_eq!(expired.len(), 1);
    assert_eq!(
        tool_failure_text(&expired[0].response),
        "the Collide window did not answer within 10 s"
    );
    assert_eq!(body(&expired[0].response)["id"], json!(1));
}

#[test]
fn a_tools_call_with_no_tool_name_is_invalid_params_and_not_forwarded() {
    let mut rig = rig();
    for params in [json!({}), json!({ "name": 3 }), json!("open")] {
        let answer = response(rig.endpoint.handle(&post(&rpc("tools/call", 1, params))));
        assert_eq!(body(&answer)["error"]["code"], json!(code::INVALID_PARAMS));
    }
    assert!(rig.window.sent().is_empty());
}

// ---- the cap --------------------------------------------------------------------------------------------------

#[test]
fn the_21st_screen_changing_call_of_an_ask_is_refused() {
    let mut rig = rig();
    start_ask(&mut rig, "ask-1", "ask-token-1");
    assert_eq!(MAX_SCREEN_CHANGING_CALLS, 20);
    for n in 0..20 {
        pending(
            rig.endpoint
                .handle(&ask_call("ask-token-1", "highlight", n)),
        );
    }
    let answer = response(
        rig.endpoint
            .handle(&ask_call("ask-token-1", "highlight", 21)),
    );
    let said = tool_failure_text(&answer);
    assert!(said.contains("20"), "{said}");
    assert!(said.contains("nothing was shown"), "{said}");
    assert_eq!(rig.window.sent().len(), 20, "the refused call went nowhere");
    assert_eq!(rig.endpoint.calls_made("ask-1"), Some(20));
}

#[test]
fn a_query_does_not_count_against_the_cap() {
    let mut rig = rig();
    start_ask(&mut rig, "ask-1", "ask-token-1");
    for n in 0..30 {
        pending(rig.endpoint.handle(&ask_call("ask-token-1", "query", n)));
    }
    assert_eq!(rig.endpoint.calls_made("ask-1"), Some(0));
    pending(rig.endpoint.handle(&ask_call("ask-token-1", "open", 99)));
}

#[test]
fn each_ask_has_its_own_count() {
    let mut rig = rig();
    start_ask(&mut rig, "ask-1", "ask-token-1");
    start_ask(&mut rig, "ask-2", "ask-token-2");
    for n in 0..20 {
        pending(rig.endpoint.handle(&ask_call("ask-token-1", "open", n)));
    }
    pending(rig.endpoint.handle(&ask_call("ask-token-2", "open", 1)));
    assert_eq!(rig.endpoint.calls_made("ask-2"), Some(1));
}

#[test]
fn a_window_reload_does_not_reset_the_count() {
    let mut rig = rig();
    start_ask(&mut rig, "ask-1", "ask-token-1");
    for n in 0..20 {
        pending(
            rig.endpoint
                .handle(&ask_call("ask-token-1", "show_groups", n)),
        );
    }
    let gone = rig.endpoint.window_gone();
    assert_eq!(gone.cancel.len(), 1);
    rig.window.set_open(true);
    assert_eq!(
        rig.endpoint.calls_made("ask-1"),
        Some(20),
        "the count survived the reload"
    );
    let answer = response(
        rig.endpoint
            .handle(&ask_call("ask-token-1", "show_groups", 21)),
    );
    assert!(tool_failure_text(&answer).contains("20"));
    assert_eq!(rig.window.sent().len(), 20);
}

#[test]
fn a_call_that_never_reached_a_window_is_not_counted() {
    let mut rig = rig_with(FakeWindow::default());
    start_ask(&mut rig, "ask-1", "ask-token-1");
    response(rig.endpoint.handle(&ask_call("ask-token-1", "open", 1)));
    assert_eq!(rig.endpoint.calls_made("ask-1"), Some(0));
}

#[test]
fn a_call_from_an_endpoint_token_client_has_no_cap_and_is_external() {
    let mut rig = rig();
    for n in 0..25 {
        pending(rig.endpoint.handle(&post(&call("open", n))));
    }
    assert!(rig.window.sent().iter().all(|e| e.caller == "external"));
}

// ---- the guards -----------------------------------------------------------------------------------------------

fn assert_refused(rig: &mut Rig, request: HttpRequest, status: u16) -> HttpResponse {
    let answer = response(rig.endpoint.handle(&request));
    assert_eq!(answer.status, status, "{}", text(&answer));
    assert!(
        rig.window.sent().is_empty(),
        "a refused request reached the window"
    );
    assert!(
        body(&answer)["error"]["message"].is_string(),
        "{}",
        text(&answer)
    );
    answer
}

#[test]
fn a_foreign_host_is_refused() {
    let mut rig = rig();
    for host in [
        "evil.example",
        "evil.example:47821",
        "127.0.0.1:1",
        "127.0.0.1",
        "localhost.evil.example:47821",
        "",
    ] {
        assert_refused(
            &mut rig,
            with_header(post(&call("open", 1)), "Host", host),
            403,
        );
    }
    assert_refused(
        &mut rig,
        without_header(post(&call("open", 1)), "Host"),
        403,
    );
}

#[test]
fn the_loopback_hosts_of_this_port_are_accepted_in_any_case() {
    let mut rig = rig();
    for host in [
        format!("127.0.0.1:{PORT}"),
        format!("localhost:{PORT}"),
        format!("LocalHost:{PORT}"),
    ] {
        let answer = response(rig.endpoint.handle(&with_header(
            post(&rpc("ping", 1, json!({}))),
            "host",
            &host,
        )));
        assert_eq!(answer.status, 200, "{host}");
    }
}

#[test]
fn a_foreign_origin_is_refused_and_a_missing_one_is_not() {
    let mut rig = rig();
    for origin in [
        "http://evil.example",
        "null",
        "http://127.0.0.1:1",
        "https://127.0.0.1:47821",
        "http://localhost.evil.example:47821",
    ] {
        assert_refused(
            &mut rig,
            with_header(post(&call("open", 1)), "Origin", origin),
            403,
        );
    }
    for origin in [
        format!("http://127.0.0.1:{PORT}"),
        format!("http://localhost:{PORT}"),
    ] {
        let answer = response(rig.endpoint.handle(&with_header(
            post(&rpc("ping", 1, json!({}))),
            "Origin",
            &origin,
        )));
        assert_eq!(answer.status, 200, "{origin}");
    }
}

#[test]
fn a_content_type_that_is_not_json_is_refused() {
    let mut rig = rig();
    for content_type in [
        "text/plain",
        "application/x-www-form-urlencoded",
        "application/jsonp",
        "multipart/form-data; boundary=x",
    ] {
        assert_refused(
            &mut rig,
            with_header(post(&call("open", 1)), "Content-Type", content_type),
            415,
        );
    }
    assert_refused(
        &mut rig,
        without_header(post(&call("open", 1)), "Content-Type"),
        415,
    );
}

#[test]
fn json_with_a_charset_or_in_capitals_is_accepted() {
    let mut rig = rig();
    for content_type in ["application/json; charset=utf-8", "Application/JSON"] {
        let request = with_header(
            post(&rpc("ping", 1, json!({}))),
            "Content-Type",
            content_type,
        );
        assert_eq!(
            response(rig.endpoint.handle(&request)).status,
            200,
            "{content_type}"
        );
    }
}

fn padded_ping(total: usize) -> Vec<u8> {
    let shell = br#"{"jsonrpc":"2.0","id":1,"method":"ping","params":{"pad":""}}"#.len();
    let mut body = br#"{"jsonrpc":"2.0","id":1,"method":"ping","params":{"pad":""#.to_vec();
    body.extend(std::iter::repeat_n(b'x', total - shell));
    body.extend_from_slice(br#""}}"#);
    assert_eq!(body.len(), total);
    body
}

#[test]
fn a_body_over_1_mb_is_refused_and_one_of_exactly_1_mb_is_not() {
    let mut rig = rig();
    assert_eq!(MAX_BODY_BYTES, 1024 * 1024);
    let mut request = post(&json!({}));
    request.body = padded_ping(MAX_BODY_BYTES);
    assert_eq!(response(rig.endpoint.handle(&request)).status, 200);
    request.body = padded_ping(MAX_BODY_BYTES + 1);
    let answer = assert_refused(&mut rig, request, 413);
    assert!(
        body(&answer)["error"]["message"]
            .as_str()
            .unwrap()
            .contains("1 MB")
    );
}

#[test]
fn a_wrong_or_missing_token_is_refused() {
    let mut rig = rig();
    for header_value in [
        "Bearer wrong",
        "Bearer ",
        "Bearer endpoint-token-000",
        "Bearer endpoint-token-00011",
        "Basic endpoint-token-0001",
        "endpoint-token-0001",
    ] {
        let answer = assert_refused(
            &mut rig,
            with_header(post(&call("open", 1)), "Authorization", header_value),
            401,
        );
        assert_eq!(header(&answer, "www-authenticate"), Some("Bearer"));
    }
    assert_refused(
        &mut rig,
        without_header(post(&call("open", 1)), "Authorization"),
        401,
    );
}

#[test]
fn a_refusal_never_holds_a_token() {
    let mut rig = rig();
    let answer = assert_refused(
        &mut rig,
        with_header(
            post(&call("open", 1)),
            "Authorization",
            "Bearer sekret-guess",
        ),
        401,
    );
    let said = text(&answer);
    assert!(
        !said.contains("sekret-guess") && !said.contains(TOKEN),
        "{said}"
    );
}

#[test]
fn the_token_may_be_the_last_path_segment() {
    let mut rig = rig();
    let mut request = without_header(post(&rpc("ping", 1, json!({}))), "Authorization");
    request.path = format!("/mcp/{TOKEN}");
    assert_eq!(response(rig.endpoint.handle(&request)).status, 200);
    request.path = format!("/mcp/{TOKEN}?x=1");
    assert_eq!(response(rig.endpoint.handle(&request)).status, 200);
    request.path = "/mcp/wrong".into();
    assert_refused(&mut rig, request.clone(), 401);
    request.path = "/mcp/".into();
    assert_refused(&mut rig, request, 401);
}

#[test]
fn a_wrong_header_token_is_not_rescued_by_a_good_path_token() {
    let mut rig = rig();
    let mut request = with_header(
        post(&rpc("ping", 1, json!({}))),
        "Authorization",
        "Bearer wrong",
    );
    request.path = format!("/mcp/{TOKEN}");
    assert_refused(&mut rig, request, 401);
}

#[test]
fn a_repeated_guarded_header_is_refused() {
    let mut rig = rig();
    for name in ["Host", "Origin", "Content-Type", "Authorization"] {
        let mut request = post(&rpc("ping", 1, json!({})));
        if name == "Origin" {
            request
                .headers
                .push(("Origin".into(), format!("http://127.0.0.1:{PORT}")));
        }
        let again = request
            .headers
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .cloned()
            .unwrap();
        request.headers.push(again);
        assert_refused(&mut rig, request, 400);
    }
}

#[test]
fn the_guards_run_before_the_method_is_looked_at() {
    let mut rig = rig();
    let mut request = with_header(post(&json!({})), "Authorization", "Bearer wrong");
    request.method = "GET".into();
    assert_eq!(response(rig.endpoint.handle(&request)).status, 401);
    let mut request = with_header(post(&json!({})), "Host", "evil.example");
    request.method = "DELETE".into();
    assert_eq!(response(rig.endpoint.handle(&request)).status, 403);
}

// ---- asks and their tokens ------------------------------------------------------------------------------------

#[test]
fn an_ask_token_works_for_its_ask_and_names_it_as_the_caller() {
    let mut rig = rig();
    start_ask(&mut rig, "ask-7", "ask-token-7");
    pending(rig.endpoint.handle(&ask_call("ask-token-7", "open", 1)));
    assert_eq!(rig.window.sent()[0].caller, "ask-7");
    assert_eq!(rig.endpoint.live_asks(), ["ask-7"]);
}

#[test]
fn an_ask_token_is_dead_when_the_ask_ends() {
    let mut rig = rig();
    start_ask(&mut rig, "ask-7", "ask-token-7");
    rig.endpoint.end_ask("ask-7");
    assert_refused(&mut rig, ask_call("ask-token-7", "open", 1), 401);
    assert_eq!(rig.endpoint.calls_made("ask-7"), None);
    assert!(rig.endpoint.live_asks().is_empty());
}

#[test]
fn an_ask_needs_a_token_the_endpoint_does_not_already_use() {
    let mut rig = rig();
    assert!(rig.endpoint.register_ask("a", "").is_err());
    assert!(rig.endpoint.register_ask("", "t").is_err());
    assert!(rig.endpoint.register_ask("a", TOKEN).is_err());
    start_ask(&mut rig, "a", "ask-token-a");
    assert!(rig.endpoint.register_ask("a", "ask-token-other").is_err());
    assert!(rig.endpoint.register_ask("b", "ask-token-a").is_err());
    assert!(
        rig.endpoint
            .register_ask("external", "ask-token-x")
            .is_err()
    );
}

// ---- the window closing ---------------------------------------------------------------------------------------

#[test]
fn closing_the_window_cancels_every_live_ask_and_a_new_window_starts_with_none() {
    let mut rig = rig();
    start_ask(&mut rig, "ask-1", "ask-token-1");
    start_ask(&mut rig, "ask-2", "ask-token-2");
    let gone = rig.endpoint.window_gone();
    let cancelled: Vec<&str> = gone.cancel.iter().map(|c| c.ask.as_str()).collect();
    assert_eq!(cancelled, ["ask-1", "ask-2"]);
    assert!(
        rig.endpoint.live_asks().is_empty(),
        "a new window starts with no asks"
    );
    assert!(
        rig.endpoint.window_gone().cancel.is_empty(),
        "an ask is cancelled once"
    );
    start_ask(&mut rig, "ask-3", "ask-token-3");
    assert_eq!(rig.endpoint.live_asks(), ["ask-3"]);
    let again = rig.endpoint.window_gone();
    assert_eq!(again.cancel.len(), 1);
    assert_eq!(again.cancel[0].ask, "ask-3");
}

#[test]
fn closing_the_window_answers_the_calls_waiting_on_it() {
    let mut rig = rig();
    pending(rig.endpoint.handle(&post(&call("open", 1))));
    pending(rig.endpoint.handle(&post(&call("open", 2))));
    let gone = rig.endpoint.window_gone();
    assert_eq!(gone.completed.len(), 2);
    for completed in &gone.completed {
        assert_eq!(tool_failure_text(&completed.response), NO_WINDOW_TEXT);
    }
    assert!(rig.endpoint.expire().is_empty());
}

#[test]
fn an_ended_ask_is_not_cancelled_when_the_window_closes() {
    let mut rig = rig();
    start_ask(&mut rig, "ask-1", "ask-token-1");
    rig.endpoint.end_ask("ask-1");
    assert!(rig.endpoint.window_gone().cancel.is_empty());
}

// ---- the ui_tools methods -------------------------------------------------------------------------------------

#[test]
fn token_and_rotate_are_handled_here_and_never_reach_the_window_or_the_daemon() {
    let mut rig = rig();
    let Rpc::Handled(token) = rig
        .endpoint
        .rpc(&owner_request("ui_tools.token", json!({})))
    else {
        panic!("ui_tools.token was forwarded");
    };
    let Response::Success { result, id, .. } = token.response else {
        panic!("failed")
    };
    assert_eq!(result, json!({ "token": TOKEN }));
    assert_eq!(id, RequestId::Number(9));
    let Rpc::Handled(rotated) = rig
        .endpoint
        .rpc(&owner_request("ui_tools.rotate", json!({})))
    else {
        panic!("ui_tools.rotate was forwarded");
    };
    let Response::Success { result, .. } = rotated.response else {
        panic!("failed")
    };
    assert_eq!(result, json!({ "token": "rotated-token-0001" }));
    assert!(
        rig.window.sent().is_empty(),
        "nothing went to the window's queue"
    );
    assert!(token.completed.is_empty() && rotated.completed.is_empty());
}

#[test]
fn rotating_ends_every_client_of_the_old_token() {
    let mut rig = rig();
    let old = response(rig.endpoint.handle(&post(&rpc("ping", 1, json!({})))));
    assert_eq!(old.status, 200);
    rig.endpoint
        .rpc(&owner_request("ui_tools.rotate", json!({})));
    assert_refused(&mut rig, post(&rpc("ping", 2, json!({}))), 401);
    let fresh = with_header(
        post(&rpc("ping", 3, json!({}))),
        "Authorization",
        "Bearer rotated-token-0001",
    );
    assert_eq!(response(rig.endpoint.handle(&fresh)).status, 200);
}

#[test]
fn another_method_is_forwarded_and_an_unknown_ui_tools_method_is_not() {
    let mut rig = rig();
    assert!(matches!(
        rig.endpoint.rpc(&owner_request("daemon.status", json!({}))),
        Rpc::Forward
    ));
    assert!(matches!(
        rig.endpoint.rpc(&owner_request("ask.start", json!({}))),
        Rpc::Forward
    ));
    let Rpc::Handled(handled) = rig.endpoint.rpc(&owner_request("ui_tools.nope", json!({}))) else {
        panic!("a ui_tools method was forwarded to the daemon");
    };
    let Response::Failure { error, .. } = handled.response else {
        panic!("succeeded")
    };
    assert_eq!(error.code, code::METHOD_NOT_FOUND);
}

#[test]
fn a_reply_with_bad_params_is_invalid_params_and_completes_nothing() {
    let mut rig = rig();
    let id = pending(rig.endpoint.handle(&post(&call("open", 1))));
    for params in [
        json!({}),
        json!({ "requestId": id }),
        json!({ "requestId": id, "content": [], "actor": "x" }),
    ] {
        let Rpc::Handled(handled) = rig.endpoint.rpc(&owner_request("ui_tools.reply", params))
        else {
            panic!("forwarded")
        };
        let Response::Failure { error, .. } = handled.response else {
            panic!("succeeded")
        };
        assert_eq!(error.code, code::INVALID_PARAMS);
        assert!(handled.completed.is_empty());
    }
    assert_eq!(
        reply(&mut rig, &id, "ok").len(),
        1,
        "the call is still waiting"
    );
}

// ---- setup ----------------------------------------------------------------------------------------------------

#[test]
fn a_manifest_that_fails_its_check_is_refused_at_construction() {
    let bad = ToolManifest::from_json(
        r#"[{"name":"a","description":"d","inputSchema":{"type":"object"}},
            {"name":"a","description":"d","inputSchema":{"type":"object"}}]"#,
    )
    .unwrap();
    let result = UiToolsEndpoint::new(
        PORT,
        bad,
        Box::new(FakeTokens::new()),
        Box::new(FakeWindow::open()),
        Box::new(FakeClocks::default()),
    );
    assert!(result.is_err());
}

#[test]
#[should_panic(expected = "ui_tools: endpoint token store")]
fn the_keychain_token_store_is_a_stub_tagged_for_stage_6b() {
    let _ = KeychainTokens.current();
}
