//! Tests for the hand-written subset of JSON Schema that checks a setting's value (ts-0098-nx): each keyword that
//! passes and fails, each keyword outside the subset refused, a `default` held to its own schema, a deep object, and
//! a value that must not appear in a message.

use collide_core::setting_value::{
    SCHEMA_KEYWORDS, SchemaProblem, check_setting_schema, check_value,
};
use serde_json::{Value, json};

/// The constraint texts of every violation of `value` against `schema`.
fn violations(schema: Value, value: Value) -> Vec<String> {
    check_value(&schema, &value)
        .into_iter()
        .map(|violation| violation.to_string())
        .collect()
}

fn passes(schema: Value, value: Value) {
    let found = violations(schema.clone(), value.clone());
    assert!(found.is_empty(), "{value} should meet {schema}: {found:?}");
}

fn fails_with(schema: Value, value: Value, part: &str) {
    let found = violations(schema.clone(), value.clone());
    assert!(
        found.iter().any(|text| text.contains(part)),
        "{value} against {schema} should fail with `{part}`, and gave {found:?}"
    );
}

// ---- each keyword ----

#[test]
fn type_accepts_each_json_type_and_refuses_the_others() {
    let cases = [
        ("string", json!("a"), json!(1)),
        ("boolean", json!(true), json!("true")),
        ("integer", json!(2), json!(2.5)),
        ("number", json!(2.5), json!("2.5")),
        ("array", json!([1]), json!({})),
        ("object", json!({}), json!([])),
        ("null", json!(null), json!(0)),
    ];
    for (name, good, bad) in cases {
        passes(json!({ "type": name }), good);
        fails_with(json!({ "type": name }), bad, &format!("type {name}"));
    }
}

#[test]
fn an_integer_written_with_a_zero_fraction_is_still_an_integer() {
    passes(json!({ "type": "integer" }), json!(2.0));
}

#[test]
fn enum_accepts_a_listed_value_and_refuses_another() {
    let schema = json!({ "enum": ["fast", "slow", 3] });
    passes(schema.clone(), json!("fast"));
    passes(schema.clone(), json!(3.0));
    fails_with(schema, json!("medium"), "is not one of");
}

#[test]
fn const_accepts_only_its_value() {
    passes(json!({ "const": { "a": [1, 2] } }), json!({ "a": [1, 2] }));
    fails_with(json!({ "const": 1 }), json!(2), "must equal 1");
}

#[test]
fn minimum_and_maximum_are_inclusive() {
    let schema = json!({ "type": "integer", "minimum": 1, "maximum": 4 });
    passes(schema.clone(), json!(1));
    passes(schema.clone(), json!(4));
    fails_with(schema.clone(), json!(0), "under the minimum of 1");
    fails_with(schema, json!(5), "over the maximum of 4");
}

#[test]
fn a_range_does_not_apply_to_a_value_that_is_no_number() {
    passes(json!({ "minimum": 1, "maximum": 2 }), json!("x"));
}

#[test]
fn min_length_and_max_length_count_characters() {
    let schema = json!({ "type": "string", "minLength": 2, "maxLength": 3 });
    passes(schema.clone(), json!("ab"));
    passes(schema.clone(), json!("äöü"));
    fails_with(schema.clone(), json!("a"), "shorter than minLength 2");
    fails_with(schema, json!("abcd"), "longer than maxLength 3");
}

#[test]
fn pattern_matches_anywhere_unless_anchored() {
    passes(json!({ "pattern": "ab" }), json!("xxabxx"));
    passes(json!({ "pattern": "^[a-z]+$" }), json!("abc"));
    fails_with(
        json!({ "pattern": "^[a-z]+$" }),
        json!("ab1"),
        "does not match the pattern `^[a-z]+$`",
    );
}

#[test]
fn items_checks_every_element_and_names_its_place() {
    let schema = json!({ "type": "array", "items": { "type": "integer", "minimum": 0 } });
    passes(schema.clone(), json!([0, 1, 2]));
    fails_with(schema.clone(), json!([0, -1]), "at `[1]`");
    fails_with(schema, json!([0, "x"]), "at `[1]`");
}

#[test]
fn properties_required_and_additional_properties_are_checked() {
    let schema = json!({
        "type": "object",
        "properties": { "depth": { "type": "integer" }, "name": { "type": "string" } },
        "required": ["depth"],
        "additionalProperties": false
    });
    passes(schema.clone(), json!({ "depth": 1, "name": "x" }));
    fails_with(
        schema.clone(),
        json!({ "name": "x" }),
        "missing the required property `depth`",
    );
    fails_with(schema.clone(), json!({ "depth": "deep" }), "at `.depth`");
    fails_with(schema, json!({ "depth": 1, "extra": 1 }), "at `.extra`");
}

#[test]
fn additional_properties_may_be_a_schema() {
    let schema = json!({
        "type": "object",
        "properties": { "a": { "type": "string" } },
        "additionalProperties": { "type": "integer" }
    });
    passes(schema.clone(), json!({ "a": "x", "b": 1 }));
    fails_with(schema, json!({ "a": "x", "b": "no" }), "at `.b`");
}

#[test]
fn a_deep_object_names_the_whole_path_of_the_failure() {
    let schema = json!({
        "type": "object",
        "properties": { "lanes": { "type": "array", "items": {
            "type": "object", "properties": { "size": { "type": "integer", "minimum": 1 } }
        } } }
    });
    passes(
        schema.clone(),
        json!({ "lanes": [{ "size": 1 }, { "size": 9 }] }),
    );
    fails_with(
        schema,
        json!({ "lanes": [{ "size": 1 }, { "size": 0 }] }),
        "at `.lanes[1].size`",
    );
}

#[test]
fn every_failure_of_one_value_is_listed() {
    let found = violations(
        json!({ "type": "string", "minLength": 5, "pattern": "^x" }),
        json!("ab"),
    );
    assert_eq!(found.len(), 2, "{found:?}");
}

// ---- the message never holds the value ----

#[test]
fn a_message_names_the_constraint_and_the_type_and_never_the_value() {
    let secret = "sk-live-4242424242424242";
    for schema in [
        json!({ "type": "integer" }),
        json!({ "type": "string", "maxLength": 4 }),
        json!({ "type": "string", "pattern": "^[0-9]+$" }),
        json!({ "type": "string", "enum": ["a", "b"] }),
        json!({ "type": "string", "const": "a" }),
    ] {
        let found = violations(schema.clone(), json!(secret));
        assert!(!found.is_empty(), "{schema} should refuse the value");
        for text in found {
            assert!(
                !text.contains(secret),
                "the message holds the value: {text}"
            );
            assert!(
                !text.contains("4242"),
                "the message holds part of the value: {text}"
            );
            assert!(
                text.contains("it is "),
                "the message names the type: {text}"
            );
        }
    }
}

#[test]
fn a_value_inside_an_object_stays_out_of_the_message() {
    let secret = "hunter2-hunter2";
    let found = violations(
        json!({ "type": "object", "properties": { "token": { "type": "integer" } } }),
        json!({ "token": secret }),
    );
    assert_eq!(found.len(), 1);
    assert!(!found[0].contains(secret), "{}", found[0]);
}

// ---- the schema check at pack load ----

fn problems(schema: Value) -> Vec<SchemaProblem> {
    check_setting_schema("config.lanePool", &schema)
}

#[test]
fn a_sound_schema_has_no_problem() {
    let sound = [
        json!({ "type": "integer", "minimum": 1, "default": 2 }),
        json!({ "type": "string", "pattern": "^a", "minLength": 1, "maxLength": 9, "title": "t",
                "description": "d" }),
        json!({ "enum": ["a", "b"], "default": "a" }),
        json!({ "type": "object", "properties": { "a": { "type": "string" } }, "required": ["a"],
                "additionalProperties": false, "default": { "a": "x" } }),
        json!({ "type": "array", "items": { "type": "integer" }, "default": [1] }),
        json!({ "type": "string", "secret": true }),
        json!({ "type": "integer", "overridable": true, "repoOnly": false }),
        json!({ "const": 1 }),
    ];
    for schema in sound {
        assert_eq!(problems(schema.clone()), vec![], "{schema}");
    }
}

#[test]
fn a_keyword_outside_the_subset_is_refused_naming_it_and_its_place() {
    for keyword in [
        "$ref",
        "oneOf",
        "anyOf",
        "allOf",
        "not",
        "format",
        "$schema",
        "x-note",
        "if",
        "dependencies",
    ] {
        let found = problems(json!({ "type": "string", (keyword): 1 }));
        assert_eq!(
            found,
            vec![SchemaProblem::UnsupportedKeyword {
                at: "config.lanePool".into(),
                keyword: keyword.into()
            }],
            "{keyword}"
        );
        let text = found[0].to_string();
        assert!(
            text.contains(keyword) && text.contains("config.lanePool"),
            "{text}"
        );
    }
}

#[test]
fn the_message_for_an_unsupported_keyword_lists_what_is_supported() {
    let text = problems(json!({ "oneOf": [] }))[0].to_string();
    for keyword in SCHEMA_KEYWORDS {
        assert!(text.contains(keyword), "{keyword} missing from: {text}");
    }
}

#[test]
fn an_unsupported_keyword_below_the_top_is_refused_with_its_own_place() {
    let found = problems(json!({
        "type": "object",
        "properties": { "depth": { "type": "integer", "format": "int32" } }
    }));
    assert_eq!(
        found,
        vec![SchemaProblem::UnsupportedKeyword {
            at: "config.lanePool.properties.depth".into(),
            keyword: "format".into()
        }]
    );
    let found = problems(json!({ "type": "array", "items": { "$ref": "#/x" } }));
    assert_eq!(
        found,
        vec![SchemaProblem::UnsupportedKeyword {
            at: "config.lanePool.items".into(),
            keyword: "$ref".into()
        }]
    );
}

#[test]
fn a_setting_flag_is_allowed_only_at_the_top() {
    let found = problems(json!({
        "type": "object",
        "properties": { "a": { "type": "string", "secret": true } }
    }));
    assert_eq!(
        found,
        vec![SchemaProblem::UnsupportedKeyword {
            at: "config.lanePool.properties.a".into(),
            keyword: "secret".into()
        }]
    );
}

#[test]
fn a_supported_keyword_of_the_wrong_shape_is_refused_naming_it() {
    let cases = [
        json!({ "type": "text" }),
        json!({ "type": ["string", "null"] }),
        json!({ "enum": [] }),
        json!({ "enum": "a" }),
        json!({ "minimum": "1" }),
        json!({ "maximum": null }),
        json!({ "minLength": -1 }),
        json!({ "maxLength": 1.5 }),
        json!({ "pattern": 3 }),
        json!({ "pattern": "(" }),
        json!({ "items": [] }),
        json!({ "properties": [] }),
        json!({ "properties": { "a": 1 } }),
        json!({ "required": "a" }),
        json!({ "required": [1] }),
        json!({ "additionalProperties": 1 }),
    ];
    for schema in cases {
        let found = problems(schema.clone());
        assert!(
            matches!(found.as_slice(), [SchemaProblem::BadKeyword { .. }]),
            "{schema}: {found:?}"
        );
    }
}

#[test]
fn a_default_that_fails_its_own_schema_is_refused_naming_the_setting() {
    let found = problems(json!({ "type": "integer", "minimum": 1, "default": 0 }));
    assert_eq!(found.len(), 1, "{found:?}");
    let text = found[0].to_string();
    assert!(
        text.contains("config.lanePool")
            && text.contains("default")
            && text.contains("minimum of 1"),
        "{text}"
    );
    let found = problems(json!({ "type": "string", "default": 3 }));
    assert!(
        matches!(found.as_slice(), [SchemaProblem::DefaultFails { .. }]),
        "{found:?}"
    );
}

#[test]
fn a_default_below_the_top_is_held_to_its_own_schema_too() {
    let found = problems(json!({
        "type": "object",
        "properties": { "depth": { "type": "integer", "default": "deep" } }
    }));
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(
        found[0]
            .to_string()
            .contains("config.lanePool.properties.depth")
    );
}

#[test]
fn a_schema_that_is_no_mapping_is_left_to_the_manifest_check() {
    assert_eq!(check_setting_schema("config.a", &json!("string")), vec![]);
}

#[test]
fn a_default_is_not_checked_when_the_schema_is_unsound() {
    // One problem, the keyword's: not a second one for a default that cannot be judged.
    let found = problems(json!({ "type": "integer", "minimum": "1", "default": 0 }));
    assert_eq!(found.len(), 1, "{found:?}");
}
