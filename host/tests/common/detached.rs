//! A command started so that this test process is not among its parents.
//!
//! The daemon decides who is asking by walking the peer's parents and reading each one's environment, and the system's
//! answer for a process is the environment it was started with. A test process that was started inside a phase of a
//! Collide run has the run's `COLLIDE_RUN` there, and no test can change that, so a `collide` command that is the
//! test's child looks like the agent of the run the tests are run in, whatever environment the command itself has.
//! A command started here has a shell between it and this process, and that shell is orphaned (its parent is
//! `launchd`) before the command starts, so the walk ends there: a person at a terminal (ts-0074-cu).
//!
//! The command's output goes to files in the test's own [`TempRoot`], and [`Detached::wait`] reads them back.

use super::{TempRoot, wait_until};
use std::fs;
use std::os::unix::process::ExitStatusExt;
use std::path::PathBuf;
use std::process::{Command, ExitStatus, Output, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc;
use std::time::Duration;

/// `$1` is the folder for the files, the rest is the command. The outer shell ends at once; the subshell it leaves
/// waits for it to be gone (`$$` is the outer shell's pid in a subshell) before it starts the command, so the command
/// never has the test among its parents, then records the command's pid and, when it ends, its exit code.
const LAUNCHER: &str = r#"dir=$1; shift
(
  while kill -0 $$ 2>/dev/null; do sleep 0.01; done
  "$@" </dev/null >"$dir/out" 2>"$dir/err" &
  pid=$!
  echo $pid >"$dir/pid.tmp"; mv "$dir/pid.tmp" "$dir/pid"
  wait $pid
  echo $? >"$dir/code.tmp"; mv "$dir/code.tmp" "$dir/code"
) </dev/null >/dev/null 2>&1 &
"#;

/// How long [`Detached::wait`] gives a command: a run's whole phase, with a fake Claude, takes seconds.
const WAIT: Duration = Duration::from_secs(120);

static COUNTER: AtomicU32 = AtomicU32::new(0);

pub struct Detached {
    dir: PathBuf,
}

impl Detached {
    /// Starts `command` below `root`. The command's program, arguments, working directory and environment are used
    /// as they are, so a command built with `env_clear()` and its own variables has just those. Its standard input,
    /// output and error are not used: the input is empty and the output is [`Detached::wait`]'s.
    pub fn start(root: &TempRoot, command: &Command) -> Detached {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = root.join(&format!("detached-{n}"));
        fs::create_dir(&dir).unwrap();
        let mut launcher = Command::new("/bin/sh");
        launcher
            .arg("-c")
            .arg(LAUNCHER)
            .arg("sh")
            .arg(&dir)
            .arg(command.get_program())
            .args(command.get_args())
            .env_clear()
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        for (name, value) in command.get_envs() {
            if let Some(value) = value {
                launcher.env(name, value);
            }
        }
        if let Some(cwd) = command.get_current_dir() {
            launcher.current_dir(cwd);
        }
        // Waited for here, so the outer shell is gone and not a zombie when the subshell looks for it.
        let status = launcher.status().expect("/bin/sh starts the launcher");
        assert!(status.success(), "the launcher exited with {status}");
        Detached { dir }
    }

    fn read(&self, name: &str) -> Option<String> {
        fs::read_to_string(self.dir.join(name)).ok()
    }

    /// The command's pid, once it has started.
    pub fn pid(&self) -> u32 {
        wait_until("the command's pid", WAIT, || self.read("pid").is_some());
        self.read("pid").unwrap().trim().parse().unwrap()
    }

    /// Kills the command, as a closed terminal does.
    pub fn kill(&self) {
        // SAFETY: `kill` takes two integers; the pid is the one the launcher recorded for this test's own command.
        unsafe { libc::kill(self.pid() as libc::pid_t, libc::SIGKILL) };
    }

    fn ended(&self) -> bool {
        self.dir.join("code").exists()
    }

    /// The command's output lines as it writes them, until it ends.
    pub fn lines(&self) -> mpsc::Receiver<String> {
        let (out, code) = (self.dir.join("out"), self.dir.join("code"));
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let (mut seen, mut pending) = (0, Vec::new());
            loop {
                // Looked at first, so a command that has ended has all of its output read.
                let ended = code.exists();
                let bytes = fs::read(&out).unwrap_or_default();
                if bytes.len() > seen {
                    pending.extend_from_slice(&bytes[seen..]);
                    seen = bytes.len();
                    while let Some(at) = pending.iter().position(|b| *b == b'\n') {
                        let line: Vec<u8> = pending.drain(..=at).collect();
                        let line = String::from_utf8_lossy(&line[..line.len() - 1]).into_owned();
                        if tx.send(line).is_err() {
                            return;
                        }
                    }
                } else if ended {
                    return;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        });
        rx
    }

    /// Waits for the command to end and returns what it wrote and its exit code.
    pub fn wait(&self) -> Output {
        wait_until("the command to end", WAIT, || self.ended());
        let code: i32 = self.read("code").unwrap().trim().parse().unwrap();
        let bytes = |name| fs::read(self.dir.join(name)).unwrap_or_default();
        Output {
            status: ExitStatus::from_raw(code << 8),
            stdout: bytes("out"),
            stderr: bytes("err"),
        }
    }
}

impl Drop for Detached {
    /// A command that is still there when its test ends (the test panicked) is this test's own and is ended.
    fn drop(&mut self) {
        if !self.ended()
            && let Some(pid) = self
                .read("pid")
                .and_then(|text| text.trim().parse::<i32>().ok())
        {
            // SAFETY: `kill` takes two integers; the pid is the one the launcher recorded for this test's command,
            // which has not ended.
            unsafe { libc::kill(pid, libc::SIGKILL) };
        }
    }
}
