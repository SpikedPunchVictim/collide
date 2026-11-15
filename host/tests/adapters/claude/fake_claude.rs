//! Fake Claude on its own, as a plain child with no daemon and no session (ts-0062-rj): it writes what a recording says,
//! waits for what the recording expects on stdin, and exits as the recording says.

use super::common::TempRoot;
use super::fake::{FakeClaude, recording, said, value_of_line};
use serde_json::{Value, json};
use std::fs;
use std::path::PathBuf;

const TURN: &[u8] = b"{\"type\":\"user\",\"message\":{\"role\":\"user\",\"content\":[{\"type\":\"text\",\"text\":\"go\"}]}}\n";

/// A recording the test writes itself, in the root's folder.
fn script(root: &TempRoot, steps: &[&str]) -> PathBuf {
    let path = root.join("recording.jsonl");
    fs::write(&path, steps.join("\n")).unwrap();
    path
}

/// Every line fake Claude writes before its stdout closes, as values.
fn all_lines(fake: &mut FakeClaude) -> Vec<Value> {
    std::iter::from_fn(|| fake.next_line())
        .map(|line| value_of_line(&line))
        .collect()
}

#[test]
fn a_recording_is_replayed_line_for_line_after_the_turn_it_waits_for() {
    let root = TempRoot::new();
    let path = recording("report");
    let mut fake = FakeClaude::start(&root, &path);
    fake.send(TURN);
    fake.close_stdin();
    assert_eq!(all_lines(&mut fake), said(&path));
    let ended = fake.finish();
    assert_eq!(ended.code, Some(0), "{}", ended.stderr);
}

#[test]
fn nothing_is_written_before_the_turn_the_recording_waits_for_arrives() {
    let root = TempRoot::new();
    let path = recording("report");
    let mut fake = FakeClaude::start(&root, &path);
    // Stdin closes with no turn: fake Claude ends, having written nothing.
    fake.close_stdin();
    assert_eq!(all_lines(&mut fake), Vec::<Value>::new());
    let ended = fake.finish();
    assert_eq!(ended.code, Some(98));
    assert!(ended.stderr.contains("stdin closed"), "{}", ended.stderr);
}

#[test]
fn a_line_that_is_not_what_the_recording_expects_ends_it_with_97_and_says_what_it_got() {
    let root = TempRoot::new();
    let path = script(
        &root,
        &[r#"{"expect":{"type":"control_response","response":{"response":{"behavior":"allow"}}}}"#],
    );
    let mut fake = FakeClaude::start(&root, &path);
    fake.send(TURN);
    let ended = fake.finish();
    assert_eq!(ended.code, Some(97));
    assert!(
        ended.stderr.contains("control_response"),
        "{}",
        ended.stderr
    );
    assert!(ended.stderr.contains("\"user\""), "{}", ended.stderr);
}

#[test]
fn an_expectation_is_a_subset_so_a_line_may_hold_more() {
    let root = TempRoot::new();
    let path = script(
        &root,
        &[
            r#"{"expect":{"message":{"content":[{"text":"go"}]}}}"#,
            r#"{"say":{"type":"system","subtype":"init"}}"#,
        ],
    );
    let mut fake = FakeClaude::start(&root, &path);
    fake.send(TURN);
    assert_eq!(
        value_of_line(&fake.next_line().unwrap()),
        json!({"type": "system", "subtype": "init"})
    );
    assert_eq!(fake.finish().code, Some(0));
}

#[test]
fn a_list_in_an_expectation_must_have_the_length_it_names() {
    let root = TempRoot::new();
    let path = script(&root, &[r#"{"expect":{"message":{"content":[]}}}"#]);
    let mut fake = FakeClaude::start(&root, &path);
    fake.send(TURN);
    assert_eq!(fake.finish().code, Some(97));
}

#[test]
fn an_exit_step_ends_it_with_the_recorded_code_and_a_stderr_step_is_written_first() {
    let root = TempRoot::new();
    let path = recording("exit-code");
    let mut fake = FakeClaude::start(&root, &path);
    fake.send(TURN);
    assert_eq!(all_lines(&mut fake), said(&path));
    let ended = fake.finish();
    assert_eq!(ended.code, Some(3));
    assert_eq!(ended.stderr, "Error: the model connection was lost\n");
}

#[test]
fn a_recording_that_waits_for_stdin_to_close_before_its_exit_step_does_so() {
    let root = TempRoot::new();
    let path = recording("error-result");
    let mut fake = FakeClaude::start(&root, &path);
    fake.send(TURN);
    // The last line is written, and the process stays up until stdin closes.
    let lines = std::iter::from_fn(|| fake.next_line())
        .take(said(&path).len())
        .count();
    assert_eq!(lines, said(&path).len());
    assert_eq!(fake.finish().code, Some(1));
}

#[test]
fn a_text_step_writes_a_line_that_is_not_json() {
    let root = TempRoot::new();
    let path = script(&root, &[r#"{"say_text":"not json at all"}"#]);
    let mut fake = FakeClaude::start(&root, &path);
    assert_eq!(fake.next_line().unwrap(), b"not json at all\n");
    assert_eq!(
        value_of_line(b"not json at all\n"),
        json!("not json at all")
    );
    assert_eq!(fake.finish().code, Some(0));
}

#[test]
fn a_comment_and_a_blank_line_are_skipped() {
    let root = TempRoot::new();
    let path = script(
        &root,
        &[
            "# a comment",
            "",
            r#"{"say":{"type":"system","subtype":"init"}}"#,
        ],
    );
    let mut fake = FakeClaude::start(&root, &path);
    fake.close_stdin();
    assert_eq!(all_lines(&mut fake).len(), 1);
    assert_eq!(fake.finish().code, Some(0));
}

#[test]
fn a_step_it_does_not_know_ends_it_with_96() {
    let root = TempRoot::new();
    let path = script(&root, &[r#"{"dance":true}"#]);
    let fake = FakeClaude::start(&root, &path);
    let ended = fake.finish();
    assert_eq!(ended.code, Some(96));
    assert!(ended.stderr.contains("dance"), "{}", ended.stderr);
}

#[test]
fn every_recording_is_valid_json_per_line_and_says_something() {
    let dir =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/adapters/claude/fixtures/recordings");
    let mut seen = 0;
    for entry in fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        let text = fs::read_to_string(&path).unwrap();
        for line in text
            .lines()
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
        {
            serde_json::from_str::<serde_json::Value>(line)
                .unwrap_or_else(|e| panic!("{}: {e}: {line}", path.display()));
        }
        assert!(!said(&path).is_empty(), "{}", path.display());
        seen += 1;
    }
    assert_eq!(seen, 21, "the recordings: add a test for a new one");
}

#[test]
fn a_dump_env_step_and_a_grandchild_dump_env_step_write_the_environment_each_process_has() {
    let root = TempRoot::new();
    let child = root.join("child-env.json");
    let grandchild = root.join("grandchild-env.json");
    let path = script(
        &root,
        &[
            &json!({"dump_env": child}).to_string(),
            &json!({"grandchild_dump_env": grandchild}).to_string(),
        ],
    );
    let mut fake = FakeClaude::start(&root, &path);
    fake.close_stdin();
    assert_eq!(fake.finish().code, Some(0));
    for dumped in [&child, &grandchild] {
        let env: Value = serde_json::from_str(&fs::read_to_string(dumped).unwrap()).unwrap();
        assert!(env.get("HOME").is_some(), "{dumped:?}: {env}");
    }
}

// ---- session-start hooks (ts-0068-wx) ------------------------------------------------------------------------------

fn settings_with_hook(root: &TempRoot, command: &str) -> String {
    let path = root.join("settings.json");
    let settings =
        json!({"hooks": {"SessionStart": [{"hooks": [{"type": "command", "command": command}]}]}});
    fs::write(&path, settings.to_string()).unwrap();
    path.to_str().unwrap().to_string()
}

#[test]
fn with_emulate_hooks_it_runs_the_settings_session_start_hook_before_its_first_step() {
    let root = TempRoot::new();
    let settings = settings_with_hook(&root, "echo marker >&2");
    let path = script(&root, &[r#"{"say": {"type": "after"}}"#]);
    let mut fake =
        FakeClaude::start_as_claude(&root, &path, true, &["-p", "--settings", &settings]);
    fake.close_stdin();
    let lines = all_lines(&mut fake);
    assert_eq!(lines[0]["subtype"], "hook_started");
    assert_eq!(lines[0]["hook_event"], "SessionStart");
    assert_eq!(lines[1]["subtype"], "hook_response");
    assert_eq!(lines[1]["stderr"], "marker\n");
    assert_eq!(lines[1]["stdout"], "");
    assert_eq!(lines[1]["outcome"], "success");
    assert_eq!(lines[2], json!({"type": "after"}));
    assert_eq!(fake.finish().code, Some(0));
}

#[test]
fn a_hook_that_fails_is_reported_as_an_error_with_its_exit_code() {
    let root = TempRoot::new();
    let settings = settings_with_hook(&root, "exit 3");
    let path = script(&root, &[]);
    let mut fake = FakeClaude::start_as_claude(&root, &path, true, &["--settings", &settings]);
    fake.close_stdin();
    let lines = all_lines(&mut fake);
    assert_eq!(lines[1]["outcome"], "error");
    assert_eq!(lines[1]["exit_code"], 3);
}

#[test]
fn without_a_settings_word_or_without_emulate_hooks_no_hook_runs() {
    let root = TempRoot::new();
    let settings = settings_with_hook(&root, "echo marker >&2");
    let path = script(&root, &[r#"{"say": {"type": "after"}}"#]);
    for (emulate, words) in [(true, vec!["-p"]), (false, vec!["--settings", &settings])] {
        let mut fake = FakeClaude::start_as_claude(&root, &path, emulate, &words);
        fake.close_stdin();
        assert_eq!(all_lines(&mut fake), vec![json!({"type": "after"})]);
        assert_eq!(fake.finish().code, Some(0));
    }
}
