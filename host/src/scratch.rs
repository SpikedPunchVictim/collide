//! The scratch source (ts-0075-hj; 0.1, 0.10): the tasks a person types on a board that no source tracks. They live in
//! the store, one list for each board, and are never written into the repository (0.1). Their statuses are a list of
//! [`StatusDecl`], the shape a provider declares in `initialize` ([`statuses`]); no other code matches on a status
//! name. Their two ops are `create` and `setStatus`.
//!
//! [`ScratchSource`] is a [`TaskProvider`] over the store, for one board, so the daemon calls it as it will call the
//! providers the supervisor runs. `get`, `list` and `apply` are real, and `initialize` says what the source is. `watch`
//! and `preview` answer [`ProviderError::Unsupported`]: nothing outside the daemon writes a scratch task, so there is
//! nothing to watch, and the capabilities say `preview` is not declared. Stage 4b finishes this type behind the
//! supervisor.
//!
//! A key is `scratch:<n>` and its long id is `<n>`, a number counted inside the board. The version is the count of
//! writes to the task, as text, from 1.

use crate::scratch_store::ScratchRow;
use crate::store::{Store, StoreError};
use collide_core::change::{Effect, FieldDiff, ItemState};
use collide_core::events::EventBody;
use collide_core::item::{Category, OpDecl, StatusDecl};
use collide_core::ports::{ProviderError, TaskProvider};
use collide_core::provider::{
    ApplyParams, ApplyResult, Capabilities, ChangeCause, GetParams, GetResult, InitializeParams,
    InitializeResult, ListParams, ListResult, PreviewResult, Scope, Task, TaskChanged, WatchParams,
    WatchResult,
};
use serde_json::json;
use std::collections::BTreeMap;
use std::sync::Arc;

/// The name of this source: the part of a key before the colon.
pub const SOURCE: &str = "scratch";

/// The statuses of a scratch task, in order, each with its category. Today each is named for the category it maps to,
/// and `backlog` is not one: a prompt typed on a board is ready to run. The first is the status a new task has. The
/// store keeps the name, and the category always comes from this list.
pub fn statuses() -> Vec<StatusDecl> {
    [
        ("ready", Category::Ready),
        ("active", Category::Active),
        ("review", Category::Review),
        ("done", Category::Done),
        ("dropped", Category::Dropped),
    ]
    .into_iter()
    .map(|(name, category)| StatusDecl {
        name: name.to_string(),
        category,
    })
    .collect()
}

/// The state of the status called `name` in `statuses`, or `None` when the list has no such status.
fn state_in(statuses: &[StatusDecl], name: &str) -> Option<ItemState> {
    statuses
        .iter()
        .find(|decl| decl.name == name)
        .map(|decl| ItemState {
            status: decl.name.clone(),
            category: decl.category,
        })
}

/// The status each category writes, from the declared list: the first status declared with that category. This is what
/// `initialize` reports as `categoryStatus`, and what `setStatus` by category looks up.
fn category_status(statuses: &[StatusDecl]) -> BTreeMap<Category, String> {
    statuses.iter().fold(BTreeMap::new(), |mut map, decl| {
        map.entry(decl.category)
            .or_insert_with(|| decl.name.clone());
        map
    })
}

/// A category as the wire writes it, such as `active`.
fn category_name(category: Category) -> String {
    json!(category).as_str().unwrap_or_default().to_string()
}

/// The names of `statuses` as a list for a message, such as `ready, active, review`.
fn status_words(statuses: &[StatusDecl]) -> String {
    statuses
        .iter()
        .map(|decl| decl.name.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

/// The title of a prompt: its first line that has any text, with the spaces around it taken off. `None` for a prompt
/// that has no text at all.
pub fn title_of(prompt: &str) -> Option<&str> {
    prompt.lines().map(str::trim).find(|line| !line.is_empty())
}

/// An id as a client gives it, split at the first colon: `scratch:3` is `(Some("scratch"), "3")` and `3` is
/// `(None, "3")`.
pub fn split_id(id: &str) -> (Option<&str>, &str) {
    match id.split_once(':') {
        Some((source, bare)) => (Some(source), bare),
        None => (None, id),
    }
}

/// The `task.changed` event of one write: the effect's key, version and states, and the status as the one field that
/// changed. The actor is the event's, set by the daemon from the connection; the source names none of its own. The
/// cause is `journal`, the closest of the five: the change is in the daemon's own record and no checkout, remote or
/// startup read made it.
pub fn changed_event(effect: &Effect) -> EventBody {
    let status = |state: &Option<ItemState>| match state {
        Some(state) => json!(state.status),
        None => serde_json::Value::Null,
    };
    EventBody::TaskChanged {
        source: SOURCE.to_string(),
        task: TaskChanged {
            key: effect.key.clone(),
            version: effect.version.clone(),
            from: effect.from.clone(),
            to: effect.to.clone(),
            diff: BTreeMap::from([(
                "status".to_string(),
                FieldDiff::Changed {
                    from: status(&effect.from),
                    to: status(&effect.to),
                },
            )]),
            cause: ChangeCause::Journal,
            actor: None,
            automated: None,
            checkout: None,
        },
    }
}

fn task_of(
    statuses: &[StatusDecl],
    row: &ScratchRow,
    with_body: bool,
) -> Result<Task, ProviderError> {
    let state = state_in(statuses, &row.status).ok_or_else(|| ProviderError::Failed {
        message: format!(
            "scratch task {} has the status {:?}, and the source's statuses are {}",
            row.num,
            row.status,
            status_words(statuses)
        ),
    })?;
    Ok(Task {
        key: format!("{SOURCE}:{}", row.num),
        id: row.num.to_string(),
        aliases: Vec::new(),
        title: row.title.clone(),
        body: with_body.then(|| row.prompt.clone()),
        state,
        version: row.version.to_string(),
        checkout: None,
        path: None,
        kind: None,
        links: Vec::new(),
        owners: Vec::new(),
        checklist: None,
        labels: Vec::new(),
        collections: Vec::new(),
        template_fields: BTreeMap::new(),
        fields: BTreeMap::new(),
        features: Vec::new(),
        feature_problems: Vec::new(),
    })
}

fn failed(error: StoreError) -> ProviderError {
    ProviderError::Failed {
        message: error.to_string(),
    }
}

fn refused(message: impl Into<String>) -> Result<ApplyResult, ProviderError> {
    Ok(ApplyResult::Refused {
        message: message.into(),
    })
}

/// The scratch tasks of one board.
pub struct ScratchSource {
    store: Arc<Store>,
    board: String,
    statuses: Vec<StatusDecl>,
}

impl ScratchSource {
    pub fn new(store: Arc<Store>, board: impl Into<String>) -> ScratchSource {
        ScratchSource {
            store,
            board: board.into(),
            statuses: statuses(),
        }
    }

    fn create(&self, params: &ApplyParams) -> Result<ApplyResult, ProviderError> {
        let Some(prompt) = params.args.get("prompt").and_then(|value| value.as_str()) else {
            return refused("`create` needs a `prompt`, which is text");
        };
        let Some(title) = title_of(prompt) else {
            return refused("`create` needs a `prompt` with some text in it");
        };
        let Some(first) = self.statuses.first() else {
            return Err(ProviderError::Failed {
                message: "the scratch source declares no statuses".to_string(),
            });
        };
        let row = self
            .store
            .add_scratch_task(&self.board, title, prompt, &first.name)
            .map_err(failed)?;
        let task = task_of(&self.statuses, &row, true)?;
        Ok(ApplyResult::Ok {
            version: task.version.clone(),
            files: Vec::new(),
            items: vec![Effect {
                key: task.key.clone(),
                version: task.version.clone(),
                from: None,
                to: Some(task.state.clone()),
            }],
            task,
        })
    }

    fn set_status(&self, params: &ApplyParams) -> Result<ApplyResult, ProviderError> {
        let Some(id) = params.id.as_deref() else {
            return refused("`setStatus` needs the id of the task");
        };
        let status = match self.target_status(params) {
            Ok(status) => status,
            Err(message) => return refused(message),
        };
        let status = status.as_str();
        let Some(row) = self.row(id)? else {
            return refused(format!("this board has no scratch task `{id}`"));
        };
        let current = task_of(&self.statuses, &row, true)?;
        if params
            .version
            .as_deref()
            .is_some_and(|version| version != current.version)
        {
            return Ok(ApplyResult::Conflict { current });
        }
        if row.status == status {
            return Ok(ApplyResult::Ok {
                version: current.version.clone(),
                files: Vec::new(),
                items: Vec::new(),
                task: current,
            });
        }
        let Some(changed) = self
            .store
            .set_scratch_status(&self.board, row.num, status)
            .map_err(failed)?
        else {
            return refused(format!("this board has no scratch task `{id}`"));
        };
        let task = task_of(&self.statuses, &changed, true)?;
        Ok(ApplyResult::Ok {
            version: task.version.clone(),
            files: Vec::new(),
            items: vec![Effect {
                key: task.key.clone(),
                version: task.version.clone(),
                from: Some(current.state),
                to: Some(task.state.clone()),
            }],
            task,
        })
    }

    /// The status `setStatus` writes: the one named by `status`, or the one the list declares for `category`. Exactly
    /// one of the two args is given. The error is the refusal's message.
    fn target_status(&self, params: &ApplyParams) -> Result<String, String> {
        let statuses = status_words(&self.statuses);
        let by_category = category_status(&self.statuses);
        let categories = by_category
            .keys()
            .map(|category| category_name(*category))
            .collect::<Vec<_>>()
            .join(", ");
        match (params.args.get("status"), params.args.get("category")) {
            (Some(status), None) => {
                let name = status.as_str().unwrap_or_default();
                if state_in(&self.statuses, name).is_none() {
                    return Err(format!(
                        "`{name}` is not a status of a scratch task; the statuses are {statuses}"
                    ));
                }
                Ok(name.to_string())
            }
            (None, Some(category)) => serde_json::from_value::<Category>(category.clone())
                .ok()
                .and_then(|category| by_category.get(&category))
                .cloned()
                .ok_or_else(|| {
                    format!(
                        "{category} is not a category a scratch task has a status for; \
                         the categories are {categories}"
                    )
                }),
            _ => Err(format!(
                "`setStatus` takes one of `status` ({statuses}) or `category` ({categories}), and not both"
            )),
        }
    }

    /// The task a long id names on this board. An id that is not a number names no task.
    fn row(&self, id: &str) -> Result<Option<ScratchRow>, ProviderError> {
        match id.parse::<u64>() {
            Ok(num) => self.store.scratch_task(&self.board, num).map_err(failed),
            Err(_) => Ok(None),
        }
    }
}

impl TaskProvider for ScratchSource {
    fn initialize(&self, _params: &InitializeParams) -> Result<InitializeResult, ProviderError> {
        Ok(InitializeResult {
            scope: Scope::Global,
            id_pattern: "^[1-9][0-9]*$".to_string(),
            short_id_pattern: "^[1-9][0-9]*$".to_string(),
            template_fields: Vec::new(),
            statuses: self.statuses.clone(),
            category_status: category_status(&self.statuses),
            fields: Vec::new(),
            ops: vec![
                OpDecl {
                    name: "create".to_string(),
                    moves_to: self.statuses.first().map(|decl| decl.category),
                    args: Some(json!({
                        "type": "object",
                        "properties": { "prompt": { "type": "string" } },
                        "required": ["prompt"],
                        "additionalProperties": false
                    })),
                    checkouts: Vec::new(),
                },
                OpDecl {
                    name: "setStatus".to_string(),
                    moves_to: None,
                    args: Some(json!({
                        "type": "object",
                        "properties": {
                            "status": { "enum": self.statuses.iter().map(|decl| &decl.name).collect::<Vec<_>>() },
                            "category": { "enum": category_status(&self.statuses).keys().collect::<Vec<_>>() }
                        },
                        "description": "give exactly one of status or category",
                        "oneOf": [{ "required": ["status"] }, { "required": ["category"] }],
                        "additionalProperties": false
                    })),
                    checkouts: Vec::new(),
                },
            ],
            capabilities: Capabilities {
                watch: false,
                create: true,
                collections: false,
                preview: false,
            },
            kinds: Vec::new(),
            links: Vec::new(),
        })
    }

    fn list(&self, _params: &ListParams) -> Result<ListResult, ProviderError> {
        let rows = self.store.scratch_tasks(&self.board).map_err(failed)?;
        Ok(ListResult {
            tasks: rows
                .iter()
                .map(|row| task_of(&self.statuses, row, false))
                .collect::<Result<_, _>>()?,
            collections: Vec::new(),
            invalid: Vec::new(),
            next_cursor: None,
        })
    }

    fn get(&self, params: &GetParams) -> Result<GetResult, ProviderError> {
        let task = self
            .row(&params.id)?
            .map(|row| task_of(&self.statuses, &row, true))
            .transpose()?;
        Ok(GetResult { task })
    }

    fn watch(&self, _params: &WatchParams) -> Result<WatchResult, ProviderError> {
        Err(ProviderError::Unsupported {
            method: "watch".to_string(),
        })
    }

    fn apply(&self, params: &ApplyParams) -> Result<ApplyResult, ProviderError> {
        match params.op.as_str() {
            "create" => self.create(params),
            "setStatus" => self.set_status(params),
            other => refused(format!(
                "scratch tasks have the ops `create` and `setStatus`, and no op `{other}`"
            )),
        }
    }

    fn preview(&self, _params: &ApplyParams) -> Result<PreviewResult, ProviderError> {
        Err(ProviderError::Unsupported {
            method: "preview".to_string(),
        })
    }
}
