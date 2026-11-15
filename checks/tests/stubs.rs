//! Positive controls for the stub count (0.15): every stub carries a tag that is in the registry, and every
//! registered tag names a stage of the `Stage` enum.

mod support;

use collide_checks::stubs;
use support::Tree;

/// A Rust or TypeScript stub, joined at run time so that this file holds no stub of its own.
fn rust_stub(tag: &str) -> String {
    format!("fn f() {{ {}(\"{tag}\") }}\n", ["todo", "!"].concat())
}

fn ts_stub(tag: &str) -> String {
    format!(
        "export const f = () => {}('{tag}')\n",
        ["unimplemented"].concat()
    )
}

fn registry(entries: &[(&str, &str)]) -> String {
    let body: Vec<String> = entries
        .iter()
        .map(|(tag, stage)| format!("\"{tag}\": \"{stage}\""))
        .collect();
    format!("{{{}}}", body.join(", "))
}

fn tree(registry_text: &str) -> Tree {
    let tree = Tree::new();
    tree.write("stub-tags.json", registry_text);
    tree
}

#[test]
fn a_registered_stub_passes_and_is_counted_per_tag() {
    let tree = tree(&registry(&[("engine: join", "8"), ("sdk: ctx", "4b")]));
    tree.write("core/src/a.rs", &rust_stub("engine: join"));
    tree.write("core/src/b.rs", &rust_stub("engine: join"));
    tree.write("packages/sdk/src/c.ts", &ts_stub("sdk: ctx"));
    let report = stubs::check(tree.root()).unwrap();
    assert_eq!(report.problems, Vec::<String>::new());
    assert_eq!(report.counts["engine: join"], 2);
    assert_eq!(report.counts["sdk: ctx"], 1);
}

#[test]
fn a_registered_tag_with_no_stub_counts_zero() {
    let tree = tree(&registry(&[("engine: join", "8")]));
    tree.write("core/src/a.rs", "pub fn f() {}\n");
    let report = stubs::check(tree.root()).unwrap();
    assert_eq!(report.problems, Vec::<String>::new());
    assert_eq!(report.counts["engine: join"], 0);
}

#[test]
fn a_rust_stub_with_an_unregistered_tag_fails() {
    let tree = tree(&registry(&[("engine: join", "8")]));
    tree.write("core/src/a.rs", &rust_stub("engine: nope"));
    let report = stubs::check(tree.root()).unwrap();
    assert_eq!(report.problems.len(), 1);
    assert!(
        report.problems[0].starts_with("core/src/a.rs:1:")
            && report.problems[0].contains("engine: nope")
    );
}

#[test]
fn a_typescript_stub_with_an_unregistered_tag_fails() {
    let tree = tree(&registry(&[("sdk: ctx", "4b")]));
    tree.write("app/src/a.tsx", &ts_stub("ui: nope"));
    assert_eq!(stubs::check(tree.root()).unwrap().problems.len(), 1);
}

#[test]
fn a_stub_with_no_tag_or_a_tag_that_is_not_a_literal_fails() {
    let tree = tree(&registry(&[]));
    tree.write(
        "core/src/a.rs",
        &format!("fn f() {{ {}() }}\n", ["todo", "!"].concat()),
    );
    tree.write(
        "core/src/b.rs",
        &format!(
            "fn f(t: &str) {{ {}(t) }}\n",
            ["unimplemented", "!"].concat()
        ),
    );
    tree.write(
        "app/src/c.ts",
        &format!("export const c = (t: string) => {}(t)\n", "unimplemented"),
    );
    tree.write(
        "app/src/d.ts",
        &format!(
            "export const d = () => {}(`sdk: ${{1}}`)\n",
            "unimplemented"
        ),
    );
    assert_eq!(stubs::check(tree.root()).unwrap().problems.len(), 4);
}

#[test]
fn a_tag_in_a_call_that_spans_lines_is_read() {
    let tree = tree(&registry(&[("sdk: ctx", "4b")]));
    tree.write(
        "app/src/a.ts",
        &format!(
            "export const a = () => {}(\n  'sdk: ctx',\n)\n",
            "unimplemented"
        ),
    );
    let report = stubs::check(tree.root()).unwrap();
    assert_eq!(report.problems, Vec::<String>::new());
    assert_eq!(report.counts["sdk: ctx"], 1);
}

#[test]
fn a_registered_tag_that_names_no_stage_fails() {
    let tree = tree(&registry(&[("engine: join", "99")]));
    let report = stubs::check(tree.root()).unwrap();
    assert_eq!(report.problems.len(), 1);
    assert!(report.problems[0].contains("engine: join") && report.problems[0].contains("99"));
}

#[test]
fn every_stage_of_the_enum_is_a_valid_registry_value() {
    let ids = [
        "1", "2", "3a", "3b", "3c", "3d", "4a", "4b", "4c", "5", "6a", "6b", "6c", "7", "8", "9a",
        "9b", "9c", "9d", "10", "11", "12", "13", "14", "15",
    ];
    let entries: Vec<(String, &str)> = ids.iter().map(|id| (format!("t: s{id}"), *id)).collect();
    let refs: Vec<(&str, &str)> = entries.iter().map(|(t, s)| (t.as_str(), *s)).collect();
    let tree = tree(&registry(&refs));
    assert_eq!(
        stubs::check(tree.root()).unwrap().problems,
        Vec::<String>::new()
    );
}

#[test]
fn a_stage_heading_is_not_a_stage_id() {
    let tree = tree(&registry(&[("sdk: ctx", "Stage 4b: Providers")]));
    assert_eq!(stubs::check(tree.root()).unwrap().problems.len(), 1);
}

#[test]
fn a_missing_or_malformed_registry_fails() {
    let tree = Tree::new();
    assert!(stubs::check(tree.root()).is_err());
    tree.write("stub-tags.json", "[1, 2]");
    assert!(stubs::check(tree.root()).is_err());
}

#[test]
fn tests_fixtures_comments_and_the_declaration_of_unimplemented_are_not_stubs() {
    let tree = tree(&registry(&[]));
    tree.write("core/tests/a.rs", &rust_stub("not: counted"));
    tree.write("core/tests/fixtures/b.rs", &rust_stub("not: counted"));
    tree.write("packages/sdk/test/c.ts", &ts_stub("not: counted"));
    tree.write(
        "core/src/d.rs",
        &format!("// {}", rust_stub("not: counted")),
    );
    tree.write("app/src/e.ts", &format!("// {}", ts_stub("not: counted")));
    tree.write("app/src/f.ts", &format!(" * {}", ts_stub("not: counted")));
    tree.write(
        "packages/sdk/src/g.ts",
        &format!(
            "export function {}(tag: string): never {{ throw new Error(tag) }}\n",
            "unimplemented"
        ),
    );
    tree.write("packages/sdk/dist/h.ts", &ts_stub("not: counted"));
    tree.write("packages/sdk/src/i.d.ts", &ts_stub("not: counted"));
    let report = stubs::check(tree.root()).unwrap();
    assert_eq!(report.problems, Vec::<String>::new());
    assert!(report.counts.is_empty());
}
