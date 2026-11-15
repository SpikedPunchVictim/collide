//! Tests for the schema pipeline (ts-0008). The sample types are in `fixtures/`; the registry tests also run
//! against the real registry, which later items fill in.

mod fixtures;

use collide_core::schema::{self, Registry};
use fixtures::{Sample, Step};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

fn sample_json() -> Value {
    json!({
        "name": "demo",
        "retries": 3,
        "budgetBytes": 1_000_000,
        "note": "hello",
        "labels": { "team": "core" },
        "steps": [{ "kind": "agent", "prompt": "go", "readOnly": true }, { "kind": "wait" }],
        "level1": { "level2": { "level3": { "value": "deep" } } }
    })
}

/// Parses a file of the registry's output.
fn schema_file(registry: &Registry, name: &str) -> Value {
    let text = &registry.render()[name];
    serde_json::from_str(text).unwrap()
}

/// Every object schema with `properties` must set `additionalProperties: false`.
fn collect_open_objects(value: &Value, path: &str, found: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            if map.contains_key("properties")
                && map.get("additionalProperties") != Some(&json!(false))
            {
                found.push(path.to_string());
            }
            for (key, child) in map {
                collect_open_objects(child, &format!("{path}/{key}"), found);
            }
        }
        Value::Array(items) => {
            for (i, child) in items.iter().enumerate() {
                collect_open_objects(child, &format!("{path}/{i}"), found);
            }
        }
        _ => {}
    }
}

fn open_objects(registry: &Registry) -> Vec<String> {
    let mut found = Vec::new();
    for (name, text) in registry.render() {
        let value: Value = serde_json::from_str(&text).unwrap();
        collect_open_objects(&value, &name, &mut found);
    }
    found
}

/// The files in `dir` that end in `.json`, by file name.
fn read_json_files(dir: &Path) -> BTreeMap<String, String> {
    let mut files = BTreeMap::new();
    for entry in fs::read_dir(dir).unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()))
    {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|ext| ext == "json") {
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            files.insert(name, fs::read_to_string(&path).unwrap());
        }
    }
    files
}

fn assert_in_sync(registry: &Registry, dir: &Path) {
    let expected = registry.render();
    let actual = read_json_files(dir);
    let mut problems = Vec::new();
    for (name, text) in &expected {
        match actual.get(name) {
            None => problems.push(format!("missing: {name}")),
            Some(on_disk) if on_disk != text => problems.push(format!("changed: {name}")),
            Some(_) => {}
        }
    }
    for name in actual.keys().filter(|name| !expected.contains_key(*name)) {
        problems.push(format!("stale: {name}"));
    }
    assert!(
        problems.is_empty(),
        "schemas in {} differ from the registry ({}); run `pnpm gen`",
        dir.display(),
        problems.join(", ")
    );
}

// Schema generation.

#[test]
fn registry_writes_an_input_and_an_output_schema_per_type() {
    let names: Vec<String> = fixtures::registry().render().into_keys().collect();
    assert_eq!(
        names,
        [
            "Sample.input.json",
            "Sample.output.json",
            "Step.input.json",
            "Step.output.json"
        ]
    );
}

#[test]
fn rendering_twice_gives_identical_text() {
    assert_eq!(fixtures::registry().render(), fixtures::registry().render());
}

#[test]
fn schema_is_draft_2020_12_and_names_its_type() {
    let schema = schema_file(&fixtures::registry(), "Sample.input.json");
    assert_eq!(
        schema["$schema"],
        "https://json-schema.org/draft/2020-12/schema"
    );
    assert_eq!(schema["title"], "Sample");
}

#[test]
fn input_and_output_schemas_differ_in_required_fields() {
    let registry = fixtures::registry();
    let input = schema_file(&registry, "Sample.input.json");
    let output = schema_file(&registry, "Sample.output.json");
    assert!(
        !input["required"]
            .as_array()
            .unwrap()
            .contains(&json!("labels"))
    );
    assert!(
        output["required"]
            .as_array()
            .unwrap()
            .contains(&json!("labels"))
    );
}

#[test]
fn integer_fields_use_uint_formats() {
    let schema = schema_file(&fixtures::registry(), "Sample.input.json");
    assert_eq!(schema["properties"]["retries"]["format"], "uint32");
    assert_eq!(schema["properties"]["budgetBytes"]["format"], "uint64");
}

#[test]
fn every_fixture_object_sets_additional_properties_false() {
    assert_eq!(open_objects(&fixtures::registry()), Vec::<String>::new());
}

#[test]
fn every_real_object_sets_additional_properties_false() {
    assert_eq!(open_objects(&schema::registry()), Vec::<String>::new());
}

// `Wait {}`.

#[test]
fn wait_variant_schema_has_only_the_tag() {
    let schema = schema_file(&fixtures::registry(), "Step.input.json");
    let wait = schema["oneOf"]
        .as_array()
        .unwrap()
        .iter()
        .find(|variant| variant["properties"]["kind"]["const"] == "wait")
        .expect("no wait variant");
    assert_eq!(wait["properties"].as_object().unwrap().len(), 1);
    assert_eq!(wait["additionalProperties"], false);
}

#[test]
fn wait_variant_is_an_object_on_the_wire() {
    assert_eq!(
        serde_json::to_value(Step::Wait {}).unwrap(),
        json!({ "kind": "wait" })
    );
    assert_eq!(
        serde_json::from_value::<Step>(json!({ "kind": "wait" })).unwrap(),
        Step::Wait {}
    );
}

#[test]
fn wait_variant_rejects_an_unknown_key() {
    assert!(serde_json::from_value::<Step>(json!({ "kind": "wait", "extra": 1 })).is_err());
}

// Unknown keys.

#[test]
fn unknown_key_at_the_top_level_is_rejected() {
    let mut doc = sample_json();
    doc["extra"] = json!(1);
    assert!(serde_json::from_value::<Sample>(doc).is_err());
}

#[test]
fn unknown_key_at_depth_three_is_rejected() {
    let mut doc = sample_json();
    doc["level1"]["level2"]["level3"]["extra"] = json!(1);
    assert!(serde_json::from_value::<Sample>(doc).is_err());
}

#[test]
fn unknown_key_in_a_tagged_variant_is_rejected() {
    let mut doc = sample_json();
    doc["steps"][0]["extra"] = json!(1);
    assert!(serde_json::from_value::<Sample>(doc).is_err());
}

// Round trips.

#[test]
fn sample_round_trips_through_json() {
    let sample: Sample = serde_json::from_value(sample_json()).unwrap();
    assert_eq!(serde_json::to_value(&sample).unwrap(), sample_json());
}

#[test]
fn tagged_enum_round_trips() {
    let step = Step::Agent {
        prompt: "go".into(),
        read_only: true,
    };
    let back: Step = serde_json::from_value(serde_json::to_value(&step).unwrap()).unwrap();
    assert_eq!(back, step);
}

#[test]
fn absent_optional_field_stays_absent() {
    let mut doc = sample_json();
    doc.as_object_mut().unwrap().remove("note");
    let sample: Sample = serde_json::from_value(doc.clone()).unwrap();
    assert_eq!(sample.note, None);
    assert_eq!(serde_json::to_value(&sample).unwrap(), doc);
}

#[test]
fn map_round_trips_and_defaults_to_empty() {
    let mut doc = sample_json();
    doc.as_object_mut().unwrap().remove("labels");
    let sample: Sample = serde_json::from_value(doc).unwrap();
    assert!(sample.labels.is_empty());
    assert_eq!(serde_json::to_value(&sample).unwrap()["labels"], json!({}));
}

// Drift.

#[test]
fn fixture_schemas_match_the_registry() {
    assert_in_sync(
        &fixtures::registry(),
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/schemas"),
    );
}

#[test]
fn committed_schemas_match_the_registry() {
    assert_in_sync(
        &schema::registry(),
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("../schemas"),
    );
}
