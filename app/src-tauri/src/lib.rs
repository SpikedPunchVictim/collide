pub mod shell;

use collide_core::owner::{RpcError, code};
use collide_core::ui_tools::ToolManifest;
use collide_host::clock::SystemClocks;
use collide_host::ui_tools::{KeychainTokens, UiToolsEndpoint};
use serde_json::Value;
use shell::{ChannelEvent, ChannelWindow, EventChannel, Shell, ShellHandle, SocketLink};
use std::sync::Arc;
use tauri::{Emitter, Manager};

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

fn build_shell(channel: Arc<dyn EventChannel>) -> Shell {
    // The manifest is empty until the generator of Stage 6 writes the real one from the tools' TypeScript.
    let endpoint = UiToolsEndpoint::new(
        UNBOUND_PORT,
        ToolManifest(Vec::new()),
        Box::new(KeychainTokens),
        Box::new(ChannelWindow::new(channel)),
        Box::new(SystemClocks),
    )
    .expect("an empty manifest is valid");
    Shell::new(endpoint, Box::new(SocketLink))
}

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let channel: Arc<dyn EventChannel> = Arc::new(WebviewChannel(app.handle().clone()));
            app.manage(Arc::new(ShellHandle::spawn(move || build_shell(channel))));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![rpc])
        .run(tauri::generate_context!())
        .expect("error while running Collide");
}
