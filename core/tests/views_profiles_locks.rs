//! Tests for views, profiles and the lock files (ts-0026): validation as pure functions, the generated schemas,
//! closed structs in every file, and the fixture files.

use collide_core::lock::{
    LockError, PackLock, PackSource, SourceError, UserPacks, UserPacksError, check_lock,
    check_user_packs,
};
use collide_core::pack::PackManifest;
use collide_core::profile::{
    MIN_COMPACT_AT, Profile, ProfileError, ProfileKey, RepoInstructions, RoleConfig,
    USER_PROFILE_KEYS, check_profile, user_profile_refusals,
};
use collide_core::schema;
use collide_core::view::{
    Follow, LayoutKind, MAX_CEL_SOURCE_BYTES, MAX_LIST_ENTRIES, SortDir, View, ViewError,
    check_view, is_namespaced_name,
};
use collide_core::yaml::{load_yaml, load_yaml_file};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

fn load<T: DeserializeOwned>(text: &str) -> T {
    load_yaml(text.as_bytes(), "test.yaml").unwrap_or_else(|error| panic!("{error}\n{text}"))
}

fn parse_error<T: DeserializeOwned>(text: &str) -> String {
    load_yaml::<T>(text.as_bytes(), "test.yaml")
        .err()
        .unwrap_or_else(|| panic!("should not parse:\n{text}"))
        .to_string()
}

fn fixture(path: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../fixtures")
        .join(path)
}

fn fixture_error<T: DeserializeOwned>(path: &str) -> String {
    load_yaml_file::<T>(&fixture(path))
        .err()
        .unwrap_or_else(|| panic!("{path} should not parse"))
        .to_string()
}

fn schema_file(name: &str) -> Value {
    serde_json::from_str(&schema::registry().render()[name]).unwrap()
}

// Views.

#[test]
fn a_view_with_a_sorted_layout_validates() {
    let view: View = load(
        "name: MVP\nlayout: { default: board, groupBy: category, sort: { by: id, dir: asc } }\n",
    );
    assert_eq!(check_view(&view), Ok(()));
    let layout = view.layout.unwrap();
    assert_eq!(layout.default, Some(LayoutKind::Board));
    assert_eq!(layout.group_by.as_deref(), Some("category"));
    let sort = layout.sort.unwrap();
    assert_eq!((sort.by.as_str(), sort.dir), ("id", SortDir::Asc));
}

#[test]
fn layout_sort_is_in_the_generated_view_schema() {
    for file in ["View.input.json", "View.output.json"] {
        let schema = schema_file(file);
        let layout = &schema["$defs"]["Layout"]["properties"];
        assert!(layout.get("sort").is_some(), "{file}: {layout}");
        assert_eq!(
            schema["$defs"]["Sort"]["required"],
            json!(["by", "dir"]),
            "{file}"
        );
    }
}

#[test]
fn a_sort_with_an_unknown_direction_fails() {
    let error = parse_error::<View>("name: MVP\nlayout: { sort: { by: id, dir: up } }\n");
    assert!(error.contains("up"), "{error}");
}

#[test]
fn a_sort_direction_is_asc_or_desc_in_the_schema() {
    let schema = schema_file("View.input.json");
    assert_eq!(schema["$defs"]["SortDir"]["enum"], json!(["asc", "desc"]));
}

#[test]
fn a_sort_needs_both_by_and_dir() {
    parse_error::<View>("name: MVP\nlayout: { sort: { by: id } }\n");
    parse_error::<View>("name: MVP\nlayout: { sort: { dir: asc } }\n");
}

#[test]
fn a_view_needs_a_name() {
    parse_error::<View>("where: 'true'\n");
}

#[test]
fn the_cel_source_cap_is_one_kilobyte() {
    assert_eq!(MAX_CEL_SOURCE_BYTES, 1024);
}

fn view_where(source: &str) -> View {
    View {
        where_: Some(source.to_string()),
        ..load::<View>("name: MVP\n")
    }
}

#[test]
fn a_where_of_exactly_one_kilobyte_passes() {
    let source = "a".repeat(MAX_CEL_SOURCE_BYTES);
    assert_eq!(check_view(&view_where(&source)), Ok(()));
}

#[test]
fn a_where_over_one_kilobyte_fails() {
    let source = "a".repeat(MAX_CEL_SOURCE_BYTES + 1);
    assert_eq!(
        check_view(&view_where(&source)),
        Err(vec![ViewError::WhereTooLong { bytes: 1025 }])
    );
}

#[test]
fn the_cap_counts_bytes_not_characters() {
    // 513 two-byte characters: 513 characters, 1026 bytes.
    let source = "é".repeat(513);
    assert_eq!(
        check_view(&view_where(&source)),
        Err(vec![ViewError::WhereTooLong { bytes: 1026 }])
    );
}

#[test]
fn an_empty_where_fails() {
    assert_eq!(
        check_view(&view_where("  ")),
        Err(vec![ViewError::EmptyWhere])
    );
}

#[test]
fn the_cel_in_where_is_kept_as_text_and_not_parsed() {
    let view = view_where("this is ((( not CEL");
    assert_eq!(check_view(&view), Ok(()));
}

#[test]
fn a_view_reads_every_key_of_the_plan_example() {
    let view: View = load_yaml_file(&fixture("views/edge-mvp.yaml")).unwrap();
    assert_eq!(view.name, "MVP");
    assert!(view.where_.as_deref().unwrap().contains("collections"));
    assert_eq!(view.include, ["plan:T-01-042", "github:acme/edge#212"]);
    assert_eq!(view.exclude, ["plan:T-01-043"]);
    let closure = view.closure.as_ref().unwrap();
    assert_eq!(closure.goal, "plan:T-01-042");
    assert_eq!(closure.follow, Follow::DependsOn);
    assert!(view.workflow.as_deref().unwrap().ends_with("/loop"));
    assert_eq!(view.profile.as_deref(), Some("small-fix"));
    assert!(view.queue);
    assert_eq!(check_view(&view), Ok(()));
}

#[test]
fn a_view_round_trips_through_json() {
    let view: View = load_yaml_file(&fixture("views/edge-mvp.yaml")).unwrap();
    let written = serde_json::to_string(&view).unwrap();
    assert_eq!(serde_json::from_str::<View>(&written).unwrap(), view);
    let value = serde_json::to_value(&view).unwrap();
    assert_eq!(value["layout"]["sort"], json!({ "by": "id", "dir": "asc" }));
    assert_eq!(value["layout"]["groupBy"], json!("category"));
    assert!(value.get("where").is_some());
}

#[test]
fn a_bare_view_writes_only_its_name() {
    let view: View = load("name: MVP\n");
    assert_eq!(
        serde_json::to_value(&view).unwrap(),
        json!({ "name": "MVP" })
    );
}

#[test]
fn a_view_refuses_every_problem_in_one_pass() {
    let view: View = load(
        "name: ''\ninclude: ['']\nexclude: [plan:T-1, '']\nclosure: { goal: '', follow: depends_on }\n\
         workflow: loop\nprofile: ''\nlayout: { groupBy: '', sort: { by: '', dir: desc } }\n",
    );
    assert_eq!(
        check_view(&view),
        Err(vec![
            ViewError::EmptyName,
            ViewError::EmptyTaskKey {
                key: "include",
                index: 0
            },
            ViewError::EmptyTaskKey {
                key: "exclude",
                index: 1
            },
            ViewError::EmptyClosureGoal,
            ViewError::BadName {
                key: "workflow",
                name: "loop".into()
            },
            ViewError::BadName {
                key: "profile",
                name: String::new()
            },
            ViewError::EmptyLayoutField {
                key: "layout.groupBy"
            },
            ViewError::EmptyLayoutField {
                key: "layout.sort.by"
            },
        ])
    );
}

#[test]
fn a_closure_follows_only_depends_on() {
    parse_error::<View>("name: MVP\nclosure: { goal: plan:T-1, follow: blocks }\n");
}

#[test]
fn a_layout_is_a_board_or_a_run_order() {
    let view: View = load("name: MVP\nlayout: { default: run-order }\n");
    assert_eq!(view.layout.unwrap().default, Some(LayoutKind::RunOrder));
    parse_error::<View>("name: MVP\nlayout: { default: table }\n");
}

#[test]
fn a_namespaced_name_is_pack_slash_name() {
    for good in ["collide/review", "ferry/loop"] {
        assert!(is_namespaced_name(good), "{good}");
    }
    for bad in ["", "loop", "/loop", "ferry/", "a/b/c", "or chard/loop"] {
        assert!(!is_namespaced_name(bad), "{bad:?}");
    }
}

#[test]
fn an_unknown_key_fails_in_a_view() {
    let error = parse_error::<View>("name: MVP\nfilter: 'true'\n");
    assert!(error.contains("filter"), "{error}");
    let error = parse_error::<View>("name: MVP\nlayout: { sortBy: id }\n");
    assert!(error.contains("sortBy"), "{error}");
    let error =
        parse_error::<View>("name: MVP\nclosure: { goal: a, follow: depends_on, depth: 2 }\n");
    assert!(error.contains("depth"), "{error}");
    let error =
        parse_error::<View>("name: MVP\nlayout: { sort: { by: id, dir: asc, nulls: last } }\n");
    assert!(error.contains("nulls"), "{error}");
}

#[test]
fn the_invalid_view_fixtures_fail() {
    assert!(fixture_error::<View>("views/invalid/sort-unknown-dir.yaml").contains("up"));
    assert!(fixture_error::<View>("views/invalid/unknown-key.yaml").contains("filter"));
    assert!(fixture_error::<View>("views/invalid/unknown-layout-key.yaml").contains("sortBy"));
    // Parses, and the cap refuses it.
    let view: View = load_yaml_file(&fixture("views/invalid/where-over-1kb.yaml")).unwrap();
    let errors = check_view(&view).unwrap_err();
    assert!(
        matches!(errors.as_slice(), [ViewError::WhereTooLong { bytes }] if *bytes > MAX_CEL_SOURCE_BYTES),
        "{errors:?}"
    );
}

// Profiles.

#[test]
fn the_plan_example_profile_validates() {
    let profile: Profile = load_yaml_file(&fixture("profiles/small-fix.yaml")).unwrap();
    assert_eq!(check_profile(&profile), Ok(()));
    assert_eq!(profile.packs.off, ["codemap"]);
    assert!(profile.packs.on.is_empty());
    assert_eq!(profile.workflow.as_deref(), Some("collide/review"));
    assert_eq!(profile.launch["maxRounds"], json!(1));
    assert_eq!(profile.models["strong"], "sonnet");
    let builder = &profile.roles["collide/builder"];
    assert_eq!(builder.compact_at, Some(120_000));
    assert_eq!(builder.config, Some(RoleConfig::None));
    assert_eq!(profile.config.len(), 1);
    assert_eq!(
        profile.config.values().next().unwrap()["lanePool"],
        json!(1)
    );
}

#[test]
fn a_profile_role_takes_repo_instructions_worktree_or_none() {
    for (text, expected) in [
        ("none", RepoInstructions::None),
        ("worktree", RepoInstructions::Worktree),
    ] {
        let profile: Profile = load(&format!(
            "roles:\n  collide/builder: {{ repo_instructions: {text} }}\n"
        ));
        assert_eq!(check_profile(&profile), Ok(()), "{text}");
        assert_eq!(
            profile.roles["collide/builder"].repo_instructions,
            Some(expected),
            "{text}"
        );
    }
}

#[test]
fn a_profile_role_with_a_bad_repo_instructions_fails_naming_the_key() {
    let error =
        parse_error::<Profile>("roles:\n  collide/builder: { repo_instructions: sometimes }\n");
    assert!(error.contains("repo_instructions"), "{error}");
    assert!(error.contains("sometimes"), "{error}");
    let schema = schema_file("Profile.input.json");
    assert_eq!(
        schema["$defs"]["RepoInstructions"]["enum"],
        json!(["worktree", "none"])
    );
    assert!(
        schema["$defs"]["RoleOptions"]["properties"]
            .get("repo_instructions")
            .is_some()
    );
}

#[test]
fn an_empty_profile_is_valid() {
    let profile: Profile = load("{}\n");
    assert_eq!(check_profile(&profile), Ok(()));
    assert_eq!(profile.keys_set(), Vec::<ProfileKey>::new());
}

#[test]
fn a_profile_round_trips_through_json() {
    let profile: Profile = load_yaml_file(&fixture("profiles/small-fix.yaml")).unwrap();
    let written = serde_json::to_string(&profile).unwrap();
    assert_eq!(serde_json::from_str::<Profile>(&written).unwrap(), profile);
}

#[test]
fn a_role_config_of_none_stays_a_choice_and_not_a_missing_value() {
    let profile: Profile = load("roles:\n  collide/builder: { config: none }\n");
    let value = serde_json::to_value(&profile).unwrap();
    assert_eq!(
        value["roles"]["collide/builder"],
        json!({ "config": "none" })
    );
    parse_error::<Profile>("roles:\n  collide/builder: { config: project }\n");
}

#[test]
fn an_unknown_key_fails_in_a_profile() {
    let error = fixture_error::<Profile>("profiles/invalid/unknown-key.yaml");
    assert!(error.contains("workflows"), "{error}");
    let error = fixture_error::<Profile>("profiles/invalid/unknown-role-key.yaml");
    assert!(error.contains("tool_output_max"), "{error}");
    let error = parse_error::<Profile>("packs: { enable: [a] }\n");
    assert!(error.contains("enable"), "{error}");
}

#[test]
fn a_compact_at_under_the_floor_is_refused_with_the_floor() {
    let profile: Profile =
        load_yaml_file(&fixture("profiles/invalid/compact-at-below-floor.yaml")).unwrap();
    let errors = check_profile(&profile).unwrap_err();
    assert_eq!(
        errors,
        [ProfileError::CompactAtBelowFloor {
            role: "collide/builder".into(),
            value: 50_000
        }]
    );
    assert!(errors[0].to_string().contains("100000"), "{}", errors[0]);
    let at_floor: Profile = load(&format!(
        "roles:\n  collide/builder: {{ compact_at: {MIN_COMPACT_AT} }}\n"
    ));
    assert_eq!(check_profile(&at_floor), Ok(()));
}

#[test]
fn a_profile_refuses_every_problem_in_one_pass() {
    let profile: Profile = load(
        "packs: { on: [a, ''], off: [a] }\nworkflow: review\nmodels: { strong: '' }\n\
         roles: { builder: {} }\nconfig: { '': { k: 1 } }\n",
    );
    assert_eq!(
        check_profile(&profile),
        Err(vec![
            ProfileError::EmptyPackName { key: "on" },
            ProfileError::PackSwitchedBothWays { pack: "a".into() },
            ProfileError::BadWorkflowName {
                name: "review".into()
            },
            ProfileError::EmptyModel {
                alias: "strong".into()
            },
            ProfileError::BadRoleName {
                name: "builder".into()
            },
            ProfileError::EmptyConfigPack,
        ])
    );
}

#[test]
fn a_user_profile_that_sets_only_what_it_may_has_no_refusals() {
    let profile: Profile = load_yaml_file(&fixture("profiles/user-lane-pool.yaml")).unwrap();
    assert_eq!(check_profile(&profile), Ok(()));
    assert_eq!(user_profile_refusals(&profile), []);
}

#[test]
fn a_key_a_user_profile_may_not_set_is_data_that_names_the_key() {
    let profile: Profile =
        load_yaml_file(&fixture("profiles/invalid/user-sets-workflow.yaml")).unwrap();
    assert_eq!(check_profile(&profile), Ok(()));
    let refusals = user_profile_refusals(&profile);
    let keys: Vec<ProfileKey> = refusals.iter().map(|refusal| refusal.key).collect();
    assert_eq!(
        keys,
        [
            ProfileKey::Workflow,
            ProfileKey::Launch,
            ProfileKey::Models,
            ProfileKey::Roles
        ]
    );
    assert_eq!(
        refusals[0].to_string(),
        "a user profile may not set `workflow`"
    );
}

#[test]
fn every_profile_key_is_allowed_to_a_user_or_refused_by_name() {
    let all = load::<Profile>(
        "description: d\npacks: { on: [a] }\nworkflow: p/w\nlaunch: { k: 1 }\nmodels: { a: b }\n\
         roles: { p/r: {} }\nconfig: { p: { k: 1 } }\n",
    );
    let set = all.keys_set();
    assert_eq!(set.len(), 7);
    let refused: Vec<ProfileKey> = user_profile_refusals(&all)
        .into_iter()
        .map(|refusal| refusal.key)
        .collect();
    for key in set {
        assert_eq!(
            USER_PROFILE_KEYS.contains(&key),
            !refused.contains(&key),
            "{key}"
        );
    }
}

#[test]
fn profile_keys_are_named_as_the_file_writes_them() {
    let profile: Profile = load("description: d\nlaunch: { k: 1 }\n");
    let value = serde_json::to_value(&profile).unwrap();
    let written: Vec<&str> = value
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    for key in profile.keys_set() {
        assert!(written.contains(&key.as_str()), "{key}");
    }
}

#[test]
fn a_pack_manifest_holds_views_and_profiles() {
    let manifest: PackManifest = load(
        "apiVersion: collide/v0\nname: ferry\nviews:\n  MVP: { name: MVP, where: 'true' }\nprofiles:\n  greenfield: { workflow: ferry/greenfield }\n",
    );
    assert_eq!(manifest.views["MVP"].name, "MVP");
    assert_eq!(
        manifest.profiles["greenfield"].workflow.as_deref(),
        Some("ferry/greenfield")
    );
    let error = parse_error::<PackManifest>(
        "apiVersion: collide/v0\nname: ferry\nviews:\n  MVP: { name: MVP, sort: id }\n",
    );
    assert!(error.contains("sort"), "{error}");
}

// Lock files.

#[test]
fn a_lock_entry_carries_source_commit_and_tree() {
    let lock: PackLock = load_yaml_file(&fixture("locks/packs.lock")).unwrap();
    assert_eq!(check_lock(&lock), Ok(()));
    assert_eq!(lock.packs.len(), 2);
    let entry = &lock.packs[0];
    assert_eq!(entry.source, "git:github.com/org/ferry-pack//pack@v1.2");
    assert_eq!(entry.commit, "3f786850e387550fdab836ed7e6dc881de23001b");
    assert_eq!(entry.tree, "89e6c98d92887913cadf06b2adb97f26cde4849b");
}

#[test]
fn a_lock_with_no_packs_is_valid() {
    let lock: PackLock = load("{}\n");
    assert_eq!(check_lock(&lock), Ok(()));
}

#[test]
fn the_lock_files_and_the_users_packs_round_trip_through_json() {
    for path in ["locks/packs.lock", "locks/packs-npm-later.lock"] {
        let lock: PackLock = load_yaml_file(&fixture(path)).unwrap();
        let value = serde_json::to_value(&lock).unwrap();
        let again: PackLock = serde_json::from_value(value).unwrap();
        assert_eq!(again, lock, "{path}");
    }
    let packs: UserPacks = load_yaml_file(&fixture("locks/packs.yaml")).unwrap();
    let value = serde_json::to_value(&packs).unwrap();
    let again: UserPacks = serde_json::from_value(value).unwrap();
    assert_eq!(again, packs);
}

#[test]
fn a_lock_source_for_a_package_manager_validates_without_a_schema_change() {
    let lock: PackLock = load_yaml_file(&fixture("locks/packs-npm-later.lock")).unwrap();
    assert_eq!(check_lock(&lock), Ok(()));
    assert_eq!(
        PackSource::parse(&lock.packs[0].source),
        Ok(PackSource::Other {
            scheme: "npm".into(),
            rest: "@org/ferry-pack@1.2.0".into()
        })
    );
    // The schema says "a scheme, a colon, something" and names no scheme.
    let schema = schema_file("PackLock.input.json");
    let source = &schema["$defs"]["LockEntry"]["properties"]["source"];
    assert_eq!(source["type"], json!("string"));
    assert_eq!(source["pattern"], json!("^[a-z][a-z0-9+.-]*:.+$"));
    assert!(
        source.get("enum").is_none() && source.get("oneOf").is_none(),
        "{source}"
    );
}

#[test]
fn the_schema_pattern_agrees_with_the_source_parser() {
    for text in [
        "git:x",
        "repo:.collide/packs/a",
        "dev:~/a",
        "npm:@a/b@1",
        "a+b.c-d:x",
    ] {
        assert!(PackSource::parse(text).is_ok(), "{text}");
    }
    for text in [
        "",
        "x",
        "Git:x",
        "1git:x",
        "git@github.com:a/b",
        ":x",
        "git:",
    ] {
        assert!(PackSource::parse(text).is_err(), "{text:?}");
    }
}

#[test]
fn a_source_splits_at_its_scheme() {
    assert_eq!(
        PackSource::parse("git:github.com/org/p//pack@v1"),
        Ok(PackSource::Git {
            locator: "github.com/org/p//pack@v1".into()
        })
    );
    assert_eq!(
        PackSource::parse("repo:.collide/packs/ferry")
            .unwrap()
            .scheme(),
        "repo"
    );
    assert_eq!(PackSource::parse("dev:~/dev/p").unwrap().scheme(), "dev");
    assert_eq!(PackSource::parse("npm:p").unwrap().scheme(), "npm");
}

#[test]
fn a_git_source_that_starts_with_a_dash_is_refused() {
    assert_eq!(
        PackSource::parse("git:--upload-pack=evil"),
        Err(SourceError::FlagLikeLocator {
            text: "git:--upload-pack=evil".into()
        })
    );
}

#[test]
fn an_unknown_key_fails_in_every_file() {
    let error = fixture_error::<PackLock>("locks/invalid/unknown-key.lock");
    assert!(error.contains("version"), "{error}");
    let error = fixture_error::<UserPacks>("locks/invalid/packs-unknown-key.yaml");
    assert!(error.contains("enabled"), "{error}");
    let error = parse_error::<PackLock>("pins: []\n");
    assert!(error.contains("pins"), "{error}");
    let error = parse_error::<UserPacks>("pins: []\n");
    assert!(error.contains("pins"), "{error}");
}

#[test]
fn a_lock_entry_without_a_tree_fails() {
    let error = fixture_error::<PackLock>("locks/invalid/missing-tree.lock");
    assert!(error.contains("tree"), "{error}");
}

#[test]
fn a_commit_that_is_not_a_hex_object_id_is_refused() {
    let lock: PackLock = load_yaml_file(&fixture("locks/invalid/short-commit.lock")).unwrap();
    assert_eq!(
        check_lock(&lock),
        Err(vec![LockError::BadObjectId {
            index: 0,
            field: "commit",
            value: "3f78685".into()
        }])
    );
}

#[test]
fn a_sha256_id_and_an_uppercase_id() {
    let sha256 = "a".repeat(64);
    let lock: PackLock = load(&format!(
        "packs:\n  - {{ source: 'git:h/p', commit: {sha256}, tree: {sha256} }}\n"
    ));
    assert_eq!(check_lock(&lock), Ok(()));
    let upper = "A".repeat(40);
    let lock: PackLock = load(&format!(
        "packs:\n  - {{ source: 'git:h/p', commit: {upper}, tree: {upper} }}\n"
    ));
    assert_eq!(check_lock(&lock).unwrap_err().len(), 2);
}

#[test]
fn a_lock_refuses_a_repo_or_dev_source() {
    let lock: PackLock = load_yaml_file(&fixture("locks/invalid/repo-source.lock")).unwrap();
    assert_eq!(
        check_lock(&lock),
        Err(vec![LockError::NotPinned {
            index: 0,
            scheme: "repo".into()
        }])
    );
    let id = "a".repeat(40);
    let lock: PackLock = load(&format!(
        "packs:\n  - {{ source: 'dev:~/p', commit: {id}, tree: {id} }}\n"
    ));
    assert_eq!(check_lock(&lock).unwrap_err().len(), 1);
}

#[test]
fn a_lock_refuses_one_source_pinned_twice() {
    let id = "a".repeat(40);
    let lock: PackLock = load(&format!(
        "packs:\n  - {{ source: 'git:h/p', commit: {id}, tree: {id} }}\n  - {{ source: 'git:h/q', commit: {id}, tree: {id} }}\n  - {{ source: 'git:h/p', commit: {id}, tree: {id} }}\n"
    ));
    assert_eq!(
        check_lock(&lock),
        Err(vec![LockError::DuplicateSource {
            source: "git:h/p".into(),
            first: 0,
            second: 2
        }])
    );
}

#[test]
fn a_lock_refuses_a_source_with_no_scheme() {
    let id = "a".repeat(40);
    let lock: PackLock = load(&format!(
        "packs:\n  - {{ source: 'github.com/org/p', commit: {id}, tree: {id} }}\n"
    ));
    assert!(matches!(
        check_lock(&lock).unwrap_err().as_slice(),
        [LockError::BadSource { index: 0, .. }]
    ));
}

#[test]
fn the_user_lock_is_the_same_file_as_the_repo_lock() {
    let names: Vec<String> = schema::registry().render().into_keys().collect();
    let locks: Vec<&String> = names.iter().filter(|name| name.contains("Lock")).collect();
    assert_eq!(locks, ["PackLock.input.json", "PackLock.output.json"]);
}

#[test]
fn the_user_packs_file_lists_git_and_dev_sources_in_order() {
    let packs: UserPacks = load_yaml_file(&fixture("locks/packs.yaml")).unwrap();
    assert_eq!(check_user_packs(&packs), Ok(()));
    let sources: Vec<&str> = packs
        .packs
        .iter()
        .map(|entry| entry.source.as_str())
        .collect();
    assert_eq!(
        sources,
        [
            "git:github.com/org/ferry-pack//pack@v1.2",
            "dev:~/dev/ferry-pack"
        ]
    );
}

#[test]
fn the_user_packs_file_refuses_a_repo_source_and_a_repeat() {
    let packs: UserPacks =
        load_yaml_file(&fixture("locks/invalid/packs-repo-source.yaml")).unwrap();
    assert_eq!(
        check_user_packs(&packs),
        Err(vec![UserPacksError::RepoSource { index: 0 }])
    );
    let packs: UserPacks = load("packs:\n  - { source: 'dev:~/p' }\n  - { source: 'dev:~/p' }\n");
    assert_eq!(
        check_user_packs(&packs),
        Err(vec![UserPacksError::DuplicateSource {
            source: "dev:~/p".into(),
            first: 0,
            second: 1
        }])
    );
    let packs: UserPacks = load("packs:\n  - { source: ferry }\n");
    assert!(matches!(
        check_user_packs(&packs).unwrap_err().as_slice(),
        [UserPacksError::BadSource { index: 0, .. }]
    ));
}

#[test]
fn the_user_packs_file_admits_a_package_manager_source() {
    let packs: UserPacks = load("packs:\n  - { source: 'npm:@org/p@1' }\n");
    assert_eq!(check_user_packs(&packs), Ok(()));
}

// Schemas.

#[test]
fn the_registry_has_every_top_level_type_of_this_item() {
    let names: Vec<String> = schema::registry().render().into_keys().collect();
    for name in ["View", "Profile", "PackLock", "UserPacks"] {
        for side in ["input", "output"] {
            let file = format!("{name}.{side}.json");
            assert!(names.contains(&file), "{file}");
        }
    }
}

#[test]
fn every_struct_in_these_schemas_is_closed() {
    fn open_objects(value: &Value, at: &str, found: &mut Vec<String>) {
        match value {
            Value::Object(map) => {
                let is_struct = map.contains_key("properties");
                if is_struct && map.get("additionalProperties") != Some(&json!(false)) {
                    found.push(at.to_string());
                }
                for (key, child) in map {
                    open_objects(child, &format!("{at}/{key}"), found);
                }
            }
            Value::Array(items) => {
                for (index, child) in items.iter().enumerate() {
                    open_objects(child, &format!("{at}/{index}"), found);
                }
            }
            _ => {}
        }
    }
    for name in ["View", "Profile", "PackLock", "UserPacks"] {
        for side in ["input", "output"] {
            let mut found = Vec::new();
            open_objects(
                &schema_file(&format!("{name}.{side}.json")),
                name,
                &mut found,
            );
            assert_eq!(found, Vec::<String>::new(), "{name}.{side}");
        }
    }
}

#[test]
fn the_pack_manifest_schema_holds_views_and_profiles() {
    let schema = schema_file("PackManifest.input.json");
    assert!(schema["properties"].get("views").is_some());
    assert!(schema["properties"].get("profiles").is_some());
}

fn view_with_keys(include: usize, exclude: usize) -> View {
    let keys = |count: usize| (0..count).map(|index| format!("plan:T-{index}")).collect();
    View {
        include: keys(include),
        exclude: keys(exclude),
        ..load::<View>("name: MVP\n")
    }
}

#[test]
fn include_and_exclude_of_exactly_the_cap_pass() {
    assert_eq!(
        check_view(&view_with_keys(MAX_LIST_ENTRIES, MAX_LIST_ENTRIES)),
        Ok(())
    );
}

#[test]
fn include_and_exclude_over_the_cap_fail_each_with_its_count() {
    let errors =
        check_view(&view_with_keys(MAX_LIST_ENTRIES + 1, MAX_LIST_ENTRIES + 2)).unwrap_err();
    assert_eq!(
        errors,
        vec![
            ViewError::TooManyTaskKeys {
                key: "include",
                entries: 1001
            },
            ViewError::TooManyTaskKeys {
                key: "exclude",
                entries: 1002
            },
        ]
    );
    assert_eq!(
        errors[0].to_string(),
        "`include:` has 1001 entries; a list is capped at 1000"
    );
}
