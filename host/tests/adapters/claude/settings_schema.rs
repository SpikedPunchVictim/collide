//! The generated `settings.json` against Claude's published schema, offline (ts-0068-wx; 0.3, Settings that fail
//! validation are ignored silently under `-p`). The schema is vendored (`fixtures/claude-code-settings.schema.json`; its
//! source note says where it came from) and Rust has no JSON Schema validator here, so the check is in two steps:
//!
//! 1. this test renders the adapter's settings and compares them with `fixtures/generated-settings.golden.json`. When
//!    they differ it writes the new golden next to the old and fails, so a change to the settings is a visible change
//!    to a committed file;
//! 2. `packages/claude-contract` validates that golden file against the schema with `ajv` (`pnpm test`), and checks
//!    that a settings file the schema rejects is rejected, so the validator is known to be able to fail.
//!
//! The test never touches the network.

use super::{plain_role, request};
use collide_host::adapters::claude::snapshot::snapshot_files;
use serde_json::Map;
use std::fs;
use std::path::PathBuf;

const DEFINITION: &str = "---\nname: builder\ndescription: Builds the thing\n---\nYou build.\n";

fn golden_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/adapters/claude/fixtures/generated-settings.golden.json")
}

#[test]
fn settings_golden_is_what_the_adapter_generates() {
    let files = snapshot_files(&request(plain_role()), DEFINITION, &Map::new()).expect("files");
    let path = golden_path();
    let golden = fs::read_to_string(&path).unwrap_or_default();
    if golden != files.settings {
        fs::write(&path, &files.settings).expect("the golden file is written");
        panic!(
            "the generated settings.json changed; {} was rewritten. Run `pnpm --filter @collide/claude-contract test` \
             to validate it against Claude's schema, then keep the change.",
            path.display()
        );
    }
}

#[test]
fn the_vendored_schema_is_the_file_the_source_note_describes() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/adapters/claude/fixtures");
    let schema = fs::read(dir.join("claude-code-settings.schema.json")).expect("the schema");
    let note = fs::read_to_string(dir.join("claude-code-settings.schema.source.md")).expect("note");
    assert!(
        note.contains(&format!("{} bytes", group_thousands(schema.len()))),
        "the note's size is the file's"
    );
    let value: serde_json::Value = serde_json::from_slice(&schema).expect("the schema is JSON");
    assert!(value["properties"]["hooks"].is_object());
    assert!(value["properties"]["autoMemoryEnabled"].is_object());
}

fn group_thousands(number: usize) -> String {
    let digits = number.to_string();
    let mut out = String::new();
    for (at, digit) in digits.chars().enumerate() {
        if at > 0 && (digits.len() - at).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    out
}
