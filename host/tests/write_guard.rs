//! The guards on a write to a checkout (ts-0114-hr; 0.4, Line lock and Root busy; 0.10, Ref-scoped sources): a write to
//! a checkout with a rebase, merge, cherry-pick, revert or bisect in progress, or a live phase, is queued and applied
//! when the checkout is free. The daemon runs in the test's process over its real socket, and the plan provider is the
//! built-in one from a bundle built here, over a copy of `host/tests/fixtures/task-cli` in a fixture repository.
//!
//! Every path is below the test's own root under `/tmp`, which is also the home. The daemon's captured environment is
//! made by a login shell that clears the environment, and every git command clears it too. Nothing here touches the
//! user's home, keychain or a real daemon.

mod common;

use collide_core::change::ActorKind;
use collide_core::events::EventBody;
use collide_core::owner::{
    BoardOpenParams, BoardOpenResult, QueueAnswer, QueueAnswerParams, QueueAnswerResult, QueueItem,
    QueueKind, QueueListParams, QueueListResult, TaskApplyParams, TaskApplyResult, TaskGetParams,
    TaskGetResult,
};
use collide_core::provider::{ApplyParams, ApplyResult, ChangeCause};
use collide_host::base_env::CaptureOptions;
use collide_host::board_load::LoadContext;
use collide_host::builtin_packs::{BuiltinPacks, Environment};
use collide_host::checkout::root_busy;
use collide_host::config_get::LoadContextFactory;
use collide_host::daemon::{Daemon, DaemonOptions, DaemonPaths};
use collide_host::owner_client::OwnerClient;
use collide_host::pack_cache::ensure_sdk;
use collide_host::pack_location::bundle_sdk;
use collide_host::store::Store;
use collide_host::write_guard::{LineLocks, LivePhases};
use common::git_repo::{GitRepo, git_command};
use common::{Bundle, TempRoot, repo_root, wait_until};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

const DEADLINE: Duration = Duration::from_secs(20);
const WAIT: Option<Duration> = Some(Duration::from_secs(60));

/// How often the daemon's writer looks at a checkout that stayed busy. A test also reaches the writer with a Retry.
const POLL: Duration = Duration::from_millis(100);

const EPIC: &str = ".collide/planning/epics/ep-001-aa-alpha/tasks";
const SECOND: &str = "ts-0002-bb";
const TWELFTH: &str = "ts-0012-dd";

fn task_file(id: &str) -> String {
    let name = match id {
        SECOND => "ts-0002-bb-second.md",
        TWELFTH => "ts-0012-dd-twelve-a.md",
        other => panic!("the fixture has no file for {other}"),
    };
    format!("{EPIC}/{name}")
}

// ---- the busy states, with no daemon ------------------------------------------------------------------------------

/// The words each state gets and the file or folder git keeps while a checkout is in it.
const STATES: [(&str, &str, bool); 6] = [
    ("rebase-merge", "a rebase", true),
    ("rebase-apply", "a rebase", true),
    ("MERGE_HEAD", "a merge", false),
    ("CHERRY_PICK_HEAD", "a cherry-pick", false),
    ("REVERT_HEAD", "a revert", false),
    ("BISECT_LOG", "a bisect", false),
];

fn git_env(root: &Path) -> Vec<(String, String)> {
    vec![
        ("HOME".to_string(), root.display().to_string()),
        ("GIT_CONFIG_GLOBAL".to_string(), "/dev/null".to_string()),
        ("GIT_CONFIG_SYSTEM".to_string(), "/dev/null".to_string()),
        ("GIT_CONFIG_NOSYSTEM".to_string(), "1".to_string()),
    ]
}

fn committed_repo() -> GitRepo {
    let repo = GitRepo::new();
    repo.write("README.md", b"fixture\n");
    repo.commit_all("start");
    repo
}

#[test]
fn root_busy_says_nothing_of_a_checkout_that_is_in_no_operation() {
    let repo = committed_repo();
    let env = git_env(repo.root().path());
    assert_eq!(root_busy(&env, repo.work()).unwrap(), None);
}

#[test]
fn root_busy_names_each_of_the_six_states_from_the_file_git_keeps() {
    let repo = committed_repo();
    let env = git_env(repo.root().path());
    for (name, words, is_dir) in STATES {
        let path = repo.git_dir().join(name);
        if is_dir {
            fs::create_dir(&path).unwrap();
        } else {
            fs::write(&path, "x\n").unwrap();
        }
        assert_eq!(root_busy(&env, repo.work()).unwrap(), Some(words), "{name}");
        if is_dir {
            fs::remove_dir(&path).unwrap();
        } else {
            fs::remove_file(&path).unwrap();
        }
        assert_eq!(root_busy(&env, repo.work()).unwrap(), None, "{name} gone");
    }
}

/// Two branches that edit one line, and a rebase of the second onto the first that stops on the conflict. The
/// checkout is on `topic`, in the middle of a rebase.
fn stop_in_a_rebase(repo: &GitRepo) {
    repo.write("line.txt", b"one\n");
    repo.commit_all("line");
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

fn abort_the_rebase(repo: &GitRepo) {
    repo.git(&["rebase", "--abort"]);
}

#[test]
fn root_busy_sees_a_real_rebase_that_stopped_on_a_conflict_until_it_is_aborted() {
    let repo = committed_repo();
    let env = git_env(repo.root().path());
    stop_in_a_rebase(&repo);
    assert_eq!(root_busy(&env, repo.work()).unwrap(), Some("a rebase"));
    abort_the_rebase(&repo);
    assert_eq!(root_busy(&env, repo.work()).unwrap(), None);
}

#[test]
fn root_busy_reads_the_linked_checkout_it_is_given_and_not_the_main_one() {
    let repo = committed_repo();
    let env = git_env(repo.root().path());
    let linked = repo.root().join("linked");
    repo.git(&[
        "worktree",
        "add",
        "--quiet",
        linked.to_str().unwrap(),
        "-b",
        "side",
    ]);
    let git_dir = repo.git(&[
        "-C",
        linked.to_str().unwrap(),
        "rev-parse",
        "--absolute-git-dir",
    ]);
    fs::write(Path::new(&git_dir).join("MERGE_HEAD"), "x\n").unwrap();
    assert_eq!(root_busy(&env, &linked).unwrap(), Some("a merge"));
    assert_eq!(root_busy(&env, repo.work()).unwrap(), None);
}

#[test]
fn root_busy_is_an_error_for_a_folder_git_cannot_read() {
    let root = TempRoot::new();
    let folder = root.join("not-a-repo");
    fs::create_dir(&folder).unwrap();
    assert!(root_busy(&git_env(root.path()), &folder).is_err());
}

// ---- the line lock and the live phases ----------------------------------------------------------------------------

#[test]
fn the_line_lock_lets_one_writer_at_a_time_hold_a_root() {
    let root = TempRoot::new();
    let locks = Arc::new(LineLocks::new());
    let active = Arc::new(AtomicUsize::new(0));
    let overlapped = Arc::new(AtomicBool::new(false));
    let workers: Vec<_> = (0..4)
        .map(|_| {
            let (locks, active, overlapped) = (locks.clone(), active.clone(), overlapped.clone());
            let path = root.path().to_path_buf();
            std::thread::spawn(move || {
                for _ in 0..200 {
                    locks.hold(&path, || {
                        if active.fetch_add(1, Ordering::SeqCst) != 0 {
                            overlapped.store(true, Ordering::SeqCst);
                        }
                        std::thread::yield_now();
                        active.fetch_sub(1, Ordering::SeqCst);
                    });
                }
            })
        })
        .collect();
    for worker in workers {
        worker.join().unwrap();
    }
    assert!(
        !overlapped.load(Ordering::SeqCst),
        "two writers held one root at once"
    );
}

#[test]
fn the_line_locks_of_two_roots_do_not_wait_for_each_other() {
    let root = TempRoot::new();
    let (one, two) = (root.join("one"), root.join("two"));
    fs::create_dir(&one).unwrap();
    fs::create_dir(&two).unwrap();
    let locks = Arc::new(LineLocks::new());
    let (in_one, in_two) = (
        Arc::new(AtomicBool::new(false)),
        Arc::new(AtomicBool::new(false)),
    );
    // Each holder waits, with a deadline, for the other to be inside its own root: they can only both be there when
    // the two roots do not share a lock.
    let meet =
        |locks: Arc<LineLocks>, path: PathBuf, mine: Arc<AtomicBool>, other: Arc<AtomicBool>| {
            std::thread::spawn(move || {
                locks.hold(&path, || {
                    mine.store(true, Ordering::SeqCst);
                    let until = std::time::Instant::now() + DEADLINE;
                    while !other.load(Ordering::SeqCst) && std::time::Instant::now() < until {
                        std::thread::yield_now();
                    }
                    other.load(Ordering::SeqCst)
                })
            })
        };
    let first = meet(locks.clone(), one, in_one.clone(), in_two.clone());
    let second = meet(locks, two, in_two, in_one);
    assert!(
        first.join().unwrap(),
        "the first root waited for the second"
    );
    assert!(
        second.join().unwrap(),
        "the second root waited for the first"
    );
}

#[test]
fn a_live_phase_blocks_its_own_checkout_until_it_ends() {
    let root = TempRoot::new();
    let (one, two) = (root.join("one"), root.join("two"));
    fs::create_dir(&one).unwrap();
    fs::create_dir(&two).unwrap();
    let live = LivePhases::new();
    live.started("R-1", &one, false);
    assert_eq!(live.blocking(&one).as_deref(), Some("R-1"));
    assert_eq!(live.blocking(&two), None);
    live.ended("R-1");
    assert_eq!(live.blocking(&one), None);
}

#[test]
fn a_phase_started_at_the_root_does_not_block_a_write_there() {
    let root = TempRoot::new();
    let live = LivePhases::new();
    live.started("R-1", root.path(), true);
    assert_eq!(live.blocking(root.path()), None);
}

#[test]
fn a_phase_that_ends_is_told_once_and_one_that_was_never_there_is_not_told() {
    let root = TempRoot::new();
    let live = LivePhases::new();
    live.ended("R-9");
    assert!(!live.take_released());
    live.started("R-1", root.path(), false);
    live.ended("R-1");
    assert!(live.take_released());
    assert!(!live.take_released());
}

#[test]
fn a_checkout_is_the_same_one_by_a_link_to_it() {
    let root = TempRoot::new();
    let real = root.join("real");
    fs::create_dir(&real).unwrap();
    let link = root.join("link");
    std::os::unix::fs::symlink(&real, &link).unwrap();
    let live = LivePhases::new();
    live.started("R-1", &real, false);
    assert_eq!(live.blocking(&link).as_deref(), Some("R-1"));
}

// ---- the daemon ---------------------------------------------------------------------------------------------------

/// A login shell that prints a cleared environment, as the capture asks: `HOME` and `COLLIDE_DAEMON_ROOT` of the
/// test's root and the `PATH` the tests run with, and nothing else of the test process's own.
fn clean_shell(root: &Path) -> String {
    let path = std::env::var("PATH").expect("the tests run with a PATH");
    let home = root.display();
    let script = format!(
        "#!/bin/sh\nexec /usr/bin/env -i HOME='{home}' COLLIDE_DAEMON_ROOT='{home}' PATH='{path}' /bin/sh -c \"$4\"\n"
    );
    let file = root.join("clean-shell");
    fs::write(&file, script).unwrap();
    fs::set_permissions(&file, fs::Permissions::from_mode(0o755)).unwrap();
    file.to_str().unwrap().to_string()
}

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

/// A fixture repository holding a copy of the plan fixture, and the daemon over it.
struct Rig {
    repo: GitRepo,
    bundle: Bundle,
    cache: PathBuf,
    daemon: Option<Daemon>,
    board: String,
}

impl Rig {
    fn new() -> Rig {
        let repo = GitRepo::new();
        copy_tree(&repo_root().join("host/tests/fixtures/task-cli"), &repo, "");
        repo.commit_all("fixture");
        let mut rig = Rig {
            repo,
            bundle: common::bundle("write-guard"),
            cache: PathBuf::new(),
            daemon: None,
            board: String::new(),
        };
        rig.cache = rig.root().join("cache");
        rig.start();
        rig.board = rig.open_board();
        rig
    }

    fn root(&self) -> &Path {
        self.repo.root().path()
    }

    /// The board's root as the daemon keeps it.
    fn work(&self) -> PathBuf {
        fs::canonicalize(self.repo.work()).unwrap()
    }

    fn options(&self) -> DaemonOptions {
        let root = self.root();
        let sdk = ensure_sdk(&self.cache, &bundle_sdk(&self.bundle).unwrap()).unwrap();
        let (cache, builtin) = (self.cache.clone(), self.bundle.0.clone());
        let factory: LoadContextFactory = Arc::new(move |path| {
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
            DaemonPaths::under(root),
            clean_shell(root),
            Box::new(std::io::sink()),
        );
        options.process_tree = common::test_process_tree();
        options.capture = CaptureOptions {
            timeout: Duration::from_secs(10),
            cap_bytes: 1024 * 1024,
            fallback: BTreeMap::new(),
        };
        options.home = Some(root.display().to_string());
        options.git_env = git_env(root);
        options.load_context = Some(factory);
        options.write_poll = POLL;
        options
    }

    fn start(&mut self) {
        self.daemon = Some(Daemon::start(self.options()).expect("the daemon starts"));
    }

    fn stop(&mut self) {
        self.daemon.take().expect("a daemon is running").stop();
    }

    fn daemon(&self) -> &Daemon {
        self.daemon.as_ref().expect("a daemon is running")
    }

    fn call<R: DeserializeOwned>(&self, method: &str, params: &impl Serialize) -> R {
        let mut client = OwnerClient::connect(self.daemon().paths(), "write-guard-test")
            .expect("the client connects");
        client
            .call(method, params, WAIT)
            .unwrap_or_else(|error| panic!("{method}: {error}"))
    }

    fn open_board(&self) -> String {
        let opened: BoardOpenResult = self.call(
            "board.open",
            &BoardOpenParams {
                root: self.work().to_str().unwrap().to_string(),
                init_git: false,
            },
        );
        opened.board.expect("the checkout is a board").id
    }

    fn version(&self, id: &str) -> String {
        let got: TaskGetResult = self.call(
            "task.get",
            &TaskGetParams {
                board: self.board.clone(),
                id: format!("plan:{id}"),
                checkout: None,
            },
        );
        got.task.expect("the task is there").version
    }

    /// `setStatus` of `id` to `status`, at the version the task has now, in `checkout` (the board's root when `None`).
    fn set_status(&self, id: &str, status: &str, checkout: Option<&Path>) -> TaskApplyResult {
        let version = self.version(id);
        self.set_status_at(id, status, &version, checkout)
    }

    fn set_status_at(
        &self,
        id: &str,
        status: &str,
        version: &str,
        checkout: Option<&Path>,
    ) -> TaskApplyResult {
        self.call(
            "task.apply",
            &TaskApplyParams {
                board: self.board.clone(),
                source: Some("plan".to_string()),
                apply: ApplyParams {
                    actor: None,
                    op: "setStatus".to_string(),
                    id: Some(id.to_string()),
                    args: BTreeMap::from([("status".to_string(), json!(status))]),
                    version: Some(version.to_string()),
                    checkout: checkout.map(|path| path.to_str().unwrap().to_string()),
                },
                commit: false,
            },
        )
    }

    fn status_on_disk(&self, id: &str) -> String {
        let text = fs::read_to_string(self.repo.work().join(task_file(id))).unwrap();
        text.lines()
            .find_map(|line| line.strip_prefix("status: "))
            .expect("the file has a status")
            .to_string()
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

    fn answer(&self, item: &str, answer: QueueAnswer) -> QueueAnswerResult {
        self.call(
            "queue.answer",
            &QueueAnswerParams {
                item: item.to_string(),
                answer,
            },
        )
    }

    /// The keys of the `task.changed` events recorded so far, oldest first, with who made each. The ones the corpus
    /// records at startup, for the tasks the board already has, are not writes and are left out (ts-0116-wh).
    fn changed(&self) -> Vec<(String, ActorKind)> {
        self.daemon()
            .events_since(0)
            .unwrap()
            .into_iter()
            .filter_map(|event| match event.body {
                EventBody::TaskChanged { task, .. } if task.cause != ChangeCause::Startup => {
                    Some((task.key, event.actor.kind))
                }
                _ => None,
            })
            .collect()
    }

    fn wait_for_status(&self, id: &str, status: &str) {
        wait_until(&format!("{id} to be {status} on disk"), DEADLINE, || {
            self.status_on_disk(id) == status
        });
    }

    /// Waits until `count` changes are recorded: the Queue item goes when its write is taken, a moment before it lands.
    fn wait_for_changes(&self, count: usize) {
        wait_until(&format!("{count} recorded changes"), DEADLINE, || {
            self.changed().len() >= count
        });
    }

    fn wait_for_queue_to_empty(&self) {
        wait_until("the Queue to empty", DEADLINE, || self.queue().is_empty());
    }
}

fn queued(result: &TaskApplyResult) -> (String, String) {
    assert!(
        result.result.is_none(),
        "a queued write has no result: {result:?}"
    );
    let queued = result.queued.as_ref().expect("the write was queued");
    (queued.item.clone(), queued.reason.clone())
}

#[test]
fn a_write_to_a_free_checkout_is_applied_and_not_queued() {
    let rig = Rig::new();
    let answer = rig.set_status(SECOND, "active", None);
    assert!(
        matches!(answer.result, Some(ApplyResult::Ok { .. })),
        "{answer:?}"
    );
    assert!(answer.queued.is_none());
    assert_eq!(rig.status_on_disk(SECOND), "active");
    assert!(rig.queue().is_empty());
}

#[test]
fn a_write_during_a_rebase_is_queued_with_the_reason_and_applies_after_the_abort() {
    let rig = Rig::new();
    stop_in_a_rebase(&rig.repo);
    let before = rig.status_on_disk(SECOND);

    let answer = rig.set_status(SECOND, "active", None);

    let (item, reason) = queued(&answer);
    assert_eq!(
        reason,
        format!("a rebase is in progress in {}", rig.work().display())
    );
    assert_eq!(rig.status_on_disk(SECOND), before, "the file is unchanged");
    let items = rig.queue();
    assert_eq!(items.len(), 1, "{items:?}");
    assert_eq!(items[0].id, item);
    assert_eq!(items[0].kind, QueueKind::Conflict);
    assert_eq!(
        items[0].task.as_deref(),
        Some(format!("plan:{SECOND}").as_str())
    );
    assert!(items[0].title.contains(&reason), "{}", items[0].title);
    assert!(
        rig.changed().is_empty(),
        "no event for a write that was not made"
    );

    abort_the_rebase(&rig.repo);
    rig.wait_for_status(SECOND, "active");
    rig.wait_for_changes(1);
    rig.wait_for_queue_to_empty();
    assert_eq!(
        rig.changed(),
        [(format!("plan:{SECOND}"), ActorKind::Person)]
    );
}

#[test]
fn a_write_waits_for_each_of_the_other_states_too() {
    let rig = Rig::new();
    for (made, (name, words, is_dir)) in STATES.into_iter().enumerate() {
        let path = rig.repo.git_dir().join(name);
        if is_dir {
            fs::create_dir(&path).unwrap();
        } else {
            fs::write(&path, "x\n").unwrap();
        }
        let id = if name == "rebase-merge" {
            SECOND
        } else {
            TWELFTH
        };
        let answer = rig.set_status(id, "active", None);
        let (_, reason) = queued(&answer);
        assert!(reason.starts_with(words), "{name}: {reason}");
        if is_dir {
            fs::remove_dir(&path).unwrap();
        } else {
            fs::remove_file(&path).unwrap();
        }
        rig.wait_for_changes(made + 1);
        rig.wait_for_queue_to_empty();
        rig.wait_for_status(id, "active");
        // Put the task back for the next state, by the file, as a person would.
        let file = rig.repo.work().join(task_file(id));
        let text = fs::read_to_string(&file).unwrap();
        fs::write(&file, text.replacen("status: active", "status: ready", 1)).unwrap();
    }
}

#[test]
fn several_queued_writes_apply_in_the_order_they_were_made() {
    let rig = Rig::new();
    stop_in_a_rebase(&rig.repo);
    queued(&rig.set_status(TWELFTH, "active", None));
    queued(&rig.set_status(SECOND, "active", None));
    assert_eq!(rig.queue().len(), 2);

    abort_the_rebase(&rig.repo);
    rig.wait_for_changes(2);
    rig.wait_for_queue_to_empty();

    let keys: Vec<String> = rig.changed().into_iter().map(|(key, _)| key).collect();
    assert_eq!(keys, [format!("plan:{TWELFTH}"), format!("plan:{SECOND}")]);
}

#[test]
fn a_write_made_after_a_queued_one_waits_behind_it_when_the_checkout_frees() {
    let rig = Rig::new();
    stop_in_a_rebase(&rig.repo);
    queued(&rig.set_status(TWELFTH, "active", None));
    abort_the_rebase(&rig.repo);
    // The rebase is over and the writer has not looked yet, or has: either way the new write follows the queued one.
    let answer = rig.set_status(SECOND, "active", None);
    assert!(
        matches!(answer.result, Some(ApplyResult::Ok { .. })),
        "{answer:?}"
    );
    rig.wait_for_status(TWELFTH, "active");
    let keys: Vec<String> = rig.changed().into_iter().map(|(key, _)| key).collect();
    assert_eq!(keys, [format!("plan:{TWELFTH}"), format!("plan:{SECOND}")]);
}

#[test]
fn a_queued_write_survives_a_restart_and_applies_when_the_rebase_ends() {
    let mut rig = Rig::new();
    stop_in_a_rebase(&rig.repo);
    let before = rig.status_on_disk(SECOND);
    let (item, _) = queued(&rig.set_status(SECOND, "active", None));

    rig.stop();
    rig.start();

    let items = rig.queue();
    assert_eq!(items.len(), 1, "{items:?}");
    assert_eq!(items[0].id, item);
    assert_eq!(
        rig.status_on_disk(SECOND),
        before,
        "it did not apply while the rebase went on"
    );
    abort_the_rebase(&rig.repo);
    rig.wait_for_status(SECOND, "active");
    rig.wait_for_queue_to_empty();
}

#[test]
fn a_write_made_after_a_restart_does_not_take_the_id_of_a_write_that_waits() {
    let mut rig = Rig::new();
    stop_in_a_rebase(&rig.repo);
    let (first, _) = queued(&rig.set_status(SECOND, "active", None));
    rig.stop();
    rig.start();
    let (second, _) = queued(&rig.set_status(TWELFTH, "active", None));
    assert_ne!(first, second);
    assert_eq!(rig.queue().len(), 2);
}

#[test]
fn retry_applies_a_queued_write_that_can_go_now_and_says_when_it_cannot() {
    let rig = Rig::new();
    stop_in_a_rebase(&rig.repo);
    let (item, _) = queued(&rig.set_status(SECOND, "active", None));
    assert_eq!(
        rig.answer(&item, QueueAnswer::Retry {}),
        QueueAnswerResult::Queued { item: item.clone() }
    );
    abort_the_rebase(&rig.repo);
    // The writer may have looked first; a Retry for an item that is gone is not an item to find.
    wait_until("the write to apply", DEADLINE, || {
        rig.status_on_disk(SECOND) == "active" || {
            let _ = rig.answer(&item, QueueAnswer::Retry {});
            false
        }
    });
}

#[test]
fn dismiss_drops_a_queued_write_for_good() {
    let rig = Rig::new();
    stop_in_a_rebase(&rig.repo);
    let before = rig.status_on_disk(SECOND);
    let (item, _) = queued(&rig.set_status(SECOND, "active", None));

    assert_eq!(
        rig.answer(&item, QueueAnswer::Dismiss {}),
        QueueAnswerResult::Done {}
    );

    assert!(rig.queue().is_empty());
    abort_the_rebase(&rig.repo);
    // A later write to the same checkout drains first, so by the time it is applied nothing is left to apply.
    let after = rig.set_status(TWELFTH, "active", None);
    assert!(
        matches!(after.result, Some(ApplyResult::Ok { .. })),
        "{after:?}"
    );
    assert_eq!(rig.status_on_disk(SECOND), before);
}

// ---- a live phase -------------------------------------------------------------------------------------------------

#[test]
fn a_write_to_a_checkout_with_a_live_phase_is_queued_and_applies_when_the_phase_ends() {
    let rig = Rig::new();
    let work = rig.work();
    let live = rig.daemon().live_phases();
    live.started("R-7", &work, false);
    let before = rig.status_on_disk(SECOND);

    let answer = rig.set_status(SECOND, "active", None);

    let (_, reason) = queued(&answer);
    assert_eq!(
        reason,
        format!("a phase of run R-7 is running in {}", work.display())
    );
    assert_eq!(rig.status_on_disk(SECOND), before);
    live.ended("R-7");
    rig.wait_for_status(SECOND, "active");
    rig.wait_for_queue_to_empty();
}

#[test]
fn queued_writes_behind_a_phase_apply_in_order_when_it_ends() {
    let rig = Rig::new();
    let live = rig.daemon().live_phases();
    live.started("R-7", &rig.work(), false);
    queued(&rig.set_status(TWELFTH, "active", None));
    queued(&rig.set_status(SECOND, "active", None));

    live.ended("R-7");
    rig.wait_for_changes(2);
    rig.wait_for_queue_to_empty();

    let keys: Vec<String> = rig.changed().into_iter().map(|(key, _)| key).collect();
    assert_eq!(keys, [format!("plan:{TWELFTH}"), format!("plan:{SECOND}")]);
}

#[test]
fn a_phase_in_one_checkout_does_not_hold_back_a_write_to_another() {
    let rig = Rig::new();
    let other = rig.root().join("other");
    rig.repo.git(&[
        "worktree",
        "add",
        "--quiet",
        other.to_str().unwrap(),
        "-b",
        "other",
    ]);
    let other = fs::canonicalize(other).unwrap();
    rig.daemon().live_phases().started("R-7", &other, false);

    let answer = rig.set_status(SECOND, "active", None);

    assert!(
        matches!(answer.result, Some(ApplyResult::Ok { .. })),
        "{answer:?}"
    );
    assert!(rig.queue().is_empty());
}

#[test]
fn a_queued_write_whose_version_is_old_becomes_a_conflict_item_and_overwrites_nothing() {
    let rig = Rig::new();
    let live = rig.daemon().live_phases();
    live.started("R-7", &rig.work(), false);
    let old = rig.version(SECOND);
    let (write, _) = queued(&rig.set_status_at(SECOND, "active", &old, None));
    // Someone edits the file while the phase runs: the version the write was asked at is gone.
    let file = rig.repo.work().join(task_file(SECOND));
    let text = fs::read_to_string(&file).unwrap();
    fs::write(&file, text.replacen("status: ready", "status: review", 1)).unwrap();

    live.ended("R-7");

    wait_until("the stale write to become a conflict", DEADLINE, || {
        rig.queue().iter().any(|item| item.id != write)
    });
    let items = rig.queue();
    assert_eq!(items.len(), 1, "{items:?}");
    assert_eq!(items[0].kind, QueueKind::Conflict);
    assert_eq!(
        items[0].task.as_deref(),
        Some(format!("plan:{SECOND}").as_str())
    );
    assert!(
        items[0].title.contains("changed since"),
        "{}",
        items[0].title
    );
    assert!(
        items[0].title.contains("review"),
        "the current status: {}",
        items[0].title
    );
    assert_eq!(
        rig.status_on_disk(SECOND),
        "review",
        "the edit was not overwritten"
    );
    assert!(rig.changed().is_empty());
}

#[test]
fn a_phase_that_started_at_the_root_does_not_queue_a_write_there() {
    let rig = Rig::new();
    rig.daemon().live_phases().started("R-8", &rig.work(), true);

    let answer = rig.set_status(SECOND, "active", None);

    assert!(
        matches!(answer.result, Some(ApplyResult::Ok { .. })),
        "{answer:?}"
    );
    assert_eq!(rig.status_on_disk(SECOND), "active");
    assert!(rig.queue().is_empty());
}

#[test]
fn a_phase_that_started_at_the_root_still_waits_for_a_rebase_there() {
    let rig = Rig::new();
    rig.daemon().live_phases().started("R-8", &rig.work(), true);
    stop_in_a_rebase(&rig.repo);

    let (_, reason) = queued(&rig.set_status(SECOND, "active", None));

    assert!(reason.starts_with("a rebase is in progress"), "{reason}");
}

/// A run context as a daemon leaves it in the store while a phase is in use, with only what the daemon reads.
fn live_context(run: &str, workspace: &Path) -> String {
    json!({
        "run": run,
        "repo": workspace,
        "task_file": null,
        "alias": null,
        "budget_usd": null,
        "task_text": "a task",
        "model": null,
        "session_id": "session-1",
        "attempt": 0,
        "spent_usd": 0.0,
        "run_state": { "state": "running" },
        "workspace_state": "in_use",
        "workspace_path": workspace,
        "git_dir": null,
        "branch": "run-branch",
        "workspace_recorded": true,
    })
    .to_string()
}

#[test]
fn the_registry_of_live_phases_is_rebuilt_at_start_from_a_live_run_in_the_store() {
    let mut rig = Rig::new();
    let work = rig.work();
    rig.stop();
    Store::open(&DaemonPaths::under(rig.root()).store_dir)
        .unwrap()
        .set_run_context("R-5", &live_context("R-5", &work))
        .unwrap();
    rig.start();
    let before = rig.status_on_disk(SECOND);

    let (_, reason) = queued(&rig.set_status(SECOND, "active", None));

    assert_eq!(
        reason,
        format!("a phase of run R-5 is running in {}", work.display())
    );
    assert_eq!(rig.status_on_disk(SECOND), before);
}

#[test]
fn a_run_whose_workspace_is_idle_in_the_store_blocks_nothing_after_a_restart() {
    let mut rig = Rig::new();
    let work = rig.work();
    rig.stop();
    let idle = live_context("R-5", &work).replace("in_use", "idle");
    Store::open(&DaemonPaths::under(rig.root()).store_dir)
        .unwrap()
        .set_run_context("R-5", &idle)
        .unwrap();
    rig.start();

    let answer = rig.set_status(SECOND, "active", None);

    assert!(
        matches!(answer.result, Some(ApplyResult::Ok { .. })),
        "{answer:?}"
    );
}

// ---- a scratch write ----------------------------------------------------------------------------------------------

#[test]
fn a_scratch_write_is_never_queued_even_where_a_rebase_and_a_phase_hold_the_checkout() {
    let rig = Rig::new();
    stop_in_a_rebase(&rig.repo);
    rig.daemon()
        .live_phases()
        .started("R-7", &rig.work(), false);

    let answer: TaskApplyResult = rig.call(
        "task.apply",
        &TaskApplyParams {
            board: rig.board.clone(),
            source: Some("scratch".to_string()),
            apply: ApplyParams {
                actor: None,
                op: "create".to_string(),
                id: None,
                args: BTreeMap::from([("prompt".to_string(), json!("Write a haiku"))]),
                version: None,
                checkout: None,
            },
            commit: false,
        },
    );

    assert!(
        matches!(answer.result, Some(ApplyResult::Ok { .. })),
        "{answer:?}"
    );
    assert!(answer.queued.is_none());
    assert!(rig.queue().is_empty());
}

#[test]
fn a_queued_write_answer_has_no_result_on_the_wire() {
    let rig = Rig::new();
    stop_in_a_rebase(&rig.repo);
    let wire: Value = rig.call(
        "task.apply",
        &json!({
            "board": rig.board,
            "source": "plan",
            "apply": {
                "op": "setStatus", "id": SECOND, "args": { "status": "active" },
                "version": rig.version(SECOND)
            },
            "commit": false
        }),
    );
    assert!(wire.get("result").is_none(), "{wire}");
    assert!(wire["queued"]["item"].is_string(), "{wire}");
    assert!(
        wire["queued"]["reason"]
            .as_str()
            .unwrap()
            .starts_with("a rebase is in progress in "),
        "{wire}"
    );
}
