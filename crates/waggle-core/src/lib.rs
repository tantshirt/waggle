//! waggle domain types and pure transforms.
//!
//! **AD-1: this crate performs no I/O.** No file, socket, process, clock, or random
//! source. Everything here is a pure function over owned types, so it can be tested
//! without a substrate, a method installation, or a network.

pub mod pins;
pub mod version;

pub use pins::{parse_pins, range};
pub use version::{Compatibility, Version, VersionRange};
