//! A file-task run through the daemon (ts-0065-kx): `run.start_file` makes a worktree, runs the built-in builder under a
//! shim with fake Claude as `claude`, feeds the session from the journal and settles the phase. Each test has a
//! daemon of its own under a temp root: its environment is `env_clear()` plus what the test sets, `HOME` is the root
//! (so `models.yaml` is a fixture), git reads no configuration, and nothing here reads the owner's files. Every process
//! the test starts is reaped by `Fixture`'s `Drop`.

use super::fake::{fake_claude_script, recording, said};
use super::{ASSIGNED, common};
use collide_core::adapter::AgentSession;
use collide_core::agent::{AgentEvent, SessionStart, Usage};
use collide_core::events::EventBody;
use collide_core::owner::{
    LaunchProblem, QueueItem, QueueKind, QueueListParams, QueueListResult, RunStartFileParams,
    RunStartResult,
};
use collide_core::state::{FailCause, PauseCause, PhaseState, RunState, WorkspaceState};
use collide_host::adapters::claude::session::ClaudeSession;
use collide_host::base_env::CaptureOptions;
use collide_host::daemon::{Daemon, DaemonOptions, DaemonPaths};
use collide_host::disk::FreeSpace;
use collide_host::file_run::PHASE_NAME;
use collide_host::owner_client::OwnerClient;
use collide_host::phases::{PhaseKey, PhaseOptions, Phases};
use collide_host::store::PhaseRecord;
use common::procs::Guarded;
use common::{PYTHON, TempRoot, kill_child_of, signal_if_same, wait_until};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const FLOOR_HIGH: u64 = u64::MAX;
const FLOOR_LOW: u64 = 0;

/// Free space as a test sets it: the readings are returned in order and the last one repeats.
pub(super) struct FakeFree(Mutex<Vec<u64>>);

impl FakeFree {
    pub(super) fn new(readings: &[u64]) -> Arc<FakeFree> {
        Arc::new(FakeFree(Mutex::new(readings.to_vec())))
    }
}

impl FreeSpace for FakeFree {
    fn free_bytes(&self, _path: &Path) -> io::Result<u64> {
        let mut readings = self.0.lock().unwrap();
        Ok(if readings.len() > 1 {
            readings.remove(0)
        } else {
            readings[0]
        })
    }
}

#[derive(Default)]
struct Setup {
    /// Shell lines run before the capture's command, as the owner's login shell would run its profile.
    profile: String,
    /// How long the capture takes.
    capture_delay_secs: f32,
    free: Option<Arc<FakeFree>>,
}

struct Fixture {
    root: TempRoot,
    daemon: Option<Daemon>,
    phases: Phases,
    client: OwnerClient,
    repo: PathBuf,
    task: PathBuf,
}

pub(super) fn git(dir: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args([
            "-c",
            "user.name=test",
            "-c",
            "user.email=test@example.invalid",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .current_dir(dir)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

impl Fixture {
    /// A daemon whose `claude` is fake Claude playing `recording_path`.
    fn new(recording_path: &Path, setup: Setup) -> Fixture {
        let root = TempRoot::new();
        let bin = root.join("bin");
        fs::create_dir(&bin).unwrap();
        let wrapper = format!(
            "#!/bin/sh\nexec {PYTHON} {} {}\n",
            fake_claude_script().display(),
            recording_path.display()
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
                "#!/bin/sh\nsleep {}\nexport PATH={}:/usr/bin:/bin\n{}\neval \"$4\"\n",
                setup.capture_delay_secs,
                bin.display(),
                setup.profile
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

        let paths = DaemonPaths::under(root.path());
        let mut options = DaemonOptions::new(
            paths.clone(),
            shell.to_str().unwrap().to_string(),
            Box::new(io::sink()),
        );
        options.capture = CaptureOptions {
            timeout: Duration::from_secs(10),
            cap_bytes: 1024 * 1024,
            fallback: BTreeMap::from([("PATH".to_string(), "/usr/bin:/bin".to_string())]),
        };
        options.shim_exe = Some(PathBuf::from(env!("CARGO_BIN_EXE_collide")));
        options.phases = PhaseOptions {
            reply_wait: Duration::from_millis(1500),
            unresponsive_after: Duration::from_millis(800),
            ..PhaseOptions::default()
        };
        options.processes = Arc::new(Guarded::under(paths.runs_dir.clone(), Vec::new()));
        options.home = Some(home.to_str().unwrap().to_string());
        options.git_env = vec![
            ("GIT_CONFIG_GLOBAL".to_string(), "/dev/null".to_string()),
            ("GIT_CONFIG_SYSTEM".to_string(), "/dev/null".to_string()),
            ("GIT_CONFIG_NOSYSTEM".to_string(), "1".to_string()),
        ];
        if let Some(free) = setup.free {
            options.free_space = free;
        }
        let daemon = Daemon::start(options).expect("the daemon starts");
        let phases = daemon.phases();
        let client = OwnerClient::connect(daemon.paths(), "file-run-test").expect("connects");
        Fixture {
            root,
            daemon: Some(daemon),
            phases,
            client,
            repo: fs::canonicalize(repo).unwrap(),
            task,
        }
    }

    fn start_params(&self) -> RunStartFileParams {
        RunStartFileParams {
            repo: self.repo.to_str().unwrap().to_string(),
            task_file: self.task.to_str().unwrap().to_string(),
            alias: None,
            budget_usd: None,
        }
    }

    fn start(&mut self, params: &RunStartFileParams) -> RunStartResult {
        self.client
            .call("run.start_file", params, Some(Duration::from_secs(60)))
            .expect("the call is answered")
    }

    fn started(&mut self) -> String {
        match self.start(&self.start_params()) {
            RunStartResult::Started { run } => run,
            RunStartResult::Refused { problems } => panic!("refused: {problems:?}"),
        }
    }

    fn key(run: &str) -> PhaseKey {
        PhaseKey::new(run, PHASE_NAME).unwrap()
    }

    fn wait_final(&self, run: &str) -> PhaseState {
        let key = Fixture::key(run);
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

    fn wait_run(&self, run: &str, want: impl Fn(&RunState) -> bool) -> RunState {
        wait_until("the run reaches its state", Duration::from_secs(30), || {
            self.phases.run_state(run).is_some_and(|state| want(&state))
        });
        self.phases.run_state(run).unwrap()
    }

    fn paths(&self) -> DaemonPaths {
        DaemonPaths::under(self.root.path())
    }

    fn workspace_dir(&self, run: &str) -> PathBuf {
        self.root.join("data/workspaces").join(run)
    }

    fn record(&self, run: &str) -> Option<PhaseRecord> {
        let store = collide_host::store::Store::open(&self.paths().store_dir).unwrap();
        store.phase_record(run, PHASE_NAME).unwrap()
    }

    fn events(&self) -> Vec<EventBody> {
        self.daemon
            .as_ref()
            .unwrap()
            .events_since(0)
            .unwrap()
            .into_iter()
            .map(|event| event.body)
            .collect()
    }

    fn queue(&mut self, run: &str) -> Vec<QueueItem> {
        let result: QueueListResult = self
            .client
            .call(
                "queue.list",
                &QueueListParams {
                    board: None,
                    run: Some(run.to_string()),
                    kinds: vec![],
                },
                Some(Duration::from_secs(10)),
            )
            .unwrap();
        result.items
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        // The daemon stops its watchers; the shims go on, so each phase's shim and child are stopped by identity.
        let runs = self.paths().runs_dir;
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
        drop(self.daemon.take());
    }
}

/// `report.jsonl` with extra steps after its first step (the `expect` of the turn).
fn recording_with(root_for_file: &Path, name: &str, extra: &[Value]) -> PathBuf {
    let text = fs::read_to_string(recording("report")).unwrap();
    let mut out = Vec::new();
    let mut inserted = false;
    for line in text.lines() {
        out.push(line.to_string());
        if !inserted && line.starts_with("{\"expect\"") {
            out.extend(extra.iter().map(|step| step.to_string()));
            inserted = true;
        }
    }
    let path = root_for_file.join(name);
    fs::write(&path, out.join("\n")).unwrap();
    path
}

fn state_of_workspace_events(events: &[EventBody]) -> Vec<WorkspaceState> {
    events
        .iter()
        .filter_map(|body| match body {
            EventBody::WorkspaceTransition { to, .. } => Some(*to),
            _ => None,
        })
        .collect()
}

fn last_usage_of(recording_path: &PathBuf) -> Usage {
    let mut session = ClaudeSession::new(SessionStart::Assign {
        uuid: ASSIGNED.to_string(),
    });
    session.send_turn("go");
    let mut last = None;
    for line in said(recording_path) {
        let mut bytes = serde_json::to_vec(&line).unwrap();
        bytes.push(b'\n');
        for stamped in session.feed(&bytes, 1).events {
            if let AgentEvent::Usage { usage } = stamped.event {
                last = Some(usage);
            }
        }
    }
    last.expect("the recording reports usage")
}

fn first_problem(result: RunStartResult) -> LaunchProblem {
    match result {
        RunStartResult::Refused { mut problems } => problems.remove(0),
        other => panic!("expected a refusal, got {other:?}"),
    }
}

// ---- the run -----------------------------------------------------------------------------------------------------

#[test]
fn a_file_task_phase_runs_in_a_new_worktree_and_ends_with_its_report() {
    let path = recording("report");
    let mut fx = Fixture::new(&path, Setup::default());
    let run = fx.started();
    assert_eq!(fx.wait_final(&run), PhaseState::Ended {});
    let tree = fx.workspace_dir(&run);
    assert!(tree.join("README").is_file(), "the worktree holds the repo");
    assert!(tree.join(".git").exists());
    let record = fx.record(&run).expect("the phase has a record");
    assert_eq!(
        record.report.as_deref(),
        Some("The notes start with the first line.")
    );
    assert_eq!(record.failure, None);
    let ended = fx.wait_run(&run, |state| matches!(state, RunState::Ended { .. }));
    assert!(matches!(ended, RunState::Ended { .. }));
    // The run's state is read from its phases, so it is `ended` a moment before the end hook records the workspace
    // idle: wait for that event.
    wait_until("the workspace goes idle", Duration::from_secs(10), || {
        state_of_workspace_events(&fx.events()).last() == Some(&WorkspaceState::Idle)
    });
    assert_eq!(
        state_of_workspace_events(&fx.events()),
        [
            WorkspaceState::Provisioning,
            WorkspaceState::Ready,
            WorkspaceState::InUse,
            WorkspaceState::Idle
        ]
    );
}

#[test]
fn the_phase_record_keeps_the_last_usage_its_session_reported() {
    let path = recording("report");
    let mut fx = Fixture::new(&path, Setup::default());
    let run = fx.started();
    fx.wait_final(&run);
    let record = fx.record(&run).unwrap();
    assert_eq!(record.usage, Some(last_usage_of(&path)));
}

#[test]
fn the_first_turn_is_the_text_of_the_task_file_and_the_first_stdin_line_the_journal_holds() {
    let mut fx = Fixture::new(&recording("report"), Setup::default());
    let run = fx.started();
    fx.wait_final(&run);
    let journal = fs::read(
        fx.paths()
            .runs_dir
            .join(&run)
            .join(PHASE_NAME)
            .join("journal"),
    )
    .unwrap();
    let read = collide_core::journal::read_journal(&journal);
    let first_in = read
        .records
        .iter()
        .find(|record| record.direction == collide_core::journal::Direction::In)
        .expect("a write was journalled");
    let text = String::from_utf8_lossy(&first_in.bytes);
    assert!(text.contains("Add a line to README."), "{text}");
}

/// A recording whose child writes its init line and exits 0 with no `result`.
fn no_result_recording(root: &TempRoot) -> PathBuf {
    let path = root.join("no-result.jsonl");
    fs::write(
        &path,
        [
            r#"{"expect":{"type":"user"}}"#,
            r#"{"say":{"type":"system","subtype":"init","session_id":"7c1b0e52-9f3a-4d2e-8b61-0a5d3c9e4f17","cwd":"/work","tools":[],"mcp_servers":[],"model":"m","permissionMode":"default","apiKeySource":"none"}}"#,
            r#"{"exit":0}"#,
        ]
        .join("\n"),
    )
    .unwrap();
    path
}

#[test]
fn a_child_that_exits_0_with_no_result_ends_failed_with_the_cause_no_result() {
    let root = TempRoot::new();
    let path = no_result_recording(&root);
    let mut fx = Fixture::new(&path, Setup::default());
    let run = fx.started();
    assert_eq!(fx.wait_final(&run), PhaseState::Failed { cause: None });
    assert_eq!(
        fx.record(&run).unwrap().failure.as_deref(),
        Some("no_result")
    );
    let failed = fx.wait_run(&run, |state| matches!(state, RunState::Failed {}));
    assert_eq!(failed, RunState::Failed {});
}

/// The reads before the phase ends: the workspace volume, the run directory's, the workspace's own check, the run
/// directory before the phase, and the phase start's own check.
const READS_BEFORE_THE_PHASE_ENDS: usize = 5;

#[test]
fn a_phase_with_no_result_on_a_volume_under_the_floor_fails_with_the_cause_disk() {
    let root = TempRoot::new();
    let path = no_result_recording(&root);
    let mut readings = vec![FLOOR_HIGH; READS_BEFORE_THE_PHASE_ENDS];
    readings.push(FLOOR_LOW);
    let mut fx = Fixture::new(
        &path,
        Setup {
            free: Some(FakeFree::new(&readings)),
            ..Setup::default()
        },
    );
    let run = fx.started();
    assert_eq!(
        fx.wait_final(&run),
        PhaseState::Failed {
            cause: Some(FailCause::Disk)
        }
    );
    assert_eq!(fx.record(&run).unwrap().failure.as_deref(), Some("disk"));
}

#[test]
fn a_phase_with_no_result_on_a_volume_over_the_floor_keeps_the_cause_no_result() {
    let root = TempRoot::new();
    let path = no_result_recording(&root);
    let mut fx = Fixture::new(
        &path,
        Setup {
            free: Some(FakeFree::new(&[FLOOR_HIGH])),
            ..Setup::default()
        },
    );
    let run = fx.started();
    assert_eq!(fx.wait_final(&run), PhaseState::Failed { cause: None });
    assert_eq!(
        fx.record(&run).unwrap().failure.as_deref(),
        Some("no_result")
    );
}

// ---- settling in either order -------------------------------------------------------------------------------------

/// A `claude` written in Python: it reads the first turn, writes the lines of the `report` recording, and then ends as
/// `ending` says, so a test chooses whether its stdout ends before or after the process does.
fn claude_ending(root: &TempRoot, ending: &str) -> PathBuf {
    let lines: Vec<String> = said(&recording("report"))
        .iter()
        .map(|line| serde_json::to_string(line).unwrap())
        .collect();
    let script = root.join("claude.py");
    fs::write(
        &script,
        format!(
            "import os, subprocess, sys, time\nLINES = {lines:?}\nsys.stdin.readline()\n{ending}\n",
            lines = lines
        ),
    )
    .unwrap();
    script
}

/// Starts a fixture whose `claude` runs `script` instead of fake Claude.
fn fixture_running(script: &Path) -> Fixture {
    let fx = Fixture::new(&recording("report"), Setup::default());
    fs::write(
        fx.root.join("bin/claude"),
        format!("#!/bin/sh\nexec {PYTHON} {}\n", script.display()),
    )
    .unwrap();
    fx
}

#[test]
fn a_phase_whose_output_ends_before_the_process_does_settles_with_its_last_lines() {
    let root = TempRoot::new();
    let script = claude_ending(
        &root,
        "for line in LINES:\n    sys.stdout.write(line + '\\n')\nsys.stdout.flush()\nos.close(1)\ntime.sleep(1.5)",
    );
    let mut fx = fixture_running(&script);
    let run = fx.started();
    assert_eq!(fx.wait_final(&run), PhaseState::Ended {});
    assert_eq!(
        fx.record(&run).unwrap().report.as_deref(),
        Some("The notes start with the first line.")
    );
}

#[test]
fn a_phase_whose_process_exits_before_its_output_ends_settles_with_its_last_lines() {
    let root = TempRoot::new();
    // A grandchild keeps stdout open for a second after the child has exited.
    let script = claude_ending(
        &root,
        "subprocess.Popen(['/bin/sleep', '1'])\nfor line in LINES:\n    sys.stdout.write(line + '\\n')\nsys.stdout.flush()",
    );
    let mut fx = fixture_running(&script);
    let run = fx.started();
    assert_eq!(fx.wait_final(&run), PhaseState::Ended {});
    assert_eq!(
        fx.record(&run).unwrap().report.as_deref(),
        Some("The notes start with the first line.")
    );
}

// ---- the environment ------------------------------------------------------------------------------------------

#[test]
fn neither_the_phase_nor_its_grandchild_sees_the_claude_variables_the_daemon_started_with() {
    let root = TempRoot::new();
    let child = root.join("child-env.json");
    let grandchild = root.join("grandchild-env.json");
    let path = recording_with(
        root.path(),
        "env.jsonl",
        &[
            json!({"dump_env": child}),
            json!({"grandchild_dump_env": grandchild}),
        ],
    );
    let mut fx = Fixture::new(
        &path,
        Setup {
            profile: "export CLAUDECODE=1 CLAUDE_PID=4242 CLAUDE_CODE_FOO=bar KEEP_ME=kept ANTHROPIC_BASE_URL=http://gateway.invalid"
                .to_string(),
            ..Setup::default()
        },
    );
    let run = fx.started();
    assert_eq!(fx.wait_final(&run), PhaseState::Ended {});
    for dumped in [&child, &grandchild] {
        let env: Value = serde_json::from_str(&fs::read_to_string(dumped).unwrap()).unwrap();
        let env = env.as_object().unwrap();
        for stripped in ["CLAUDECODE", "CLAUDE_PID", "CLAUDE_CODE_FOO"] {
            assert!(!env.contains_key(stripped), "{dumped:?} has {stripped}");
        }
        assert_eq!(env.get("KEEP_ME"), Some(&json!("kept")), "{dumped:?}");
        assert_eq!(
            env.get("ANTHROPIC_BASE_URL"),
            Some(&json!("http://gateway.invalid")),
            "{dumped:?}: no ANTHROPIC_ variable is stripped"
        );
        assert_eq!(env.get("COLLIDE_RUN"), Some(&json!(run)), "{dumped:?}");
        assert!(env.contains_key("COLLIDE_RUN_START"), "{dumped:?}");
    }
}

#[test]
fn a_context_variable_is_set_after_the_roles_env_so_the_role_cannot_override_it() {
    use collide_host::adapters::claude::launch::COMPACT_AT_VAR;
    let role_env = BTreeMap::from([
        (COMPACT_AT_VAR.to_string(), "1".to_string()),
        ("OTHER".to_string(), "from-role".to_string()),
    ]);
    let context = BTreeMap::from([(COMPACT_AT_VAR.to_string(), "120000".to_string())]);
    let layered = collide_host::file_run::layered_env(&role_env, &context, "run-1", 7);
    assert_eq!(layered[COMPACT_AT_VAR], "120000");
    assert_eq!(layered["OTHER"], "from-role");
    assert_eq!(layered["COLLIDE_RUN"], "run-1");
    assert_eq!(layered["COLLIDE_RUN_START"], "7");
}

// ---- waiting for the base environment ----------------------------------------------------------------------------

#[test]
fn a_run_started_before_the_base_environment_is_captured_waits_for_the_capture() {
    let root = TempRoot::new();
    let dumped = root.join("env.json");
    let path = recording_with(root.path(), "wait.jsonl", &[json!({"dump_env": dumped})]);
    let mut fx = Fixture::new(
        &path,
        Setup {
            profile: "export FROM_THE_CAPTURE=yes".to_string(),
            capture_delay_secs: 1.5,
            ..Setup::default()
        },
    );
    let began = Instant::now();
    let run = fx.started();
    assert!(
        began.elapsed() >= Duration::from_millis(1000),
        "the call came back after {:?}, before the capture could have ended",
        began.elapsed()
    );
    assert_eq!(fx.wait_final(&run), PhaseState::Ended {});
    let env: Value = serde_json::from_str(&fs::read_to_string(&dumped).unwrap()).unwrap();
    assert_eq!(env["FROM_THE_CAPTURE"], json!("yes"));
}

// ---- the disk floor ---------------------------------------------------------------------------------------------

fn assert_paused_for_disk(fx: &mut Fixture, run: &str) {
    assert_eq!(
        fx.phases.run_state(run),
        Some(RunState::Paused {
            cause: PauseCause::Disk
        })
    );
    let items = fx.queue(run);
    assert_eq!(items.len(), 1, "{items:?}");
    assert_eq!(items[0].kind, QueueKind::DiskFull);
    assert_eq!(items[0].run.as_deref(), Some(run));
    assert!(items[0].title.contains("disk space"), "{}", items[0].title);
    assert_eq!(
        fx.phases.state(&Fixture::key(run)),
        None,
        "no phase was started"
    );
}

#[test]
fn free_space_under_the_floor_on_the_workspace_volume_pauses_the_run_before_the_worktree() {
    let free = FakeFree::new(&[FLOOR_LOW]);
    let mut fx = Fixture::new(
        &recording("report"),
        Setup {
            free: Some(free),
            ..Setup::default()
        },
    );
    let run = fx.started();
    assert_paused_for_disk(&mut fx, &run);
    assert!(!fx.workspace_dir(&run).exists(), "no worktree was made");
}

#[test]
fn free_space_under_the_floor_on_the_run_directory_volume_pauses_the_run_too() {
    // The first reading is the workspace's volume, the second the run directory's.
    let free = FakeFree::new(&[FLOOR_HIGH, FLOOR_LOW]);
    let mut fx = Fixture::new(
        &recording("report"),
        Setup {
            free: Some(free),
            ..Setup::default()
        },
    );
    let run = fx.started();
    assert_paused_for_disk(&mut fx, &run);
    assert!(!fx.workspace_dir(&run).exists());
}

#[test]
fn free_space_that_drops_before_the_phase_starts_leaves_a_ready_worktree_and_no_phase() {
    // Readings: workspace volume, run directory volume, the workspace's own check, then the run directory before the phase.
    let free = FakeFree::new(&[FLOOR_HIGH, FLOOR_HIGH, FLOOR_HIGH, FLOOR_LOW]);
    let mut fx = Fixture::new(
        &recording("report"),
        Setup {
            free: Some(free),
            ..Setup::default()
        },
    );
    let run = fx.started();
    assert_paused_for_disk(&mut fx, &run);
    assert!(fx.workspace_dir(&run).join("README").is_file());
    assert_eq!(
        state_of_workspace_events(&fx.events()),
        [WorkspaceState::Provisioning, WorkspaceState::Ready]
    );
}

// ---- what is refused ----------------------------------------------------------------------------------------------

#[test]
fn an_alias_with_no_mapping_is_refused_with_its_name() {
    let mut fx = Fixture::new(&recording("report"), Setup::default());
    let mut params = fx.start_params();
    params.alias = Some("nowhere".to_string());
    let problem = first_problem(fx.start(&params));
    assert!(problem.message.contains("nowhere"), "{}", problem.message);
}

#[test]
fn a_task_file_that_is_not_there_is_refused_and_no_run_is_recorded() {
    let mut fx = Fixture::new(&recording("report"), Setup::default());
    let mut params = fx.start_params();
    params.task_file = fx.root.join("missing.md").to_str().unwrap().to_string();
    let problem = first_problem(fx.start(&params));
    assert!(
        problem.message.contains("missing.md"),
        "{}",
        problem.message
    );
    assert!(
        !fx.events()
            .iter()
            .any(|body| matches!(body, EventBody::RunTransition { .. }))
    );
}

#[test]
fn the_run_is_recorded_running_before_its_phase_and_its_start_is_that_events_time() {
    let mut fx = Fixture::new(&recording("report"), Setup::default());
    let run = fx.started();
    fx.wait_final(&run);
    let events = fx.daemon.as_ref().unwrap().events_since(0).unwrap();
    let first_run_event = events
        .iter()
        .find(|event| matches!(&event.body, EventBody::RunTransition { run: r, .. } if *r == run))
        .expect("a run event");
    let first_phase_event = events
        .iter()
        .find(|event| matches!(&event.body, EventBody::PhaseTransition { run: r, .. } if *r == run))
        .expect("a phase event");
    assert!(first_run_event.seq < first_phase_event.seq);
    assert_eq!(
        fx.phases.run_start_us(&run),
        Some(first_run_event.wall_ms * 1000)
    );
}
