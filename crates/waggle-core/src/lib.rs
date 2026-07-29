//! waggle domain types and pure transforms.
//!
//! **AD-1: this crate performs no I/O.** No file, socket, process, clock, or random
//! source. Everything here is a pure function over owned types, so it can be tested
//! without a substrate, a method installation, or a network.

pub mod artifact;
pub mod compile;
pub mod gate;
pub mod lint;
pub mod merge;
pub mod pins;
pub mod version;

pub use artifact::{ArtifactEvent, ArtifactKind, Priority};
pub use compile::{compile_persona, CompileError, CompileReport, MenuItem, PersonaPack};
pub use gate::{authorize, GateError, GateVerdict, Role, Verdict};
pub use lint::{has_errors, Finding, Severity};
pub use merge::{merge, merge_layers};
pub use pins::{parse_pins, range};
pub use version::{Compatibility, Version, VersionRange};
