//! The harness of the live half of the contract test (ts-0070-mk): it runs the real, pinned `claude` on the adapter's own
//! launch line, drives the Claude session with what the binary writes, and keeps what it wrote. Nothing here runs unless
//! `COLLIDE_REAL_CLAUDE=1` is set ([`unavailable`]); a test that is skipped says why.
//!
//! **What a real run may touch.** The child gets an environment of its own (`env_clear()`): `PATH`, `LANG`, the owner's
//! `HOME` and `USER` (the login lives under the real home: a temp `HOME` is "not logged in", measured on 2.1.325), and
//! the auto-updater off. Every case runs with config none (`--setting-sources ""`), so the owner's settings, hooks,
//! `CLAUDE.md` and plugins do not load, and with auto memory off (the snapshot's settings). The working directory,
//! the snapshot and everything Collide writes are in a `TempRoot`. Every case has a budget cap on its line
//! (`--max-budget-usd`), and the spend ledger ([`LEDGER_ENV`]) refuses a run that could take the total over its limit.
//!
//! **What is kept.** With `COLLIDE_RECORD_DIR` set, a case writes its stream to `<dir>/<case>.jsonl` after [`Redactor`]
//! has replaced everything that names the machine, the account or a session. The raw stream is only ever in memory.
//!
//! **What a recording is for.** [`Shapes`] reads the recordings in `fixtures/real-recordings/` and keeps, for each kind of
//! line, the paths every recorded line of that kind had and the JSON types they had. A live line that lacks one of those
//! paths, or has another type there, fails the contract test: that is format drift.

use super::common::TempRoot;
use collide_core::adapter::{AgentAdapter, AgentSession, StampedEvent};
use collide_core::agent::{
    AgentEvent, ModelRef, PermissionReply, PermissionRequest, SessionStart, TurnEndReason,
};
use collide_core::pack::Role;
use collide_core::testing::sample_launch_request;
use collide_host::adapters::claude::ClaudeAdapter;
use collide_host::adapters::claude::session::ClaudeSession;
use collide_host::adapters::claude::snapshot::snapshot_files;
use collide_host::builtin_roles::BUILDER_DEFINITION;
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::{Duration, Instant};

/// The file the spend ledger is kept in, when the run keeps one: one line a run, `name`, `cap`, `cost`, tab separated.
pub const LEDGER_ENV: &str = "COLLIDE_REAL_LEDGER";
/// The most the runs of one ledger may add up to, in US dollars (ts-0070-mk: $1.50 for the item, re-runs included).
pub const LEDGER_LIMIT_USD: f64 = 1.50;
/// The cap of one contract case (`--max-budget-usd`).
pub const CASE_CAP_USD: f64 = 0.05;

/// How long a case waits for a line from `claude`.
const PATIENCE: Duration = Duration::from_secs(120);

/// Why a live test cannot run, or `None` when it can.
pub fn unavailable() -> Option<String> {
    if std::env::var("COLLIDE_REAL_CLAUDE").as_deref() != Ok("1") {
        return Some(
            "COLLIDE_REAL_CLAUDE=1 is not set (this test runs the real `claude` and spends money)"
                .to_string(),
        );
    }
    if !binary().is_file() {
        return Some(format!("there is no `claude` at {}", binary().display()));
    }
    None
}

/// The `claude` under test: `COLLIDE_REAL_CLAUDE_BIN`, else `~/.local/bin/claude`.
pub fn binary() -> PathBuf {
    match std::env::var_os("COLLIDE_REAL_CLAUDE_BIN") {
        Some(path) => PathBuf::from(path),
        None => {
            PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".local/bin/claude")
        }
    }
}

/// The environment of a real `claude` child, and nothing else: see the module's note on what it may touch.
pub fn child_env() -> BTreeMap<String, String> {
    let var = |name: &str| std::env::var(name).unwrap_or_default();
    BTreeMap::from([
        ("PATH".into(), "/usr/bin:/bin".into()),
        ("LANG".into(), "en_US.UTF-8".into()),
        ("HOME".into(), var("HOME")),
        ("USER".into(), var("USER")),
        ("DISABLE_AUTOUPDATER".into(), "1".into()),
    ])
}

// ---- the spend ledger ------------------------------------------------------------------------------------------------

fn ledger_total(path: &Path) -> f64 {
    fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter_map(|line| line.split('\t').nth(2)?.parse::<f64>().ok())
        .sum()
}

/// Refuses (panics) when `cap` added to what the ledger has spent is over [`LEDGER_LIMIT_USD`]. No ledger, no check.
pub fn check_ledger(name: &str, cap: f64) {
    let Some(path) = std::env::var_os(LEDGER_ENV) else {
        println!("no {LEDGER_ENV}: the spend of `{name}` is not added up");
        return;
    };
    let spent = ledger_total(Path::new(&path));
    assert!(
        spent + cap <= LEDGER_LIMIT_USD + 1e-9,
        "`{name}` is not run: ${spent:.4} spent and a cap of ${cap} would pass ${LEDGER_LIMIT_USD}"
    );
}

/// Adds one run to the ledger.
pub fn add_to_ledger(name: &str, cap: f64, cost: f64) {
    println!("spend: {name} cap ${cap} cost ${cost:.6}");
    if let Some(path) = std::env::var_os(LEDGER_ENV) {
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .expect("the ledger opens");
        writeln!(file, "{name}\t{cap}\t{cost}").expect("the ledger is written");
    }
}

// ---- a run -------------------------------------------------------------------------------------------------------------

/// A folder for a case: the working directory of `claude`, the snapshots, the recordings' scratch.
pub struct Live {
    pub root: TempRoot,
    pub work: PathBuf,
}

impl Live {
    pub fn new() -> Live {
        let root = TempRoot::new();
        let work = root.join("work");
        fs::create_dir(&work).unwrap();
        Live { root, work }
    }
}

/// One case: the turns to send, how the session starts, and who answers a permission request.
pub struct Case<'a> {
    pub name: &'a str,
    pub turns: Vec<&'a str>,
    pub session: SessionStart,
    pub cap_usd: f64,
    /// Hooks added to the snapshot's `settings.json`: event name to hook entries.
    pub extra_hooks: Vec<(&'a str, Value)>,
    pub answer: Box<dyn FnMut(&PermissionRequest) -> PermissionReply + 'a>,
}

impl<'a> Case<'a> {
    /// A fresh session of one turn that allows every request.
    pub fn new(name: &'a str, prompt: &'a str) -> Case<'a> {
        Case {
            name,
            turns: vec![prompt],
            session: SessionStart::Assign { uuid: new_uuid() },
            cap_usd: CASE_CAP_USD,
            extra_hooks: Vec::new(),
            answer: Box::new(|_| PermissionReply::Allow {}),
        }
    }
}

/// A random version-4 UUID for `--session-id`, from the system's random source.
pub fn new_uuid() -> String {
    let mut bytes = [0u8; 16];
    fs::File::open("/dev/urandom")
        .and_then(|mut f| std::io::Read::read_exact(&mut f, &mut bytes))
        .expect("the system's random source");
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &hex[..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..]
    )
}

/// What one run of `claude` left, as the session read it.
pub struct Outcome {
    /// Every line of stdout, as JSON, in order.
    pub lines: Vec<Value>,
    /// When each line arrived, from the start of the run.
    pub arrived: Vec<Duration>,
    pub events: Vec<StampedEvent>,
    /// `None` for an outcome read from a recording.
    pub code: Option<i32>,
    pub stderr: String,
    pub argv: Vec<String>,
    pub wall: Duration,
}

impl Outcome {
    /// `type` or `type/subtype`, as the shapes are keyed.
    pub fn kind(line: &Value) -> String {
        let text = |key| line.get(key).and_then(Value::as_str);
        match (text("type"), text("subtype")) {
            (Some(kind), Some(sub)) => format!("{kind}/{sub}"),
            (Some(kind), None) => kind.to_string(),
            _ => "?".to_string(),
        }
    }

    pub fn of_kind(&self, kind: &str) -> Vec<&Value> {
        self.lines
            .iter()
            .filter(|line| Outcome::kind(line) == kind)
            .collect()
    }

    pub fn kinds(&self) -> BTreeSet<String> {
        self.lines.iter().map(Outcome::kind).collect()
    }

    pub fn agent_events(&self) -> Vec<&AgentEvent> {
        self.events.iter().map(|s| &s.event).collect()
    }

    /// The last `result` line.
    pub fn result(&self) -> &Value {
        self.lines
            .iter()
            .rev()
            .find(|line| line.get("type").and_then(Value::as_str) == Some("result"))
            .expect("the stream has a result line")
    }

    pub fn cost(&self) -> f64 {
        self.lines
            .iter()
            .rev()
            .find_map(|line| line.get("total_cost_usd").and_then(Value::as_f64))
            .unwrap_or(0.0)
    }

    /// The most lines that arrived within any one second.
    pub fn peak_lines_per_second(&self) -> usize {
        (0..self.arrived.len())
            .map(|from| {
                self.arrived[from..]
                    .iter()
                    .take_while(|at| **at < self.arrived[from] + Duration::from_secs(1))
                    .count()
            })
            .max()
            .unwrap_or(0)
    }

    /// An outcome read from a recording: its lines fed to a session that starts as `start`, and nothing else. Used by the
    /// default tests, which run nothing.
    pub fn from_recording(lines: Vec<Value>, start: SessionStart) -> Outcome {
        let mut session = ClaudeSession::new(start).expecting_canary();
        session.send_turn("a recorded turn");
        let mut events = Vec::new();
        for (n, line) in lines.iter().enumerate() {
            events.extend(
                session
                    .feed(line.to_string().as_bytes(), n as u64 + 1)
                    .events,
            );
        }
        Outcome {
            arrived: Vec::new(),
            lines,
            events,
            code: None,
            stderr: String::new(),
            argv: Vec::new(),
            wall: Duration::ZERO,
        }
    }
}

/// Kills the child if the case ends early.
struct Reaped(Option<Child>);

impl Drop for Reaped {
    fn drop(&mut self) {
        if let Some(mut child) = self.0.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn write_all(stdin: &mut Option<ChildStdin>, writes: &[Vec<u8>]) {
    for bytes in writes {
        let stdin = stdin.as_mut().expect("stdin is open");
        stdin.write_all(bytes).expect("claude reads stdin");
        stdin.flush().expect("claude reads stdin");
    }
}

/// Runs `case` on the real binary, in `live.work`, on the adapter's launch line with config none. Returns what it
/// left, adds its cost to the ledger and, with `COLLIDE_RECORD_DIR`, writes the redacted stream there.
pub fn run(live: &Live, case: Case) -> Outcome {
    check_ledger(case.name, case.cap_usd);
    let Case {
        name,
        turns,
        session: start,
        cap_usd,
        extra_hooks,
        mut answer,
    } = case;

    let mut request = sample_launch_request();
    request.role_name = "collide/builder".into();
    request.role = serde_json::from_value::<Role>(json!({ "agent": "claude", "config": "none" }))
        .expect("a role");
    request.session = start.clone();
    request.model = Some(ModelRef {
        name: "haiku".into(),
        id: None,
    });
    request.cwd = live.work.clone();
    request.budget_left_usd = Some(cap_usd);
    request.snapshot_dir = live.root.join(&format!("snap-{name}"));
    fs::create_dir(&request.snapshot_dir).unwrap();
    let files = snapshot_files(&request, BUILDER_DEFINITION, &Map::new()).expect("the snapshot");
    for (file, text) in files.named() {
        let text = if file == "settings.json" && !extra_hooks.is_empty() {
            with_hooks(text, &extra_hooks)
        } else {
            text.to_string()
        };
        fs::write(request.snapshot_dir.join(file), text).unwrap();
    }
    let spec = ClaudeAdapter.launch(&request).expect("the launch line");

    let began = Instant::now();
    let mut child = Command::new(binary())
        .args(&spec.argv)
        .current_dir(&live.work)
        .env_clear()
        .envs(child_env())
        .envs(&spec.env)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("claude starts");
    let mut stdin = child.stdin.take();
    let stdout = child.stdout.take().expect("piped");
    let mut stderr = child.stderr.take().expect("piped");
    let mut reaped = Reaped(Some(child));
    let (send, lines) = mpsc::channel::<(Duration, Vec<u8>)>();
    std::thread::spawn(move || {
        let mut stdout = BufReader::new(stdout);
        loop {
            let mut line = Vec::new();
            match stdout.read_until(b'\n', &mut line) {
                Ok(0) | Err(_) => return,
                Ok(_) => {
                    if send.send((began.elapsed(), line)).is_err() {
                        return;
                    }
                }
            }
        }
    });
    let stderr = std::thread::spawn(move || {
        let mut text = String::new();
        let _ = std::io::Read::read_to_string(&mut stderr, &mut text);
        text
    });

    let mut session = ClaudeAdapter.session(&request);
    let mut events: Vec<StampedEvent> = Vec::new();
    let step = session.start();
    events.extend(step.events);
    let mut turns = turns.into_iter();
    let step = session.send_turn(turns.next().expect("a turn"));
    events.extend(step.events);
    write_all(&mut stdin, &step.writes);

    let mut raw: Vec<Value> = Vec::new();
    let mut arrived = Vec::new();
    loop {
        let (at, bytes) = match lines.recv_timeout(PATIENCE) {
            Ok(line) => line,
            Err(RecvTimeoutError::Disconnected) => break,
            Err(RecvTimeoutError::Timeout) => {
                panic!("`{name}`: claude wrote nothing for {PATIENCE:?}")
            }
        };
        let text = String::from_utf8_lossy(&bytes);
        let value: Value = serde_json::from_str(text.trim_end())
            .unwrap_or_else(|e| panic!("`{name}`: a line of stdout is not JSON ({e})"));
        arrived.push(at);
        raw.push(value);
        let step = session.feed(text.trim_end().as_bytes(), at.as_nanos() as u64);
        for stamped in step.events {
            let event = stamped.event.clone();
            events.push(stamped);
            match event {
                AgentEvent::PermissionRequest { request } => {
                    let reply = answer(&request);
                    let step = session.answer(&request.id, reply);
                    events.extend(step.events);
                    write_all(&mut stdin, &step.writes);
                }
                AgentEvent::TurnEnd { .. } => match turns.next() {
                    Some(text) => {
                        let step = session.send_turn(text);
                        events.extend(step.events);
                        write_all(&mut stdin, &step.writes);
                    }
                    None => stdin = None,
                },
                _ => {}
            }
        }
    }
    drop(stdin);
    let code = reaped
        .0
        .take()
        .expect("not reaped")
        .wait()
        .expect("claude is reaped")
        .code();
    let outcome = Outcome {
        lines: raw,
        arrived,
        events,
        code,
        stderr: stderr.join().expect("stderr"),
        argv: spec.argv,
        wall: began.elapsed(),
    };
    add_to_ledger(name, cap_usd, outcome.cost());
    if let Some(dir) = std::env::var_os("COLLIDE_RECORD_DIR") {
        let mut redactor = Redactor::new(live);
        let text: String = outcome
            .lines
            .iter()
            .map(|line| format!("{}\n", redactor.line(line)))
            .collect();
        fs::write(Path::new(&dir).join(format!("{name}.jsonl")), text).expect("the recording");
    }
    outcome
}

/// `settings` with `hooks` added: each event's entries are appended to the snapshot's own.
fn with_hooks(settings: &str, hooks: &[(&str, Value)]) -> String {
    let mut value: Value = serde_json::from_str(settings).expect("settings are JSON");
    let events = value["hooks"].as_object_mut().expect("settings have hooks");
    for (event, entry) in hooks {
        match events.entry(event.to_string()).or_insert_with(|| json!([])) {
            Value::Array(entries) => entries.push(entry.clone()),
            _ => panic!("hooks.{event} is not a list"),
        }
    }
    serde_json::to_string_pretty(&value).unwrap()
}

/// The turn of a result: `true` when the outcome's last turn ended `end_turn`.
pub fn ended_normally(outcome: &Outcome) -> bool {
    outcome.agent_events().iter().rev().any(|event| {
        matches!(
            event,
            AgentEvent::TurnEnd {
                reason: TurnEndReason::EndTurn {},
                ..
            }
        )
    })
}

// ---- redaction ---------------------------------------------------------------------------------------------------------

/// The stand-in session id the fixtures use.
pub const SESSION_STAND_IN: &str = "7c1b0e52-9f3a-4d2e-8b61-0a5d3c9e4f17";

/// Keys whose values are account details. Removed wherever they are.
const ACCOUNT_KEYS: [&str; 9] = [
    "credential_expires_at",
    "email",
    "account",
    "accountUuid",
    "organization",
    "organizationUuid",
    "orgId",
    "orgName",
    "emailAddress",
];

/// What a recording must not hold after [`Redactor`]: a recording that does is refused whole.
const FORBIDDEN: [&str; 6] = [
    "/Users/",
    "/private/",
    "/var/folders",
    "/tmp/",
    "sk-ant-",
    "eyJ",
];

/// The redaction rule, applied to every string in every line before anything is saved:
///
/// 1. the run's working directory becomes `/work/repo`, its snapshot `/work/snap`, the rest of its temp root `/work`,
///    the owner's home `/home/user` and the directory the binary is installed in `/opt/claude`;
/// 2. the owner's user name becomes `user`, and anything that looks like an e-mail address `user@example.com`;
/// 3. a `session_id` becomes [`SESSION_STAND_IN`] (a second session, one that ends in 8), any other UUID a counted
///    stand-in from `00000000-0000-4000-8000-000000000010`; the `msg_`, `toolu_`, `req_` and `srvtoolu_` ids that are an
///    `id`, `tool_use_id` or `parent_tool_use_id` count up as `msg_01`, ...; a `request_id` becomes `req-01`, ...; the
///    messaging socket path (`/tmp/cc-socks/<pid>.sock`, which `claude` makes outside its working directory) becomes
///    `/work/cc-socks/1.sock`;
/// 4. an account key ([`ACCOUNT_KEYS`]) is removed, and the figures, reset times and billing state in a
///    `rate_limit_info` become fixed values of the same type;
/// 5. the result is refused whole if it still holds a [`FORBIDDEN`] word, the user name or the real home.
///
/// The prompts in a recording are the case's own text and so need no rule. Nothing is kept that is not in the stream.
pub struct Redactor {
    paths: Vec<(String, String)>,
    user: String,
    home: String,
    uuids: HashMap<String, String>,
    ids: HashMap<String, String>,
    requests: HashMap<String, String>,
    sessions: HashMap<String, String>,
}

impl Redactor {
    pub fn new(live: &Live) -> Redactor {
        let canonical = |path: &Path| fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        let text = |path: PathBuf| path.display().to_string();
        let mut paths = Vec::new();
        for (from, to) in [
            (live.work.clone(), "/work/repo"),
            (live.root.path().to_path_buf(), "/work"),
        ] {
            paths.push((text(canonical(&from)), to.to_string()));
            paths.push((text(from), to.to_string()));
        }
        if let Some(dir) = fs::canonicalize(binary())
            .ok()
            .and_then(|b| b.parent().map(Path::to_path_buf))
        {
            paths.push((text(dir), "/opt/claude".to_string()));
        }
        Redactor::with(
            paths,
            std::env::var("USER").unwrap_or_default(),
            std::env::var("HOME").unwrap_or_default(),
        )
    }

    /// A redactor for these directories (to become the stand-ins above), user name and home.
    pub fn with(mut paths: Vec<(String, String)>, user: String, home: String) -> Redactor {
        paths.push((home.clone(), "/home/user".to_string()));
        // The snapshot is below the root, so its name is kept apart from `/work` by this order: longest first.
        paths.sort_by_key(|(from, _)| std::cmp::Reverse(from.len()));
        paths.retain(|(from, _)| !from.is_empty());
        Redactor {
            paths,
            user,
            home,
            uuids: HashMap::new(),
            ids: HashMap::new(),
            requests: HashMap::new(),
            sessions: HashMap::new(),
        }
    }

    /// The line, redacted. Panics when the result still holds something the rule forbids.
    pub fn line(&mut self, line: &Value) -> Value {
        let mut redacted = self.value(line, None);
        if let Some(info) = redacted.get_mut("rate_limit_info") {
            neutralize_limits(info);
        }
        let text = redacted.to_string();
        for word in FORBIDDEN {
            assert!(!text.contains(word), "redaction left `{word}` in: {text}");
        }
        assert!(
            self.user.is_empty() || !text.contains(&self.user),
            "redaction left the user name"
        );
        assert!(
            self.home.is_empty() || !text.contains(&self.home),
            "redaction left the home"
        );
        redacted
    }

    fn value(&mut self, value: &Value, key: Option<&str>) -> Value {
        match value {
            Value::String(text) => Value::String(self.text(text, key)),
            Value::Array(items) => Value::Array(items.iter().map(|v| self.value(v, key)).collect()),
            Value::Object(map) => Value::Object(
                map.iter()
                    .filter(|(k, _)| !ACCOUNT_KEYS.contains(&k.as_str()))
                    .map(|(k, v)| (k.clone(), self.value(v, Some(k))))
                    .collect(),
            ),
            other => other.clone(),
        }
    }

    fn text(&mut self, text: &str, key: Option<&str>) -> String {
        if key == Some("request_id") {
            let next = self.requests.len() + 1;
            return self
                .requests
                .entry(text.to_string())
                .or_insert_with(|| format!("req-{next:02}"))
                .clone();
        }
        if key == Some("session_id") {
            let next = self.sessions.len();
            return self
                .sessions
                .entry(text.to_string())
                .or_insert_with(|| {
                    // The first session is the fixtures' stand-in; a second one differs in its last digit.
                    format!(
                        "{}{}",
                        &SESSION_STAND_IN[..35],
                        char::from(b'7' + next as u8)
                    )
                })
                .clone();
        }
        if key == Some("messaging_socket_path") {
            return "/work/cc-socks/1.sock".to_string();
        }
        let mut out = text.to_string();
        for (from, to) in &self.paths {
            out = out.replace(from.as_str(), to);
        }
        if !self.user.is_empty() {
            out = out.replace(&self.user, "user");
        }
        out = replace_emails(&out);
        for (found, stand_in) in &self.sessions {
            out = out.replace(found.as_str(), stand_in);
        }
        out = self.replace_uuids(&out);
        if matches!(key, Some("id" | "tool_use_id" | "parent_tool_use_id")) {
            for prefix in ["srvtoolu_", "toolu_", "msg_", "req_"] {
                out = self.replace_prefixed(&out, prefix);
            }
        }
        out
    }

    fn replace_uuids(&mut self, text: &str) -> String {
        let bytes = text.as_bytes();
        let mut out = String::new();
        let mut at = 0;
        while at < text.len() {
            if at + 36 <= text.len() && is_uuid(&bytes[at..at + 36]) && !word_byte_before(bytes, at)
            {
                let found = &text[at..at + 36];
                let next = self.uuids.len();
                let stand_in = self
                    .uuids
                    .entry(found.to_string())
                    .or_insert_with(|| format!("00000000-0000-4000-8000-{:012}", next + 10))
                    .clone();
                out.push_str(&stand_in);
                at += 36;
            } else {
                let ch = text[at..].chars().next().unwrap();
                out.push(ch);
                at += ch.len_utf8();
            }
        }
        out
    }

    /// `msg_01AbC...` becomes `msg_01`, `msg_02`, ... in the order the ids are first seen.
    fn replace_prefixed(&mut self, text: &str, prefix: &str) -> String {
        let mut out = String::new();
        let mut rest = text;
        while let Some(at) = rest.find(prefix) {
            let (before, after) = rest.split_at(at);
            let boundary = before
                .chars()
                .next_back()
                .is_none_or(|c| !c.is_alphanumeric());
            let tail = &after[prefix.len()..];
            let len = tail
                .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                .unwrap_or(tail.len());
            out.push_str(before);
            if boundary && len > 0 {
                let id = format!("{prefix}{}", &tail[..len]);
                let count = self.ids.keys().filter(|k| k.starts_with(prefix)).count() + 1;
                let stand_in = self
                    .ids
                    .entry(id)
                    .or_insert_with(|| format!("{prefix}{count:02}"))
                    .clone();
                out.push_str(&stand_in);
            } else {
                out.push_str(&after[..prefix.len() + len]);
            }
            rest = &tail[len..];
        }
        out.push_str(rest);
        out
    }
}

/// A `rate_limit_info` states the owner's own usage: its figures, reset times and billing state become fixed values of
/// the same type (`status` and `rateLimitType` stay: they are what the adapter reads).
fn neutralize_limits(info: &mut Value) {
    match info {
        Value::Object(map) => {
            for (key, value) in map.iter_mut() {
                match (key.as_str(), &*value) {
                    ("status" | "rateLimitType", _) => {}
                    ("resetsAt", Value::Number(_)) => *value = json!(1_800_000_000),
                    (_, Value::Number(_)) => *value = json!(0.5),
                    (_, Value::String(_)) => *value = json!("stand-in"),
                    _ => neutralize_limits(value),
                }
            }
        }
        Value::Array(items) => items.iter_mut().for_each(neutralize_limits),
        _ => {}
    }
}

fn word_byte_before(bytes: &[u8], at: usize) -> bool {
    at > 0 && (bytes[at - 1].is_ascii_alphanumeric())
}

fn is_uuid(bytes: &[u8]) -> bool {
    bytes.len() == 36
        && bytes.iter().enumerate().all(|(i, b)| match i {
            8 | 13 | 18 | 23 => *b == b'-',
            _ => b.is_ascii_hexdigit(),
        })
}

/// Each word that is `name@host.tld` becomes `user@example.com`.
fn replace_emails(text: &str) -> String {
    let mut out = Vec::new();
    for word in text.split(' ') {
        let core = word.trim_matches(|c: char| !(c.is_alphanumeric() || "@._+-".contains(c)));
        let looks_like_email = core.split_once('@').is_some_and(|(name, host)| {
            !name.is_empty() && host.contains('.') && !host.ends_with('.')
        });
        out.push(if looks_like_email {
            word.replace(core, "user@example.com")
        } else {
            word.to_string()
        });
    }
    out.join(" ")
}

// ---- shapes ------------------------------------------------------------------------------------------------------------

/// Keys under which the content is the model's or a tool's own: the shape stops at them.
const OPAQUE: [&str; 6] = [
    "input",
    "updatedInput",
    "content",
    "tool_input",
    "message_content",
    "result_meta",
];

/// Keys whose children are named by data (a model name), kept as one path.
const KEYED: [&str; 2] = ["modelUsage", "model_usage_since_last_result"];

fn type_of(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn paths_of(value: &Value, at: &str, out: &mut BTreeMap<String, BTreeSet<&'static str>>) {
    match value {
        Value::Object(map) => {
            out.entry(at.to_string()).or_default().insert("object");
            for (key, child) in map {
                let key = if KEYED.contains(&at.rsplit('.').next().unwrap_or("")) {
                    "*"
                } else {
                    key
                };
                let path = if at.is_empty() {
                    key.to_string()
                } else {
                    format!("{at}.{key}")
                };
                if OPAQUE.contains(&key) {
                    out.entry(path).or_default().insert(type_of(child));
                } else {
                    paths_of(child, &path, out);
                }
            }
        }
        Value::Array(items) => {
            out.entry(at.to_string()).or_default().insert("array");
            for item in items {
                paths_of(item, &format!("{at}[]"), out);
            }
        }
        leaf => {
            out.entry(at.to_string()).or_default().insert(type_of(leaf));
        }
    }
}

/// What the recorded lines of each kind have in common: the paths all of them had, and the types each path had.
#[derive(Debug, Default)]
pub struct Shapes {
    kinds: BTreeMap<String, BTreeMap<String, BTreeSet<&'static str>>>,
}

impl Shapes {
    /// The shapes of `lines`: for each kind, the paths present in every line of it.
    pub fn of(lines: &[Value]) -> Shapes {
        let mut seen: BTreeMap<String, Vec<BTreeMap<String, BTreeSet<&'static str>>>> =
            BTreeMap::new();
        for line in lines {
            let mut paths = BTreeMap::new();
            paths_of(line, "", &mut paths);
            paths.remove("");
            seen.entry(Outcome::kind(line)).or_default().push(paths);
        }
        let kinds = seen
            .into_iter()
            .map(|(kind, each)| {
                let mut common = each[0].clone();
                for paths in &each[1..] {
                    common.retain(|path, _| paths.contains_key(path));
                    for (path, types) in common.iter_mut() {
                        types.extend(paths[path].iter().copied());
                    }
                }
                (kind, common)
            })
            .collect();
        Shapes { kinds }
    }

    pub fn kinds(&self) -> BTreeSet<&str> {
        self.kinds.keys().map(String::as_str).collect()
    }

    /// How `line` differs from the recorded shape of its kind: a path it lacks, or a type it has that was never recorded.
    /// A kind that was never recorded differs in nothing. Extra paths are not drift.
    pub fn drift(&self, line: &Value) -> Vec<String> {
        let kind = Outcome::kind(line);
        let Some(recorded) = self.kinds.get(&kind) else {
            return Vec::new();
        };
        let mut now = BTreeMap::new();
        paths_of(line, "", &mut now);
        let mut found = Vec::new();
        for (path, types) in recorded {
            match now.get(path) {
                None => found.push(format!("{kind}: `{path}` is gone")),
                Some(have) => {
                    if !have.iter().all(|t| types.contains(t)) {
                        found.push(format!(
                            "{kind}: `{path}` is now {have:?}, recorded {types:?}"
                        ));
                    }
                }
            }
        }
        found
    }
}

/// The folder of the recordings made from the real binary.
pub fn recordings_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/adapters/claude/fixtures/real-recordings")
}

/// The lines of the recording `name`.
pub fn recorded(name: &str) -> Vec<Value> {
    let path = recordings_dir().join(format!("{name}.jsonl"));
    fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
        .lines()
        .map(|line| serde_json::from_str(line).expect("a recorded line is JSON"))
        .collect()
}

/// Every recording in the folder, as one list of lines.
pub fn all_recorded() -> Vec<Value> {
    let mut names: Vec<String> = fs::read_dir(recordings_dir())
        .map(|entries| {
            entries
                .flatten()
                .filter_map(|e| e.path().file_stem()?.to_str().map(String::from))
                .filter(|name| recordings_dir().join(format!("{name}.jsonl")).is_file())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names.iter().flat_map(|name| recorded(name)).collect()
}

// ---- a run with no turn ------------------------------------------------------------------------------------------------

/// What `get_context_usage` says on the adapter's fixed line plus `extra_args` and `extra_env`: one control request, no
/// turn, so no model call and no cost. This is how the contract test reads which context window `claude` works with.
pub fn context_usage(
    live: &Live,
    extra_args: &[&str],
    extra_env: &[(&str, &str)],
) -> Result<Value, String> {
    use collide_host::adapters::claude::launch::FIXED_ARGV;
    check_ledger("context-usage", CASE_CAP_USD);
    let mut command = Command::new(binary());
    command
        .args(FIXED_ARGV)
        .args(["--model", "haiku", "--max-budget-usd", "0.05"])
        .args(["--setting-sources", "", "--no-session-persistence"])
        .args(extra_args)
        .current_dir(&live.work)
        .env_clear()
        .envs(child_env())
        .envs(extra_env.iter().copied());
    let request = json!({"type": "control_request", "request_id": "r1", "request": {"subtype": "get_context_usage"}});
    let input = format!("{request}\n");
    let run = collide_host::bounded_run::run_bounded(
        command,
        Some(input.as_bytes()),
        Duration::from_secs(60),
    )
    .map_err(|e| format!("did not run: {e}"))?;
    add_to_ledger("context-usage", CASE_CAP_USD, 0.0);
    let stdout = String::from_utf8_lossy(&run.stdout);
    for line in stdout.lines() {
        if let Ok(value) = serde_json::from_str::<Value>(line)
            && value["type"] == "control_response"
        {
            return Ok(value["response"]["response"].clone());
        }
    }
    Err(format!("no control response: {}", stdout.trim()))
}
