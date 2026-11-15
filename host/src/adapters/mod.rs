//! The agent adapters of `host` (0.3). `core` holds the two traits ([`collide_core::adapter`]) and nothing that names
//! a tool; each adapter is a module here. v0 ships one, Claude Code, for runs. The ACP engine and the profile of each
//! ACP agent are Stage 3d and Stage 6b and are not outlined here.

pub mod claude;
