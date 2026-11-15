//! `collide-checks names | deps | stubs | all [--root <dir>]`: runs a structural check over the repository and exits 1
//! when it finds a problem, 2 when it cannot run. `pnpm check:names`, `check:deps` and `check:stubs` run one each.

use collide_checks::{deps, names, stubs};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (which, root) = match args.as_slice() {
        [which] => (which.as_str(), default_root()),
        [which, flag, dir] if flag == "--root" => (which.as_str(), PathBuf::from(dir)),
        _ => return usage(),
    };
    let checks: &[&str] = match which {
        "names" | "deps" | "stubs" => &[which],
        "all" => &["names", "deps", "stubs"],
        _ => return usage(),
    };
    let mut failed = false;
    for check in checks {
        match run(check, &root) {
            Ok(ok) => failed |= !ok,
            Err(error) => {
                eprintln!("{check}: cannot run: {error}");
                return ExitCode::from(2);
            }
        }
    }
    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn usage() -> ExitCode {
    eprintln!("usage: collide-checks names|deps|stubs|all [--root <dir>]");
    ExitCode::from(2)
}

/// The repository root: the parent of this crate's directory.
fn default_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the crate sits in the repository")
        .to_path_buf()
}

/// Runs one check, prints its result and returns whether it passed.
fn run(check: &str, root: &Path) -> std::io::Result<bool> {
    let problems: Vec<String> = match check {
        "names" => names::check(root)?.iter().map(|v| v.to_string()).collect(),
        "deps" => deps::check(root)?,
        _ => {
            let report = stubs::check(root)?;
            println!("stub count per tag (count, tag, stage that fills it):");
            for (tag, count) in &report.counts {
                println!("{count:>5}  {tag}  (stage {})", report.stages[tag]);
            }
            println!("{:>5}  total", report.counts.values().sum::<usize>());
            report.problems
        }
    };
    for problem in &problems {
        eprintln!("{check}: {problem}");
    }
    println!(
        "{check}: {}",
        if problems.is_empty() {
            "ok".to_string()
        } else {
            format!("{} problem(s)", problems.len())
        }
    );
    Ok(problems.is_empty())
}
