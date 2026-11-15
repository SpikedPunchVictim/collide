//! The CLI's token is never put in an environment variable (0.2). A function-by-function proof would need a model of
//! every function that could ever be written, so this is a scan: in every file of `host/src`, no call that sets a
//! variable (`set_var`, `.env(`, `.envs(`, `.env_clone(`) is within [`WINDOW`] lines of the word "token". A call that
//! gives a child a variable for another purpose has to stay away from a token, and a change that does not fails here
//! and says where. The scan reads source, so it needs no running daemon and no real socket.

use std::fs;
use std::path::{Path, PathBuf};

const SETTERS: [&str; 4] = ["set_var", ".env(", ".envs(", ".env_clone("];

/// Lines either side of a setter that may not mention a token.
const WINDOW: usize = 6;

fn rust_files(dir: &Path, found: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            rust_files(&path, found);
        } else if path.extension().is_some_and(|e| e == "rs") {
            found.push(path);
        }
    }
}

/// The places in `text` where a setter is near the word "token", as `line: text`.
fn setters_near_a_token(text: &str) -> Vec<String> {
    let lines: Vec<&str> = text.lines().collect();
    let mut found = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let code = line.split("//").next().unwrap_or("");
        if !SETTERS.iter().any(|s| code.contains(s)) {
            continue;
        }
        let from = i.saturating_sub(WINDOW);
        let to = (i + WINDOW + 1).min(lines.len());
        if lines[from..to]
            .iter()
            .any(|near| near.to_lowercase().contains("token"))
        {
            found.push(format!("{}: {}", i + 1, line.trim()));
        }
    }
    found
}

#[test]
fn no_function_in_host_puts_a_token_in_an_environment_variable() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    rust_files(&src, &mut files);
    assert!(
        files.len() > 10,
        "the scan found only {} files",
        files.len()
    );
    let mut problems = Vec::new();
    for file in files {
        let text = fs::read_to_string(&file).unwrap();
        for hit in setters_near_a_token(&text) {
            problems.push(format!("{}:{hit}", file.display()));
        }
    }
    assert!(
        problems.is_empty(),
        "a token must not go in an environment variable:\n{}",
        problems.join("\n")
    );
}

#[test]
fn the_scan_does_see_a_token_set_in_an_environment() {
    let bad = "fn start() {\n    let token = read();\n    cmd.env(\"X\", token);\n}\n";
    assert_eq!(setters_near_a_token(bad).len(), 1);
    let bad_var = "std::env::set_var(\"COLLIDE_TOKEN\", t);";
    assert_eq!(setters_near_a_token(bad_var).len(), 1);
}

#[test]
fn the_scan_leaves_a_setter_far_from_any_token_alone() {
    let fine = "cmd.env(\"HOME\", home);\n";
    assert!(setters_near_a_token(fine).is_empty());
    let far = format!(
        "// the token is a file\n{}cmd.env(\"HOME\", h);\n",
        "\n".repeat(20)
    );
    assert!(setters_near_a_token(&far).is_empty());
}
