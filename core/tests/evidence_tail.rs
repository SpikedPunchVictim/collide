//! The evidence a failed phase keeps (ts-0091-pz; 0.3, The stream protocol): the end of the agent's stderr, bounded.

use collide_core::adapter::{EVIDENCE_BYTES, evidence_tail};

#[test]
fn text_shorter_than_the_bound_is_kept_whole() {
    assert_eq!(
        evidence_tail("model not found"),
        Some("model not found".to_string())
    );
    let exact = "x".repeat(EVIDENCE_BYTES);
    assert_eq!(evidence_tail(&exact), Some(exact));
}

#[test]
fn surrounding_white_space_is_dropped_before_the_bound_is_applied() {
    assert_eq!(evidence_tail("  boom\n\n"), Some("boom".to_string()));
    let text = format!("{}\n", "y".repeat(EVIDENCE_BYTES));
    assert_eq!(
        evidence_tail(&text).map(|kept| kept.len()),
        Some(EVIDENCE_BYTES)
    );
}

#[test]
fn nothing_but_white_space_is_no_evidence() {
    assert_eq!(evidence_tail(""), None);
    assert_eq!(evidence_tail(" \n\t\n"), None);
}

#[test]
fn text_longer_than_the_bound_keeps_its_end() {
    let text = format!("{}tail", "a".repeat(EVIDENCE_BYTES + 10));
    let kept = evidence_tail(&text).expect("evidence");
    assert_eq!(kept.len(), EVIDENCE_BYTES);
    assert!(kept.ends_with("aatail"));
}

#[test]
fn a_cut_inside_a_multi_byte_character_moves_to_the_next_boundary() {
    // Three-byte characters never fit 2048 evenly: the cut falls inside one and moves on.
    let text = "\u{20ac}".repeat(EVIDENCE_BYTES);
    let kept = evidence_tail(&text).expect("evidence");
    assert!(kept.len() <= EVIDENCE_BYTES);
    assert_eq!(kept.len() % 3, 0);
    assert!(kept.chars().all(|c| c == '\u{20ac}'));
    assert!(text.ends_with(&kept));
    // A four-byte character at the front of the window.
    let text = format!("{}{}", "\u{1f600}", "b".repeat(EVIDENCE_BYTES - 1));
    let kept = evidence_tail(&text).expect("evidence");
    assert_eq!(kept, "b".repeat(EVIDENCE_BYTES - 1));
}
