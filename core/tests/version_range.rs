//! Tests for the version range of `requires` (ts-0096-ph, 0.7): the five operators, the forms that are refused by
//! name, numeric comparison, and the first version in a tool's output.

use collide_core::version_range::{RangeError, Version, VersionRange, version_in_range};

fn v(text: &str) -> Version {
    Version::parse(text).unwrap_or_else(|| panic!("{text} is a version"))
}

fn unknown(form: &str) -> RangeError {
    RangeError::UnknownForm { form: form.into() }
}

#[test]
fn a_range_is_met_or_not_by_each_of_the_five_operators() {
    // (range, version, met)
    let table = [
        (">=1.2", "1.2.0", true),
        (">=1.2", "1.1.9", false),
        (">1.2", "1.2.0", false),
        (">1.2", "1.2.1", true),
        ("<=1.2", "1.2.0", true),
        ("<=1.2", "1.2.1", false),
        ("<1.2", "1.1.9", true),
        ("<1.2", "1.2.0", false),
        ("=1.2", "1.2.0", true),
        ("=1.2.3", "1.2.3", true),
        ("=1.2.3", "1.2.4", false),
    ];
    for (range, version, met) in table {
        assert_eq!(
            version_in_range(range, &v(version)),
            Ok(met),
            "{range} against {version}"
        );
    }
}

#[test]
fn comparators_joined_by_spaces_must_all_hold() {
    assert_eq!(version_in_range(">=0.1 <0.2", &v("0.1.0")), Ok(true));
    assert_eq!(version_in_range(">=0.1 <0.2", &v("0.2.0")), Ok(false));
    assert_eq!(version_in_range(">=0.1 <0.2", &v("0.0.9")), Ok(false));
    assert_eq!(version_in_range("  >=0.1   <0.2 ", &v("0.1.5")), Ok(true));
}

#[test]
fn versions_compare_as_numbers_and_not_as_text() {
    assert_eq!(version_in_range(">=1.10", &v("1.9.0")), Ok(false));
    assert_eq!(version_in_range(">=1.10", &v("1.10.0")), Ok(true));
    assert_eq!(version_in_range("<2.0", &v("10.0.0")), Ok(false));
    assert_eq!(version_in_range(">=22.18", &v("22.2.0")), Ok(false));
}

#[test]
fn a_missing_patch_is_zero_in_a_range_and_in_a_version() {
    assert_eq!(v("22.18"), v("22.18.0"));
    assert_eq!(version_in_range(">=22.18", &v("22.18")), Ok(true));
}

#[test]
fn a_form_that_is_not_known_is_refused_naming_it() {
    let table = [
        "^1",
        "~1",
        "1.x",
        "*",
        "1.2",
        ">=1",
        ">= 1.2",
        ">=1.2,<2.0",
        ">=1.2.3.4",
        ">=v1.2",
        ">=1.2-beta",
        "latest",
        ">>1.2",
        "!=1.2",
    ];
    for form in table {
        let error = VersionRange::parse(form).unwrap_err();
        let RangeError::UnknownForm { form: named } = &error else {
            panic!("{form}: {error:?}");
        };
        assert!(
            form.split_whitespace().any(|word| word == named),
            "{form} named {named}"
        );
    }
}

#[test]
fn a_form_error_names_the_word_that_is_wrong_and_says_what_is_allowed() {
    let error = VersionRange::parse(">=1.0 ^2").unwrap_err();
    assert_eq!(error, unknown("^2"));
    let text = error.to_string();
    assert!(text.contains("`^2`"), "{text}");
    assert!(text.contains(">=0.1 <0.2"), "{text}");
}

#[test]
fn an_empty_range_is_refused() {
    assert_eq!(VersionRange::parse(""), Err(RangeError::Empty));
    assert_eq!(VersionRange::parse("   "), Err(RangeError::Empty));
}

#[test]
fn a_version_is_major_minor_and_an_optional_patch_of_digits() {
    for bad in [
        "",
        "1",
        "1.",
        ".1",
        "1.2.3.4",
        "1.x",
        "v1.2",
        "1.2-beta",
        "1..2",
        "+1.2",
        "-1.2",
        " 1.2",
        "99999999999999999999.0",
    ] {
        assert_eq!(Version::parse(bad), None, "{bad:?}");
    }
    assert_eq!(v("01.02.03"), v("1.2.3"));
}

#[test]
fn the_first_version_in_a_tools_output_is_found() {
    let table = [
        ("v22.18.0", Some("22.18.0")),
        ("git version 2.43.0\n", Some("2.43.0")),
        ("pnpm 10.4\n", Some("10.4.0")),
        ("tool 1.2.3-beta.4 (build 9.9.9)", Some("1.2.3")),
        ("Tool 3 of 4, version 5.6.7", Some("5.6.7")),
        ("10.5.3.1", Some("10.5.3")),
        ("no digits here", None),
        ("just 7 and 8", None),
        ("", None),
    ];
    for (output, found) in table {
        assert_eq!(Version::find_in(output), found.map(v), "output {output:?}");
    }
}

#[test]
fn a_version_is_shown_with_all_three_parts() {
    assert_eq!(v("22.18").to_string(), "22.18.0");
}
