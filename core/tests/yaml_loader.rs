//! Tests for the hardened YAML loader (ts-0023, 0.7 and 0.9): one test per hazard. Where the library's own
//! defaults let the hazard through, the test also runs `serde-saphyr` with `Options::default()` as the positive
//! control, so the test shows what the hardened options change.

use collide_core::yaml::{MAX_YAML_BYTES, YamlError, load_yaml, load_yaml_file};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Workflow {
    lane: String,
}

/// A manifest-shaped document, so the tests read like a manifest without being one.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Doc {
    name: String,
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    enabled: Option<bool>,
    #[serde(default)]
    config: BTreeMap<String, Value>,
    #[serde(default)]
    workflows: BTreeMap<String, Workflow>,
    #[serde(default)]
    tags: Vec<String>,
}

fn load(text: &str) -> Result<Doc, YamlError> {
    load_yaml(text.as_bytes(), "doc.yaml")
}

/// The library as it comes, for the positive controls.
fn with_defaults(text: &str) -> Result<Doc, serde_saphyr::Error> {
    serde_saphyr::from_str_with_options(text, serde_saphyr::Options::default())
}

/// The parts of a rejected document, so a test names only what it checks.
struct Rejected {
    line: Option<usize>,
    column: Option<usize>,
    key_path: Option<String>,
    text: String,
}

fn rejected(result: Result<Doc, YamlError>) -> Rejected {
    let error = result.expect_err("the loader accepted a document it should reject");
    let text = error.to_string();
    match error {
        YamlError::Invalid {
            line,
            column,
            key_path,
            ..
        } => Rejected {
            line,
            column,
            key_path,
            text,
        },
        other => panic!("expected YamlError::Invalid, got {other:?}"),
    }
}

/// Runs `f` on a thread with a small stack: a loader that recursed 100,000 levels would overflow it.
fn on_small_stack<R: Send + 'static>(f: impl FnOnce() -> R + Send + 'static) -> R {
    std::thread::Builder::new()
        .stack_size(256 * 1024)
        .spawn(f)
        .expect("spawn")
        .join()
        .expect("the loader panicked")
}

fn scratch_file(test: &str, bytes: &[u8]) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("yaml_loader");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("{test}.yaml"));
    std::fs::write(&path, bytes).unwrap();
    path
}

const FAST: Duration = Duration::from_secs(2);

// Scalars.

#[test]
fn no_stays_a_string_in_an_untyped_value() {
    let text = "name: x\nconfig:\n  flag: no\n  other: yes\n";
    let doc = load(text).unwrap();
    assert_eq!(doc.config["flag"], json!("no"));
    assert_eq!(doc.config["other"], json!("yes"));
    // control: the defaults read YAML 1.1 booleans
    assert_eq!(with_defaults(text).unwrap().config["flag"], json!(false));
}

#[test]
fn no_stays_a_string_in_a_string_field() {
    assert_eq!(load("name: no\n").unwrap().name, "no");
}

#[test]
fn no_is_not_a_boolean_in_a_bool_field() {
    let text = "name: x\nenabled: no\n";
    let bad = rejected(load(text));
    assert_eq!((bad.line, bad.column), (Some(2), Some(10)));
    // control: the defaults turn `no` into `false`
    assert_eq!(with_defaults(text).unwrap().enabled, Some(false));
}

#[test]
fn true_and_false_are_still_booleans() {
    assert_eq!(
        load("name: x\nenabled: false\n").unwrap().enabled,
        Some(false)
    );
}

#[test]
fn a_quoted_decimal_stays_a_string() {
    assert_eq!(
        load("name: x\nversion: \"0.10\"\n").unwrap().version,
        Some("0.10".to_string())
    );
}

#[test]
fn an_unquoted_decimal_in_a_string_field_is_an_error_at_its_column() {
    let text = "name: x\nversion: 0.10\n";
    let bad = rejected(load(text));
    assert_eq!((bad.line, bad.column), (Some(2), Some(10)));
    assert!(bad.text.contains("0.10"), "{}", bad.text);
    assert_eq!(bad.key_path.as_deref(), Some("version"));
    // control: the defaults take it, as the text "0.10"
    assert_eq!(
        with_defaults(text).unwrap().version.as_deref(),
        Some("0.10")
    );
}

#[test]
fn a_quoted_exponent_stays_a_string() {
    assert_eq!(
        load("name: x\nversion: \"1e10\"\n").unwrap().version,
        Some("1e10".to_string())
    );
}

#[test]
fn an_unquoted_exponent_in_a_string_field_is_an_error() {
    let text = "name: x\nversion: 1e10\n";
    let bad = rejected(load(text));
    assert_eq!((bad.line, bad.column), (Some(2), Some(10)));
    assert!(bad.text.contains("1e10"), "{}", bad.text);
    // control
    assert_eq!(
        with_defaults(text).unwrap().version.as_deref(),
        Some("1e10")
    );
}

// Duplicate keys. The library's default is already an error; the loader sets it by name so a change of the
// default cannot weaken it, and these tests pin it.

#[test]
fn a_duplicate_top_level_key_is_an_error() {
    let bad = rejected(load("name: x\nname: y\n"));
    assert_eq!((bad.line, bad.column), (Some(2), Some(1)));
    assert!(bad.text.contains("duplicate"), "{}", bad.text);
}

#[test]
fn a_duplicate_key_inside_a_map_is_an_error() {
    let text = "name: x\nworkflows:\n  build:\n    lane: a\n  build:\n    lane: b\n";
    let bad = rejected(load(text));
    assert_eq!((bad.line, bad.column), (Some(5), Some(3)));
    assert!(bad.text.contains("duplicate"), "{}", bad.text);
}

#[test]
fn a_duplicate_key_inside_an_untyped_value_is_an_error() {
    let text = "name: x\nconfig:\n  a: 1\n  a: 2\n";
    assert!(rejected(load(text)).text.contains("duplicate"));
}

// Anchors, aliases, merge keys.

#[test]
fn an_anchor_is_an_error_whose_text_says_so() {
    let text = "name: &n x\n";
    let bad = rejected(load(text));
    assert_eq!(bad.line, Some(1));
    assert!(bad.text.contains("anchors are not allowed"), "{}", bad.text);
    // control: the defaults accept it
    assert!(with_defaults(text).is_ok());
}

#[test]
fn an_alias_is_an_error_whose_text_says_so() {
    let bad = rejected(load("name: x\ntags: [*n]\n"));
    assert_eq!(bad.line, Some(2));
    assert!(bad.text.contains("aliases are not allowed"), "{}", bad.text);
}

#[test]
fn an_anchor_with_its_alias_is_rejected_at_the_anchor() {
    let text = "name: x\nworkflows:\n  a: &w\n    lane: x\n  b: *w\n";
    let bad = rejected(load(text));
    assert_eq!(bad.line, Some(4)); // the library puts an anchor at the start of its node
    assert!(bad.text.contains("anchors are not allowed"), "{}", bad.text);
    // control
    assert!(with_defaults(text).is_ok());
}

#[test]
fn a_merge_key_is_an_error_whose_text_says_so() {
    let text = "name: x\nworkflows:\n  a:\n    <<: {lane: x}\n";
    let bad = rejected(load(text));
    assert_eq!((bad.line, bad.column), (Some(4), Some(5)));
    assert!(bad.text.contains("merge keys"), "{}", bad.text);
    // control: the defaults merge it
    assert_eq!(with_defaults(text).unwrap().workflows["a"].lane, "x");
}

// Bombs and size.

/// The classic alias bomb: each level is nine aliases of the one before.
fn alias_bomb(levels: usize) -> String {
    let mut text = String::from("name: x\nconfig:\n  l0: &l0 [x, x, x, x, x, x, x, x, x]\n");
    for level in 1..=levels {
        let aliases = vec![format!("*l{}", level - 1); 9].join(", ");
        text.push_str(&format!("  l{level}: &l{level} [{aliases}]\n"));
    }
    text
}

#[test]
fn an_alias_bomb_fails_fast() {
    let text = alias_bomb(9); // 9^10 leaves if expanded
    let started = Instant::now();
    let bad = on_small_stack(move || rejected(load(&text)));
    assert!(started.elapsed() < FAST, "took {:?}", started.elapsed());
    assert!(bad.text.contains("anchors are not allowed"), "{}", bad.text);
}

#[test]
fn an_alias_bomb_expands_under_the_defaults() {
    // control: four levels (about 75,000 nodes) are read, and the defaults only stop it at their own node cap
    assert!(with_defaults(&alias_bomb(4)).is_ok());
}

fn nested(depth: usize) -> String {
    format!(
        "name: x\nconfig:\n  k: {}{}\n",
        "[".repeat(depth),
        "]".repeat(depth)
    )
}

#[test]
fn nesting_of_100_000_levels_fails_fast_on_a_small_stack() {
    let text = nested(100_000);
    let started = Instant::now();
    let bad = on_small_stack(move || rejected(load(&text)));
    assert!(started.elapsed() < FAST, "took {:?}", started.elapsed());
    assert!(bad.text.contains("32 levels"), "{}", bad.text);
}

#[test]
fn nesting_deeper_than_32_levels_is_an_error_and_less_is_fine() {
    // 2 levels for the mapping and the map under `config`, then the sequences
    assert!(load(&nested(20)).is_ok());
    assert!(load(&nested(40)).is_err());
    // control: the defaults allow 40
    assert!(with_defaults(&nested(40)).is_ok());
}

#[test]
fn block_style_nesting_deeper_than_32_levels_is_an_error() {
    let mut text = String::from("name: x\nconfig:\n");
    for level in 0..40 {
        text.push_str(&format!("{}k{level}:\n", "  ".repeat(level + 1)));
    }
    text.push_str(&format!("{}leaf: 1\n", "  ".repeat(41)));
    let bad = rejected(load(&text));
    assert!(bad.text.contains("32 levels"), "{}", bad.text);
    // control: the defaults allow it
    assert!(with_defaults(&text).is_ok());
}

#[test]
fn more_than_100_000_nodes_is_an_error() {
    let items = vec!["1"; 100_001].join(",");
    let text = format!("name: x\nconfig:\n  k: [{items}]\n");
    assert!(text.len() < MAX_YAML_BYTES);
    let bad = rejected(load(&text));
    assert!(bad.text.contains("100,000 nodes"), "{}", bad.text);
    // control: the defaults allow it
    assert!(with_defaults(&text).is_ok());
}

#[test]
fn a_file_over_the_cap_fails_before_parsing() {
    // not YAML at all: a parse would fail with a position, the cap fails first
    let path = scratch_file("over_cap", &vec![b'{'; MAX_YAML_BYTES + 1]);
    let error = load_yaml_file::<Doc>(&path).unwrap_err();
    assert!(matches!(error, YamlError::TooLarge { .. }), "{error:?}");
    let text = error.to_string();
    assert!(text.contains(&path.display().to_string()), "{text}");
    assert!(text.contains(&MAX_YAML_BYTES.to_string()), "{text}");
}

#[test]
fn bytes_over_the_cap_fail_before_parsing() {
    let error = load_yaml::<Doc>(&vec![b'{'; MAX_YAML_BYTES + 1], "label").unwrap_err();
    assert!(matches!(error, YamlError::TooLarge { .. }), "{error:?}");
}

#[test]
fn a_file_of_exactly_the_cap_loads() {
    let mut bytes = b"name: x\n".to_vec();
    bytes.resize(MAX_YAML_BYTES, b'#');
    let path = scratch_file("at_cap", &bytes);
    assert_eq!(load_yaml_file::<Doc>(&path).unwrap().name, "x");
}

#[test]
fn a_missing_file_is_an_error_naming_the_path() {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("yaml_loader_no_such_file.yaml");
    let error = load_yaml_file::<Doc>(&path).unwrap_err();
    assert!(matches!(error, YamlError::Read { .. }), "{error:?}");
    assert!(error.to_string().contains("yaml_loader_no_such_file.yaml"));
}

#[test]
fn bytes_that_are_not_utf8_are_an_error() {
    let error = load_yaml::<Doc>(b"name: \xff\xfe\n", "label").unwrap_err();
    assert!(error.to_string().contains("UTF-8"), "{error}");
}

// Unknown keys, templates, and the shape of an error.

#[test]
fn an_unknown_top_level_key_names_its_path() {
    let bad = rejected(load("name: x\nflavor: 1\n"));
    assert_eq!(bad.key_path.as_deref(), Some("flavor"));
    assert_eq!((bad.line, bad.column), (Some(2), Some(1)));
    assert!(bad.text.contains("flavor"), "{}", bad.text);
}

#[test]
fn an_unknown_nested_key_names_the_path_to_it() {
    let text = "name: x\nworkflows:\n  build:\n    lane: a\n    speed: 1\n";
    let bad = rejected(load(text));
    assert_eq!(bad.key_path.as_deref(), Some("workflows.build.speed"));
    assert_eq!((bad.line, bad.column), (Some(5), Some(5)));
    assert!(bad.text.contains("workflows.build.speed"), "{}", bad.text);
}

#[test]
fn an_unquoted_template_is_an_error_at_its_column_with_the_hint() {
    let bad = rejected(load("name: {{ task.id }}\n"));
    assert_eq!((bad.line, bad.column), (Some(1), Some(7)));
    assert!(bad.text.contains("quote the template"), "{}", bad.text);
}

#[test]
fn an_unquoted_template_in_a_nested_list_gets_the_hint() {
    let bad = rejected(load("name: x\nworkflows:\n  a:\n    lane: {{ lane }}\n"));
    assert_eq!((bad.line, bad.column), (Some(4), Some(11)));
    assert!(bad.text.contains("quote the template"), "{}", bad.text);
}

#[test]
fn a_quoted_template_is_a_string() {
    assert_eq!(
        load("name: \"{{ task.id }}\"\n").unwrap().name,
        "{{ task.id }}"
    );
}

#[test]
fn the_hint_is_only_for_templates() {
    let bad = rejected(load("name: x\nversion: 0.10\n"));
    assert!(!bad.text.contains("quote the template"), "{}", bad.text);
}

#[test]
fn an_error_names_its_origin_line_and_column() {
    let error = load_yaml::<Doc>(b"name: x\nflavor: 1\n", "packs/plan/pack.yaml").unwrap_err();
    let text = error.to_string();
    assert!(text.starts_with("packs/plan/pack.yaml:2:1: "), "{text}");
}

#[test]
fn a_file_error_names_the_file() {
    let path = scratch_file("unknown_key", b"name: x\nflavor: 1\n");
    let text = load_yaml_file::<Doc>(&path).unwrap_err().to_string();
    assert!(
        text.starts_with(&format!("{}:2:1: ", path.display())),
        "{text}"
    );
}

#[test]
fn a_second_document_is_an_error() {
    assert!(load("name: x\n---\nname: y\n").is_err());
}

#[test]
fn a_valid_document_loads() {
    let text = "name: plan\nversion: \"1.0\"\nenabled: true\ntags: [a, b]\nworkflows:\n  build:\n    lane: main\n";
    let doc = load(text).unwrap();
    assert_eq!(doc.name, "plan");
    assert_eq!(doc.tags, vec!["a", "b"]);
    assert_eq!(doc.workflows["build"].lane, "main");
}
