//! Choosing the model (ts-0090-jd; 0.3, Who picks what; S7, O8, O9, O11). The session reads the model out of the
//! `session/new` response, in the stable form (`configOptions`) or the draft one (`models`), writes the request that
//! sets it, holds the first prompt until that is answered, and never picks a model the request did not name. The
//! agent's lines are written out here; the real shim runs the fake child for each form at the end.

use super::fake::{play, recording};
use super::session::{SESSION, events_of, exited, line, written};
use collide_core::adapter::{
    AgentAdapter, AgentSession, PhaseFailure, PhaseOutcome, StampedEvent, Step,
};
use collide_core::agent::{AgentEvent, ModelRef, TurnEndReason};
use collide_core::journal::{Direction, encode_record, read_journal};
use collide_core::replay::replay_journal_bytes;
use collide_core::shim::ExitEnd;
use collide_core::testing::acp::{initialize_response, message_chunk, prompt_response, update};
use collide_core::testing::sample_launch_request;
use collide_host::adapters::acp::AcpAdapter;
use collide_host::adapters::acp::options::AcpOptions;
use collide_host::adapters::acp::profile::AcpProfile;
use collide_host::adapters::acp::session::AcpSession;
use serde_json::{Value, json};
use std::path::PathBuf;

const A: &str = "stand-in-model-a";
const B: &str = "stand-in-model-b";
const C: &str = "stand-in-model-c";
const HOSTED: &str = "stand-in-hosted/big-model";
const ODD: &str = "ollama/qwen2.5:7b";

fn options(model: Option<&str>) -> AcpOptions {
    let options = AcpOptions::new("/work/repo", "do the task");
    match model {
        Some(id) => options.with_model(id),
        None => options,
    }
}

// ---- what the agent says ---------------------------------------------------------------------------------------

fn option_entry(id: &str) -> Value {
    json!({ "value": id, "name": id })
}

/// A model option of `configOptions`, with a flat list.
fn model_option(config_id: &str, current: &str, ids: &[&str]) -> Value {
    let list: Vec<Value> = ids.iter().map(|id| option_entry(id)).collect();
    json!({
        "id": config_id, "name": "Model", "category": "model", "type": "select",
        "currentValue": current, "options": list
    })
}

/// A model option whose list is groups of options.
fn grouped_model_option(config_id: &str, current: &str, groups: &[(&str, &[&str])]) -> Value {
    let list: Vec<Value> = groups
        .iter()
        .map(|(name, ids)| {
            let options: Vec<Value> = ids.iter().map(|id| option_entry(id)).collect();
            json!({ "group": name, "name": name, "options": options })
        })
        .collect();
    json!({
        "id": config_id, "name": "Model", "category": "model", "type": "select",
        "currentValue": current, "options": list
    })
}

fn mode_option() -> Value {
    json!({
        "id": "mode", "name": "Mode", "category": "mode", "type": "select", "currentValue": "build",
        "options": [option_entry("build"), option_entry("plan")]
    })
}

/// The draft form: `models`, with `currentModelId` and `availableModels`.
fn draft_models(current: &str, ids: &[&str]) -> Value {
    let list: Vec<Value> = ids
        .iter()
        .map(|id| json!({ "modelId": id, "name": id }))
        .collect();
    json!({ "currentModelId": current, "availableModels": list })
}

/// The `session/new` response (request 1) with these fields besides the session id.
fn new_response(extra: Value) -> Vec<u8> {
    let mut result = json!({ "sessionId": SESSION });
    for (key, value) in extra.as_object().expect("fields") {
        result[key] = value.clone();
    }
    line(json!({ "jsonrpc": "2.0", "id": 1, "result": result }))
}

fn with_config_options(options: Vec<Value>) -> Vec<u8> {
    new_response(json!({ "configOptions": options }))
}

fn with_draft_models(current: &str, ids: &[&str]) -> Vec<u8> {
    new_response(json!({ "models": draft_models(current, ids) }))
}

/// The answer to request `id`: a result with these fields.
fn answer(id: u64, result: Value) -> Vec<u8> {
    line(json!({ "jsonrpc": "2.0", "id": id, "result": result }))
}

fn error_answer(id: u64) -> Vec<u8> {
    line(
        json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32602, "message": "TEXT-STAND-IN" } }),
    )
}

// ---- what the session does ----------------------------------------------------------------------------------

/// A started session that has read `initialize`, so the next line it reads is the answer to `session/new`.
fn at_new(model: Option<&str>) -> AcpSession {
    let mut session = AcpSession::new(options(model));
    session.start();
    session.feed(&initialize_response(0), 100);
    session
}

/// The step that reads the `session/new` response `response`.
fn new_step(model: Option<&str>, response: &[u8]) -> (AcpSession, Step) {
    let mut session = at_new(model);
    let step = session.feed(response, 110);
    (session, step)
}

fn methods(step: &Step) -> Vec<String> {
    written(step)
        .iter()
        .map(|message| message["method"].as_str().unwrap_or("").to_string())
        .collect()
}

fn other(name: &str, raw: Option<Value>) -> AgentEvent {
    AgentEvent::Other {
        raw_type: name.into(),
        raw,
    }
}

fn unavailable(offered: u64) -> AgentEvent {
    other("model_unavailable", Some(json!({ "offered": offered })))
}

fn bound() -> AgentEvent {
    AgentEvent::SessionBound { id: SESSION.into() }
}

fn refused() -> PhaseOutcome {
    PhaseOutcome::Failed {
        cause: PhaseFailure::ModelUnavailable,
    }
}

fn set_config(config_id: &str, value: &str) -> Value {
    json!({
        "jsonrpc": "2.0", "id": 2, "method": "session/set_config_option",
        "params": { "sessionId": SESSION, "configId": config_id, "value": value }
    })
}

fn set_model(id: &str) -> Value {
    json!({
        "jsonrpc": "2.0", "id": 2, "method": "session/set_model",
        "params": { "sessionId": SESSION, "modelId": id }
    })
}

/// The first prompt, as request `id`.
fn first_prompt(id: u64) -> Value {
    json!({
        "jsonrpc": "2.0", "id": id, "method": "session/prompt",
        "params": { "sessionId": SESSION, "prompt": [{ "type": "text", "text": "do the task" }] }
    })
}

fn config_after(current: &str) -> Value {
    json!({ "configOptions": [mode_option(), model_option("model", current, &[A, B])] })
}

// ---- the model in `configOptions` ----------------------------------------------------------------------------

#[test]
fn an_id_the_config_option_offers_is_set_with_set_config_option_and_no_prompt_is_written_yet() {
    let response = with_config_options(vec![mode_option(), model_option("model", A, &[A, B])]);
    let (_, step) = new_step(Some(B), &response);
    assert_eq!(events_of(&step), vec![bound()]);
    assert_eq!(written(&step), vec![set_config("model", B)]);
}

#[test]
fn the_request_names_the_option_by_its_own_id_not_by_the_word_model() {
    let response = with_config_options(vec![model_option("model_choice", A, &[A, B])]);
    let (_, step) = new_step(Some(B), &response);
    assert_eq!(written(&step), vec![set_config("model_choice", B)]);
}

#[test]
fn an_id_in_a_group_of_a_grouped_list_is_offered() {
    let option = grouped_model_option("model", A, &[("Local", &[A]), ("Other", &[B, HOSTED])]);
    let (_, step) = new_step(Some(B), &with_config_options(vec![option]));
    assert_eq!(written(&step), vec![set_config("model", B)]);
}

#[test]
fn an_id_that_is_already_current_writes_no_model_request_and_the_prompt_follows_at_once() {
    let response = with_config_options(vec![model_option("model", B, &[A, B])]);
    let (_, step) = new_step(Some(B), &response);
    assert_eq!(events_of(&step), vec![bound()]);
    assert_eq!(written(&step), vec![first_prompt(2)]);
}

#[test]
fn an_id_that_is_not_offered_writes_nothing_and_is_model_unavailable_with_the_count_offered() {
    let response = with_config_options(vec![model_option("model", A, &[A, B])]);
    let (mut session, step) = new_step(Some(C), &response);
    assert_eq!(events_of(&step), vec![bound(), unavailable(2)]);
    assert!(step.writes.is_empty());
    assert_eq!(session.finish(&exited()).outcome, refused());
}

#[test]
fn the_id_is_not_in_the_event_that_says_the_model_is_unavailable() {
    let response = with_config_options(vec![model_option("model", A, &[A, B])]);
    let (_, step) = new_step(Some(ODD), &response);
    let kept = format!("{:?}", step.events);
    assert!(!kept.contains("qwen"), "{kept}");
}

#[test]
fn a_grouped_list_counts_every_option_of_every_group() {
    let option = grouped_model_option("model", A, &[("Local", &[A]), ("Other", &[B, HOSTED])]);
    let (_, step) = new_step(Some(C), &with_config_options(vec![option]));
    assert_eq!(events_of(&step), vec![bound(), unavailable(3)]);
}

#[test]
fn a_hosted_model_the_agent_lists_is_never_chosen_when_the_request_names_another() {
    // S7, O11: the agent lists its own hosted models next to the local ones.
    let response = with_config_options(vec![model_option("model", A, &[A, HOSTED])]);
    let (_, step) = new_step(Some(C), &response);
    assert!(step.writes.is_empty());
    assert_eq!(events_of(&step), vec![bound(), unavailable(2)]);
}

#[test]
fn a_request_with_no_model_keeps_the_agents_own_even_when_it_lists_a_hosted_one() {
    let response = with_config_options(vec![model_option("model", A, &[A, HOSTED])]);
    let (_, step) = new_step(None, &response);
    assert_eq!(written(&step), vec![first_prompt(2)]);
}

#[test]
fn an_answer_that_holds_the_model_as_current_lets_the_prompt_follow() {
    let response = with_config_options(vec![mode_option(), model_option("model", A, &[A, B])]);
    let (mut session, _) = new_step(Some(B), &response);
    let step = session.feed(&answer(2, config_after(B)), 120);
    assert!(step.events.is_empty());
    assert_eq!(written(&step), vec![first_prompt(3)]);
}

#[test]
fn the_prompt_is_held_until_the_answer_and_the_writes_after_session_new_are_model_then_prompt() {
    let response = with_config_options(vec![model_option("model", A, &[A, B])]);
    let (mut session, first) = new_step(Some(B), &response);
    let notice = session.feed(&update(SESSION, json!({ "sessionUpdate": "plan" })), 115);
    let second = session.feed(&answer(2, config_after(B)), 120);
    let all: Vec<String> = [&first, &notice, &second]
        .iter()
        .flat_map(|step| methods(step))
        .collect();
    assert_eq!(all, ["session/set_config_option", "session/prompt"]);
    assert!(
        notice.writes.is_empty(),
        "nothing is written while it waits"
    );
}

#[test]
fn an_error_in_answer_to_set_config_option_writes_no_prompt_and_fails_the_phase() {
    let response = with_config_options(vec![model_option("model", A, &[A, B])]);
    let (mut session, _) = new_step(Some(B), &response);
    let step = session.feed(&error_answer(2), 120);
    assert!(step.writes.is_empty());
    assert_eq!(events_of(&step), vec![other("model_set_error", None)]);
    assert_eq!(session.finish(&exited()).outcome, refused());
}

#[test]
fn an_answer_that_leaves_another_model_current_writes_no_prompt_and_fails_the_phase() {
    // S7, O8: an agent can answer and keep its own model. Only `currentValue` after the write is believed.
    let response = with_config_options(vec![model_option("model", A, &[A, B])]);
    let (mut session, _) = new_step(Some(B), &response);
    let step = session.feed(&answer(2, config_after(A)), 120);
    assert!(step.writes.is_empty());
    assert_eq!(events_of(&step), vec![other("model_not_set", None)]);
    assert_eq!(session.finish(&exited()).outcome, refused());
}

#[test]
fn an_answer_with_no_options_is_not_believed() {
    for result in [json!({}), json!({ "configOptions": [mode_option()] })] {
        let response = with_config_options(vec![model_option("model", A, &[A, B])]);
        let (mut session, _) = new_step(Some(B), &response);
        let step = session.feed(&answer(2, result), 120);
        assert!(step.writes.is_empty());
        assert_eq!(events_of(&step), vec![other("model_not_set", None)]);
    }
}

// ---- the model in the draft `models` --------------------------------------------------------------------------

#[test]
fn an_id_the_draft_list_offers_is_set_with_set_model_and_no_prompt_is_written_yet() {
    let (_, step) = new_step(Some(B), &with_draft_models(A, &[A, B]));
    assert_eq!(events_of(&step), vec![bound()]);
    assert_eq!(written(&step), vec![set_model(B)]);
}

#[test]
fn a_draft_id_that_is_already_current_writes_no_model_request() {
    let (_, step) = new_step(Some(B), &with_draft_models(B, &[A, B]));
    assert_eq!(written(&step), vec![first_prompt(2)]);
}

#[test]
fn a_draft_id_that_is_not_offered_writes_nothing_and_is_model_unavailable() {
    let (mut session, step) = new_step(Some(C), &with_draft_models(A, &[A, B]));
    assert_eq!(events_of(&step), vec![bound(), unavailable(2)]);
    assert!(step.writes.is_empty());
    assert_eq!(session.finish(&exited()).outcome, refused());
}

#[test]
fn a_hosted_draft_model_is_never_chosen_when_the_request_names_another() {
    let (_, step) = new_step(Some(C), &with_draft_models(A, &[A, HOSTED]));
    assert!(step.writes.is_empty());
    let (_, step) = new_step(None, &with_draft_models(A, &[A, HOSTED]));
    assert_eq!(methods(&step), ["session/prompt"]);
}

#[test]
fn an_empty_success_to_set_model_lets_the_prompt_follow() {
    let (mut session, _) = new_step(Some(B), &with_draft_models(A, &[A, B]));
    let step = session.feed(&answer(2, json!({})), 120);
    assert!(step.events.is_empty());
    assert_eq!(written(&step), vec![first_prompt(3)]);
}

#[test]
fn the_meta_a_real_agent_answers_set_model_with_is_a_success() {
    // S7: opencode answered `set_model` with `{"_meta":{"opencode":...}}`.
    let (mut session, _) = new_step(Some(B), &with_draft_models(A, &[A, B]));
    let step = session.feed(&answer(2, json!({ "_meta": { "stand-in": {} } })), 120);
    assert_eq!(written(&step), vec![first_prompt(3)]);
}

#[test]
fn an_error_in_answer_to_set_model_writes_no_prompt_and_fails_the_phase() {
    let (mut session, _) = new_step(Some(B), &with_draft_models(A, &[A, B]));
    let step = session.feed(&error_answer(2), 120);
    assert!(step.writes.is_empty());
    assert_eq!(events_of(&step), vec![other("model_set_error", None)]);
    assert_eq!(session.finish(&exited()).outcome, refused());
}

#[test]
fn a_current_model_update_after_set_model_is_an_other_and_changes_nothing() {
    let (mut session, _) = new_step(Some(B), &with_draft_models(A, &[A, B]));
    let notice = session.feed(
        &update(
            SESSION,
            json!({ "sessionUpdate": "current_model_update", "currentModelId": A }),
        ),
        115,
    );
    assert_eq!(
        events_of(&notice),
        vec![other("current_model_update", None)]
    );
    assert!(notice.writes.is_empty());
    let step = session.feed(&answer(2, json!({})), 120);
    assert_eq!(written(&step), vec![first_prompt(3)]);
}

// ---- both forms, one form, neither ----------------------------------------------------------------------------

#[test]
fn with_both_forms_in_one_response_config_options_wins() {
    let response = new_response(json!({
        "configOptions": [model_option("model", A, &[A, B])],
        "models": draft_models(A, &[A, B])
    }));
    let (_, step) = new_step(Some(B), &response);
    assert_eq!(methods(&step), ["session/set_config_option"]);
}

#[test]
fn when_both_forms_are_there_an_id_only_the_draft_list_has_is_not_offered() {
    let response = new_response(json!({
        "configOptions": [model_option("model", A, &[A])],
        "models": draft_models(A, &[A, B])
    }));
    let (_, step) = new_step(Some(B), &response);
    assert!(step.writes.is_empty());
    assert_eq!(events_of(&step), vec![bound(), unavailable(1)]);
}

#[test]
fn config_options_with_no_model_option_fall_back_to_the_draft_list() {
    let response = new_response(json!({
        "configOptions": [mode_option()],
        "models": draft_models(A, &[A, B])
    }));
    let (_, step) = new_step(Some(B), &response);
    assert_eq!(written(&step), vec![set_model(B)]);
}

#[test]
fn a_request_with_no_model_writes_no_model_request_in_any_form() {
    let responses = [
        with_config_options(vec![model_option("model", A, &[A, B])]),
        with_draft_models(A, &[A, B]),
        new_response(json!({})),
    ];
    for response in responses {
        let (_, step) = new_step(None, &response);
        assert_eq!(events_of(&step), vec![bound()]);
        assert_eq!(written(&step), vec![first_prompt(2)]);
    }
}

#[test]
fn an_agent_with_no_list_and_no_way_to_set_one_fails_the_phase_when_a_model_is_asked_for() {
    let (mut session, step) = new_step(Some(B), &new_response(json!({})));
    assert_eq!(events_of(&step), vec![bound(), unavailable(0)]);
    assert!(step.writes.is_empty());
    assert_eq!(session.finish(&exited()).outcome, refused());
}

#[test]
fn an_agent_with_no_list_runs_when_no_model_is_asked_for() {
    let (mut session, step) = new_step(None, &new_response(json!({})));
    assert_eq!(written(&step), vec![first_prompt(2)]);
    session.feed(&message_chunk(SESSION, "done"), 150);
    session.feed(&prompt_response(2, "end_turn", None), 200);
    assert_eq!(session.finish(&exited()).outcome, PhaseOutcome::Ended);
}

#[test]
fn a_model_list_that_is_not_a_list_offers_nothing() {
    let responses = [
        new_response(json!({ "models": "none" })),
        new_response(json!({ "models": { "availableModels": 3 } })),
        new_response(json!({ "configOptions": { "id": "model" } })),
        with_config_options(vec![json!({ "category": "model", "currentValue": A })]),
    ];
    for response in responses {
        let (_, step) = new_step(Some(B), &response);
        assert_eq!(events_of(&step), vec![bound(), unavailable(0)]);
    }
}

// ---- the id is only data -------------------------------------------------------------------------------------

#[test]
fn an_id_with_a_slash_and_a_colon_is_written_as_given_in_either_form() {
    let response = with_config_options(vec![model_option("model", A, &[A, ODD])]);
    let (_, step) = new_step(Some(ODD), &response);
    assert_eq!(written(&step), vec![set_config("model", ODD)]);
    let text = String::from_utf8(step.writes[0].clone()).unwrap();
    assert!(text.contains("\"ollama/qwen2.5:7b\""), "{text}");

    let (_, step) = new_step(Some(ODD), &with_draft_models(A, &[A, ODD]));
    assert_eq!(written(&step), vec![set_model(ODD)]);
}

#[test]
fn launch_puts_no_part_of_the_model_in_argv_env_or_a_path() {
    let mut request = sample_launch_request();
    request.model = Some(ModelRef {
        name: "qwenlocal".into(),
        id: Some(ODD.into()),
    });
    request.cwd = PathBuf::from("/work/repo");
    request.snapshot_dir = PathBuf::from("/snap");
    let profile = AcpProfile::new("test-agent", "test-acp-agent", vec!["serve".into()]);
    let spec = AcpAdapter::new(profile).launch(&request).unwrap();
    let mut seen: Vec<String> = spec.argv.clone();
    seen.extend(spec.env.iter().flat_map(|(k, v)| [k.clone(), v.clone()]));
    seen.push(spec.cwd.display().to_string());
    for word in seen {
        assert!(!word.contains("qwen") && !word.contains("ollama"), "{word}");
    }
}

#[test]
fn the_adapter_hands_the_requests_model_id_to_its_session_and_the_name_when_there_is_no_id() {
    let profile = AcpProfile::new("test-agent", "test-acp-agent", vec![]);
    let adapter = AcpAdapter::new(profile);
    let asked = |model: ModelRef, listed: &[&str]| {
        let mut request = sample_launch_request();
        request.model = Some(model);
        let mut session = adapter.session(&request);
        session.start();
        session.feed(&initialize_response(0), 100);
        session.feed(&with_draft_models(listed[0], listed), 110)
    };
    let with_id = ModelRef {
        name: "qwenlocal".into(),
        id: Some(ODD.into()),
    };
    let step = asked(with_id, &[A, ODD]);
    assert_eq!(written(&step), vec![set_model(ODD)]);
    let bare = ModelRef {
        name: B.into(),
        id: None,
    };
    let step = asked(bare, &[A, B]);
    assert_eq!(written(&step), vec![set_model(B)]);
}

#[test]
fn a_request_with_no_model_gives_a_session_that_writes_no_model_request() {
    let adapter = AcpAdapter::new(AcpProfile::new("test-agent", "test-acp-agent", vec![]));
    let mut session = adapter.session(&sample_launch_request());
    session.start();
    session.feed(&initialize_response(0), 100);
    let step = session.feed(&with_draft_models(A, &[A, B]), 110);
    assert_eq!(methods(&step), ["session/prompt"]);
}

// ---- a turn is not sent before the model is set ------------------------------------------------------------

#[test]
fn a_turn_sent_while_the_model_is_not_set_is_not_written() {
    let (mut session, _) = new_step(Some(B), &with_draft_models(A, &[A, B]));
    let step = session.send_turn("early");
    assert!(step.writes.is_empty());
    assert_eq!(events_of(&step), vec![other("turn_not_sent", None)]);
}

#[test]
fn a_turn_sent_after_the_model_was_refused_is_not_written() {
    let (mut session, _) = new_step(Some(C), &with_draft_models(A, &[A, B]));
    let step = session.send_turn("later");
    assert!(step.writes.is_empty());
    assert_eq!(events_of(&step), vec![other("turn_not_sent", None)]);
}

#[test]
fn a_turn_sent_after_the_model_was_set_and_the_first_turn_ended_is_written() {
    let (mut session, _) = new_step(Some(B), &with_draft_models(A, &[A, B]));
    session.feed(&answer(2, json!({})), 120);
    session.feed(&prompt_response(3, "end_turn", None), 200);
    let step = session.send_turn("next");
    assert_eq!(methods(&step), ["session/prompt"]);
    assert_eq!(written(&step)[0]["id"], 4);
}

#[test]
fn a_model_that_was_set_gives_an_ended_phase_when_the_turn_ends_well() {
    let (mut session, _) = new_step(Some(B), &with_draft_models(A, &[A, B]));
    session.feed(&answer(2, json!({})), 120);
    session.feed(&message_chunk(SESSION, "done"), 150);
    let step = session.feed(&prompt_response(3, "end_turn", None), 200);
    assert!(matches!(
        events_of(&step).last(),
        Some(AgentEvent::TurnEnd {
            reason: TurnEndReason::EndTurn {},
            ..
        })
    ));
    assert_eq!(session.finish(&exited()).outcome, PhaseOutcome::Ended);
}

#[test]
fn a_later_config_option_update_stays_an_other_and_writes_nothing() {
    let (mut session, _) = new_step(Some(B), &with_draft_models(A, &[A, B]));
    session.feed(&answer(2, json!({})), 120);
    let step = session.feed(
        &update(SESSION, json!({ "sessionUpdate": "config_option_update" })),
        130,
    );
    assert_eq!(events_of(&step), vec![other("config_option_update", None)]);
    assert!(step.writes.is_empty());
}

// ---- replay --------------------------------------------------------------------------------------------------------

/// The `in` and `out` records of a journal as lines of JSON.
fn journal_lines(journal: &[u8], direction: Direction) -> Vec<Vec<u8>> {
    read_journal(journal)
        .records
        .into_iter()
        .filter(|record| record.direction == direction)
        .map(|record| record.bytes)
        .collect()
}

fn methods_written(journal: &[u8]) -> Vec<String> {
    journal_lines(journal, Direction::In)
        .iter()
        .map(|bytes| {
            let message: Value = serde_json::from_slice(bytes).unwrap();
            message["method"].as_str().unwrap_or("").to_string()
        })
        .collect()
}

fn no_reaction(_: &mut AcpSession, _: &AgentEvent) -> Step {
    Step::default()
}

fn played(name: &str, model: &str) -> super::fake::Played {
    play(&recording(name), options(Some(model)), &[], no_reaction)
}

#[test]
fn replayed_from_the_journal_each_form_reaches_the_state_of_the_live_session() {
    for (name, model) in [
        ("model-config-options", B),
        ("model-set-model", "stand-in/model:b"),
        ("model-not-offered", C),
    ] {
        let live = played(name, model);
        let mut rebuilt = AcpSession::new(options(Some(model)));
        let replay =
            replay_journal_bytes(&mut rebuilt, &live.journal).expect("the journal replays");
        assert_eq!(replay.step.events, live.fed_events(), "{name}");
        assert!(replay.step.writes.is_empty(), "{name}");
        assert_eq!(rebuilt, live.session, "{name}");
        let mut live = live;
        assert_eq!(
            rebuilt.finish(&live.exit),
            live.session.finish(&live.exit),
            "{name}"
        );
    }
}

#[test]
fn a_journal_cut_between_the_model_request_and_its_answer_writes_the_prompt_when_the_answer_comes()
{
    let live = played("model-config-options", B);
    let records = read_journal(&live.journal).records;
    let asked = records
        .iter()
        .position(|record| {
            record.direction == Direction::In
                && String::from_utf8_lossy(&record.bytes).contains("set_config_option")
        })
        .expect("the model request is in the journal");
    let answered = records
        .iter()
        .position(|record| {
            record.direction == Direction::Out
                && String::from_utf8_lossy(&record.bytes).contains("\"configOptions\"")
                && record.seq > records[asked].seq
        })
        .expect("the answer is in the journal");
    let prompt = records
        .iter()
        .find(|record| {
            record.direction == Direction::In
                && String::from_utf8_lossy(&record.bytes).contains("session/prompt")
        })
        .expect("the prompt is in the journal");

    // The journal as it was when the daemon went down: the request is there, the answer is not.
    let mut cut = Vec::new();
    for record in &records[..=asked] {
        cut.extend(encode_record(record).unwrap());
    }
    let mut rebuilt = AcpSession::new(options(Some(B)));
    replay_journal_bytes(&mut rebuilt, &cut).unwrap();
    assert!(
        rebuilt.send_turn("early").writes.is_empty(),
        "the model is not set yet"
    );
    let step = rebuilt.feed(&records[answered].bytes, records[answered].wall_ns);
    assert_eq!(step.writes, std::slice::from_ref(&prompt.bytes));
}

// ---- the fake child under the real shim ---------------------------------------------------------------------------

fn bound_then_one_turn(events: &[StampedEvent]) {
    let events: Vec<AgentEvent> = events.iter().map(|stamped| stamped.event.clone()).collect();
    assert_eq!(events.first(), Some(&bound()));
    assert!(matches!(
        events.last(),
        Some(AgentEvent::TurnEnd {
            reason: TurnEndReason::EndTurn {},
            ..
        })
    ));
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, AgentEvent::Other { raw_type, .. } if raw_type == "model_unavailable"))
    );
}

#[test]
fn under_the_shim_a_child_with_config_options_is_set_through_set_config_option_then_serves_a_turn()
{
    let mut live = played("model-config-options", B);
    bound_then_one_turn(&live.events);
    assert_eq!(
        methods_written(&live.journal),
        [
            "initialize",
            "session/new",
            "session/set_config_option",
            "session/prompt"
        ]
    );
    assert_eq!(live.session.finish(&live.exit).outcome, PhaseOutcome::Ended);
}

#[test]
fn under_the_shim_a_child_with_only_models_is_set_through_set_model_with_a_slash_and_a_colon_in_the_id()
 {
    let mut live = played("model-set-model", "stand-in/model:b");
    bound_then_one_turn(&live.events);
    assert_eq!(
        methods_written(&live.journal),
        [
            "initialize",
            "session/new",
            "session/set_model",
            "session/prompt"
        ]
    );
    let sent = journal_lines(&live.journal, Direction::In);
    let text = String::from_utf8(sent[2].clone()).unwrap();
    assert!(text.contains("\"stand-in/model:b\""), "{text}");
    assert_eq!(live.session.finish(&live.exit).outcome, PhaseOutcome::Ended);
}

#[test]
fn under_the_shim_a_child_that_offers_other_ids_gets_no_request_and_is_stopped_by_the_host() {
    let mut live = played("model-not-offered", C);
    let events = live.agent_events();
    assert_eq!(events.last(), Some(&unavailable(2)));
    assert_eq!(
        methods_written(&live.journal),
        ["initialize", "session/new"]
    );
    let exited_clean = matches!(
        live.exit.record.as_ref().map(|record| &record.end),
        Some(ExitEnd::Exited { code: 0 })
    );
    assert!(exited_clean, "{:?}", live.exit);
    assert_eq!(live.session.finish(&live.exit).outcome, refused());
}
