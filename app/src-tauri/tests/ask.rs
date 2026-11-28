//! An ask end to end (ts-0201-us): the shell, the listener of the UI-tools endpoint, a daemon in this process that
//! starts its child under the real shim, and fake Claude. The fake calls a tool of the endpoint, the call reaches a
//! fake window as `ui_tools.request`, the window's reply comes back into the fake's stream, and the ask's events and
//! outcome come back to the window.
//!
//! The rules: the daemon is on a root of the test's own, its login shell clears the environment, and `claude` is a
//! script that plays a recording of `host/tests/adapters/claude/fixtures/`. Tokens and secrets live in
//! `MemorySecretStore`, never the keychain. Nothing here starts a real `claude` or reaches the network. The shim is the
//! `collide` binary the workspace builds (`cargo build --workspace`, or `cargo test --workspace`).

mod common;

use collide_app::endpoint::{EndpointServer, Log, MemoryPortStore};
use collide_app::shell::{
    AskConnector, AskRelay, ChannelEvent, ChannelWindow, EventChannel, RecordingOpener, Shell,
    ShellHandle, SocketAskConnector, SocketLink,
};
use collide_core::owner::{Request, RpcError, code};
use collide_core::ui_tools::{REQUEST_EVENT, ToolManifest};
use collide_host::base_env::CaptureOptions;
use collide_host::clock::SystemClocks;
use collide_host::daemon::{Daemon, DaemonOptions};
use collide_host::secret_store::MemorySecretStore;
use collide_host::ui_tools::{
    ENDPOINT_TOKEN_ITEM, HttpRequest, SecretStoreTokens, UiToolsEndpoint,
};
use common::{AlreadyThere, SharedLog, StopsAtThisTest, TempRoot, TestClocks, access, wait_until};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const WAIT: Duration = Duration::from_secs(30);
const PYTHON: &str = "/usr/bin/python3";
const REPLY_TEXT: &str = "the field is empty";

// ---- the pieces -------------------------------------------------------------------------------------------------

/// A window that keeps what it was sent.
#[derive(Clone, Default)]
struct Window(Arc<Mutex<Vec<ChannelEvent>>>);

impl Window {
    fn events(&self) -> Vec<ChannelEvent> {
        self.0.lock().unwrap().clone()
    }

    fn of(&self, method: &str) -> Vec<ChannelEvent> {
        self.events()
            .into_iter()
            .filter(|event| event.method == method)
            .collect()
    }

    fn wait_for(&self, what: &str, method: &str, count: usize) -> Vec<ChannelEvent> {
        wait_until(what, WAIT, || self.of(method).len() >= count);
        self.of(method)
    }
}

impl EventChannel for Window {
    fn is_open(&self) -> bool {
        true
    }

    fn emit(&self, event: &ChannelEvent) {
        self.0.lock().unwrap().push(event.clone());
    }
}

/// The real connector, which keeps the token of each `ask.start` it sent, so a test can look for it everywhere.
struct Recording {
    inner: SocketAskConnector,
    tokens: Arc<Mutex<Vec<String>>>,
}

impl AskConnector for Recording {
    fn start(&mut self, request: &Request, relay: AskRelay) -> Result<(), RpcError> {
        if let Some(token) = request.params["token"].as_str() {
            self.tokens.lock().unwrap().push(token.to_string());
        }
        self.inner.start(request, relay)
    }
}

static WORLDS: AtomicU32 = AtomicU32::new(0);

struct World {
    /// Makes the ask ids of this world its own.
    unique: String,
    root: TempRoot,
    shell: Arc<ShellHandle>,
    server: Option<EndpointServer>,
    window: Window,
    daemon: Option<Daemon>,
    log: SharedLog,
    secrets: Arc<MemorySecretStore>,
    ask_tokens: Arc<Mutex<Vec<String>>>,
    app_log: Arc<Mutex<Vec<String>>>,
    /// The folders the asks of the entries `mine` and `other` read, which are their children's working folders.
    mine: PathBuf,
    other: PathBuf,
}

/// The path of the shim: the `collide` binary beside the test's own `deps` folder.
fn collide_binary() -> PathBuf {
    let exe = fs::canonicalize(std::env::current_exe().unwrap()).unwrap();
    let path = exe
        .parent()
        .and_then(Path::parent)
        .expect("the test executable is in target/<profile>/deps")
        .join("collide");
    assert!(
        path.is_file(),
        "{} is not built: run `cargo build --workspace` first",
        path.display()
    );
    path
}

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../host/tests/adapters/claude/fixtures")
}

/// Writes `file` as an executable by a rename, so a file that already ran is never written over.
fn write_executable(file: &Path, text: &str) {
    let fresh = file.with_extension("new");
    fs::write(&fresh, text).unwrap();
    fs::set_permissions(&fresh, fs::Permissions::from_mode(0o755)).unwrap();
    fs::rename(&fresh, file).unwrap();
}

impl World {
    fn new(recording: &str) -> World {
        let root = TempRoot::new();
        let bin = root.path().join("bin");
        fs::create_dir(&bin).unwrap();
        write_executable(
            &bin.join("claude"),
            &format!(
                "#!/bin/sh\nif [ \"$*\" = \"--version\" ]; then echo \"2.1.325 (Claude Code)\"; exit 0; fi\n\
                 exec {PYTHON} {} {} \"$@\"\n",
                fixtures().join("fake_claude.py").display(),
                fixtures()
                    .join("recordings")
                    .join(format!("{recording}.jsonl"))
                    .display(),
            ),
        );
        let shell = root.path().join("fake-shell");
        write_executable(
            &shell,
            &format!(
                "#!/bin/sh\nexec /usr/bin/env -i HOME='{home}' COLLIDE_DAEMON_ROOT='{home}' \
                 PATH='{bin}:/usr/bin:/bin' /bin/sh -c \"$4\"\n",
                home = root.path().display(),
                bin = bin.display()
            ),
        );
        let home = root.path().join("home");
        let folder = |name: &str| {
            let dir = root.path().join(name);
            fs::create_dir(&dir).unwrap();
            fs::canonicalize(dir).unwrap()
        };
        let (mine, other) = (folder("read-mine"), folder("read-other"));
        fs::create_dir_all(home.join(".config/collide")).unwrap();
        fs::write(
            home.join(".config/collide/config.yaml"),
            format!(
                "agents:\n  mine:\n    kind: claude-code\n    model: haiku\n    read: [{}]\n  other:\n    kind: \
                 claude-code\n    model: haiku\n    read: [{}]\n  outside:\n    kind: external\n",
                mine.display(),
                other.display()
            ),
        )
        .unwrap();

        let secrets = Arc::new(MemorySecretStore::new());
        let log = SharedLog::default();
        let paths = root.paths();
        let mut options = DaemonOptions::new(
            paths.clone(),
            shell.display().to_string(),
            Box::new(log.clone()),
        );
        options.process_tree = Some(Arc::new(StopsAtThisTest(
            collide_host::daemon::system_process_tree(&paths, None),
        )));
        options.home = Some(home.display().to_string());
        options.capture = CaptureOptions {
            timeout: Duration::from_secs(10),
            cap_bytes: 1024 * 1024,
            fallback: BTreeMap::from([("PATH".to_string(), "/usr/bin:/bin".to_string())]),
        };
        options.shim_exe = Some(collide_binary());
        options.secret_store = secrets.clone();
        options.git_env = vec![
            ("HOME".to_string(), home.display().to_string()),
            ("GIT_CONFIG_GLOBAL".to_string(), "/dev/null".to_string()),
            ("GIT_CONFIG_SYSTEM".to_string(), "/dev/null".to_string()),
            ("GIT_CONFIG_NOSYSTEM".to_string(), "1".to_string()),
        ];
        let daemon = Daemon::start(options).expect("the daemon starts");

        let window = Window::default();
        let ask_tokens: Arc<Mutex<Vec<String>>> = Arc::default();
        let app_log: Arc<Mutex<Vec<String>>> = Arc::default();
        let access = access(&root, AlreadyThere, &TestClocks::default());
        let shell = {
            let (window, tokens, secrets, lines) = (
                window.clone(),
                ask_tokens.clone(),
                secrets.clone(),
                app_log.clone(),
            );
            let log: Log = Arc::new(move |line| lines.lock().unwrap().push(line.to_string()));
            Arc::new(ShellHandle::spawn(move || {
                let endpoint = UiToolsEndpoint::new(
                    0,
                    manifest(),
                    Box::new(SecretStoreTokens::new(secrets, log)),
                    Box::new(ChannelWindow::new(Arc::new(window.clone()))),
                    Box::new(SystemClocks),
                )
                .expect("the fixture manifest is valid");
                let connector = Recording {
                    inner: SocketAskConnector::new(Arc::clone(&access)),
                    tokens,
                };
                Shell::new(
                    endpoint,
                    Box::new(SocketLink::new(access)),
                    Box::new(RecordingOpener::default()),
                )
                .with_asks(Box::new(connector), Arc::new(window))
            }))
        };
        let lines = app_log.clone();
        let server = EndpointServer::start(
            &MemoryPortStore::default(),
            Arc::clone(&shell),
            Arc::new(move |line| lines.lock().unwrap().push(line.to_string())),
        )
        .expect("the listener starts");
        World {
            unique: format!(
                "{}-{}",
                std::process::id(),
                WORLDS.fetch_add(1, Ordering::SeqCst)
            ),
            root,
            shell,
            server: Some(server),
            window,
            daemon: Some(daemon),
            log,
            secrets,
            ask_tokens,
            app_log,
            mine,
            other,
        }
    }

    /// The id of the ask called `name` in this world. A sweep by `COLLIDE_RUN` finds a process of any test with the
    /// same id, so no two tests share one.
    fn id(&self, name: &str) -> String {
        format!("{name}-{}", self.unique)
    }

    fn start(&self, name: &str, agent: &str) -> Result<Value, RpcError> {
        self.shell.rpc(
            "ask.start".to_string(),
            Some(json!({ "ask": self.id(name), "agent": agent, "request": "show the field" })),
        )
    }

    /// Answers the window's pending call `n` (from 1) with `text`, as the window does.
    fn reply(&self, n: usize, text: &str) {
        let requests = self
            .window
            .wait_for("the call reaches the window", REQUEST_EVENT, n);
        let id = requests[n - 1].params["requestId"]
            .as_str()
            .expect("a request id");
        let answer = self.shell.rpc(
            "ui_tools.reply".to_string(),
            Some(json!({ "requestId": id, "content": [{ "type": "text", "text": text }] })),
        );
        assert_eq!(answer, Ok(json!({})));
    }

    fn outcomes(&self, count: usize) -> Vec<Value> {
        self.window
            .wait_for("the outcomes reach the window", "ask.outcome", count)
            .into_iter()
            .map(|event| event.params)
            .collect()
    }

    fn pid_in(folder: &Path) -> u32 {
        let file = folder.join("fake.pid");
        wait_until("the fake wrote its pid", WAIT, || {
            fs::read_to_string(&file).is_ok_and(|text| text.trim().parse::<u32>().is_ok())
        });
        fs::read_to_string(&file).unwrap().trim().parse().unwrap()
    }

    fn endpoint_token(&self) -> String {
        use collide_host::secret_store::SecretStore;
        self.secrets
            .get(ENDPOINT_TOKEN_ITEM)
            .expect("the endpoint token was made")
            .expose()
            .to_string()
    }

    fn port(&self) -> u16 {
        self.server.as_ref().unwrap().port()
    }

    /// The status the endpoint answers a `tools/call` with `token` with.
    fn status_with(&self, token: &str) -> u16 {
        let body = json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": { "name": "query", "arguments": { "surface": "field" } }
        });
        self.shell
            .http(HttpRequest {
                method: "POST".into(),
                path: "/mcp".into(),
                headers: vec![
                    ("Host".into(), format!("127.0.0.1:{}", self.port())),
                    ("Content-Type".into(), "application/json".into()),
                    ("Authorization".into(), format!("Bearer {token}")),
                ],
                body: serde_json::to_vec(&body).unwrap(),
            })
            .status
    }
}

impl Drop for World {
    fn drop(&mut self) {
        // The window goes first, which cancels each live ask; the daemon then stops what is left.
        self.shell.window_gone();
        drop(self.server.take());
        drop(self.daemon.take());
    }
}

fn manifest() -> ToolManifest {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/ui-tools/manifest.json");
    let text = fs::read_to_string(path).expect("the manifest fixture");
    ToolManifest::from_json(&text).expect("the fixture is a manifest")
}

fn files_under(dir: &Path, found: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        match fs::symlink_metadata(&path) {
            Ok(meta) if meta.is_dir() => files_under(&path, found),
            Ok(meta) if meta.is_file() => found.push(path),
            _ => {}
        }
    }
}

/// Every file under `root` that holds `needle`. A socket or a link is not read.
fn files_holding(root: &Path, needle: &str) -> Vec<PathBuf> {
    let mut files = Vec::new();
    files_under(root, &mut files);
    files
        .into_iter()
        .filter(|file| {
            fs::read(file).is_ok_and(|bytes| String::from_utf8_lossy(&bytes).contains(needle))
        })
        .collect()
}

// ---- the tests --------------------------------------------------------------------------------------------------

#[test]
fn the_fakes_call_reaches_the_window_and_the_reply_comes_back_into_its_stream() {
    let world = World::new("ask-call");
    assert_eq!(world.start("e1", "mine"), Ok(json!({})));
    world.reply(1, REPLY_TEXT);
    let outcomes = world.outcomes(1);
    assert_eq!(outcomes[0]["ask"], world.id("e1").as_str());
    assert_eq!(outcomes[0]["outcome"]["end"], json!({ "type": "finished" }));

    // The call the window was sent is the agent's, as the ask's caller.
    let request = &world.window.of(REQUEST_EVENT)[0].params;
    assert_eq!(request["caller"], world.id("e1").as_str());

    // The call is in the events the window got. The daemon's events have no result of a tool, so the reply is looked
    // for in the stream the fake wrote, which the phase keeps.
    let events = world.window.of("ask.event");
    let seen =
        serde_json::to_string(&events.iter().map(|e| &e.params).collect::<Vec<_>>()).unwrap();
    assert!(
        seen.contains("mcp__collide__query"),
        "the call is not in the stream: {seen}"
    );
    let stream = files_holding(&world.root.path().join("data/runs"), REPLY_TEXT);
    assert!(
        !stream.is_empty(),
        "the reply did not come back into the fake's stream"
    );

    // The call comes before the end of the ask; nothing follows the outcome.
    let order: Vec<String> = world
        .window
        .events()
        .into_iter()
        .map(|e| e.method)
        .collect();
    assert_eq!(order.last().map(String::as_str), Some("ask.outcome"));
    assert!(
        order.iter().position(|m| m == REQUEST_EVENT)
            < order.iter().position(|m| m == "ask.outcome")
    );
}

#[test]
fn an_ask_token_is_dead_after_its_ask_ends() {
    let world = World::new("ask-call");
    world.start("e1", "mine").unwrap();
    world.reply(1, REPLY_TEXT);
    world.outcomes(1);
    let token = world.ask_tokens.lock().unwrap()[0].clone();
    assert_eq!(
        world.status_with(&token),
        401,
        "an ended ask's token must be refused"
    );
}

#[test]
fn neither_token_is_in_a_log_a_file_or_an_event() {
    let world = World::new("ask-call");
    world.start("e1", "mine").unwrap();
    world.reply(1, REPLY_TEXT);
    world.outcomes(1);
    let ask_token = world.ask_tokens.lock().unwrap()[0].clone();
    let endpoint_token = world.endpoint_token();
    assert_ne!(ask_token, endpoint_token);
    let events = serde_json::to_string(
        &world
            .window
            .events()
            .iter()
            .map(|e| (&e.method, &e.params))
            .collect::<Vec<_>>(),
    )
    .unwrap();
    let daemon_log = world.log.text();
    let app_log = world.app_log.lock().unwrap().join("\n");
    for (name, token) in [
        ("ask token", &ask_token),
        ("endpoint token", &endpoint_token),
    ] {
        assert!(
            !events.contains(token.as_str()),
            "the {name} is in an event"
        );
        assert!(
            !daemon_log.contains(token.as_str()),
            "the {name} is in the daemon's log"
        );
        assert!(
            !app_log.contains(token.as_str()),
            "the {name} is in the listener's log"
        );
        let files = files_holding(world.root.path(), token);
        assert!(files.is_empty(), "the {name} is in files: {files:?}");
    }
}

#[test]
fn the_ask_folder_has_a_record_and_no_mcp_json_after_the_ask() {
    let world = World::new("ask-call");
    world.start("e1", "mine").unwrap();
    world.reply(1, REPLY_TEXT);
    world.outcomes(1);
    let dir = world.root.path().join("data/asks").join(world.id("e1"));
    wait_until("the record is written", WAIT, || {
        dir.join("record.json").is_file()
    });
    assert!(
        !dir.join("mcp.json").exists(),
        "mcp.json is left in the ask's folder"
    );
    let record: Value =
        serde_json::from_str(&fs::read_to_string(dir.join("record.json")).unwrap()).unwrap();
    assert_eq!(record["outcome"]["end"], json!({ "type": "finished" }));
    assert_eq!(
        record["tool_calls"].as_array().map(Vec::len),
        Some(1),
        "{record}"
    );
}

#[test]
fn closing_the_window_cancels_every_live_ask_and_each_child_is_gone() {
    let world = World::new("ask-hold");
    assert_eq!(world.start("h1", "mine"), Ok(json!({})));
    assert_eq!(world.start("h2", "other"), Ok(json!({})));
    let pids = [World::pid_in(&world.mine), World::pid_in(&world.other)];
    world.shell.window_gone();
    let outcomes = world.outcomes(2);
    for outcome in &outcomes {
        assert_eq!(
            outcome["outcome"]["end"],
            json!({ "type": "cancelled" }),
            "{outcome}"
        );
    }
    let mut asks: Vec<&str> = outcomes
        .iter()
        .map(|o| o["ask"].as_str().unwrap())
        .collect();
    asks.sort_unstable();
    let mut expected = [world.id("h1"), world.id("h2")];
    expected.sort_unstable();
    assert_eq!(asks, expected);
    for pid in pids {
        wait_until("the child is gone", WAIT, || !common::pid_alive(pid));
    }
}

#[test]
fn a_cancel_of_an_ask_that_has_ended_is_answered() {
    let world = World::new("ask-hold");
    world.start("h1", "mine").unwrap();
    World::pid_in(&world.mine);
    let h1 = json!({ "ask": world.id("h1") });
    let cancelled = world.shell.rpc("ask.cancel".to_string(), Some(h1.clone()));
    assert_eq!(cancelled, Ok(json!({})));
    world.outcomes(1);
    // The window and the daemon race at the end: a second cancel is answered all the same.
    assert_eq!(
        world.shell.rpc("ask.cancel".to_string(), Some(h1)),
        Ok(json!({}))
    );
}

#[test]
fn an_external_agent_and_an_empty_request_are_refused_and_start_nothing() {
    let world = World::new("ask-call");
    let external = world
        .start("x1", "outside")
        .expect_err("an external agent cannot answer an ask");
    assert_eq!(external.code, code::CONFLICT);
    assert!(
        external.message.contains("external"),
        "{}",
        external.message
    );
    let unknown = world
        .start("x2", "nobody")
        .expect_err("an unknown agent is refused");
    assert_eq!(unknown.code, code::NOT_FOUND);
    let empty = world
        .shell
        .rpc(
            "ask.start".to_string(),
            Some(json!({ "ask": world.id("x3"), "agent": "mine", "request": "" })),
        )
        .expect_err("an empty request is refused");
    assert_eq!(empty.code, code::INVALID_PARAMS);
    assert!(
        world.window.events().is_empty(),
        "{:?}",
        world.window.events()
    );
    assert!(
        !world
            .root
            .path()
            .join("data/asks")
            .join(world.id("x1"))
            .exists()
    );
    // A refusal ends the id's token, so the same id starts an ask that works.
    assert_eq!(world.start("x1", "mine"), Ok(json!({})));
}
