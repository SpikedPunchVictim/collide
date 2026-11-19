//! The `roles` setting of `collide/plan` and its check against a source's kinds (ts-0135-cx; 0.10, Planning
//! sources). A role is something the planning flow does (`proposal`, `question`, `spike`, `decision`, `deferred`); the
//! setting maps each to the kinds of the board's planning source that play it. Pure: no I/O.
//!
//! - **A role is on** when it names at least one kind the source has. A kind the source lacks is a [`RoleProblem`]
//!   that names the role and the kind; the source still works.
//! - **A role is off** when it names no kind, is not in the map, or none of the kinds it names is the source's. Each
//!   off role has a reason ([`RoleOff`]). An empty role is a choice, so it is off and no problem.

use crate::item::KindDecl;
use serde_json::Value;
use std::collections::BTreeMap;
use std::fmt;

/// The roles, in the order they are checked and reported.
pub const ROLES: [&str; 5] = ["proposal", "question", "spike", "decision", "deferred"];

/// A role to the names of the kinds that play it.
pub type Roles = BTreeMap<String, Vec<String>>;

/// A kind a role names that the source does not have.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoleProblem {
    pub role: String,
    pub kind: String,
    pub source: String,
}

impl fmt::Display for RoleProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "the role `{}` names the kind `{}`, which the source `{}` does not have",
            self.role, self.kind, self.source
        )
    }
}

/// A role that does nothing on this source, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoleOff {
    pub role: String,
    pub reason: String,
}

/// What [`check_roles`] found.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RoleCheck {
    pub problems: Vec<RoleProblem>,
    pub off: Vec<RoleOff>,
}

/// The map a `roles` value holds, or why it is not one. The setting's schema has already said what a layer may write,
/// so this is for a value that came by another way.
pub fn read_roles(value: &Value) -> Result<Roles, String> {
    serde_json::from_value(value.clone())
        .map_err(|error| format!("`roles` is a map from a role to a list of kind names: {error}"))
}

/// Checks `roles` against the `kinds` of the source called `source`.
pub fn check_roles(roles: &Roles, source: &str, kinds: &[KindDecl]) -> RoleCheck {
    let mut check = RoleCheck::default();
    for role in ROLES {
        let named = roles.get(role).map(Vec::as_slice).unwrap_or_default();
        if named.is_empty() {
            let reason = if roles.contains_key(role) {
                "it names no kind"
            } else {
                "it is not mapped"
            };
            check.off.push(RoleOff {
                role: role.to_string(),
                reason: reason.to_string(),
            });
            continue;
        }
        let missing: Vec<&String> = named
            .iter()
            .filter(|name| !kinds.iter().any(|kind| &kind.name == *name))
            .collect();
        check
            .problems
            .extend(missing.iter().map(|kind| RoleProblem {
                role: role.to_string(),
                kind: (*kind).clone(),
                source: source.to_string(),
            }));
        if missing.len() == named.len() {
            check.off.push(RoleOff {
                role: role.to_string(),
                reason: format!("the source `{source}` has none of the kinds it names"),
            });
        }
    }
    check
}
