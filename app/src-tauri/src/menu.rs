//! The app menu's one item of its own, "Start with defaults..." (ts-0215-gt; plan, Stage 6a, Start with defaults), as
//! plain code with no Tauri type, so a test needs no runtime. [`crate::run`] builds the menu and wires the event.
//!
//! The shell owns the item, not the page, so it works when the page is blank or broken. The item reloads the window
//! with `defaults` on the URL's fragment. The page sees it, tells the shell with `shell.set_defaults`
//! ([`crate::shell::SET_DEFAULTS`]), and the shell makes its connection to the daemon with the defaults flag on
//! `initialize`. The page cannot send `initialize` itself.

/// The id of the menu item. A menu event carries the id of the item that made it.
pub const START_WITH_DEFAULTS: &str = "start-with-defaults";

/// The text of the menu item.
pub const START_WITH_DEFAULTS_LABEL: &str = "Start with Defaults\u{2026}";

/// The item's accelerator: Command, Option, Shift and S. It holds Command on purpose. AppKit offers a key-down to the
/// menu's key equivalents before the first responder only when Command is down. A chord without Command reaches the
/// menu only if the web view leaves the key unhandled, and WebKit takes Escape itself, so the earlier chord
/// (Control-Option-Shift-Escape) did nothing in the built app (ts-0219-qf). macOS reserves Option-Command-Escape
/// (Force Quit, and held with Shift, force quit of the front app), Option-Command-D (the Dock), Option-Command-H
/// (Hide Others) and Option-Command-Shift-Q (log out); this chord is none of those, and the page binds no chord that
/// holds Command or Option.
pub const START_WITH_DEFAULTS_KEY: &str = "CmdOrCtrl+Alt+Shift+S";

/// Reloads the window. The app's wiring gives the webview's; a test gives one that records.
pub trait Reloader {
    /// Reloads the window with the defaults flag on its URL (`defaults`), or without it. The error says why it could
    /// not.
    fn reload(&self, defaults: bool) -> Result<(), String>;
}

/// The script that reloads the page: it sets or clears the `defaults` key of the fragment, keeps the other keys, then
/// reloads. A fixed text, so nothing a page or a person writes reaches the webview's `eval`.
pub fn reload_script(defaults: bool) -> &'static str {
    if defaults {
        "const keys = new URLSearchParams(location.hash.slice(1)); keys.set('defaults', ''); \
         location.hash = keys.toString(); location.reload()"
    } else {
        "const keys = new URLSearchParams(location.hash.slice(1)); keys.delete('defaults'); \
         history.replaceState(null, '', location.pathname + location.search + '#' + keys.toString()); \
         location.reload()"
    }
}

/// Handles one menu event. It returns whether the event was this module's: the item with [`START_WITH_DEFAULTS`] as its
/// id reloads the window in defaults mode, and any other id is left to the caller. A reload that fails is said on
/// stderr, as the shell says its other failures there.
pub fn on_menu_event(id: &str, reloader: &dyn Reloader) -> bool {
    if id != START_WITH_DEFAULTS {
        return false;
    }
    if let Err(why) = reloader.reload(true) {
        eprintln!("collide: could not start with defaults: {why}");
    }
    true
}
