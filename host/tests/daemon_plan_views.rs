//! The plan pack's views and its `roles` setting through the daemon (ts-0135-cx; 0.10, Planning sources and Views):
//! `task.views` lists `collide/plan/spikes`, `collide/plan/decisions-needed` and the built-in `ready`; `task.list`
//! finds a view by its full name or its bare one; `ready` is what the ready filter lists, with a cycle and an unknown
//! blocker in its report; and a `roles` mapping that names a kind the source lacks is a problem on the line. The
//! provider is the real built-in `collide/plan` pack from a built bundle, run by `node`, over a temporary repository.
//! A pack view is named `<pack>/<key>`, and the pack is `collide/plan`, so its views have three parts.
//!
//! Every path is below the test's own root under `/tmp`, which is also the home. Nothing here touches the user's
//! home, keychain or a real daemon.

mod common;

use collide_core::owner::{
    BoardListParams, BoardListResult, RpcError, TaskListResult, TaskViewsParams, TaskViewsResult,
    ViewEntry, ViewSource,
};
use collide_core::ready::UnknownTarget;
use common::config_fixture::Fixture;
use common::plan_rig::{EPIC, Rig, list_until, rig, rpc, trust};
use serde_json::json;

const PLANNING: &str = ".collide/planning";

fn doc(kind: &str, id: &str, status: &str, extra: &str, body: &str) -> String {
    format!(
        "---\nid: {id}\nkind: {kind}\nstatus: {status}\ntitle: \"Made-up {kind}\"\nowners: []\n{extra}---\n\n{body}"
    )
}

fn blocked_by(to: &str) -> String {
    format!("links:\n  - {{ type: blocked-by, to: {to} }}\n")
}

/// One item per file under the plan folder: the tasks of the ready fixtures, two spikes, two questions and two
/// proposals.
fn items() -> Vec<(String, String)> {
    let epic = format!("{PLANNING}/epics/ep-001-aa-alpha");
    let task = |id: &str, status: &str, links: &str| {
        (
            format!("{epic}/tasks/{id}-work.md"),
            doc("task", id, status, links, "## Goal\n"),
        )
    };
    let question = |id: &str, status: &str, extra: &str| {
        (
            format!("{epic}/questions/{id}-why.md"),
            doc(
                "question",
                id,
                status,
                extra,
                "## Question\n\n## Decision\n",
            ),
        )
    };
    let proposal = |id: &str, status: &str| {
        (
            format!("{epic}/proposals/{id}-idea.md"),
            doc("proposal", id, status, "", "## Decision\n"),
        )
    };
    let spike = |id: &str, status: &str| {
        (
            format!("{PLANNING}/spikes/{id}-probe/record.md"),
            doc("spike", id, status, "", "# Made-up spike\n"),
        )
    };
    vec![
        task("ts-0001-aa", "ready", "links: []\n"),
        task("ts-0002-bb", "ready", &blocked_by("ts-0001-aa")),
        task("ts-0003-cc", "ready", &blocked_by("ts-9999-zz")),
        task("ts-0004-dd", "ready", &blocked_by("ts-0005-ee")),
        task("ts-0005-ee", "ready", &blocked_by("ts-0004-dd")),
        task("ts-0006-ff", "done", "links: []\n"),
        task("ts-0007-gg", "ready", &blocked_by("ts-0006-ff")),
        question("qu-0001-aa", "open", ""),
        question("qu-0002-bb", "resolved", "resolved_by: human:2026-11-18\n"),
        proposal("pr-0001-aa", "proposed"),
        proposal("pr-0002-bb", "draft"),
        spike("sp-0001-aa", "planned"),
        spike("sp-0002-bb", "running"),
    ]
}

/// A trusted board over the fixture items, with `extra` files written first. Returns once the plan source has listed.
fn board(extra: &[(&str, &str)]) -> (Rig, String) {
    let rig = rig("daemon-plan-views");
    rig.write(&format!("{PLANNING}/epics/ep-001-aa-alpha/epic.md"), EPIC);
    for (path, text) in items() {
        rig.write(&path, &text);
    }
    for (path, text) in extra {
        rig.write(path, text);
    }
    rig.commit();
    let board = rig.board();
    trust(&rig, &board);
    list_until(&rig, &board, "the plan source lists", |list| {
        !list.refreshing && list.tasks.len() == 14
    });
    (rig, board)
}

fn named(rig: &Fixture, board: &str, name: &str) -> Result<TaskListResult, RpcError> {
    rpc(
        rig,
        "task.list",
        &json!({ "board": board, "view": { "type": "named", "name": name } }),
    )
}

fn ids(list: &TaskListResult) -> Vec<String> {
    list.tasks
        .iter()
        .map(|task| task.key.strip_prefix("plan:").unwrap().to_string())
        .collect()
}

fn views(rig: &Fixture, board: &str) -> Vec<ViewEntry> {
    let result: TaskViewsResult = rpc(
        rig,
        "task.views",
        &TaskViewsParams {
            board: board.to_string(),
        },
    )
    .unwrap();
    result.views
}

fn entry<'a>(views: &'a [ViewEntry], name: &str) -> &'a ViewEntry {
    views
        .iter()
        .find(|view| view.view.name == name)
        .unwrap_or_else(|| panic!("no view {name}: {views:?}"))
}

fn line_problems(rig: &Fixture) -> Vec<String> {
    let result: BoardListResult = rpc(rig, "board.list", &BoardListParams {}).unwrap();
    result
        .boards
        .iter()
        .flat_map(|board| board.lines.iter())
        .flat_map(|line| line.problems.clone())
        .collect()
}

#[test]
fn task_views_lists_the_plan_packs_two_views_and_the_built_in_ready_each_with_its_source() {
    let (rig, board) = board(&[]);

    let found = views(&rig, &board);

    let ready = entry(&found, "ready");
    assert_eq!(ready.source, ViewSource::Builtin {});
    assert_eq!((ready.error.as_deref(), ready.view.queue), (None, false));
    assert_eq!(ready.view.where_, None);
    let pack = ViewSource::Pack {
        pack: "collide/plan".into(),
    };
    let spikes = entry(&found, "collide/plan/spikes");
    assert_eq!(spikes.source, pack);
    assert_eq!(spikes.error, None);
    assert!(!spikes.view.queue);
    let needed = entry(&found, "collide/plan/decisions-needed");
    assert_eq!(needed.source, pack);
    assert!(needed.view.queue, "it feeds a Queue lane");
    assert_eq!(needed.error, None);
    assert_eq!(found[0].view.name, "ready", "the built-in view is first");
}

#[test]
fn spikes_lists_the_spikes_and_no_other_kind() {
    let (rig, board) = board(&[]);

    let listed = named(&rig, &board, "collide/plan/spikes").unwrap();

    assert_eq!(ids(&listed), ["sp-0001-aa", "sp-0002-bb"]);
    assert_eq!(listed.view.unwrap().members, 2);
    assert_eq!(
        entry(&views(&rig, &board), "collide/plan/spikes").task_count,
        2
    );
}

#[test]
fn decisions_needed_lists_the_open_questions_and_the_proposals_that_are_proposed_and_nothing_else()
{
    let (rig, board) = board(&[]);

    let listed = named(&rig, &board, "collide/plan/decisions-needed").unwrap();

    assert_eq!(ids(&listed), ["pr-0001-aa", "qu-0001-aa"]);
    let counted = entry(&views(&rig, &board), "collide/plan/decisions-needed").task_count;
    assert_eq!(counted, 2);
}

#[test]
fn ready_is_what_the_ready_filter_lists_and_the_report_names_the_cycle_and_the_unknown_blocker() {
    let (rig, board) = board(&[]);

    let listed = named(&rig, &board, "ready").unwrap();
    let filtered: TaskListResult =
        rpc(&rig, "task.list", &json!({ "board": board, "ready": true })).unwrap();

    assert_eq!(
        ids(&listed),
        ["qu-0001-aa", "sp-0001-aa", "ts-0001-aa", "ts-0007-gg"]
    );
    assert_eq!(ids(&listed), ids(&filtered), "--ready and the view agree");
    let report = listed.view.expect("a report");
    assert_eq!((report.name.as_str(), report.members), ("ready", 4));
    assert_eq!(report.cycle, ["plan:ts-0004-dd", "plan:ts-0005-ee"]);
    assert_eq!(
        report.unknown_blockers,
        [UnknownTarget {
            from: "plan:ts-0003-cc".into(),
            to: "ts-9999-zz".into()
        }]
    );
    assert_eq!(entry(&views(&rig, &board), "ready").task_count, 4);
}

#[test]
fn a_bare_name_lists_what_the_full_name_does_with_the_same_report() {
    let (rig, board) = board(&[]);

    for (bare, full) in [
        ("spikes", "collide/plan/spikes"),
        ("decisions-needed", "collide/plan/decisions-needed"),
    ] {
        let by_bare = named(&rig, &board, bare).unwrap();
        let by_full = named(&rig, &board, full).unwrap();
        assert_eq!(by_bare.tasks, by_full.tasks, "{bare}");
        assert_eq!(by_bare.view, by_full.view, "{bare}");
        assert_eq!(
            by_bare.view.unwrap().name,
            full,
            "the report names the view found"
        );
    }
}

#[test]
fn a_name_no_view_has_is_not_found_and_lists_the_full_names() {
    let (rig, board) = board(&[]);

    let error = named(&rig, &board, "nope").unwrap_err();

    assert_eq!(error.code, -32010, "{error:?}");
    for name in [
        "ready",
        "collide/plan/spikes",
        "collide/plan/decisions-needed",
    ] {
        assert!(error.message.contains(name), "{}", error.message);
    }
}

const ALPHA: &str = "apiVersion: collide/v0\nname: alpha\nversion: 1.0.0\n\
                     views:\n  mine:\n    name: mine\n    where: \"true\"\n\
                     \x20 spikes:\n    name: spikes\n    where: \"false\"\n";
const BETA: &str = "apiVersion: collide/v0\nname: beta\nversion: 1.0.0\n\
                    views:\n  mine:\n    name: mine\n    where: \"false\"\n";
const PACKS: &str =
    "packs:\n  - source: repo:.collide/packs/alpha\n  - source: repo:.collide/packs/beta\n";

#[test]
fn two_packs_with_one_key_and_no_tie_break_are_an_error_naming_both_and_the_pack_that_supplies_the_source_wins()
 {
    let (rig, board) = board(&[
        (".collide/board.yaml", PACKS),
        (".collide/packs/alpha/pack.yaml", ALPHA),
        (".collide/packs/beta/pack.yaml", BETA),
    ]);

    let error = named(&rig, &board, "mine").unwrap_err();

    assert_eq!(error.code, -32602, "{error:?}");
    for name in ["alpha/mine", "beta/mine"] {
        assert!(error.message.contains(name), "{}", error.message);
    }
    // `spikes` is declared by `alpha` and by `collide/plan`, which supplies the board's `plan` source: it wins.
    let listed = named(&rig, &board, "spikes").unwrap();
    assert_eq!(ids(&listed), ["sp-0001-aa", "sp-0002-bb"]);
    assert_eq!(listed.view.unwrap().name, "collide/plan/spikes");
    // The full names still find the others.
    assert_eq!(
        ids(&named(&rig, &board, "alpha/spikes").unwrap()),
        Vec::<String>::new()
    );
    assert_eq!(named(&rig, &board, "alpha/mine").unwrap().tasks.len(), 14);
}

#[test]
fn a_view_file_named_ready_cannot_replace_the_built_in_one() {
    let (rig, board) = board(&[(
        ".collide/views/ready.yaml",
        "name: ready\ninclude: [plan:ts-0002-bb]\n",
    )]);

    let listed = named(&rig, &board, "ready").unwrap();
    let found = views(&rig, &board);

    assert_eq!(
        ids(&listed),
        ["qu-0001-aa", "sp-0001-aa", "ts-0001-aa", "ts-0007-gg"]
    );
    let shadowed: Vec<&ViewEntry> = found
        .iter()
        .filter(|view| view.view.name == "ready")
        .collect();
    assert_eq!(shadowed.len(), 2, "{found:?}");
    assert!(shadowed[1].error.as_deref().unwrap().contains("built-in"));
}

#[test]
fn the_default_roles_are_no_problem_on_the_line() {
    let (rig, _board) = board(&[]);

    assert_eq!(line_problems(&rig), Vec::<String>::new());
}

#[test]
fn a_role_that_names_a_kind_the_source_lacks_is_a_problem_naming_role_and_kind_and_the_tasks_still_list()
 {
    let (rig, board) = board(&[(
        ".collide/config.yaml",
        "config:\n  collide/plan:\n    roles:\n      proposal: [proposal]\n      decision: [adr]\n",
    )]);

    let list = list_until(&rig, &board, "the plan source lists", |list| {
        !list.refreshing && list.tasks.len() == 14
    });
    let problems = line_problems(&rig);

    assert_eq!(list.tasks.len(), 14);
    let named_it: Vec<&String> = problems
        .iter()
        .filter(|problem| problem.contains("`decision`") && problem.contains("`adr`"))
        .collect();
    assert_eq!(named_it.len(), 1, "{problems:?}");
    assert_eq!(problems.len(), 1, "{problems:?}");
}
