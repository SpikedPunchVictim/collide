//! Maps the words after `collide task <id> <op>` to the op's `args`, by the JSON Schema the provider declares for
//! the op in `initialize` (ts-0020).
//!
//! **The rule.** Each word is `name=value`, which sets the property `name`, or a bare value, which sets the op's
//! *positional* property: its only required property; failing that the only required property of the first
//! alternative in its `oneOf` or `anyOf` (so `setStatus active` sets `status`); failing that its only property. An
//! op with no such property takes no bare value, and a second bare value is an error. A value is read as the
//! property's declared `type` (`integer`, `number`, `boolean`; `array` and `object` as JSON) and is text otherwise.
//!
//! **What is checked here**, so a mistake is named at the command line: unknown names, the required names and the
//! `oneOf` alternatives, `enum` and the primitive `type`s. Lengths and patterns are the provider's to check, and it
//! does, with `refused`.

use collide_core::item::OpDecl;
use collide_core::provider::InitializeResult;
use serde_json::{Map, Value};
use std::collections::BTreeMap;

/// The ops that apply to an item of `kind`: the provider's own, then the kind's (a kind's op of one name replaces
/// the provider's).
pub fn declared_ops(init: &InitializeResult, kind: Option<&str>) -> Vec<OpDecl> {
    let mut ops = init.ops.clone();
    if let Some(decl) = init
        .kinds
        .iter()
        .find(|decl| Some(decl.name.as_str()) == kind)
    {
        for op in &decl.ops {
            ops.retain(|existing| existing.name != op.name);
            ops.push(op.clone());
        }
    }
    ops
}

fn properties(schema: &Value) -> Option<&Map<String, Value>> {
    schema.get("properties").and_then(Value::as_object)
}

fn required_of(schema: &Value) -> Vec<&str> {
    schema
        .get("required")
        .and_then(Value::as_array)
        .map(|names| names.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default()
}

/// The `oneOf` (or `anyOf`) alternatives of a schema.
fn alternatives(schema: &Value) -> Option<&Vec<Value>> {
    ["oneOf", "anyOf"]
        .iter()
        .find_map(|key| schema.get(*key).and_then(Value::as_array))
}

/// The property that a bare value sets, by the rule above.
pub fn positional(op: &OpDecl) -> Option<String> {
    let schema = op.args.as_ref()?;
    let sole = |names: Vec<&str>| (names.len() == 1).then(|| names[0].to_string());
    sole(required_of(schema))
        .or_else(|| {
            let first = alternatives(schema)?.first()?;
            sole(required_of(first))
        })
        .or_else(|| {
            let names: Vec<&str> = properties(schema)?.keys().map(String::as_str).collect();
            sole(names)
        })
}

fn type_name(declared: &Value) -> Option<&str> {
    declared.get("type").and_then(Value::as_str)
}

/// `text` as the value of a property declared as `declared`.
fn convert(name: &str, declared: Option<&Value>, text: &str) -> Result<Value, String> {
    let wrong = |wanted: &str| format!("`{name}` must be {wanted}, not `{text}`");
    match declared.and_then(type_name) {
        Some("integer") => text
            .parse::<i64>()
            .map(Value::from)
            .map_err(|_| wrong("a whole number")),
        Some("number") => text
            .parse::<f64>()
            .ok()
            .and_then(|number| serde_json::Number::from_f64(number).map(Value::Number))
            .ok_or_else(|| wrong("a number")),
        Some("boolean") => match text {
            "true" => Ok(Value::Bool(true)),
            "false" => Ok(Value::Bool(false)),
            _ => Err(wrong("true or false")),
        },
        Some("array") | Some("object") => serde_json::from_str(text).map_err(|_| wrong("JSON")),
        _ => Ok(Value::String(text.to_string())),
    }
}

/// What a person types for `op`: its properties, `[optional]` or required, then the positional one.
pub fn describe(op: &OpDecl) -> String {
    let mut line = op.name.clone();
    let Some(schema) = &op.args else {
        return line;
    };
    let required = required_of(schema);
    let in_alternatives: Vec<&str> = alternatives(schema)
        .map(|alts| alts.iter().flat_map(required_of).collect())
        .unwrap_or_default();
    let mut names: Vec<(&String, &Value)> = properties(schema)
        .map(|props| props.iter().collect())
        .unwrap_or_default();
    // The bare-value property first, then the required ones, so that the line reads in the order a person types.
    let bare = positional(op);
    names.sort_by_key(|(name, _)| {
        (
            bare.as_deref() != Some(name.as_str()),
            !required.contains(&name.as_str()),
        )
    });
    for (name, declared) in names {
        let shape = match declared.get("enum").and_then(Value::as_array) {
            Some(values) => values
                .iter()
                .map(|value| value.as_str().map_or(value.to_string(), str::to_string))
                .collect::<Vec<_>>()
                .join("|"),
            None => type_name(declared).unwrap_or("text").to_string(),
        };
        let word = format!("{name}=<{shape}>");
        let optional = !required.contains(&name.as_str());
        let alt = in_alternatives.contains(&name.as_str());
        line.push_str(&match (optional, alt) {
            (false, _) => format!(" {word}"),
            (true, true) => format!(" [{word}]*"),
            (true, false) => format!(" [{word}]"),
        });
    }
    if alternatives(schema).is_some() {
        line.push_str("  (* give exactly one of these)");
    }
    if let Some(name) = positional(op) {
        line.push_str(&format!("  (a bare value sets {name})"));
    }
    line
}

/// Builds the op's `args` from `words` and checks them against the declared schema.
pub fn map_args(op: &OpDecl, words: &[String]) -> Result<BTreeMap<String, Value>, String> {
    let schema = op.args.clone().unwrap_or(Value::Null);
    let declared = properties(&schema);
    let mut args: BTreeMap<String, Value> = BTreeMap::new();
    let mut bare: Option<&str> = None;
    for word in words {
        let (name, text) = match word.split_once('=') {
            Some((name, text)) if !name.is_empty() => (name.to_string(), text),
            _ => match positional(op) {
                Some(name) if bare.is_none() => {
                    bare = Some(word);
                    (name, word.as_str())
                }
                Some(_) => {
                    return Err(format!(
                        "`{word}` is a second bare value; name each value as name=value"
                    ));
                }
                None => {
                    return Err(format!(
                        "`{word}` is not name=value, and op `{}` has no property that a bare value could set",
                        op.name
                    ));
                }
            },
        };
        if let Some(declared) = declared {
            let allows_more = schema
                .get("additionalProperties")
                .is_some_and(|extra| extra != &Value::Bool(false));
            if !declared.contains_key(&name) && !allows_more {
                let known: Vec<&str> = declared.keys().map(String::as_str).collect();
                return Err(format!(
                    "op `{}` has no argument `{name}`; its arguments are: {}",
                    op.name,
                    if known.is_empty() {
                        "none".to_string()
                    } else {
                        known.join(", ")
                    }
                ));
            }
        }
        if args.contains_key(&name) {
            return Err(format!("`{name}` is given twice"));
        }
        let value = convert(&name, declared.and_then(|props| props.get(&name)), text)?;
        args.insert(name, value);
    }
    check(op, &schema, &args)?;
    Ok(args)
}

fn check(op: &OpDecl, schema: &Value, args: &BTreeMap<String, Value>) -> Result<(), String> {
    let missing: Vec<&str> = required_of(schema)
        .into_iter()
        .filter(|name| !args.contains_key(*name))
        .collect();
    if !missing.is_empty() {
        return Err(format!(
            "op `{}` needs {}: {}",
            op.name,
            missing.join(", "),
            describe(op)
        ));
    }
    if let Some(options) = alternatives(schema) {
        let met = options
            .iter()
            .filter(|option| {
                required_of(option)
                    .iter()
                    .all(|name| args.contains_key(*name))
            })
            .count();
        let one_of = schema.get("oneOf").is_some();
        let names: Vec<String> = options
            .iter()
            .map(|option| required_of(option).join("+"))
            .collect();
        if met == 0 {
            return Err(format!(
                "op `{}` needs one of: {}",
                op.name,
                names.join(", ")
            ));
        }
        if one_of && met > 1 {
            return Err(format!(
                "op `{}` takes only one of: {}",
                op.name,
                names.join(", ")
            ));
        }
    }
    if let Some(declared) = properties(schema) {
        for (name, value) in args {
            let Some(spec) = declared.get(name) else {
                continue;
            };
            if let Some(allowed) = spec.get("enum").and_then(Value::as_array)
                && !allowed.contains(value)
            {
                let list: Vec<String> = allowed
                    .iter()
                    .map(|v| v.as_str().map_or(v.to_string(), str::to_string))
                    .collect();
                return Err(format!(
                    "`{name}` must be one of {}, not {}",
                    list.join(", "),
                    value.as_str().map_or(value.to_string(), str::to_string)
                ));
            }
        }
    }
    Ok(())
}

/// An argument of `create` that was refused before anything was written: its name and the reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArgRefused {
    pub name: String,
    pub message: String,
}

/// Adds the arguments in `extra` to the `args` of a `create`, and checks them all against the op's declaration.
///
/// Every name in `args` (the fields `task.create` has of its own) and in `extra` must be a property the op declares;
/// the refusal lists the ones it declares. A value in `extra` that is text is read as the type the op declares for its
/// name, as a word after `collide task <id> <op>` is ([`convert`]: a list or an object is JSON), so the CLI has one way
/// to give a value. What the op requires is not checked: the source refuses a missing argument in its own words.
/// An op that declares no properties (or no op at all) gives nothing to check against, so the arguments go through
/// as they are and the source answers.
pub fn admit_create_args(
    op: Option<&OpDecl>,
    args: &mut BTreeMap<String, Value>,
    extra: BTreeMap<String, Value>,
) -> Result<(), ArgRefused> {
    let Some(declared) = op.and_then(|op| op.args.as_ref()).and_then(properties) else {
        args.extend(extra);
        return Ok(());
    };
    if let Some(name) = args
        .keys()
        .chain(extra.keys())
        .find(|name| !declared.contains_key(*name))
    {
        let known: Vec<&str> = declared.keys().map(String::as_str).collect();
        return Err(ArgRefused {
            name: name.clone(),
            message: format!(
                "op `create` has no argument `{name}`; its arguments are: {}",
                if known.is_empty() {
                    "none".to_string()
                } else {
                    known.join(", ")
                }
            ),
        });
    }
    for (name, value) in extra {
        if args.contains_key(&name) {
            return Err(ArgRefused {
                message: format!("`{name}` is given twice"),
                name,
            });
        }
        let value = match value {
            Value::String(text) => {
                convert(&name, declared.get(&name), &text).map_err(|message| ArgRefused {
                    name: name.clone(),
                    message,
                })?
            }
            other => other,
        };
        args.insert(name, value);
    }
    Ok(())
}
