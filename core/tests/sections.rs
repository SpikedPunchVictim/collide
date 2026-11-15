//! Tests for the section rules (ts-0049; 0.10, "A proposal's features"): which two names are one section, which are
//! near-duplicates, and how the sections in use are counted, ordered and grouped.

use collide_core::item::Feature;
use collide_core::schema;
use collide_core::section::{
    NearDuplicateGroup, ProposalFeatures, SectionUse, Sections, near_duplicate, same_section,
    section_uses,
};
use serde_json::{Value, json};

// One-section matching.

#[test]
fn names_that_differ_only_in_case_are_one_section() {
    assert!(same_section("Packs", "packs"));
}

#[test]
fn names_that_differ_only_in_surrounding_spaces_are_one_section() {
    assert!(same_section("  Packs ", "Packs"));
}

#[test]
fn a_space_inside_a_name_still_tells_two_names_apart() {
    assert!(!same_section("Task view", "Taskview"));
}

#[test]
fn a_plural_is_another_section() {
    assert!(!same_section("Pack", "Packs"));
}

#[test]
fn two_empty_names_are_one_section() {
    assert!(same_section("", "  "));
}

// The near-duplicate rule.

#[test]
fn a_plural_and_its_singular_are_near_duplicates() {
    assert!(near_duplicate("Pack", "packs"));
}

#[test]
fn a_hyphen_and_a_space_do_not_tell_names_apart() {
    assert!(near_duplicate("Task view", "task-view"));
}

#[test]
fn an_underscore_does_not_tell_names_apart() {
    assert!(near_duplicate("task_view", "Task view"));
}

#[test]
fn only_one_final_s_is_dropped() {
    assert!(!near_duplicate("Pack", "Packss"));
}

#[test]
fn an_s_that_is_not_final_is_kept() {
    assert!(!near_duplicate("Tasks view", "Task view"));
}

#[test]
fn different_words_are_not_near_duplicates() {
    assert!(!near_duplicate("Daemon", "Tasks"));
}

#[test]
fn names_that_are_one_section_are_near_duplicates() {
    assert!(near_duplicate("Packs", "packs"));
}

#[test]
fn a_name_of_one_letter_matches_its_case_and_its_plural() {
    assert!(near_duplicate("A", "a"));
    assert!(near_duplicate("a", "As"));
}

#[test]
fn a_name_of_one_letter_is_not_near_another_letter() {
    assert!(!near_duplicate("a", "b"));
}

#[test]
fn s_alone_keeps_its_s_so_it_is_not_taken_for_an_empty_name() {
    assert!(!near_duplicate("s", "-"));
    assert!(!near_duplicate("s", "a"));
}

#[test]
fn s_alone_matches_itself_in_any_case_and_double_s() {
    assert!(near_duplicate("s", "S"));
    assert!(near_duplicate("s", "ss"));
}

#[test]
fn bus_and_bu_are_grouped_which_is_a_known_false_match() {
    assert!(near_duplicate("Bus", "Bu"));
}

#[test]
fn names_of_only_separators_are_near_nothing_but_themselves() {
    assert!(!near_duplicate("-", "_"));
    assert!(!near_duplicate(" - ", ""));
}

// Counting, ordering and grouping.

fn feature(number: u32, section: &str) -> Feature {
    Feature {
        id: format!("F{number}"),
        title: format!("Feature {number}"),
        section: section.to_string(),
        summary: String::new(),
    }
}

fn features(sections: &[&str]) -> Vec<Feature> {
    sections
        .iter()
        .enumerate()
        .map(|(index, section)| feature(index as u32 + 1, section))
        .collect()
}

fn proposal<'a>(id: &'a str, features: &'a [Feature]) -> ProposalFeatures<'a> {
    ProposalFeatures {
        proposal: id,
        features,
    }
}

/// `(name, features, proposals)` of each section in order.
fn shown(result: &Sections) -> Vec<(&str, u32, u32)> {
    result
        .sections
        .iter()
        .map(|s| (s.name.as_str(), s.feature_count, s.proposal_count))
        .collect()
}

#[test]
fn a_section_used_in_two_proposals_is_one_section_that_names_both() {
    let (a, b) = (features(&["Packs", "Packs"]), features(&["Packs"]));
    let result = section_uses(&[proposal("pr-0002-bb", &b), proposal("pr-0001-aa", &a)]);
    assert_eq!(
        result.sections,
        [SectionUse {
            name: "Packs".to_string(),
            feature_count: 3,
            proposal_count: 2,
            proposals: vec!["pr-0001-aa".to_string(), "pr-0002-bb".to_string()],
        }]
    );
}

#[test]
fn packs_and_packs_in_another_case_are_one_section() {
    let (a, b) = (features(&["Packs"]), features(&["packs"]));
    let result = section_uses(&[proposal("pr-0001-aa", &a), proposal("pr-0002-bb", &b)]);
    assert_eq!(shown(&result), [("Packs", 2, 2)]);
}

#[test]
fn a_section_is_shown_in_the_spelling_most_of_its_features_use() {
    let a = features(&["packs", "Packs", "Packs"]);
    let b = features(&["packs"]);
    let c = features(&["packs"]);
    // `packs` has three features and `Packs` two, though `Packs` is in the lowest proposal.
    let result = section_uses(&[
        proposal("pr-0001-aa", &a),
        proposal("pr-0002-bb", &b),
        proposal("pr-0003-cc", &c),
    ]);
    assert_eq!(shown(&result), [("packs", 5, 3)]);
}

#[test]
fn on_a_tie_the_spelling_comes_from_the_lowest_proposal_id() {
    let (low, high) = (features(&["packs"]), features(&["Packs"]));
    // Given in the order that would favour the other spelling.
    let result = section_uses(&[proposal("pr-0009-zz", &high), proposal("pr-0002-bb", &low)]);
    assert_eq!(shown(&result), [("packs", 2, 2)]);
}

#[test]
fn a_tie_inside_one_proposal_takes_the_spelling_that_sorts_first() {
    let one = features(&["packs", "Packs"]);
    let result = section_uses(&[proposal("pr-0001-aa", &one)]);
    assert_eq!(shown(&result), [("Packs", 2, 1)]);
}

#[test]
fn a_spelling_is_taken_without_its_surrounding_spaces() {
    let one = features(&[" Packs "]);
    let result = section_uses(&[proposal("pr-0001-aa", &one)]);
    assert_eq!(shown(&result), [("Packs", 1, 1)]);
}

#[test]
fn sections_are_sorted_by_feature_count_highest_first_then_by_name() {
    let one = features(&["Beta", "Alpha", "Gamma", "Gamma", "Delta", "Delta", "Delta"]);
    let result = section_uses(&[proposal("pr-0001-aa", &one)]);
    assert_eq!(
        shown(&result),
        [
            ("Delta", 3, 1),
            ("Gamma", 2, 1),
            ("Alpha", 1, 1),
            ("Beta", 1, 1)
        ]
    );
}

#[test]
fn the_name_order_ignores_case() {
    let one = features(&["beta", "Alpha", "Cat"]);
    let result = section_uses(&[proposal("pr-0001-aa", &one)]);
    assert_eq!(
        shown(&result),
        [("Alpha", 1, 1), ("beta", 1, 1), ("Cat", 1, 1)]
    );
}

#[test]
fn the_empty_section_comes_last_whatever_its_count() {
    let one = features(&["", "", "", "Packs"]);
    let result = section_uses(&[proposal("pr-0001-aa", &one)]);
    assert_eq!(shown(&result), [("Packs", 1, 1), ("", 3, 1)]);
}

#[test]
fn a_proposal_with_no_features_is_in_no_section() {
    let (some, none) = (features(&["Packs"]), features(&[]));
    let result = section_uses(&[proposal("pr-0001-aa", &some), proposal("pr-0002-bb", &none)]);
    assert_eq!(result.sections[0].proposals, ["pr-0001-aa"]);
}

#[test]
fn no_proposal_gives_no_sections_and_no_groups() {
    assert_eq!(
        section_uses(&[]),
        Sections {
            sections: vec![],
            near_duplicates: vec![]
        }
    );
}

fn groups(result: &Sections) -> Vec<Vec<&str>> {
    result
        .near_duplicates
        .iter()
        .map(|group| group.names.iter().map(String::as_str).collect())
        .collect()
}

#[test]
fn a_singular_and_a_plural_section_are_one_group() {
    let one = features(&["Pack", "packs", "packs"]);
    let result = section_uses(&[proposal("pr-0001-aa", &one)]);
    assert_eq!(groups(&result), [["packs", "Pack"]]);
}

#[test]
fn a_space_and_a_hyphen_make_a_group() {
    let one = features(&["Task view", "task-view", "Daemon", "Tasks"]);
    let result = section_uses(&[proposal("pr-0001-aa", &one)]);
    assert_eq!(groups(&result), [["Task view", "task-view"]]);
}

#[test]
fn daemon_and_tasks_are_not_grouped() {
    let one = features(&["Daemon", "Tasks"]);
    let result = section_uses(&[proposal("pr-0001-aa", &one)]);
    assert_eq!(groups(&result), Vec::<Vec<&str>>::new());
}

#[test]
fn two_spellings_of_one_section_are_not_a_group_of_their_own() {
    let one = features(&["Packs", "packs"]);
    let result = section_uses(&[proposal("pr-0001-aa", &one)]);
    assert_eq!(groups(&result), Vec::<Vec<&str>>::new());
}

#[test]
fn three_near_duplicate_sections_are_one_group_in_section_order() {
    let one = features(&["Pack", "Packs", "Packs", "pack_s", "pack_s", "pack_s"]);
    let result = section_uses(&[proposal("pr-0001-aa", &one)]);
    // Three sections (the names differ beyond case), and all three reduce to `pack`.
    assert_eq!(groups(&result), [["pack_s", "Packs", "Pack"]]);
}

#[test]
fn bus_and_bu_are_one_group_a_known_false_match() {
    let one = features(&["Bus", "Bu"]);
    let result = section_uses(&[proposal("pr-0001-aa", &one)]);
    assert_eq!(groups(&result), [["Bu", "Bus"]]);
}

#[test]
fn the_empty_section_is_in_no_group() {
    let one = features(&["", "-", "_"]);
    let result = section_uses(&[proposal("pr-0001-aa", &one)]);
    assert_eq!(groups(&result), Vec::<Vec<&str>>::new());
}

#[test]
fn groups_come_in_the_order_of_their_first_section() {
    let one = features(&["Beta", "Beta", "Beta", "Alpha", "Alphas", "Beta"]);
    let two = features(&["Gamma", "Gammas", "Gammas", "Gammas", "Gammas", "Gammas"]);
    let result = section_uses(&[proposal("pr-0001-aa", &one), proposal("pr-0002-bb", &two)]);
    assert_eq!(groups(&result), [["Gammas", "Gamma"], ["Alpha", "Alphas"]]);
}

// The shape of `collide sections --json`.

#[test]
fn the_json_shape_names_its_keys_in_camel_case() {
    let (a, b) = (features(&["Pack"]), features(&["packs"]));
    let result = section_uses(&[proposal("pr-0001-aa", &a), proposal("pr-0002-bb", &b)]);
    assert_eq!(
        serde_json::to_value(&result).unwrap(),
        json!({
            "sections": [
                { "name": "Pack", "featureCount": 1, "proposalCount": 1, "proposals": ["pr-0001-aa"] },
                { "name": "packs", "featureCount": 1, "proposalCount": 1, "proposals": ["pr-0002-bb"] },
            ],
            "nearDuplicates": [{ "names": ["Pack", "packs"] }],
        })
    );
}

#[test]
fn the_json_shape_refuses_a_key_it_does_not_know() {
    let group = json!({ "names": ["a"], "extra": 1 });
    assert!(serde_json::from_value::<NearDuplicateGroup>(group).is_err());
}

fn schema_keys(name: &str) -> Vec<String> {
    let rendered = schema::registry().render();
    let file = rendered
        .get(&format!("{name}.output.json"))
        .unwrap_or_else(|| panic!("{name} is not in the registry"));
    let document: Value = serde_json::from_str(file).unwrap();
    let mut keys: Vec<String> = document["properties"]
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect();
    keys.sort();
    keys
}

#[test]
fn the_section_types_are_in_the_schema() {
    assert_eq!(schema_keys("Sections"), ["nearDuplicates", "sections"]);
}
