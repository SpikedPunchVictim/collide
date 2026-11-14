use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn now_ms() -> f64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs_f64() * 1000.0
}

/// Process start time (sec, usec) from proc_bsdinfo, the key that pairs with a pid so a reused pid is never trusted.
pub fn start_time(pid: i32) -> Option<(u64, u64)> {
    unsafe {
        let mut info: libc::proc_bsdinfo = std::mem::zeroed();
        let n = libc::proc_pidinfo(
            pid,
            libc::PROC_PIDTBSDINFO,
            0,
            &mut info as *mut _ as *mut libc::c_void,
            std::mem::size_of::<libc::proc_bsdinfo>() as i32,
        );
        if n as usize == std::mem::size_of::<libc::proc_bsdinfo>() {
            Some((info.pbi_start_tvsec, info.pbi_start_tvusec))
        } else {
            None
        }
    }
}

pub fn pid_alive(pid: i32) -> bool {
    unsafe { libc::kill(pid, 0) == 0 || *libc::__error() == libc::EPERM }
}

/// PROC_PIDCOALITIONINFO is flavor 20. The struct is coalition_id[2] (resource, jetsam) plus 3 reserved u64.
pub fn coalition(pid: i32) -> Option<(u64, u64)> {
    unsafe {
        let mut buf = [0u64; 5];
        let n = libc::proc_pidinfo(pid, 20, 0, buf.as_mut_ptr() as *mut libc::c_void, 40);
        if n == 40 {
            Some((buf[0], buf[1]))
        } else {
            None
        }
    }
}

pub fn ppid_of(pid: i32) -> Option<i32> {
    unsafe {
        let mut info: libc::proc_bsdinfo = std::mem::zeroed();
        let n = libc::proc_pidinfo(
            pid,
            libc::PROC_PIDTBSDINFO,
            0,
            &mut info as *mut _ as *mut libc::c_void,
            std::mem::size_of::<libc::proc_bsdinfo>() as i32,
        );
        if n as usize == std::mem::size_of::<libc::proc_bsdinfo>() {
            Some(info.pbi_ppid as i32)
        } else {
            None
        }
    }
}

/// Write a file atomically: tmp, fsync, rename, fsync the directory.
pub fn atomic_write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let tmp = path.with_extension("tmp");
    let mut f = std::fs::File::create(&tmp)?;
    f.write_all(bytes)?;
    f.sync_all()?;
    std::fs::rename(&tmp, path)?;
    if let Some(d) = path.parent() {
        std::fs::File::open(d)?.sync_all()?;
    }
    Ok(())
}

pub fn sock_path_ok(p: &Path) -> Result<(), String> {
    let n = p.as_os_str().len();
    if n > 100 {
        return Err(format!("socket path {} is {} bytes; AF_UNIX allows 103 on macOS", p.display(), n));
    }
    Ok(())
}
