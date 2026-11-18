//! The files and the cache of views in the host (ts-0119-tj; 0.10, Views): where a personal view is read and written,
//! what a board id may name, and when an evaluation is worked out again. No daemon runs and no process starts. Every
//! path is below the test's own temp root, which stands in for the home.

mod common;

use collide_core::corpus::Corpus;
use collide_core::item::{Category, StatusDecl};
use collide_core::owner::ViewSource;
use collide_core::provider::{ListResult, Scope};
use collide_core::testing::{default_initialize, fake_task};
use collide_core::view::View;
use collide_core::view_eval::compile_view;
use collide_core::view_expr::Budgets;
use collide_host::views::{SaveError, ViewCache, personal_candidates, personal_dir, save_view};
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
