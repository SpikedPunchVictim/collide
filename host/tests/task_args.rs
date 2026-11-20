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

// ---- `create`'s arguments, as `task.create` admits them (ts-0149-kq) ----

use collide_host::task_args::admit_create_args;

/// The `create` that Collide's own plan declares, as far as its argument names and types go.
fn collides_create() -> OpDecl {
    op(
        "create",
        json!({
            "type": "object",
            "properties": {
                "kind": { "enum": ["task", "epic"] },
                "epic": { "type": "string" },
                "title": { "type": "string" },
                "seed": { "type": "integer" },
                "root": { "enum": ["shared", "private"] },
                "raisedBy": { "type": "string" }
            },
            "required": ["title"],
            "additionalProperties": false
        }),
    )
}

/// A `create` of another source: a list, an integer and a boolean that Collide's plan does not have.
fn foreign_create() -> OpDecl {
    op(
        "create",
        json!({
            "type": "object",
            "properties": {
                "title": { "type": "string" },
                "tags": { "type": "array", "items": { "type": "string" } },
                "numbers": { "type": "array", "items": { "type": "integer" } },
                "points": { "type": "integer" },
                "flag": { "type": "boolean" },
                "note": {}
            },
            "additionalProperties": false
        }),
    )
}

fn fields(pairs: &[(&str, Value)]) -> BTreeMap<String, Value> {
    pairs
        .iter()
        .map(|(name, value)| (name.to_string(), value.clone()))
        .collect()
}

fn admitted(
    op: &OpDecl,
    own: &[(&str, Value)],
    extra: &[(&str, Value)],
) -> Result<BTreeMap<String, Value>, String> {
    let mut args = fields(own);
    admit_create_args(Some(op), &mut args, fields(extra))
        .map(|()| args)
        .map_err(|refused| refused.message)
}

#[test]
fn the_fields_collides_own_create_has_pass_through_unchanged() {
    let own = [
        ("title", json!("A title")),
        ("kind", json!("task")),
        ("epic", json!("ep-001-aa")),
        ("seed", json!(7)),
        ("root", json!("private")),
    ];

    let args = admitted(&collides_create(), &own, &[]).unwrap();

    assert_eq!(args, fields(&own));
}

#[test]
fn an_undeclared_name_is_refused_as_a_field_or_an_extra_and_the_declared_ones_are_listed() {
    let in_extra = admitted(
        &collides_create(),
        &[("title", json!("T"))],
        &[("colour", json!("red"))],
    )
    .unwrap_err();
    let as_field = admitted(
        &foreign_create(),
        &[("title", json!("T")), ("kind", json!("spike"))],
        &[],
    )
    .unwrap_err();

    assert_eq!(
        in_extra,
        "op `create` has no argument `colour`; its arguments are: epic, kind, raisedBy, root, seed, title"
    );
    assert_eq!(
        as_field,
        "op `create` has no argument `kind`; its arguments are: flag, note, numbers, points, tags, title"
    );
}

#[test]
fn text_is_read_as_the_declared_type_as_the_words_of_an_op_are() {
    let args = admitted(
        &foreign_create(),
        &[("title", json!("T"))],
        &[
            ("tags", json!("[\"a\",\"b\"]")),
            ("points", json!("-3")),
            ("flag", json!("true")),
            ("note", json!("1,2")),
        ],
    )
    .unwrap();

    assert_eq!(
        args,
        fields(&[
            ("title", json!("T")),
            ("tags", json!(["a", "b"])),
            ("points", json!(-3)),
            ("flag", json!(true)),
            ("note", json!("1,2")),
        ])
    );
}

#[test]
fn text_that_is_not_its_declared_type_is_refused_naming_the_argument() {
    for (name, text, said) in [
        ("flag", "yes", "`flag` must be true or false, not `yes`"),
        ("numbers", "1,2", "`numbers` must be JSON, not `1,2`"),
        ("points", "3x", "`points` must be a whole number, not `3x`"),
        ("tags", "a,b", "`tags` must be JSON, not `a,b`"),
    ] {
        let refused = admitted(&foreign_create(), &[], &[(name, json!(text))]).unwrap_err();

        assert!(refused.contains(said), "{name}={text}: {refused}");
    }
}

#[test]
fn a_value_that_is_not_text_is_the_sources_to_check() {
    let args = admitted(
        &foreign_create(),
        &[],
        &[("flag", json!(true)), ("tags", json!([1]))],
    )
    .unwrap();

    assert_eq!(args, fields(&[("flag", json!(true)), ("tags", json!([1]))]));
}

#[test]
fn a_name_that_is_a_field_and_an_extra_is_refused() {
    let refused = admitted(
        &collides_create(),
        &[("title", json!("T"))],
        &[("title", json!("U"))],
    )
    .unwrap_err();

    assert_eq!(refused, "`title` is given twice");
}

#[test]
fn an_op_that_declares_no_properties_is_not_checked_and_the_source_answers() {
    let bare: OpDecl = serde_json::from_value(json!({ "name": "create" })).unwrap();
    let mut args = fields(&[("title", json!("T"))]);

    admit_create_args(Some(&bare), &mut args, fields(&[("anything", json!("x"))])).unwrap();
    admit_create_args(None, &mut args, fields(&[("other", json!("y"))])).unwrap();

    assert_eq!(
        args,
        fields(&[
            ("title", json!("T")),
            ("anything", json!("x")),
            ("other", json!("y"))
        ])
    );
}
