//! The agent adapters of `host` (0.3). `core` holds the two traits ([`collide_core::adapter`]) and nothing that names
//! a tool; each adapter is a module here. v0 ships one, Claude Code, for runs. The ACP engine ([`acp`]) is the second
//! `AgentSession` (Stage 3d); the profile of each ACP agent is Stage 3d's `ts-0088-pk` and Stage 6b's.

pub mod acp;
pub mod claude;
