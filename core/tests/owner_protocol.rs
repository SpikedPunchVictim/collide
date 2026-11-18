//! Tests for the owner's JSON-RPC (ts-0032-pp): the envelope, the error codes, the handshake, the method list, every
//! request and result type, the event notifications and the generated schemas. A method's sample is written by
//! hand below, so the wire form of every request and result is pinned in one place.

use collide_core::owner::{
    Ack, CODES, MAX_MESSAGE_BYTES, Method, Notice, Notification, OWNER_PROTOCOL,
    OwnerInitializeParams, OwnerInitializeResult, Request, RequestId, Response, Routed, RpcError,
    VersionRefused, check_initialize, code, encode_line, parse_request, route, tokens_match,
};
use collide_core::schema;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::collections::BTreeSet;

fn round_trip<T: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug>(
    value: T,
    wire: Value,
) {
    assert_eq!(serde_json::to_value(&value).unwrap(), wire, "written form");
    assert_eq!(
        serde_json::from_value::<T>(wire).unwrap(),
        value,
        "read form"
    );
}

/// Whether every key and value of `sample` is in `written`, so a field the type lost shows. `written` may hold more:
/// a default the sample left out.
fn contains(written: &Value, sample: &Value) -> bool {
    match (written, sample) {
        (Value::Object(w), Value::Object(s)) => s
            .iter()
            .all(|(key, value)| w.get(key).is_some_and(|found| contains(found, value))),
        (Value::Array(w), Value::Array(s)) => {
            w.len() == s.len() && w.iter().zip(s).all(|(w, s)| contains(w, s))
        }
        _ => written == sample,
    }
}

fn schema_names() -> BTreeSet<String> {
    schema::registry().render().into_keys().collect()
}

// The pieces the samples share.

fn task() -> Value {
    json!({
        "key": "plan:ts-0001",
        "id": "ts-0001",
        "title": "A task",
        "state": { "status": "ready", "category": "ready" },
        "version": "v1"
    })
}

fn board() -> Value {
    json!({
        "id": "bd-1",
        "root": "/work/repo",
        "lines": [{
            "ref_name": "main",
            "root": "/work/repo",
            "snapshot": "abc123",
            "trust": { "state": "untrusted", "hash": "h1" },
            "problems": [".collide/board.yaml: packs must be a list"]
        }]
    })
}

fn run_summary() -> Value {
    json!({
        "id": "R-118",
        "tasks": [{ "key": "plan:ts-0001", "status": "active" }],
        "line": "main",
        "workflow": "collide/quick",
        "state": { "state": "running" },
        "started_ms": 1_700_000_000_000u64,
        "phases": [{
            "name": "build",
            "kind": "agent",
            "round": 1,
            "instances": [{ "id": "i1", "session_id": "s-1", "state": { "state": "running" } }],
            "started_ms": 1_700_000_001_000u64,
            "gate_results": { "tests": "ok" }
        }],
        "warnings": [{
            "kind": "instruction_files_changed", "phase": "build",
            "files": [".claude/skills/stand-in/SKILL.md", "CLAUDE.md"]
        }]
    })
}

fn workspace() -> Value {
    json!({
        "id": "ws-1",
        "kind": "task",
        "path": "/work/wt",
        "git_dir": "/work/repo/.git/worktrees/wt",
        "state": "in_use",
        "run": "R-118"
    })
}

fn pack() -> Value {
    json!({ "name": "collide", "source": "builtin", "origin": "builtin", "enabled": true, "dev": false })
}

/// A pack with a working copy to use, offered by `pack list`, and one `pack add` asks the person to trust.
fn pack_to_trust() -> Value {
    json!({
        "name": "ferry", "source": "git:file:///srv/ferry.git//pack@main", "origin": "user", "enabled": false,
        "dev": false, "working_copy": true,
        "review": {
            "name": "ferry", "version": "1.0.0", "commit": "a".repeat(40), "tree": "b".repeat(40),
            "hooks": [{ "key": "gates.lint", "script": "hooks/lint.sh" }],
            "providers": [], "observers": [], "requires": ["tool git"],
        },
    })
}

fn config_entry() -> Value {
    json!({ "key": "ask.timeout_s", "value": 180, "layer": "default", "secret": false })
}

fn queue_item() -> Value {
    json!({
        "id": "q-1",
        "kind": "permission",
        "run": "R-118",
        "phase": "build",
        "since_ms": 1_700_000_003_000u64,
        "title": "Permission requested: Bash",
        "permission": { "id": "p1", "tool_call_id": "c1", "kind": "execute", "input": { "command": "ls" } },
        "untrusted_input": true,
        "answers": ["permission"]
    })
}

fn apply_params() -> Value {
    json!({ "op": "setStatus", "id": "ts-0001", "args": { "status": "active" } })
}

fn event() -> Value {
    json!({
        "seq": 5,
        "wall_ms": 1_700_000_000_000u64,
        "actor": { "kind": "person" },
        "experiment": null,
        "body": { "event": "run.transition", "run": "R-118", "from": null, "to": { "state": "running" } }
    })
}

fn output_entry() -> Value {
    json!({ "index": 4, "wall_ms": 1_700_000_000_500u64, "event": { "type": "message", "text": "hello" } })
}

fn context_record() -> Value {
    let filled = json!({ "status": "filled", "text": "a rule matched" });
    let missing = json!({ "status": "not_available" });
    json!({
        "why_it_asks": filled,
        "what_it_would_do": missing,
        "task": missing,
        "what_led_here": missing,
        "history": missing,
        "agents_own_words": missing
    })
}

/// One sample of each method: its params, then its result.
fn samples() -> Vec<(Method, Value, Value)> {
    vec![
        (
            Method::Initialize,
            json!({ "protocol": 1, "token": "secret", "client": "collide-cli" }),
            json!({ "protocol": 1, "daemon_version": "0.0.0", "actor": { "kind": "person" } }),
        ),
        (
            Method::DaemonStatus,
            json!({}),
            json!({
                "daemon_version": "0.0.0", "protocol": 1, "pid": 4242,
                "started_ms": 1_700_000_000_000u64, "uptime_secs": 60, "head_seq": 12,
                "live_runs": 1, "store": { "state": "ok" }, "warnings": []
            }),
        ),
        (
            Method::DaemonRecaptureEnvironment,
            json!({}),
            json!({ "capture": { "outcome": "captured", "shell": "/bin/zsh", "variables": 84 } }),
        ),
        (
            Method::DaemonAgentVersions,
            json!({}),
            json!({ "agents": [{
                "agent": "claude", "in_use": "2.1.325", "tested": { "from": "2.1.323", "through": "2.1.325" },
                "in_tested_range": true,
                "published": { "latest": "2.1.325", "stable": "2.1.311", "fetched_ms": 1_700_000_000_000u64 }
            }] }),
        ),
        (
            Method::DaemonSubscribe,
            json!({ "since": 10 }),
            json!({ "head_seq": 12, "oldest_seq": 1, "replayed": 2 }),
        ),
        (Method::BoardList, json!({}), json!({ "boards": [board()] })),
        (
            Method::BoardOpen,
            json!({ "root": "/work/repo", "init_git": false }),
            json!({
                "board": board(),
                "checks": [{ "id": "git_repo", "ok": true, "required": true, "message": "a git repo" }]
            }),
        ),
        (
            Method::BoardInit,
            json!({ "board": "bd-1", "dry_run": true }),
            json!({ "files": [".collide/board.yaml"], "written": false }),
        ),
        (
            Method::BoardTrust,
            json!({
                "board": "bd-1", "line": "main", "hash": "h1",
                "hosts": ["example.invalid"], "dev_folders": ["/work/ferry"]
            }),
            json!({ "trust": { "state": "trusted", "hash": "h1" } }),
        ),
        (
            Method::BoardTrustReview,
            json!({ "board": "bd-1", "line": "main" }),
            json!({
                "trust": { "state": "untrusted", "hash": "h2" },
                "problems": ["pack git:x: needs a first fetch"],
                "changes": [
                    { "name": "board.yaml", "kind": "changed", "lines": ["@@ -1 +1 @@", "-a", "+b"] },
                    {
                        "name": "pack repo:.collide/packs/ferry", "kind": "added",
                        "files": [{ "path": "pack.yaml", "kind": "added", "lines": ["+name: ferry"] }],
                        "note": "the old objects are gone"
                    }
                ],
                "packs": [{
                    "source": "repo:.collide/packs/ferry",
                    "review": {
                        "name": "ferry", "tree": "t1", "hooks": [], "providers": [], "observers": [], "requires": []
                    }
                }],
                "hosts": ["example.invalid"],
                "dev_folders": ["/work/ferry"]
            }),
        ),
        (
            Method::TaskList,
            json!({ "board": "bd-1", "view": { "type": "named", "name": "edge-mvp" }, "ready": true }),
            json!({
                "tasks": [task()], "collections": [], "invalid": [],
                "view": {
                    "name": "edge-mvp", "members": 1, "errors": { "no such field": 2 }, "over_task_budget": 0,
                    "pass_budget_exhausted": false, "not_reached": 0, "unmatched_include": ["plan:x"],
                    "unmatched_exclude": [], "invalid_items": 1, "steps": [["plan:ts-0001"]], "cycle": []
                }
            }),
        ),
        (
            Method::TaskGet,
            json!({ "board": "bd-1", "id": "12" }),
            json!({
                "task": task(),
                "ambiguous": [],
                "copies": [{ "checkout": "main", "path": "a.md", "state": { "status": "ready", "category": "ready" }, "version": "v1" }],
                "collisions": [],
                "runs": ["R-118"],
                "latest_run": { "id": "R-117", "workflow": "collide/quick", "outcome": "merged", "state": {} }
            }),
        ),
        (
            Method::TaskApply,
            json!({ "board": "bd-1", "apply": apply_params(), "commit": false }),
            json!({ "result": { "outcome": "refused", "message": "illegal transition" } }),
        ),
        (
            Method::TaskPreview,
            json!({ "board": "bd-1", "apply": apply_params() }),
            json!({ "outcome": "ok", "items": [], "files": [] }),
        ),
        (
            Method::TaskViews,
            json!({ "board": "bd-1" }),
            json!({ "views": [{ "view": { "name": "edge-mvp" }, "source": { "type": "repo" }, "task_count": 3, "error_count": 0 }] }),
        ),
        (
            Method::TaskSaveView,
            json!({ "board": "bd-1", "view": { "name": "mine" }, "target": "personal", "overwrite": true }),
            json!({ "path": "/home/u/.config/collide/boards/bd-1/views/mine.yaml" }),
        ),
        (
            Method::TaskSections,
            json!({ "board": "bd-1" }),
            json!({ "sections": [], "nearDuplicates": [] }),
        ),
        (
            Method::RunStart,
            json!({ "board": "bd-1", "tasks": ["plan:ts-0001"], "launch": {}, "mode": "headless" }),
            json!({ "outcome": "started", "run": "R-118" }),
        ),
        (
            Method::RunStartFile,
            json!({
                "repo": "/work/repo", "task_file": "/work/task.md", "alias": "sonnet", "budget_usd": 1.5,
                "config": "none"
            }),
            json!({ "outcome": "started", "run": "R-119" }),
        ),
        (
            Method::RunPreview,
            json!({ "board": "bd-1", "tasks": ["plan:ts-0001"], "launch": {}, "mode": "headless" }),
            json!({
                "line": "main", "workflow": "collide/quick",
                "phases": [{ "name": "build", "kind": "agent" }],
                "implementations": { "workspace": "builtin", "integrate": "collide/merge-local" },
                "budget_usd": 2.5, "launch": {},
                "tasks": [{ "key": "plan:ts-0001", "status": "ready", "accepted": true }],
                "profile": { "name": "default", "reason": "board_default" },
                "settings": [{ "key": "ask.timeout_s", "value": 180, "layer": "default" }],
                "user_hooks": [{ "phase": "build", "name": "pre-tool", "flags": ["changes behaviour"] }],
                "badges": ["dev_pack"]
            }),
        ),
        (
            Method::RunList,
            json!({ "scope": "live" }),
            json!({ "runs": [run_summary()] }),
        ),
        (
            Method::RunGet,
            json!({ "run": "R-118" }),
            json!({
                "summary": run_summary(), "snapshot_commit": "abc123", "launch": {},
                "implementations": { "workspace": "builtin" }, "rounds": { "build": 1 },
                "fingerprint": {}, "annotations": []
            }),
        ),
        (
            Method::RunRecord,
            json!({ "run": "R-119" }),
            json!({
                "run": "R-119",
                "state": { "state": "ended", "outcome": "the phase ended" },
                "report": "Added a line to README.",
                "workspace": "/data/workspaces/R-119",
                "wall_ms": 3100,
                "warnings": [{
                    "kind": "instruction_files_changed", "phase": "build", "files": ["CLAUDE.md"]
                }],
                "calls": [{
                    "scope": "turn", "quality": "exact", "wall_ms": 3100, "model_ms": 2300,
                    "input": 4, "output": 270, "cache_read": 46000, "cache_write": 38000,
                    "cost": { "amount": 0.0084, "currency": "USD" }
                }, {
                    "scope": "compaction", "quality": "exact", "input": 1
                }]
            }),
        ),
        (
            Method::RunStop,
            json!({ "run": "R-118" }),
            json!({
                "state": { "state": "stopped" }, "signalled": 3,
                "survivors": [{ "pid": 99, "command": "/bin/sleep 600" }]
            }),
        ),
        (
            Method::RunDiff,
            json!({ "run": "R-118" }),
            json!({
                "base": "abc", "head": "def",
                "files": [{ "path": "a.rs", "status": "modified", "additions": 3, "deletions": 1 }],
                "collapsed": [{ "path": "node_modules", "files": 120, "bytes": 4096 }],
                "more": 0
            }),
        ),
        (
            Method::RunOutput,
            json!({ "run": "R-118", "phase": "build" }),
            json!({ "entries": [output_entry()], "next": 5 }),
        ),
        (
            Method::RunIntervene,
            json!({ "run": "R-118", "phase": "build", "note": "check the cache" }),
            json!({}),
        ),
        (
            Method::RunTakeOver,
            json!({ "run": "R-118", "phase": "build", "cols": 80, "rows": 24 }),
            json!({ "terminal": "t-1", "session_id": "s-1", "replay": "aGk=" }),
        ),
        (
            Method::RunTerminalInput,
            json!({ "terminal": "t-1", "data": "bHMK" }),
            json!({}),
        ),
        (
            Method::RunTerminalResize,
            json!({ "terminal": "t-1", "cols": 100, "rows": 30 }),
            json!({}),
        ),
        (Method::RunDetach, json!({ "terminal": "t-1" }), json!({})),
        (
            Method::RunEndTakeOver,
            json!({ "run": "R-118", "phase": "build" }),
            json!({}),
        ),
        (
            Method::QueueList,
            json!({ "kinds": ["permission"] }),
            json!({ "items": [queue_item()] }),
        ),
        (
            Method::QueueContext,
            json!({ "item": "q-1" }),
            context_record(),
        ),
        (
            Method::QueueAnswer,
            json!({ "item": "q-1", "answer": { "type": "permission", "reply": { "type": "allow" } } }),
            json!({ "outcome": "done" }),
        ),
        (
            Method::WorkspaceList,
            json!({ "board": "bd-1" }),
            json!({ "workspaces": [workspace()] }),
        ),
        (
            Method::WorkspaceRemove,
            json!({ "id": "ws-1" }),
            json!({ "outcome": "removed" }),
        ),
        (
            Method::WorkspaceResume,
            json!({ "id": "ws-1" }),
            json!({ "workspace": workspace() }),
        ),
        (
            Method::ConfigGet,
            json!({ "key": "ask.timeout_s" }),
            json!({ "entries": [config_entry()] }),
        ),
        (
            Method::ConfigSet,
            json!({ "key": "ask.timeout_s", "value": 240, "layer": "user", "secret": false }),
            json!({ "entry": config_entry() }),
        ),
        (
            Method::ConfigPackList,
            json!({ "board": "bd-1" }),
            json!({ "packs": [pack(), { "name": "scratch", "source": "dev:/work/scratch", "origin": "offered",
                "enabled": false, "dev": false }] }),
        ),
        (
            Method::ConfigPackAdd,
            json!({ "source": "git:https://example.com/p.git", "confirm": true }),
            json!({ "pack": pack_to_trust(), "written": false, "note": "nothing was written" }),
        ),
        (
            Method::ConfigPackLink,
            json!({ "board": "bd-1", "name": "collide", "folder": "/work/packs/collide" }),
            json!({ "pack": pack(), "written": true }),
        ),
        (
            Method::ConfigPackEnable,
            json!({ "board": "bd-1", "name": "collide", "enabled": true }),
            json!({ "pack": pack(), "written": false }),
        ),
        (
            Method::ConfigPackUpdate,
            json!({ "board": "bd-1", "name": "ferry", "to": "v1.3", "confirm": true, "approve_hosts": ["github.com"] }),
            json!({
                "pack": pack(),
                "old": { "commit": "a".repeat(40), "tree": "b".repeat(40) },
                "new": { "commit": "c".repeat(40), "tree": "d".repeat(40) },
                "changes": [{ "path": "pack.yaml", "kind": "modified", "lines": ["@@ -1 +1 @@", "-a", "+b"] }],
                "written": true,
            }),
        ),
    ]
}

fn request(method: &str, params: Value) -> Request {
    serde_json::from_value(json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params }))
        .unwrap()
}

// The method list.

#[test]
fn every_method_has_a_sample_and_every_sample_names_a_method() {
    let sampled: Vec<Method> = samples().into_iter().map(|(method, ..)| method).collect();
    assert_eq!(sampled, Method::ALL, "samples are in the list's order");
}

#[test]
fn method_names_are_unique_and_start_with_their_family() {
    let mut seen = BTreeSet::new();
    for method in Method::ALL {
        assert!(seen.insert(method.name()), "{} twice", method.name());
        match method {
            Method::Initialize => assert_eq!(method.name(), "initialize"),
            _ => assert!(
                method
                    .name()
                    .starts_with(&format!("{}.", method.family().name())),
                "{} is not in the {} family",
                method.name(),
                method.family().name()
            ),
        }
        assert_eq!(Method::from_name(method.name()), Some(*method));
    }
}

#[test]
fn the_planned_families_each_have_a_method_and_ask_and_ui_tools_have_none() {
    let families: BTreeSet<&str> = Method::ALL.iter().map(|m| m.family().name()).collect();
    for named in ["daemon", "run", "task", "queue", "workspace", "config"] {
        assert!(families.contains(named), "{named} has no method");
    }
    assert!(!families.contains("ask"), "ask.* is ts-0034-hh's");
    assert!(!families.contains("ui_tools"), "ui_tools.* is ts-0035-re's");
}

#[test]
fn every_method_says_what_it_serves() {
    for method in Method::ALL {
        let serves = method.serves();
        assert!(
            serves.starts_with("collide ") || serves.starts_with("0."),
            "{}: `{serves}` names neither a CLI command nor a plan section",
            method.name()
        );
    }
}

// Round trips.

#[test]
fn every_request_round_trips() {
    for (method, params, _) in samples() {
        let written = method
            .normalize_params(&params)
            .unwrap_or_else(|e| panic!("{}: {e}", method.name()));
        assert!(
            contains(&written, &params),
            "{}: {written} lost {params}",
            method.name()
        );
        let again = method.normalize_params(&written).unwrap();
        assert_eq!(written, again, "{} is not stable", method.name());
    }
}

#[test]
fn every_result_round_trips() {
    for (method, _, result) in samples() {
        let written = method
            .normalize_result(&result)
            .unwrap_or_else(|e| panic!("{}: {e}", method.name()));
        assert!(
            contains(&written, &result),
            "{}: {written} lost {result}",
            method.name()
        );
        let again = method.normalize_result(&written).unwrap();
        assert_eq!(written, again, "{} is not stable", method.name());
    }
}

#[test]
fn a_request_with_no_params_reads_as_empty_params() {
    let written = Method::DaemonStatus.normalize_params(&Value::Null).unwrap();
    assert_eq!(written, json!({}));
}

#[test]
fn every_request_type_refuses_an_actor_key() {
    for (method, params, _) in samples() {
        let mut with_actor = params.clone();
        with_actor
            .as_object_mut()
            .unwrap()
            .insert("actor".into(), json!({ "kind": "person" }));
        let error = method
            .normalize_params(&with_actor)
            .expect_err(&format!("{} took an actor", method.name()));
        assert!(
            error.to_string().contains("unknown field `actor`"),
            "{}: {error}",
            method.name()
        );
    }
}

#[test]
fn every_request_type_refuses_any_unknown_key() {
    for (method, params, _) in samples() {
        let mut extra = params.clone();
        extra
            .as_object_mut()
            .unwrap()
            .insert("surprise".into(), json!(1));
        assert!(
            method.normalize_params(&extra).is_err(),
            "{} took an unknown key",
            method.name()
        );
    }
}

#[test]
fn routing_a_request_with_an_actor_names_the_actor_rule() {
    let mut params = samples()[1].1.clone();
    params
        .as_object_mut()
        .unwrap()
        .insert("actor".into(), json!({ "kind": "person" }));
    let error = route(true, &request("daemon.status", params)).unwrap_err();
    assert_eq!(error.code, code::INVALID_PARAMS);
    assert!(error.message.contains("actor"), "{}", error.message);
    assert!(
        error.message.contains("connection"),
        "the message says where the actor comes from: {}",
        error.message
    );
}

// Specific shapes.

#[test]
fn the_ack_is_an_empty_object() {
    round_trip(Ack {}, json!({}));
    assert!(serde_json::from_value::<Ack>(json!({ "x": 1 })).is_err());
}

#[test]
fn a_run_start_is_started_or_refused_with_every_problem() {
    let refused = json!({
        "outcome": "refused",
        "problems": [
            { "task": "plan:ts-0001", "message": "the workflow needs a `spec` field" },
            { "message": "no workflow named" }
        ]
    });
    let parsed = Method::RunStart.normalize_result(&refused).unwrap();
    assert_eq!(parsed, refused);
}

#[test]
fn every_queue_answer_round_trips() {
    let answers = [
        json!({ "type": "permission", "reply": { "type": "deny", "message": "no" } }),
        json!({ "type": "text", "text": "use sqlite" }),
        json!({ "type": "proceed" }),
        json!({ "type": "stop" }),
        json!({ "type": "retry" }),
        json!({ "type": "raise_cap", "amount": { "amount": 2.0, "currency": "USD" } }),
        json!({ "type": "accept" }),
        json!({ "type": "discard" }),
        json!({ "type": "reprompt", "prompt": "also add a test" }),
        json!({ "type": "acknowledge" }),
        json!({ "type": "resume" }),
        json!({ "type": "dismiss" }),
    ];
    for answer in answers {
        let params = json!({ "item": "q-1", "answer": answer });
        let written = Method::QueueAnswer.normalize_params(&params).unwrap();
        assert_eq!(written, params);
    }
}

#[test]
fn a_queue_answer_with_a_key_its_variant_lacks_is_refused() {
    let params = json!({ "item": "q-1", "answer": { "type": "accept", "commit": "abc" } });
    assert!(Method::QueueAnswer.normalize_params(&params).is_err());
}

#[test]
fn every_queue_answer_result_round_trips() {
    for result in [
        json!({ "outcome": "done" }),
        json!({ "outcome": "merged", "commit": "abc123" }),
        json!({ "outcome": "integrating", "url": "https://example.com/pr/1", "head": "abc" }),
        json!({ "outcome": "queued", "item": "q-9" }),
        json!({ "outcome": "refused", "message": "the range edits the agent's config" }),
    ] {
        assert_eq!(
            Method::QueueAnswer.normalize_result(&result).unwrap(),
            result
        );
    }
}

#[test]
fn a_queued_write_has_no_result_and_names_its_item_and_why() {
    let queued = json!({
        "queued": { "item": "write-1", "reason": "a rebase is in progress in /work" }
    });
    assert_eq!(Method::TaskApply.normalize_result(&queued).unwrap(), queued);
    let done = json!({ "result": { "outcome": "refused", "message": "no" } });
    assert_eq!(Method::TaskApply.normalize_result(&done).unwrap(), done);
}

#[test]
fn a_run_diff_may_carry_one_files_patch() {
    let result = json!({
        "base": "abc", "head": "def", "files": [], "collapsed": [], "more": 0,
        "patch": { "path": "a.rs", "text": "@@ -1 +1 @@", "bytes": 11, "truncated": false }
    });
    assert_eq!(Method::RunDiff.normalize_result(&result).unwrap(), result);
}

#[test]
fn a_store_that_was_moved_aside_is_reported_in_the_status() {
    let result = json!({
        "daemon_version": "0.0.0", "protocol": 1, "pid": 1, "started_ms": 1, "uptime_secs": 0,
        "head_seq": 0, "live_runs": 0, "warnings": ["started with setsid"],
        "store": { "state": "quarantined", "moved_to": "/s/store.bad", "reason": "not a database" }
    });
    assert_eq!(
        Method::DaemonStatus.normalize_result(&result).unwrap(),
        result
    );
}

#[test]
fn an_environment_capture_that_fell_back_names_the_shell_and_the_reason() {
    let result = json!({
        "capture": { "outcome": "fell_back", "shell": "/bin/zsh", "reason": "timed out after 10 s" }
    });
    assert_eq!(
        Method::DaemonRecaptureEnvironment
            .normalize_result(&result)
            .unwrap(),
        result
    );
}

#[test]
fn a_version_outside_the_range_carries_its_smoke_check_and_a_failed_fetch_keeps_its_error() {
    let result = json!({ "agents": [{
        "agent": "claude", "in_use": "2.2.0", "tested": { "from": "2.1.323", "through": "2.1.325" },
        "in_tested_range": false,
        "smoke": { "passed": false, "at_ms": 1_700_000_000_000u64, "reason": "no result line" },
        "published": { "latest": "2.1.325", "fetched_ms": 1_699_999_000_000u64, "error": "timed out" }
    }] });
    assert_eq!(
        Method::DaemonAgentVersions
            .normalize_result(&result)
            .unwrap(),
        result
    );
}

#[test]
fn a_config_entry_for_a_secret_has_no_value() {
    let result = json!({ "entries": [{ "key": "pack.token", "layer": "user", "secret": true }] });
    assert_eq!(Method::ConfigGet.normalize_result(&result).unwrap(), result);
}

#[test]
fn the_phase_kind_names_are_the_workflows() {
    use collide_core::owner::PhaseKindWire;
    use collide_core::workflow::PhaseKind;
    for kind in [
        PhaseKind::Agent,
        PhaseKind::Fanout,
        PhaseKind::Script,
        PhaseKind::Wait,
        PhaseKind::Human,
    ] {
        let wire = PhaseKindWire::from(kind);
        assert_eq!(serde_json::to_value(wire).unwrap(), json!(kind.name()));
    }
}

// The envelope.

#[test]
fn a_request_round_trips_with_a_number_and_with_a_text_id() {
    round_trip(
        Request {
            jsonrpc: Default::default(),
            id: RequestId::Number(7),
            method: "daemon.status".into(),
            params: json!({}),
        },
        json!({ "jsonrpc": "2.0", "id": 7, "method": "daemon.status", "params": {} }),
    );
    round_trip(
        Request {
            jsonrpc: Default::default(),
            id: RequestId::Text("a-1".into()),
            method: "daemon.status".into(),
            params: Value::Null,
        },
        json!({ "jsonrpc": "2.0", "id": "a-1", "method": "daemon.status" }),
    );
}

#[test]
fn a_request_that_is_not_json_rpc_2_is_refused() {
    let old = json!({ "jsonrpc": "1.0", "id": 1, "method": "daemon.status" });
    assert!(serde_json::from_value::<Request>(old).is_err());
    let extra = json!({ "jsonrpc": "2.0", "id": 1, "method": "m", "actor": "x" });
    assert!(serde_json::from_value::<Request>(extra).is_err());
}

#[test]
fn a_response_is_a_result_or_an_error() {
    round_trip(
        Response::success(RequestId::Number(7), json!({ "ok": true })),
        json!({ "jsonrpc": "2.0", "id": 7, "result": { "ok": true } }),
    );
    round_trip(
        Response::failure(
            Some(RequestId::Number(7)),
            RpcError::method_not_found("nope"),
        ),
        json!({
            "jsonrpc": "2.0", "id": 7,
            "error": { "code": -32601, "message": "method not found: `nope`" }
        }),
    );
    round_trip(
        Response::failure(None, RpcError::new(code::PARSE_ERROR, "bad json")),
        json!({ "jsonrpc": "2.0", "id": null, "error": { "code": -32700, "message": "bad json" } }),
    );
}

#[test]
fn an_error_may_carry_data() {
    round_trip(
        RpcError {
            code: code::NOT_FOUND,
            message: "no run `R-9`".into(),
            data: Some(json!({ "kind": "run", "id": "R-9" })),
        },
        json!({ "code": -32010, "message": "no run `R-9`", "data": { "kind": "run", "id": "R-9" } }),
    );
}

#[test]
fn a_message_is_one_line() {
    let line = encode_line(&Response::success(
        RequestId::Number(1),
        json!({ "text": "two\nlines" }),
    ));
    assert!(line.ends_with('\n'));
    assert_eq!(line.matches('\n').count(), 1, "{line:?}");
}

#[test]
fn text_that_is_not_json_answers_parse_error_with_a_null_id() {
    let Err(response) = parse_request("{not json") else {
        panic!("parsed")
    };
    let Response::Failure { id, error, .. } = response else {
        panic!("not a failure")
    };
    assert_eq!(id, None);
    assert_eq!(error.code, -32700);
}

#[test]
fn json_that_is_not_a_request_answers_invalid_request() {
    for text in [
        "[1,2]",
        "42",
        r#"{"jsonrpc":"2.0","id":1}"#,
        r#"{"id":1,"method":"m"}"#,
    ] {
        let Err(Response::Failure { error, .. }) = parse_request(text) else {
            panic!("{text} parsed")
        };
        assert_eq!(error.code, -32600, "{text}");
    }
}

#[test]
fn an_invalid_request_with_a_readable_id_answers_that_id() {
    let Err(Response::Failure { id, .. }) = parse_request(r#"{"jsonrpc":"2.0","id":9}"#) else {
        panic!("parsed")
    };
    assert_eq!(id, Some(RequestId::Number(9)));
}

#[test]
fn a_message_over_the_limit_is_refused_naming_both_sizes() {
    let text = format!(
        r#"{{"jsonrpc":"2.0","id":1,"method":"m","params":"{}"}}"#,
        "x".repeat(MAX_MESSAGE_BYTES)
    );
    let Err(Response::Failure { error, .. }) = parse_request(&text) else {
        panic!("parsed")
    };
    assert_eq!(error.code, code::INVALID_REQUEST);
    assert!(
        error.message.contains(&MAX_MESSAGE_BYTES.to_string()),
        "{}",
        error.message
    );
    assert!(
        error.message.contains(&text.len().to_string()),
        "{}",
        error.message
    );
}

#[test]
fn a_good_line_parses_to_the_request() {
    let line = r#"{"jsonrpc":"2.0","id":3,"method":"daemon.status","params":{}}"#;
    let parsed = parse_request(line).unwrap();
    assert_eq!(parsed.method, "daemon.status");
    assert_eq!(parsed.id, RequestId::Number(3));
}

// The error codes.

#[test]
fn the_standard_codes_are_json_rpcs() {
    assert_eq!(code::PARSE_ERROR, -32700);
    assert_eq!(code::INVALID_REQUEST, -32600);
    assert_eq!(code::METHOD_NOT_FOUND, -32601);
    assert_eq!(code::INVALID_PARAMS, -32602);
    assert_eq!(code::INTERNAL_ERROR, -32603);
}

#[test]
fn the_owners_codes_are_distinct_documented_and_in_the_server_range() {
    let mut seen = BTreeSet::new();
    for (value, name, meaning) in CODES {
        assert!(seen.insert(*value), "{name}: code {value} twice");
        assert!(!meaning.is_empty(), "{name} has no meaning");
        let own = !matches!(*value, -32700 | -32603..=-32600);
        if own {
            assert!(
                (-32099..=-32000).contains(value),
                "{name}: {value} is outside the server range"
            );
        }
    }
    for value in [
        code::NOT_INITIALIZED,
        code::ALREADY_INITIALIZED,
        code::VERSION_REFUSED,
        code::TOKEN_REFUSED,
        code::NOT_PERMITTED,
        code::NOT_FOUND,
    ] {
        assert!(seen.contains(&value), "{value} is not in the table");
    }
}

// The handshake.

fn hello(protocol: u32, token: Option<&str>) -> OwnerInitializeParams {
    OwnerInitializeParams {
        protocol,
        token: token.map(str::to_string),
        client: Some("collide-cli".into()),
    }
}

#[test]
fn a_matching_version_and_token_are_accepted() {
    assert_eq!(
        check_initialize(&hello(OWNER_PROTOCOL, Some("t")), "t"),
        Ok(())
    );
}

#[test]
fn an_older_client_is_refused_naming_both_versions() {
    let error = check_initialize(&hello(OWNER_PROTOCOL + 4, Some("t")), "t").unwrap_err();
    assert_eq!(error.code, code::VERSION_REFUSED);
    assert!(
        error.message.contains(&format!("{}", OWNER_PROTOCOL + 4)),
        "{}",
        error.message
    );
    assert!(
        error.message.contains(&format!("{OWNER_PROTOCOL}")),
        "{}",
        error.message
    );
    assert_eq!(
        error.data,
        Some(json!({ "client": OWNER_PROTOCOL + 4, "daemon": OWNER_PROTOCOL }))
    );
}

#[test]
fn the_version_refusal_says_which_side_is_older() {
    let newer = VersionRefused {
        client: 2,
        daemon: 1,
    }
    .to_string();
    let older = VersionRefused {
        client: 1,
        daemon: 2,
    }
    .to_string();
    assert!(newer.contains("newer"), "{newer}");
    assert!(older.contains("older"), "{older}");
}

#[test]
fn a_wrong_token_is_refused_with_its_own_code() {
    let error = check_initialize(&hello(OWNER_PROTOCOL, Some("nope")), "t").unwrap_err();
    assert_eq!(error.code, code::TOKEN_REFUSED);
    assert_ne!(error.code, code::VERSION_REFUSED);
    assert!(
        !error.message.contains("t\""),
        "the message never shows a token"
    );
}

#[test]
fn a_missing_token_is_refused_with_the_same_code_as_a_wrong_one() {
    let missing = check_initialize(&hello(OWNER_PROTOCOL, None), "t").unwrap_err();
    let wrong = check_initialize(&hello(OWNER_PROTOCOL, Some("x")), "t").unwrap_err();
    assert_eq!(missing.code, code::TOKEN_REFUSED);
    assert_eq!(missing.code, wrong.code);
    assert!(missing.message.contains("missing"), "{}", missing.message);
}

#[test]
fn the_token_is_checked_before_the_version() {
    let error = check_initialize(&hello(OWNER_PROTOCOL + 1, Some("nope")), "t").unwrap_err();
    assert_eq!(
        error.code,
        code::TOKEN_REFUSED,
        "an unauthenticated peer learns no version"
    );
}

#[test]
fn tokens_compare_whole_values() {
    assert!(tokens_match("abcd", "abcd"));
    assert!(!tokens_match("abcd", "abce"));
    assert!(!tokens_match("abcd", "abc"));
    assert!(!tokens_match("abc", "abcd"));
    assert!(!tokens_match("", "a"));
}

#[test]
fn the_handshake_params_and_result_round_trip() {
    round_trip(
        hello(1, Some("t")),
        json!({ "protocol": 1, "token": "t", "client": "collide-cli" }),
    );
    round_trip(
        OwnerInitializeResult {
            protocol: 1,
            daemon_version: "0.0.0".into(),
            actor: serde_json::from_value(json!({ "kind": "agent", "run": "R-1" })).unwrap(),
        },
        json!({ "protocol": 1, "daemon_version": "0.0.0", "actor": { "kind": "agent", "run": "R-1" } }),
    );
}

// Routing.

#[test]
fn an_unknown_method_answers_method_not_found() {
    for initialized in [false, true] {
        let error = route(initialized, &request("daemon.nope", json!({}))).unwrap_err();
        assert_eq!(error.code, -32601);
        assert!(error.message.contains("daemon.nope"), "{}", error.message);
    }
}

#[test]
fn the_ask_and_ui_tools_families_are_unknown_here() {
    for method in ["ask.start", "ask.cancel", "ask.test", "ui_tools.reply"] {
        let error = route(true, &request(method, json!({}))).unwrap_err();
        assert_eq!(error.code, code::METHOD_NOT_FOUND, "{method}");
    }
}

#[test]
fn a_method_before_initialize_is_refused() {
    let error = route(false, &request("daemon.status", json!({}))).unwrap_err();
    assert_eq!(error.code, code::NOT_INITIALIZED);
    assert!(error.message.contains("initialize"), "{}", error.message);
}

#[test]
fn initialize_routes_with_its_params_and_only_once() {
    let hello = request("initialize", json!({ "protocol": 1, "token": "t" }));
    assert!(matches!(route(false, &hello), Ok(Routed::Initialize(_))));
    let error = route(true, &hello).unwrap_err();
    assert_eq!(error.code, code::ALREADY_INITIALIZED);
}

#[test]
fn bad_params_answer_invalid_params_naming_the_method() {
    let error = route(true, &request("run.stop", json!({ "run": 5 }))).unwrap_err();
    assert_eq!(error.code, -32602);
    assert!(error.message.contains("run.stop"), "{}", error.message);
}

#[test]
fn every_method_routes_after_initialize() {
    for (method, params, _) in samples() {
        if method == Method::Initialize {
            continue;
        }
        let routed = route(true, &request(method.name(), params)).unwrap();
        assert!(matches!(routed, Routed::Call(found) if found == method));
    }
}

// The notifications.

#[test]
fn every_notice_round_trips_through_its_envelope() {
    let notices = [
        ("daemon.event", event()),
        (
            "run.output",
            json!({ "run": "R-118", "phase": "build", "entry": output_entry() }),
        ),
        (
            "run.terminal_output",
            json!({ "terminal": "t-1", "data": "aGk=" }),
        ),
        (
            "queue.changed",
            json!({ "count": 2, "added": ["q-2"], "removed": [] }),
        ),
    ];
    for (name, params) in notices {
        let wire = json!({ "jsonrpc": "2.0", "method": name, "params": params });
        let envelope: Notification = serde_json::from_value(wire.clone()).unwrap();
        let notice = Notice::from_notification(&envelope).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(notice.method(), name);
        let back = serde_json::to_value(notice.to_notification()).unwrap();
        assert!(contains(&back, &wire), "{name}: {back}");
    }
}

#[test]
fn the_event_notice_carries_the_sequence_number_and_the_actor_the_daemon_recorded() {
    let envelope: Notification = serde_json::from_value(
        json!({ "jsonrpc": "2.0", "method": "daemon.event", "params": event() }),
    )
    .unwrap();
    let Notice::DaemonEvent(found) = Notice::from_notification(&envelope).unwrap() else {
        panic!("not an event")
    };
    assert_eq!(found.seq, 5);
    assert_eq!(
        serde_json::to_value(&found.actor).unwrap(),
        json!({ "kind": "person" })
    );
}

#[test]
fn an_event_notice_with_a_key_the_event_lacks_is_refused() {
    let mut params = event();
    params
        .as_object_mut()
        .unwrap()
        .insert("extra".into(), json!(1));
    let envelope: Notification = serde_json::from_value(
        json!({ "jsonrpc": "2.0", "method": "daemon.event", "params": params }),
    )
    .unwrap();
    assert!(Notice::from_notification(&envelope).is_err());
}

#[test]
fn an_unknown_notification_is_an_error() {
    let envelope: Notification =
        serde_json::from_value(json!({ "jsonrpc": "2.0", "method": "run.nope", "params": {} }))
            .unwrap();
    assert!(Notice::from_notification(&envelope).is_err());
}

// The schemas.

#[test]
fn every_method_has_a_request_and_a_result_schema_in_the_registry() {
    let names = schema_names();
    for method in Method::ALL {
        for root in [method.params_schema(), method.result_schema()] {
            for side in ["input", "output"] {
                let file = format!("{root}.{side}.json");
                assert!(
                    names.contains(&file),
                    "{}: {file} is not registered",
                    method.name()
                );
            }
        }
    }
}

#[test]
fn the_envelope_error_and_notice_types_are_registered() {
    let names = schema_names();
    for root in [
        "OwnerRequest",
        "OwnerResponse",
        "OwnerNotification",
        "OwnerNotice",
        "RpcError",
    ] {
        assert!(names.contains(&format!("{root}.input.json")), "{root}");
        assert!(names.contains(&format!("{root}.output.json")), "{root}");
    }
}

#[test]
fn no_two_methods_share_a_request_schema_unless_by_design() {
    let mut owners: std::collections::BTreeMap<String, Vec<&str>> = Default::default();
    for method in Method::ALL {
        owners
            .entry(method.params_schema())
            .or_default()
            .push(method.name());
    }
    let shared: Vec<_> = owners.iter().filter(|(_, m)| m.len() > 1).collect();
    assert_eq!(
        shared.len(),
        1,
        "only run.preview shares run.start's request: {shared:?}"
    );
    assert_eq!(shared[0].1, &vec!["run.start", "run.preview"]);
}

#[test]
fn a_file_run_request_without_config_leaves_it_out_and_a_bad_config_is_refused() {
    use collide_core::owner::RunStartFileParams;
    use collide_core::profile::RoleConfig;
    let plain: RunStartFileParams =
        serde_json::from_value(json!({ "repo": "/r", "task_file": "/t.md" })).unwrap();
    assert_eq!(plain.config, None);
    assert!(
        !serde_json::to_value(&plain)
            .unwrap()
            .as_object()
            .unwrap()
            .contains_key("config")
    );
    for (word, config) in [("none", RoleConfig::None), ("user", RoleConfig::User)] {
        let asked: RunStartFileParams =
            serde_json::from_value(json!({ "repo": "/r", "task_file": "/t.md", "config": word }))
                .unwrap();
        assert_eq!(asked.config, Some(config));
    }
    let bad = serde_json::from_value::<RunStartFileParams>(
        json!({ "repo": "/r", "task_file": "/t.md", "config": "project" }),
    );
    assert!(bad.is_err());
}

#[test]
fn a_run_without_warnings_reads_as_an_empty_list_and_a_warning_refuses_an_unknown_key() {
    use collide_core::owner::{RunSummary, RunWarning};
    let mut written = run_summary();
    written.as_object_mut().unwrap().remove("warnings");
    let summary: RunSummary = serde_json::from_value(written).unwrap();
    assert!(summary.warnings.is_empty());
    let bad = serde_json::from_value::<RunWarning>(
        json!({ "kind": "instruction_files_changed", "phase": "build", "files": [], "extra": 1 }),
    );
    assert!(bad.is_err());
    let unknown_kind = serde_json::from_value::<RunWarning>(
        json!({ "kind": "something_else", "phase": "build", "files": [] }),
    );
    assert!(unknown_kind.is_err());
}
