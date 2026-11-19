//! `collide task new`, `collide tasks --view` and `--checkout`, `collide task <id> --json` and a write on a key
//! (ts-0120-wm): the commands over a daemon in the test's own process, in a fixture repository (a copy of
//! `host/tests/fixtures/task-cli`) under a temp root that is also the home. The plan provider is the built-in one, from
//! a bundle built here. What needs the real binary (starting the daemon, the help text, the exit codes of a bad
//! command line) is in `task_daemon_process.rs`. Nothing here touches the user's home, keychain or a real daemon, and
//! no `collide` binary is run.

mod common;

use collide_core::change::{Actor, ActorKind};
use collide_core::events::EventBody;
use collide_core::owner::{
    BoardOpenParams, BoardOpenResult, BoardTrustParams, BoardTrustResult, BoardTrustReviewParams,
    BoardTrustReviewResult, QueueAnswer, QueueAnswerParams, QueueAnswerResult, QueueItem,
    QueueListParams, QueueListResult, TaskGetParams, TaskGetResult, TaskListParams, TaskListResult,
    TrustState, ViewRef,
};
use collide_core::state::RunState;
use collide_host::base_env::CaptureOptions;
use collide_host::board_load::LoadContext;
use collide_host::builtin_packs::{BuiltinPacks, Environment};
use collide_host::config_get::LoadContextFactory;
use collide_host::daemon::{Daemon, DaemonOptions, DaemonPaths};
use collide_host::owner_client::OwnerClient;
use collide_host::pack_cache::ensure_sdk;
use collide_host::pack_location::bundle_sdk;
use collide_host::run_store::RunLaunch;
use collide_host::task_cli::{Command, exit, parse_task, parse_tasks};
use collide_host::task_daemon::{run_with, wants_daemon};
use common::git_repo::{GitRepo, git_command};
use common::{Bundle, repo_root, wait_until};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Long enough for a loaded machine: every wait has at least this.
const DEADLINE: Duration = Duration::from_secs(30);

/// How often the daemon's writer looks at a checkout that stayed busy, and how often it reads the checkouts again.
const POLL: Duration = Duration::from_millis(100);

const EPIC: &str = ".collide/planning/epics/ep-001-aa-alpha/tasks";
const SECOND_FILE: &str = "ts-0002-bb-second.md";
const THIRD_FILE: &str = "ts-0003-cc-third.md";

const READY_NOW: &str = "name: ready-now\nwhere: \"status == 'ready'\"\n";
const BROKEN: &str = "name: broken\nwhere: \"status ==\"\n";

fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(|word| word.to_string()).collect()
}

/// `collide <words>` as the binary reads it: the words after the command's name go to its parser.
fn parsed(words: &[&str]) -> Command {
    let rest = args(&words[1..]);
    let command = match words[0] {
        "tasks" => parse_tasks(&rest),
        "task" => parse_task(&rest),
        other => panic!("no parser for {other}"),
    };
    command.unwrap_or_else(|error| panic!("{words:?}: {}", error.message))
}

fn error_of(words: &[&str]) -> String {
    let rest = args(&words[1..]);
    let result = match words[0] {
        "tasks" => parse_tasks(&rest),
        _ => parse_task(&rest),
    };
    match result {
        Err(error) => {
            assert_eq!(error.code, exit::USAGE, "{words:?}");
            error.message
        }
        Ok(command) => panic!("{words:?} should be refused, and is {command:?}"),
    }
}

// ---- reading the command line --------------------------------------------------------------------------------------

#[test]
fn tasks_takes_a_view_and_a_checkout_in_both_forms() {
    for words in [
        vec!["tasks", "--view", "mine", "--checkout", "../wt"],
        vec!["tasks", "--view=mine", "--checkout=../wt", "--ready"],
    ] {
        let Command::Tasks(options) = parsed(&words) else {
            panic!("{words:?} is a tasks command")
        };
        assert_eq!(options.view.as_deref(), Some("mine"), "{words:?}");
        assert_eq!(options.checkout.as_deref(), Some("../wt"), "{words:?}");
    }
}

#[test]
fn tasks_without_the_new_options_has_neither() {
    let Command::Tasks(options) = parsed(&["tasks", "--ready", "--kind", "task", "--json"]) else {
        panic!("a tasks command")
    };
    assert_eq!((options.view, options.checkout), (None, None));
}

#[test]
fn an_option_with_no_value_or_given_twice_is_refused_and_named() {
    for (words, named) in [
        (vec!["tasks", "--view"], "--view"),
        (vec!["tasks", "--view="], "--view"),
        (vec!["tasks", "--checkout"], "--checkout"),
        (vec!["tasks", "--view", "a", "--view", "b"], "--view"),
        (
            vec!["tasks", "--checkout", "a", "--checkout=b"],
            "--checkout",
        ),
        (vec!["tasks", "--views", "a"], "--views"),
        (vec!["tasks", "--no-such"], "--no-such"),
    ] {
        let message = error_of(&words);
        assert!(message.contains(named), "{words:?}: {message}");
    }
}

#[test]
fn task_new_takes_one_prompt_and_json() {
    let Command::New(options) = parsed(&["task", "new", "try the cache", "--json"]) else {
        panic!("a task new command")
    };
    assert_eq!(
        (options.prompt.as_str(), options.json),
        ("try the cache", true)
    );
}

#[test]
fn task_new_needs_one_prompt_that_has_text() {
    for words in [
        vec!["task", "new"],
        vec!["task", "new", "a", "b"],
        vec!["task", "new", "  "],
        vec!["task", "new", ""],
    ] {
        error_of(&words);
    }
}

#[test]
fn task_new_refuses_the_options_of_a_write_to_a_checkout_and_names_each() {
    for (words, named) in [
        (vec!["task", "new", "x", "--commit"], "--commit"),
        (vec!["task", "new", "x", "--checkout", "wt"], "--checkout"),
        (vec!["task", "new", "x", "--wait", "5"], "--wait"),
        (vec!["task", "new", "x", "--no-wait"], "--no-wait"),
        (vec!["task", "new", "x", "--bogus"], "--bogus"),
    ] {
        let message = error_of(&words);
        assert!(message.contains(named), "{words:?}: {message}");
    }
}

#[test]
fn only_the_forms_that_need_the_corpus_go_through_the_daemon() {
    for (words, through) in [
        (vec!["tasks"], false),
        (vec!["tasks", "--ready", "--kind", "task", "--json"], false),
        (vec!["tasks", "--view", "mine"], true),
        (vec!["tasks", "--checkout", "wt"], true),
        (vec!["task", "12"], false),
        (vec!["task", "12", "--json"], true),
        (vec!["task", "scratch:3", "--json"], true),
        (vec!["task", "12", "setStatus", "active"], false),
        (vec!["task", "scratch:3", "setStatus", "done"], true),
        (vec!["task", "plan:ts-0002-bb", "setStatus", "done"], true),
        (vec!["task", "new", "try it"], true),
        (vec!["task", "create", "a title"], false),
        (vec!["task", "reid", "12"], false),
        (vec!["sections"], false),
    ] {
        let command = if words[0] == "sections" {
            collide_host::task_cli::parse_sections(&[]).unwrap()
        } else {
            parsed(&words)
        };
        assert_eq!(wants_daemon(&command), through, "{words:?}");
    }
}

// ---- a daemon over a fixture repository ----------------------------------------------------------------------------

fn copy_tree(from: &Path, repo: &GitRepo, to: &str) {
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name().to_str().unwrap().to_string();
        let target = if to.is_empty() {
            name
        } else {
            format!("{to}/{name}")
        };
        if entry.path().is_dir() {
            copy_tree(&entry.path(), repo, &target);
        } else {
            repo.write(&target, &fs::read(entry.path()).unwrap());
        }
    }
}

fn git_env(root: &Path) -> Vec<(String, String)> {
    vec![
        ("HOME".to_string(), root.display().to_string()),
        ("GIT_CONFIG_GLOBAL".to_string(), "/dev/null".to_string()),
        ("GIT_CONFIG_SYSTEM".to_string(), "/dev/null".to_string()),
        ("GIT_CONFIG_NOSYSTEM".to_string(), "1".to_string()),
    ]
}

/// A login shell that clears the environment, so the daemon's capture never runs the user's own shell.
fn clean_shell(root: &Path) -> String {
    let shell = root.join("fake-shell");
    fs::write(&shell, "#!/bin/sh\neval \"$4\"\n").unwrap();
    fs::set_permissions(&shell, fs::Permissions::from_mode(0o755)).unwrap();
    shell.display().to_string()
}

/// A fixture repository with the plan fixture and two views, and the daemon over it.
struct Rig {
    repo: GitRepo,
    /// Kept so the packs bundle lives as long as the daemon.
    _bundle: Bundle,
    daemon: Daemon,
    board: String,
}

impl Rig {
    fn new() -> Rig {
        Rig::with_files(&[])
    }

    /// A rig whose repository also commits `files` (path and text).
    fn with_files(files: &[(&str, String)]) -> Rig {
        let repo = GitRepo::new();
        for (path, text) in files {
            repo.write(path, text.as_bytes());
        }
        copy_tree(&repo_root().join("host/tests/fixtures/task-cli"), &repo, "");
        repo.write("line.txt", b"one\n");
        repo.write(".collide/views/ready-now.yaml", READY_NOW.as_bytes());
        repo.write(".collide/views/broken.yaml", BROKEN.as_bytes());
        repo.commit_all("fixture");
        let bundle = common::bundle("task-daemon-cli");
        let root = repo.root().path().to_path_buf();
        let cache = root.join("cache");
        let sdk = ensure_sdk(&cache, &bundle_sdk(&bundle).unwrap()).unwrap();
        let builtin = bundle.0.clone();
        let factory: LoadContextFactory = std::sync::Arc::new(move |path| {
            Ok(LoadContext::new(
                cache.clone(),
                sdk.clone(),
                Environment {
                    path: path.to_os_string(),
                },
                BuiltinPacks::new(builtin.clone()),
            ))
        });
        let mut options = DaemonOptions::new(
            DaemonPaths::under(&root),
            clean_shell(&root),
            Box::new(std::io::sink()),
        );
        options.process_tree = common::test_process_tree();
        options.capture = CaptureOptions {
            timeout: Duration::from_secs(10),
            cap_bytes: 1024 * 1024,
            fallback: BTreeMap::new(),
        };
        options.home = Some(root.display().to_string());
        options.git_env = git_env(&root);
        options.load_context = Some(factory);
        options.write_poll = POLL;
        options.corpus_recheck = POLL;
        let daemon = Daemon::start(options).expect("the daemon starts");
        let mut rig = Rig {
            repo,
            _bundle: bundle,
            daemon,
            board: String::new(),
        };
        rig.board = rig.open_board(&rig.work());
        rig.trust(&rig.board);
        rig
    }

    fn root(&self) -> &Path {
        self.repo.root().path()
    }

    /// The board's root as the daemon keeps it.
    fn work(&self) -> PathBuf {
        fs::canonicalize(self.repo.work()).unwrap()
    }

    fn paths(&self) -> DaemonPaths {
        self.daemon.paths().clone()
    }

    fn call<R: DeserializeOwned>(&self, method: &str, params: &impl Serialize) -> R {
        let mut client = OwnerClient::connect(self.daemon.paths(), "task-daemon-cli-test")
            .expect("the client connects");
        client
            .call(method, params, Some(Duration::from_secs(60)))
            .unwrap_or_else(|error| panic!("{method}: {error}"))
    }

    fn open_board(&self, root: &Path) -> String {
        let opened: BoardOpenResult = self.call(
            "board.open",
            &BoardOpenParams {
                root: root.display().to_string(),
                init_git: false,
            },
        );
        opened.board.expect("the folder is a board").id
    }

    /// Trusts the line, as the owner does in the app, so its views are read.
    fn trust(&self, board: &str) {
        let review: BoardTrustReviewResult = self.call(
            "board.trust_review",
            &BoardTrustReviewParams {
                board: board.to_string(),
                line: "main".to_string(),
            },
        );
        let hash = match review.trust {
            TrustState::Trusted { hash } | TrustState::Untrusted { hash } => hash,
        };
        let _: BoardTrustResult = self.call(
            "board.trust",
            &BoardTrustParams {
                board: board.to_string(),
                line: "main".to_string(),
                hash,
                hosts: Vec::new(),
                dev_folders: Vec::new(),
            },
        );
    }

    /// `collide <words>` from `cwd`: the exit code, standard output and standard error.
    fn cli_in(&self, cwd: &Path, words: &[&str]) -> (u8, String, String) {
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = run_with(&self.paths(), &parsed(words), cwd, &mut out, &mut err);
        (
            code,
            String::from_utf8(out).unwrap(),
            String::from_utf8(err).unwrap(),
        )
    }

    fn cli(&self, words: &[&str]) -> (u8, String, String) {
        self.cli_in(&self.work(), words)
    }

    /// `task.list` through the daemon, as the command's own call is made, for the keys it gives.
    fn listed(&self, params: TaskListParams) -> TaskListResult {
        self.call("task.list", &params)
    }

    fn list_params(&self) -> TaskListParams {
        TaskListParams {
            board: self.board.clone(),
            view: None,
            ready: false,
            kind: None,
            checkout: None,
            limit: None,
            cursor: None,
        }
    }

    fn settled_keys(&self, params: TaskListParams) -> Vec<String> {
        let mut keys = Vec::new();
        wait_until("the first listing ends", DEADLINE, || {
            let listed = self.listed(params.clone());
            keys = listed.tasks.iter().map(|task| task.key.clone()).collect();
            !listed.refreshing
        });
        keys
    }

    fn get(&self, id: &str) -> TaskGetResult {
        self.call(
            "task.get",
            &TaskGetParams {
                board: self.board.clone(),
                id: id.to_string(),
                checkout: None,
            },
        )
    }

    fn queue(&self) -> Vec<QueueItem> {
        let listed: QueueListResult = self.call(
            "queue.list",
            &QueueListParams {
                board: None,
                run: None,
                kinds: Vec::new(),
            },
        );
        listed.items
    }

    fn file_text(&self, name: &str) -> String {
        fs::read_to_string(self.repo.work().join(EPIC).join(name)).unwrap()
    }

    fn status_of(&self, name: &str) -> String {
        self.file_text(name)
            .lines()
            .find_map(|line| line.strip_prefix("status: "))
            .expect("the file has a status")
            .to_string()
    }

    /// Records a run of `collide/quick` on the tasks `keys`, and ends it in `to`.
    fn run(&self, run: &str, keys: &[&str], to: RunState) {
        let phases = self.daemon.phases();
        let actor = Actor {
            kind: ActorKind::Workflow,
            run: Some(run.to_string()),
            phase: None,
        };
        let transition = |from: Option<RunState>, to: RunState| {
            phases
                .append_event(
                    actor.clone(),
                    EventBody::RunTransition {
                        run: run.to_string(),
                        from,
                        to,
                    },
                )
                .unwrap();
        };
        transition(None, RunState::Running {});
        let linked = self
            .daemon
            .store()
            .set_run_launch(
                run,
                &RunLaunch {
                    board: Some(self.board.clone()),
                    tasks: keys
                        .iter()
                        .map(|key| collide_core::owner::RunTaskRef {
                            key: key.to_string(),
                            status: "doing".to_string(),
                        })
                        .collect(),
                    line: "main".to_string(),
                    workflow: "collide/quick".to_string(),
                },
            )
            .unwrap();
        assert!(linked, "{run} has a run.transition before it is linked");
        transition(Some(RunState::Running {}), to);
    }

    /// A second worktree of the repository on a new branch, beside the checkout. Returns its real path.
    fn worktree(&self, name: &str, branch: &str) -> PathBuf {
        let path = self.root().join(name);
        self.repo.git(&[
            "worktree",
            "add",
            "-q",
            "-b",
            branch,
            path.to_str().unwrap(),
        ]);
        fs::canonicalize(path).unwrap()
    }
}

fn json(text: &str) -> Value {
    serde_json::from_str(text).unwrap_or_else(|error| panic!("{error}: {text}"))
}

fn first_words(text: &str) -> Vec<String> {
    text.lines()
        .map(|line| line.split_whitespace().next().unwrap().to_string())
        .collect()
}

// ---- task new ------------------------------------------------------------------------------------------------------

#[test]
fn task_new_prints_the_key_and_the_task_is_in_a_view_of_that_board_only() {
    let rig = Rig::new();
    let other = GitRepo::new();
    other.write("README.md", b"second\n");
    other.write(".collide/views/ready-now.yaml", READY_NOW.as_bytes());
    other.commit_all("second board");
    let second = rig.open_board(&fs::canonicalize(other.work()).unwrap());
    rig.trust(&second);

    let (code, out, err) = rig.cli(&["task", "new", "try the cache"]);

    assert_eq!((code, err.as_str()), (exit::OK, ""), "{out}");
    let key = out.trim_end().to_string();
    assert!(
        key.strip_prefix("scratch:")
            .is_some_and(|n| n.parse::<u32>().is_ok()),
        "a scratch key and nothing else: {out:?}"
    );
    let (code, here, _) = rig.cli(&["tasks", "--view", "ready-now"]);
    assert_eq!(code, exit::OK);
    assert!(
        here.lines()
            .any(|line| line.starts_with(&key) && line.ends_with("try the cache")),
        "{here}"
    );
    let (code, there, err) = rig.cli_in(other.work(), &["tasks", "--view", "ready-now"]);
    assert_eq!((code, err.as_str()), (exit::OK, ""), "{there}");
    assert!(!there.contains("try the cache"), "{there}");
}

#[test]
fn task_new_with_json_prints_the_providers_result_and_nothing_else() {
    let rig = Rig::new();
    let (code, out, _) = rig.cli(&["task", "new", "first line\nsecond line", "--json"]);
    assert_eq!(code, exit::OK);
    let result = json(&out);
    assert_eq!(result["outcome"], "ok");
    assert_eq!(result["task"]["title"], "first line");
    assert!(
        result["task"]["key"]
            .as_str()
            .unwrap()
            .starts_with("scratch:")
    );
}

#[test]
fn a_new_scratch_task_can_be_changed_by_its_key_through_the_daemon() {
    let rig = Rig::new();
    let key = rig
        .cli(&["task", "new", "try the cache"])
        .1
        .trim()
        .to_string();

    let (code, out, err) = rig.cli(&["task", &key, "setStatus", "done"]);

    assert_eq!((code, err.as_str()), (exit::OK, ""));
    assert!(out.contains("status  ready -> done"), "{out}");
    let (code, _, err) = rig.cli(&["task", &key, "setStatus", "no-such-status"]);
    assert_eq!(code, exit::USAGE, "{err}");
    assert!(err.contains("`status` must be one of"), "{err}");
}

#[test]
fn an_op_the_source_does_not_declare_is_a_usage_error_that_lists_the_ones_it_does() {
    let rig = Rig::new();
    let key = rig
        .cli(&["task", "new", "try the cache"])
        .1
        .trim()
        .to_string();
    let (code, out, err) = rig.cli(&["task", &key, "bogus"]);
    assert_eq!(code, exit::USAGE, "{out}");
    assert!(err.contains("unknown op `bogus`"), "{err}");
    assert!(err.contains("setStatus"), "{err}");
    assert!(!err.contains("create"), "create takes no id: {err}");
}

// ---- tasks --view --------------------------------------------------------------------------------------------------

#[test]
fn a_view_prints_exactly_the_members_task_list_gives_for_it_in_the_same_order() {
    let rig = Rig::new();
    rig.cli(&["task", "new", "try the cache"]);
    let mut params = rig.list_params();
    params.view = Some(ViewRef::Named {
        name: "ready-now".to_string(),
    });
    let expected = rig.settled_keys(params);
    assert!(expected.len() > 3, "the view has members: {expected:?}");

    let (code, out, err) = rig.cli(&["tasks", "--view", "ready-now"]);

    assert_eq!((code, err.as_str()), (exit::OK, ""));
    assert_eq!(first_words(&out), expected);
}

#[test]
fn a_view_with_json_prints_the_daemons_answer_with_the_views_report() {
    let rig = Rig::new();
    let (code, out, _) = rig.cli(&["tasks", "--view", "ready-now", "--json"]);
    assert_eq!(code, exit::OK);
    let document = json(&out);
    assert_eq!(document["view"]["name"], "ready-now");
    assert_eq!(
        document["view"]["members"].as_u64().unwrap() as usize,
        document["tasks"].as_array().unwrap().len()
    );
}

#[test]
fn ready_and_kind_apply_on_top_of_a_view() {
    let rig = Rig::new();
    let mut params = rig.list_params();
    params.view = Some(ViewRef::Named {
        name: "ready-now".to_string(),
    });
    params.ready = true;
    params.kind = Some("task".to_string());
    let expected = rig.settled_keys(params);
    let all = rig.settled_keys(TaskListParams {
        view: Some(ViewRef::Named {
            name: "ready-now".to_string(),
        }),
        ..rig.list_params()
    });
    assert!(expected.len() < all.len(), "the rule leaves some out");

    let (code, out, _) = rig.cli(&["tasks", "--view", "ready-now", "--ready", "--kind", "task"]);

    assert_eq!(code, exit::OK);
    assert_eq!(first_words(&out), expected);
}

const SPIKE_VIEWS: [(&str, &str); 2] = [
    (
        ".collide/views/spikes.yaml",
        "name: spikes\ninclude: [plan:sp-0001-aa]\n",
    ),
    (
        ".collide/views/clean-spikes.yaml",
        "name: clean-spikes\ninclude: [plan:sp-0002-bb]\n",
    ),
];

/// A spike record with `body` below its frontmatter.
fn spike_text(id: &str, body: &str) -> String {
    format!(
        "---\nid: {id}\nkind: spike\nstatus: results\ntitle: \"Made-up spike\"\nowners: []\nlinks: []\n---\n\n{body}"
    )
}

/// A rig with two spikes: `sp-0001-aa` has two claim blocks that are not right, `sp-0002-bb` has one that is.
fn spike_rig() -> Rig {
    let bad = "# Spike\n\n```claim\nlabel: measured\nsays: no id\n```\n\n\
               ```claim\nid: Q1.c2\nlabel: guessed\nsays: a bad label\n```\n";
    let good = "# Spike\n\n```claim\nid: Q1.c1\nlabel: measured\nsays: fine\n```\n";
    let mut files = vec![
        (
            ".collide/planning/spikes/sp-0001-aa-bad/record.md",
            spike_text("sp-0001-aa", bad),
        ),
        (
            ".collide/planning/spikes/sp-0002-bb-good/record.md",
            spike_text("sp-0002-bb", good),
        ),
    ];
    files.extend(
        SPIKE_VIEWS
            .iter()
            .map(|(path, text)| (*path, (*text).to_string())),
    );
    Rig::with_files(&files)
}

#[test]
fn a_view_over_spikes_prints_the_count_of_part_problems_on_standard_error_and_exits_0() {
    let rig = spike_rig();

    let (code, out, err) = rig.cli(&["tasks", "--view", "spikes"]);

    assert_eq!(code, exit::OK, "{err}");
    assert_eq!(first_words(&out), ["plan:sp-0001-aa"]);
    assert_eq!(
        err,
        "collide: view `spikes` has 2 problem(s) in the parts of its items\n"
    );
}

#[test]
fn a_view_over_spikes_with_no_problem_says_nothing_and_json_carries_the_count() {
    let rig = spike_rig();

    let (code, out, err) = rig.cli(&["tasks", "--view", "clean-spikes"]);

    assert_eq!((code, err.as_str()), (exit::OK, ""));
    assert_eq!(first_words(&out), ["plan:sp-0002-bb"]);
    let (code, out, err) = rig.cli(&["tasks", "--view", "spikes", "--json"]);
    assert_eq!(code, exit::OK);
    assert_eq!(json(&out)["view"]["part_problems"], 2);
    assert!(err.contains("2 problem(s) in the parts"), "{err}");
}

/// A rig with two spikes and no view file of its own, so `spikes` is the plan pack's view.
fn plain_spike_rig() -> Rig {
    let record = |id: &str, folder: &str| {
        (
            format!(".collide/planning/spikes/{folder}/record.md"),
            spike_text(id, "# Spike\n"),
        )
    };
    let files = [
        record("sp-0001-aa", "sp-0001-aa-one"),
        record("sp-0002-bb", "sp-0002-bb-two"),
    ];
    let files: Vec<(&str, String)> = files
        .iter()
        .map(|(path, text)| (path.as_str(), text.clone()))
        .collect();
    Rig::with_files(&files)
}

#[test]
fn a_bare_view_name_gives_the_same_list_as_the_full_name_and_the_same_json() {
    let rig = plain_spike_rig();

    let (code, bare, err) = rig.cli(&["tasks", "--view", "spikes"]);
    let (full_code, full, full_err) = rig.cli(&["tasks", "--view", "collide/plan/spikes"]);

    assert_eq!((code, full_code), (exit::OK, exit::OK), "{err}{full_err}");
    assert_eq!(first_words(&bare), ["plan:sp-0001-aa", "plan:sp-0002-bb"]);
    assert_eq!((&bare, &err), (&full, &full_err));
    let (_, bare, _) = rig.cli(&["tasks", "--view", "spikes", "--json"]);
    let (_, full, _) = rig.cli(&["tasks", "--view", "collide/plan/spikes", "--json"]);
    assert_eq!(json(&bare)["view"], json(&full)["view"]);
    assert_eq!(json(&bare)["view"]["name"], "collide/plan/spikes");
}

#[test]
fn a_view_name_no_view_has_exits_3_and_lists_the_full_names() {
    let rig = plain_spike_rig();

    let (code, out, err) = rig.cli(&["tasks", "--view", "nope"]);

    assert_eq!((code, out.as_str()), (exit::ID, ""));
    for name in [
        "ready",
        "collide/plan/spikes",
        "collide/plan/decisions-needed",
    ] {
        assert!(err.contains(name), "{err}");
    }
}

#[test]
fn the_built_in_ready_view_lists_what_the_ready_flag_lists() {
    let rig = Rig::new();

    let (code, by_view, err) = rig.cli(&["tasks", "--view", "ready"]);
    let (_, by_flag, _) = rig.cli(&["tasks", "--ready"]);

    assert_eq!(code, exit::OK, "{err}");
    assert!(!by_view.is_empty(), "the fixture has ready tasks");
    assert_eq!(by_view, by_flag);
}

#[test]
fn json_of_a_spike_carries_its_claims_and_the_problems_of_its_claim_blocks() {
    let rig = spike_rig();

    let bad = json(&rig.cli(&["task", "sp-0001-aa", "--json"]).1);
    let good = json(&rig.cli(&["task", "sp-0002-bb", "--json"]).1);

    assert_eq!(bad["task"]["state"]["status"], "results");
    let problems = bad["task"]["partProblems"].as_array().unwrap();
    let lines: Vec<u64> = problems
        .iter()
        .map(|p| p["line"].as_u64().unwrap())
        .collect();
    assert_eq!(lines, [12, 17]);
    assert!(bad["task"].get("claims").is_none(), "{bad}");
    assert_eq!(
        good["task"]["claims"],
        json!([{ "id": "Q1.c1", "label": "measured", "loadBearing": false }])
    );
    assert!(good["task"].get("partProblems").is_none(), "{good}");
}

#[test]
fn a_view_with_an_error_prints_the_error_and_no_member_and_exits_10() {
    let rig = Rig::new();
    let (code, out, err) = rig.cli(&["tasks", "--view", "broken"]);
    assert_eq!(code, exit::VIEW_FAILED);
    assert_eq!(exit::VIEW_FAILED, 10);
    assert_eq!(out, "", "no member is printed");
    assert!(err.contains("view `broken` has an error"), "{err}");
}

#[test]
fn a_view_with_an_error_in_json_still_prints_one_document_with_no_task() {
    let rig = Rig::new();
    let (code, out, _) = rig.cli(&["tasks", "--view", "broken", "--json"]);
    assert_eq!(code, exit::VIEW_FAILED);
    let document = json(&out);
    assert!(document["view"]["error"].is_string(), "{document}");
    assert_eq!(document["tasks"].as_array().unwrap().len(), 0);
}

#[test]
fn a_view_the_board_does_not_have_exits_3_and_names_the_views_it_has() {
    let rig = Rig::new();
    let (code, out, err) = rig.cli(&["tasks", "--view", "nope"]);
    assert_eq!(code, exit::ID, "{out}");
    assert!(err.contains("ready-now"), "{err}");
}

// ---- tasks --checkout ----------------------------------------------------------------------------------------------

#[test]
fn checkout_limits_the_list_to_the_copies_in_that_worktree() {
    let rig = Rig::new();
    let wt = rig.worktree("wt", "side");
    rig.repo.git(&["worktree", "list"]);
    fs::write(
        wt.join(EPIC).join("ts-0099-zz-only-here.md"),
        "---\nid: ts-0099-zz\nkind: task\nstatus: ready\ntitle: \"Only in the worktree\"\nowners: []\n\
         links: []\n---\n\n\
         ## Goal\n\nMade up.\n",
    )
    .unwrap();
    let mut params = rig.list_params();
    params.checkout = Some(wt.display().to_string());
    wait_until("the worktree's task is listed", DEADLINE, || {
        rig.settled_keys(params.clone())
            .contains(&"plan:ts-0099-zz".to_string())
    });

    let (code, there, err) = rig.cli(&["tasks", "--checkout", "../wt"]);
    assert_eq!((code, err.as_str()), (exit::OK, ""));
    let (code, here, _) = rig.cli(&["tasks", "--checkout", "."]);
    assert_eq!(code, exit::OK);

    assert!(there.contains("plan:ts-0099-zz"), "{there}");
    assert!(!here.contains("plan:ts-0099-zz"), "{here}");
    assert!(here.contains("plan:ts-0002-bb"), "{here}");
}

#[test]
fn checkout_takes_a_view_and_limits_it() {
    let rig = Rig::new();
    let (code, out, err) = rig.cli(&["tasks", "--view", "ready-now", "--checkout", "."]);
    assert_eq!((code, err.as_str()), (exit::OK, ""));
    assert!(out.contains("plan:ts-0002-bb"), "{out}");
}

#[test]
fn a_checkout_that_is_not_a_worktree_of_the_repository_exits_2_and_prints_no_task() {
    let rig = Rig::new();
    let stranger = GitRepo::new();
    for given in [
        "/",
        "no-such-folder",
        stranger.work().to_str().unwrap(),
        "wt-not-added",
    ] {
        let (code, out, err) = rig.cli(&["tasks", "--checkout", given]);
        assert_eq!(code, exit::USAGE, "{given}: {err}");
        assert_eq!(out, "", "{given}");
        assert!(err.contains("--checkout"), "{given}: {err}");
    }
}

// ---- task <id> --json ----------------------------------------------------------------------------------------------

#[test]
fn json_has_the_fields_the_links_in_both_directions_the_version_and_the_copies() {
    let rig = Rig::new();
    let (code, out, err) = rig.cli(&["task", "ts-0002-bb", "--json"]);
    assert_eq!((code, err.as_str()), (exit::OK, ""));
    let document = json(&out);

    let task = &document["task"];
    assert_eq!(task["key"], "plan:ts-0002-bb");
    assert_eq!(task["title"], "Second task, free to start");
    assert_eq!(task["state"]["status"], "ready");
    assert_eq!(task["state"]["category"], "ready");
    assert_eq!(
        task["version"].as_str().unwrap(),
        rig.get("plan:ts-0002-bb").task.unwrap().version
    );
    assert!(task["body"].as_str().unwrap().contains("Made-up content"));
    let links = document["links"].as_array().unwrap();
    let find = |dir: &str, kind: &str, key: &str| {
        links
            .iter()
            .any(|link| link["dir"] == dir && link["type"] == kind && link["key"] == key)
    };
    assert!(find("in", "blocked-by", "plan:ts-0003-cc"), "{links:?}");
    assert!(find("out", "part-of", "plan:ep-001-aa"), "{links:?}");
    assert_eq!(document["copies"].as_array().unwrap().len(), 1);
    assert_eq!(document["ambiguous"].as_array().unwrap().len(), 0);
    assert!(document.get("ops").is_none(), "{document}");
}

#[test]
fn json_of_an_item_that_blocks_on_another_has_the_link_out_with_the_other_ends_state() {
    let rig = Rig::new();
    let (_, out, _) = rig.cli(&["task", "3", "--json"]);
    let document = json(&out);
    let link = document["links"]
        .as_array()
        .unwrap()
        .iter()
        .find(|link| link["type"] == "blocked-by")
        .expect("the link out")
        .clone();
    assert_eq!(link["dir"], "out");
    assert_eq!(link["key"], "plan:ts-0002-bb");
    assert_eq!(link["status"], "ready");
    assert_eq!(link["category"], "ready");
}

#[test]
fn json_has_the_latest_run_that_ended_and_the_runs() {
    let rig = Rig::new();
    rig.cli(&["tasks", "--view", "ready-now"]);
    rig.run("r1", &["plan:ts-0002-bb"], RunState::Failed {});

    let (_, out, _) = rig.cli(&["task", "ts-0002-bb", "--json"]);

    let document = json(&out);
    assert_eq!(document["latest_run"]["id"], "r1");
    assert_eq!(document["latest_run"]["outcome"], "failed");
    assert_eq!(document["runs"], serde_json::json!(["r1"]));
}

#[test]
fn json_takes_a_key_and_a_short_form_and_both_give_the_same_item() {
    let rig = Rig::new();
    let keyed = json(&rig.cli(&["task", "plan:ts-0003-cc", "--json"]).1);
    for query in ["3", "0003", "3-cc", "ts-3"] {
        let (code, out, err) = rig.cli(&["task", query, "--json"]);
        assert_eq!((code, err.as_str()), (exit::OK, ""), "{query}");
        assert_eq!(json(&out), keyed, "{query}");
    }
}

#[test]
fn json_of_a_short_id_that_fits_two_items_lists_both_and_exits_3() {
    let rig = Rig::new();
    let (code, out, err) = rig.cli(&["task", "12", "--json"]);
    assert_eq!(code, exit::ID);
    assert!(err.contains("matches 2 items"), "{err}");
    let document = json(&out);
    assert!(document.get("task").is_none(), "{document}");
    assert_eq!(
        document["ambiguous"],
        serde_json::json!(["plan:ts-0012-dd", "plan:ts-0012-ee"])
    );
}

#[test]
fn json_of_a_key_held_at_two_paths_says_ambiguous_lists_each_location_and_exits_3() {
    let rig = Rig::new();
    let wt = rig.worktree("wt", "side");
    let moved = wt.join(EPIC).join(THIRD_FILE);
    let elsewhere = wt.join(".collide/planning/epics/ep-002-bb-beta/tasks");
    fs::rename(&moved, elsewhere.join(THIRD_FILE)).unwrap();
    wait_until("the key is a collision", DEADLINE, || {
        !rig.listed(rig.list_params()).collisions.is_empty()
    });

    let (code, out, err) = rig.cli(&["task", "plan:ts-0003-cc", "--json"]);

    assert_eq!(code, exit::ID);
    assert!(err.contains("ambiguous"), "{err}");
    let document = json(&out);
    assert!(document.get("task").is_none(), "{document}");
    assert_eq!(
        document["ambiguous"],
        serde_json::json!(["plan:ts-0003-cc"])
    );
    let locations = document["collisions"][0]["locations"].as_array().unwrap();
    assert_eq!(locations.len(), 2, "{document}");
    assert!(locations.iter().all(|place| place["path"].is_string()));
}

#[test]
fn json_of_an_id_that_names_nothing_exits_3_and_prints_no_document() {
    let rig = Rig::new();
    for query in ["ts-9999-zz", "plan:ts-9999-zz"] {
        let (code, out, err) = rig.cli(&["task", query, "--json"]);
        assert_eq!(code, exit::ID, "{query}: {err}");
        assert_eq!(out, "", "{query}");
        assert!(err.contains("ts-9999-zz"), "{query}: {err}");
    }
}

// ---- a write that waits --------------------------------------------------------------------------------------------

/// Two branches that edit one line, and a rebase of the second onto the first that stops on the conflict.
fn stop_in_a_rebase(repo: &GitRepo) {
    repo.git(&["checkout", "--quiet", "-b", "topic"]);
    repo.write("line.txt", b"topic\n");
    repo.commit_all("topic edits the line");
    repo.git(&["checkout", "--quiet", "main"]);
    repo.write("line.txt", b"main\n");
    repo.commit_all("main edits the line");
    repo.git(&["checkout", "--quiet", "topic"]);
    let rebase = git_command(repo.root().path(), repo.work())
        .args(["rebase", "main"])
        .output()
        .expect("git runs");
    assert!(
        !rebase.status.success(),
        "the rebase should stop on the conflict"
    );
}

#[test]
fn a_write_in_a_rebase_says_why_names_the_queue_item_and_exits_11_with_no_wait() {
    let rig = Rig::new();
    stop_in_a_rebase(&rig.repo);
    let before = rig.status_of(SECOND_FILE);

    let (code, out, err) = rig.cli(&[
        "task",
        "plan:ts-0002-bb",
        "setStatus",
        "active",
        "--no-wait",
    ]);

    assert_eq!(code, exit::QUEUED, "{err}");
    assert_eq!(exit::QUEUED, 11);
    assert_eq!(out, "");
    let queue = rig.queue();
    assert_eq!(queue.len(), 1, "{queue:?}");
    assert!(queue[0].id.starts_with("write-"), "{queue:?}");
    assert!(err.contains("is queued: a rebase is in progress"), "{err}");
    assert!(
        err.contains(&format!("queue item {}", queue[0].id)),
        "{err}"
    );
    assert!(err.contains("--no-wait"), "{err}");
    assert_eq!(rig.status_of(SECOND_FILE), before, "nothing was written");
}

#[test]
fn a_wait_that_runs_out_leaves_the_write_queued_and_exits_11() {
    let rig = Rig::new();
    stop_in_a_rebase(&rig.repo);

    let (code, _, err) = rig.cli(&[
        "task",
        "plan:ts-0002-bb",
        "setStatus",
        "active",
        "--wait",
        "1",
    ]);

    assert_eq!(code, exit::QUEUED, "{err}");
    assert!(err.contains("did not apply in 1 seconds"), "{err}");
    assert_eq!(rig.queue().len(), 1, "the write is still in the Queue");
    assert_eq!(rig.status_of(SECOND_FILE), "ready");
}

#[test]
fn a_write_waits_for_a_live_phase_and_reports_it_applied_when_the_phase_ends() {
    let rig = Rig::new();
    let version = rig.get("plan:ts-0002-bb").task.unwrap().version;
    let live = rig.daemon.live_phases();
    live.started("R-7", &rig.work(), false);
    let paths = rig.paths();
    let cwd = rig.work();
    let command = parsed(&["task", "plan:ts-0002-bb", "setStatus", "active"]);
    let cli = std::thread::spawn(move || {
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = run_with(&paths, &command, &cwd, &mut out, &mut err);
        (
            code,
            String::from_utf8(out).unwrap(),
            String::from_utf8(err).unwrap(),
        )
    });
    wait_until("the write is queued", DEADLINE, || !rig.queue().is_empty());

    live.ended("R-7");
    let (code, out, err) = cli.join().unwrap();

    assert_eq!(code, exit::OK, "{err}");
    assert_ne!(
        rig.get("plan:ts-0002-bb").task.unwrap().version,
        version,
        "the version changed"
    );
    assert!(
        err.contains("is queued: a phase of run R-7 is running in"),
        "{err}"
    );
    assert!(out.contains("the queued write was applied"), "{out}");
    assert_eq!(rig.status_of(SECOND_FILE), "active");
    assert!(rig.queue().is_empty());
}

#[test]
fn a_queued_write_that_a_person_dismisses_exits_5_says_so_and_writes_nothing() {
    let rig = Rig::new();
    let live = rig.daemon.live_phases();
    live.started("R-7", &rig.work(), false);
    let paths = rig.paths();
    let cwd = rig.work();
    let command = parsed(&["task", "plan:ts-0002-bb", "setStatus", "active"]);
    let cli = std::thread::spawn(move || {
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = run_with(&paths, &command, &cwd, &mut out, &mut err);
        (
            code,
            String::from_utf8(out).unwrap(),
            String::from_utf8(err).unwrap(),
        )
    });
    wait_until("the write is queued", DEADLINE, || !rig.queue().is_empty());
    let item = rig.queue()[0].id.clone();

    let answered: QueueAnswerResult = rig.call(
        "queue.answer",
        &QueueAnswerParams {
            item,
            answer: QueueAnswer::Dismiss {},
        },
    );
    let (code, out, err) = cli.join().unwrap();
    live.ended("R-7");

    assert_eq!(answered, QueueAnswerResult::Done {});
    assert_eq!(code, exit::REFUSED, "{err}");
    assert_eq!(out, "", "it does not say applied");
    assert!(
        err.contains(
            "the queued write is no longer queued and plan:ts-0002-bb did not change: \
             it was dismissed in the Queue, or it changed nothing"
        ),
        "{err}"
    );
    assert_eq!(rig.status_of(SECOND_FILE), "ready");
}

#[test]
fn a_queued_write_whose_file_changed_while_it_waited_is_not_applied_and_exits_5_with_the_reason() {
    let rig = Rig::new();
    let live = rig.daemon.live_phases();
    live.started("R-7", &rig.work(), false);
    let paths = rig.paths();
    let cwd = rig.work();
    let command = parsed(&["task", "plan:ts-0002-bb", "setStatus", "active"]);
    let cli = std::thread::spawn(move || {
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = run_with(&paths, &command, &cwd, &mut out, &mut err);
        (
            code,
            String::from_utf8(out).unwrap(),
            String::from_utf8(err).unwrap(),
        )
    });
    wait_until("the write is queued", DEADLINE, || !rig.queue().is_empty());
    let file = rig.repo.work().join(EPIC).join(SECOND_FILE);
    let text = fs::read_to_string(&file).unwrap();
    fs::write(
        &file,
        format!("{text}\nEdited by a person while the write waited.\n"),
    )
    .unwrap();

    live.ended("R-7");
    let (code, _, err) = cli.join().unwrap();

    assert_eq!(code, exit::REFUSED, "{err}");
    assert!(err.contains("the queued write was not applied"), "{err}");
    assert_eq!(rig.status_of(SECOND_FILE), "ready");
}

#[test]
fn a_write_to_a_free_checkout_is_applied_at_once_and_says_nothing_of_a_queue() {
    let rig = Rig::new();
    let (code, out, err) = rig.cli(&["task", "plan:ts-0002-bb", "setStatus", "active"]);
    assert_eq!((code, err.as_str()), (exit::OK, ""));
    assert!(out.contains("status  ready -> active"), "{out}");
    assert_eq!(rig.status_of(SECOND_FILE), "active");
}

#[test]
fn an_argument_the_op_does_not_take_is_a_usage_error_and_nothing_is_written() {
    let rig = Rig::new();
    let (code, _, err) = rig.cli(&["task", "plan:ts-0002-bb", "setStatus", "no-such"]);
    assert_eq!(code, exit::USAGE, "{err}");
    assert!(err.contains("`status` must be one of"), "{err}");
    assert_eq!(rig.status_of(SECOND_FILE), "ready");
}

#[test]
fn a_write_by_a_key_with_json_prints_the_providers_result() {
    let rig = Rig::new();
    let (code, out, _) = rig.cli(&["task", "plan:ts-0002-bb", "setStatus", "active", "--json"]);
    assert_eq!(code, exit::OK);
    assert_eq!(json(&out)["outcome"], "ok");
}

#[test]
fn a_queued_write_with_json_prints_one_document_that_says_where_it_stands() {
    let rig = Rig::new();
    stop_in_a_rebase(&rig.repo);
    let (code, out, _) = rig.cli(&[
        "task",
        "plan:ts-0002-bb",
        "setStatus",
        "active",
        "--no-wait",
        "--json",
    ]);
    assert_eq!(code, exit::QUEUED);
    let document = json(&out);
    assert_eq!(document["settled"], "queued");
    assert!(
        document["queued"]["item"]
            .as_str()
            .unwrap()
            .starts_with("write-")
    );
    assert!(
        document["queued"]["reason"]
            .as_str()
            .unwrap()
            .contains("a rebase")
    );
}

// ---- a person-only write (ts-0128-bb) ------------------------------------------------------------------------------

fn proposal_status(rig: &Rig) -> String {
    let text = fs::read_to_string(rig.repo.work().join(PROPOSAL)).unwrap();
    text.lines()
        .find_map(|line| line.strip_prefix("status: "))
        .unwrap()
        .to_string()
}

#[test]
fn a_person_only_write_says_a_confirmation_waits_in_the_app_and_exits_12() {
    let rig = Rig::with_files(&[(PROPOSAL, proposal_text(None))]);

    let (code, out, err) = rig.cli(&["task", "plan:pr-0003-km", "setStatus", "rejected"]);

    assert_eq!(code, exit::CONFIRMATION, "{err}");
    assert_eq!(exit::CONFIRMATION, 12);
    assert_eq!(out, "");
    assert!(err.contains("a confirmation waits in the app"), "{err}");
    let queue = rig.queue();
    assert_eq!(queue.len(), 1, "{queue:?}");
    assert_eq!(queue[0].kind, collide_core::owner::QueueKind::Confirmation);
    assert!(
        err.contains(&format!("queue item {}", queue[0].id)),
        "{err}"
    );
    assert!(err.contains("nothing was written"), "{err}");
    assert_eq!(proposal_status(&rig), "draft", "nothing was written");
}

#[test]
fn json_of_a_person_only_write_carries_the_confirmation_and_exits_12() {
    let rig = Rig::with_files(&[(PROPOSAL, proposal_text(None))]);

    let (code, out, _) = rig.cli(&["task", "plan:pr-0003-km", "setStatus", "accepted", "--json"]);

    assert_eq!(code, exit::CONFIRMATION);
    let document = json(&out);
    let queue = rig.queue();
    assert_eq!(document["confirmation"]["item"], json!(queue[0].id));
    assert!(
        document["confirmation"]["reason"]
            .as_str()
            .unwrap()
            .contains("accept"),
        "{document}"
    );
    assert_eq!(proposal_status(&rig), "draft");
}

#[test]
fn a_write_to_a_proposal_that_is_not_a_persons_is_made_as_before() {
    let rig = Rig::with_files(&[(PROPOSAL, proposal_text(None))]);

    let (code, _, err) = rig.cli(&["task", "plan:pr-0003-km", "setStatus", "proposed"]);

    assert_eq!(code, exit::OK, "{err}");
    assert_eq!(proposal_status(&rig), "proposed");
    assert!(rig.queue().is_empty());
}

// ---- the item JSON that the direct path used to print (ts-0020, ts-0047) ------------------------------------------

const PROPOSAL: &str =
    ".collide/planning/epics/ep-001-aa-alpha/proposals/pr-0003-km-features-in-proposals.md";

fn proposal_text(features_part: Option<&str>) -> String {
    let features = features_part.map_or(String::new(), |text| format!("## Features\n\n{text}"));
    format!(
        "---\nid: pr-0003-km\nkind: proposal\nstatus: draft\ntitle: \"Features in proposals\"\nowners: []\n\
         links: []\n---\n\n\
         ## Context\n\nMade-up context.\n\n{features}## Decision\n"
    )
}

#[test]
fn json_carries_the_epic_as_the_part_of_link_and_the_body() {
    let rig = Rig::new();
    let document = json(&rig.cli(&["task", "ts-0003-cc", "--json"]).1);
    let part_of: Vec<&Value> = document["links"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|link| link["type"] == "part-of")
        .collect();
    assert_eq!(part_of.len(), 1, "{document}");
    assert_eq!(part_of[0]["key"], "plan:ep-001-aa");
    assert!(
        document["task"]["body"]
            .as_str()
            .unwrap()
            .contains("Made-up content")
    );
}

#[test]
fn json_lists_the_features_of_a_proposal_and_the_problems_of_its_features_part() {
    let features = "### Packs\n- **F1 Trust prompt**: first\n- **F2 Pack lock**: second\n\
                    ### Unsorted\n- **F3 Badge**: third\nbad\n";
    let rig = Rig::with_files(&[(PROPOSAL, proposal_text(Some(features)))]);

    let document = json(&rig.cli(&["task", "pr-0003-km", "--json"]).1);

    let task = &document["task"];
    let found: Vec<(&str, &str)> = task["features"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| (f["id"].as_str().unwrap(), f["section"].as_str().unwrap()))
        .collect();
    assert_eq!(found, [("F1", "Packs"), ("F2", "Packs"), ("F3", "")]);
    assert!(
        task["featureProblems"][0]["reason"]
            .as_str()
            .unwrap()
            .contains("neither an entry nor a continuation"),
        "{task}"
    );
}

#[test]
fn json_lists_the_decisions_of_a_proposal_and_the_problems_of_its_log_through_task_get() {
    let log = "## Decision\n\n- **D1 Keep the files**: Plain files.\n  Evidence: ts-9999-zz\n\
               - **D2 Use a lock**: A lock file.\n  Passed on: A daemon, since it is one more process.\n  \
               Evidence: ts-0003-cc\n";
    let text = proposal_text(None).replace("## Decision\n", log);
    let rig = Rig::with_files(&[(PROPOSAL, text)]);

    let document = json(&rig.cli(&["task", "pr-0003-km", "--json"]).1);

    let task = &document["task"];
    let ids: Vec<&str> = task["decisions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|decision| decision["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["D1", "D2"], "{task}");
    assert_eq!(task["decisions"][1]["evidence"][0]["status"], "ready");
    let problems = task["partProblems"].as_array().unwrap();
    assert_eq!(problems.len(), 2, "{task}");
    assert!(problems.iter().all(|problem| problem["part"] == "Decision"));
    assert!(problems[0]["reason"].as_str().unwrap().starts_with("D1 "));
}

#[test]
fn json_of_a_proposal_with_a_plain_text_decision_section_has_no_decisions_or_part_problems() {
    let text = proposal_text(None).replace("## Decision\n", "## Decision\n\nWe chose files.\n");
    let rig = Rig::with_files(&[(PROPOSAL, text)]);
    let document = json(&rig.cli(&["task", "pr-0003-km", "--json"]).1);
    let task = &document["task"];
    for key in ["decisions", "assumptions", "findings", "partProblems"] {
        assert!(task.get(key).is_none(), "{key} in {task}");
    }
}

#[test]
fn json_of_a_proposal_with_no_features_part_has_neither_features_nor_problems() {
    let rig = Rig::with_files(&[(PROPOSAL, proposal_text(None))]);
    let document = json(&rig.cli(&["task", "pr-0003", "--json"]).1);
    let task = &document["task"];
    assert!(task.get("features").is_none(), "{task}");
    assert!(task.get("featureProblems").is_none(), "{task}");
}
