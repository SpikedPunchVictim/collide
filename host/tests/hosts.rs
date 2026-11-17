//! The hosts a fetch may contact (ts-0100-pu; 0.7, Fetch rules): `~/.config/collide/hosts.yaml`. The folder of the
//! user's files is a parameter, so these tests use a temp root and never the real home.

mod common;

use collide_host::hosts::KnownHosts;
use common::TempRoot;
use std::fs;
use std::os::unix::fs::PermissionsExt;

#[test]
fn a_home_with_no_file_knows_no_host() {
    let root = TempRoot::new();
    let hosts = KnownHosts::for_home(root.path());
    assert!(!hosts.is_known("github.com").unwrap());
    assert!(!root.join(".config").exists(), "a read makes nothing");
}

#[test]
fn the_file_is_hosts_yaml_in_the_users_collide_folder() {
    let root = TempRoot::new();
    let hosts = KnownHosts::for_home(root.path());
    assert_eq!(hosts.file(), root.join(".config/collide/hosts.yaml"));
}

#[test]
fn a_recorded_host_is_known_and_another_is_not() {
    let root = TempRoot::new();
    let hosts = KnownHosts::for_home(root.path());
    hosts.record("github.com").unwrap();
    assert!(hosts.is_known("github.com").unwrap());
    assert!(!hosts.is_known("gitlab.com").unwrap());
}

#[test]
fn a_host_is_matched_in_lower_case() {
    let root = TempRoot::new();
    let hosts = KnownHosts::for_home(root.path());
    hosts.record("GitHub.com").unwrap();
    assert!(hosts.is_known("github.com").unwrap());
    assert!(hosts.is_known("GITHUB.COM").unwrap());
}

#[test]
fn recording_a_host_twice_lists_it_once() {
    let root = TempRoot::new();
    let hosts = KnownHosts::for_home(root.path());
    hosts.record("github.com").unwrap();
    hosts.record("github.com").unwrap();
    let text = fs::read_to_string(hosts.file()).unwrap();
    assert_eq!(text.matches("github.com").count(), 1, "{text}");
}

#[test]
fn hosts_are_written_in_the_order_they_were_recorded() {
    let root = TempRoot::new();
    let hosts = KnownHosts::for_home(root.path());
    hosts.record("b.example").unwrap();
    hosts.record("a.example").unwrap();
    let text = fs::read_to_string(hosts.file()).unwrap();
    assert!(
        text.find("b.example").unwrap() < text.find("a.example").unwrap(),
        "{text}"
    );
}

#[test]
fn the_file_is_private_and_no_temp_file_is_left() {
    let root = TempRoot::new();
    let hosts = KnownHosts::for_home(root.path());
    hosts.record("github.com").unwrap();
    let mode = fs::metadata(hosts.file()).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
    let names: Vec<_> = fs::read_dir(hosts.file().parent().unwrap())
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert_eq!(names, ["hosts.yaml"]);
}

#[test]
fn a_file_that_does_not_read_is_an_error_naming_the_file_and_is_left_as_it_is() {
    let root = TempRoot::new();
    let hosts = KnownHosts::for_home(root.path());
    fs::create_dir_all(hosts.file().parent().unwrap()).unwrap();
    fs::write(hosts.file(), "hosts: [unclosed\n").unwrap();
    let error = hosts.is_known("github.com").unwrap_err().to_string();
    assert!(error.contains("hosts.yaml"), "{error}");
    let error = hosts.record("github.com").unwrap_err().to_string();
    assert!(error.contains("hosts.yaml"), "{error}");
    assert_eq!(
        fs::read_to_string(hosts.file()).unwrap(),
        "hosts: [unclosed\n"
    );
}

#[test]
fn an_unknown_key_in_the_file_is_an_error() {
    let root = TempRoot::new();
    let hosts = KnownHosts::for_home(root.path());
    fs::create_dir_all(hosts.file().parent().unwrap()).unwrap();
    fs::write(hosts.file(), "hosts: []\ntrusted: []\n").unwrap();
    assert!(
        hosts
            .is_known("a")
            .unwrap_err()
            .to_string()
            .contains("trusted")
    );
}
