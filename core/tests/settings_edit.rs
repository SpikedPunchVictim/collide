//! Tests for editing a settings file (ts-0099-bw): a value put at a path of a [`ConfigFile`], and the comment test
//! that decides whether the file may be rewritten (`yaml_out` drops comments, so a file with one is not rewritten).

use collide_core::settings::ConfigFile;
use collide_core::yaml::load_yaml;
use collide_core::yaml_out::{has_comment, to_yaml};
use serde_json::json;

fn file(text: &str) -> ConfigFile {
    load_yaml(text.as_bytes(), "config.yaml").unwrap_or_else(|error| panic!("{error}\n{text}"))
}

// ---- a value at a path ----

#[test]
fn a_value_goes_to_a_key_of_collides_own() {
    let changed = ConfigFile::default()
        .with_value(&["ask", "timeout_s"], json!(90))
        .unwrap();
    assert_eq!(changed.ask.timeout_s, Some(90));
}

#[test]
fn a_value_goes_to_a_pack_key_below_config() {
    let changed = ConfigFile::default()
        .with_value(&["config", "ferry", "lanePool"], json!(4))
        .unwrap();
    assert_eq!(changed.config["ferry"]["lanePool"], json!(4));
}

#[test]
fn what_the_file_already_holds_is_kept() {
    let before = file("disk: { floor_gib: 3 }\nconfig: { ferry: { greeting: hi } }\n");
    let after = before
        .with_value(&["config", "ferry", "lanePool"], json!(4))
        .unwrap();
    assert_eq!(after.disk.floor_gib, Some(3));
    assert_eq!(after.config["ferry"]["greeting"], json!("hi"));
    assert_eq!(after.config["ferry"]["lanePool"], json!(4));
}

#[test]
fn a_value_replaces_the_one_at_the_path() {
    let before = file("ask: { timeout_s: 60 }\n");
    assert_eq!(
        before
            .with_value(&["ask", "timeout_s"], json!(90))
            .unwrap()
            .ask
            .timeout_s,
        Some(90)
    );
}

#[test]
fn the_file_it_was_called_on_is_not_changed() {
    let before = file("ask: { timeout_s: 60 }\n");
    let _ = before.with_value(&["ask", "timeout_s"], json!(90)).unwrap();
    assert_eq!(before.ask.timeout_s, Some(60));
}

#[test]
fn a_value_of_the_wrong_type_for_a_key_of_collides_own_is_an_error() {
    let error = ConfigFile::default()
        .with_value(&["ask", "timeout_s"], json!("soon"))
        .unwrap_err();
    assert!(error.contains("timeout_s"), "{error}");
}

#[test]
fn a_path_the_file_cannot_hold_is_an_error_naming_it() {
    let error = ConfigFile::default()
        .with_value(&["ask", "nope"], json!(1))
        .unwrap_err();
    assert!(error.contains("nope"), "{error}");
}

#[test]
fn an_empty_path_is_an_error() {
    assert!(ConfigFile::default().with_value(&[], json!(1)).is_err());
}

// ---- the comment test ----

#[test]
fn a_comment_on_its_own_line_is_a_comment() {
    assert!(has_comment("# my notes\ndisk: { floor_gib: 3 }\n"));
    assert!(has_comment("disk:\n  # why\n  floor_gib: 3\n"));
    assert!(has_comment("   # indented\n"));
}

#[test]
fn a_comment_after_a_value_is_a_comment() {
    assert!(has_comment("disk: { floor_gib: 3 } # the floor\n"));
    assert!(has_comment("ask:\n  agent: mine\t# tab before it\n"));
}

#[test]
fn a_hash_inside_a_quoted_string_is_not_a_comment() {
    assert!(!has_comment("ask: { agent: \"a # b\" }\n"));
    assert!(!has_comment("ask: { agent: 'a # b' }\n"));
    assert!(!has_comment(
        "config:\n  ferry:\n    greeting: \"#1 fan\"\n"
    ));
}

#[test]
fn a_hash_in_the_middle_of_a_plain_word_is_not_a_comment() {
    assert!(!has_comment("config:\n  ferry:\n    color: red#1\n"));
    assert!(!has_comment(
        "config:\n  ferry:\n    url: http://h/p#frag\n"
    ));
}

#[test]
fn an_apostrophe_in_a_plain_value_does_not_hide_a_later_comment() {
    assert!(has_comment(
        "config:\n  ferry:\n    note: it's fine # but this is one\n"
    ));
}

#[test]
fn a_quote_that_closes_leaves_a_comment_after_it_a_comment() {
    assert!(has_comment("ask: { agent: \"mine\" } # picked\n"));
}

#[test]
fn an_escaped_quote_does_not_end_a_double_quoted_string() {
    assert!(!has_comment("ask: { agent: \"say \\\"# no\\\"\" }\n"));
}

#[test]
fn a_file_with_no_hash_has_no_comment() {
    assert!(!has_comment(""));
    assert!(!has_comment("disk:\n  floor_gib: 3\n"));
}

#[test]
fn text_the_writer_made_with_no_header_has_no_comment() {
    let written = to_yaml(&file("ask: { agent: \"a # b\", timeout_s: 90 }\n"), "").unwrap();
    assert!(!has_comment(&written), "{written}");
}

#[test]
fn text_the_writer_made_with_a_header_has_one() {
    assert!(has_comment(
        &to_yaml(&ConfigFile::default(), "a header").unwrap()
    ));
}
