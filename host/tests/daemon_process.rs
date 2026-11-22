//! The daemon as a process (ts-0056-ag): the `collide-daemon` binary started the way the CLI starts it, with `start`.
//! What is checked here and not in `daemon_socket.rs`: a start that finds a daemon already running, two starts at once
//! giving one daemon (the `flock`), a daemon that survives the terminal it was started from closing, the soft
//! open-file limit raised in the real process, a path that is too long failing fast from the real binary, and the
//! start's own errors (a lock that is held, a daemon that ends at once).
//!
//! Every daemon runs in a folder of its own under `/tmp` (`COLLIDE_DAEMON_ROOT`), with `HOME` and `SHELL` pointing
//! into that folder, so none of these tests touches the user's daemon files, a daemon of the user's, `launchctl` or
//! the user's login shell. A guard stops each daemon a test starts, by the pid that daemon reports on that test's own
//! socket, and never signals any other process.

mod common;

use collide_host::daemon::DaemonPaths;
use collide_host::launch::{StartError, StartRequest, ensure_daemon};
use common::TempRoot;
use serde_json::{Value, json};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};

const DAEMON: &str = env!("CARGO_BIN_EXE_collide-daemon");
const COLLIDE: &str = env!("CARGO_BIN_EXE_collide");
const PYTHON: &str = "/usr/bin/python3";

/// A login shell that prints its environment, so the daemon's capture never runs the user's own shell.
fn fake_shell(root: &TempRoot) -> String {
    root.script("fake-shell", "#!/bin/sh\neval \"$4\"\n")
        .to_str()
        .unwrap()
        .to_string()
}

/// The command that starts the daemon, or runs it, with an environment of the test's own.
fn daemon_command(root: &TempRoot, shell: &str) -> Command {
    let mut command = Command::new(DAEMON);
    command
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", root.path())
        .env("SHELL", shell)
        .env("COLLIDE_DAEMON_ROOT", root.path())
        .stdin(Stdio::null());
    command
}

fn start_command(root: &TempRoot, shell: &str) -> Command {
    let mut command = daemon_command(root, shell);
    command.arg("start");
    command
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

/// Asks the daemon on `paths` for its status, with the token from its token file. `None` when nothing answers.
fn status(paths: &DaemonPaths) -> Option<Value> {
    let stream = UnixStream::connect(&paths.socket).ok()?;
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok()?;
    let token = fs::read_to_string(&paths.token_file).ok()?;
    let mut writer = stream.try_clone().ok()?;
    let mut reader = BufReader::new(stream);
    let mut ask = |id: u64, method: &str, params: Value| -> Option<Value> {
        let request = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        writer.write_all(format!("{request}\n").as_bytes()).ok()?;
        let mut line = String::new();
        reader.read_line(&mut line).ok()?;
        serde_json::from_str(&line).ok()
    };
    ask(
        1,
        "initialize",
        json!({"protocol": 1, "token": token.trim()}),
    )?;
    Some(ask(2, "daemon.status", json!({}))?["result"].clone())
}

fn pid_of(paths: &DaemonPaths) -> i32 {
    status(paths).expect("the daemon answers")["pid"]
        .as_i64()
        .unwrap() as i32
}

fn alive(pid: i32) -> bool {
    // SAFETY: `kill` with signal 0 sends nothing; it only asks whether the pid exists.
    unsafe { libc::kill(pid, 0) == 0 }
}

/// Stops the daemon that answers on a test's socket when the test ends, however it ends: `SIGTERM`, a wait for it to
/// go, then `SIGKILL`. The pid comes from that daemon's own status, so it is a daemon this test started.
struct DaemonGuard {
    paths: DaemonPaths,
    pid: Option<i32>,
}

impl DaemonGuard {
    fn new(root: &TempRoot) -> DaemonGuard {
        DaemonGuard::at(DaemonPaths::under(root.path()))
    }

    /// A guard for a daemon whose root is not the test's own folder, but below it.
    fn at(paths: DaemonPaths) -> DaemonGuard {
        DaemonGuard { paths, pid: None }
    }

    fn paths(&self) -> &DaemonPaths {
        &self.paths
    }

    /// The daemon's pid, remembered so the guard can stop it even when it later stops answering.
    fn pid(&mut self) -> i32 {
        let pid = pid_of(&self.paths);
        self.pid = Some(pid);
        pid
    }

    fn wait_gone(pid: i32, within: Duration) -> bool {
        let until = Instant::now() + within;
        while Instant::now() < until {
            if !alive(pid) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        !alive(pid)
    }
}

impl Drop for DaemonGuard {
    fn drop(&mut self) {
        let pid = self.pid.or_else(|| {
            status(&self.paths)
                .and_then(|s| s["pid"].as_i64())
                .map(|p| p as i32)
        });
        let Some(pid) = pid else { return };
        // SAFETY: `kill` takes two integers; the pid is a daemon this test started, from its own socket.
        unsafe { libc::kill(pid, libc::SIGTERM) };
        if !DaemonGuard::wait_gone(pid, Duration::from_secs(5)) {
            // SAFETY: as above.
            unsafe { libc::kill(pid, libc::SIGKILL) };
            DaemonGuard::wait_gone(pid, Duration::from_secs(5));
        }
    }
}

#[test]
fn a_start_makes_a_daemon_and_a_second_start_finds_it_running() {
    let root = TempRoot::new();
    let shell = fake_shell(&root);
    let mut guard = DaemonGuard::new(&root);

    let first = start_command(&root, &shell).output().unwrap();
    assert!(first.status.success(), "{first:?}");
    assert_eq!(stdout_of(&first), "started");
    let pid = guard.pid();
    assert_eq!(pid_of(guard.paths()), pid);

    let second = start_command(&root, &shell).output().unwrap();
    assert!(second.status.success(), "{second:?}");
    assert_eq!(stdout_of(&second), "already running");
    assert_eq!(pid_of(guard.paths()), pid, "a second daemon took over");

    // The files a start leaves: the token at 0600, the lock file holding the socket path.
    let paths = guard.paths().clone();
    let token = fs::metadata(&paths.token_file).unwrap();
    assert_eq!(token.permissions().mode() & 0o7777, 0o600);
    assert_eq!(
        fs::read_to_string(&paths.lock_file).unwrap(),
        paths.socket.to_str().unwrap()
    );
    // The launcher does not wait on the daemon, and the daemon's output went to its log.
    assert!(
        fs::read_to_string(&paths.log_file)
            .unwrap()
            .contains("listening on")
    );
}

#[test]
fn two_starts_at_once_give_one_daemon() {
    // Three rounds, each in a folder of its own: a start that raced would show in one of them.
    for round in 0..3 {
        let root = TempRoot::new();
        let shell = fake_shell(&root);
        let mut guard = DaemonGuard::new(&root);

        let a = start_command(&root, &shell)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let b = start_command(&root, &shell)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let (a, b) = (a.wait_with_output().unwrap(), b.wait_with_output().unwrap());
        assert!(
            a.status.success() && b.status.success(),
            "round {round}: {a:?} {b:?}"
        );
        let mut said = [stdout_of(&a), stdout_of(&b)];
        said.sort();
        assert_eq!(said, ["already running", "started"], "round {round}");

        guard.pid();
        // One daemon: a second one would have failed to bind, and said so in the log.
        let log = fs::read_to_string(&guard.paths().log_file).unwrap();
        assert_eq!(
            log.matches("listening on").count(),
            1,
            "round {round}: {log}"
        );
        assert!(!log.contains("cannot start"), "round {round}: {log}");
    }
}

/// Starts the daemon from a child process in a session and on a terminal of its own, then hangs the terminal up.
/// Prints the session's id, then the signal that ended the session's leader.
const TERMINAL_PY: &str = "import os, pty, sys, subprocess, time\n\
pid, fd = pty.fork()\n\
if pid == 0:\n\
\x20   r = subprocess.run([sys.argv[1], 'start'])\n\
\x20   print('launched', r.returncode, flush=True)\n\
\x20   time.sleep(120)\n\
\x20   os._exit(0)\n\
buf = b''\n\
while b'launched' not in buf:\n\
\x20   buf += os.read(fd, 1024)\n\
print('session', pid, flush=True)\n\
os.close(fd)\n\
_, status = os.waitpid(pid, 0)\n\
print('hangup', os.WTERMSIG(status) if os.WIFSIGNALED(status) else 0, flush=True)\n";

#[test]
fn a_daemon_started_from_a_terminal_survives_the_terminal_closing() {
    let root = TempRoot::new();
    let shell = fake_shell(&root);
    let mut guard = DaemonGuard::new(&root);

    let mut command = Command::new(PYTHON);
    command
        .args(["-I", "-c", TERMINAL_PY, DAEMON])
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", root.path())
        .env("SHELL", &shell)
        .env("COLLIDE_DAEMON_ROOT", root.path())
        .stdin(Stdio::null());
    let output = command.output().unwrap();
    let text = String::from_utf8_lossy(&output.stdout).into_owned();
    assert!(
        output.status.success(),
        "{text} {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let session: i32 = text
        .lines()
        .find_map(|l| l.strip_prefix("session "))
        .expect("the session's id")
        .parse()
        .unwrap();
    // The terminal's hangup did reach the session: its leader ended by SIGHUP.
    assert!(text.contains(&format!("hangup {}", libc::SIGHUP)), "{text}");

    // The daemon answers, and it is in a session of its own, not the terminal's.
    let pid = guard.pid();
    assert!(status(guard.paths()).is_some());
    // SAFETY: `getsid` takes a pid and reads the kernel's table.
    let daemon_session = unsafe { libc::getsid(pid) };
    assert_ne!(
        daemon_session, session,
        "the daemon is in the terminal's session"
    );
    assert_eq!(
        daemon_session, pid,
        "the daemon is its own session's leader"
    );
}

#[test]
fn the_daemon_raises_its_soft_open_file_limit_when_it_starts() {
    let root = TempRoot::new();
    let shell = fake_shell(&root);
    let log = root.join("stderr.log");
    // `ulimit -S` lowers only the soft limit, as launchd's 256 is; the daemon is then started in place by `exec`.
    let mut child = Command::new("/bin/sh")
        .args(["-c", &format!("ulimit -S -n 256; exec '{DAEMON}'")])
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", root.path())
        .env("SHELL", &shell)
        .env("COLLIDE_DAEMON_ROOT", root.path())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(fs::File::create(&log).unwrap())
        .spawn()
        .unwrap();
    let _stop = StopChild(&mut child);
    let until = Instant::now() + Duration::from_secs(10);
    let text = loop {
        let text = fs::read_to_string(&log).unwrap_or_default();
        if text.contains("open-file") || Instant::now() > until {
            break text;
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    assert!(
        text.contains("open-file soft limit 4096 (was 256)"),
        "{text}"
    );
}

/// Stops a child this test spawned, and reaps it, when the test ends.
struct StopChild<'a>(&'a mut Child);

impl Drop for StopChild<'_> {
    fn drop(&mut self) {
        // SAFETY: `kill` takes two integers; the pid is this test's own child, not yet waited for.
        unsafe { libc::kill(self.0.id() as i32, libc::SIGTERM) };
        let until = Instant::now() + Duration::from_secs(5);
        while Instant::now() < until {
            if self.0.try_wait().ok().flatten().is_some() {
                return;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn a_sigterm_stops_the_daemon_and_removes_its_socket() {
    let root = TempRoot::new();
    let shell = fake_shell(&root);
    let mut child = daemon_command(&root, &shell)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let _stop = StopChild(&mut child);
    let paths = DaemonPaths::under(root.path());
    let until = Instant::now() + Duration::from_secs(10);
    while status(&paths).is_none() && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(status(&paths).is_some());
    // SAFETY: `kill` takes two integers; the pid is this test's own child.
    unsafe { libc::kill(_stop.0.id() as i32, libc::SIGTERM) };
    let ended = loop {
        if let Some(status) = _stop.0.try_wait().unwrap() {
            break status;
        }
        assert!(
            Instant::now() < until + Duration::from_secs(10),
            "the daemon did not stop"
        );
        std::thread::sleep(Duration::from_millis(20));
    };
    assert!(ended.success(), "{ended:?}");
    assert!(!paths.socket.exists());
}

#[test]
fn a_socket_path_that_is_too_long_fails_fast_from_the_real_binary() {
    let root = TempRoot::new();
    let shell = fake_shell(&root);
    let long = root.path().join("x".repeat(100));
    let began = Instant::now();
    let output = Command::new(DAEMON)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("SHELL", &shell)
        .env("COLLIDE_DAEMON_ROOT", &long)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(began.elapsed() < Duration::from_secs(5));
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(long.join("run").join("d.sock").to_str().unwrap()),
        "{stderr}"
    );
    assert!(stderr.contains("103"), "{stderr}");
}

#[test]
fn a_start_whose_daemon_cannot_start_says_why_from_the_log() {
    let root = TempRoot::new();
    let shell = fake_shell(&root);
    let long = root.path().join("y".repeat(100));
    let output = Command::new(DAEMON)
        .arg("start")
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("SHELL", &shell)
        .env("COLLIDE_DAEMON_ROOT", &long)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("exit status 1") && stderr.contains("103"),
        "{stderr}"
    );
    // The lock file's folder was made by the start, below the long root; nothing else of the daemon's.
    assert!(!long.join("data").join("store").exists());
}

/// A stand-in for the daemon that ends at once, as a daemon with a broken install would.
fn failing_daemon(root: &TempRoot) -> StartRequest {
    let exe = root.script(
        "failing-daemon",
        "#!/bin/sh\necho 'boom: cannot start' >&2\nexit 3\n",
    );
    let paths = DaemonPaths::under(root.path());
    let mut request = StartRequest::new(exe, &paths);
    request.wait = Duration::from_secs(5);
    request
}

#[test]
fn a_daemon_that_ends_before_its_socket_answers_is_an_error_with_its_log() {
    let root = TempRoot::new();
    let request = failing_daemon(&root);
    let error = ensure_daemon(&request).unwrap_err();
    let StartError::Exited {
        status, log_tail, ..
    } = &error
    else {
        panic!("expected Exited, got {error}");
    };
    assert_eq!(status, "exit status 3");
    assert!(log_tail.contains("boom: cannot start"), "{log_tail}");
    assert!(
        error
            .to_string()
            .contains(request.log_file.to_str().unwrap())
    );
}

#[test]
fn a_start_waits_for_the_lock_and_gives_up_when_it_is_not_released() {
    let root = TempRoot::new();
    let mut request = failing_daemon(&root);
    request.wait = Duration::from_millis(300);
    fs::create_dir_all(request.lock_file.parent().unwrap()).unwrap();
    let held = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&request.lock_file)
        .unwrap();
    // SAFETY: `flock` takes a descriptor the test holds open and an operation.
    assert_eq!(
        unsafe { libc::flock(held.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) },
        0
    );

    let began = Instant::now();
    let error = ensure_daemon(&request).unwrap_err();
    assert!(matches!(error, StartError::LockTimeout { .. }), "{error}");
    assert!(began.elapsed() >= Duration::from_millis(300));
    assert!(
        error
            .to_string()
            .contains(request.lock_file.to_str().unwrap())
    );
    // Nothing was started behind the held lock.
    assert!(!Path::new(&request.log_file).exists());
}

#[test]
fn a_start_that_waited_for_the_lock_goes_ahead_when_it_is_released() {
    let root = TempRoot::new();
    let request = failing_daemon(&root);
    fs::create_dir_all(request.lock_file.parent().unwrap()).unwrap();
    let held = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&request.lock_file)
        .unwrap();
    // SAFETY: `flock` takes a descriptor the test holds open and an operation.
    assert_eq!(
        unsafe { libc::flock(held.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) },
        0
    );
    let release = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(300));
        drop(held);
    });
    // Once the lock is free the start runs, and the failing stand-in ends: the start did not stay stuck on the lock.
    let error = ensure_daemon(&request).unwrap_err();
    release.join().unwrap();
    assert!(matches!(error, StartError::Exited { .. }), "{error}");
}

// ---- a HOME that names no folder (ts-0164-pa) ---------------------------------------------------------------------

/// A home that was deleted: a name below the test's folder that is never made. The test's own folder is its parent, so
/// nothing of the machine's is read or written.
fn gone_home(root: &TempRoot) -> PathBuf {
    root.join("gone")
}

/// `program` with a cleared environment whose `HOME` is `home` and whose daemon root is `daemon_root`.
fn command_with_home(program: &str, root: &TempRoot, home: &Path, daemon_root: &Path) -> Command {
    let mut command = Command::new(program);
    command
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", home)
        .env("SHELL", fake_shell(root))
        .env("COLLIDE_DAEMON_ROOT", daemon_root)
        .stdin(Stdio::null());
    command
}

/// Runs `command` and waits for it to end for up to `deadline`. A command that does not end is killed and reaped, and
/// the test fails: a daemon that served instead of refusing would otherwise run for as long as the test run does.
fn output_within(command: &mut Command, deadline: Duration) -> Output {
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let until = Instant::now() + deadline;
    while child.try_wait().unwrap().is_none() {
        if Instant::now() >= until {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("the command did not end within {deadline:?}: it served instead of refusing");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    child.wait_with_output().unwrap()
}

fn assert_refused_for_home(output: &Output, home: &Path) {
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains(home.to_str().unwrap()), "{stderr}");
    assert!(stderr.contains("HOME"), "{stderr}");
}

#[test]
fn a_daemon_whose_home_names_no_folder_refuses_and_makes_nothing() {
    let root = TempRoot::new();
    let home = gone_home(&root);
    let output = output_within(
        &mut command_with_home(DAEMON, &root, &home, root.path()),
        Duration::from_secs(30),
    );
    assert_refused_for_home(&output, &home);
    assert!(!home.exists(), "the refusal made HOME");
    assert!(
        !root.join("run").exists(),
        "the refusal made the root's run folder"
    );
    assert!(
        !root.join("data").exists(),
        "the refusal made the root's data folder"
    );
}

#[test]
fn a_start_below_a_home_that_names_no_folder_refuses_and_does_not_make_it() {
    // The owner's case: `COLLIDE_DAEMON_ROOT` is below the deleted `HOME`, so making the root makes the home.
    let root = TempRoot::new();
    let home = gone_home(&root);
    let daemon_root = home.join("daemon");
    // A daemon that did start would be below the deleted home; the guard ends it, by the pid it reports.
    let _guard = DaemonGuard::at(DaemonPaths::under(&daemon_root));
    let output = command_with_home(DAEMON, &root, &home, &daemon_root)
        .arg("start")
        .output()
        .unwrap();
    assert_refused_for_home(&output, &home);
    assert!(!home.exists(), "the refusal made HOME");
}

#[test]
fn the_cli_prints_the_refusal_when_its_home_names_no_folder() {
    let root = TempRoot::new();
    let home = gone_home(&root);
    let daemon_root = home.join("daemon");
    // A daemon that did start would be below the deleted home; the guard ends it, by the pid it reports.
    let _guard = DaemonGuard::at(DaemonPaths::under(&daemon_root));
    let mut command = command_with_home(COLLIDE, &root, &home, &daemon_root);
    command.args(["stop", "no-such-run"]);
    let output = common::detached::Detached::start(&root, &command).wait();
    assert_refused_for_home(&output, &home);
    assert!(!home.exists(), "the CLI made HOME");
}

#[test]
fn a_start_that_names_a_home_in_its_request_is_refused_when_it_is_gone() {
    let root = TempRoot::new();
    let home = gone_home(&root);
    let daemon_root = home.join("daemon");
    let mut request = failing_daemon(&root);
    let paths = DaemonPaths::under(&daemon_root);
    request.lock_file = paths.lock_file.clone();
    request.socket = paths.socket.clone();
    request.log_file = paths.log_file.clone();
    request
        .env
        .insert("HOME".to_string(), home.display().to_string());
    let error = ensure_daemon(&request).unwrap_err().to_string();
    assert!(error.contains(home.to_str().unwrap()), "{error}");
    assert!(error.contains("HOME"), "{error}");
    assert!(!home.exists(), "the start made HOME");
}

#[test]
fn a_root_that_names_no_folder_is_still_made_by_a_start() {
    let root = TempRoot::new();
    let daemon_root = root.join("fresh").join("daemon");
    let paths = DaemonPaths::under(&daemon_root);
    let mut guard = DaemonGuard::at(paths.clone());
    assert!(!daemon_root.exists());
    let output = command_with_home(DAEMON, &root, root.path(), &daemon_root)
        .arg("start")
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert_eq!(stdout_of(&output), "started");
    guard.pid();
    assert!(paths.token_file.is_file());
}
