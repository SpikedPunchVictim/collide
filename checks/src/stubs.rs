//! The stub count (0.15): every stub carries a tag, every tag is in the registry, and every registered tag names a
//! stage of `Stage`. The plan's rule that a stage is complete when CI counts zero stubs for its tags rests on this.
//!
//! A stub is a call of the Rust macros `todo!` or `unimplemented!`, or of the TypeScript function `unimplemented`,
//! with the tag as one string literal: `todo!("engine: fan-out join")`, `unimplemented('sdk: ctx')`. The registry is
//! `stub-tags.json` at the repository root, an object from tag to stage id (`"4b"`, `"12"`).
//!
//! Test code is not skeleton: `tests/`, `test/` and `fixtures/` directories, declaration files, comments and the
//! declaration of the TypeScript `unimplemented` function itself are not counted.

use crate::walk;
use collide_core::stage::Stage;
use serde_json::Value;
use std::collections::BTreeMap;
use std::io;
use std::path::Path;

/// The registry's path below the repository root.
pub const REGISTRY: &str = "stub-tags.json";

const NOT_SKELETON: [&str; 3] = ["tests", "test", "fixtures"];

/// A stub call as found in a file: its 1-based line and its tag, or `None` when the tag is not one string literal.
struct Found {
    rel: String,
    line: usize,
    tag: Option<String>,
}

pub struct Report {
    /// Stubs per registered tag, with `0` for a tag that has none.
    pub counts: BTreeMap<String, usize>,
    /// The stage id of each registered tag.
    pub stages: BTreeMap<String, String>,
    /// What fails the check, one message per problem.
    pub problems: Vec<String>,
}

/// Counts the stubs under `root` against the registry. An error when the registry cannot be read or is not an object
/// of strings; every other problem is in the report.
pub fn check(root: &Path) -> io::Result<Report> {
    let registry = load_registry(&root.join(REGISTRY))?;
    let mut report = Report {
        counts: BTreeMap::new(),
        stages: BTreeMap::new(),
        problems: Vec::new(),
    };
    for (tag, stage) in &registry {
        report.counts.insert(tag.clone(), 0);
        report.stages.insert(tag.clone(), stage.clone());
        if serde_json::from_value::<Stage>(Value::String(stage.clone())).is_err() {
            report.problems.push(format!(
                "{REGISTRY}: tag \"{tag}\" names stage \"{stage}\", which is not a Stage id"
            ));
        }
    }
    for stub in find_stubs(root)? {
        let place = format!("{}:{}", stub.rel, stub.line);
        match stub.tag {
            None => report
                .problems
                .push(format!("{place}: a stub's tag must be one string literal")),
            Some(tag) => match report.counts.get_mut(&tag) {
                Some(count) => *count += 1,
                None => report
                    .problems
                    .push(format!("{place}: stub tag \"{tag}\" is not in {REGISTRY}")),
            },
        }
    }
    Ok(report)
}

fn load_registry(path: &Path) -> io::Result<BTreeMap<String, String>> {
    let bad = |why: String| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{}: {why}", path.display()),
        )
    };
    let value: Value =
        serde_json::from_str(&std::fs::read_to_string(path)?).map_err(|e| bad(e.to_string()))?;
    let Value::Object(map) = value else {
        return Err(bad("must be an object from tag to stage id".into()));
    };
    map.into_iter()
        .map(|(tag, stage)| match stage {
            Value::String(stage) => Ok((tag, stage)),
            _ => Err(bad(format!("tag \"{tag}\" must map to a stage id string"))),
        })
        .collect()
}

fn find_stubs(root: &Path) -> io::Result<Vec<Found>> {
    let mut found = Vec::new();
    for entry in walk::files(root, &["rs", "ts", "tsx"], &NOT_SKELETON)? {
        if entry.rel.ends_with(".d.ts") {
            continue;
        }
        let text = std::fs::read_to_string(&entry.path)?;
        let calls: &[&str] = if entry.rel.ends_with(".rs") {
            &["todo!", "unimplemented!"]
        } else {
            &["unimplemented"]
        };
        for call in calls {
            scan(&entry.rel, &text, call, &mut found);
        }
    }
    found.sort_by(|a, b| (&a.rel, a.line).cmp(&(&b.rel, b.line)));
    Ok(found)
}

/// Adds every `call(` in `text` that is a use, not a comment, a longer name or the declaration of the function.
fn scan(rel: &str, text: &str, call: &str, found: &mut Vec<Found>) {
    for (at, _) in text.match_indices(call) {
        let before = &text[..at];
        let line_start = before.rfind('\n').map_or(0, |n| n + 1);
        let line_before = &before[line_start..];
        let after = text[at + call.len()..].trim_start();
        let is_word_start = before
            .chars()
            .next_back()
            .is_none_or(|c| !(c.is_alphanumeric() || c == '_' || c == '$' || c == '.'));
        let is_comment =
            line_before.contains("//") || line_before.trim_start().starts_with(['*', '/']);
        let is_declaration = before.trim_end().ends_with("function");
        if !is_word_start || is_comment || is_declaration || !after.starts_with('(') {
            continue;
        }
        found.push(Found {
            rel: rel.to_string(),
            line: before.matches('\n').count() + 1,
            tag: literal_argument(&after[1..]),
        });
    }
}

/// The text of a lone string literal at the start of `args` (up to the call's closing paren), or `None`. A literal
/// may use `'` or `"`; it holds no escape, so a tag stays greppable.
fn literal_argument(args: &str) -> Option<String> {
    let args = args.trim_start();
    let quote = args.chars().next().filter(|c| *c == '\'' || *c == '"')?;
    let body = &args[1..];
    let end = body.find(quote)?;
    let (tag, rest) = (&body[..end], body[end + 1..].trim_start());
    let closes = rest.starts_with(')')
        || rest.starts_with(",)")
        || rest
            .strip_prefix(',')
            .is_some_and(|r| r.trim_start().starts_with(')'));
    (closes && !tag.contains('\\') && !tag.is_empty()).then(|| tag.to_string())
}
