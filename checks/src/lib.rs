//! The structural checks of `IMPLEMENTATION_PLAN.md` 0.15 ("Checks from the skeleton on"). Each module is one check
//! over a repository root and returns what it found; `main` and the tests decide what a finding means.
//!
//! This crate's own text must pass the checks, so the names it looks for are joined at run time, never written whole.

pub mod deps;
pub mod names;
pub mod stubs;
mod walk;
