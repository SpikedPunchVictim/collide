//! Prototype collide-daemon: adopts shims from the state dir, supervises them, relays attaches.
use crate::frame::{read_frame, write_frame};
use crate::shim::{run_dir, Spec};
use crate::sys::{now_ms, pid_alive, start_time};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::Write;
use std::os::unix::net::{UnixListener, UnixStream};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Clone, Debug)]
struct Run {
    id: String,
    phase: String, // running | exited | lost
    shim_pid: i32,
    child_pid: i32,
    mode: String,
    exit: Option<Value>,
}

type Table = Arc<Mutex<HashMap<String, Run>>>;

fn log(state: &Path, ev: Value) {
    let mut v = ev;
    v["t_ms"] = json!(now_ms());
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(state.join("daemon.log")) {
        let _ = writeln!(f, "{}", v);
    }
}

fn classify(dir: &Path) -> Option<Run> {
    let info: Value = serde_json::from_slice(&std::fs::read(dir.join("shim.json")).ok()?).ok()?;
    let shim_pid = info["shim_pid"].as_i64()? as i32;
    let exit: Option<Value> = std::fs::read(dir.join("exit.json")).ok().and_then(|b| serde_json::from_slice(&b).ok());
    let want = (info["shim_start"][0].as_u64()?, info["shim_start"][1].as_u64()?);
    let alive = pid_alive(shim_pid) && start_time(shim_pid) == Some(want);
    let phase = if exit.is_some() { "exited" } else if alive { "running" } else { "lost" };
    Some(Run {
        id: info["id"].as_str()?.to_string(),
        phase: phase.into(),
        shim_pid,
        child_pid: info["child_pid"].as_i64()? as i32,
        mode: info["mode"].as_str()?.to_string(),
        exit,
    })
}

fn run_json(r: &Run) -> Value {
    json!({"id": r.id, "phase": r.phase, "shim_pid": r.shim_pid, "child_pid": r.child_pid, "mode": r.mode, "exit": r.exit})
}

pub fn sock(state: &Path) -> PathBuf { state.join("daemon.sock") }

pub fn run(state: &Path) -> Result<(), String> {
    std::fs::create_dir_all(state.join("runs")).map_err(|e| e.to_string())?;
    // single instance: flock on the lock file; the winner writes pid and socket path into it
    let lock = std::fs::OpenOptions::new().create(true).write(true).open(state.join("daemon.lock")).map_err(|e| e.to_string())?;
    use std::os::fd::AsRawFd;
    if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        return Err("another daemon holds the lock".into());
    }
    let _ = lock.set_len(0);
    let mut lw = &lock;
    let _ = writeln!(lw, "{}\n{}", std::process::id(), sock(state).display());

    // raise the soft open-file limit (launchd starts jobs at 256)
    let mut rl = libc::rlimit { rlim_cur: 0, rlim_max: 0 };
    unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, &mut rl) };
    let before = rl.rlim_cur;
    rl.rlim_cur = std::cmp::min(rl.rlim_max, 8192);
    let rc = unsafe { libc::setrlimit(libc::RLIMIT_NOFILE, &rl) };
    let _ = rc;

    // adopt before binding the socket
    let table: Table = Arc::new(Mutex::new(HashMap::new()));
    let t0 = now_ms();
    if let Ok(rd) = std::fs::read_dir(state.join("runs")) {
        for e in rd.flatten() {
            if let Some(r) = classify(&e.path()) {
                log(state, json!({"ev": "adopt", "id": r.id, "phase": r.phase, "shim_pid": r.shim_pid}));
                table.lock().unwrap().insert(r.id.clone(), r);
            }
        }
    }
    let _ = std::fs::remove_file(sock(state));
    crate::sys::sock_path_ok(&sock(state))?;
    let listener = UnixListener::bind(sock(state)).map_err(|e| e.to_string())?;
    log(state, json!({"ev": "up", "pid": std::process::id(), "nofile_before": before, "nofile_after": rl.rlim_cur,
        "adopt_ms": now_ms() - t0, "ppid": crate::sys::ppid_of(std::process::id() as i32)}));

    // supervisor: notice phase changes (a shim gone with no exit.json is `lost`)
    {
        let (table, state) = (table.clone(), state.to_path_buf());
        std::thread::spawn(move || loop {
            std::thread::sleep(Duration::from_millis(100));
            let ids: Vec<String> = table.lock().unwrap().keys().cloned().collect();
            for id in ids {
                let Some(new) = classify(&run_dir(&state, &id)) else { continue };
                let mut t = table.lock().unwrap();
                if let Some(old) = t.get(&id) {
                    if old.phase != new.phase {
                        log(&state, json!({"ev": "phase", "id": id, "from": old.phase, "to": new.phase, "exit": new.exit}));
                    }
                }
                t.insert(id, new);
            }
        });
    }

    for conn in listener.incoming() {
        let Ok(conn) = conn else { continue };
        let (table, state) = (table.clone(), state.to_path_buf());
        std::thread::spawn(move || {
            let _ = serve(conn, &state, table);
        });
    }
    Ok(())
}

fn serve(mut conn: UnixStream, state: &Path, table: Table) -> Result<(), String> {
    loop {
        let (t, p) = read_frame(&mut conn).map_err(|e| e.to_string())?;
        if t != b'C' { return Err("expected C".into()); }
        let cmd: Value = serde_json::from_slice(&p).map_err(|e| e.to_string())?;
        let reply = match cmd["cmd"].as_str().unwrap_or("") {
            "ping" => json!({"ok": true, "pid": std::process::id(), "state": state.display().to_string()}),
            "list" => json!({"ok": true, "runs": table.lock().unwrap().values().map(run_json).collect::<Vec<_>>()}),
            "spawn" => match spawn(state, &table, &cmd) { Ok(v) => v, Err(e) => json!({"ok": false, "error": e}) },
            "attach" => {
                let id = cmd["id"].as_str().unwrap_or("").to_string();
                let sp = run_dir(state, &id).join("shim.sock");
                match UnixStream::connect(&sp) {
                    Ok(shim) => {
                        write_frame(&mut conn, b'J', json!({"ok": true}).to_string().as_bytes()).map_err(|e| e.to_string())?;
                        return relay(conn, shim);
                    }
                    Err(e) => json!({"ok": false, "error": format!("connect {}: {}", sp.display(), e)}),
                }
            }
            c => json!({"ok": false, "error": format!("unknown cmd {c}")}),
        };
        write_frame(&mut conn, b'J', reply.to_string().as_bytes()).map_err(|e| e.to_string())?;
    }
}

fn relay(client: UnixStream, shim: UnixStream) -> Result<(), String> {
    let (mut c_r, mut s_w) = (client.try_clone().map_err(|e| e.to_string())?, shim.try_clone().map_err(|e| e.to_string())?);
    let (mut s_r, mut c_w) = (shim, client);
    let up = std::thread::spawn(move || {
        while let Ok((t, p)) = read_frame(&mut c_r) {
            if write_frame(&mut s_w, t, &p).is_err() { break; }
        }
        let _ = s_w.shutdown(std::net::Shutdown::Both);
    });
    while let Ok((t, p)) = read_frame(&mut s_r) {
        if write_frame(&mut c_w, t, &p).is_err() { break; }
    }
    let _ = c_w.shutdown(std::net::Shutdown::Both);
    let _ = up.join();
    Ok(())
}

fn spawn(state: &Path, table: &Table, cmd: &Value) -> Result<Value, String> {
    let spec: Spec = serde_json::from_value(cmd["spec"].clone()).map_err(|e| e.to_string())?;
    let dir = run_dir(state, &spec.id);
    if dir.exists() { return Err(format!("run {} exists", spec.id)); }
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("spec.json"), serde_json::to_vec(&spec).unwrap()).map_err(|e| e.to_string())?;
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let err = std::fs::File::create(dir.join("shim.err")).map_err(|e| e.to_string())?;
    let t0 = now_ms();
    let mut c = Command::new(exe);
    c.arg("shim").arg(&dir).stdin(Stdio::null()).stdout(Stdio::null()).stderr(err);
    unsafe { c.pre_exec(|| { libc::setsid(); Ok(()) }); }
    let child = c.spawn().map_err(|e| e.to_string())?;
    std::mem::forget(child); // the shim is not our responsibility to reap beyond init
    for _ in 0..300 {
        if let Some(r) = classify(&dir) {
            log(state, json!({"ev": "spawned", "id": r.id, "shim_pid": r.shim_pid, "child_pid": r.child_pid, "ms": now_ms() - t0}));
            let v = run_json(&r);
            table.lock().unwrap().insert(r.id.clone(), r);
            return Ok(json!({"ok": true, "run": v, "spawn_ms": now_ms() - t0}));
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    Err("shim did not come up; see shim.err".into())
}
