//! The three checks over this repository itself. They run with `cargo test --workspace`, so a change that breaks one
//! fails the same test run as the unit tests; CI also runs each one on its own to print the stub counts.

use collide_checks::{deps, names, stubs};
use std::path::{Path, PathBuf};

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

#[test]
fn no_name_check_passes_on_the_repository() {
    let found = names::check(&repo()).unwrap();
    assert!(
        found.is_empty(),
        "{}",
        found
            .iter()
            .map(|v| v.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn core_has_no_tauri_sqlite_or_process_dependency() {
    let found = deps::check(&repo()).unwrap();
    assert!(found.is_empty(), "{}", found.join("\n"));
}

#[test]
fn every_stub_of_the_repository_is_registered_under_a_stage() {
    let report = stubs::check(&repo()).unwrap();
    assert!(report.problems.is_empty(), "{}", report.problems.join("\n"));
}

/// The tags of the `core` outlines (ts-0019-vm) and the stage that fills each one. A tag is not complete until its
/// stage has replaced every stub that carries it, so each must count at least one stub today.
const CORE_OUTLINE_TAGS: [(&str, &str); 10] = [
    ("launch: capability refusal", "3b"),
    ("resolve: model alias", "3b"),
    ("resolve: implementations", "7"),
    ("engine: run preview", "8"),
    ("engine: start run", "8"),
    ("engine: transition", "8"),
    ("engine: fan-out join", "8"),
    ("engine: read-only check", "8"),
    ("engine: adopt run", "8"),
    ("engine: workflow ops", "8"),
];

#[test]
fn every_core_outline_tag_names_its_stage_and_counts_at_least_one_stub() {
    let report = stubs::check(&repo()).unwrap();
    for (tag, stage) in CORE_OUTLINE_TAGS {
        assert_eq!(
            report.stages.get(tag).map(String::as_str),
            Some(stage),
            "tag \"{tag}\" is not registered under stage {stage}"
        );
        assert!(
            report.counts[tag] >= 1,
            "tag \"{tag}\" is registered but no stub carries it"
        );
    }
}

/// The tags of the `host` outlines (ts-0040-kd, with the real peer lookups of ts-0033-zj) and the stage that fills each
/// one: the daemon, the peer and process lookups, the shim, the store and the clocks are Stage 3a, the OTLP receiver
/// is Stage 9b. Each must count at least one stub today.
const HOST_OUTLINE_TAGS: [(&str, &str); 26] = [
    ("daemon: entry", "3a"),
    ("daemon: lock file start", "3a"),
    ("daemon: launchd job", "3a"),
    ("daemon: setsid start", "3a"),
    ("daemon: base environment", "3a"),
    ("daemon: socket server", "3a"),
    ("auth: peer credentials", "3a"),
    ("auth: process tree", "3a"),
    ("daemon: adopt shims", "3a"),
    ("shim: entry", "3a"),
    ("shim: spawn child", "3a"),
    ("shim: reap", "3a"),
    ("shim: journal writer", "3a"),
    ("shim: exit record", "3a"),
    ("shim: pty ring", "3a"),
    ("shim: daemon link", "3a"),
    ("store: open and migrate", "3a"),
    ("store: event log", "3a"),
    ("store: quarantine", "3a"),
    ("clock: system readings", "3a"),
    ("clock: timer", "3a"),
    ("clock: sleep gap", "3a"),
    ("otlp: receiver", "9b"),
    ("otlp: run token", "9b"),
    ("otlp: request check", "9b"),
    ("otlp: drop identity", "9b"),
];

#[test]
fn every_host_outline_tag_names_its_stage_and_counts_at_least_one_stub() {
    let report = stubs::check(&repo()).unwrap();
    for (tag, stage) in HOST_OUTLINE_TAGS {
        assert_eq!(
            report.stages.get(tag).map(String::as_str),
            Some(stage),
            "tag \"{tag}\" is not registered under stage {stage}"
        );
        assert!(
            report.counts[tag] >= 1,
            "tag \"{tag}\" is registered but no stub carries it"
        );
    }
}

/// The tags of the rest of the `host` outlines (ts-0041-xf) and the stage that fills each one: the Claude adapter, its
/// version checks and the sampler's tick are Stage 3b, `collide stop` and the stop order Stage 3a, the provider
/// supervisor and the config and pack commands Stage 4b, the hook executor and `collide report` Stage 7, observers
/// Stage 12, the resource sources Stage 13. Each must count at least one stub today.
const HOST_ADAPTER_EXECUTOR_TAGS: [(&str, &str); 27] = [
    ("claude: capabilities", "3b"),
    ("claude: launch line", "3b"),
    ("claude: session start", "3b"),
    ("claude: stream parser", "3b"),
    ("claude: turn input", "3b"),
    ("claude: replay", "3b"),
    ("claude: version check", "3b"),
    ("claude: smoke check", "3b"),
    ("claude: settings canary", "3b"),
    ("hooks: invocation", "7"),
    ("hooks: result file", "7"),
    ("stop: find tree", "3a"),
    ("stop: escalation", "3a"),
    ("supervisor: provider start", "4b"),
    ("supervisor: restart with backoff", "4b"),
    ("supervisor: provider calls", "4b"),
    ("supervisor: observers", "12"),
    ("sampler: tick and sleep", "3b"),
    ("sampler: process tree", "13"),
    ("sampler: network", "13"),
    ("sampler: gpu", "13"),
    ("sampler: containers", "13"),
    ("cli: run", "3b"),
    ("cli: stop", "3a"),
    ("cli: report", "7"),
    ("cli: config", "4b"),
    ("cli: pack", "4b"),
];

#[test]
fn every_adapter_executor_supervisor_sampler_and_cli_tag_names_its_stage_and_counts_at_least_one_stub()
 {
    let report = stubs::check(&repo()).unwrap();
    for (tag, stage) in HOST_ADAPTER_EXECUTOR_TAGS {
        assert_eq!(
            report.stages.get(tag).map(String::as_str),
            Some(stage),
            "tag \"{tag}\" is not registered under stage {stage}"
        );
        assert!(
            report.counts.get(tag).copied().unwrap_or(0) >= 1,
            "tag \"{tag}\" is registered but no stub carries it"
        );
    }
}
