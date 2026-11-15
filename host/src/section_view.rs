//! What `collide sections` prints (ts-0049). The text form is one line per section, then the near-duplicate groups; the
//! JSON form is `collide_core::section::Sections` as it is.

use collide_core::section::Sections;

/// What the print calls the section of the features that are unsorted.
const UNSORTED: &str = "Unsorted";

fn count(number: u32, word: &str) -> String {
    if number == 1 {
        format!("{number} {word}")
    } else {
        format!("{number} {word}s")
    }
}

/// Each section with its feature count and proposal count in columns, the empty one last as Unsorted, then the
/// near-duplicate groups under a line that says they are suggestions.
pub fn sections_text(sections: &Sections) -> String {
    if sections.sections.is_empty() {
        return "no sections: no proposal that is not dropped has a feature\n".to_string();
    }
    let rows: Vec<[String; 3]> = sections
        .sections
        .iter()
        .map(|section| {
            let name = if section.name.is_empty() {
                UNSORTED
            } else {
                section.name.as_str()
            };
            [
                name.to_string(),
                count(section.feature_count, "feature"),
                count(section.proposal_count, "proposal"),
            ]
        })
        .collect();
    let width = |column: usize| {
        rows.iter()
            .map(|row| row[column].chars().count())
            .max()
            .unwrap_or(0)
    };
    let (w0, w1) = (width(0), width(1));
    let mut text = String::new();
    for row in &rows {
        text.push_str(&format!("{:w0$}  {:w1$}  {}\n", row[0], row[1], row[2]));
    }
    if !sections.near_duplicates.is_empty() {
        text.push_str("\nnear-duplicates (a suggestion; nothing is merged)\n");
        for group in &sections.near_duplicates {
            text.push_str(&format!("  {}\n", group.names.join(", ")));
        }
    }
    text
}
