//! A string that is never empty. An id, an outcome or a section's text that arrives as `""` is refused when it is
//! read, and the schema says so with `minLength: 1`, so a blank value never stands in for a missing one (0.6).

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::fmt;

/// A string with at least one character.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct NonEmpty(String);

/// The text was empty.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmptyText;

impl fmt::Display for EmptyText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("must not be empty")
    }
}

impl std::error::Error for EmptyText {}

impl NonEmpty {
    pub fn new(text: impl Into<String>) -> Result<NonEmpty, EmptyText> {
        NonEmpty::try_from(text.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for NonEmpty {
    type Error = EmptyText;

    fn try_from(text: String) -> Result<NonEmpty, EmptyText> {
        if text.is_empty() {
            Err(EmptyText)
        } else {
            Ok(NonEmpty(text))
        }
    }
}

impl TryFrom<&str> for NonEmpty {
    type Error = EmptyText;

    fn try_from(text: &str) -> Result<NonEmpty, EmptyText> {
        NonEmpty::try_from(text.to_string())
    }
}

impl From<NonEmpty> for String {
    fn from(text: NonEmpty) -> String {
        text.0
    }
}

impl JsonSchema for NonEmpty {
    fn inline_schema() -> bool {
        true
    }

    fn schema_name() -> Cow<'static, str> {
        Cow::Borrowed("NonEmpty")
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({ "type": "string", "minLength": 1 })
    }
}
