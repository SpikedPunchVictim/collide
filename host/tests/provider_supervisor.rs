//! Tests for the provider supervisor and the client under it (ts-0108-dq). Each test starts the fake provider
//! (`tests/fixtures/fake_provider.py`) as a real child with a cleared environment and a temp root for `HOME` and
//! `COLLIDE_DAEMON_ROOT`. No test sleeps: the supervisor's waits go to a recording `Pause`, a test waits on a
//! condition with a deadline, and the timeouts are a few hundred milliseconds.

use collide_core::item::Category;
use collide_core::ports::{ProviderError, TaskProvider};
use collide_core::provider::{
    ApplyParams, ApplyResult, GetParams, InitializeParams, ListParams, PreviewResult,
    ProviderEvent, WatchParams,
};
use collide_host::provider_client::CallTimeouts;
use collide_host::supervisor::{Pause, SupervisedProvider, SupervisorPolicy};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

mod common;
use common::fake_provider::{base_env, fake_launch};
use common::{TempRoot, pid_alive, wait_until};

const WITHIN: Duration = Duration::from_secs(10);
const NAME: &str = "demo/plan:plan";

/// Records each wait the supervisor asks for and runs `on_pause` with the number of waits so far (0 for the first).
struct Pauses {
    asked: Mutex<Vec<Duration>>,
    on_pause: Box<dyn Fn(usize) + Send + Sync>,
}

struct SharedPauses(Arc<Pauses>);

impl Pause for SharedPauses {
    fn pause(&self, how_long: Duration) {
        let index = {
            let mut asked = self.0.asked.lock().unwrap();
            asked.push(how_long);
            asked.len() - 1
        };
        (self.0.on_pause)(index);
    }
}

fn seconds(list: &[u64]) -> Vec<Duration> {
    list.iter().map(|s| Duration::from_secs(*s)).collect()
}

/// A supervisor of the fake provider with `modes`, and what it asked to wait. Not started: `init` does that.
struct Rig {
    // The supervisor goes first, so its child is gone before the root is.
    provider: SupervisedProvider,
    pauses: Arc<Pauses>,
    root: TempRoot,
}

fn policy() -> SupervisorPolicy {
    SupervisorPolicy {
        timeouts: CallTimeouts {
            initialize: WITHIN,
            list: WITHIN,
            get: WITHIN,
            apply: WITHIN,
            preview: WITHIN,
            watch: WITHIN,
        },
        stop_grace: Duration::from_millis(200),
        ..SupervisorPolicy::default()
    }
}

fn params() -> InitializeParams {
    InitializeParams {
        config: BTreeMap::from([
            ("mode".to_string(), json!("x")),
            ("count".to_string(), json!(3)),
        ]),
        root: "/work/main".to_string(),
    }
}

impl Rig {
    fn with(
        root: TempRoot,
        modes: &[&str],
        policy: SupervisorPolicy,
        on_pause: impl Fn(usize) + Send + Sync + 'static,
    ) -> Rig {
        let pauses = Arc::new(Pauses {
            asked: Mutex::new(Vec::new()),
            on_pause: Box::new(on_pause),
        });
        let launch = fake_launch(&root, modes);
        let provider = SupervisedProvider::new(NAME, launch, policy)
            .with_pause(Box::new(SharedPauses(pauses.clone())));
        Rig {
            provider,
            pauses,
            root,
        }
    }

    /// Started and initialized.
    fn up(modes: &[&str]) -> Rig {
        let rig = Rig::with(TempRoot::new(), modes, policy(), |_| {});
        rig.provider
            .initialize(&params())
            .expect("the fake provider starts");
        rig
    }

    fn asked(&self) -> Vec<Duration> {
        self.pauses.asked.lock().unwrap().clone()
    }

    /// The fields of `get echo`: what the child saw, and which child it is.
    fn echo(&self) -> Value {
        let got = self.provider.get(&get("echo")).expect("get echo");
        serde_json::to_value(got.task.expect("a task").fields).unwrap()
    }

    fn pid(&self) -> u64 {
        self.echo()["pid"].as_u64().unwrap()
    }

    fn stopped_running(&self) {
        wait_until("the supervisor to see the child gone", WITHIN, || {
            !self.provider.is_running()
        });
    }
}

fn get(id: &str) -> GetParams {
    GetParams {
        id: id.to_string(),
        checkout: None,
    }
}

fn op(name: &str) -> ApplyParams {
    ApplyParams {
        actor: None,
        op: name.to_string(),
        id: Some("fake-1".to_string()),
        args: BTreeMap::new(),
        version: Some("v1".to_string()),
        checkout: None,
    }
}

fn failed_message(error: ProviderError) -> String {
    match error {
        ProviderError::Failed { message } => message,
        other => panic!("wanted Failed, got {other:?}"),
    }
}

fn next_event(events: &Receiver<ProviderEvent>) -> ProviderEvent {
    events
        .recv_timeout(WITHIN)
        .expect("an event within the deadline")
}

fn kinds(events: &[ProviderEvent]) -> Vec<&'static str> {
    events
        .iter()
        .map(|event| match event {
            ProviderEvent::Changed(_) => "changed",
            ProviderEvent::Renamed(_) => "renamed",
            ProviderEvent::Removed(_) => "removed",
            ProviderEvent::Invalid(_) => "invalid",
            ProviderEvent::Collision(_) => "collision",
            ProviderEvent::Stale(_) => "stale",
            ProviderEvent::Unreadable { .. } => "unreadable",
        })
        .collect()
}

const SIX: [&str; 6] = [
    "changed",
    "renamed",
    "removed",
    "invalid",
    "collision",
    "stale",
];

// ---- the protocol: initialize ---------------------------------------------------------------------------------------

#[test]
fn initialize_sends_the_packs_settings_and_the_root_and_returns_what_the_provider_declares() {
    let rig = Rig::with(TempRoot::new(), &[], policy(), |_| {});
    let result = rig.provider.initialize(&params()).unwrap();
    assert_eq!(result.statuses[0].name, "todo");
    assert_eq!(result.statuses[0].category, Category::Ready);
    assert!(result.capabilities.watch && result.capabilities.preview);
    let seen = rig.echo();
    assert_eq!(
        seen["init"],
        json!({"config": {"count": 3, "mode": "x"}, "root": "/work/main"})
    );
}

#[test]
fn a_result_that_declares_move_is_accepted_since_keep_defined_it() {
    let rig = Rig::with(TempRoot::new(), &["declares-move"], policy(), |_| {});
    let result = rig.provider.initialize(&params()).unwrap();
    assert_eq!(result.ops[0].name, "move");
    assert!(rig.provider.is_running());
}

#[test]
fn a_call_before_initialize_fails_and_starts_no_child() {
    let rig = Rig::with(TempRoot::new(), &[], policy(), |_| {});
    let message = failed_message(
        rig.provider
            .list(&ListParams {
                checkout: None,
                cursor: None,
            })
            .unwrap_err(),
    );
    assert!(
        message.contains(NAME) && message.contains("initialize"),
        "{message}"
    );
    assert!(!rig.provider.is_running());
}

// ---- the protocol: calls --------------------------------------------------------------------------------------------

#[test]
fn list_get_apply_and_preview_reach_the_one_running_child() {
    let rig = Rig::up(&[]);
    let list = rig
        .provider
        .list(&ListParams {
            checkout: None,
            cursor: None,
        })
        .unwrap();
    assert_eq!(list.tasks[0].id, "fake-1");
    assert_eq!(
        rig.provider.get(&get("fake-7")).unwrap().task.unwrap().id,
        "fake-7"
    );
    assert!(matches!(
        rig.provider.apply(&op("ok")).unwrap(),
        ApplyResult::Ok { .. }
    ));
    assert!(matches!(
        rig.provider.preview(&op("ok")).unwrap(),
        PreviewResult::Ok { .. }
    ));
    let first = rig.pid();
    assert_eq!(rig.pid(), first, "every call went to the same child");
    assert!(rig.asked().is_empty());
}

#[test]
fn apply_answers_ok_conflict_and_refused_as_the_provider_gave_them() {
    let rig = Rig::up(&[]);
    match rig.provider.apply(&op("ok")).unwrap() {
        ApplyResult::Ok {
            version,
            files,
            items,
            ..
        } => {
            assert_eq!(version, "v2");
            assert_eq!(files, ["a.md"]);
            assert_eq!(items[0].key, "fake:1");
        }
        other => panic!("{other:?}"),
    }
    assert!(matches!(
        rig.provider.apply(&op("conflict")).unwrap(),
        ApplyResult::Conflict { .. }
    ));
    match rig.provider.apply(&op("refused")).unwrap() {
        ApplyResult::Refused { message } => assert_eq!(message, "illegal transition"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn preview_answers_ok_conflict_and_refused_as_the_provider_gave_them() {
    let rig = Rig::up(&[]);
    assert!(matches!(
        rig.provider.preview(&op("ok")).unwrap(),
        PreviewResult::Ok { .. }
    ));
    assert!(matches!(
        rig.provider.preview(&op("conflict")).unwrap(),
        PreviewResult::Conflict { .. }
    ));
    match rig.provider.preview(&op("refused")).unwrap() {
        PreviewResult::Refused { message } => assert_eq!(message, "illegal transition"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_missing_id_is_an_answer_and_not_an_error() {
    let rig = Rig::up(&[]);
    assert_eq!(rig.provider.get(&get("missing")).unwrap().task, None);
}

#[test]
fn a_json_rpc_error_is_failed_with_the_providers_message_and_the_child_stays() {
    let rig = Rig::up(&[]);
    let first = rig.pid();
    let message = failed_message(rig.provider.get(&get("boom")).unwrap_err());
    assert!(message.contains("disk on fire"), "{message}");
    assert!(
        message.contains(NAME) && message.contains("`get`"),
        "{message}"
    );
    assert_eq!(rig.pid(), first);
}

#[test]
fn a_provider_that_does_not_declare_preview_answers_unsupported_and_is_not_asked() {
    // It would crash if it were asked.
    let rig = Rig::up(&["no-preview", "crash-on=preview"]);
    assert_eq!(
        rig.provider.preview(&op("ok")),
        Err(ProviderError::Unsupported {
            method: "preview".to_string()
        })
    );
    assert!(rig.provider.is_running());
}

#[test]
fn a_method_the_provider_answers_with_method_not_found_is_unsupported() {
    let rig = Rig::up(&["unknown=preview"]);
    assert_eq!(
        rig.provider.preview(&op("ok")),
        Err(ProviderError::Unsupported {
            method: "preview".to_string()
        })
    );
    assert!(
        rig.provider.is_running(),
        "an unsupported method is not a crash"
    );
}

#[test]
fn a_reply_that_is_not_json_fails_that_call_and_the_next_call_works() {
    let rig = Rig::up(&["garbage-on=list"]);
    let message = failed_message(
        rig.provider
            .list(&ListParams {
                checkout: None,
                cursor: None,
            })
            .unwrap_err(),
    );
    assert!(
        message.contains("not JSON") && message.contains("`list`"),
        "{message}"
    );
    assert!(rig.provider.get(&get("fake-2")).unwrap().task.is_some());
}

#[test]
fn noise_on_the_providers_stderr_does_not_disturb_the_protocol() {
    let rig = Rig::up(&["noise"]);
    assert_eq!(
        rig.provider.get(&get("fake-3")).unwrap().task.unwrap().id,
        "fake-3"
    );
}

// ---- notifications --------------------------------------------------------------------------------------------------

#[test]
fn after_watch_the_six_notifications_arrive_as_typed_events_in_order() {
    let rig = Rig::up(&["events-after=watch"]);
    let events = rig.provider.take_events().expect("the events channel");
    rig.provider
        .watch(&WatchParams {
            checkouts: vec!["main".to_string()],
        })
        .unwrap();
    let got: Vec<ProviderEvent> = (0..6).map(|_| next_event(&events)).collect();
    assert_eq!(kinds(&got), SIX);
    match &got[0] {
        ProviderEvent::Changed(changed) => assert_eq!(changed.key, "fake:1"),
        other => panic!("{other:?}"),
    }
    match &got[4] {
        ProviderEvent::Collision(collision) => assert_eq!(collision.locations.len(), 2),
        other => panic!("{other:?}"),
    }
}

#[test]
fn events_that_come_before_a_reply_are_delivered_and_the_call_still_answers() {
    let rig = Rig::up(&["events-before=list"]);
    let events = rig.provider.take_events().unwrap();
    let list = rig
        .provider
        .list(&ListParams {
            checkout: None,
            cursor: None,
        })
        .unwrap();
    assert_eq!(list.tasks.len(), 1);
    let got: Vec<ProviderEvent> = (0..6).map(|_| next_event(&events)).collect();
    assert_eq!(kinds(&got), SIX);
}

#[test]
fn the_events_channel_can_be_taken_once() {
    let rig = Rig::up(&[]);
    assert!(rig.provider.take_events().is_some());
    assert!(rig.provider.take_events().is_none());
}

#[test]
fn a_watch_is_asked_again_of_the_next_child_and_its_events_reach_the_same_channel() {
    let root = TempRoot::new();
    let flag = root.join("crashed-once").display().to_string();
    let once = format!("crash-once=list:{flag}");
    let rig = Rig::with(root, &["events-after=watch", &once], policy(), |_| {});
    rig.provider.initialize(&params()).unwrap();
    let events = rig.provider.take_events().unwrap();
    let watch = WatchParams {
        checkouts: vec!["main".to_string()],
    };
    rig.provider.watch(&watch).unwrap();
    // The list kills the child, which is started again and told to watch again, and then answers.
    rig.provider
        .list(&ListParams {
            checkout: None,
            cursor: None,
        })
        .unwrap();
    let got: Vec<ProviderEvent> = (0..12).map(|_| next_event(&events)).collect();
    let mut want = SIX.to_vec();
    want.extend(SIX);
    assert_eq!(kinds(&got), want);
}

// ---- restart with backoff -------------------------------------------------------------------------------------------

#[test]
fn a_child_that_exits_between_calls_is_started_again_after_one_second_and_told_what_the_first_was()
{
    let rig = Rig::up(&["exit-after=1"]);
    let first = rig.pid();
    rig.stopped_running();
    let second = rig.pid();
    assert_ne!(first, second, "a new child");
    assert_eq!(rig.asked(), seconds(&[1]));
    assert_eq!(
        rig.echo()["init"],
        json!({"config": {"count": 3, "mode": "x"}, "root": "/work/main"})
    );
}

#[test]
fn a_successful_call_resets_the_wait_to_one_second() {
    let rig = Rig::up(&["exit-after=1"]);
    for _ in 0..3 {
        rig.pid();
        rig.stopped_running();
    }
    rig.pid();
    assert_eq!(rig.asked(), seconds(&[1, 1, 1]));
    assert_eq!(rig.provider.crashes_in_a_row(), 0);
}

#[test]
fn crashes_in_a_row_wait_one_second_then_two_and_the_call_after_them_succeeds() {
    let root = TempRoot::new();
    let down = root.join("down");
    let down_text = format!("down-file={}", down.display());
    let down_for_pause = down.clone();
    let rig = Rig::with(
        root,
        &["exit-after=1", &down_text],
        policy(),
        move |index| {
            // The first wait is before the second child, which finds the file and exits; the file goes in the
            // second wait.
            if index == 1 {
                std::fs::remove_file(&down_for_pause).unwrap();
            }
        },
    );
    rig.provider.initialize(&params()).unwrap();
    let first = rig.pid();
    std::fs::write(&down, "").unwrap();
    rig.stopped_running();
    let next = rig.pid();
    assert_ne!(first, next);
    assert_eq!(rig.asked(), seconds(&[1, 2]));
    assert_eq!(
        rig.provider.crashes_in_a_row(),
        0,
        "the call that worked reset the count"
    );
}

#[test]
fn the_fifth_crash_in_a_row_gives_gone_and_nothing_is_started_after_it() {
    let root = TempRoot::new();
    let down = root.join("down");
    let down_text = format!("down-file={}", down.display());
    let rig = Rig::with(root, &["exit-after=1", &down_text], policy(), |_| {});
    rig.provider.initialize(&params()).unwrap();
    rig.pid();
    std::fs::write(&down, "").unwrap();
    rig.stopped_running();
    let list = ListParams {
        checkout: None,
        cursor: None,
    };
    assert!(!rig.provider.is_gone());
    assert_eq!(rig.provider.list(&list), Err(ProviderError::Gone));
    assert!(
        rig.provider.is_gone(),
        "a gone provider says so without a call"
    );
    assert_eq!(rig.asked(), seconds(&[1, 2, 4, 8]));
    assert_eq!(rig.provider.crashes_in_a_row(), 5);
    let why = rig.provider.last_failure().expect("the reason");
    assert!(why.contains(NAME), "{why}");
    // Gone stays Gone: no wait, no start.
    assert_eq!(rig.provider.get(&get("fake-1")), Err(ProviderError::Gone));
    assert_eq!(rig.asked(), seconds(&[1, 2, 4, 8]));
    assert!(!rig.provider.is_running());
}

#[test]
fn an_explicit_retry_starts_the_child_once_more_and_resets_the_count() {
    let root = TempRoot::new();
    let down = root.join("down");
    let down_text = format!("down-file={}", down.display());
    let rig = Rig::with(root, &["exit-after=1", &down_text], policy(), |_| {});
    rig.provider.initialize(&params()).unwrap();
    rig.pid();
    std::fs::write(&down, "").unwrap();
    rig.stopped_running();
    let list = ListParams {
        checkout: None,
        cursor: None,
    };
    assert_eq!(rig.provider.list(&list), Err(ProviderError::Gone));
    std::fs::remove_file(&down).unwrap();
    rig.provider.retry().expect("the retry starts it");
    assert_eq!(rig.provider.crashes_in_a_row(), 0);
    assert_eq!(rig.provider.last_failure(), None);
    assert!(rig.provider.list(&list).is_ok());
    assert_eq!(
        rig.echo()["init"],
        json!({"config": {"count": 3, "mode": "x"}, "root": "/work/main"}),
        "the retry told it what the first start did"
    );
}

#[test]
fn a_retry_before_any_initialize_fails() {
    let rig = Rig::with(TempRoot::new(), &[], policy(), |_| {});
    let message = failed_message(rig.provider.retry().unwrap_err());
    assert!(
        message.contains(NAME) && message.contains("initialize"),
        "{message}"
    );
}

#[test]
fn a_read_that_was_in_flight_when_the_child_died_is_asked_again_of_the_new_child() {
    let root = TempRoot::new();
    let flag = root.join("crashed-once").display().to_string();
    let once = format!("crash-once=list:{flag}");
    let rig = Rig::with(root, &[&once], policy(), |_| {});
    rig.provider.initialize(&params()).unwrap();
    let list = rig
        .provider
        .list(&ListParams {
            checkout: None,
            cursor: None,
        })
        .unwrap();
    assert_eq!(list.tasks.len(), 1);
    assert_eq!(rig.asked(), seconds(&[1]));
}

#[test]
fn a_read_that_kills_the_child_every_time_gives_gone_after_five() {
    let rig = Rig::up(&["crash-on=list"]);
    let result = rig.provider.list(&ListParams {
        checkout: None,
        cursor: None,
    });
    assert_eq!(result, Err(ProviderError::Gone));
    assert_eq!(rig.asked(), seconds(&[1, 2, 4, 8]));
}

#[test]
fn an_apply_that_was_in_flight_when_the_child_died_is_not_asked_again() {
    let rig = Rig::up(&["crash-on=apply"]);
    let message = failed_message(rig.provider.apply(&op("ok")).unwrap_err());
    assert!(
        message.contains(NAME) && message.contains("`apply`"),
        "{message}"
    );
    assert!(message.contains("may have"), "{message}");
    assert!(rig.asked().is_empty(), "no wait and no second ask");
    assert!(!rig.provider.is_running());
    assert_eq!(rig.provider.crashes_in_a_row(), 1);
    // The next call starts a new child, after the first wait.
    rig.pid();
    assert_eq!(rig.asked(), seconds(&[1]));
}

// ---- timeouts -------------------------------------------------------------------------------------------------------

#[test]
fn a_child_that_does_not_answer_fails_the_call_at_the_timeout_and_the_next_call_starts_a_new_child()
{
    let mut short = policy();
    short.timeouts.list = Duration::from_millis(300);
    // A long grace: a hung child is killed at the timeout, and does not wait out the grace.
    short.stop_grace = Duration::from_secs(60);
    let rig = Rig::with(TempRoot::new(), &["hang-on=list"], short, |_| {});
    rig.provider.initialize(&params()).unwrap();
    let first = rig.pid();
    let started = Instant::now();
    let message = failed_message(
        rig.provider
            .list(&ListParams {
                checkout: None,
                cursor: None,
            })
            .unwrap_err(),
    );
    assert!(started.elapsed() < WITHIN, "the call came back");
    assert!(
        message.contains(NAME) && message.contains("`list`"),
        "{message}"
    );
    assert!(message.contains("0.3 s"), "{message}");
    assert!(!rig.provider.is_running(), "the hung child was killed");
    wait_until("the hung child to be gone", WITHIN, || {
        !pid_alive(first as u32)
    });
    let second = rig.pid();
    assert_ne!(first, second);
    assert_eq!(rig.asked(), seconds(&[1]));
}

#[test]
fn each_method_has_its_own_timeout() {
    let mut short = policy();
    short.timeouts.get = Duration::from_millis(300);
    let rig = Rig::with(TempRoot::new(), &["hang-on=get"], short, |_| {});
    rig.provider.initialize(&params()).unwrap();
    // `list` has the long timeout and answers; `get` has the short one and does not.
    assert!(
        rig.provider
            .list(&ListParams {
                checkout: None,
                cursor: None
            })
            .is_ok()
    );
    let message = failed_message(rig.provider.get(&get("fake-1")).unwrap_err());
    assert!(message.contains("`get`"), "{message}");
}

// ---- the child, its folder and its environment ----------------------------------------------------------------------

#[test]
fn the_child_starts_in_the_packs_own_folder() {
    let rig = Rig::up(&[]);
    let cwd = rig.echo()["cwd"].as_str().unwrap().to_string();
    let pack = rig.root.join("pack");
    assert_eq!(
        std::fs::canonicalize(cwd).unwrap(),
        std::fs::canonicalize(pack).unwrap()
    );
}

/// The names a child of `python3` has when it is given nothing: the system adds a few of its own.
fn names_of_an_empty_environment() -> Vec<String> {
    let output = std::process::Command::new(common::PYTHON)
        .args(["-c", "import os; print(chr(10).join(sorted(os.environ)))"])
        .env_clear()
        .output()
        .unwrap();
    String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(str::to_string)
        .collect()
}

#[test]
fn the_childs_environment_is_the_given_one_and_the_pack_data_folder_and_nothing_else() {
    let root = TempRoot::new();
    let mut launch = fake_launch(&root, &[]);
    let mut env = base_env(&root);
    env.insert("COLLIDE_TEST_MARK".to_string(), "x".to_string());
    launch.env = Some(env);
    let provider = SupervisedProvider::new(NAME, launch, policy());
    provider.initialize(&params()).unwrap();
    let fields =
        serde_json::to_value(provider.get(&get("echo")).unwrap().task.unwrap().fields).unwrap();
    let mut want = names_of_an_empty_environment();
    want.extend(
        [
            "PATH",
            "HOME",
            "COLLIDE_DAEMON_ROOT",
            "COLLIDE_TEST_MARK",
            "COLLIDE_PACK_DATA",
        ]
        .map(str::to_string),
    );
    want.sort();
    want.dedup();
    assert_eq!(fields["env"], json!(want));
    drop(provider);
}

#[test]
fn a_launch_with_no_environment_is_refused_and_starts_no_child() {
    let root = TempRoot::new();
    let mut launch = fake_launch(&root, &[]);
    launch.env = None;
    let provider = SupervisedProvider::new(NAME, launch, policy());
    let message = failed_message(provider.initialize(&params()).unwrap_err());
    assert!(
        message.contains(NAME) && message.contains("no base environment"),
        "{message}"
    );
    assert!(!provider.is_running(), "no child was started");
}

// ---- stopping -------------------------------------------------------------------------------------------------------

#[test]
fn stop_closes_the_childs_input_and_the_child_ends_and_calls_need_initialize_again() {
    let rig = Rig::up(&[]);
    let pid = rig.pid();
    assert!(!rig.provider.is_stopped());
    rig.provider.stop();
    assert!(rig.provider.is_stopped(), "a stop is told to a reader");
    assert!(!rig.provider.is_running());
    wait_until("the child to be gone", WITHIN, || !pid_alive(pid as u32));
    let message = failed_message(rig.provider.get(&get("fake-1")).unwrap_err());
    assert!(message.contains("initialize"), "{message}");
    assert!(
        !rig.provider.is_running(),
        "nothing restarts a stopped provider"
    );
}

#[test]
fn stop_kills_a_child_that_ignores_the_closed_input_once_the_grace_is_over() {
    let rig = Rig::up(&["ignore-eof"]);
    let pid = rig.pid();
    let started = Instant::now();
    rig.provider.stop();
    assert!(started.elapsed() < WITHIN);
    wait_until("the child to be gone", WITHIN, || !pid_alive(pid as u32));
}

#[test]
fn dropping_the_supervisor_ends_its_child() {
    let rig = Rig::up(&["ignore-eof"]);
    let pid = rig.pid();
    let Rig { provider, root, .. } = rig;
    drop(provider);
    wait_until("the child to be gone", WITHIN, || !pid_alive(pid as u32));
    drop(root);
}
