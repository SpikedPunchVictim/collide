//! The bridge's daemon: a `collide-daemon` process that is the bridge's own child, in the root it was given.
//!
//! The test rule. The daemon has `env_clear()` and then `PATH`, `HOME` and `COLLIDE_DAEMON_ROOT` (the root, both) and
//! `SHELL` (the login shell of the test, which puts fake Claude first on the `PATH`). It reads no file of the person's,
//! loads no launchd job (it is under `COLLIDE_DAEMON_ROOT`) and fetches no published version. The bridge ends it, so
//! unlike the real start (`LaunchStarter`) it is never left behind.

use collide_app::shell::DaemonStarter;
use collide_host::daemon::DaemonPaths;
use std::fs::File;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

/// How long a start waits for the daemon's socket to answer.
const START_WAIT: Duration = Duration::from_secs(30);

/// How long the daemon is given to end after `SIGTERM` before it is killed.
const END_WAIT: Duration = Duration::from_secs(10);

pub struct ChildDaemon {
    exe: PathBuf,
    root: PathBuf,
    shell: PathBuf,
    paths: DaemonPaths,
    child: Mutex<Option<Child>>,
    /// Set when the bridge is ending, so that a late call or the event pump does not start the daemon again.
    retired: AtomicBool,
}

impl ChildDaemon {
    /// `exe` is the `collide-daemon` beside the bridge, and `shell` the login shell it is told to run.
    pub fn new(exe: PathBuf, root: &Path, shell: PathBuf) -> ChildDaemon {
        ChildDaemon {
            exe,
            root: root.to_path_buf(),
            shell,
            paths: DaemonPaths::under(root),
            child: Mutex::new(None),
            retired: AtomicBool::new(false),
        }
    }

    /// Whether the daemon answers on its socket.
    pub fn answers(&self) -> bool {
        UnixStream::connect(&self.paths.socket).is_ok()
    }

    fn spawn(&self) -> Result<Child, String> {
        // What the daemon says is kept in a file of the root, for a failure's report.
        let err = File::create(self.root.join("daemon.err"))
            .map_err(|error| format!("could not make the daemon's error file: {error}"))?;
        Command::new(&self.exe)
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("HOME", &self.root)
            .env("COLLIDE_DAEMON_ROOT", &self.root)
            .env("SHELL", &self.shell)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(err)
            .spawn()
            .map_err(|error| format!("could not start {}: {error}", self.exe.display()))
    }

    /// From now on no start is made.
    pub fn retire(&self) {
        self.retired.store(true, Ordering::SeqCst);
    }

    /// Ends the daemon: `SIGTERM`, as a person's stop would, and a kill when it does not end.
    pub fn end(&self) {
        let Some(mut child) = self.child.lock().unwrap_or_else(|e| e.into_inner()).take() else {
            return;
        };
        let _ = Command::new("/bin/kill")
            .args(["-TERM", &child.id().to_string()])
            .status();
        let until = Instant::now() + END_WAIT;
        while Instant::now() < until {
            if matches!(child.try_wait(), Ok(Some(_))) {
                return;
            }
            thread::sleep(Duration::from_millis(50));
        }
        let _ = child.kill();
        let _ = child.wait();
    }
}

impl DaemonStarter for ChildDaemon {
    fn ensure_running(&self) -> Result<(), String> {
        let mut slot = self.child.lock().unwrap_or_else(|e| e.into_inner());
        if self.answers() {
            return Ok(());
        }
        if self.retired.load(Ordering::SeqCst) {
            return Err("the bridge is ending".to_string());
        }
        // A daemon that ended is made again, as the real start does.
        if slot
            .as_mut()
            .is_some_and(|child| matches!(child.try_wait(), Ok(Some(_))))
        {
            *slot = None;
        }
        if slot.is_none() {
            *slot = Some(self.spawn()?);
        }
        let until = Instant::now() + START_WAIT;
        while Instant::now() < until {
            if self.answers() {
                return Ok(());
            }
            if let Some(status) = slot
                .as_mut()
                .and_then(|child| child.try_wait().ok().flatten())
            {
                *slot = None;
                return Err(format!("the daemon ended while it started: {status}"));
            }
            thread::sleep(Duration::from_millis(25));
        }
        Err(format!(
            "the daemon did not answer within {} s",
            START_WAIT.as_secs()
        ))
    }
}
