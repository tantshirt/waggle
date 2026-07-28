//! Version comparison for the compatibility preflight (FR-28, AD-18).
//!
//! Pure. No I/O — detection lives in the adapter crates; this only decides.
//!
//! AD-18 requires a supported *range*, not a single pinned tag, and requires waggle to
//! refuse outside it rather than degrade unpredictably. The refusal has to name both the
//! found and the expected version (NFR-4), so [`Compatibility`] carries enough to say so.

use serde::Serialize;
use std::cmp::Ordering;
use std::fmt;

/// A `major.minor.patch` version.
///
/// Deliberately not full semver: upstream tags look like `v0.4.26` and method versions
/// look like `6.10.0`. Pre-release and build metadata are parsed off and ignored rather
/// than ordered, because ordering them correctly is subtle and we do not need it. If that
/// changes, take a real semver dependency instead of growing this.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Version {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
}

impl Version {
    pub const fn new(major: u64, minor: u64, patch: u64) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }

    /// Parse `1.2.3`, `v1.2.3`, `1.2`, or `1`. Missing components are zero.
    ///
    /// Anything after the first `-` or `+` is discarded (pre-release / build metadata).
    /// Returns `None` rather than guessing — AD-18 says refuse, do not degrade.
    pub fn parse(raw: &str) -> Option<Self> {
        let s = raw.trim();
        let s = s
            .strip_prefix('v')
            .or_else(|| s.strip_prefix('V'))
            .unwrap_or(s);
        // Drop pre-release / build metadata; we compare release lines only.
        // A delimiter with nothing after it is malformed, not "no metadata" — AD-18 says
        // refuse rather than guess, so `1.2.3-` is rejected instead of read as `1.2.3`.
        let s = match s.find(['-', '+']) {
            Some(i) => {
                if s[i + 1..].is_empty() {
                    return None;
                }
                &s[..i]
            }
            None => s,
        };
        if s.is_empty() {
            return None;
        }

        let mut parts = s.split('.');
        let major = parse_component(parts.next())?;
        // A missing component is 0; a *present but malformed* one is an error.
        let minor = match parts.next() {
            None => 0,
            Some(p) => parse_component(Some(p))?,
        };
        let patch = match parts.next() {
            None => 0,
            Some(p) => parse_component(Some(p))?,
        };
        if parts.next().is_some() {
            return None; // more than three components: not a shape we understand
        }
        Some(Self::new(major, minor, patch))
    }
}

fn parse_component(p: Option<&str>) -> Option<u64> {
    let p = p?;
    if p.is_empty() || !p.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    p.parse().ok()
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.major, self.minor, self.patch).cmp(&(other.major, other.minor, other.patch))
    }
}

/// A supported range: inclusive minimum, exclusive maximum.
///
/// Exclusive at the top so `>=0.4.0, <0.5.0` reads naturally and a new minor release is
/// refused until someone deliberately widens the range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct VersionRange {
    pub min_inclusive: Version,
    pub max_exclusive: Option<Version>,
}

impl VersionRange {
    pub const fn new(min_inclusive: Version, max_exclusive: Option<Version>) -> Self {
        Self {
            min_inclusive,
            max_exclusive,
        }
    }

    /// Parse `>=0.4.0,<0.5.0`. Whitespace is ignored. An unbounded top is allowed.
    pub fn parse(raw: &str) -> Option<Self> {
        let mut min = None;
        let mut max = None;
        for clause in raw.split(',') {
            let clause = clause.trim();
            if clause.is_empty() {
                continue;
            }
            if let Some(rest) = clause.strip_prefix(">=") {
                min = Some(Version::parse(rest)?);
            } else if let Some(rest) = clause.strip_prefix('<') {
                max = Some(Version::parse(rest)?);
            } else {
                return None; // unknown operator: refuse rather than guess
            }
        }
        Some(Self::new(min?, max))
    }

    pub fn check(&self, found: Version) -> Compatibility {
        if found < self.min_inclusive {
            return Compatibility::TooOld;
        }
        if let Some(max) = self.max_exclusive {
            if found >= max {
                return Compatibility::TooNew;
            }
        }
        Compatibility::Supported
    }
}

impl fmt::Display for VersionRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.max_exclusive {
            Some(max) => write!(f, ">={}, <{}", self.min_inclusive, max),
            None => write!(f, ">={}", self.min_inclusive),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Compatibility {
    Supported,
    TooOld,
    TooNew,
}

impl Compatibility {
    pub const fn is_supported(self) -> bool {
        matches!(self, Compatibility::Supported)
    }

    /// Human-facing reason, for the refusal message required by NFR-4.
    pub const fn reason(self) -> &'static str {
        match self {
            Compatibility::Supported => "within the supported range",
            Compatibility::TooOld => "older than the supported range",
            Compatibility::TooNew => "newer than the supported range",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_shapes_upstream_actually_uses() {
        // Buzz release tag
        assert_eq!(Version::parse("v0.4.26"), Some(Version::new(0, 4, 26)));
        // BMAD method version
        assert_eq!(Version::parse("6.10.0"), Some(Version::new(6, 10, 0)));
        // BMAD external module tag
        assert_eq!(Version::parse("v1.19.1"), Some(Version::new(1, 19, 1)));
        // short forms
        assert_eq!(Version::parse("1.95"), Some(Version::new(1, 95, 0)));
        assert_eq!(Version::parse("7"), Some(Version::new(7, 0, 0)));
        // whitespace is tolerated; detection reads from files
        assert_eq!(Version::parse("  v0.4.26\n"), Some(Version::new(0, 4, 26)));
    }

    #[test]
    fn drops_prerelease_and_build_metadata() {
        assert_eq!(
            Version::parse("6.10.1-next.22"),
            Some(Version::new(6, 10, 1))
        );
        assert_eq!(
            Version::parse("1.1.3+spec-1.1.0"),
            Some(Version::new(1, 1, 3))
        );
    }

    #[test]
    fn refuses_garbage_rather_than_guessing() {
        for bad in [
            "", "v", "abc", "1.x.3", "1..3", "1.2.3.4", "-1.0.0", "1.2.3-",
        ] {
            assert_eq!(Version::parse(bad), None, "should not parse: {bad:?}");
        }
    }

    #[test]
    fn orders_by_component_not_lexically() {
        // The bug this guards: "0.4.9" > "0.4.26" under string comparison.
        assert!(Version::parse("0.4.26").unwrap() > Version::parse("0.4.9").unwrap());
        assert!(Version::parse("0.10.0").unwrap() > Version::parse("0.9.99").unwrap());
        assert!(Version::parse("1.0.0").unwrap() > Version::parse("0.99.99").unwrap());
    }

    #[test]
    fn range_boundaries_are_min_inclusive_max_exclusive() {
        let r = VersionRange::parse(">=0.4.0,<0.5.0").unwrap();
        assert_eq!(
            r.check(Version::parse("0.4.0").unwrap()),
            Compatibility::Supported
        );
        assert_eq!(
            r.check(Version::parse("0.4.26").unwrap()),
            Compatibility::Supported
        );
        assert_eq!(
            r.check(Version::parse("0.4.99").unwrap()),
            Compatibility::Supported
        );
        assert_eq!(
            r.check(Version::parse("0.3.99").unwrap()),
            Compatibility::TooOld
        );
        // exactly the exclusive max is refused
        assert_eq!(
            r.check(Version::parse("0.5.0").unwrap()),
            Compatibility::TooNew
        );
    }

    #[test]
    fn unbounded_range_accepts_anything_above_the_floor() {
        let r = VersionRange::parse(">=6.10.0").unwrap();
        assert_eq!(r.max_exclusive, None);
        assert_eq!(
            r.check(Version::parse("99.0.0").unwrap()),
            Compatibility::Supported
        );
        assert_eq!(
            r.check(Version::parse("6.9.99").unwrap()),
            Compatibility::TooOld
        );
    }

    #[test]
    fn range_parse_refuses_unknown_operators() {
        // We do not silently treat `^` or `~` as something else.
        assert!(VersionRange::parse("^0.4.0").is_none());
        assert!(VersionRange::parse("~0.4.0").is_none());
        assert!(VersionRange::parse(">0.4.0").is_none());
        assert!(
            VersionRange::parse("<0.5.0").is_none(),
            "a range needs a minimum"
        );
    }

    #[test]
    fn round_trips_through_display() {
        let r = VersionRange::parse(">=0.4.0,<0.5.0").unwrap();
        assert_eq!(r.to_string(), ">=0.4.0, <0.5.0");
        assert_eq!(VersionRange::parse(&r.to_string()), Some(r));
    }
}
