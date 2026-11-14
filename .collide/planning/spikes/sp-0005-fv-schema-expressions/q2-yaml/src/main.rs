//! Q2: Rust YAML crates against the hazards that matter for pack.yaml / board.yaml / config.yaml.
//!
//! Subcommands (all print their results):
//!   scalars      the scalar table: one scalar written unquoted, deserialized into several Rust types
//!   templates    `{{ task.id }}` and other indicator-led plain scalars
//!   duplicates   duplicate mapping keys into a struct, a map and a Value
//!   errors       error text and line/column for the common mistakes
//!   merge        anchors, aliases and merge keys used legitimately
//!   bombs        alias expansion bombs and deep nesting, each in a child process with an RSS and time limit
//!   perf         parse time for a 1 MB manifest-shaped document
//!   all          everything above
//! Internal: `worker bomb <backend> <levels> <target>` and `worker deep <backend> <depth>`.

use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Backend {
    SerdeYaml09,
    SerdeYamlNg,
    SerdeNorway,
    SaphyrDefault,
    SaphyrStrictBool,
    SaphyrLint,
    SaphyrHardened,
}

const BACKENDS: [Backend; 7] = [
    Backend::SerdeYaml09,
    Backend::SerdeYamlNg,
    Backend::SerdeNorway,
    Backend::SaphyrDefault,
    Backend::SaphyrStrictBool,
    Backend::SaphyrLint,
    Backend::SaphyrHardened,
];

impl Backend {
    fn name(self) -> &'static str {
        match self {
            Backend::SerdeYaml09 => "serde_yaml 0.9.34 (archived)",
            Backend::SerdeYamlNg => "serde_yaml_ng 0.10.0",
            Backend::SerdeNorway => "serde_norway 0.9.42",
            Backend::SaphyrDefault => "serde-saphyr 2.1.1 default",
            Backend::SaphyrStrictBool => "serde-saphyr strict_booleans",
            Backend::SaphyrLint => "serde-saphyr strict_bool+no_schema",
            Backend::SaphyrHardened => "serde-saphyr hardened (see saphyr_options)",
        }
    }
    fn key(self) -> &'static str {
        match self {
            Backend::SerdeYaml09 => "serde_yaml",
            Backend::SerdeYamlNg => "serde_yaml_ng",
            Backend::SerdeNorway => "serde_norway",
            Backend::SaphyrDefault => "saphyr",
            Backend::SaphyrStrictBool => "saphyr_sb",
            Backend::SaphyrLint => "saphyr_lint",
            Backend::SaphyrHardened => "saphyr_hard",
        }
    }
    fn from_key(k: &str) -> Backend {
        *BACKENDS.iter().find(|b| b.key() == k).expect("unknown backend key")
    }
}

struct ErrInfo {
    msg: String,
    loc: Option<(u64, u64)>,
}

fn saphyr_options(b: Backend) -> serde_saphyr::Options {
    match b {
        Backend::SaphyrStrictBool => serde_saphyr::options! { strict_booleans: true },
        Backend::SaphyrLint => serde_saphyr::options! { strict_booleans: true, no_schema: true },
        // What the Collide loader would use: YAML 1.2 booleans only, quote-required strings, no anchors, aliases or
        // merge keys at all (a manifest has no use for them), duplicate keys an error (the default), and small caps.
        Backend::SaphyrHardened => serde_saphyr::options! {
            strict_booleans: true,
            no_schema: true,
            merge_keys: serde_saphyr::MergeKeyPolicy::Error,
            budget: serde_saphyr::budget! { max_aliases: 0, max_anchors: 0, max_nodes: 100_000, max_depth: 32 },
        },
        _ => serde_saphyr::Options::default(),
    }
}

fn parse<T: DeserializeOwned>(b: Backend, s: &str) -> Result<T, ErrInfo> {
    match b {
        Backend::SerdeYaml09 => serde_yaml_09::from_str(s).map_err(|e| ErrInfo {
            loc: e.location().map(|l| (l.line() as u64, l.column() as u64)),
            msg: e.to_string(),
        }),
        Backend::SerdeYamlNg => serde_yaml_ng::from_str(s).map_err(|e| ErrInfo {
            loc: e.location().map(|l| (l.line() as u64, l.column() as u64)),
            msg: e.to_string(),
        }),
        Backend::SerdeNorway => serde_norway::from_str(s).map_err(|e| ErrInfo {
            loc: e.location().map(|l| (l.line() as u64, l.column() as u64)),
            msg: e.to_string(),
        }),
        Backend::SaphyrDefault | Backend::SaphyrStrictBool | Backend::SaphyrLint | Backend::SaphyrHardened => {
            serde_saphyr::from_str_with_options(s, saphyr_options(b)).map_err(|e| ErrInfo {
                loc: e.location().map(|l| (l.line(), l.column())),
                msg: e.to_string(),
            })
        }
    }
}

#[derive(Deserialize, Debug)]
struct One<T> {
    k: T,
}

fn first_line(s: &str) -> String {
    let l = s.lines().next().unwrap_or("");
    let l = if l.len() > 90 { format!("{}...", &l[..90]) } else { l.to_string() };
    l
}

fn show<T: std::fmt::Debug>(r: Result<One<T>, ErrInfo>) -> String {
    match r {
        Ok(o) => format!("{:?}", o.k),
        Err(_) => "ERR".to_string(),
    }
}

/// Groups backends whose output is identical and prints one line per group.
fn print_grouped(rows: Vec<(Backend, String)>, indent: &str) {
    let mut groups: Vec<(String, Vec<&'static str>)> = vec![];
    for (b, text) in &rows {
        match groups.iter_mut().find(|(t, _)| t == text) {
            Some((_, names)) => names.push(b.key()),
            None => groups.push((text.clone(), vec![b.key()])),
        }
    }
    if groups.len() == 1 {
        println!("{indent}all backends: {}", groups[0].0);
    } else {
        for (text, names) in groups {
            println!("{indent}[{}] {}", names.join(","), text);
        }
    }
}

// ---------------------------------------------------------------- scalars
const SCALARS: &[&str] = &[
    "no", "yes", "on", "off", "No", "NO", "Yes", "YES", "On", "OFF", "y", "n", "Y", "N", "true", "True", "TRUE", "false", "FALSE",
    "null", "Null", "~", "", "0.10", "1.0", "1.10", "1.10.0", "0x1f", "0o17", "017", "0b101", "1e3", "1e10", "12e3", "1_000",
    ".5", "+1", "007", "123", "1:30", "1:30:00", ".inf", ".nan", "2026-11-13", "2026-11-13T10:00:00Z", "=", "<<",
];

fn scalars() {
    println!("== scalars: `k: <scalar>` (unquoted) into String | bool | i64 | f64 | Option<String> | serde_json::Value");
    println!("   Each line is one scalar; backends with identical output are grouped.");
    for sc in SCALARS {
        let doc = format!("k: {sc}\n");
        println!("scalar `{sc}`");
        let rows = BACKENDS
            .iter()
            .map(|&b| {
                let s = show(parse::<One<String>>(b, &doc));
                let bo = show(parse::<One<bool>>(b, &doc));
                let i = show(parse::<One<i64>>(b, &doc));
                let f = show(parse::<One<f64>>(b, &doc));
                let o = show(parse::<One<Option<String>>>(b, &doc));
                let v = show(parse::<One<Value>>(b, &doc));
                (b, format!("String={s} bool={bo} i64={i} f64={f} Option<String>={o} Value={v}"))
            })
            .collect();
        print_grouped(rows, "  ");
    }
}

// ---------------------------------------------------------------- templates
const TEMPLATES: &[(&str, &str)] = &[
    ("unquoted {{ task.id }}", "k: {{ task.id }}\n"),
    ("double-quoted", "k: \"{{ task.id }}\"\n"),
    ("single-quoted", "k: '{{ task.id }}'\n"),
    ("template inside plain text: `x {{ task.id }} y`", "k: x {{ task.id }} y\n"),
    ("template first, then text: `{{ task.id }}-x`", "k: {{ task.id }}-x\n"),
    ("template in a flow sequence: `[{{ task.id }}]`", "k: [{{ task.id }}]\n"),
    ("jinja block: `{% if x %}`", "k: {% if x %}\n"),
    ("GitHub style: `${{ secrets.X }}`", "k: ${{ secrets.X }}\n"),
    ("block scalar |", "k: |\n  {{ task.id }}\n"),
    ("leading `@task`", "k: @task\n"),
    ("leading backtick", "k: `x`\n"),
    ("leading `%x`", "k: %x\n"),
    ("leading `*x` (alias)", "k: *x\n"),
    ("leading `&x` (anchor, no value)", "k: &x\n"),
    ("leading `!x` (tag)", "k: !x\n"),
    ("colon-space inside: `a: b`", "k: a: b\n"),
    ("hash inside: `a #b` (comment)", "k: a #b\n"),
];

fn templates() {
    println!("== templates and indicator-led plain scalars into String (and into Value when String fails)");
    for (label, doc) in TEMPLATES {
        println!("{label}   {}", doc.trim_end().replace('\n', "\\n"));
        let rows = BACKENDS
            .iter()
            .map(|&b| {
                let s = match parse::<One<String>>(b, doc) {
                    Ok(o) => format!("String={:?}", o.k),
                    Err(e) => format!(
                        "ERR at {} : {}",
                        e.loc.map(|(l, c)| format!("{l}:{c}")).unwrap_or("?".into()),
                        first_line(&e.msg)
                    ),
                };
                (b, s)
            })
            .collect();
        print_grouped(rows, "  ");
    }
    println!("-- the unquoted {{{{ task.id }}}} into serde_json::Value (what a loose `config:` or a Value-typed field sees)");
    let rows = BACKENDS
        .iter()
        .map(|&b| {
            (b, match parse::<One<Value>>(b, "k: {{ task.id }}\n") {
                Ok(o) => format!("Value={}", o.k),
                Err(e) => format!("ERR: {}", first_line(&e.msg)),
            })
        })
        .collect();
    print_grouped(rows, "  ");
}

// ---------------------------------------------------------------- duplicates
#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
struct A {
    a: String,
}
#[derive(Deserialize, Debug)]
struct Outer {
    m: BTreeMap<String, String>,
}

fn duplicates() {
    println!("== duplicate mapping keys");
    let cases: Vec<(&str, &str)> = vec![
        ("struct field twice", "a: x\na: y\n"),
        ("flow mapping `{a: x, a: y}` into struct", "{a: x, a: y}\n"),
        ("quoted twin: `a` and `\"a\"`", "a: x\n\"a\": y\n"),
    ];
    for (label, doc) in cases {
        println!("{label}: {}", doc.trim_end().replace('\n', "\\n"));
        let rows = BACKENDS
            .iter()
            .map(|&b| {
                (b, match parse::<A>(b, doc) {
                    Ok(v) => format!("accepted -> {v:?}"),
                    Err(e) => format!("rejected at {}: {}", e.loc.map(|(l, c)| format!("{l}:{c}")).unwrap_or("?".into()), first_line(&e.msg)),
                })
            })
            .collect();
        print_grouped(rows, "  ");
    }
    let doc = "m:\n  lanePool: x\n  lanePool: y\n";
    println!("map field (`workflows:`-like BTreeMap) with a repeated key: {}", doc.trim_end().replace('\n', "\\n"));
    let rows = BACKENDS
        .iter()
        .map(|&b| {
            (b, match parse::<Outer>(b, doc) {
                Ok(v) => format!("accepted -> {:?}", v.m),
                Err(e) => format!("rejected at {}: {}", e.loc.map(|(l, c)| format!("{l}:{c}")).unwrap_or("?".into()), first_line(&e.msg)),
            })
        })
        .collect();
    print_grouped(rows, "  ");
    println!("same, into serde_json::Value:");
    let rows = BACKENDS
        .iter()
        .map(|&b| {
            (b, match parse::<Value>(b, doc) {
                Ok(v) => format!("accepted -> {v}"),
                Err(e) => format!("rejected: {}", first_line(&e.msg)),
            })
        })
        .collect();
    print_grouped(rows, "  ");
}

// ---------------------------------------------------------------- errors
#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
struct Man {
    name: String,
    version: String,
    #[serde(default)]
    workflows: BTreeMap<String, Wf>,
}
#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
struct Wf {
    phases: Vec<String>,
}

fn errors() {
    println!("== error text and location (line:column, as each crate reports them)");
    let cases: Vec<(&str, &str)> = vec![
        ("syntax: unterminated quote on line 2", "name: p\nversion: \"1\nworkflows: {}\n"),
        ("syntax: unclosed flow sequence at end of input", "name: p\nworkflows: [1, 2\n"),
        ("type: sequence where String expected (line 2)", "name: p\nversion: [1]\n"),
        ("unknown field `flavor` (line 3)", "name: p\nversion: \"1\"\nflavor: x\n"),
        ("missing field `version`", "name: p\n"),
        ("duplicate field `name` (line 2)", "name: p\nname: q\nversion: \"1\"\n"),
        ("tab indentation (line 3)", "name: p\nversion: \"1\"\nworkflows:\n\tw: {}\n"),
        ("deep: wrong type in workflows.w.phases (line 5)", "name: p\nversion: \"1\"\nworkflows:\n  w:\n    phases: nope\n    extra: 1\n"),
        ("unquoted version 0.10 into String", "name: p\nversion: 0.10\n"),
    ];
    for (label, doc) in cases {
        println!("{label}");
        for &b in &BACKENDS {
            match parse::<Man>(b, doc) {
                Ok(m) => println!("  {:<38} accepted: version={:?}", b.name(), m.version),
                Err(e) => println!(
                    "  {:<38} loc={:<6} {}",
                    b.name(),
                    e.loc.map(|(l, c)| format!("{l}:{c}")).unwrap_or("none".into()),
                    first_line(&e.msg)
                ),
            }
        }
    }
}

// ---------------------------------------------------------------- merge keys
#[derive(Deserialize, Debug)]
#[allow(dead_code)]
struct Merged {
    base: BTreeMap<String, i64>,
    derived: BTreeMap<String, i64>,
}

fn merge() {
    println!("== anchors, aliases and merge keys used as intended");
    let doc = "base: &b {x: 1, y: 2}\nderived:\n  <<: *b\n  y: 3\n";
    println!("{}", doc.trim_end().replace('\n', "\\n"));
    let rows = BACKENDS
        .iter()
        .map(|&b| {
            (b, match parse::<Merged>(b, doc) {
                Ok(v) => format!("derived={:?}", v.derived),
                Err(e) => format!("ERR: {}", first_line(&e.msg)),
            })
        })
        .collect();
    print_grouped(rows, "  ");
    let doc = "a: &x 1\nb: *x\n";
    let rows = BACKENDS
        .iter()
        .map(|&b| {
            (b, match parse::<BTreeMap<String, i64>>(b, doc) {
                Ok(v) => format!("{v:?}"),
                Err(e) => format!("ERR: {}", first_line(&e.msg)),
            })
        })
        .collect();
    println!("scalar alias `a: &x 1 / b: *x`:");
    print_grouped(rows, "  ");
}

// ---------------------------------------------------------------- bombs
fn bomb_doc(levels: usize) -> String {
    let mut s = String::from("l0: &l0 [x, x, x, x, x, x, x, x, x]\n");
    for i in 1..=levels {
        let p = i - 1;
        let refs = vec![format!("*l{p}"); 9].join(", ");
        s.push_str(&format!("l{i}: &l{i} [{refs}]\n"));
    }
    s
}

fn worker_bomb(b: Backend, levels: usize, target: &str) {
    let doc = bomb_doc(levels);
    let t0 = Instant::now();
    let res: Result<String, ErrInfo> = match target {
        "value" => parse::<Value>(b, &doc).map(|v| {
            // count nodes without recursion depth problems (depth is 2)
            fn count(v: &Value) -> u64 {
                match v {
                    Value::Array(a) => 1 + a.iter().map(count).sum::<u64>(),
                    Value::Object(o) => 1 + o.values().map(count).sum::<u64>(),
                    _ => 1,
                }
            }
            format!("parsed, {} value nodes", count(&v))
        }),
        "ignored" => parse::<serde::de::IgnoredAny>(b, &doc).map(|_| "parsed (IgnoredAny)".to_string()),
        _ => panic!("target"),
    };
    let ms = t0.elapsed().as_secs_f64() * 1000.0;
    match res {
        Ok(m) => println!("OK {m} in {ms:.0} ms"),
        Err(e) => println!("ERR {} (after {ms:.0} ms) {}", first_line(&e.msg), e.loc.map(|(l, c)| format!("at {l}:{c}")).unwrap_or_default()),
    }
}

fn worker_deep(b: Backend, depth: usize) {
    let doc = format!("k: {}{}\n", "[".repeat(depth), "]".repeat(depth));
    let t0 = Instant::now();
    let r = parse::<serde::de::IgnoredAny>(b, &doc);
    let ms = t0.elapsed().as_secs_f64() * 1000.0;
    match r {
        Ok(_) => println!("OK parsed depth {depth} in {ms:.0} ms"),
        Err(e) => println!("ERR {} (after {ms:.1} ms)", first_line(&e.msg)),
    }
}

struct Outcome {
    status: String,
    wall: f64,
    rss_mb: f64,
    stdout: String,
}

/// Runs this binary as a worker, polls its RSS with `ps`, kills it past the limits, and reads rusage via wait4.
fn run_limited(args: &[&str], rss_limit_mb: f64, wall_limit: Duration) -> Outcome {
    let exe = std::env::current_exe().unwrap();
    let mut child = std::process::Command::new(exe)
        .args(["worker"])
        .args(args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let pid = child.id() as libc::pid_t;
    let mut stdout = child.stdout.take().unwrap();
    let reader = std::thread::spawn(move || {
        let mut s = String::new();
        std::io::Read::read_to_string(&mut stdout, &mut s).ok();
        s
    });
    let t0 = Instant::now();
    let mut killed: Option<&str> = None;
    let mut peak_poll_mb = 0f64;
    let (status, rusage) = loop {
        let mut st: libc::c_int = 0;
        let mut ru: libc::rusage = unsafe { std::mem::zeroed() };
        let r = unsafe { libc::wait4(pid, &mut st, libc::WNOHANG, &mut ru) };
        if r == pid {
            break (st, ru);
        }
        let out = std::process::Command::new("ps").args(["-o", "rss=", "-p", &pid.to_string()]).output().unwrap();
        if let Ok(kb) = String::from_utf8_lossy(&out.stdout).trim().parse::<f64>() {
            peak_poll_mb = peak_poll_mb.max(kb / 1024.0);
            if kb / 1024.0 > rss_limit_mb && killed.is_none() {
                killed = Some("killed: RSS limit");
                unsafe { libc::kill(pid, libc::SIGKILL) };
            }
        }
        if t0.elapsed() > wall_limit && killed.is_none() {
            killed = Some("killed: wall limit");
            unsafe { libc::kill(pid, libc::SIGKILL) };
        }
        std::thread::sleep(Duration::from_millis(15));
    };
    let wall = t0.elapsed().as_secs_f64();
    let out = reader.join().unwrap();
    let rss_mb = (rusage.ru_maxrss as f64 / 1024.0 / 1024.0).max(peak_poll_mb);
    let status_text = if let Some(k) = killed {
        k.to_string()
    } else if libc::WIFEXITED(status) {
        format!("exit {}", libc::WEXITSTATUS(status))
    } else {
        format!("signal {}", libc::WTERMSIG(status))
    };
    Outcome { status: status_text, wall, rss_mb, stdout: out.trim().to_string() }
}

fn bombs() {
    let rss_limit = 3000.0;
    let wall = Duration::from_secs(25);
    println!("== alias expansion bomb. Level N: l0 is 9 scalars; each level is 9 aliases of the previous one,");
    println!("   so level N expands to 9^(N+1) scalars. Target serde_json::Value. Limits: {rss_limit} MB RSS, {} s.", wall.as_secs());
    println!("{:<38} {:>6} {:>16} {:>9} {:>9}  {}", "backend", "level", "expanded leaves", "wall s", "peak MB", "result");
    for &b in &BACKENDS {
        let mut stopped = false;
        for levels in [2usize, 4, 5, 6, 7, 8] {
            if stopped {
                break;
            }
            let leaves = 9u64.pow(levels as u32 + 1);
            let o = run_limited(&["bomb", b.key(), &levels.to_string(), "value"], rss_limit, wall);
            println!("{:<38} {:>6} {:>16} {:>9.2} {:>9.0}  {} | {}", b.name(), levels, leaves, o.wall, o.rss_mb, o.status, o.stdout);
            if o.status.starts_with("killed") || o.stdout.starts_with("ERR") {
                stopped = true;
            }
        }
    }
    println!("-- same bomb into IgnoredAny (the caller discards the data; does the parser still replay it?)");
    for &b in &BACKENDS {
        for levels in [5usize, 6, 7] {
            let leaves = 9u64.pow(levels as u32 + 1);
            let o = run_limited(&["bomb", b.key(), &levels.to_string(), "ignored"], rss_limit, wall);
            println!("{:<38} {:>6} {:>16} {:>9.2} {:>9.0}  {} | {}", b.name(), levels, leaves, o.wall, o.rss_mb, o.status, o.stdout);
            if o.status.starts_with("killed") || o.stdout.starts_with("ERR") {
                break;
            }
        }
    }
    println!("== deep nesting: `k: [[[...]]]` into IgnoredAny, each in a child process");
    println!("{:<38} {:>9} {:>9} {:>9}  {}", "backend", "depth", "wall s", "peak MB", "result");
    for &b in &BACKENDS {
        for depth in [100usize, 1_000, 100_000] {
            let o = run_limited(&["deep", b.key(), &depth.to_string()], rss_limit, wall);
            println!("{:<38} {:>9} {:>9.2} {:>9.0}  {} | {}", b.name(), depth, o.wall, o.rss_mb, o.status, o.stdout);
        }
    }
}

// ---------------------------------------------------------------- perf
fn perf_doc() -> String {
    let mut s = String::from("name: bigpack\nversion: \"1.0\"\nworkflows:\n");
    for i in 0..3000 {
        s.push_str(&format!(
            "  wf{i}:\n    phases: [build, test, review]\n    note: \"workflow number {i} with some text to give the scalars weight\"\n    extra: {{a: {i}, b: [1, 2, 3], c: true}}\n"
        ));
    }
    s
}

fn perf() {
    let doc = perf_doc();
    println!("== parse time for a {:.2} MB manifest-shaped document (3000 workflows), median of 5, release build", doc.len() as f64 / 1e6);
    for &b in &BACKENDS {
        let mut times = vec![];
        let mut ok = true;
        for _ in 0..5 {
            let t0 = Instant::now();
            let r = parse::<Value>(b, &doc);
            times.push(t0.elapsed().as_secs_f64() * 1000.0);
            if r.is_err() {
                ok = false;
            }
        }
        times.sort_by(|a, b| a.partial_cmp(b).unwrap());
        println!("  {:<38} {:>8.1} ms {}", b.name(), times[2], if ok { "" } else { "(ERR)" });
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("scalars") => scalars(),
        Some("templates") => templates(),
        Some("duplicates") => duplicates(),
        Some("errors") => errors(),
        Some("merge") => merge(),
        Some("bombs") => bombs(),
        Some("perf") => perf(),
        Some("all") => {
            scalars();
            templates();
            duplicates();
            errors();
            merge();
            bombs();
            perf();
        }
        Some("worker") => match args[1].as_str() {
            "bomb" => worker_bomb(Backend::from_key(&args[2]), args[3].parse().unwrap(), &args[4]),
            "deep" => worker_deep(Backend::from_key(&args[2]), args[3].parse().unwrap()),
            _ => panic!("worker kind"),
        },
        _ => eprintln!("usage: s5-yaml scalars|templates|duplicates|errors|merge|bombs|perf|all"),
    }
}
