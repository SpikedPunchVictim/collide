use std::process::ExitCode;

const USAGE: &str = "usage: collide [schema]";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        [] => println!("collide"),
        ["schema"] => print!("{}", collide_host::schema_list::listing()),
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    }
    ExitCode::SUCCESS
}
