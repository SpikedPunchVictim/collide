//! Starting the daemon: plain setsid, or a one-shot launchd job. Used by `s2d start` and by the Tauri app.
use crate::daemon::sock;
use crate::frame::{read_frame, write_frame};
use crate::sys::now_ms;
use serde_json::{json, Value};
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

pub fn rpc(state: &Path, cmd: Value) -> Result<Value, String> {
    let mut c = UnixStream::connect(sock(state)).map_err(|e| e.to_string())?;
    c.set_read_timeout(Some(Duration::from_secs(5))).ok();
    write_frame(&mut c, b'C', cmd.to_string().as_bytes()).map_err(|e| e.to_string())?;
    let (_, p) = read_frame(&mut c).map_err(|e| e.to_string())?;
    serde_json::from_slice(&p).map_err(|e| e.to_string())
}

pub fn ping(state: &Path) -> Option<Value> {
    rpc(state, json!({"cmd": "ping"})).ok()
}

fn wait_up(state: &Path, secs: u64) -> Option<Value> {
    let t0 = now_ms();
    while now_ms() - t0 < (secs * 1000) as f64 {
        if let Some(v) = ping(state) { return Some(v); }
        std::thread::sleep(Duration::from_millis(20));
    }
    None
}

/// `s2d detach`: the launchd job's program. It forks the daemon into its own session and exits at once.
pub fn detach(state: &Path, exe: &Path) -> Result<(), String> {
    let out = std::fs::OpenOptions::new().create(true).append(true).open(state.join("daemon.out")).map_err(|e| e.to_string())?;
    let err = out.try_clone().map_err(|e| e.to_string())?;
    let mut c = Command::new(exe);
    c.arg("daemon").arg("--state").arg(state).stdin(Stdio::null()).stdout(out).stderr(err);
    unsafe { c.pre_exec(|| { libc::setsid(); Ok(()) }); }
    let child = c.spawn().map_err(|e| e.to_string())?;
    std::mem::forget(child);
    Ok(())
}

pub fn start_setsid(state: &Path, exe: &Path) -> Result<Value, String> {
    let t0 = now_ms();
    detach(state, exe)?;
    let p = wait_up(state, 15).ok_or("daemon did not answer within 15 s")?;
    Ok(json!({"how": "setsid", "ping": p, "ms": now_ms() - t0}))
}

pub fn start_launchd(state: &Path, exe: &Path, work: &Path) -> Result<Value, String> {
    let t0 = now_ms();
    let uid = unsafe { libc::getuid() };
    let label = format!("dev.collide.spike.s2.daemon-{}", std::process::id());
    let plist = work.join(format!("{label}.plist"));
    let path = std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin".into());
    let home = std::env::var("HOME").unwrap_or_default();
    let xml = format!(r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>Label</key><string>{label}</string>
<key>ProgramArguments</key><array><string>{exe}</string><string>detach</string><string>--state</string><string>{state}</string></array>
<key>RunAtLoad</key><true/>
<key>KeepAlive</key><false/>
<key>AbandonProcessGroup</key><true/>
<key>ProcessType</key><string>Interactive</string>
<key>EnvironmentVariables</key><dict><key>PATH</key><string>{path}</string><key>HOME</key><string>{home}</string></dict>
<key>StandardOutPath</key><string>{state}/launchd.out</string>
<key>StandardErrorPath</key><string>{state}/launchd.out</string>
</dict></plist>
"#, exe = exe.display(), state = state.display());
    std::fs::write(&plist, xml).map_err(|e| e.to_string())?;
    let boot = Command::new("launchctl").arg("bootstrap").arg(format!("gui/{uid}")).arg(&plist).output().map_err(|e| e.to_string())?;
    if !boot.status.success() {
        let _ = std::fs::remove_file(&plist);
        return Err(format!("launchctl bootstrap failed: {}", String::from_utf8_lossy(&boot.stderr)));
    }
    let p = wait_up(state, 15);
    let up_ms = now_ms() - t0;
    let out = Command::new("launchctl").arg("bootout").arg(format!("gui/{uid}/{label}")).output().map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(&plist);
    let gone = !Command::new("launchctl").arg("print").arg(format!("gui/{uid}/{label}")).output().map(|o| o.status.success()).unwrap_or(false);
    let p = p.ok_or("daemon did not answer within 15 s")?;
    Ok(json!({"how": "launchd", "label": label, "ping": p, "up_ms": up_ms, "bootout_ok": out.status.success(), "job_gone": gone, "plist_removed": !plist.exists()}))
}
