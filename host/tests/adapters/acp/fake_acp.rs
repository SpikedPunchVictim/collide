//! The fake ACP child on its own, as a plain child with no daemon and no session (ts-0086-tm): it writes what a
//! recording says, waits for what the recording expects on stdin, remembers the ids the host chose and exits as the
//! recording says. Then the recordings in the folder: each is JSON on every line, says something, holds only steps the
//! child knows, and follows the shapes of the ACP schema and of the scripted double of `core`.

use super::common::TempRoot;
use super::fake::{FakeAcp, recording, recordings_dir, said, steps, value_of_line};
use collide_core::testing::acp as double;
use serde_json::{Value, json};
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};

const SESSION: &str = "ses-stand-in-1";

fn initialize(id: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "method": "initialize", "params": {
        "protocolVersion": 1,
        "clientCapabilities": {"session": {"compaction": {}, "configOptions": {}, "notices": {}}}}})
}

fn session_new(id: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "method": "session/new", "params": {"cwd": "/work/repo", "mcpServers": []}})
}

fn session_prompt(id: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "method": "session/prompt", "params": {
        "sessionId": SESSION, "prompt": [{"type": "text", "text": "go"}]}})
}

/// A recording the test writes itself, in the root's folder.
fn script(root: &TempRoot, steps: &[&str]) -> PathBuf {
    let path = root.join("recording.jsonl");
    fs::write(&path, steps.join("\n")).unwrap();
    path
}

/// `value` with each `{"$id": name}` in it replaced by the id `ids` holds for the name.
fn resolve(value: &Value, ids: &[(&str, Value)]) -> Value {
    match value {
        Value::Object(map) if map.len() == 1 && map.contains_key("$id") => {
            let name = map["$id"].as_str().expect("a name");
            ids.iter()
                .find(|(known, _)| *known == name)
                .unwrap_or_else(|| panic!("no id named {name}"))
                .1
                .clone()
        }
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(key, item)| (key.clone(), resolve(item, ids)))
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.iter().map(|item| resolve(item, ids)).collect()),
        other => other.clone(),
    }
}

/// Runs a recording the test writes: sends `input`, closes stdin, and returns every line the child wrote, with how it
/// ended.
fn run(steps: &[&str], input: &[Value]) -> (Vec<Value>, super::fake::Ended) {
    let root = TempRoot::new();
    let path = script(&root, steps);
    let mut fake = FakeAcp::start(&root, &path);
    for message in input {
        fake.send(message);
    }
    fake.close_stdin();
    let lines = std::iter::from_fn(|| fake.next_value()).collect();
    (lines, fake.finish())
}

/// The next `count` lines the child writes.
fn lines(fake: &mut FakeAcp, count: usize) -> Vec<Value> {
    (0..count)
        .map(|_| fake.next_value().expect("a line"))
        .collect()
}

// ---- the child ------------------------------------------------------------------------------------------------------

#[test]
fn a_recording_is_replayed_line_for_line_with_the_ids_the_host_chose() {
    let root = TempRoot::new();
    let path = recording("prompt-turn");
    let mut fake = FakeAcp::start(&root, &path);
    let ids = [
        ("init", json!(41)),
        ("new", json!(42)),
        ("prompt", json!(43)),
    ];
    let mut got = Vec::new();
    fake.send(&initialize(json!(41)));
    got.extend(lines(&mut fake, 1));
    fake.send(&session_new(json!(42)));
    got.extend(lines(&mut fake, 1));
    fake.send(&session_prompt(json!(43)));
    got.extend(lines(&mut fake, said(&path).len() - 2));
    let want: Vec<Value> = said(&path).iter().map(|said| resolve(said, &ids)).collect();
    assert_eq!(got, want);
    // The last line is written, and the child stays up until stdin closes.
    let ended = fake.finish();
    assert_eq!(ended.code, Some(0), "{}", ended.stderr);
}

#[test]
fn the_response_to_initialize_carries_the_id_of_the_request_it_read() {
    for id in [json!(0), json!(41), json!("init-1")] {
        let root = TempRoot::new();
        let mut fake = FakeAcp::start(&root, &recording("handshake-bare"));
        fake.send(&initialize(id.clone()));
        let response = fake.next_value().expect("a response");
        assert_eq!(response["id"], id);
        assert_eq!(response["result"]["protocolVersion"], 1);
        fake.send(&session_new(json!(1)));
        assert!(fake.next_value().is_some());
        assert_eq!(fake.finish().code, Some(0));
    }
}

#[test]
fn nothing_is_written_before_the_line_the_recording_waits_for_arrives() {
    let root = TempRoot::new();
    let mut fake = FakeAcp::start(&root, &recording("handshake-bare"));
    assert!(fake.quiet_for(Duration::from_millis(300)));
    fake.send(&initialize(json!(1)));
    assert!(fake.next_value().is_some());
    assert!(fake.quiet_for(Duration::from_millis(300)));
    fake.send(&session_new(json!(2)));
    assert!(fake.next_value().is_some());
    assert_eq!(fake.finish().code, Some(0));
}

#[test]
fn an_expectation_is_a_subset_so_a_line_may_hold_more() {
    let (lines, ended) = run(
        &[
            r#"{"expect":{"method":"initialize","params":{"protocolVersion":1}}}"#,
            r#"{"say":{"jsonrpc":"2.0","result":{}}}"#,
        ],
        &[initialize(json!(1))],
    );
    assert_eq!(lines, vec![json!({"jsonrpc": "2.0", "result": {}})]);
    assert_eq!(ended.code, Some(0), "{}", ended.stderr);
}

#[test]
fn a_list_in_an_expectation_must_have_the_length_it_names() {
    for wanted in [
        r#"{"expect":{"params":{"prompt":[]}}}"#,
        r#"{"expect":{"params":{"prompt":[{"type":"text"},{"type":"text"}]}}}"#,
    ] {
        let (_, ended) = run(&[wanted], &[session_prompt(json!(1))]);
        assert_eq!(ended.code, Some(97), "{wanted}");
    }
    let (_, ended) = run(
        &[r#"{"expect":{"params":{"prompt":[{"type":"text"}]}}}"#],
        &[session_prompt(json!(1))],
    );
    assert_eq!(ended.code, Some(0), "{}", ended.stderr);
}

#[test]
fn a_line_that_does_not_match_ends_it_with_97_and_says_what_it_wanted_and_what_it_got() {
    let (lines, ended) = run(
        &[
            r#"{"expect":{"method":"session/cancel"}}"#,
            r#"{"say":{"never":"written"}}"#,
        ],
        &[initialize(json!(1))],
    );
    assert_eq!(ended.code, Some(97));
    assert!(ended.stderr.contains("session/cancel"), "{}", ended.stderr);
    assert!(ended.stderr.contains("initialize"), "{}", ended.stderr);
    assert_eq!(lines, Vec::<Value>::new());
}

#[test]
fn a_line_that_is_not_json_where_one_is_expected_ends_it_with_97_and_is_quoted() {
    let root = TempRoot::new();
    let path = script(&root, &[r#"{"expect":{"method":"initialize"}}"#]);
    let mut fake = FakeAcp::start(&root, &path);
    fake.send_bytes(b"this is not json\n");
    let ended = fake.finish();
    assert_eq!(ended.code, Some(97));
    assert!(
        ended.stderr.contains("this is not json"),
        "{}",
        ended.stderr
    );
}

#[test]
fn stdin_closing_while_an_expectation_waits_ends_it_with_98() {
    let (lines, ended) = run(&[r#"{"expect":{"method":"initialize"}}"#], &[]);
    assert_eq!(ended.code, Some(98));
    assert!(ended.stderr.contains("stdin closed"), "{}", ended.stderr);
    assert_eq!(lines, Vec::<Value>::new());
}

#[test]
fn a_step_it_does_not_know_ends_it_with_96_and_names_it() {
    let (_, ended) = run(&[r#"{"dance":true}"#], &[]);
    assert_eq!(ended.code, Some(96));
    assert!(ended.stderr.contains("dance"), "{}", ended.stderr);
}

#[test]
fn a_recording_line_that_is_not_json_ends_it_with_96() {
    let (_, ended) = run(&["this is not a step"], &[]);
    assert_eq!(ended.code, Some(96));
    assert!(ended.stderr.contains("not a step"), "{}", ended.stderr);
}

#[test]
fn an_id_never_remembered_ends_it_with_99_in_a_say_and_in_an_expectation() {
    for step in [
        r#"{"say":{"id":{"$id":"init"},"result":{}}}"#,
        r#"{"expect":{"id":{"$id":"init"}}}"#,
    ] {
        let (lines, ended) = run(&[step], &[]);
        assert_eq!(ended.code, Some(99), "{step}");
        assert!(ended.stderr.contains("init"), "{}", ended.stderr);
        assert_eq!(lines, Vec::<Value>::new());
    }
}

#[test]
fn a_notification_has_no_id_to_remember() {
    let cancel =
        json!({"jsonrpc": "2.0", "method": "session/cancel", "params": {"sessionId": SESSION}});
    let (_, ended) = run(
        &[
            r#"{"expect":{"method":"session/cancel"},"as":"cancel"}"#,
            r#"{"say":{"id":{"$id":"cancel"}}}"#,
        ],
        &[cancel],
    );
    assert_eq!(ended.code, Some(99));
}

#[test]
fn each_name_keeps_the_id_it_was_given() {
    let (lines, ended) = run(
        &[
            r#"{"expect":{"method":"initialize"},"as":"a"}"#,
            r#"{"expect":{"method":"session/new"},"as":"b"}"#,
            r#"{"say":{"first":{"$id":"a"},"second":{"$id":"b"},"again":[{"$id":"a"}]}}"#,
        ],
        &[initialize(json!(7)), session_new(json!("n-8"))],
    );
    assert_eq!(
        lines,
        vec![json!({"first": 7, "second": "n-8", "again": [7]})]
    );
    assert_eq!(ended.code, Some(0), "{}", ended.stderr);
}

#[test]
fn a_remembered_id_can_be_matched_in_a_later_expectation() {
    let steps = [
        r#"{"expect":{"method":"initialize"},"as":"init"}"#,
        r#"{"expect":{"method":"echo","params":{"id":{"$id":"init"}}}}"#,
    ];
    let echo = |id: u64| json!({"jsonrpc": "2.0", "method": "echo", "params": {"id": id}});
    let (_, ended) = run(&steps, &[initialize(json!(5)), echo(5)]);
    assert_eq!(ended.code, Some(0), "{}", ended.stderr);
    let (_, ended) = run(&steps, &[initialize(json!(5)), echo(6)]);
    assert_eq!(ended.code, Some(97));
}

#[test]
fn a_say_text_step_writes_a_line_that_is_not_json() {
    let root = TempRoot::new();
    let path = script(&root, &[r#"{"say_text":"not json at all"}"#]);
    let mut fake = FakeAcp::start(&root, &path);
    assert_eq!(fake.next_line().unwrap(), b"not json at all\n");
    assert_eq!(
        value_of_line(b"not json at all\n"),
        json!("not json at all")
    );
    assert_eq!(fake.finish().code, Some(0));
}

#[test]
fn an_exit_step_ends_it_with_the_recorded_code_and_a_stderr_step_is_written_first() {
    let root = TempRoot::new();
    let path = script(
        &root,
        &[
            r#"{"stderr":"the model connection was lost"}"#,
            r#"{"say":{"jsonrpc":"2.0","result":{}}}"#,
            r#"{"exit":3}"#,
            r#"{"say":{"never":"written"}}"#,
        ],
    );
    let mut fake = FakeAcp::start(&root, &path);
    // Stdin stays open: the exit step does not wait for it.
    assert_eq!(
        fake.next_value(),
        Some(json!({"jsonrpc": "2.0", "result": {}}))
    );
    assert_eq!(fake.next_value(), None);
    let ended = fake.finish();
    assert_eq!(ended.code, Some(3));
    assert_eq!(ended.stderr, "the model connection was lost\n");
}

#[test]
fn a_comment_and_a_blank_line_are_skipped() {
    let (lines, ended) = run(
        &[
            "# a comment",
            "",
            r#"{"say":{"jsonrpc":"2.0","result":{}}}"#,
        ],
        &[],
    );
    assert_eq!(lines.len(), 1);
    assert_eq!(ended.code, Some(0), "{}", ended.stderr);
}

#[test]
fn after_its_last_step_it_waits_for_stdin_to_close_and_then_exits_0() {
    let root = TempRoot::new();
    let path = script(&root, &[r#"{"say":{"jsonrpc":"2.0","result":{}}}"#]);
    let mut fake = FakeAcp::start(&root, &path);
    assert!(fake.next_value().is_some());
    assert!(fake.quiet_for(Duration::from_millis(300)));
    assert!(fake.is_running());
    let ended = fake.finish();
    assert_eq!(ended.code, Some(0), "{}", ended.stderr);
}

#[test]
fn a_wait_eof_step_holds_the_steps_after_it_until_stdin_closes() {
    let root = TempRoot::new();
    let path = script(
        &root,
        &[
            r#"{"wait_eof":true}"#,
            r#"{"say":{"jsonrpc":"2.0","result":{"after":"eof"}}}"#,
        ],
    );
    let mut fake = FakeAcp::start(&root, &path);
    assert!(fake.quiet_for(Duration::from_millis(300)));
    fake.close_stdin();
    assert_eq!(
        fake.next_value(),
        Some(json!({"jsonrpc": "2.0", "result": {"after": "eof"}}))
    );
    assert_eq!(fake.finish().code, Some(0));
}

#[test]
fn a_sleep_step_holds_the_steps_after_it_for_that_long() {
    let root = TempRoot::new();
    let path = script(
        &root,
        &[
            r#"{"sleep":0.4}"#,
            r#"{"say":{"jsonrpc":"2.0","result":{}}}"#,
        ],
    );
    let started = Instant::now();
    let mut fake = FakeAcp::start(&root, &path);
    assert!(fake.next_value().is_some());
    assert!(started.elapsed() >= Duration::from_millis(350));
    assert_eq!(fake.finish().code, Some(0));
}

// ---- the recordings -------------------------------------------------------------------------------------------------

const STEP_KEYS: [&str; 7] = [
    "say", "say_text", "stderr", "expect", "wait_eof", "sleep", "exit",
];

/// Every `{"$id": name}` in `value`, as the names.
fn id_names(value: &Value, into: &mut Vec<String>) {
    match value {
        Value::Object(map) if map.len() == 1 && map.contains_key("$id") => {
            into.push(map["$id"].as_str().expect("a name").to_string());
        }
        Value::Object(map) => map.values().for_each(|item| id_names(item, into)),
        Value::Array(items) => items.iter().for_each(|item| id_names(item, into)),
        _ => {}
    }
}

#[test]
fn every_recording_is_json_on_each_line_says_something_and_holds_only_steps_the_child_knows() {
    let mut seen = 0;
    for entry in fs::read_dir(recordings_dir()).unwrap() {
        let path = entry.unwrap().path();
        let text = fs::read_to_string(&path).unwrap();
        for line in text
            .lines()
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
        {
            serde_json::from_str::<Value>(line)
                .unwrap_or_else(|e| panic!("{}: {e}: {line}", path.display()));
        }
        assert!(!said(&path).is_empty(), "{}", path.display());
        let mut remembered = Vec::new();
        for step in steps(&path) {
            let object = step.as_object().expect("a step is an object");
            let known: Vec<&&str> = STEP_KEYS
                .iter()
                .filter(|k| object.contains_key(**k))
                .collect();
            assert_eq!(known.len(), 1, "{}: {step}", path.display());
            let allowed = if object.contains_key("expect") { 2 } else { 1 };
            assert!(object.len() <= allowed, "{}: {step}", path.display());
            if object.len() == 2 {
                assert!(object.contains_key("as"), "{}: {step}", path.display());
            }
            let mut used = Vec::new();
            id_names(&step, &mut used);
            for name in used {
                assert!(
                    remembered.contains(&name),
                    "{}: {name} is used before an `as` names it",
                    path.display()
                );
            }
            if let Some(name) = object.get("as") {
                remembered.push(name.as_str().expect("a name").to_string());
            }
        }
        seen += 1;
    }
    assert_eq!(seen, 5, "the recordings: add a test for a new one");
}

/// What a recording says in answer to `initialize` and to `session/new`.
fn handshake_results(name: &str) -> (Value, Value) {
    let said = said(&recording(name));
    let result = |has: &str| {
        said.iter()
            .find(|line| line["result"].get(has).is_some())
            .unwrap_or_else(|| panic!("{name} says no {has}"))["result"]
            .clone()
    };
    (result("protocolVersion"), result("sessionId"))
}

#[test]
fn the_handshake_with_config_options_offers_a_model_option_in_the_shape_the_schema_gives() {
    let (initialize, new) = handshake_results("handshake-config-options");
    assert_eq!(initialize["protocolVersion"], 1);
    assert!(initialize["agentCapabilities"]["mcpCapabilities"].is_object());
    assert!(initialize["agentCapabilities"]["promptCapabilities"].is_object());
    assert!(initialize["agentInfo"]["name"].is_string());
    assert!(new.get("models").is_none());
    let options = new["configOptions"].as_array().expect("a list");
    let model: Vec<&Value> = options
        .iter()
        .filter(|option| option["category"] == "model")
        .collect();
    assert_eq!(model.len(), 1);
    let model = model[0];
    assert_eq!(model["type"], "select");
    assert!(model["id"].is_string() && model["name"].is_string());
    let offered = model["options"].as_array().expect("a flat list");
    assert!(offered.len() >= 2);
    assert!(
        offered
            .iter()
            .all(|o| o["value"].is_string() && o["name"].is_string())
    );
    assert!(offered.iter().any(|o| o["value"] == model["currentValue"]));
    assert!(options.iter().any(|option| option["category"] != "model"));
}

#[test]
fn the_handshake_with_only_models_has_the_draft_shape() {
    let (initialize, new) = handshake_results("handshake-models");
    assert_eq!(initialize["protocolVersion"], 1);
    assert!(new.get("configOptions").is_none());
    let models = &new["models"];
    let offered = models["availableModels"].as_array().expect("a list");
    assert!(offered.len() >= 2);
    assert!(
        offered
            .iter()
            .all(|m| m["modelId"].is_string() && m["name"].is_string())
    );
    assert!(
        offered
            .iter()
            .any(|m| m["modelId"] == models["currentModelId"])
    );
}

#[test]
fn the_handshake_with_neither_names_no_model() {
    let (initialize, new) = handshake_results("handshake-bare");
    assert_eq!(initialize["protocolVersion"], 1);
    assert!(new["sessionId"].is_string());
    assert!(new.get("configOptions").is_none() && new.get("models").is_none());
}

fn parsed(bytes: Vec<u8>) -> Value {
    serde_json::from_slice(&bytes).unwrap()
}

/// The `say` of `name` that `matches`.
fn said_where(name: &str, matches: impl Fn(&Value) -> bool) -> Value {
    said(&recording(name))
        .into_iter()
        .find(|line| matches(line))
        .unwrap_or_else(|| panic!("{name} has no such line"))
}

fn update_of(kind: &'static str) -> impl Fn(&Value) -> bool {
    move |line| line["params"]["update"]["sessionUpdate"] == kind
}

#[test]
fn the_turn_and_the_permission_request_follow_the_shapes_of_the_double_in_core() {
    let turn = said_where("prompt-turn", update_of("agent_message_chunk"));
    let text = turn["params"]["update"]["content"]["text"]
        .as_str()
        .unwrap();
    assert_eq!(turn, parsed(double::message_chunk(SESSION, text)));

    let call = said_where("prompt-turn", update_of("tool_call"));
    let update = &call["params"]["update"];
    assert_eq!(update["kind"], "read");
    let (id, title) = (
        update["toolCallId"].as_str().unwrap(),
        update["title"].as_str().unwrap(),
    );
    assert_eq!(call, parsed(double::tool_call(SESSION, id, title, "read")));

    let mut bare = said_where("handshake-bare", |line| {
        line["result"].get("sessionId").is_some()
    });
    assert_eq!(bare["id"], json!({"$id": "new"}));
    bare["id"] = json!(1);
    assert_eq!(bare, parsed(double::new_session_response(1, SESSION)));

    let mut end = said_where("prompt-turn", |line| {
        line["result"].get("stopReason").is_some()
    });
    end["id"] = json!(2);
    let usage = end["result"].get("usage").cloned();
    assert_eq!(end, parsed(double::prompt_response(2, "end_turn", usage)));

    let request = said_where("permission-request", |line| {
        line["method"] == "session/request_permission"
    });
    let params = &request["params"];
    assert_eq!(params["options"], double::standard_options());
    let want = double::permission_request(
        request["id"].as_u64().unwrap(),
        SESSION,
        params["toolCall"].clone(),
        params["options"].clone(),
    );
    assert_eq!(request, parsed(want));
}

#[test]
fn the_permission_recording_waits_for_the_answer_with_the_id_the_agent_used() {
    let root = TempRoot::new();
    let path = recording("permission-request");
    let mut fake = FakeAcp::start(&root, &path);
    fake.send(&initialize(json!(10)));
    fake.next_value().unwrap();
    fake.send(&session_new(json!(11)));
    fake.next_value().unwrap();
    fake.send(&session_prompt(json!(12)));
    let request = fake.next_value().unwrap();
    assert_eq!(request["method"], "session/request_permission");
    let agent_id = request["id"].clone();
    // Only the answer to the agent's own id lets the turn go on.
    fake.send(&json!({"jsonrpc": "2.0", "id": agent_id, "result": {
        "outcome": {"outcome": "selected", "optionId": "once"}}}));
    let tail = lines(&mut fake, said(&path).len() - 2 - 1);
    assert_eq!(tail.last().unwrap()["id"], 12);
    assert_eq!(tail.last().unwrap()["result"]["stopReason"], "end_turn");
    assert_eq!(fake.finish().code, Some(0));
}
