//! `collide stop` through a real daemon process (ts-0059-xs): the CLI starts `collide-daemon`, which adopts a shim that
//! the test started, and stops the run the shim's child belongs to. Also: a daemon the CLI starts from inside a run has
//! the run's variables stripped, a run's agent may stop only its own run, and the usage and the unknown run.
//!
//! SAFETY of these tests. The daemon is a separate process with the real process table, so these tests cannot hand it
//! a guard. What keeps it to the test's own processes:
//!
//! - Every run id is unique to the test (`test-<pid>-<nanos>-<n>`), so no process on the machine carries it.
//! - Before the first `collide stop`, the test computes the kill set the daemon will compute (same marker, same roots)
//!   with a read-only scan and asserts that every pid in it is one the test recorded, or is below one
//!   (`assert_the_kill_set_is_ours`); if not, it panics with nothing signalled.
//! - The stop reaches only that set, found by the run's marker and the recorded child.
//! - Every process started has a cleared environment with `HOME` and `COLLIDE_DAEMON_ROOT` in the test's own folder
//!   under `/tmp`, and a guard kills what is left, by pid and start time, and the daemon by the pid it reports.

mod common;

use collide_host::daemon::DaemonPaths;
use collide_host::shim::{ChildStdio, ShimSpec, files};
use collide_host::stop::{Root, StopRequest};
use collide_host::sysproc::{self, EnvRead};
use common::procs::{
    Guarded, HOSTILE, Owned, SLEEP_ORPHANS, assert_the_kill_set_is_ours, listed_by_ps, read_pids,
    unique_run_id,
};
use common::{PYTHON, ShimProc, TempRoot, daemon_status, wait_until};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::Arc;
use std::time::Duration;

const COLLIDE: &str = env!("CARGO_BIN_EXE_collide");
// Cargo builds the daemon for these tests only when a test names it.
const _DAEMON: &str = env!("CARGO_BIN_EXE_collide-daemon");

/// A login shell that prints its environment, so the daemon's capture never runs the user's own shell.
fn fake_shell(root: &TempRoot) -> String {
    root.script("fake-shell", "#!/bin/sh\neval \"$4\"\n")
        .to_str()
        .unwrap()
        .to_string()
}

/// `collide <args>` with an environment of the test's own: nothing of the caller's, and `extra` on top. It is not a
/// child of this process (see `common::detached`), so a test run inside a run of Collide is not its run's agent.
fn collide(root: &TempRoot, args: &[&str], extra: &[(&str, &str)]) -> Output {
    let mut command = Command::new(COLLIDE);
    command
        .args(args)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", root.path())
        .env("SHELL", fake_shell(root))
        .env("COLLIDE_DAEMON_ROOT", root.path())
        .stdin(Stdio::null());
    for (name, value) in extra {
        command.env(name, value);
    }
    common::detached::Detached::start(root, &command).wait()
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// Stops the daemon the CLI started, by the pid it reports on this test's own socket, and kills what the test recorded.
struct Guards {
    paths: DaemonPaths,
    owned: Arc<Owned>,
}

impl Drop for Guards {
    fn drop(&mut self) {
        self.owned.kill_all();
        let pid = daemon_status(&self.paths).and_then(|s| s["pid"].as_i64());
        if let Some(pid) = pid {
            // SAFETY: `kill` takes two integers; the pid is the one the daemon on this test's own socket reports.
            unsafe { libc::kill(pid as libc::pid_t, libc::SIGTERM) };
            wait_until("the daemon ends", Duration::from_secs(10), || {
                // SAFETY: signal 0 sends nothing.
                unsafe { libc::kill(pid as libc::pid_t, 0) != 0 }
            });
        }
    }
}

/// A shim with `script` as its child, in a run directory under the test's runs directory, carrying `run` as its run id.
/// Returns the shim, the pid file the script writes, and the child's root.
fn start_shim(root: &TempRoot, run: &str, script: &str) -> (ShimProc, PathBuf) {
    let paths = DaemonPaths::under(root.path());
    let dir = paths.runs_dir.join(run).join("p1");
    fs::create_dir_all(&dir).unwrap();
    let script_path = root.join(&format!("{run}.py"));
    fs::write(&script_path, script).unwrap();
    let pidfile = root.join(&format!("{run}.pids"));
    fs::write(&pidfile, "").unwrap();
    let spec = ShimSpec {
        program: PYTHON.into(),
        args: vec![
            script_path.to_str().unwrap().into(),
            pidfile.to_str().unwrap().into(),
        ],
        env: BTreeMap::from([
            ("PATH".to_string(), "/usr/bin:/bin".to_string()),
            ("COLLIDE_RUN".to_string(), run.to_string()),
        ]),
        cwd: root.path().to_path_buf(),
        run_dir: dir.clone(),
        stdio: ChildStdio::Pipes,
        daemon: None,
    };
    fs::write(dir.join(files::SPEC), spec.to_json().to_string()).unwrap();
    (ShimProc::start(root, &dir), pidfile)
}

/// Waits for the child's identity and the fixture's pids, records them as the test's, and returns the fixture's pids
/// and the child's root.
fn record(shim: &ShimProc, pidfile: &Path, tags: &[&str], owned: &Owned) -> (Vec<u32>, Root) {
    let child_file = shim.dir.join(files::CHILD);
    wait_until("child.json", Duration::from_secs(20), || {
        fs::read_to_string(&child_file).is_ok_and(|t| t.contains("started_unix_us"))
    });
    wait_until("the fixture's pids", Duration::from_secs(20), || {
        read_pids(&pidfile.to_path_buf(), tags).is_some()
    });
    wait_until("shim.json", Duration::from_secs(20), || {
        shim.dir.join(files::SHIM).exists()
    });
    let identity = collide_host::phases::read_identity(&child_file).expect("child.json reads");
    owned.add(identity.pid);
    let pids = read_pids(&pidfile.to_path_buf(), tags).unwrap();
    for pid in pids.values() {
        owned.add(*pid);
    }
    (
        pids.values().copied().collect(),
        Root {
            pid: identity.pid,
            started_us: identity.started_unix_us,
        },
    )
}

/// Panics, with nothing signalled, unless the daemon's kill set for `run` is the test's own.
fn assert_the_daemons_kill_set_is_ours(
    root: &TempRoot,
    owned: &Arc<Owned>,
    run: &str,
    child: Root,
) {
    let table = Guarded::under(
        DaemonPaths::under(root.path()).runs_dir,
        vec![owned.clone()],
    );
    let mut request = StopRequest::new(run);
    request.roots = vec![child];
    assert_the_kill_set_is_ours(&table, &request);
}

#[test]
fn collide_stop_stops_a_run_through_the_daemon_and_prints_the_survivor_report() {
    let root = TempRoot::new();
    let owned = Arc::new(Owned::default());
    let _guards = Guards {
        paths: DaemonPaths::under(root.path()),
        owned: owned.clone(),
    };
    let run = unique_run_id();
    let (shim, pidfile) = start_shim(&root, &run, HOSTILE);
    let (pids, child) = record(&shim, &pidfile, &["root", "setsid", "orphan"], &owned);
    assert_the_daemons_kill_set_is_ours(&root, &owned, &run, child);

    let output = collide(&root, &["stop", &run, "--grace", "1"], &[]);
    let stdout = text(&output.stdout);
    assert_eq!(
        output.status.code(),
        Some(0),
        "{stdout} {}",
        text(&output.stderr)
    );
    assert!(
        stdout.contains(&format!("run {run}: stopped; 3 process(es) signalled")),
        "{stdout}"
    );
    // Another test's orphans, or any other process that looks like one, may be listed as suspects: the report is
    // machine-wide, and it only reports them. What it must not list is anything of this run's tree.
    assert!(stdout.contains("survivors: "), "{stdout}");
    for pid in &pids {
        assert!(
            !stdout
                .lines()
                .any(|l| l.trim_start().starts_with(&format!("{pid} "))),
            "{pid} survived: {stdout}"
        );
    }
    wait_until("ps lists none of the tree", Duration::from_secs(20), || {
        listed_by_ps(&pids).is_empty()
    });
}

#[test]
fn collide_stop_lists_an_unrecorded_double_forked_orphan_as_a_survivor() {
    let root = TempRoot::new();
    let owned = Arc::new(Owned::default());
    let _guards = Guards {
        paths: DaemonPaths::under(root.path()),
        owned: owned.clone(),
    };
    let run = unique_run_id();
    let (shim, pidfile) = start_shim(&root, &run, SLEEP_ORPHANS);
    let (pids, child) = record(&shim, &pidfile, &["root", "orphan_a", "orphan_b"], &owned);
    assert_the_daemons_kill_set_is_ours(&root, &owned, &run, child);

    let output = collide(&root, &["stop", &run, "--grace", "1"], &[]);
    let stdout = text(&output.stdout);
    assert_eq!(
        output.status.code(),
        Some(0),
        "{stdout} {}",
        text(&output.stderr)
    );
    let orphans = read_pids(&pidfile, &["orphan_a", "orphan_b"]).unwrap();
    for pid in [orphans["orphan_a"], orphans["orphan_b"]].iter() {
        assert!(
            stdout.contains(&format!("{pid} /bin/sleep (suspect)")),
            "{pid} in {stdout}"
        );
        assert!(
            listed_by_ps(&[*pid]).contains(pid),
            "a suspect is reported and not signalled"
        );
    }
    // The root of the tree is gone.
    let root_pid = read_pids(&pidfile, &["root"]).unwrap()["root"];
    wait_until("the root is gone", Duration::from_secs(20), || {
        listed_by_ps(&[root_pid]).is_empty()
    });
    let _ = pids;
}

#[test]
fn collide_stop_of_a_run_the_daemon_does_not_know_exits_3() {
    let root = TempRoot::new();
    let _guards = Guards {
        paths: DaemonPaths::under(root.path()),
        owned: Arc::new(Owned::default()),
    };
    let run = unique_run_id();
    let output = collide(&root, &["stop", &run], &[]);
    assert_eq!(output.status.code(), Some(3), "{}", text(&output.stderr));
    assert!(
        text(&output.stderr).contains("no run"),
        "{}",
        text(&output.stderr)
    );
}

#[test]
fn collide_stop_with_a_bad_command_line_exits_2_and_starts_no_daemon() {
    let root = TempRoot::new();
    for args in [
        &["stop"][..],
        &["stop", "a", "b"],
        &["stop", "a", "--grace"],
    ] {
        let output = collide(&root, args, &[]);
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        assert!(text(&output.stderr).contains("usage: collide stop"));
    }
    assert!(daemon_status(&DaemonPaths::under(root.path())).is_none());
}

#[test]
fn a_daemon_the_cli_starts_from_inside_a_run_has_the_runs_variables_stripped() {
    let root = TempRoot::new();
    let paths = DaemonPaths::under(root.path());
    let _guards = Guards {
        paths: paths.clone(),
        owned: Arc::new(Owned::default()),
    };
    let run = unique_run_id();
    let other = unique_run_id();
    // A run's agent asks to stop another run: the daemon refuses it, so nothing is signalled, and the CLI has started
    // the daemon from inside the run.
    let output = collide(
        &root,
        &["stop", &other],
        &[
            ("COLLIDE_RUN", &run),
            ("COLLIDE_RUN_START", "1"),
            ("COLLIDE_SPAN", "span-1"),
        ],
    );
    assert_eq!(output.status.code(), Some(1), "{}", text(&output.stderr));
    assert!(
        text(&output.stderr).contains("may not stop"),
        "{}",
        text(&output.stderr)
    );
    let pid = daemon_status(&paths).expect("the daemon runs")["pid"]
        .as_u64()
        .unwrap() as u32;
    assert_eq!(
        sysproc::env_vars_of(
            pid,
            &[
                "COLLIDE_RUN",
                "COLLIDE_RUN_START",
                "COLLIDE_SPAN",
                "COLLIDE_RUNTIME"
            ]
        ),
        EnvRead::Read(vec![None, None, None, Some("1".to_string())])
    );
}
