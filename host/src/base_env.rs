//! The base environment (0.2, The base environment; measured S4). An app started from Finder or launchd gets a
//! minimal `PATH` and no `LANG`, so the daemon captures the login shell's environment, and everything Collide starts
//! gets that environment minus a list of variables.
//!
//! - **The capture** runs `$SHELL -l -i -c` and has it print `env -0` between nonce markers, with stdin from
//!   `/dev/null`, in a session of its own that is killed after reading, under [`CAPTURE_TIMEOUT`], with output capped
//!   at [`CAPTURE_CAP_BYTES`]. On failure the daemon falls back to the minimal environment and logs the shell and the
//!   reason ([`EnvSource::Minimal`]). `LANG` is set when missing ([`FALLBACK_LANG`]).
//! - **It is captured again when the app starts** (R5), and each new phase uses the newest capture. A phase already
//!   running keeps the environment it started with, which is why [`BaseEnvironment`] is a value a phase copies and not
//!   a view of the daemon's current one.
//! - **What a child does not get** is the adapter's [`EnvStrip`] (0.3), [`ALWAYS_STRIPPED`] and nothing else.
//!
//! | tag | function | what it does |
//! |---|---|---|
//! | `daemon: base environment` | [`BaseEnvironment::capture`], [`BaseEnvironment::for_child`] | the capture, and the environment of one child |

use crate::clock::{Clock, Clocks, Reading, Timer};
use crate::stop::RUNTIME_MARKER;
use collide_core::adapter::EnvStrip;
use std::collections::BTreeMap;
use std::io::Read;
use std::os::unix::io::AsRawFd;
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

/// The most output the capture reads, in bytes (0.2: 1 MiB).
pub const CAPTURE_CAP_BYTES: usize = 1024 * 1024;

/// How long the capture may take before its session is killed. The plan says "under a timeout" and gives no number;
/// a login shell that loads a heavy profile takes seconds.
pub const CAPTURE_TIMEOUT: Duration = Duration::from_secs(10);

/// The variables removed from every child whatever the adapter says (0.2): terminal session ids, which name a
/// terminal the child is not in, launchd's service markers, and the runtime mark. The daemon carries the mark, so its
/// login shell hands it to the capture; a child that kept it would be taken for a runtime process, and a stop would
/// never signal it. Only a shim carries the mark, set on its own command.
pub const ALWAYS_STRIPPED: [&str; 5] = [
    "TERM_SESSION_ID",
    "ITERM_SESSION_ID",
    "XPC_SERVICE_NAME",
    "XPC_FLAGS",
    RUNTIME_MARKER,
];

/// The `LANG` set when the captured environment has none. The plan does not name a value.
pub const FALLBACK_LANG: &str = "en_US.UTF-8";

/// Where an environment came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnvSource {
    /// The login shell `shell` printed it.
    LoginShell { shell: String },
    /// The capture failed, so this is the minimal environment. `shell` is the shell that was tried, when there was
    /// one, and `reason` is why it failed, for the log.
    Minimal {
        shell: Option<String>,
        reason: String,
    },
}

/// What a capture is run with. The defaults are the plan's; a test gives a short timeout, a small cap and its own
/// fallback so that it never waits and never depends on the environment of the process that runs it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureOptions {
    /// How long the shell may take, on the uptime clock (a work timeout, 0.2 Clocks).
    pub timeout: Duration,
    /// The most output read, in bytes.
    pub cap_bytes: usize,
    /// The minimal environment a failed capture falls back to. It is the daemon's own: what launchd gave it
    /// (`USER`, `HOME`, `PATH` from the job's plist).
    pub fallback: BTreeMap<String, String>,
}

impl Default for CaptureOptions {
    fn default() -> Self {
        CaptureOptions {
            timeout: CAPTURE_TIMEOUT,
            cap_bytes: CAPTURE_CAP_BYTES,
            fallback: std::env::vars_os()
                .filter_map(|(k, v)| Some((k.into_string().ok()?, v.into_string().ok()?)))
                .collect(),
        }
    }
}

/// One capture of the base environment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaseEnvironment {
    pub vars: BTreeMap<String, String>,
    pub source: EnvSource,
    /// The wall-clock reading when the capture finished, so the newest of two can be told.
    pub captured_at: Reading,
}

impl BaseEnvironment {
    /// Captures the environment of `shell`'s login session. It does not fail: a capture that fails returns
    /// [`EnvSource::Minimal`] with the reason.
    pub fn capture(shell: &str, clocks: &dyn Clocks) -> BaseEnvironment {
        BaseEnvironment::capture_with(shell, clocks, &CaptureOptions::default())
    }

    /// [`BaseEnvironment::capture`] with the timeout, the cap and the fallback given.
    pub fn capture_with(
        shell: &str,
        clocks: &dyn Clocks,
        options: &CaptureOptions,
    ) -> BaseEnvironment {
        let (mut vars, source) = match run_login_shell(shell, clocks, options) {
            Ok(vars) => (
                vars,
                EnvSource::LoginShell {
                    shell: shell.to_string(),
                },
            ),
            Err(reason) => (
                options.fallback.clone(),
                EnvSource::Minimal {
                    shell: Some(shell.to_string()),
                    reason,
                },
            ),
        };
        if vars.get("LANG").is_none_or(|lang| lang.is_empty()) {
            vars.insert("LANG".to_string(), FALLBACK_LANG.to_string());
        }
        BaseEnvironment {
            vars,
            source,
            captured_at: clocks.read(Clock::Wall),
        }
    }

    /// The environment of one child: this capture, minus `strip` and [`ALWAYS_STRIPPED`], plus `extra` (the adapter's
    /// own variables, and the run's `COLLIDE_RUN` and `COLLIDE_RUN_START`), never with the runtime mark.
    pub fn for_child(
        &self,
        strip: &EnvStrip,
        extra: &BTreeMap<String, String>,
    ) -> BTreeMap<String, String> {
        let mut env: BTreeMap<String, String> = self
            .vars
            .iter()
            .filter(|(name, _)| {
                let name = name.as_str();
                !ALWAYS_STRIPPED.contains(&name)
                    && !strip.names.iter().any(|n| n == name)
                    && !strip.prefixes.iter().any(|p| name.starts_with(p.as_str()))
            })
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        env.extend(extra.iter().map(|(k, v)| (k.clone(), v.clone())));
        // A layer over the base (a role's `env`) never gives the mark back: a child with it would never be stopped.
        env.remove(RUNTIME_MARKER);
        env
    }
}

impl EnvSource {
    /// One line for the daemon's log: the shell, and the reason when the capture fell back.
    pub fn describe(&self) -> String {
        match self {
            EnvSource::LoginShell { shell } => format!("base environment captured from {shell}"),
            EnvSource::Minimal { shell, reason } => format!(
                "base environment fell back to the minimal one: shell {}: {reason}",
                shell.as_deref().unwrap_or("(none)")
            ),
        }
    }
}

/// How far a poll waits before the loop looks at the timer again, in milliseconds.
const POLL_SLICE_MS: i32 = 50;

/// The bytes read from the shell, and why reading stopped.
enum ReadEnd {
    /// The closing marker was seen.
    Marked,
    /// The shell closed its output.
    Closed,
    OverCap,
    TimedOut,
}

fn marker(kind: &str, nonce: &str) -> String {
    format!("COLLIDE-ENV-{kind}-{nonce}")
}

/// 64 bits from the system's random source, as hex, so a profile that prints something cannot print a marker.
fn nonce() -> String {
    let mut bytes = [0u8; 8];
    let from_system = std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut bytes))
        .is_ok();
    if !from_system {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos() as u64);
        bytes = (now ^ u64::from(std::process::id())).to_le_bytes();
    }
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Runs `shell -l -i -c` and returns the variables it printed between the markers.
fn run_login_shell(
    shell: &str,
    clocks: &dyn Clocks,
    options: &CaptureOptions,
) -> Result<BTreeMap<String, String>, String> {
    let nonce = nonce();
    let (begin, end) = (marker("BEGIN", &nonce), marker("END", &nonce));
    let script = format!("printf '%s' '{begin}'; env -0; printf '%s' '{end}'");
    let mut command = Command::new(shell);
    command
        .args(["-l", "-i", "-c", &script])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    // SAFETY: `setsid` takes no arguments and is async-signal-safe, which `pre_exec` requires. The shell becomes the
    // leader of a session and of a process group of its own, whose id is its pid.
    unsafe {
        command.pre_exec(|| {
            if libc::setsid() == -1 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut child = command
        .spawn()
        .map_err(|e| format!("could not start {shell}: {e}"))?;
    let timer = Timer::start(Clock::Uptime, options.timeout, clocks.read(Clock::Uptime))
        .map_err(|e| e.to_string())?;
    let mut stdout = child.stdout.take().expect("stdout is piped");
    let mut buf = Vec::new();
    let outcome = read_output(&mut stdout, &mut buf, &end, options.cap_bytes, || {
        timer.is_over(clocks.read(Clock::Uptime)).unwrap_or(true)
    });
    // The session is killed after reading, whatever was read, and before the shell is waited for: the shell's pid is
    // the group's id, and while it is unreaped nothing else can take it.
    kill_session(&child);
    let status = child.wait();
    match outcome {
        Err(e) => Err(format!("could not read the output of {shell}: {e}")),
        Ok(ReadEnd::TimedOut) => Err(format!(
            "{shell} did not finish within {} s",
            options.timeout.as_secs_f64()
        )),
        Ok(ReadEnd::OverCap) => Err(format!(
            "{shell} printed more than the {}-byte cap",
            options.cap_bytes
        )),
        Ok(ReadEnd::Closed) => Err(format!(
            "{shell} printed no environment ({})",
            describe_status(status)
        )),
        Ok(ReadEnd::Marked) => parse_between(&buf, begin.as_bytes(), end.as_bytes())
            .ok_or_else(|| format!("{shell} printed its closing marker with no opening one")),
    }
}

fn describe_status(status: std::io::Result<std::process::ExitStatus>) -> String {
    match status {
        Ok(status) => match (status.code(), status.signal()) {
            (Some(code), _) => format!("exit status {code}"),
            (None, Some(signal)) => format!("ended by signal {signal}"),
            _ => "ended".to_string(),
        },
        Err(e) => format!("its status could not be read: {e}"),
    }
}

/// Kills the shell's session: its process group, which has the shell's pid as its id because the shell called `setsid`.
fn kill_session(child: &Child) {
    // SAFETY: `killpg` takes two integers. The group is the one `setsid` made for this child, which has not been
    // waited for, so its id is still this child's.
    unsafe {
        libc::killpg(child.id() as libc::pid_t, libc::SIGKILL);
    }
}

/// Reads `stdout` into `buf` until `end` is in it, the shell closes its output, the cap is passed or `over` says the
/// time is up. It polls, so that the timer is looked at while the shell prints nothing.
fn read_output(
    stdout: &mut std::process::ChildStdout,
    buf: &mut Vec<u8>,
    end: &str,
    cap: usize,
    over: impl Fn() -> bool,
) -> std::io::Result<ReadEnd> {
    let fd = stdout.as_raw_fd();
    let mut chunk = [0u8; 8192];
    loop {
        if over() {
            return Ok(ReadEnd::TimedOut);
        }
        let mut poll = libc::pollfd {
            fd,
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: `poll` points at one `pollfd` that lives for the call, and the count says so.
        let ready = unsafe { libc::poll(&mut poll, 1, POLL_SLICE_MS) };
        if ready < 0 {
            let error = std::io::Error::last_os_error();
            if error.kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            return Err(error);
        }
        if ready == 0 {
            continue;
        }
        let before = buf.len();
        match stdout.read(&mut chunk) {
            Ok(0) => return Ok(ReadEnd::Closed),
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        }
        if buf.len() > cap {
            return Ok(ReadEnd::OverCap);
        }
        // A marker can straddle two reads, so the search starts a marker's length before the new bytes.
        let from = before.saturating_sub(end.len());
        if find(&buf[from..], end.as_bytes()).is_some() {
            return Ok(ReadEnd::Marked);
        }
    }
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// The variables between the markers: `NAME=value` entries ended by NULs.
fn parse_between(buf: &[u8], begin: &[u8], end: &[u8]) -> Option<BTreeMap<String, String>> {
    let start = find(buf, begin)? + begin.len();
    let stop = start + find(&buf[start..], end)?;
    let mut vars = BTreeMap::new();
    for entry in buf[start..stop].split(|b| *b == 0) {
        let text = String::from_utf8_lossy(entry);
        if let Some((name, value)) = text.split_once('=')
            && !name.is_empty()
        {
            vars.insert(name.to_string(), value.to_string());
        }
    }
    Some(vars)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_capture_is_capped_at_one_mebibyte() {
        assert_eq!(CAPTURE_CAP_BYTES, 1_048_576);
    }

    #[test]
    fn terminal_session_ids_and_xpc_markers_are_always_stripped() {
        for name in [
            "TERM_SESSION_ID",
            "ITERM_SESSION_ID",
            "XPC_SERVICE_NAME",
            "XPC_FLAGS",
        ] {
            assert!(ALWAYS_STRIPPED.contains(&name), "{name}");
        }
    }

    #[test]
    fn the_runtime_marker_is_always_stripped() {
        assert!(ALWAYS_STRIPPED.contains(&"COLLIDE_RUNTIME"));
    }

    #[test]
    fn the_variables_between_the_markers_are_read_and_the_rest_is_ignored() {
        let text = b"banner\nB:A=1\0EMPTY=\0X=a=b\0noequals\0=novalue\0:Ebye";
        let vars = parse_between(text, b"B:", b":E").unwrap();
        assert_eq!(vars.get("A").map(String::as_str), Some("1"));
        assert_eq!(vars.get("EMPTY").map(String::as_str), Some(""));
        assert_eq!(vars.get("X").map(String::as_str), Some("a=b"));
        assert_eq!(vars.len(), 3);
    }

    #[test]
    fn a_closing_marker_with_no_opening_one_is_not_an_environment() {
        assert!(parse_between(b"A=1\0:E", b"B:", b":E").is_none());
    }
}
