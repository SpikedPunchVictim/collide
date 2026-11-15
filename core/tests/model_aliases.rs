//! Tests for the model aliases (ts-0060, 0.3 Who picks what): `models.yaml` as data, the safe token, and the
//! resolution of an alias for one tool, with `model.id` kept opaque.

use collide_core::agent::{ModelRef, UserModels};
use collide_core::resolve::{ResolveError, is_safe_token, resolve_model_alias};
use collide_core::yaml::load_yaml;
use std::collections::BTreeMap;

fn model(name: &str) -> ModelRef {
    ModelRef {
        name: name.into(),
        id: None,
    }
}

fn mapping(entries: &[(&str, ModelRef)]) -> BTreeMap<String, ModelRef> {
    entries
        .iter()
        .map(|(alias, model)| (alias.to_string(), model.clone()))
        .collect()
}

// --- The safe token ---

#[test]
fn a_safe_token_is_one_to_64_characters_of_letters_digits_dot_underscore_and_dash() {
    for good in [
        "opus",
        "sonnet-4.5",
        "Qwen_2.5",
        "a",
        "v1.0-rc_1",
        &"a".repeat(64),
    ] {
        assert!(is_safe_token(good), "{good}");
    }
}

#[test]
fn a_safe_token_rejects_what_could_climb_out_of_a_directory_or_become_a_flag() {
    let long = "a".repeat(65);
    for bad in [
        "",
        "-x",
        ".x",
        "a..b",
        "..",
        "a/b",
        "ollama/qwen2.5:7b",
        "a b",
        "a:b",
        "a\nb",
        "é",
        &long,
    ] {
        assert!(!is_safe_token(bad), "{bad:?}");
    }
}

// --- Resolution ---

#[test]
fn a_mapped_alias_resolves_to_its_model() {
    let table = mapping(&[("fast", model("haiku")), ("strong", model("opus"))]);
    assert_eq!(
        resolve_model_alias("strong", &table, None),
        Ok(model("opus"))
    );
}

#[test]
fn an_unmapped_alias_is_refused_with_its_name() {
    let table = mapping(&[("fast", model("haiku"))]);
    let error = resolve_model_alias("strong", &table, None).unwrap_err();
    assert_eq!(
        error,
        ResolveError::UnmappedAlias {
            alias: "strong".into()
        }
    );
    assert!(error.to_string().contains("`strong`"), "{error}");
}

#[test]
fn no_mapping_at_all_leaves_every_alias_unmapped() {
    let error = resolve_model_alias("fast", &BTreeMap::new(), None).unwrap_err();
    assert!(matches!(error, ResolveError::UnmappedAlias { .. }));
}

#[test]
fn a_name_that_fails_the_safe_token_is_refused_with_the_alias_and_the_name() {
    let table = mapping(&[("fast", model("../escape"))]);
    let error = resolve_model_alias("fast", &table, None).unwrap_err();
    assert_eq!(
        error,
        ResolveError::UnsafeModelName {
            alias: "fast".into(),
            name: "../escape".into()
        }
    );
    let text = error.to_string();
    assert!(
        text.contains("fast") && text.contains("../escape"),
        "{text}"
    );
}

#[test]
fn a_name_that_fails_a_packs_pattern_is_refused_with_the_pattern() {
    let table = mapping(&[("fast", model("sonnet-4"))]);
    let error = resolve_model_alias("fast", &table, Some("^[a-z]+$")).unwrap_err();
    assert_eq!(
        error,
        ResolveError::ModelNameFailsPattern {
            alias: "fast".into(),
            name: "sonnet-4".into(),
            pattern: "^[a-z]+$".into()
        }
    );
    assert!(error.to_string().contains("^[a-z]+$"), "{error}");
}

#[test]
fn a_name_that_meets_a_packs_pattern_resolves() {
    let table = mapping(&[("fast", model("sonnet"))]);
    assert_eq!(
        resolve_model_alias("fast", &table, Some("^[a-z]+$")),
        Ok(model("sonnet"))
    );
}

#[test]
fn a_pattern_that_is_not_a_regular_expression_is_refused_with_the_pattern() {
    let table = mapping(&[("fast", model("sonnet"))]);
    let error = resolve_model_alias("fast", &table, Some("([a-z")).unwrap_err();
    assert!(
        matches!(&error, ResolveError::InvalidModelNamePattern { pattern, .. } if pattern == "([a-z"),
        "{error:?}"
    );
    assert!(error.to_string().contains("([a-z"), "{error}");
}

#[test]
fn the_safe_token_is_checked_before_the_packs_pattern() {
    // A pattern such as `.*` would let a name with a slash through; the safe token is Collide's own rule.
    let table = mapping(&[("fast", model("a/b"))]);
    let error = resolve_model_alias("fast", &table, Some(".*")).unwrap_err();
    assert!(
        matches!(error, ResolveError::UnsafeModelName { .. }),
        "{error:?}"
    );
}

#[test]
fn a_model_id_passes_through_and_is_not_held_to_the_safe_token_or_the_pattern() {
    let qwen = ModelRef {
        name: "qwen".into(),
        id: Some("ollama/qwen2.5:7b".into()),
    };
    let table = mapping(&[("local", qwen.clone())]);
    assert!(!is_safe_token("ollama/qwen2.5:7b"));
    assert_eq!(
        resolve_model_alias("local", &table, Some("^[a-z]+$")),
        Ok(qwen)
    );
}

#[test]
fn a_refusal_never_repeats_the_model_id() {
    let id = "ollama/qwen2.5:7b";
    let bad_name = ModelRef {
        name: "Qwen 2".into(),
        id: Some(id.into()),
    };
    let wrong_shape = ModelRef {
        name: "qwen2".into(),
        id: Some(id.into()),
    };
    let table = mapping(&[("a", bad_name), ("b", wrong_shape)]);
    for (alias, pattern) in [("a", None), ("b", Some("^[a-z]+$"))] {
        let error = resolve_model_alias(alias, &table, pattern).unwrap_err();
        assert!(!format!("{error} {error:?}").contains(id), "{error}");
    }
}

// --- models.yaml as data ---

fn read(text: &str) -> UserModels {
    load_yaml(text.as_bytes(), "models.yaml").unwrap()
}

#[test]
fn models_yaml_maps_aliases_per_tool() {
    let models = read(
        "tools:\n  claude:\n    fast: { name: haiku }\n    strong: { name: opus }\n  opencode:\n    fast:\n      name: qwen\n      id: ollama/qwen2.5:7b\n",
    );
    assert_eq!(
        models.mapping_for("claude"),
        mapping(&[("fast", model("haiku")), ("strong", model("opus"))])
    );
    assert_eq!(
        models.mapping_for("opencode")["fast"].id.as_deref(),
        Some("ollama/qwen2.5:7b")
    );
}

#[test]
fn an_alias_never_switches_the_tool() {
    let models = read("tools:\n  claude:\n    fast: { name: haiku }\n");
    assert!(models.mapping_for("opencode").is_empty());
    let error = resolve_model_alias("fast", &models.mapping_for("opencode"), None).unwrap_err();
    assert!(matches!(error, ResolveError::UnmappedAlias { .. }));
}

#[test]
fn an_empty_models_file_maps_nothing() {
    assert_eq!(UserModels::default().mapping_for("claude"), BTreeMap::new());
    assert_eq!(read("tools: {}\n"), UserModels::default());
}

#[test]
fn models_yaml_refuses_a_key_it_does_not_know_and_names_it() {
    for text in [
        "tools:\n  claude:\n    fast: { name: haiku, source: bedrock }\n",
        "tool:\n  claude: {}\n",
    ] {
        let error = load_yaml::<UserModels>(text.as_bytes(), "models.yaml")
            .unwrap_err()
            .to_string();
        assert!(error.contains("models.yaml"), "{error}");
    }
}
