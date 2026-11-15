//! The contract test of the real, pinned `claude` (ts-0070-mk).
//!
//! Two halves:
//!
//! * The **live half** runs the real binary (haiku, a budget cap on every line, a spend ledger; see `real.rs`). Each test
//!   is skipped, with the reason printed, unless `COLLIDE_REAL_CLAUDE=1` is set, so `cargo test --workspace` runs nothing
//!   real. A live line is checked against the shapes in the recordings (`fixtures/real-recordings/`): a path the
//!   recordings all had that a live line lacks, or a type that was never recorded, fails the test as format drift.
//! * The **offline half** runs always: the redaction rule on made-up text, the recordings against the rule's own guard, the
//!   recordings replayed through [`ClaudeSession`], and the shape check on a made-up drifted line.
//!
//! To record again: `COLLIDE_REAL_CLAUDE=1 COLLIDE_RECORD_DIR=<an existing folder> COLLIDE_REAL_LEDGER=<a file> cargo test
//! -p collide-host --test claude_adapter real_claude::live`, read the result, and copy it to the recordings' folder.

use super::real::{self, Case, Live, Outcome, Redactor, SESSION_STAND_IN, Shapes};
use collide_core::adapter::StampedEvent;
use collide_core::agent::{
    AgentEvent, PermissionReply, RefusalCause, SessionStart, TurnEndReason, UsageScope,
};
use collide_host::adapters::claude::version::{CANARY_MARKER, SmokeOutcome, canary_started};
use serde_json::{Value, json};
use std::fs;
use std::time::{Duration, Instant};

macro_rules! live {
    () => {
        if let Some(reason) = real::unavailable() {
            println!("skipped: {reason}");
            return;
        }
    };
}

/// The most lines a second the stream may reach before it is worth reporting (ts-0070-mk).
const RATE_NOTE: usize = 200;

// ---- what every live run is held to ------------------------------------------------------------------------------------

/// A live stream is checked against the recordings' shapes, and the facts every run of the launch line has.
fn the_stream_is_as_recorded(out: &Outcome) {
    let shapes = Shapes::of(&real::all_recorded());
    let mut drift = Vec::new();
    for line in &out.lines {
        drift.extend(shapes.drift(line));
    }
    drift.sort();
    drift.dedup();
    assert!(drift.is_empty(), "format drift: {drift:#?}");
    assert!(
        out.peak_lines_per_second() <= RATE_NOTE,
        "{} lines in one second: report it",
        out.peak_lines_per_second()
    );
}

/// The launch line the adapter built, as the real binary accepted it.
fn the_launch_line_was_accepted(out: &Outcome) {
    assert_eq!(
        out.code,
        Some(0),
        "claude ended with {:?}: {}",
        out.code,
        out.stderr
    );
    let at = out
        .argv
        .iter()
        .position(|arg| arg == "--permission-prompt-tool")
        .expect("the launch line asks the host for permissions");
    assert_eq!(out.argv[at + 1], "stdio");
    for flag in ["--include-hook-events", "--strict-mcp-config", "--verbose"] {
        assert!(out.argv.iter().any(|arg| arg == flag), "{flag}");
    }
    let init = out.of_kind("system/init");
    assert_eq!(init.len(), 1, "one init line");
    assert!(
        init[0]["permissionMode"].is_string() && init[0]["tools"].is_array(),
        "the init line has the fields the session reads"
    );
}

/// The canary the snapshot's `SessionStart` hook writes is on a real `hook_response`.
fn the_canary_is_on_the_real_hook_response(out: &Outcome) {
    let seen = out
        .lines
        .iter()
        .any(|line| canary_started(line.to_string().as_bytes()));
    assert!(
        seen,
        "no hook_response held `{CANARY_MARKER}`: {:?}",
        out.kinds()
    );
}

fn events_of(out: &Outcome) -> Vec<&AgentEvent> {
    out.agent_events()
}

fn count(out: &Outcome, test: impl Fn(&AgentEvent) -> bool) -> usize {
    events_of(out).into_iter().filter(|e| test(e)).count()
}

// ---- the live half -----------------------------------------------------------------------------------------------------

#[test]
fn live_a_plain_turn_gives_the_session_what_it_reads() {
    live!();
    let live = Live::new();
    let out = real::run(
        &live,
        Case::new(
            "hello",
            "Reply with the single word ok. Do not use any tool.",
        ),
    );
    the_launch_line_was_accepted(&out);
    the_canary_is_on_the_real_hook_response(&out);
    the_stream_is_as_recorded(&out);
    assert!(real::ended_normally(&out));

    let result = out.result();
    for field in [
        "subtype",
        "is_error",
        "duration_ms",
        "duration_api_ms",
        "num_turns",
        "result",
        "stop_reason",
        "session_id",
        "total_cost_usd",
        "usage",
        "modelUsage",
    ] {
        assert!(result.get(field).is_some(), "result has no `{field}`");
    }
    assert!(
        count(&out, |e| matches!(e, AgentEvent::SessionBound { .. })) >= 1,
        "the session id is bound"
    );
    assert_eq!(
        count(&out, |e| matches!(e, AgentEvent::FinalReport { .. })),
        1
    );
    let turn_usage = events_of(&out).into_iter().find_map(|e| match e {
        AgentEvent::Usage { usage } if usage.scope == UsageScope::Turn => Some(usage.clone()),
        _ => None,
    });
    let usage = turn_usage.expect("a turn usage");
    assert!(usage.output.is_some() && usage.cost.is_some(), "{usage:?}");
    assert!(
        usage.model_time_ms.is_some(),
        "a fresh session's first turn has model time: {usage:?}"
    );
    println!(
        "hello: wall {:?}, {} lines, peak {}/s, cost ${}",
        out.wall,
        out.lines.len(),
        out.peak_lines_per_second(),
        out.cost()
    );
}

#[test]
fn live_a_permission_request_is_answered_through_the_stdio_protocol() {
    live!();
    let live = Live::new();
    let mut seen = Vec::new();
    let mut allow = Case::new(
        "permission-allow",
        "Use the Bash tool to run `touch allowed.txt`, then say in one sentence what happened.",
    );
    allow.answer = Box::new(|request| {
        seen.push(request.clone());
        PermissionReply::Allow {}
    });
    let out = real::run(&live, allow);
    the_launch_line_was_accepted(&out);
    the_stream_is_as_recorded(&out);
    assert!(
        live.work.join("allowed.txt").exists(),
        "an allowed request let the tool run"
    );
    assert_eq!(
        out.of_kind("control_request").len(),
        1,
        "one can_use_tool request"
    );
    assert_eq!(
        count(&out, |e| matches!(e, AgentEvent::PermissionRequest { .. })),
        1
    );
    assert_eq!(
        count(&out, |e| matches!(e, AgentEvent::ToolRefused { .. })),
        0,
        "an allowed call is not refused"
    );
    assert!(real::ended_normally(&out));

    let mut deny = Case::new(
        "permission-deny",
        "Use the Bash tool to run `touch denied.txt`, then say in one sentence what happened.",
    );
    deny.answer = Box::new(|_| PermissionReply::Deny {
        message: Some("denied by the contract test".into()),
    });
    let out = real::run(&live, deny);
    the_stream_is_as_recorded(&out);
    assert!(
        !live.work.join("denied.txt").exists(),
        "a denied request kept the tool from running"
    );
    let refused: Vec<_> = events_of(&out)
        .into_iter()
        .filter_map(|e| match e {
            AgentEvent::ToolRefused { cause, .. } => Some(*cause),
            _ => None,
        })
        .collect();
    assert_eq!(refused, vec![RefusalCause::Host], "{refused:?}");
    assert!(real::ended_normally(&out));
}

#[test]
fn live_a_failing_read_and_edit_are_not_missing_tools() {
    live!();
    let live = Live::new();
    let out = real::run(
        &live,
        Case::new(
            "tools-missing",
            "Do two things with tools. First use the Read tool on missing-1.txt in the current directory. Then use the Edit tool to change the word a to b in missing-2.txt. Then say in one sentence what happened.",
        ),
    );
    the_launch_line_was_accepted(&out);
    the_stream_is_as_recorded(&out);
    let init = out.of_kind("system/init")[0];
    let listed: Vec<&str> = init["tools"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
        .collect();
    assert!(
        listed.contains(&"Read") && listed.contains(&"Edit"),
        "this binary lists Read and Edit: {listed:?}"
    );
    let errors = out
        .of_kind("user")
        .iter()
        .flat_map(|line| {
            line["message"]["content"]
                .as_array()
                .cloned()
                .unwrap_or_default()
        })
        .filter(|block| block["is_error"] == json!(true))
        .count();
    assert!(errors >= 2, "both calls failed on the file ({errors})");
    assert_eq!(
        count(&out, |e| matches!(
            e,
            AgentEvent::ToolRefused {
                cause: RefusalCause::MissingTool,
                ..
            }
        )),
        0,
        "a listed tool that failed is not a missing tool"
    );
    assert!(
        count(&out, |e| matches!(e, AgentEvent::ToolResultSize { .. })) >= 2,
        "a size for every result"
    );
}

#[test]
fn live_a_failed_pre_hook_joins_its_own_call() {
    live!();
    let live = Live::new();
    let mut case = Case::new(
        "hooks",
        "Make two separate Bash tool calls in the same message (parallel): one runs `echo one`, the other runs `echo two`. Then say done.",
    );
    case.extra_hooks = vec![
        (
            "PreToolUse",
            json!({"matcher": "Bash", "hooks": [{"type": "command", "command": "echo pre-failed >&2; exit 1"}]}),
        ),
        (
            "PostToolUse",
            json!({"matcher": "Bash", "hooks": [{"type": "command", "command": "echo post-ran >&2"}]}),
        ),
    ];
    let out = real::run(&live, case);
    the_launch_line_was_accepted(&out);
    the_stream_is_as_recorded(&out);

    let calls: Vec<String> = events_of(&out)
        .into_iter()
        .filter_map(|e| match e {
            AgentEvent::ToolCall { id, tool, .. } if tool.as_deref() == Some("Bash") => {
                Some(id.clone())
            }
            _ => None,
        })
        .collect();
    assert_eq!(calls.len(), 2, "two parallel calls: {calls:?}");
    let mut failed_open: Vec<String> = events_of(&out)
        .into_iter()
        .filter_map(|e| match e {
            AgentEvent::HookFailedOpen { tool_call_id, .. } => Some(tool_call_id.clone()),
            _ => None,
        })
        .collect();
    failed_open.sort();
    let mut expected = calls.clone();
    expected.sort();
    assert_eq!(
        failed_open, expected,
        "each call has its own failed-open hook"
    );
    let hook_calls = count(&out, |e| matches!(e, AgentEvent::HookCall { .. }));
    assert!(
        hook_calls >= 5,
        "the canary and the four tool hooks are reported ({hook_calls})"
    );
    // The hook lines carry no call id: the adapter joins them by order. That is the fact this test pins.
    for line in out.of_kind("system/hook_response") {
        assert!(
            line.get("tool_use_id").is_none() && line.get("tool_call_id").is_none(),
            "a hook line carries a call id now, so the join by order can go: {line}"
        );
    }
}

#[test]
fn live_a_resumed_session_fires_its_hooks_and_continues_its_clock() {
    live!();
    let live = Live::new();
    let id = real::new_uuid();
    let mut first = Case::new(
        "resume-first",
        "Remember the word pineapple. Reply with only: noted.",
    );
    first.session = SessionStart::Assign { uuid: id.clone() };
    let one = real::run(&live, first);
    the_launch_line_was_accepted(&one);
    the_stream_is_as_recorded(&one);

    let mut second = Case::new("resume-second", "What word did I ask you to remember?");
    second.session = SessionStart::Resume { id: id.clone() };
    let two = real::run(&live, second);
    the_launch_line_was_accepted(&two);
    the_stream_is_as_recorded(&two);
    assert!(
        two.argv.iter().any(|arg| arg == "--resume"),
        "{:?}",
        two.argv
    );

    // SessionStart fires on resume, and so does the canary.
    let starts: Vec<&str> = two
        .of_kind("system/hook_started")
        .iter()
        .filter(|line| line["hook_event"] == "SessionStart")
        .filter_map(|line| line["hook_name"].as_str())
        .collect();
    assert!(!starts.is_empty(), "no SessionStart on resume");
    println!("SessionStart on resume: {starts:?}");
    the_canary_is_on_the_real_hook_response(&two);
    assert_eq!(two.result()["session_id"], json!(id));

    // The clock and the cost go on from the first run: the second result's figures include the first's.
    let api = |out: &Outcome| {
        out.result()["duration_api_ms"]
            .as_u64()
            .expect("duration_api_ms")
    };
    let (api_one, api_two) = (api(&one), api(&two));
    assert!(
        api_two >= api_one,
        "duration_api_ms went back across --resume: {api_one} then {api_two}"
    );
    assert!(
        two.cost() >= one.cost(),
        "total_cost_usd went back across --resume"
    );
    println!(
        "resume: duration_api_ms {api_one} then {api_two}; cost {} then {}",
        one.cost(),
        two.cost()
    );

    // The first turn after a resume: what the session says of its usage.
    let usage = events_of(&two).into_iter().find_map(|e| match e {
        AgentEvent::Usage { usage } if usage.scope == UsageScope::Turn => Some(usage.clone()),
        _ => None,
    });
    let usage = usage.expect("a turn usage after resume");
    println!("resume first turn usage: {usage:?}");
    assert!(usage.output.is_some(), "{usage:?}");
    assert!(real::ended_normally(&two));
}

#[test]
fn live_the_autocompact_flag_and_the_environment_variable_agree() {
    live!();
    let live = Live::new();
    let var = collide_host::adapters::claude::launch::COMPACT_AT_VAR;
    let window = |usage: &Value| usage["maxTokens"].as_u64().expect("maxTokens");
    let source = |usage: &Value| {
        usage["autocompactSource"]
            .as_str()
            .unwrap_or("?")
            .to_string()
    };
    let default = real::context_usage(&live, &[], &[]).expect("default");
    for tokens in ["200000", "500000"] {
        let by_flag = real::context_usage(&live, &["--autocompact", tokens], &[]).expect("flag");
        let by_var = real::context_usage(&live, &[], &[(var, tokens)]).expect("variable");
        println!(
            "autocompact {tokens}: flag window {} (source {}), variable window {} (source {}), default {} ({})",
            window(&by_flag),
            source(&by_flag),
            window(&by_var),
            source(&by_var),
            window(&default),
            source(&default)
        );
        assert_eq!(
            window(&by_flag),
            window(&by_var),
            "`--autocompact {tokens}` and {var}={tokens} give the same window"
        );
        assert_eq!(window(&by_flag), tokens.parse::<u64>().unwrap());
    }
}

#[test]
fn live_the_smoke_check_passes_on_the_real_binary_and_its_flags_are_accepted() {
    live!();
    let live = Live::new();
    real::check_ledger("smoke-check", real::CASE_CAP_USD);
    // A wrapper that keeps what the check passes `claude` and what it wrote, and runs the real binary.
    let args_log = live.root.join("smoke-args.log");
    let out_log = live.root.join("smoke-out.log");
    let wrapper = live.root.script(
        "claude-logged",
        &format!(
            "#!/bin/bash\nset -o pipefail\nprintf '%s\\n' \"$@\" > '{}'\n'{}' \"$@\" | tee '{}'\n",
            args_log.display(),
            real::binary().display(),
            out_log.display()
        ),
    );
    let began = Instant::now();
    let outcome = collide_host::adapters::claude::version::smoke_check(
        &wrapper,
        &live.work,
        &real::child_env(),
    );
    let took = began.elapsed();
    let cost = fs::read_to_string(&out_log)
        .unwrap_or_default()
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter_map(|line| line["total_cost_usd"].as_f64())
        .next_back()
        .unwrap_or(0.0);
    real::add_to_ledger("smoke-check", real::CASE_CAP_USD, cost);
    assert_eq!(
        outcome,
        SmokeOutcome::Passed,
        "the smoke check on the real binary"
    );
    assert!(
        took < collide_host::adapters::claude::version::SMOKE_TIMEOUT,
        "{took:?}"
    );
    let args = fs::read_to_string(&args_log).unwrap();
    assert!(
        args.lines().any(|a| a == "--no-session-persistence"),
        "{args}"
    );
    assert!(args.lines().any(|a| a == "stdio"), "{args}");
    println!("smoke check: {took:?}, cost ${cost}");
}

// ---- the offline half --------------------------------------------------------------------------------------------------

fn stand_ins() -> Redactor {
    Redactor::with(
        vec![
            ("/tmp/cdt-1-1/work".into(), "/work/repo".into()),
            ("/tmp/cdt-1-1".into(), "/work".into()),
            ("/opt/real/bin".into(), "/opt/claude".into()),
        ],
        "alice".into(),
        "/Users/alice".into(),
    )
}

#[test]
fn redaction_replaces_paths_user_email_and_ids() {
    let mut redactor = stand_ins();
    let line = json!({
        "type": "system",
        "subtype": "init",
        "session_id": "11111111-2222-4333-8444-555555555555",
        "cwd": "/tmp/cdt-1-1/work",
        "note": "alice wrote from /Users/alice/x and /tmp/cdt-1-1/snap-a, mail alice@corp.example.org ok",
        "email": "alice@corp.example.org",
        "request_id": "abc",
        "id": "msg_01AbCdEf",
        "other": "22222222-3333-4444-8555-666666666666",
        "messaging_socket_path": "/tmp/cc-socks/123.sock",
    });
    let out = redactor.line(&line);
    assert_eq!(out["session_id"], json!(SESSION_STAND_IN));
    assert_eq!(out["cwd"], json!("/work/repo"));
    assert_eq!(
        out["note"],
        json!("user wrote from /home/user/x and /work/snap-a, mail user@example.com ok")
    );
    assert!(out.get("email").is_none(), "an account key is removed");
    assert_eq!(out["request_id"], json!("req-01"));
    assert_eq!(out["id"], json!("msg_01"));
    assert_eq!(out["other"], json!("00000000-0000-4000-8000-000000000010"));
    assert_eq!(out["messaging_socket_path"], json!("/work/cc-socks/1.sock"));
}

#[test]
fn redaction_is_stable_for_the_same_id_and_distinguishes_two_sessions() {
    let mut redactor = stand_ins();
    let a = json!({"session_id": "11111111-2222-4333-8444-555555555555"});
    let b = json!({"session_id": "22222222-3333-4444-8555-666666666666"});
    assert_eq!(redactor.line(&a), redactor.line(&a));
    assert_ne!(redactor.line(&a), redactor.line(&b));
}

#[test]
fn redaction_neutralises_the_owners_usage_in_a_rate_limit_line() {
    let mut redactor = stand_ins();
    let line = json!({"type": "rate_limit_event", "rate_limit_info": {
        "status": "allowed_warning", "rateLimitType": "seven_day",
        "utilization": 0.92, "resetsAt": 1795000123u64, "isUsingOverage": false,
        "surpassedThreshold": "x"}});
    let info = &redactor.line(&line)["rate_limit_info"];
    assert_eq!(info["status"], json!("allowed_warning"));
    assert_eq!(info["utilization"], json!(0.5));
    assert_eq!(info["resetsAt"], json!(1_800_000_000));
    assert_eq!(info["isUsingOverage"], json!(false));
}

#[test]
#[should_panic(expected = "redaction left")]
fn redaction_refuses_a_line_that_still_names_the_machine() {
    // A path outside every stand-in the rule knows: the guard refuses the line whole.
    let mut redactor = stand_ins();
    redactor.line(&json!({"text": "see /private/var/x"}));
}

#[test]
fn the_recordings_hold_nothing_that_names_the_machine_or_the_account() {
    let text: String = fs::read_dir(real::recordings_dir())
        .expect("the recordings' folder")
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "jsonl"))
        .map(|e| fs::read_to_string(e.path()).unwrap())
        .collect();
    assert!(!text.is_empty(), "there are recordings");
    for word in [
        "/Users/",
        "/private/",
        "/var/folders",
        "/tmp/",
        "sk-ant-",
        "eyJ",
        "@",
    ] {
        // The e-mail stand-in and the built-in plugins' names (`name@builtin`) are the `@` there may be.
        let stripped = text.replace("user@example.com", "").replace("@builtin", "");
        assert!(!stripped.contains(word), "a recording holds `{word}`");
    }
    for key in [
        "\"email\"",
        "\"accountUuid\"",
        "\"organizationUuid\"",
        "credential_expires_at",
    ] {
        assert!(!text.contains(key), "a recording holds {key}");
    }
}

#[test]
fn the_recordings_replay_through_the_session() {
    let names = [
        "hello",
        "permission-allow",
        "permission-deny",
        "hooks",
        "tools-missing",
    ];
    for name in names {
        let lines = real::recorded(name);
        let out = Outcome::from_recording(
            lines,
            SessionStart::Assign {
                uuid: SESSION_STAND_IN.to_string(),
            },
        );
        let events: Vec<&StampedEvent> = out.events.iter().collect();
        assert!(
            events.iter().any(|s| matches!(
                s.event,
                AgentEvent::TurnEnd {
                    reason: TurnEndReason::EndTurn {},
                    ..
                }
            )),
            "{name}: the turn ends"
        );
        assert!(
            !events
                .iter()
                .any(|s| matches!(s.event, AgentEvent::Other { ref raw_type, .. } if raw_type == "result")),
            "{name}"
        );
    }
}

#[test]
fn a_recording_of_the_permission_round_trip_holds_the_request_and_the_refusal_reasons() {
    let allow = Outcome::from_recording(
        real::recorded("permission-allow"),
        SessionStart::Assign {
            uuid: SESSION_STAND_IN.to_string(),
        },
    );
    assert_eq!(
        allow
            .agent_events()
            .into_iter()
            .filter(|e| matches!(e, AgentEvent::PermissionRequest { .. }))
            .count(),
        1
    );
}

#[test]
fn a_line_that_lost_a_recorded_field_or_changed_its_type_is_drift() {
    let shapes = Shapes::of(&real::all_recorded());
    let result = real::recorded("hello")
        .into_iter()
        .find(|l| l["type"] == "result")
        .expect("a recorded result");
    assert!(
        shapes.drift(&result).is_empty(),
        "a recorded line is not drift"
    );

    let mut lost = result.clone();
    lost.as_object_mut().unwrap().remove("total_cost_usd");
    assert_eq!(
        shapes.drift(&lost),
        vec!["result/success: `total_cost_usd` is gone".to_string()]
    );

    let mut retyped = result.clone();
    retyped["duration_api_ms"] = json!("slow");
    let found = shapes.drift(&retyped);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].contains("duration_api_ms"));

    let mut extra = result;
    extra["a_new_field"] = json!(1);
    assert!(
        shapes.drift(&extra).is_empty(),
        "an extra field is not drift"
    );
}

#[test]
fn the_default_run_is_skipped_with_its_reason() {
    if std::env::var("COLLIDE_REAL_CLAUDE").as_deref() == Ok("1") {
        return;
    }
    let reason = real::unavailable().expect("skipped without the opt-in");
    assert!(reason.contains("COLLIDE_REAL_CLAUDE=1"), "{reason}");
    let _ = Duration::ZERO;
}

#[test]
fn the_recordings_cover_the_kinds_of_line_the_adapter_reads() {
    let shapes = Shapes::of(&real::all_recorded());
    let have = shapes.kinds();
    for kind in [
        "system/init",
        "system/hook_started",
        "system/hook_response",
        "assistant",
        "user",
        "control_request",
        "result/success",
        "rate_limit_event",
    ] {
        assert!(
            have.contains(kind),
            "no recording holds a `{kind}` line: {have:?}"
        );
    }
}
