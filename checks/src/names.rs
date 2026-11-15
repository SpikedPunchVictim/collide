//! The no-names check (0.15; 0.1): the reference pack's name appears only under pack directories and test fixtures,
//! and the Claude env var names appear only inside the Claude adapter.
//!
//! It reads source and configuration files (`SCANNED`). Markdown, the planning records and the research notes are
//! prose and are not read (see `walk`).

use crate::walk;
use std::fmt;
use std::io;
use std::path::Path;

pub const RULE_REFERENCE_PACK: &str = "reference-pack-name";
pub const RULE_CLAUDE_ENV: &str = "claude-env-name";

/// Extensions of the files that are read.
const SCANNED: [&str; 14] = [
    "rs", "ts", "tsx", "js", "mjs", "cjs", "json", "yaml", "yml", "toml", "html", "css", "sh", "py",
];

/// Where the Claude adapter lives (0.1, `host`): a path ending in `/` is a directory, any other entry is one file.
/// The adapter is not written yet; this is the path its outline takes.
const ADAPTER_PATHS: [&str; 3] = [
    "host/src/adapters/claude/",
    "host/src/adapters/claude.rs",
    "host/tests/adapters/claude/",
];

/// Pack directories and test fixtures, where the reference pack's name is allowed: anything under a top-level
/// `packs/`, and anything under a directory named `fixtures`.
fn may_name_the_reference_pack(rel: &str) -> bool {
    rel.starts_with("packs/") || rel.split('/').any(|part| part == "fixtures")
}

fn is_claude_adapter(rel: &str) -> bool {
    ADAPTER_PATHS.iter().any(|a| {
        if a.ends_with('/') {
            rel.starts_with(a)
        } else {
            rel == *a
        }
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    /// Relative to the root, with `/` separators.
    pub path: String,
    /// 1-based.
    pub line: usize,
    pub rule: &'static str,
}

impl fmt::Display for Violation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let why = match self.rule {
            RULE_REFERENCE_PACK => {
                "the reference pack's name belongs only under packs/ and test fixtures"
            }
            _ => "a Claude env var name belongs only in the Claude adapter",
        };
        write!(f, "{}:{}: {}: {why}", self.path, self.line, self.rule)
    }
}

/// Every violation under `root`, in path and line order.
pub fn check(root: &Path) -> io::Result<Vec<Violation>> {
    let reference_pack = ["orch", "ard"].concat();
    let mut found = Vec::new();
    for entry in walk::files(root, &SCANNED, &[])? {
        let name_allowed = may_name_the_reference_pack(&entry.rel);
        let env_allowed = is_claude_adapter(&entry.rel);
        if name_allowed && env_allowed {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&entry.path) else {
            continue;
        };
        for (index, line) in text.lines().enumerate() {
            if !name_allowed && line.to_ascii_lowercase().contains(&reference_pack) {
                found.push(Violation {
                    path: entry.rel.clone(),
                    line: index + 1,
                    rule: RULE_REFERENCE_PACK,
                });
            }
            if !env_allowed && names_a_claude_env_var(line) {
                found.push(Violation {
                    path: entry.rel.clone(),
                    line: index + 1,
                    rule: RULE_CLAUDE_ENV,
                });
            }
        }
    }
    Ok(found)
}

/// Whether `line` holds a word that starts with one of the Claude env var prefixes. The word boundary keeps an
/// identifier such as `RULE_<prefix>` from counting.
fn names_a_claude_env_var(line: &str) -> bool {
    let claude_code = ["CLAUDE", "CODE"].concat();
    let claude_family = ["CLAUDE", "_"].concat();
    let anthropic_family = ["ANTHROPIC", "_"].concat();
    [claude_code, claude_family, anthropic_family]
        .iter()
        .any(|prefix| {
            line.match_indices(prefix.as_str()).any(|(at, _)| {
                line[..at]
                    .chars()
                    .next_back()
                    .is_none_or(|c| !(c.is_ascii_alphanumeric() || c == '_'))
            })
        })
}
