//! Tests for the pure side of trust (ts-0097-kw): the text the hash is of, the diff of two input lists, the file's
//! types through the YAML writer and the hardened loader, the protected-glob roots and the pack review.

use collide_core::owner::TrustState;
use collide_core::pack::PackManifest;
use collide_core::trust::{
    ChangeKind, InputChange, InputError, LineTrust, TrustFile, TrustInput, canonical_text,
    diff_inputs, pack_review, protected_root,
};
use collide_core::yaml::load_yaml;
use collide_core::yaml_out::to_yaml;

const ID_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const ID_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const ID_C: &str = "cccccccccccccccccccccccccccccccccccccccc";

fn input(name: &str, id: &str) -> TrustInput {
    TrustInput::new(name, id)
}

// ---- canonical_text ----

#[test]
fn the_text_is_name_nul_id_lf_for_each_input_sorted_by_name() {
    let text = canonical_text(&[input("b", ID_B), input("a", ID_A)]).unwrap();
    assert_eq!(text, format!("a\0{ID_A}\nb\0{ID_B}\n"));
}

#[test]
fn the_text_does_not_depend_on_the_order_of_the_inputs() {
    let one = [input("x", ID_A), input("y", ID_B), input("z", ID_C)];
    let other = [input("z", ID_C), input("x", ID_A), input("y", ID_B)];
    assert_eq!(canonical_text(&one), canonical_text(&other));
}

#[test]
fn an_input_listed_twice_with_one_id_is_written_once() {
    let text = canonical_text(&[input("a", ID_A), input("a", ID_A)]).unwrap();
    assert_eq!(text, format!("a\0{ID_A}\n"));
}

#[test]
fn one_name_with_two_ids_is_refused() {
    let error = canonical_text(&[input("a", ID_A), input("a", ID_B)]).unwrap_err();
    assert_eq!(error, InputError::DuplicateName { name: "a".into() });
}

#[test]
fn a_name_or_an_id_with_a_nul_or_a_newline_or_nothing_is_refused() {
    for bad in [
        input("a\nb", ID_A),
        input("a\0b", ID_A),
        input("", ID_A),
        input("a", ""),
        input("a", "x\ny"),
    ] {
        assert!(
            matches!(
                canonical_text(std::slice::from_ref(&bad)),
                Err(InputError::BadPart { .. })
            ),
            "{bad:?}"
        );
    }
}

#[test]
fn a_name_cannot_pass_for_two_inputs() {
    // Without the check, this one input would write the same text as the two below it.
    let forged = input(&format!("a\0{ID_A}\nb"), ID_B);
    assert!(canonical_text(&[forged]).is_err());
    assert!(canonical_text(&[input("a", ID_A), input("b", ID_B)]).is_ok());
}

// ---- diff_inputs ----

#[test]
fn the_diff_lists_added_removed_and_changed_inputs_in_name_order() {
    let old = [
        input("board.yaml", ID_A),
        input("config.yaml", ID_A),
        input("views/old.yaml", ID_A),
    ];
    let new = [
        input("views/new.yaml", ID_B),
        input("board.yaml", ID_A),
        input("config.yaml", ID_B),
    ];
    assert_eq!(
        diff_inputs(&old, &new),
        vec![
            InputChange {
                name: "config.yaml".into(),
                kind: ChangeKind::Changed,
                old: Some(ID_A.into()),
                new: Some(ID_B.into()),
            },
            InputChange {
                name: "views/new.yaml".into(),
                kind: ChangeKind::Added,
                old: None,
                new: Some(ID_B.into()),
            },
            InputChange {
                name: "views/old.yaml".into(),
                kind: ChangeKind::Removed,
                old: Some(ID_A.into()),
                new: None,
            },
        ]
    );
}

#[test]
fn equal_lists_have_no_diff_and_the_diff_does_not_depend_on_input_order() {
    let one = [input("a", ID_A), input("b", ID_B)];
    let other = [input("b", ID_B), input("a", ID_A)];
    assert!(diff_inputs(&one, &other).is_empty());
    let changed = [input("b", ID_C), input("a", ID_A), input("c", ID_C)];
    assert_eq!(diff_inputs(&one, &changed), diff_inputs(&other, &changed));
}

#[test]
fn with_nothing_before_every_input_is_added() {
    let diff = diff_inputs(&[], &[input("a", ID_A), input("b", ID_B)]);
    assert_eq!(diff.len(), 2);
    assert!(diff.iter().all(|change| change.kind == ChangeKind::Added));
}

// ---- the file ----

fn sample_file() -> TrustFile {
    let mut file = TrustFile::default();
    file.set_line(
        "/work/main-repo",
        "main",
        LineTrust {
            hash: ID_A.into(),
            inputs: vec![
                input("board.yaml", ID_B),
                input("pack repo:.collide/p", ID_C),
            ],
        },
    );
    file.set_line(
        "/work/main-repo",
        "feature/x",
        LineTrust {
            hash: ID_B.into(),
            inputs: vec![],
        },
    );
    file.set_line(
        "/work/other",
        "main",
        LineTrust {
            hash: ID_C.into(),
            inputs: vec![input("board.yaml", ID_A)],
        },
    );
    file.set_pack("git:https://example.invalid/p.git", ID_A);
    file.add_dev("/work/dev-pack");
    file
}

#[test]
fn the_file_is_read_back_equal_by_the_hardened_loader() {
    let file = sample_file();
    let text = to_yaml(&file, "Collide trust.").unwrap();
    let back: TrustFile = load_yaml(text.as_bytes(), "trust.yaml").unwrap();
    assert_eq!(back, file, "{text}");
}

#[test]
fn an_empty_file_and_a_file_with_only_some_keys_load() {
    let empty: TrustFile = load_yaml(b"{}\n", "trust.yaml").unwrap();
    assert_eq!(empty, TrustFile::default());
    let some: TrustFile = load_yaml(b"dev:\n  - \"/x\"\n", "trust.yaml").unwrap();
    assert_eq!(some.dev, vec!["/x".to_string()]);
}

#[test]
fn an_unknown_key_and_a_duplicate_key_are_errors_naming_the_file() {
    let error = load_yaml::<TrustFile>(b"trusted: true\n", "trust.yaml").unwrap_err();
    assert!(error.to_string().starts_with("trust.yaml"), "{error}");
    let duplicate = b"packs: {}\npacks: {}\n";
    let error = load_yaml::<TrustFile>(duplicate, "trust.yaml").unwrap_err();
    assert!(error.to_string().starts_with("trust.yaml"), "{error}");
}

#[test]
fn two_lines_of_one_board_and_two_boards_are_separate_entries() {
    let file = sample_file();
    assert_eq!(file.line("/work/main-repo", "main").unwrap().hash, ID_A);
    assert_eq!(
        file.line("/work/main-repo", "feature/x").unwrap().hash,
        ID_B
    );
    assert_eq!(file.line("/work/other", "main").unwrap().hash, ID_C);
    assert!(file.line("/work/other", "feature/x").is_none());
    assert!(file.line("/nowhere", "main").is_none());
}

#[test]
fn setting_a_line_again_replaces_it_and_dev_folders_stay_sorted_and_unique() {
    let mut file = sample_file();
    file.set_line(
        "/work/main-repo",
        "main",
        LineTrust {
            hash: ID_C.into(),
            inputs: vec![],
        },
    );
    assert_eq!(file.line("/work/main-repo", "main").unwrap().hash, ID_C);
    file.add_dev("/a");
    file.add_dev("/work/dev-pack");
    file.add_dev("/a");
    assert_eq!(
        file.dev,
        vec!["/a".to_string(), "/work/dev-pack".to_string()]
    );
    file.set_pack("git:https://example.invalid/p.git", ID_B);
    assert_eq!(file.packs["git:https://example.invalid/p.git"].tree, ID_B);
}

#[test]
fn the_state_is_the_owner_wire_type() {
    let trusted = TrustState::Trusted { hash: ID_A.into() };
    let text = serde_json::to_string(&trusted).unwrap();
    assert_eq!(
        text,
        format!("{{\"state\":\"trusted\",\"hash\":\"{ID_A}\"}}")
    );
    let back: TrustState = serde_json::from_str(&text).unwrap();
    assert_eq!(back, trusted);
    let untrusted: TrustState =
        serde_json::from_str(&format!("{{\"state\":\"untrusted\",\"hash\":\"{ID_B}\"}}")).unwrap();
    assert_eq!(untrusted, TrustState::Untrusted { hash: ID_B.into() });
}

// ---- protected_root ----

#[test]
fn a_protected_glob_is_hashed_at_its_folder_before_the_first_wildcard() {
    for (glob, root) in [
        ("src/gen/**/*.rs", "src/gen"),
        (".github/workflows/*.yml", ".github/workflows"),
        ("src/foo*.rs", "src"),
        ("a/b/c.txt", "a/b/c.txt"),
        ("docs/", "docs"),
        ("./docs//api/", "docs/api"),
        ("a/[bc]/d", "a"),
        ("a/b{c,d}", "a"),
        ("a/b?/c", "a"),
        ("*.lock", ""),
        ("**/*.lock", ""),
    ] {
        assert_eq!(protected_root(glob), root, "{glob}");
    }
}

// ---- pack_review ----

fn manifest(text: &str) -> PackManifest {
    load_yaml(text.as_bytes(), "pack.yaml").unwrap()
}

const FULL_PACK: &str = r#"
apiVersion: collide/v0
name: ferry
version: 1.2.0
requires:
  sdk: ">=0.1 <0.2"
  tools:
    - { name: pnpm, version: ">=9.0", hint: "install pnpm" }
    - { name: git }
  agents: [claude]
workspaces:
  worktree:
    create: { script: hooks/create.sh }
    cleanup: { script: hooks/cleanup.sh }
integrations:
  merge: { script: hooks/merge.sh }
gates:
  tests: { script: hooks/tests.sh }
bindings:
  - { point: task.before_status, gate: tests }
  - { point: task.after_status, script: hooks/notify.sh }
providers:
  tasks: { script: providers/tasks.ts }
observers:
  audit: { script: observers/audit.sh, events: ["run.*", "task.transition"] }
workflows:
  loop:
    phases:
      build: { script: hooks/build.sh }
      wait-ci: { wait: hooks/ci.sh, timeout: 10m }
"#;

#[test]
fn the_review_lists_the_hooks_providers_observers_and_requires_of_a_pack() {
    let review = pack_review(&manifest(FULL_PACK));
    assert_eq!(review.name, "ferry");
    assert_eq!(review.version.as_deref(), Some("1.2.0"));
    let hooks: Vec<(&str, &str)> = review
        .hooks
        .iter()
        .map(|hook| (hook.key.as_str(), hook.script.as_str()))
        .collect();
    for expected in [
        ("workspaces.worktree.create", "hooks/create.sh"),
        ("workspaces.worktree.cleanup", "hooks/cleanup.sh"),
        ("integrations.merge", "hooks/merge.sh"),
        ("gates.tests", "hooks/tests.sh"),
        ("bindings[1]", "hooks/notify.sh"),
        ("workflows.loop.phases.build", "hooks/build.sh"),
        ("workflows.loop.phases.wait-ci", "hooks/ci.sh"),
    ] {
        assert!(hooks.contains(&expected), "{expected:?} not in {hooks:?}");
    }
    assert_eq!(review.providers.len(), 1);
    assert_eq!(review.providers[0].key, "providers.tasks");
    assert_eq!(review.providers[0].script, "providers/tasks.ts");
    assert_eq!(review.observers.len(), 1);
    assert_eq!(review.observers[0].script, "observers/audit.sh");
    assert!(
        review.observers[0].key.contains("run.*"),
        "{:?}",
        review.observers[0]
    );
    assert!(review.observers[0].key.contains("task.transition"));
    assert_eq!(
        review.requires,
        vec![
            "sdk >=0.1 <0.2".to_string(),
            "tool pnpm >=9.0 (install pnpm)".to_string(),
            "tool git".to_string(),
            "agent claude".to_string(),
        ]
    );
}

#[test]
fn a_pack_with_nothing_to_run_has_an_empty_review() {
    let review = pack_review(&manifest("apiVersion: collide/v0\nname: bare\n"));
    assert!(review.hooks.is_empty() && review.providers.is_empty() && review.observers.is_empty());
    assert!(review.requires.is_empty());
    let text = review.render("repo:p", None);
    assert!(text.contains("bare") && text.contains("repo:p"), "{text}");
    assert!(text.contains("Hooks it runs: none"), "{text}");
}

#[test]
fn the_rendered_review_names_the_source_the_commit_and_each_script() {
    let review = pack_review(&manifest(FULL_PACK));
    let text = review.render("git:https://example.invalid/ferry.git", Some("abc123"));
    for part in [
        "ferry 1.2.0",
        "git:https://example.invalid/ferry.git",
        "abc123",
        "hooks/create.sh",
        "providers/tasks.ts",
        "observers/audit.sh",
        "pnpm",
    ] {
        assert!(text.contains(part), "{part} not in\n{text}");
    }
    assert!(text.lines().all(|line| line.len() <= 120), "{text}");
}
