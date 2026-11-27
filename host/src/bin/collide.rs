use collide_host::commands;
use collide_host::task_cli::{self, HELP, USAGE, exit};
use collide_host::task_daemon;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let rest = args.get(1..).unwrap_or_default();
    // The per-child shim (`collide shim <run-dir>`): the daemon starts it, and nobody types it, so it is not listed.
    if args.first().map(String::as_str) == Some("shim") {
        return ExitCode::from(u8::try_from(collide_host::shim::run(rest)).unwrap_or(1));
    }
    // The guide is text that is compiled in: it reads nothing, starts no daemon and writes nothing.
    if args.first().map(String::as_str) == Some("guide") {
        if let Some(extra) = rest.first() {
            eprintln!("collide guide takes no argument; got `{extra}`");
            return ExitCode::from(exit::USAGE);
        }
        print!("{}", collide_host::guide::text());
        return ExitCode::SUCCESS;
    }
    if args.first().map(String::as_str) == Some("pack") {
        return match std::env::current_dir() {
            Ok(cwd) => ExitCode::from(collide_host::pack_cli::run(
                rest,
                &cwd,
                &mut std::io::stdin().lock(),
                &mut std::io::stdout().lock(),
                &mut std::io::stderr().lock(),
            )),
            Err(error) => {
                eprintln!("collide pack: cannot read the working directory: {error}");
                ExitCode::from(exit::FAILED)
            }
        };
    }
    if matches!(args.first().map(String::as_str), Some("init" | "trust")) {
        let word = args[0].as_str();
        let Ok(cwd) = std::env::current_dir() else {
            eprintln!("collide {word}: cannot read the working directory");
            return ExitCode::from(exit::FAILED);
        };
        let (out, err) = (&mut std::io::stdout().lock(), &mut std::io::stderr().lock());
        return ExitCode::from(if word == "init" {
            collide_host::board_cli::run_init(rest, &cwd, out, err)
        } else {
            collide_host::board_cli::run_trust(rest, &cwd, &mut std::io::stdin().lock(), out, err)
        });
    }
    if args.first().map(String::as_str) == Some("board") {
        let (out, err) = (&mut std::io::stdout().lock(), &mut std::io::stderr().lock());
        return match (rest.first().map(String::as_str), std::env::current_dir()) {
            (Some("remove"), Ok(cwd)) => ExitCode::from(collide_host::board_cli::run_remove(
                &rest[1..],
                &cwd,
                out,
                err,
            )),
            (Some("remove"), Err(_)) => {
                eprintln!("collide board remove: cannot read the working directory");
                ExitCode::from(exit::FAILED)
            }
            (other, _) => {
                let what = other.map_or("needs a subcommand".to_string(), |word| {
                    format!("has no subcommand `{word}`")
                });
                eprintln!("collide board {what}\n{}", collide_host::board_cli::USAGE);
                ExitCode::from(exit::USAGE)
            }
        };
    }
    if let Some(code) = args
        .first()
        .and_then(|name| commands::execute_stub(name, rest))
    {
        return ExitCode::from(code);
    }
    let parsed = match args.first().map(String::as_str) {
        None => {
            println!("collide");
            return ExitCode::SUCCESS;
        }
        Some("schema") if rest.is_empty() => {
            print!("{}", collide_host::schema_list::listing());
            return ExitCode::SUCCESS;
        }
        Some("help" | "--help" | "-h") => {
            println!(
                "{}\n\n{USAGE}\n{}\n\n{HELP}\n\n{}\n\n{}\n{}\n{}\n{}",
                commands::tree_text(),
                collide_host::run_cli::USAGE,
                collide_host::run_cli::HELP,
                collide_host::config_cli::USAGE,
                collide_host::config_set_cli::USAGE,
                collide_host::pack_cli::USAGE,
                collide_host::board_cli::USAGE
            );
            return ExitCode::SUCCESS;
        }
        Some("run") => {
            return ExitCode::from(collide_host::run_cli::run(
                rest,
                &mut std::io::stdin().lock(),
                &mut std::io::stdout().lock(),
                &mut std::io::stderr().lock(),
            ));
        }
        Some("config") => {
            return match std::env::current_dir() {
                Ok(cwd) => ExitCode::from(collide_host::config_cli::run_config(
                    rest,
                    &cwd,
                    &mut std::io::stdin().lock(),
                    &mut std::io::stdout().lock(),
                    &mut std::io::stderr().lock(),
                )),
                Err(error) => {
                    eprintln!("collide config: cannot read the working directory: {error}");
                    ExitCode::from(exit::FAILED)
                }
            };
        }
        Some("stop") => {
            return ExitCode::from(collide_host::stop_cli::run(
                rest,
                &mut std::io::stdout().lock(),
                &mut std::io::stderr().lock(),
            ));
        }
        Some("tasks") => task_cli::parse_tasks(rest),
        Some("task") => task_cli::parse_task(rest),
        Some("sections") => task_cli::parse_sections(rest),
        Some(_) => {
            let names: Vec<&str> = commands::COMMANDS.iter().map(|c| c.name).collect();
            eprintln!("usage: collide [{} | help]", names.join(" | "));
            return ExitCode::from(exit::USAGE);
        }
    };
    let command = match parsed {
        Ok(command) => command,
        Err(error) => {
            eprintln!("collide: {}\n{USAGE}", error.message);
            return ExitCode::from(error.code);
        }
    };
    let cwd = match std::env::current_dir() {
        Ok(cwd) => cwd,
        Err(error) => {
            eprintln!("collide: cannot read the working directory: {error}");
            return ExitCode::from(exit::FAILED);
        }
    };
    let code = task_daemon::run(
        &command,
        &cwd,
        &mut std::io::stdout().lock(),
        &mut std::io::stderr().lock(),
    );
    ExitCode::from(code)
}
