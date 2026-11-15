//! Hook outcomes, refused tool calls and the failed-open observation (ts-0063-rw; 0.3, The stream protocol; 0.13).
//! The refusal cases are S9's addendum, c0 to c7b: the shapes are built from its table, never copied from a recording
//! of the owner's. The recordings are played through fake Claude under the real shim.

use super::fake::{Played, assigned, play};
use super::session::{
    allow_all, answer_nothing, bare, events as events_of, line, result_line, session,
};
use collide_core::adapter::{AgentSession, ExitInfo, Step};
use collide_core::agent::{
    AgentEvent, HookDecision, HookFailure, HookOutcome, PermissionReply, RefusalCause,
};
use collide_core::journal::{Direction, read_journal};
use serde_json::{Value, json};

/// The events of `step` that these tests are about: the usage and size events are in `usage.rs` and `tool_results.rs`.
fn events(step: &Step) -> Vec<AgentEvent> {
    bare(events_of(step))
}

// ---- lines ---------------------------------------------------------------------------------------------------------

fn tool_use(id: &str, tool: &str) -> Vec<u8> {
    line(json!({"type": "assistant", "message": {"content": [
        {"type": "tool_use", "id": id, "name": tool, "input": {"command": "touch out.txt"}}]}}))
}

fn hook_started(name: &str, event: &str) -> Vec<u8> {
    line(
        json!({"type": "system", "subtype": "hook_started", "hook_id": "h1", "hook_name": name, "hook_event": event}),
    )
}

fn hook_response(name: &str, event: &str, outcome: &str, exit_code: Option<i32>) -> Vec<u8> {
    let mut response = json!({
        "type": "system", "subtype": "hook_response", "hook_id": "h1", "hook_name": name, "hook_event": event,
        "output": "", "stdout": "", "stderr": "", "outcome": outcome,
    });
    if let Some(code) = exit_code {
        response["exit_code"] = json!(code);
    }
    line(response)
}

fn hook_response_with(name: &str, stdout: &str, stderr: &str, exit_code: i32) -> Vec<u8> {
    line(json!({
        "type": "system", "subtype": "hook_response", "hook_id": "h1", "hook_name": name,
        "hook_event": name.split(':').next().unwrap(), "output": stdout, "stdout": stdout, "stderr": stderr,
        "exit_code": exit_code, "outcome": if exit_code == 0 { "success" } else { "error" },
    }))
}

fn no_verdict(event: &str, tool_use_id: &str) -> Vec<u8> {
    line(json!({
        "type": "system", "subtype": "informational", "level": "warning", "tag": "hook_no_verdict",
        "hook_no_verdict": {"hook_event": event, "tool_use_id": tool_use_id, "timed_out": true},
        "content": "A hook didn't answer in time, so it didn't check this action.",
    }))
}

/// A `user` line with one tool result for `id`, and `meta` as its `tool_result_meta` entry (`None`: no such key).
fn tool_result(id: &str, is_error: bool, meta: Option<Value>) -> Vec<u8> {
    let mut user = json!({"type": "user", "message": {"role": "user", "content": [
        {"type": "tool_result", "content": "text that says nothing reliable", "is_error": is_error, "tool_use_id": id}]}});
    if let Some(mut meta) = meta {
        meta["id"] = json!(id);
        user["tool_result_meta"] = json!([meta]);
    }
    line(user)
}

fn decision(decision: &str, source: &str) -> Value {
    json!({"permission_decision": {"decision": decision, "source": source}})
}

fn is_hook_event(event: &AgentEvent) -> bool {
    matches!(
        event,
        AgentEvent::HookCall { .. }
            | AgentEvent::HookFailedOpen { .. }
            | AgentEvent::ToolRefused { .. }
    )
}

/// Feeds each line in order and returns the hook and refusal events they make, in order.
fn feed_all(session: &mut impl AgentSession, lines: &[Vec<u8>]) -> Vec<AgentEvent> {
    lines
        .iter()
        .enumerate()
        .flat_map(|(at, bytes)| events(&session.feed(bytes, at as u64 + 1)))
        .filter(is_hook_event)
        .collect()
}

fn hook_call(
    name: &str,
    event: &str,
    outcome: HookOutcome,
    exit_code: Option<i32>,
    decision: Option<HookDecision>,
) -> AgentEvent {
    AgentEvent::HookCall {
        name: name.into(),
        event: event.into(),
        outcome,
        exit_code,
        decision,
    }
}

fn refused(id: &str, cause: RefusalCause) -> AgentEvent {
    AgentEvent::ToolRefused {
        id: id.into(),
        cause,
    }
}

fn failed_open(name: &str, event: &str, id: &str, cause: HookFailure) -> AgentEvent {
    AgentEvent::HookFailedOpen {
        name: name.into(),
        event: event.into(),
        tool_call_id: id.into(),
        cause,
    }
}

/// The events of a played recording that are hook or refusal events, in order.
fn hook_events(played: &Played) -> Vec<AgentEvent> {
    played
        .agent_events()
        .into_iter()
        .filter(is_hook_event)
        .collect()
}

// ---- hook outcomes -------------------------------------------------------------------------------------------------

#[test]
fn hook_started_adds_no_event_and_hook_response_gives_one_hook_call() {
    let mut session = session();
    assert_eq!(
        events(&session.feed(&hook_started("PreToolUse:Bash", "PreToolUse"), 1)),
        []
    );
    let step = session.feed(
        &hook_response("PreToolUse:Bash", "PreToolUse", "success", Some(0)),
        2,
    );
    assert_eq!(
        events(&step),
        [hook_call(
            "PreToolUse:Bash",
            "PreToolUse",
            HookOutcome::Success,
            Some(0),
            None
        )]
    );
    assert_eq!(step.events[0].wall_ns, Some(2));
}

#[test]
fn a_hook_call_is_read_for_each_outcome_s9_measured() {
    // (name, event, outcome, exit_code) -> the HookCall: pass, exit-2 deny, exit 1, exit 2 on PostToolUse, timeout.
    for (name, event, outcome, code, want) in [
        (
            "PreToolUse:Bash",
            "PreToolUse",
            "success",
            Some(0),
            (HookOutcome::Success, Some(0)),
        ),
        (
            "PreToolUse:Bash",
            "PreToolUse",
            "error",
            Some(2),
            (HookOutcome::Error, Some(2)),
        ),
        (
            "PreToolUse:Bash",
            "PreToolUse",
            "error",
            Some(1),
            (HookOutcome::Error, Some(1)),
        ),
        (
            "PostToolUse:Bash",
            "PostToolUse",
            "error",
            Some(2),
            (HookOutcome::Error, Some(2)),
        ),
        (
            "PreToolUse:Bash",
            "PreToolUse",
            "cancelled",
            Some(1),
            (HookOutcome::Cancelled, Some(1)),
        ),
        (
            "SessionStart:startup",
            "SessionStart",
            "success",
            None,
            (HookOutcome::Success, None),
        ),
    ] {
        let step = session().feed(&hook_response(name, event, outcome, code), 1);
        assert_eq!(
            events(&step),
            [hook_call(name, event, want.0, want.1, None)],
            "{name} {outcome} {code:?}"
        );
    }
}

#[test]
fn a_hook_response_the_parser_cannot_read_is_other_and_names_the_line() {
    for bytes in [
        hook_response("PreToolUse:Bash", "PreToolUse", "a_new_outcome", Some(0)),
        line(
            json!({"type": "system", "subtype": "hook_response", "hook_event": "PreToolUse", "outcome": "success"}),
        ),
        line(
            json!({"type": "system", "subtype": "hook_response", "hook_name": "n", "outcome": "success"}),
        ),
    ] {
        let step = session().feed(&bytes, 1);
        assert_eq!(
            events(&step),
            [AgentEvent::Other {
                raw_type: "system/hook_response".into(),
                raw: None
            }]
        );
    }
}

#[test]
fn a_hook_exit_code_that_is_not_a_32_bit_integer_is_left_out() {
    let step = session().feed(
        &line(json!({"type": "system", "subtype": "hook_response", "hook_name": "n", "hook_event": "e",
                     "outcome": "error", "exit_code": 99_999_999_999u64})),
        1,
    );
    assert_eq!(
        events(&step),
        [hook_call("n", "e", HookOutcome::Error, None, None)]
    );
}

#[test]
fn a_hooks_stdout_is_read_for_a_decision_and_only_the_decision_is_kept() {
    for (stdout, want) in [
        (
            r#"{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":"r"}}"#,
            Some(HookDecision::Deny),
        ),
        (
            r#"{"hookSpecificOutput":{"permissionDecision":"ask"}}"#,
            Some(HookDecision::Ask),
        ),
        (
            r#"{"hookSpecificOutput":{"permissionDecision":"allow"}}"#,
            Some(HookDecision::Allow),
        ),
        (
            r#"{"decision":"block","reason":"r"}"#,
            Some(HookDecision::Block),
        ),
        (r#"{"decision":"approve"}"#, Some(HookDecision::Allow)),
        ("  {\"decision\":\"block\"}\n", Some(HookDecision::Block)),
        // Nothing to read: no output, output that is not JSON (S9: invalid JSON looks like a pass), a decision this
        // version does not know, a decision that is not a word.
        ("", None),
        ("{\"hookSpecificOutput\": not json", None),
        ("plain text the hook printed", None),
        (r#"{"decision":"maybe"}"#, None),
        (r#"{"decision":7}"#, None),
        (r#"{"continue":true}"#, None),
        (r#"["decision","block"]"#, None),
    ] {
        let step = session().feed(&hook_response_with("PostToolUse:Bash", stdout, "", 0), 1);
        let [AgentEvent::HookCall { decision, .. }] = &events(&step)[..] else {
            panic!("{stdout}: {:?}", step.events)
        };
        assert_eq!(*decision, want, "{stdout}");
    }
}

#[test]
fn neither_stdout_nor_stderr_nor_output_reaches_any_event_or_write() {
    // Every kind of line that carries hook text: a response with all three, a no-verdict line, a blocked prompt.
    let secret = "SECRET-HOOK-TEXT";
    let mut session = session();
    let mut seen = Vec::new();
    let mut written = Vec::new();
    for bytes in [
        tool_use("toolu_01", "Bash"),
        hook_started("PreToolUse:Bash", "PreToolUse"),
        line(
            json!({"type": "system", "subtype": "hook_response", "hook_name": "PreToolUse:Bash",
                    "hook_event": "PreToolUse", "output": secret, "stdout": format!("{{\"decision\":\"block\",\"reason\":\"{secret}\"}}"),
                    "stderr": secret, "exit_code": 1, "outcome": "error"}),
        ),
        no_verdict("PreToolUse", "toolu_01"),
        tool_result(
            "toolu_01",
            false,
            Some(decision("accept", "user_temporary")),
        ),
        result_line(json!({"result": "Done."})),
    ] {
        let step = session.feed(&bytes, 1);
        seen.extend(step.events);
        written.extend(step.writes);
    }
    let finished = session.finish(&ExitInfo {
        record: None,
        stderr_tail: String::new(),
    });
    seen.extend(finished.step.events);
    let seen: Vec<AgentEvent> = seen.into_iter().map(|stamped| stamped.event).collect();
    let text = serde_json::to_string(&seen).unwrap();
    assert!(text.contains("hook_call"), "the call itself is recorded");
    assert!(!text.contains(secret), "{text}");
    assert!(written.is_empty());
}

// ---- refused calls: S9's cases c0 to c7b ---------------------------------------------------------------------------

fn refusal_of(is_error: bool, meta: Option<Value>) -> Option<RefusalCause> {
    let mut session = session();
    session.feed(&tool_use("toolu_01", "Bash"), 1);
    let step = session.feed(&tool_result("toolu_01", is_error, meta), 2);
    match &events(&step)[..] {
        [] => None,
        [AgentEvent::ToolRefused { id, cause }] => {
            assert_eq!(id, "toolu_01");
            Some(*cause)
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn each_of_s9s_cases_c0_to_c7b_is_classified_from_the_metadata() {
    let hook = json!({"non_execution_kind": "permission-rule", "permission_decision":
        {"decision": "reject", "source": "hook", "reason_type": "hook"}});
    let host = json!({"non_execution_kind": "permission-rule", "permission_decision":
        {"decision": "reject", "source": "user_reject"}});
    let auto = json!({"non_execution_kind": "user-rejected", "permission_decision":
        {"decision": "reject", "source": "config"}});
    let rule = json!({"non_execution_kind": "permission-rule", "permission_decision":
        {"decision": "reject", "source": "config", "reason_type": "subcommandResults"}});
    let ran = decision("accept", "user_temporary");
    for (case, is_error, meta, want) in [
        ("c0 allowed control", false, Some(ran.clone()), None),
        (
            "c1 hook deny, JSON",
            true,
            Some(hook.clone()),
            Some(RefusalCause::Hook),
        ),
        (
            "c2 hook deny, exit 2",
            true,
            Some(hook.clone()),
            Some(RefusalCause::Hook),
        ),
        ("c3a hook ask, host allows", false, Some(ran.clone()), None),
        (
            "c3b hook ask, host denies",
            true,
            Some(host.clone()),
            Some(RefusalCause::Host),
        ),
        ("c4a hook exit 1", false, Some(ran.clone()), None),
        ("c4b hook invalid JSON", false, Some(ran.clone()), None),
        ("c4c hook timeout", false, Some(ran.clone()), None),
        ("c5 PostToolUse block", false, Some(ran.clone()), None),
        (
            "c6a host deny",
            true,
            Some(host.clone()),
            Some(RefusalCause::Host),
        ),
        (
            "c6b auto deny, no prompt tool",
            true,
            Some(auto.clone()),
            Some(RefusalCause::Rule),
        ),
        (
            "c7a disallowed tool",
            true,
            None,
            Some(RefusalCause::MissingTool),
        ),
        (
            "c7b disallowed rule",
            true,
            Some(rule.clone()),
            Some(RefusalCause::Rule),
        ),
    ] {
        assert_eq!(refusal_of(is_error, meta), want, "{case}");
    }
}

#[test]
fn a_shape_that_is_none_of_those_is_an_unknown_refusal() {
    for (case, meta) in [
        (
            "a source of a later claude",
            json!({"non_execution_kind": "permission-rule", "permission_decision": {"decision": "reject", "source": "plugin"}}),
        ),
        (
            "a rejection with no source",
            json!({"permission_decision": {"decision": "reject"}}),
        ),
        (
            "a call that did not run, with no decision",
            json!({"non_execution_kind": "something-new"}),
        ),
        (
            "a source that is not a word",
            json!({"non_execution_kind": "permission-rule", "permission_decision": {"decision": "reject", "source": 4}}),
        ),
    ] {
        assert_eq!(
            refusal_of(true, Some(meta)),
            Some(RefusalCause::Unknown),
            "{case}"
        );
    }
}

#[test]
fn a_call_that_ran_is_not_refused_whether_or_not_its_result_was_an_error() {
    // A command that fails is a tool failure, not a refusal: the call ran.
    assert_eq!(
        refusal_of(true, Some(decision("accept", "user_temporary"))),
        None
    );
    assert_eq!(
        refusal_of(false, Some(json!({"non_execution_kind": null}))),
        None
    );
    // And a result with no metadata that is no error is a call that ran.
    assert_eq!(refusal_of(false, None), None);
}

#[test]
fn the_cause_comes_from_the_metadata_never_from_the_results_text() {
    // S9: a deliberate hook deny reads `PreToolUse:Bash hook error: ...`, a trap for text matching.
    let mut session = session();
    session.feed(&tool_use("toolu_01", "Bash"), 1);
    let bytes = line(
        json!({"type": "user", "message": {"role": "user", "content": [
        {"type": "tool_result", "content": "PreToolUse:Bash hook error: denied by the permission settings", "is_error": true, "tool_use_id": "toolu_01"}]},
        "tool_result_meta": [{"id": "toolu_01", "non_execution_kind": "permission-rule",
            "permission_decision": {"decision": "reject", "source": "user_reject"}}]}),
    );
    assert_eq!(
        events(&session.feed(&bytes, 2)),
        [refused("toolu_01", RefusalCause::Host)]
    );
}

#[test]
fn several_results_in_one_line_are_joined_to_their_metadata_on_the_call_id() {
    let mut session = session();
    session.feed(&tool_use("toolu_01", "Bash"), 1);
    session.feed(&tool_use("toolu_02", "Bash"), 1);
    let bytes = line(
        json!({"type": "user", "message": {"role": "user", "content": [
        {"type": "tool_result", "content": "a", "is_error": true, "tool_use_id": "toolu_01"},
        {"type": "tool_result", "content": "b", "is_error": true, "tool_use_id": "toolu_02"}]},
        // The metadata is in the other order, and one call has none.
        "tool_result_meta": [{"id": "toolu_01", "non_execution_kind": "permission-rule",
            "permission_decision": {"decision": "reject", "source": "hook"}}]}),
    );
    assert_eq!(
        events(&session.feed(&bytes, 2)),
        [
            refused("toolu_01", RefusalCause::Hook),
            refused("toolu_02", RefusalCause::MissingTool)
        ]
    );
}

#[test]
fn a_user_line_with_no_tool_result_is_other_as_before() {
    let step = session().feed(
        &line(json!({"type": "user", "message": {"role": "user", "content": [{"type": "text", "text": "hi"}]}})),
        1,
    );
    assert_eq!(
        events(&step),
        [AgentEvent::Other {
            raw_type: "user".into(),
            raw: None
        }]
    );
}

// ---- the failed-open observation -----------------------------------------------------------------------------------

fn ran() -> Vec<u8> {
    tool_result(
        "toolu_01",
        false,
        Some(decision("accept", "user_temporary")),
    )
}

#[test]
fn a_hook_that_exited_1_and_whose_tool_ran_has_failed_open() {
    let mut session = session();
    let all = feed_all(
        &mut session,
        &[
            tool_use("toolu_01", "Bash"),
            hook_response("PreToolUse:Bash", "PreToolUse", "error", Some(1)),
            ran(),
        ],
    );
    assert_eq!(
        all,
        [
            hook_call(
                "PreToolUse:Bash",
                "PreToolUse",
                HookOutcome::Error,
                Some(1),
                None
            ),
            failed_open(
                "PreToolUse:Bash",
                "PreToolUse",
                "toolu_01",
                HookFailure::ExitError
            ),
        ]
    );
}

#[test]
fn a_timed_out_hook_whose_tool_ran_has_failed_open_with_no_verdict_and_is_one_hook_call() {
    let mut session = session();
    let all = feed_all(
        &mut session,
        &[
            tool_use("toolu_01", "Bash"),
            hook_response("PreToolUse:Bash", "PreToolUse", "cancelled", Some(1)),
            no_verdict("PreToolUse", "toolu_01"),
            ran(),
        ],
    );
    assert_eq!(
        all,
        [
            hook_call(
                "PreToolUse:Bash",
                "PreToolUse",
                HookOutcome::Cancelled,
                Some(1),
                None
            ),
            failed_open(
                "PreToolUse:Bash",
                "PreToolUse",
                "toolu_01",
                HookFailure::NoVerdict
            ),
        ]
    );
}

#[test]
fn a_no_verdict_with_no_hook_response_before_it_is_the_hook_call_and_the_failure_both() {
    // Without `--include-hook-events` the informational line is the only sign (S9). It names the event and the call,
    // and not the hook.
    let mut session = session();
    let all = feed_all(
        &mut session,
        &[
            tool_use("toolu_01", "Bash"),
            no_verdict("PreToolUse", "toolu_01"),
            ran(),
        ],
    );
    assert_eq!(
        all,
        [
            hook_call(
                "PreToolUse",
                "PreToolUse",
                HookOutcome::Cancelled,
                None,
                None
            ),
            failed_open(
                "PreToolUse",
                "PreToolUse",
                "toolu_01",
                HookFailure::NoVerdict
            ),
        ]
    );
}

#[test]
fn a_hook_that_exits_2_is_a_block_and_has_not_failed_open_before_or_after_the_tool() {
    for (name, event) in [
        ("PreToolUse:Bash", "PreToolUse"),
        ("PostToolUse:Bash", "PostToolUse"),
    ] {
        let mut session = session();
        let all = feed_all(
            &mut session,
            &[
                tool_use("toolu_01", "Bash"),
                hook_response(name, event, "error", Some(2)),
                ran(),
            ],
        );
        assert!(
            !all.iter()
                .any(|e| matches!(e, AgentEvent::HookFailedOpen { .. })),
            "{name}"
        );
    }
}

#[test]
fn a_hook_that_passed_has_not_failed_open() {
    let mut session = session();
    let all = feed_all(
        &mut session,
        &[
            tool_use("toolu_01", "Bash"),
            hook_response("PreToolUse:Bash", "PreToolUse", "success", Some(0)),
            ran(),
        ],
    );
    assert_eq!(all.len(), 1);
}

#[test]
fn a_failed_post_tool_hook_has_failed_open_too_and_says_which_event_it_hooked() {
    let mut session = session();
    let all = feed_all(
        &mut session,
        &[
            tool_use("toolu_01", "Bash"),
            hook_response("PostToolUse:Bash", "PostToolUse", "error", Some(1)),
            ran(),
        ],
    );
    assert_eq!(
        all.last(),
        Some(&failed_open(
            "PostToolUse:Bash",
            "PostToolUse",
            "toolu_01",
            HookFailure::ExitError
        ))
    );
}

#[test]
fn a_failed_hook_whose_tool_did_not_run_has_not_failed_open() {
    // The host denied the call after the hook failed: nothing ran, so nothing was left unguarded.
    let host = json!({"non_execution_kind": "permission-rule", "permission_decision": {"decision": "reject", "source": "user_reject"}});
    let mut session = session();
    let all = feed_all(
        &mut session,
        &[
            tool_use("toolu_01", "Bash"),
            hook_response("PreToolUse:Bash", "PreToolUse", "error", Some(1)),
            tool_result("toolu_01", true, Some(host)),
        ],
    );
    assert_eq!(
        all,
        [
            hook_call(
                "PreToolUse:Bash",
                "PreToolUse",
                HookOutcome::Error,
                Some(1),
                None
            ),
            refused("toolu_01", RefusalCause::Host),
        ]
    );
}

#[test]
fn a_failed_hook_is_joined_to_the_call_of_its_own_tool() {
    let mut session = session();
    let all = feed_all(
        &mut session,
        &[
            tool_use("toolu_01", "Read"),
            tool_use("toolu_02", "Bash"),
            hook_response("PreToolUse:Bash", "PreToolUse", "error", Some(1)),
            tool_result(
                "toolu_01",
                false,
                Some(decision("accept", "user_temporary")),
            ),
            tool_result(
                "toolu_02",
                false,
                Some(decision("accept", "user_temporary")),
            ),
        ],
    );
    let failures: Vec<&AgentEvent> = all
        .iter()
        .filter(|e| matches!(e, AgentEvent::HookFailedOpen { .. }))
        .collect();
    assert_eq!(
        failures,
        [&failed_open(
            "PreToolUse:Bash",
            "PreToolUse",
            "toolu_02",
            HookFailure::ExitError
        )]
    );
}

#[test]
fn a_failed_hook_that_is_not_a_tool_hook_never_fails_open() {
    let mut session = session();
    let all = feed_all(
        &mut session,
        &[
            hook_response("SessionStart:startup", "SessionStart", "error", Some(1)),
            tool_use("toolu_01", "Bash"),
            ran(),
        ],
    );
    assert_eq!(all.len(), 1, "{all:?}");
}

#[test]
fn a_failed_hook_waits_for_no_tool_once_the_turn_has_ended() {
    let mut session = session();
    let all = feed_all(
        &mut session,
        &[
            tool_use("toolu_01", "Bash"),
            hook_response("PreToolUse:Bash", "PreToolUse", "error", Some(1)),
            result_line(json!({"result": "stopped"})),
            // The next turn calls the same tool, with a hook that passes.
            tool_use("toolu_02", "Bash"),
            hook_response("PreToolUse:Bash", "PreToolUse", "success", Some(0)),
            tool_result(
                "toolu_02",
                false,
                Some(decision("accept", "user_temporary")),
            ),
        ],
    );
    assert!(
        !all.iter()
            .any(|e| matches!(e, AgentEvent::HookFailedOpen { .. })),
        "{all:?}"
    );
}

// ---- S9's recorded cases, through fake Claude under the real shim ---------------------------------------------------

#[test]
fn the_hook_deny_recording_is_an_exit_2_hook_call_and_a_hook_refusal_and_no_failure() {
    let played = play("hook-deny", assigned(), &["touch the file"], allow_all);
    assert_eq!(
        hook_events(&played),
        [
            hook_call(
                "PreToolUse:Bash",
                "PreToolUse",
                HookOutcome::Error,
                Some(2),
                None
            ),
            refused("toolu_01", RefusalCause::Hook),
        ]
    );
}

#[test]
fn the_json_deny_recording_reads_the_decision_and_the_refusal_and_keeps_no_output() {
    let played = play("hook-deny-json", assigned(), &["touch the file"], allow_all);
    assert_eq!(
        hook_events(&played),
        [
            hook_call(
                "PreToolUse:Bash",
                "PreToolUse",
                HookOutcome::Success,
                Some(0),
                Some(HookDecision::Deny)
            ),
            refused("toolu_01", RefusalCause::Hook),
        ]
    );
    assert_no_hook_text_reaches_an_event_or_a_write(&played);
}

#[test]
fn the_hook_failure_recording_has_failed_open_and_the_call_was_asked_and_ran() {
    let played = play("hook-failure", assigned(), &["touch the file"], allow_all);
    assert_eq!(
        hook_events(&played),
        [
            hook_call(
                "PreToolUse:Bash",
                "PreToolUse",
                HookOutcome::Error,
                Some(1),
                None
            ),
            failed_open(
                "PreToolUse:Bash",
                "PreToolUse",
                "toolu_01",
                HookFailure::ExitError
            ),
        ]
    );
}

#[test]
fn the_hook_no_verdict_recording_has_one_hook_call_and_has_failed_open() {
    let played = play(
        "hook-no-verdict",
        assigned(),
        &["touch the file"],
        allow_all,
    );
    assert_eq!(
        hook_events(&played),
        [
            hook_call(
                "PreToolUse:Bash",
                "PreToolUse",
                HookOutcome::Cancelled,
                Some(1),
                None
            ),
            failed_open(
                "PreToolUse:Bash",
                "PreToolUse",
                "toolu_01",
                HookFailure::NoVerdict
            ),
        ]
    );
}

#[test]
fn the_hook_block_recording_is_an_exit_2_post_tool_hook_on_a_call_that_ran_and_no_failure() {
    let played = play("hook-block", assigned(), &["touch the file"], allow_all);
    assert_eq!(
        hook_events(&played),
        [hook_call(
            "PostToolUse:Bash",
            "PostToolUse",
            HookOutcome::Error,
            Some(2),
            None
        )]
    );
}

#[test]
fn the_host_deny_recording_is_a_host_refusal() {
    let played = play(
        "host-deny",
        assigned(),
        &["touch the file"],
        |session, event| match event {
            AgentEvent::PermissionRequest { request } => session.answer(
                &request.id,
                PermissionReply::Deny {
                    message: Some("Not in this phase.".into()),
                },
            ),
            _ => Step::default(),
        },
    );
    assert_eq!(
        hook_events(&played),
        [refused("toolu_01", RefusalCause::Host)]
    );
}

#[test]
fn the_disallowed_tool_recording_is_a_missing_tool_refusal() {
    let played = play(
        "disallowed-tool",
        assigned(),
        &["touch the file"],
        allow_all,
    );
    assert_eq!(
        hook_events(&played),
        [refused("toolu_01", RefusalCause::MissingTool)]
    );
}

#[test]
fn a_recording_with_no_hook_and_no_refusal_has_neither_event() {
    let played = play(
        "permission-request",
        assigned(),
        &["touch the file"],
        allow_all,
    );
    assert_eq!(hook_events(&played), []);
    let played = play("report", assigned(), &["go"], answer_nothing);
    assert_eq!(hook_events(&played), []);
}

// ---- no hook stdout or stderr reaches the store ----------------------------------------------------------------------

/// Every event the session gave back, and every message it wrote (which the journal keeps as `in` records), holds none
/// of the hook text the recordings carry. Until `ts-0065-kx` stores events, this is the whole of "the store"; it runs
/// again over the daemon's rows once they exist.
fn assert_no_hook_text_reaches_an_event_or_a_write(played: &Played) {
    let events = serde_json::to_string(&played.agent_events()).unwrap();
    let writes: Vec<String> = read_journal(&played.journal)
        .records
        .iter()
        .filter(|record| record.direction == Direction::In)
        .map(|record| String::from_utf8_lossy(&record.bytes).into_owned())
        .collect();
    for marker in ["SECRET-STDOUT-MARKER", "SECRET-STDERR-MARKER"] {
        assert!(!events.contains(marker), "{marker} in {events}");
        assert!(
            writes.iter().all(|write| !write.contains(marker)),
            "{marker} in a write"
        );
    }
}

#[test]
fn over_every_hook_recording_no_hook_text_reaches_an_event_or_a_write() {
    for name in [
        "hook-deny",
        "hook-deny-json",
        "hook-failure",
        "hook-block",
        "hook-no-verdict",
    ] {
        let played = play(name, assigned(), &["touch the file"], allow_all);
        assert_no_hook_text_reaches_an_event_or_a_write(&played);
        // The plain text the recordings' hooks print on stderr is not in an event either.
        let events = serde_json::to_string(&played.agent_events()).unwrap();
        for text in [
            "Policy: no writes.",
            "the hook crashed",
            "Policy: review the output first.",
        ] {
            assert!(!events.contains(text), "{name}: {text}");
        }
    }
}
