//! The daemon's log, with a size cap and one rotated file (ts-0056 decision 8; 0.2, Durability). [`CappedLog`] appends
//! to `daemon.log`; when a write would take the file over the cap, `daemon.log` is renamed to `daemon.log.1`, which
//! replaces the rotated file that was there, and a new `daemon.log` is started. So the log never holds more than twice
//! the cap on disk, and the latest lines are always in `daemon.log` or, just after a rotation, in `daemon.log.1`.
//!
//! The daemon's standard error also goes to `daemon.log` when a launch redirects it (a panic message, a line before the
//! log is open). A descriptor keeps the file it was given, so after one rotation it would write to `daemon.log.1`, and
//! after the second to a file nobody can reach. So [`CappedLog::open_following`] takes a descriptor to follow: after
//! each rotation the new `daemon.log` is `dup2`'d onto it, and those lines stay in the current log. The daemon's binary
//! passes descriptor 2 only when [`fd_is_file`] says it is `daemon.log` (a daemon started by hand, with standard error
//! on a terminal, is left alone).
//!
//! A single write over the cap is cut to the cap, so a line cannot take the file past it.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::os::fd::{AsRawFd, RawFd};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

/// The cap of `daemon.log`, in bytes. The plan sets none: ten megabytes is some tens of thousands of lines, enough to
/// show a day of a busy daemon, and the two files together are a small part of a disk.
pub const DEFAULT_LOG_CAP_BYTES: u64 = 10 * 1024 * 1024;

/// A log file that rotates once at a size cap.
pub struct CappedLog {
    path: PathBuf,
    rotated: PathBuf,
    cap: u64,
    file: File,
    size: u64,
    follow: Option<RawFd>,
}

impl CappedLog {
    /// Opens `path` for appending (making it, mode `0600`, when it is not there). `cap` is at least one byte.
    pub fn open(path: &Path, cap: u64) -> io::Result<CappedLog> {
        CappedLog::open_following(path, cap, None)
    }

    /// As [`CappedLog::open`], and `follow` is a descriptor that is writing to `path` now (the daemon's standard
    /// error): after each rotation the new file is `dup2`'d onto it.
    pub fn open_following(path: &Path, cap: u64, follow: Option<RawFd>) -> io::Result<CappedLog> {
        let file = open_append(path)?;
        let size = file.metadata()?.len();
        let mut rotated = path.as_os_str().to_owned();
        rotated.push(".1");
        Ok(CappedLog {
            path: path.to_path_buf(),
            rotated: PathBuf::from(rotated),
            cap: cap.max(1),
            file,
            size,
            follow,
        })
    }

    fn rotate(&mut self) -> io::Result<()> {
        fs::rename(&self.path, &self.rotated)?;
        self.file = open_append(&self.path)?;
        self.size = 0;
        if let Some(fd) = self.follow {
            // SAFETY: both descriptors are open: `file` is ours and `fd` was given to us as one that is.
            if unsafe { libc::dup2(self.file.as_raw_fd(), fd) } < 0 {
                return Err(io::Error::last_os_error());
            }
        }
        Ok(())
    }
}

/// Whether descriptor `fd` is open on the file at `path` (the same device and inode).
pub fn fd_is_file(fd: RawFd, path: &Path) -> bool {
    let Ok(wanted) = fs::metadata(path) else {
        return false;
    };
    let mut stat: libc::stat = unsafe { std::mem::zeroed() };
    // SAFETY: `stat` is a valid, writable `stat`.
    if unsafe { libc::fstat(fd, &mut stat) } != 0 {
        return false;
    }
    stat.st_ino as u64 == wanted.ino() && stat.st_dev as u64 == wanted.dev()
}

fn open_append(path: &Path) -> io::Result<File> {
    OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .open(path)
}

impl Write for CappedLog {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let keep = &buf[..buf.len().min(self.cap as usize)];
        if self.size > 0 && self.size + keep.len() as u64 > self.cap {
            self.rotate()?;
        }
        self.file.write_all(keep)?;
        self.size += keep.len() as u64;
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    /// A folder for one test; `Drop` removes the files in it by their own paths and then the folder.
    struct Dir(PathBuf);

    impl Dir {
        fn new() -> Dir {
            let n = COUNTER.fetch_add(1, Ordering::SeqCst);
            let dir =
                std::env::temp_dir().join(format!("collide-logcap-{}-{n}", std::process::id()));
            fs::create_dir_all(&dir).unwrap();
            Dir(dir)
        }
    }

    impl Drop for Dir {
        fn drop(&mut self) {
            for name in ["daemon.log", "daemon.log.1"] {
                let _ = fs::remove_file(self.0.join(name));
            }
            let _ = fs::remove_dir(&self.0);
        }
    }

    fn len(path: &Path) -> u64 {
        fs::metadata(path).map(|m| m.len()).unwrap_or(0)
    }

    #[test]
    fn the_log_stays_under_its_cap_and_keeps_one_rotated_file() {
        let dir = Dir::new();
        let log_path = dir.0.join("daemon.log");
        let mut log = CappedLog::open(&log_path, 100).unwrap();
        for n in 0..50 {
            writeln!(log, "line number {n:04}").unwrap();
            assert!(len(&log_path) <= 100, "after line {n}");
        }
        let rotated = dir.0.join("daemon.log.1");
        assert!(len(&rotated) > 0 && len(&rotated) <= 100);
        let newest = fs::read_to_string(&log_path).unwrap();
        assert!(newest.contains("line number 0049"), "{newest}");
    }

    #[test]
    fn a_second_rotation_replaces_the_first_rotated_file() {
        let dir = Dir::new();
        let log_path = dir.0.join("daemon.log");
        let mut log = CappedLog::open(&log_path, 40).unwrap();
        for n in 0..30 {
            writeln!(log, "entry {n:03}").unwrap();
        }
        let older = fs::read_to_string(dir.0.join("daemon.log.1")).unwrap();
        assert!(!older.contains("entry 000"), "{older}");
        assert_eq!(
            fs::read_dir(&dir.0).unwrap().count(),
            2,
            "only daemon.log and daemon.log.1"
        );
    }

    #[test]
    fn a_log_that_is_already_over_the_cap_rotates_on_the_first_write() {
        let dir = Dir::new();
        let log_path = dir.0.join("daemon.log");
        fs::write(&log_path, "x".repeat(500)).unwrap();
        let mut log = CappedLog::open(&log_path, 100).unwrap();
        writeln!(log, "fresh").unwrap();
        assert_eq!(fs::read_to_string(&log_path).unwrap(), "fresh\n");
        assert_eq!(len(&dir.0.join("daemon.log.1")), 500);
    }

    #[test]
    fn a_single_write_over_the_cap_is_cut_to_the_cap() {
        let dir = Dir::new();
        let log_path = dir.0.join("daemon.log");
        let mut log = CappedLog::open(&log_path, 10).unwrap();
        assert_eq!(log.write(&[b'y'; 50]).unwrap(), 50);
        assert_eq!(len(&log_path), 10);
    }

    #[test]
    fn a_followed_descriptor_writes_to_the_current_log_after_two_rotations() {
        let dir = Dir::new();
        let log_path = dir.0.join("daemon.log");
        // A descriptor of the test's own, standing for standard error under launchd.
        let mut followed = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .unwrap();
        assert!(fd_is_file(followed.as_raw_fd(), &log_path));
        let mut log = CappedLog::open_following(&log_path, 40, Some(followed.as_raw_fd())).unwrap();
        for n in 0..30 {
            writeln!(log, "entry {n:03}").unwrap();
        }
        writeln!(followed, "panic message").unwrap();
        let current = fs::read_to_string(&log_path).unwrap();
        assert!(current.contains("panic message"), "{current}");
        assert!(fd_is_file(followed.as_raw_fd(), &log_path));
    }

    #[test]
    fn a_descriptor_on_another_file_is_not_the_log() {
        let dir = Dir::new();
        let log_path = dir.0.join("daemon.log");
        fs::write(&log_path, "").unwrap();
        let other = File::open(dir.0.join("daemon.log")).unwrap();
        assert!(fd_is_file(other.as_raw_fd(), &log_path));
        fs::write(dir.0.join("daemon.log.1"), "").unwrap();
        let elsewhere = File::open(dir.0.join("daemon.log.1")).unwrap();
        assert!(!fd_is_file(elsewhere.as_raw_fd(), &log_path));
    }
}
