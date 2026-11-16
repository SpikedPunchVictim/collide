//! Tests for the YAML writer (ts-0093): what Collide writes is read back equal by the hardened loader, every string
//! survives, the text is the same each time, and a leading comment changes nothing that is read.

use collide_core::board::Board;
use collide_core::lock::{PackLock, UserPacks};
use collide_core::profile::Profile;
use collide_core::settings::ConfigFile;
use collide_core::yaml::{load_yaml, load_yaml_file};
use collide_core::yaml_out::to_yaml;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn fixture(path: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../fixtures")
        .join(path)
}

/// Writes `value`, reads it back through the hardened loader, and returns the text.
fn round_trip<T>(value: &T) -> String
where
    T: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug,
{
    let text = to_yaml(value, "").unwrap();
    let back: T =
        load_yaml(text.as_bytes(), "out.yaml").unwrap_or_else(|error| panic!("{error}\n{text}"));
    assert_eq!(&back, value, "{text}");
    text
}

// The types Collide writes.

#[test]
fn a_board_round_trips() {
    let board: Board = load_yaml_file(&fixture("board/board.yaml")).unwrap();
    assert!(!board.packs.is_empty(), "the fixture is not trivial");
    round_trip(&board);
}

#[test]
fn the_smallest_board_and_an_empty_one_round_trip() {
    let minimal: Board = load_yaml_file(&fixture("board/minimal.yaml")).unwrap();
    round_trip(&minimal);
    assert_eq!(round_trip(&Board::default()), "{}\n");
}

#[test]
fn a_pack_lock_round_trips_and_is_written_in_the_order_of_its_fields() {
    let lock: PackLock = load_yaml_file(&fixture("locks/packs.lock")).unwrap();
    let text = round_trip(&lock);
    let first = text.lines().nth(1).unwrap();
    assert!(first.trim_start().starts_with("- source: \"git:"), "{text}");
    assert!(
        text.lines()
            .nth(2)
            .unwrap()
            .trim_start()
            .starts_with("commit: "),
        "{text}"
    );
    assert!(
        text.lines()
            .nth(3)
            .unwrap()
            .trim_start()
            .starts_with("tree: "),
        "{text}"
    );
}

#[test]
fn an_empty_pack_lock_is_written_with_an_empty_list() {
    assert_eq!(round_trip(&PackLock::default()), "packs: []\n");
}

#[test]
fn a_pack_lock_has_this_exact_text() {
    let lock: PackLock = load_yaml(
        b"packs:\n  - source: \"git:https://example.com/a.git\"\n    \
          commit: \"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\"\n    \
          tree: \"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\"\n",
        "t.lock",
    )
    .unwrap();
    assert_eq!(
        to_yaml(&lock, "Written by Collide.\nDo not edit.").unwrap(),
        "# Written by Collide.\n# Do not edit.\npacks:\n  - source: \"git:https://example.com/a.git\"\n    \
         commit: \"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\"\n    \
         tree: \"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\"\n"
    );
}

#[test]
fn user_packs_round_trip() {
    let packs: UserPacks = load_yaml_file(&fixture("locks/packs.yaml")).unwrap();
    assert!(!packs.packs.is_empty());
    round_trip(&packs);
    round_trip(&UserPacks::default());
}

#[test]
fn a_config_file_round_trips_with_every_section() {
    for path in [
        "config/config.yaml",
        "config/repo-config.yaml",
        "config/disk-and-claude.yaml",
    ] {
        let file: ConfigFile = load_yaml_file(&fixture(path)).unwrap();
        round_trip(&file);
    }
    let all: ConfigFile = load_yaml(
        b"ask: { agent: a, timeout_s: 60, budget_usd: 1.0 }\ndisk: { floor_gib: 8 }\n\
          claude: { check_published: false }\n",
        "t.yaml",
    )
    .unwrap();
    let text = round_trip(&all);
    assert!(text.contains("budget_usd: 1.0\n"), "{text}");
    assert_eq!(round_trip(&ConfigFile::default()), "{}\n");
}

#[test]
fn a_profile_round_trips() {
    let profile: Profile = load_yaml_file(&fixture("profiles/small-fix.yaml")).unwrap();
    round_trip(&profile);
}

// Strings.

/// Text that a loader could read as a number, a boolean, `null`, a mapping, a list, a comment, a tag, an anchor, an
/// alias, a directive or a document marker if it were written plain.
const AWKWARD: &[&str] = &[
    "yes",
    "no",
    "on",
    "off",
    "y",
    "n",
    "true",
    "false",
    "True",
    "NULL",
    "null",
    "~",
    "",
    " ",
    "  x  ",
    "1",
    "0.5",
    "1e3",
    "0x1F",
    "0o7",
    "-1",
    "+1",
    ".inf",
    "-.INF",
    ".nan",
    "a: b",
    "a:b",
    ": b",
    "# x",
    "x #y",
    "- x",
    "-",
    "? x",
    "[a]",
    "{a}",
    "{{ x }}",
    "a, b",
    "*a",
    "&a",
    "!tag",
    "!!str x",
    "|",
    ">",
    "%x",
    "@x",
    "`x`",
    "---",
    "...",
    "'",
    "\"",
    "it's",
    "say \"hi\"",
    "back\\slash",
    "line\nbreak",
    "cr\r\nlf",
    "tab\there",
    "\t",
    "\u{1}",
    "\u{7f}",
    "\u{85}",
    "\u{2028}",
    "\u{2029}",
    "\u{feff}",
    "caf\u{e9}",
    "\u{65e5}\u{672c}\u{8a9e}",
    "\u{1f600}",
    "2026-11-16",
    "12:30",
    "1_000",
    "<<",
    "=",
    "git:https://example.com/x.git",
    "/abs/path",
    "~/home",
];

#[test]
fn every_awkward_string_comes_back_as_the_same_string() {
    let values: Vec<String> = AWKWARD.iter().map(ToString::to_string).collect();
    round_trip(&values);
}

#[test]
fn a_string_is_written_double_quoted_even_when_it_is_plain_text() {
    let text = to_yaml(&vec!["yes", "plain"], "").unwrap();
    assert_eq!(text, "- \"yes\"\n- \"plain\"\n");
}

#[test]
fn a_string_is_the_same_as_a_map_value_and_an_untyped_value() {
    let map: BTreeMap<String, String> = AWKWARD
        .iter()
        .enumerate()
        .map(|(index, text)| (format!("k{index}"), text.to_string()))
        .collect();
    round_trip(&map);
    let untyped: Value = serde_json::to_value(&map).unwrap();
    round_trip(&untyped);
}

#[test]
fn a_string_written_at_the_root_is_read_back() {
    round_trip(&"yes".to_string());
}

// Keys.

#[test]
fn a_key_is_plain_when_it_is_a_word_and_quoted_otherwise() {
    let map: BTreeMap<String, u32> = [
        "collide/builder",
        "plain_key-1",
        "a b",
        "",
        "true",
        "Null",
        "no",
        "1",
        "-x",
        "caf\u{e9}",
        "a:b",
        "#c",
    ]
    .iter()
    .enumerate()
    .map(|(index, key)| (key.to_string(), index as u32))
    .collect();
    let text = round_trip(&map);
    assert!(text.contains("\"collide/builder\": 0\n"), "{text}");
    assert!(text.contains("plain_key-1: 1\n"), "{text}");
    for quoted in [
        "\"a b\": 2\n",
        "\"\": 3\n",
        "\"true\": 4\n",
        "\"Null\": 5\n",
        "\"no\": 6\n",
        "\"1\": 7\n",
        "\"-x\": 8\n",
    ] {
        assert!(text.contains(quoted), "{quoted} in {text}");
    }
}

#[test]
fn a_map_of_maps_with_keys_that_need_quoting_round_trips() {
    let value = json!({
        "collide/builder": { "repo_instructions": "none" },
        "x y": { "a/b": ["c d"] }
    });
    round_trip(&value);
}

// Shape.

#[test]
fn an_empty_map_and_an_empty_list_are_written_flow_style() {
    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    struct Holder {
        list: Vec<String>,
        map: BTreeMap<String, String>,
        nested: Vec<Vec<String>>,
    }
    let value = Holder {
        list: vec![],
        map: BTreeMap::new(),
        nested: vec![vec![], vec!["a".into()]],
    };
    let text = round_trip(&value);
    assert!(text.contains("list: []\n"), "{text}");
    assert!(text.contains("map: {}\n"), "{text}");
    assert_eq!(round_trip(&BTreeMap::<String, String>::new()), "{}\n");
    assert_eq!(round_trip(&Vec::<String>::new()), "[]\n");
}

#[test]
fn nesting_of_maps_and_lists_in_each_other_round_trips_in_block_style() {
    let value = json!({
        "a": [ { "b": 1, "c": [ { "d": [ [1, 2], [], {} ] } ] }, [ { "e": "f" } ], "g" ],
        "h": { "i": { "j": { "k": [true, false, null] } } }
    });
    let text = round_trip(&value);
    assert!(
        !text.contains('{') || text.contains("{}"),
        "no flow maps but the empty one: {text}"
    );
    assert!(text.lines().all(|line| line.len() <= 120), "{text}");
}

#[test]
fn a_list_of_maps_starts_each_map_on_the_dash_line() {
    let text = to_yaml(&json!([{ "a": 1, "b": 2 }, { "a": 3 }]), "").unwrap();
    assert_eq!(text, "- a: 1\n  b: 2\n- a: 3\n");
}

#[test]
fn scalars_other_than_strings_are_written_as_they_are_read() {
    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    struct Scalars {
        yes: bool,
        no: bool,
        small: i64,
        big: u64,
        float: f64,
        whole_float: f64,
        exp: f64,
        none: Option<String>,
        some: Option<String>,
    }
    let value = Scalars {
        yes: true,
        no: false,
        small: i64::MIN,
        big: u64::MAX,
        float: -0.1,
        whole_float: 1.0,
        exp: 1e21,
        none: None,
        some: Some("x".into()),
    };
    let text = round_trip(&value);
    assert!(text.contains("none: null\n"), "{text}");
    assert!(
        text.contains("whole_float: 1.0\n"),
        "a float stays a float: {text}"
    );
}

#[test]
fn an_untyped_value_keeps_the_type_of_each_scalar() {
    let value = json!({ "i": 1, "f": 1.5, "w": 2.0, "b": true, "n": null, "s": "1", "t": "true", "u": "null" });
    let text = round_trip(&value);
    assert!(text.contains("w: 2.0\n"), "{text}");
}

// Output rules.

#[test]
fn the_same_value_gives_the_same_text_twice() {
    let board: Board = load_yaml_file(&fixture("board/board.yaml")).unwrap();
    assert_eq!(to_yaml(&board, "h").unwrap(), to_yaml(&board, "h").unwrap());
    let map: BTreeMap<String, Vec<String>> = AWKWARD
        .iter()
        .map(|text| (text.to_string(), vec![text.to_string()]))
        .collect();
    assert_eq!(to_yaml(&map, "").unwrap(), to_yaml(&map, "").unwrap());
}

#[test]
fn the_text_ends_with_one_newline_and_has_no_trailing_space() {
    let text = to_yaml(&json!({ "a": [1, { "b": "c" }], "d": {} }), "head").unwrap();
    assert!(text.ends_with('\n') && !text.ends_with("\n\n"), "{text:?}");
    assert!(text.lines().all(|line| line == line.trim_end()), "{text:?}");
}

#[test]
fn a_leading_comment_block_does_not_change_what_is_read() {
    let board: Board = load_yaml_file(&fixture("board/board.yaml")).unwrap();
    let plain = to_yaml(&board, "").unwrap();
    let headed = to_yaml(&board, "Written by Collide.\n\nEdit with `collide config`.").unwrap();
    assert!(
        headed.starts_with("# Written by Collide.\n#\n# Edit with `collide config`.\n"),
        "{headed}"
    );
    assert!(!plain.starts_with('#'));
    let a: Board = load_yaml(plain.as_bytes(), "a.yaml").unwrap();
    let b: Board = load_yaml(headed.as_bytes(), "b.yaml").unwrap();
    assert_eq!(a, b);
    assert_eq!(a, board);
    assert_eq!(
        headed
            .strip_prefix("# Written by Collide.\n#\n# Edit with `collide config`.\n")
            .unwrap(),
        plain
    );
}

#[test]
fn a_header_cannot_break_out_of_its_comment() {
    let headed = to_yaml(
        &json!({ "a": 1 }),
        "one\r\ntwo\u{85}three\u{2028}four\u{2029}five",
    )
    .unwrap();
    let value: Value = load_yaml(headed.as_bytes(), "h.yaml").unwrap();
    assert_eq!(value, json!({ "a": 1 }));
    assert_eq!(
        headed.lines().filter(|line| !line.starts_with('#')).count(),
        1,
        "{headed:?}"
    );
}

#[test]
fn an_empty_header_writes_no_comment_and_a_trailing_newline_in_it_adds_no_line() {
    assert_eq!(to_yaml(&json!({ "a": 1 }), "").unwrap(), "a: 1\n");
    assert_eq!(to_yaml(&json!({ "a": 1 }), "x\n").unwrap(), "# x\na: 1\n");
}

#[test]
fn a_value_that_has_no_text_form_is_an_error_and_not_a_bad_file() {
    let map: BTreeMap<(u8, u8), u8> = BTreeMap::from([((1, 2), 3)]);
    assert!(to_yaml(&map, "").is_err());
}
