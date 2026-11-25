//! What the webview is allowed (0.9, "The webview shows untrusted text"; R5): the capability file grants the `rpc`
//! command, listen and unlisten, and nothing else, and the content security policy names no remote origin. The files
//! are read as they are; no Tauri runtime starts. The check in the bundled app's webview (the policy in force, no other
//! command callable) is the owner's, in `ts-0185-ge` and `ts-0197-uj`.

use collide_app::{RPC_COMMAND, csp};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::Path;

fn read(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

fn json_file(name: &str) -> Value {
    serde_json::from_str(&read(name)).unwrap_or_else(|error| panic!("{name} is not JSON: {error}"))
}

// ---- the capability ------------------------------------------------------------------------------------------------

/// What is wrong with a capability file, in words. Empty when it grants the command and the channel and nothing else.
fn capability_problems(capability: &Value) -> Vec<String> {
    let mut problems = Vec::new();
    if capability["windows"] != json!(["main"]) {
        problems.push(format!(
            "windows is {}, not [\"main\"]",
            capability["windows"]
        ));
    }
    let want = [
        format!("allow-{RPC_COMMAND}"),
        "core:event:allow-listen".to_string(),
        "core:event:allow-unlisten".to_string(),
    ];
    if capability["permissions"] != json!(want) {
        problems.push(format!(
            "permissions are {}, not {want:?}",
            capability["permissions"]
        ));
    }
    // A grant to a remote page, a webview that is not the window's, or a platform list widens who may call.
    for key in ["remote", "webviews", "platforms", "local"] {
        if capability.get(key).is_some() {
            problems.push(format!("`{key}` widens or narrows the grant"));
        }
    }
    problems
}

#[test]
fn the_main_window_may_call_rpc_listen_and_unlisten_and_nothing_else() {
    let capability = json_file("capabilities/main.json");
    assert_eq!(capability_problems(&capability), Vec::<String>::new());
}

#[test]
fn a_capability_that_grants_more_is_found() {
    let more = json!({
        "windows": ["main"],
        "permissions": ["allow-rpc", "core:event:allow-listen", "core:event:allow-unlisten", "core:default"],
    });
    assert_eq!(capability_problems(&more).len(), 1);
    let without_rpc = json!({
        "windows": ["main"],
        "permissions": ["core:event:allow-listen", "core:event:allow-unlisten"],
    });
    assert_eq!(capability_problems(&without_rpc).len(), 1);
    let any_window = json!({ "windows": ["*"], "permissions": ["allow-rpc"] });
    assert_eq!(capability_problems(&any_window).len(), 2);
}

#[test]
fn there_is_one_capability_file_so_no_other_grants_the_window_more() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("capabilities");
    let names: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, vec!["main.json".to_string()]);
}

#[test]
fn the_app_manifest_names_the_one_command_the_handler_serves() {
    // The command is named in an app manifest (`build.rs`), so a capability has to grant it; and it is the only one.
    assert!(read("build.rs").contains(&format!("commands(&[\"{RPC_COMMAND}\"])")));
    assert!(read("src/lib.rs").contains(&format!("generate_handler![{RPC_COMMAND}]")));
}

// ---- the content security policy -----------------------------------------------------------------------------------

type Policy = BTreeMap<String, Vec<String>>;

fn policy(key: &str) -> Policy {
    csp::directives(key).into_iter().collect()
}

/// The sources the built policy may name, and no others: what the bundle serves, and Tauri's own channel.
const ALLOWED_SOURCES: [&str; 5] = ["'self'", "'none'", "data:", "ipc:", "http://ipc.localhost"];

/// What is wrong with a built policy, in words.
fn csp_problems(policy: &Policy) -> Vec<String> {
    let mut problems = Vec::new();
    let want_directives = [
        "default-src",
        "script-src",
        "style-src",
        "img-src",
        "font-src",
        "connect-src",
        "object-src",
        "base-uri",
        "form-action",
        "frame-ancestors",
    ];
    let names: Vec<&str> = policy.keys().map(String::as_str).collect();
    for want in want_directives {
        if !names.contains(&want) {
            problems.push(format!("`{want}` is missing"));
        }
    }
    for name in &names {
        if !want_directives.contains(name) {
            problems.push(format!(
                "`{name}` is not a directive the policy is meant to have"
            ));
        }
    }
    for (name, sources) in policy {
        for source in sources {
            if !ALLOWED_SOURCES.contains(&source.as_str()) {
                problems.push(format!(
                    "`{name}` names `{source}`, which is not the bundle or Tauri's channel"
                ));
            }
        }
    }
    let only = |name: &str, want: &[&str]| {
        let got: Vec<&str> = policy
            .get(name)
            .map(|s| s.iter().map(String::as_str).collect())
            .unwrap_or_default();
        (got != want).then(|| format!("`{name}` is {got:?}, not {want:?}"))
    };
    let exact = [
        only("default-src", &["'none'"]),
        only("script-src", &["'self'"]),
        only("style-src", &["'self'"]),
        only("img-src", &["'self'", "data:"]),
        only("connect-src", &["ipc:", "http://ipc.localhost"]),
        only("object-src", &["'none'"]),
        only("base-uri", &["'none'"]),
        only("form-action", &["'none'"]),
        only("frame-ancestors", &["'none'"]),
    ];
    problems.extend(exact.into_iter().flatten());
    problems
}

#[test]
fn the_built_policy_is_the_strictest_and_names_no_remote_origin() {
    assert_eq!(csp_problems(&policy("csp")), Vec::<String>::new());
}

#[test]
fn the_built_policy_has_no_unsafe_source_and_no_origin_but_tauris_channel() {
    for (name, sources) in csp::directives("csp") {
        for source in sources {
            assert!(!source.contains("unsafe"), "{name} has {source}");
            assert!(!source.contains('*'), "{name} has {source}");
            // A source with `://` is an origin, and Tauri's channel is the only one.
            assert!(
                !source.contains("://") || source == "http://ipc.localhost",
                "{name} names {source}"
            );
        }
    }
}

#[test]
fn a_policy_with_a_remote_origin_or_unsafe_inline_is_found() {
    let mut remote = policy("csp");
    remote
        .get_mut("connect-src")
        .unwrap()
        .push("https://api.example.com".into());
    assert_eq!(
        csp_problems(&remote).len(),
        2,
        "{:?}",
        csp_problems(&remote)
    );
    let mut inline = policy("csp");
    inline
        .get_mut("script-src")
        .unwrap()
        .push("'unsafe-inline'".into());
    assert!(!csp_problems(&inline).is_empty());
    let mut missing = policy("csp");
    missing.remove("frame-ancestors");
    assert!(!csp_problems(&missing).is_empty());
}

#[test]
fn the_dev_policy_adds_only_loopback_and_inline_and_the_built_one_has_neither() {
    let built = policy("csp");
    let dev = policy("devCsp");
    assert_eq!(
        built.keys().collect::<Vec<_>>(),
        dev.keys().collect::<Vec<_>>()
    );
    let allowed_extra = ["'unsafe-inline'", "http://127.0.0.1:*", "ws://127.0.0.1:*"];
    for (name, sources) in &dev {
        for source in sources {
            let in_built = built[name].contains(source);
            assert!(
                in_built || allowed_extra.contains(&source.as_str()),
                "devCsp {name} adds {source}"
            );
        }
        for source in &built[name] {
            assert!(sources.contains(source), "devCsp {name} drops {source}");
        }
    }
    let header = csp::built();
    for extra in ["127.0.0.1", "ws:", "unsafe-"] {
        assert!(
            !header.contains(extra),
            "the built policy has {extra}: {header}"
        );
    }
}

#[test]
fn the_built_policy_is_one_line_of_these_directives() {
    let header = csp::built();
    assert!(!header.contains(['\n', '\r']), "{header}");
    let mut sent: Vec<&str> = header.split("; ").collect();
    sent.sort_unstable();
    assert_eq!(
        sent,
        [
            "base-uri 'none'",
            "connect-src ipc: http://ipc.localhost",
            "default-src 'none'",
            "font-src 'self'",
            "form-action 'none'",
            "frame-ancestors 'none'",
            "img-src 'self' data:",
            "object-src 'none'",
            "script-src 'self'",
            "style-src 'self'",
        ]
    );
}

#[test]
fn the_bridge_page_differs_from_the_built_policy_in_one_source_and_nothing_else() {
    // The test bridge's page reaches the bridge over HTTP at its own origin, where the app uses Tauri's channel.
    let built = csp::built();
    let bridge = csp::for_bridge_page();
    assert_ne!(built, bridge);
    assert_eq!(
        bridge.replace("connect-src 'self' ipc:", "connect-src ipc:"),
        built
    );
    assert!(!bridge.contains("unsafe-"), "{bridge}");
}

#[test]
fn a_configuration_with_no_policy_is_found() {
    // The configuration before this item: no `app.security`.
    let before: Value =
        serde_json::from_str(r#"{ "app": { "windows": [{ "title": "Collide" }] } }"#).unwrap();
    assert!(before["app"]["security"]["csp"].is_null());
    assert!(!json_file("tauri.conf.json")["app"]["security"]["csp"].is_null());
}
