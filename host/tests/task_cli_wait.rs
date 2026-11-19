//! The wait options of a write (ts-0114-hr, ts-0136-rr; 0.4, Root busy): `--wait <seconds>` and `--no-wait` as the
//! command line reads them. A write to a busy checkout is queued by the daemon and the command waits for the Queue
//! item; that wait is tested over a daemon in `task_daemon_cli.rs`.

use collide_host::task_cli::{Command, TaskOptions, Wait, WriteOptions, parse_task};
use std::time::Duration;

// ---- the options --------------------------------------------------------------------------------------------------

fn task_options(args: &[&str]) -> Result<TaskOptions, String> {
    let args: Vec<String> = args.iter().map(|word| word.to_string()).collect();
    match parse_task(&args) {
        Ok(Command::Task(options)) => Ok(options),
        Ok(other) => panic!("expected a task: {other:?}"),
        Err(error) => Err(error.message),
    }
}

#[test]
fn a_write_waits_by_default_for_ten_minutes() {
    let options = task_options(&["3", "setStatus", "active"]).unwrap();
    assert_eq!(options.write.wait, Wait::Default);
    assert_eq!(
        collide_host::task_cli::DEFAULT_WAIT,
        Duration::from_secs(600)
    );
}

#[test]
fn wait_takes_a_number_of_seconds_in_either_form_and_no_wait_takes_none() {
    for args in [
        ["3", "setStatus", "active", "--wait", "5"],
        ["3", "setStatus", "active", "--wait=5", ""],
    ] {
        let args: Vec<&str> = args.into_iter().filter(|word| !word.is_empty()).collect();
        assert_eq!(
            task_options(&args).unwrap().write.wait,
            Wait::Seconds(5),
            "{args:?}"
        );
    }
    let options = task_options(&["3", "setStatus", "active", "--no-wait"]).unwrap();
    assert_eq!(options.write.wait, Wait::Never);
}

#[test]
fn wait_and_no_wait_together_or_twice_or_with_no_number_are_usage_errors() {
    for args in [
        &["3", "setStatus", "active", "--wait", "5", "--no-wait"][..],
        &["3", "setStatus", "active", "--no-wait", "--wait", "5"],
        &["3", "setStatus", "active", "--wait", "5", "--wait", "6"],
        &["3", "setStatus", "active", "--no-wait", "--no-wait"],
        &["3", "setStatus", "active", "--wait"],
        &["3", "setStatus", "active", "--wait", "soon"],
        &["3", "setStatus", "active", "--wait", "-1"],
        &["3", "setStatus", "active", "--wait="],
    ] {
        assert!(task_options(args).is_err(), "{args:?}");
    }
}

#[test]
fn a_wait_option_without_an_op_is_a_usage_error_like_checkout_and_commit() {
    let message = task_options(&["3", "--no-wait"]).unwrap_err();
    assert!(message.contains("--no-wait"), "{message}");
    assert!(message.contains("are for a write"), "{message}");
    assert_eq!(WriteOptions::default().wait, Wait::Default);
}

#[test]
fn help_and_usage_name_the_wait_options() {
    assert!(collide_host::task_cli::USAGE.contains("[--wait <seconds> | --no-wait]"));
    assert!(collide_host::task_cli::HELP.contains("--no-wait"));
    assert!(collide_host::task_cli::HELP.contains("10 minutes"));
}
