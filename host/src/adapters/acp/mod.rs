//! The ACP engine (0.3, Two core traits; Stage 3d): the second [`collide_core::adapter::AgentSession`]. ACP is a
//! stateful JSON-RPC conversation, so the session is the client: it holds the handshake, the request ids and the
//! requests the agent has not had answered. The protocol is written by hand; no ACP crate is used.
//!
//! | tag | where | what it does |
//! |---|---|---|
//! | `acp: options` | [`options::AcpOptions`] | the session's own data: working folder, first prompt, MCP servers |
//! | `acp: session` | [`session::AcpSession`] | the handshake, a prompt turn, the turn's end, replay from the journal |
//! | `acp: profile` | [`profile::AcpProfile`] | one agent as data: program, strip list, config paths, claims |
//! | `acp: probe` | [`probe::probe`], [`probe::Probed::merge_into`] | what an `initialize` response implies |
//! | `acp: adapter` | [`AcpAdapter`] | capabilities, launch, a session from a request, config paths, take-over |
//!
//! Nothing here names an agent: [`AcpAdapter`] is built from a profile, and the profiles of this stage are a test's.
//! The first real profile comes with Stage 6b.

pub mod options;
pub mod probe;
pub mod profile;
pub mod session;
use collide_core::adapter::{
    AgentAdapter, AgentSession, CommandSpec, ConfigPath, LaunchError, LaunchRequest, LaunchSpec,
    SessionRef, refuse_unmet_capabilities,
};
use collide_core::agent::{Capabilities, SessionStart};
use collide_core::resolve::is_safe_token;
use options::{AcpOptions, McpServer};
use probe::Probed;
use profile::AcpProfile;
use std::collections::BTreeMap;

/// The adapter for any ACP agent: the engine ([`session::AcpSession`]) and one [`AcpProfile`]. It has no state a phase
/// changes. What it holds is set when it is built: the probe's answer once one has run, the MCP servers a session asks
/// the agent for, and the capabilities a role requires.
#[derive(Debug, Clone)]
pub struct AcpAdapter {
    profile: AcpProfile,
    probed: Option<Probed>,
    mcp_servers: Vec<McpServer>,
    required: Vec<String>,
}

impl AcpAdapter {
    /// An adapter for the agent `profile` describes, with no probe, no MCP servers and no required capability.
    pub fn new(profile: AcpProfile) -> Self {
        Self {
            profile,
            probed: None,
            mcp_servers: Vec::new(),
            required: Vec::new(),
        }
    }

    /// The same adapter with the answer of a probe: [`AgentAdapter::capabilities`] then has it merged in.
    pub fn probed(mut self, probed: Probed) -> Self {
        self.probed = Some(probed);
        self
    }

    /// The same adapter whose sessions ask the agent for these servers in `session/new`.
    pub fn with_mcp_servers(mut self, servers: Vec<McpServer>) -> Self {
        self.mcp_servers = servers;
        self
    }

    /// The same adapter whose `launch` refuses what these capability names say and the table does not promise. The
    /// role key that carries them is a decision for the owner (`pack::Role` has none yet), so the caller that knows
    /// the role's list passes it here.
    pub fn requiring(mut self, required: Vec<String>) -> Self {
        self.required = required;
        self
    }
}

impl AgentAdapter for AcpAdapter {
    /// The profile's table, with the probe's answer merged in once there is one.
    fn capabilities(&self) -> Capabilities {
        let table = self.profile.capabilities();
        match &self.probed {
            Some(probed) => probed.merge_into(table),
            None => table,
        }
    }

    /// The profile's program and arguments, the profile's strip list and the request's working folder. It adds no
    /// environment and writes no file. The first prompt is not here: the session writes it (`AcpOptions`).
    ///
    /// A required capability the table does not say `Yes` for is refused with its name ([`LaunchError::Capability`]):
    /// the first of them, in the order the role wrote them. An ACP agent names its own session, so a request that
    /// assigns an id is refused. So is a resume, until the session has `session/load`.
    fn launch(&self, request: &LaunchRequest) -> Result<LaunchSpec, LaunchError> {
        if let Err(refusals) =
            refuse_unmet_capabilities(&self.profile.agent, &self.capabilities(), &self.required)
        {
            let first = refusals
                .into_iter()
                .next()
                .expect("a refusal list is not empty");
            return Err(LaunchError::Capability(first));
        }
        if request.session != (SessionStart::AgentAssigned {}) {
            return Err(LaunchError::Invalid {
                message: format!(
                    "`{}` is an ACP agent: it names its own session, so a phase starts with AgentAssigned \
                     (an assigned id cannot be sent, and a resume is not built yet)",
                    self.profile.agent
                ),
            });
        }
        let mut argv = vec![self.profile.program.clone()];
        argv.extend(self.profile.args.iter().cloned());
        Ok(LaunchSpec {
            argv,
            env: BTreeMap::new(),
            env_strip: self.profile.env_strip.clone(),
            cwd: request.cwd.clone(),
        })
    }

    /// An [`session::AcpSession`] for the request's working folder and first prompt, with the adapter's servers.
    fn session(&self, request: &LaunchRequest) -> Box<dyn AgentSession + Send> {
        let options = AcpOptions::new(request.cwd.clone(), request.prompt.clone())
            .with_mcp_servers(self.mcp_servers.clone());
        Box::new(session::AcpSession::new(options))
    }

    fn config_paths(&self) -> Vec<ConfigPath> {
        self.profile.config_paths.clone()
    }

    /// The profile's resume command with the session id added, in the session's folder. `None` for a profile with no
    /// such command, and for an id that is not a safe token (0.5): it would become an argument, and could pass for a
    /// flag.
    fn interactive_command(&self, session: &SessionRef) -> Option<CommandSpec> {
        let mut argv = self.profile.resume_argv.clone()?;
        if !is_safe_token(&session.id) {
            return None;
        }
        argv.push(session.id.clone());
        Some(CommandSpec {
            argv,
            cwd: session.cwd.clone(),
        })
    }
}
