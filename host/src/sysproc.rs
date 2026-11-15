//! What the system says about a connection and about a process (ts-0052-xt), for [`crate::auth`]: the peer's uid and
//! pid, the daemon's own uid, a process's parent, its environment (for the run marker) and its executable. These are
//! the only calls into the system that [`crate::auth`] makes, so the `unsafe` of the daemon's connection rules is
//! here and nowhere else.
//!
//! On macOS they are `getpeereid` and `getsockopt(LOCAL_PEERPID)` for a connection, `geteuid` for the daemon,
//! `proc_pidinfo(PROC_PIDTBSDINFO)` for the parent, `sysctl(KERN_PROCARGS2)` for the environment and `proc_pidpath` for
//! the executable. Every process reader answers `None` for a process it cannot look up (it is gone, it is another
//! user's, or macOS hides it), and never panics. Off macOS nothing is read: the peer's credentials are an
//! `Unsupported` error, the daemon's uid is [`NO_UID`] (which no peer has), and every process lookup is `None`, so
//! a connection is refused and not guessed at.
//!
//! The stop (ts-0059-xs, [`crate::stop`]) reads the same way: every pid the system lists ([`all_pids`]), one record per
//! process ([`record_of`]) that carries `p_uniqueid`, the id macOS never gives to a second process, and the variables of
//! a process's environment ([`env_vars_of`]). Nothing here sends a signal.

use std::io;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;

/// The daemon's uid where it cannot be read: not a uid a process has, so a uid check against it never passes.
pub const NO_UID: u32 = u32::MAX;

/// The uid the daemon runs as (its effective uid, which is also what `getpeereid` reports for a peer).
pub fn daemon_uid() -> u32 {
    imp::daemon_uid()
}

/// The uid of the process on the other end of `stream`, as it was when it connected.
pub fn peer_uid(stream: &UnixStream) -> io::Result<u32> {
    imp::peer_uid(stream)
}

/// Whether the process is stopped (`SIGSTOP`, or a job-control stop), read from its status. `None` when it cannot be
/// looked up. A shim that is stopped answers nothing, which the daemon reports as unresponsive without sending it
/// anything.
pub fn is_stopped(pid: u32) -> Option<bool> {
    imp::is_stopped(pid)
}

/// Whether the process has ended and has not been waited for (a zombie), read from its status. `None` when it cannot
/// be looked up, which macOS may also answer for a zombie. A zombie is not running, whatever `kill(pid, 0)` says of it.
pub fn is_zombie(pid: u32) -> Option<bool> {
    imp::is_zombie(pid)
}

/// The pid of the process on the other end of `stream`, as it was when it connected.
pub fn peer_pid(stream: &UnixStream) -> io::Result<u32> {
    imp::peer_pid(stream)
}

/// The parent of `pid`; `None` at the top of the tree (pid 0 is the kernel's) or when `pid` cannot be looked up.
pub fn parent_pid(pid: u32) -> Option<u32> {
    imp::parent_pid(pid)
}

/// When `pid` started, in microseconds since the Unix epoch, as the kernel records it (`p_starttime`); `None` when
/// `pid` cannot be looked up. The start time with the pid names one process for good, where a pid alone can be reused.
pub fn start_time_us(pid: u32) -> Option<u64> {
    imp::start_time_us(pid)
}

/// The value of the variable `name` in `pid`'s environment as it was when the process started; `None` when it is
/// not there, when macOS hides the environment (a platform binary's) or when `pid` cannot be looked up.
pub fn env_var_of(pid: u32, name: &str) -> Option<String> {
    let args = imp::procargs(pid)?;
    env_var_in(&args, name)
}

/// The path of the executable `pid` runs, with its links resolved, or `None` when it cannot be read.
pub fn executable_of(pid: u32) -> Option<PathBuf> {
    imp::executable_of(pid)
}

/// What one look at a process says. `unique_id` is `p_uniqueid`, which macOS never reuses, so `(pid, unique_id)`
/// names one process for good where a pid alone can name a second one later. `parent_unique_id` is the parent's
/// `p_uniqueid` as the kernel recorded it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProcRecord {
    pub pid: u32,
    pub parent: u32,
    pub uid: u32,
    pub unique_id: u64,
    pub parent_unique_id: u64,
    /// When the process started, in microseconds since the Unix epoch (`p_starttime`).
    pub started_us: u64,
    /// The process has ended and has not been waited for.
    pub zombie: bool,
}

/// The record of `pid`, or `None` when it cannot be looked up. The identity is read before and after the rest, and a
/// pid that became a different process in between is `None`, so a record is of one process.
pub fn record_of(pid: u32) -> Option<ProcRecord> {
    imp::record_of(pid)
}

/// Every pid the system lists, in no order. The list is a snapshot: a pid in it may be gone by the time it is read.
pub fn all_pids() -> io::Result<Vec<u32>> {
    imp::all_pids()
}

/// What reading the environment of a process gave.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnvRead {
    /// The system would not say: the process is gone or is another user's.
    Unreadable,
    /// The system answered with the arguments and nothing after them: an Apple platform binary's environment is
    /// hidden (measured S3, 0.2).
    Hidden,
    /// The value of each name asked for, in order; `None` for a variable that is not set.
    Read(Vec<Option<String>>),
}

/// The values of `names` in the environment `pid` started with.
pub fn env_vars_of(pid: u32, names: &[&str]) -> EnvRead {
    match imp::procargs(pid) {
        None => EnvRead::Unreadable,
        Some(args) => env_vars_in(&args, names),
    }
}

/// [`env_vars_of`] on a buffer `sysctl(KERN_PROCARGS2)` filled. A buffer with nothing after the arguments is
/// [`EnvRead::Hidden`]; a malformed one is [`EnvRead::Unreadable`].
pub fn env_vars_in(args: &[u8], names: &[&str]) -> EnvRead {
    match environment_in(args) {
        None => EnvRead::Unreadable,
        Some([]) => EnvRead::Hidden,
        Some(_) => EnvRead::Read(names.iter().map(|name| env_var_in(args, name)).collect()),
    }
}

/// Reads the value of `name` from the buffer `sysctl(KERN_PROCARGS2)` fills: the argument count as a native `i32`, the
/// executable's path, NUL padding, the `argc` arguments, then the environment, each string ended by a NUL, the
/// environment ended by an empty string. Only the environment is searched, so an argument that looks like
/// `NAME=value` is not a match, and `NAME` must be the whole name before the `=`. The first of two is the one found,
/// as `getenv` would. A value that is not UTF-8 is read with replacement characters.
pub fn env_var_in(args: &[u8], name: &str) -> Option<String> {
    let mut rest = environment_in(args)?;
    let prefix = [name.as_bytes(), b"="].concat();
    loop {
        let end = rest.iter().position(|&b| b == 0).unwrap_or(rest.len());
        let entry = &rest[..end];
        if entry.is_empty() {
            return None;
        }
        if let Some(value) = entry.strip_prefix(prefix.as_slice()) {
            return Some(String::from_utf8_lossy(value).into_owned());
        }
        if end == rest.len() {
            return None;
        }
        rest = &rest[end + 1..];
    }
}

/// The part of a `KERN_PROCARGS2` buffer after the arguments, where the environment starts; `None` for a buffer that
/// is cut short or malformed. It is empty when the system left the environment out.
fn environment_in(args: &[u8]) -> Option<&[u8]> {
    let argc = i32::from_ne_bytes(args.get(..4)?.try_into().ok()?);
    let argc = usize::try_from(argc).ok()?;
    let mut rest = &args[4..];
    // The executable's path, and the NULs that pad it.
    rest = &rest[rest.iter().position(|&b| b == 0)?..];
    rest = &rest[rest.iter().position(|&b| b != 0)?..];
    for _ in 0..argc {
        rest = &rest[rest.iter().position(|&b| b == 0)? + 1..];
    }
    Some(rest)
}

#[cfg(target_os = "macos")]
mod imp {
    use std::ffi::{c_int, c_void};
    use std::io;
    use std::mem::{MaybeUninit, size_of};
    use std::os::fd::AsRawFd;
    use std::os::unix::net::UnixStream;
    use std::path::PathBuf;

    /// How many times a read of the process arguments is tried again when the buffer was too small: the arguments
    /// of a process do not change, but the size asked for and the buffer filled are two calls.
    const PROCARGS_TRIES: usize = 3;

    pub fn daemon_uid() -> u32 {
        // SAFETY: `geteuid` takes no arguments and cannot fail.
        unsafe { libc::geteuid() }
    }

    pub fn peer_uid(stream: &UnixStream) -> io::Result<u32> {
        let mut uid: libc::uid_t = 0;
        let mut gid: libc::gid_t = 0;
        // SAFETY: the descriptor is open for as long as the borrow of `stream`, and the two pointers are to locals
        // that live through the call.
        let rc = unsafe { libc::getpeereid(stream.as_raw_fd(), &mut uid, &mut gid) };
        if rc == 0 {
            Ok(uid)
        } else {
            Err(io::Error::last_os_error())
        }
    }

    pub fn peer_pid(stream: &UnixStream) -> io::Result<u32> {
        let mut pid: libc::pid_t = 0;
        let mut len = size_of::<libc::pid_t>() as libc::socklen_t;
        // SAFETY: the descriptor is open for as long as the borrow of `stream`; the value pointer is to a local
        // `pid_t` and `len` is its size, both alive through the call.
        let rc = unsafe {
            libc::getsockopt(
                stream.as_raw_fd(),
                libc::SOL_LOCAL,
                libc::LOCAL_PEERPID,
                (&mut pid as *mut libc::pid_t).cast::<c_void>(),
                &mut len,
            )
        };
        if rc != 0 {
            return Err(io::Error::last_os_error());
        }
        u32::try_from(pid)
            .ok()
            .filter(|&p| p > 0)
            .ok_or_else(|| io::Error::other(format!("the peer's pid is {pid}")))
    }

    /// The kernel's BSD record of `pid`, or `None` when it cannot be looked up.
    fn bsd_info(pid: u32) -> Option<libc::proc_bsdinfo> {
        let pid = c_int::try_from(pid).ok()?;
        let mut info = MaybeUninit::<libc::proc_bsdinfo>::zeroed();
        let size = size_of::<libc::proc_bsdinfo>() as c_int;
        // SAFETY: the buffer is a zeroed `proc_bsdinfo` of `size` bytes, alive through the call; the kernel writes
        // at most `size` bytes and the return value says how many it wrote.
        let written = unsafe {
            libc::proc_pidinfo(
                pid,
                libc::PROC_PIDTBSDINFO,
                0,
                info.as_mut_ptr().cast::<c_void>(),
                size,
            )
        };
        if written != size {
            return None;
        }
        // SAFETY: the call wrote the whole struct (`written == size`), and zeroed memory is a valid
        // `proc_bsdinfo` in any case (integers and arrays of integers only).
        Some(unsafe { info.assume_init() })
    }

    pub fn parent_pid(pid: u32) -> Option<u32> {
        let info = bsd_info(pid)?;
        let parent = info.pbi_ppid;
        (parent != 0 && parent != info.pbi_pid).then_some(parent)
    }

    /// `SSTOP` of `<sys/proc.h>`: the status of a stopped process.
    const SSTOP: u32 = 4;

    pub fn is_stopped(pid: u32) -> Option<bool> {
        Some(bsd_info(pid)?.pbi_status == SSTOP)
    }

    /// `SZOMB` of `<sys/proc.h>`: the status of a process that ended and was not waited for.
    const SZOMB: u32 = 5;

    pub fn is_zombie(pid: u32) -> Option<bool> {
        Some(bsd_info(pid)?.pbi_status == SZOMB)
    }

    pub fn start_time_us(pid: u32) -> Option<u64> {
        let info = bsd_info(pid)?;
        info.pbi_start_tvsec
            .checked_mul(1_000_000)?
            .checked_add(info.pbi_start_tvusec)
    }

    /// `PROC_PIDUNIQIDENTIFIERINFO` of `<sys/proc_info.h>`: the flavor that returns `struct proc_uniqidentifierinfo`
    /// (`p_uuid`, `p_uniqueid`, `p_puniqueid`, then version and reserved fields). libc does not have it.
    const PROC_PIDUNIQIDENTIFIERINFO: c_int = 17;

    /// The bytes of `p_uuid`, then `p_uniqueid` and `p_puniqueid` at these offsets, as native `u64`s.
    const UNIQUE_ID_OFFSET: usize = 16;
    const PARENT_UNIQUE_ID_OFFSET: usize = 24;
    const UNIQUE_INFO_NEEDED: usize = 32;

    /// `(p_uniqueid, p_puniqueid)` of `pid`, or `None` when it cannot be looked up.
    fn unique_ids(pid: u32) -> Option<(u64, u64)> {
        let pid = c_int::try_from(pid).ok()?;
        // Room for the struct with some to spare, zeroed; the kernel says how many bytes it wrote.
        let mut buffer = [0u8; 128];
        // SAFETY: `buffer` is `buffer.len()` bytes, alive through the call, and the kernel writes at most the size
        // it is told.
        let written = unsafe {
            libc::proc_pidinfo(
                pid,
                PROC_PIDUNIQIDENTIFIERINFO,
                0,
                buffer.as_mut_ptr().cast::<c_void>(),
                buffer.len() as c_int,
            )
        };
        if usize::try_from(written).ok()? < UNIQUE_INFO_NEEDED {
            return None;
        }
        let read = |at: usize| u64::from_ne_bytes(buffer[at..at + 8].try_into().expect("8 bytes"));
        Some((read(UNIQUE_ID_OFFSET), read(PARENT_UNIQUE_ID_OFFSET)))
    }

    pub fn record_of(pid: u32) -> Option<super::ProcRecord> {
        let (unique_id, parent_unique_id) = unique_ids(pid)?;
        let info = bsd_info(pid)?;
        // The pid may have ended and been given to another process between the two reads.
        if info.pbi_pid != pid || unique_ids(pid)?.0 != unique_id {
            return None;
        }
        Some(super::ProcRecord {
            pid,
            parent: info.pbi_ppid,
            uid: info.pbi_uid,
            unique_id,
            parent_unique_id,
            started_us: info
                .pbi_start_tvsec
                .checked_mul(1_000_000)?
                .checked_add(info.pbi_start_tvusec)?,
            zombie: info.pbi_status == SZOMB,
        })
    }

    pub fn all_pids() -> io::Result<Vec<u32>> {
        // SAFETY: a null buffer of size 0 asks only for the count.
        let count = unsafe { libc::proc_listallpids(std::ptr::null_mut(), 0) };
        let count = usize::try_from(count).map_err(|_| io::Error::last_os_error())?;
        // Processes start between the two calls: room for 64 more, and the call says how many it filled.
        let mut pids = vec![0 as c_int; count + 64];
        // SAFETY: `pids` holds `size` bytes, alive through the call, and the kernel writes at most that many.
        let filled = unsafe {
            libc::proc_listallpids(
                pids.as_mut_ptr().cast::<c_void>(),
                (pids.len() * size_of::<c_int>()) as c_int,
            )
        };
        let filled = usize::try_from(filled).map_err(|_| io::Error::last_os_error())?;
        pids.truncate(filled);
        Ok(pids
            .into_iter()
            .filter_map(|pid| u32::try_from(pid).ok())
            .filter(|&pid| pid > 0)
            .collect())
    }

    pub fn procargs(pid: u32) -> Option<Vec<u8>> {
        let pid = c_int::try_from(pid).ok()?;
        let mut mib = [libc::CTL_KERN, libc::KERN_PROCARGS2, pid];
        for _ in 0..PROCARGS_TRIES {
            let mut size: libc::size_t = 0;
            // SAFETY: `mib` is three `c_int`s as `namelen` says, `size` is a local, and no new value is passed.
            let rc = unsafe {
                libc::sysctl(
                    mib.as_mut_ptr(),
                    mib.len() as libc::c_uint,
                    std::ptr::null_mut(),
                    &mut size,
                    std::ptr::null_mut(),
                    0,
                )
            };
            if rc != 0 || size == 0 {
                return None;
            }
            // The size can grow between the two calls; a little room, and a retry when it was not enough.
            let mut buffer = vec![0u8; size + 1024];
            let mut len: libc::size_t = buffer.len();
            // SAFETY: `buffer` has `len` bytes, which the kernel is told and does not exceed; `mib` as above.
            let rc = unsafe {
                libc::sysctl(
                    mib.as_mut_ptr(),
                    mib.len() as libc::c_uint,
                    buffer.as_mut_ptr().cast::<c_void>(),
                    &mut len,
                    std::ptr::null_mut(),
                    0,
                )
            };
            if rc == 0 {
                buffer.truncate(len);
                return Some(buffer);
            }
            if io::Error::last_os_error().raw_os_error() != Some(libc::ENOMEM) {
                return None;
            }
        }
        None
    }

    pub fn executable_of(pid: u32) -> Option<PathBuf> {
        use std::os::unix::ffi::OsStringExt;
        let pid = c_int::try_from(pid).ok()?;
        let mut buffer = vec![0u8; libc::PROC_PIDPATHINFO_MAXSIZE as usize];
        // SAFETY: `buffer` has the number of bytes passed as its size, alive through the call.
        let len = unsafe {
            libc::proc_pidpath(
                pid,
                buffer.as_mut_ptr().cast::<c_void>(),
                buffer.len() as u32,
            )
        };
        let len = usize::try_from(len).ok().filter(|&l| l > 0)?;
        buffer.truncate(len);
        Some(PathBuf::from(std::ffi::OsString::from_vec(buffer)))
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use std::io;
    use std::os::unix::net::UnixStream;
    use std::path::PathBuf;

    fn unsupported() -> io::Error {
        io::Error::new(
            io::ErrorKind::Unsupported,
            "peer credentials are read on macOS only",
        )
    }

    pub fn daemon_uid() -> u32 {
        super::NO_UID
    }

    pub fn peer_uid(_: &UnixStream) -> io::Result<u32> {
        Err(unsupported())
    }

    pub fn peer_pid(_: &UnixStream) -> io::Result<u32> {
        Err(unsupported())
    }

    pub fn parent_pid(_: u32) -> Option<u32> {
        None
    }

    pub fn is_stopped(_: u32) -> Option<bool> {
        None
    }

    pub fn is_zombie(_: u32) -> Option<bool> {
        None
    }

    pub fn start_time_us(_: u32) -> Option<u64> {
        None
    }

    pub fn procargs(_: u32) -> Option<Vec<u8>> {
        None
    }

    pub fn record_of(_: u32) -> Option<super::ProcRecord> {
        None
    }

    pub fn all_pids() -> io::Result<Vec<u32>> {
        Err(unsupported())
    }

    pub fn executable_of(_: u32) -> Option<PathBuf> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A buffer as `KERN_PROCARGS2` fills it.
    fn procargs(path: &str, args: &[&str], env: &[&str]) -> Vec<u8> {
        let mut buffer = (args.len() as i32).to_ne_bytes().to_vec();
        buffer.extend_from_slice(path.as_bytes());
        buffer.extend_from_slice(&[0, 0, 0]);
        for arg in args {
            buffer.extend_from_slice(arg.as_bytes());
            buffer.push(0);
        }
        for var in env {
            buffer.extend_from_slice(var.as_bytes());
            buffer.push(0);
        }
        // The empty string that ends the environment, and the strings the kernel adds after it.
        buffer.push(0);
        buffer.extend_from_slice(b"executable_path=/bin/x\0");
        buffer
    }

    #[test]
    fn a_variable_is_found_in_the_environment() {
        let buffer = procargs(
            "/bin/x",
            &["x", "-c"],
            &["A=1", "COLLIDE_RUN=run-0001-ab", "B=2"],
        );
        assert_eq!(
            env_var_in(&buffer, "COLLIDE_RUN").as_deref(),
            Some("run-0001-ab")
        );
    }

    #[test]
    fn a_missing_variable_is_none() {
        let buffer = procargs("/bin/x", &["x"], &["A=1"]);
        assert_eq!(env_var_in(&buffer, "COLLIDE_RUN"), None);
    }

    #[test]
    fn an_empty_value_is_a_value() {
        let buffer = procargs("/bin/x", &["x"], &["COLLIDE_RUN="]);
        assert_eq!(env_var_in(&buffer, "COLLIDE_RUN").as_deref(), Some(""));
    }

    #[test]
    fn only_the_whole_name_matches() {
        let buffer = procargs(
            "/bin/x",
            &["x"],
            &["COLLIDE_RUN_X=1", "XCOLLIDE_RUN=2", "COLLIDE_RUN"],
        );
        assert_eq!(env_var_in(&buffer, "COLLIDE_RUN"), None);
    }

    #[test]
    fn an_argument_that_looks_like_a_variable_is_not_one() {
        let buffer = procargs("/bin/x", &["x", "COLLIDE_RUN=from-argv"], &["A=1"]);
        assert_eq!(env_var_in(&buffer, "COLLIDE_RUN"), None);
    }

    #[test]
    fn the_strings_after_the_environment_are_not_searched() {
        let mut buffer = procargs("/bin/x", &["x"], &["A=1"]);
        buffer.extend_from_slice(b"COLLIDE_RUN=after\0");
        assert_eq!(env_var_in(&buffer, "COLLIDE_RUN"), None);
    }

    #[test]
    fn the_first_of_two_is_the_one_found() {
        let buffer = procargs(
            "/bin/x",
            &["x"],
            &["COLLIDE_RUN=first", "COLLIDE_RUN=second"],
        );
        assert_eq!(env_var_in(&buffer, "COLLIDE_RUN").as_deref(), Some("first"));
    }

    #[test]
    fn a_value_that_is_not_utf8_is_read_with_replacement_characters() {
        let mut buffer = procargs("/bin/x", &["x"], &[]);
        buffer.truncate(buffer.len() - "executable_path=/bin/x\0".len() - 1);
        buffer.extend_from_slice(b"COLLIDE_RUN=a\xffb\0\0");
        assert_eq!(
            env_var_in(&buffer, "COLLIDE_RUN").as_deref(),
            Some("a\u{fffd}b")
        );
    }

    #[test]
    fn a_buffer_cut_short_is_none_and_never_a_panic() {
        let buffer = procargs("/bin/x", &["x", "y"], &["COLLIDE_RUN=run-0001-ab"]);
        for cut in 0..buffer.len() {
            let _ = env_var_in(&buffer[..cut], "COLLIDE_RUN");
        }
        assert_eq!(env_var_in(&buffer[..3], "COLLIDE_RUN"), None);
        assert_eq!(env_var_in(&[], "COLLIDE_RUN"), None);
    }

    #[test]
    fn a_negative_argument_count_is_none() {
        let mut buffer = procargs("/bin/x", &["x"], &["COLLIDE_RUN=1"]);
        buffer[..4].copy_from_slice(&(-1i32).to_ne_bytes());
        assert_eq!(env_var_in(&buffer, "COLLIDE_RUN"), None);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn a_process_has_a_start_time_that_is_in_the_past_and_not_long_ago() {
        use std::time::{SystemTime, UNIX_EPOCH};
        let started = start_time_us(std::process::id()).expect("this process can be read");
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_micros() as u64;
        assert!(started <= now, "{started} is after {now}");
        assert!(
            now - started < 3_600_000_000,
            "the test run is under an hour old"
        );
    }

    #[test]
    fn a_pid_that_is_not_there_has_no_start_time() {
        assert_eq!(start_time_us(u32::MAX), None);
    }

    #[test]
    fn a_buffer_with_nothing_after_the_arguments_is_a_hidden_environment() {
        // What macOS gives for an Apple platform binary (measured S3): the arguments, and then nothing.
        let mut buffer = (2i32).to_ne_bytes().to_vec();
        buffer.extend_from_slice(b"/bin/sleep\0\0\0\0\0\0/bin/sleep\x0030\0");
        assert_eq!(
            env_vars_in(&buffer, &["COLLIDE_RUN"]),
            EnvRead::Hidden,
            "hidden"
        );
        assert_eq!(env_var_in(&buffer, "COLLIDE_RUN"), None);
    }

    #[test]
    fn an_environment_that_is_read_answers_each_name_in_order() {
        let buffer = procargs(
            "/bin/x",
            &["x"],
            &["COLLIDE_RUNTIME=1", "COLLIDE_RUN=run-1"],
        );
        assert_eq!(
            env_vars_in(&buffer, &["COLLIDE_RUN", "COLLIDE_RUNTIME", "OTHER"]),
            EnvRead::Read(vec![Some("run-1".into()), Some("1".into()), None])
        );
    }

    #[test]
    fn an_environment_with_no_variables_is_read_and_not_hidden() {
        let buffer = procargs("/bin/x", &["x"], &[]);
        assert_eq!(
            env_vars_in(&buffer, &["COLLIDE_RUN"]),
            EnvRead::Read(vec![None])
        );
    }

    #[test]
    fn a_malformed_buffer_is_unreadable() {
        assert_eq!(env_vars_in(&[], &["A"]), EnvRead::Unreadable);
        assert_eq!(env_vars_in(&[1, 0], &["A"]), EnvRead::Unreadable);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn this_process_has_one_record_that_a_second_look_confirms() {
        let me = std::process::id();
        let first = record_of(me).expect("this process can be read");
        assert_eq!(first.pid, me);
        assert!(first.unique_id > 0);
        assert!(!first.zombie);
        assert_eq!(first.parent, parent_pid(me).unwrap());
        assert_eq!(Some(first.started_us), start_time_us(me));
        assert_eq!(record_of(me), Some(first));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn a_child_has_its_own_unique_id_and_names_its_parent_by_it() {
        use std::process::Command;
        // A child this test starts, waited for at the end.
        let mut child = Command::new("/bin/sleep").arg("30").spawn().unwrap();
        let record = record_of(child.id()).expect("the child can be read");
        let me = record_of(std::process::id()).unwrap();
        assert_ne!(record.unique_id, me.unique_id);
        assert_eq!(record.parent, me.pid);
        assert_eq!(record.parent_unique_id, me.unique_id);
        child.kill().unwrap();
        child.wait().unwrap();
        assert_eq!(record_of(record.pid), None, "reaped, so there is no record");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn the_pid_list_holds_this_process_and_no_zero() {
        let pids = all_pids().unwrap();
        assert!(pids.contains(&std::process::id()));
        assert!(!pids.contains(&0));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn a_platform_binarys_environment_is_hidden_and_another_binarys_is_read() {
        use std::process::Command;
        // Children this test starts and waits for; each has a variable of its own.
        let mut hidden = Command::new("/bin/sleep")
            .arg("30")
            .env("COLLIDE_RUN", "sysproc-unit-test")
            .spawn()
            .unwrap();
        let mut read = Command::new("/usr/bin/python3")
            .args(["-c", "import time; time.sleep(30)"])
            .env("COLLIDE_RUN", "sysproc-unit-test")
            .spawn()
            .unwrap();
        let seen = |pid: u32| {
            let until = std::time::Instant::now() + std::time::Duration::from_secs(10);
            loop {
                // The python3 stand-in execs the real interpreter a moment after it starts.
                let got = env_vars_of(pid, &["COLLIDE_RUN"]);
                if got != EnvRead::Unreadable || std::time::Instant::now() > until {
                    return got;
                }
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
        };
        assert_eq!(seen(hidden.id()), EnvRead::Hidden);
        let until = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while env_vars_of(read.id(), &["COLLIDE_RUN"])
            != EnvRead::Read(vec![Some("sysproc-unit-test".into())])
            && std::time::Instant::now() < until
        {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert_eq!(
            env_vars_of(read.id(), &["COLLIDE_RUN"]),
            EnvRead::Read(vec![Some("sysproc-unit-test".into())])
        );
        for child in [&mut hidden, &mut read] {
            child.kill().unwrap();
            child.wait().unwrap();
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn a_stopped_child_is_stopped_and_an_unreaped_one_is_not_live() {
        use std::process::Command;
        use std::time::{Duration, Instant};
        // A child this test starts, so signalling it touches nothing else; it is waited for at the end.
        let mut child = Command::new("/bin/sleep").arg("30").spawn().unwrap();
        let pid = child.id();
        let wait_for = |what: &str, done: &dyn Fn() -> bool| {
            let until = Instant::now() + Duration::from_secs(5);
            while !done() && Instant::now() < until {
                std::thread::sleep(Duration::from_millis(10));
            }
            assert!(done(), "timed out waiting for {what}");
        };
        assert_eq!(is_stopped(pid), Some(false));
        assert_eq!(is_zombie(pid), Some(false));
        // SAFETY: `kill` takes two integers, and the pid is the child started above, not yet waited for.
        unsafe { libc::kill(pid as libc::pid_t, libc::SIGSTOP) };
        wait_for("the child to stop", &|| is_stopped(pid) == Some(true));
        // SAFETY: as above.
        unsafe { libc::kill(pid as libc::pid_t, libc::SIGKILL) };
        // The kernel may answer `Some(true)` for a zombie or give no record at all; either way it is not live.
        wait_for("the child to stop being live", &|| {
            is_zombie(pid) != Some(false)
        });
        child.wait().unwrap();
    }
}
