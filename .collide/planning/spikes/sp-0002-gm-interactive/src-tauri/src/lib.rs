use s2lib::frame::{read_frame, write_frame};
use s2lib::start;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::Write;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tauri::ipc::{Channel, InvokeResponseBody, Request};
use tauri::State;

const SPIKE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/..");

fn canon(p: PathBuf) -> PathBuf { std::fs::canonicalize(&p).unwrap_or(p) }
fn state_dir() -> PathBuf { canon(Path::new(SPIKE).join("work")).join("state") }
fn exe_path() -> PathBuf { canon(Path::new(SPIKE).join("target/debug/s2d")) }

#[derive(Default)]
struct Conns {
    next: Mutex<u64>,
    map: Mutex<HashMap<u64, Arc<Mutex<UnixStream>>>>,
}

fn out_dir() -> PathBuf {
    let d = Path::new(SPIKE).join("out/app");
    let _ = std::fs::create_dir_all(&d);
    d
}

#[tauri::command]
fn app_config() -> Value {
    std::fs::read(state_dir().join("app.json")).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or(json!({}))
}

#[tauri::command]
fn app_info() -> Value {
    json!({"pid": std::process::id(), "state": state_dir().display().to_string()})
}

#[tauri::command]
fn rpc(cmd: Value) -> Result<Value, String> {
    start::rpc(&state_dir(), cmd)
}

#[tauri::command]
fn attach(id: String, rows: u16, cols: u16, on_data: Channel<InvokeResponseBody>, conns: State<'_, Conns>) -> Result<u64, String> {
    let mut s = UnixStream::connect(s2lib::daemon::sock(&state_dir())).map_err(|e| e.to_string())?;
    write_frame(&mut s, b'C', json!({"cmd": "attach", "id": id}).to_string().as_bytes()).map_err(|e| e.to_string())?;
    let (_, p) = read_frame(&mut s).map_err(|e| e.to_string())?;
    let reply: Value = serde_json::from_slice(&p).map_err(|e| e.to_string())?;
    if reply["ok"] != true { return Err(reply.to_string()); }
    let mut rd = s.try_clone().map_err(|e| e.to_string())?;
    let w = Arc::new(Mutex::new(s));
    let n = { let mut g = conns.next.lock().unwrap(); *g += 1; *g };
    conns.map.lock().unwrap().insert(n, w.clone());
    {
        let mut g = w.lock().unwrap();
        let mut b = Vec::new();
        b.extend_from_slice(&rows.to_be_bytes());
        b.extend_from_slice(&cols.to_be_bytes());
        let _ = write_frame(&mut *g, b'R', &b);
    }
    std::thread::spawn(move || {
        while let Ok((t, p)) = read_frame(&mut rd) {
            let mut m = Vec::with_capacity(p.len() + 1);
            m.push(t);
            m.extend_from_slice(&p);
            if on_data.send(InvokeResponseBody::Raw(m)).is_err() { break; }
        }
        let _ = on_data.send(InvokeResponseBody::Raw(vec![b'Z']));
    });
    Ok(n)
}

#[tauri::command]
fn write_input(req: Request<'_>, conns: State<'_, Conns>) -> Result<(), String> {
    let n: u64 = req.headers().get("conn").and_then(|v| v.to_str().ok()).and_then(|s| s.parse().ok()).ok_or("no conn header")?;
    let tauri::ipc::InvokeBody::Raw(bytes) = req.body() else { return Err("expected raw body".into()) };
    let c = conns.map.lock().unwrap().get(&n).cloned().ok_or("unknown conn")?;
    let mut g = c.lock().unwrap();
    write_frame(&mut *g, b'I', bytes).map_err(|e| e.to_string())
}

#[tauri::command]
fn resize(conn: u64, rows: u16, cols: u16, conns: State<'_, Conns>) -> Result<(), String> {
    let c = conns.map.lock().unwrap().get(&conn).cloned().ok_or("unknown conn")?;
    let mut b = Vec::new();
    b.extend_from_slice(&rows.to_be_bytes());
    b.extend_from_slice(&cols.to_be_bytes());
    let mut g = c.lock().unwrap();
    write_frame(&mut *g, b'R', &b).map_err(|e| e.to_string())
}

#[tauri::command]
fn report(name: String, data: Value) -> Result<(), String> {
    let mut f = std::fs::File::create(out_dir().join(format!("{name}.json"))).map_err(|e| e.to_string())?;
    f.write_all(data.to_string().as_bytes()).map_err(|e| e.to_string())
}

/// The environment forbids Screen Recording for screencapture, so the app snapshots its own webview
/// (WKWebView takeSnapshot needs no permission). Written as out/shots/<name>.png.
#[tauri::command]
fn snapshot(window: tauri::WebviewWindow, name: String) -> Result<(), String> {
    let dir = Path::new(SPIKE).join("out/shots");
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join(format!("{name}.png"));
    window
        .with_webview(move |wv| unsafe {
            use block2::RcBlock;
            use objc2::rc::Retained;
            use objc2_app_kit::{NSBitmapImageFileType, NSBitmapImageRep, NSImage};
            use objc2_foundation::{NSDictionary, NSError};
            use objc2_web_kit::{WKSnapshotConfiguration, WKWebView};
            let webview: &WKWebView = &*(wv.inner() as *const WKWebView);
            let cfg = WKSnapshotConfiguration::new(objc2::MainThreadMarker::new().expect("main thread"));
            let block = RcBlock::new(move |img: *mut NSImage, _err: *mut NSError| {
                if img.is_null() { return; }
                let img: Retained<NSImage> = Retained::retain(img).unwrap();
                if let Some(tiff) = img.TIFFRepresentation() {
                    if let Some(rep) = NSBitmapImageRep::imageRepWithData(&tiff) {
                        let props = NSDictionary::new();
                        if let Some(png) = rep.representationUsingType_properties(NSBitmapImageFileType::PNG, &props) {
                            let _ = std::fs::write(&path, png.to_vec());
                        }
                    }
                }
            });
            webview.takeSnapshotWithConfiguration_completionHandler(Some(&cfg), &block);
        })
        .map_err(|e| e.to_string())
}

/// Menu bar memory check (added after S2 for the menu bar decision). Runs only when `S2_MENUBAR` is
/// `destroy` or `hide`: a tray icon, then a scripted open → close → reopen → exit, with marks written to
/// out/menubar/marks.jsonl for scripts/menubar-mem.mjs to sample memory against. No daemon is started.
mod menubar {
    use std::io::Write;
    use std::path::Path;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::{Duration, SystemTime, UNIX_EPOCH};
    use tauri::menu::{Menu, MenuItem};
    use tauri::tray::TrayIconBuilder;
    use tauri::{ActivationPolicy, AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

    pub static MAIN_LOADED: AtomicBool = AtomicBool::new(false);

    pub fn mark(ev: &str) {
        if std::env::var_os("S2_MENUBAR").is_none() {
            return;
        }
        let dir = Path::new(super::SPIKE).join("out/menubar");
        let _ = std::fs::create_dir_all(&dir);
        let t = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs_f64() * 1000.0;
        let line = serde_json::json!({"t_ms": t, "ev": ev, "pid": std::process::id()}).to_string();
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(dir.join("marks.jsonl")) {
            let _ = writeln!(f, "{line}");
        }
    }

    pub fn setup(app: &mut tauri::App, mode: String) -> Result<(), Box<dyn std::error::Error>> {
        let quit = MenuItem::with_id(app, "quit", "Quit S2Spike", true, None::<&str>)?;
        let menu = Menu::with_items(app, &[&quit])?;
        let icon = app.default_window_icon().cloned().ok_or("no default window icon")?;
        let tray = TrayIconBuilder::with_id("s2")
            .icon(icon)
            .menu(&menu)
            .on_menu_event(|app, e| {
                if e.id().as_ref() == "quit" {
                    app.exit(0)
                }
            })
            .build(app)?;
        app.manage(tray);
        mark(&format!("tray-ready:{mode}"));
        let h = app.handle().clone();
        std::thread::spawn(move || script(h, mode));
        Ok(())
    }

    fn script(h: AppHandle, mode: String) {
        while !MAIN_LOADED.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(50));
        }
        std::thread::sleep(Duration::from_secs(20));
        let Some(w) = h.get_webview_window("main") else {
            mark("error:no-main-window");
            h.exit(2);
            return;
        };
        mark("close-begin");
        let r = if mode == "destroy" { w.destroy() } else { w.hide() };
        if let Err(e) = r {
            mark(&format!("error:close:{e}"));
        }
        if let Err(e) = h.set_activation_policy(ActivationPolicy::Accessory) {
            mark(&format!("error:policy:{e}"));
        }
        mark("closed");
        std::thread::sleep(Duration::from_secs(45));
        mark("reopen-begin");
        let _ = h.set_activation_policy(ActivationPolicy::Regular);
        let r = if mode == "destroy" {
            WebviewWindowBuilder::new(&h, "main2", WebviewUrl::App("index.html".into()))
                .title("S2 Spike")
                .inner_size(1000.0, 700.0)
                .position(80.0, 80.0)
                .build()
                .map(|_| ())
        } else {
            w.show()
        };
        if let Err(e) = r {
            mark(&format!("error:reopen:{e}"));
        }
        mark("reopen-returned");
        std::thread::sleep(Duration::from_secs(20));
        mark("exit");
        h.exit(0);
    }
}

pub fn run() {
    let menubar_mode = std::env::var("S2_MENUBAR").ok().filter(|m| m == "destroy" || m == "hide");
    if menubar_mode.is_none() {
        let mut note = json!({"app_pid": std::process::id()});
        // start the daemon on demand the way the plan says; the config picks setsid or the one-shot launchd job
        let cfg = app_config();
        let st = state_dir();
        if start::ping(&st).is_none() {
            let how = cfg["start"].as_str().unwrap_or("launchd");
            let r = if how == "launchd" { start::start_launchd(&st, &exe_path(), &canon(Path::new(SPIKE).join("work"))) } else { start::start_setsid(&st, &exe_path()) };
            note["start"] = match r { Ok(v) => v, Err(e) => json!({"error": e}) };
        } else {
            note["start"] = json!({"already_running": true});
        }
        let _ = std::fs::write(st.join("app-start.json"), note.to_string());
    }
    let keep_alive = menubar_mode.is_some();
    let app = tauri::Builder::default()
        .manage(Conns::default())
        .invoke_handler(tauri::generate_handler![app_config, app_info, rpc, attach, write_input, resize, report, snapshot])
        .on_page_load(|wv, payload| {
            if payload.event() == tauri::webview::PageLoadEvent::Finished {
                menubar::mark(&format!("loaded:{}", wv.label()));
                if wv.label() == "main" {
                    menubar::MAIN_LOADED.store(true, std::sync::atomic::Ordering::SeqCst);
                }
            }
        })
        .setup(move |app| {
            if let Some(mode) = menubar_mode {
                menubar::setup(app, mode)?;
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("tauri build failed");
    app.run(move |_h, ev| {
        // menu bar mode: closing the last window keeps the app alive; only an explicit exit(code) quits
        if let tauri::RunEvent::ExitRequested { api, code, .. } = ev {
            if keep_alive && code.is_none() {
                api.prevent_exit();
            }
        }
    });
}
