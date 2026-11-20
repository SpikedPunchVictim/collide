//! The version read, its cache, and the smoke check, with fake Claude as the binary (ts-0068-wx; 0.3, A tested
//! version range). Each test runs its processes with an environment of its own (`env_clear()`, then what the test
//! sets), in a temp root, and never reads the owner's files.

use super::common::{Grandchild, TempRoot};
use super::fake::{BinarySpec, ClaudeBinary, recording};
use collide_host::adapters::claude::version::{
    ClaudeVersion, SmokeOutcome, VersionCache, VersionError, binary_key, read_version, smoke_check,
    smoke_check_within,
};
use std::collections::BTreeMap;
use std::fs;
use std::time::Duration;

fn env(root: &TempRoot) -> BTreeMap<String, String> {
    BTreeMap::from([
        ("PATH".to_string(), "/usr/bin:/bin".to_string()),
        (
            "HOME".to_string(),
            root.path().to_str().unwrap().to_string(),
        ),
    ])
}

fn binary(root: &TempRoot, version: &str, recording_name: &str) -> ClaudeBinary {
    ClaudeBinary::write(
        root.path(),
        &BinarySpec::new(version, &recording(recording_name)),
    )
}

#[test]
fn the_version_is_what_the_binary_prints_for_dash_dash_version() {
    let root = TempRoot::new();
    let claude = binary(&root, "2.1.324", "report");
    assert_eq!(
        read_version(&claude.path, &env(&root)),
        Ok(ClaudeVersion::new(2, 1, 324))
    );
    assert_eq!(claude.calls(), ["--version"]);
}

#[test]
fn a_binary_that_prints_no_version_is_unreadable() {
    let root = TempRoot::new();
    let claude = binary(&root, "banana", "report");
    assert!(matches!(
        read_version(&claude.path, &env(&root)),
        Err(VersionError::Unreadable { .. })
    ));
}

#[test]
fn a_binary_that_cannot_be_started_names_itself_in_the_error() {
    let root = TempRoot::new();
    let missing = root.join("no-such-claude");
    let Err(VersionError::NotRun { message }) = read_version(&missing, &env(&root)) else {
        panic!("expected NotRun")
    };
    assert!(message.contains("no-such-claude"), "{message}");
}

#[test]
fn a_binary_that_exits_nonzero_is_not_run_and_the_error_says_how() {
    let root = TempRoot::new();
    let path = root.script("claude", "#!/bin/sh\necho broken >&2\nexit 7\n");
    let Err(VersionError::NotRun { message }) = read_version(&path, &env(&root)) else {
        panic!("expected NotRun")
    };
    assert!(
        message.contains("code 7") && message.contains("broken"),
        "{message}"
    );
}

#[test]
fn the_binary_gets_exactly_the_environment_it_is_given() {
    let root = TempRoot::new();
    let dump = root.join("env.txt");
    let path = root.script(
        "claude",
        &format!("#!/bin/sh\nenv > {}\necho 2.1.325\n", dump.display()),
    );
    read_version(
        &path,
        &BTreeMap::from([("ONLY".to_string(), "this".to_string())]),
    )
    .unwrap();
    let seen = fs::read_to_string(dump).unwrap();
    assert!(seen.contains("ONLY=this"), "{seen}");
    assert!(!seen.contains("HOME="), "{seen}");
}

#[test]
fn a_binary_that_hangs_is_ended_and_reported_not_waited_for() {
    let root = TempRoot::new();
    let path = root.script("claude", "#!/bin/sh\nexec sleep 60\n");
    let started = std::time::Instant::now();
    let result = collide_host::adapters::claude::version::read_version_within(
        &path,
        &env(&root),
        Duration::from_millis(300),
    );
    assert!(started.elapsed() < Duration::from_secs(10));
    let Err(VersionError::NotRun { message }) = result else {
        panic!("expected NotRun")
    };
    assert!(message.contains("did not end"), "{message}");
}

// ---- the cache ---------------------------------------------------------------------------------------------------

#[test]
fn a_binary_is_asked_its_version_once_however_often_it_is_read() {
    let root = TempRoot::new();
    let claude = binary(&root, "2.1.325", "report");
    let cache = VersionCache::new();
    for _ in 0..3 {
        let (version, key) = cache.version(&claude.path, &env(&root)).unwrap();
        assert_eq!(version, ClaudeVersion::new(2, 1, 325));
        assert_eq!(key, binary_key(&claude.path).unwrap());
    }
    assert_eq!(claude.version_reads(), 1);
}

#[test]
fn a_binary_whose_modification_time_changed_is_asked_again() {
    let root = TempRoot::new();
    let claude = binary(&root, "2.1.325", "report");
    let cache = VersionCache::new();
    cache.version(&claude.path, &env(&root)).unwrap();
    // An upgrade in place: the same path, a new file.
    let upgraded = binary(&root, "2.1.326", "report");
    let later = std::time::SystemTime::now() + Duration::from_secs(5);
    fs::File::options()
        .write(true)
        .open(&upgraded.path)
        .unwrap()
        .set_modified(later)
        .unwrap();
    let (version, _) = cache.version(&claude.path, &env(&root)).unwrap();
    assert_eq!(version, ClaudeVersion::new(2, 1, 326));
    assert_eq!(claude.version_reads(), 2);
}

#[test]
fn a_failed_read_is_not_cached_so_the_next_call_tries_again() {
    let root = TempRoot::new();
    let claude = binary(&root, "banana", "report");
    let cache = VersionCache::new();
    assert!(cache.version(&claude.path, &env(&root)).is_err());
    assert!(cache.version(&claude.path, &env(&root)).is_err());
    assert_eq!(claude.version_reads(), 2);
}

#[test]
fn a_path_that_does_not_exist_is_an_error_not_a_panic() {
    let root = TempRoot::new();
    let cache = VersionCache::new();
    assert!(matches!(
        cache.version(&root.join("none"), &env(&root)),
        Err(VersionError::NotRun { .. })
    ));
}

// ---- the smoke check ---------------------------------------------------------------------------------------------

fn smoke(root: &TempRoot, recording_name: &str) -> (SmokeOutcome, ClaudeBinary) {
    let claude = binary(root, "2.2.0", recording_name);
    let work = root.join("work");
    fs::create_dir_all(&work).unwrap();
    (smoke_check(&claude.path, &work, &env(root)), claude)
}

#[test]
fn a_turn_that_ends_with_a_report_passes_the_smoke_check() {
    let root = TempRoot::new();
    let (outcome, claude) = smoke(&root, "report");
    assert_eq!(outcome, SmokeOutcome::Passed);
    assert_eq!(claude.smoke_runs(), 1);
}

#[test]
fn the_smoke_line_is_the_phases_fixed_words_with_the_cheap_model_a_budget_and_no_user_settings() {
    let root = TempRoot::new();
    let (_, claude) = smoke(&root, "report");
    let call = claude.calls().remove(0);
    for word in [
        "-p",
        "--input-format stream-json",
        "--output-format stream-json",
        "--verbose",
        "--include-hook-events",
        "--permission-prompt-tool stdio",
        "--strict-mcp-config",
        "--model haiku",
        "--max-budget-usd 0.05",
        "--no-session-persistence",
    ] {
        assert!(call.contains(word), "{word} in {call}");
    }
}

#[test]
fn a_turn_that_ends_in_an_error_result_fails_and_says_how_it_ended() {
    let root = TempRoot::new();
    let (SmokeOutcome::Failed { reason }, _) = smoke(&root, "error-result") else {
        panic!("expected a failure")
    };
    assert!(reason.contains("not `end_turn`"), "{reason}");
}

#[test]
fn a_refusal_fails_the_smoke_check() {
    let root = TempRoot::new();
    let (outcome, _) = smoke(&root, "refusal");
    assert!(
        matches!(outcome, SmokeOutcome::Failed { .. }),
        "{outcome:?}"
    );
}

#[test]
fn a_process_that_dies_with_no_result_fails_with_its_stderr() {
    let root = TempRoot::new();
    let (SmokeOutcome::Failed { reason }, _) = smoke(&root, "exit-code") else {
        panic!("expected a failure")
    };
    assert!(reason.contains("no `result` line"), "{reason}");
    assert!(reason.contains("the model connection was lost"), "{reason}");
}

#[test]
fn a_stream_of_another_shape_fails_the_smoke_check_and_does_not_panic() {
    let root = TempRoot::new();
    let path = root.script(
        "claude",
        "#!/bin/sh\ncat > /dev/null\necho '{\"type\":\"new_kind\"}'\necho not json\n",
    );
    let outcome = smoke_check(&path, root.path(), &env(&root));
    assert!(
        matches!(outcome, SmokeOutcome::Failed { .. }),
        "{outcome:?}"
    );
}

#[test]
fn a_binary_that_cannot_be_started_fails_the_smoke_check() {
    let root = TempRoot::new();
    let outcome = smoke_check(&root.join("none"), root.path(), &env(&root));
    assert!(
        matches!(outcome, SmokeOutcome::Failed { .. }),
        "{outcome:?}"
    );
}

#[test]
fn a_turn_that_never_ends_fails_within_the_limit_and_leaves_no_child() {
    let root = TempRoot::new();
    let pid_file = root.join("pid");
    let path = root.script(
        "claude",
        &format!(
            "#!/bin/sh\necho $$ > {}\nexec sleep 60\n",
            pid_file.display()
        ),
    );
    let started = std::time::Instant::now();
    let outcome = smoke_check_within(&path, root.path(), &env(&root), Duration::from_millis(500));
    assert!(started.elapsed() < Duration::from_secs(10));
    let SmokeOutcome::Failed { reason } = outcome else {
        panic!("expected a failure")
    };
    assert!(reason.contains("did not end"), "{reason}");
    let pid: u32 = fs::read_to_string(&pid_file)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    super::common::wait_until("the child is gone", Duration::from_secs(5), || {
        !super::common::pid_alive(pid)
    });
}

#[test]
fn a_grandchild_that_holds_the_pipes_does_not_hold_the_check() {
    let root = TempRoot::new();
    let path = root.script("claude", "#!/bin/sh\nsleep 5 &\nexec sleep 60\n");
    let started = std::time::Instant::now();
    let outcome = smoke_check_within(&path, root.path(), &env(&root), Duration::from_millis(300));
    assert!(
        matches!(outcome, SmokeOutcome::Failed { .. }),
        "{outcome:?}"
    );
    assert!(
        started.elapsed() < Duration::from_secs(4),
        "{:?}",
        started.elapsed()
    );
}

// ---- the gate a phase passes -------------------------------------------------------------------------------------

mod gate {
    use super::*;
    use collide_host::claude_versions::{ClaudeVersions, Gate, NO_SOURCE, VersionsOptions};

    fn versions(root: &TempRoot) -> std::sync::Arc<ClaudeVersions> {
        ClaudeVersions::new(VersionsOptions::new(
            Some(root.path().to_str().unwrap().to_string()),
            std::sync::Arc::new(|_| {}),
        ))
    }

    fn outside(root: &TempRoot, smoke: &str) -> ClaudeBinary {
        let phase = recording("report");
        let smoke = recording(smoke);
        ClaudeBinary::write(
            root.path(),
            &BinarySpec {
                smoke: Some(&smoke),
                ..BinarySpec::new("2.2.0", &phase)
            },
        )
    }

    #[test]
    fn a_version_in_the_range_starts_with_no_smoke_check() {
        let root = TempRoot::new();
        let claude = binary(&root, "2.1.324", "report");
        let versions = versions(&root);
        let gate = versions.gate(&claude.path, root.path(), &env(&root));
        assert!(
            matches!(gate, Gate::Ready { version, .. } if version == ClaudeVersion::new(2, 1, 324))
        );
        assert_eq!(claude.smoke_runs(), 0);
        let status = versions.status().agents.remove(0);
        assert_eq!(status.in_use.as_deref(), Some("2.1.324"));
        assert_eq!(status.in_tested_range, Some(true));
        assert_eq!(status.smoke, None);
    }

    #[test]
    fn a_version_outside_the_range_is_smoke_checked_once_and_a_pass_marks_the_chip() {
        let root = TempRoot::new();
        let claude = outside(&root, "report");
        let versions = versions(&root);
        for _ in 0..3 {
            let gate = versions.gate(&claude.path, root.path(), &env(&root));
            assert!(matches!(gate, Gate::Ready { .. }), "{gate:?}");
        }
        assert_eq!(claude.smoke_runs(), 1, "a pass is kept for the binary");
        assert_eq!(claude.version_reads(), 1);
        let status = versions.status().agents.remove(0);
        assert_eq!(status.in_tested_range, Some(false));
        let smoke = status.smoke.expect("the chip is marked");
        assert!(smoke.passed);
        assert_eq!(smoke.reason, None);
        assert!(smoke.at_ms > 0);
    }

    #[test]
    fn a_failed_smoke_check_refuses_naming_the_version_and_the_range_and_is_tried_again() {
        let root = TempRoot::new();
        let claude = outside(&root, "refusal");
        let versions = versions(&root);
        let Gate::Refused { version, message } =
            versions.gate(&claude.path, root.path(), &env(&root))
        else {
            panic!("expected a refusal")
        };
        assert_eq!(version, ClaudeVersion::new(2, 2, 0));
        assert!(message.contains("2.2.0"), "{message}");
        assert!(message.contains("2.1.323 to 2.1.325"), "{message}");
        assert!(message.contains("smoke check failed"), "{message}");
        let status = versions.status().agents.remove(0);
        let smoke = status.smoke.expect("the failure is shown");
        assert!(!smoke.passed);
        assert!(smoke.reason.is_some());
        versions.gate(&claude.path, root.path(), &env(&root));
        assert_eq!(claude.smoke_runs(), 2, "a failure is not kept");
    }

    #[test]
    fn a_binary_that_prints_no_version_is_unreadable_and_the_status_says_why() {
        let root = TempRoot::new();
        let claude = binary(&root, "banana", "report");
        let versions = versions(&root);
        let gate = versions.gate(&claude.path, root.path(), &env(&root));
        assert!(matches!(gate, Gate::Unreadable { .. }), "{gate:?}");
        let status = versions.status().agents.remove(0);
        assert_eq!(status.in_use, None);
        assert!(status.in_use_error.is_some());
        assert_eq!(status.in_tested_range, None);
    }

    #[test]
    fn the_status_names_the_tested_range() {
        let root = TempRoot::new();
        let status = versions(&root).status().agents.remove(0);
        assert_eq!(status.agent, "claude");
        assert_eq!(status.tested.from, "2.1.323");
        assert_eq!(status.tested.through, "2.1.325");
        let published = status.published.expect("no source: the chip says so");
        assert_eq!(published.latest, None);
        assert_eq!(published.error.as_deref(), Some(NO_SOURCE));
    }
}

#[test]
fn a_version_whose_stdout_a_grandchild_holds_open_is_not_read_as_empty() {
    let root = TempRoot::new();
    let pid_file = root.join("pid");
    let mut sleeper = Grandchild::new(pid_file.clone());
    let path = root.script(
        "claude",
        &format!(
            "#!/bin/sh\n/bin/sleep 30 2>/dev/null &\necho $! > {}\necho '2.1.324 (Claude Code)'\n",
            pid_file.display()
        ),
    );
    let result = read_version(&path, &env(&root));
    sleeper.pid(Duration::from_secs(10));
    let Err(VersionError::NotRun { message }) = result else {
        panic!("expected NotRun, got {result:?}")
    };
    assert!(message.contains("not read in time"), "{message}");
}

#[test]
fn a_smoke_check_whose_stdout_a_grandchild_holds_open_says_so() {
    let root = TempRoot::new();
    let pid_file = root.join("pid");
    let mut sleeper = Grandchild::new(pid_file.clone());
    let path = root.script(
        "claude",
        &format!(
            "#!/bin/sh\n/bin/sleep 30 2>/dev/null &\necho $! > {}\n",
            pid_file.display()
        ),
    );
    let outcome = smoke_check_within(&path, root.path(), &env(&root), Duration::from_secs(20));
    sleeper.pid(Duration::from_secs(10));
    let SmokeOutcome::Failed { reason } = outcome else {
        panic!("expected a failure")
    };
    assert!(reason.contains("not read in time"), "{reason}");
}
