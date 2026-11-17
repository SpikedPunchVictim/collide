//! The shim over real children (ts-0054-cy): python3 processes started through [`Shim`], with the files they leave in
//! a run directory read back. The children are python3 and not a platform binary, as in the other real-process tests.
//!
//! What is checked here and not in the unit tests of `shim.rs`: the exit read from `wait4` (a known code, a signal),
//! a burst of output and one over-long line through the pumps, the journal's order against what the child received,
//! a session replayed from the journal the real shim wrote, the same journal cut at every byte, a writer that fails
//! with `ENOSPC` under the journal and under each log, the caps, a PTY, and the entry `run`.
//!
//! Each test makes a folder of its own under the system temp folder and removes exactly the files in it. A child is
//! killed and reaped by a guard if its test fails before it finished; nothing else is signalled.

use collide_core::adapter::{AgentSession, StampedEvent};
use collide_core::agent::PermissionReply;
use collide_core::journal::{Direction, JournalEnd, JournalRead, read_journal};
use collide_core::replay::replay_journal_bytes;
use collide_core::shim::{ExitEnd, ExitRecord, ShimMessage, parse_exit_record};
use collide_core::testing::NormalizedDialect;
use collide_core::testing::contract::ChildDialect;
use collide_host::shim::{
    self, ChildIdentity, ChildStdio, Limits, Shim, ShimOutcome, ShimSpec, Sinks, exit_code, files,
    line_cut_marker, log_full_marker,
};
use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const PYTHON: &str = "/usr/bin/python3";
/// ENOSPC is 28 on macOS and on Linux.
const ENOSPC: i32 = 28;

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// A run directory made for one test. `Drop` makes it writable again and removes the files in it by their own paths,
/// and then the folder (never a recursive delete).
struct RunDir(PathBuf);

impl RunDir {
    fn new() -> RunDir {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("collide-shimtest-{}-{n}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        RunDir(dir)
    }

    fn path(&self) -> &Path {
        &self.0
    }

    /// A spec for a python3 child that runs `script`, with a complete environment of just `PATH`.
    fn spec(&self, script: &str) -> ShimSpec {
        ShimSpec {
            program: PYTHON.into(),
            args: vec!["-u".into(), "-c".into(), script.into()],
            env: BTreeMap::from([("PATH".to_string(), "/usr/bin:/bin".to_string())]),
            cwd: self.0.clone(),
            run_dir: self.0.clone(),
            stdio: ChildStdio::Pipes,
            daemon: None,
        }
    }

    fn read(&self, name: &str) -> Vec<u8> {
        fs::read(self.0.join(name)).unwrap_or_else(|e| panic!("reading {name}: {e}"))
    }

    fn text(&self, name: &str) -> String {
        String::from_utf8(self.read(name)).unwrap()
    }

    fn journal(&self) -> JournalRead {
        read_journal(&self.read(files::JOURNAL))
    }

    fn exit_json(&self) -> ExitRecord {
        parse_exit_record(&self.read(files::EXIT_RECORD)).expect("exit.json is a record")
    }
}

impl Drop for RunDir {
    fn drop(&mut self) {
        let _ = fs::set_permissions(&self.0, fs::Permissions::from_mode(0o700));
        if let Ok(entries) = fs::read_dir(&self.0) {
            for entry in entries.flatten() {
                let _ = fs::remove_file(entry.path());
            }
        }
        let _ = fs::remove_dir(&self.0);
    }
}

/// A running shim that is killed and reaped if its test ends before [`Running::finish`], so a failing test leaves no
/// child behind. It signals only the child this shim started.
struct Running(Option<Shim>);

impl Running {
    fn start(spec: &ShimSpec) -> Running {
        Running(Some(
            Shim::start(spec, small_limits()).expect("the shim starts"),
        ))
    }

    fn start_with(spec: &ShimSpec, limits: Limits, sinks: Sinks) -> Running {
        Running(Some(
            Shim::start_with(spec, limits, sinks).expect("the shim starts"),
        ))
    }

    fn shim(&self) -> &Shim {
        self.0.as_ref().expect("not finished")
    }

    fn finish(mut self) -> ShimOutcome {
        self.0
            .take()
            .unwrap()
            .finish()
            .expect("the child is reaped")
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        if let Some(shim) = self.0.take() {
            let pid = shim.identity().pid as libc::pid_t;
            // SAFETY: `kill` takes two integers, and the pid is the child this test's shim started and has not reaped.
            unsafe { libc::kill(pid, libc::SIGKILL) };
            let _ = shim::reap(shim.identity().pid);
        }
    }
}

/// Real caps with a short wait for the pipes, so a test that leaves one open does not wait two seconds.
fn small_limits() -> Limits {
    Limits {
        drain_after_exit: Duration::from_secs(5),
        ..Limits::default()
    }
}

fn run_to_end(script: &str) -> (RunDir, ShimOutcome) {
    let dir = RunDir::new();
    let running = Running::start(&dir.spec(script));
    let outcome = running.finish();
    (dir, outcome)
}

/// A writer that fails with `ENOSPC` after `ok_writes` whole writes, and keeps what the others took.
#[derive(Clone)]
struct FullDisk {
    ok_writes: usize,
    state: Arc<Mutex<(usize, Vec<u8>)>>,
}

impl FullDisk {
    fn after(ok_writes: usize) -> FullDisk {
        FullDisk {
            ok_writes,
            state: Arc::default(),
        }
    }

    fn taken(&self) -> Vec<u8> {
        self.state.lock().unwrap().1.clone()
    }
}

impl Write for FullDisk {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut state = self.state.lock().unwrap();
        if state.0 >= self.ok_writes {
            return Err(io::Error::from_raw_os_error(ENOSPC));
        }
        state.0 += 1;
        state.1.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// The real files for a run directory, with one of them replaced.
fn sinks_with(
    dir: &RunDir,
    journal: Option<FullDisk>,
    stdout_log: Option<FullDisk>,
    stderr_log: Option<FullDisk>,
) -> Sinks {
    let mut sinks = Sinks::files(dir.path()).unwrap();
    if let Some(disk) = journal {
        sinks.journal = Box::new(disk);
    }
    if let Some(disk) = stdout_log {
        sinks.stdout_log = Box::new(disk);
    }
    if let Some(disk) = stderr_log {
        sinks.stderr_log = Box::new(disk);
    }
    sinks
}

fn out_lines(read: &JournalRead) -> Vec<Vec<u8>> {
    read.records
        .iter()
        .filter(|r| r.direction == Direction::Out)
        .map(|r| r.bytes.clone())
        .collect()
}

// ---- the exit comes from wait4 ----------------------------------------------------------------------------------

#[test]
fn a_child_that_exits_zero_is_recorded_as_exit_zero_in_memory_and_on_disk() {
    let (dir, outcome) = run_to_end("import sys; sys.exit(0)");
    assert!(outcome.write_error.is_none());
    assert_eq!(outcome.keeper.record().end, ExitEnd::Exited { code: 0 });
    assert_eq!(&dir.exit_json(), outcome.keeper.record());
}

#[test]
fn a_child_that_exits_with_a_known_code_is_recorded_with_that_code() {
    let (dir, outcome) = run_to_end("import sys; sys.exit(73)");
    assert_eq!(outcome.keeper.record().end, ExitEnd::Exited { code: 73 });
    assert_eq!(dir.exit_json().end, ExitEnd::Exited { code: 73 });
}

#[test]
fn a_child_ended_by_a_signal_is_recorded_with_the_signal() {
    let (dir, outcome) = run_to_end("import os, signal; os.kill(os.getpid(), signal.SIGKILL)");
    assert_eq!(outcome.keeper.record().end, ExitEnd::Signaled { signal: 9 });
    assert_eq!(dir.exit_json().end, ExitEnd::Signaled { signal: 9 });
}

#[test]
fn the_exit_record_carries_cpu_time_and_peak_memory_from_rusage() {
    let script = "x = sum(i * i for i in range(2_000_000)); big = bytearray(40 * 1024 * 1024)";
    let (_, outcome) = run_to_end(script);
    let record = outcome.keeper.record();
    assert!(record.cpu_user_us > 0, "{record:?}");
    assert!(record.max_rss_bytes > 40 * 1024 * 1024, "{record:?}");
    assert_eq!(record.protocol, collide_core::shim::SHIM_PROTOCOL);
}

#[test]
fn the_exit_record_is_served_until_the_daemon_acknowledges_it() {
    let (_, mut outcome) = run_to_end("pass");
    assert!(matches!(
        outcome.keeper.pending_message(),
        Some(ShimMessage::Exited { .. })
    ));
    outcome.keeper.acknowledge();
    assert_eq!(outcome.keeper.pending_message(), None);
}

#[test]
fn the_children_identity_is_in_the_run_directory() {
    let dir = RunDir::new();
    let running = Running::start(&dir.spec("import sys; sys.stdin.read()"));
    let identity = running.shim().identity();
    let on_disk: serde_json::Value = serde_json::from_slice(&dir.read(files::CHILD)).unwrap();
    assert_eq!(ChildIdentity::from_json(&on_disk), Some(identity));
    #[cfg(target_os = "macos")]
    assert_eq!(
        collide_host::sysproc::start_time_us(identity.pid),
        Some(identity.started_unix_us),
        "the start time is the kernel's"
    );
    running.shim().close_stdin().unwrap();
    running.finish();
}

#[test]
fn the_child_has_exactly_the_environment_it_was_given() {
    let (dir, _) = run_to_end("import os; print(sorted(os.environ)); print('HOME' in os.environ)");
    let text = dir.text(files::STDOUT_LOG);
    assert!(text.contains("'PATH'"), "{text}");
    assert!(
        text.ends_with("False\n"),
        "HOME leaked into the child: {text}"
    );
}

#[test]
fn the_child_leads_a_session_of_its_own() {
    let script = "import os; print(os.getsid(0) == os.getpid(), os.getpgrp() == os.getpid())";
    let (dir, _) = run_to_end(script);
    assert_eq!(dir.text(files::STDOUT_LOG), "True True\n");
}

// ---- starting fails cleanly ---------------------------------------------------------------------------------------

#[test]
fn a_program_that_is_not_there_is_an_error_naming_it() {
    let dir = RunDir::new();
    let mut spec = dir.spec("");
    spec.program = "/nonexistent/collide-no-such-program".into();
    let message = Shim::start(&spec, Limits::default())
        .err()
        .expect("no child to start")
        .to_string();
    assert!(message.contains("collide-no-such-program"), "{message}");
}

#[test]
fn a_run_directory_that_is_not_there_starts_no_child() {
    let dir = RunDir::new();
    let mut spec = dir.spec("pass");
    spec.run_dir = dir.path().join("missing");
    let message = Shim::start(&spec, Limits::default())
        .err()
        .expect("no child to start")
        .to_string();
    assert!(message.contains("missing"), "{message}");
}

#[test]
fn a_run_directory_with_a_journal_in_it_is_not_written_over() {
    let dir = RunDir::new();
    fs::write(dir.path().join(files::JOURNAL), b"earlier run").unwrap();
    assert!(Shim::start(&dir.spec("pass"), Limits::default()).is_err());
    assert_eq!(dir.read(files::JOURNAL), b"earlier run");
}

#[test]
fn the_files_the_shim_makes_are_private() {
    let (dir, _) = run_to_end("print('x')");
    for name in [
        files::JOURNAL,
        files::STDOUT_LOG,
        files::STDERR_LOG,
        files::EXIT_RECORD,
        files::CHILD,
    ] {
        let mode = fs::metadata(dir.path().join(name))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600, "{name}");
    }
}

// ---- output: a burst, a long line, odd bytes ----------------------------------------------------------------------

#[test]
fn a_burst_of_output_is_in_the_journal_and_the_log_whole_and_in_order() {
    let count = 20_000;
    let script = format!("for i in range({count}): print('line', i)");
    let (dir, outcome) = run_to_end(&script);
    assert!(outcome.problems.is_empty(), "{:?}", outcome.problems);
    let journal = dir.journal();
    assert_eq!(journal.end, JournalEnd::Clean);
    let expected: Vec<Vec<u8>> = (0..count)
        .map(|i| format!("line {i}\n").into_bytes())
        .collect();
    assert_eq!(out_lines(&journal), expected);
    assert_eq!(dir.read(files::STDOUT_LOG), expected.concat());
    assert!(
        journal
            .records
            .windows(2)
            .all(|pair| pair[0].wall_ns <= pair[1].wall_ns),
        "wall_ns never goes back"
    );
}

#[test]
fn stderr_goes_to_its_log_and_not_to_the_journal() {
    let (dir, _) = run_to_end("import sys; print('out'); print('err', file=sys.stderr)");
    assert_eq!(dir.text(files::STDERR_LOG), "err\n");
    assert_eq!(dir.text(files::STDOUT_LOG), "out\n");
    assert_eq!(out_lines(&dir.journal()), vec![b"out\n".to_vec()]);
}

#[test]
fn one_over_long_line_is_cut_and_marked_in_the_journal_and_the_log() {
    let dir = RunDir::new();
    let limits = Limits {
        line_bytes: 1000,
        ..small_limits()
    };
    let script = "print('x' * 50_000); print('after')";
    let running = Running::start_with(&dir.spec(script), limits, Sinks::files(dir.path()).unwrap());
    running.finish();
    let mut cut = vec![b'x'; 1000];
    cut.extend_from_slice(line_cut_marker(49_000).as_bytes());
    cut.push(b'\n');
    let expected = vec![cut, b"after\n".to_vec()];
    assert_eq!(out_lines(&dir.journal()), expected);
    assert_eq!(dir.read(files::STDOUT_LOG), expected.concat());
}

#[test]
fn bytes_that_are_not_text_pass_through_unchanged() {
    let script =
        "import sys; sys.stdout.buffer.write(b'\\xff\\xfe\\x00bin\\nrest'); sys.stdout.flush()";
    let (dir, _) = run_to_end(script);
    let journal = dir.journal();
    assert_eq!(
        out_lines(&journal),
        vec![b"\xff\xfe\x00bin\n".to_vec(), b"rest".to_vec()],
        "a last line with no newline is kept as it came"
    );
}

#[test]
fn a_log_over_its_cap_stops_growing_with_a_final_marker_and_the_journal_goes_on() {
    let dir = RunDir::new();
    let limits = Limits {
        log_bytes: 300,
        ..small_limits()
    };
    let script = "for i in range(100): print('0123456789' * 2)";
    let running = Running::start_with(&dir.spec(script), limits, Sinks::files(dir.path()).unwrap());
    let outcome = running.finish();
    assert!(outcome.problems.is_empty(), "{:?}", outcome.problems);
    let log = dir.text(files::STDOUT_LOG);
    let marker = log_full_marker(300);
    assert!(log.ends_with(&marker), "{log}");
    assert!(log.len() <= 300 + marker.len());
    assert_eq!(log.matches(&marker).count(), 1);
    assert_eq!(
        out_lines(&dir.journal()).len(),
        100,
        "the journal has no cap"
    );
    assert_eq!(outcome.keeper.record().end, ExitEnd::Exited { code: 0 });
}

#[test]
fn the_stderr_log_has_its_own_cap() {
    let dir = RunDir::new();
    let limits = Limits {
        log_bytes: 100,
        ..small_limits()
    };
    let script = "import sys\nfor i in range(50): print('e' * 20, file=sys.stderr)\nprint('fine')";
    let running = Running::start_with(&dir.spec(script), limits, Sinks::files(dir.path()).unwrap());
    running.finish();
    assert!(dir.text(files::STDERR_LOG).ends_with(&log_full_marker(100)));
    assert_eq!(dir.text(files::STDOUT_LOG), "fine\n");
}

#[test]
fn a_grandchild_that_keeps_the_pipe_open_does_not_hold_back_the_exit_record() {
    let dir = RunDir::new();
    let limits = Limits {
        drain_after_exit: Duration::from_millis(200),
        ..Limits::default()
    };
    let script = "import subprocess\n\
        p = subprocess.Popen(['/bin/sleep', '30'])\n\
        print(p.pid, flush=True)\n";
    let running = Running::start_with(&dir.spec(script), limits, Sinks::files(dir.path()).unwrap());
    let started = Instant::now();
    let outcome = running.finish();
    let waited = started.elapsed();
    let pid: libc::pid_t = dir
        .text(files::STDOUT_LOG)
        .trim()
        .parse()
        .expect("the sleeper's pid");
    // The sleeper is this test's own grandchild, started just above; stopping it lets its pipe close.
    // SAFETY: `kill` takes two integers, and the pid is the one the child printed for the process it started.
    unsafe { libc::kill(pid, libc::SIGKILL) };
    assert!(waited < Duration::from_secs(10), "waited {waited:?}");
    assert_eq!(outcome.keeper.record().end, ExitEnd::Exited { code: 0 });
    assert!(dir.path().join(files::EXIT_RECORD).exists());
}

// ---- the journal's order against what the child received ----------------------------------------------------------

#[test]
fn each_in_record_is_journaled_before_the_child_can_answer_it() {
    let dir = RunDir::new();
    let script = "import sys\nfor line in sys.stdin: sys.stdout.write('got:' + line)";
    let running = Running::start(&dir.spec(script));
    let mut sent = Vec::new();
    let mut returned = Vec::new();
    for i in 0..50 {
        let line = format!("message {i}\n");
        returned.push(running.shim().write_to_child(line.as_bytes()).unwrap());
        sent.push(line);
    }
    running.shim().close_stdin().unwrap();
    running.finish();

    let journal = dir.journal();
    assert_eq!(journal.end, JournalEnd::Clean);
    let ins: Vec<_> = journal
        .records
        .iter()
        .filter(|r| r.direction == Direction::In)
        .collect();
    assert_eq!(
        ins.iter()
            .flat_map(|r| r.bytes.clone())
            .collect::<Vec<u8>>(),
        sent.concat().into_bytes(),
        "the journal holds exactly what the child was sent, in order"
    );
    assert_eq!(returned, ins.iter().map(|r| r.seq).collect::<Vec<_>>());
    for (i, line) in sent.iter().enumerate() {
        let echo = format!("got:{line}").into_bytes();
        let out = journal
            .records
            .iter()
            .find(|r| r.direction == Direction::Out && r.bytes == echo)
            .unwrap_or_else(|| panic!("no echo of message {i}"));
        assert!(
            ins[i].seq < out.seq,
            "message {i} was journaled as {} but its echo is {}",
            ins[i].seq,
            out.seq
        );
        assert!(ins[i].wall_ns <= out.wall_ns);
    }
}

#[test]
fn seq_is_one_counter_for_both_directions() {
    let dir = RunDir::new();
    let script = "import sys\nprint('hello', flush=True)\nsys.stdin.readline()\nprint('bye')";
    let running = Running::start(&dir.spec(script));
    running.shim().write_to_child(b"go\n").unwrap();
    running.finish();
    let seqs: Vec<u64> = dir.journal().records.iter().map(|r| r.seq).collect();
    assert_eq!(seqs, vec![0, 1, 2]);
}

#[test]
fn a_write_the_journal_cannot_take_never_reaches_the_child() {
    let dir = RunDir::new();
    let script = "import sys\nfor line in sys.stdin: print('received', line.strip())";
    let sinks = sinks_with(&dir, Some(FullDisk::after(0)), None, None);
    let running = Running::start_with(&dir.spec(script), small_limits(), sinks);
    let error = running.shim().write_to_child(b"secret\n").unwrap_err();
    assert!(error.to_string().contains("journal"), "{error}");
    assert!(error.to_string().contains("space"), "{error}");
    running.shim().close_stdin().unwrap();
    let outcome = running.finish();
    assert_eq!(dir.text(files::STDOUT_LOG), "", "the child got nothing");
    assert!(outcome.problems.iter().any(|p| p.starts_with("journal:")));
}

#[test]
fn a_write_to_a_closed_stdin_is_an_error_after_it_was_journaled() {
    let dir = RunDir::new();
    let running = Running::start(&dir.spec("import sys; sys.stdin.read()"));
    running.shim().close_stdin().unwrap();
    assert!(running.shim().write_to_child(b"late\n").is_err());
    running.finish();
}

#[test]
fn a_write_of_no_bytes_is_refused() {
    let dir = RunDir::new();
    let running = Running::start(&dir.spec("import sys; sys.stdin.read()"));
    assert!(running.shim().write_to_child(b"").is_err());
    running.shim().close_stdin().unwrap();
    running.finish();
}

// ---- replay over the journal the real shim wrote -------------------------------------------------------------------

/// What a live session saw, as the shim's hook fed it.
struct Live {
    session: <NormalizedDialect as ChildDialect>::Session,
    events: Vec<StampedEvent>,
}

struct Conversation {
    journal: Vec<u8>,
    live: Live,
}

/// A real child that says what a normalized agent says (a handshake, a message and two permission requests), with a
/// live session fed through the shim's hook, a turn written to it and one request answered. The journal bytes come
/// from the real files.
fn conversation() -> Conversation {
    let dialect = NormalizedDialect;
    let mut lines = dialect.handshake();
    lines.push(dialect.message("working"));
    lines.push(dialect.permission_request(1));
    lines.push(dialect.permission_request(2));
    let wanted = lines.len();
    let args: Vec<String> = lines
        .iter()
        .map(|line| {
            String::from_utf8(line.clone())
                .unwrap()
                .trim_end()
                .to_string()
        })
        .collect();
    let script = "import sys\nfor line in sys.argv[1:]: print(line)\nfor line in sys.stdin: pass";

    let live = Arc::new(Mutex::new(Live {
        session: dialect.session(),
        events: Vec::new(),
    }));
    let seen = Arc::new(AtomicUsize::new(0));
    let hook_live = Arc::clone(&live);
    let hook_seen = Arc::clone(&seen);

    let dir = RunDir::new();
    let mut spec = dir.spec(script);
    spec.args.extend(args);
    let mut sinks = Sinks::files(dir.path()).unwrap();
    sinks.on_line = Some(Box::new(move |line, wall_ns| {
        let mut live = hook_live.lock().unwrap();
        let step = live.session.feed(line, wall_ns);
        live.events.extend(step.events);
        hook_seen.fetch_add(1, Ordering::SeqCst);
    }));
    let running = Running::start_with(&spec, small_limits(), sinks);

    let start = live.lock().unwrap().session.start();
    let turn = live.lock().unwrap().session.send_turn("fix the bug");
    for write in start.writes.iter().chain(&turn.writes) {
        running.shim().write_to_child(write).unwrap();
    }
    let deadline = Instant::now() + Duration::from_secs(20);
    while seen.load(Ordering::SeqCst) < wanted {
        assert!(
            Instant::now() < deadline,
            "the child's lines did not arrive"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    let answer = live
        .lock()
        .unwrap()
        .session
        .answer("1", PermissionReply::Allow {});
    for write in &answer.writes {
        running.shim().write_to_child(write).unwrap();
    }
    running.shim().close_stdin().unwrap();
    running.finish();

    let journal = dir.read(files::JOURNAL);
    let live = Arc::try_unwrap(live)
        .ok()
        .expect("the shim is done with the hook");
    Conversation {
        journal,
        live: live.into_inner().unwrap(),
    }
}

#[test]
fn a_session_replayed_from_the_shims_journal_reaches_the_state_of_the_live_one() {
    let Conversation { journal, mut live } = conversation();
    let dialect = NormalizedDialect;
    let mut rebuilt = dialect.session();
    let replay = replay_journal_bytes(&mut rebuilt, &journal).expect("the real journal replays");
    assert_eq!(replay.end, JournalEnd::Clean);
    assert_eq!(
        replay.step.events, live.events,
        "the same events, with the stored times"
    );
    assert!(
        replay.step.writes.is_empty(),
        "a replay writes nothing to the child"
    );

    // The request that is still pending is answered the same way, and the next turn is numbered the same way.
    let live_answer = live.session.answer("2", PermissionReply::Allow {});
    assert!(!live_answer.writes.is_empty());
    assert_eq!(rebuilt.answer("2", PermissionReply::Allow {}), live_answer);
    assert_eq!(rebuilt.send_turn("next"), live.session.send_turn("next"));
}

#[test]
fn a_journal_cut_at_any_byte_replays_up_to_the_last_whole_record() {
    let Conversation { journal, live } = conversation();
    let whole = read_journal(&journal);
    assert_eq!(whole.end, JournalEnd::Clean);
    let mut ends = Vec::new();
    let mut offset = 0;
    for record in &whole.records {
        offset += collide_core::journal::RECORD_HEADER_BYTES + record.bytes.len();
        ends.push(offset);
    }
    assert_eq!(offset, journal.len());

    for cut in 0..=journal.len() {
        let expected = ends.iter().filter(|&&end| end <= cut).count();
        let mut session = NormalizedDialect.session();
        let replay = replay_journal_bytes(&mut session, &journal[..cut])
            .unwrap_or_else(|e| panic!("cut at {cut}: {e}"));
        assert_eq!(replay.records, expected, "cut at {cut}");
        assert_eq!(
            replay.valid_len,
            expected.checked_sub(1).map_or(0, |i| ends[i]),
            "cut at {cut}"
        );
        if ends.contains(&cut) || cut == 0 {
            assert_eq!(replay.end, JournalEnd::Clean, "cut at {cut}");
        } else {
            assert!(
                matches!(replay.end, JournalEnd::Cut { .. }),
                "cut at {cut}: {:?}",
                replay.end
            );
        }
        assert_eq!(
            replay.step.events,
            live.events[..replay.step.events.len()],
            "cut at {cut}: a cut journal replays a prefix of the live events"
        );
    }
}

// ---- a full disk under the stream, the logs and the exit record --------------------------------------------------------

#[test]
fn a_journal_that_fills_up_mid_stream_does_not_crash_the_shim_or_stop_the_child() {
    let dir = RunDir::new();
    let disk = FullDisk::after(3);
    let sinks = sinks_with(&dir, Some(disk.clone()), None, None);
    let script = "for i in range(1000): print('line', i)";
    let running = Running::start_with(&dir.spec(script), small_limits(), sinks);
    let outcome = running.finish();

    assert_eq!(
        outcome.keeper.record().end,
        ExitEnd::Exited { code: 0 },
        "the child ran to its end"
    );
    assert!(outcome.write_error.is_none(), "exit.json is written");
    assert_eq!(dir.exit_json().end, ExitEnd::Exited { code: 0 });
    assert!(
        outcome
            .problems
            .iter()
            .any(|p| p.starts_with("journal:") && p.contains("space")),
        "{:?}",
        outcome.problems
    );
    let kept = read_journal(&disk.taken());
    assert_eq!(kept.records.len(), 3);
    assert_eq!(kept.end, JournalEnd::Clean);
    assert_eq!(
        dir.text(files::STDOUT_LOG).lines().count(),
        1000,
        "the log is a different file and went on"
    );
}

#[test]
fn a_stdout_log_that_fills_up_does_not_stop_the_journal_or_the_child() {
    let dir = RunDir::new();
    let sinks = sinks_with(&dir, None, Some(FullDisk::after(0)), None);
    let script = "for i in range(500): print('line', i)";
    let running = Running::start_with(&dir.spec(script), small_limits(), sinks);
    let outcome = running.finish();
    assert_eq!(outcome.keeper.record().end, ExitEnd::Exited { code: 0 });
    assert_eq!(out_lines(&dir.journal()).len(), 500);
    assert!(
        outcome
            .problems
            .iter()
            .any(|p| p.starts_with("stdout log:")),
        "{:?}",
        outcome.problems
    );
    assert_eq!(
        outcome
            .problems
            .iter()
            .filter(|p| p.starts_with("stdout log:"))
            .count(),
        1,
        "one failure is reported once"
    );
}

#[test]
fn a_stderr_log_that_fills_up_is_reported_and_stdout_goes_on() {
    let dir = RunDir::new();
    let sinks = sinks_with(&dir, None, None, Some(FullDisk::after(0)));
    let script = "import sys\nfor i in range(200): print('e', file=sys.stderr); print('o')";
    let running = Running::start_with(&dir.spec(script), small_limits(), sinks);
    let outcome = running.finish();
    assert!(
        outcome
            .problems
            .iter()
            .any(|p| p.starts_with("stderr log:"))
    );
    assert_eq!(dir.text(files::STDOUT_LOG).lines().count(), 200);
}

#[test]
fn a_shim_that_cannot_write_exit_json_keeps_the_record_and_leaves_no_file() {
    let dir = RunDir::new();
    // The child takes the write permission off the run directory, so the files the shim holds open still take
    // writes and only a new file (the temp file of `exit.json`) is refused, as on a full disk. It waits for its
    // identity file first: `Shim::start` renames that file in after the child starts, and a chmod that came first
    // refused the rename under load.
    let script = format!(
        "import os, sys, time\nwhile not os.path.exists(os.path.join(sys.argv[1], {:?})):\n    time.sleep(0.01)\n\
         os.chmod(sys.argv[1], 0o500)\nsys.exit(5)",
        files::CHILD
    );
    let mut spec = dir.spec(&script);
    spec.args.push(dir.path().display().to_string());
    let running = Running::start(&spec);
    let mut outcome = running.finish();
    fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o700)).unwrap();

    assert!(outcome.write_error.is_some(), "the write was refused");
    assert!(!dir.path().join(files::EXIT_RECORD).exists());
    assert!(!dir.path().join("exit.json.tmp").exists());
    assert_eq!(
        outcome.keeper.pending_message(),
        Some(ShimMessage::Exited {
            exit: ExitRecord {
                end: ExitEnd::Exited { code: 5 },
                ..outcome.keeper.record().clone()
            }
        }),
        "the record is served from memory"
    );
    outcome.keeper.acknowledge();
    assert_eq!(outcome.keeper.pending_message(), None);
}

// ---- a PTY ----------------------------------------------------------------------------------------------------------------

#[test]
fn a_pty_child_has_a_terminal_and_its_output_is_in_the_ring_and_the_log() {
    let dir = RunDir::new();
    let script = "import os, sys\n\
        print('tty' if sys.stdout.isatty() else 'notty')\n\
        print(os.get_terminal_size())\n\
        print('got', sys.stdin.readline().strip())";
    let mut spec = dir.spec(script);
    spec.stdio = ChildStdio::Pty { rows: 24, cols: 80 };
    let running = Running::start(&spec);
    running.shim().write_to_child(b"hello\n").unwrap();
    let outcome = running.finish();
    assert_eq!(outcome.keeper.record().end, ExitEnd::Exited { code: 0 });
    let log = dir.text(files::STDOUT_LOG);
    for expected in ["tty", "columns=80", "lines=24", "got hello"] {
        assert!(log.contains(expected), "{expected} not in {log:?}");
    }
    assert!(
        !log.contains("notty"),
        "the child saw a pipe, not a terminal: {log:?}"
    );
}

#[test]
fn a_pty_ring_replays_what_the_session_printed_so_far() {
    let dir = RunDir::new();
    let script = "import sys\nprint('so far', flush=True)\nsys.stdin.readline()";
    let mut spec = dir.spec(script);
    spec.stdio = ChildStdio::Pty { rows: 24, cols: 80 };
    let running = Running::start(&spec);
    let deadline = Instant::now() + Duration::from_secs(20);
    while !String::from_utf8_lossy(&running.shim().ring_snapshot()).contains("so far") {
        assert!(
            Instant::now() < deadline,
            "the output did not reach the ring"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    running.shim().write_to_child(b"\n").unwrap();
    running.finish();
}

#[test]
fn a_pty_has_no_stdin_to_close() {
    let dir = RunDir::new();
    let mut spec = dir.spec("import sys; sys.stdin.readline()");
    spec.stdio = ChildStdio::Pty { rows: 24, cols: 80 };
    let running = Running::start(&spec);
    assert!(running.shim().close_stdin().is_err());
    running.shim().write_to_child(b"\n").unwrap();
    running.finish();
}

// ---- the entry --------------------------------------------------------------------------------------------------------------

fn write_spec(dir: &RunDir, spec: &ShimSpec) {
    fs::write(dir.path().join(files::SPEC), spec.to_json().to_string()).unwrap();
}

#[test]
fn the_entry_runs_the_child_of_spec_json_and_writes_exit_json() {
    let dir = RunDir::new();
    write_spec(
        &dir,
        &dir.spec("import sys; print('from the child'); sys.exit(4)"),
    );
    // No daemon is there to acknowledge the record, so the shim is told not to wait for one: the record is on disk.
    let limits = Limits {
        ack_wait: Duration::ZERO,
        ..Limits::default()
    };
    let code = shim::run_with(&[dir.path().display().to_string()], limits);
    assert_eq!(
        code,
        exit_code::OK,
        "the shim's own code is not the child's"
    );
    assert_eq!(dir.exit_json().end, ExitEnd::Exited { code: 4 });
    assert_eq!(dir.text(files::STDOUT_LOG), "from the child\n");
}

#[test]
fn the_entry_with_the_wrong_arguments_is_a_usage_error() {
    assert_eq!(shim::run(&[]), exit_code::USAGE);
    assert_eq!(
        shim::run(&["a".to_string(), "b".to_string()]),
        exit_code::USAGE
    );
}

#[test]
fn the_entry_with_no_spec_is_a_usage_error() {
    let dir = RunDir::new();
    assert_eq!(
        shim::run(&[dir.path().display().to_string()]),
        exit_code::USAGE
    );
    assert!(!dir.path().join(files::EXIT_RECORD).exists());
}

#[test]
fn the_entry_with_a_spec_that_is_not_json_is_a_usage_error() {
    let dir = RunDir::new();
    fs::write(dir.path().join(files::SPEC), "{not json").unwrap();
    assert_eq!(
        shim::run(&[dir.path().display().to_string()]),
        exit_code::USAGE
    );
}

#[test]
fn the_entry_with_a_program_that_is_not_there_fails_and_writes_no_exit_json() {
    let dir = RunDir::new();
    let mut spec = dir.spec("");
    spec.program = "/nonexistent/collide-no-such-program".into();
    write_spec(&dir, &spec);
    assert_eq!(
        shim::run(&[dir.path().display().to_string()]),
        exit_code::FAILED
    );
    assert!(!dir.path().join(files::EXIT_RECORD).exists());
}
