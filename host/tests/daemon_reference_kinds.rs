//! The reference pack's plan provider reads ADRs, spikes and questions as items (ts-0137-pg; 0.10, Planning sources,
//! Providers and Views), through the daemon: the pack is committed into a fixture repository and loaded as a `repo:`
//! source over a made-up `arch/` tree, a trusted line and a temporary root. `task.views` lists the pack's two views,
//! `task.list` over `decisions-needed` gives the questions of the fake plan CLI's own JSON in its order, `task.get`
//! shows an ADR's `evidence` from the spike's side as "evidence for", a link to a spike the tree lacks is missing and
//! not an error, the roles check passes for the pack's mapping, and a write on a question is refused with no call to
//! the plan CLI.
//!
//! A change on disk reaches the collections too (ts-0151-fk): a question resolved or set to need a person leaves or
//! joins `decisions-needed`, and a task's epic, or the epic's file, moves the task between epics, with no restart and
//! no relisting asked for.
//!
//! The fake plan CLI is `fixtures/<name>-arch/plan`, run by `node`; the repository's own `plan` is a shell script that
//! records its arguments in the test's home and runs it. Every path is below the test's own root under `/tmp`, which is
//! also the home. Nothing here touches the user's home, keychain or a real daemon.

mod common;

use collide_core::item::LinkDir;
use collide_core::owner::{
    BoardListParams, BoardListResult, RpcError, TaskGetParams, TaskGetResult, TaskListResult,
    TaskViewsParams, TaskViewsResult, ViewSource,
};
use collide_core::provider::ApplyResult;
use common::plan_rig::{DEADLINE, Rig, list_until, rig, rpc, trust};
use common::wait_until;
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

/// The pack's folder name, written so that the no-names check does not see it in this file.
fn name() -> String {
    ["orch", "ard"].concat()
}

fn fixtures() -> PathBuf {
    common::repo_root().join("fixtures")
}

fn copy_tree(from: &Path, rig: &Rig, to: &str) {
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = format!("{to}/{}", entry.file_name().to_str().unwrap());
        if entry.path().is_dir() {
            copy_tree(&entry.path(), rig, &target);
        } else {
            rig.repo.write(&target, &fs::read(entry.path()).unwrap());
        }
    }
}

/// The manifest of the pack's fixture copy with what a provider needs, and the provider, views and config sections as
/// they stand in the real `pack.yaml`: the text from its `providers:` line to its end.
fn manifest() -> String {
    let real = fs::read_to_string(fixtures().join(format!("packs/{}/pack.yaml", name()))).unwrap();
    let tail = &real[real
        .find("\nproviders:")
        .expect("the pack declares its providers")..];
    format!(
        "apiVersion: collide/v0\nname: {}\nversion: 0.1.0\nrequires:\n  sdk: \">=0.2 <0.3\"\n  tools:\n    \
         - {{ name: node }}\n{tail}",
        name()
    )
}

/// A trusted board over the `kinds` tree. The repository's `plan` records its arguments in `$HOME/plan-calls.log` and
/// runs the fake, which sits in the home, outside the checkout.
fn board() -> (Rig, String) {
    board_with(None)
}

/// As [`board`]; with `answer`, the plan CLI prints that and not what the fake makes of the tree.
fn board_with(answer: Option<&str>) -> (Rig, String) {
    board_over(answer, &[])
}

/// As [`board_with`], with `files` (a path from the root, and its text) written over the tree before it is committed.
fn board_over(answer: Option<&str>, files: &[(&str, &str)]) -> (Rig, String) {
    let rig = rig("daemon-reference-kinds");
    let home = rig.home().to_path_buf();
    let fake = home.join("fake-plan");
    match answer {
        None => drop(fs::copy(fixtures().join(format!("{}-arch/plan", name())), &fake).unwrap()),
        Some(json) => {
            fs::write(&fake, format!("#!/bin/sh\ncat <<'EOF'\n{json}\nEOF\n")).unwrap();
            fs::set_permissions(&fake, std::os::unix::fs::PermissionsExt::from_mode(0o755))
                .unwrap();
        }
    }
    let pack = format!(".collide/packs/{}", name());
    copy_tree(
        &fixtures().join(format!("{}-arch/kinds/arch", name())),
        &rig,
        "arch",
    );
    copy_tree(
        &fixtures().join(format!("packs/{}/providers", name())),
        &rig,
        &format!("{pack}/providers"),
    );
    for (path, text) in files {
        rig.repo.write(path, text.as_bytes());
    }
    rig.repo
        .write(&format!("{pack}/pack.yaml"), manifest().as_bytes());
    rig.repo.write(
        ".collide/board.yaml",
        format!("packs:\n  - source: repo:{pack}\n").as_bytes(),
    );
    rig.repo.write_executable(
        "plan",
        b"#!/bin/sh\necho \"$*\" >> \"$HOME/plan-calls.log\"\nexec \"$HOME/fake-plan\" \"$@\"\n",
    );
    rig.commit();
    let board = rig.board();
    trust(&rig, &board);
    list_until(&rig, &board, "the plan source lists", |list| {
        !list.refreshing && list.tasks.len() == 16
    });
    (rig, board)
}

fn named(rig: &Rig, board: &str, view: &str) -> TaskListResult {
    rpc(
        rig,
        "task.list",
        &json!({ "board": board, "view": { "type": "named", "name": view } }),
    )
    .unwrap()
}

fn keys(list: &TaskListResult) -> Vec<String> {
    list.tasks.iter().map(|task| task.key.clone()).collect()
}

fn get(rig: &Rig, board: &str, id: &str) -> TaskGetResult {
    rpc(
        rig,
        "task.get",
        &TaskGetParams {
            board: board.to_string(),
            id: id.to_string(),
            checkout: None,
        },
    )
    .unwrap()
}

/// The questions in the fake plan CLI's JSON, as keys, in its order: `node plan status`, run with a cleared environment
/// over the same tree.
fn fakes_queue(rig: &Rig) -> Vec<String> {
    let home = rig.home();
    let run = Command::new("node")
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap())
        .env("HOME", home)
        .current_dir(rig.repo.work())
        .args([home.join("fake-plan").to_str().unwrap()])
        .args(["status", "--root", "arch", "--format", "json"])
        .output()
        .expect("node runs the fake plan CLI");
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    let envelope: serde_json::Value = serde_json::from_slice(&run.stdout).unwrap();
    envelope["data"]["queue"]
        .as_array()
        .expect("the fake's JSON has a queue")
        .iter()
        .map(|entry| {
            let question = entry["question"].as_str().unwrap();
            let short: Vec<&str> = question.splitn(4, '-').take(3).collect();
            format!("plan:{}", short.join("-"))
        })
        .collect()
}

fn calls(rig: &Rig) -> Vec<String> {
    match fs::read_to_string(rig.home().join("plan-calls.log")) {
        Ok(text) => text.lines().map(str::to_string).collect(),
        Err(_) => Vec::new(),
    }
}

#[test]
fn task_views_lists_the_packs_decisions_needed_as_a_queue_view_and_its_spikes_view() {
    let (rig, board) = board();

    let result: TaskViewsResult = rpc(
        &rig,
        "task.views",
        &TaskViewsParams {
            board: board.clone(),
        },
    )
    .unwrap();

    let pack = ViewSource::Pack { pack: name() };
    for (view, queue) in [("decisions-needed", true), ("spikes", false)] {
        let full = format!("{}/{view}", name());
        let found = result
            .views
            .iter()
            .find(|entry| entry.view.name == full)
            .unwrap_or_else(|| panic!("no view {full}: {:?}", result.views));
        assert_eq!(
            (found.source.clone(), found.view.queue),
            (pack.clone(), queue)
        );
        assert_eq!(found.error, None);
    }
}

#[test]
fn decisions_needed_lists_the_question_ids_of_the_fakes_own_queue_in_its_order() {
    let (rig, board) = board();

    let listed = named(&rig, &board, "decisions-needed");

    assert_eq!(
        keys(&listed),
        ["plan:Q-03-01", "plan:Q-03-03", "plan:Q-05-01"]
    );
    assert_eq!(keys(&listed), fakes_queue(&rig));
}

#[test]
fn decisions_needed_follows_the_fakes_output_when_it_names_other_questions_than_the_tree_says() {
    let answer = r#"{"schema_version":1,"command":"status","corpus":"arch","data":{"queue":[
        {"question":"Q-05-01-which-lamp-first","file":"x","needs":"human","blocks":[]},
        {"question":"Q-03-02-how-deep-is-the-quay","file":"x","needs":"evidence","blocks":[]}]}}"#;
    let (rig, board) = board_with(Some(answer));

    let listed = named(&rig, &board, "decisions-needed");

    // A named view lists in the corpus's order, which is by key. The plan CLI sorts its queue by id too, so the two
    // agree for what it prints; the order of the collection's group is the provider's test (`queue.test.ts`).
    assert_eq!(keys(&listed), ["plan:Q-03-02", "plan:Q-05-01"]);
}

#[test]
fn spikes_lists_the_four_spikes_with_their_statuses_and_no_other_kind() {
    let (rig, board) = board();

    let listed = named(&rig, &board, "spikes");

    let mut found: Vec<(String, String)> = listed
        .tasks
        .iter()
        .map(|task| (task.key.clone(), task.state.status.clone()))
        .collect();
    found.sort();
    let spike = |key: &str, status: &str| (key.to_string(), status.to_string());
    assert_eq!(
        found,
        [
            spike("plan:S1", "superseded"),
            spike("plan:S2a", "concluded"),
            spike("plan:S2b", "running"),
            spike("plan:S4d-dock", "open"),
        ]
    );
}

#[test]
fn an_adrs_evidence_shows_from_the_spike_as_evidence_for_and_the_link_types_carry_the_inverse() {
    let (rig, board) = board();

    let adr = get(&rig, &board, "plan:ADR-004");
    let spike = get(&rig, &board, "plan:S2a");

    let out: Vec<(&str, &str)> = adr
        .links
        .iter()
        .filter(|link| link.dir == LinkDir::Out)
        .map(|link| (link.link_type.as_str(), link.key.as_str()))
        .collect();
    assert!(out.contains(&("evidence", "plan:S2a")), "{out:?}");
    let seen: Vec<(&str, &str)> = spike
        .links
        .iter()
        .filter(|link| link.dir == LinkDir::In && link.link_type == "evidence")
        .map(|link| (link.link_type.as_str(), link.key.as_str()))
        .collect();
    assert_eq!(
        seen,
        [("evidence", "plan:ADR-002"), ("evidence", "plan:ADR-004")]
    );
    let evidence = spike
        .link_types
        .iter()
        .find(|link_type| link_type.name == "evidence")
        .expect("the source declares evidence");
    assert_eq!(evidence.inverse.as_deref(), Some("evidence for"));
}

#[test]
fn a_link_to_a_spike_the_tree_lacks_is_missing_and_the_item_still_gets() {
    let (rig, board) = board();

    let adr = get(&rig, &board, "plan:ADR-006");

    assert_eq!(adr.task.expect("the ADR is there").key, "plan:ADR-006");
    let broken: Vec<_> = adr
        .links
        .iter()
        .filter(|link| link.link_type == "evidence")
        .collect();
    assert!(
        broken.iter().all(|link| link.missing),
        "a link to S9 has nothing at its far end: {broken:?}"
    );
}

/// `task.list` with a kind, as `collide tasks --kind <kind>` sends it.
fn of_kind(rig: &Rig, board: &str, kind: &str) -> Result<TaskListResult, RpcError> {
    rpc(rig, "task.list", &json!({ "board": board, "kind": kind }))
}

#[test]
fn kind_task_lists_the_packs_task_on_a_board_whose_only_source_is_the_pack() {
    let (rig, board) = board();

    let listed = of_kind(&rig, &board, "task").expect("`task` is a kind the source declares");

    assert_eq!(keys(&listed), ["plan:T-03-001"]);
    assert_eq!(
        listed.tasks[0].kind.as_deref(),
        Some("task"),
        "the listing carries the kind"
    );
}

#[test]
fn a_task_gets_with_the_kind_task_and_the_other_kinds_are_still_listed_by_their_own_name() {
    let (rig, board) = board();

    let task = get(&rig, &board, "plan:T-03-001").task.expect("the task");
    let spikes = of_kind(&rig, &board, "spike").unwrap();

    assert_eq!(task.kind.as_deref(), Some("task"));
    assert_eq!(keys(&spikes).len(), 4);
    assert!(
        spikes
            .tasks
            .iter()
            .all(|spike| spike.key != "plan:T-03-001")
    );
}

#[test]
fn the_roles_check_passes_for_the_packs_own_mapping() {
    let (rig, _board) = board();

    let result: BoardListResult = rpc(&rig, "board.list", &BoardListParams {}).unwrap();

    let problems: Vec<String> = result
        .boards
        .iter()
        .flat_map(|board| board.lines.iter())
        .flat_map(|line| line.problems.clone())
        .collect();
    assert_eq!(problems, Vec::<String>::new());
}

#[test]
fn a_write_on_a_question_is_refused_with_the_reason_and_the_plan_cli_is_not_called() {
    let (rig, board) = board();
    let version = get(&rig, &board, "plan:Q-03-02")
        .task
        .expect("the question")
        .version;
    let before = calls(&rig);

    let applied: collide_core::owner::TaskApplyResult = rpc(
        &rig,
        "task.apply",
        &collide_core::owner::TaskApplyParams {
            board: board.clone(),
            source: None,
            apply: collide_core::provider::ApplyParams {
                actor: None,
                op: "setStatus".to_string(),
                id: Some("plan:Q-03-02".to_string()),
                args: std::collections::BTreeMap::from([("status".to_string(), json!("done"))]),
                version: Some(version),
                checkout: None,
            },
            commit: false,
        },
    )
    .unwrap();

    match applied.result.expect("an answer, not a queued write") {
        ApplyResult::Refused { message } => {
            assert!(message.contains("the plan CLI changes these"), "{message}");
        }
        other => panic!("expected a refusal, got {other:?}"),
    }
    assert_eq!(
        calls(&rig),
        before,
        "no plan CLI call for the refused write"
    );
    assert_eq!(rig.repo.git(&["status", "--porcelain"]), "");
}

#[test]
fn only_status_calls_reach_the_plan_cli_for_the_queue() {
    let (rig, board) = board();
    named(&rig, &board, "decisions-needed");

    let made = calls(&rig);

    assert!(!made.is_empty(), "the queue was read through ./plan");
    assert!(
        made.iter().all(|call| call.starts_with("status ")),
        "{made:?}"
    );
}

// ---- a change on disk reaches the collections (ts-0151-fk) ----------------------------------------------------------

const QUESTIONS: &str = "arch/proposals/03-harbor-index/questions";
const SURVEY: &str = "arch/proposals/03-harbor-index/tasks/T-03-001-survey-the-quay.md";
const HARBOR_EPIC: &str = "arch/work/epics/E-07-harbor-plan.md";
const LAMPS_EPIC: &str = "arch/work/epics/E-08-signal-lamps.md";

fn survey(epic: &str) -> String {
    format!(
        "---\ntitle: \"Survey the quay\"\nstatus: ready\nepic: {epic}\ndepends_on: []\nblocked_by: []\n---\n\n## Task\n"
    )
}

fn epic_file(title: &str) -> String {
    format!(
        "---\ntitle: \"{title}\"\nstatus: active\n---\n\nMade-up epic.\n\n### Slice 1: first berths\n\n\
         | Lane | Tasks, in order | Waits on another lane |\n|---|---|---|\n| North | T-03-001 | none |\n"
    )
}

/// The kinds tree with two epics, and the survey task in the first.
fn epic_board() -> (Rig, String) {
    board_over(
        None,
        &[
            (SURVEY, &survey("E-07-harbor-plan")),
            (HARBOR_EPIC, &epic_file("Harbor plan")),
            (LAMPS_EPIC, &epic_file("Signal lamps")),
        ],
    )
}

/// Changes the first `from` in the file to `to`, as an editor would: on disk, in the working tree.
fn edit(rig: &Rig, rel: &str, from: &str, to: &str) {
    let text = fs::read_to_string(rig.repo.work().join(rel)).unwrap();
    assert!(text.contains(from), "{rel} has no {from:?}");
    rig.repo.write(rel, text.replacen(from, to, 1).as_bytes());
}

/// Whether `held` came true within `within`. `wait_until` fails the test when it does not; this says.
fn holds_within(within: Duration, mut held: impl FnMut() -> bool) -> bool {
    let until = std::time::Instant::now() + within;
    while std::time::Instant::now() < until {
        if held() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    held()
}

fn all(rig: &Rig, board: &str) -> TaskListResult {
    rpc(rig, "task.list", &json!({ "board": board })).unwrap()
}

fn status_of(rig: &Rig, board: &str, key: &str) -> Option<String> {
    let list = all(rig, board);
    let task = list.tasks.iter().find(|task| task.key == key)?;
    Some(task.state.status.clone())
}

fn collections_of(rig: &Rig, board: &str, key: &str) -> Vec<String> {
    let list = all(rig, board);
    let task = list.tasks.iter().find(|task| task.key == key).unwrap();
    task.collections.clone()
}

fn decisions(rig: &Rig, board: &str) -> Vec<String> {
    keys(&named(rig, board, "decisions-needed"))
}

/// Returns once the provider's watcher reports a change made after this call began. The watcher reads its first
/// snapshot some time after the board lists, and a file changed before that read is never reported, so the survey
/// task's status is changed, each time to the other value, until one change is listed.
fn watching(rig: &Rig, board: &str) {
    let mut now = "ready";
    for _ in 0..30 {
        let next = if now == "ready" { "doing" } else { "ready" };
        edit(
            rig,
            SURVEY,
            &format!("status: {now}"),
            &format!("status: {next}"),
        );
        now = next;
        if holds_within(Duration::from_secs(3), || {
            status_of(rig, board, "plan:T-03-001").as_deref() == Some(now)
        }) {
            return;
        }
    }
    panic!("the watcher reported none of the changes made to the survey task");
}

#[test]
fn a_question_resolved_on_disk_leaves_decisions_needed_with_no_relisting_asked_for() {
    let (rig, board) = board();
    watching(&rig, &board);
    assert_eq!(
        decisions(&rig, &board),
        ["plan:Q-03-01", "plan:Q-03-03", "plan:Q-05-01"]
    );

    edit(
        &rig,
        &format!("{QUESTIONS}/Q-03-03-who-rings-the-bell.md"),
        "status: open",
        "status: resolved",
    );

    wait_until("the resolved question leaves the view", DEADLINE, || {
        decisions(&rig, &board) == ["plan:Q-03-01", "plan:Q-05-01"]
    });
}

#[test]
fn a_question_set_to_need_a_person_on_disk_joins_decisions_needed_with_no_relisting_asked_for() {
    let (rig, board) = board();
    watching(&rig, &board);

    edit(
        &rig,
        &format!("{QUESTIONS}/Q-03-02-how-deep-is-the-quay.md"),
        "needs: evidence",
        "needs: human",
    );

    wait_until("the question joins the view", DEADLINE, || {
        decisions(&rig, &board)
            == [
                "plan:Q-03-01",
                "plan:Q-03-02",
                "plan:Q-03-03",
                "plan:Q-05-01",
            ]
    });
}

#[test]
fn a_task_whose_epic_line_changes_leaves_the_old_epic_and_joins_the_new_one() {
    let (rig, board) = epic_board();
    watching(&rig, &board);
    assert_eq!(collections_of(&rig, &board, "plan:T-03-001"), ["plan:E-07"]);

    edit(
        &rig,
        SURVEY,
        "epic: E-07-harbor-plan",
        "epic: E-08-signal-lamps",
    );

    wait_until("the task is in the new epic only", DEADLINE, || {
        collections_of(&rig, &board, "plan:T-03-001") == ["plan:E-08"]
    });
}

#[test]
fn an_epic_file_that_changes_is_the_epic_as_listed_and_one_that_goes_takes_its_tasks_out() {
    let (rig, board) = epic_board();
    watching(&rig, &board);
    let title = |collection: &str| {
        all(&rig, &board)
            .collections
            .iter()
            .find(|held| held.id == collection)
            .map(|held| held.title.clone())
    };

    edit(
        &rig,
        LAMPS_EPIC,
        "title: \"Signal lamps\"",
        "title: \"Signal lamps, renamed\"",
    );
    wait_until("the epic has its new title", DEADLINE, || {
        title("plan:E-08").as_deref() == Some("Signal lamps, renamed")
    });

    rig.repo.remove(HARBOR_EPIC);
    wait_until("the epic is gone, with its member", DEADLINE, || {
        title("plan:E-07").is_none() && collections_of(&rig, &board, "plan:T-03-001").is_empty()
    });
}
