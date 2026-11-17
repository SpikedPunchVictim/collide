//! Part 2 of ts-0103-sa: the candidate lines of `repo_instructions.rs` run once against the real `claude`, to see what
//! loads. Part 2 of ts-0106-nr (candidate 5, the snapshot plugin with the worktree's skills) is the last test here, on
//! its own terms: [`PLUGIN_CAP_USD`] a session and [`PLUGIN_LEDGER_LIMIT_USD`] in all. Every test here is skipped, with
//! the reason printed, unless `COLLIDE_REAL_CLAUDE=1` and a ledger (`COLLIDE_REAL_LEDGER`) are set, so `cargo test
//! --workspace` runs nothing real.
//!
//! **Terms (owner, 2026-11-16).** Haiku only; every session has `--max-budget-usd` of [`SESSION_CAP_USD`] or less on
//! its line; the ledger refuses a run that could take the item over [`ITEM_LEDGER_LIMIT_USD`]. The child has
//! `real::child_env()` (the owner's `HOME` and `USER`, for the login only) and `--setting-sources ""` (config none)
//! where Collide starts it, so the owner's settings, hooks, plugins and `CLAUDE.md` do not load. Project files are in
//! neither `user` nor `""`, so what loads from the worktree is the same. An agent's own `claude` has
//! `--setting-sources project,local` and no `--mcp-config`. The repositories, worktrees and markers are in a temp root.
//!
//! **What is recorded.** Only what concerns the fixture's files: loaded or not, and the stream line that shows it (a
//! word, a skill name, a marker file). Anything in a stream that comes from the owner's account or home is never
//! written down or printed: [`Findings`] holds booleans and counts. No raw stream is kept.
//!
//! The tests report what they measure and assert only that the session ran. Whether a file loaded is the measurement,
//! not a pass or a fail.

use super::real;
use super::repo_instructions::{
    AGENT_NAME, AGENT_WORD, Candidate, Fixture, IMPORT_WORD, LOCAL_HOOK_MARKER, LOCAL_HOOK_TEXT,
    Line, MCP_MARKER, MCP_NAME, NESTED_WORD, PLUGIN_NAME, PROJECT_HOOK_MARKER, PROJECT_HOOK_TEXT,
    ROOT_WORD, SKILL_NAME, SKILL_WORD, candidate_line, holds_only_the_skills_link,
};
use collide_core::adapter::{AgentAdapter, LaunchRequest};
use collide_host::adapters::claude::ClaudeAdapter;
use collide_host::adapters::claude::version::CANARY_MARKER;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

/// The cap of one session (`--max-budget-usd`), and the most the item's sessions may add up to.
pub const SESSION_CAP_USD: f64 = 0.10;
pub const ITEM_LEDGER_LIMIT_USD: f64 = 0.50;

/// The same for ts-0106-nr (owner, 2026-11-16): one haiku session capped at five cents, and ten cents in all.
pub const PLUGIN_CAP_USD: f64 = 0.05;
pub const PLUGIN_LEDGER_LIMIT_USD: f64 = 0.10;

/// How long a session may take before it is ended.
const SESSION_TIMEOUT: Duration = Duration::from_secs(180);

/// The one prompt of every session. The words it asks for are in the fixture's files and nowhere in the prompt.
pub const PROMPT: &str = "Use the Read tool on sub/notes.txt. Then reply with exactly one line and nothing else, in \
    this form: ROOT=<word>; NESTED=<word>; IMPORT=<word>; SKILL=<word>; MEMORY=<yes or no>. ROOT is the word in the \
    sentence about the root word, NESTED the word in the sentence about the sub word, IMPORT the word in the \
    sentence about the import word, and SKILL the skill word in the description of the skill named repo-canary-skill. \
    Write none for a word that is not in your instructions or skills. MEMORY is yes if your instructions describe a \
    persistent auto memory directory. Do not guess.";

macro_rules! live {
    () => {
        if let Some(reason) = unavailable() {
            println!("skipped: {reason}");
            return;
        }
    };
}

/// Why a live test cannot run, or `None` when it can: the opt-in of `real.rs`, and a ledger to add the spend up in.
pub fn unavailable() -> Option<String> {
    unavailable_within(ITEM_LEDGER_LIMIT_USD)
}

/// [`unavailable`] for an item whose ledger has the limit `limit`.
fn unavailable_within(limit: f64) -> Option<String> {
    real::unavailable().or_else(|| {
        std::env::var_os(real::LEDGER_ENV).is_none().then(|| {
            format!(
                "{} names no ledger (the item's spend is added up in one, with a limit of ${limit})",
                real::LEDGER_ENV
            )
        })
    })
}

// ---- the lines ---------------------------------------------------------------------------------------------------

/// The request of a session Collide starts: config none, haiku and a cap on the line.
pub fn collide_request(fixture: &Fixture, cwd: &Path) -> LaunchRequest {
    collide_request_capped(fixture, cwd, SESSION_CAP_USD)
}

/// [`collide_request`] with a cap of `cap` dollars on the line.
pub fn collide_request_capped(fixture: &Fixture, cwd: &Path, cap: f64) -> LaunchRequest {
    let mut request = fixture.request(json!({ "agent": "claude", "config": "none" }), cwd);
    request.model = Some(collide_core::agent::ModelRef {
        name: "haiku".into(),
        id: None,
    });
    request.budget_left_usd = Some(cap);
    request
}

/// The line `claude` is started with when an agent starts it itself in its worktree: project and local settings, no
/// user settings and no `--mcp-config`. The prompt is the last word.
pub fn agents_own_line(cwd: &Path) -> Line {
    let words = [
        "-p",
        "--output-format",
        "stream-json",
        "--verbose",
        "--include-hook-events",
        "--no-session-persistence",
        "--model",
        "haiku",
        "--max-budget-usd",
        &SESSION_CAP_USD.to_string(),
        "--setting-sources",
        "project,local",
        PROMPT,
    ];
    Line {
        argv: words.map(String::from).to_vec(),
        env: BTreeMap::new(),
        cwd: cwd.to_path_buf(),
    }
}

/// The first turn as Collide's session writes it on stdin.
fn first_turn(request: &LaunchRequest) -> Vec<u8> {
    let mut session = ClaudeAdapter.session(request);
    session.send_turn(PROMPT).writes.concat()
}

// ---- what a session leaves ---------------------------------------------------------------------------------------

/// What one real session left that concerns the fixture, and counts of the rest.
#[derive(Debug, Default, PartialEq)]
pub struct Findings {
    pub exit: Option<i32>,
    pub result: String,
    pub claude_version: String,
    /// The fixture's own names in the `init` line's lists, by list. `skills` is true for the skill's own name or the
    /// name with a plugin's prefix; `plugins` is true for the fixture's plugin.
    pub init_has: BTreeMap<&'static str, bool>,
    /// The entry of the `init` line's `skills` list that is the fixture's skill, as listed (the plugin's prefix, if
    /// any, is the fixture's own plugin name).
    pub skill_listed_as: Option<String>,
    /// How many entries each list of the `init` line has (the owner's own are in these, never named).
    pub init_counts: BTreeMap<&'static str, usize>,
    /// The fixture's words in the model's reply.
    pub reply_has: BTreeMap<&'static str, bool>,
    /// What the reply said to `MEMORY=`; `None` when it said neither.
    pub memory: Option<bool>,
    pub session_start_hooks: usize,
    /// The marker texts that a `SessionStart` hook response held, in this order: the snapshot's canary, the
    /// project hook, the local hook.
    pub hook_texts: Vec<&'static str>,
    pub markers: Vec<&'static str>,
    pub cost: f64,
}

/// Whether the list `key` of `init` has an entry that is `name` or an object named `name`.
fn lists(init: &Value, key: &str, name: &str) -> bool {
    init[key].as_array().is_some_and(|entries| {
        entries
            .iter()
            .any(|entry| entry.as_str() == Some(name) || entry["name"].as_str() == Some(name))
    })
}

/// The entry of the list `key` of `init` that is `name` or `<prefix>:name`, as listed. Only the fixture's name can
/// match, so only it is returned.
fn listed_as(init: &Value, key: &str, name: &str) -> Option<String> {
    let suffix = format!(":{name}");
    init[key]
        .as_array()?
        .iter()
        .filter_map(|entry| entry.as_str().or_else(|| entry["name"].as_str()))
        .find(|entry| *entry == name || entry.ends_with(&suffix))
        .map(str::to_string)
}

/// `MEMORY=yes` or `MEMORY=no` in `reply`.
fn memory_answer(reply: &str) -> Option<bool> {
    let after = reply.split("MEMORY=").nth(1)?.trim_start();
    let answer: String = after
        .chars()
        .take_while(|c| c.is_ascii_alphabetic())
        .collect::<String>()
        .to_lowercase();
    match answer.as_str() {
        "yes" => Some(true),
        "no" => Some(false),
        _ => None,
    }
}

/// What the stream's lines and the markers say. Only the fixture's names and words are looked for; the rest is counted.
pub fn findings(lines: &[Value], exit: Option<i32>, markers: Vec<&'static str>) -> Findings {
    let mut found = Findings {
        exit,
        markers,
        ..Findings::default()
    };
    let kind = |line: &Value, kind: &str, subtype: &str| {
        line["type"] == kind && line["subtype"] == subtype
    };
    if let Some(init) = lines.iter().find(|line| kind(line, "system", "init")) {
        found.claude_version = init["claude_code_version"]
            .as_str()
            .unwrap_or("")
            .to_string();
        for (key, name) in [
            ("agents", AGENT_NAME),
            ("mcp_servers", MCP_NAME),
            ("plugins", PLUGIN_NAME),
        ] {
            found.init_has.insert(key, lists(init, key, name));
        }
        found.skill_listed_as = listed_as(init, "skills", SKILL_NAME);
        found
            .init_has
            .insert("skills", found.skill_listed_as.is_some());
        for key in [
            "skills",
            "agents",
            "mcp_servers",
            "plugins",
            "slash_commands",
            "tools",
        ] {
            let count = init[key].as_array().map_or(0, Vec::len);
            found.init_counts.insert(key, count);
        }
        let added = init["additional_directories"]
            .as_array()
            .map_or(0, Vec::len);
        found.init_counts.insert("additional_directories", added);
    }
    for line in lines {
        if kind(line, "system", "hook_started") && line["hook_event"] == "SessionStart" {
            found.session_start_hooks += 1;
        }
        if kind(line, "system", "hook_response") && line["hook_event"] == "SessionStart" {
            let said = ["output", "stdout", "stderr"]
                .iter()
                .filter_map(|key| line[*key].as_str())
                .collect::<String>();
            for text in [CANARY_MARKER, PROJECT_HOOK_TEXT, LOCAL_HOOK_TEXT] {
                if said.contains(text) && !found.hook_texts.contains(&text) {
                    found.hook_texts.push(text);
                }
            }
        }
        if let Some(cost) = line["total_cost_usd"].as_f64() {
            found.cost = cost;
        }
    }
    if let Some(result) = lines.iter().rev().find(|line| line["type"] == "result") {
        found.result = result["subtype"].as_str().unwrap_or("").to_string();
        let reply = result["result"].as_str().unwrap_or("");
        for (key, word) in [
            ("root", ROOT_WORD),
            ("nested", NESTED_WORD),
            ("import", IMPORT_WORD),
            ("skill", SKILL_WORD),
            ("agent", AGENT_WORD),
        ] {
            found.reply_has.insert(key, reply.contains(word));
        }
        found.memory = memory_answer(reply);
    }
    found
}

impl Findings {
    /// One line of what is safe to print: booleans, counts and the fixture's own words.
    pub fn describe(&self) -> String {
        let yes = |loaded: bool| if loaded { "loaded" } else { "not loaded" };
        let has = |map: &BTreeMap<&'static str, bool>, key: &str| {
            yes(map.get(key).copied().unwrap_or(false))
        };
        let marker = |name: &str| {
            if self.markers.contains(&name) {
                "written"
            } else {
                "absent"
            }
        };
        format!(
            "claude {} exit {:?} result {} | words: root {}, nested {}, import {}, skill {}, agent {} | init lists: \
             skill {} (as {:?}), agent {}, mcp server {}, plugin {} | hooks: {} SessionStart, texts {:?}; markers: \
             project {}, local {}, \
             mcp {} | counts {:?} | memory {:?} | cost ${:.6}",
            self.claude_version,
            self.exit,
            self.result,
            has(&self.reply_has, "root"),
            has(&self.reply_has, "nested"),
            has(&self.reply_has, "import"),
            has(&self.reply_has, "skill"),
            has(&self.reply_has, "agent"),
            has(&self.init_has, "skills"),
            self.skill_listed_as,
            has(&self.init_has, "agents"),
            has(&self.init_has, "mcp_servers"),
            has(&self.init_has, "plugins"),
            self.session_start_hooks,
            self.hook_texts,
            marker(PROJECT_HOOK_MARKER),
            marker(LOCAL_HOOK_MARKER),
            marker(MCP_MARKER),
            self.init_counts,
            self.memory,
            self.cost,
        )
    }
}

// ---- one session -------------------------------------------------------------------------------------------------

/// Runs `line` once on the real binary, with the markers cleared first, `stdin` on its input, and the ledger asked
/// first and told after. Returns what it left. A session that cannot be run or does not end is a panic that names the
/// session only: nothing the binary wrote is in the message.
pub fn run_session(fixture: &Fixture, name: &str, line: &Line, stdin: &[u8]) -> Findings {
    run_session_within(
        fixture,
        name,
        line,
        stdin,
        SESSION_CAP_USD,
        ITEM_LEDGER_LIMIT_USD,
    )
}

/// [`run_session`] for an item with its own terms: a session of `cap` dollars at most and a ledger limit of `limit`.
/// Both guards are here, for every item: the opt-in, then the ledger.
pub fn run_session_within(
    fixture: &Fixture,
    name: &str,
    line: &Line,
    stdin: &[u8],
    cap: f64,
    limit: f64,
) -> Findings {
    // A second guard, here and not only in each test's `live!`: nothing reaches `claude` without the opt-in.
    if let Some(reason) = real::unavailable() {
        panic!("`{name}` is not run: {reason}");
    }
    real::check_ledger_within(name, cap, limit);
    fixture.clear_markers();
    let mut command = Command::new(real::binary());
    command
        .args(&line.argv)
        .current_dir(&line.cwd)
        .env_clear()
        .envs(real::child_env())
        .envs(&line.env);
    let ran = collide_host::bounded_run::run_bounded(command, Some(stdin), SESSION_TIMEOUT);
    let ran = match ran {
        Ok(ran) => ran,
        Err(_) => {
            // The spend of a session that did not end is not known, so the ledger takes its whole cap.
            real::add_to_ledger(name, cap, cap);
            panic!("`{name}`: claude did not end within {SESSION_TIMEOUT:?}");
        }
    };
    let lines: Vec<Value> = String::from_utf8_lossy(&ran.stdout)
        .lines()
        .filter_map(|text| serde_json::from_str(text).ok())
        .collect();
    let found = findings(&lines, ran.code, fixture.markers_present());
    real::add_to_ledger(name, cap, found.cost);
    println!("stderr: {} bytes", ran.stderr.len());
    println!("finding `{name}`: {}", found.describe());
    assert!(!lines.is_empty(), "`{name}`: claude wrote no stream line");
    assert_eq!(found.exit, Some(0), "`{name}`: claude ended with a failure");
    assert!(!found.result.is_empty(), "`{name}`: no result line");
    found
}

/// A session on candidate `candidate` in a worktree: Collide's own line.
fn collide_session(name: &str, candidate: Candidate) -> Findings {
    let fixture = Fixture::new();
    let worktree = fixture.worktree("wt");
    let request = collide_request(&fixture, &worktree);
    let line = candidate_line(&fixture, candidate, &request);
    println!("root: {}", fixture.root().path().display());
    let found = run_session(&fixture, name, &line, &first_turn(&request));
    assert!(
        found.hook_texts.contains(&CANARY_MARKER),
        "`{name}`: the snapshot's canary hook did not run"
    );
    found
}

// ---- the live tests ----------------------------------------------------------------------------------------------

#[test]
fn live_candidate_1_the_current_line_in_the_worktree() {
    live!();
    collide_session("candidate-1-baseline", Candidate::Baseline);
}

#[test]
fn live_candidate_2_add_dir_and_the_variable_in_the_worktree() {
    live!();
    collide_session("candidate-2-add-dir", Candidate::AddDir);
}

#[test]
fn live_candidate_3_the_instruction_text_as_an_appended_prompt_file() {
    live!();
    collide_session("candidate-3-appended-file", Candidate::AppendedPromptFile);
}

#[test]
fn live_the_persons_checkout_with_untracked_local_settings_and_mcp_config() {
    live!();
    let fixture = Fixture::new();
    let checkout = fixture.repo.work().to_path_buf();
    fixture.add_local_files(&checkout);
    let request = collide_request(&fixture, &checkout);
    let line = candidate_line(&fixture, Candidate::Baseline, &request);
    println!("root: {}", fixture.root().path().display());
    run_session(
        &fixture,
        "checkout-snapshot-line",
        &line,
        &first_turn(&request),
    );
}

#[test]
fn live_an_agents_own_claude_in_a_worktree_it_made_with_git_worktree_add() {
    live!();
    let fixture = Fixture::new();
    let worktree = fixture.worktree("agent-wt");
    let line = agents_own_line(&worktree);
    println!("root: {}", fixture.root().path().display());
    run_session(&fixture, "agent-own-claude", &line, b"");
}

/// ts-0106-nr: candidate 5, candidate 2 plus the snapshot plugin whose skills are the worktree's, in a worktree.
/// Collide's own line with a cap of [`PLUGIN_CAP_USD`], against the ledger limit [`PLUGIN_LEDGER_LIMIT_USD`].
#[test]
fn live_the_snapshot_plugin_with_the_worktrees_skills_in_the_worktree() {
    if let Some(reason) = unavailable_within(PLUGIN_LEDGER_LIMIT_USD) {
        println!("skipped: {reason}");
        return;
    }
    let fixture = Fixture::new();
    let worktree = fixture.worktree("wt");
    let request = collide_request_capped(&fixture, &worktree, PLUGIN_CAP_USD);
    let line = candidate_line(&fixture, Candidate::AddDirSkillsPlugin, &request);
    println!("root: {}", fixture.root().path().display());
    let name = "candidate-5-skills-plugin";
    let found = run_session_within(
        &fixture,
        name,
        &line,
        &first_turn(&request),
        PLUGIN_CAP_USD,
        PLUGIN_LEDGER_LIMIT_USD,
    );
    assert!(
        found.hook_texts.contains(&CANARY_MARKER),
        "`{name}`: the snapshot's canary hook did not run"
    );
}

// ---- offline: the harness itself ---------------------------------------------------------------------------------

/// The lines of a session the way `claude` writes them, made up: nothing in them is from a real session.
fn made_up_stream(skills: &[&str], reply: &str) -> Vec<Value> {
    vec![
        json!({"type": "system", "subtype": "init", "claude_code_version": "9.9.9",
            "skills": skills, "agents": ["general-purpose", AGENT_NAME],
            "mcp_servers": [{"name": MCP_NAME, "status": "connected"}, {"name": "other", "status": "failed"}],
            "plugins": [{"name": "p"}], "slash_commands": ["a", "b"], "tools": ["Read"],
            "additional_directories": ["/work"]}),
        json!({"type": "system", "subtype": "hook_started", "hook_event": "SessionStart"}),
        json!({"type": "system", "subtype": "hook_response", "hook_event": "SessionStart",
            "stderr": format!("{CANARY_MARKER}\n"), "stdout": "", "output": ""}),
        json!({"type": "system", "subtype": "hook_started", "hook_event": "SessionStart"}),
        json!({"type": "system", "subtype": "hook_response", "hook_event": "SessionStart",
            "stderr": format!("{PROJECT_HOOK_TEXT}\n"), "stdout": "", "output": ""}),
        json!({"type": "result", "subtype": "success", "total_cost_usd": 0.0123, "result": reply}),
    ]
}

#[test]
fn findings_read_the_init_lists_the_hooks_the_reply_and_the_cost() {
    let reply =
        format!("ROOT={ROOT_WORD}; NESTED=none; IMPORT={IMPORT_WORD}; SKILL=none; MEMORY=No");
    let lines = made_up_stream(&["x", SKILL_NAME], &reply);
    let found = findings(&lines, Some(0), vec![PROJECT_HOOK_MARKER]);
    assert_eq!(found.claude_version, "9.9.9");
    assert!(found.init_has["skills"]);
    assert!(found.init_has["agents"]);
    assert!(found.init_has["mcp_servers"]);
    assert_eq!(found.init_counts["mcp_servers"], 2);
    assert_eq!(found.init_counts["skills"], 2);
    assert_eq!(found.session_start_hooks, 2);
    assert_eq!(found.hook_texts, [CANARY_MARKER, PROJECT_HOOK_TEXT]);
    assert!(found.reply_has["root"]);
    assert!(!found.reply_has["nested"]);
    assert!(found.reply_has["import"]);
    assert!(!found.reply_has["skill"]);
    assert_eq!(found.memory, Some(false));
    assert_eq!(found.result, "success");
    assert_eq!(found.cost, 0.0123);
    assert_eq!(found.markers, [PROJECT_HOOK_MARKER]);
}

#[test]
fn findings_say_not_loaded_for_a_name_the_stream_does_not_hold() {
    let lines = made_up_stream(
        &["x"],
        "ROOT=none; NESTED=none; IMPORT=none; SKILL=none; MEMORY=maybe",
    );
    let found = findings(&lines, Some(0), Vec::new());
    assert!(!found.init_has["skills"]);
    assert_eq!(
        found.reply_has.values().filter(|loaded| **loaded).count(),
        0
    );
    assert_eq!(found.memory, None);
}

#[test]
fn what_findings_print_holds_no_name_from_the_stream_but_the_fixtures() {
    let lines = vec![
        json!({"type": "system", "subtype": "init", "claude_code_version": "9.9.9",
            "skills": ["owner-private-skill"], "agents": ["owner-private-agent"],
            "mcp_servers": [{"name": "owner-private-server"}], "plugins": [{"name": "owner-private-plugin"}],
            "slash_commands": ["/owner-private-command"], "cwd": "/home/owner-private-path"}),
        json!({"type": "result", "subtype": "success", "total_cost_usd": 0.01, "result": "owner-private-reply"}),
    ];
    let printed = findings(&lines, Some(0), Vec::new()).describe();
    assert!(!printed.contains("owner-private"), "{printed}");
    assert!(printed.contains("claude 9.9.9"), "{printed}");
}

#[test]
fn every_collide_line_of_part_2_is_haiku_capped_at_ten_cents_and_config_none() {
    let fixture = Fixture::new();
    let worktree = fixture.worktree("wt");
    let request = collide_request(&fixture, &worktree);
    for candidate in [
        Candidate::Baseline,
        Candidate::AddDir,
        Candidate::AppendedPromptFile,
    ] {
        let line = candidate_line(&fixture, candidate, &request);
        assert_eq!(line.value_of("--model"), Some("haiku"), "{candidate:?}");
        assert_eq!(
            line.value_of("--max-budget-usd"),
            Some("0.1"),
            "{candidate:?}"
        );
        assert_eq!(
            line.value_of("--setting-sources"),
            Some(""),
            "{candidate:?}"
        );
        assert!(line.argv.iter().any(|word| word == "--strict-mcp-config"));
    }
}

#[test]
fn an_agents_own_line_has_project_and_local_sources_and_no_user_source_and_no_mcp_config() {
    let fixture = Fixture::new();
    let worktree = fixture.worktree("wt");
    let line = agents_own_line(&worktree);
    assert_eq!(line.value_of("--setting-sources"), Some("project,local"));
    assert_eq!(line.value_of("--model"), Some("haiku"));
    assert_eq!(line.value_of("--max-budget-usd"), Some("0.1"));
    for flag in [
        "--mcp-config",
        "--strict-mcp-config",
        "--settings",
        "--agents",
    ] {
        assert!(!line.argv.iter().any(|word| word == flag), "{flag}");
    }
    assert_eq!(line.cwd, worktree);
    assert!(line.env.is_empty());
}

#[test]
fn the_first_turn_is_one_stream_json_user_message_with_the_prompt() {
    let fixture = Fixture::new();
    let request = collide_request(&fixture, fixture.repo.work());
    let bytes = first_turn(&request);
    let text = String::from_utf8(bytes).unwrap();
    assert_eq!(text.lines().count(), 1);
    let turn: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(turn["type"], "user");
    assert_eq!(turn["message"]["content"][0]["text"], PROMPT);
}

#[test]
fn the_prompt_does_not_hold_the_words_it_asks_for() {
    for word in [ROOT_WORD, NESTED_WORD, IMPORT_WORD, SKILL_WORD, AGENT_WORD] {
        assert!(!PROMPT.contains(word), "{word}");
    }
}

#[test]
fn the_items_ledger_refuses_a_session_that_could_pass_fifty_cents() {
    let root = super::common::TempRoot::new();
    let ledger = root.join("ledger");
    std::fs::write(&ledger, "a\t0.1\t0.09\nb\t0.1\t0.31\n").unwrap();
    // $0.40 spent: a $0.10 cap reaches the limit exactly, a second one passes it.
    assert_eq!(
        real::ledger_refusal(&ledger, "c", 0.10, ITEM_LEDGER_LIMIT_USD),
        None
    );
    std::fs::write(&ledger, "a\t0.1\t0.09\nb\t0.1\t0.31\nc\t0.1\t0.05\n").unwrap();
    let refusal = real::ledger_refusal(&ledger, "d", 0.10, ITEM_LEDGER_LIMIT_USD).expect("refused");
    assert!(
        refusal.contains("`d` is not run") && refusal.contains("$0.5"),
        "{refusal}"
    );
    assert_eq!(
        real::ledger_refusal(&root.join("none"), "e", 0.10, ITEM_LEDGER_LIMIT_USD),
        None
    );
}

#[test]
fn the_part_2_tests_are_skipped_without_the_opt_in_and_say_why() {
    if std::env::var("COLLIDE_REAL_CLAUDE").as_deref() == Ok("1") {
        return;
    }
    let reason = unavailable().expect("skipped without the opt-in");
    assert!(reason.contains("COLLIDE_REAL_CLAUDE=1"), "{reason}");
}

#[test]
fn a_session_is_not_run_when_no_ledger_is_named() {
    if std::env::var_os(real::LEDGER_ENV).is_some() {
        return;
    }
    let refused = std::panic::catch_unwind(|| {
        real::check_ledger_within("x", SESSION_CAP_USD, ITEM_LEDGER_LIMIT_USD)
    });
    let message = *refused
        .expect_err("no ledger, no run")
        .downcast::<String>()
        .unwrap();
    assert!(message.contains("names no ledger"), "{message}");
}

#[test]
fn a_session_is_not_run_without_the_opt_in_even_when_its_caller_does_not_check() {
    // `run_session` checks the opt-in itself, so a test that skips the `live!` check (or a stub of `unavailable` made
    // to see a test fail) still cannot reach the real `claude` (ts-0103-sa, the session outside the ledger).
    if std::env::var("COLLIDE_REAL_CLAUDE").as_deref() == Ok("1")
        || std::env::var_os(real::LEDGER_ENV).is_some()
    {
        return;
    }
    let fixture = Fixture::new();
    let line = Line {
        argv: vec!["--version".to_string()],
        env: BTreeMap::new(),
        cwd: fixture.root().path().to_path_buf(),
    };
    let refused = std::panic::catch_unwind(|| run_session(&fixture, "no-opt-in", &line, b""))
        .expect_err("no opt-in, no run");
    let message = *refused.downcast::<String>().unwrap();
    assert!(message.contains("COLLIDE_REAL_CLAUDE=1"), "{message}");
}

// ---- offline: ts-0106-nr ----------------------------------------------------------------------------------------

#[test]
fn findings_find_the_skill_under_a_plugin_prefix_and_the_plugin_itself() {
    let prefixed = format!("{PLUGIN_NAME}:{SKILL_NAME}");
    let mut lines = made_up_stream(&["x", &prefixed], "SKILL=none");
    lines[0]["plugins"] = json!([{"name": "owner-private-plugin"}, {"name": PLUGIN_NAME}]);
    let found = findings(&lines, Some(0), Vec::new());
    assert!(found.init_has["skills"]);
    assert_eq!(found.skill_listed_as.as_deref(), Some(prefixed.as_str()));
    assert!(found.init_has["plugins"]);
    assert_eq!(found.init_counts["plugins"], 2);
    let printed = found.describe();
    assert!(printed.contains(&prefixed), "{printed}");
    assert!(!printed.contains("owner-private"), "{printed}");
}

#[test]
fn findings_say_the_skill_and_the_plugin_are_not_listed_when_the_stream_has_neither() {
    let lines = made_up_stream(
        &["x", "other-skill", "p:not-repo-canary-skill-2"],
        "SKILL=none",
    );
    let found = findings(&lines, Some(0), Vec::new());
    assert!(!found.init_has["skills"]);
    assert_eq!(found.skill_listed_as, None);
    assert!(!found.init_has["plugins"]);
}

#[test]
fn the_plugin_session_is_haiku_capped_at_five_cents_config_none_and_has_the_plugin() {
    let fixture = Fixture::new();
    let worktree = fixture.worktree("wt");
    let request = collide_request_capped(&fixture, &worktree, PLUGIN_CAP_USD);
    let line = candidate_line(&fixture, Candidate::AddDirSkillsPlugin, &request);
    assert_eq!(line.value_of("--model"), Some("haiku"));
    assert_eq!(line.value_of("--max-budget-usd"), Some("0.05"));
    assert_eq!(line.value_of("--setting-sources"), Some(""));
    assert!(line.argv.iter().any(|word| word == "--strict-mcp-config"));
    let plugin = line.value_of("--plugin-dir").expect("the plugin line");
    assert_eq!(
        holds_only_the_skills_link(Path::new(plugin), &worktree),
        Ok(())
    );
}

#[test]
fn the_plugin_ledger_allows_one_session_and_refuses_a_second_that_could_pass_ten_cents() {
    let root = super::common::TempRoot::new();
    let ledger = root.join("ledger");
    let refusal = |spent: &str| {
        std::fs::write(&ledger, spent).unwrap();
        real::ledger_refusal(&ledger, "next", PLUGIN_CAP_USD, PLUGIN_LEDGER_LIMIT_USD)
    };
    assert_eq!(refusal(""), None);
    assert_eq!(
        refusal("a\t0.05\t0.05\n"),
        None,
        "a cap reaches the limit exactly"
    );
    let refused = refusal("a\t0.05\t0.06\n").expect("refused");
    assert!(
        refused.contains("`next` is not run") && refused.contains("$0.1"),
        "{refused}"
    );
}
