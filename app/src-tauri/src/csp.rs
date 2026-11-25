//! The webview's content security policy (0.9, "The webview shows untrusted text"; R5), as `tauri.conf.json` sets it in
//! `app.security.csp`. Tauri sends it as a header with the page it serves from the bundle. The test bridge and the
//! fake-owner page send the same one ([`built`]), so a browser test runs under the policy the app does.
//!
//! The strictest policy the built app runs under. No remote origin is named anywhere, and none of the directives has
//! `'unsafe-inline'` or `'unsafe-eval'`.
//!
//! - `default-src 'none'`: what no directive below names is blocked: frames, workers, media and manifests.
//! - `script-src 'self'`: only the bundle's own scripts. No inline script and no `eval`.
//! - `style-src 'self'`: only the bundle's stylesheet. The components set styles through the DOM, which is allowed.
//! - `img-src 'self' data:`: the app's own icons. An image in a task's text never loads.
//! - `font-src 'self'`: the bundle's own fonts. The app uses the system's.
//! - `connect-src ipc: http://ipc.localhost`: Tauri's own channel (`ipc:` where it uses a custom scheme, the host name
//!   where it does not). No `fetch` or websocket to anywhere else.
//! - `object-src 'none'`: no plug-ins.
//! - `base-uri 'none'`: an injected `<base>` cannot redirect the page's relative URLs.
//! - `form-action 'none'`: no form can post anywhere.
//! - `frame-ancestors 'none'`: no page can frame the window.
//!
//! `app.security.devCsp` is only for a dev run that has a page of its own with the Vite dev server's origin and
//! its hot-reload websocket. It adds loopback sources and `'unsafe-inline'` there and nowhere in `csp`. A test keeps
//! the two apart.

use serde_json::Value;

/// The configuration file, read when the program is built.
const CONFIG: &str = include_str!("../tauri.conf.json");

/// The directives of the policy under `key` (`csp` or `devCsp`), in file order, each with its sources.
pub fn directives(key: &str) -> Vec<(String, Vec<String>)> {
    let config: Value = serde_json::from_str(CONFIG).expect("tauri.conf.json is JSON");
    let policy = config["app"]["security"][key]
        .as_object()
        .unwrap_or_else(|| panic!("tauri.conf.json: app.security.{key} is a map of directives"));
    policy
        .iter()
        .map(|(name, sources)| {
            let sources = sources
                .as_array()
                .unwrap_or_else(|| panic!("app.security.{key}.{name} is a list of sources"))
                .iter()
                .map(|source| source.as_str().expect("a source is a string").to_string())
                .collect();
            (name.clone(), sources)
        })
        .collect()
}

/// The built app's policy as a header value: `name source source; name source`.
pub fn built() -> String {
    directives("csp")
        .iter()
        .map(|(name, sources)| format!("{name} {}", sources.join(" ")))
        .collect::<Vec<_>>()
        .join("; ")
}

/// The policy the test bridge sends with its page: [`built`], with `'self'` first in `connect-src`. The page there
/// calls the bridge over HTTP at its own origin, where the app calls Tauri's channel. Nothing else differs.
pub fn for_bridge_page() -> String {
    built().replace("connect-src ", "connect-src 'self' ")
}
