//! The fake provider: an in-memory [`TaskProvider`] (0.10, Protocol). It answers `list`, `get` and `apply` from the
//! tasks it holds, with the three outcomes of the protocol (`ok`, `conflict`, `refused`), so an engine test reads
//! and writes tasks with no process and no files.
//!
//! What it does, so a test can count on it:
//!
//! - `list` returns every task in the order it was added, in one page. With a `checkout`, a task that names a
//!   checkout is listed for that checkout only, and a task that names none is listed for every checkout;
//! - `get` finds a task by `id` or by one of its `aliases`, and answers `task: None` for a missing id;
//! - `apply` and `preview` run an op the provider declares in `initialize` ([`default_initialize`] declares
//!   `setStatus`, `start`, `finish` and `create`). An op with an `id` and a `status` argument moves the task to that
//!   status, which must be declared; an op that `moves_to` a category moves it to the first status of the category;
//!   either way the task's `version` goes from `v1` to `v2`, and so on. `create`, with a `title` and no `id`,
//!   makes the task `fake-NNNN` in the first status. An unknown op, a missing task or an undeclared status is
//!   `refused` with a message; a `version` that is not the task's current one is a `conflict` that carries the task;
//! - `preview` is what `apply` would answer with nothing written, and is `Unsupported` unless the provider declares
//!   it;
//! - [`FakeProvider::applied`] is every `apply` call, in order, refused ones too.

use crate::change::{Effect, ItemState};
use crate::item::{Category, OpDecl, StatusDecl};
use crate::ports::{ProviderError, TaskProvider};
use crate::provider::{
    ApplyParams, ApplyResult, Capabilities, GetParams, GetResult, InitializeParams,
    InitializeResult, ListParams, ListResult, PreviewResult, Scope, Task, WatchParams, WatchResult,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::Mutex;

/// The `initialize` result the fake gives unless a test replaces it: global scope, four statuses (`todo`, `doing`,
/// `review`, `done`, one for each of four categories), the ops `setStatus`, `start`, `finish` and `create`, and the
/// capabilities `watch`, `create` and `preview`.
pub fn default_initialize() -> InitializeResult {
    let statuses = [
        ("todo", Category::Backlog),
        ("doing", Category::Active),
        ("review", Category::Review),
        ("done", Category::Done),
    ];
    let op = |name: &str, moves_to: Option<Category>, args: Option<Value>| OpDecl {
        name: name.into(),
        moves_to,
        args,
        checkouts: Vec::new(),
    };
    InitializeResult {
        scope: Scope::Global,
        id_pattern: "^fake-[0-9]{4}$".into(),
        short_id_pattern: "^fake-[0-9]{4}$".into(),
        template_fields: Vec::new(),
        statuses: statuses
            .iter()
            .map(|(name, category)| StatusDecl {
                name: (*name).into(),
                category: *category,
            })
            .collect(),
        category_status: statuses
            .iter()
            .map(|(name, category)| (*category, (*name).to_string()))
            .collect(),
        fields: Vec::new(),
        ops: vec![
            op(
                "setStatus",
                None,
                Some(json!({ "type": "object", "properties": { "status": { "type": "string" } } })),
            ),
            op("start", Some(Category::Active), None),
            op("finish", Some(Category::Done), None),
            op(
                "create",
                None,
                Some(json!({ "type": "object", "properties": { "title": { "type": "string" } } })),
            ),
        ],
        capabilities: Capabilities {
            watch: true,
            create: true,
            collections: false,
            preview: true,
        },
        kinds: Vec::new(),
        links: Vec::new(),
    }
}

/// A task with an id, a title and a state, at version `v1`. Its `key` is `fake:` and the id, and every other field
/// is empty, so a test sets the few that it cares about.
pub fn fake_task(id: &str, title: &str, status: &str, category: Category) -> Task {
    Task {
        key: format!("fake:{id}"),
        id: id.into(),
        aliases: Vec::new(),
        title: title.into(),
        body: None,
        state: ItemState {
            status: status.into(),
            category,
        },
        version: "v1".into(),
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
        decisions: Vec::new(),
        assumptions: Vec::new(),
        findings: Vec::new(),
        part_problems: Vec::new(),
        coverage: None,
    }
}

/// `v1` to `v2`, and a version that is not `v` and a number to `v1`.
fn next_version(version: &str) -> String {
    let number = version
        .strip_prefix('v')
        .and_then(|digits| digits.parse::<u64>().ok())
        .unwrap_or(0);
    format!("v{}", number + 1)
}

fn visible_from(task: &Task, checkout: Option<&str>) -> bool {
    match (checkout, task.checkout.as_deref()) {
        (Some(wanted), Some(held)) => wanted == held,
        _ => true,
    }
}

fn files_of(task: &Task) -> Vec<String> {
    vec![
        task.path
            .clone()
            .unwrap_or_else(|| format!("tasks/{}.md", task.id)),
    ]
}

/// What an op would do, before anything is written.
enum Plan {
    /// The task as it will be, whether it is new, and the effect.
    Write {
        task: Task,
        created: bool,
        effect: Effect,
    },
    Conflict(Task),
    Refused(String),
}

struct State {
    init: InitializeResult,
    tasks: Vec<Task>,
    applied: Vec<ApplyParams>,
}

impl State {
    fn plan(&self, params: &ApplyParams) -> Plan {
        let refused = |message: String| Plan::Refused(message);
        let Some(decl) = self.init.ops.iter().find(|op| op.name == params.op) else {
            return refused(format!("the provider declares no op `{}`", params.op));
        };
        let Some(id) = &params.id else {
            return self.plan_create(params);
        };
        let Some(current) = self
            .tasks
            .iter()
            .find(|task| &task.id == id || task.aliases.contains(id))
        else {
            return refused(format!("no task `{id}`"));
        };
        if let Some(version) = &params.version
            && version != &current.version
        {
            return Plan::Conflict(current.clone());
        }
        let state = if let Some(status) = params.args.get("status").and_then(Value::as_str) {
            match self.init.statuses.iter().find(|decl| decl.name == status) {
                Some(decl) => ItemState {
                    status: decl.name.clone(),
                    category: decl.category,
                },
                None => return refused(format!("no status `{status}`")),
            }
        } else if let Some(category) = decl.moves_to {
            match self
                .init
                .statuses
                .iter()
                .find(|decl| decl.category == category)
            {
                Some(decl) => ItemState {
                    status: decl.name.clone(),
                    category,
                },
                None => return refused(format!("no status in category {category:?}")),
            }
        } else {
            current.state.clone()
        };
        let mut task = current.clone();
        task.state = state;
        task.version = next_version(&current.version);
        let effect = Effect {
            key: task.key.clone(),
            version: task.version.clone(),
            from: Some(current.state.clone()),
            to: Some(task.state.clone()),
        };
        Plan::Write {
            task,
            created: false,
            effect,
        }
    }

    /// An op with no `id` makes a task, and only `create` does.
    fn plan_create(&self, params: &ApplyParams) -> Plan {
        if params.op != "create" {
            return Plan::Refused(format!("op `{}` needs an `id`", params.op));
        }
        let Some(title) = params.args.get("title").and_then(Value::as_str) else {
            return Plan::Refused("`create` needs a `title`".into());
        };
        let Some(first) = self.init.statuses.first() else {
            return Plan::Refused("the provider declares no status".into());
        };
        let id = format!("fake-{:04}", self.tasks.len() + 1);
        let task = fake_task(&id, title, &first.name, first.category);
        let effect = Effect {
            key: task.key.clone(),
            version: task.version.clone(),
            from: None,
            to: Some(task.state.clone()),
        };
        Plan::Write {
            task,
            created: true,
            effect,
        }
    }
}

/// An in-memory task source. It takes `&self`, like the trait, and keeps its tasks behind a lock.
pub struct FakeProvider {
    state: Mutex<State>,
}

impl Default for FakeProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl FakeProvider {
    /// A provider with [`default_initialize`] and no tasks.
    pub fn new() -> Self {
        Self {
            state: Mutex::new(State {
                init: default_initialize(),
                tasks: Vec::new(),
                applied: Vec::new(),
            }),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state
            .lock()
            .expect("the fake's lock is never held across a panic")
    }

    /// The same provider with this `initialize` result, which also decides its ops, statuses and capabilities.
    pub fn with_initialize(self, init: InitializeResult) -> Self {
        self.lock().init = init;
        self
    }

    /// The same provider holding one more task, last in the list.
    pub fn with_task(self, task: Task) -> Self {
        self.lock().tasks.push(task);
        self
    }

    /// Every `apply` call so far, in order, refused ones too. `preview` is not one.
    pub fn applied(&self) -> Vec<ApplyParams> {
        self.lock().applied.clone()
    }
}

impl TaskProvider for FakeProvider {
    fn initialize(&self, _params: &InitializeParams) -> Result<InitializeResult, ProviderError> {
        Ok(self.lock().init.clone())
    }

    fn list(&self, params: &ListParams) -> Result<ListResult, ProviderError> {
        let state = self.lock();
        let tasks = state
            .tasks
            .iter()
            .filter(|task| visible_from(task, params.checkout.as_deref()))
            .cloned()
            .collect();
        Ok(ListResult {
            tasks,
            collections: Vec::new(),
            invalid: Vec::new(),
            next_cursor: None,
        })
    }

    fn get(&self, params: &GetParams) -> Result<GetResult, ProviderError> {
        let state = self.lock();
        let task = state
            .tasks
            .iter()
            .filter(|task| visible_from(task, params.checkout.as_deref()))
            .find(|task| task.id == params.id || task.aliases.contains(&params.id))
            .cloned();
        Ok(GetResult { task })
    }

    fn watch(&self, _params: &WatchParams) -> Result<WatchResult, ProviderError> {
        if self.lock().init.capabilities.watch {
            Ok(WatchResult {})
        } else {
            Err(ProviderError::Unsupported {
                method: "watch".into(),
            })
        }
    }

    fn apply(&self, params: &ApplyParams) -> Result<ApplyResult, ProviderError> {
        let mut state = self.lock();
        state.applied.push(params.clone());
        Ok(match state.plan(params) {
            Plan::Refused(message) => ApplyResult::Refused { message },
            Plan::Conflict(current) => ApplyResult::Conflict { current },
            Plan::Write {
                task,
                created,
                effect,
            } => {
                let files = files_of(&task);
                let version = task.version.clone();
                if created {
                    state.tasks.push(task.clone());
                } else if let Some(held) = state.tasks.iter_mut().find(|held| held.id == task.id) {
                    *held = task.clone();
                }
                ApplyResult::Ok {
                    task,
                    version,
                    files,
                    items: vec![effect],
                }
            }
        })
    }

    fn preview(&self, params: &ApplyParams) -> Result<PreviewResult, ProviderError> {
        let state = self.lock();
        if !state.init.capabilities.preview {
            return Err(ProviderError::Unsupported {
                method: "preview".into(),
            });
        }
        Ok(match state.plan(params) {
            Plan::Refused(message) => PreviewResult::Refused { message },
            Plan::Conflict(current) => PreviewResult::Conflict {
                current: Box::new(current),
            },
            Plan::Write { task, effect, .. } => PreviewResult::Ok {
                files: files_of(&task),
                items: vec![effect],
            },
        })
    }
}
