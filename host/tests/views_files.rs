//! The files and the cache of views in the host (ts-0119-tj; 0.10, Views): where a personal view is read and written,
//! what a board id may name, and when an evaluation is worked out again. No daemon runs and no process starts. Every
//! path is below the test's own temp root, which stands in for the home.

mod common;

use collide_core::corpus::Corpus;
use collide_core::item::{Category, StatusDecl};
use collide_core::owner::ViewSource;
use collide_core::provider::{ListResult, Scope};
use collide_core::resolve::is_safe_token;
use collide_core::testing::{default_initialize, fake_task};
use collide_core::view::View;
use collide_core::view_eval::compile_view;
use collide_core::view_expr::Budgets;
use collide_host::views::{
    SaveError, ViewCache, check_view_name, personal_candidates, personal_dir, save_view,
    view_file_stem,
};
use common::TempRoot;
use std::fs;
use std::path::Path;

fn write(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

fn view(name: &str) -> View {
    serde_json::from_value(serde_json::json!({ "name": name, "where": "status == 'ready'" }))
        .unwrap()
}

#[test]
fn a_board_id_that_is_not_a_safe_name_is_an_error_and_no_path_outside_the_folder() {
    let root = TempRoot::new();
    // Where `../escape` would lead from the board's folder: a valid view that must never be read.
    write(
        &root.path().join(".config/collide/escape/views/loot.yaml"),
        "name: loot\ninclude: [plan:1]\n",
    );
    for id in ["../escape", "a/b", "..", "", ".hidden", "-flag", "x y"] {
        let folder = personal_dir(root.path(), id);
        assert!(folder.unwrap_err().contains("safe name"), "{id:?}");
        let listed = personal_candidates(root.path(), id);
        assert!(
            listed.is_err(),
            "{id:?} reads nothing: {:?}",
            listed.map(|found| found.len())
        );
    }
    assert!(
        !root.path().join(".config/collide/boards").exists(),
        "nothing was made"
    );
}

#[test]
fn a_personal_folder_lists_its_yaml_files_and_names_the_one_that_is_not_a_regular_file() {
    let root = TempRoot::new();
    let folder = personal_dir(root.path(), "board-1").unwrap();
    write(&folder.join("fine.yaml"), "name: fine\ninclude: [plan:1]\n");
    write(&folder.join("notes.txt"), "not a view\n");
    write(&root.path().join("elsewhere.yaml"), "name: elsewhere\n");
    std::os::unix::fs::symlink(root.path().join("elsewhere.yaml"), folder.join("link.yaml"))
        .unwrap();

    let found = personal_candidates(root.path(), "board-1").unwrap();

    let names: Vec<(&str, bool)> = found
        .iter()
        .map(|candidate| (candidate.view.name.as_str(), candidate.error.is_some()))
        .collect();
    assert_eq!(names, [("fine", false), ("link", true)]);
    assert!(
        found
            .iter()
            .all(|candidate| candidate.source == ViewSource::Personal {})
    );
    let link = found
        .iter()
        .find(|candidate| candidate.view.name == "link")
        .unwrap();
    assert!(link.error.as_deref().unwrap().contains("regular file"));
}

fn corpus_of(shorts: &[&str]) -> Corpus {
    let mut init = default_initialize();
    init.scope = Scope::Global;
    init.statuses = vec![StatusDecl {
        name: "ready".into(),
        category: Category::Ready,
    }];
    let mut corpus = Corpus::default();
    corpus.declare_source("plan", &init);
    let tasks = shorts
        .iter()
        .map(|short| {
            let mut task = fake_task(short, short, "ready", Category::Ready);
            task.key = format!("plan:{short}");
            task
        })
        .collect();
    let list = ListResult {
        tasks,
        collections: Vec::new(),
        invalid: Vec::new(),
        next_cursor: None,
    };
    corpus.load("plan", None, list).unwrap();
    corpus
}

#[test]
fn the_cache_works_a_view_out_once_for_one_corpus_and_again_when_the_corpus_changes() {
    let cache = ViewCache::new(None);
    let mut corpus = corpus_of(&["1", "2"]);
    let compiled = compile_view(&view("v"), 2, &Budgets::default()).unwrap();

    let first = cache
        .evaluate("board-1", &compiled, corpus.stamp(), || corpus.clone())
        .unwrap();
    let again = cache
        .evaluate("board-1", &compiled, corpus.stamp(), || {
            panic!("a hit reads no corpus")
        })
        .unwrap();
    assert_eq!(first.members, ["plan:1", "plan:2"]);
    assert_eq!(again, first);
    assert_eq!(cache.evaluations(), 1);

    corpus.mark_stale("plan", "late").unwrap();
    cache
        .evaluate("board-1", &compiled, corpus.stamp(), || corpus.clone())
        .unwrap();
    assert_eq!(
        cache.evaluations(),
        2,
        "a changed corpus is worked out again"
    );

    let other = view("w");
    let compiled = compile_view(&other, 2, &Budgets::default()).unwrap();
    cache
        .evaluate("board-1", &compiled, corpus.stamp(), || corpus.clone())
        .unwrap();
    cache
        .evaluate("board-2", &compiled, corpus.stamp(), || corpus.clone())
        .unwrap();
    assert_eq!(
        cache.evaluations(),
        4,
        "a view and a board each have their own"
    );
}

// ---- a folder that is a link is not written through ----------------------------------------------------------------

fn is_empty(dir: &Path) -> bool {
    fs::read_dir(dir).unwrap().next().is_none()
}

#[test]
fn a_views_folder_that_is_a_link_is_refused_and_nothing_appears_in_the_folder_it_points_to() {
    let root = TempRoot::new();
    let outside = root.path().join("outside");
    fs::create_dir_all(&outside).unwrap();
    let base = root.path().join("repo");
    fs::create_dir_all(base.join(".collide")).unwrap();
    std::os::unix::fs::symlink(&outside, base.join(".collide/views")).unwrap();

    let refused = save_view(
        &view("mine"),
        &base,
        &base.join(".collide/views"),
        true,
        0o644,
    );

    let Err(SaveError::Blocked(message)) = refused else {
        panic!("a link is refused: {refused:?}");
    };
    assert!(
        message.contains(".collide/views"),
        "names the folder: {message}"
    );
    assert!(message.contains("link"), "{message}");
    assert!(is_empty(&outside), "nothing was written through the link");
}

#[test]
fn a_collide_folder_that_is_a_link_is_refused_and_nothing_appears_in_the_folder_it_points_to() {
    let root = TempRoot::new();
    let outside = root.path().join("outside");
    fs::create_dir_all(&outside).unwrap();
    let base = root.path().join("repo");
    fs::create_dir_all(&base).unwrap();
    std::os::unix::fs::symlink(&outside, base.join(".collide")).unwrap();

    let refused = save_view(
        &view("mine"),
        &base,
        &base.join(".collide/views"),
        false,
        0o644,
    );

    let Err(SaveError::Blocked(message)) = refused else {
        panic!("a link is refused: {refused:?}");
    };
    assert!(message.contains(".collide"), "names the folder: {message}");
    assert!(
        is_empty(&outside),
        "no `views` folder was made through the link"
    );
}

#[test]
fn a_view_file_that_is_a_link_is_refused_even_with_overwrite_and_its_target_is_not_changed() {
    let root = TempRoot::new();
    let folder = root.path().join("repo/.collide/views");
    fs::create_dir_all(&folder).unwrap();
    let target = root.path().join("elsewhere.yaml");
    fs::write(&target, "name: elsewhere\n").unwrap();
    std::os::unix::fs::symlink(&target, folder.join("mine.yaml")).unwrap();

    for overwrite in [false, true] {
        let refused = save_view(
            &view("mine"),
            &root.path().join("repo"),
            &folder,
            overwrite,
            0o644,
        );
        assert!(refused.is_err(), "overwrite {overwrite}: {refused:?}");
        assert_eq!(fs::read_to_string(&target).unwrap(), "name: elsewhere\n");
    }
}

#[test]
fn a_missing_folder_is_still_made_and_a_real_one_is_written_into() {
    let root = TempRoot::new();
    let base = root.path().join("repo");
    fs::create_dir_all(&base).unwrap();
    let folder = base.join(".collide/views");

    let path = save_view(&view("mine"), &base, &folder, false, 0o644).unwrap();

    assert_eq!(path, folder.join("mine.yaml"));
    assert!(path.is_file());
    assert!(save_view(&view("mine"), &base, &folder, true, 0o644).is_ok());
}

// ---- a view's name apart from its file name (ts-0217-dy) -------------------------------------------------------

#[test]
fn the_file_name_is_made_from_the_name_lowercased_with_runs_of_other_characters_as_one_dash() {
    for (name, stem) in [
        ("Slow spots", "slow-spots"),
        ("SLOW   spots", "slow-spots"),
        ("Slow, spots & more!", "slow-spots-more"),
        ("a_b.c", "a_b.c"),
        ("v1.2 final", "v1.2-final"),
        ("Crème brûlée", "cr-me-br-l-e"),
    ] {
        assert_eq!(view_file_stem(name), stem, "{name:?}");
    }
}

#[test]
fn the_file_name_loses_dashes_and_dots_at_its_ends() {
    for (name, stem) in [
        ("  -Slow spots-  ", "slow-spots"),
        (".hidden", "hidden"),
        ("trailing...", "trailing"),
        ("--a--", "a"),
    ] {
        assert_eq!(view_file_stem(name), stem, "{name:?}");
    }
}

#[test]
fn the_file_name_is_cut_at_64_characters() {
    let long = "word ".repeat(30);
    let stem = view_file_stem(&long);
    assert_eq!(stem.len(), 64);
    assert!(stem.starts_with("word-word-word"), "{stem}");
}

#[test]
fn a_cut_that_ends_on_a_dash_or_a_dot_trims_it() {
    for joint in [" ", "."] {
        let name = format!("{}{joint}b", "a".repeat(63));
        assert_eq!(view_file_stem(&name), "a".repeat(63), "{name:?}");
    }
}

#[test]
fn a_name_that_leaves_nothing_gets_view_and_a_short_hash_of_the_name() {
    let one = view_file_stem("日本語");
    let two = view_file_stem("中文");
    assert!(one.starts_with("view-"), "{one}");
    assert_eq!(one.len(), "view-".len() + 8, "{one}");
    assert_eq!(
        one,
        view_file_stem("日本語"),
        "the same name gives the same file"
    );
    assert_ne!(one, two, "another name gives another file");
    for blank in ["...", "---", "! ? !"] {
        assert!(view_file_stem(blank).starts_with("view-"), "{blank:?}");
    }
}

#[test]
fn every_file_name_is_a_safe_token() {
    let long = "x".repeat(200);
    for name in [
        "Slow spots",
        "a..b",
        "...",
        "../x",
        "-",
        ".",
        "日本語",
        "a - b",
        "UPPER",
        "v1.2",
        "tab\tname",
        "é",
        &long,
        "a.-.b",
        "  ",
        "x/y",
        "..a..",
    ] {
        let stem = view_file_stem(name);
        assert!(is_safe_token(&stem), "{name:?} gave {stem:?}");
    }
}

fn file_names(folder: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(folder)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    names
}

/// A folder inside a base, as the daemon passes them for a personal view.
fn folders(root: &TempRoot) -> (std::path::PathBuf, std::path::PathBuf) {
    let base = root.path().join("board");
    (base.clone(), base.join("views"))
}

#[test]
fn slow_spots_is_saved_as_slow_spots_yaml_and_loads_back_under_the_name_typed() {
    let root = TempRoot::new();
    let folder = personal_dir(root.path(), "board-1").unwrap();
    let base = folder.parent().unwrap().to_path_buf();

    let path = save_view(&view("Slow spots"), &base, &folder, false, 0o600).unwrap();

    assert_eq!(path, folder.join("slow-spots.yaml"));
    let listed = personal_candidates(root.path(), "board-1").unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].view.name, "Slow spots");
    assert!(listed[0].error.is_none());
    let loaded: View = collide_core::yaml::load_yaml_file(&path).unwrap();
    assert_eq!(loaded.name, "Slow spots");
}

#[test]
fn spaces_at_the_ends_of_the_name_are_not_part_of_it() {
    let root = TempRoot::new();
    let (base, folder) = folders(&root);

    let path = save_view(&view("  Slow spots \t"), &base, &folder, false, 0o600).unwrap();

    let loaded: View = collide_core::yaml::load_yaml_file(&path).unwrap();
    assert_eq!(loaded.name, "Slow spots");
    assert_eq!(file_names(&folder), ["slow-spots.yaml"]);
}

#[test]
fn saving_the_same_name_again_is_refused_unless_with_overwrite_which_replaces_the_same_file() {
    let root = TempRoot::new();
    let (base, folder) = folders(&root);
    let first = save_view(&view("Slow spots"), &base, &folder, false, 0o600).unwrap();
    let before = fs::read_to_string(&first).unwrap();
    let mut changed = view("Slow spots");
    changed.include = vec!["plan:1".to_string()];

    let refused = save_view(&changed, &base, &folder, false, 0o600);

    assert!(matches!(refused, Err(SaveError::Exists(_))), "{refused:?}");
    assert_eq!(fs::read_to_string(&first).unwrap(), before);
    let again = save_view(&changed, &base, &folder, true, 0o600).unwrap();
    assert_eq!(again, first);
    assert_ne!(fs::read_to_string(&first).unwrap(), before);
    assert_eq!(file_names(&folder), ["slow-spots.yaml"]);
}

#[test]
fn a_hand_written_file_that_holds_the_name_is_the_file_an_overwrite_replaces() {
    let root = TempRoot::new();
    let (base, folder) = folders(&root);
    write(
        &folder.join("perf.yaml"),
        "name: Slow spots\ninclude: [plan:9]\n",
    );

    let refused = save_view(&view("Slow spots"), &base, &folder, false, 0o600);
    assert!(matches!(refused, Err(SaveError::Exists(_))), "{refused:?}");
    assert_eq!(file_names(&folder), ["perf.yaml"], "no second file");

    let path = save_view(&view("Slow spots"), &base, &folder, true, 0o600).unwrap();

    assert_eq!(path, folder.join("perf.yaml"));
    assert_eq!(file_names(&folder), ["perf.yaml"], "no second file");
    let loaded: View = collide_core::yaml::load_yaml_file(&path).unwrap();
    assert_eq!(loaded, view("Slow spots"));
}

#[test]
fn a_name_that_differs_only_in_case_is_refused_and_no_file_changes() {
    let root = TempRoot::new();
    let (base, folder) = folders(&root);
    let first = save_view(&view("Slow spots"), &base, &folder, false, 0o600).unwrap();
    let before = fs::read_to_string(&first).unwrap();

    for overwrite in [false, true] {
        let refused = save_view(&view("Slow Spots"), &base, &folder, overwrite, 0o600);

        let Err(SaveError::Exists(message)) = refused else {
            panic!("overwrite {overwrite}: {refused:?}");
        };
        assert!(
            message.contains("Slow spots"),
            "names the view it clashes with: {message}"
        );
        assert!(message.contains("another name"), "asks for one: {message}");
        assert_eq!(fs::read_to_string(&first).unwrap(), before);
        assert_eq!(file_names(&folder), ["slow-spots.yaml"]);
    }
}

#[test]
fn a_file_at_the_made_name_that_holds_another_name_or_does_not_parse_is_refused() {
    let root = TempRoot::new();
    let (base, folder) = folders(&root);
    write(&folder.join("slow-spots.yaml"), "name: Something else\n");
    write(&folder.join("fast-spots.yaml"), "name: [not, a, name\n");

    for (name, file) in [
        ("Slow spots", "slow-spots.yaml"),
        ("Fast spots", "fast-spots.yaml"),
    ] {
        let before = fs::read_to_string(folder.join(file)).unwrap();
        for overwrite in [false, true] {
            let refused = save_view(&view(name), &base, &folder, overwrite, 0o600);

            let Err(SaveError::Exists(message)) = refused else {
                panic!("{name} {overwrite}: {refused:?}");
            };
            assert!(message.contains(file), "names the file: {message}");
            assert!(message.contains("another name"), "asks for one: {message}");
            assert_eq!(fs::read_to_string(folder.join(file)).unwrap(), before);
        }
    }
    assert_eq!(file_names(&folder), ["fast-spots.yaml", "slow-spots.yaml"]);
}

#[test]
fn a_link_at_the_made_file_name_is_refused_even_with_overwrite_and_its_target_is_not_changed() {
    let root = TempRoot::new();
    let (base, folder) = folders(&root);
    fs::create_dir_all(&folder).unwrap();
    let target = root.path().join("elsewhere.yaml");
    fs::write(&target, "name: Slow spots\n").unwrap();
    std::os::unix::fs::symlink(&target, folder.join("slow-spots.yaml")).unwrap();

    for overwrite in [false, true] {
        let refused = save_view(&view("Slow spots"), &base, &folder, overwrite, 0o600);

        assert!(
            matches!(refused, Err(SaveError::Blocked(_))),
            "{overwrite}: {refused:?}"
        );
        assert_eq!(fs::read_to_string(&target).unwrap(), "name: Slow spots\n");
    }
}

#[test]
fn a_view_saved_before_this_item_with_its_name_equal_to_its_file_name_saves_as_before() {
    let root = TempRoot::new();
    let (base, folder) = folders(&root);
    write(&folder.join("mine.yaml"), "name: mine\nwhere: \"true\"\n");

    let refused = save_view(&view("mine"), &base, &folder, false, 0o600);
    assert!(matches!(refused, Err(SaveError::Exists(_))), "{refused:?}");
    let path = save_view(&view("mine"), &base, &folder, true, 0o600).unwrap();

    assert_eq!(path, folder.join("mine.yaml"));
    assert_eq!(file_names(&folder), ["mine.yaml"]);
}

#[test]
fn a_name_the_file_system_cannot_hold_is_a_file_named_for_it_and_two_such_names_do_not_clash() {
    let root = TempRoot::new();
    let (base, folder) = folders(&root);

    let one = save_view(&view("日本語"), &base, &folder, false, 0o600).unwrap();
    let two = save_view(&view("中文"), &base, &folder, false, 0o600).unwrap();

    assert_ne!(one, two);
    assert!(
        one.file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("view-")
    );
    let loaded: View = collide_core::yaml::load_yaml_file(&one).unwrap();
    assert_eq!(loaded.name, "日本語");
}

#[test]
fn a_name_that_cannot_be_a_view_is_refused_with_what_to_change_and_nothing_is_written() {
    let root = TempRoot::new();
    let (base, folder) = folders(&root);
    let long = "x".repeat(65);
    for (name, says) in [
        ("a/b", "/"),
        ("collide/plan/spikes", "/"),
        ("two\nlines", "control"),
        ("tab\there", "control"),
        (long.as_str(), "64"),
        ("   ", "name"),
        ("", "name"),
        ("ready", "built-in"),
        ("Bottlenecks", "built-in"),
        ("BOTTLENECKS", "built-in"),
        ("All tasks", "All tasks"),
        ("all TASKS", "All tasks"),
    ] {
        let refused = save_view(&view(name), &base, &folder, false, 0o600);

        let Err(SaveError::Invalid(message)) = refused else {
            panic!("{name:?}: {refused:?}");
        };
        assert!(
            message.contains(says),
            "{name:?} should say {says:?}: {message}"
        );
    }
    assert!(!folder.exists(), "a refused name makes no folder");
}

#[test]
fn a_name_of_64_characters_with_spaces_inside_is_a_view() {
    let name = format!("{} {}", "a".repeat(31), "b".repeat(32));
    assert_eq!(name.chars().count(), 64);
    assert_eq!(check_view_name(&name).unwrap(), name);
    assert_eq!(check_view_name("  Slow spots  ").unwrap(), "Slow spots");
    assert!(check_view_name(&format!("{name}c")).is_err());
}
