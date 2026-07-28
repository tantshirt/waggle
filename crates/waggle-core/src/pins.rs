//! Parsing for the pins file (`BUZZ_VERSION`).
//!
//! Pure: takes the file *contents*, never a path. Reading is the caller's job (AD-1).
//!
//! AD-18 requires the supported range to live in exactly one committed location, and
//! NFR-5 requires no floating versions anywhere. This file is that location.

use crate::VersionRange;
use std::collections::BTreeMap;

/// Parse `KEY=VALUE` lines, ignoring blanks and `#` comments.
///
/// Values may be quoted. Later keys win, so an override is possible without editing
/// earlier lines.
pub fn parse_pins(contents: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        let v = v.trim().trim_matches('"').trim_matches('\'');
        out.insert(k.trim().to_string(), v.to_string());
    }
    out
}

/// Look up a range by key, e.g. `BUZZ_SUPPORTED`.
pub fn range(pins: &BTreeMap<String, String>, key: &str) -> Option<VersionRange> {
    VersionRange::parse(pins.get(key)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
# a comment
BUZZ_VERSION=v0.4.26
BUZZ_SUPPORTED=">=0.4.0,<0.5.0"

BMAD_METHOD_VERSION=6.10.0
  BMAD_SUPPORTED = >=6.10.0,<7.0.0
"#;

    #[test]
    fn ignores_comments_and_blanks_and_trims() {
        let p = parse_pins(SAMPLE);
        assert_eq!(p.get("BUZZ_VERSION").map(String::as_str), Some("v0.4.26"));
        assert_eq!(
            p.get("BMAD_METHOD_VERSION").map(String::as_str),
            Some("6.10.0")
        );
        assert!(!p.contains_key("# a comment"));
    }

    #[test]
    fn strips_quotes_and_surrounding_space() {
        let p = parse_pins(SAMPLE);
        assert_eq!(
            p.get("BUZZ_SUPPORTED").map(String::as_str),
            Some(">=0.4.0,<0.5.0")
        );
        // key had leading spaces, value had spaces around `=`
        assert_eq!(
            p.get("BMAD_SUPPORTED").map(String::as_str),
            Some(">=6.10.0,<7.0.0")
        );
    }

    #[test]
    fn ranges_resolve_from_pins() {
        let p = parse_pins(SAMPLE);
        let r = range(&p, "BUZZ_SUPPORTED").expect("range should parse");
        assert_eq!(r.to_string(), ">=0.4.0, <0.5.0");
        assert!(range(&p, "NOPE").is_none());
    }

    #[test]
    fn the_committed_pins_file_is_valid() {
        // Guards NFR-5: the real file must always parse and carry both ranges.
        let contents = include_str!("../../../BUZZ_VERSION");
        let p = parse_pins(contents);
        assert!(
            range(&p, "BUZZ_SUPPORTED").is_some(),
            "BUZZ_SUPPORTED missing or malformed"
        );
        assert!(
            range(&p, "BMAD_SUPPORTED").is_some(),
            "BMAD_SUPPORTED missing or malformed"
        );
        assert!(p.contains_key("BUZZ_VERSION"));
        assert!(p.contains_key("BMAD_METHOD_VERSION"));
    }
}
