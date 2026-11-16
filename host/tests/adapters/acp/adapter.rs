//! `AcpAdapter` built from a test profile (ts-0088-pk): the capability table, the launch spec, a session against the
//! fake child, the config paths and the interactive command. The profile is built here and names no real agent.

use super::common::TempRoot;
use super::fake::{FakeAcp, recording};
use super::session::{SESSION, written};
use collide_core::adapter::{
    AgentAdapter, ConfigPath, ConfigRoot, EnvStrip, LaunchError, LaunchRequest, SessionRef,
};
use collide_core::agent::{
    AgentEvent, Capabilities, McpServers, ModelSelect, PermissionHost, SessionResume, SessionStart,
    Support, UsageQuality, UsageReport, UsageScope,
};
use collide_core::testing::acp::initialize_response;
use collide_core::testing::sample_launch_request;
use collide_host::adapters::acp::AcpAdapter;
use collide_host::adapters::acp::options::McpServer;
use collide_host::adapters::acp::probe::Probed;
use collide_host::adapters::acp::profile::AcpProfile;
use std::collections::BTreeMap;
use std::path::PathBuf;

const PROGRAM: &str = "test-acp-agent";
const AGENT: &str = "test-agent";
const SECRET_PROMPT: &str = "the-secret-words-of-the-first-prompt";

fn profile() -> AcpProfile {
    AcpProfile::new(AGENT, PROGRAM, vec!["serve".into(), "--quiet".into()])
}

fn adapter() -> AcpAdapter {
    AcpAdapter::new(profile())
}

fn request() -> LaunchRequest {
    let mut request = sample_launch_request();
    request.cwd = PathBuf::from("/work/repo");
    request.prompt = SECRET_PROMPT.into();
    request
}

fn names(list: &[&str]) -> Vec<String> {
    list.iter().map(|name| name.to_string()).collect()
}

fn refusal_of(result: Result<collide_core::adapter::LaunchSpec, LaunchError>) -> (String, Support) {
    match result {
        Err(LaunchError::Capability(refusal)) => (refusal.capability, refusal.support),
        other => panic!("expected a capability refusal, got {other:?}"),
    }
}

// ---- capabilities -------------------------------------------------------------------------------------------------

#[test]
fn a_profile_with_no_usage_claim_gives_the_engines_fixed_flags_and_unknown_for_the_rest() {
    assert_eq!(
        adapter().capabilities(),
        Capabilities {
            headless_stream: Support::Yes,
            permission_host: PermissionHost {
                support: Support::Yes,
                deny_message: Support::No,
                input_at_request: Support::No,
                request_reason: Support::Unknown,
            },
            session_resume: SessionResume::Unknown,
            background: Support::No,
            budget_native: Support::No,
            tool_allowlist: Support::No,
            named_agent: Support::No,
            usage_report: UsageReport::default(),
            model_select: ModelSelect::Session,
            take_over: Support::No,
            mcp_servers: McpServers {
                support: Support::Yes,
                stdio: Support::Yes,
                http: Support::Unknown,
                sse: Support::No,
            },
            ..Capabilities::default()
        }
    );
}

#[test]
fn what_the_profile_claims_about_usage_limits_and_take_over_is_in_the_table() {
    let mut profile = profile();
    profile.usage_report = UsageReport {
        support: Support::Yes,
        granularity: vec![UsageScope::Turn],
        cost: Support::No,
        quality: Some(UsageQuality::Partial),
        ..UsageReport::default()
    };
    profile.limit_signal = Support::No;
    profile.resume_argv = Some(names(&["test-acp-agent", "--session"]));
    let table = AcpAdapter::new(profile.clone()).capabilities();
    assert_eq!(table.usage_report, profile.usage_report);
    assert_eq!(table.limit_signal, Support::No);
    assert_eq!(table.take_over, Support::Yes);
}

#[test]
fn the_engines_flags_are_the_same_for_every_profile() {
    let mut other = AcpProfile::new("other-agent", "other-acp-agent", vec![]);
    other.env_strip = EnvStrip {
        names: names(&["OTHER_AGENT_MARK"]),
        prefixes: vec![],
    };
    let (one, two) = (
        adapter().capabilities(),
        AcpAdapter::new(other).capabilities(),
    );
    assert_eq!(one, two);
}

#[test]
fn a_probe_that_has_run_is_merged_into_the_table_and_nothing_else_moves() {
    let before = adapter().capabilities();
    let probed = Probed {
        session_resume: SessionResume::Replay,
        mcp_http: Support::Yes,
    };
    let after = adapter().probed(probed).capabilities();
    let mut want = before;
    want.session_resume = SessionResume::Replay;
    want.mcp_servers.http = Support::Yes;
    assert_eq!(after, want);
}

// ---- launch -------------------------------------------------------------------------------------------------------

#[test]
fn launch_gives_the_profiles_argv_the_requests_folder_and_the_profiles_strip_list() {
    let mut profile = profile();
    profile.env_strip = EnvStrip {
        names: names(&["TEST_AGENT_MARK"]),
        prefixes: names(&["TEST_AGENT_"]),
    };
    let spec = AcpAdapter::new(profile).launch(&request()).unwrap();
    assert_eq!(spec.argv, names(&[PROGRAM, "serve", "--quiet"]));
    assert_eq!(spec.cwd, PathBuf::from("/work/repo"));
    assert_eq!(spec.env_strip.names, names(&["TEST_AGENT_MARK"]));
    assert_eq!(spec.env_strip.prefixes, names(&["TEST_AGENT_"]));
    assert_eq!(spec.env, BTreeMap::new());
}

#[test]
fn the_first_prompt_is_in_neither_argv_nor_env() {
    let spec = adapter().launch(&request()).unwrap();
    assert!(spec.argv.iter().all(|word| !word.contains(SECRET_PROMPT)));
    assert!(
        spec.env
            .values()
            .all(|value| !value.contains(SECRET_PROMPT))
    );
    assert!(spec.env.keys().all(|key| !key.contains(SECRET_PROMPT)));
}

#[test]
fn a_required_capability_that_is_unknown_is_refused_with_its_name() {
    let adapter = adapter().requiring(names(&["session_resume"]));
    let (name, support) = refusal_of(adapter.launch(&request()));
    assert_eq!(
        (name.as_str(), support),
        ("session_resume", Support::Unknown)
    );
}

#[test]
fn a_required_capability_that_is_no_is_refused_with_its_name() {
    let adapter = adapter().requiring(names(&["background"]));
    let (name, support) = refusal_of(adapter.launch(&request()));
    assert_eq!((name.as_str(), support), ("background", Support::No));
}

#[test]
fn a_required_sub_flag_the_engine_says_no_to_is_refused_by_its_dotted_name() {
    let adapter = adapter().requiring(names(&["permission_host.deny_message"]));
    let (name, support) = refusal_of(adapter.launch(&request()));
    assert_eq!(
        (name.as_str(), support),
        ("permission_host.deny_message", Support::No)
    );
}

#[test]
fn the_refusal_names_the_agent_of_the_profile() {
    let adapter = adapter().requiring(names(&["background"]));
    match adapter.launch(&request()) {
        Err(LaunchError::Capability(refusal)) => assert_eq!(refusal.agent, AGENT),
        other => panic!("expected a capability refusal, got {other:?}"),
    }
}

#[test]
fn a_required_capability_that_is_yes_launches() {
    let adapter = adapter().requiring(names(&[
        "headless_stream",
        "permission_host",
        "mcp_servers.stdio",
    ]));
    assert!(adapter.launch(&request()).is_ok());
}

#[test]
fn a_probe_that_says_yes_lets_a_role_that_requires_it_launch() {
    let required = names(&["session_resume", "mcp_servers.http"]);
    assert!(
        adapter()
            .requiring(required.clone())
            .launch(&request())
            .is_err()
    );
    let probed = Probed {
        session_resume: SessionResume::Replay,
        mcp_http: Support::Yes,
    };
    assert!(
        adapter()
            .requiring(required)
            .probed(probed)
            .launch(&request())
            .is_ok()
    );
}

#[test]
fn a_session_id_cannot_be_assigned_or_resumed_because_the_session_cannot_carry_it_yet() {
    let starts = [
        SessionStart::Assign {
            uuid: "7c1b0e52-9f3a-4d2e-8b61-0a5d3c9e4f17".into(),
        },
        SessionStart::Resume {
            id: "ses-stand-in-1".into(),
        },
    ];
    for start in starts {
        let mut request = request();
        request.session = start.clone();
        match adapter().launch(&request) {
            Err(LaunchError::Invalid { message }) => {
                assert!(message.contains("AgentAssigned"), "{message}")
            }
            other => panic!("{start:?}: expected an invalid request, got {other:?}"),
        }
    }
}

#[test]
fn launch_writes_no_file() {
    let root = TempRoot::new();
    let mut request = request();
    request.cwd = root.path().to_path_buf();
    request.snapshot_dir = root.join("snapshot");
    adapter().launch(&request).unwrap();
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
}

// ---- session ------------------------------------------------------------------------------------------------------

#[test]
fn the_first_two_lines_a_session_writes_are_initialize_and_session_new_in_the_requests_folder() {
    let mut session = adapter().session(&request());
    let first = written(&session.start());
    assert_eq!(first.len(), 1);
    assert_eq!(first[0]["method"], "initialize");
    let second = written(&session.feed(&initialize_response(0), 100));
    assert_eq!(second.len(), 1);
    assert_eq!(second[0]["method"], "session/new");
    assert_eq!(second[0]["params"]["cwd"], "/work/repo");
    assert_eq!(second[0]["params"]["mcpServers"], serde_json::json!([]));
}

#[test]
fn a_session_against_the_fake_child_writes_what_the_child_expects_and_binds_its_id() {
    let root = TempRoot::new();
    let mut request = request();
    request.cwd = root.join("work");
    let mut child = FakeAcp::start(&root, &recording("handshake-bare"));
    let mut session = adapter().session(&request);

    let initialize = session.start().writes.remove(0);
    child.send_bytes(&initialize);
    let response = child.next_line().expect("the child answers initialize");
    let mut step = session.feed(&response, 100);
    assert!(step.events.is_empty());
    let new = step.writes.remove(0);
    assert_eq!(written_method(&initialize), "initialize");
    assert_eq!(written_method(&new), "session/new");

    child.send_bytes(&new);
    let response = child.next_line().expect("the child answers session/new");
    let step = session.feed(&response, 110);
    let events: Vec<AgentEvent> = step
        .events
        .into_iter()
        .map(|stamped| stamped.event)
        .collect();
    assert_eq!(
        events,
        vec![AgentEvent::SessionBound { id: SESSION.into() }]
    );
    assert_eq!(child.finish().code, Some(0));
}

fn written_method(bytes: &[u8]) -> String {
    let value: serde_json::Value = serde_json::from_slice(bytes).unwrap();
    value["method"].as_str().unwrap().to_string()
}

#[test]
fn the_servers_of_the_adapter_are_in_session_new_and_the_prompt_is_the_requests() {
    let server = McpServer::Http {
        name: "stand-in".into(),
        url: "http://127.0.0.1:1/mcp".into(),
        headers: vec![],
    };
    let mut session = adapter()
        .with_mcp_servers(vec![server.clone()])
        .session(&request());
    session.start();
    let new = written(&session.feed(&initialize_response(0), 100));
    assert_eq!(
        new[0]["params"]["mcpServers"],
        serde_json::json!([server.to_json()])
    );
    let prompt = written(&session.feed(
        &collide_core::testing::acp::new_session_response(1, SESSION),
        110,
    ));
    assert_eq!(prompt[0]["method"], "session/prompt");
    assert_eq!(prompt[0]["params"]["prompt"][0]["text"], SECRET_PROMPT);
}

// ---- config paths and the interactive command ---------------------------------------------------------------------

#[test]
fn the_config_paths_are_the_profiles_and_empty_for_a_profile_that_has_none() {
    assert_eq!(adapter().config_paths(), vec![]);
    let mut profile = profile();
    profile.config_paths = vec![
        ConfigPath {
            root: ConfigRoot::Repo,
            pattern: ".test-agent/**".into(),
        },
        ConfigPath {
            root: ConfigRoot::Home,
            pattern: ".config/test-agent/**".into(),
        },
    ];
    assert_eq!(
        AcpAdapter::new(profile.clone()).config_paths(),
        profile.config_paths
    );
}

#[test]
fn the_interactive_command_is_the_profiles_with_the_session_id_last_in_the_sessions_folder() {
    let mut profile = profile();
    profile.resume_argv = Some(names(&["test-acp-agent", "--session"]));
    let session = SessionRef {
        id: "ses_stand_in_1".into(),
        cwd: PathBuf::from("/work/tree"),
    };
    let command = AcpAdapter::new(profile)
        .interactive_command(&session)
        .unwrap();
    assert_eq!(
        command.argv,
        names(&["test-acp-agent", "--session", "ses_stand_in_1"])
    );
    assert_eq!(command.cwd, PathBuf::from("/work/tree"));
}

#[test]
fn a_profile_with_no_resume_command_has_no_interactive_command() {
    let session = SessionRef {
        id: "ses_stand_in_1".into(),
        cwd: PathBuf::from("/work/tree"),
    };
    assert_eq!(adapter().interactive_command(&session), None);
}

#[test]
fn a_session_id_that_could_pass_for_a_flag_gets_no_interactive_command() {
    let mut profile = profile();
    profile.resume_argv = Some(names(&["test-acp-agent", "--session"]));
    let adapter = AcpAdapter::new(profile);
    for id in ["--evil", "-x", "", "a b", "../up"] {
        let session = SessionRef {
            id: id.into(),
            cwd: PathBuf::from("/work/tree"),
        };
        assert_eq!(adapter.interactive_command(&session), None, "{id:?}");
    }
}
