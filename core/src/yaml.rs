//! The one YAML entry point in `core` (0.7). Every manifest and settings file loads through [`load_yaml`] or
//! [`load_yaml_file`]; nothing else in the workspace calls `serde-saphyr` or reads YAML any other way.
//!
//! YAML is untrusted input (0.9), so the library is never used with its defaults. The options, in
//! `hardened_options`, are:
//! - `strict_booleans`: only `true` and `false` (any case) are booleans. The defaults read the YAML 1.1 forms, so
//!   `no` would become `false` in a bool field or an untyped value;
//! - `no_schema`: an unquoted scalar that looks like a number or a boolean (`0.10`, `1e10`, `true`) is an error
//!   in a `String` field, which has to be quoted. A quoted one, and `no`, stay strings;
//! - `merge_keys: Error`: `<<` is an error;
//! - a budget of no anchors and no aliases (so an alias bomb cannot start), at most 100,000 nodes, and nesting
//!   at most 32 levels deep, in block and in flow style;
//! - `duplicate_keys: Error`: a key written twice is an error, in a struct and in a map. This is also the
//!   library's default, and it is set by name so that a change of the default cannot weaken it.
//!
//! The file size is capped before anything is parsed: [`MAX_YAML_BYTES`].
//!
//! Typed targets carry the rest: derive `#[serde(deny_unknown_fields)]` on every struct (an unknown key is then an
//! error whose key path is in [`YamlError::Invalid`]), and use `String` for text, never an untyped value.

use serde::de::DeserializeOwned;
use serde_saphyr::budget::BudgetBreach;
use serde_saphyr::{Error as SaphyrError, MergeKeyPolicy, Options};
use std::fmt;
use std::fs::File;
use std::io::Read;
use std::path::Path;

/// The largest YAML file or byte string the loader parses: 256 KiB. Manifests and settings are kilobytes; the S5
/// spike suggested 64 KiB, and this leaves four times that room. It is also about the size at which a compact
/// document reaches the node cap, so the size cap and [`MAX_NODES`] bound the work together: a bigger file is
/// refused without reading it all.
pub const MAX_YAML_BYTES: usize = 256 * 1024;

/// The most nodes (scalars, sequences and mappings) a document may have.
pub const MAX_NODES: usize = 100_000;

/// The deepest nesting a document may have, counting the root mapping.
pub const MAX_DEPTH: usize = 32;

/// The hint added to an error at an unquoted `{{ }}`.
pub const QUOTE_TEMPLATE_HINT: &str = "quote the template";

/// Why a document was not loaded. Every variant names where it came from (`origin`, a file path or a label).
#[derive(Debug)]
pub enum YamlError {
    /// The input is over [`MAX_YAML_BYTES`]. It was not parsed.
    TooLarge { origin: String, cap: usize },
    /// The file could not be opened or read.
    Read {
        origin: String,
        source: std::io::Error,
    },
    /// The text is not valid YAML, is not UTF-8, breaks one of the hardened rules, or does not fit the type.
    /// `line` and `column` start at 1, where the library gives them. `key_path` is the path to the key
    /// (`workflows.build.speed`), where the library was reading one. `hint` says how to fix a known mistake.
    Invalid {
        origin: String,
        line: Option<usize>,
        column: Option<usize>,
        key_path: Option<String>,
        message: String,
        hint: Option<&'static str>,
    },
}

impl fmt::Display for YamlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            YamlError::TooLarge { origin, cap } => write!(
                f,
                "{origin}: the file is larger than the cap of {cap} bytes, so it was not parsed"
            ),
            YamlError::Read { origin, source } => {
                write!(f, "{origin}: cannot read the file: {source}")
            }
            YamlError::Invalid {
                origin,
                line,
                column,
                key_path,
                message,
                hint,
            } => {
                match (line, column) {
                    (Some(line), Some(column)) => write!(f, "{origin}:{line}:{column}: ")?,
                    _ => write!(f, "{origin}: ")?,
                }
                write!(f, "{message}")?;
                if let Some(path) = key_path {
                    write!(f, " (key path: {path})")?;
                }
                if let Some(hint) = hint {
                    write!(f, " (hint: {hint})")?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for YamlError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            YamlError::Read { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// The hardened option set. Nothing here is left to the library's defaults; see the module docs.
fn hardened_options() -> Options {
    serde_saphyr::options! {
        strict_booleans: true,
        no_schema: true,
        merge_keys: MergeKeyPolicy::Error,
        duplicate_keys: serde_saphyr::DuplicateKeyPolicy::Error,
        with_snippet: false,
        budget: serde_saphyr::budget! {
            max_aliases: 0,
            max_anchors: 0,
            max_nodes: MAX_NODES,
            max_depth: MAX_DEPTH,
            flow_nesting_limit: MAX_DEPTH
        },
    }
}

/// Parses `bytes` as one YAML document of type `T`. `origin` is a file path or a label and is named in every
/// error. Input over [`MAX_YAML_BYTES`] is refused before it is parsed.
pub fn load_yaml<T: DeserializeOwned>(bytes: &[u8], origin: &str) -> Result<T, YamlError> {
    if bytes.len() > MAX_YAML_BYTES {
        return Err(YamlError::TooLarge {
            origin: origin.to_string(),
            cap: MAX_YAML_BYTES,
        });
    }
    let text = std::str::from_utf8(bytes).map_err(|error| YamlError::Invalid {
        origin: origin.to_string(),
        line: None,
        column: None,
        key_path: None,
        message: format!(
            "the text is not valid UTF-8 (first bad byte at offset {})",
            error.valid_up_to()
        ),
        hint: None,
    })?;

    let mut key_path = None;
    let result = serde_saphyr::with_deserializer_from_str_with_options(
        text,
        hardened_options(),
        |deserializer| {
            serde_path_to_error::deserialize::<_, T>(deserializer).map_err(|error| {
                key_path = Some(error.path().to_string());
                error.into_inner()
            })
        },
    );
    result.map_err(|error| invalid(origin, text, &error, key_path))
}

/// Reads the file at `path` and loads it as [`load_yaml`] does, with the path as the origin. The size is
/// checked from the file's metadata before the file is read, and the read stops one byte past the cap, so a file
/// that grows meanwhile cannot get past it.
pub fn load_yaml_file<T: DeserializeOwned>(path: &Path) -> Result<T, YamlError> {
    let origin = path.display().to_string();
    let read_error = |source| YamlError::Read {
        origin: origin.clone(),
        source,
    };
    let file = File::open(path).map_err(read_error)?;
    let size = file.metadata().map_err(read_error)?.len();
    if size > MAX_YAML_BYTES as u64 {
        return Err(YamlError::TooLarge {
            origin,
            cap: MAX_YAML_BYTES,
        });
    }
    let mut bytes = Vec::with_capacity(size as usize);
    file.take(MAX_YAML_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(read_error)?;
    load_yaml(&bytes, &origin)
}

/// Turns the library's error into ours: the hardened rules in words an author understands, the position, the key
/// path, and the template hint.
fn invalid(origin: &str, text: &str, error: &SaphyrError, key_path: Option<String>) -> YamlError {
    let location = error.location();
    let line = location.as_ref().map(|l| l.line() as usize);
    let column = location.as_ref().map(|l| l.column() as usize);
    let hint = match (line, column) {
        (Some(line), Some(column)) if starts_with_template(text, line, column) => {
            Some(QUOTE_TEMPLATE_HINT)
        }
        _ => None,
    };
    YamlError::Invalid {
        origin: origin.to_string(),
        line,
        column,
        // `.` is the root, and a `?` is a key the library could not name (a duplicate or a merge key)
        key_path: key_path.filter(|path| path != "." && !path.contains('?')),
        message: explain(error).unwrap_or_else(|| plain_message(error, line, column)),
        hint,
    }
}

/// The message for an error that breaks one of the hardened rules. `None` leaves the library's own text.
fn explain(error: &SaphyrError) -> Option<String> {
    match error {
        SaphyrError::AliasError { error, .. } | SaphyrError::WithSnippet { error, .. } => {
            explain(error)
        }
        SaphyrError::Budget { breach, .. } => match breach {
            BudgetBreach::Anchors { .. } => {
                Some("YAML anchors are not allowed: write the value out".into())
            }
            BudgetBreach::Aliases { .. } => {
                Some("YAML aliases are not allowed: write the value out".into())
            }
            BudgetBreach::Depth { .. } => Some(format!(
                "the document is nested deeper than {MAX_DEPTH} levels"
            )),
            BudgetBreach::Nodes { .. } => Some("the document has more than 100,000 nodes".into()), // MAX_NODES, with its comma
            _ => None,
        },
        SaphyrError::UnknownAnchor { .. } => {
            Some("YAML aliases are not allowed: write the value out".into())
        }
        SaphyrError::MergeKeyNotAllowed { .. } => {
            Some("YAML merge keys are not allowed: write the keys out".into())
        }
        _ => None,
    }
}

/// The library's text without its trailing " at line L, column C", which the error carries separately.
fn plain_message(error: &SaphyrError, line: Option<usize>, column: Option<usize>) -> String {
    let text = error.to_string();
    // the parser's own wording for flow nesting over `flow_nesting_limit`, which is set to `MAX_DEPTH`
    if text.starts_with("recursion limit exceeded") {
        return format!("the document is nested deeper than {MAX_DEPTH} levels");
    }
    if let (Some(line), Some(column)) = (line, column) {
        let suffix = format!(" at line {line}, column {column}");
        if let Some(stripped) = text.strip_suffix(&suffix) {
            return stripped.to_string();
        }
    }
    text
}

/// Whether the text at `line` and `column` (both from 1) starts a `{{`: an unquoted template, which YAML reads
/// as a mapping inside a mapping.
fn starts_with_template(text: &str, line: usize, column: usize) -> bool {
    text.lines().nth(line.saturating_sub(1)).is_some_and(|row| {
        row.chars()
            .skip(column.saturating_sub(1))
            .take(2)
            .eq("{{".chars())
    })
}
