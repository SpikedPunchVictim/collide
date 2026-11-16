//! Tests for the registry of Collide's own settings (ts-0093): the keys with their types, ranges and defaults, the
//! keys of each flag as data, and the tie between the registry and the keys `ConfigFile` can hold.

use collide_core::schema;
use collide_core::settings::{ConfigFile, MAX_TIMEOUT_S, MIN_TIMEOUT_S};
use collide_core::settings_registry::{
    COLLIDE_SETTINGS, CollideSetting, SettingDefault, ValueType, collide_setting, overridable_keys,
    repo_file_keys,
};
use serde_json::{Value, json};
use std::collections::BTreeSet;

fn keys_where(flag: impl Fn(&CollideSetting) -> bool) -> Vec<&'static str> {
    COLLIDE_SETTINGS
        .iter()
        .filter(|setting| flag(setting))
        .map(|setting| setting.key)
        .collect()
}

fn schema_file(name: &str) -> Value {
    serde_json::from_str(&schema::registry().render()[name]).unwrap()
}

#[test]
fn the_registry_holds_the_five_keys_in_order() {
    let keys: Vec<&str> = COLLIDE_SETTINGS.iter().map(|s| s.key).collect();
    assert_eq!(
        keys,
        [
            "ask.agent",
            "ask.timeout_s",
            "ask.budget_usd",
            "disk.floor_gib",
            "claude.check_published"
        ]
    );
}

#[test]
fn each_key_has_its_type_range_and_default() {
    let table: Vec<_> = COLLIDE_SETTINGS
        .iter()
        .map(|s| (s.key, s.value_type, s.min, s.max, s.default))
        .collect();
    assert_eq!(
        table,
        [
            ("ask.agent", ValueType::String, None, None, None),
            (
                "ask.timeout_s",
                ValueType::Integer,
                Some(5),
                Some(1800),
                None
            ),
            ("ask.budget_usd", ValueType::Number, None, None, None),
            (
                "disk.floor_gib",
                ValueType::Integer,
                Some(1),
                None,
                Some(SettingDefault::Integer(5))
            ),
            (
                "claude.check_published",
                ValueType::Boolean,
                None,
                None,
                Some(SettingDefault::Boolean(true))
            ),
        ]
    );
}

#[test]
fn a_default_is_written_as_the_json_value_a_file_would_hold() {
    let defaults: Vec<_> = COLLIDE_SETTINGS
        .iter()
        .map(|s| (s.key, s.default_value()))
        .filter(|(_, value)| value.is_some())
        .collect();
    assert_eq!(
        defaults,
        [
            ("disk.floor_gib", Some(json!(5))),
            ("claude.check_published", Some(json!(true)))
        ]
    );
}

#[test]
fn a_value_type_is_named_as_json_schema_names_it() {
    let names: Vec<_> = COLLIDE_SETTINGS
        .iter()
        .map(|s| s.value_type.as_str())
        .collect();
    assert_eq!(names, ["string", "integer", "number", "integer", "boolean"]);
}

#[test]
fn the_keys_of_each_flag_are_listed_so_a_change_is_deliberate() {
    assert_eq!(
        keys_where(|s| s.overridable),
        ["ask.timeout_s", "disk.floor_gib"]
    );
    assert_eq!(
        keys_where(|s| s.per_user),
        ["ask.agent", "claude.check_published"]
    );
    assert_eq!(keys_where(|s| s.repo_only), Vec::<&str>::new());
}

#[test]
fn the_helpers_read_the_flags() {
    assert_eq!(overridable_keys(), ["ask.timeout_s", "disk.floor_gib"]);
    assert_eq!(
        repo_file_keys(),
        ["ask.timeout_s", "ask.budget_usd", "disk.floor_gib"]
    );
}

#[test]
fn a_key_is_found_by_its_path_and_an_unknown_one_is_not() {
    assert_eq!(
        collide_setting("disk.floor_gib").unwrap().key,
        "disk.floor_gib"
    );
    assert!(collide_setting("disk.floor").is_none());
    assert!(collide_setting("lanePool").is_none());
}

#[test]
fn no_key_contradicts_itself() {
    for setting in COLLIDE_SETTINGS {
        let key = setting.key;
        assert!(
            !(setting.repo_only && (setting.overridable || setting.per_user)),
            "{key}: a key the repo owns cannot be the user's"
        );
        assert!(
            !(setting.per_user && setting.overridable),
            "{key}: a per-user key has no repo value to override"
        );
        assert!(key.split('.').count() == 2, "{key} is `section.key`");
        if let Some(default) = setting.default {
            let fits = matches!(
                (default, setting.value_type),
                (SettingDefault::Boolean(_), ValueType::Boolean)
                    | (SettingDefault::Integer(_), ValueType::Integer)
            );
            assert!(fits, "{key}: the default is not of the key's type");
            if let SettingDefault::Integer(value) = default {
                assert!(setting.min.is_none_or(|min| value >= min), "{key}");
                assert!(setting.max.is_none_or(|max| value <= max), "{key}");
            }
        }
    }
}

#[test]
fn keys_are_unique() {
    let keys: BTreeSet<&str> = COLLIDE_SETTINGS.iter().map(|s| s.key).collect();
    assert_eq!(keys.len(), COLLIDE_SETTINGS.len());
}

#[test]
fn the_timeout_range_is_the_one_the_file_check_uses() {
    let timeout = collide_setting("ask.timeout_s").unwrap();
    assert_eq!(timeout.min, Some(MIN_TIMEOUT_S as i64));
    assert_eq!(timeout.max, Some(MAX_TIMEOUT_S as i64));
}

/// The `section.key` paths of `ConfigFile` that Collide owns, read from its generated schema: every property that
/// is a section (`ask`, `disk`, `claude`) and not `agents` or `config`, which are other contracts' (`agents.<id>.*`
/// is `ts-0099-bw`, and `config` holds pack keys).
fn config_file_keys() -> BTreeSet<String> {
    let schema = schema_file("ConfigFile.input.json");
    let mut keys = BTreeSet::new();
    for (section, property) in schema["properties"].as_object().unwrap() {
        if section == "agents" || section == "config" {
            continue;
        }
        let reference = property["$ref"]
            .as_str()
            .unwrap_or_else(|| panic!("{section}"));
        let name = reference.rsplit('/').next().unwrap();
        for key in schema["$defs"][name]["properties"]
            .as_object()
            .unwrap()
            .keys()
        {
            keys.insert(format!("{section}.{key}"));
        }
    }
    keys
}

#[test]
fn every_key_collide_owns_in_config_file_is_in_the_registry_and_the_other_way() {
    let registry: BTreeSet<String> = COLLIDE_SETTINGS.iter().map(|s| s.key.to_string()).collect();
    assert_eq!(config_file_keys(), registry);
}

#[test]
fn a_file_that_sets_every_key_reports_exactly_the_registry_keys() {
    let file: ConfigFile = collide_core::yaml::load_yaml(
        b"ask: { agent: a, timeout_s: 60, budget_usd: 1.5 }\n\
          disk: { floor_gib: 8 }\n\
          claude: { check_published: false }\n",
        "test.yaml",
    )
    .unwrap();
    let mut set: Vec<&str> = file.collide_keys_set();
    set.sort_unstable();
    let mut registry: Vec<&str> = COLLIDE_SETTINGS.iter().map(|s| s.key).collect();
    registry.sort_unstable();
    assert_eq!(set, registry);
}

#[test]
fn the_registry_ranges_are_in_the_generated_schema() {
    let schema = schema_file("ConfigFile.input.json");
    let property = |section: &str, key: &str| {
        let name = schema["properties"][section]["$ref"]
            .as_str()
            .unwrap()
            .rsplit('/')
            .next()
            .unwrap();
        schema["$defs"][name]["properties"][key].clone()
    };
    for setting in COLLIDE_SETTINGS {
        let (section, key) = setting.key.split_once('.').unwrap();
        let property = property(section, key);
        assert_eq!(
            property.get("minimum").and_then(Value::as_i64),
            setting.min,
            "{}",
            setting.key
        );
        assert_eq!(
            property.get("maximum").and_then(Value::as_i64),
            setting.max,
            "{}",
            setting.key
        );
    }
}
