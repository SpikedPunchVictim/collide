//! Q4: the `cel` crate (cel-rust 0.15.0) on an Orchard-shaped corpus. Shared by two binaries: the stock crate and a
//! copy with a cooperative step counter patched into `Value::resolve_val` (feature `patched`).
//!
//! Subcommands:
//!   ordinary                         ordinary view expressions, per-task, over 1567 tasks
//!   estimate [n]                     the static cost estimate for each expression, and (patched) the measured steps
//!   limits                           the nested-expression cases, each in a child process with RSS and wall limits
//!   scaling                          nested expressions at growing corpus sizes (exponent of the growth)
//!   hostile                          one task whose list field is big, evaluated with an innocent-looking expression
//!   threads                          evaluate on a thread with a deadline; what the thread does after
//!   parse                            deep and large expressions: parse and evaluate, each in a child process
//! Internal: `worker ...`.

use crate::corpus::{Task, corpus};
use cel::common::ast::{Expr, IdedExpr};
use cel::{Context, Env, Value};
use std::sync::Arc;
use std::time::{Duration, Instant};

pub const N_TASKS: usize = 1567;

pub const ORDINARY: &[(&str, &str)] = &[
    ("E1 status", "status == 'ready'"),
    ("E2 membership", "'plan:E-07' in collections || 'mvp' in labels"),
    ("E3 list macro", "size(depends_on) == 0 && labels.exists(l, l.startsWith('p'))"),
    ("E4 string", "title.contains('edge') && author == 'me'"),
    ("E5 regex", "id.matches('^T-0[0-9]-1.*') && category != 'dropped'"),
    ("E6 all+in", "depends_on.all(d, d.startsWith('T-')) && !(kind in ['bug', 'spike'])"),
];

pub const NESTED: &[(&str, &str)] = &[
    ("Q1 filter/exists (plan's example)", "tasks.filter(t, tasks.exists(u, t.id in u.depends_on))"),
    ("Q2 triple exists", "tasks.filter(t, tasks.exists(u, tasks.exists(v, v.id == t.id && u.id == v.id && false)))"),
    ("Q3 nested map (n^2 strings)", "tasks.map(t, tasks.map(u, t.id + u.id))"),
    ("Q4 nested map of maps (n^3)", "tasks.map(t, tasks.map(u, tasks.map(v, [t.id, u.id, v.id])))"),
];

pub fn env() -> Arc<Env> {
    Arc::new(Env::stdlib())
}

fn task_values(t: &Task) -> Vec<(&'static str, Value)> {
    let v = |x: &dyn erased::Ser| x.to_value();
    vec![
        ("key", v(&t.key)),
        ("id", v(&t.id)),
        ("source", v(&t.source)),
        ("title", v(&t.title)),
        ("status", v(&t.status)),
        ("category", v(&t.category)),
        ("kind", v(&t.kind)),
        ("labels", v(&t.labels)),
        ("collections", v(&t.collections)),
        ("depends_on", v(&t.depends_on)),
        ("author", v(&t.author)),
    ]
}

mod erased {
    use cel::Value;
    use serde::Serialize;
    pub trait Ser {
        fn to_value(&self) -> Value;
    }
    impl<T: Serialize> Ser for T {
        fn to_value(&self) -> Value {
            cel::to_value(self).expect("to_value")
        }
    }
}

#[cfg(feature = "patched")]
pub fn steps_set_limit(l: u64) {
    cel::steps::set_limit(l);
}
#[cfg(feature = "patched")]
pub fn steps_used() -> Option<u64> {
    Some(cel::steps::used())
}
#[cfg(not(feature = "patched"))]
pub fn steps_set_limit(_l: u64) {}
#[cfg(not(feature = "patched"))]
pub fn steps_used() -> Option<u64> {
    None
}

fn flavor() -> &'static str {
    if cfg!(feature = "patched") { "patched (step counter)" } else { "stock" }
}

// ------------------------------------------------------------------ static cost estimate
/// Upper bound on node evaluations, the unit the step counter counts. `tasks` has `n` elements and every other list
/// is assumed to hold at most `k` (the loader would enforce `k`). Saturating.
pub struct Sizes {
    pub n: u128,
    pub k: u128,
}

fn size_of(e: &IdedExpr, s: &Sizes) -> u128 {
    match &e.expr {
        Expr::Ident(name) if name == "tasks" => s.n,
        Expr::List(l) => l.elements.len() as u128,
        Expr::Comprehension(c) => size_of(&c.iter_range, s),
        Expr::Call(c) if c.func_name == "_+_" && c.args.len() == 2 => size_of(&c.args[0], s).saturating_add(size_of(&c.args[1], s)),
        _ => s.k,
    }
}

pub fn estimate(e: &IdedExpr, s: &Sizes) -> u128 {
    let sum = |it: &mut dyn Iterator<Item = u128>| it.fold(0u128, |a, b| a.saturating_add(b));
    match &e.expr {
        Expr::Unspecified | Expr::Literal(_) | Expr::Ident(_) => 1,
        Expr::Select(sel) => 1 + estimate(&sel.operand, s),
        Expr::Call(c) => {
            let mut cost = 1 + c.target.as_ref().map(|t| estimate(t, s)).unwrap_or(0);
            cost = cost.saturating_add(sum(&mut c.args.iter().map(|a| estimate(a, s))));
            if c.func_name == "@in" && c.args.len() == 2 {
                cost = cost.saturating_add(size_of(&c.args[1], s)); // a membership test scans the list
            }
            cost
        }
        Expr::List(l) => 1 + sum(&mut l.elements.iter().map(|x| estimate(x, s))),
        Expr::Map(m) => 1 + sum(&mut m.entries.iter().map(|en| match &en.expr {
            cel::common::ast::EntryExpr::MapEntry(me) => estimate(&me.key, s).saturating_add(estimate(&me.value, s)),
            cel::common::ast::EntryExpr::StructField(f) => estimate(&f.value, s),
        })),
        Expr::Struct(st) => 1 + sum(&mut st.entries.iter().map(|en| match &en.expr {
            cel::common::ast::EntryExpr::MapEntry(me) => estimate(&me.key, s).saturating_add(estimate(&me.value, s)),
            cel::common::ast::EntryExpr::StructField(f) => estimate(&f.value, s),
        })),
        Expr::Comprehension(c) => {
            let per_iter = estimate(&c.loop_cond, s).saturating_add(estimate(&c.loop_step, s));
            1u128
                .saturating_add(estimate(&c.iter_range, s))
                .saturating_add(estimate(&c.accu_init, s))
                .saturating_add(size_of(&c.iter_range, s).saturating_mul(per_iter))
                .saturating_add(estimate(&c.result, s))
        }
    }
}

pub fn node_count(e: &IdedExpr) -> usize {
    let sum = |it: &mut dyn Iterator<Item = usize>| it.sum::<usize>();
    1 + match &e.expr {
        Expr::Select(sel) => node_count(&sel.operand),
        Expr::Call(c) => c.target.as_ref().map(|t| node_count(t)).unwrap_or(0) + sum(&mut c.args.iter().map(node_count)),
        Expr::List(l) => sum(&mut l.elements.iter().map(node_count)),
        Expr::Comprehension(c) => [&c.iter_range, &c.accu_init, &c.loop_cond, &c.loop_step, &c.result].iter().map(|x| node_count(x)).sum(),
        _ => 0,
    }
}

// ------------------------------------------------------------------ helpers
fn summarize(v: &Value) -> String {
    match v {
        Value::List(l) => format!("list of {}", l.len()),
        Value::Bool(b) => format!("bool {b}"),
        other => format!("{other:?}").chars().take(60).collect(),
    }
}

fn nested_ctx(n: usize) -> Context<'static, 'static> {
    let tasks = corpus(n);
    let mut ctx = Context::with_env(env());
    ctx.add_variable_from_value("tasks", cel::to_value(&tasks).unwrap());
    ctx
}

fn run_nested(src: &str, n: usize, limit: Option<u64>) -> String {
    let env = env();
    let program = env.compile(src).expect("compile");
    let ctx = nested_ctx(n);
    steps_set_limit(limit.unwrap_or(u64::MAX));
    let t0 = Instant::now();
    let r = program.execute(&ctx);
    let ms = t0.elapsed().as_secs_f64() * 1000.0;
    let steps = steps_used().map(|s| format!(", {s} steps")).unwrap_or_default();
    match r {
        Ok(v) => format!("OK {} in {ms:.1} ms{steps}", summarize(&v)),
        Err(e) => format!("ERR {} after {ms:.1} ms{steps}", format!("{e}").chars().take(70).collect::<String>()),
    }
}

// ------------------------------------------------------------------ subcommands
fn ordinary() {
    println!("== ordinary view expressions, evaluated per task over {N_TASKS} tasks ({}), release build", flavor());
    let tasks = corpus(N_TASKS);
    let pre: Vec<_> = tasks.iter().map(task_values).collect();
    let env = env();
    let mut ctx = Context::with_env(env.clone());
    // bind-only pass
    let mut bind = vec![];
    for _ in 0..7 {
        let t0 = Instant::now();
        for vals in &pre {
            for (k, v) in vals {
                ctx.add_variable_from_value(*k, v.clone());
            }
        }
        bind.push(t0.elapsed().as_secs_f64() * 1000.0);
    }
    bind.sort_by(|a, b| a.partial_cmp(b).unwrap());
    println!("  binding the 11 task variables only: median {:.2} ms for {N_TASKS} tasks ({:.2} us/task)", bind[3], bind[3] * 1000.0 / N_TASKS as f64);
    println!("  {:<18} {:>12} {:>14} {:>16} {:>9} {:>11}", "expression", "compile us", "bind+eval ms", "bind+eval us/task", "matches", "steps/task");
    for (name, src) in ORDINARY {
        let t0 = Instant::now();
        let program = env.compile(src).expect("compile");
        let compile_us = t0.elapsed().as_secs_f64() * 1e6;
        let mut times = vec![];
        let mut matches = 0;
        let mut steps_per_task = None;
        for rep in 0..7 {
            let mut m = 0;
            steps_set_limit(u64::MAX);
            let t0 = Instant::now();
            for vals in &pre {
                for (k, v) in vals {
                    ctx.add_variable_from_value(*k, v.clone());
                }
                if let Ok(Value::Bool(true)) = program.execute(&ctx) {
                    m += 1;
                }
            }
            times.push(t0.elapsed().as_secs_f64() * 1000.0);
            matches = m;
            if rep == 0 {
                steps_per_task = steps_used().map(|s| s as f64 / N_TASKS as f64);
            }
        }
        times.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let med = times[3];
        println!(
            "  {:<18} {:>12.0} {:>14.2} {:>16.2} {:>9} {:>11}",
            name,
            compile_us,
            med,
            med * 1000.0 / N_TASKS as f64,
            matches,
            steps_per_task.map(|s| format!("{s:.1}")).unwrap_or("-".into())
        );
    }
}

fn estimate_cmd(n: usize) {
    println!("== static cost estimate (AST walk, before evaluation), tasks={n}, other lists <= 64 entries ({})", flavor());
    println!("   estimate = upper bound on node evaluations; steps = what the counter measured (patched build only)");
    println!("{:<36} {:>8} {:>16} {:>14} {:>12}", "expression", "nodes", "estimate", "steps measured", "estimate us");
    let env = env();
    let sizes = Sizes { n: n as u128, k: 64 };
    let all: Vec<(&str, &str)> = ORDINARY.iter().chain(NESTED.iter()).copied().collect();
    for (name, src) in all {
        let program = env.compile(src).unwrap();
        let t0 = Instant::now();
        let est = estimate(program.expression(), &sizes);
        let est_us = t0.elapsed().as_secs_f64() * 1e6;
        let measured = if cfg!(feature = "patched") && name.starts_with('Q') && !name.starts_with("Q4") {
            let ctx = nested_ctx(n);
            steps_set_limit(u64::MAX);
            let _ = program.execute(&ctx);
            steps_used().map(|s| s.to_string()).unwrap_or("-".into())
        } else if cfg!(feature = "patched") && name.starts_with('E') {
            let tasks = corpus(n);
            let mut ctx = Context::with_env(env.clone());
            steps_set_limit(u64::MAX);
            for t in &tasks {
                for (k, v) in task_values(t) {
                    ctx.add_variable_from_value(k, v);
                }
                let _ = program.execute(&ctx);
            }
            // an estimate is per evaluation; report the mean steps per task
            steps_used().map(|s| format!("{:.1}/task", s as f64 / n as f64)).unwrap_or("-".into())
        } else {
            "-".into()
        };
        println!("{:<36} {:>8} {:>16} {:>14} {:>12.1}", name, node_count(program.expression()), est, measured, est_us);
    }
}

fn scaling() {
    println!("== nested expressions at growing corpus sizes, in this process ({}), one evaluation each", flavor());
    for (idx, (name, src)) in NESTED.iter().take(3).enumerate() {
        println!("{name}: {src}");
        let mut prev: Option<f64> = None;
        // the triple exists is cubic: n=400 would take minutes and n=800 about 15 minutes
        let sizes: &[usize] = if idx == 1 { &[50, 100, 200] } else { &[100, 200, 400, 800] };
        for &n in sizes {
            let env = env();
            let program = env.compile(src).unwrap();
            let ctx = nested_ctx(n);
            steps_set_limit(u64::MAX);
            let t0 = Instant::now();
            let r = program.execute(&ctx);
            let ms = t0.elapsed().as_secs_f64() * 1000.0;
            let growth = prev.map(|p| format!("x{:.1}", ms / p)).unwrap_or("-".into());
            println!(
                "  n={n:<5} {:>10.1} ms  growth for doubling n: {:<6} {}{}",
                ms,
                growth,
                r.as_ref().map(summarize).unwrap_or_else(|e| format!("ERR {e}")),
                steps_used().map(|s| format!(", {s} steps")).unwrap_or_default()
            );
            prev = Some(ms);
        }
    }
}

fn hostile_cmd() {
    println!("== a hostile task: one list field with M entries, an innocent expression over it ({})", flavor());
    println!("   expression: labels.all(a, labels.exists(b, a == b))   (what a view author could write over `labels`)");
    for m in [1_000usize, 5_000, 20_000] {
        println!("  M={m}: {}", run_hostile(m, None));
    }
}

fn run_hostile(m: usize, limit: Option<u64>) -> String {
    let env = env();
    let program = env.compile("labels.all(a, labels.exists(b, a == b))").unwrap();
    let mut ctx = Context::with_env(env);
    let labels: Vec<String> = (0..m).map(|i| format!("label-{i}")).collect();
    ctx.add_variable_from_value("labels", cel::to_value(&labels).unwrap());
    steps_set_limit(limit.unwrap_or(u64::MAX));
    let t0 = Instant::now();
    let r = program.execute(&ctx);
    let ms = t0.elapsed().as_secs_f64() * 1000.0;
    let steps = steps_used().map(|s| format!(", {s} steps")).unwrap_or_default();
    match r {
        Ok(v) => format!("OK {} in {ms:.0} ms{steps}", summarize(&v)),
        Err(e) => format!("ERR {} after {ms:.0} ms{steps}", format!("{e}").chars().take(60).collect::<String>()),
    }
}

fn cpu_ms() -> f64 {
    let mut ru: libc::rusage = unsafe { std::mem::zeroed() };
    unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut ru) };
    let tv = |t: libc::timeval| t.tv_sec as f64 * 1000.0 + t.tv_usec as f64 / 1000.0;
    tv(ru.ru_utime) + tv(ru.ru_stime)
}
fn maxrss_mb() -> f64 {
    let mut ru: libc::rusage = unsafe { std::mem::zeroed() };
    unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut ru) };
    ru.ru_maxrss as f64 / 1024.0 / 1024.0
}

/// Runs `src` on a thread; the caller waits `deadline_ms`; then reports what the abandoned thread does.
fn threads_worker(src: &str, deadline_ms: u64, observe_s: u64) {
    let src = src.to_string();
    let (tx, rx) = std::sync::mpsc::channel();
    let cpu0 = cpu_ms();
    let t0 = Instant::now();
    std::thread::spawn(move || {
        let env = env();
        let program = env.compile(&src).unwrap();
        let ctx = nested_ctx(N_TASKS);
        let r = program.execute(&ctx);
        let _ = tx.send(r.map(|v| summarize(&v)).map_err(|e| e.to_string()));
    });
    match rx.recv_timeout(Duration::from_millis(deadline_ms)) {
        Ok(r) => println!("finished inside the deadline: {r:?} in {:.0} ms", t0.elapsed().as_secs_f64() * 1000.0),
        Err(_) => {
            println!("deadline {deadline_ms} ms hit at {:.0} ms: the caller moves on, process CPU so far {:.0} ms, RSS {:.0} MB", t0.elapsed().as_secs_f64() * 1000.0, cpu_ms() - cpu0, maxrss_mb());
            for s in 1..=observe_s {
                std::thread::sleep(Duration::from_secs(1));
                println!("  +{s} s after giving up: process CPU {:.0} ms (wall since start {:.0} ms), max RSS {:.0} MB", cpu_ms() - cpu0, t0.elapsed().as_secs_f64() * 1000.0, maxrss_mb());
            }
        }
    }
}

// ------------------------------------------------------------------ workers (run in child processes)
fn worker(args: &[String]) {
    match args[0].as_str() {
        "nested" => {
            let idx: usize = args[1].parse().unwrap();
            let n: usize = args[2].parse().unwrap();
            let limit = args.get(3).map(|s| s.parse().unwrap());
            println!("{}", run_nested(NESTED[idx].1, n, limit));
        }
        "hostile" => {
            let m: usize = args[1].parse().unwrap();
            let limit = args.get(2).map(|s| s.parse().unwrap());
            println!("{}", run_hostile(m, limit));
        }
        "threads" => threads_worker(NESTED[args[1].parse::<usize>().unwrap()].1, args[2].parse().unwrap(), args[3].parse().unwrap()),
        "parse" => {
            let kind = args[1].as_str();
            let size: usize = args[2].parse().unwrap();
            let src = match kind {
                "parens" => format!("{}1{}", "(".repeat(size), ")".repeat(size)),
                "plus" => vec!["1"; size].join("+"),
                "lists" => format!("{}1{}", "[".repeat(size), "]".repeat(size)),
                "not" => format!("{}true", "!".repeat(size)),
                "select" => format!("a{}", ".b".repeat(size)),
                "andchain" => vec!["true"; size].join(" && "),
                _ => panic!("kind"),
            };
            let env = env();
            let t0 = Instant::now();
            let p = env.compile(&src);
            let ms = t0.elapsed().as_secs_f64() * 1000.0;
            match p {
                Err(e) => println!("compile ERR after {ms:.1} ms: {}", format!("{e}").lines().next().unwrap_or("").chars().take(80).collect::<String>()),
                Ok(p) => {
                    let t1 = Instant::now();
                    let mut ctx = Context::with_env(env);
                    ctx.add_variable_from_value("a", cel::to_value(&serde_json::json!({})).unwrap());
                    let r = p.execute(&ctx);
                    println!(
                        "compile ok in {ms:.1} ms ({} bytes, {} nodes); eval {} in {:.1} ms",
                        src.len(),
                        node_count(p.expression()),
                        match r {
                            Ok(v) => summarize(&v),
                            Err(e) => format!("ERR {}", format!("{e}").chars().take(60).collect::<String>()),
                        },
                        t1.elapsed().as_secs_f64() * 1000.0
                    );
                }
            }
        }
        _ => panic!("worker kind"),
    }
}

struct Outcome {
    status: String,
    wall: f64,
    rss_mb: f64,
    stdout: String,
}

fn run_limited(args: &[&str], rss_limit_mb: f64, wall_limit: Duration) -> Outcome {
    let exe = std::env::current_exe().unwrap();
    let mut child = std::process::Command::new(exe)
        .arg("worker")
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
    let mut peak = 0f64;
    let (status, ru) = loop {
        let mut st: libc::c_int = 0;
        let mut ru: libc::rusage = unsafe { std::mem::zeroed() };
        let r = unsafe { libc::wait4(pid, &mut st, libc::WNOHANG, &mut ru) };
        if r == pid {
            break (st, ru);
        }
        let out = std::process::Command::new("ps").args(["-o", "rss=", "-p", &pid.to_string()]).output().unwrap();
        if let Ok(kb) = String::from_utf8_lossy(&out.stdout).trim().parse::<f64>() {
            peak = peak.max(kb / 1024.0);
            if kb / 1024.0 > rss_limit_mb && killed.is_none() {
                killed = Some("killed: RSS limit");
                unsafe { libc::kill(pid, libc::SIGKILL) };
            }
        }
        if t0.elapsed() > wall_limit && killed.is_none() {
            killed = Some("killed: wall limit");
            unsafe { libc::kill(pid, libc::SIGKILL) };
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let wall = t0.elapsed().as_secs_f64();
    let out = reader.join().unwrap();
    let status_text = if let Some(k) = killed {
        k.to_string()
    } else if libc::WIFEXITED(status) {
        format!("exit {}", libc::WEXITSTATUS(status))
    } else {
        format!("signal {}", libc::WTERMSIG(status))
    };
    Outcome { status: status_text, wall, rss_mb: (ru.ru_maxrss as f64 / 1024.0 / 1024.0).max(peak), stdout: out.trim().to_string() }
}

fn limits() {
    let rss = 4000.0;
    let wall = Duration::from_secs(30);
    println!("== nested expressions over {N_TASKS} tasks, one child process per run; killed at {rss} MB RSS or {} s ({})", wall.as_secs(), flavor());
    println!("{:<36} {:>10} {:>9} {:>8}  {}", "case", "limit", "wall s", "peak MB", "result");
    for (i, (name, _)) in NESTED.iter().enumerate() {
        let limits: Vec<Option<u64>> = if cfg!(feature = "patched") { vec![None, Some(10_000_000), Some(50_000_000)] } else { vec![None] };
        for lim in limits {
            let mut args = vec!["nested".to_string(), i.to_string(), N_TASKS.to_string()];
            if let Some(l) = lim {
                args.push(l.to_string());
            }
            let a: Vec<&str> = args.iter().map(String::as_str).collect();
            let o = run_limited(&a, rss, wall);
            println!("{:<36} {:>10} {:>9.2} {:>8.0}  {} | {}", name, lim.map(|l| format!("{l}")).unwrap_or("none".into()), o.wall, o.rss_mb, o.status, o.stdout);
        }
    }
    println!("-- hostile single-task list ({})", flavor());
    for m in [1_000usize, 5_000, 20_000] {
        let limits: Vec<Option<u64>> = if cfg!(feature = "patched") { vec![None, Some(10_000_000)] } else { vec![None] };
        for lim in limits {
            let mut args = vec!["hostile".to_string(), m.to_string()];
            if let Some(l) = lim {
                args.push(l.to_string());
            }
            let a: Vec<&str> = args.iter().map(String::as_str).collect();
            let o = run_limited(&a, rss, wall);
            println!("{:<36} {:>10} {:>9.2} {:>8.0}  {} | {}", format!("labels.all/exists, M={m}"), lim.map(|l| format!("{l}")).unwrap_or("none".into()), o.wall, o.rss_mb, o.status, o.stdout);
        }
    }
}

fn threads_cmd() {
    println!("== evaluate on a thread, give up at a deadline: what the abandoned thread does");
    for (i, dl) in [(0usize, 300u64), (2, 300)] {
        println!("{} (deadline {dl} ms):", NESTED[i].0);
        let o = run_limited(&["threads", &i.to_string(), &dl.to_string(), "4"], 4000.0, Duration::from_secs(20));
        for l in o.stdout.lines() {
            println!("  {l}");
        }
        println!("  child: {} after {:.1} s wall, peak {:.0} MB", o.status, o.wall, o.rss_mb);
    }
    println!("== kill a child process at a deadline instead (process isolation)");
    for dl in [100u64, 300] {
        let o = run_limited(&["nested", "1", &N_TASKS.to_string()], 4000.0, Duration::from_millis(dl));
        println!("  Q2 in a child, wall limit {dl} ms: {} at {:.0} ms wall, peak {:.0} MB", o.status, o.wall * 1000.0, o.rss_mb);
    }
    let t0 = Instant::now();
    let o = run_limited(&["nested", "0", "10"], 4000.0, Duration::from_secs(10));
    println!("  cost of the isolation itself: a child that builds 10 tasks and runs Q1 takes {:.0} ms wall end to end ({})", t0.elapsed().as_secs_f64() * 1000.0, o.stdout);
}

fn parse_cmd() {
    println!("== deep and large expressions, each in a child process ({})", flavor());
    println!("{:<10} {:>9} {:>8} {:>8}  {}", "shape", "size", "wall s", "peak MB", "result");
    for (kind, sizes) in [
        ("parens", vec![100usize, 1_000, 10_000, 100_000]),
        ("lists", vec![100, 1_000, 10_000, 100_000]),
        ("not", vec![100, 1_000, 10_000, 100_000]),
        ("select", vec![100, 1_000, 10_000]),
        ("plus", vec![100, 1_000, 10_000, 100_000]),
        ("andchain", vec![1_000, 10_000, 100_000]),
    ] {
        for size in sizes {
            let o = run_limited(&["parse", kind, &size.to_string()], 4000.0, Duration::from_secs(30));
            println!("{:<10} {:>9} {:>8.2} {:>8.0}  {} | {}", kind, size, o.wall, o.rss_mb, o.status, o.stdout);
        }
    }
}

pub fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("ordinary") => ordinary(),
        Some("estimate") => estimate_cmd(args.get(1).map(|s| s.parse().unwrap()).unwrap_or(200)),
        Some("limits") => limits(),
        Some("scaling") => scaling(),
        Some("hostile") => hostile_cmd(),
        Some("threads") => threads_cmd(),
        Some("parse") => parse_cmd(),
        Some("worker") => worker(&args[1..]),
        _ => eprintln!("usage: cel-bench ordinary|estimate [n]|limits|scaling|hostile|threads|parse"),
    }
}
