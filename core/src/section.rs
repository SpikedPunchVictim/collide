//! The sections of proposals' features (revision 14; 0.10, "A proposal's features"). A section is named by the
//! `###` heading a feature is under. There is no registry: two names are one section when they match after trimming,
//! in any case, and near-duplicates are names that a person probably meant as one. Everything here is pure, so the
//! CLI's `collide sections` and the Planning view agree. A near-duplicate group is a suggestion shown to a person,
//! and nothing here merges anything.

use crate::item::Feature;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet};

/// The key two names share when they are one section: trimmed and lowercased. The empty key is the section of a
/// feature that is unsorted.
fn section_key(name: &str) -> String {
    name.trim().to_lowercase()
}

/// Whether two section names are one section: they match after trimming, in any case.
pub fn same_section(a: &str, b: &str) -> bool {
    section_key(a) == section_key(b)
}

/// The key two names share when they are near-duplicates: lowercased, with spaces, hyphens and underscores taken
/// out, and one final `s` dropped. There is no key for a name with nothing but separators in it, so such a name is
/// near nothing. An `s` that is all that is left is kept, so `s` alone is not taken for an empty name.
///
/// The rule is deliberately plain, and it has false matches that a person sorts out: `Bus` and `Bu` share a key.
pub fn near_duplicate_key(name: &str) -> Option<String> {
    let mut key: String = section_key(name)
        .chars()
        .filter(|c| !matches!(c, ' ' | '-' | '_'))
        .collect();
    if key.is_empty() {
        return None;
    }
    if key.len() > 1 && key.ends_with('s') {
        key.pop();
    }
    Some(key)
}

/// Whether two names are near-duplicates: one section, or one near-duplicate key (`Pack` and `packs`, `Task view`
/// and `task-view`).
pub fn near_duplicate(a: &str, b: &str) -> bool {
    if same_section(a, b) {
        return true;
    }
    match (near_duplicate_key(a), near_duplicate_key(b)) {
        (Some(a), Some(b)) => a == b,
        _ => false,
    }
}

/// The features of one proposal that is not dropped, as `section_uses` takes them.
#[derive(Debug, Clone, Copy)]
pub struct ProposalFeatures<'a> {
    /// The proposal's id.
    pub proposal: &'a str,
    pub features: &'a [Feature],
}

/// One section in use: its name as it is shown, how many features are in it and in how many proposals. The empty
/// `name` is the section of the features that are unsorted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SectionUse {
    pub name: String,
    pub feature_count: u32,
    pub proposal_count: u32,
    /// The ids of the proposals that have a feature in the section, in order.
    pub proposals: Vec<String>,
}

/// Sections that are near-duplicates of one another, as they are shown, in the order of `Sections.sections`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NearDuplicateGroup {
    pub names: Vec<String>,
}

/// The sections in use and the near-duplicate groups among them: the content of `collide sections --json`.
/// `sections` is sorted by feature count, highest first, then by name (in any case); the empty section is last.
/// `near_duplicates` lists groups of two or more sections, in the order of their first section.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Sections {
    pub sections: Vec<SectionUse>,
    pub near_duplicates: Vec<NearDuplicateGroup>,
}

/// What is known of one section while the features are counted.
#[derive(Default)]
struct Tally<'a> {
    features: u32,
    proposals: BTreeSet<&'a str>,
    /// Each spelling in use: how many features use it, and the lowest id of a proposal that does.
    spellings: BTreeMap<&'a str, (u32, &'a str)>,
}

impl<'a> Tally<'a> {
    /// The spelling most of the features use; on a tie, the one from the lowest proposal id; on a further tie, the
    /// one that sorts first.
    fn display_name(&self) -> &'a str {
        self.spellings
            .iter()
            .min_by_key(|(spelling, (count, lowest))| (Reverse(*count), *lowest, **spelling))
            .map(|(spelling, _)| *spelling)
            .unwrap_or_default()
    }
}

/// The sections in use over the features of proposals that are not dropped (the caller leaves the others out), and
/// the near-duplicate groups among them.
pub fn section_uses(proposals: &[ProposalFeatures]) -> Sections {
    let mut tallies: BTreeMap<String, Tally> = BTreeMap::new();
    for proposal in proposals {
        for feature in proposal.features {
            let spelling = feature.section.trim();
            let tally = tallies.entry(section_key(spelling)).or_default();
            tally.features += 1;
            tally.proposals.insert(proposal.proposal);
            let entry = tally
                .spellings
                .entry(spelling)
                .or_insert((0, proposal.proposal));
            entry.0 += 1;
            entry.1 = entry.1.min(proposal.proposal);
        }
    }
    let mut sections: Vec<(String, SectionUse)> = tallies
        .into_iter()
        .map(|(key, tally)| {
            let section = SectionUse {
                name: tally.display_name().to_string(),
                feature_count: tally.features,
                proposal_count: tally.proposals.len() as u32,
                proposals: tally.proposals.iter().map(|id| id.to_string()).collect(),
            };
            (key, section)
        })
        .collect();
    sections.sort_by(|(a_key, a), (b_key, b)| {
        (a_key.is_empty(), Reverse(a.feature_count), a_key, &a.name).cmp(&(
            b_key.is_empty(),
            Reverse(b.feature_count),
            b_key,
            &b.name,
        ))
    });
    let near_duplicates = groups(&sections);
    Sections {
        sections: sections.into_iter().map(|(_, section)| section).collect(),
        near_duplicates,
    }
}

/// The groups of two or more sections that share a near-duplicate key, in the order of their first section. The
/// empty section is in none.
fn groups(sections: &[(String, SectionUse)]) -> Vec<NearDuplicateGroup> {
    let mut by_key: Vec<(String, Vec<String>)> = Vec::new();
    for (_, section) in sections {
        let Some(key) = near_duplicate_key(&section.name) else {
            continue;
        };
        match by_key.iter_mut().find(|(known, _)| *known == key) {
            Some((_, names)) => names.push(section.name.clone()),
            None => by_key.push((key, vec![section.name.clone()])),
        }
    }
    by_key
        .into_iter()
        .filter(|(_, names)| names.len() > 1)
        .map(|(_, names)| NearDuplicateGroup { names })
        .collect()
}
