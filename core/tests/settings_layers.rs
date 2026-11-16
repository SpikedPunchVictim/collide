//! Tests for the four settings layers of 0.7 as a pure function (ts-0098-nx): defaults, the user's files, the repo's
//! file with the `overridable` flip, and the run's profile, with the layer asserted in every case. Also the refusals
//! (per-user keys in the repo, `repoOnly` keys in the user's files and profiles, secrets, values that fail the
//! schema) and the lookup of a profile by name. The layers are small in-memory values; nothing is read from disk.

use collide_core::owner::{ConfigEntry, SettingLayer};
use collide_core::pack::SchemaObject;
use collide_core::profile::{Profile, ProfileLookupError, ProfileSource, find_profile};
use collide_core::settings::ConfigFile;
use collide_core::settings_layers::{
    LayerInputs, PackSettings, ProfileUse, SettingProblem, Settings, settings_layers,
};
use collide_core::yaml::load_yaml;
use serde_json::{Value, json};
use std::collections::BTreeMap;

fn file(text: &str) -> ConfigFile {
    load_yaml(text.as_bytes(), "config.yaml").unwrap_or_else(|error| panic!("{error}\n{text}"))
}

fn profile(text: &str) -> Profile {
    load_yaml(text.as_bytes(), "profile.yaml").unwrap_or_else(|error| panic!("{error}\n{text}"))
}

fn schemas(pairs: &[(&str, Value)]) -> BTreeMap<String, SchemaObject> {
    pairs
        .iter()
        .map(|(key, schema)| (key.to_string(), SchemaObject(schema.clone())))
        .collect()
}

/// The settings of the pack `ferry` in these tests.
fn ferry() -> BTreeMap<String, SchemaObject> {
    schemas(&[
        (
            "lanePool",
            json!({ "type": "integer", "minimum": 1, "default": 2, "overridable": true }),
        ),
        ("greeting", json!({ "type": "string", "default": "hello" })),
        (
            "layout",
            json!({ "type": "string", "repoOnly": true, "default": "plain" }),
        ),
        ("notifyHook", json!({ "type": "string", "secret": true })),
        ("mode", json!({ "enum": ["fast", "slow"] })),
        ("depth", json!({ "type": "integer", "minimum": 1 })),
    ])
}

/// A second pack, to tell one pack's keys from another's.
fn other() -> BTreeMap<String, SchemaObject> {
    schemas(&[("color", json!({ "type": "string", "default": "red" }))])
}

/// Everything one run of the function reads.
struct World {
    ferry: BTreeMap<String, SchemaObject>,
    other: BTreeMap<String, SchemaObject>,
    user: ConfigFile,
    board: ConfigFile,
    repo: ConfigFile,
    profile: Option<(String, ProfileSource, Profile)>,
}

impl World {
    fn new() -> World {
        World {
            ferry: ferry(),
            other: other(),
            user: ConfigFile::default(),
            board: ConfigFile::default(),
            repo: ConfigFile::default(),
            profile: None,
        }
    }

    fn user(mut self, text: &str) -> World {
        self.user = file(text);
        self
    }

    fn board(mut self, text: &str) -> World {
        self.board = file(text);
        self
    }

    fn repo(mut self, text: &str) -> World {
        self.repo = file(text);
        self
    }

    fn profile(mut self, source: ProfileSource, text: &str) -> World {
        self.profile = Some(("work".to_string(), source, profile(text)));
        self
    }

    fn run(&self) -> Settings {
        let packs = [
            PackSettings {
                pack: "ferry",
                config: &self.ferry,
            },
            PackSettings {
                pack: "other",
                config: &self.other,
            },
        ];
        settings_layers(&LayerInputs {
            packs: &packs,
            user: &self.user,
            user_board: &self.board,
            repo: &self.repo,
            profile: self
                .profile
                .as_ref()
                .map(|(name, source, profile)| ProfileUse {
                    name,
                    source,
                    profile,
                }),
        })
    }

    /// The entry for `key`; panics when there is none.
    fn entry(&self, key: &str) -> ConfigEntry {
        let settings = self.run();
        settings
            .entries
            .iter()
            .find(|entry| entry.key == key)
            .unwrap_or_else(|| panic!("no entry for {key}; keys: {:?}", keys(&settings)))
            .clone()
    }

    fn problems(&self) -> Vec<SettingProblem> {
        self.run().problems
    }
}

fn keys(settings: &Settings) -> Vec<&str> {
    settings
        .entries
        .iter()
        .map(|entry| entry.key.as_str())
        .collect()
}

fn texts(problems: &[SettingProblem]) -> String {
    problems
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n")
}

/// Asserts that the key has this value and layer.
fn assert_entry(world: &World, key: &str, value: Value, layer: SettingLayer) {
    let entry = world.entry(key);
    assert_eq!(entry.value, Some(value), "{key}: {entry:?}");
    assert_eq!(entry.layer, layer, "{key}: {entry:?}");
    assert!(!entry.secret, "{key}");
}

/// Asserts exactly one problem, about `key`, whose text holds every part.
fn assert_one_problem(world: &World, key: &str, parts: &[&str]) {
    let problems = world.problems();
    assert_eq!(problems.len(), 1, "{}", texts(&problems));
    assert_eq!(problems[0].key, key, "{}", problems[0]);
    for part in parts {
        assert!(
            problems[0].message.contains(part),
            "`{part}` missing from: {}",
            problems[0]
        );
    }
    assert!(
        problems[0].message.contains(key),
        "the key is not named: {}",
        problems[0]
    );
}

// ---- the defaults and the order of the entries ----

#[test]
fn with_no_file_at_all_every_key_has_its_default_and_says_default() {
    let world = World::new();
    assert_entry(&world, "disk.floor_gib", json!(5), SettingLayer::Default);
    assert_entry(
        &world,
        "claude.check_published",
        json!(true),
        SettingLayer::Default,
    );
    assert_entry(&world, "ferry.lanePool", json!(2), SettingLayer::Default);
    assert_entry(
        &world,
        "ferry.greeting",
        json!("hello"),
        SettingLayer::Default,
    );
    assert_entry(&world, "other.color", json!("red"), SettingLayer::Default);
    assert_eq!(world.problems(), vec![]);
}

#[test]
fn a_key_with_no_default_and_no_value_is_listed_with_no_value_and_says_default() {
    let world = World::new();
    for key in ["ask.agent", "ask.timeout_s", "ask.budget_usd", "ferry.mode"] {
        let entry = world.entry(key);
        assert_eq!(entry.value, None, "{key}");
        assert_eq!(entry.layer, SettingLayer::Default, "{key}");
        assert!(!entry.secret, "{key}");
    }
}

#[test]
fn the_entries_list_collides_keys_first_in_registry_order_then_each_pack_by_key() {
    let settings = World::new().run();
    assert_eq!(
        keys(&settings),
        [
            "ask.agent",
            "ask.timeout_s",
            "ask.budget_usd",
            "disk.floor_gib",
            "claude.check_published",
            "ferry.depth",
            "ferry.greeting",
            "ferry.lanePool",
            "ferry.layout",
            "ferry.mode",
            "ferry.notifyHook",
            "other.color",
        ]
    );
}

// ---- the user's layer and the repo's ----

#[test]
fn a_key_set_only_in_the_users_file_says_user() {
    let world = World::new().user("disk: { floor_gib: 3 }\nconfig: { ferry: { greeting: hi } }\n");
    assert_entry(&world, "disk.floor_gib", json!(3), SettingLayer::User);
    assert_entry(&world, "ferry.greeting", json!("hi"), SettingLayer::User);
}

#[test]
fn a_key_set_only_in_the_repos_file_says_repo() {
    let world =
        World::new().repo("ask: { budget_usd: 2.5 }\nconfig: { ferry: { greeting: team } }\n");
    assert_entry(&world, "ask.budget_usd", json!(2.5), SettingLayer::Repo);
    assert_entry(&world, "ferry.greeting", json!("team"), SettingLayer::Repo);
}

#[test]
fn a_key_set_in_both_resolves_to_the_repos_value_and_says_repo() {
    let world = World::new()
        .user("ask: { budget_usd: 1.0 }\nconfig: { ferry: { greeting: mine } }\n")
        .repo("ask: { budget_usd: 2.5 }\nconfig: { ferry: { greeting: team } }\n");
    assert_entry(&world, "ask.budget_usd", json!(2.5), SettingLayer::Repo);
    assert_entry(&world, "ferry.greeting", json!("team"), SettingLayer::Repo);
    assert_eq!(world.problems(), vec![]);
}

#[test]
fn an_overridable_key_set_in_both_resolves_to_the_users_value_and_says_user() {
    let world = World::new()
        .user(
            "disk: { floor_gib: 2 }\nask: { timeout_s: 30 }\nconfig: { ferry: { lanePool: 4 } }\n",
        )
        .repo(
            "disk: { floor_gib: 9 }\nask: { timeout_s: 600 }\nconfig: { ferry: { lanePool: 1 } }\n",
        );
    assert_entry(&world, "disk.floor_gib", json!(2), SettingLayer::User);
    assert_entry(&world, "ask.timeout_s", json!(30), SettingLayer::User);
    assert_entry(&world, "ferry.lanePool", json!(4), SettingLayer::User);
}

#[test]
fn an_overridable_key_set_only_in_the_repo_says_repo() {
    let world = World::new().repo("disk: { floor_gib: 9 }\nconfig: { ferry: { lanePool: 1 } }\n");
    assert_entry(&world, "disk.floor_gib", json!(9), SettingLayer::Repo);
    assert_entry(&world, "ferry.lanePool", json!(1), SettingLayer::Repo);
}

#[test]
fn the_users_value_for_the_flip_beats_the_repo_even_when_the_repo_set_it_lower() {
    let world = World::new()
        .user("disk: { floor_gib: 20 }\n")
        .repo("disk: { floor_gib: 1 }\n");
    assert_entry(&world, "disk.floor_gib", json!(20), SettingLayer::User);
}

#[test]
fn a_per_board_user_file_wins_over_the_global_user_file_and_both_say_user() {
    let world = World::new()
        .user("disk: { floor_gib: 2 }\nconfig: { ferry: { greeting: global, depth: 3 } }\n")
        .board("disk: { floor_gib: 7 }\nconfig: { ferry: { greeting: board } }\n");
    assert_entry(&world, "disk.floor_gib", json!(7), SettingLayer::User);
    assert_entry(&world, "ferry.greeting", json!("board"), SettingLayer::User);
    assert_entry(&world, "ferry.depth", json!(3), SettingLayer::User);
}

#[test]
fn the_repo_still_beats_the_per_board_user_file_for_a_key_that_is_not_overridable() {
    let world = World::new()
        .board("config: { ferry: { greeting: board } }\n")
        .repo("config: { ferry: { greeting: team } }\n");
    assert_entry(&world, "ferry.greeting", json!("team"), SettingLayer::Repo);
}

// ---- the profile, last ----

#[test]
fn a_repo_profile_is_the_top_layer_and_says_profile() {
    let world = World::new()
        .user("disk: { floor_gib: 2 }\n")
        .repo("config: { ferry: { greeting: team } }\n")
        .profile(
            ProfileSource::Repo,
            "config: { ferry: { greeting: quick, lanePool: 1 }, disk: { floor_gib: 8 } }\n",
        );
    assert_entry(
        &world,
        "ferry.greeting",
        json!("quick"),
        SettingLayer::Profile,
    );
    assert_entry(&world, "ferry.lanePool", json!(1), SettingLayer::Profile);
    assert_entry(&world, "disk.floor_gib", json!(8), SettingLayer::Profile);
    assert_eq!(world.problems(), vec![]);
}

#[test]
fn a_keys_a_profile_does_not_set_keep_the_layer_below() {
    let world = World::new()
        .user("config: { ferry: { depth: 4 } }\n")
        .profile(
            ProfileSource::Repo,
            "config: { ferry: { greeting: quick } }\n",
        );
    assert_entry(&world, "ferry.depth", json!(4), SettingLayer::User);
    assert_entry(&world, "ferry.lanePool", json!(2), SettingLayer::Default);
}

#[test]
fn a_user_profile_that_sets_an_overridable_key_loads_and_says_profile() {
    let world = World::new().repo("disk: { floor_gib: 9 }\n").profile(
        ProfileSource::User,
        "config: { ferry: { lanePool: 3 }, disk: { floor_gib: 2 }, ask: { timeout_s: 45 } }\n",
    );
    assert_entry(&world, "ferry.lanePool", json!(3), SettingLayer::Profile);
    assert_entry(&world, "disk.floor_gib", json!(2), SettingLayer::Profile);
    assert_entry(&world, "ask.timeout_s", json!(45), SettingLayer::Profile);
    assert_eq!(world.problems(), vec![]);
}

#[test]
fn a_user_profile_that_sets_a_key_not_marked_overridable_is_refused_naming_the_key() {
    let world = World::new()
        .repo("config: { ferry: { greeting: team } }\n")
        .profile(
            ProfileSource::User,
            "config: { ferry: { greeting: mine } }\n",
        );
    assert_one_problem(&world, "ferry.greeting", &["overridable", "work"]);
    assert_entry(&world, "ferry.greeting", json!("team"), SettingLayer::Repo);
}

#[test]
fn a_user_profile_that_sets_a_collide_key_not_marked_overridable_is_refused_naming_it() {
    let world = World::new().profile(ProfileSource::User, "config: { ask: { budget_usd: 5 } }\n");
    assert_one_problem(&world, "ask.budget_usd", &["overridable"]);
    assert_eq!(world.entry("ask.budget_usd").value, None);
}

#[test]
fn a_user_profile_with_one_overridable_key_and_one_other_applies_the_first_and_refuses_the_second()
{
    let world = World::new().profile(
        ProfileSource::User,
        "config: { ferry: { lanePool: 3, greeting: mine } }\n",
    );
    assert_one_problem(&world, "ferry.greeting", &["overridable"]);
    assert_entry(&world, "ferry.lanePool", json!(3), SettingLayer::Profile);
}

#[test]
fn a_user_profile_that_sets_a_top_level_key_it_may_not_is_refused_naming_it() {
    let world = World::new().profile(
        ProfileSource::User,
        "workflow: collide/review\nlaunch: { maxRounds: 1 }\n",
    );
    let problems = world.problems();
    assert_eq!(problems.len(), 2, "{}", texts(&problems));
    assert!(
        problems
            .iter()
            .any(|p| p.key == "workflow" && p.message.contains("`workflow`"))
    );
    assert!(
        problems
            .iter()
            .any(|p| p.key == "launch" && p.message.contains("`launch`"))
    );
}

#[test]
fn a_repo_only_key_in_a_user_profile_is_refused_naming_the_key() {
    let world = World::new().profile(
        ProfileSource::User,
        "config: { ferry: { layout: fancy } }\n",
    );
    assert_one_problem(&world, "ferry.layout", &["repoOnly"]);
    assert_entry(
        &world,
        "ferry.layout",
        json!("plain"),
        SettingLayer::Default,
    );
}

#[test]
fn a_pack_profile_that_sets_another_packs_key_is_refused_naming_the_key() {
    let world = World::new().profile(
        ProfileSource::Pack("ferry".into()),
        "config: { other: { color: blue }, ferry: { greeting: ahoy } }\n",
    );
    assert_one_problem(&world, "other.color", &["ferry"]);
    assert_entry(
        &world,
        "ferry.greeting",
        json!("ahoy"),
        SettingLayer::Profile,
    );
    assert_entry(&world, "other.color", json!("red"), SettingLayer::Default);
}

#[test]
fn a_pack_profile_that_sets_a_collide_key_is_refused() {
    let world = World::new().profile(
        ProfileSource::Pack("ferry".into()),
        "config: { disk: { floor_gib: 8 } }\n",
    );
    assert_one_problem(&world, "disk.floor_gib", &["ferry"]);
}

#[test]
fn a_repo_profile_may_set_a_repo_only_key_and_any_key_a_repo_file_may_set() {
    let world = World::new().profile(
        ProfileSource::Repo,
        "config: { ferry: { layout: fancy }, ask: { budget_usd: 3 } }\n",
    );
    assert_entry(
        &world,
        "ferry.layout",
        json!("fancy"),
        SettingLayer::Profile,
    );
    assert_entry(&world, "ask.budget_usd", json!(3), SettingLayer::Profile);
    assert_eq!(world.problems(), vec![]);
}

#[test]
fn a_repo_profile_that_sets_a_per_user_key_is_refused_naming_it() {
    let world = World::new().profile(
        ProfileSource::Repo,
        "config: { claude: { check_published: false }, ask: { agent: mine } }\n",
    );
    let problems = world.problems();
    assert_eq!(problems.len(), 2, "{}", texts(&problems));
    assert!(problems.iter().any(|p| p.key == "claude.check_published"));
    assert!(problems.iter().any(|p| p.key == "ask.agent"));
    assert_entry(
        &world,
        "claude.check_published",
        json!(true),
        SettingLayer::Default,
    );
}

#[test]
fn a_profile_that_sets_an_agent_entry_is_refused() {
    let world = World::new().profile(
        ProfileSource::Repo,
        "config: { agents: { mine: { model: haiku } } }\n",
    );
    let problems = world.problems();
    assert_eq!(problems.len(), 1, "{}", texts(&problems));
    assert!(problems[0].message.contains("agents"), "{}", problems[0]);
}

// ---- per-user keys, repo-only keys, secrets ----

#[test]
fn a_per_user_key_in_the_repos_file_is_refused_naming_the_key_and_not_applied() {
    let world = World::new().repo("claude: { check_published: false }\n");
    assert_one_problem(&world, "claude.check_published", &["repo"]);
    assert_entry(
        &world,
        "claude.check_published",
        json!(true),
        SettingLayer::Default,
    );
}

#[test]
fn ask_agent_in_the_repos_file_is_refused_naming_the_key() {
    let world = World::new().repo("ask: { agent: mine }\n");
    assert_one_problem(&world, "ask.agent", &[]);
}

#[test]
fn a_repo_only_key_in_the_users_file_is_refused_naming_the_key_and_not_applied() {
    let world = World::new().user("config: { ferry: { layout: fancy } }\n");
    assert_one_problem(&world, "ferry.layout", &["repoOnly", "user"]);
    assert_entry(
        &world,
        "ferry.layout",
        json!("plain"),
        SettingLayer::Default,
    );
}

#[test]
fn a_repo_only_key_in_the_per_board_user_file_is_refused_too() {
    let world = World::new().board("config: { ferry: { layout: fancy } }\n");
    assert_one_problem(&world, "ferry.layout", &["repoOnly"]);
}

#[test]
fn the_same_repo_only_key_in_the_repos_file_loads_and_says_repo() {
    let world = World::new().repo("config: { ferry: { layout: fancy } }\n");
    assert_entry(&world, "ferry.layout", json!("fancy"), SettingLayer::Repo);
    assert_eq!(world.problems(), vec![]);
}

const SECRET: &str = "sk-live-4242-4242-4242";

#[test]
fn a_secret_in_the_users_file_is_a_problem_naming_the_key_and_never_the_value() {
    let world = World::new().user(&format!(
        "config: {{ ferry: {{ notifyHook: \"{SECRET}\" }} }}\n"
    ));
    assert_one_problem(&world, "ferry.notifyHook", &["secret", "keychain"]);
    assert!(!texts(&world.problems()).contains(SECRET));
}

#[test]
fn a_secret_in_the_repos_file_the_board_file_or_a_profile_is_a_problem_too() {
    let text = format!("config: {{ ferry: {{ notifyHook: \"{SECRET}\" }} }}\n");
    let worlds = [
        World::new().repo(&text),
        World::new().board(&text),
        World::new().profile(ProfileSource::Repo, &text),
        World::new().profile(ProfileSource::User, &text),
    ];
    for world in worlds {
        let problems = world.problems();
        assert!(
            problems.iter().any(|p| p.key == "ferry.notifyHook"),
            "{}",
            texts(&problems)
        );
        assert!(!texts(&problems).contains(SECRET));
    }
}

#[test]
fn a_secret_entry_has_no_value_says_secret_and_says_user() {
    let world = World::new();
    let entry = world.entry("ferry.notifyHook");
    assert_eq!(entry.value, None);
    assert!(entry.secret);
    assert_eq!(entry.layer, SettingLayer::User);
}

#[test]
fn a_secret_in_a_file_does_not_make_its_entry_carry_the_value() {
    let world = World::new().user(&format!(
        "config: {{ ferry: {{ notifyHook: \"{SECRET}\" }} }}\n"
    ));
    let entry = world.entry("ferry.notifyHook");
    assert_eq!(entry.value, None);
    assert!(!format!("{entry:?}").contains(SECRET));
}

// ---- values against the schema ----

#[test]
fn a_value_of_the_wrong_type_is_refused_naming_the_key_and_the_constraint_and_not_the_value() {
    let world = World::new().user(&format!(
        "config: {{ ferry: {{ lanePool: \"{SECRET}\" }} }}\n"
    ));
    assert_one_problem(
        &world,
        "ferry.lanePool",
        &["type integer", "it is a string"],
    );
    assert!(!texts(&world.problems()).contains(SECRET));
    assert_entry(&world, "ferry.lanePool", json!(2), SettingLayer::Default);
}

#[test]
fn a_value_under_the_minimum_is_refused_naming_the_key_and_the_constraint() {
    let world = World::new().repo("config: { ferry: { depth: 0 } }\n");
    assert_one_problem(&world, "ferry.depth", &["minimum of 1", "repo"]);
}

#[test]
fn a_value_outside_the_enum_is_refused_naming_the_key_and_the_constraint() {
    let world = World::new().user("config: { ferry: { mode: medium } }\n");
    assert_one_problem(&world, "ferry.mode", &["is not one of", "\"fast\""]);
}

#[test]
fn a_profile_value_is_checked_against_the_schema_too() {
    let world = World::new().profile(ProfileSource::Repo, "config: { ferry: { depth: 0 } }\n");
    assert_one_problem(&world, "ferry.depth", &["minimum of 1", "work"]);
}

#[test]
fn a_collide_key_in_a_profile_is_checked_against_the_registry_range() {
    let world = World::new().profile(ProfileSource::User, "config: { disk: { floor_gib: 0 } }\n");
    assert_one_problem(&world, "disk.floor_gib", &["minimum of 1"]);
}

#[test]
fn a_value_that_is_refused_in_one_layer_leaves_the_layer_below_in_force() {
    let world = World::new()
        .user("config: { ferry: { depth: 5 } }\n")
        .repo("config: { ferry: { depth: 0 } }\n");
    assert_one_problem(&world, "ferry.depth", &["minimum"]);
    assert_entry(&world, "ferry.depth", json!(5), SettingLayer::User);
}

#[test]
fn every_problem_is_reported_not_only_the_first() {
    let world = World::new()
        .user("config: { ferry: { layout: x, mode: nope } }\n")
        .repo("claude: { check_published: false }\n");
    let problems = world.problems();
    let found: Vec<&str> = problems.iter().map(|p| p.key.as_str()).collect();
    assert_eq!(found.len(), 3, "{}", texts(&problems));
    for key in ["ferry.layout", "ferry.mode", "claude.check_published"] {
        assert!(found.contains(&key), "{key} in {found:?}");
    }
}

// ---- keys the schemas do not know ----

#[test]
fn a_key_a_loaded_pack_does_not_declare_is_a_problem_naming_it_and_the_ones_it_has() {
    let world = World::new().user("config: { ferry: { lanPool: 3 } }\n");
    assert_one_problem(&world, "ferry.lanPool", &["lanePool"]);
}

#[test]
fn a_pack_the_board_does_not_load_is_left_alone() {
    let world = World::new().user("config: { elsewhere: { anything: 1 } }\n");
    assert_eq!(world.problems(), vec![]);
}

#[test]
fn a_pack_section_named_like_a_collide_root_is_a_problem() {
    let world = World::new().user("config: { disk: { floor_gib: 3 } }\n");
    assert_one_problem(&world, "disk", &["disk"]);
}

// ---- finding a profile by name ----

fn profiles(names: &[&str]) -> BTreeMap<String, Profile> {
    names
        .iter()
        .map(|name| (name.to_string(), profile("description: p\n")))
        .collect()
}

fn pack_profiles() -> Vec<(String, BTreeMap<String, Profile>)> {
    vec![
        ("ferry".to_string(), profiles(&["greenfield", "small-fix"])),
        ("collide/plan".to_string(), profiles(&["tidy"])),
    ]
}

#[test]
fn a_name_is_found_in_the_repos_profiles() {
    let (repo, user, packs) = (
        profiles(&["small-fix"]),
        profiles(&["mine"]),
        pack_profiles(),
    );
    let found = find_profile("small-fix", &repo, &user, &packs).unwrap();
    assert_eq!(found.source, ProfileSource::Repo);
}

#[test]
fn a_name_is_found_in_the_users_profiles() {
    let (repo, user, packs) = (
        profiles(&["small-fix"]),
        profiles(&["mine"]),
        pack_profiles(),
    );
    let found = find_profile("mine", &repo, &user, &packs).unwrap();
    assert_eq!(found.source, ProfileSource::User);
}

#[test]
fn a_pack_profile_is_found_by_the_packs_name_and_the_profiles_name() {
    let (repo, user, packs) = (profiles(&[]), profiles(&[]), pack_profiles());
    let found = find_profile("ferry/greenfield", &repo, &user, &packs).unwrap();
    assert_eq!(found.source, ProfileSource::Pack("ferry".into()));
    let found = find_profile("collide/plan/tidy", &repo, &user, &packs).unwrap();
    assert_eq!(found.source, ProfileSource::Pack("collide/plan".into()));
}

#[test]
fn a_pack_profile_is_not_found_by_its_bare_name() {
    let (repo, user, packs) = (profiles(&[]), profiles(&[]), pack_profiles());
    assert!(matches!(
        find_profile("greenfield", &repo, &user, &packs),
        Err(ProfileLookupError::NotFound { .. })
    ));
}

#[test]
fn a_name_found_in_two_places_is_an_error_naming_both() {
    let (repo, user, packs) = (
        profiles(&["small-fix"]),
        profiles(&["small-fix"]),
        pack_profiles(),
    );
    let error = find_profile("small-fix", &repo, &user, &packs).unwrap_err();
    let text = error.to_string();
    assert!(matches!(error, ProfileLookupError::InTwoPlaces { .. }));
    assert!(text.contains("small-fix"), "{text}");
    assert!(text.contains("repo") && text.contains("user"), "{text}");
}

#[test]
fn an_unknown_name_is_an_error_naming_the_name_and_where_it_looked() {
    let (repo, user, packs) = (profiles(&["a"]), profiles(&["b"]), pack_profiles());
    let error = find_profile("nope", &repo, &user, &packs).unwrap_err();
    let text = error.to_string();
    assert!(matches!(error, ProfileLookupError::NotFound { .. }));
    assert!(text.contains("nope"), "{text}");
    for place in [
        ".collide/profiles",
        "~/.config/collide/profiles",
        "ferry",
        "collide/plan",
    ] {
        assert!(text.contains(place), "{place} missing from: {text}");
    }
}
