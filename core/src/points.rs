//! Named points (0.4) and the derived points a pack declares over them.

use crate::stage::Stage;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

/// The object a point is attached to (0.4, Named points).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Object {
    Run,
    Workspace,
    Phase,
    Task,
    Experiment,
}

/// The point catalogue (0.4, Named points), in the table's order. Serialized as its dotted name. Adding a point is
/// a schema change: `Point.input.json` lists the names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum Point {
    #[serde(rename = "run.before_start")]
    RunBeforeStart,
    #[serde(rename = "run.after_end")]
    RunAfterEnd,
    #[serde(rename = "run.before_integrate")]
    RunBeforeIntegrate,
    #[serde(rename = "run.after_integrate")]
    RunAfterIntegrate,
    #[serde(rename = "workspace.after_create")]
    WorkspaceAfterCreate,
    #[serde(rename = "workspace.before_remove")]
    WorkspaceBeforeRemove,
    #[serde(rename = "phase.before_start")]
    PhaseBeforeStart,
    #[serde(rename = "phase.after_end")]
    PhaseAfterEnd,
    #[serde(rename = "task.before_status")]
    TaskBeforeStatus,
    #[serde(rename = "task.after_status")]
    TaskAfterStatus,
    #[serde(rename = "task.before_op")]
    TaskBeforeOp,
    #[serde(rename = "task.after_op")]
    TaskAfterOp,
    #[serde(rename = "task.before_move")]
    TaskBeforeMove,
    #[serde(rename = "task.after_move")]
    TaskAfterMove,
    #[serde(rename = "experiment.before_arm")]
    ExperimentBeforeArm,
    #[serde(rename = "experiment.after_arm")]
    ExperimentAfterArm,
}

impl Point {
    /// Every point, in the catalogue's order.
    pub const ALL: [Point; 16] = [
        Point::RunBeforeStart,
        Point::RunAfterEnd,
        Point::RunBeforeIntegrate,
        Point::RunAfterIntegrate,
        Point::WorkspaceAfterCreate,
        Point::WorkspaceBeforeRemove,
        Point::PhaseBeforeStart,
        Point::PhaseAfterEnd,
        Point::TaskBeforeStatus,
        Point::TaskAfterStatus,
        Point::TaskBeforeOp,
        Point::TaskAfterOp,
        Point::TaskBeforeMove,
        Point::TaskAfterMove,
        Point::ExperimentBeforeArm,
        Point::ExperimentAfterArm,
    ];

    /// The dotted name, as a pack writes it: `run.before_start`.
    pub fn name(self) -> &'static str {
        match self {
            Point::RunBeforeStart => "run.before_start",
            Point::RunAfterEnd => "run.after_end",
            Point::RunBeforeIntegrate => "run.before_integrate",
            Point::RunAfterIntegrate => "run.after_integrate",
            Point::WorkspaceAfterCreate => "workspace.after_create",
            Point::WorkspaceBeforeRemove => "workspace.before_remove",
            Point::PhaseBeforeStart => "phase.before_start",
            Point::PhaseAfterEnd => "phase.after_end",
            Point::TaskBeforeStatus => "task.before_status",
            Point::TaskAfterStatus => "task.after_status",
            Point::TaskBeforeOp => "task.before_op",
            Point::TaskAfterOp => "task.after_op",
            Point::TaskBeforeMove => "task.before_move",
            Point::TaskAfterMove => "task.after_move",
            Point::ExperimentBeforeArm => "experiment.before_arm",
            Point::ExperimentAfterArm => "experiment.after_arm",
        }
    }

    /// The stage that makes this point fire (0.4: "A point fires once the stage that owns it lands").
    pub fn stage(self) -> Stage {
        match self {
            Point::RunBeforeStart
            | Point::RunAfterEnd
            | Point::WorkspaceAfterCreate
            | Point::WorkspaceBeforeRemove => Stage::S7,
            Point::RunBeforeIntegrate
            | Point::RunAfterIntegrate
            | Point::PhaseBeforeStart
            | Point::PhaseAfterEnd
            | Point::TaskBeforeStatus
            | Point::TaskAfterStatus
            | Point::TaskBeforeOp
            | Point::TaskAfterOp => Stage::S8,
            Point::TaskBeforeMove | Point::TaskAfterMove => Stage::S12,
            Point::ExperimentBeforeArm | Point::ExperimentAfterArm => Stage::S15,
        }
    }

    /// The object the point is on: the part of the name before its dot, as an enum.
    pub fn object(self) -> Object {
        match self {
            Point::RunBeforeStart
            | Point::RunAfterEnd
            | Point::RunBeforeIntegrate
            | Point::RunAfterIntegrate => Object::Run,
            Point::WorkspaceAfterCreate | Point::WorkspaceBeforeRemove => Object::Workspace,
            Point::PhaseBeforeStart | Point::PhaseAfterEnd => Object::Phase,
            Point::TaskBeforeStatus
            | Point::TaskAfterStatus
            | Point::TaskBeforeOp
            | Point::TaskAfterOp
            | Point::TaskBeforeMove
            | Point::TaskAfterMove => Object::Task,
            Point::ExperimentBeforeArm | Point::ExperimentAfterArm => Object::Experiment,
        }
    }

    /// The point with this dotted name. A name that is not in the catalogue is refused, and the error names it.
    pub fn parse(name: &str) -> Result<Point, PointError> {
        Point::ALL
            .into_iter()
            .find(|point| point.name() == name)
            .ok_or_else(|| PointError::NotInCatalogue {
                point: name.to_string(),
            })
    }
}

/// A derived point (0.4): a name in the pack's namespace over one core point, with what it matches and a `when:`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DerivedPoint {
    #[schemars(length(min = 1))]
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
    NotInCatalogue { point: String },
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
            PointError::NotInCatalogue { point } => {
                write!(f, "`{point}` is not a point in the catalogue")
            }
        }
    }
}

impl std::error::Error for PointError {}

/// `"before"` or `"after"`, from the part of a point's name after its last dot.
pub(crate) fn side(point: &str) -> Option<&'static str> {
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
    known: &[Point],
) -> Result<(), Vec<PointError>> {
    let mut errors = Vec::new();
    for (name, point) in points {
        let (point_name, on) = (name.clone(), point.on.clone());
        if points.contains_key(&point.on) {
            errors.push(PointError::DerivedFromDerived {
                point: point_name,
                on,
            });
        } else if !known.iter().any(|core| core.name() == point.on) {
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
