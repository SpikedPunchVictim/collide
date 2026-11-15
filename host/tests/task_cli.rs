//! End-to-end tests for `collide tasks` and `collide task` (ts-0020): the binary runs against a fixture repository
//! in a temp folder (items copied from `host/tests/fixtures/task-cli`, never this repository's own), started on the
//! real plan provider from a bundle built here, and on a stand-in provider for the outcomes the real one cannot be
//! made to give. They need `node` and `pnpm` on `PATH`, like the loader's tests.

use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

mod common;
use common::{Bundle, repo_root, scratch};

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

/// Runs `collide` in `cwd` with the packs bundle at `packs`, and `env` added.
fn collide_with(cwd: &Path, packs: &Path, env: &[(&str, &str)], args: &[&str]) -> Run {
    let Output {
        status,
        stdout,
        stderr,
    } = Command::new(env!("CARGO_BIN_EXE_collide"))
        .args(args)
        .current_dir(cwd)
        .env("COLLIDE_BUILTIN_PACKS", packs)
        .envs(env.iter().copied())
        .output()
        .expect("collide should run");
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
fn tasks_prints_each_items_epic_beside_its_id_with_the_ready_rules_verdict() {
    let (bundle, repo) = (bundle(), Repo::new("list"));
    let run = collide(&repo, &bundle, &["tasks"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert_eq!(
        run.stdout,
        "\
ep-001-aa               active           epic  Alpha epic
ep-001-aa / ts-0001-aa  done             task  First task
ep-001-aa / ts-0002-bb  ready            task  Second task, free to start
ep-001-aa / ts-0003-cc  ready (blocked)  task  Third task, waits for the second
ep-001-aa / ts-0006-hh  ready (blocked)  task  Loop, first half
ep-001-aa / ts-0007-ii  ready (blocked)  task  Loop, second half
ep-001-aa / ts-0008-jj  ready (blocked)  task  Blocked by an id that is no item
ep-001-aa / ts-0012-dd  ready            task  Twelve, in alpha
ep-002-bb               active           epic  Beta epic
ep-002-bb / ts-0004-ff  dropped          task  Dropped task
ep-002-bb / ts-0005-gg  ready            task  Free, since its blocker is dropped
ep-002-bb / ts-0012-ee  review           task  Twelve, in beta
"
    );
}

#[test]
fn a_cycle_and_an_unknown_blocker_are_reported_on_stderr() {
    let (bundle, repo) = (bundle(), Repo::new("warnings"));
    let run = collide(&repo, &bundle, &["tasks"]);
    assert!(
        run.stderr
            .contains("collide: blocked-by cycle, none is ready: ts-0006-hh, ts-0007-ii"),
        "{}",
        run.stderr
    );
    assert!(
        run.stderr.contains(
            "collide: ts-0008-jj is blocked by ts-9999-zz, which is not an item; it is not ready"
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
    // In the provider's order (epic folder, then file name). Done and dropped blockers free an item; a ready
    // blocker, a cycle and an unknown blocker do not.
    assert_eq!(ids(&json(&run)), ["ts-0002-bb", "ts-0012-dd", "ts-0005-gg"]);
}

#[test]
fn json_carries_the_epic_ready_flag_and_no_invalid_items() {
    let (bundle, repo) = (bundle(), Repo::new("json"));
    let document = json(&collide(&repo, &bundle, &["tasks", "--json"]));
    assert_eq!(document["invalid"], serde_json::json!([]));
    assert_eq!(
        document["cycles"],
        serde_json::json!([["ts-0006-hh", "ts-0007-ii"]])
    );
    assert_eq!(
        document["unknownTargets"],
        serde_json::json!([{ "from": "ts-0008-jj", "to": "ts-9999-zz" }])
    );
    let tasks = document["tasks"].as_array().unwrap();
    let by_id = |id: &str| tasks.iter().find(|task| task["id"] == id).unwrap();
    assert_eq!(by_id("ts-0005-gg")["epic"], "ep-002-bb");
    assert_eq!(by_id("ts-0005-gg")["ready"], true);
    assert_eq!(by_id("ts-0003-cc")["ready"], false);
    assert_eq!(by_id("ep-001-aa")["epic"], Value::Null);
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
fn an_unknown_kind_fails_listing_the_declared_kinds() {
    let (bundle, repo) = (bundle(), Repo::new("kind-unknown"));
    let run = collide(&repo, &bundle, &["tasks", "--kind", "spike"]);
    assert_eq!(run.code, 2);
    assert!(
        run.stderr
            .contains("unknown kind `spike`; the provider declares: task, epic"),
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
fn task_shows_the_epic_beside_the_id_and_each_link_target_with_its_epic() {
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
    blocked-by ts-0004-ff (ep-002)
    blocked-by ts-0001-aa (ep-001)

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
    assert!(typo.contains("blocked-by ts-9999-zz (not found)"), "{typo}");
    let looped = collide(&repo, &bundle, &["task", "6"]).stdout;
    assert!(
        looped.contains("no (in a blocked-by cycle: ts-0006-hh, ts-0007-ii)"),
        "{looped}"
    );
    let done = collide(&repo, &bundle, &["task", "ts-1"]).stdout;
    assert!(done.contains("  ready   no (status is done)"), "{done}");
}

#[test]
fn task_json_carries_the_epic() {
    let (bundle, repo) = (bundle(), Repo::new("show-json"));
    let task = json(&collide(&repo, &bundle, &["task", "ts-0003-cc", "--json"]));
    assert_eq!(task["epic"], "ep-001-aa");
    assert_eq!(task["ready"], false);
    assert!(task["body"].as_str().unwrap().contains("Made-up content"));
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
             ep-001-aa / ts-0012-dd  task  Twelve, in alpha\n  \
             ep-002-bb / ts-0012-ee  task  Twelve, in beta\n\
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
            .contains("unknown op `explode` for ts-0003-cc (task); the provider declares:"),
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
        vec!["task", "3", "setStatus", "--commit"],
        vec!["nonsense"],
    ] {
        let run = collide(&repo, &bundle, &args);
        assert_eq!(run.code, 2, "{args:?}: {}", run.stderr);
        assert!(run.stderr.contains("usage:"), "{args:?}: {}", run.stderr);
    }
}

#[test]
fn help_prints_the_usage_the_exit_codes_and_where_the_provider_is_found() {
    let run = collide_with(&std::env::temp_dir(), Path::new("/nowhere"), &[], &["help"]);
    assert_eq!(run.code, 0);
    assert!(
        run.stdout
            .contains("collide tasks [--ready] [--kind <kind>] [--json]")
    );
    assert!(
        run.stdout.contains(
            "exit codes: 0 ok, 1 failed, 2 wrong command line, 3 id not found or ambiguous"
        )
    );
    assert!(run.stdout.contains("COLLIDE_BUILTIN_PACKS"));
    for line in [
        "collide task create <title> [epic=<epic>] [kind=task|epic] [seed=<number>] [--json]",
        "collide task reid <id> [seed=<number>] [--json]",
        "Nothing is committed: review with git diff.",
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

#[test]
fn a_missing_bundle_says_what_was_looked_for_and_where() {
    let repo = Repo::new("no-bundle");
    let missing = scratch("task-cli-no-such-bundle");
    let run = collide_with(&repo.dir, &missing, &[], &["tasks"]);
    assert_eq!(run.code, 1);
    assert!(
        run.stderr.contains("COLLIDE_BUILTIN_PACKS is set to"),
        "{}",
        run.stderr
    );
    assert!(run.stderr.contains("no-such-bundle"), "{}", run.stderr);
    assert!(run.stderr.contains("plan/pack.yaml"), "{}", run.stderr);
}

// A stand-in provider.

struct Fake(PathBuf);

impl Fake {
    fn new() -> Self {
        Self(repo_root().join("host/tests/fixtures/fake-provider"))
    }

    fn run(&self, mode: &str, args: &[&str]) -> Run {
        let repo = Repo::new(&format!("fake-{mode}-{}", args.join("-")));
        collide_with(&repo.dir, &self.0, &[("FAKE_MODE", mode)], args)
    }
}

#[test]
fn a_conflict_prints_the_current_state_says_nothing_was_written_and_exits_4() {
    let run = Fake::new().run("conflict", &["task", "1", "setStatus", "active"]);
    assert_eq!(run.code, 4);
    assert!(run.stdout.is_empty());
    assert!(
        run.stderr.contains(
            "conflict: ep-001-aa / ts-0001-aa changed after it was read; nothing was written."
        ),
        "{}",
        run.stderr
    );
    assert!(run.stderr.contains("  status  active"), "{}", run.stderr);
}

#[test]
fn a_conflict_with_json_prints_the_result() {
    let run = Fake::new().run("conflict", &["task", "1", "setStatus", "active", "--json"]);
    assert_eq!(run.code, 4);
    assert_eq!(json(&run)["outcome"], "conflict");
    assert_eq!(json(&run)["current"]["version"], "v2");
}

#[test]
fn notifications_from_the_provider_are_skipped() {
    let run = Fake::new().run("notify", &["tasks"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert!(
        run.stdout.contains("ep-001-aa / ts-0001-aa"),
        "{}",
        run.stdout
    );
}

#[test]
fn an_error_reply_is_surfaced_with_its_message() {
    let run = Fake::new().run("rpc-error", &["tasks"]);
    assert_eq!(run.code, 1);
    assert!(
        run.stderr
            .contains("the provider refused `list` (error -32000): the fake source is on fire"),
        "{}",
        run.stderr
    );
}

#[test]
fn a_provider_that_exits_is_reported_and_its_stderr_passes_through() {
    let run = Fake::new().run("exit", &["tasks"]);
    assert_eq!(run.code, 1);
    assert!(
        run.stderr.contains("fake provider: giving up"),
        "{}",
        run.stderr
    );
    assert!(
        run.stderr
            .contains("the provider exited (exit status: 3) before it answered `list`"),
        "{}",
        run.stderr
    );
}

#[test]
fn an_answer_that_is_not_a_list_result_is_reported() {
    let run = Fake::new().run("garbage", &["tasks"]);
    assert_eq!(run.code, 1);
    assert!(
        run.stderr
            .contains("the provider's answer to `list` is not valid"),
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
            "is ts-0001-aa, which is not an epic",
        ),
        (
            &["task", "create", "T", "epic=9"],
            3,
            "no item matches `ep-9`",
        ),
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
            && twelve.stderr.contains(&format!("ep-001-aa / {id_a}"))
            && twelve.stderr.contains(&format!("ep-001-aa / {id_b}")),
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
            "ok  ep-001-aa / ts-0002-ra\n  id      ts-0002-bb -> ts-0002-ra\n  \
             file    {third}\n  file    {renamed}\n  removed {copy}\n\
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
    assert_eq!(result["outcome"], "ok");
    assert_eq!(result["task"]["id"], "ts-0002-ra");
}

#[test]
fn reid_of_an_id_nobody_has_exits_3_and_writes_nothing() {
    let (bundle, repo) = (bundle(), Repo::new("reid-none"));
    let run = collide(&repo, &bundle, &["task", "reid", "ts-0099-zz"]);
    assert_eq!(run.code, 3);
    assert!(git(&repo.dir, &["status", "--porcelain"]).is_empty());
}
