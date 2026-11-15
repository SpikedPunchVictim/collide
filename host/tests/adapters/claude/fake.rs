//! Fake Claude, started for a test (`fixtures/fake_claude.py`; its script format is in that file's docstring).
//!
//! Two ways to run it, both with an environment of the test's own (`env_clear()`, then `HOME` and `PATH`) and no daemon:
//!
//! - [`FakeClaude`] is a plain child with pipes, for the tests of fake Claude itself and for a session fed by hand;
//! - [`play`] runs it under the real shim ([`collide_host::shim::Shim`]) with a live [`ClaudeSession`] fed from the shim's
//!   line hook, as the daemon will, so the journal the shim wrote is the one a replay is checked against.
//!
//! Both reap what they start: a test that fails first still leaves no child behind.

use super::{ASSIGNED, common};
use collide_core::adapter::AgentSession;
use collide_core::adapter::{ExitInfo, StampedEvent, Step};
use collide_core::agent::{AgentEvent, SessionStart};
use collide_host::adapters::claude::session::ClaudeSession;
use collide_host::shim::{self, ChildStdio, Limits, Shim, ShimSpec, Sinks, files};
use common::{PYTHON, TempRoot};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

/// How long a test waits for a line from fake Claude before it calls the script stuck.
const PATIENCE: Duration = Duration::from_secs(20);

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/adapters/claude/fixtures")
}

pub fn fake_claude_script() -> PathBuf {
    fixtures().join("fake_claude.py")
}

/// The recording `name`, a file of `fixtures/recordings/`.
pub fn recording(name: &str) -> PathBuf {
    fixtures().join("recordings").join(format!("{name}.jsonl"))
}

/// What a recording says fake Claude writes to stdout, in order: the `say` steps as the JSON they hold and the
/// `say_text` steps as strings.
pub fn said(recording: &PathBuf) -> Vec<Value> {
    let text =
        fs::read_to_string(recording).unwrap_or_else(|e| panic!("{}: {e}", recording.display()));
    let mut lines = Vec::new();
    for line in text
        .lines()
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
    {
        let step: Value = serde_json::from_str(line).expect("a step is JSON");
        if let Some(said) = step.get("say") {
            lines.push(said.clone());
        } else if let Some(said) = step.get("say_text") {
            lines.push(said.clone());
        }
    }
    lines
}

/// A line fake Claude wrote, as the value it says: the JSON it holds, or the text when it is not JSON.
pub fn value_of_line(line: &[u8]) -> Value {
    let text = String::from_utf8(line.to_vec()).expect("a line of text");
    let text = text.strip_suffix('\n').expect("a line ends with a newline");
    serde_json::from_str(text).unwrap_or_else(|_| Value::String(text.to_string()))
}

/// What fake Claude left when it ended.
#[derive(Debug)]
pub struct Ended {
    pub code: Option<i32>,
    pub stderr: String,
}

/// Fake Claude as a plain child of the test, with its stdin and stdout piped.
pub struct FakeClaude {
    child: Option<Child>,
    stdin: Option<ChildStdin>,
    lines: Receiver<Vec<u8>>,
    stderr: Option<JoinHandle<String>>,
}

impl FakeClaude {
    /// Starts fake Claude on the script file `path`, with `root` as its home and its working directory.
    pub fn start(root: &TempRoot, path: &PathBuf) -> FakeClaude {
        let mut child = Command::new(PYTHON)
            .arg(fake_claude_script())
            .arg(path)
            .current_dir(root.path())
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("HOME", root.path())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("python3 starts fake claude");
        let stdin = child.stdin.take();
        let mut stdout = BufReader::new(child.stdout.take().expect("piped"));
        let (send, lines) = mpsc::channel();
        std::thread::spawn(move || {
            loop {
                let mut line = Vec::new();
                match stdout.read_until(b'\n', &mut line) {
                    Ok(0) | Err(_) => return,
                    Ok(_) => {
                        if send.send(line).is_err() {
                            return;
                        }
                    }
                }
            }
        });
        let mut stderr = child.stderr.take().expect("piped");
        let stderr = std::thread::spawn(move || {
            let mut text = String::new();
            let _ = std::io::Read::read_to_string(&mut stderr, &mut text);
            text
        });
        FakeClaude {
            child: Some(child),
            stdin,
            lines,
            stderr: Some(stderr),
        }
    }

    /// The next line fake Claude wrote to stdout, with its newline; `None` once stdout has closed.
    pub fn next_line(&mut self) -> Option<Vec<u8>> {
        match self.lines.recv_timeout(PATIENCE) {
            Ok(line) => Some(line),
            Err(RecvTimeoutError::Disconnected) => None,
            Err(RecvTimeoutError::Timeout) => panic!("fake claude wrote nothing for {PATIENCE:?}"),
        }
    }

    pub fn send(&mut self, bytes: &[u8]) {
        let stdin = self.stdin.as_mut().expect("stdin is open");
        stdin.write_all(bytes).expect("fake claude reads stdin");
        stdin.flush().expect("fake claude reads stdin");
    }

    pub fn close_stdin(&mut self) {
        self.stdin = None;
    }

    /// Closes stdin, waits for fake Claude to end and reads its exit code and stderr.
    pub fn finish(mut self) -> Ended {
        self.close_stdin();
        let status = self.child.take().expect("not finished").wait().unwrap();
        Ended {
            code: status.code(),
            stderr: self.stderr.take().unwrap().join().unwrap(),
        }
    }
}

impl Drop for FakeClaude {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

// ---- a live session under the real shim -------------------------------------------------------------------------------

/// The shim and its child, killed and reaped if the test ends before [`play`] finishes it.
struct Running(Option<Shim>);

impl Drop for Running {
    fn drop(&mut self) {
        if let Some(shim) = self.0.take() {
            let pid = shim.identity().pid;
            // SAFETY: `kill` takes two integers; the pid is the child this test's shim started and has not reaped.
            unsafe { libc::kill(pid as libc::pid_t, libc::SIGKILL) };
            let _ = shim::reap(pid);
        }
    }
}

/// A live session and everything it made, behind the lock the shim's line hook and the driver share.
struct Live {
    session: ClaudeSession,
    events: Vec<StampedEvent>,
}

/// What [`play`] leaves: the live session as it ended, all of its events in order (a fed event has the stored time
/// of its line; one made by `start`, `send_turn` or `answer` has none), the bytes of the journal the real shim
/// wrote, and the exit information `finish` takes.
pub struct Played {
    pub session: ClaudeSession,
    pub events: Vec<StampedEvent>,
    pub journal: Vec<u8>,
    pub exit: ExitInfo,
}

impl Played {
    /// The events a replay makes: those that came from a line of the child's.
    pub fn fed_events(&self) -> Vec<StampedEvent> {
        self.events
            .iter()
            .filter(|stamped| stamped.wall_ns.is_some())
            .cloned()
            .collect()
    }

    /// The agent events only, in order.
    pub fn agent_events(&self) -> Vec<AgentEvent> {
        self.events.iter().map(|s| s.event.clone()).collect()
    }
}

/// Runs fake Claude on `recording` under the real shim, with a live session that starts as `start`. The session sends
/// `turns[0]` at once and the next one each time a turn ends, and `react` is called with each event the child's lines
/// make, to answer a request or cancel (what it returns is written to the child and journaled). Stdin closes after
/// the last turn ends. A recording must end every turn it has with a `result`, or the run waits for one.
pub fn play(
    name: &str,
    start: SessionStart,
    turns: &[&str],
    mut react: impl FnMut(&mut ClaudeSession, &AgentEvent) -> Step,
) -> Played {
    let root = TempRoot::new();
    let run_dir = root.join("run");
    fs::create_dir(&run_dir).unwrap();
    let spec = ShimSpec {
        program: PYTHON.into(),
        args: vec![
            fake_claude_script().display().to_string(),
            recording(name).display().to_string(),
        ],
        env: BTreeMap::from([
            ("PATH".to_string(), "/usr/bin:/bin".to_string()),
            ("HOME".to_string(), root.path().display().to_string()),
        ]),
        cwd: root.path().to_path_buf(),
        run_dir: run_dir.clone(),
        stdio: ChildStdio::Pipes,
        daemon: None,
    };

    let live = Arc::new(Mutex::new(Live {
        session: ClaudeSession::new(start),
        events: Vec::new(),
    }));
    let (seen, events) = mpsc::channel::<AgentEvent>();
    let hook_live = Arc::clone(&live);
    let mut sinks = Sinks::files(&run_dir).unwrap();
    sinks.on_line = Some(Box::new(move |line, wall_ns| {
        let step = {
            let mut live = hook_live.lock().unwrap();
            let step = live.session.feed(line, wall_ns);
            live.events.extend(step.events.clone());
            step
        };
        for stamped in step.events {
            let _ = seen.send(stamped.event);
        }
    }));
    let limits = Limits {
        drain_after_exit: Duration::from_secs(5),
        ..Limits::default()
    };
    let running = Running(Some(
        Shim::start_with(&spec, limits, sinks).expect("the shim starts"),
    ));
    let shim = || running.0.as_ref().expect("not finished");

    // One call of the live session: its events are kept, and what it writes goes to the child (and so to the journal).
    // The lock is held for the call and not for the write, so the shim's line hook is never kept waiting.
    let drive = |call: &mut dyn FnMut(&mut ClaudeSession) -> Step| {
        let step = {
            let mut live = live.lock().unwrap();
            let step = call(&mut live.session);
            live.events.extend(step.events.clone());
            step
        };
        for bytes in &step.writes {
            shim().write_to_child(bytes).expect("the child reads stdin");
        }
    };
    let mut turns = turns.iter();
    drive(&mut |session| session.start());
    let first = turns.next().expect("a turn");
    drive(&mut |session| session.send_turn(first));
    loop {
        let event = events
            .recv_timeout(PATIENCE)
            .expect("an event from the child");
        drive(&mut |session| react(session, &event));
        if matches!(event, AgentEvent::TurnEnd { .. }) {
            match turns.next() {
                Some(text) => drive(&mut |session| session.send_turn(text)),
                None => break,
            }
        }
    }
    shim().close_stdin().unwrap();
    let outcome = shim().finish().expect("the child is reaped");
    let exit = ExitInfo {
        record: Some(outcome.keeper.record().clone()),
        stderr_tail: fs::read_to_string(run_dir.join(files::STDERR_LOG)).unwrap_or_default(),
    };
    let journal = fs::read(run_dir.join(files::JOURNAL)).unwrap();
    drop(running);
    let Live { session, events } = Arc::try_unwrap(live)
        .ok()
        .expect("the shim is done with the hook")
        .into_inner()
        .unwrap();
    Played {
        session,
        events,
        journal,
        exit,
    }
}

/// The session id of the sessions these tests start.
pub fn assigned() -> SessionStart {
    SessionStart::Assign {
        uuid: ASSIGNED.to_string(),
    }
}
