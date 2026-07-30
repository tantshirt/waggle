//! CLI command handlers.
//!
//! Clap parsing and process wiring stay in `main`. Shared helpers live in
//! `crate::common` / `crate::output` / `crate::exit`. Large command bodies are
//! gradually moving into this module tree.

pub mod gate;
pub mod sync;
