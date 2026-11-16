//! The one YAML writer in `core` (0.7). Collide writes five files that the loader only reads: `board.yaml`,
//! `trust.yaml`, `packs.yaml`, the two `packs.lock` files and the user's `config.yaml`. [`to_yaml`] turns any
//! `Serialize` value into the text of one of them, and [`crate::yaml::load_yaml`] reads that text back equal.
//!
//! The rules are few, so no string needs a quoting rule and no value can change type on the way back:
//! - a string is always double-quoted, with the escapes JSON has, and every control character, line separator and
//!   byte-order mark as `\uXXXX`. Written this way `"yes"`, `"1"` and `"null"` stay strings;
//! - a key is plain when it is a word (a letter or `_`, then letters, digits, `_` or `-`) that is not a word the
//!   YAML 1.1 reads as a boolean or `null`; any other key is quoted as a string is;
//! - maps and lists are in block style, in the order the type declares its fields. An empty one is `{}` or `[]`;
//! - a float keeps its `.0`, so it is read back as a float;
//! - the text ends with one newline and has no trailing space.
//!
//! The value is first written as JSON text, which keeps the order of every map as the type wrote it, and read into
//! [`Node`], an ordered tree. A comment in a file a person edited is not in the value, so rewriting such a file
//! loses it: the header argument puts back a comment of Collide's own, and the caller decides what to do about a
//! comment it finds.

use serde::Serialize;
use serde::de::{Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
use std::fmt;

/// A parsed JSON value that keeps the order of a map's keys. A number is kept as the text it is written with.
enum Node {
    Null,
    Bool(bool),
    Number(String),
    Text(String),
    List(Vec<Node>),
    Map(Vec<(String, Node)>),
}

struct NodeVisitor;

impl<'de> Visitor<'de> for NodeVisitor {
    type Value = Node;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("any JSON value")
    }

    fn visit_unit<E>(self) -> Result<Node, E> {
        Ok(Node::Null)
    }

    fn visit_bool<E>(self, value: bool) -> Result<Node, E> {
        Ok(Node::Bool(value))
    }

    fn visit_i64<E>(self, value: i64) -> Result<Node, E> {
        Ok(Node::Number(value.to_string()))
    }

    fn visit_u64<E>(self, value: u64) -> Result<Node, E> {
        Ok(Node::Number(value.to_string()))
    }

    fn visit_f64<E: serde::de::Error>(self, value: f64) -> Result<Node, E> {
        // `serde_json` writes `1.0` for a whole float; `f64`'s own text would be `1`, an integer.
        serde_json::Number::from_f64(value)
            .map(|number| Node::Number(number.to_string()))
            .ok_or_else(|| E::custom("a float that is not finite has no YAML form here"))
    }

    fn visit_str<E>(self, value: &str) -> Result<Node, E> {
        Ok(Node::Text(value.to_string()))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Node, A::Error> {
        let mut items = Vec::new();
        while let Some(item) = seq.next_element()? {
            items.push(item);
        }
        Ok(Node::List(items))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Node, A::Error> {
        let mut entries = Vec::new();
        while let Some(entry) = map.next_entry::<String, Node>()? {
            entries.push(entry);
        }
        Ok(Node::Map(entries))
    }
}

impl<'de> Deserialize<'de> for Node {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Node, D::Error> {
        deserializer.deserialize_any(NodeVisitor)
    }
}

/// Words that YAML 1.1 reads as a boolean or `null`. A key that is one of them is quoted, in any case.
const RESERVED_WORDS: [&str; 9] = ["true", "false", "null", "yes", "no", "on", "off", "y", "n"];

fn is_plain_key(key: &str) -> bool {
    let mut chars = key.chars();
    chars
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        && !RESERVED_WORDS
            .iter()
            .any(|word| key.eq_ignore_ascii_case(word))
}

/// A string as a YAML double-quoted scalar.
fn quote(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() || matches!(c, '\u{2028}' | '\u{2029}' | '\u{feff}') => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn key_text(key: &str) -> String {
    if is_plain_key(key) {
        key.to_string()
    } else {
        quote(key)
    }
}

/// The text of a value that takes one line: a scalar, or an empty map or list. `None` for a map or list with items.
fn inline(node: &Node) -> Option<String> {
    match node {
        Node::Null => Some("null".to_string()),
        Node::Bool(value) => Some(value.to_string()),
        Node::Number(text) => Some(text.clone()),
        Node::Text(text) => Some(quote(text)),
        Node::List(items) if items.is_empty() => Some("[]".to_string()),
        Node::Map(entries) if entries.is_empty() => Some("{}".to_string()),
        Node::List(_) | Node::Map(_) => None,
    }
}

/// Writes the lines of a map or list that has items, each indented by `indent` spaces.
fn block(node: &Node, indent: usize, out: &mut String) {
    let pad = " ".repeat(indent);
    match node {
        Node::Map(entries) => {
            for (key, value) in entries {
                out.push_str(&format!("{pad}{}:", key_text(key)));
                match inline(value) {
                    Some(text) => out.push_str(&format!(" {text}\n")),
                    None => {
                        out.push('\n');
                        block(value, indent + 2, out);
                    }
                }
            }
        }
        Node::List(items) => {
            for item in items {
                match (inline(item), item) {
                    (Some(text), _) => out.push_str(&format!("{pad}- {text}\n")),
                    // A map starts on the dash's line: its first line is written with the dash in place of its indent.
                    (None, Node::Map(_)) => {
                        let mut lines = String::new();
                        block(item, indent + 2, &mut lines);
                        out.push_str(&format!("{pad}- {}", &lines[indent + 2..]));
                    }
                    (None, _) => {
                        out.push_str(&format!("{pad}-\n"));
                        block(item, indent + 2, out);
                    }
                }
            }
        }
        scalar => {
            out.push_str(&format!("{pad}{}\n", inline(scalar).unwrap_or_default()));
        }
    }
}

/// The comment block: each line of `header` as `# line`, an empty line as `#`. Nothing for an empty header. A line
/// separator other than `\n` and `\r\n` is a space, so no line of the header can end its comment early.
fn comment_block(header: &str) -> String {
    header
        .replace(['\u{85}', '\u{2028}', '\u{2029}'], " ")
        .lines()
        .map(|line| {
            if line.is_empty() {
                "#\n".to_string()
            } else {
                format!("# {line}\n")
            }
        })
        .collect()
}

/// Turns `value` into YAML text, after a comment block made of `header` (pass `""` for none). The error is the
/// value's own: a map whose key is not text, or a float that is not finite.
pub fn to_yaml<T: Serialize + ?Sized>(
    value: &T,
    header: &str,
) -> Result<String, serde_json::Error> {
    let json = serde_json::to_string(value)?;
    let node: Node = serde_json::from_str(&json)?;
    let mut out = comment_block(header);
    match inline(&node) {
        Some(text) => out.push_str(&format!("{text}\n")),
        None => block(&node, 0, &mut out),
    }
    Ok(out)
}

/// Whether `text` has a YAML comment: a `#` outside a quoted scalar that starts a line or follows a space or tab. A
/// reading on the safe side: a `#` inside a block scalar counts, so a file is refused a rewrite when in doubt, and
/// a comment is never dropped by one.
pub fn has_comment(text: &str) -> bool {
    // A quote opens a quoted scalar only where a scalar can start; an apostrophe inside a plain word does not.
    const SCALAR_STARTS: [char; 6] = [':', '-', ',', '[', '{', '?'];
    let mut quote: Option<char> = None;
    let mut previous = '\n';
    let mut last_significant: Option<char> = None;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if let Some(open) = quote {
            // A backslash escape in a double-quoted scalar, and a doubled apostrophe in a single-quoted one, each
            // take the next character with them.
            let escaped = (open == '"' && c == '\\')
                || (open == '\'' && c == open && chars.peek() == Some(&'\''));
            if escaped {
                chars.next();
            } else if c == open {
                quote = None;
            }
        } else if c == '#' && matches!(previous, '\n' | ' ' | '\t') {
            return true;
        } else if (c == '"' || c == '\'')
            && last_significant.is_none_or(|last| SCALAR_STARTS.contains(&last))
        {
            quote = Some(c);
        }
        if c == '\n' {
            last_significant = None;
        } else if !c.is_whitespace() && quote.is_none() {
            last_significant = Some(c);
        }
        previous = c;
    }
    false
}
