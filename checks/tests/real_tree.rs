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

/// The tags of the `host` outlines (ts-0040-kd) and the stage that fills each
/// one: the daemon and the shim are Stage 3a, the OTLP receiver is Stage 9b.
/// Each must count at least one stub today. A tag whose item has replaced its last stub moves to [`FILLED_TAGS`].
const HOST_OUTLINE_TAGS: [(&str, &str); 5] = [
    ("daemon: launchd job", "3a"),
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

/// The tags whose item has replaced every stub, with the stage they belong to and the item that filled them. Each
/// stays in `stub-tags.json` and counts zero: that is how CI shows a stage complete (0.15), and a stub that comes back
/// under a filled tag fails here.
const FILLED_TAGS: [(&str, &str, &str); 21] = [
    ("clock: system readings", "3a", "ts-0051-wh"),
    ("clock: timer", "3a", "ts-0051-wh"),
    ("clock: sleep gap", "3a", "ts-0051-wh"),
    ("auth: peer credentials", "3a", "ts-0052-xt"),
    ("auth: process tree", "3a", "ts-0052-xt"),
    ("store: open and migrate", "3a", "ts-0053-gn"),
    ("store: event log", "3a", "ts-0053-gn"),
    ("store: quarantine", "3a", "ts-0053-gn"),
    ("shim: entry", "3a", "ts-0054-cy"),
    ("shim: spawn child", "3a", "ts-0054-cy"),
    ("shim: reap", "3a", "ts-0054-cy"),
    ("shim: journal writer", "3a", "ts-0054-cy"),
    ("shim: exit record", "3a", "ts-0054-cy"),
    ("shim: pty ring", "3a", "ts-0054-cy"),
    ("daemon: entry", "3a", "ts-0056-ag"),
    ("daemon: socket server", "3a", "ts-0056-ag"),
    ("daemon: lock file start", "3a", "ts-0056-ag"),
    ("daemon: setsid start", "3a", "ts-0056-ag"),
    ("daemon: base environment", "3a", "ts-0056-ag"),
    ("shim: daemon link", "3a", "ts-0057-ck"),
    ("daemon: adopt shims", "3a", "ts-0057-ck"),
];

#[test]
fn every_filled_tag_stays_registered_under_its_stage_and_counts_no_stub() {
    let report = stubs::check(&repo()).unwrap();
    for (tag, stage, item) in FILLED_TAGS {
        assert_eq!(
            report.stages.get(tag).map(String::as_str),
            Some(stage),
            "tag \"{tag}\" (filled by {item}) is not registered under stage {stage}"
        );
        assert_eq!(
            report.counts.get(tag).copied().unwrap_or(0),
            0,
            "tag \"{tag}\" was filled by {item}, but a stub carries it again"
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

/// The tag of the `ui_tools` request handler (ts-0035-re) and the stage that fills it: the endpoint token's place in the
/// OS keychain is Stage 6b, with the listener. It must count at least one stub today.
const UI_TOOLS_TAGS: [(&str, &str); 1] = [("ui_tools: endpoint token store", "6b")];

#[test]
fn every_ui_tools_tag_names_its_stage_and_counts_at_least_one_stub() {
    let report = stubs::check(&repo()).unwrap();
    for (tag, stage) in UI_TOOLS_TAGS {
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

/// The tags of the shell's `rpc` command and event channel (ts-0017-qb) and the stage that fills each one: the minimal
/// shell of Stage 3c connects to the daemon and carries its notices to the window, and Stage 6b adds the ask token. Each
/// must count at least one stub today.
const SHELL_TAGS: [(&str, &str); 3] = [
    ("shell: daemon connection", "3c"),
    ("shell: daemon events", "3c"),
    ("shell: ask token", "6b"),
];

#[test]
fn every_shell_tag_names_its_stage_and_counts_at_least_one_stub() {
    let report = stubs::check(&repo()).unwrap();
    for (tag, stage) in SHELL_TAGS {
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
