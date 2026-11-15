//! Helpers the host's integration tests share: the repo root, scratch folders under `target/tmp`, and the built-in
//! packs bundle, built once per test binary by the repo's build script into a scratch folder (so no test depends on
//! a staged copy and none skips). The bundle is read-only, so it is unlocked and deleted when the last user is done,
//! also when a test panics: `rm -rf target/tmp` and `cargo clean` must work.

#![allow(dead_code)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};

pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("host/ sits directly under the repo root")
        .to_path_buf()
}

pub fn scratch(name: &str) -> PathBuf {
    Path::new(env!("CARGO_TARGET_TMPDIR")).join(name)
}

/// The build script rebuilds the SDK's `dist/` in place, so two builds must not overlap.
static BUILD_LOCK: Mutex<()> = Mutex::new(());

/// Runs `pnpm build:packs --out <out>` from the repo root. A leftover read-only `out` from an earlier run is the
/// build's own to replace.
pub fn build_packs(out: &Path) {
    let _guard = BUILD_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let status = Command::new("node")
        .args(["scripts/build-packs.ts", "--out"])
        .arg(out)
        .current_dir(repo_root())
        .status()
        .expect("node should run the build script");
    assert!(status.success(), "build-packs exited with {status}");
}

/// Takes the write bits back so that a read-only tree can be deleted (`rm -rf target/tmp`, `cargo clean`).
pub fn make_writable(path: &Path) {
    let Ok(meta) = fs::symlink_metadata(path) else {
        return;
    };
    let mut permissions = meta.permissions();
    permissions.set_mode(permissions.mode() | 0o200);
    fs::set_permissions(path, permissions).unwrap();
    if meta.is_dir() {
        for entry in fs::read_dir(path).unwrap() {
            make_writable(&entry.unwrap().path());
        }
    }
}

pub fn remove_read_only(path: &Path) {
    make_writable(path);
    let _ = fs::remove_dir_all(path);
}

/// A built bundle that is unlocked and deleted when the guard drops, also when the test panics.
pub struct Scratch(pub PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        remove_read_only(&self.0);
    }
}

impl std::ops::Deref for Scratch {
    type Target = Path;
    fn deref(&self) -> &Path {
        &self.0
    }
}

/// The shared bundle: built by the first test that asks, and removed when the last one is done.
static SHARED_USERS: Mutex<usize> = Mutex::new(0);

pub struct Bundle(pub PathBuf);

/// The bundle in the scratch folder `name`. Each test binary uses one name.
pub fn bundle(name: &str) -> Bundle {
    let out = scratch(name);
    let mut users = SHARED_USERS.lock().unwrap_or_else(|e| e.into_inner());
    if *users == 0 {
        build_packs(&out);
    }
    *users += 1;
    Bundle(out)
}

impl Drop for Bundle {
    fn drop(&mut self) {
        let mut users = SHARED_USERS.lock().unwrap_or_else(|e| e.into_inner());
        *users -= 1;
        if *users == 0 {
            remove_read_only(&self.0);
        }
    }
}

impl std::ops::Deref for Bundle {
    type Target = Path;
    fn deref(&self) -> &Path {
        &self.0
    }
}

static TEMP_COUNTER: AtomicU32 = AtomicU32::new(0);

/// A folder `/tmp/cdt-<pid>-<n>` that one test makes and owns, short enough for a socket path (limited to 103 bytes;
/// `target/tmp` is longer). The daemon tests put every path the daemon uses below it, so none touches the user's own
/// daemon files. `Drop` makes the folders below it writable again and removes the folder by its own path.
pub struct TempRoot(PathBuf);

impl TempRoot {
    pub fn new() -> TempRoot {
        let n = TEMP_COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = PathBuf::from(format!("/tmp/cdt-{}-{n}", std::process::id()));
        fs::create_dir(&dir).unwrap();
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).unwrap();
        TempRoot(dir)
    }

    pub fn path(&self) -> &Path {
        &self.0
    }

    pub fn join(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }

    /// Writes an executable script (mode 0755) and returns its path.
    pub fn script(&self, name: &str, body: &str) -> PathBuf {
        let path = self.0.join(name);
        fs::write(&path, body).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        path
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        assert!(
            self.0.to_str().unwrap().starts_with("/tmp/cdt-"),
            "a test folder"
        );
        open_up(&self.0);
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Gives back owner access to the folders below `dir`, so a test that made one unreadable still cleans up.
fn open_up(dir: &Path) {
    let Ok(meta) = fs::symlink_metadata(dir) else {
        return;
    };
    if !meta.is_dir() {
        return;
    }
    let _ = fs::set_permissions(dir, fs::Permissions::from_mode(0o700));
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            open_up(&entry.path());
        }
    }
}

// ---- processes the link tests start ------------------------------------------------------------------------------

pub const PYTHON: &str = "/usr/bin/python3";

/// Polls `done` every 20 ms for up to `within`, and panics naming `what` when it never holds.
pub fn wait_until(what: &str, within: std::time::Duration, mut done: impl FnMut() -> bool) {
    let until = std::time::Instant::now() + within;
    while std::time::Instant::now() < until {
        if done() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(done(), "timed out after {within:?} waiting for: {what}");
}

/// Whether the pid names a live process (a zombie counts: it has not been reaped).
pub fn pid_alive(pid: u32) -> bool {
    // SAFETY: signal 0 sends nothing; it only asks whether the pid exists.
    unsafe { libc::kill(pid as libc::pid_t, 0) == 0 }
}

/// Sends `signal` to `pid` only when the process there started at `started_us`, so a pid that was reused is never hit.
pub fn signal_if_same(pid: u32, started_us: u64, signal: i32) {
    if collide_host::sysproc::start_time_us(pid) == Some(started_us) {
        // SAFETY: `kill` takes two integers; the pid was checked against the start time this test recorded.
        unsafe { libc::kill(pid as libc::pid_t, signal) };
    }
}

/// Stops the child of a shim that a test started, by the identity file the shim wrote (`child.json`): its process
/// group first (the child is a session leader), then the pid, each only if the start time still matches.
pub fn kill_child_of(run_dir: &Path) {
    let Ok(text) = fs::read_to_string(run_dir.join("child.json")) else {
        return;
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
        return;
    };
    let Some(identity) = collide_host::shim::ChildIdentity::from_json(&value) else {
        return;
    };
    if collide_host::sysproc::start_time_us(identity.pid) == Some(identity.started_unix_us) {
        // SAFETY: the pid's start time matched the one the shim recorded, so it is that child, and it is a session
        // leader (the shim made it one), so its group id is its pid.
        unsafe {
            libc::kill(-(identity.pid as libc::pid_t), libc::SIGKILL);
            libc::kill(identity.pid as libc::pid_t, libc::SIGKILL);
        }
    }
}

/// A `collide shim` process that a test started in a folder of its own. `Drop` continues it (in case a test stopped it),
/// kills it, reaps it, and stops its child by `child.json`. It signals nothing else.
pub struct ShimProc {
    pub dir: PathBuf,
    child: Option<std::process::Child>,
}

impl ShimProc {
    /// Starts `collide shim <dir>` with an environment of the test's own (`HOME` and `COLLIDE_DAEMON_ROOT` in `root`).
    /// `spec.json` is already in `dir`.
    pub fn start(root: &TempRoot, dir: &Path) -> ShimProc {
        let child = Command::new(env!("CARGO_BIN_EXE_collide"))
            .arg("shim")
            .arg(dir)
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("HOME", root.path())
            .env("COLLIDE_DAEMON_ROOT", root.path())
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("collide shim starts");
        ShimProc {
            dir: dir.to_path_buf(),
            child: Some(child),
        }
    }

    pub fn pid(&self) -> u32 {
        self.child.as_ref().expect("not reaped").id()
    }

    /// The shim's exit status once it has ended, within `within`.
    pub fn wait_exit(&mut self, within: std::time::Duration) -> Option<std::process::ExitStatus> {
        let until = std::time::Instant::now() + within;
        loop {
            if let Some(status) = self.child.as_mut()?.try_wait().unwrap() {
                return Some(status);
            }
            if std::time::Instant::now() >= until {
                return None;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }

    /// Sends `signal` to this shim.
    pub fn signal(&self, signal: i32) {
        // SAFETY: `kill` takes two integers; the pid is this test's own, not yet reaped child.
        unsafe { libc::kill(self.pid() as libc::pid_t, signal) };
    }
}

impl Drop for ShimProc {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            // SAFETY: the pid is this test's own child, which has not been reaped (`take` leaves nothing to reap twice).
            unsafe {
                libc::kill(child.id() as libc::pid_t, libc::SIGCONT);
            }
            let _ = child.kill();
            let _ = child.wait();
        }
        kill_child_of(&self.dir);
    }
}

// ---- the daemon over its socket -----------------------------------------------------------------------------------

/// The daemon's `daemon.status` over its real socket, with the token from its token file. `None` when nothing answers.
pub fn daemon_status(paths: &collide_host::daemon::DaemonPaths) -> Option<serde_json::Value> {
    use std::io::{BufRead, BufReader, Write};
    let stream = std::os::unix::net::UnixStream::connect(&paths.socket).ok()?;
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(5)))
        .ok()?;
    let token = fs::read_to_string(&paths.token_file).ok()?;
    let mut writer = stream.try_clone().ok()?;
    let mut reader = BufReader::new(stream);
    let mut ask = |id: u64, method: &str, params: serde_json::Value| -> Option<serde_json::Value> {
        let request =
            serde_json::json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        writer.write_all(format!("{request}\n").as_bytes()).ok()?;
        let mut line = String::new();
        reader.read_line(&mut line).ok()?;
        serde_json::from_str(&line).ok()
    };
    ask(
        1,
        "initialize",
        serde_json::json!({"protocol": 1, "token": token.trim()}),
    )?;
    Some(ask(2, "daemon.status", serde_json::json!({}))?["result"].clone())
}
