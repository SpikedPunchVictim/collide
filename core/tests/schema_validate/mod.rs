//! A small JSON Schema validator for tests, so that a document can be checked against a schema that
//! `collide_core::schema` generated, without a validator crate in the workspace. It reads only the keywords
//! `schemars` writes for the contract types: `$ref` into `$defs`, `type`, `enum`, `const`, `anyOf`, `oneOf`,
//! `properties`, `required`, `additionalProperties`, `items`, `minLength` and `minimum`. `format`, `pattern`,
//! `default` and `description` are not checked. A keyword it does not know fails the test, so the schema cannot
//! start to use one that this check would skip.

use serde_json::Value;

const KNOWN: &[&str] = &[
    "$ref",
    "$schema",
    "$defs",
    "title",
    "description",
    "default",
    "format",
    "pattern",
    "type",
    "enum",
    "const",
    "anyOf",
    "oneOf",
    "properties",
    "required",
    "additionalProperties",
    "items",
    "minLength",
    "minimum",
];

/// Every way `doc` breaks `root`, each with the path to the value, or an empty list when it validates.
pub fn violations(root: &Value, doc: &Value) -> Vec<String> {
    let mut found = Vec::new();
    check(root, root, doc, "$", &mut found);
    found
}

fn type_matches(name: &str, doc: &Value) -> bool {
    match name {
        "string" => doc.is_string(),
        "boolean" => doc.is_boolean(),
        "object" => doc.is_object(),
        "array" => doc.is_array(),
        "null" => doc.is_null(),
        "integer" => doc.is_i64() || doc.is_u64() || doc.as_f64().is_some_and(|n| n.fract() == 0.0),
        "number" => doc.is_number(),
        other => panic!("unknown schema type `{other}`"),
    }
}

fn check(root: &Value, schema: &Value, doc: &Value, path: &str, found: &mut Vec<String>) {
    let Some(map) = schema.as_object() else {
        // `true` accepts everything and `false` nothing.
        if schema == &Value::Bool(false) {
            found.push(format!("{path}: not allowed"));
        }
        return;
    };
    for key in map.keys() {
        assert!(
            KNOWN.contains(&key.as_str()),
            "schema keyword `{key}` is not checked"
        );
    }
    if let Some(Value::String(reference)) = map.get("$ref") {
        let name = reference
            .strip_prefix("#/$defs/")
            .unwrap_or_else(|| panic!("reference `{reference}` is not into $defs"));
        check(root, &root["$defs"][name], doc, path, found);
    }
    if let Some(kind) = map.get("type") {
        let names: Vec<&str> = match kind {
            Value::String(name) => vec![name.as_str()],
            Value::Array(names) => names.iter().filter_map(Value::as_str).collect(),
            other => panic!("bad type {other}"),
        };
        if !names.iter().any(|name| type_matches(name, doc)) {
            found.push(format!("{path}: expected {names:?}"));
            return;
        }
    }
    if let Some(Value::Array(allowed)) = map.get("enum")
        && !allowed.contains(doc)
    {
        found.push(format!("{path}: {doc} is not one of {allowed:?}"));
    }
    if let Some(expected) = map.get("const")
        && expected != doc
    {
        found.push(format!("{path}: {doc} is not {expected}"));
    }
    for (keyword, exactly_one) in [("anyOf", false), ("oneOf", true)] {
        if let Some(Value::Array(options)) = map.get(keyword) {
            let matching = options
                .iter()
                .filter(|option| {
                    let mut inner = Vec::new();
                    check(root, option, doc, path, &mut inner);
                    inner.is_empty()
                })
                .count();
            if matching == 0 || (exactly_one && matching != 1) {
                found.push(format!(
                    "{path}: matches {matching} of the {keyword} options"
                ));
            }
        }
    }
    if let (Some(Value::Number(min)), Some(text)) = (map.get("minLength"), doc.as_str())
        && (text.chars().count() as u64) < min.as_u64().unwrap()
    {
        found.push(format!("{path}: shorter than {min} characters"));
    }
    if let (Some(Value::Number(min)), Some(number)) = (map.get("minimum"), doc.as_f64())
        && number < min.as_f64().unwrap()
    {
        found.push(format!("{path}: {number} is under {min}"));
    }
    if let Some(object) = doc.as_object() {
        let properties = map.get("properties").and_then(Value::as_object);
        for (key, value) in object {
            let child = format!("{path}.{key}");
            match properties.and_then(|properties| properties.get(key)) {
                Some(property) => check(root, property, value, &child, found),
                None => match map.get("additionalProperties") {
                    Some(Value::Bool(false)) => found.push(format!("{child}: unknown key")),
                    Some(extra) if extra.is_object() => check(root, extra, value, &child, found),
                    _ => {}
                },
            }
        }
        if let Some(Value::Array(required)) = map.get("required") {
            for key in required.iter().filter_map(Value::as_str) {
                if !object.contains_key(key) {
                    found.push(format!("{path}: missing `{key}`"));
                }
            }
        }
    }
    if let (Some(items), Some(list)) = (map.get("items"), doc.as_array()) {
        for (index, item) in list.iter().enumerate() {
            check(root, items, item, &format!("{path}[{index}]"), found);
        }
    }
}
