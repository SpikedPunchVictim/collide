use s2lib::{daemon, shim, start, sys};
use std::path::PathBuf;

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned()
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let exe = std::env::current_exe().unwrap();
    let state = PathBuf::from(arg(&args, "--state").unwrap_or_else(|| std::env::var("S2_STATE").unwrap_or_else(|_| "work/state".into())));
    let state = std::env::current_dir().unwrap().join(&state);
    let work = state.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| PathBuf::from("."));
    let res: Result<(), String> = match args.first().map(|s| s.as_str()) {
        Some("daemon") => daemon::run(&state),
        Some("shim") => shim::run(std::path::Path::new(&args[1])),
        Some("detach") => start::detach(&state, &exe),
        Some("start") => {
            let how = arg(&args, "--how").unwrap_or_else(|| "setsid".into());
            let r = if how == "launchd" { start::start_launchd(&state, &exe, &work) } else { start::start_setsid(&state, &exe) };
            r.map(|v| println!("{v}"))
        }
        Some("coalition") => {
            for p in &args[1..] {
                if let Ok(pid) = p.parse::<i32>() {
                    println!("{}", serde_json::json!({"pid": pid, "ppid": sys::ppid_of(pid), "coalition": sys::coalition(pid).map(|c| c.0), "jetsam": sys::coalition(pid).map(|c| c.1)}));
                }
            }
            Ok(())
        }
        Some("rpc") => start::rpc(&state, serde_json::from_str(&args[1]).unwrap()).map(|v| println!("{v}")),
        _ => Err("usage: s2d daemon|shim <dir>|detach|start [--how setsid|launchd]|coalition <pid>...|rpc <json> [--state dir]".into()),
    };
    if let Err(e) = res {
        eprintln!("s2d: {e}");
        std::process::exit(1);
    }
}
