//! Permission answers, stop and a usage limit's pause through the daemon (ts-0066-zb): a request is held in the Queue and
//! answered over `queue.answer`, also after the daemon restarted; a stop writes `interrupt` before it closes stdin and
//! before it signals; a limit pauses the phase and resumes it by session id at the wall clock's reset time; a reset
//! that passed while no daemon ran resumes at the next start; a logout is a host shutdown.
//!
//! Each test has a daemon of its own under a temp root, built like `file_run.rs`'s (`env_clear()`, `HOME` in the root,
//! git without configuration), but a `Rig` can stop its daemon and start another on the same root. The clocks are real
//! time plus the offsets a test sets, and the limit timer's sleep is injected: it waits a millisecond and moves the
//! wall and continuous clocks by a step the test chooses, so no test waits out a pause. Every process a test starts is
//! reaped by `Rig`'s `Drop`.

use super::common;
use super::fake::{fake_claude_script, recording};
use super::file_run::{FakeFree, git};
use collide_core::agent::PermissionReply;
use collide_core::events::EventBody;
use collide_core::owner::{
    QueueAnswer, QueueAnswerParams, QueueAnswerResult, QueueItem, QueueKind, QueueListParams,
    QueueListResult, RunStartFileParams, RunStartResult, RunStopParams, RunStopResult,
};
use collide_core::state::{PauseCause, PhaseState, RunState};
use collide_host::base_env::CaptureOptions;
use collide_host::clock::{Clock, Clocks, Reading, SystemClocks};
use collide_host::daemon::{Daemon, DaemonOptions, DaemonPaths};
use collide_host::file_run::PHASE_NAME;
use collide_host::login::{End, LoginSessions};
use collide_host::owner_client::OwnerClient;
use collide_host::phases::{PhaseKey, PhaseOptions, Phases};
use collide_host::sysproc;
use common::procs::Guarded;
use common::{PYTHON, TempRoot, kill_child_of, signal_if_same, wait_until};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

/// The reset time of `recordings/limit.jsonl` (`resetsAt`), in seconds on the wall clock.
const RESET_SECS: u64 = 1_794_960_000;
const HOUR: Duration = Duration::from_secs(3600);

/// Real time plus the distance the injected sleep has moved the wall and the continuous clocks. The wall clock starts
/// `wall_base` after the epoch, so a test chooses whether the reset is ahead of it. The uptime clock is the real one:
/// a machine that sleeps does not move it.
pub(crate) struct TestClocks {
    pub(crate) real: SystemClocks,
    pub(crate) began: Instant,
    pub(crate) wall_base: Duration,
    pub(crate) moved: Mutex<Duration>,
    /// Time that passed with the machine awake: it moves all three clocks.
    pub(crate) awake: Mutex<Duration>,
    pub(crate) step: Mutex<Duration>,
    /// The injected sleeps that moved the clocks (a sleep while the step is zero only waits, so these are the sleeps a test asked for).
    pub(crate) moving_sleeps: AtomicUsize,
}

impl TestClocks {
    pub(crate) fn new(wall_base_secs: u64) -> Arc<TestClocks> {
        Arc::new(TestClocks {
            real: SystemClocks,
            began: Instant::now(),
            wall_base: Duration::from_secs(wall_base_secs),
            moved: Mutex::new(Duration::ZERO),
            awake: Mutex::new(Duration::ZERO),
            step: Mutex::new(Duration::ZERO),
            moving_sleeps: AtomicUsize::new(0),
        })
    }

    /// How far each injected sleep moves the clocks.
    pub(crate) fn set_step(&self, step: Duration) {
        *self.step.lock().unwrap() = step;
    }

    /// `gap` passed with the machine awake, while no code ran.
    pub(crate) fn advance(&self, gap: Duration) {
        *self.awake.lock().unwrap() += gap;
    }

    /// The machine slept for `gap` while no code ran.
    pub(crate) fn jump(&self, gap: Duration) {
        *self.moved.lock().unwrap() += gap;
    }

    pub(crate) fn sleeps(&self) -> usize {
        self.moving_sleeps.load(Ordering::SeqCst)
    }

    pub(crate) fn sleep(&self) {
        thread::sleep(Duration::from_millis(1));
        let step = *self.step.lock().unwrap();
        if !step.is_zero() {
            self.moving_sleeps.fetch_add(1, Ordering::SeqCst);
        }
        self.jump(step);
    }
}

pub(crate) fn nanos(duration: Duration) -> u64 {
    u64::try_from(duration.as_nanos()).unwrap()
}

impl Clocks for TestClocks {
    fn read(&self, clock: Clock) -> Reading {
        let moved = *self.moved.lock().unwrap();
        let awake = *self.awake.lock().unwrap();
        let nanos = match clock {
            Clock::Uptime => self.real.read(Clock::Uptime).nanos + nanos(awake),
            Clock::Continuous => self.real.read(Clock::Continuous).nanos + nanos(moved + awake),
            Clock::Wall => nanos(self.wall_base + self.began.elapsed() + moved + awake),
        };
        Reading { clock, nanos }
    }
}

/// The login session as a test sets it: it can be told that the user is logging out.
pub(crate) struct TestLogin {
    leaving: AtomicBool,
}

impl LoginSessions for TestLogin {
    fn current(&self) -> Option<String> {
        Some("session-1".to_string())
    }

    fn logging_out(&self) -> bool {
        self.leaving.load(Ordering::SeqCst)
    }
}

pub(crate) struct Settings {
    /// Where the wall clock starts, in seconds: before the reset by default.
    pub(crate) wall_base_secs: u64,
    pub(crate) min_pause: Duration,
    pub(crate) free: Option<Arc<FakeFree>>,
    pub(crate) interrupt_wait: Duration,
    pub(crate) close_wait: Duration,
    /// How often the daemon's sampler reads the clocks.
    pub(crate) sampler_tick: Duration,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            wall_base_secs: RESET_SECS - 3600,
            min_pause: Duration::from_secs(30),
            free: None,
            interrupt_wait: Duration::from_millis(1500),
            close_wait: Duration::from_millis(1500),
            sampler_tick: Duration::from_millis(20),
        }
    }
}

pub(crate) struct Rig {
    pub(crate) root: TempRoot,
    pub(crate) daemon: Option<Daemon>,
    pub(crate) phases: Phases,
    pub(crate) client: Option<OwnerClient>,
    pub(crate) repo: PathBuf,
    pub(crate) task: PathBuf,
    pub(crate) shell: PathBuf,
    pub(crate) argv_log: PathBuf,
    pub(crate) clocks: Arc<TestClocks>,
    pub(crate) login: Arc<TestLogin>,
    pub(crate) settings: Settings,
}

impl Rig {
    /// A daemon whose `claude` plays `first`, or `second` when it is started with `--resume`.
    pub(crate) fn new(first: &Path, second: &Path, settings: Settings) -> Rig {
        let root = TempRoot::new();
        let bin = root.join("bin");
        fs::create_dir(&bin).unwrap();
        let argv_log = root.join("argv.log");
        let wrapper = format!(
            "#!/bin/sh\nif [ \"$*\" = \"--version\" ]; then echo \"2.1.325 (Claude Code)\"; exit 0; fi\necho \"$*\" >> {log}\ncase \" $* \" in\n  *\" --resume \"*) rec={second} ;;\n  *) rec={first} ;;\nesac\nexec {PYTHON} {script} --emulate-hooks \"$rec\" \"$@\"\n",
            log = argv_log.display(),
            first = first.display(),
            second = second.display(),
            script = fake_claude_script().display(),
        );
        let wrapper_path = bin.join("claude");
        fs::write(&wrapper_path, wrapper).unwrap();
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&wrapper_path, fs::Permissions::from_mode(0o755)).unwrap();
        }
        let shell = root.script(
            "fake-shell",
            &format!(
                "#!/bin/sh\nexport PATH={}:/usr/bin:/bin\neval \"$4\"\n",
                bin.display()
            ),
        );
        let home = root.join("home");
        fs::create_dir_all(home.join(".config/collide")).unwrap();
        fs::write(
            home.join(".config/collide/models.yaml"),
            "tools:\n  claude:\n    fast: { name: haiku }\n    strong: { name: opus }\n",
        )
        .unwrap();
        let repo = root.join("repo");
        fs::create_dir(&repo).unwrap();
        git(&repo, &["init", "-q", "-b", "main"]);
        fs::write(repo.join("README"), "hello\n").unwrap();
        git(&repo, &["add", "."]);
        git(&repo, &["commit", "-q", "-m", "start"]);
        let task = root.join("task.md");
        fs::write(&task, "Add a line to README.\n").unwrap();

        let clocks = TestClocks::new(settings.wall_base_secs);
        let login = Arc::new(TestLogin {
            leaving: AtomicBool::new(false),
        });
        let (daemon, phases, client) = start_daemon(&root, &shell, &clocks, &login, &settings);
        Rig {
            repo: fs::canonicalize(repo).unwrap(),
            root,
            daemon: Some(daemon),
            phases,
            client: Some(client),
            task,
            shell,
            argv_log,
            clocks,
            login,
            settings,
        }
    }

    /// Stops the daemon. Its shims and their children go on, as they do when the real daemon ends.
    pub(crate) fn stop_daemon(&mut self) {
        self.client = None;
        drop(self.daemon.take());
    }

    pub(crate) fn start_daemon(&mut self) {
        let (daemon, phases, client) = start_daemon(
            &self.root,
            &self.shell,
            &self.clocks,
            &self.login,
            &self.settings,
        );
        self.daemon = Some(daemon);
        self.phases = phases;
        self.client = Some(client);
    }

    pub(crate) fn restart(&mut self) {
        self.stop_daemon();
        self.start_daemon();
    }

    pub(crate) fn call<P: Serialize, R: DeserializeOwned>(
        &mut self,
        method: &str,
        params: &P,
    ) -> R {
        self.client
            .as_mut()
            .expect("a daemon runs")
            .call(method, params, Some(Duration::from_secs(60)))
            .unwrap_or_else(|error| panic!("{method}: {error:?}"))
    }

    pub(crate) fn started(&mut self, budget_usd: Option<f64>) -> String {
        let params = RunStartFileParams {
            repo: self.repo.to_str().unwrap().to_string(),
            task_file: self.task.to_str().unwrap().to_string(),
            alias: None,
            budget_usd,
            config: None,
        };
        match self.call("run.start_file", &params) {
            RunStartResult::Started { run } => run,
            RunStartResult::Refused { problems } => panic!("refused: {problems:?}"),
        }
    }

    pub(crate) fn key(run: &str) -> PhaseKey {
        PhaseKey::new(run, PHASE_NAME).unwrap()
    }

    pub(crate) fn wait_final(&self, run: &str) -> PhaseState {
        let key = Rig::key(run);
        wait_until("the phase is final", Duration::from_secs(30), || {
            self.phases.state(&key).is_some_and(|state| {
                matches!(
                    state,
                    PhaseState::Ended {}
                        | PhaseState::Failed { .. }
                        | PhaseState::Stopped {}
                        | PhaseState::Lost {}
                )
            })
        });
        self.phases.state(&key).unwrap()
    }

    pub(crate) fn wait_run(&self, run: &str, want: impl Fn(&RunState) -> bool) -> RunState {
        wait_until("the run reaches its state", Duration::from_secs(30), || {
            self.phases.run_state(run).is_some_and(|state| want(&state))
        });
        self.phases.run_state(run).unwrap()
    }

    pub(crate) fn wait_running(&self, run: &str) {
        let key = Rig::key(run);
        wait_until("the phase runs", Duration::from_secs(30), || {
            self.phases.state(&key) == Some(PhaseState::Running {})
        });
    }

    pub(crate) fn queue(&mut self, run: &str) -> Vec<QueueItem> {
        let result: QueueListResult = self.call(
            "queue.list",
            &QueueListParams {
                board: None,
                run: Some(run.to_string()),
                kinds: vec![],
            },
        );
        result.items
    }

    pub(crate) fn wait_item(&mut self, run: &str, kind: QueueKind) -> QueueItem {
        let until = Instant::now() + Duration::from_secs(30);
        loop {
            if let Some(item) = self.queue(run).into_iter().find(|item| item.kind == kind) {
                return item;
            }
            assert!(Instant::now() < until, "no {kind:?} item for {run}");
            thread::sleep(Duration::from_millis(20));
        }
    }

    pub(crate) fn answer(&mut self, item: &QueueItem, answer: QueueAnswer) -> QueueAnswerResult {
        self.call(
            "queue.answer",
            &QueueAnswerParams {
                item: item.id.clone(),
                answer,
            },
        )
    }

    pub(crate) fn stop(&mut self, run: &str) -> RunStopResult {
        self.call(
            "run.stop",
            &RunStopParams {
                run: run.to_string(),
                grace_secs: Some(1),
            },
        )
    }

    pub(crate) fn events(&self) -> Vec<EventBody> {
        self.daemon
            .as_ref()
            .unwrap()
            .events_since(0)
            .unwrap()
            .into_iter()
            .map(|event| event.body)
            .collect()
    }

    /// The runs the event log has seen, in the order they began.
    pub(crate) fn run_ids(&self) -> Vec<String> {
        let mut runs = Vec::new();
        for body in self.events() {
            if let EventBody::RunTransition { run, .. } = body
                && !runs.contains(&run)
            {
                runs.push(run);
            }
        }
        runs
    }

    /// The command line of each launch of `claude`, in order.
    pub(crate) fn launches(&self) -> Vec<Vec<String>> {
        fs::read_to_string(&self.argv_log)
            .unwrap_or_default()
            .lines()
            .map(|line| line.split_whitespace().map(str::to_string).collect())
            .collect()
    }

    pub(crate) fn workspace_dir(&self, run: &str) -> PathBuf {
        self.root.join("data/workspaces").join(run)
    }
}

pub(crate) fn start_daemon(
    root: &TempRoot,
    shell: &Path,
    clocks: &Arc<TestClocks>,
    login: &Arc<TestLogin>,
    settings: &Settings,
) -> (Daemon, Phases, OwnerClient) {
    let paths = DaemonPaths::under(root.path());
    let mut options = DaemonOptions::new(
        paths.clone(),
        shell.to_str().unwrap().to_string(),
        Box::new(io::sink()),
    );
    options.process_tree = super::common::test_process_tree();
    options.capture = CaptureOptions {
        timeout: Duration::from_secs(10),
        cap_bytes: 1024 * 1024,
        fallback: BTreeMap::from([("PATH".to_string(), "/usr/bin:/bin".to_string())]),
    };
    options.shim_exe = Some(PathBuf::from(env!("CARGO_BIN_EXE_collide")));
    options.phases = PhaseOptions {
        reply_wait: Duration::from_millis(1500),
        unresponsive_after: Duration::from_millis(800),
        interrupt_wait: settings.interrupt_wait,
        close_wait: settings.close_wait,
        ..PhaseOptions::default()
    };
    options.processes = Arc::new(Guarded::under(paths.runs_dir.clone(), Vec::new()));
    options.home = Some(root.join("home").to_str().unwrap().to_string());
    options.git_env = vec![
        ("GIT_CONFIG_GLOBAL".to_string(), "/dev/null".to_string()),
        ("GIT_CONFIG_SYSTEM".to_string(), "/dev/null".to_string()),
        ("GIT_CONFIG_NOSYSTEM".to_string(), "1".to_string()),
    ];
    if let Some(free) = &settings.free {
        options.free_space = free.clone();
    }
    options.clocks = clocks.clone();
    options.login = login.clone();
    let sleeper = clocks.clone();
    options.sleep = Arc::new(move |_| sleeper.sleep());
    options.limit_poll = Duration::from_secs(1);
    options.limit_min_pause = settings.min_pause;
    options.sampler_tick = settings.sampler_tick;
    let daemon = Daemon::start(options).expect("the daemon starts");
    let phases = daemon.phases();
    let client = OwnerClient::connect(daemon.paths(), "pause-test").expect("connects");
    (daemon, phases, client)
}

impl Drop for Rig {
    fn drop(&mut self) {
        // The daemon stops its watchers; the shims go on, so each phase's shim and child are stopped by identity.
        let runs = DaemonPaths::under(self.root.path()).runs_dir;
        if let Ok(entries) = fs::read_dir(&runs) {
            for run in entries.flatten() {
                let phase_dir = run.path().join(PHASE_NAME);
                if let Ok(text) = fs::read_to_string(phase_dir.join("shim.json"))
                    && let Ok(value) = serde_json::from_str::<Value>(&text)
                    && let Some(identity) = collide_host::shim::ChildIdentity::from_json(&value)
                {
                    signal_if_same(identity.pid, identity.started_unix_us, libc::SIGKILL);
                }
                kill_child_of(&phase_dir);
            }
        }
        self.client = None;
        drop(self.daemon.take());
    }
}

// ---- recordings ---------------------------------------------------------------------------------------------------

/// A copy of the recording `name` in `root`, with `edit` applied to each of its lines (a line `edit` returns `None`
/// for is dropped).
pub(crate) fn edited(
    root: &TempRoot,
    name: &str,
    edit: impl Fn(&str) -> Option<String>,
) -> PathBuf {
    let text = fs::read_to_string(recording(name)).unwrap();
    let lines: Vec<String> = text.lines().filter_map(edit).collect();
    let path = root.join(&format!("{name}-edited.jsonl"));
    fs::write(&path, lines.join("\n")).unwrap();
    path
}

/// A recording that is the lines `lines`, one step each.
pub(crate) fn written(root: &TempRoot, name: &str, lines: &[Value]) -> PathBuf {
    let path = root.join(name);
    let text: Vec<String> = lines.iter().map(Value::to_string).collect();
    fs::write(&path, text.join("\n")).unwrap();
    path
}

/// The lines of `recordings/limit.jsonl` that set the scene: the turn, the init, the stream before the limit.
pub(crate) fn said_line(recording_name: &str, ty: &str) -> Value {
    let text = fs::read_to_string(recording(recording_name)).unwrap();
    text.lines()
        .filter(|line| line.starts_with("{\"say\""))
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .find(|step| step["say"]["type"] == ty)
        .unwrap_or_else(|| panic!("{recording_name} has no {ty} line"))
}

// ---- permission requests ------------------------------------------------------------------------------------------

pub(crate) fn permission_item(rig: &mut Rig, run: &str) -> QueueItem {
    let item = rig.wait_item(run, QueueKind::Permission);
    let request = item
        .permission
        .as_ref()
        .expect("the item carries the request");
    assert_eq!(request.id, "req-01");
    assert_eq!(request.tool.as_deref(), Some("Bash"));
    item
}

#[test]
fn a_permission_request_waits_in_the_queue_and_an_allow_over_rpc_answers_it() {
    let mut rig = Rig::new(
        &recording("permission-request"),
        &recording("report"),
        Settings::default(),
    );
    let run = rig.started(None);
    let item = permission_item(&mut rig, &run);
    assert_eq!(item.run.as_deref(), Some(run.as_str()));
    assert_eq!(item.phase.as_deref(), Some(PHASE_NAME));
    assert_eq!(item.answers, [collide_core::owner::AnswerKind::Permission]);
    assert_eq!(
        rig.phases.state(&Rig::key(&run)),
        Some(PhaseState::Running {}),
        "the phase waits, it is not paused"
    );

    let result = rig.answer(
        &item,
        QueueAnswer::Permission {
            reply: PermissionReply::Allow {},
        },
    );
    assert_eq!(result, QueueAnswerResult::Done {});
    assert_eq!(rig.wait_final(&run), PhaseState::Ended {});
    assert!(rig.queue(&run).is_empty());
    let events = rig.events();
    assert!(events.iter().any(|body| matches!(
        body,
        EventBody::PermissionRequested { request, tool, .. } if request == "req-01" && tool == "Bash"
    )));
    assert!(events.iter().any(
        |body| matches!(body, EventBody::PermissionAnswered { request, .. } if request == "req-01")
    ));
}

#[test]
fn a_deny_over_rpc_reaches_claude_with_its_message() {
    let root = TempRoot::new();
    let denied = edited(&root, "permission-request", |line| {
        if line.starts_with("{\"expect\": {\"type\": \"control_response\"")
            || line.starts_with("{\"expect\":{\"type\":\"control_response\"")
        {
            Some(
                json!({"expect": {"type": "control_response", "response": {"subtype": "success",
                    "request_id": "req-01", "response": {"behavior": "deny", "message": "Not now."}}}})
                .to_string(),
            )
        } else {
            Some(line.to_string())
        }
    });
    let mut rig = Rig::new(&denied, &recording("report"), Settings::default());
    let run = rig.started(None);
    let item = permission_item(&mut rig, &run);
    let result = rig.answer(
        &item,
        QueueAnswer::Permission {
            reply: PermissionReply::Deny {
                message: Some("Not now.".to_string()),
            },
        },
    );
    assert_eq!(result, QueueAnswerResult::Done {});
    // The fake exits 97 on an answer that does not match, so a phase that ended Ended saw the deny and its message.
    assert_eq!(rig.wait_final(&run), PhaseState::Ended {});
}

#[test]
fn an_answer_of_the_wrong_kind_is_refused_and_the_request_stays() {
    let mut rig = Rig::new(
        &recording("permission-request"),
        &recording("report"),
        Settings::default(),
    );
    let run = rig.started(None);
    let item = permission_item(&mut rig, &run);
    let error = rig
        .client
        .as_mut()
        .unwrap()
        .call::<_, QueueAnswerResult>(
            "queue.answer",
            &QueueAnswerParams {
                item: item.id.clone(),
                answer: QueueAnswer::Resume {},
            },
            Some(Duration::from_secs(10)),
        )
        .expect_err("an item that takes a permission answer takes no resume");
    assert!(format!("{error:?}").contains("takes"), "{error:?}");
    assert_eq!(rig.queue(&run).len(), 1);
    rig.answer(
        &item,
        QueueAnswer::Permission {
            reply: PermissionReply::Allow {},
        },
    );
    assert_eq!(rig.wait_final(&run), PhaseState::Ended {});
}

#[test]
fn a_pending_request_survives_a_daemon_restart_and_can_be_answered() {
    let mut rig = Rig::new(
        &recording("permission-request"),
        &recording("report"),
        Settings::default(),
    );
    let run = rig.started(None);
    let before = permission_item(&mut rig, &run);

    rig.restart();

    let after = permission_item(&mut rig, &run);
    assert_eq!(after.id, before.id);
    assert_eq!(
        rig.wait_run(&run, |state| matches!(state, RunState::Running {})),
        RunState::Running {}
    );
    let result = rig.answer(
        &after,
        QueueAnswer::Permission {
            reply: PermissionReply::Allow {},
        },
    );
    assert_eq!(result, QueueAnswerResult::Done {});
    assert_eq!(rig.wait_final(&run), PhaseState::Ended {});
    assert!(rig.queue(&run).is_empty());
}

#[test]
fn a_request_that_was_answered_is_not_asked_again_after_a_restart() {
    let mut rig = Rig::new(
        &recording("permission-request"),
        &recording("report"),
        Settings::default(),
    );
    let run = rig.started(None);
    let item = permission_item(&mut rig, &run);
    rig.answer(
        &item,
        QueueAnswer::Permission {
            reply: PermissionReply::Allow {},
        },
    );
    assert_eq!(rig.wait_final(&run), PhaseState::Ended {});
    rig.restart();
    assert!(rig.queue(&run).is_empty());
}

// ---- stop ---------------------------------------------------------------------------------------------------------

#[test]
fn a_child_that_ends_its_turn_on_interrupt_ends_stopped_with_no_signal_sent() {
    let mut rig = Rig::new(
        &recording("cancelled"),
        &recording("cancelled"),
        Settings::default(),
    );
    let began_us = real_now_us();
    let run = rig.started(None);
    rig.wait_running(&run);
    let stopped = rig.stop(&run);
    assert_eq!(stopped.state, RunState::Stopped {});
    assert_eq!(stopped.signalled, 0);
    assert_eq!(rig.wait_final(&run), PhaseState::Stopped {});
    // The rig's wall clock starts at a fixed time (`wall_base_secs`), which real time has passed, so the stop's scan
    // takes any orphan of this user's that started since then for a suspect: another program's, too. Only one that
    // started during this test can be the run's.
    let survivors: Vec<_> = stopped
        .survivors
        .iter()
        .filter(|s| {
            !s.command.ends_with("(suspect)")
                || sysproc::start_time_us(s.pid).is_none_or(|started| started >= began_us)
        })
        .collect();
    assert!(survivors.is_empty(), "{survivors:?}");
}

/// Real time now, in microseconds since the epoch: the clock the operating system dates a process by.
fn real_now_us() -> u64 {
    let since = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("the clock is after the epoch");
    u64::try_from(since.as_micros()).expect("microseconds fit in u64")
}

#[test]
fn a_child_that_ignores_interrupt_gets_stdin_closed_before_it_is_signalled() {
    let root = TempRoot::new();
    let seen_eof = root.join("saw-eof");
    // It does not read stdin until `wait_eof`, so the interrupt is not answered; it marks the end of stdin, and holds on.
    let stubborn = written(
        &root,
        "stubborn.jsonl",
        &[
            json!({"expect": {"type": "user"}}),
            said_line("report", "system"),
            json!({"wait_eof": true}),
            json!({"touch": seen_eof}),
            json!({"sleep": 60}),
        ],
    );
    let mut rig = Rig::new(
        &stubborn,
        &stubborn,
        Settings {
            interrupt_wait: Duration::from_millis(300),
            close_wait: Duration::from_millis(1500),
            ..Settings::default()
        },
    );
    let run = rig.started(None);
    rig.wait_running(&run);
    assert!(!seen_eof.exists());
    let stopped = rig.stop(&run);
    assert_eq!(stopped.state, RunState::Stopped {});
    assert!(
        seen_eof.exists(),
        "stdin was closed while the child could still act, before any signal"
    );
    assert!(stopped.signalled >= 1, "a child that holds on is signalled");
    assert_eq!(rig.wait_final(&run), PhaseState::Stopped {});
}

// ---- a usage limit -------------------------------------------------------------------------------------------------

fn reset_ms() -> u64 {
    RESET_SECS * 1000
}

fn paused_for_limit(rig: &mut Rig, run: &str) -> QueueItem {
    let item = rig.wait_item(run, QueueKind::UsageLimit);
    assert_eq!(
        rig.wait_run(run, |state| matches!(state, RunState::Paused { .. })),
        RunState::Paused {
            cause: PauseCause::UsageLimit
        }
    );
    item
}

fn value_after<'a>(argv: &'a [String], flag: &str) -> &'a str {
    let at = argv
        .iter()
        .position(|word| word == flag)
        .unwrap_or_else(|| panic!("no {flag} in {argv:?}"));
    &argv[at + 1]
}

#[test]
fn a_limit_pauses_the_phase_and_resumes_it_by_session_id_at_the_wall_clock_reset() {
    let mut rig = Rig::new(
        &recording("limit"),
        &recording("report"),
        Settings::default(),
    );
    let run = rig.started(Some(1.0));
    let item = paused_for_limit(&mut rig, &run);
    assert_eq!(item.reset_ms, Some(reset_ms()));
    assert_eq!(
        item.answers,
        [
            collide_core::owner::AnswerKind::Resume,
            collide_core::owner::AnswerKind::Stop
        ]
    );
    assert!(matches!(
        rig.phases.state(&Rig::key(&run)),
        Some(PhaseState::Paused {
            cause: PauseCause::UsageLimit
        })
    ));
    assert_eq!(rig.launches().len(), 1, "nothing resumes before the reset");

    // The wall clock is an hour before the reset: each sleep moves it ten minutes.
    rig.clocks.set_step(Duration::from_secs(600));
    assert_eq!(rig.wait_final(&run), PhaseState::Ended {});
    assert!(rig.clocks.sleeps() >= 6, "{} sleeps", rig.clocks.sleeps());

    let launches = rig.launches();
    assert_eq!(launches.len(), 2);
    let session = value_after(&launches[0], "--session-id").to_string();
    assert_eq!(value_after(&launches[1], "--resume"), session);
    assert!(!launches[1].contains(&"--session-id".to_string()));
    let first: f64 = value_after(&launches[0], "--max-budget-usd")
        .parse()
        .unwrap();
    let second: f64 = value_after(&launches[1], "--max-budget-usd")
        .parse()
        .unwrap();
    assert_eq!(first, 1.0);
    // The limit recording's session had spent 0.0041.
    assert!(
        (second - (1.0 - 0.0041)).abs() < 1e-9,
        "budget left {second}"
    );
    assert!(rig.queue(&run).is_empty());
    assert_eq!(
        rig.wait_run(&run, |state| matches!(state, RunState::Ended { .. })),
        rig.phases.run_state(&run).unwrap()
    );
}

#[test]
fn a_reset_that_passed_during_a_sleep_resumes_on_the_first_wake_without_a_countdown() {
    let mut rig = Rig::new(
        &recording("limit"),
        &recording("report"),
        Settings::default(),
    );
    let run = rig.started(None);
    paused_for_limit(&mut rig, &run);

    // The machine sleeps for two hours inside the next injected sleep: the wall and the continuous clocks pass the
    // reset, the uptime clock does not move.
    rig.clocks.set_step(2 * HOUR);
    assert_eq!(rig.wait_final(&run), PhaseState::Ended {});
    assert!(
        rig.clocks.sleeps() <= 2,
        "{} sleeps after the pause",
        rig.clocks.sleeps()
    );
    assert_eq!(rig.launches().len(), 2);
}

#[test]
fn a_reset_that_is_already_past_still_waits_out_the_least_pause() {
    // The wall clock starts an hour after the reset (a stale reset time); the least pause is thirty minutes on the
    // continuous clock, and each sleep moves ten.
    let mut rig = Rig::new(
        &recording("limit"),
        &recording("report"),
        Settings {
            wall_base_secs: RESET_SECS + 3600,
            min_pause: Duration::from_secs(1800),
            ..Settings::default()
        },
    );
    let run = rig.started(None);
    paused_for_limit(&mut rig, &run);
    assert_eq!(rig.launches().len(), 1);
    rig.clocks.set_step(Duration::from_secs(600));
    assert_eq!(rig.wait_final(&run), PhaseState::Ended {});
    assert!(rig.clocks.sleeps() >= 3, "{} sleeps", rig.clocks.sleeps());
}

#[test]
fn a_resume_answer_resumes_a_limited_run_before_its_reset() {
    let mut rig = Rig::new(
        &recording("limit"),
        &recording("report"),
        Settings::default(),
    );
    let run = rig.started(None);
    let item = paused_for_limit(&mut rig, &run);
    let result = rig.answer(&item, QueueAnswer::Resume {});
    assert_eq!(result, QueueAnswerResult::Done {});
    assert_eq!(rig.wait_final(&run), PhaseState::Ended {});
    assert_eq!(rig.launches().len(), 2);
    assert!(rig.queue(&run).is_empty());
}

#[test]
fn a_stop_answer_stops_a_limited_run_and_nothing_resumes_it() {
    let mut rig = Rig::new(
        &recording("limit"),
        &recording("report"),
        Settings::default(),
    );
    let run = rig.started(None);
    let item = paused_for_limit(&mut rig, &run);
    let result = rig.answer(&item, QueueAnswer::Stop {});
    assert_eq!(result, QueueAnswerResult::Done {});
    rig.wait_run(&run, |state| matches!(state, RunState::Stopped {}));
    rig.clocks.set_step(2 * HOUR);
    thread::sleep(Duration::from_millis(200));
    assert_eq!(rig.launches().len(), 1);
    assert!(rig.queue(&run).is_empty());
}

#[test]
fn run_stop_reaches_a_limited_run() {
    let mut rig = Rig::new(
        &recording("limit"),
        &recording("report"),
        Settings::default(),
    );
    let run = rig.started(None);
    paused_for_limit(&mut rig, &run);
    let stopped = rig.stop(&run);
    assert_eq!(stopped.state, RunState::Stopped {});
    assert!(rig.queue(&run).is_empty());
    rig.clocks.set_step(2 * HOUR);
    thread::sleep(Duration::from_millis(200));
    assert_eq!(rig.launches().len(), 1, "a stopped run is not resumed");
}

#[test]
fn a_limit_pause_survives_a_restart_and_resumes_at_the_reset() {
    let mut rig = Rig::new(
        &recording("limit"),
        &recording("report"),
        Settings::default(),
    );
    let run = rig.started(None);
    let before = paused_for_limit(&mut rig, &run);
    rig.restart();
    let after = rig.wait_item(&run, QueueKind::UsageLimit);
    assert_eq!(after, before);
    assert_eq!(
        rig.wait_run(&run, |state| matches!(state, RunState::Paused { .. })),
        RunState::Paused {
            cause: PauseCause::UsageLimit
        }
    );
    rig.clocks.set_step(2 * HOUR);
    assert_eq!(rig.wait_final(&run), PhaseState::Ended {});
}

#[test]
fn a_reset_that_passed_while_no_daemon_ran_is_resumed_by_the_next_daemon() {
    let mut rig = Rig::new(
        &recording("limit"),
        &recording("report"),
        Settings::default(),
    );
    let run = rig.started(Some(1.0));
    paused_for_limit(&mut rig, &run);
    rig.stop_daemon();
    // The reset comes and goes while nothing runs.
    rig.clocks.jump(2 * HOUR);
    rig.start_daemon();
    assert_eq!(rig.wait_final(&run), PhaseState::Ended {});
    let launches = rig.launches();
    assert_eq!(launches.len(), 2);
    assert_eq!(
        value_after(&launches[1], "--resume"),
        value_after(&launches[0], "--session-id")
    );
    // A restarted daemon carries on with the budget left, which it read from the stored run.
    let second: f64 = value_after(&launches[1], "--max-budget-usd")
        .parse()
        .unwrap();
    assert!(
        (second - (1.0 - 0.0041)).abs() < 1e-9,
        "budget left {second}"
    );
    assert!(rig.queue(&run).is_empty());
}

// ---- an error that is not a limit ----------------------------------------------------------------------------------

#[test]
fn another_api_status_fails_the_phase_instead_of_pausing_it() {
    let root = TempRoot::new();
    let server_error = edited(&root, "limit", |line| {
        if line.contains("rate_limit_event") {
            None
        } else {
            Some(
                line.replace("\"api_error_status\":429", "\"api_error_status\":500")
                    .replace("\"api_error_status\": 429", "\"api_error_status\": 500")
                    .replace("\"error\":\"rate_limit\"", "\"error\":\"unknown\"")
                    .replace("\"error\": \"rate_limit\"", "\"error\": \"unknown\""),
            )
        }
    });
    let mut rig = Rig::new(&server_error, &server_error, Settings::default());
    let run = rig.started(None);
    assert!(matches!(rig.wait_final(&run), PhaseState::Failed { .. }));
    rig.wait_run(&run, |state| matches!(state, RunState::Failed {}));
    let store =
        collide_host::store::Store::open(&DaemonPaths::under(rig.root.path()).store_dir).unwrap();
    let record = store.phase_record(&run, PHASE_NAME).unwrap().unwrap();
    assert_eq!(record.failure.as_deref(), Some("api_500"));
    assert!(rig.queue(&run).is_empty());
    assert_eq!(rig.launches().len(), 1);
}

#[test]
fn a_429_with_no_limited_event_pauses_the_phase_too() {
    let root = TempRoot::new();
    let bare = edited(&root, "limit", |line| {
        (!line.contains("rate_limit_event")).then(|| line.to_string())
    });
    let mut rig = Rig::new(&bare, &recording("report"), Settings::default());
    let run = rig.started(None);
    let item = paused_for_limit(&mut rig, &run);
    assert_eq!(item.reset_ms, None, "no event, no reset time");
}

// ---- the budget ------------------------------------------------------------------------------------------------

#[test]
fn a_budget_error_pauses_the_phase_for_a_person_and_its_cost_counts() {
    let mut rig = Rig::new(
        &recording("error-result"),
        &recording("report"),
        Settings::default(),
    );
    let run = rig.started(Some(0.0001));
    let item = rig.wait_item(&run, QueueKind::BudgetExceeded);
    assert_eq!(item.answers, [collide_core::owner::AnswerKind::Stop]);
    assert_eq!(
        rig.wait_run(&run, |state| matches!(state, RunState::Paused { .. })),
        RunState::Paused {
            cause: PauseCause::AskHuman
        }
    );
    let events = rig.events();
    assert!(events.iter().any(|body| matches!(
        body,
        EventBody::BudgetExceeded { run: r, .. } if *r == run
    )));
    assert!(events.iter().any(|body| matches!(
        body,
        EventBody::AskHumanAsked { run: r, cause, queue_item } if *r == run && cause == "budget" && *queue_item == item.id
    )));
    // The failed turn's cost is the run's spend: it is in the stored context the next start reads.
    let store =
        collide_host::store::Store::open(&DaemonPaths::under(rig.root.path()).store_dir).unwrap();
    let contexts = store.run_contexts().unwrap();
    let (_, context) = contexts
        .iter()
        .find(|(r, _)| *r == run)
        .expect("a stored context");
    let context: Value = serde_json::from_str(context).unwrap();
    assert!(
        (context["spent_usd"].as_f64().unwrap() - 0.00353).abs() < 1e-9,
        "{context}"
    );

    let result = rig.answer(&item, QueueAnswer::Stop {});
    assert_eq!(result, QueueAnswerResult::Done {});
    rig.wait_run(&run, |state| matches!(state, RunState::Stopped {}));
    assert!(rig.queue(&run).is_empty());
}

// ---- a run paused for disk -------------------------------------------------------------------------------------

const FLOOR_HIGH: u64 = u64::MAX;
const FLOOR_LOW: u64 = 0;

/// A rig whose first read of free space is under the floor (the workspace's volume), and the rest are over it.
fn disk_paused() -> (Rig, String) {
    let mut rig = Rig::new(
        &recording("report"),
        &recording("report"),
        Settings {
            free: Some(FakeFree::new(&[FLOOR_LOW, FLOOR_HIGH])),
            ..Settings::default()
        },
    );
    let run = rig.started(None);
    rig.wait_item(&run, QueueKind::DiskFull);
    assert_eq!(
        rig.phases.run_state(&run),
        Some(RunState::Paused {
            cause: PauseCause::Disk
        })
    );
    (rig, run)
}

#[test]
fn a_resume_answer_continues_a_disk_paused_run() {
    let (mut rig, run) = disk_paused();
    let item = rig.wait_item(&run, QueueKind::DiskFull);
    let result = rig.answer(&item, QueueAnswer::Resume {});
    assert_eq!(result, QueueAnswerResult::Done {});
    assert_eq!(rig.wait_final(&run), PhaseState::Ended {});
    assert!(rig.workspace_dir(&run).join("README").is_file());
    assert!(rig.queue(&run).is_empty());
}

#[test]
fn a_stop_answer_stops_a_disk_paused_run() {
    let (mut rig, run) = disk_paused();
    let item = rig.wait_item(&run, QueueKind::DiskFull);
    let result = rig.answer(&item, QueueAnswer::Stop {});
    assert_eq!(result, QueueAnswerResult::Done {});
    rig.wait_run(&run, |state| matches!(state, RunState::Stopped {}));
    assert!(rig.queue(&run).is_empty());
    assert_eq!(rig.phases.state(&Rig::key(&run)), None, "no phase ever ran");
}

#[test]
fn run_stop_reaches_a_disk_paused_run() {
    let (mut rig, run) = disk_paused();
    let stopped = rig.stop(&run);
    assert_eq!(stopped.state, RunState::Stopped {});
    assert_eq!(stopped.signalled, 0);
    assert!(rig.queue(&run).is_empty());
}

#[test]
fn a_disk_pause_survives_a_restart_and_can_be_resumed() {
    let (mut rig, run) = disk_paused();
    rig.restart();
    let item = rig.wait_item(&run, QueueKind::DiskFull);
    let result = rig.answer(&item, QueueAnswer::Resume {});
    assert_eq!(result, QueueAnswerResult::Done {});
    assert_eq!(rig.wait_final(&run), PhaseState::Ended {});
}

// ---- the login session ------------------------------------------------------------------------------------------

#[test]
fn a_daemon_that_ends_at_logout_is_recorded_as_a_host_shutdown() {
    let mut rig = Rig::new(
        &recording("report"),
        &recording("report"),
        Settings::default(),
    );
    assert_eq!(rig.daemon.as_ref().unwrap().previous_end(), End::First);
    rig.login.leaving.store(true, Ordering::SeqCst);
    rig.restart();
    assert_eq!(
        rig.daemon.as_ref().unwrap().previous_end(),
        End::HostShutdown
    );
}

#[test]
fn a_daemon_that_ends_with_no_logout_is_recorded_as_stopped() {
    let mut rig = Rig::new(
        &recording("report"),
        &recording("report"),
        Settings::default(),
    );
    rig.restart();
    assert_eq!(rig.daemon.as_ref().unwrap().previous_end(), End::Stopped);
}

// ---- a run's own agent ------------------------------------------------------------------------------------------

/// Set in the environment of the child that plays a run's agent: the daemon root and the calls it makes, as JSON.
const AGENT_CALLS: &str = "COLLIDE_TEST_AGENT_CALLS";
const AGENT_REPLY: &str = "AGENT-REPLY ";

/// Not a test on its own: it is the client that [`as_a_run_agent`] starts in a child process of this test binary, whose
/// environment carries the run marker, so the daemon sees a run's agent at the other end of the socket. Run on its
/// own (no `AGENT_CALLS`) it does nothing. Each call is answered with a line holding the error code, or `ok`.
#[test]
fn agent_client_child() {
    let Ok(text) = std::env::var(AGENT_CALLS) else {
        return;
    };
    let spec: Value = serde_json::from_str(&text).unwrap();
    let paths = DaemonPaths::under(Path::new(spec["root"].as_str().unwrap()));
    let mut client = OwnerClient::connect(&paths, "agent-child").expect("the child connects");
    for call in spec["calls"].as_array().unwrap() {
        let answer = client.call::<_, Value>(
            call["method"].as_str().unwrap(),
            &call["params"],
            Some(Duration::from_secs(30)),
        );
        let reply = match answer {
            Ok(_) => json!("ok"),
            Err(collide_host::owner_client::ClientError::Rpc(error)) => json!(error.code),
            Err(other) => json!(format!("{other}")),
        };
        println!("{AGENT_REPLY}{reply}");
    }
}

/// Makes `calls` (`(method, params)`) from a process whose environment has the run marker, as a run's agent has, and
/// returns what each came back as: the error code, or `"ok"`.
fn as_a_run_agent(rig: &Rig, run: &str, calls: &[(&str, Value)]) -> Vec<Value> {
    let spec = json!({
        "root": rig.root.path(),
        "calls": calls.iter().map(|(method, params)| json!({"method": method, "params": params})).collect::<Vec<_>>(),
    });
    let output = Command::new(std::env::current_exe().unwrap())
        .args([
            "pause::agent_client_child",
            "--exact",
            "--nocapture",
            "--test-threads=1",
        ])
        .env_clear()
        .env("HOME", rig.root.path())
        .env("COLLIDE_DAEMON_ROOT", rig.root.path())
        .env(collide_host::auth::RUN_MARKER, run)
        .env(AGENT_CALLS, spec.to_string())
        .stdin(Stdio::null())
        .output()
        .expect("the child runs");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "{stdout}{}",
        String::from_utf8_lossy(&output.stderr)
    );
    stdout
        .lines()
        // The harness prints the test's name on the same line as the first output.
        .filter_map(|line| line.split_once(AGENT_REPLY).map(|(_, reply)| reply))
        .map(|reply| serde_json::from_str(reply).unwrap())
        .collect()
}

#[test]
fn a_run_agent_may_not_answer_the_queue_or_start_a_run_but_a_person_may_answer() {
    use collide_core::owner::code::NOT_PERMITTED;
    let mut rig = Rig::new(
        &recording("permission-request"),
        &recording("report"),
        Settings::default(),
    );
    let run = rig.started(None);
    let item = permission_item(&mut rig, &run);
    let journal = DaemonPaths::under(rig.root.path())
        .runs_dir
        .join(&run)
        .join(PHASE_NAME)
        .join("journal");
    let written_before = fs::metadata(&journal).unwrap().len();
    let runs_before = rig.run_ids();

    let answer = serde_json::to_value(QueueAnswerParams {
        item: item.id.clone(),
        answer: QueueAnswer::Permission {
            reply: PermissionReply::Allow {},
        },
    })
    .unwrap();
    let start = json!({
        "repo": rig.repo,
        "task_file": rig.task,
    });
    let replies = as_a_run_agent(
        &rig,
        &run,
        &[("queue.answer", answer), ("run.start_file", start)],
    );
    assert_eq!(
        replies,
        [json!(NOT_PERMITTED), json!(NOT_PERMITTED)],
        "{replies:?}"
    );

    assert_eq!(
        rig.queue(&run),
        std::slice::from_ref(&item),
        "the request is still pending"
    );
    assert_eq!(
        fs::metadata(&journal).unwrap().len(),
        written_before,
        "nothing was written to the child, or read from it"
    );
    assert_eq!(rig.run_ids(), runs_before, "no run was recorded");
    assert_eq!(
        rig.phases.state(&Rig::key(&run)),
        Some(PhaseState::Running {})
    );

    // The refusal is about the caller: a person's client answers the same item.
    let result = rig.answer(
        &item,
        QueueAnswer::Permission {
            reply: PermissionReply::Allow {},
        },
    );
    assert_eq!(result, QueueAnswerResult::Done {});
    assert_eq!(rig.wait_final(&run), PhaseState::Ended {});
}
