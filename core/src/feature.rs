//! A task's link to a feature (revision 14; 0.8, 0.10 "A proposal's features"): the parse of a feature target such as
//! `pr-0003-km#F2`, and the state a feature has, which is derived and never written. Both are pure: a provider and the
//! CLI call the same functions, so they agree.

use crate::item::Category;
use std::fmt;

/// A feature target read apart: the id of the proposal, and the number of the feature in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeatureTarget {
    pub item: String,
    pub number: u32,
}

impl FeatureTarget {
    /// The feature as the proposal's Features part names it: `F2`.
    pub fn feature_id(&self) -> String {
        format!("F{}", self.number)
    }
}

/// Why a link target is not a feature target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FeatureTargetError {
    /// There is no `#`.
    NoFragment,
    /// Nothing comes before the `#`.
    NoItem,
    /// More than one `#`.
    TwoFragments,
    /// What follows the `#` is not `F` and a number from 1 with no leading zero.
    BadFragment(String),
}

impl fmt::Display for FeatureTargetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FeatureTargetError::NoFragment => {
                write!(f, "a feature target names a feature, as pr-0003-km#F2")
            }
            FeatureTargetError::NoItem => {
                write!(
                    f,
                    "a feature target starts with the id of a proposal, as pr-0003-km#F2"
                )
            }
            FeatureTargetError::TwoFragments => {
                write!(f, "a feature target has one `#`, as pr-0003-km#F2")
            }
            FeatureTargetError::BadFragment(fragment) => write!(
                f,
                "the feature `#{fragment}` must be F and a number from 1 with no leading zero, as #F2"
            ),
        }
    }
}

impl std::error::Error for FeatureTargetError {}

/// The number of a fragment `F<n>`, `n` from 1 with no leading zero.
fn feature_number(fragment: &str) -> Option<u32> {
    let digits = fragment.strip_prefix('F')?;
    let first = digits.bytes().next()?;
    if !(b'1'..=b'9').contains(&first) || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
}

/// Reads a feature target, `<item id>#F<n>`. Core does not know an id's pattern (a provider declares it), so the item
/// part is only checked to be there; whether it names a proposal with that feature is the provider's to check.
pub fn parse_feature_target(to: &str) -> Result<FeatureTarget, FeatureTargetError> {
    let Some((item, fragment)) = to.split_once('#') else {
        return Err(FeatureTargetError::NoFragment);
    };
    if fragment.contains('#') {
        return Err(FeatureTargetError::TwoFragments);
    }
    if item.is_empty() {
        return Err(FeatureTargetError::NoItem);
    }
    match feature_number(fragment) {
        Some(number) => Ok(FeatureTarget {
            item: item.to_string(),
            number,
        }),
        None => Err(FeatureTargetError::BadFragment(fragment.to_string())),
    }
}

/// A proposal's six statuses, in the order 0.10 gives them.
const PROPOSAL_STATUSES: [&str; 6] = [
    "draft",
    "proposed",
    "accepted",
    "deferred",
    "rejected",
    "superseded",
];

/// The state of a feature (0.10): `planned`, `deferred`, `dropped`, `accepted`, "n of m done" or `done`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeatureState {
    Planned,
    Deferred,
    Dropped,
    Accepted,
    /// `done` of `total` tasks done, over the tasks that are not dropped.
    Progress {
        done: u32,
        total: u32,
    },
    Done,
}

impl fmt::Display for FeatureState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FeatureState::Planned => write!(f, "planned"),
            FeatureState::Deferred => write!(f, "deferred"),
            FeatureState::Dropped => write!(f, "dropped"),
            FeatureState::Accepted => write!(f, "accepted"),
            FeatureState::Progress { done, total } => write!(f, "{done} of {total} done"),
            FeatureState::Done => write!(f, "done"),
        }
    }
}

/// A status that is none of a proposal's six.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownProposalStatus(pub String);

impl fmt::Display for UnknownProposalStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "`{}` is not a proposal status; the statuses are {}",
            self.0,
            PROPOSAL_STATUSES.join(", ")
        )
    }
}

impl std::error::Error for UnknownProposalStatus {}

/// The state of a feature, from its proposal's status and the categories of the tasks that implement it. The proposal
/// decides until it is `accepted`: `planned` while it is `draft` or `proposed`, `deferred` with it, `dropped` when it is
/// `rejected` or `superseded`. Once accepted, the feature is `accepted` until a task that is not dropped implements it,
/// then "n of m done" over those tasks, and `done` when all of them are.
pub fn derived_state(
    proposal_status: &str,
    tasks: &[Category],
) -> Result<FeatureState, UnknownProposalStatus> {
    match proposal_status {
        "draft" | "proposed" => Ok(FeatureState::Planned),
        "deferred" => Ok(FeatureState::Deferred),
        "rejected" | "superseded" => Ok(FeatureState::Dropped),
        "accepted" => {
            let live: Vec<&Category> = tasks
                .iter()
                .filter(|category| **category != Category::Dropped)
                .collect();
            let total = live.len() as u32;
            let done = live
                .iter()
                .filter(|category| ***category == Category::Done)
                .count() as u32;
            Ok(match (total, done) {
                (0, _) => FeatureState::Accepted,
                (total, done) if done == total => FeatureState::Done,
                (total, done) => FeatureState::Progress { done, total },
            })
        }
        other => Err(UnknownProposalStatus(other.to_string())),
    }
}
