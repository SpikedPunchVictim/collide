//! The fake ACP child, started for a test (`fixtures/fake_acp.py`; its step format is in that file's docstring).
//!
//! [`FakeAcp`] is a plain child of the test with its stdin and stdout piped, and no daemon, shim or session: the test
//! is the client. It has an environment of its own (`env_clear()`, then `HOME` and `COLLIDE_DAEMON_ROOT` in a temp
//! root) and is reaped on drop, so a test that fails first still leaves no child behind.

use super::common::{PYTHON, TempRoot};
use serde_json::Value;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread::JoinHandle;
use std::time::Duration;

/// How long a test waits for a line from the child before it calls the recording stuck.
const PATIENCE: Duration = Duration::from_secs(20);

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/adapters/acp/fixtures")
}

pub fn fake_acp_script() -> PathBuf {
    fixtures().join("fake_acp.py")
}

/// The folder of the recordings.
pub fn recordings_dir() -> PathBuf {
    fixtures().join("recordings")
}

/// The recording `name`, a file of `fixtures/recordings/`.
pub fn recording(name: &str) -> PathBuf {
    recordings_dir().join(format!("{name}.jsonl"))
}

/// The steps of a recording, in order: a blank line and a comment are not steps.
pub fn steps(recording: &Path) -> Vec<Value> {
    let text =
        fs::read_to_string(recording).unwrap_or_else(|e| panic!("{}: {e}", recording.display()));
    text.lines()
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| serde_json::from_str(l).expect("a step is JSON"))
        .collect()
}

/// What a recording says the child writes to stdout, in order: the `say` steps as the JSON they hold (with any
/// `{"$id": name}` still in it) and the `say_text` steps as strings.
pub fn said(recording: &Path) -> Vec<Value> {
    steps(recording)
        .into_iter()
        .filter_map(|step| step.get("say").or_else(|| step.get("say_text")).cloned())
        .collect()
}

/// A line the child wrote, as the value it says: the JSON it holds, or the text when it is not JSON.
pub fn value_of_line(line: &[u8]) -> Value {
    let text = String::from_utf8(line.to_vec()).expect("a line of text");
    let text = text.strip_suffix('\n').expect("a line ends with a newline");
    serde_json::from_str(text).unwrap_or_else(|_| Value::String(text.to_string()))
}

/// What the child left when it ended.
#[derive(Debug)]
pub struct Ended {
    pub code: Option<i32>,
    pub stderr: String,
}

/// The fake ACP child as a plain child of the test.
pub struct FakeAcp {
    child: Option<Child>,
    stdin: Option<ChildStdin>,
    lines: Receiver<Vec<u8>>,
    stderr: Option<JoinHandle<String>>,
}

impl FakeAcp {
    /// Starts the child on the recording file `path`, with `root` as its home, its daemon root and its working
    /// folder.
    pub fn start(root: &TempRoot, path: &Path) -> FakeAcp {
        let mut child = Command::new(PYTHON)
            .arg(fake_acp_script())
            .arg(path)
            .current_dir(root.path())
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("HOME", root.path())
            .env("COLLIDE_DAEMON_ROOT", root.path())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("python3 starts the fake ACP child");
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
        FakeAcp {
            child: Some(child),
            stdin,
            lines,
            stderr: Some(stderr),
        }
    }

    /// The next line the child wrote to stdout, with its newline; `None` once stdout has closed.
    pub fn next_line(&mut self) -> Option<Vec<u8>> {
        match self.lines.recv_timeout(PATIENCE) {
            Ok(line) => Some(line),
            Err(RecvTimeoutError::Disconnected) => None,
            Err(RecvTimeoutError::Timeout) => {
                panic!("the fake ACP child wrote nothing for {PATIENCE:?}")
            }
        }
    }

    /// The next line, as the value it says.
    pub fn next_value(&mut self) -> Option<Value> {
        self.next_line().map(|line| value_of_line(&line))
    }

    /// Whether the child writes nothing for `within`. A line that arrives in that time is taken, and is a `false`.
    pub fn quiet_for(&mut self, within: Duration) -> bool {
        matches!(
            self.lines.recv_timeout(within),
            Err(RecvTimeoutError::Timeout)
        )
    }

    /// Writes `bytes` to the child's stdin.
    pub fn send_bytes(&mut self, bytes: &[u8]) {
        let stdin = self.stdin.as_mut().expect("stdin is open");
        stdin.write_all(bytes).expect("the child reads stdin");
        stdin.flush().expect("the child reads stdin");
    }

    /// Writes `message` as one line of JSON.
    pub fn send(&mut self, message: &Value) {
        let mut bytes = serde_json::to_vec(message).expect("a message is JSON");
        bytes.push(b'\n');
        self.send_bytes(&bytes);
    }

    pub fn close_stdin(&mut self) {
        self.stdin = None;
    }

    /// Whether the child is still running.
    pub fn is_running(&mut self) -> bool {
        let child = self.child.as_mut().expect("not finished");
        child.try_wait().expect("the child can be asked").is_none()
    }

    /// Closes stdin, waits for the child to end and reads its exit code and stderr.
    pub fn finish(mut self) -> Ended {
        self.close_stdin();
        let status = self.child.take().expect("not finished").wait().unwrap();
        Ended {
            code: status.code(),
            stderr: self.stderr.take().unwrap().join().unwrap(),
        }
    }
}

impl Drop for FakeAcp {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
