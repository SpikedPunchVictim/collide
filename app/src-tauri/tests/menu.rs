//! The app menu's one item of its own, "Start with defaults..." (ts-0215-gt), as plain code: the handler of a menu
//! event and the script that reloads the window, with no Tauri runtime and no window. [`collide_app::run`] wires them.

use collide_app::menu::{
    Reloader, START_WITH_DEFAULTS, START_WITH_DEFAULTS_KEY, on_menu_event, reload_script,
};
use std::sync::{Arc, Mutex};

/// A reloader that keeps the modes it was asked for and reloads nothing.
#[derive(Clone, Default)]
struct Recording {
    asked: Arc<Mutex<Vec<bool>>>,
    fail: bool,
}

impl Reloader for Recording {
    fn reload(&self, defaults: bool) -> Result<(), String> {
        self.asked.lock().unwrap().push(defaults);
        if self.fail {
            Err("no window to reload".to_string())
        } else {
            Ok(())
        }
    }
}

impl Recording {
    fn asked(&self) -> Vec<bool> {
        self.asked.lock().unwrap().clone()
    }
}

#[test]
fn the_menu_item_asks_for_a_reload_in_defaults_mode() {
    let reloader = Recording::default();
    assert!(on_menu_event(START_WITH_DEFAULTS, &reloader));
    assert_eq!(reloader.asked(), [true]);
}

#[test]
fn an_event_of_another_item_reloads_nothing_and_is_not_taken() {
    let reloader = Recording::default();
    assert!(!on_menu_event("quit", &reloader));
    assert!(!on_menu_event("", &reloader));
    assert!(reloader.asked().is_empty(), "{:?}", reloader.asked());
}

#[test]
fn a_reload_that_fails_is_still_a_taken_event() {
    let reloader = Recording {
        fail: true,
        ..Recording::default()
    };
    assert!(on_menu_event(START_WITH_DEFAULTS, &reloader));
    assert_eq!(reloader.asked(), [true]);
}

#[test]
fn the_accelerator_is_control_option_shift_escape() {
    let parts: Vec<&str> = START_WITH_DEFAULTS_KEY.split('+').collect();
    assert_eq!(parts, ["Ctrl", "Alt", "Shift", "Escape"]);
}

#[test]
fn the_script_puts_the_defaults_flag_on_the_fragment_and_reloads_and_the_other_takes_it_off() {
    let on = reload_script(true);
    assert!(on.contains("keys.set('defaults', '')"), "{on}");
    assert!(on.contains("location.hash = keys.toString()"), "{on}");
    assert!(on.contains("location.reload()"), "{on}");
    let off = reload_script(false);
    assert!(off.contains("history.replaceState"), "{off}");
    assert!(off.contains("keys.delete('defaults')"), "{off}");
    assert!(!off.contains("keys.set"), "{off}");
    assert!(off.contains("location.reload()"), "{off}");
}
