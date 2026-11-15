//! Named points (0.4) and the derived points a pack declares over them.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

/// The core points this item defines, in the catalogue's order: the five new task points and
/// `task.before_status`, which 0.4 already named. This is the start of the point catalogue, which `ts-0018-dd`
/// completes; the check below takes the set as an argument so the catalogue can supply it.
pub const CORE_POINTS: &[&str] = &[
    "task.before_status",
    "task.after_status",
    "task.before_op",
    "task.after_op",
    "task.before_move",
    "task.after_move",
];

/// A derived point (0.4): a name in the pack's namespace over one core point, with what it matches and a `when:`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DerivedPoint {
    pub on: String,
    #[serde(rename = "match", default, skip_serializing_if = "Option::is_none")]
    pub matches: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PointError {
    UnknownCorePoint { point: String, on: String },
    DerivedFromDerived { point: String, on: String },
    SideMismatch { point: String, on: String },
    NoSide { point: String },
}

impl fmt::Display for PointError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PointError::UnknownCorePoint { point, on } => {
                write!(
                    f,
                    "derived point `{point}` is on `{on}`, which is not a core point"
                )
            }
            PointError::DerivedFromDerived { point, on } => {
                write!(
                    f,
                    "derived point `{point}` is on `{on}`, which is a derived point"
                )
            }
            PointError::SideMismatch { point, on } => {
                write!(
                    f,
                    "derived point `{point}` is on the other side (before or after) of `{on}`"
                )
            }
            PointError::NoSide { point } => {
                write!(
                    f,
                    "derived point `{point}` has no `before_` or `after_` side in its name"
                )
            }
        }
    }
}

impl std::error::Error for PointError {}

/// `"before"` or `"after"`, from the part of a point's name after its last dot.
fn side(point: &str) -> Option<&'static str> {
    let name = point.rsplit('.').next().unwrap_or(point);
    if name.starts_with("before_") {
        Some("before")
    } else if name.starts_with("after_") {
        Some("after")
    } else {
        None
    }
}

/// Checks a pack's `points:` against the known core points. A derived point must be on a known core point, never
/// on another derived point, and on the same side as it: `before_*` from `before_*`. Returns every refusal, in
/// the order of the points' names.
pub fn check_derived_points(
    points: &BTreeMap<String, DerivedPoint>,
    known_core_points: &[&str],
) -> Result<(), Vec<PointError>> {
    let mut errors = Vec::new();
    for (name, point) in points {
        let (point_name, on) = (name.clone(), point.on.clone());
        if points.contains_key(&point.on) {
            errors.push(PointError::DerivedFromDerived {
                point: point_name,
                on,
            });
        } else if !known_core_points.contains(&point.on.as_str()) {
            errors.push(PointError::UnknownCorePoint {
                point: point_name,
                on,
            });
        } else if side(name).is_none() {
            errors.push(PointError::NoSide { point: point_name });
        } else if side(name) != side(&point.on) {
            errors.push(PointError::SideMismatch {
                point: point_name,
                on,
            });
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}
