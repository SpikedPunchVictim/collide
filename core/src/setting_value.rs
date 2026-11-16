//! Whether a setting's value meets the pack's schema (0.7, Settings: "one JSON Schema per key"; Stage 4a).
//!
//! The workspace has no validator crate, so this is a hand-written subset. It covers the keywords a setting uses:
//! `type`, `enum`, `const`, `minimum`, `maximum`, `minLength`, `maxLength`, `pattern`, `items`, `properties`,
//! `required` and `additionalProperties`, and it reads `default`, `description` and `title` without checking them.
//! A setting may also carry `secret`, `overridable` and `repoOnly` beside its keywords; they are the layers' and sit
//! at the top of the schema only.
//!
//! **Fail closed.** [`check_setting_schema`] runs when a pack loads. A keyword outside the subset (`$ref`, `oneOf`,
//! `format`) is a problem naming it, so a pack never relies on a check that does not run. A supported keyword of the
//! wrong shape is a problem too, and so is a `default` that does not meet its own schema.
//!
//! [`check_value`] names the constraint and the value's type, and never the value: a secret that reached a file by
//! mistake must not be copied into a message.
//!
//! Differences from JSON Schema that a pack author may meet: `type` is one name, not a list; `pattern` is a Rust
//! regular expression (`regex` crate), not ECMAScript; `minimum` and `maximum` compare as 64-bit floats.

use regex::{Regex, RegexBuilder};
use serde_json::{Map, Value};
use std::fmt;

/// The keywords of the subset, checked or read.
pub const SCHEMA_KEYWORDS: [&str; 15] = [
    "type",
    "enum",
    "const",
    "minimum",
    "maximum",
    "minLength",
    "maxLength",
    "pattern",
    "items",
    "properties",
    "required",
    "additionalProperties",
    "default",
    "description",
    "title",
];

/// The names a setting may add at the top of its schema. They are not JSON Schema: the layers read them.
pub const SETTING_FLAGS: [&str; 3] = ["secret", "overridable", "repoOnly"];

/// The names `type` may take.
const TYPES: [&str; 7] = [
    "string", "number", "integer", "boolean", "array", "object", "null",
];

/// The size a `pattern` may compile to, in bytes. A pack's pattern is untrusted input (0.9).
const PATTERN_SIZE_LIMIT: usize = 1 << 20;

/// What is wrong with a setting's schema. `at` is a place in the pack: `config.lanePool`, or below it
/// `config.opts.properties.depth`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchemaProblem {
    /// A keyword outside the subset.
    UnsupportedKeyword { at: String, keyword: String },
    /// A supported keyword whose value is the wrong shape. `expected` says what it should be.
    BadKeyword {
        at: String,
        keyword: &'static str,
        expected: &'static str,
    },
    /// A `default` that does not meet its own schema. `violation` says how.
    DefaultFails { at: String, violation: Violation },
}

impl fmt::Display for SchemaProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SchemaProblem::UnsupportedKeyword { at, keyword } => write!(
                f,
                "`{at}`: the keyword `{keyword}` is not supported in a setting's schema; use only {}",
                SCHEMA_KEYWORDS.join(", ")
            ),
            SchemaProblem::BadKeyword {
                at,
                keyword,
                expected,
            } => write!(f, "`{at}`: `{keyword}` must be {expected}"),
            SchemaProblem::DefaultFails { at, violation } => write!(
                f,
                "`{at}`: `default` does not meet the setting's own schema: {violation}"
            ),
        }
    }
}

impl std::error::Error for SchemaProblem {}

/// One way a value fails its schema. `at` is the place inside the value (`.depth`, `[2]`; empty for the value
/// itself) and `found` the type of what is there. The text never holds the value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    pub at: String,
    pub constraint: String,
    pub found: &'static str,
}

impl fmt::Display for Violation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let place = if self.at.is_empty() {
            String::new()
        } else {
            format!("at `{}` ", self.at)
        };
        write!(f, "{place}{} (it is {})", self.constraint, self.found)
    }
}

/// The JSON type of `value`, with the article: `a string`, `an integer`.
pub fn type_of(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "a boolean",
        Value::Number(_) if is_integer(value) => "an integer",
        Value::Number(_) => "a number",
        Value::String(_) => "a string",
        Value::Array(_) => "an array",
        Value::Object(_) => "an object",
    }
}

/// A number with no fraction, which JSON Schema calls an integer even when it is written `2.0`.
fn is_integer(value: &Value) -> bool {
    match value {
        Value::Number(number) => {
            number.is_i64()
                || number.is_u64()
                || number.as_f64().is_some_and(|float| float.fract() == 0.0)
        }
        _ => false,
    }
}

fn compile(pattern: &str) -> Option<Regex> {
    RegexBuilder::new(pattern)
        .size_limit(PATTERN_SIZE_LIMIT)
        .build()
        .ok()
}

/// Equality for `enum` and `const`: numbers compare by value, so `2` equals `2.0`.
fn same(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Number(a), Value::Number(b)) => a.as_f64() == b.as_f64(),
        (Value::Array(a), Value::Array(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(x, y)| same(x, y))
        }
        (Value::Object(a), Value::Object(b)) => {
            a.len() == b.len()
                && a.iter()
                    .all(|(key, x)| b.get(key).is_some_and(|y| same(x, y)))
        }
        _ => left == right,
    }
}

// ---- the schema check, at pack load ------------------------------------------------------------------------------

/// Checks the schema of one setting, `at` being its place in the pack (`config.lanePool`). The flags of
/// [`SETTING_FLAGS`] are allowed at the top. A schema that is not a mapping is [`crate::pack_check`]'s to refuse and
/// gives no problem here.
pub fn check_setting_schema(at: &str, schema: &Value) -> Vec<SchemaProblem> {
    let mut problems = Vec::new();
    if let Some(map) = schema.as_object() {
        walk(at, map, true, &mut problems);
    }
    problems
}

fn bad(at: &str, keyword: &'static str, expected: &'static str, out: &mut Vec<SchemaProblem>) {
    out.push(SchemaProblem::BadKeyword {
        at: at.to_string(),
        keyword,
        expected,
    });
}

/// A sub-schema: `items`, a `properties` entry, or an `additionalProperties` that is not a boolean.
fn sub_schema(at: String, value: &Value, keyword: &'static str, out: &mut Vec<SchemaProblem>) {
    match value.as_object() {
        Some(map) => walk(&at, map, false, out),
        None => bad(&at, keyword, "a mapping", out),
    }
}

fn walk(at: &str, schema: &Map<String, Value>, top: bool, out: &mut Vec<SchemaProblem>) {
    let before = out.len();
    for (keyword, value) in schema {
        match keyword.as_str() {
            "type" => {
                if !value.as_str().is_some_and(|name| TYPES.contains(&name)) {
                    bad(
                        at,
                        "type",
                        "one of string, number, integer, boolean, array, object, null",
                        out,
                    );
                }
            }
            "enum" => {
                if !value.as_array().is_some_and(|list| !list.is_empty()) {
                    bad(at, "enum", "a list with at least one value", out);
                }
            }
            "minimum" | "maximum" => {
                if !value.is_number() {
                    let name = if keyword == "minimum" {
                        "minimum"
                    } else {
                        "maximum"
                    };
                    bad(at, name, "a number", out);
                }
            }
            "minLength" | "maxLength" => {
                if value.as_u64().is_none() {
                    let name = if keyword == "minLength" {
                        "minLength"
                    } else {
                        "maxLength"
                    };
                    bad(at, name, "a whole number, 0 or more", out);
                }
            }
            "pattern" => match value.as_str() {
                Some(pattern) if compile(pattern).is_some() => {}
                Some(_) => bad(at, "pattern", "a regular expression that compiles", out),
                None => bad(at, "pattern", "text", out),
            },
            "items" => sub_schema(format!("{at}.items"), value, "items", out),
            "properties" => match value.as_object() {
                Some(properties) => {
                    for (name, property) in properties {
                        sub_schema(
                            format!("{at}.properties.{name}"),
                            property,
                            "properties",
                            out,
                        );
                    }
                }
                None => bad(at, "properties", "a mapping of schemas", out),
            },
            "required" => {
                let names = value
                    .as_array()
                    .is_some_and(|list| list.iter().all(Value::is_string));
                if !names {
                    bad(at, "required", "a list of property names", out);
                }
            }
            "additionalProperties" => {
                if !value.is_boolean() {
                    sub_schema(
                        format!("{at}.additionalProperties"),
                        value,
                        "additionalProperties",
                        out,
                    );
                }
            }
            "const" | "default" | "description" | "title" => {}
            flag if top && SETTING_FLAGS.contains(&flag) => {}
            other => out.push(SchemaProblem::UnsupportedKeyword {
                at: at.to_string(),
                keyword: other.to_string(),
            }),
        }
    }
    // A default is checked only against a schema that is itself sound.
    if out.len() == before
        && let Some(default) = schema.get("default")
    {
        let node = Value::Object(schema.clone());
        for violation in check_value(&node, default) {
            out.push(SchemaProblem::DefaultFails {
                at: at.to_string(),
                violation,
            });
        }
    }
}

// ---- the value check ---------------------------------------------------------------------------------------------

/// Every way `value` fails `schema`, in the order of the keywords. Empty when it meets it. A keyword that is not
/// supported is not checked here: [`check_setting_schema`] refused it when the pack loaded.
pub fn check_value(schema: &Value, value: &Value) -> Vec<Violation> {
    let mut found = Vec::new();
    check_at(schema, value, "", &mut found);
    found
}

fn fail(at: &str, value: &Value, constraint: String, out: &mut Vec<Violation>) {
    out.push(Violation {
        at: at.to_string(),
        constraint,
        found: type_of(value),
    });
}

fn check_at(schema: &Value, value: &Value, at: &str, out: &mut Vec<Violation>) {
    let Some(schema) = schema.as_object() else {
        return;
    };
    if let Some(name) = schema.get("type").and_then(Value::as_str) {
        let fits = match name {
            "string" => value.is_string(),
            "boolean" => value.is_boolean(),
            "integer" => is_integer(value),
            "number" => value.is_number(),
            "array" => value.is_array(),
            "object" => value.is_object(),
            "null" => value.is_null(),
            _ => true,
        };
        if !fits {
            fail(at, value, format!("must be of type {name}"), out);
            return;
        }
    }
    if let Some(Value::Array(allowed)) = schema.get("enum")
        && !allowed.iter().any(|one| same(one, value))
    {
        let list: Vec<String> = allowed.iter().map(Value::to_string).collect();
        fail(at, value, format!("is not one of {}", list.join(", ")), out);
    }
    if let Some(only) = schema.get("const")
        && !same(only, value)
    {
        fail(at, value, format!("must equal {only}"), out);
    }
    if let Value::Number(number) = value
        && let Some(found) = number.as_f64()
    {
        if let Some(min) = schema.get("minimum").and_then(Value::as_f64)
            && found < min
        {
            fail(at, value, format!("is under the minimum of {min}"), out);
        }
        if let Some(max) = schema.get("maximum").and_then(Value::as_f64)
            && found > max
        {
            fail(at, value, format!("is over the maximum of {max}"), out);
        }
    }
    if let Value::String(text) = value {
        let length = text.chars().count() as u64;
        if let Some(min) = schema.get("minLength").and_then(Value::as_u64)
            && length < min
        {
            fail(at, value, format!("is shorter than minLength {min}"), out);
        }
        if let Some(max) = schema.get("maxLength").and_then(Value::as_u64)
            && length > max
        {
            fail(at, value, format!("is longer than maxLength {max}"), out);
        }
        if let Some(pattern) = schema.get("pattern").and_then(Value::as_str)
            && let Some(regex) = compile(pattern)
            && !regex.is_match(text)
        {
            fail(
                at,
                value,
                format!("does not match the pattern `{pattern}`"),
                out,
            );
        }
    }
    if let (Value::Array(list), Some(items)) = (value, schema.get("items")) {
        for (index, item) in list.iter().enumerate() {
            check_at(items, item, &format!("{at}[{index}]"), out);
        }
    }
    if let Value::Object(map) = value {
        object_rules(schema, map, value, at, out);
    }
}

fn object_rules(
    schema: &Map<String, Value>,
    map: &Map<String, Value>,
    value: &Value,
    at: &str,
    out: &mut Vec<Violation>,
) {
    let properties = schema.get("properties").and_then(Value::as_object);
    if let Some(Value::Array(required)) = schema.get("required") {
        for name in required.iter().filter_map(Value::as_str) {
            if !map.contains_key(name) {
                fail(
                    at,
                    value,
                    format!("is missing the required property `{name}`"),
                    out,
                );
            }
        }
    }
    if let Some(properties) = properties {
        for (name, property) in properties {
            if let Some(found) = map.get(name) {
                check_at(property, found, &format!("{at}.{name}"), out);
            }
        }
    }
    match schema.get("additionalProperties") {
        Some(Value::Bool(false)) => {
            for name in map
                .keys()
                .filter(|name| !properties.is_some_and(|p| p.contains_key(*name)))
            {
                fail(
                    &format!("{at}.{name}"),
                    &map[name],
                    "is not a property the schema lists".to_string(),
                    out,
                );
            }
        }
        Some(extra @ Value::Object(_)) => {
            for (name, found) in map {
                if !properties.is_some_and(|p| p.contains_key(name)) {
                    check_at(extra, found, &format!("{at}.{name}"), out);
                }
            }
        }
        _ => {}
    }
}
