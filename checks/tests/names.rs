//! Positive controls for the no-names check (0.15): a seeded string in each forbidden place fails, a seeded string in
//! each allowed place passes, and a clean tree passes.

mod support;

use collide_checks::names;
use support::{Tree, reference_pack};

/// The Claude env var names the check looks for, joined at run time so that this file is clean under the check.
fn claude_code() -> String {
    ["CLAUDE", "CODE"].concat()
}

fn anthropic_key() -> String {
    ["ANTHROPIC", "_API_KEY"].concat()
}

fn rules(tree: &Tree) -> Vec<(String, &'static str)> {
    names::check(tree.root())
        .unwrap()
        .into_iter()
        .map(|v| (v.path, v.rule))
        .collect()
}

#[test]
fn a_clean_tree_passes() {
    let tree = Tree::new();
    tree.write(
        "core/src/lib.rs",
        "pub const NAME: &str = \"collide-core\";\n",
    );
    tree.write("host/src/lib.rs", "pub fn run() {}\n");
    tree.write("app/src/App.tsx", "export const App = () => null\n");
    assert_eq!(rules(&tree), vec![]);
}

#[test]
fn the_reference_pack_name_in_core_fails() {
    let tree = Tree::new();
    tree.write("core/src/lib.rs", &format!("// {}\n", reference_pack()));
    assert_eq!(
        rules(&tree),
        vec![("core/src/lib.rs".to_string(), names::RULE_REFERENCE_PACK)]
    );
}

#[test]
fn the_reference_pack_name_is_found_in_any_case_and_in_every_scanned_extension() {
    let tree = Tree::new();
    let name = reference_pack();
    tree.write(
        "host/src/a.rs",
        &format!("const A: &str = \"{}\";\n", name.to_uppercase()),
    );
    tree.write("app/src/b.ts", &format!("export const b = '{name}'\n"));
    tree.write(
        "app/src/c.tsx",
        &format!("export const c = <p>{name}</p>\n"),
    );
    tree.write("host/c.toml", &format!("name = \"{name}\"\n"));
    tree.write("schemas/d.json", &format!("{{\"x\": \"{name}\"}}\n"));
    tree.write("scripts/e.mjs", &format!("// {name}\n"));
    assert_eq!(rules(&tree).len(), 6);
}

#[test]
fn the_reference_pack_name_is_allowed_under_pack_directories_and_test_fixtures() {
    let tree = Tree::new();
    let name = reference_pack();
    tree.write("packs/plan/pack.yaml", &format!("name: {name}\n"));
    tree.write(
        "packs/plan/src/index.ts",
        &format!("export const n = '{name}'\n"),
    );
    tree.write("fixtures/packs/loop/pack.yaml", &format!("name: {name}\n"));
    tree.write(
        "core/tests/fixtures/a.json",
        &format!("{{\"pack\": \"{name}\"}}\n"),
    );
    tree.write(
        "host/tests/fixtures/pack/pack.yaml",
        &format!("name: {name}\n"),
    );
    assert_eq!(rules(&tree), vec![]);
}

#[test]
fn a_test_file_that_is_not_a_fixture_may_not_name_the_reference_pack() {
    let tree = Tree::new();
    tree.write(
        "core/tests/hooks.rs",
        &format!("const PACK: &str = \"{}\";\n", reference_pack()),
    );
    assert_eq!(rules(&tree).len(), 1);
}

#[test]
fn a_claude_env_var_name_outside_the_adapter_fails() {
    let tree = Tree::new();
    tree.write(
        "core/src/lib.rs",
        &format!("const A: &str = \"{}\";\n", claude_code()),
    );
    tree.write(
        "host/src/lib.rs",
        &format!("const B: &str = \"{}\";\n", anthropic_key()),
    );
    tree.write(
        "packs/plan/src/index.ts",
        &format!("process.env.{}\n", claude_code()),
    );
    tree.write(
        "app/src/App.tsx",
        &format!("// {}\n", ["ANTHROPIC", "_"].concat()),
    );
    let found = rules(&tree);
    assert_eq!(found.len(), 4);
    assert!(
        found
            .iter()
            .all(|(_, rule)| *rule == names::RULE_CLAUDE_ENV)
    );
}

#[test]
fn the_claude_env_family_is_matched_by_prefix() {
    let tree = Tree::new();
    tree.write(
        "host/src/a.rs",
        &format!(
            "const A: &str = \"{}\";\n",
            ["CLAUDE", "_CODE_DISABLE_AUTO_MEMORY"].concat()
        ),
    );
    assert_eq!(rules(&tree).len(), 1);
}

#[test]
fn a_claude_env_var_name_inside_the_adapter_passes() {
    let tree = Tree::new();
    let name = claude_code();
    tree.write(
        "host/src/adapters/claude/mod.rs",
        &format!("const A: &str = \"{name}\";\n"),
    );
    tree.write(
        "host/src/adapters/claude/launch.rs",
        &format!("const A: &str = \"{}\";\n", anthropic_key()),
    );
    tree.write(
        "host/src/adapters/claude.rs",
        &format!("const A: &str = \"{name}\";\n"),
    );
    assert_eq!(rules(&tree), vec![]);
}

#[test]
fn a_directory_that_only_starts_with_the_adapter_name_is_not_the_adapter() {
    let tree = Tree::new();
    tree.write(
        "host/src/adapters/claude_extra/mod.rs",
        &format!("const A: &str = \"{}\";\n", claude_code()),
    );
    assert_eq!(rules(&tree).len(), 1);
}

#[test]
fn the_word_claude_alone_is_not_an_env_var_name() {
    let tree = Tree::new();
    tree.write(
        "core/src/lib.rs",
        &format!("// {}.md and the Claude adapter\n", ["CLAUDE"].concat()),
    );
    assert_eq!(rules(&tree), vec![]);
}

#[test]
fn a_violation_names_its_file_and_line() {
    let tree = Tree::new();
    tree.write(
        "core/src/lib.rs",
        &format!("fn a() {{}}\n\nconst B: &str = \"{}\";\n", reference_pack()),
    );
    let v = names::check(tree.root()).unwrap();
    assert_eq!((v[0].path.as_str(), v[0].line), ("core/src/lib.rs", 3));
}

#[test]
fn prose_records_private_and_build_output_are_not_scanned() {
    let tree = Tree::new();
    let name = reference_pack();
    tree.write("IMPLEMENTATION_PLAN.md", &name);
    tree.write("docs/notes.md", &name);
    tree.write(".collide/planning/spikes/s/run.sh", &name);
    tree.write("private/key.json", &name);
    tree.write("target/debug/x.json", &name);
    tree.write("node_modules/p/index.js", &name);
    tree.write("app/src-tauri/resources/builtin-packs/p/pack.yaml", &name);
    assert_eq!(rules(&tree), vec![]);
}
