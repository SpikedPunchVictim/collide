pub mod csp;
pub mod endpoint;
pub mod http;
pub mod menu;
pub mod shell;

use collide_core::owner::{RpcError, code};
use collide_core::ui_tools::ToolManifest;
use collide_host::clock::SystemClocks;
use collide_host::daemon::{DaemonPaths, ROOT_ENV};
use collide_host::secret_store::{SecretStore, secret_store_for_root};
use collide_host::ui_tools::{SecretStoreTokens, UiToolsEndpoint};
use endpoint::{DaemonPortStore, EndpointServer, Log, stderr_log};
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

/// The port the UI-tools endpoint has until its listener is bound and tells it the real one
/// ([`EndpointServer::start`]).
/// No `Host` names port 0, so no request is accepted before then.
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
    secrets: Arc<dyn SecretStore>,
    log: Log,
) -> Shell {
    // The manifest is empty until the generator of Stage 6 writes the real one from the tools' TypeScript.
    let endpoint = UiToolsEndpoint::new(
        UNBOUND_PORT,
        ToolManifest(Vec::new()),
        Box::new(SecretStoreTokens::new(secrets, log)),
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

/// Tells the shell that the main window's page is gone ([`ShellHandle::window_gone`]).
fn window_gone<R: tauri::Runtime>(from: &impl Manager<R>) {
    if let Some(shell) = from.try_state::<Arc<ShellHandle>>() {
        shell.window_gone();
    }
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
            // The only place the keychain is chosen: a daemon root that is overridden gets a store in memory.
            let secrets = secret_store_for_root(std::env::var(ROOT_ENV).ok().as_deref());
            let shell = Arc::new(ShellHandle::spawn(move || {
                build_shell(
                    shell_channel,
                    shell_access,
                    Box::new(picker),
                    secrets,
                    stderr_log(),
                )
            }));
            app.manage(Arc::clone(&shell));
            // The listener starts on a thread of its own: it asks the daemon for the port, which may start the daemon.
            let ports = DaemonPortStore::new(Arc::clone(&access));
            thread::Builder::new()
                .name("collide-endpoint-start".into())
                .spawn(
                    move || match EndpointServer::start(&ports, shell, stderr_log()) {
                        Ok(server) => server.wait(),
                        Err(message) => eprintln!("collide: {message}"),
                    },
                )?;
            // The pump lives as long as the app does, so its stop flag is never set.
            thread::Builder::new()
                .name("collide-events".into())
                .spawn(move || {
                    let stop = AtomicBool::new(false);
                    pump_daemon_events(&access, channel.as_ref(), &stop, EVENT_RETRY);
                })?;
            Ok(())
        })
        // The page that was going to answer the endpoint's calls is gone when the window closes or reloads.
        .on_window_event(|window, event| {
            if window.label() == MAIN_WINDOW && matches!(event, tauri::WindowEvent::Destroyed) {
                window_gone(window);
            }
        })
        .on_page_load(|webview, payload| {
            let started = payload.event() == tauri::webview::PageLoadEvent::Started;
            if webview.label() == MAIN_WINDOW && started {
                window_gone(webview);
            }
        })
        .invoke_handler(tauri::generate_handler![rpc])
        .run(tauri::generate_context!())
        .expect("error while running Collide");
}
