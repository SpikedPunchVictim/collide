//! The daemon's launchd start (ts-0058-ad). Two kinds of test:
//!
//! - Tests of the real `collide-daemon start` with a `launchctl` that is a script of the test's own (`COLLIDE_LAUNCHCTL`),
//!   so a refused bootstrap can be forced and its arguments read without launchd being touched.
//! - One test of the real launchd, which runs only when `COLLIDE_REAL_LAUNCHD=1` is set, on macOS, with a GUI session,
//!   and says why when it skips. Its job's label starts with `dev.collide.test.<pid>.<n>`, its root is the test's own
//!   folder, and a guard boots out that exact label and stops the daemon by the pid its own socket reported.
//!
//! Every process a test starts has a cleared environment with `HOME` and `COLLIDE_DAEMON_ROOT` in the test's folder, so
//! none of them touches the user's daemon files, and no test here calls the real `launchctl` by default.

mod common;

use collide_host::daemon::DaemonPaths;
use collide_host::launch::{
    LABEL_FILE, Launchctl, StartKind, StartRequest, SystemLaunchctl, ensure_daemon,
};
use common::{TempRoot, daemon_status, wait_until};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

const DAEMON: &str = env!("CARGO_BIN_EXE_collide-daemon");
const REAL_LAUNCHCTL: &str = "/bin/launchctl";
const USER: &str = "collide-test";

fn uid() -> u32 {
    // SAFETY: `getuid` takes no arguments and cannot fail.
    unsafe { libc::getuid() }
}

/// A login shell for the daemon's environment capture that records what it was started with, then prints the
/// environment as a login shell would. It records its parent (the daemon), and runs `python3` as a child to write what
/// `USER`, `HOME` and `PATH` that child saw.
fn recording_shell(root: &TempRoot) -> PathBuf {
    root.script(
        "recording-shell",
        &format!(
            "#!/bin/sh\necho $PPID > '{ppid}'\n/usr/bin/python3 -I -c 'import json, os, sys; \
             json.dump({{k: os.environ.get(k) for k in (\"USER\", \"HOME\", \"PATH\")}}, open(sys.argv[1], \"w\"))' '{seen}'\n\
             eval \"$4\"\n",
            ppid = root.join("shell-parent").display(),
            seen = root.join("python-saw.json").display(),
        ),
    )
}

/// `collide-daemon start` with an environment of the test's own and `launchctl` as given.
fn start(root: &TempRoot, shell: &Path, launchctl: Option<&Path>) -> Output {
    let mut command = Command::new(DAEMON);
    command
        .arg("start")
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("USER", USER)
        .env("HOME", root.path())
        .env("SHELL", shell)
        .env("COLLIDE_DAEMON_ROOT", root.path())
        .stdin(Stdio::null());
    if let Some(launchctl) = launchctl {
        command.env("COLLIDE_LAUNCHCTL", launchctl);
    }
    command.output().unwrap()
}

/// Stops the daemon that answers on a test's socket when the test ends: `SIGTERM`, a wait, then `SIGKILL`. The pid comes
/// from that daemon's own status.
struct DaemonGuard(DaemonPaths);

impl Drop for DaemonGuard {
    fn drop(&mut self) {
        let Some(pid) = daemon_status(&self.0).and_then(|s| s["pid"].as_i64()) else {
            return;
        };
        stop_pid(pid as i32);
    }
}

fn stop_pid(pid: i32) {
    // SAFETY: `kill` takes two integers; the pid is a daemon this test started, from its own socket.
    unsafe { libc::kill(pid, libc::SIGTERM) };
    let gone = |within: Duration| {
        let until = std::time::Instant::now() + within;
        while std::time::Instant::now() < until {
            // SAFETY: signal 0 sends nothing; it asks whether the pid exists.
            if unsafe { libc::kill(pid, 0) } != 0 {
                return true;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        false
    };
    if !gone(Duration::from_secs(5)) {
        // SAFETY: as above.
        unsafe { libc::kill(pid, libc::SIGKILL) };
        gone(Duration::from_secs(5));
    }
}

fn plists_in(run_dir: &Path) -> Vec<PathBuf> {
    fs::read_dir(run_dir)
        .map(|entries| {
            entries
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|e| e == "plist"))
                .collect()
        })
        .unwrap_or_default()
}

/// A `launchctl` that records its arguments, keeps a copy of the plist it was asked to load, and refuses, as it does
/// over SSH.
fn refusing_launchctl(root: &TempRoot) -> PathBuf {
    root.script(
        "fake-launchctl",
        &format!(
            "#!/bin/sh\necho \"$@\" >> '{log}'\n[ \"$1\" = bootstrap ] && cp \"$3\" '{seen}'\n\
             echo 'Bootstrap failed: 125: Domain does not support specified action' >&2\nexit 1\n",
            log = root.join("launchctl.log").display(),
            seen = root.join("seen.plist").display(),
        ),
    )
}

#[test]
fn a_refused_bootstrap_starts_the_daemon_with_setsid_and_warns() {
    let root = TempRoot::new();
    let shell = recording_shell(&root);
    let launchctl = refusing_launchctl(&root);
    let paths = DaemonPaths::under(root.path());
    let _guard = DaemonGuard(paths.clone());

    let output = start(&root, &shell, Some(&launchctl));

    // The run is not refused: the daemon started, and the start says so on stdout and warns on stderr.
    assert!(output.status.success(), "{output:?}");
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "started");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("started with setsid"), "{stderr}");
    assert!(stderr.contains("may still end with"), "{stderr}");
    assert!(
        stderr.contains("Domain does not support specified action"),
        "{stderr}"
    );
    assert!(daemon_status(&paths).is_some(), "the daemon answers");
}

#[test]
fn the_bootstrap_that_was_refused_named_the_gui_domain_and_a_plist_under_the_root() {
    let root = TempRoot::new();
    let shell = recording_shell(&root);
    let launchctl = refusing_launchctl(&root);
    let _guard = DaemonGuard(DaemonPaths::under(root.path()));

    let output = start(&root, &shell, Some(&launchctl));
    assert!(output.status.success(), "{output:?}");

    let run = fs::canonicalize(root.path()).unwrap().join("run");
    let calls = fs::read_to_string(root.join("launchctl.log")).unwrap();
    let lines: Vec<&str> = calls.lines().collect();
    assert_eq!(lines.len(), 1, "{calls}");
    let fields: Vec<&str> = lines[0].split(' ').collect();
    assert_eq!(fields[0], "bootstrap");
    assert_eq!(fields[1], format!("gui/{}", uid()));
    let plist = Path::new(fields[2]);
    assert!(plist.starts_with(&run), "{}", fields[2]);
    assert!(!fields[2].contains("LaunchAgents"));

    // The plist launchctl was handed had the daemon, the three variables and the forwarded root, and it is gone now.
    let seen = fs::read_to_string(root.join("seen.plist")).unwrap();
    let exe = fs::canonicalize(DAEMON).unwrap();
    assert!(seen.contains(exe.to_str().unwrap()), "{seen}");
    let canonical_root = fs::canonicalize(root.path()).unwrap();
    for (name, value) in [
        ("USER", USER),
        ("HOME", canonical_root.to_str().unwrap()),
        ("PATH", "/usr/bin:/bin"),
        ("COLLIDE_DAEMON_ROOT", canonical_root.to_str().unwrap()),
    ] {
        assert!(
            seen.contains(&format!("<key>{name}</key>\n    <string>{value}</string>")),
            "{name}: {seen}"
        );
    }
    assert!(!plist.exists(), "the plist is still there");
    assert!(plists_in(&run).is_empty());
    assert!(
        !run.join(LABEL_FILE).exists(),
        "a refused job is not recorded"
    );
}

#[test]
fn a_start_under_an_overridden_root_does_not_try_launchd_unless_asked() {
    let root = TempRoot::new();
    let shell = recording_shell(&root);
    let _guard = DaemonGuard(DaemonPaths::under(root.path()));

    let output = start(&root, &shell, None);
    assert!(output.status.success(), "{output:?}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("started with setsid"), "{stderr}");
    assert!(!stderr.contains("launchd said"), "{stderr}");
}

// ---- the real launchd ---------------------------------------------------------------------------------------------

static JOBS: AtomicU32 = AtomicU32::new(0);

/// Why the real launchd test cannot run here, or `None` when it can.
fn real_launchd_unavailable() -> Option<String> {
    if std::env::var("COLLIDE_REAL_LAUNCHD").as_deref() != Ok("1") {
        return Some("COLLIDE_REAL_LAUNCHD=1 is not set".to_string());
    }
    if !cfg!(target_os = "macos") {
        return Some("this is not macOS".to_string());
    }
    // A read-only question: does the user's GUI domain exist? Over SSH with nobody logged in, or in CI, it does not.
    let gui = Command::new(REAL_LAUNCHCTL)
        .args(["print", &format!("gui/{}", uid())])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    match gui {
        Ok(status) if status.success() => None,
        other => Some(format!("no GUI session for uid {} ({other:?})", uid())),
    }
}

/// The test's job: boots out the exact label the start recorded in the test's own run directory, and only when it has
/// the test's prefix, then stops the daemon by the pid its own socket reported.
struct RealJobGuard {
    paths: DaemonPaths,
    prefix: String,
    daemon_pid: Option<i32>,
}

impl RealJobGuard {
    fn label(&self) -> Option<String> {
        let label = fs::read_to_string(self.paths.socket_dir.join(LABEL_FILE)).ok()?;
        let label = label.trim().to_string();
        label.starts_with(&self.prefix).then_some(label)
    }
}

impl Drop for RealJobGuard {
    fn drop(&mut self) {
        let pid = self.daemon_pid.or_else(|| {
            daemon_status(&self.paths).and_then(|s| s["pid"].as_i64().map(|p| p as i32))
        });
        if let Some(label) = self.label() {
            let _ = SystemLaunchctl::new(REAL_LAUNCHCTL).bootout(&format!("gui/{}/{label}", uid()));
        }
        if let Some(pid) = pid {
            stop_pid(pid);
        }
    }
}

#[test]
fn a_real_launchd_job_starts_the_daemon_with_its_environment_and_a_python_child() {
    if let Some(reason) = real_launchd_unavailable() {
        println!("skipped: {reason}");
        return;
    }
    let root = TempRoot::new();
    let shell = recording_shell(&root);
    let paths = DaemonPaths::under(root.path());
    let n = JOBS.fetch_add(1, Ordering::SeqCst);
    let prefix = format!("dev.collide.test.{}.{n}", std::process::id());
    let mut guard = RealJobGuard {
        paths: paths.clone(),
        prefix: prefix.clone(),
        daemon_pid: None,
    };

    let mut request = StartRequest::new(PathBuf::from(DAEMON), &paths);
    request.label_prefix = prefix.clone();
    request.launchctl = Some(std::sync::Arc::new(SystemLaunchctl::new(REAL_LAUNCHCTL)));
    // The job gets exactly this environment (plus `USER`, `HOME` and `PATH` below), never the test's own.
    for (name, value) in [
        ("USER", USER),
        ("HOME", root.path().to_str().unwrap()),
        ("PATH", "/usr/bin:/bin"),
        ("SHELL", shell.to_str().unwrap()),
        ("COLLIDE_DAEMON_ROOT", root.path().to_str().unwrap()),
    ] {
        request.env.insert(name.to_string(), value.to_string());
    }
    let started = ensure_daemon(&request).expect("the launchd start");
    assert_eq!(started, StartKind::Launchd);

    // The job is ours and loaded, and its process is the daemon that answers.
    let label = guard
        .label()
        .expect("the start recorded a label of the test's prefix");
    let daemon_pid = daemon_status(&paths).expect("the daemon answers")["pid"]
        .as_i64()
        .unwrap() as i32;
    guard.daemon_pid = Some(daemon_pid);
    let launchctl = SystemLaunchctl::new(REAL_LAUNCHCTL);
    let target = format!("gui/{}/{label}", uid());
    assert_eq!(
        launchctl.process_of(&target).unwrap(),
        Some(daemon_pid as u32)
    );

    // The plist is gone now that the daemon answers.
    let run = paths.socket_dir.clone();
    assert!(plists_in(&run).is_empty(), "{:?}", plists_in(&run));

    // The daemon has USER, HOME and PATH, as the plist set them.
    let environment = Command::new("/bin/ps")
        .args(["-Eww", "-p", &daemon_pid.to_string(), "-o", "command="])
        .output()
        .unwrap();
    let environment = String::from_utf8_lossy(&environment.stdout).into_owned();
    println!("daemon environment (ps -E): {environment}");
    for expected in [
        format!("USER={USER}"),
        format!("HOME={}", fs::canonicalize(root.path()).unwrap().display()),
        "PATH=/usr/bin:/bin".to_string(),
    ] {
        assert!(
            environment.contains(&expected),
            "{expected} not in {environment}"
        );
    }

    // A python3 child ran under the daemon: the login shell of the environment capture ran it. Its parent is the daemon.
    wait_until("the python child's record", Duration::from_secs(10), || {
        root.join("python-saw.json").exists()
    });
    let parent: i32 = fs::read_to_string(root.join("shell-parent"))
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    assert_eq!(parent, daemon_pid, "the shell's parent is the daemon");
    let saw: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(root.join("python-saw.json")).unwrap()).unwrap();
    println!("python3 child saw: {saw}");
    assert_eq!(saw["USER"], USER);
    assert_eq!(saw["PATH"], "/usr/bin:/bin");
    assert_eq!(
        saw["HOME"],
        fs::canonicalize(root.path()).unwrap().to_str().unwrap()
    );

    // Ending the test's job leaves nothing loaded under its label.
    drop(guard);
    assert!(
        launchctl.process_of(&target).is_err(),
        "the job is still loaded after the guard booted it out"
    );
}
