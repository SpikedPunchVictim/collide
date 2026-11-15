//! The phase's snapshot files (0.3): `settings.json` with auto memory off and the canary hook, `agents.json` with the
//! role's definition under `collide-<pack>-<role>`, and `mcp.json`. The adapter renders them; the daemon writes them.

use super::{plain_role, request};
use collide_host::adapters::claude::launch::{AGENTS_FILE, MCP_FILE, SETTINGS_FILE};
use collide_host::adapters::claude::snapshot::snapshot_files;
use collide_host::adapters::claude::version::CANARY_MARKER;
use serde_json::{Map, Value, json};

const DEFINITION: &str = "---\nname: builder\ndescription: Builds the thing\nmodel: opus\ntools: Read, Grep, mcp__demo__map\n---\nYou build.\n\nCarefully.\n";

fn parse(text: &str) -> Value {
    serde_json::from_str(text).expect("the file is JSON")
}

fn files(definition: &str) -> collide_host::adapters::claude::snapshot::SnapshotFiles {
    snapshot_files(&request(plain_role()), definition, &Map::new()).expect("a snapshot")
}

#[test]
fn the_files_carry_the_names_the_launch_line_passes() {
    let files = files(DEFINITION);
    let names: Vec<&str> = files.named().iter().map(|(name, _)| *name).collect();
    assert_eq!(names, [AGENTS_FILE, SETTINGS_FILE, MCP_FILE]);
}

#[test]
fn settings_turn_auto_memory_off() {
    let settings = parse(&files(DEFINITION).settings);
    assert_eq!(settings["autoMemoryEnabled"], json!(false));
}

#[test]
fn settings_carry_a_session_start_hook_that_writes_the_canary_marker_to_stderr() {
    let settings = parse(&files(DEFINITION).settings);
    let hooks = settings["hooks"]["SessionStart"][0]["hooks"]
        .as_array()
        .unwrap();
    assert_eq!(hooks.len(), 1);
    assert_eq!(hooks[0]["type"], "command");
    let command = hooks[0]["command"].as_str().unwrap();
    assert!(command.contains(CANARY_MARKER), "{command}");
    assert_eq!(
        command,
        format!("echo {CANARY_MARKER} >&2"),
        "stderr, so it is not added to the model's context"
    );
}

#[test]
fn the_definition_becomes_the_agent_named_after_the_pack_and_the_role() {
    let agents = parse(&files(DEFINITION).agents);
    let agent = &agents["collide-demo-builder"];
    assert_eq!(agent["description"], "Builds the thing");
    assert_eq!(agent["prompt"], "You build.\n\nCarefully.");
    assert_eq!(agent["tools"], json!(["Read", "Grep", "mcp__demo__map"]));
    assert_eq!(agents.as_object().unwrap().len(), 1);
}

#[test]
fn the_definitions_own_model_is_left_out_because_the_launch_line_names_it() {
    let agents = parse(&files(DEFINITION).agents);
    assert!(agents["collide-demo-builder"].get("model").is_none());
}

#[test]
fn a_tools_list_written_as_yaml_is_read_the_same() {
    let definition = "---\ndescription: d\ntools:\n  - Read\n  - Glob\n---\nbody\n";
    let agents = parse(&files(definition).agents);
    assert_eq!(
        agents["collide-demo-builder"]["tools"],
        json!(["Read", "Glob"])
    );
}

#[test]
fn a_definition_with_no_tools_key_has_none_in_the_file() {
    let agents = parse(&files("---\ndescription: d\n---\nbody\n").agents);
    assert!(agents["collide-demo-builder"].get("tools").is_none());
}

#[test]
fn a_definition_without_frontmatter_or_a_description_is_refused() {
    for bad in [
        "just a prompt",
        "---\nname: x\n---\nbody",
        "---\ndescription: d\n",
    ] {
        let error = snapshot_files(&request(plain_role()), bad, &Map::new());
        assert!(error.is_err(), "{bad:?}");
    }
}

#[test]
fn a_malformed_frontmatter_is_refused_with_a_message() {
    let error = snapshot_files(
        &request(plain_role()),
        "---\ndescription: [unclosed\n---\nbody",
        &Map::new(),
    )
    .unwrap_err()
    .to_string();
    assert!(error.contains("definition"), "{error}");
}

#[test]
fn mcp_config_holds_the_servers_it_is_given_and_none_by_default() {
    assert_eq!(parse(&files(DEFINITION).mcp), json!({ "mcpServers": {} }));
    let servers = json!({ "demo": { "command": "demo-mcp", "args": [] } });
    let given = snapshot_files(
        &request(plain_role()),
        DEFINITION,
        servers.as_object().unwrap(),
    )
    .unwrap();
    assert_eq!(parse(&given.mcp), json!({ "mcpServers": servers }));
}

#[test]
fn two_calls_with_the_same_request_give_the_same_files() {
    assert_eq!(files(DEFINITION), files(DEFINITION));
}

#[test]
fn every_file_ends_in_a_newline() {
    let files = files(DEFINITION);
    for (name, text) in files.named() {
        assert!(text.ends_with('\n'), "{name}");
    }
}
