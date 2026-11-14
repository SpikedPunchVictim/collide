//! S3 gap check: how Rust std and tokio reap a setsid'd child, and whether
//! rusage survives. Prints one line per test. Run: cargo run --offline
use std::os::unix::process::CommandExt;
use std::process::Command;

const AGENT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../fixtures/fake_agent.py");

fn agent() -> Command {
    let mut c = Command::new("/usr/bin/python3");
    c.args([AGENT, "--seconds", "0.3", "--burn", "0.5", "--alloc-mb", "100", "--exit", "3"])
        .stdout(std::process::Stdio::null());
    unsafe {
        c.pre_exec(|| {
            if libc::setsid() == -1 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    c
}

fn wait4(pid: i32, flags: i32) -> String {
    let mut status = 0;
    let mut ru: libc::rusage = unsafe { std::mem::zeroed() };
    let r = unsafe { libc::wait4(pid, &mut status, flags, &mut ru) };
    if r == -1 {
        return format!("wait4 error: {}", std::io::Error::last_os_error());
    }
    if r == 0 {
        return "wait4: still running".into();
    }
    let cpu = ru.ru_utime.tv_sec as f64 + ru.ru_utime.tv_usec as f64 / 1e6
        + ru.ru_stime.tv_sec as f64 + ru.ru_stime.tv_usec as f64 / 1e6;
    format!(
        "wait4 ok: exit={} cpu_s={:.3} maxrss_mb={:.1}",
        libc::WEXITSTATUS(status),
        cpu,
        ru.ru_maxrss as f64 / 1048576.0
    )
}

fn main() {
    // T1 std, pre_exec(setsid), reap with libc::wait4 instead of Child::wait.
    let mut child = agent().spawn().unwrap();
    let pid = child.id() as i32;
    let sid = unsafe { libc::getsid(pid) };
    println!("T1 std+pre_exec(setsid): getsid(child)==child: {}", sid == pid);
    println!("T1 {}", wait4(pid, 0));
    println!("T1 then Child::try_wait(): {:?}", child.try_wait());

    // T2 std Child::wait(): ExitStatus only.
    let mut child = agent().spawn().unwrap();
    let st = child.wait().unwrap();
    println!("T2 std Child::wait(): {:?} (ExitStatus has no rusage field); wait4 after: {}", st, wait4(child.id() as i32, libc::WNOHANG));

    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    rt.block_on(async {
        // T3 tokio wait(): tokio reaps; rusage is gone.
        let mut child = tokio::process::Command::from(agent()).spawn().unwrap();
        let pid = child.id().unwrap() as i32;
        let st = child.wait().await.unwrap();
        println!("T3 tokio Child::wait(): {:?}; wait4 after: {}", st, wait4(pid, libc::WNOHANG));

        // T4 tokio: drop the Child without waiting. Does tokio's orphan reaper
        // take the exit before our own wait4?
        let child = tokio::process::Command::from(agent()).spawn().unwrap();
        let pid = child.id().unwrap() as i32;
        drop(child);
        std::thread::sleep(std::time::Duration::from_millis(1500));
        // Spawning and waiting for another child drives tokio's SIGCHLD handling.
        let mut other = tokio::process::Command::new("/usr/bin/true").spawn().unwrap();
        other.wait().await.unwrap();
        println!("T4 tokio dropped Child, after another tokio wait: {}", wait4(pid, libc::WNOHANG));

        // T5 tokio: keep the Child, reap it ourselves with wait4, then ask tokio.
        let mut child = tokio::process::Command::from(agent()).spawn().unwrap();
        let pid = child.id().unwrap() as i32;
        let got = tokio::task::spawn_blocking(move || wait4(pid, 0)).await.unwrap();
        println!("T5 tokio Child kept, our wait4 first: {}; then tokio try_wait(): {:?}", got, child.try_wait());
    });
}
