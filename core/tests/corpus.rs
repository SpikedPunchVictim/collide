//! Tests for the corpus core (ts-0115): what a board holds, merged from each source's `list` for each checkout. The
//! data comes from `FakeProvider`, so no test reads a file or starts a process.

use collide_core::change::{FieldDiff, ItemState, Rename};
use collide_core::corpus::{
    Applied, CheckoutInfo, CheckoutKind, Corpus, CorpusError, Found, ListFilter, Page, reconcile,
    reconcile_by_source,
};
use collide_core::hook::HookLink;
use collide_core::item::{Category, KindDecl, LinkDir, LinkTarget, LinkTypeDecl, StatusDecl};
use collide_core::owner::task::{TaskGetResult, TaskListResult};
use collide_core::ports::TaskProvider;
use collide_core::provider::{
    ChangeCause, ListParams, ListResult, ProviderEvent, RemovedCause, Scope, SourceStale, Task,
    TaskChanged, TaskInvalid, TaskRemoved,
};
use collide_core::testing::{FakeProvider, default_initialize, fake_task};
use serde_json::json;

const ROOT: &str = "/repo";
const WS_A: &str = "/repo-ws/a";
const WS_B: &str = "/repo-ws/b";
const OTHER_ROOT: &str = "/repo-line";
const OTHER_WS: &str = "/repo-line-ws/c";

// --- fixtures ---

/// The statuses a `plan` source declares: six names for six categories.
fn plan_statuses() -> Vec<StatusDecl> {
    [
        ("specified", Category::Backlog),
        ("ready", Category::Ready),
        ("doing", Category::Active),
        ("in_review", Category::Review),
        ("done", Category::Done),
        ("dropped", Category::Dropped),
    ]
    .iter()
    .map(|(name, category)| StatusDecl {
        name: (*name).into(),
        category: *category,
    })
    .collect()
}

fn category_of(status: &str) -> Category {
    plan_statuses()
        .iter()
        .find(|decl| decl.name == status)
        .map_or(Category::Backlog, |decl| decl.category)
}

fn link_type(name: &str, target: LinkTarget) -> LinkTypeDecl {
    LinkTypeDecl {
        name: name.into(),
        dir: LinkDir::Out,
        blocking: false,
        target,
    }
}

/// `plan`: ref-scoped, with the six statuses and three link types, one of which targets a feature.
fn plan_init() -> collide_core::provider::InitializeResult {
    let mut init = default_initialize();
    init.scope = Scope::Ref;
    init.statuses = plan_statuses();
    init.links = vec![
        link_type("blocked-by", LinkTarget::Item),
        link_type("relates-to", LinkTarget::Item),
        link_type("covers", LinkTarget::Feature),
    ];
    init
}

/// `gh`: a global source with an open and a closed status.
fn gh_init() -> collide_core::provider::InitializeResult {
    let mut init = default_initialize();
    init.statuses = vec![
        StatusDecl {
            name: "open".into(),
            category: Category::Ready,
        },
        StatusDecl {
            name: "closed".into(),
            category: Category::Done,
        },
    ];
    init.links = vec![link_type("relates-to", LinkTarget::Item)];
    init
}

/// A `plan` task in `checkout` at `path`. The category is the declared one for the status.
fn plan_task(short: &str, status: &str, checkout: &str, path: &str) -> Task {
    let mut task = fake_task(
        short,
        &format!("title of {short}"),
        status,
        category_of(status),
    );
    task.key = format!("plan:{short}");
    task.checkout = Some(checkout.into());
    task.path = Some(path.into());
    task
}

/// The same task, at the path its short id gives, in `checkout`.
fn at(short: &str, status: &str, checkout: &str) -> Task {
    plan_task(short, status, checkout, &format!("arch/{short}.md"))
}

fn with_link(mut task: Task, link_type: &str, to: &str) -> Task {
    task.links.push(
        serde_json::from_value(json!({ "type": link_type, "to": to })).expect("a link reads"),
    );
    task
}

fn gh_task(number: u32, status: &str) -> Task {
    let category = if status == "open" {
        Category::Ready
    } else {
        Category::Done
    };
    let mut task = fake_task(&format!("acme/edge#{number}"), "an issue", status, category);
    task.key = format!("gh:acme/edge#{number}");
    task
}

fn checkouts() -> Vec<CheckoutInfo> {
    let info = |path: &str, kind| CheckoutInfo {
        path: path.into(),
        kind,
    };
    let workspace = |root: &str| CheckoutKind::Workspace {
        forked_from: root.into(),
    };
    vec![
        info(ROOT, CheckoutKind::Line { default: true }),
        info(WS_A, workspace(ROOT)),
        info(WS_B, workspace(ROOT)),
        info(OTHER_ROOT, CheckoutKind::Line { default: false }),
        info(OTHER_WS, workspace(OTHER_ROOT)),
    ]
}

/// What `plan`'s `list` gives for `checkout` when the provider holds `tasks`.
fn list_of(tasks: &[Task], checkout: &str) -> ListResult {
    let mut provider = FakeProvider::new().with_initialize(plan_init());
    for task in tasks {
        provider = provider.with_task(task.clone());
    }
    provider
        .list(&ListParams {
            checkout: Some(checkout.into()),
            cursor: None,
        })
        .expect("the fake lists")
}

/// A corpus of the source `plan`, loaded the way the daemon does it: one `list` for each checkout it is given.
fn plan_corpus(tasks: &[Task], listed: &[&str]) -> Corpus {
    let mut corpus = Corpus::default();
    corpus.set_checkouts(checkouts());
    corpus.declare_source("plan", &plan_init());
    for checkout in listed {
        corpus
            .load("plan", Some(checkout), list_of(tasks, checkout))
            .unwrap();
    }
    corpus
}

fn all_checkouts() -> [&'static str; 5] {
    [ROOT, WS_A, WS_B, OTHER_ROOT, OTHER_WS]
}

fn state(status: &str) -> ItemState {
    ItemState {
        status: status.into(),
        category: category_of(status),
    }
}

fn shown(corpus: &Corpus, key: &str) -> Vec<(String, String)> {
    corpus
        .rows()
        .iter()
        .filter(|row| row.task.key == key)
        .map(|row| {
            (
                row.task.checkout.clone().unwrap_or_default(),
                row.task.state.status.clone(),
            )
        })
        .collect()
}

fn invalid_item(path: &str, checkout: &str, reason: &str) -> TaskInvalid {
    TaskInvalid {
        path: path.into(),
        checkout: Some(checkout.into()),
        reason: reason.into(),
    }
}

fn changed_to(key: &str, checkout: &str, status: &str, version: &str) -> TaskChanged {
    TaskChanged {
        key: key.into(),
        version: version.into(),
        from: None,
        to: Some(ItemState {
            status: status.into(),
            category: category_of(status),
        }),
        diff: Default::default(),
        cause: ChangeCause::Worktree,
        actor: None,
        automated: None,
        checkout: Some(checkout.into()),
    }
}

/// Everything the corpus answers, as text, so that two corpora can be compared.
fn snapshot(corpus: &Corpus) -> String {
    let keys: Vec<_> = corpus
        .rows()
        .iter()
        .map(|row| row.task.key.clone())
        .collect();
    let answers: Vec<_> = keys
        .iter()
        .map(|key| (corpus.get(key), corpus.links_of(key)))
        .collect();
    format!(
        "{:?}",
        (
            corpus.rows(),
            answers,
            corpus.invalid(),
            corpus.collisions(),
            corpus.divergences(),
            corpus.stale_sources(),
        )
    )
}

// --- divergence ---

#[test]
fn a_task_doing_in_a_workspace_and_ready_at_its_root_shows_the_root_and_reports_the_divergence() {
    let tasks = [at("T-01", "ready", ROOT), at("T-01", "doing", WS_A)];
    let corpus = plan_corpus(&tasks, &[ROOT, WS_A]);

    assert_eq!(shown(&corpus, "plan:T-01"), [(ROOT.into(), "ready".into())]);
    let divergences = corpus.divergences();
    assert_eq!(divergences.len(), 1);
    let divergence = &divergences[0];
    assert_eq!(divergence.key, "plan:T-01");
    assert_eq!(divergence.checkout, WS_A);
    assert_eq!(divergence.state, state("doing"));
    assert_eq!(divergence.root, ROOT);
    assert_eq!(divergence.root_state, state("ready"));
    assert_eq!(corpus.rows()[0].divergence, divergences);
}

#[test]
fn a_workspace_that_agrees_with_its_root_is_not_divergent() {
    let tasks = [at("T-01", "ready", ROOT), at("T-01", "ready", WS_A)];
    let corpus = plan_corpus(&tasks, &[ROOT, WS_A]);
    assert!(corpus.divergences().is_empty());
}

#[test]
fn two_workspaces_of_one_root_each_compare_with_the_root() {
    let tasks = [
        at("T-01", "ready", ROOT),
        at("T-01", "doing", WS_A),
        at("T-01", "ready", WS_B),
    ];
    let corpus = plan_corpus(&tasks, &[ROOT, WS_A, WS_B]);
    let checkouts: Vec<_> = corpus
        .divergences()
        .into_iter()
        .map(|d| d.checkout)
        .collect();
    assert_eq!(checkouts, [WS_A]);

    let both = [
        at("T-01", "ready", ROOT),
        at("T-01", "doing", WS_A),
        at("T-01", "done", WS_B),
    ];
    let corpus = plan_corpus(&both, &[ROOT, WS_A, WS_B]);
    let checkouts: Vec<_> = corpus
        .divergences()
        .into_iter()
        .map(|d| d.checkout)
        .collect();
    assert_eq!(checkouts, [WS_A, WS_B]);
}

#[test]
fn another_lines_root_is_a_copy_and_never_a_divergence() {
    let tasks = [at("T-01", "ready", ROOT), at("T-01", "done", OTHER_ROOT)];
    let corpus = plan_corpus(&tasks, &[ROOT, OTHER_ROOT]);

    assert!(corpus.divergences().is_empty());
    assert_eq!(shown(&corpus, "plan:T-01"), [(ROOT.into(), "ready".into())]);
    let copies = corpus.get("plan:T-01").copies;
    let at_checkout: Vec<_> = copies
        .iter()
        .map(|c| (c.checkout.as_str(), &c.state))
        .collect();
    assert_eq!(
        at_checkout,
        [(ROOT, &state("ready")), (OTHER_ROOT, &state("done"))]
    );
}

#[test]
fn a_workspace_of_another_line_is_compared_with_that_lines_root() {
    let tasks = [
        at("T-01", "ready", ROOT),
        at("T-01", "done", OTHER_ROOT),
        at("T-01", "done", OTHER_WS),
    ];
    let corpus = plan_corpus(&tasks, &[ROOT, OTHER_ROOT, OTHER_WS]);
    assert!(
        corpus.divergences().is_empty(),
        "it agrees with its own root"
    );

    let tasks = [
        at("T-01", "ready", ROOT),
        at("T-01", "done", OTHER_ROOT),
        at("T-01", "doing", OTHER_WS),
    ];
    let corpus = plan_corpus(&tasks, &[ROOT, OTHER_ROOT, OTHER_WS]);
    let divergences = corpus.divergences();
    assert_eq!(divergences.len(), 1);
    assert_eq!(divergences[0].checkout, OTHER_WS);
    assert_eq!(divergences[0].root, OTHER_ROOT);
    assert_eq!(divergences[0].root_state, state("done"));
}

#[test]
fn a_task_only_in_a_workspace_is_shown_from_it_as_branch_only() {
    let tasks = [at("T-01", "doing", WS_B), at("T-01", "ready", WS_A)];
    let corpus = plan_corpus(&tasks, &[ROOT, WS_A, WS_B]);

    let rows = corpus.rows();
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0].task.checkout.as_deref(),
        Some(WS_A),
        "the first in path order"
    );
    assert!(rows[0].branch_only);
    assert!(
        corpus.divergences().is_empty(),
        "there is no root copy to differ from"
    );
}

#[test]
fn a_task_at_the_default_root_is_not_branch_only() {
    let tasks = [at("T-01", "ready", ROOT)];
    let corpus = plan_corpus(&tasks, &[ROOT]);
    assert!(!corpus.rows()[0].branch_only);
}

#[test]
fn a_task_of_a_global_source_has_no_checkout_and_is_not_branch_only() {
    let mut corpus = Corpus::default();
    corpus.set_checkouts(checkouts());
    corpus.declare_source("gh", &gh_init());
    let list = ListResult {
        tasks: vec![gh_task(7, "open")],
        collections: vec![],
        invalid: vec![],
        next_cursor: None,
    };
    corpus.load("gh", None, list).unwrap();

    let rows = corpus.rows();
    assert_eq!(rows.len(), 1);
    assert!(!rows[0].branch_only);
    assert!(corpus.get("gh:acme/edge#7").copies.is_empty());
}

#[test]
fn the_default_roots_copy_is_shown_even_when_another_checkout_comes_first_in_path_order() {
    let early = "/aa-ws";
    let late_root = "/zz-root";
    let mut corpus = Corpus::default();
    corpus.set_checkouts(vec![
        CheckoutInfo {
            path: late_root.into(),
            kind: CheckoutKind::Line { default: true },
        },
        CheckoutInfo {
            path: early.into(),
            kind: CheckoutKind::Workspace {
                forked_from: late_root.into(),
            },
        },
    ]);
    corpus.declare_source("plan", &plan_init());
    let tasks = [at("T-01", "doing", early), at("T-01", "ready", late_root)];
    for checkout in [early, late_root] {
        corpus
            .load("plan", Some(checkout), list_of(&tasks, checkout))
            .unwrap();
    }
    assert_eq!(
        shown(&corpus, "plan:T-01"),
        [(late_root.into(), "ready".into())]
    );
    assert!(!corpus.rows()[0].branch_only);
}

// --- collisions ---

#[test]
fn one_short_id_at_two_paths_in_two_checkouts_is_a_collision_and_no_divergence() {
    let tasks = [
        plan_task("T-01", "ready", ROOT, "arch/a/T-01-first.md"),
        plan_task("T-01", "doing", WS_A, "arch/b/T-01-second.md"),
    ];
    let corpus = plan_corpus(&tasks, &[ROOT, WS_A]);

    assert!(corpus.divergences().is_empty());
    let collisions = corpus.collisions();
    assert_eq!(collisions.len(), 1);
    assert_eq!(collisions[0].short_id, "T-01");
    let places: Vec<_> = collisions[0]
        .locations
        .iter()
        .map(|place| (place.checkout.as_str(), place.path.as_str()))
        .collect();
    assert_eq!(
        places,
        [
            (ROOT, "arch/a/T-01-first.md"),
            (WS_A, "arch/b/T-01-second.md")
        ]
    );
}

#[test]
fn the_same_path_in_two_checkouts_with_two_states_is_a_divergence_and_no_collision() {
    let tasks = [at("T-01", "ready", ROOT), at("T-01", "doing", WS_A)];
    let corpus = plan_corpus(&tasks, &[ROOT, WS_A]);
    assert!(corpus.collisions().is_empty());
    assert_eq!(corpus.divergences().len(), 1);
}

#[test]
fn a_lookup_of_a_collided_id_says_it_is_ambiguous() {
    let tasks = [
        plan_task("T-01", "ready", ROOT, "arch/a/T-01-first.md"),
        plan_task("T-01", "doing", WS_A, "arch/b/T-01-second.md"),
    ];
    let corpus = plan_corpus(&tasks, &[ROOT, WS_A]);

    let Found::Collided(key, collision) = corpus.lookup("plan:T-01") else {
        panic!("a collided key is not one task");
    };
    assert_eq!(key, "plan:T-01");
    assert_eq!(collision.locations.len(), 2);
    let got = corpus.get("T-01");
    assert!(got.task.is_none());
    assert_eq!(got.ambiguous, ["plan:T-01"]);
    assert_eq!(got.collisions.len(), 1);
    assert_eq!(got.copies.len(), 2);
}

#[test]
fn a_collision_ends_when_one_copy_moves_to_the_other_path() {
    let tasks = [
        plan_task("T-01", "ready", ROOT, "arch/a/T-01-first.md"),
        plan_task("T-01", "doing", WS_A, "arch/b/T-01-second.md"),
    ];
    let mut corpus = plan_corpus(&tasks, &[ROOT, WS_A]);
    let moved = plan_task("T-01", "doing", WS_A, "arch/a/T-01-first.md");
    let event = changed_to("plan:T-01", WS_A, "doing", "v2");
    corpus.apply_changed("plan", &event, Some(moved)).unwrap();
    assert!(corpus.collisions().is_empty());
    assert!(matches!(corpus.lookup("plan:T-01"), Found::One(_)));
}

// --- categories ---

#[test]
fn the_category_is_the_one_the_source_declared_for_the_status() {
    let mut wrong = at("T-01", "doing", ROOT);
    wrong.state.category = Category::Done;
    let corpus = plan_corpus(&[wrong], &[ROOT]);
    assert_eq!(corpus.rows()[0].task.state.category, Category::Active);
}

#[test]
fn the_statuses_of_a_source_are_kept_in_its_order() {
    let corpus = plan_corpus(&[], &[]);
    let names: Vec<_> = corpus
        .statuses("plan")
        .iter()
        .map(|s| s.name.as_str())
        .collect();
    assert_eq!(
        names,
        [
            "specified",
            "ready",
            "doing",
            "in_review",
            "done",
            "dropped"
        ]
    );
    assert!(corpus.statuses("nothing").is_empty());
}

#[test]
fn a_status_a_kind_declares_maps_to_the_kinds_category() {
    let mut init = plan_init();
    init.kinds = vec![KindDecl {
        name: "proposal".into(),
        statuses: vec![StatusDecl {
            name: "draft".into(),
            category: Category::Active,
        }],
        transitions: vec![],
        ops: vec![],
        roles: vec![],
    }];
    let mut task = at("PR-1", "draft", ROOT);
    task.kind = Some("proposal".into());
    task.state.category = Category::Backlog;
    let mut corpus = Corpus::default();
    corpus.declare_source("plan", &init);
    corpus
        .load("plan", Some(ROOT), list_of(&[task], ROOT))
        .unwrap();
    assert_eq!(corpus.rows()[0].task.state.category, Category::Active);
    assert!(corpus.invalid().is_empty());
}

#[test]
fn a_status_the_list_gave_that_the_source_did_not_declare_is_an_invalid_item_not_a_task() {
    let mut odd = at("T-02", "waiting", ROOT);
    odd.state.category = Category::Backlog;
    let mut list = list_of(&[at("T-01", "ready", ROOT), odd], ROOT);
    list.invalid
        .push(invalid_item("arch/broken.md", ROOT, "conflict markers"));
    let mut corpus = Corpus::default();
    corpus.declare_source("plan", &plan_init());
    corpus.load("plan", Some(ROOT), list).unwrap();

    let keys: Vec<_> = corpus
        .rows()
        .iter()
        .map(|row| row.task.key.clone())
        .collect();
    assert_eq!(keys, ["plan:T-01"]);
    assert!(matches!(corpus.lookup("plan:T-02"), Found::None));
    let invalid = corpus.invalid();
    assert_eq!(
        invalid.len(),
        2,
        "it counts with the source's other invalid items"
    );
    let odd = invalid
        .iter()
        .find(|entry| entry.item.path == "arch/T-02.md")
        .expect("the item is listed by its path");
    assert_eq!(odd.source, "plan");
    assert_eq!(odd.item.checkout.as_deref(), Some(ROOT));
    assert!(odd.item.reason.contains("`waiting`"), "{}", odd.item.reason);
    assert_eq!(corpus.list_result().invalid.len(), 2);
}

#[test]
fn a_changed_notification_with_an_undeclared_status_makes_the_item_invalid() {
    let mut corpus = plan_corpus(&[at("T-01", "ready", ROOT)], &[ROOT]);
    let mut event = changed_to("plan:T-01", ROOT, "waiting", "v2");
    event.to.as_mut().unwrap().category = Category::Backlog;
    let applied = corpus.apply_changed("plan", &event, None).unwrap();

    assert_eq!(applied, Applied::Done);
    assert!(corpus.rows().is_empty(), "the task is not listed");
    let invalid = corpus.invalid();
    assert_eq!(invalid.len(), 1);
    assert_eq!(invalid[0].item.path, "arch/T-01.md");
    assert!(invalid[0].item.reason.contains("`waiting`"));

    // The path is fixed: a later change brings the task back and clears the item.
    let fixed = changed_to("plan:T-01", ROOT, "doing", "v3");
    let applied = corpus.apply_changed("plan", &fixed, None).unwrap();
    assert_eq!(
        applied,
        Applied::NeedsTask,
        "the copy was dropped, so the task is fetched"
    );
    let task = at("T-01", "doing", ROOT);
    corpus.apply_changed("plan", &fixed, Some(task)).unwrap();
    assert_eq!(shown(&corpus, "plan:T-01"), [(ROOT.into(), "doing".into())]);
    assert!(corpus.invalid().is_empty());
}

// --- links ---

fn two_sources() -> Corpus {
    let plan = [
        with_link(at("T-01", "doing", ROOT), "blocked-by", "gh:acme/edge#7"),
        with_link(at("T-02", "ready", ROOT), "blocked-by", "T-01"),
    ];
    let mut corpus = plan_corpus(&plan, &[ROOT]);
    corpus.declare_source("gh", &gh_init());
    let list = ListResult {
        tasks: vec![with_link(gh_task(7, "open"), "relates-to", "plan:T-02")],
        collections: vec![],
        invalid: vec![],
        next_cursor: None,
    };
    corpus.load("gh", None, list).unwrap();
    corpus
}

fn link<'a>(links: &'a [HookLink], dir: LinkDir, key: &str) -> &'a HookLink {
    links
        .iter()
        .find(|link| link.dir == dir && link.key == key)
        .unwrap_or_else(|| panic!("no {dir:?} link to {key} in {links:?}"))
}

#[test]
fn links_resolve_one_hop_in_both_directions_across_two_sources() {
    let corpus = two_sources();

    let out = corpus.links_of("plan:T-01");
    assert_eq!(out.len(), 2);
    let to_issue = link(&out, LinkDir::Out, "gh:acme/edge#7");
    assert_eq!(to_issue.link_type, "blocked-by");
    assert_eq!(to_issue.status, "open");
    assert_eq!(to_issue.category, Category::Ready);
    assert!(!to_issue.stale && !to_issue.missing);
    let from_t02 = link(&out, LinkDir::In, "plan:T-02");
    assert_eq!(from_t02.link_type, "blocked-by");
    assert_eq!(from_t02.status, "ready");
    assert_eq!(from_t02.category, Category::Ready);

    // The issue's own link out to a plan task, and the plan task's link in to the issue.
    let issue = corpus.links_of("gh:acme/edge#7");
    assert_eq!(link(&issue, LinkDir::Out, "plan:T-02").status, "ready");
    assert_eq!(link(&issue, LinkDir::In, "plan:T-01").status, "doing");
}

#[test]
fn a_link_carries_the_kind_of_the_other_end() {
    let mut corpus = two_sources();
    let mut epic = at("E-1", "doing", ROOT);
    epic.kind = Some("epic".into());
    let task = with_link(at("T-09", "ready", ROOT), "relates-to", "E-1");
    let list = list_of(&[epic, task], ROOT);
    corpus.load("plan", Some(ROOT), list).unwrap();
    let links = corpus.links_of("plan:T-09");
    assert_eq!(link(&links, LinkDir::Out, "plan:E-1").kind, "epic");
}

#[test]
fn a_link_to_a_source_that_is_stale_is_marked_stale() {
    let mut corpus = two_sources();
    corpus.mark_stale("gh", "rate limited").unwrap();

    let links = corpus.links_of("plan:T-01");
    assert!(link(&links, LinkDir::Out, "gh:acme/edge#7").stale);
    assert!(!link(&links, LinkDir::In, "plan:T-02").stale);
    let issue = corpus.links_of("gh:acme/edge#7");
    assert!(!link(&issue, LinkDir::Out, "plan:T-02").stale);
    assert_eq!(corpus.stale_sources(), [("gh", "rate limited")]);

    corpus.mark_fresh("gh").unwrap();
    assert!(corpus.stale_sources().is_empty());
}

#[test]
fn a_link_to_a_key_the_corpus_does_not_hold_is_reported_as_missing() {
    let tasks = [with_link(
        at("T-01", "doing", ROOT),
        "blocked-by",
        "plan:T-99",
    )];
    let corpus = plan_corpus(&tasks, &[ROOT]);

    let links = corpus.links_of("plan:T-01");
    assert_eq!(links.len(), 1, "the link is not dropped");
    let missing = &links[0];
    assert_eq!(missing.key, "plan:T-99");
    assert!(missing.missing);
    assert_eq!(missing.kind, "");
    assert_eq!(missing.status, "");
    assert_eq!(missing.category, Category::Backlog);
    assert!(!missing.stale);
}

#[test]
fn a_bare_target_is_a_short_id_an_id_or_an_alias_in_the_source_of_the_link() {
    let mut aliased = at("T-03", "done", ROOT);
    aliased.id = "T-03-the-long-name".into();
    aliased.aliases = vec!["three".into()];
    let tasks = [
        aliased,
        with_link(at("T-01", "doing", ROOT), "blocked-by", "T-03"),
        with_link(
            at("T-02", "doing", ROOT),
            "blocked-by",
            "T-03-the-long-name",
        ),
        with_link(at("T-04", "doing", ROOT), "blocked-by", "three"),
    ];
    let corpus = plan_corpus(&tasks, &[ROOT]);
    for holder in ["plan:T-01", "plan:T-02", "plan:T-04"] {
        let links = corpus.links_of(holder);
        let out = link(&links, LinkDir::Out, "plan:T-03");
        assert!(
            !out.missing && out.category == Category::Done,
            "{holder}: {out:?}"
        );
    }
}

#[test]
fn a_link_whose_type_targets_a_feature_is_not_resolved_here() {
    let tasks = [
        with_link(at("PR-1", "doing", ROOT), "covers", "pr-0003-km#F2"),
        with_link(at("T-01", "doing", ROOT), "covers", "plan:T-02"),
        at("T-02", "ready", ROOT),
    ];
    let corpus = plan_corpus(&tasks, &[ROOT]);
    assert!(corpus.links_of("plan:PR-1").is_empty());
    assert!(corpus.links_of("plan:T-01").is_empty());
    assert!(
        corpus.links_of("plan:T-02").is_empty(),
        "no link in from a feature link"
    );
}

#[test]
fn links_follow_the_task_a_notification_changed() {
    let mut corpus = two_sources();
    assert_eq!(corpus.links_of("plan:T-01").len(), 2);

    let without = at("T-02", "ready", ROOT);
    let event = changed_to("plan:T-02", ROOT, "ready", "v3");
    corpus.apply_changed("plan", &event, Some(without)).unwrap();
    let links = corpus.links_of("plan:T-01");
    assert_eq!(links.len(), 1, "the link in from T-02 is gone");
    assert_eq!(links[0].dir, LinkDir::Out);
}

// --- reconcile ---

fn reconciled(cached: &[Task], fresh: &[Task]) -> Vec<ProviderEvent> {
    reconcile(
        &plan_corpus(cached, &[ROOT, WS_A]),
        &plan_corpus(fresh, &[ROOT, WS_A]),
    )
}

#[test]
fn reconcile_of_an_unchanged_source_gives_no_event() {
    let tasks = [
        at("T-01", "ready", ROOT),
        at("T-01", "doing", WS_A),
        at("T-02", "done", ROOT),
    ];
    assert!(reconciled(&tasks, &tasks).is_empty());
}

#[test]
fn reconcile_of_a_changed_task_gives_one_startup_event_with_the_old_and_new_state() {
    let before = [at("T-01", "ready", ROOT), at("T-02", "done", ROOT)];
    let mut after = at("T-01", "doing", ROOT);
    after.version = "v2".into();
    let events = reconciled(&before, &[after, at("T-02", "done", ROOT)]);

    assert_eq!(events.len(), 1);
    let ProviderEvent::Changed(event) = &events[0] else {
        panic!("expected a task.changed, got {events:?}");
    };
    assert_eq!(event.key, "plan:T-01");
    assert_eq!(event.cause, ChangeCause::Startup);
    assert_eq!(event.from, Some(state("ready")));
    assert_eq!(event.to, Some(state("doing")));
    assert_eq!(event.version, "v2");
    assert_eq!(event.checkout.as_deref(), Some(ROOT));
    assert_eq!(event.actor, None);
}

#[test]
fn reconcile_names_what_changed_in_the_diff() {
    let before = [at("T-01", "ready", ROOT)];
    let mut after = at("T-01", "ready", ROOT);
    after.title = "a new title".into();
    after.labels = vec!["mvp".into()];
    after.version = "v2".into();
    let events = reconciled(&before, &[after]);
    let [ProviderEvent::Changed(event)] = events.as_slice() else {
        panic!("expected one task.changed, got {events:?}");
    };
    let title = &event.diff["title"];
    assert_eq!(
        serde_json::to_value(title).unwrap(),
        json!({ "from": "title of T-01", "to": "a new title" })
    );
    assert!(
        matches!(&event.diff["labels"], FieldDiff::Set { added, .. } if added == &[json!("mvp")])
    );
}

#[test]
fn reconcile_of_a_new_task_gives_one_event_with_from_null() {
    let events = reconciled(
        &[at("T-01", "ready", ROOT)],
        &[at("T-01", "ready", ROOT), at("T-02", "ready", WS_A)],
    );
    let [ProviderEvent::Changed(event)] = events.as_slice() else {
        panic!("expected one task.changed, got {events:?}");
    };
    assert_eq!(event.key, "plan:T-02");
    assert_eq!(event.from, None);
    assert_eq!(event.to, Some(state("ready")));
    assert_eq!(event.checkout.as_deref(), Some(WS_A));
    let wire = serde_json::to_value(event).unwrap();
    assert_eq!(wire["from"], json!(null));
    assert_eq!(wire["cause"], json!("startup"));
}

#[test]
fn reconcile_of_a_task_that_is_gone_gives_one_removed_with_cause_unknown() {
    let events = reconciled(
        &[at("T-01", "ready", ROOT), at("T-02", "ready", ROOT)],
        &[at("T-01", "ready", ROOT)],
    );
    assert_eq!(
        events,
        [ProviderEvent::Removed(TaskRemoved {
            key: "plan:T-02".into(),
            cause: RemovedCause::Unknown,
            checkout: Some(ROOT.into()),
        })]
    );
}

#[test]
fn reconcile_gives_one_event_for_each_copy() {
    let before = [at("T-01", "ready", ROOT), at("T-01", "ready", WS_A)];
    let after = [at("T-01", "doing", ROOT), at("T-01", "doing", WS_A)];
    let mut after: Vec<Task> = after.into();
    for task in &mut after {
        task.version = "v2".into();
    }
    assert_eq!(reconciled(&before, &after).len(), 2);
}

#[test]
fn reconcile_of_a_task_that_became_invalid_gives_the_invalid_item_and_no_removal() {
    let mut cached = plan_corpus(&[at("T-01", "ready", ROOT)], &[ROOT]);
    let mut list = list_of(&[], ROOT);
    list.invalid
        .push(invalid_item("arch/T-01.md", ROOT, "conflict markers"));
    let mut fresh = Corpus::default();
    fresh.set_checkouts(checkouts());
    fresh.declare_source("plan", &plan_init());
    fresh.load("plan", Some(ROOT), list).unwrap();

    let events = reconcile(&cached, &fresh);
    assert_eq!(
        events,
        [ProviderEvent::Invalid(invalid_item(
            "arch/T-01.md",
            ROOT,
            "conflict markers"
        ))]
    );
    // And once it is the cache, the same list gives nothing.
    cached = fresh.clone();
    assert!(reconcile(&cached, &fresh).is_empty());
}

#[test]
fn reconcile_by_source_names_the_source_of_each_event_the_invalid_ones_too() {
    let cached = plan_corpus(&[at("T-01", "ready", ROOT)], &[ROOT]);
    let mut list = list_of(&[at("T-01", "doing", ROOT)], ROOT);
    list.invalid
        .push(invalid_item("arch/bad.md", ROOT, "conflict markers"));
    let mut fresh = Corpus::default();
    fresh.set_checkouts(checkouts());
    fresh.declare_source("plan", &plan_init());
    fresh.load("plan", Some(ROOT), list).unwrap();

    let events = reconcile_by_source(&cached, &fresh);
    assert_eq!(events.len(), 2);
    assert!(
        events.iter().all(|(source, _)| source == "plan"),
        "{events:?}"
    );
    assert_eq!(
        events
            .iter()
            .map(|(_, event)| event.clone())
            .collect::<Vec<_>>(),
        reconcile(&cached, &fresh)
    );
}

#[test]
fn reconcile_says_nothing_of_what_it_was_not_given_a_list_for() {
    let cached = plan_corpus(
        &[at("T-01", "ready", ROOT), at("T-02", "ready", WS_A)],
        &[ROOT, WS_A],
    );
    let fresh = plan_corpus(&[at("T-01", "ready", ROOT)], &[ROOT]);
    assert!(
        reconcile(&cached, &fresh).is_empty(),
        "the list of WS_A was not read"
    );

    let mut stale = plan_corpus(&[], &[ROOT, WS_A]);
    stale.mark_stale("plan", "could not read").unwrap();
    assert!(
        reconcile(&cached, &stale).is_empty(),
        "a stale source's lists are not the truth"
    );
}

// --- renames ---

#[test]
fn a_rename_moves_the_key_and_the_old_key_resolves_to_the_new_one() {
    let mut task = at("T-01", "ready", ROOT);
    task.id = "T-01-the-old-slug".into();
    let mut corpus = plan_corpus(&[task], &[ROOT]);
    let rename = Rename {
        from: "plan:T-01".into(),
        to: "plan:T-05".into(),
        checkout: Some(ROOT.into()),
    };
    corpus.apply_renamed("plan", &rename).unwrap();

    let keys: Vec<_> = corpus
        .rows()
        .iter()
        .map(|row| row.task.key.clone())
        .collect();
    assert_eq!(keys, ["plan:T-05"]);
    assert_eq!(corpus.lookup("plan:T-01"), Found::One("plan:T-05".into()));
    assert_eq!(corpus.lookup("plan:T-05"), Found::One("plan:T-05".into()));
    let got = corpus.get("plan:T-01");
    assert_eq!(got.task.unwrap().key, "plan:T-05");
}

#[test]
fn a_rename_without_a_checkout_moves_every_copy() {
    let tasks = [at("T-01", "ready", ROOT), at("T-01", "doing", WS_A)];
    let mut corpus = plan_corpus(&tasks, &[ROOT, WS_A]);
    let rename = Rename {
        from: "plan:T-01".into(),
        to: "plan:T-05".into(),
        checkout: None,
    };
    corpus.apply_renamed("plan", &rename).unwrap();
    assert_eq!(corpus.get("plan:T-05").copies.len(), 2);
    assert_eq!(corpus.divergences().len(), 1);
    assert_eq!(corpus.divergences()[0].key, "plan:T-05");
}

#[test]
fn a_rename_in_one_checkout_moves_that_copy_and_leaves_the_others() {
    let tasks = [at("T-01", "ready", ROOT), at("T-01", "doing", WS_A)];
    let mut corpus = plan_corpus(&tasks, &[ROOT, WS_A]);
    let rename = Rename {
        from: "plan:T-01".into(),
        to: "plan:T-05".into(),
        checkout: Some(WS_A.into()),
    };
    corpus.apply_renamed("plan", &rename).unwrap();

    assert_eq!(shown(&corpus, "plan:T-01"), [(ROOT.into(), "ready".into())]);
    assert_eq!(shown(&corpus, "plan:T-05"), [(WS_A.into(), "doing".into())]);
    assert_eq!(
        corpus.lookup("plan:T-01"),
        Found::One("plan:T-01".into()),
        "the old key still holds the root"
    );
    assert!(corpus.divergences().is_empty());
}

#[test]
fn a_link_to_the_old_key_follows_the_rename_in_both_directions() {
    let tasks = [
        at("T-01", "done", ROOT),
        with_link(at("T-02", "ready", ROOT), "blocked-by", "plan:T-01"),
    ];
    let mut corpus = plan_corpus(&tasks, &[ROOT]);
    let rename = Rename {
        from: "plan:T-01".into(),
        to: "plan:T-05".into(),
        checkout: None,
    };
    corpus.apply_renamed("plan", &rename).unwrap();

    let out = corpus.links_of("plan:T-02");
    assert_eq!(link(&out, LinkDir::Out, "plan:T-05").status, "done");
    let into = corpus.links_of("plan:T-05");
    assert_eq!(link(&into, LinkDir::In, "plan:T-02").status, "ready");
}

// --- invalid items, and a fix ---

#[test]
fn an_invalid_notification_adds_the_item_and_removes_the_task_of_that_path() {
    let mut corpus = plan_corpus(
        &[at("T-01", "ready", ROOT), at("T-02", "ready", ROOT)],
        &[ROOT],
    );
    let item = invalid_item("arch/T-01.md", ROOT, "conflict markers");
    corpus.apply_invalid("plan", &item).unwrap();

    let keys: Vec<_> = corpus
        .rows()
        .iter()
        .map(|row| row.task.key.clone())
        .collect();
    assert_eq!(keys, ["plan:T-02"]);
    let invalid = corpus.invalid();
    assert_eq!(invalid.len(), 1);
    assert_eq!(invalid[0].item, &item);
}

#[test]
fn a_copy_that_moved_no_longer_answers_for_its_old_path() {
    let mut corpus = plan_corpus(&[at("T-01", "ready", ROOT)], &[ROOT]);
    let moved = plan_task("T-01", "ready", ROOT, "arch/e-02/T-01.md");
    let event = changed_to("plan:T-01", ROOT, "ready", "v2");
    corpus.apply_changed("plan", &event, Some(moved)).unwrap();

    let stale_path = invalid_item("arch/T-01.md", ROOT, "a file that is not there now");
    corpus.apply_invalid("plan", &stale_path).unwrap();
    assert_eq!(shown(&corpus, "plan:T-01"), [(ROOT.into(), "ready".into())]);

    let here = invalid_item("arch/e-02/T-01.md", ROOT, "conflict markers");
    corpus.apply_invalid("plan", &here).unwrap();
    assert!(corpus.rows().is_empty());
}

#[test]
fn a_later_change_for_the_path_brings_the_task_back_and_clears_the_item() {
    let mut corpus = plan_corpus(&[at("T-01", "ready", ROOT)], &[ROOT]);
    let item = invalid_item("arch/T-01.md", ROOT, "conflict markers");
    corpus.apply_invalid("plan", &item).unwrap();

    let fixed = at("T-01", "doing", ROOT);
    let event = changed_to("plan:T-01", ROOT, "doing", "v2");
    corpus.apply_changed("plan", &event, Some(fixed)).unwrap();

    assert_eq!(shown(&corpus, "plan:T-01"), [(ROOT.into(), "doing".into())]);
    assert!(corpus.invalid().is_empty());
}

#[test]
fn a_task_listed_twice_in_one_checkout_is_one_task_and_one_invalid_item() {
    let first = plan_task("T-01", "ready", ROOT, "arch/a/T-01.md");
    let second = plan_task("T-01", "doing", ROOT, "arch/b/T-01.md");
    let mut corpus = Corpus::default();
    corpus.declare_source("plan", &plan_init());
    let list = ListResult {
        tasks: vec![first, second],
        collections: vec![],
        invalid: vec![],
        next_cursor: None,
    };
    corpus.load("plan", Some(ROOT), list).unwrap();

    assert_eq!(corpus.rows().len(), 1);
    assert_eq!(
        corpus.rows()[0].task.path.as_deref(),
        Some("arch/a/T-01.md")
    );
    let invalid = corpus.invalid();
    assert_eq!(invalid.len(), 1);
    assert_eq!(invalid[0].item.path, "arch/b/T-01.md");
}

// --- incremental updates ---

#[test]
fn a_changed_notification_for_a_held_copy_updates_its_state_without_the_task() {
    let mut corpus = plan_corpus(&[at("T-01", "ready", ROOT)], &[ROOT]);
    let event = changed_to("plan:T-01", ROOT, "doing", "v2");
    assert_eq!(
        corpus.apply_changed("plan", &event, None).unwrap(),
        Applied::Done
    );

    let task = corpus.rows()[0].task;
    assert_eq!(task.state, state("doing"));
    assert_eq!(task.version, "v2");
}

#[test]
fn a_changed_notification_for_a_task_the_corpus_does_not_hold_asks_for_the_task() {
    let mut corpus = plan_corpus(&[], &[ROOT]);
    let event = changed_to("plan:T-09", ROOT, "ready", "v1");
    assert_eq!(
        corpus.apply_changed("plan", &event, None).unwrap(),
        Applied::NeedsTask
    );
    assert!(corpus.rows().is_empty());

    let task = at("T-09", "ready", ROOT);
    assert_eq!(
        corpus.apply_changed("plan", &event, Some(task)).unwrap(),
        Applied::Done
    );
    assert_eq!(corpus.rows().len(), 1);
}

#[test]
fn a_changed_notification_with_no_new_state_removes_that_copy() {
    let tasks = [at("T-01", "ready", ROOT), at("T-01", "doing", WS_A)];
    let mut corpus = plan_corpus(&tasks, &[ROOT, WS_A]);
    let mut event = changed_to("plan:T-01", WS_A, "doing", "v2");
    event.to = None;
    corpus.apply_changed("plan", &event, None).unwrap();
    assert_eq!(corpus.get("plan:T-01").copies.len(), 1);
    assert!(corpus.divergences().is_empty());
}

#[test]
fn a_removed_notification_removes_one_checkouts_copy_and_the_last_copy_ends_the_task() {
    let tasks = [at("T-01", "ready", ROOT), at("T-01", "doing", WS_A)];
    let mut corpus = plan_corpus(&tasks, &[ROOT, WS_A]);
    let removed = |checkout: Option<&str>| TaskRemoved {
        key: "plan:T-01".into(),
        cause: RemovedCause::Deleted,
        checkout: checkout.map(Into::into),
    };
    corpus.apply_removed("plan", &removed(Some(WS_A))).unwrap();
    assert_eq!(corpus.get("plan:T-01").copies.len(), 1);
    corpus.apply_removed("plan", &removed(Some(ROOT))).unwrap();
    assert!(matches!(corpus.lookup("plan:T-01"), Found::None));
    assert!(corpus.rows().is_empty());
}

#[test]
fn a_source_that_was_never_declared_is_refused_by_name() {
    let mut corpus = Corpus::default();
    let error = corpus.load("nope", None, list_of(&[], ROOT)).unwrap_err();
    assert_eq!(error, CorpusError::UnknownSource("nope".into()));
    assert!(error.to_string().contains("nope"));
    assert!(corpus.mark_stale("nope", "x").is_err());
}

#[test]
fn a_load_replaces_the_checkouts_earlier_list_and_leaves_the_other_checkouts() {
    let tasks = [at("T-01", "ready", ROOT), at("T-02", "ready", WS_A)];
    let mut corpus = plan_corpus(&tasks, &[ROOT, WS_A]);
    corpus
        .load(
            "plan",
            Some(ROOT),
            list_of(&[at("T-03", "ready", ROOT)], ROOT),
        )
        .unwrap();
    let keys: Vec<_> = corpus
        .rows()
        .iter()
        .map(|row| row.task.key.clone())
        .collect();
    assert_eq!(keys, ["plan:T-02", "plan:T-03"]);
}

// --- idempotence ---

fn assert_twice_is_once(base: &[Task], apply: impl Fn(&mut Corpus) -> Result<(), CorpusError>) {
    let mut once = plan_corpus(base, &[ROOT, WS_A]);
    apply(&mut once).unwrap();
    let mut twice = once.clone();
    apply(&mut twice).unwrap();
    assert_eq!(snapshot(&once), snapshot(&twice));
    assert_ne!(
        snapshot(&once),
        snapshot(&plan_corpus(base, &[ROOT, WS_A])),
        "the notification changed something, or the test shows nothing"
    );
}

#[test]
fn applying_a_changed_notification_twice_is_the_same_as_once() {
    let base = [at("T-01", "ready", ROOT), at("T-01", "ready", WS_A)];
    assert_twice_is_once(&base, |corpus| {
        let event = changed_to("plan:T-01", WS_A, "doing", "v2");
        corpus.apply_changed("plan", &event, None).map(|_| ())
    });
    assert_twice_is_once(&base, |corpus| {
        let event = changed_to("plan:T-02", ROOT, "doing", "v1");
        let task = at("T-02", "doing", ROOT);
        corpus.apply_changed("plan", &event, Some(task)).map(|_| ())
    });
}

#[test]
fn applying_a_renamed_notification_twice_is_the_same_as_once() {
    let base = [at("T-01", "ready", ROOT), at("T-01", "ready", WS_A)];
    for checkout in [None, Some(WS_A)] {
        assert_twice_is_once(&base, |corpus| {
            let rename = Rename {
                from: "plan:T-01".into(),
                to: "plan:T-05".into(),
                checkout: checkout.map(Into::into),
            };
            corpus.apply_renamed("plan", &rename)
        });
    }
}

#[test]
fn applying_a_removed_notification_twice_is_the_same_as_once() {
    let base = [at("T-01", "ready", ROOT), at("T-01", "ready", WS_A)];
    assert_twice_is_once(&base, |corpus| {
        let removed = TaskRemoved {
            key: "plan:T-01".into(),
            cause: RemovedCause::Deleted,
            checkout: Some(WS_A.into()),
        };
        corpus.apply_removed("plan", &removed)
    });
}

#[test]
fn applying_an_invalid_notification_twice_is_the_same_as_once() {
    let base = [at("T-01", "ready", ROOT), at("T-02", "ready", ROOT)];
    assert_twice_is_once(&base, |corpus| {
        corpus.apply_invalid(
            "plan",
            &invalid_item("arch/T-01.md", ROOT, "conflict markers"),
        )
    });
}

#[test]
fn applying_a_stale_notification_twice_is_the_same_as_once() {
    let base = [at("T-01", "ready", ROOT)];
    assert_twice_is_once(&base, |corpus| {
        let stale = SourceStale {
            reason: "could not read".into(),
            checkout: None,
        };
        corpus.mark_stale("plan", &stale.reason)
    });
}

// --- lookup ---

#[test]
fn a_key_a_long_id_and_an_alias_find_the_same_task() {
    let mut task = at("T-03", "done", ROOT);
    task.id = "T-03-the-long-name".into();
    task.aliases = vec!["three".into()];
    let corpus = plan_corpus(&[task], &[ROOT]);
    for query in [
        "plan:T-03",
        "T-03",
        "T-03-the-long-name",
        "three",
        "plan:three",
    ] {
        assert_eq!(
            corpus.lookup(query),
            Found::One("plan:T-03".into()),
            "{query}"
        );
    }
    assert_eq!(corpus.lookup("T-04"), Found::None);
}

#[test]
fn a_short_form_of_a_planning_id_finds_the_one_task_that_has_the_number() {
    let tasks = [
        at("ts-0012-ua", "ready", ROOT),
        at("ts-0013-ub", "ready", ROOT),
        at("ts-0012-qb", "ready", ROOT),
    ];
    let corpus = plan_corpus(&tasks, &[ROOT]);
    assert_eq!(corpus.lookup("13"), Found::One("plan:ts-0013-ub".into()));
    assert_eq!(corpus.lookup("12-ua"), Found::One("plan:ts-0012-ua".into()));
    assert_eq!(
        corpus.lookup("12"),
        Found::Ambiguous(vec!["plan:ts-0012-qb".into(), "plan:ts-0012-ua".into()])
    );
    let got = corpus.get("12");
    assert!(got.task.is_none());
    assert_eq!(got.ambiguous, ["plan:ts-0012-qb", "plan:ts-0012-ua"]);
}

#[test]
fn one_name_in_two_sources_is_ambiguous_until_the_key_says_which() {
    let mut corpus = two_sources();
    corpus.declare_source("other", &gh_init());
    let mut twin = gh_task(7, "closed");
    twin.key = "other:acme/edge#7".into();
    let list = ListResult {
        tasks: vec![twin],
        collections: vec![],
        invalid: vec![],
        next_cursor: None,
    };
    corpus.load("other", None, list).unwrap();
    assert!(matches!(corpus.lookup("acme/edge#7"), Found::Ambiguous(keys) if keys.len() == 2));
    assert_eq!(
        corpus.lookup("gh:acme/edge#7"),
        Found::One("gh:acme/edge#7".into())
    );
}

// --- task.list: filters and pages (ts-0116-wh) ---

fn keys_of(listed: &TaskListResult) -> Vec<String> {
    listed.tasks.iter().map(|task| task.key.clone()).collect()
}

fn five_tasks() -> Corpus {
    let tasks = [
        at("T-01", "done", ROOT),
        at("T-02", "ready", ROOT),
        with_link(at("T-03", "ready", ROOT), "blocked-by", "T-01"),
        with_link(at("T-04", "ready", ROOT), "blocked-by", "T-05"),
        at("T-05", "doing", ROOT),
    ];
    plan_corpus(&tasks, &[ROOT])
}

#[test]
fn a_list_with_no_filter_and_no_limit_gives_every_row_in_key_order() {
    let corpus = five_tasks();
    let listed = corpus.list(&ListFilter::default(), &Page::default());
    assert_eq!(
        keys_of(&listed),
        [
            "plan:T-01",
            "plan:T-02",
            "plan:T-03",
            "plan:T-04",
            "plan:T-05"
        ]
    );
    assert_eq!(listed.next_cursor, None);
}

#[test]
fn pages_follow_the_cursor_without_a_repeat_or_a_gap() {
    let corpus = five_tasks();
    let filter = ListFilter::default();
    let first = corpus.list(
        &filter,
        &Page {
            limit: Some(2),
            cursor: None,
        },
    );
    assert_eq!(keys_of(&first), ["plan:T-01", "plan:T-02"]);
    let cursor = first.next_cursor.clone().expect("more rows follow");
    let second = corpus.list(
        &filter,
        &Page {
            limit: Some(2),
            cursor: Some(&cursor),
        },
    );
    assert_eq!(keys_of(&second), ["plan:T-03", "plan:T-04"]);
    let cursor = second.next_cursor.clone().expect("one row follows");
    let third = corpus.list(
        &filter,
        &Page {
            limit: Some(2),
            cursor: Some(&cursor),
        },
    );
    assert_eq!(keys_of(&third), ["plan:T-05"]);
    assert_eq!(third.next_cursor, None);
}

#[test]
fn a_cursor_survives_a_change_to_the_row_it_names() {
    let mut corpus = five_tasks();
    let first = corpus.list(
        &ListFilter::default(),
        &Page {
            limit: Some(2),
            cursor: None,
        },
    );
    let cursor = first.next_cursor.clone().unwrap();
    corpus
        .apply_removed(
            "plan",
            &TaskRemoved {
                key: "plan:T-02".into(),
                cause: RemovedCause::Unknown,
                checkout: Some(ROOT.into()),
            },
        )
        .unwrap();
    let second = corpus.list(
        &ListFilter::default(),
        &Page {
            limit: Some(2),
            cursor: Some(&cursor),
        },
    );
    assert_eq!(keys_of(&second), ["plan:T-03", "plan:T-04"]);
}

#[test]
fn only_the_first_page_carries_the_board_wide_lists() {
    let mut corpus = five_tasks();
    corpus
        .apply_invalid("plan", &invalid_item("arch/bad.md", ROOT, "no frontmatter"))
        .unwrap();
    let filter = ListFilter::default();
    let first = corpus.list(
        &filter,
        &Page {
            limit: Some(2),
            cursor: None,
        },
    );
    assert_eq!(first.invalid.len(), 1);
    let cursor = first.next_cursor.clone().unwrap();
    let second = corpus.list(
        &filter,
        &Page {
            limit: Some(2),
            cursor: Some(&cursor),
        },
    );
    assert!(second.invalid.is_empty() && second.collections.is_empty());
    assert!(second.divergent.is_empty() && second.collisions.is_empty());
}

#[test]
fn the_ready_filter_keeps_what_the_ready_rule_keeps() {
    let corpus = five_tasks();
    let listed = corpus.list(
        &ListFilter {
            ready: true,
            ..Default::default()
        },
        &Page::default(),
    );
    assert_eq!(keys_of(&listed), ["plan:T-02", "plan:T-03"]);
}

#[test]
fn the_kind_filter_keeps_the_tasks_of_that_kind() {
    let mut init = plan_init();
    init.kinds = ["proposal", "story"]
        .iter()
        .map(|name| KindDecl {
            name: (*name).into(),
            statuses: vec![],
            transitions: vec![],
            ops: vec![],
            roles: vec![],
        })
        .collect();
    let mut one = at("T-01", "ready", ROOT);
    one.kind = Some("proposal".into());
    let mut two = at("T-02", "ready", ROOT);
    two.kind = Some("story".into());
    let mut corpus = Corpus::default();
    corpus.set_checkouts(checkouts());
    corpus.declare_source("plan", &init);
    corpus
        .load("plan", Some(ROOT), list_of(&[one, two], ROOT))
        .unwrap();
    let filter = ListFilter {
        kind: Some("story"),
        ..Default::default()
    };
    assert_eq!(
        keys_of(&corpus.list(&filter, &Page::default())),
        ["plan:T-02"]
    );
}

#[test]
fn the_checkout_filter_lists_the_copies_in_that_checkout() {
    let tasks = [
        at("T-01", "ready", ROOT),
        at("T-01", "doing", WS_A),
        at("T-02", "ready", ROOT),
        at("T-03", "specified", WS_A),
    ];
    let corpus = plan_corpus(&tasks, &[ROOT, WS_A]);
    let filter = ListFilter {
        checkout: Some(WS_A),
        ..Default::default()
    };
    let listed = corpus.list(&filter, &Page::default());
    let rows: Vec<_> = listed
        .tasks
        .iter()
        .map(|task| (task.key.as_str(), task.state.status.as_str()))
        .collect();
    assert_eq!(rows, [("plan:T-01", "doing"), ("plan:T-03", "specified")]);
}

#[test]
fn the_first_page_names_the_divergent_keys_the_collisions_and_the_stale_sources() {
    let tasks = [
        at("T-01", "ready", ROOT),
        at("T-01", "doing", WS_A),
        plan_task("T-02", "ready", ROOT, "arch/a/T-02.md"),
        plan_task("T-02", "ready", WS_A, "arch/b/T-02.md"),
    ];
    let mut corpus = plan_corpus(&tasks, &[ROOT, WS_A]);
    corpus.mark_stale("plan", "rate limited").unwrap();
    let listed = corpus.list(&ListFilter::default(), &Page::default());
    assert_eq!(listed.divergent, ["plan:T-01"]);
    assert_eq!(listed.collisions.len(), 1);
    assert_eq!(listed.collisions[0].short_id, "T-02");
    assert_eq!(listed.stale_sources.len(), 1);
    assert_eq!(listed.stale_sources[0].source, "plan");
    assert_eq!(listed.stale_sources[0].reason, "rate limited");
    assert!(
        !listed.refreshing,
        "the corpus does not know a listing is running"
    );
}

#[test]
fn a_renamed_key_is_a_former_key_of_the_new_one() {
    let mut corpus = plan_corpus(&[at("T-01", "ready", ROOT)], &[ROOT]);
    let rename = Rename {
        from: "plan:T-01".into(),
        to: "plan:T-05".into(),
        checkout: Some(ROOT.into()),
    };
    corpus.apply_renamed("plan", &rename).unwrap();
    assert_eq!(corpus.former_keys("plan:T-05"), ["plan:T-01"]);
    assert_eq!(
        corpus.former_keys("plan:T-01"),
        ["plan:T-01"],
        "the old key resolves to the new one"
    );
    assert!(corpus.former_keys("plan:T-09").is_empty());
}

#[test]
fn a_copy_is_found_by_key_and_checkout() {
    let tasks = [at("T-01", "ready", ROOT), at("T-01", "doing", WS_A)];
    let corpus = plan_corpus(&tasks, &[ROOT, WS_A]);
    let copy = corpus
        .copy("plan:T-01", Some(WS_A))
        .expect("a copy is there");
    assert_eq!(copy.state.status, "doing");
    assert!(corpus.copy("plan:T-01", Some(WS_B)).is_none());
    assert!(corpus.copy("plan:T-09", Some(ROOT)).is_none());
}

// --- the wire types ---

#[test]
fn the_output_serialises_to_task_list_result_and_task_get_result() {
    let mut with_body = at("T-01", "ready", ROOT);
    with_body.body = Some("the text".into());
    let tasks = [
        with_body,
        at("T-01", "doing", WS_A),
        at("T-02", "done", ROOT),
    ];
    let mut list = list_of(&tasks, ROOT);
    list.collections =
        serde_json::from_value(json!([{ "id": "plan:E-07", "title": "Epic 7" }])).unwrap();
    let mut corpus = plan_corpus(&tasks, &[WS_A]);
    corpus.load("plan", Some(ROOT), list).unwrap();

    let listed = corpus.list_result();
    assert_eq!(listed.tasks.len(), 2);
    assert!(
        listed.tasks.iter().all(|task| task.body.is_none()),
        "a list carries no body"
    );
    assert_eq!(listed.collections.len(), 1);
    let wire = serde_json::to_value(&listed).unwrap();
    let back: TaskListResult = serde_json::from_value(wire.clone()).unwrap();
    assert_eq!(serde_json::to_value(&back).unwrap(), wire);

    let got = corpus.get("plan:T-01");
    assert_eq!(got.task.as_ref().unwrap().body.as_deref(), Some("the text"));
    assert_eq!(got.copies.len(), 2);
    assert!(got.runs.is_empty() && got.latest_run.is_none());
    let wire = serde_json::to_value(&got).unwrap();
    let back: TaskGetResult = serde_json::from_value(wire.clone()).unwrap();
    assert_eq!(serde_json::to_value(&back).unwrap(), wire);
}

#[test]
fn a_lookup_of_nothing_is_an_answer_with_no_task() {
    let corpus = plan_corpus(&[], &[ROOT]);
    let got = corpus.get("plan:T-01");
    assert!(got.task.is_none() && got.ambiguous.is_empty() && got.copies.is_empty());
}

#[test]
fn the_rows_are_in_key_order_whatever_order_the_lists_came_in() {
    let tasks = [
        at("T-03", "ready", ROOT),
        at("T-01", "ready", WS_A),
        at("T-02", "ready", ROOT),
    ];
    let corpus = plan_corpus(&tasks, &all_checkouts());
    let keys: Vec<_> = corpus
        .rows()
        .iter()
        .map(|row| row.task.key.clone())
        .collect();
    assert_eq!(keys, ["plan:T-01", "plan:T-02", "plan:T-03"]);
    let reversed = [
        at("T-02", "ready", ROOT),
        at("T-01", "ready", WS_A),
        at("T-03", "ready", ROOT),
    ];
    assert_eq!(
        snapshot(&corpus),
        snapshot(&plan_corpus(&reversed, &all_checkouts()))
    );
}

// --- caps (ts-0117): a task whose lists or strings pass 0.9's caps is an invalid item ---

fn ready_task(short: &str) -> Task {
    at(short, "ready", ROOT)
}

fn capped_corpus(tasks: &[Task]) -> Corpus {
    plan_corpus(tasks, &[ROOT])
}

#[test]
fn a_task_with_1001_labels_is_an_invalid_item_with_the_reason_and_is_not_listed() {
    let mut heavy = ready_task("T-02");
    heavy.labels = (0..1001).map(|index| format!("l{index}")).collect();
    let corpus = capped_corpus(&[ready_task("T-01"), heavy]);

    let keys: Vec<_> = corpus
        .rows()
        .iter()
        .map(|row| row.task.key.clone())
        .collect();
    assert_eq!(keys, ["plan:T-01"]);
    let invalid = corpus.invalid();
    assert_eq!(invalid.len(), 1);
    assert_eq!(invalid[0].source, "plan");
    assert_eq!(invalid[0].item.path, "arch/T-02.md");
    assert_eq!(invalid[0].item.checkout.as_deref(), Some(ROOT));
    assert!(invalid[0].item.reason.contains("`labels`") && invalid[0].item.reason.contains("1001"));
}

#[test]
fn a_task_with_a_4097_byte_title_is_an_invalid_item_and_a_4096_byte_title_is_a_task() {
    let mut long = ready_task("T-02");
    long.title = "t".repeat(4097);
    let mut limit = ready_task("T-03");
    limit.title = "t".repeat(4096);
    let corpus = capped_corpus(&[long, limit]);

    let keys: Vec<_> = corpus
        .rows()
        .iter()
        .map(|row| row.task.key.clone())
        .collect();
    assert_eq!(keys, ["plan:T-03"]);
    let invalid = corpus.invalid();
    assert_eq!(invalid.len(), 1);
    assert!(invalid[0].item.reason.contains("`title`") && invalid[0].item.reason.contains("4097"));
}

#[test]
fn an_over_cap_task_is_listed_with_the_sources_other_invalid_items() {
    let mut heavy = ready_task("T-02");
    heavy.collections = (0..1001).map(|index| format!("c{index}")).collect();
    let mut list = list_of(&[ready_task("T-01"), heavy], ROOT);
    list.invalid
        .push(invalid_item("arch/broken.md", ROOT, "conflict markers"));
    let mut corpus = Corpus::default();
    corpus.declare_source("plan", &plan_init());
    corpus.load("plan", Some(ROOT), list).unwrap();

    let paths: Vec<_> = corpus
        .invalid()
        .iter()
        .map(|entry| entry.item.path.clone())
        .collect();
    assert_eq!(paths, ["arch/T-02.md", "arch/broken.md"]);
    assert_eq!(
        corpus.list_result().invalid.len(),
        2,
        "the list answer carries both"
    );
}

#[test]
fn a_change_that_brings_an_over_cap_task_makes_it_invalid_and_a_fixed_one_brings_it_back() {
    let mut corpus = capped_corpus(&[ready_task("T-01")]);
    let mut heavy = ready_task("T-01");
    heavy.labels = (0..1001).map(|index| format!("l{index}")).collect();
    let event = changed_to("plan:T-01", ROOT, "ready", "v2");
    assert_eq!(
        corpus.apply_changed("plan", &event, Some(heavy)).unwrap(),
        Applied::Done
    );

    assert!(corpus.rows().is_empty(), "the copy that was held is gone");
    assert_eq!(corpus.invalid().len(), 1);

    let fixed = ready_task("T-01");
    let event = changed_to("plan:T-01", ROOT, "ready", "v3");
    corpus.apply_changed("plan", &event, Some(fixed)).unwrap();
    assert_eq!(corpus.rows().len(), 1);
    assert!(
        corpus.invalid().is_empty(),
        "the path is fixed, so the item is cleared"
    );
}
