//! The planning commands over a board with a `layout` setting and a private root (ts-0136-rr; 0.10, Planning
//! sources): `collide tasks` lists the items from the layout's folders and from the private root, and `collide task
//! create` writes into the layout's folder or the private root. The commands run in the test's process over a daemon
//! with the real built-in `collide/plan` pack from a built bundle. Every path is below the test's own root under
//! `/tmp`, which is also the home. Nothing here touches the user's home, keychain or a real daemon.

mod common;

use collide_host::task_cli::{Command, exit, parse_task, parse_tasks};
use collide_host::task_daemon::run_with;
use common::plan_rig::{EPIC, Rig, rig, task, trust};
use common::wait_until;
use std::time::Duration;

const DEADLINE: Duration = Duration::from_secs(30);

/// A trusted board whose `layout.root` is `docs/plan`, with the epic `ep-001-aa` and the task `ts-0001-aa` there, and
/// one item left at the default root, which the layout hides.
fn board() -> Rig {
    let rig = rig("task-cli-plan");
    rig.write(
        ".collide/config.yaml",
        "config:\n  collide/plan:\n    layout:\n      root: docs/plan\n",
    );
    rig.write("docs/plan/epics/ep-001-aa-alpha/epic.md", EPIC);
    rig.write(
        "docs/plan/epics/ep-001-aa-alpha/tasks/ts-0001-aa-in-docs.md",
        &task("ts-0001-aa"),
    );
    rig.write(
        ".collide/planning/epics/ep-009-zz-left/tasks/ts-0090-zz-left.md",
        &task("ts-0090-zz"),
    );
    rig.commit();
    let board = rig.board();
    trust(&rig, &board);
    rig
}

fn command(words: &[&str]) -> Command {
    let rest: Vec<String> = words[1..].iter().map(|word| word.to_string()).collect();
    let parsed = if words[0] == "tasks" {
        parse_tasks(&rest)
    } else {
        parse_task(&rest)
    };
    parsed.unwrap_or_else(|error| panic!("{words:?}: {}", error.message))
}

/// `collide <words>` from the repository: the exit code, standard output and standard error.
fn cli(rig: &Rig, words: &[&str]) -> (u8, String, String) {
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = run_with(
        rig.daemon.paths(),
        &command(words),
        rig.repo.work(),
        &mut out,
        &mut err,
    );
    (
        code,
        String::from_utf8(out).unwrap(),
        String::from_utf8(err).unwrap(),
    )
}

fn keys(out: &str) -> Vec<String> {
    out.lines()
        .map(|line| line.split_whitespace().next().unwrap().to_string())
        .collect()
}

#[test]
fn tasks_lists_the_items_of_the_layouts_folder_and_not_the_default_root() {
    let rig = board();

    let (code, out, err) = cli(&rig, &["tasks"]);

    assert_eq!(code, exit::OK, "{err}");
    assert_eq!(keys(&out), ["plan:ep-001-aa", "plan:ts-0001-aa"]);
}

#[test]
fn create_writes_into_the_layouts_folder() {
    let rig = board();

    let (code, out, err) = cli(
        &rig,
        &["task", "create", "Made in docs", "epic=ep-001-aa", "seed=1"],
    );

    assert_eq!(code, exit::OK, "{err}");
    let file = "docs/plan/epics/ep-001-aa-alpha/tasks/ts-0002-ra-made-in-docs.md";
    assert!(out.contains(file), "{out}");
    assert!(rig.repo.work().join(file).exists());
    assert_eq!(
        rig.repo.git(&["status", "--porcelain"]),
        format!("?? {file}")
    );
}

#[test]
fn a_private_item_is_made_listed_and_shown_while_the_repository_stays_clean() {
    let rig = board();

    let (code, out, err) = cli(
        &rig,
        &[
            "task",
            "create",
            "Only mine",
            "epic=ep-001-aa",
            "root=private",
            "seed=1",
        ],
    );

    assert_eq!(code, exit::OK, "{err}");
    assert!(out.contains("ts-0002-ra"), "{out}");
    assert_eq!(rig.repo.git(&["status", "--porcelain"]), "");
    wait_until("the private item is listed", DEADLINE, || {
        keys(&cli(&rig, &["tasks"]).1).contains(&"plan:ts-0002-ra".to_string())
    });
    let (code, shown, err) = cli(&rig, &["task", "ts-0002-ra"]);
    assert_eq!(code, exit::OK, "{err}");
    assert!(shown.starts_with("ep-001-aa / ts-0002-ra\n"), "{shown}");
    assert!(shown.contains("path    private:"), "{shown}");
}
