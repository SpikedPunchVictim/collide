//! Tests for the mapping from the words of `collide task <id> <op> ...` to an op's `args` (ts-0020), over the
//! schemas the plan provider declares.

use collide_core::item::OpDecl;
use collide_host::task_args::{describe, map_args, positional};
use serde_json::{Value, json};
use std::collections::BTreeMap;

fn op(name: &str, args: Value) -> OpDecl {
    serde_json::from_value(json!({ "name": name, "args": args })).unwrap()
}

fn set_status() -> OpDecl {
    let statuses = json!(["ready", "active", "review", "done", "dropped"]);
    op(
        "setStatus",
        json!({
            "type": "object",
            "properties": { "status": { "enum": statuses }, "category": { "enum": statuses } },
            "oneOf": [{ "required": ["status"] }, { "required": ["category"] }],
            "additionalProperties": false
        }),
    )
}

fn link() -> OpDecl {
    op(
        "link",
        json!({
            "type": "object",
            "properties": { "to": { "type": "string" }, "type": { "enum": ["blocked-by"] } },
            "required": ["to"],
            "additionalProperties": false
        }),
    )
}

fn unclaim() -> OpDecl {
    op(
        "unclaim",
        json!({ "type": "object", "properties": {}, "additionalProperties": false }),
    )
}

fn words(list: &[&str]) -> Vec<String> {
    list.iter().map(|word| word.to_string()).collect()
}

fn args(op: &OpDecl, list: &[&str]) -> Result<BTreeMap<String, Value>, String> {
    map_args(op, &words(list))
}

#[test]
fn a_bare_value_sets_the_only_required_property() {
    assert_eq!(
        args(&link(), &["ts-0001-aa"]).unwrap()["to"],
        json!("ts-0001-aa")
    );
    assert_eq!(positional(&link()).as_deref(), Some("to"));
}

#[test]
fn a_bare_value_sets_the_required_property_of_the_first_alternative() {
    assert_eq!(
        args(&set_status(), &["active"]).unwrap()["status"],
        json!("active")
    );
    assert_eq!(positional(&set_status()).as_deref(), Some("status"));
}

#[test]
fn name_value_words_set_named_properties() {
    let mapped = args(&set_status(), &["category=review"]).unwrap();
    assert_eq!(
        mapped,
        BTreeMap::from([("category".to_string(), json!("review"))])
    );
    let mapped = args(&link(), &["to=ts-0001-aa", "type=blocked-by"]).unwrap();
    assert_eq!(mapped["type"], json!("blocked-by"));
}

#[test]
fn a_value_may_contain_an_equals_sign_after_the_name() {
    let claim = op(
        "claim",
        json!({ "type": "object", "properties": { "claimant": { "type": "string" } }, "required": ["claimant"] }),
    );
    assert_eq!(
        args(&claim, &["claimant=a=b"]).unwrap()["claimant"],
        json!("a=b")
    );
}

#[test]
fn an_op_with_no_properties_takes_no_words() {
    assert!(args(&unclaim(), &[]).unwrap().is_empty());
    let error = args(&unclaim(), &["x"]).unwrap_err();
    assert!(
        error.contains("no property that a bare value could set"),
        "{error}"
    );
}

#[test]
fn a_second_bare_value_is_an_error() {
    let error = args(&link(), &["ts-0001-aa", "ts-0002-bb"]).unwrap_err();
    assert!(error.contains("second bare value"), "{error}");
}

#[test]
fn an_unknown_name_lists_the_arguments_the_op_has() {
    let error = args(&link(), &["to=ts-0001-aa", "colour=red"]).unwrap_err();
    assert!(error.contains("no argument `colour`"), "{error}");
    assert!(error.contains("to, type"), "{error}");
}

#[test]
fn a_value_outside_the_enum_lists_the_allowed_ones() {
    let error = args(&set_status(), &["finished"]).unwrap_err();
    assert!(
        error.contains("ready, active, review, done, dropped"),
        "{error}"
    );
}

#[test]
fn a_missing_required_name_is_an_error() {
    let error = args(&link(), &["type=blocked-by"]).unwrap_err();
    assert!(error.contains("needs to"), "{error}");
}

#[test]
fn a_oneof_with_none_or_both_alternatives_is_an_error() {
    assert!(
        args(&set_status(), &[])
            .unwrap_err()
            .contains("needs one of")
    );
    let both = args(&set_status(), &["status=active", "category=active"]).unwrap_err();
    assert!(both.contains("only one of"), "{both}");
}

#[test]
fn a_name_given_twice_is_an_error() {
    let error = args(&link(), &["to=ts-0001-aa", "to=ts-0002-bb"]).unwrap_err();
    assert!(error.contains("given twice"), "{error}");
}

#[test]
fn values_are_read_as_the_declared_type() {
    let typed = op(
        "typed",
        json!({
            "type": "object",
            "properties": {
                "count": { "type": "integer" },
                "ratio": { "type": "number" },
                "flag": { "type": "boolean" },
                "tags": { "type": "array" },
                "note": { "type": "string" }
            }
        }),
    );
    let mapped = args(
        &typed,
        &[
            "count=3",
            "ratio=0.5",
            "flag=true",
            "tags=[\"a\",\"b\"]",
            "note=42",
        ],
    )
    .unwrap();
    assert_eq!(mapped["count"], json!(3));
    assert_eq!(mapped["ratio"], json!(0.5));
    assert_eq!(mapped["flag"], json!(true));
    assert_eq!(mapped["tags"], json!(["a", "b"]));
    assert_eq!(mapped["note"], json!("42"));
    assert!(
        args(&typed, &["count=three"])
            .unwrap_err()
            .contains("whole number")
    );
    assert!(
        args(&typed, &["flag=yes"])
            .unwrap_err()
            .contains("true or false")
    );
}

#[test]
fn an_op_without_a_schema_takes_only_name_value_words_and_no_checks() {
    let bare: OpDecl = serde_json::from_value(json!({ "name": "poke" })).unwrap();
    assert!(args(&bare, &[]).unwrap().is_empty());
    assert_eq!(args(&bare, &["a=1"]).unwrap()["a"], json!("1"));
}

#[test]
fn describe_gives_the_op_its_words_and_its_bare_value() {
    assert_eq!(
        describe(&link()),
        "link to=<string> [type=<blocked-by>]  (a bare value sets to)"
    );
    assert_eq!(describe(&unclaim()), "unclaim");
    let text = describe(&set_status());
    assert!(
        text.starts_with("setStatus [status=<ready|active|review|done|dropped>]*"),
        "{text}"
    );
}
