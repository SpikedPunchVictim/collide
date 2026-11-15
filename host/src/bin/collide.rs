use collide_host::commands;
use collide_host::task_cli::{self, HELP, USAGE, exit};
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let rest = args.get(1..).unwrap_or_default();
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
            println!("{}\n\n{USAGE}\n\n{HELP}", commands::tree_text());
            return ExitCode::SUCCESS;
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
    let code = task_cli::run(
        &command,
        &cwd,
        &mut std::io::stdout().lock(),
        &mut std::io::stderr().lock(),
    );
    ExitCode::from(code)
}
