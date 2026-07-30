//! AD-20's exit-code taxonomy. Mirrors buzz-cli's shape so the two compose in scripts.

/// Everything succeeded.
pub const OK: u8 = 0;
/// The caller asked for something impossible — bad flag, missing input.
pub const USER: u8 = 1;
/// An upstream contract was violated: version out of range, schema unrecognized.
pub const UPSTREAM: u8 = 2;
/// Something broke that is neither the caller's fault nor upstream's.
pub const SYSTEM: u8 = 3;
