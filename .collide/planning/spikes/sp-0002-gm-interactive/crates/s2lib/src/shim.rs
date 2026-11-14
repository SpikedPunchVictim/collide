//! One shim per child. It holds the child's stdin pipe or PTY master, reaps with wait4, writes exit.json.
use crate::frame::{read_frame, write_frame};
use crate::sys::{atomic_write, now_ms, start_time};
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Spec {
    pub id: String,
    pub argv: Vec<String>,
    pub cwd: String,
    pub mode: String, // "pty" or "pipe"
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub env_remove: Vec<String>,
    #[serde(default = "d_rows")]
    pub rows: u16,
    #[serde(default = "d_cols")]
    pub cols: u16,
}
fn d_rows() -> u16 { 40 }
fn d_cols() -> u16 { 120 }

const RING_MAX: usize = 1 << 20;

struct Shared {
    clients: Vec<(u64, Arc<Mutex<UnixStream>>)>,
    ring: Vec<u8>,
    exit: Option<String>,
}

type Input = Arc<Mutex<Option<Box<dyn Write + Send>>>>;

pub fn run(dir: &Path) -> Result<(), String> {
    let spec: Spec = serde_json::from_slice(&std::fs::read(dir.join("spec.json")).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    unsafe { libc::setsid() };
    let sock = dir.join("shim.sock");
    let _ = std::fs::remove_file(&sock);
    crate::sys::sock_path_ok(&sock)?;
    let listener = UnixListener::bind(&sock).map_err(|e| format!("bind {}: {}", sock.display(), e))?;
    let shared = Arc::new(Mutex::new(Shared { clients: vec![], ring: vec![], exit: None }));
    let input: Input = Arc::new(Mutex::new(None));
    let resize: Arc<Mutex<Option<Box<dyn portable_pty::MasterPty + Send>>>> = Arc::new(Mutex::new(None));
    let reader_done = Arc::new(AtomicBool::new(false));
    let mut ptylog = std::fs::OpenOptions::new().create(true).append(true).open(dir.join("pty.log")).ok();
    let pid: i32;
    let mut child_holder: Option<Box<dyn portable_pty::Child + Send + Sync>> = None;
    let mut std_child: Option<std::process::Child> = None;

    if spec.mode == "pty" {
        let pair = native_pty_system()
            .openpty(PtySize { rows: spec.rows, cols: spec.cols, pixel_width: 0, pixel_height: 0 })
            .map_err(|e| e.to_string())?;
        let mut cmd = CommandBuilder::new(&spec.argv[0]);
        cmd.args(&spec.argv[1..]);
        cmd.cwd(&spec.cwd);
        for k in &spec.env_remove { cmd.env_remove(k); }
        for (k, v) in &spec.env { cmd.env(k, v); }
        let child = pair.slave.spawn_command(cmd).map_err(|e| format!("spawn {:?}: {}", spec.argv, e))?;
        drop(pair.slave);
        pid = child.process_id().ok_or("no pid")? as i32;
        let mut reader = pair.master.try_clone_reader().map_err(|e| e.to_string())?;
        *input.lock().unwrap() = Some(pair.master.take_writer().map_err(|e| e.to_string())?);
        *resize.lock().unwrap() = Some(pair.master);
        child_holder = Some(child);
        let (sh, rd) = (shared.clone(), reader_done.clone());
        std::thread::spawn(move || {
            let mut buf = vec![0u8; 16384];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        if let Some(f) = ptylog.as_mut() { let _ = f.write_all(&buf[..n]); }
                        let mut s = sh.lock().unwrap();
                        s.ring.extend_from_slice(&buf[..n]);
                        if s.ring.len() > RING_MAX * 2 {
                            let cut = s.ring.len() - RING_MAX;
                            s.ring.drain(..cut);
                        }
                        broadcast(&mut s, b'O', &buf[..n]);
                    }
                }
            }
            rd.store(true, Ordering::SeqCst);
        });
    } else {
        let out = std::fs::OpenOptions::new().create(true).append(true).open(dir.join("out.log")).map_err(|e| e.to_string())?;
        let err = std::fs::OpenOptions::new().create(true).append(true).open(dir.join("err.log")).map_err(|e| e.to_string())?;
        let mut c = Command::new(&spec.argv[0]);
        c.args(&spec.argv[1..]).current_dir(&spec.cwd).stdin(Stdio::piped()).stdout(out).stderr(err);
        for k in &spec.env_remove { c.env_remove(k); }
        c.envs(&spec.env);
        let mut ch = c.spawn().map_err(|e| format!("spawn {:?}: {}", spec.argv, e))?;
        pid = ch.id() as i32;
        *input.lock().unwrap() = Some(Box::new(ch.stdin.take().unwrap()));
        std_child = Some(ch); // never call wait on it: wait4 below is the only reaper
        reader_done.store(true, Ordering::SeqCst);
    }

    let me = std::process::id() as i32;
    let started = start_time(me).unwrap_or((0, 0));
    let info = json!({"shim_pid": me, "shim_start": [started.0, started.1], "child_pid": pid, "mode": spec.mode,
        "id": spec.id, "started_ms": now_ms(), "argv0": spec.argv[0]});
    atomic_write(&dir.join("shim.json"), info.to_string().as_bytes()).map_err(|e| e.to_string())?;

    // client accept loop
    {
        let (sh, inp, rs, info_s) = (shared.clone(), input.clone(), resize.clone(), info.to_string());
        std::thread::spawn(move || {
            let mut next = 1u64;
            for conn in listener.incoming() {
                let Ok(conn) = conn else { continue };
                let id = next;
                next += 1;
                let (sh, inp, rs, info_s) = (sh.clone(), inp.clone(), rs.clone(), info_s.clone());
                std::thread::spawn(move || client(id, conn, sh, inp, rs, info_s));
            }
        });
    }

    // the only reaper
    let mut status: libc::c_int = 0;
    let mut ru: libc::rusage = unsafe { std::mem::zeroed() };
    let r = loop {
        let r = unsafe { libc::wait4(pid, &mut status, 0, &mut ru) };
        if r == -1 && unsafe { *libc::__error() } == libc::EINTR { continue; }
        break r;
    };
    let ended = now_ms();
    for _ in 0..50 {
        if reader_done.load(Ordering::SeqCst) { break; }
        std::thread::sleep(Duration::from_millis(10));
    }
    let tv = |t: libc::timeval| t.tv_sec as f64 + t.tv_usec as f64 / 1e6;
    let exit = json!({
        "id": spec.id, "wait4": r, "code": if libc::WIFEXITED(status) { Some(libc::WEXITSTATUS(status)) } else { None },
        "signal": if libc::WIFSIGNALED(status) { Some(libc::WTERMSIG(status)) } else { None },
        "utime_s": tv(ru.ru_utime), "stime_s": tv(ru.ru_stime), "maxrss_bytes": ru.ru_maxrss, "ended_ms": ended,
    });
    atomic_write(&dir.join("exit.json"), exit.to_string().as_bytes()).map_err(|e| e.to_string())?;
    {
        let mut s = shared.lock().unwrap();
        s.exit = Some(exit.to_string());
        broadcast(&mut s, b'X', exit.to_string().as_bytes());
    }
    std::thread::sleep(Duration::from_millis(1500)); // let attached clients read the exit frame
    drop(child_holder);
    drop(std_child);
    Ok(())
}

fn broadcast(s: &mut Shared, t: u8, p: &[u8]) {
    s.clients.retain(|(_, c)| {
        let mut g = c.lock().unwrap();
        write_frame(&mut *g, t, p).is_ok()
    });
}

fn client(
    id: u64,
    conn: UnixStream,
    sh: Arc<Mutex<Shared>>,
    inp: Input,
    rs: Arc<Mutex<Option<Box<dyn portable_pty::MasterPty + Send>>>>,
    info: String,
) {
    let _ = conn.set_write_timeout(Some(Duration::from_secs(3)));
    let Ok(wr) = conn.try_clone() else { return };
    let wr = Arc::new(Mutex::new(wr));
    {
        // hello, replay and subscribe under one lock: no loss, no duplicate
        let mut s = sh.lock().unwrap();
        let mut w = wr.lock().unwrap();
        if write_frame(&mut *w, b'H', info.as_bytes()).is_err() { return; }
        for chunk in s.ring.chunks(32768) {
            if write_frame(&mut *w, b'O', chunk).is_err() { return; }
        }
        if let Some(x) = &s.exit { let _ = write_frame(&mut *w, b'X', x.as_bytes()); }
        drop(w);
        s.clients.push((id, wr.clone()));
    }
    let mut rd = conn;
    loop {
        let Ok((t, p)) = read_frame(&mut rd) else { break };
        match t {
            b'I' => {
                if let Some(w) = inp.lock().unwrap().as_mut() {
                    let _ = w.write_all(&p);
                    let _ = w.flush();
                }
            }
            b'E' => { *inp.lock().unwrap() = None; }
            b'R' if p.len() == 4 => {
                let (rows, cols) = (u16::from_be_bytes([p[0], p[1]]), u16::from_be_bytes([p[2], p[3]]));
                if let Some(m) = rs.lock().unwrap().as_ref() {
                    let _ = m.resize(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 });
                }
            }
            _ => {}
        }
    }
    sh.lock().unwrap().clients.retain(|(i, _)| *i != id);
}

pub fn run_dir(state: &Path, id: &str) -> PathBuf {
    state.join("runs").join(id)
}
