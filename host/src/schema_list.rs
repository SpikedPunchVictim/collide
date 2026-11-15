//! The text of `collide schema`: every point and every event with its stage, then every type of the generated schema
//! with its keys.

use collide_core::events::EventKind;
use collide_core::points::Point;
use collide_core::schema;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

/// Adds the keys of one schema node: its own `properties`, and those of each `oneOf` or `anyOf` variant.
fn add_keys(node: &Value, keys: &mut BTreeSet<String>) {
    if let Some(properties) = node["properties"].as_object() {
        keys.extend(properties.keys().cloned());
    }
    for variants in ["oneOf", "anyOf"] {
        for variant in node[variants].as_array().into_iter().flatten() {
            add_keys(variant, keys);
        }
    }
}

/// The keys of every type that has any, by type name: each registered type and each type inside one (`$defs`).
/// Only the input schemas are read, because the output schemas have the same keys.
fn keys_by_type() -> BTreeMap<String, BTreeSet<String>> {
    let mut types: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for (file, text) in schema::registry().render() {
        if !file.ends_with(".input.json") {
            continue;
        }
        let root: Value = serde_json::from_str(&text).expect("a schema is JSON");
        let name = root["title"].as_str().expect("a schema has a title");
        let defs = root["$defs"].as_object().into_iter().flatten();
        for (name, node) in std::iter::once((name, &root)).chain(defs.map(|(n, d)| (n.as_str(), d)))
        {
            let mut keys = BTreeSet::new();
            add_keys(node, &mut keys);
            if !keys.is_empty() {
                types.entry(name.to_string()).or_default().extend(keys);
            }
        }
    }
    types
}

/// Plain text, in a fixed order: the points in catalogue order, each with its stage; the events likewise; then the
/// types and their keys, sorted.
pub fn listing() -> String {
    let mut text = String::from("points\n");
    for point in Point::ALL {
        text.push_str(&format!(
            "  {}  stage {}\n",
            point.name(),
            point.stage().id()
        ));
    }
    text.push_str("\nevents\n");
    for kind in EventKind::ALL {
        text.push_str(&format!("  {}  stage {}\n", kind.name(), kind.stage().id()));
    }
    text.push_str("\ntypes\n");
    for (name, keys) in keys_by_type() {
        text.push_str(&format!("  {name}\n"));
        for key in keys {
            text.push_str(&format!("    {key}\n"));
        }
    }
    text
}
