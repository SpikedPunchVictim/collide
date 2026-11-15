//! The daemon's login session (0.2, Starting the daemon; Stage 3b, ts-0066-zb). A daemon records the login session it
//! runs in, and the daemon that starts after it reads the record to say how the last one ended: it was stopped, it
//! ended at logout (**host shutdown**, which is not a crash), or it ended with no word (a crash).
//!
//! The record is one small file, `session.json`, written when the daemon starts and marked when it stops:
//!
//! | what the file holds | how the last daemon ended |
//! |---|---|
//! | no file | there was none: [`End::First`] |
//! | `ended: "stopped"` | it was asked to stop: [`End::Stopped`] |
//! | `ended: "host_shutdown"` | it stopped while the login session was ending: [`End::HostShutdown`] |
//! | no `ended`, and another session than now | it was ended with its session, with no time to write: [`End::HostShutdown`] |
//! | no `ended`, and the same session | it was killed or it crashed: [`End::Crash`] |
//!
//! "The login session is ending" is [`LoginSessions::logging_out`]. The real one cannot tell yet (what macOS tells a
//! launchd job at logout is unmeasured, S2), so it says no; the last two rows are what a real logout leaves, and a test
//! injects the first. The session itself is the audit session id of the process ([`SystemLoginSession`]), which macOS
//! gives each login.

use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::Path;

/// The record's file name, beside the runs' folder.
pub const FILE: &str = "session.json";

/// Where the daemon asks about its login session. A test passes its own, so a logout is a call and not a real one.
pub trait LoginSessions: Send + Sync {
    /// The login session the daemon is in, when it can be read.
    fn current(&self) -> Option<String>;
    /// Whether that session is ending now.
    fn logging_out(&self) -> bool;
}

/// How the last daemon ended, as the next one finds it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum End {
    /// No daemon ran here before.
    First,
    Stopped,
    /// Ended at logout. Not a crash.
    HostShutdown,
    Crash,
}

impl End {
    /// The line the daemon logs.
    pub fn describe(self) -> &'static str {
        match self {
            End::First => "no daemon ran here before",
            End::Stopped => "the last daemon was stopped",
            End::HostShutdown => "the last daemon ended at logout (host shutdown, not a crash)",
            End::Crash => "the last daemon ended with no word (a crash, or it was killed)",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Record {
    session: Option<String>,
    pid: u32,
    started_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    ended: Option<String>,
}

const STOPPED: &str = "stopped";
const HOST_SHUTDOWN: &str = "host_shutdown";

/// Reads the last daemon's record at `path`, says how it ended, and writes this daemon's record in its place. A record
/// that cannot be read counts as none: it is replaced, and the daemon starts.
pub fn begin(path: &Path, sessions: &dyn LoginSessions, started_ms: u64) -> io::Result<End> {
    let session = sessions.current();
    let previous: Option<Record> = fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok());
    let end = match previous {
        None => End::First,
        Some(record) => match record.ended.as_deref() {
            Some(STOPPED) => End::Stopped,
            Some(HOST_SHUTDOWN) => End::HostShutdown,
            _ if record.session != session => End::HostShutdown,
            _ => End::Crash,
        },
    };
    write(
        path,
        &Record {
            session,
            pid: std::process::id(),
            started_ms,
            ended: None,
        },
    )?;
    Ok(end)
}

/// Marks this daemon's record as ended: `host_shutdown` when the session is ending, `stopped` otherwise.
pub fn finish(path: &Path, sessions: &dyn LoginSessions) -> io::Result<End> {
    let mut record: Record = serde_json::from_slice(&fs::read(path)?)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let end = if sessions.logging_out() {
        End::HostShutdown
    } else {
        End::Stopped
    };
    record.ended = Some(
        match end {
            End::HostShutdown => HOST_SHUTDOWN,
            _ => STOPPED,
        }
        .to_string(),
    );
    write(path, &record)?;
    Ok(end)
}

fn write(path: &Path, record: &Record) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string(record).map_err(io::Error::other)?;
    let temp = path.with_extension("json.tmp");
    fs::write(&temp, text)?;
    fs::rename(&temp, path)
}

/// The audit session id of this process, which macOS gives each login. Anywhere else there is none.
pub struct SystemLoginSession;

impl LoginSessions for SystemLoginSession {
    fn current(&self) -> Option<String> {
        audit_session_id()
    }

    fn logging_out(&self) -> bool {
        false
    }
}

#[cfg(target_os = "macos")]
fn audit_session_id() -> Option<String> {
    // `struct auditinfo_addr` of <bsm/audit.h>: the id, the mask, the terminal's address, the session id and the flags.
    #[repr(C)]
    struct AuditInfoAddr {
        auid: u32,
        mask: [u32; 2],
        terminal_port: i32,
        terminal_kind: u32,
        terminal_addr: [u32; 4],
        session: i32,
        flags: u64,
    }
    unsafe extern "C" {
        fn getaudit_addr(info: *mut AuditInfoAddr, length: libc::c_int) -> libc::c_int;
    }
    let mut info = AuditInfoAddr {
        auid: 0,
        mask: [0; 2],
        terminal_port: 0,
        terminal_kind: 0,
        terminal_addr: [0; 4],
        session: 0,
        flags: 0,
    };
    // SAFETY: `info` is a `struct auditinfo_addr` of the size it is told, and the call only writes into it.
    let result = unsafe { getaudit_addr(&mut info, std::mem::size_of::<AuditInfoAddr>() as _) };
    (result == 0).then(|| info.session.to_string())
}

#[cfg(not(target_os = "macos"))]
fn audit_session_id() -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct Fake {
        session: Mutex<Option<String>>,
        out: Mutex<bool>,
    }

    impl Fake {
        fn in_session(session: &str) -> Fake {
            Fake {
                session: Mutex::new(Some(session.to_string())),
                out: Mutex::new(false),
            }
        }
    }

    impl LoginSessions for Fake {
        fn current(&self) -> Option<String> {
            self.session.lock().unwrap().clone()
        }
        fn logging_out(&self) -> bool {
            *self.out.lock().unwrap()
        }
    }

    fn file(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("collide-login-{}-{name}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        dir.join("session.json")
    }

    fn clean(path: &Path) {
        let _ = fs::remove_file(path);
        let _ = fs::remove_file(path.with_extension("json.tmp"));
        let _ = fs::remove_dir(path.parent().unwrap());
    }

    #[test]
    fn the_first_start_has_no_earlier_daemon() {
        let path = file("first");
        assert_eq!(
            begin(&path, &Fake::in_session("100"), 1).unwrap(),
            End::First
        );
        clean(&path);
    }

    #[test]
    fn a_stop_in_the_same_session_is_a_stop() {
        let path = file("stop");
        let sessions = Fake::in_session("100");
        begin(&path, &sessions, 1).unwrap();
        assert_eq!(finish(&path, &sessions).unwrap(), End::Stopped);
        assert_eq!(begin(&path, &sessions, 2).unwrap(), End::Stopped);
        clean(&path);
    }

    #[test]
    fn a_stop_while_the_session_ends_is_host_shutdown_and_no_crash() {
        let path = file("logout");
        let sessions = Fake::in_session("100");
        begin(&path, &sessions, 1).unwrap();
        *sessions.out.lock().unwrap() = true;
        assert_eq!(finish(&path, &sessions).unwrap(), End::HostShutdown);
        assert_eq!(
            begin(&path, &Fake::in_session("200"), 2).unwrap(),
            End::HostShutdown
        );
        clean(&path);
    }

    #[test]
    fn an_end_with_no_word_in_another_session_is_host_shutdown() {
        let path = file("other");
        begin(&path, &Fake::in_session("100"), 1).unwrap();
        assert_eq!(
            begin(&path, &Fake::in_session("200"), 2).unwrap(),
            End::HostShutdown
        );
        clean(&path);
    }

    #[test]
    fn an_end_with_no_word_in_the_same_session_is_a_crash() {
        let path = file("crash");
        begin(&path, &Fake::in_session("100"), 1).unwrap();
        assert_eq!(
            begin(&path, &Fake::in_session("100"), 2).unwrap(),
            End::Crash
        );
        clean(&path);
    }

    #[test]
    fn a_record_that_is_not_readable_is_replaced() {
        let path = file("garbage");
        fs::write(&path, "not json").unwrap();
        assert_eq!(
            begin(&path, &Fake::in_session("100"), 1).unwrap(),
            End::First
        );
        clean(&path);
    }

    #[test]
    fn the_real_session_reads_the_same_twice() {
        assert_eq!(SystemLoginSession.current(), SystemLoginSession.current());
    }
}
