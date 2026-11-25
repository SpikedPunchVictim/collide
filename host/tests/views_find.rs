//! Finding a view by its name and the built-in view in the host (ts-0135-cx; 0.10, Views): the whole name first, then
//! the part after the last `/`; a tie goes to the pack that supplies a source of the board, and with none it is an
//! error that lists the full names. No daemon runs and no process starts.

use collide_core::owner::ViewSource;
use collide_core::view::View;
use collide_host::views::{Candidate, Loaded, Lookup, ViewCache, builtin_candidates, find_view};

fn view(name: &str) -> View {
    serde_json::from_value(serde_json::json!({ "name": name, "where": "status == 'ready'" }))
        .unwrap()
}

fn pack(name: &str) -> ViewSource {
    ViewSource::Pack { pack: name.into() }
}

fn candidate(name: &str, source: ViewSource) -> Candidate {
    Candidate {
        view: view(name),
        source,
        error: None,
    }
}

fn loaded(candidates: Vec<Candidate>) -> Vec<Loaded> {
    ViewCache::new(None).assemble(candidates, 10)
}

fn supplying(packs: &[&str]) -> Vec<String> {
    packs.iter().map(|pack| (*pack).to_string()).collect()
}

fn found<'a>(views: &'a [Loaded], name: &str, packs: &[&str]) -> &'a str {
    &find_view(views, name, &supplying(packs))
        .unwrap_or_else(|lookup| panic!("{name}: {}", lookup.message()))
        .view
        .name
}

fn three_views() -> Vec<Loaded> {
    loaded(vec![
        candidate("mine", ViewSource::Personal {}),
        candidate("collide/spikes", pack("collide")),
        candidate("collide/decisions-needed", pack("collide")),
    ])
}

#[test]
fn the_whole_name_is_found_first() {
    let views = three_views();
    assert_eq!(found(&views, "collide/spikes", &[]), "collide/spikes");
    assert_eq!(found(&views, "mine", &[]), "mine");
}

#[test]
fn a_bare_name_finds_the_one_view_that_has_it() {
    let views = three_views();
    assert_eq!(found(&views, "spikes", &[]), "collide/spikes");
    assert_eq!(
        found(&views, "decisions-needed", &[]),
        "collide/decisions-needed"
    );
}

#[test]
fn a_whole_name_wins_over_the_bare_name_of_a_pack_view() {
    let views = loaded(vec![
        candidate("spikes", ViewSource::Repo {}),
        candidate("collide/spikes", pack("collide")),
    ]);
    assert_eq!(found(&views, "spikes", &["collide"]), "spikes");
}

#[test]
fn two_packs_with_one_key_and_neither_supplying_a_source_is_an_error_naming_both() {
    let views = loaded(vec![
        candidate("alpha/mine", pack("alpha")),
        candidate("beta/mine", pack("beta")),
    ]);
    let lookup = find_view(&views, "mine", &supplying(&["collide"])).unwrap_err();
    assert_eq!(
        lookup,
        Lookup::Ambiguous {
            name: "mine".into(),
            names: vec!["alpha/mine".into(), "beta/mine".into()],
        }
    );
    let text = lookup.message();
    assert!(
        text.contains("alpha/mine") && text.contains("beta/mine"),
        "{text}"
    );
}

#[test]
fn two_packs_that_both_supply_a_source_are_an_error_too() {
    let views = loaded(vec![
        candidate("alpha/mine", pack("alpha")),
        candidate("beta/mine", pack("beta")),
    ]);
    let lookup = find_view(&views, "mine", &supplying(&["alpha", "beta"])).unwrap_err();
    assert!(matches!(lookup, Lookup::Ambiguous { .. }), "{lookup:?}");
}

#[test]
fn the_pack_that_supplies_a_source_of_the_board_wins_a_tie() {
    let views = loaded(vec![
        candidate("collide/spikes", pack("collide")),
        candidate("quay/spikes", pack("quay")),
    ]);
    assert_eq!(found(&views, "spikes", &["quay"]), "quay/spikes");
    assert_eq!(found(&views, "spikes", &["collide"]), "collide/spikes");
}

#[test]
fn a_name_no_view_has_lists_every_view() {
    let views = three_views();
    let lookup = find_view(&views, "nope", &[]).unwrap_err();
    assert_eq!(
        lookup,
        Lookup::Missing {
            name: "nope".into(),
            names: vec![
                "mine".into(),
                "collide/spikes".into(),
                "collide/decisions-needed".into()
            ],
        }
    );
    let text = lookup.message();
    assert!(
        text.contains("\"nope\"") && text.contains("collide/spikes"),
        "{text}"
    );
}

#[test]
fn a_part_of_a_name_that_is_not_after_the_last_slash_finds_nothing() {
    let views = three_views();
    assert!(find_view(&views, "collide", &[]).is_err());
    assert!(find_view(&views, "collide/spike", &[]).is_err());
    assert!(find_view(&views, "pikes", &[]).is_err());
}

#[test]
fn the_built_in_ready_view_is_first_loads_with_no_error_and_cannot_be_replaced_by_a_file() {
    let mut candidates = builtin_candidates();
    candidates.push(candidate("ready", ViewSource::Personal {}));
    let views = loaded(candidates);
    assert_eq!(views.len(), 3);
    assert_eq!(views[0].source, ViewSource::Builtin {});
    assert_eq!(views[0].error, None);
    assert!(
        views[0]
            .compiled()
            .is_some_and(|compiled| compiled.is_builtin())
    );
    let shadowed = views[2]
        .error
        .as_deref()
        .expect("the personal copy is refused");
    assert!(shadowed.contains("built-in"), "{shadowed}");
    assert_eq!(found(&views, "ready", &[]), "ready");
    assert_eq!(
        find_view(&views, "ready", &[]).unwrap().source,
        ViewSource::Builtin {}
    );
}

#[test]
fn the_built_in_views_are_ready_then_bottlenecks_and_both_load_with_no_error() {
    let views = loaded(builtin_candidates());
    let names: Vec<&str> = views.iter().map(|each| each.view.name.as_str()).collect();
    assert_eq!(names, ["ready", "bottlenecks"]);
    for each in &views {
        assert_eq!(each.source, ViewSource::Builtin {}, "{}", each.view.name);
        assert_eq!(each.error, None, "{}", each.view.name);
        assert!(
            each.compiled()
                .is_some_and(|compiled| compiled.is_builtin())
        );
        assert_eq!(
            each.compiled()
                .map(|compiled| compiled.view().name.as_str()),
            Some(each.view.name.as_str())
        );
    }
}

#[test]
fn a_file_or_pack_view_named_bottlenecks_is_listed_with_an_error_naming_the_built_in() {
    let mut candidates = builtin_candidates();
    candidates.push(candidate("bottlenecks", ViewSource::Personal {}));
    candidates.push(candidate("bottlenecks", ViewSource::Repo {}));
    let views = loaded(candidates);
    assert_eq!(views.len(), 4);
    assert_eq!(views[1].source, ViewSource::Builtin {});
    assert_eq!(views[1].error, None);
    for shadowed in &views[2..] {
        let error = shadowed.error.as_deref().expect("the copy is refused");
        assert!(
            error.contains("`bottlenecks`") && error.contains("built-in"),
            "{error}"
        );
        assert!(shadowed.compiled().is_none());
    }
    assert_eq!(
        find_view(&views, "bottlenecks", &[]).unwrap().source,
        ViewSource::Builtin {}
    );
}
