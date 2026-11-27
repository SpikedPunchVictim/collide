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
const CORE_OUTLINE_TAGS: [(&str, &str); 8] = [
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
const HOST_OUTLINE_TAGS: [(&str, &str); 4] = [
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
const FILLED_TAGS: [(&str, &str, &str); 48] = [
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
    ("daemon: launchd job", "3a", "ts-0058-ad"),
    ("stop: find tree", "3a", "ts-0059-xs"),
    ("stop: escalation", "3a", "ts-0059-xs"),
    ("cli: stop", "3a", "ts-0059-xs"),
    ("launch: capability refusal", "3b", "ts-0060-mm"),
    ("resolve: model alias", "3b", "ts-0060-mm"),
    ("claude: capabilities", "3b", "ts-0061-bg"),
    ("claude: launch line", "3b", "ts-0061-bg"),
    ("claude: session start", "3b", "ts-0062-rj"),
    ("claude: stream parser", "3b", "ts-0062-rj"),
    ("claude: turn input", "3b", "ts-0062-rj"),
    ("claude: replay", "3b", "ts-0062-rj"),
    ("sampler: tick and sleep", "3b", "ts-0067-kw"),
    ("claude: version check", "3b", "ts-0068-wx"),
    ("claude: smoke check", "3b", "ts-0068-wx"),
    ("claude: settings canary", "3b", "ts-0068-wx"),
    ("cli: run", "3b", "ts-0069-bn"),
    ("shell: daemon connection", "3c", "ts-0077-ws"),
    ("shell: daemon events", "3c", "ts-0077-ws"),
    ("cli: config", "4a", "ts-0099-bw"),
    ("cli: pack", "4a", "ts-0101-bd"),
    ("supervisor: provider start", "4b", "ts-0108-dq"),
    ("supervisor: restart with backoff", "4b", "ts-0108-dq"),
    ("supervisor: provider calls", "4b", "ts-0108-dq"),
    ("sdk: ctx", "4b", "ts-0109-qj"),
    ("sdk: defineHook", "4b", "ts-0109-qj"),
    ("ui_tools: endpoint token store", "6b", "ts-0199-xz"),
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

/// The tags of the rest of the `host` outlines (ts-0041-xf) and the stage that fills each one: the hook executor and
/// `collide report` Stage 7, observers Stage 12, the resource sources Stage 13. (The provider supervisor's tags were
/// Stage 4b; they are in [`FILLED_TAGS`].) Each must count at least one stub today.
const HOST_ADAPTER_EXECUTOR_TAGS: [(&str, &str); 8] = [
    ("hooks: invocation", "7"),
    ("hooks: result file", "7"),
    ("supervisor: observers", "12"),
    ("sampler: process tree", "13"),
    ("sampler: network", "13"),
    ("sampler: gpu", "13"),
    ("sampler: containers", "13"),
    ("cli: report", "7"),
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

/// The tags of the shell's `rpc` command and event channel (ts-0017-qb) and the stage that fills each one: the minimal
/// shell of Stage 3c connects to the daemon and carries its notices to the window (`ts-0077-ws`, so those two tags are
/// in
/// [`FILLED_TAGS`]), and Stage 6b adds the ask token. It must count at least one stub today.
const SHELL_TAGS: [(&str, &str); 1] = [("shell: ask token", "6b")];

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

/// The `.rs` files under `dir`, in path order.
fn rust_files(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            found.extend(rust_files(&path));
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            found.push(path);
        }
    }
    found
}

/// A test that runs the `collide` binary with the environment it was started with reaches the person's real home and
/// daemon as soon as the command it runs is built: a stub check of `collide config set` did, once `set` was filled
/// (ts-0099-bw). So every place in `host/tests` that starts the binary clears the environment in the same command, and
/// gives the process a `HOME` and `COLLIDE_DAEMON_ROOT` of the test's own.
#[test]
fn every_test_that_runs_the_collide_binary_clears_its_environment() {
    let start = ["Command::new(env!(\"CARGO_BIN_EXE_collide", "\"))"].concat();
    let mut found = Vec::new();
    for file in rust_files(&repo().join("host").join("tests")) {
        let text = std::fs::read_to_string(&file).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        for (index, line) in lines.iter().enumerate() {
            if !line.contains(&start) {
                continue;
            }
            let command = lines[index..].iter().take_while(|line| !line.contains(';'));
            if !command.clone().any(|line| line.contains(".env_clear()")) {
                let rel = file.strip_prefix(repo()).unwrap().display().to_string();
                found.push(format!("{rel}: the command started at line {}", index + 1));
            }
        }
    }
    assert!(
        found.is_empty(),
        "these start `collide` with the test's own environment; clear it and set `HOME` and \
         `COLLIDE_DAEMON_ROOT` to a temp root:\n{}",
        found.join("\n")
    );
}
