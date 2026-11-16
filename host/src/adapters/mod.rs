//! The agent adapters of `host` (0.3). `core` holds the two traits ([`collide_core::adapter`]) and nothing that names
//! a tool; each adapter is a module here. v0 ships one, Claude Code, for runs. The ACP engine ([`acp`]) is the second
//! `AgentSession` (Stage 3d), with [`acp::AcpAdapter`] built from a profile; the first real profile is Stage 6b's.

pub mod acp;
pub mod claude;
