//! The ACP engine's tests (Stage 3d), and the fake ACP child they run against (ts-0086-tm). Every process a test starts
//! has an environment of its own (`env_clear()`, then `HOME` and `COLLIDE_DAEMON_ROOT` in a temp root), and is reaped.
//! No test starts a real ACP agent or a daemon.

#[path = "../../common/mod.rs"]
mod common;

mod fake;
mod fake_acp;
