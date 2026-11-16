//! Choosing the model (0.3, Who picks what; S7, O8, O9, O11). After `session/new`, the response may list the models the
//! agent offers, in two forms. This module reads them and says what the session must write; the session writes it.
//!
//! - **stable:** `configOptions`, a list. The option with `category: "model"` has an `id`, a `currentValue` and
//!   `options`: a flat list of `{ value, name }`, or groups that each hold such a list. It is set with
//!   `session/set_config_option`;
//! - **draft:** `models`, with `currentModelId` and `availableModels` (`modelId`, `name`). It is set with
//!   `session/set_model`. It is not in the stable schema; S7 read it from a real agent (O9).
//!
//! `configOptions` wins when it has a model option. The session never picks a model the request did not name, and
//! never one the agent did not list: an agent may list its own hosted models beside the local ones (O11), and may keep
//! its own model whatever its config says (O8).

use serde_json::{Value, json};

/// The request that sets the model, as the session writes it.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct ModelRequest {
    pub method: &'static str,
    pub params: Value,
    pub form: ModelForm,
}

/// Which form of the response the model came from, which is also how its answer is checked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ModelForm {
    /// `configOptions`: the answer holds the options as they stand, and the model must be current in them.
    ConfigOption,
    /// The draft `models`: an empty success is the answer.
    Draft,
}

/// What the session does about the model once `session/new` has answered.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum ModelPlan {
    /// Write no model request: the request names no model, or the agent runs it already.
    Keep,
    /// Write this request, and hold the first prompt until it is answered.
    Set(ModelRequest),
    /// The agent cannot be given the model: it lists `offered` models, none of them the one asked for.
    Unavailable { offered: usize },
}

/// What the agent said about its models: where it said it, which one runs, and the ids it offers.
struct Listing {
    form: ModelForm,
    /// The `id` of the model option; empty for the draft form.
    config_id: String,
    current: Option<String>,
    ids: Vec<String>,
}

/// The plan for the model `wanted` (the request's, or `None`), from the `session/new` `result`.
pub(super) fn plan(result: &Value, session_id: &str, wanted: Option<&str>) -> ModelPlan {
    let Some(wanted) = wanted else {
        return ModelPlan::Keep;
    };
    let listing = listing(result);
    if listing.current.as_deref() == Some(wanted) {
        return ModelPlan::Keep;
    }
    if !listing.ids.iter().any(|id| id == wanted) {
        return ModelPlan::Unavailable {
            offered: listing.ids.len(),
        };
    }
    let request = match listing.form {
        ModelForm::ConfigOption => ModelRequest {
            method: "session/set_config_option",
            params: json!({ "sessionId": session_id, "configId": listing.config_id, "value": wanted }),
            form: ModelForm::ConfigOption,
        },
        ModelForm::Draft => ModelRequest {
            method: "session/set_model",
            params: json!({ "sessionId": session_id, "modelId": wanted }),
            form: ModelForm::Draft,
        },
    };
    ModelPlan::Set(request)
}

/// Whether the answer to `session/set_config_option` (its `result`) holds `wanted` as the model option's current
/// value. An answer with no model option does not.
pub(super) fn config_option_holds(result: &Value, wanted: &str) -> bool {
    model_option(result)
        .and_then(|option| option.get("currentValue"))
        .and_then(Value::as_str)
        == Some(wanted)
}

fn listing(result: &Value) -> Listing {
    match model_option(result) {
        Some(option) => Listing {
            form: ModelForm::ConfigOption,
            config_id: text(option.get("id")),
            current: option
                .get("currentValue")
                .and_then(Value::as_str)
                .map(str::to_string),
            ids: option_values(option.get("options")),
        },
        None => draft_listing(result),
    }
}

/// The model option of a `configOptions` list: the first with `category: "model"` and an `id` to set it by.
fn model_option(result: &Value) -> Option<&Value> {
    result
        .get("configOptions")?
        .as_array()?
        .iter()
        .find(|option| {
            option.get("category").and_then(Value::as_str) == Some("model")
                && option.get("id").is_some_and(Value::is_string)
        })
}

/// The values of a flat list of options, or of the options inside each group of a grouped one.
fn option_values(options: Option<&Value>) -> Vec<String> {
    let mut values = Vec::new();
    for entry in options.and_then(Value::as_array).into_iter().flatten() {
        match entry.get("value").and_then(Value::as_str) {
            Some(value) => values.push(value.to_string()),
            None => values.extend(option_values(entry.get("options"))),
        }
    }
    values
}

/// The draft `models` of the response. It is the one place that knows the draft shape (S7, O9), so it is the one
/// thing to delete when every agent a profile names offers `configOptions`.
fn draft_listing(result: &Value) -> Listing {
    let models = result.get("models");
    let ids = models
        .and_then(|models| models.get("availableModels"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|model| model.get("modelId").and_then(Value::as_str))
        .map(str::to_string)
        .collect();
    Listing {
        form: ModelForm::Draft,
        config_id: String::new(),
        current: models
            .and_then(|models| models.get("currentModelId"))
            .and_then(Value::as_str)
            .map(str::to_string),
        ids,
    }
}

fn text(value: Option<&Value>) -> String {
    value
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}
