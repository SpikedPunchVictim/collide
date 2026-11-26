pub mod csp;
pub mod menu;
pub mod shell;

use collide_core::owner::{RpcError, code};
use collide_core::ui_tools::ToolManifest;
use collide_host::clock::SystemClocks;
use collide_host::daemon::{DaemonPaths, ROOT_ENV};
use collide_host::ui_tools::{SecretStoreTokens, UiToolsEndpoint};
use menu::{
    Reloader, START_WITH_DEFAULTS, START_WITH_DEFAULTS_KEY, START_WITH_DEFAULTS_LABEL,
    on_menu_event, reload_script,
};
use serde_json::Value;
use shell::{
    ChannelEvent, ChannelWindow, DaemonAccess, EventChannel, FolderPicker, LaunchStarter, Shell,
    ShellHandle, SocketLink, UsrBinOpen, pump_daemon_events,
};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::thread;
use std::time::Duration;
use tauri::menu::{Menu, MenuItemBuilder, PredefinedMenuItem, Submenu};
use tauri::{Emitter, Manager};
use tauri_plugin_dialog::DialogExt;

pub const NAME: &str = "collide-app";

/// The one command the webview calls (0.2). `app/src/owner/tauri.ts` has the same name; a test keeps the two equal.
pub const RPC_COMMAND: &str = "rpc";

/// The one event channel the webview listens to (0.2). `app/src/owner/tauri.ts` has the same name.
pub const EVENT_CHANNEL: &str = "collide:event";

/// The label of the window, which Tauri gives the first window when the configuration names none.
const MAIN_WINDOW: &str = "main";

/// The port the UI-tools endpoint is told it has. Its listener, and the port kept in the user's settings, are Stage 6b's,
/// and no HTTP request reaches the endpoint until then.
const UNBOUND_PORT: u16 = 0;

/// The event channel of the webview.
struct WebviewChannel(tauri::AppHandle);

impl EventChannel for WebviewChannel {
    fn is_open(&self) -> bool {
        self.0.get_webview_window(MAIN_WINDOW).is_some()
    }

    fn emit(&self, event: &ChannelEvent) {
        if let Err(error) = self.0.emit_to(MAIN_WINDOW, EVENT_CHANNEL, event) {
            eprintln!(
                "collide: could not send `{}` to the window: {error}",
                event.method
            );
        }
    }
}

/// The webview's `rpc` command: the method and its params. The shell adds the per-client token when it forwards, so no
/// argument here can hold one. A failure rejects the call with the serialized error.
#[tauri::command]
async fn rpc(
    shell: tauri::State<'_, Arc<ShellHandle>>,
    method: String,
    params: Option<Value>,
) -> Result<Value, RpcError> {
    let shell = Arc::clone(&shell);
    tauri::async_runtime::spawn_blocking(move || shell.rpc(method, params))
        .await
        .map_err(|error| {
            RpcError::new(
                code::INTERNAL_ERROR,
                format!("the call did not finish: {error}"),
            )
        })?
}

/// The system's folder picker (ts-0191-nv). It is called only from the shell's thread, never the main one, because a
/// blocking dialog on the main thread would never return. The window cannot reach the plugin: its capability allows the
/// one `rpc` command and the event channel, so the only way to the picker is `shell.pick_folder`.
struct DialogPicker(tauri::AppHandle);

impl FolderPicker for DialogPicker {
    fn pick(&mut self) -> Result<Option<PathBuf>, String> {
        match self.0.dialog().file().blocking_pick_folder() {
            None => Ok(None),
            Some(chosen) => chosen
                .into_path()
                .map(Some)
                .map_err(|error| format!("the chosen folder has no path: {error}")),
        }
    }
}

/// The window's reload (ts-0215-gt): [`reload_script`] run in the main window. The menu's handler asks for it.
struct WebviewReloader(tauri::AppHandle);

impl Reloader for WebviewReloader {
    fn reload(&self, defaults: bool) -> Result<(), String> {
        let window = self
            .0
            .get_webview_window(MAIN_WINDOW)
            .ok_or_else(|| "the window is not open".to_string())?;
        window
            .eval(reload_script(defaults))
            .map_err(|error| error.to_string())
    }
}

/// The app's menu: the standard items a window of text needs (macOS drops them when an app sets a menu of its own), and
/// "Start with Defaults..." in the app's own menu, with its accelerator.
fn build_menu(app: &tauri::AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let defaults = MenuItemBuilder::with_id(START_WITH_DEFAULTS, START_WITH_DEFAULTS_LABEL)
        .accelerator(START_WITH_DEFAULTS_KEY)
        .build(app)?;
    let application = Submenu::with_items(
        app,
        "Collide",
        true,
        &[
            &PredefinedMenuItem::about(app, None, None)?,
            &PredefinedMenuItem::separator(app)?,
            &defaults,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::hide(app, None)?,
            &PredefinedMenuItem::hide_others(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::quit(app, None)?,
        ],
    )?;
    let edit = Submenu::with_items(
        app,
        "Edit",
        true,
        &[
            &PredefinedMenuItem::undo(app, None)?,
            &PredefinedMenuItem::redo(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::cut(app, None)?,
            &PredefinedMenuItem::copy(app, None)?,
            &PredefinedMenuItem::paste(app, None)?,
            &PredefinedMenuItem::select_all(app, None)?,
        ],
    )?;
    let window = Submenu::with_items(
        app,
        "Window",
        true,
        &[
            &PredefinedMenuItem::minimize(app, None)?,
            &PredefinedMenuItem::maximize(app, None)?,
            &PredefinedMenuItem::close_window(app, None)?,
        ],
    )?;
    Menu::with_items(app, &[&application, &edit, &window])
}

fn build_shell(
    channel: Arc<dyn EventChannel>,
    access: Arc<DaemonAccess>,
    picker: Box<dyn FolderPicker>,
) -> Shell {
    // The manifest is empty until the generator of Stage 6 writes the real one from the tools' TypeScript.
    let endpoint = UiToolsEndpoint::new(
        UNBOUND_PORT,
        ToolManifest(Vec::new()),
        Box::new(SecretStoreTokens),
        Box::new(ChannelWindow::new(channel)),
        Box::new(SystemClocks),
    )
    .expect("an empty manifest is valid");
    // The one place the real opener is made: a link the person confirmed opens in their browser.
    Shell::new(
        endpoint,
        Box::new(SocketLink::new(access)),
        Box::new(UsrBinOpen),
    )
    .with_picker(picker)
}

/// The files of the daemon and the way to start it, from this process's environment (0.2, Starting the daemon).
fn daemon_access() -> Result<Arc<DaemonAccess>, String> {
    let root = std::env::var(ROOT_ENV).ok();
    let home = std::env::var("HOME").ok();
    let paths = DaemonPaths::from_env(root.as_deref(), home.as_deref())?;
    let exe = std::env::current_exe()
        .and_then(std::fs::canonicalize)
        .map_err(|error| format!("could not find the app's own executable: {error}"))?;
    Ok(Arc::new(
        DaemonAccess::new(
            paths.clone(),
            Box::new(LaunchStarter::new(paths)),
            Arc::new(SystemClocks),
            exe,
        )
        .with_home(home),
    ))
}

/// How long the event pump waits before it connects again when the stream ends.
const EVENT_RETRY: Duration = Duration::from_secs(1);

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .menu(build_menu)
        .on_menu_event(|app, event| {
            on_menu_event(event.id().as_ref(), &WebviewReloader(app.clone()));
        })
        .setup(|app| {
            let channel: Arc<dyn EventChannel> = Arc::new(WebviewChannel(app.handle().clone()));
            let access = daemon_access()?;
            let shell_access = Arc::clone(&access);
            let shell_channel = Arc::clone(&channel);
            let picker = DialogPicker(app.handle().clone());
            app.manage(Arc::new(ShellHandle::spawn(move || {
                build_shell(shell_channel, shell_access, Box::new(picker))
            })));
            // The pump lives as long as the app does, so its stop flag is never set.
            thread::Builder::new()
                .name("collide-events".into())
                .spawn(move || {
                    let stop = AtomicBool::new(false);
                    pump_daemon_events(&access, channel.as_ref(), &stop, EVENT_RETRY);
                })?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![rpc])
        .run(tauri::generate_context!())
        .expect("error while running Collide");
}
