//! The `git:` locator, `<url>//<path>@<ref>` (ts-0100-pu; 0.7, Pack sources). The grammar is a table: each form of the
//! item's success criteria, the forms that are hard to split, and each refusal. The pure part only: no git runs here.

use collide_core::lock::{
    GitLocator, LocatorError, PackLock, PackSource, SourceError, UrlTarget, check_lock,
};

fn parts(text: &str) -> (String, String, Option<String>) {
    let locator = GitLocator::parse(text).unwrap_or_else(|error| panic!("{text}: {error}"));
    (locator.url, locator.path, locator.reference)
}

fn parsed(url: &str, path: &str, reference: Option<&str>) -> (String, String, Option<String>) {
    (url.into(), path.into(), reference.map(str::to_string))
}

#[test]
fn each_form_of_the_plan_splits_into_url_path_and_ref() {
    for (text, want) in [
        (
            "https://h/o/r//pack@v1.2",
            parsed("https://h/o/r", "pack", Some("v1.2")),
        ),
        (
            "file:///tmp/r.git//pack@main",
            parsed("file:///tmp/r.git", "pack", Some("main")),
        ),
        ("/tmp/r.git//pack", parsed("/tmp/r.git", "pack", None)),
        (
            "git@h:o/r//pack@abc123",
            parsed("git@h:o/r", "pack", Some("abc123")),
        ),
        ("https://h/o/r", parsed("https://h/o/r", "", None)),
    ] {
        assert_eq!(parts(text), want, "{text}");
    }
}

#[test]
fn the_double_slash_after_a_scheme_is_part_of_the_url() {
    for (text, want) in [
        ("https://h/r//p", parsed("https://h/r", "p", None)),
        (
            "ssh://git@h/o/r//a/b@v1",
            parsed("ssh://git@h/o/r", "a/b", Some("v1")),
        ),
        ("file:///srv/r.git", parsed("file:///srv/r.git", "", None)),
        ("file:///srv/r.git//", parsed("file:///srv/r.git", "", None)),
    ] {
        assert_eq!(parts(text), want, "{text}");
    }
}

#[test]
fn an_at_sign_in_the_urls_user_part_is_not_a_ref() {
    for (text, want) in [
        ("git@h:o/r", parsed("git@h:o/r", "", None)),
        ("https://user@h/o/r", parsed("https://user@h/o/r", "", None)),
        ("https://user@h", parsed("https://user@h", "", None)),
        (
            "ssh://git@h/o/r//pack",
            parsed("ssh://git@h/o/r", "pack", None),
        ),
    ] {
        assert_eq!(parts(text), want, "{text}");
    }
}

#[test]
fn a_ref_without_a_path_is_the_text_after_the_last_at_that_follows_the_url() {
    for (text, want) in [
        ("https://h/o/r@v2", parsed("https://h/o/r", "", Some("v2"))),
        (
            "https://user@h/o/r@v2",
            parsed("https://user@h/o/r", "", Some("v2")),
        ),
        ("git@h:o/r@v2", parsed("git@h:o/r", "", Some("v2"))),
        ("/srv/repo.git@v1", parsed("/srv/repo.git", "", Some("v1"))),
        // An `@` in a folder's name, with a `/` after it, is part of the path.
        ("/srv/a@b/repo.git", parsed("/srv/a@b/repo.git", "", None)),
    ] {
        assert_eq!(parts(text), want, "{text}");
    }
}

#[test]
fn a_ref_with_a_slash_is_written_after_an_empty_path() {
    assert_eq!(
        parts("https://h/o/r//@feature/x"),
        parsed("https://h/o/r", "", Some("feature/x"))
    );
    assert_eq!(
        parts("https://h/o/r//pack@feature/x"),
        parsed("https://h/o/r", "pack", Some("feature/x"))
    );
}

#[test]
fn a_repository_on_disk_is_a_url_with_a_tilde_or_an_absolute_path() {
    assert_eq!(
        parts("~/dev/ferry-pack//pack@main"),
        parsed("~/dev/ferry-pack", "pack", Some("main"))
    );
    assert_eq!(
        parts("~/dev/ferry-pack"),
        parsed("~/dev/ferry-pack", "", None)
    );
}

#[test]
fn a_nested_path_is_kept_whole() {
    assert_eq!(
        parts("https://h/o/r//packs/ferry/v2@v1"),
        parsed("https://h/o/r", "packs/ferry/v2", Some("v1"))
    );
}

#[test]
fn without_a_ref_the_ref_is_head() {
    assert_eq!(
        GitLocator::parse("https://h/o/r//p").unwrap().reference(),
        "HEAD"
    );
    assert_eq!(
        GitLocator::parse("https://h/o/r//p@v1")
            .unwrap()
            .reference(),
        "v1"
    );
}

fn refusal(text: &str) -> LocatorError {
    match GitLocator::parse(text) {
        Ok(found) => panic!("{text:?} should be refused, and gave {found:?}"),
        Err(error) => error,
    }
}

#[test]
fn a_url_that_starts_with_a_dash_is_refused() {
    for text in ["-x", "--upload-pack=evil", "-oProxyCommand=x//p@v1"] {
        assert!(
            matches!(refusal(text), LocatorError::FlagLike { .. }),
            "{text}"
        );
    }
}

#[test]
fn a_nul_or_a_newline_anywhere_is_refused() {
    for text in [
        "https://h/o/r\0//p",
        "https://h/o/r//p\0",
        "https://h/o/r//p@v\0",
        "https://h/o/r\n//p",
        "https://h/o/r//p\n@v1",
        "https://h/o/r//p@v1\n",
    ] {
        assert!(
            matches!(refusal(text), LocatorError::Control { .. }),
            "{text:?}"
        );
    }
}

#[test]
fn an_empty_url_is_refused() {
    for text in ["", "//pack", "//pack@v1", "@v1"] {
        assert!(
            matches!(refusal(text), LocatorError::EmptyUrl { .. }),
            "{text:?}"
        );
    }
}

#[test]
fn a_path_with_a_dot_dot_or_dot_part_or_an_empty_part_is_refused() {
    for text in [
        "https://h/o/r//../x",
        "https://h/o/r//a/../b",
        "https://h/o/r//a/..",
        "https://h/o/r//a/./b",
        "https://h/o/r//a//b",
        "https://h/o/r//a/",
        "https://h/o/r///a",
    ] {
        assert!(
            matches!(refusal(text), LocatorError::BadPath { .. }),
            "{text}"
        );
    }
}

#[test]
fn an_empty_ref_or_a_ref_that_starts_with_a_dash_is_refused() {
    assert!(matches!(
        refusal("https://h/o/r//pack@"),
        LocatorError::EmptyRef { .. }
    ));
    assert!(matches!(
        refusal("https://h/o/r//pack@-x"),
        LocatorError::BadRef { .. }
    ));
    assert!(matches!(
        refusal("https://h/o/r@--force"),
        LocatorError::BadRef { .. }
    ));
}

#[test]
fn each_refusal_names_the_form_that_was_wrong() {
    for (text, wanted) in [
        ("-x", "starts with `-`"),
        ("https://h/o/r//../x", "`..`"),
        ("https://h/o/r//a//b", "empty"),
        ("https://h/o/r//p@", "ref"),
        ("", "empty"),
        ("https://h/o/r\0", "NUL"),
    ] {
        let message = refusal(text).to_string();
        assert!(message.contains(wanted), "{text:?}: {message}");
    }
}

#[test]
fn the_source_parser_refuses_a_bad_locator_with_the_locators_reason() {
    let error = PackSource::parse("git:https://h/o/r//../x").unwrap_err();
    assert!(matches!(error, SourceError::BadLocator { .. }), "{error:?}");
    assert!(error.to_string().contains("`..`"), "{error}");
    // The refusal the parser always had keeps its own message.
    assert!(matches!(
        PackSource::parse("git:--upload-pack=evil"),
        Err(SourceError::FlagLikeLocator { .. })
    ));
}

#[test]
fn a_lock_entry_with_a_bad_locator_is_refused_by_the_lock_check() {
    let lock = PackLock {
        packs: vec![collide_core::lock::LockEntry {
            source: "git:https://h/o/r//../x".into(),
            commit: "a".repeat(40),
            tree: "b".repeat(40),
        }],
    };
    let errors = check_lock(&lock).unwrap_err();
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert!(errors[0].to_string().contains("`..`"), "{}", errors[0]);
}

fn target(url: &str) -> UrlTarget {
    GitLocator::parse(url).unwrap().target()
}

#[test]
fn a_url_names_the_host_a_fetch_would_contact() {
    for (url, host) in [
        ("https://github.com/o/r", "github.com"),
        ("https://User@GitHub.com:8443/o/r", "github.com"),
        ("ssh://git@h.example/o/r", "h.example"),
        ("git@github.com:o/r", "github.com"),
        ("github.com:o/r", "github.com"),
        ("https://[::1]:8080/o/r", "::1"),
    ] {
        assert_eq!(target(url), UrlTarget::Host(host.into()), "{url}");
    }
}

#[test]
fn a_repository_on_disk_has_no_host() {
    assert_eq!(
        target("file:///tmp/r.git"),
        UrlTarget::Path("/tmp/r.git".into())
    );
    assert_eq!(target("/tmp/r.git"), UrlTarget::Path("/tmp/r.git".into()));
    assert_eq!(target("~/dev/x"), UrlTarget::Path("~/dev/x".into()));
}

#[test]
fn a_remote_helper_url_has_no_host_and_is_left_to_gits_allowlist() {
    assert_eq!(target("ext::sh -c touch%20marker"), UrlTarget::Helper);
    assert_eq!(target("fd::17"), UrlTarget::Helper);
}

#[test]
fn text_that_is_no_url_git_would_take_is_unknown() {
    for url in [
        "github.com/org/p",
        "../x",
        "repo.git",
        "https:///x",
        "ssh://",
    ] {
        assert_eq!(target(url), UrlTarget::Unknown, "{url}");
    }
}

#[test]
fn a_url_with_a_password_is_refused_and_the_refusal_masks_it() {
    // A fetch uses the person's git credentials (0.7, Fetch rules). A password in the URL would be committed in the
    // lock and printed in every message that names the source.
    for text in [
        "https://me:tok-5e1f@h/o/r//pack@v1",
        "https://me:tok-5e1f@h/o/r",
        "ssh://git:tok-5e1f@h/o/r.git//pack",
    ] {
        let error = refusal(text);
        assert!(
            matches!(error, LocatorError::Password { .. }),
            "{text}: {error:?}"
        );
        let message = error.to_string();
        assert!(!message.contains("tok-5e1f"), "{text}: {message}");
        assert!(
            message.contains("me:***@h") || message.contains("git:***@h"),
            "{text}: {message}"
        );
        assert!(message.contains("credential"), "{text}: {message}");
    }
    let error = PackSource::parse("git:https://me:tok-5e1f@h/o/r//pack").unwrap_err();
    assert!(!error.to_string().contains("tok-5e1f"), "{error}");
    assert!(!format!("{error:?}").contains("tok-5e1f"), "{error:?}");
}

#[test]
fn a_user_name_without_a_password_is_kept() {
    assert_eq!(
        parts("https://me@h/o/r//pack@v1"),
        parsed("https://me@h/o/r", "pack", Some("v1"))
    );
    assert_eq!(
        parts("ssh://git@h/o/r.git"),
        parsed("ssh://git@h/o/r.git", "", None)
    );
    assert_eq!(parts("git@h:o/r//pack"), parsed("git@h:o/r", "pack", None));
}
