//! The fake provider (ts-0022-mc): an in-memory `TaskProvider`. It answers `list`, `get` and `apply` from the tasks
//! it holds, with the three outcomes of the provider protocol (0.10), so an engine test reads and writes tasks with
//! no process and no files.

use collide_core::change::ItemState;
use collide_core::item::Category;
use collide_core::ports::{ProviderError, TaskProvider};
use collide_core::provider::{
    ApplyParams, ApplyResult, GetParams, InitializeParams, ListParams, PreviewResult, WatchParams,
};
use collide_core::testing::{FakeProvider, default_initialize, fake_task};
use serde_json::{Value, json};
use std::collections::BTreeMap;

fn provider() -> FakeProvider {
    FakeProvider::new()
        .with_task(fake_task("fake-0001", "First", "todo", Category::Backlog))
        .with_task(fake_task("fake-0002", "Second", "doing", Category::Active))
}

fn get(id: &str) -> GetParams {
    GetParams {
        id: id.into(),
        checkout: None,
    }
}

fn op(name: &str, id: Option<&str>, args: &[(&str, Value)]) -> ApplyParams {
    ApplyParams {
        op: name.into(),
        id: id.map(str::to_string),
        args: args
            .iter()
            .map(|(key, value)| (key.to_string(), value.clone()))
            .collect::<BTreeMap<_, _>>(),
        version: None,
        checkout: None,
    }
}

fn state(status: &str, category: Category) -> ItemState {
    ItemState {
        status: status.into(),
        category,
    }
}

// --- initialize, list, get ---

#[test]
fn initialize_returns_the_declaration_the_provider_was_given() {
    let mut init = default_initialize();
    init.id_pattern = "^x-[0-9]+$".into();
    let provider = FakeProvider::new().with_initialize(init.clone());
    let params = InitializeParams {
        config: BTreeMap::new(),
        root: "/repo".into(),
    };
    assert_eq!(provider.initialize(&params).unwrap(), init);
}

#[test]
fn the_default_declaration_names_statuses_in_categories_and_ops_and_declares_create_and_preview() {
    let init = default_initialize();
    assert!(
        init.statuses
            .iter()
            .any(|s| s.name == "todo" && s.category == Category::Backlog)
    );
    assert!(init.ops.iter().any(|o| o.name == "setStatus"));
    assert!(init.capabilities.create && init.capabilities.preview);
}

#[test]
fn list_returns_every_task_in_the_order_they_were_added() {
    let listed = provider()
        .list(&ListParams {
            checkout: None,
            cursor: None,
        })
        .unwrap();
    let ids: Vec<&str> = listed.tasks.iter().map(|t| t.id.as_str()).collect();
    assert_eq!(ids, vec!["fake-0001", "fake-0002"]);
    assert_eq!(listed.next_cursor, None);
}

#[test]
fn list_for_a_checkout_hides_the_tasks_of_other_checkouts_and_shows_the_ones_with_none() {
    let mut on_branch = fake_task("fake-0003", "On a branch", "todo", Category::Backlog);
    on_branch.checkout = Some("feature".into());
    let provider = provider().with_task(on_branch);
    let ids = |checkout: &str| -> Vec<String> {
        provider
            .list(&ListParams {
                checkout: Some(checkout.into()),
                cursor: None,
            })
            .unwrap()
            .tasks
            .into_iter()
            .map(|task| task.id)
            .collect()
    };
    assert_eq!(ids("feature"), vec!["fake-0001", "fake-0002", "fake-0003"]);
    assert_eq!(ids("main"), vec!["fake-0001", "fake-0002"]);
}

#[test]
fn get_finds_a_task_by_its_id() {
    let found = provider().get(&get("fake-0002")).unwrap();
    assert_eq!(found.task.unwrap().title, "Second");
}

#[test]
fn get_finds_a_task_by_one_of_its_aliases() {
    let mut aliased = fake_task("fake-0003", "Aliased", "todo", Category::Backlog);
    aliased.aliases = vec!["old-3".into()];
    let provider = provider().with_task(aliased);
    assert_eq!(
        provider.get(&get("old-3")).unwrap().task.unwrap().id,
        "fake-0003"
    );
}

#[test]
fn get_of_a_missing_id_is_an_answer_with_no_task_and_not_an_error() {
    assert_eq!(provider().get(&get("fake-9999")).unwrap().task, None);
}

#[test]
fn watch_is_answered_when_declared_and_unsupported_when_not() {
    let params = WatchParams {
        checkouts: Vec::new(),
    };
    assert!(provider().watch(&params).is_ok());
    let mut init = default_initialize();
    init.capabilities.watch = false;
    let error = FakeProvider::new()
        .with_initialize(init)
        .watch(&params)
        .unwrap_err();
    assert_eq!(
        error,
        ProviderError::Unsupported {
            method: "watch".into()
        }
    );
}

// --- apply ---

#[test]
fn apply_a_status_change_moves_the_task_bumps_its_version_and_reports_the_effect() {
    let provider = provider();
    let result = provider
        .apply(&op(
            "setStatus",
            Some("fake-0001"),
            &[("status", json!("doing"))],
        ))
        .unwrap();
    let ApplyResult::Ok {
        task,
        version,
        files,
        items,
    } = result
    else {
        panic!("expected ok, got {result:?}");
    };
    assert_eq!(task.state, state("doing", Category::Active));
    assert_eq!(version, "v2");
    assert_eq!(task.version, "v2");
    assert_eq!(files, vec!["tasks/fake-0001.md"]);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].key, "fake:fake-0001");
    assert_eq!(items[0].from, Some(state("todo", Category::Backlog)));
    assert_eq!(items[0].to, Some(state("doing", Category::Active)));
}

#[test]
fn apply_is_kept_so_a_later_get_and_list_see_it() {
    let provider = provider();
    provider
        .apply(&op(
            "setStatus",
            Some("fake-0001"),
            &[("status", json!("done"))],
        ))
        .unwrap();
    let got = provider.get(&get("fake-0001")).unwrap().task.unwrap();
    assert_eq!(got.state, state("done", Category::Done));
    assert_eq!(got.version, "v2");
}

#[test]
fn an_op_that_moves_to_a_category_picks_the_first_status_of_that_category() {
    let provider = provider();
    let result = provider
        .apply(&op("finish", Some("fake-0002"), &[]))
        .unwrap();
    let ApplyResult::Ok { task, .. } = result else {
        panic!("expected ok")
    };
    assert_eq!(task.state, state("done", Category::Done));
}

#[test]
fn apply_with_the_version_the_caller_read_succeeds() {
    let provider = provider();
    let mut params = op(
        "setStatus",
        Some("fake-0001"),
        &[("status", json!("doing"))],
    );
    params.version = Some("v1".into());
    assert!(matches!(
        provider.apply(&params).unwrap(),
        ApplyResult::Ok { .. }
    ));
}

#[test]
fn apply_with_a_stale_version_is_a_conflict_that_carries_the_current_task_and_writes_nothing() {
    let provider = provider();
    provider
        .apply(&op(
            "setStatus",
            Some("fake-0001"),
            &[("status", json!("doing"))],
        ))
        .unwrap();
    let mut stale = op("setStatus", Some("fake-0001"), &[("status", json!("done"))]);
    stale.version = Some("v1".into());
    let ApplyResult::Conflict { current } = provider.apply(&stale).unwrap() else {
        panic!("expected a conflict");
    };
    assert_eq!(current.version, "v2");
    assert_eq!(current.state, state("doing", Category::Active));
    let kept = provider.get(&get("fake-0001")).unwrap().task.unwrap();
    assert_eq!(kept.state, state("doing", Category::Active));
}

#[test]
fn apply_of_an_op_the_provider_does_not_declare_is_refused_with_a_message() {
    let result = provider()
        .apply(&op("explode", Some("fake-0001"), &[]))
        .unwrap();
    assert_eq!(
        result,
        ApplyResult::Refused {
            message: "the provider declares no op `explode`".into()
        }
    );
}

#[test]
fn apply_of_a_status_the_provider_does_not_declare_is_refused() {
    let result = provider()
        .apply(&op(
            "setStatus",
            Some("fake-0001"),
            &[("status", json!("limbo"))],
        ))
        .unwrap();
    assert_eq!(
        result,
        ApplyResult::Refused {
            message: "no status `limbo`".into()
        }
    );
}

#[test]
fn apply_to_a_missing_task_is_refused() {
    let result = provider()
        .apply(&op(
            "setStatus",
            Some("fake-9999"),
            &[("status", json!("done"))],
        ))
        .unwrap();
    assert_eq!(
        result,
        ApplyResult::Refused {
            message: "no task `fake-9999`".into()
        }
    );
}

#[test]
fn apply_create_makes_a_task_in_the_first_status_with_the_next_id() {
    let provider = provider();
    let result = provider
        .apply(&op("create", None, &[("title", json!("Third"))]))
        .unwrap();
    let ApplyResult::Ok {
        task, items, files, ..
    } = result
    else {
        panic!("expected ok")
    };
    assert_eq!(task.id, "fake-0003");
    assert_eq!(task.title, "Third");
    assert_eq!(task.state, state("todo", Category::Backlog));
    assert_eq!(items[0].from, None);
    assert_eq!(items[0].to, Some(state("todo", Category::Backlog)));
    assert_eq!(files, vec!["tasks/fake-0003.md"]);
    assert_eq!(
        provider.get(&get("fake-0003")).unwrap().task.unwrap().title,
        "Third"
    );
}

#[test]
fn apply_create_without_a_title_is_refused() {
    let result = provider().apply(&op("create", None, &[])).unwrap();
    assert_eq!(
        result,
        ApplyResult::Refused {
            message: "`create` needs a `title`".into()
        }
    );
}

#[test]
fn apply_calls_are_recorded_in_order() {
    let provider = provider();
    provider
        .apply(&op("finish", Some("fake-0002"), &[]))
        .unwrap();
    provider.apply(&op("explode", None, &[])).unwrap();
    let ops: Vec<String> = provider.applied().into_iter().map(|p| p.op).collect();
    assert_eq!(ops, vec!["finish", "explode"]);
}

// --- preview ---

#[test]
fn preview_returns_the_effects_apply_would_and_writes_nothing() {
    let provider = provider();
    let params = op(
        "setStatus",
        Some("fake-0001"),
        &[("status", json!("doing"))],
    );
    let PreviewResult::Ok { items, files } = provider.preview(&params).unwrap() else {
        panic!("expected ok");
    };
    assert_eq!(items[0].to, Some(state("doing", Category::Active)));
    assert_eq!(files, vec!["tasks/fake-0001.md"]);
    let kept = provider.get(&get("fake-0001")).unwrap().task.unwrap();
    assert_eq!(kept.state, state("todo", Category::Backlog));
    assert!(provider.applied().is_empty());
}

#[test]
fn preview_of_a_stale_version_is_a_conflict_and_of_a_bad_op_is_refused() {
    let provider = provider();
    let mut stale = op(
        "setStatus",
        Some("fake-0001"),
        &[("status", json!("doing"))],
    );
    stale.version = Some("v0".into());
    assert!(matches!(
        provider.preview(&stale).unwrap(),
        PreviewResult::Conflict { .. }
    ));
    assert!(matches!(
        provider
            .preview(&op("explode", Some("fake-0001"), &[]))
            .unwrap(),
        PreviewResult::Refused { .. }
    ));
}

#[test]
fn preview_is_unsupported_when_the_provider_does_not_declare_it() {
    let mut init = default_initialize();
    init.capabilities.preview = false;
    let provider = FakeProvider::new().with_initialize(init);
    let error = provider
        .preview(&op("finish", Some("fake-0001"), &[]))
        .unwrap_err();
    assert_eq!(
        error,
        ProviderError::Unsupported {
            method: "preview".into()
        }
    );
}

#[test]
fn the_provider_is_usable_through_a_trait_object() {
    let provider: Box<dyn TaskProvider> = Box::new(provider());
    assert!(provider.get(&get("fake-0001")).unwrap().task.is_some());
}
