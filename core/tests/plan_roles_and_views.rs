//! The `roles` setting and the views of the plan pack (ts-0135-cx; 0.7, Settings; 0.10, Planning sources and Views).
//! The real `packs/plan/pack.yaml` is read as the pack loader reads it: it must pass the manifest checks, validate
//! against the generated schema, declare its two views, and have a `roles` setting whose layers and check behave as
//! the plan says. Nothing is read from the user's home: every file is a string in the test.

mod schema_validate;

use collide_core::corpus::Corpus;
use collide_core::item::{Category, KindDecl, StatusDecl};
use collide_core::owner::SettingLayer;
use collide_core::pack::PackManifest;
use collide_core::pack_check::check_manifest;
use collide_core::provider::{Collection, ListResult, Scope, Task};
use collide_core::roles::{ROLES, Roles, check_roles, read_roles};
use collide_core::schema;
use collide_core::setting_value::{check_setting_schema, check_value};
use collide_core::settings::ConfigFile;
use collide_core::settings_layers::{LayerInputs, PackSettings, Settings, settings_layers};
use collide_core::testing::{default_initialize, fake_task};
use collide_core::view::SortDir;
use collide_core::view_eval::{compile_view, evaluate_view};
use collide_core::view_expr::Budgets;
use collide_core::yaml::{load_yaml, load_yaml_file};
use serde_json::{Value, json};
use std::path::Path;

const KEY: &str = "collide/plan.roles";

fn manifest_path() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../packs/plan/pack.yaml")
}

fn manifest() -> PackManifest {
    let path = manifest_path();
    load_yaml_file(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

fn file(text: &str) -> ConfigFile {
    load_yaml(text.as_bytes(), "config.yaml").unwrap_or_else(|error| panic!("{error}\n{text}"))
}

fn layers(user: &str, repo: &str) -> Settings {
    let manifest = manifest();
    let packs = [PackSettings {
        pack: "collide/plan",
        config: &manifest.config,
    }];
    settings_layers(&LayerInputs {
        packs: &packs,
        user: &file(user),
        user_board: &ConfigFile::default(),
        repo: &file(repo),
        profile: None,
    })
}

fn entry(settings: &Settings) -> (Option<Value>, SettingLayer) {
    let entry = settings
        .entries
        .iter()
        .find(|entry| entry.key == KEY)
        .unwrap_or_else(|| panic!("no entry for {KEY}"));
    (entry.value.clone(), entry.layer)
}

fn roles_schema() -> Value {
    manifest().config["roles"].0.clone()
}

fn kind(name: &str) -> KindDecl {
    KindDecl {
        name: name.to_string(),
        statuses: Vec::<StatusDecl>::new(),
        transitions: Vec::new(),
        ops: Vec::new(),
        roles: Vec::new(),
    }
}

/// The kinds of the markdown source.
fn markdown_kinds() -> Vec<KindDecl> {
    ["epic", "proposal", "question", "task", "spike"]
        .map(kind)
        .to_vec()
}

fn roles(value: Value) -> Roles {
    read_roles(&value).unwrap()
}

// ---- the manifest ----

#[test]
fn the_manifest_passes_the_checks_and_validates_against_the_generated_schema() {
    assert_eq!(check_manifest(&manifest()), Ok(()));
    let input: Value =
        serde_json::from_str(&schema::registry().render()["PackManifest.input.json"]).unwrap();
    let doc: Value = load_yaml_file(&manifest_path()).unwrap();
    assert_eq!(
        schema_validate::violations(&input, &doc),
        Vec::<String>::new()
    );
}

#[test]
fn the_pack_declares_three_views_and_only_decisions_needed_feeds_the_queue() {
    let manifest = manifest();
    let names: Vec<&str> = manifest.views.keys().map(String::as_str).collect();
    assert_eq!(names, ["decisions-needed", "follow-ups", "spikes"]);
    assert!(manifest.views["decisions-needed"].queue);
    assert!(!manifest.views["follow-ups"].queue);
    assert!(!manifest.views["spikes"].queue);
    assert_eq!(
        manifest.views["spikes"].where_.as_deref(),
        Some("kind == 'spike'")
    );
}

#[test]
fn follow_ups_is_open_private_items_with_a_raised_by_link_newest_first() {
    let view = &manifest().views["follow-ups"];
    let source = view.where_.as_deref().unwrap();
    for part in [
        "fields.root == 'private'",
        "'raised-by' in link_types",
        "'done'",
        "'dropped'",
    ] {
        assert!(source.contains(part), "{part} in {source}");
    }
    let sort = view
        .layout
        .as_ref()
        .and_then(|layout| layout.sort.as_ref())
        .unwrap();
    assert_eq!((sort.by.as_str(), sort.dir), ("id", SortDir::Desc));
}

#[test]
fn the_pack_does_not_declare_ready_because_core_holds_it() {
    assert!(!manifest().views.contains_key("ready"));
}

#[test]
fn every_view_compiles_and_checks() {
    for (key, view) in &manifest().views {
        assert_eq!(&view.name, key);
        compile_view(view, 100, &Budgets::default())
            .unwrap_or_else(|error| panic!("{key}: {error}"));
    }
}

// ---- the setting ----

#[test]
fn roles_is_declared_inside_the_schema_subset_and_is_neither_repo_only_nor_overridable() {
    let manifest = manifest();
    for (key, schema) in &manifest.config {
        assert_eq!(
            check_setting_schema(&format!("config.{key}"), &schema.0),
            vec![],
            "config.{key}"
        );
    }
    let schema = roles_schema();
    assert_ne!(schema.get("repoOnly"), Some(&json!(true)));
    assert_ne!(schema.get("overridable"), Some(&json!(true)));
    assert_ne!(schema.get("secret"), Some(&json!(true)));
}

#[test]
fn the_default_roles_are_the_markdown_providers_own_mapping() {
    let settings = layers("", "");
    let (value, layer) = entry(&settings);
    assert_eq!(
        value,
        Some(json!({
            "proposal": ["proposal"],
            "question": ["question"],
            "spike": ["spike"],
            "decision": ["proposal", "question"],
            "deferred": [],
        }))
    );
    assert_eq!(layer, SettingLayer::Default);
    assert_eq!(settings.problems, vec![]);
}

#[test]
fn the_users_file_may_map_roles_and_says_user() {
    let settings = layers(
        "config:\n  collide/plan:\n    roles:\n      spike: [spike, task]\n",
        "",
    );
    assert_eq!(settings.problems, vec![]);
    let (value, layer) = entry(&settings);
    assert_eq!(value, Some(json!({ "spike": ["spike", "task"] })));
    assert_eq!(layer, SettingLayer::User);
}

#[test]
fn the_repos_value_wins_over_the_users_and_replaces_the_default_as_a_whole() {
    let settings = layers(
        "config:\n  collide/plan:\n    roles:\n      spike: [task]\n",
        "config:\n  collide/plan:\n    roles:\n      spike: [spike]\n      decision: [proposal]\n",
    );
    assert_eq!(settings.problems, vec![]);
    let (value, layer) = entry(&settings);
    assert_eq!(
        value,
        Some(json!({ "spike": ["spike"], "decision": ["proposal"] }))
    );
    assert_eq!(layer, SettingLayer::Repo);
}

#[test]
fn the_schema_refuses_a_role_it_does_not_list_and_a_value_of_the_wrong_type() {
    let schema = roles_schema();
    for bad in [
        json!({ "adr": ["proposal"] }),
        json!({ "spike": "spike" }),
        json!({ "spike": [1] }),
        json!(["spike"]),
    ] {
        assert!(!check_value(&schema, &bad).is_empty(), "{bad}");
    }
    let good = json!({ "proposal": ["proposal"], "deferred": [] });
    assert!(check_value(&schema, &good).is_empty());
}

// ---- the check against a source's kinds ----

fn default_roles() -> Roles {
    let (value, _) = entry(&layers("", ""));
    roles(value.unwrap())
}

#[test]
fn the_default_roles_name_only_kinds_the_markdown_source_has() {
    let check = check_roles(&default_roles(), "plan", &markdown_kinds());
    assert_eq!(check.problems, vec![]);
}

#[test]
fn deferred_has_no_kind_by_default_so_it_is_off_with_a_reason_and_is_no_problem() {
    let check = check_roles(&default_roles(), "plan", &markdown_kinds());
    assert_eq!(check.off.len(), 1, "{:?}", check.off);
    assert_eq!(check.off[0].role, "deferred");
    assert!(
        check.off[0].reason.contains("no kind"),
        "{:?}",
        check.off[0]
    );
}

#[test]
fn a_kind_the_source_lacks_is_a_problem_that_names_the_role_the_kind_and_the_source() {
    let mapping = roles(json!({
        "proposal": ["proposal"], "question": ["question"], "spike": ["spike"],
        "decision": ["adr"], "deferred": [],
    }));
    let check = check_roles(&mapping, "plan", &markdown_kinds());
    assert_eq!(check.problems.len(), 1, "{:?}", check.problems);
    let text = check.problems[0].to_string();
    assert!(text.contains("`decision`"), "{text}");
    assert!(text.contains("`adr`"), "{text}");
    assert!(text.contains("`plan`"), "{text}");
}

#[test]
fn a_role_whose_kinds_are_all_missing_is_off_and_one_with_a_kind_left_is_on() {
    let mapping = roles(json!({
        "proposal": ["adr"], "question": ["question", "adr"], "spike": ["spike"],
        "decision": ["proposal"], "deferred": ["proposal"],
    }));
    let check = check_roles(&mapping, "plan", &markdown_kinds());
    let off: Vec<&str> = check.off.iter().map(|off| off.role.as_str()).collect();
    assert_eq!(off, ["proposal"]);
    assert!(check.off[0].reason.contains("none of the kinds"));
    assert_eq!(check.problems.len(), 2);
}

#[test]
fn a_role_that_is_not_in_the_map_is_off_and_the_five_roles_are_the_ones_checked() {
    let check = check_roles(
        &roles(json!({ "spike": ["spike"] })),
        "plan",
        &markdown_kinds(),
    );
    let off: Vec<&str> = check.off.iter().map(|off| off.role.as_str()).collect();
    assert_eq!(off, ["proposal", "question", "decision", "deferred"]);
    assert!(
        check
            .off
            .iter()
            .all(|off| off.reason.contains("not mapped"))
    );
    assert_eq!(ROLES.len(), 5);
}

#[test]
fn a_value_that_is_not_a_map_of_lists_does_not_read() {
    let message = read_roles(&json!("spike")).unwrap_err();
    assert!(message.contains("`roles`"), "{message}");
}

// ---- the views over items ----

/// An item of the plan source: its kind, its status and the category that status maps to.
fn item(id: &str, kind: &str, status: &str, category: Category) -> Task {
    let mut task = fake_task(id, id, status, category);
    task.key = format!("plan:{id}");
    task.kind = Some(kind.to_string());
    task
}

fn plan_board() -> Corpus {
    let mut init = default_initialize();
    init.scope = Scope::Global;
    init.kinds = markdown_kinds();
    // The corpus takes each task's category from the statuses its source declares.
    init.statuses = [
        ("draft", Category::Active),
        ("proposed", Category::Review),
        ("accepted", Category::Done),
        ("deferred", Category::Backlog),
        ("open", Category::Ready),
        ("resolved", Category::Done),
        ("dropped", Category::Dropped),
        ("planned", Category::Ready),
        ("concluded", Category::Done),
        ("ready", Category::Ready),
        ("active", Category::Active),
    ]
    .map(|(name, category)| StatusDecl {
        name: name.to_string(),
        category,
    })
    .to_vec();
    let mut corpus = Corpus::default();
    corpus.declare_source("plan", &init);
    let items = vec![
        item("pr-1", "proposal", "draft", Category::Active),
        item("pr-2", "proposal", "proposed", Category::Review),
        item("pr-3", "proposal", "accepted", Category::Done),
        item("pr-4", "proposal", "deferred", Category::Backlog),
        item("qu-1", "question", "open", Category::Ready),
        item("qu-2", "question", "resolved", Category::Done),
        item("qu-3", "question", "dropped", Category::Dropped),
        item("sp-1", "spike", "planned", Category::Ready),
        item("sp-2", "spike", "concluded", Category::Done),
        item("ts-1", "task", "ready", Category::Ready),
        item("ep-1", "epic", "active", Category::Active),
    ];
    corpus
        .load(
            "plan",
            None,
            ListResult {
                tasks: items,
                collections: Vec::<Collection>::new(),
                invalid: Vec::new(),
                next_cursor: None,
            },
        )
        .unwrap();
    corpus
}

fn members_of(view: &str, corpus: &Corpus) -> Vec<String> {
    let manifest = manifest();
    let view = &manifest.views[view];
    let budgets = Budgets::default();
    let compiled = compile_view(view, corpus.rows().len(), &budgets).unwrap();
    let result = evaluate_view(&compiled, corpus, &budgets).unwrap();
    assert_eq!(result.counts.error_count(), 0, "{:?}", result.counts);
    result
        .members
        .iter()
        .map(|key| key.strip_prefix("plan:").unwrap().to_string())
        .collect()
}

#[test]
fn spikes_has_the_spikes_and_no_other_kind() {
    assert_eq!(members_of("spikes", &plan_board()), ["sp-1", "sp-2"]);
}

#[test]
fn decisions_needed_has_the_open_questions_and_the_proposals_that_are_proposed_and_nothing_else() {
    assert_eq!(
        members_of("decisions-needed", &plan_board()),
        ["pr-2", "qu-1"]
    );
}
