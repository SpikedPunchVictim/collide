//! End-to-end tests for `collide tasks` and `collide task` (ts-0020): the binary runs against a fixture repository
//! in a temp folder (items copied from `host/tests/fixtures/task-cli`, never this repository's own), started on the
//! real plan provider from a bundle built here, and on a stand-in provider for the outcomes the real one cannot be
//! made to give. They need `node` and `pnpm` on `PATH`, like the loader's tests.

use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

mod common;
use collide_host::daemon::DaemonPaths;
use common::detached::Detached;
use common::{Bundle, DaemonStop, TempRoot, repo_root, scratch};

// Cargo builds the daemon for these tests only when a test names it.
const _DAEMON: &str = env!("CARGO_BIN_EXE_collide-daemon");

const BUNDLE: &str = "task-cli-packs";

fn bundle() -> Bundle {
    common::bundle(BUNDLE)
}

fn git(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args([
            "-c",
            "user.name=test",
            "-c",
            "user.email=test@example.invalid",
        ])
        .args(["-c", "commit.gpgsign=false", "-c", "core.autocrlf=false"])
        .args(args)
        .current_dir(dir)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .output()
        .expect("git should run");
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target).unwrap();
        }
    }
}

/// A git repository in a scratch folder, holding a copy of a fixture, committed. Deleted when dropped.
struct Repo {
    dir: PathBuf,
}

impl Repo {
    fn new(name: &str) -> Self {
        let dir = scratch(&format!("task-cli-{name}"));
        let _ = fs::remove_dir_all(&dir);
        copy_tree(&repo_root().join("host/tests/fixtures/task-cli"), &dir);
        git(&dir, &["init", "-q"]);
        git(&dir, &["add", "-A"]);
        git(&dir, &["commit", "-q", "-m", "fixture"]);
        Self { dir }
    }
}

impl Drop for Repo {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

struct Run {
    code: i32,
    stdout: String,
    stderr: String,
}

/// Runs `collide` in `cwd` with the packs bundle at `packs`, and `env` added. Every command goes through a daemon, so
/// each call starts its own, under a temp root that is also the `HOME`, and stops it when the call has its answer: no
/// call sees what an earlier one left in a daemon's cache. The environment is the test's own: `PATH` is kept (the
/// plan provider runs on `node`), and `HOME` and the daemon root are the temp root.
fn collide_with(cwd: &Path, packs: &Path, env: &[(&str, &str)], args: &[&str]) -> Run {
    let root = TempRoot::new();
    // Dropped after the output is read: it ends the daemon this call started, by the pid on this call's own socket.
    let _stop = DaemonStop(DaemonPaths::under(root.path()));
    let shell = root.script("fake-shell", "#!/bin/sh\neval \"$4\"\n");
    let Output {
        status,
        stdout,
        stderr,
    } = Detached::start(
        &root,
        Command::new(env!("CARGO_BIN_EXE_collide"))
            .args(args)
            .current_dir(cwd)
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .env("HOME", root.path())
            .env("SHELL", shell)
            .env("COLLIDE_DAEMON_ROOT", root.path())
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("COLLIDE_BUILTIN_PACKS", packs)
            .envs(env.iter().copied())
            .stdin(Stdio::null()),
    )
    .wait();
    Run {
        code: status.code().expect("collide exits normally"),
        stdout: String::from_utf8(stdout).unwrap(),
        stderr: String::from_utf8(stderr).unwrap(),
    }
}

fn collide(repo: &Repo, bundle: &Bundle, args: &[&str]) -> Run {
    collide_with(&repo.dir, bundle, &[], args)
}

fn json(run: &Run) -> Value {
    serde_json::from_str(&run.stdout).unwrap_or_else(|e| panic!("{e}: {}", run.stdout))
}

fn ids(document: &Value) -> Vec<String> {
    document["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|task| task["id"].as_str().unwrap().to_string())
        .collect()
}

// Listing.

#[test]
fn tasks_lists_each_item_by_key_with_its_status_kind_and_title() {
    let (bundle, repo) = (bundle(), Repo::new("list"));
    let run = collide(&repo, &bundle, &["tasks"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert_eq!(
        run.stdout,
        "\
plan:ep-001-aa   active   epic  Alpha epic
plan:ep-002-bb   active   epic  Beta epic
plan:ts-0001-aa  done     task  First task
plan:ts-0002-bb  ready    task  Second task, free to start
plan:ts-0003-cc  ready    task  Third task, waits for the second
plan:ts-0004-ff  dropped  task  Dropped task
plan:ts-0005-gg  ready    task  Free, since its blocker is dropped
plan:ts-0006-hh  ready    task  Loop, first half
plan:ts-0007-ii  ready    task  Loop, second half
plan:ts-0008-jj  ready    task  Blocked by an id that is no item
plan:ts-0012-dd  ready    task  Twelve, in alpha
plan:ts-0012-ee  review   task  Twelve, in beta
"
    );
}

#[test]
fn a_cycle_and_an_unknown_blocker_are_reported_on_stderr_by_the_ready_view() {
    let (bundle, repo) = (bundle(), Repo::new("warnings"));
    let run = collide(&repo, &bundle, &["tasks", "--ready"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert!(
        run.stderr
            .contains("collide: blocked-by cycle, none is ready: plan:ts-0006-hh, plan:ts-0007-ii"),
        "{}",
        run.stderr
    );
    assert!(
        run.stderr.contains(
            "collide: plan:ts-0008-jj is blocked by ts-9999-zz, which is not an item; it is not ready"
        ),
        "{}",
        run.stderr
    );
}

#[test]
fn ready_lists_what_the_rule_allows_and_nothing_else() {
    let (bundle, repo) = (bundle(), Repo::new("ready"));
    let run = collide(&repo, &bundle, &["tasks", "--ready", "--json"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    // In the daemon's order (by key). Done and dropped blockers free an item; a ready blocker, a cycle and an
    // unknown blocker do not.
    assert_eq!(ids(&json(&run)), ["ts-0002-bb", "ts-0005-gg", "ts-0012-dd"]);
}

#[test]
fn json_is_the_daemons_list_with_the_epic_and_the_state_of_each_item() {
    let (bundle, repo) = (bundle(), Repo::new("json"));
    let document = json(&collide(&repo, &bundle, &["tasks", "--json"]));
    assert_eq!(document["invalid"], serde_json::json!([]));
    let tasks = document["tasks"].as_array().unwrap();
    let by_id = |id: &str| tasks.iter().find(|task| task["id"] == id).unwrap();
    assert_eq!(by_id("ts-0005-gg")["links"][0]["type"], "part-of");
    assert_eq!(by_id("ts-0005-gg")["state"]["category"], "ready");
    assert_eq!(by_id("ts-0004-ff")["state"]["category"], "dropped");
    assert_eq!(by_id("ep-001-aa")["kind"], "epic");
}

#[test]
fn kind_keeps_only_items_of_that_kind() {
    let (bundle, repo) = (bundle(), Repo::new("kind"));
    let epics = json(&collide(
        &repo,
        &bundle,
        &["tasks", "--kind", "epic", "--json"],
    ));
    assert_eq!(ids(&epics), ["ep-001-aa", "ep-002-bb"]);
    let tasks = json(&collide(
        &repo,
        &bundle,
        &["tasks", "--kind=task", "--json"],
    ));
    assert_eq!(ids(&tasks).len(), 10);
    let none = collide(&repo, &bundle, &["tasks", "--kind", "task", "--ready"]);
    assert_eq!(none.stdout.lines().count(), 3);
}

#[test]
fn an_unknown_kind_fails_listing_the_kinds_the_boards_sources_declare() {
    let (bundle, repo) = (bundle(), Repo::new("kind-unknown"));
    let run = collide(&repo, &bundle, &["tasks", "--kind", "widget"]);
    assert_eq!(run.code, 2, "{}", run.stderr);
    assert_eq!(run.stdout, "");
    assert!(
        run.stderr.contains(
            "unknown kind `widget`; the board's sources declare: task, epic, proposal, spike, question"
        ),
        "{}",
        run.stderr
    );
}

#[test]
fn the_ready_rule_is_applied_before_the_kind_filter() {
    // ts-0003-cc is blocked by a task, so it stays blocked when only tasks are shown; the epic filter must not
    // hide the blockers the rule needs.
    let (bundle, repo) = (bundle(), Repo::new("rule-then-filter"));
    let ready_epics = json(&collide(
        &repo,
        &bundle,
        &["tasks", "--ready", "--kind", "epic", "--json"],
    ));
    assert!(ids(&ready_epics).is_empty());
}

// Showing one item.

#[test]
fn task_shows_the_epic_beside_the_id_and_each_link_target_with_its_state() {
    let (bundle, repo) = (bundle(), Repo::new("show"));
    let run = collide(&repo, &bundle, &["task", "5"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert_eq!(
        run.stdout,
        "\
ep-002-bb / ts-0005-gg
  title   Free, since its blocker is dropped
  kind    task
  status  ready
  ready   yes
  path    .collide/planning/epics/ep-002-bb-beta/tasks/ts-0005-gg-after-dropped.md
  links
    blocked-by plan:ts-0004-ff (dropped)
    blocked-by plan:ts-0001-aa (done)

## Goal

Made-up content for ts-0005-gg.
"
    );
}

#[test]
fn task_says_why_an_item_is_not_ready() {
    let (bundle, repo) = (bundle(), Repo::new("why"));
    let blocked = collide(&repo, &bundle, &["task", "3"]).stdout;
    assert!(
        blocked.contains("  ready   no (waiting on ts-0002-bb (ready))"),
        "{blocked}"
    );
    let typo = collide(&repo, &bundle, &["task", "8"]).stdout;
    assert!(
        typo.contains("blocked-by plan:ts-9999-zz (not found)"),
        "{typo}"
    );
    let looped = collide(&repo, &bundle, &["task", "6"]).stdout;
    assert!(
        looped.contains("  ready   no (waiting on ts-0007-ii (ready))"),
        "{looped}"
    );
    let done = collide(&repo, &bundle, &["task", "ts-1"]).stdout;
    assert!(done.contains("  ready   no (status is done)"), "{done}");
}

#[test]
fn a_number_shared_by_two_items_lists_both_and_asks_for_the_suffix() {
    let (bundle, repo) = (bundle(), Repo::new("ambiguous"));
    let run = collide(&repo, &bundle, &["task", "12"]);
    assert_eq!(run.code, 3);
    assert!(run.stdout.is_empty());
    assert!(
        run.stderr.contains(
            "collide: `12` matches 2 items:\n  \
             ep-001-aa / plan:ts-0012-dd  task  Twelve, in alpha\n  \
             ep-002-bb / plan:ts-0012-ee  task  Twelve, in beta\n\
             add the suffix or the prefix, such as 0012-dd or ts-0012, or use the full id"
        ),
        "{}",
        run.stderr
    );
}

#[test]
fn a_short_form_with_the_suffix_resolves_one_of_two() {
    let (bundle, repo) = (bundle(), Repo::new("suffix"));
    for query in ["12-ee", "0012-ee", "ts-12-ee", "ts-0012-ee"] {
        let run = collide(&repo, &bundle, &["task", query]);
        assert_eq!(run.code, 0, "{query}: {}", run.stderr);
        assert!(
            run.stdout.starts_with("ep-002-bb / ts-0012-ee\n"),
            "{query}: {}",
            run.stdout
        );
    }
}

#[test]
fn an_id_no_item_has_names_what_was_tried() {
    let (bundle, repo) = (bundle(), Repo::new("missing"));
    let run = collide(&repo, &bundle, &["task", "99-zz"]);
    assert_eq!(run.code, 3);
    assert!(
        run.stderr.contains(
            "no item matches `99-zz`: looked for the id `99-zz` and for items with that number"
        ),
        "{}",
        run.stderr
    );
    assert!(run.stderr.contains("among the 12 items"), "{}", run.stderr);
}

#[test]
fn text_that_is_no_id_is_a_usage_error() {
    let (bundle, repo) = (bundle(), Repo::new("malformed"));
    let run = collide(&repo, &bundle, &["task", "first"]);
    assert_eq!(run.code, 2);
    assert!(
        run.stderr.contains("`first` is not an item id"),
        "{}",
        run.stderr
    );
}

// Writing.

#[test]
fn set_status_changes_one_line_of_one_file() {
    let (bundle, repo) = (bundle(), Repo::new("set-status"));
    let run = collide(&repo, &bundle, &["task", "3", "setStatus", "active"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert_eq!(
        run.stdout,
        "ok  ep-001-aa / ts-0003-cc\n  status  ready -> active\n  \
         file    .collide/planning/epics/ep-001-aa-alpha/tasks/ts-0003-cc-third.md\n"
    );
    assert_eq!(
        git(&repo.dir, &["status", "--porcelain"]),
        " M .collide/planning/epics/ep-001-aa-alpha/tasks/ts-0003-cc-third.md\n"
    );
    let diff = git(&repo.dir, &["diff", "-U0"]);
    let changed: Vec<&str> = diff
        .lines()
        .filter(|line| {
            (line.starts_with('+') || line.starts_with('-'))
                && !line.starts_with("+++")
                && !line.starts_with("---")
        })
        .collect();
    assert_eq!(changed, ["-status: ready", "+status: active"]);
}

#[test]
fn a_write_from_a_linked_worktree_lands_in_the_main_checkout() {
    let (bundle, repo) = (bundle(), Repo::new("worktree"));
    let linked = repo.dir.with_file_name("task-cli-worktree-linked");
    git(
        &repo.dir,
        &["worktree", "add", "-q", linked.to_str().unwrap()],
    );
    let run = collide_with(&linked, &bundle, &[], &["task", "3", "setStatus", "active"]);
    let main_changed = git(&repo.dir, &["status", "--porcelain"]);
    let linked_changed = git(&linked, &["status", "--porcelain"]);
    let _ = fs::remove_dir_all(&linked);
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert!(
        main_changed.contains("ts-0003-cc-third.md"),
        "{main_changed}"
    );
    assert!(linked_changed.is_empty(), "{linked_changed}");
}

#[test]
fn a_write_that_changes_nothing_says_so() {
    let (bundle, repo) = (bundle(), Repo::new("no-op"));
    let run = collide(&repo, &bundle, &["task", "3", "setStatus", "ready"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert_eq!(
        run.stdout,
        "ok  ep-001-aa / ts-0003-cc: nothing to change\n"
    );
    assert!(git(&repo.dir, &["status", "--porcelain"]).is_empty());
}

#[test]
fn claim_link_and_unclaim_report_what_changed() {
    let (bundle, repo) = (bundle(), Repo::new("claim-link"));
    let claim = collide(&repo, &bundle, &["task", "ts-2", "claim", "agent-one"]);
    assert_eq!(claim.code, 0, "{}", claim.stderr);
    assert!(
        claim.stdout.contains("  owners  (none) -> agent-one\n"),
        "{}",
        claim.stdout
    );
    let link = collide(&repo, &bundle, &["task", "ts-2", "link", "ts-0012-dd"]);
    assert_eq!(link.code, 0, "{}", link.stderr);
    assert!(
        link.stdout.contains("  links   +blocked-by ts-0012-dd\n"),
        "{}",
        link.stdout
    );
    let unclaim = collide(&repo, &bundle, &["task", "ts-2", "unclaim"]);
    assert_eq!(unclaim.code, 0, "{}", unclaim.stderr);
    assert!(
        unclaim.stdout.contains("  owners  agent-one -> (none)\n"),
        "{}",
        unclaim.stdout
    );
}

#[test]
fn a_claim_on_an_item_that_shows_an_owner_is_refused_with_the_providers_message() {
    let (bundle, repo) = (bundle(), Repo::new("claim-refused"));
    assert_eq!(
        collide(&repo, &bundle, &["task", "ts-2", "claim", "agent-one"]).code,
        0
    );
    let run = collide(&repo, &bundle, &["task", "ts-2", "claim", "agent-two"]);
    assert_eq!(run.code, 5);
    assert!(run.stdout.is_empty());
    assert!(run.stderr.contains("collide: refused: "), "{}", run.stderr);
    assert!(run.stderr.contains("agent-one"), "{}", run.stderr);
}

#[test]
fn write_json_prints_the_providers_result_and_keeps_the_exit_code() {
    let (bundle, repo) = (bundle(), Repo::new("write-json"));
    let ok = collide(
        &repo,
        &bundle,
        &["task", "3", "setStatus", "status=review", "--json"],
    );
    assert_eq!(ok.code, 0, "{}", ok.stderr);
    assert_eq!(json(&ok)["outcome"], "ok");
    assert_eq!(json(&ok)["task"]["state"]["status"], "review");
    assert_eq!(
        collide(&repo, &bundle, &["task", "3", "claim", "agent-one"]).code,
        0
    );
    let refused = collide(
        &repo,
        &bundle,
        &["task", "3", "claim", "agent-two", "--json"],
    );
    assert_eq!(refused.code, 5);
    assert_eq!(json(&refused)["outcome"], "refused");
}

#[test]
fn an_unknown_op_fails_listing_the_ops_the_provider_declares() {
    let (bundle, repo) = (bundle(), Repo::new("unknown-op"));
    let run = collide(&repo, &bundle, &["task", "3", "explode"]);
    assert_eq!(run.code, 2);
    assert!(
        run.stderr
            .contains("unknown op `explode` for plan:ts-0003-cc (task); its source declares:"),
        "{}",
        run.stderr
    );
    for op in ["setStatus", "link", "claim", "unclaim", "reid"] {
        assert!(
            run.stderr.contains(&format!("\n  {op}")),
            "{op} in {}",
            run.stderr
        );
    }
    // `create` starts from no item, so it is not offered for one.
    assert!(!run.stderr.contains("\n  create"), "{}", run.stderr);
    assert!(git(&repo.dir, &["status", "--porcelain"]).is_empty());
}

#[test]
fn a_bad_argument_is_a_usage_error_and_writes_nothing() {
    let (bundle, repo) = (bundle(), Repo::new("bad-arg"));
    let run = collide(&repo, &bundle, &["task", "3", "setStatus", "finished"]);
    assert_eq!(run.code, 2);
    assert!(
        run.stderr
            .contains("`status` must be one of ready, active, review, done, dropped"),
        "{}",
        run.stderr
    );
    assert!(git(&repo.dir, &["status", "--porcelain"]).is_empty());
}

// Usage and setup failures.

#[test]
fn a_wrong_command_line_prints_usage_and_exits_2() {
    let (bundle, repo) = (bundle(), Repo::new("usage"));
    for args in [
        vec!["tasks", "--bogus"],
        vec!["tasks", "extra"],
        vec!["tasks", "--kind"],
        vec!["task"],
        vec!["task", "reid"],
        vec!["task", "create", "--seed"],
        vec!["task", "3", "setStatus", "--no-verify"],
        vec!["nonsense"],
    ] {
        let run = collide(&repo, &bundle, &args);
        assert_eq!(run.code, 2, "{args:?}: {}", run.stderr);
        assert!(run.stderr.contains("usage:"), "{args:?}: {}", run.stderr);
    }
}

#[test]
fn help_prints_the_usage_the_exit_codes_and_the_forms_that_go_through_the_daemon() {
    let run = collide_with(&std::env::temp_dir(), Path::new("/nowhere"), &[], &["help"]);
    assert_eq!(run.code, 0);
    assert!(run.stdout.contains(
        "collide tasks [--ready] [--kind <kind>] [--view <name>] [--checkout <path>] [--json]"
    ));
    for line in [
        "exit codes: 0 ok, 1 failed (also: the write was made but --commit's commit failed; no daemon), 2 wrong",
        "4 conflict (nothing was
written), 5 refused by the provider,",
        "12 a person-only op: a confirmation waits in the app",
    ] {
        assert!(run.stdout.contains(line), "{line} in {}", run.stdout);
    }
    assert!(!run.stdout.contains("COLLIDE_BUILTIN_PACKS"));
    for line in [
        "collide task create <title> [epic=<epic>] [kind=task|epic|proposal|spike|question] [seed=<number>]",
        "collide task reid <id> [seed=<number>] [--checkout <path>] [--commit] [--json]",
        "collide task renameSection <from> <to> [--checkout <path>] [--commit] [--json]",
        "The new file is written; without --commit nothing is
  committed.",
        "--commit",
        "--checkout <path>",
    ] {
        assert!(run.stdout.contains(line), "{line} in {}", run.stdout);
    }
}

#[test]
fn outside_a_repository_the_error_says_where_it_looked_from() {
    let bundle = bundle();
    let outside =
        std::env::temp_dir().join(format!("collide-task-cli-outside-{}", std::process::id()));
    fs::create_dir_all(&outside).unwrap();
    let run = collide_with(&outside, &bundle, &[], &["tasks"]);
    let _ = fs::remove_dir_all(&outside);
    assert_eq!(run.code, 1);
    assert!(
        run.stderr.contains("is not inside a git repository"),
        "{}",
        run.stderr
    );
    assert!(
        run.stderr
            .contains(outside.file_name().unwrap().to_str().unwrap()),
        "{}",
        run.stderr
    );
}

// Create and reid (ts-0030).

const ALPHA_TASKS: &str = ".collide/planning/epics/ep-001-aa-alpha/tasks";

fn commit_count(dir: &Path) -> usize {
    git(dir, &["rev-list", "--count", "--all"])
        .trim()
        .parse()
        .unwrap()
}

#[test]
fn create_writes_one_new_file_prints_it_and_commits_nothing() {
    let (bundle, repo) = (bundle(), Repo::new("create"));
    let commits = commit_count(&repo.dir);
    let run = collide(
        &repo,
        &bundle,
        &[
            "task",
            "create",
            "Fresh item, for the test",
            "epic=ep-001-aa",
            "seed=1",
        ],
    );
    assert_eq!(run.code, 0, "{}", run.stderr);
    let file = format!("{ALPHA_TASKS}/ts-0013-ra-fresh-item-for-the-test.md");
    assert_eq!(
        run.stdout,
        format!(
            "ok  created ep-001-aa / ts-0013-ra\n  title   Fresh item, for the test\n  file    {file}\n"
        )
    );
    assert_eq!(
        git(&repo.dir, &["status", "--porcelain"]),
        format!("?? {file}\n")
    );
    assert_eq!(commit_count(&repo.dir), commits);
    // The new item is listed, with its epic.
    let shown = collide(&repo, &bundle, &["task", "13-ra"]);
    assert_eq!(shown.code, 0, "{}", shown.stderr);
    assert!(shown.stdout.starts_with("ep-001-aa / ts-0013-ra\n"));
}

#[test]
fn create_takes_the_epic_by_a_short_form_and_makes_epics() {
    let (bundle, repo) = (bundle(), Repo::new("create-epic"));
    let task = collide(
        &repo,
        &bundle,
        &["task", "create", "title=In beta", "epic=2", "seed=1"],
    );
    assert_eq!(task.code, 0, "{}", task.stderr);
    assert!(
        task.stdout
            .starts_with("ok  created ep-002-bb / ts-0013-ra\n"),
        "{}",
        task.stdout
    );
    let epic = collide(
        &repo,
        &bundle,
        &["task", "create", "Gamma epic", "kind=epic", "seed=1"],
    );
    assert_eq!(epic.code, 0, "{}", epic.stderr);
    assert_eq!(
        epic.stdout,
        "ok  created ep-003-ra\n  title   Gamma epic\n  \
         file    .collide/planning/epics/ep-003-ra-gamma-epic/epic.md\n"
    );
}

#[test]
fn create_with_json_prints_the_providers_result() {
    let (bundle, repo) = (bundle(), Repo::new("create-json"));
    let run = collide(
        &repo,
        &bundle,
        &[
            "task",
            "create",
            "Json item",
            "epic=ep-001-aa",
            "seed=1",
            "--json",
        ],
    );
    assert_eq!(run.code, 0, "{}", run.stderr);
    let result = json(&run);
    assert_eq!(result["outcome"], "ok");
    assert_eq!(result["task"]["id"], "ts-0013-ra");
    assert_eq!(
        result["files"],
        serde_json::json!([format!("{ALPHA_TASKS}/ts-0013-ra-json-item.md")])
    );
}

#[test]
fn a_bad_create_is_refused_or_a_usage_error_and_writes_nothing() {
    let (bundle, repo) = (bundle(), Repo::new("create-bad"));
    let cases: [(&[&str], i32, &str); 7] = [
        (&["task", "create"], 2, "needs title"),
        (&["task", "create", "No epic"], 5, "a task needs epic"),
        (
            &["task", "create", "T", "epic=ep-009-zz"],
            3,
            "no item matches `ep-009-zz`",
        ),
        (
            &["task", "create", "T", "epic=ts-1"],
            2,
            "names plan:ts-0001-aa, which is a task, not an epic",
        ),
        (&["task", "create", "T", "epic=9"], 3, "no item matches `9`"),
        (
            &["task", "create", "T", "epic=ep-1", "speed=3"],
            2,
            "has no argument `speed`",
        ),
        (
            &["task", "3", "create", "T"],
            2,
            "`create` makes a new item",
        ),
    ];
    for (args, code, message) in cases {
        let run = collide(&repo, &bundle, args);
        assert_eq!(run.code, code, "{args:?}: {}", run.stderr);
        assert!(run.stderr.contains(message), "{args:?}: {}", run.stderr);
    }
    assert!(git(&repo.dir, &["status", "--porcelain"]).is_empty());
}

#[test]
fn two_branches_that_make_the_same_number_both_load_after_the_merge() {
    let (bundle, repo) = (bundle(), Repo::new("same-number"));
    let second = repo.dir.with_file_name("task-cli-same-number-second");
    let _ = fs::remove_dir_all(&second);
    git(
        &repo.dir,
        &[
            "clone",
            "-q",
            repo.dir.to_str().unwrap(),
            second.to_str().unwrap(),
        ],
    );
    let second = Repo { dir: second };

    // Neither clone has fetched from the other, so neither sees the other's item and both take number 13.
    git(&repo.dir, &["checkout", "-q", "-b", "branch-a"]);
    let a = collide(
        &repo,
        &bundle,
        &["task", "create", "Made on a", "epic=ep-001-aa", "seed=1"],
    );
    assert_eq!(a.code, 0, "{}", a.stderr);
    git(&repo.dir, &["add", "-A"]);
    git(&repo.dir, &["commit", "-q", "-m", "a"]);
    git(&second.dir, &["checkout", "-q", "-b", "branch-b"]);
    let b = collide(
        &second,
        &bundle,
        &["task", "create", "Made on b", "epic=ep-001-aa", "seed=2"],
    );
    assert_eq!(b.code, 0, "{}", b.stderr);
    git(&second.dir, &["add", "-A"]);
    git(&second.dir, &["commit", "-q", "-m", "b"]);
    git(&second.dir, &["fetch", "-q", "origin"]);
    git(
        &second.dir,
        &["merge", "-q", "--no-edit", "origin/branch-a"],
    );

    let line = |run: &Run| run.stdout.lines().next().unwrap().to_string();
    let (id_a, id_b) = (
        line(&a).replace("ok  created ep-001-aa / ", ""),
        line(&b).replace("ok  created ep-001-aa / ", ""),
    );
    assert!(id_a.starts_with("ts-0013-") && id_b.starts_with("ts-0013-") && id_a != id_b);

    let listed = collide(&second, &bundle, &["tasks", "--json"]);
    assert_eq!(listed.code, 0, "{}", listed.stderr);
    let document = json(&listed);
    assert_eq!(document["invalid"], serde_json::json!([]));
    let all = ids(&document);
    assert!(all.contains(&id_a) && all.contains(&id_b), "{all:?}");
    let twelve = collide(&second, &bundle, &["task", "13"]);
    assert_eq!(twelve.code, 3);
    assert!(
        twelve.stderr.contains("`13` matches 2 items:")
            && twelve.stderr.contains(&format!("ep-001-aa / plan:{id_a}"))
            && twelve.stderr.contains(&format!("ep-001-aa / plan:{id_b}")),
        "{}",
        twelve.stderr
    );
}

/// The fixture repository with a second file that has ts-0002-bb's id, committed. It sorts after the original, so
/// the scan sets it aside as the task.invalid.
fn repo_with_a_shared_id(name: &str) -> (Repo, String) {
    let repo = Repo::new(name);
    let second = format!("{ALPHA_TASKS}/ts-0002-bb-second.md");
    let copy = format!("{ALPHA_TASKS}/ts-0002-bb-zcopy.md");
    fs::copy(repo.dir.join(&second), repo.dir.join(&copy)).unwrap();
    git(&repo.dir, &["add", "-A"]);
    git(
        &repo.dir,
        &["commit", "-q", "-m", "a second file with the id"],
    );
    (repo, copy)
}

#[test]
fn reid_on_a_shared_id_renames_the_second_file_rewrites_links_and_commits_nothing() {
    let (bundle, (repo, copy)) = (bundle(), repo_with_a_shared_id("reid"));
    let listed = collide(&repo, &bundle, &["tasks"]);
    assert!(
        listed.stderr.contains(&format!(
            "collide: skipped {copy}: id \"ts-0002-bb\" is already used by {ALPHA_TASKS}/ts-0002-bb-second.md"
        )),
        "{}",
        listed.stderr
    );
    let commits = commit_count(&repo.dir);

    let run = collide(&repo, &bundle, &["task", "reid", "ts-0002-bb", "seed=1"]);

    assert_eq!(run.code, 0, "{}", run.stderr);
    let renamed = format!("{ALPHA_TASKS}/ts-0002-ra-zcopy.md");
    let third = format!("{ALPHA_TASKS}/ts-0003-cc-third.md");
    assert_eq!(
        run.stdout,
        format!(
            "ok  ep-001-aa / ts-0002-ra\n  id      plan:ts-0002-bb -> plan:ts-0002-ra\n  \
             links   rewritten in plan:ts-0003-cc\n  file    {third}\n  file    {renamed}\n  removed {copy}\n\
             nothing is committed; review the changes with git diff and git status\n"
        )
    );
    assert_eq!(commit_count(&repo.dir), commits);
    let status = git(&repo.dir, &["status", "--porcelain"]);
    let mut lines: Vec<&str> = status.lines().collect();
    lines.sort();
    assert_eq!(
        lines,
        [
            format!(" D {copy}"),
            format!(" M {third}"),
            format!("?? {renamed}")
        ]
    );
    // Only the link's target changed in the one file that linked to it.
    let diff = git(&repo.dir, &["diff", "-U0", "--", &third]);
    let changed: Vec<&str> = diff
        .lines()
        .filter(|l| {
            (l.starts_with('+') || l.starts_with('-'))
                && !l.starts_with("+++")
                && !l.starts_with("---")
        })
        .collect();
    assert_eq!(
        changed,
        [
            "-  - { type: blocked-by, to: ts-0002-bb }",
            "+  - { type: blocked-by, to: ts-0002-ra }"
        ]
    );
    // Nothing is skipped now, and the first file still has the old id.
    let after = collide(&repo, &bundle, &["tasks", "--json"]);
    assert_eq!(after.code, 0, "{}", after.stderr);
    assert!(!after.stderr.contains("skipped"), "{}", after.stderr);
    let document = json(&after);
    assert_eq!(document["invalid"], serde_json::json!([]));
    let all = ids(&document);
    assert!(all.contains(&"ts-0002-bb".to_string()) && all.contains(&"ts-0002-ra".to_string()));
}

#[test]
fn reid_is_the_same_as_the_id_followed_by_reid() {
    let (bundle, (repo, _copy)) = (bundle(), repo_with_a_shared_id("reid-order"));
    let run = collide(&repo, &bundle, &["task", "2", "reid", "seed=1", "--json"]);
    // `2` is ambiguous with the epic `ep-002-bb`; the full id is not.
    assert_eq!(run.code, 3, "{}", run.stderr);
    let run = collide(
        &repo,
        &bundle,
        &["task", "ts-0002-bb", "reid", "seed=1", "--json"],
    );
    assert_eq!(run.code, 0, "{}", run.stderr);
    let result = json(&run);
    assert_eq!(result["from"], "plan:ts-0002-bb");
    assert_eq!(result["result"]["outcome"], "ok");
    assert_eq!(result["result"]["task"]["id"], "ts-0002-ra");
    assert_eq!(result["rewritten"], serde_json::json!(["plan:ts-0003-cc"]));
}

#[test]
fn reid_of_an_id_nobody_has_exits_3_and_writes_nothing() {
    let (bundle, repo) = (bundle(), Repo::new("reid-none"));
    let run = collide(&repo, &bundle, &["task", "reid", "ts-0099-zz"]);
    assert_eq!(run.code, 3);
    assert!(git(&repo.dir, &["status", "--porcelain"]).is_empty());
}

// Where a write lands and when it is committed (ts-0031).

/// What `git` needs to commit in a temp repository, set on the `collide` process (never in a git config).
const IDENTITY: [(&str, &str); 6] = [
    ("GIT_AUTHOR_NAME", "test"),
    ("GIT_AUTHOR_EMAIL", "test@example.invalid"),
    ("GIT_COMMITTER_NAME", "test"),
    ("GIT_COMMITTER_EMAIL", "test@example.invalid"),
    ("GIT_CONFIG_GLOBAL", "/dev/null"),
    ("GIT_CONFIG_SYSTEM", "/dev/null"),
];

fn collide_in(cwd: &Path, bundle: &Bundle, args: &[&str]) -> Run {
    collide_with(cwd, bundle, &IDENTITY, args)
}

/// A linked worktree of a fixture repository, on its own branch, removed when dropped.
struct Linked {
    dir: PathBuf,
}

impl Linked {
    fn new(repo: &Repo) -> Self {
        let name = repo.dir.file_name().unwrap().to_str().unwrap();
        let dir = repo.dir.with_file_name(format!("{name}-linked"));
        let _ = fs::remove_dir_all(&dir);
        git(
            &repo.dir,
            &["worktree", "add", "-q", "-b", "side", dir.to_str().unwrap()],
        );
        Self { dir }
    }
}

impl Drop for Linked {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

const THIRD: &str = ".collide/planning/epics/ep-001-aa-alpha/tasks/ts-0003-cc-third.md";
const SECOND: &str = ".collide/planning/epics/ep-001-aa-alpha/tasks/ts-0002-bb-second.md";
const TWELVE: &str = ".collide/planning/epics/ep-001-aa-alpha/tasks/ts-0012-dd-twelve-a.md";

/// The files the newest commit changed, as `status<TAB>path` lines (no rename detection), sorted.
fn committed(dir: &Path) -> Vec<String> {
    let text = git(
        dir,
        &["show", "--no-renames", "--name-status", "--format=", "HEAD"],
    );
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    lines.sort();
    lines
}

fn head_subject(dir: &Path) -> String {
    git(dir, &["log", "-1", "--format=%s"]).trim().to_string()
}

fn text_of(dir: &Path, file: &str) -> String {
    fs::read_to_string(dir.join(file)).unwrap()
}

#[test]
fn checkout_writes_into_that_worktree_and_leaves_the_main_checkout_alone() {
    let (bundle, repo) = (bundle(), Repo::new("checkout-into"));
    let linked = Linked::new(&repo);
    let commits = commit_count(&repo.dir);
    let path = linked.dir.to_str().unwrap();
    let run = collide_in(
        &repo.dir,
        &bundle,
        &["task", "3", "setStatus", "active", "--checkout", path],
    );
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert_eq!(
        git(&linked.dir, &["status", "--porcelain"]),
        format!(" M {THIRD}\n")
    );
    assert!(text_of(&linked.dir, THIRD).contains("status: active"));
    assert!(git(&repo.dir, &["status", "--porcelain"]).is_empty());
    assert_eq!(commit_count(&repo.dir), commits);
}

#[test]
fn checkout_reads_the_item_in_that_worktree_so_one_that_exists_only_there_is_found() {
    let (bundle, repo) = (bundle(), Repo::new("checkout-only-there"));
    let linked = Linked::new(&repo);
    let path = linked.dir.to_str().unwrap();
    let made = collide_in(
        &repo.dir,
        &bundle,
        &[
            "task",
            "create",
            "Only in the worktree",
            "epic=ep-001-aa",
            "seed=1",
            "--checkout",
            path,
        ],
    );
    assert_eq!(made.code, 0, "{}", made.stderr);
    assert!(
        made.stdout.contains("ts-0013-ra"),
        "{}{}",
        made.stdout,
        made.stderr
    );
    assert!(git(&repo.dir, &["status", "--porcelain"]).is_empty());
    let set = collide_in(
        &repo.dir,
        &bundle,
        &["task", "ts-0013-ra", "setStatus", "active", "--checkout="],
    );
    // An empty path is no checkout.
    assert_eq!(set.code, 2, "{}", set.stderr);
    let set = collide_in(
        &repo.dir,
        &bundle,
        &[
            "task",
            "ts-0013-ra",
            "setStatus",
            "active",
            "--checkout",
            path,
        ],
    );
    assert_eq!(set.code, 0, "{}", set.stderr);
    let from_main = collide_in(
        &repo.dir,
        &bundle,
        &["task", "ts-0013-ra", "setStatus", "active"],
    );
    // The board lists the copy in the worktree, so the id is found; the main checkout has no such file, and the
    // provider refuses the write there.
    assert_eq!(from_main.code, 5, "{}", from_main.stderr);
    assert!(
        from_main.stderr.contains("there is no item ts-0013-ra"),
        "{}",
        from_main.stderr
    );
}

#[test]
fn checkout_may_be_given_relative_to_the_working_directory_and_may_be_the_main_checkout() {
    let (bundle, repo) = (bundle(), Repo::new("checkout-forms"));
    let linked = Linked::new(&repo);
    let run = collide_in(
        &repo.dir,
        &bundle,
        &[
            "task",
            "3",
            "setStatus",
            "active",
            "--checkout",
            &format!("../{}", linked.dir.file_name().unwrap().to_str().unwrap()),
        ],
    );
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert!(text_of(&linked.dir, THIRD).contains("status: active"));
    // The `--checkout=<path>` form.
    let name = linked.dir.file_name().unwrap().to_str().unwrap();
    let run = collide_in(
        &repo.dir,
        &bundle,
        &[
            "task",
            "ts-0012-dd",
            "setStatus",
            "active",
            &format!("--checkout=../{name}"),
        ],
    );
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert!(text_of(&linked.dir, TWELVE).contains("status: active"));
    // From the linked worktree, the main checkout is named by its path.
    let run = collide_in(
        &linked.dir,
        &bundle,
        &[
            "task",
            "ts-0002-bb",
            "setStatus",
            "active",
            "--checkout",
            repo.dir.to_str().unwrap(),
        ],
    );
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert!(text_of(&repo.dir, SECOND).contains("status: active"));
    assert!(!text_of(&linked.dir, SECOND).contains("status: active"));
}

#[test]
fn a_path_that_is_not_a_checkout_of_the_repository_is_refused_and_nothing_is_written() {
    let (bundle, repo) = (bundle(), Repo::new("checkout-refused"));
    let linked = Linked::new(&repo);
    let stranger =
        std::env::temp_dir().join(format!("collide-not-a-checkout-{}", std::process::id()));
    fs::create_dir_all(&stranger).unwrap();
    let inside = repo.dir.join(".collide");
    let stranger_text = stranger.to_str().unwrap();
    let missing = stranger.join("missing");
    let missing_text = missing.to_str().unwrap();
    for given in [
        stranger_text,
        inside.to_str().unwrap(),
        missing_text,
        "no-such-folder",
    ] {
        let run = collide_in(
            &repo.dir,
            &bundle,
            &["task", "3", "setStatus", "active", "--checkout", given],
        );
        assert_eq!(run.code, 2, "{given}: {}", run.stderr);
        assert!(run.stdout.is_empty(), "{given}: {}", run.stdout);
        assert!(
            run.stderr.contains("--checkout") && run.stderr.contains("not a checkout"),
            "{given}: {}",
            run.stderr
        );
    }
    let create = collide_in(
        &repo.dir,
        &bundle,
        &[
            "task",
            "create",
            "Nowhere",
            "epic=ep-001-aa",
            "--checkout",
            stranger_text,
        ],
    );
    assert_eq!(create.code, 2, "{}", create.stderr);
    let entries: Vec<_> = fs::read_dir(&stranger).unwrap().collect();
    let _ = fs::remove_dir_all(&stranger);
    assert!(entries.is_empty());
    assert!(git(&repo.dir, &["status", "--porcelain"]).is_empty());
    assert!(git(&linked.dir, &["status", "--porcelain"]).is_empty());
}

#[test]
fn the_default_root_is_the_main_checkout_also_with_commit_from_a_worktree() {
    let (bundle, repo) = (bundle(), Repo::new("default-root-commit"));
    let linked = Linked::new(&repo);
    let linked_commits = commit_count(&linked.dir);
    let run = collide_in(
        &linked.dir,
        &bundle,
        &["task", "3", "setStatus", "active", "--commit"],
    );
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert_eq!(committed(&repo.dir), [format!("M\t{THIRD}")]);
    assert!(git(&repo.dir, &["status", "--porcelain"]).is_empty());
    assert!(git(&linked.dir, &["status", "--porcelain"]).is_empty());
    assert!(!text_of(&linked.dir, THIRD).contains("status: active"));
    // The commit went to the main checkout's branch, not to the worktree's.
    assert_eq!(
        git(&linked.dir, &["rev-list", "--count", "HEAD"]).trim(),
        linked_commits.to_string()
    );
}

#[test]
fn commit_commits_into_the_checkout_that_was_written() {
    let (bundle, repo) = (bundle(), Repo::new("commit-checkout"));
    let linked = Linked::new(&repo);
    let main_before = git(&repo.dir, &["rev-parse", "HEAD"]);
    let run = collide_in(
        &repo.dir,
        &bundle,
        &[
            "task",
            "3",
            "setStatus",
            "active",
            "--commit",
            "--checkout",
            linked.dir.to_str().unwrap(),
        ],
    );
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert_eq!(committed(&linked.dir), [format!("M\t{THIRD}")]);
    assert_eq!(git(&repo.dir, &["rev-parse", "HEAD"]), main_before);
    assert!(git(&linked.dir, &["status", "--porcelain"]).is_empty());
}

#[test]
fn commit_takes_exactly_the_reported_file_beside_an_unrelated_dirty_and_an_unrelated_staged_file() {
    let (bundle, repo) = (bundle(), Repo::new("commit-exact"));
    let dir = &repo.dir;
    fs::write(
        dir.join(SECOND),
        format!("{}\nan unstaged edit\n", text_of(dir, SECOND)),
    )
    .unwrap();
    fs::write(
        dir.join(TWELVE),
        format!("{}\na staged edit\n", text_of(dir, TWELVE)),
    )
    .unwrap();
    git(dir, &["add", "--", TWELVE]);
    fs::write(dir.join("notes.txt"), "untracked\n").unwrap();
    let before = commit_count(dir);

    let run = collide_in(
        dir,
        &bundle,
        &["task", "3", "setStatus", "active", "--commit"],
    );
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert_eq!(commit_count(dir), before + 1);
    assert_eq!(committed(dir), [format!("M\t{THIRD}")]);
    let stat = git(dir, &["show", "--stat", "--format=", "HEAD"]);
    assert!(
        stat.contains("ts-0003-cc-third.md")
            && !stat.contains("ts-0002")
            && !stat.contains("ts-0012"),
        "{stat}"
    );
    // The unrelated edits are as they were: one unstaged, one still staged, one untracked.
    assert_eq!(
        git(dir, &["status", "--porcelain"]),
        format!(" M {SECOND}\nM  {TWELVE}\n?? notes.txt\n")
    );
    assert!(run.stdout.contains("commit"), "{}", run.stdout);
}

#[test]
fn the_commit_message_names_the_op_and_the_item() {
    let (bundle, repo) = (bundle(), Repo::new("commit-message"));
    let run = collide_in(
        &repo.dir,
        &bundle,
        &["task", "3", "setStatus", "active", "--commit"],
    );
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert_eq!(
        head_subject(&repo.dir),
        "task ts-0003-cc setStatus: Third task, waits for the second"
    );
}

#[test]
fn without_commit_the_file_is_modified_and_nothing_is_committed() {
    let (bundle, repo) = (bundle(), Repo::new("no-commit"));
    let before = git(&repo.dir, &["rev-parse", "HEAD"]);
    let run = collide_in(&repo.dir, &bundle, &["task", "3", "setStatus", "active"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert_eq!(
        git(&repo.dir, &["status", "--porcelain"]),
        format!(" M {THIRD}\n")
    );
    assert_eq!(git(&repo.dir, &["rev-parse", "HEAD"]), before);
    assert!(!run.stdout.contains("commit"), "{}", run.stdout);
}

#[test]
fn an_op_that_changes_nothing_commits_nothing() {
    let (bundle, repo) = (bundle(), Repo::new("commit-nothing"));
    let before = git(&repo.dir, &["rev-parse", "HEAD"]);
    let run = collide_in(
        &repo.dir,
        &bundle,
        &["task", "3", "setStatus", "ready", "--commit"],
    );
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert_eq!(
        run.stdout,
        "ok  ep-001-aa / ts-0003-cc: nothing to change\n"
    );
    assert_eq!(git(&repo.dir, &["rev-parse", "HEAD"]), before);
}

#[test]
fn a_refused_claim_with_commit_writes_nothing_and_commits_nothing() {
    let (bundle, repo) = (bundle(), Repo::new("commit-refused"));
    let first = collide_in(
        &repo.dir,
        &bundle,
        &["task", "ts-2", "claim", "agent-one", "--commit"],
    );
    assert_eq!(first.code, 0, "{}", first.stderr);
    let before = git(&repo.dir, &["rev-parse", "HEAD"]);
    let run = collide_in(
        &repo.dir,
        &bundle,
        &["task", "ts-2", "claim", "agent-two", "--commit"],
    );
    assert_eq!(run.code, 5, "{}", run.stderr);
    assert!(run.stderr.contains("agent-one"), "{}", run.stderr);
    assert!(run.stdout.is_empty(), "{}", run.stdout);
    assert_eq!(git(&repo.dir, &["rev-parse", "HEAD"]), before);
    assert!(git(&repo.dir, &["status", "--porcelain"]).is_empty());
}

#[test]
fn commit_after_create_commits_the_new_file_and_not_an_unrelated_dirty_one() {
    let (bundle, repo) = (bundle(), Repo::new("commit-create"));
    let dir = &repo.dir;
    fs::write(
        dir.join(SECOND),
        format!("{}\nan unstaged edit\n", text_of(dir, SECOND)),
    )
    .unwrap();
    let run = collide_in(
        dir,
        &bundle,
        &[
            "task",
            "create",
            "Fresh item, for the test",
            "epic=ep-001-aa",
            "seed=1",
            "--commit",
        ],
    );
    assert_eq!(run.code, 0, "{}", run.stderr);
    let file = format!("{ALPHA_TASKS}/ts-0013-ra-fresh-item-for-the-test.md");
    assert_eq!(committed(dir), [format!("A\t{file}")]);
    assert_eq!(
        git(dir, &["status", "--porcelain"]),
        format!(" M {SECOND}\n")
    );
    assert_eq!(
        head_subject(dir),
        "task ts-0013-ra create: Fresh item, for the test"
    );
}

#[test]
fn commit_after_reid_commits_the_new_and_the_removed_paths_and_the_rewritten_links() {
    let (bundle, (repo, copy)) = (bundle(), repo_with_a_shared_id("commit-reid"));
    let dir = &repo.dir;
    fs::write(dir.join("notes.txt"), "untracked\n").unwrap();
    let run = collide_in(
        dir,
        &bundle,
        &["task", "ts-0002-bb", "reid", "seed=1", "--commit"],
    );
    assert_eq!(run.code, 0, "{}", run.stderr);
    let renamed = copy.replace("ts-0002-bb", "ts-0002-ra");
    let lines = committed(dir);
    assert!(lines.contains(&format!("D\t{copy}")), "{lines:?}");
    assert!(lines.contains(&format!("A\t{renamed}")), "{lines:?}");
    assert_eq!(git(dir, &["status", "--porcelain"]), "?? notes.txt\n");
    assert!(
        head_subject(dir).starts_with("task ts-0002-ra reid"),
        "{}",
        head_subject(dir)
    );
}

#[test]
fn commit_after_reid_of_a_file_that_was_never_committed_commits_only_the_new_path() {
    let (bundle, repo) = (bundle(), Repo::new("commit-reid-new"));
    let dir = &repo.dir;
    let made = collide_in(
        dir,
        &bundle,
        &["task", "create", "Fresh item", "epic=ep-001-aa", "seed=1"],
    );
    assert_eq!(made.code, 0, "{}", made.stderr);
    let run = collide_in(
        dir,
        &bundle,
        &["task", "ts-0013-ra", "reid", "seed=2", "--commit"],
    );
    assert_eq!(run.code, 0, "{}", run.stderr);
    let lines = committed(dir);
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(
        lines[0].starts_with("A\t") && lines[0].contains("/ts-0013-"),
        "{lines:?}"
    );
    assert!(git(dir, &["status", "--porcelain"]).is_empty());
}

#[test]
fn a_commit_that_fails_is_reported_leaves_the_file_written_and_exits_1() {
    let (bundle, repo) = (bundle(), Repo::new("commit-fails"));
    let hook = repo.dir.join(".git/hooks/pre-commit");
    fs::write(&hook, "#!/bin/sh\necho 'the hook says no' >&2\nexit 1\n").unwrap();
    fs::set_permissions(&hook, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    let before = git(&repo.dir, &["rev-parse", "HEAD"]);
    let run = collide_in(
        &repo.dir,
        &bundle,
        &["task", "3", "setStatus", "active", "--commit"],
    );
    assert_eq!(run.code, 1, "{}", run.stderr);
    assert!(
        run.stdout.contains("ok  ep-001-aa / ts-0003-cc"),
        "{}",
        run.stdout
    );
    assert!(run.stderr.contains("the hook says no"), "{}", run.stderr);
    assert!(run.stderr.contains("git commit failed"), "{}", run.stderr);
    assert_eq!(git(&repo.dir, &["rev-parse", "HEAD"]), before);
    assert!(text_of(&repo.dir, THIRD).contains("status: active"));
    assert_eq!(
        git(&repo.dir, &["status", "--porcelain"]),
        format!(" M {THIRD}\n")
    );
}

#[test]
fn commit_and_checkout_need_an_op_and_checkout_needs_a_path() {
    let (bundle, repo) = (bundle(), Repo::new("commit-usage"));
    for args in [
        &["task", "3", "--commit"][..],
        &["task", "3", "--checkout", "."][..],
        &["task", "3", "setStatus", "active", "--checkout"][..],
        &["tasks", "--commit"][..],
    ] {
        let run = collide_in(&repo.dir, &bundle, args);
        assert_eq!(run.code, 2, "{args:?}: {}", run.stderr);
        assert!(run.stderr.contains("usage:"), "{args:?}: {}", run.stderr);
    }
    assert!(git(&repo.dir, &["status", "--porcelain"]).is_empty());
}

#[test]
fn commit_works_with_json_and_keeps_the_providers_result_on_stdout() {
    let (bundle, repo) = (bundle(), Repo::new("commit-json"));
    let run = collide_in(
        &repo.dir,
        &bundle,
        &["task", "3", "setStatus", "active", "--commit", "--json"],
    );
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert_eq!(json(&run)["outcome"], "ok");
    assert_eq!(committed(&repo.dir), [format!("M\t{THIRD}")]);
}

// Proposals and their features (ts-0047).

const ALPHA_PROPOSALS: &str = ".collide/planning/epics/ep-001-aa-alpha/proposals";
const PROPOSAL_FILE: &str = "pr-0003-km-features-in-proposals.md";

fn proposal_text(status: &str, features: &str) -> String {
    format!(
        "---\nid: pr-0003-km\nkind: proposal\nstatus: {status}\ntitle: \"Features in proposals\"\nowners: []\nlinks: []\n---\n\n\
         ## Context\n\nMade-up context.\n\n## Features\n\n{features}\n## Decision\n"
    )
}

const FEATURES_EXAMPLE: &str = "\
### Packs
- **F1 Trust prompt for repo packs**: a repo pack asks before its first run.
  A continuation line is indented by two spaces.
- **F2 Pack lock**: `packs.lock` pins each pack to a tree.

### Unsorted
- **F3 Menu bar badge**: the Queue count in the menu bar.
";

/// A fixture repository with a proposal written into the alpha epic's `proposals/` folder and committed.
fn repo_with_proposal(name: &str, status: &str, features: &str) -> Repo {
    let repo = Repo::new(name);
    let folder = repo.dir.join(ALPHA_PROPOSALS);
    fs::create_dir_all(&folder).unwrap();
    fs::write(folder.join(PROPOSAL_FILE), proposal_text(status, features)).unwrap();
    git(&repo.dir, &["add", "-A"]);
    git(&repo.dir, &["commit", "-q", "-m", "a proposal"]);
    repo
}

#[test]
fn a_proposal_lists_as_kind_proposal_with_its_status_and_category() {
    let (bundle, repo) = (
        bundle(),
        repo_with_proposal("proposal-list", "deferred", FEATURES_EXAMPLE),
    );
    let run = collide(&repo, &bundle, &["tasks", "--kind", "proposal"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert_eq!(
        run.stdout,
        "plan:pr-0003-km  deferred  proposal  Features in proposals\n"
    );
    let document = json(&collide(
        &repo,
        &bundle,
        &["tasks", "--kind", "proposal", "--json"],
    ));
    assert_eq!(
        document["tasks"][0]["state"],
        serde_json::json!({ "status": "deferred", "category": "backlog" })
    );
    assert_eq!(document["invalid"], serde_json::json!([]));
}

#[test]
fn task_prints_a_proposals_features_by_section_with_unsorted_last() {
    let (bundle, repo) = (
        bundle(),
        repo_with_proposal("proposal-show", "draft", FEATURES_EXAMPLE),
    );
    let run = collide(&repo, &bundle, &["task", "pr-0003"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    let head = "\
ep-001-aa / pr-0003-km
  title    Features in proposals
  kind     proposal
  status   draft
  ready    no (status is draft)
  path     .collide/planning/epics/ep-001-aa-alpha/proposals/pr-0003-km-features-in-proposals.md
  coverage 3 feature(s) with no task (F1, F2, F3)
  features
    Packs
      F1 Trust prompt for repo packs: a repo pack asks before its first run. A continuation line is indented by two spaces.
        state  planned
      F2 Pack lock: `packs.lock` pins each pack to a tree.
        state  planned
    Unsorted
      F3 Menu bar badge: the Queue count in the menu bar.
        state  planned

## Context
";
    assert!(run.stdout.starts_with(head), "{}", run.stdout);
    assert!(!run.stdout.contains("feature problems"), "{}", run.stdout);
}

#[test]
fn sections_come_in_order_of_first_appearance_and_a_repeated_section_groups() {
    let features = "\
- **F1 Above**: before any heading.
### Packs
- **F2 In packs**: a
### Other
- **F3 In other**: b
### Packs
- **F4 Back in packs**: c
";
    let (bundle, repo) = (
        bundle(),
        repo_with_proposal("proposal-order", "draft", features),
    );
    let run = collide(&repo, &bundle, &["task", "pr-0003"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    let shown: Vec<&str> = run
        .stdout
        .lines()
        .skip_while(|line| *line != "  features")
        .take_while(|line| !line.is_empty())
        .collect();
    assert_eq!(
        shown,
        [
            "  features",
            "    Packs",
            "      F2 In packs: a",
            "        state  planned",
            "      F4 Back in packs: c",
            "        state  planned",
            "    Other",
            "      F3 In other: b",
            "        state  planned",
            "    Unsorted",
            "      F1 Above: before any heading.",
            "        state  planned"
        ]
    );
}

#[test]
fn task_prints_the_problems_of_the_features_part_after_the_features() {
    let features = "### Packs\n- **F1 Fine**: ok\nstray text\n- **F1 Again**: dup\n";
    let (bundle, repo) = (
        bundle(),
        repo_with_proposal("proposal-problems", "draft", features),
    );
    let run = collide(&repo, &bundle, &["task", "pr-0003"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    // The frontmatter is 8 lines and the blank line after it is the 9th; the Features heading is the 14th and its first line the 16th.
    let expected = "  \
features
    Packs
      F1 Fine: ok
        state  planned
  feature problems
    line 18: this line is neither an entry nor a continuation: an entry is written - **F<n> <title>**: <summary>
    line 19: F1 is already used on line 17; each F<n> appears once in a proposal
";
    assert!(
        run.stdout.contains(&expected[2..]),
        "{}\n--- wanted\n{expected}",
        run.stdout
    );
    let listed = json(&collide(&repo, &bundle, &["tasks", "--json"]));
    assert_eq!(
        listed["invalid"],
        serde_json::json!([]),
        "a draft's problem is no task.invalid"
    );
}

#[test]
fn a_proposal_with_no_features_part_prints_no_features_in_text() {
    let repo = Repo::new("proposal-none");
    let folder = repo.dir.join(ALPHA_PROPOSALS);
    fs::create_dir_all(&folder).unwrap();
    let text = proposal_text("draft", "").replace("## Features\n\n", "");
    fs::write(folder.join(PROPOSAL_FILE), text).unwrap();
    let bundle = bundle();
    let run = collide(&repo, &bundle, &["task", "pr-0003"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert!(!run.stdout.contains("  features"), "{}", run.stdout);
    assert!(!run.stdout.contains("feature problems"), "{}", run.stdout);
}

#[test]
fn create_makes_a_draft_proposal_in_the_epics_proposals_folder() {
    let (bundle, repo) = (bundle(), Repo::new("create-proposal"));
    let run = collide(
        &repo,
        &bundle,
        &[
            "task",
            "create",
            "Features in proposals",
            "epic=ep-001-aa",
            "kind=proposal",
            "seed=1",
        ],
    );
    assert_eq!(run.code, 0, "{}", run.stderr);
    let file = format!("{ALPHA_PROPOSALS}/pr-0001-ra-features-in-proposals.md");
    assert_eq!(
        run.stdout,
        format!(
            "ok  created ep-001-aa / pr-0001-ra\n  title   Features in proposals\n  file    {file}\n"
        )
    );
    assert_eq!(
        git(
            &repo.dir,
            &["status", "--porcelain", "--untracked-files=all"]
        ),
        format!("?? {file}\n")
    );
    let shown = collide(&repo, &bundle, &["task", "pr-0001"]);
    assert!(
        shown.stdout.contains("  status  draft\n"),
        "{}",
        shown.stdout
    );
}

#[test]
fn set_status_to_proposed_is_refused_with_every_problem_until_the_part_is_fixed() {
    let features = "### Packs\nstray\n- **F1 A**: b\n- **F1 B**: c\n";
    let (bundle, repo) = (
        bundle(),
        repo_with_proposal("proposal-refuse", "draft", features),
    );
    let file = repo.dir.join(ALPHA_PROPOSALS).join(PROPOSAL_FILE);
    let before = fs::read_to_string(&file).unwrap();

    let refused = collide(
        &repo,
        &bundle,
        &["task", "pr-0003", "setStatus", "proposed"],
    );
    assert_eq!(refused.code, 5, "{}", refused.stderr);
    assert!(
        refused
            .stderr
            .contains("pr-0003-km cannot move to proposed")
            && refused.stderr.contains("2 problems")
            && refused.stderr.contains("line 17: ")
            && refused
                .stderr
                .contains("line 19: F1 is already used on line 18"),
        "{}",
        refused.stderr
    );
    assert_eq!(fs::read_to_string(&file).unwrap(), before);
    assert_eq!(git(&repo.dir, &["status", "--porcelain"]), "");

    fs::write(&file, before.replace("stray\n", "").replace("F1 B", "F2 B")).unwrap();
    git(&repo.dir, &["commit", "-q", "-am", "fixed"]);
    let moved = collide(
        &repo,
        &bundle,
        &["task", "pr-0003", "setStatus", "proposed"],
    );
    assert_eq!(moved.code, 0, "{}", moved.stderr);
    assert!(
        moved.stdout.contains("status  draft -> proposed"),
        "{}",
        moved.stdout
    );
    assert_eq!(
        git(&repo.dir, &["diff", "--numstat"]),
        format!("1\t1\t{ALPHA_PROPOSALS}/{PROPOSAL_FILE}\n")
    );
}

#[test]
fn a_proposals_set_status_takes_the_six_words_and_lists_them_when_it_is_wrong() {
    let (bundle, repo) = (
        bundle(),
        repo_with_proposal("proposal-statuses", "draft", FEATURES_EXAMPLE),
    );
    let run = collide(&repo, &bundle, &["task", "pr-0003", "setStatus", "done"]);
    assert_eq!(run.code, 2, "{}", run.stderr);
    assert!(
        run.stderr
            .contains("must be one of draft, proposed, accepted, deferred, rejected, superseded"),
        "{}",
        run.stderr
    );
    let moved = collide(
        &repo,
        &bundle,
        &["task", "pr-0003", "setStatus", "proposed"],
    );
    assert_eq!(moved.code, 0, "{}", moved.stderr);
    assert!(
        moved.stdout.contains("status  draft -> proposed"),
        "{}",
        moved.stdout
    );
}

#[test]
fn a_bare_id_accept_waits_as_a_confirmation_writes_nothing_and_exits_12() {
    let (bundle, repo) = (
        bundle(),
        repo_with_proposal("proposal-accept", "proposed", FEATURES_EXAMPLE),
    );
    let run = collide(&repo, &bundle, &["task", "pr-0003", "accept"]);
    assert_eq!(run.code, 12, "{}", run.stderr);
    assert!(run.stdout.is_empty(), "{}", run.stdout);
    assert!(
        run.stderr.contains("a confirmation waits in the app"),
        "{}",
        run.stderr
    );
    assert!(run.stderr.contains("queue item confirm-"), "{}", run.stderr);
    assert!(git(&repo.dir, &["status", "--porcelain"]).is_empty());
}

#[test]
fn a_person_only_status_through_set_status_waits_the_same_way() {
    let (bundle, repo) = (
        bundle(),
        repo_with_proposal("proposal-reject", "draft", FEATURES_EXAMPLE),
    );
    let run = collide(
        &repo,
        &bundle,
        &["task", "pr-0003", "setStatus", "rejected"],
    );
    assert_eq!(run.code, 12, "{}", run.stderr);
    assert!(git(&repo.dir, &["status", "--porcelain"]).is_empty());
}

#[test]
fn a_person_only_op_with_json_prints_the_confirmation_and_exits_12() {
    let (bundle, repo) = (
        bundle(),
        repo_with_proposal("proposal-accept-json", "proposed", FEATURES_EXAMPLE),
    );
    let run = collide(&repo, &bundle, &["task", "pr-0003", "accept", "--json"]);
    assert_eq!(run.code, 12, "{}", run.stderr);
    let document = json(&run);
    assert!(
        document["confirmation"]["item"]
            .as_str()
            .is_some_and(|item| item.starts_with("confirm-")),
        "{document}"
    );
}

// Feature links, the derived state and reid (ts-0048).

const BETA_TASKS: &str = ".collide/planning/epics/ep-002-bb-beta/tasks";

/// One more task in the beta epic, committed: its id, its status and the lines of its `links` (empty for none).
struct Implementer<'a> {
    id: &'a str,
    status: &'a str,
    links: &'a str,
}

fn add_tasks(repo: &Repo, tasks: &[Implementer]) {
    for task in tasks {
        let links = if task.links.is_empty() {
            "links: []\n".to_string()
        } else {
            format!("links:\n{}", task.links)
        };
        let text = format!(
            "---\nid: {id}\nkind: task\nstatus: {status}\ntitle: \"Work on {id}\"\nowners: []\n{links}---\n\n## Goal\n\nMade-up.\n",
            id = task.id,
            status = task.status,
        );
        fs::write(
            repo.dir
                .join(BETA_TASKS)
                .join(format!("{}-work.md", task.id)),
            text,
        )
        .unwrap();
    }
    git(&repo.dir, &["add", "-A"]);
    git(&repo.dir, &["commit", "-q", "-m", "tasks that implement"]);
}

fn implements(to: &str) -> String {
    format!("  - {{ type: implements, to: {to} }}\n")
}

#[test]
fn task_prints_a_feature_link_with_its_epic() {
    let (bundle, repo) = (
        bundle(),
        repo_with_proposal("feature-link-print", "accepted", FEATURES_EXAMPLE),
    );
    add_tasks(
        &repo,
        &[Implementer {
            id: "ts-0020-xa",
            status: "active",
            links: &implements("pr-0003-km#F2"),
        }],
    );
    let run = collide(&repo, &bundle, &["task", "ts-0020"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert!(
        run.stdout
            .contains("  links\n    implements plan:pr-0003-km#F2 (accepted)\n"),
        "{}",
        run.stdout
    );
    assert!(run.stdout.contains("ready   no (status is active)"));
}

#[test]
fn task_says_when_a_feature_link_names_no_feature_or_no_item() {
    let (bundle, repo) = (
        bundle(),
        repo_with_proposal("feature-link-broken", "accepted", FEATURES_EXAMPLE),
    );
    let links = format!(
        "{}{}",
        implements("pr-0003-km#F9"),
        implements("pr-0099-zz#F1")
    );
    add_tasks(
        &repo,
        &[Implementer {
            id: "ts-0020-xa",
            status: "ready",
            links: &links,
        }],
    );
    let run = collide(&repo, &bundle, &["task", "ts-0020"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert!(
        run.stdout.contains(
            "    implements plan:pr-0003-km#F9 (proposal or feature not found)\n    \
             implements plan:pr-0099-zz#F1 (proposal or feature not found)\n"
        ),
        "{}",
        run.stdout
    );
    let listed = collide(&repo, &bundle, &["tasks", "--json"]);
    assert!(ids(&json(&listed)).contains(&"ts-0020-xa".to_string()));
}

#[test]
fn a_feature_link_does_not_hold_back_a_ready_task() {
    let (bundle, repo) = (
        bundle(),
        repo_with_proposal("feature-link-ready", "draft", FEATURES_EXAMPLE),
    );
    add_tasks(
        &repo,
        &[Implementer {
            id: "ts-0020-xa",
            status: "ready",
            links: &implements("pr-0003-km#F1"),
        }],
    );
    let run = collide(&repo, &bundle, &["tasks", "--ready"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert!(run.stdout.contains("ts-0020-xa"), "{}", run.stdout);
}

/// The lines of a proposal's print from `  features` up to the blank line before its body.
fn features_block(stdout: &str) -> Vec<&str> {
    stdout
        .lines()
        .skip_while(|line| *line != "  features")
        .take_while(|line| !line.is_empty())
        .collect()
}

#[test]
fn task_prints_each_features_state_and_the_tasks_that_implement_it() {
    let (bundle, repo) = (
        bundle(),
        repo_with_proposal("feature-states", "accepted", FEATURES_EXAMPLE),
    );
    add_tasks(
        &repo,
        &[
            Implementer {
                id: "ts-0020-xa",
                status: "done",
                links: &implements("pr-0003-km#F1"),
            },
            Implementer {
                id: "ts-0021-xb",
                status: "active",
                links: &implements("pr-0003-km#F1"),
            },
            Implementer {
                id: "ts-0022-xc",
                status: "ready",
                links: &implements("pr-0003-km#F1"),
            },
            Implementer {
                id: "ts-0023-xd",
                status: "dropped",
                links: &implements("pr-0003-km#F3"),
            },
        ],
    );
    let run = collide(&repo, &bundle, &["task", "pr-0003"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert_eq!(
        features_block(&run.stdout),
        [
            "  features",
            "    Packs",
            "      F1 Trust prompt for repo packs: a repo pack asks before its first run. A continuation line is indented by two spaces.",
            "        state  1 of 3 done",
            "        plan:ts-0020-xa  done",
            "        plan:ts-0021-xb  active",
            "        plan:ts-0022-xc  ready",
            "      F2 Pack lock: `packs.lock` pins each pack to a tree.",
            "        state  accepted",
            "    Unsorted",
            "      F3 Menu bar badge: the Queue count in the menu bar.",
            "        state  accepted",
            "        plan:ts-0023-xd  dropped",
        ]
    );
}

#[test]
fn a_feature_is_done_when_every_task_that_is_not_dropped_is_done() {
    let (bundle, repo) = (
        bundle(),
        repo_with_proposal("feature-done", "accepted", FEATURES_EXAMPLE),
    );
    add_tasks(
        &repo,
        &[
            Implementer {
                id: "ts-0020-xa",
                status: "done",
                links: &implements("pr-0003-km#F2"),
            },
            Implementer {
                id: "ts-0021-xb",
                status: "dropped",
                links: &implements("pr-0003-km#F2"),
            },
        ],
    );
    let run = collide(&repo, &bundle, &["task", "pr-0003"]);
    assert!(
        run.stdout.contains(
            "      F2 Pack lock: `packs.lock` pins each pack to a tree.\n        state  done\n"
        ),
        "{}",
        run.stdout
    );
}

#[test]
fn a_feature_takes_its_state_from_the_proposal_until_the_proposal_is_accepted() {
    let expected = [
        ("draft", "planned"),
        ("proposed", "planned"),
        ("deferred", "deferred"),
        ("rejected", "dropped"),
        ("superseded", "dropped"),
        ("accepted", "done"),
    ];
    let bundle = bundle();
    for (status, state) in expected {
        let repo = repo_with_proposal(
            &format!("feature-from-{status}"),
            status,
            "- **F1 Only**: one\n",
        );
        add_tasks(
            &repo,
            &[Implementer {
                id: "ts-0020-xa",
                status: "done",
                links: &implements("pr-0003-km#F1"),
            }],
        );
        let run = collide(&repo, &bundle, &["task", "pr-0003"]);
        assert_eq!(run.code, 0, "{status}: {}", run.stderr);
        assert!(
            run.stdout
                .contains(&format!("      F1 Only: one\n        state  {state}\n")),
            "{status}: {}",
            run.stdout
        );
    }
}

#[test]
fn link_implements_writes_the_link_and_task_prints_it() {
    let (bundle, repo) = (
        bundle(),
        repo_with_proposal("link-implements", "accepted", FEATURES_EXAMPLE),
    );
    let run = collide(
        &repo,
        &bundle,
        &[
            "task",
            "ts-0002",
            "link",
            "pr-0003-km#F2",
            "type=implements",
        ],
    );
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert_eq!(
        git(&repo.dir, &["diff", "--numstat"]),
        "1\t0\t.collide/planning/epics/ep-001-aa-alpha/tasks/ts-0002-bb-second.md\n"
    );
    let shown = collide(&repo, &bundle, &["task", "ts-0002"]);
    assert!(
        shown
            .stdout
            .contains("    implements plan:pr-0003-km#F2 (accepted)\n"),
        "{}",
        shown.stdout
    );
    let proposal = collide(&repo, &bundle, &["task", "pr-0003"]);
    assert!(
        proposal
            .stdout
            .contains("        state  0 of 1 done\n        plan:ts-0002-bb  ready\n"),
        "{}",
        proposal.stdout
    );
}

#[test]
fn link_implements_to_a_feature_the_proposal_lacks_is_refused_naming_its_features() {
    let (bundle, repo) = (
        bundle(),
        repo_with_proposal("link-refused", "accepted", FEATURES_EXAMPLE),
    );
    for (to, wanted) in [
        ("pr-0003-km#F9", "pr-0003-km has no feature F9"),
        ("pr-0003-km", "implements links to a feature"),
        ("ts-0001-aa#F1", "ts-0001-aa is a task, not a proposal"),
    ] {
        let run = collide(
            &repo,
            &bundle,
            &["task", "ts-0002", "link", to, "type=implements"],
        );
        assert_eq!(run.code, 5, "{to}: {}", run.stderr);
        assert!(run.stderr.contains(wanted), "{to}: {}", run.stderr);
        if to.starts_with("pr-") {
            assert!(
                run.stderr.contains(
                    "F1 (Trust prompt for repo packs), F2 (Pack lock), F3 (Menu bar badge)"
                ),
                "{to}: {}",
                run.stderr
            );
        }
    }
    let blocked = collide(
        &repo,
        &bundle,
        &["task", "ts-0002", "link", "pr-0003-km#F1"],
    );
    assert_eq!(blocked.code, 5, "{}", blocked.stderr);
    assert!(
        blocked
            .stderr
            .contains("blocked-by names an item, not a feature"),
        "{}",
        blocked.stderr
    );
    assert_eq!(git(&repo.dir, &["status", "--porcelain"]), "");
}

#[test]
fn reid_of_a_proposal_keeps_the_feature_in_the_links_it_rewrites() {
    let (bundle, repo) = (
        bundle(),
        repo_with_proposal("reid-feature", "accepted", FEATURES_EXAMPLE),
    );
    add_tasks(
        &repo,
        &[Implementer {
            id: "ts-0020-xa",
            status: "active",
            links: &implements("pr-0003-km#F2"),
        }],
    );
    let run = collide(&repo, &bundle, &["task", "reid", "pr-0003", "seed=1"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    let after = fs::read_to_string(repo.dir.join(BETA_TASKS).join("ts-0020-xa-work.md")).unwrap();
    assert!(
        after.contains("  - { type: implements, to: pr-0003-ra#F2 }\n"),
        "{after}"
    );
    assert!(!after.contains("pr-0003-km"), "{after}");
    let shown = collide(&repo, &bundle, &["task", "ts-0020"]);
    assert!(
        shown
            .stdout
            .contains("    implements plan:pr-0003-ra#F2 (accepted)\n"),
        "{}",
        shown.stdout
    );
}

// `collide sections` (ts-0049).

/// A proposal in the alpha epic's `proposals/` folder, committed: its id, its status and the text of its Features part.
struct Proposed<'a> {
    id: &'a str,
    status: &'a str,
    features: &'a str,
}

fn add_proposals(repo: &Repo, proposals: &[Proposed]) {
    let folder = repo.dir.join(ALPHA_PROPOSALS);
    fs::create_dir_all(&folder).unwrap();
    for proposal in proposals {
        let text = proposal_text(proposal.status, proposal.features)
            .replace("id: pr-0003-km", &format!("id: {}", proposal.id));
        fs::write(folder.join(format!("{}-made-up.md", proposal.id)), text).unwrap();
    }
    git(&repo.dir, &["add", "-A"]);
    git(&repo.dir, &["commit", "-q", "-m", "proposals"]);
}

/// Three live proposals over four sections, with `Pack` and `packs` near each other and `Task view` and `task-view`.
fn repo_with_sections(name: &str) -> Repo {
    let repo = Repo::new(name);
    add_proposals(
        &repo,
        &[
            Proposed {
                id: "pr-0001-aa",
                status: "accepted",
                features: "### Packs\n- **F1 One**: a\n- **F2 Two**: b\n### Task view\n- **F3 Three**: c\n### Daemon\n- **F4 Four**: d\n",
            },
            Proposed {
                id: "pr-0002-bb",
                status: "draft",
                features: "### packs\n- **F1 One**: a\n### Pack\n- **F2 Two**: b\n### task-view\n- **F3 Three**: c\n### Unsorted\n- **F4 Four**: d\n",
            },
            Proposed {
                id: "pr-0004-dd",
                status: "deferred",
                features: "- **F1 One**: a above any heading\n### Packs\n- **F2 Two**: b\n",
            },
        ],
    );
    repo
}

#[test]
fn sections_lists_each_with_its_counts_unsorted_last_then_the_near_duplicate_groups() {
    let (bundle, repo) = (bundle(), repo_with_sections("sections-text"));
    let run = collide(&repo, &bundle, &["sections"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert_eq!(
        run.stdout,
        "\
Packs      4 features  3 proposals
Daemon     1 feature   1 proposal
Pack       1 feature   1 proposal
Task view  1 feature   1 proposal
task-view  1 feature   1 proposal
Unsorted   2 features  2 proposals

near-duplicates (a suggestion; nothing is merged)
  Packs, Pack
  Task view, task-view
"
    );
}

#[test]
fn sections_json_gives_the_same_content_in_one_shape() {
    let (bundle, repo) = (bundle(), repo_with_sections("sections-json"));
    let run = collide(&repo, &bundle, &["sections", "--json"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    let document = json(&run);
    assert_eq!(
        document["sections"][0],
        serde_json::json!({
            "name": "Packs",
            "featureCount": 4,
            "proposalCount": 3,
            "proposals": ["pr-0001-aa", "pr-0002-bb", "pr-0004-dd"],
        })
    );
    let names: Vec<&str> = document["sections"]
        .as_array()
        .unwrap()
        .iter()
        .map(|section| section["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        ["Packs", "Daemon", "Pack", "Task view", "task-view", ""]
    );
    assert_eq!(
        document["nearDuplicates"],
        serde_json::json!([
            { "names": ["Packs", "Pack"] },
            { "names": ["Task view", "task-view"] },
        ])
    );
}

#[test]
fn sections_do_not_count_the_features_of_rejected_and_superseded_proposals() {
    let (bundle, repo) = (bundle(), repo_with_sections("sections-dropped"));
    add_proposals(
        &repo,
        &[
            Proposed {
                id: "pr-0005-ee",
                status: "rejected",
                features: "### Packs\n- **F1 One**: a\n### Rejected only\n- **F2 Two**: b\n",
            },
            Proposed {
                id: "pr-0006-ff",
                status: "superseded",
                features: "### Packs\n- **F1 One**: a\n### Superseded only\n- **F2 Two**: b\n",
            },
        ],
    );
    let document = json(&collide(&repo, &bundle, &["sections", "--json"]));
    assert_eq!(document["sections"][0]["featureCount"], 4);
    assert_eq!(document["sections"][0]["proposalCount"], 3);
    let names: Vec<&str> = document["sections"]
        .as_array()
        .unwrap()
        .iter()
        .map(|section| section["name"].as_str().unwrap())
        .collect();
    assert!(!names.contains(&"Rejected only"), "{names:?}");
    assert!(!names.contains(&"Superseded only"), "{names:?}");
}

/// A child that is killed and reaped when it drops, so a failed test leaves no provider behind.
struct Killed(std::process::Child);

impl Drop for Killed {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Runs `renameSection` on the plan provider itself (`node packs/plan/src/main.ts`, over the repository `repo`) and
/// returns its `apply` result. `collide` has no command for it until `ts-0136-rr`. The environment is cleared, `PATH`
/// is kept for `node`, and `HOME` and the daemon root are a temp root; no pack data folder means no private root.
fn rename_section(repo: &Repo, from: &str, to: &str) -> Value {
    use std::io::{Read, Write};
    use std::process::Stdio;
    let root = TempRoot::new();
    let main = repo_root().join("packs/plan/src/main.ts");
    let mut child = Killed(
        Command::new("node")
            .arg(main)
            .current_dir(root.path())
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .env("HOME", root.path())
            .env("COLLIDE_DAEMON_ROOT", root.path())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("node should run the plan provider"),
    );
    let initialize = serde_json::json!({
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": { "config": {}, "root": repo.dir },
    });
    let apply = serde_json::json!({
        "jsonrpc": "2.0", "id": 2, "method": "apply",
        "params": { "op": "renameSection", "args": { "from": from, "to": to } },
    });
    let mut stdin = child.0.stdin.take().unwrap();
    writeln!(stdin, "{initialize}").unwrap();
    writeln!(stdin, "{apply}").unwrap();
    // The provider serves until its input ends, so closing it ends the child.
    drop(stdin);
    let mut stdout = child.0.stdout.take().unwrap();
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut text = String::new();
        let _ = stdout.read_to_string(&mut text);
        let _ = sender.send(text);
    });
    let text = receiver
        .recv_timeout(std::time::Duration::from_secs(60))
        .expect("the plan provider should answer and exit");
    let replies: Vec<Value> = text
        .lines()
        .map(|line| serde_json::from_str(line).unwrap_or_else(|e| panic!("{e}: {line}")))
        .collect();
    assert_eq!(replies.len(), 2, "{text}");
    replies[1]["result"].clone()
}

#[test]
fn sections_json_after_a_rename_has_no_near_duplicate_group_for_the_renamed_name() {
    let (bundle, repo) = (bundle(), repo_with_sections("sections-rename"));
    let before = json(&collide(&repo, &bundle, &["sections", "--json"]));
    assert_eq!(
        before["nearDuplicates"],
        serde_json::json!([
            { "names": ["Packs", "Pack"] },
            { "names": ["Task view", "task-view"] },
        ])
    );

    let reply = rename_section(&repo, "Pack", "packs");
    assert_eq!(reply["outcome"], "ok", "{reply}");
    assert_eq!(reply["files"].as_array().unwrap().len(), 1, "{reply}");

    let run = collide(&repo, &bundle, &["sections", "--json"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    let after = json(&run);
    assert_eq!(
        after["nearDuplicates"],
        serde_json::json!([{ "names": ["Task view", "task-view"] }])
    );
    assert_eq!(
        after["sections"][0],
        serde_json::json!({
            "name": "Packs",
            "featureCount": 5,
            "proposalCount": 3,
            "proposals": ["pr-0001-aa", "pr-0002-bb", "pr-0004-dd"],
        })
    );
}

#[test]
fn sections_do_not_count_items_that_are_not_proposals() {
    let (bundle, repo) = (bundle(), Repo::new("sections-no-proposals"));
    let run = collide(&repo, &bundle, &["sections"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert_eq!(
        run.stdout,
        "no sections: no proposal that is not dropped has a feature\n"
    );
    let document = json(&collide(&repo, &bundle, &["sections", "--json"]));
    assert_eq!(
        document,
        serde_json::json!({ "sections": [], "nearDuplicates": [] })
    );
}

#[test]
fn sections_count_a_drafts_features_that_parsed_and_leave_its_problems_to_task() {
    let (bundle, repo) = (bundle(), Repo::new("sections-draft-problem"));
    add_proposals(
        &repo,
        &[Proposed {
            id: "pr-0001-aa",
            status: "draft",
            features: "### Packs\n- **F1 Fine**: ok\nstray text\n",
        }],
    );
    let run = collide(&repo, &bundle, &["sections"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert_eq!(run.stdout, "Packs  1 feature  1 proposal\n");
}

#[test]
fn sections_take_no_argument_and_no_other_option() {
    let (bundle, repo) = (bundle(), Repo::new("sections-usage"));
    for (args, wanted) in [
        (&["sections", "packs"][..], "takes no argument"),
        (&["sections", "--ready"][..], "unknown option `--ready`"),
        (&["sections", "--commit"][..], "unknown option `--commit`"),
    ] {
        let run = collide(&repo, &bundle, args);
        assert_eq!(run.code, 2, "{args:?}: {}", run.stderr);
        assert!(run.stderr.contains(wanted), "{args:?}: {}", run.stderr);
        assert!(run.stderr.contains("usage:"), "{args:?}: {}", run.stderr);
    }
}

#[test]
fn help_states_the_matching_and_the_near_duplicate_rules() {
    let (bundle, repo) = (bundle(), Repo::new("sections-help"));
    let run = collide(&repo, &bundle, &["help"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    for wanted in [
        "collide sections [--json]",
        "match after trimming, in any case",
        "spaces, hyphens and underscores are taken out and one final s is dropped",
        "Pack and packs",
        "Task view and task-view",
        "never merges",
    ] {
        assert!(run.stdout.contains(wanted), "{wanted}: {}", run.stdout);
    }
}

#[test]
fn sections_report_an_invalid_item_on_stderr_as_tasks_does() {
    let (bundle, repo) = (bundle(), repo_with_sections("sections-invalid"));
    let folder = repo.dir.join(ALPHA_PROPOSALS);
    fs::write(folder.join("broken.md"), "---\nid: [\n---\n").unwrap();
    let run = collide(&repo, &bundle, &["sections"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert!(run.stderr.contains("collide: skipped"), "{}", run.stderr);
    assert!(run.stdout.starts_with("Packs "), "{}", run.stdout);
}
