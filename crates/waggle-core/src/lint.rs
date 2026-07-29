//! Portability lint for generated artifacts (FR-6, AD-8, AD-9, NFR-6).
//!
//! Pure: takes artifact text, returns findings. Reading files is the caller's job.
//!
//! The rules encode AD-8's kind policy so it is enforced rather than merely written down:
//!
//! - **Forbidden** — substrate-proprietary kinds. They work on the wire but no standard
//!   NIP-29 client reads them, which would quietly destroy the portability the product
//!   claims. Emitting one is an error, not a warning.
//! - **Reserved** — ranges the substrate has claimed for its own use. Emitting into one
//!   risks colliding with the host. Error.
//! - **Ephemeral** — `20000`–`29999` is never stored by the relay. Anything auditable
//!   landing there is unrecoverable, so AD-9 forbids it. Error.
//! - **Unrecognized** — a kind we neither bless nor forbid. Warning: it may be fine, but
//!   AD-8 requires a written rationale before waggle claims a kind, and nothing should
//!   appear in output without someone having decided.

use serde::Serialize;

/// Kinds waggle deliberately uses. Standard-first, per AD-8.
pub const ALLOWED_KINDS: [(u32, &str); 8] = [
    (0, "profile metadata (NIP-01)"),
    (7, "reaction — the gate trigger (NIP-25)"),
    (
        9,
        "group chat message — artifacts, handoffs, gate records (NIP-29)",
    ),
    (1617, "git patch (NIP-34)"),
    (1621, "git issue (NIP-34)"),
    (1630, "status: open (NIP-34)"),
    (1631, "status: applied/merged (NIP-34)"),
    (1632, "status: closed (NIP-34)"),
];

/// Substrate-proprietary kinds. Work on the wire, unreadable by standard clients.
pub const FORBIDDEN_KINDS: [(u32, &str); 2] = [
    (
        40002,
        "Buzz-only rich content — no standard NIP-29 client reads it",
    ),
    (
        40003,
        "Buzz-only edits — no standard NIP-29 client reads it",
    ),
];

/// Ranges the substrate reserves for itself.
pub const RESERVED_RANGES: [(u32, u32, &str); 3] = [
    (43001, 43006, "substrate job dispatch"),
    (46001, 46012, "substrate workflows"),
    (48001, 48001, "substrate audit chain"),
];

/// Ephemeral range — never persisted by the relay (AD-9).
pub const EPHEMERAL_RANGE: (u32, u32) = (20000, 29999);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// Compilation must fail.
    Error,
    /// Reported; compilation proceeds.
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Finding {
    pub severity: Severity,
    /// Artifact the finding came from, e.g. a file name.
    pub artifact: String,
    pub kind: u32,
    pub reason: String,
}

/// Classify a single kind against the policy.
pub fn classify(artifact: &str, kind: u32) -> Option<Finding> {
    if let Some((_, why)) = FORBIDDEN_KINDS.iter().find(|(k, _)| *k == kind) {
        return Some(Finding {
            severity: Severity::Error,
            artifact: artifact.to_string(),
            kind,
            reason: format!("kind {kind} is forbidden: {why}"),
        });
    }

    if let Some((lo, hi, what)) = RESERVED_RANGES
        .iter()
        .find(|(lo, hi, _)| kind >= *lo && kind <= *hi)
    {
        return Some(Finding {
            severity: Severity::Error,
            artifact: artifact.to_string(),
            kind,
            reason: format!(
                "kind {kind} is inside the substrate-reserved range {lo}–{hi} ({what})"
            ),
        });
    }

    let (elo, ehi) = EPHEMERAL_RANGE;
    if kind >= elo && kind <= ehi {
        return Some(Finding {
            severity: Severity::Error,
            artifact: artifact.to_string(),
            kind,
            reason: format!(
                "kind {kind} is ephemeral ({elo}–{ehi}); the relay never stores it, so nothing \
                 auditable may use it (AD-9)"
            ),
        });
    }

    if ALLOWED_KINDS.iter().any(|(k, _)| *k == kind) {
        return None;
    }

    Some(Finding {
        severity: Severity::Warning,
        artifact: artifact.to_string(),
        kind,
        reason: format!(
            "kind {kind} is not on waggle's allowed list; AD-8 requires a written rationale in \
             the kind registry before claiming a kind"
        ),
    })
}

/// Scan artifact text for explicit kind references and classify each.
///
/// Recognizes `kind: N`, `kind:N`, and `"kind": N` — the forms our generated YAML and JSON
/// actually use. This is deliberately narrow: a looser scan would flag every integer in a
/// canvas template and train people to ignore the lint.
pub fn scan(artifact: &str, text: &str) -> Vec<Finding> {
    let mut out = Vec::new();
    let bytes = text.as_bytes();
    let mut i = 0usize;

    while let Some(pos) = text[i..].find("kind") {
        let start = i + pos;
        let mut j = start + 4;

        // optional closing quote from `"kind"`
        if bytes.get(j) == Some(&b'"') {
            j += 1;
        }
        // require a colon
        if bytes.get(j) != Some(&b':') {
            i = start + 4;
            continue;
        }
        j += 1;
        while bytes.get(j) == Some(&b' ') {
            j += 1;
        }

        let ds = j;
        while j < bytes.len() && bytes[j].is_ascii_digit() {
            j += 1;
        }
        if j > ds {
            if let Ok(kind) = text[ds..j].parse::<u32>() {
                if let Some(f) = classify(artifact, kind) {
                    out.push(f);
                }
            }
        }
        i = start + 4;
    }

    out
}

/// Whether any finding is fatal.
pub fn has_errors(findings: &[Finding]) -> bool {
    findings.iter().any(|f| f.severity == Severity::Error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allowed_kinds_produce_nothing() {
        for (k, _) in ALLOWED_KINDS {
            assert_eq!(classify("a", k), None, "kind {k} should be allowed");
        }
    }

    #[test]
    fn substrate_proprietary_kinds_are_errors() {
        for (k, _) in FORBIDDEN_KINDS {
            let f = classify("a", k).expect("must be flagged");
            assert_eq!(f.severity, Severity::Error);
            assert!(f.reason.contains("forbidden"), "{}", f.reason);
        }
    }

    #[test]
    fn reserved_ranges_are_errors_including_their_boundaries() {
        for k in [43001, 43006, 46001, 46012, 48001] {
            let f = classify("a", k).unwrap_or_else(|| panic!("kind {k} should be flagged"));
            assert_eq!(f.severity, Severity::Error, "kind {k}");
        }
        // just outside the reserved ranges: not an error, but unrecognized
        for k in [43000, 43007, 46000, 46013] {
            let f = classify("a", k).unwrap();
            assert_eq!(f.severity, Severity::Warning, "kind {k}");
        }
    }

    #[test]
    fn ephemeral_kinds_are_errors_because_nothing_stores_them() {
        for k in [20000, 20001, 25000, 29999] {
            let f = classify("a", k).unwrap();
            assert_eq!(f.severity, Severity::Error);
            assert!(f.reason.contains("ephemeral"), "{}", f.reason);
        }
        assert_eq!(classify("a", 19999).unwrap().severity, Severity::Warning);
        assert_eq!(classify("a", 30000).unwrap().severity, Severity::Warning);
    }

    #[test]
    fn unknown_kinds_warn_rather_than_pass_silently() {
        let f = classify("a", 31337).unwrap();
        assert_eq!(f.severity, Severity::Warning);
        assert!(f.reason.contains("rationale"));
    }

    #[test]
    fn scan_recognizes_the_forms_our_output_uses() {
        let findings = scan("gen.yaml", "kind: 40002\nkind:9\n\"kind\": 46001\n");
        let kinds: Vec<_> = findings.iter().map(|f| f.kind).collect();
        // 9 is allowed and must not appear
        assert_eq!(kinds, vec![40002, 46001], "got {findings:?}");
        assert!(has_errors(&findings));
    }

    #[test]
    fn scan_ignores_numbers_that_are_not_kinds() {
        // A canvas full of prose and tables must not trip the lint, or nobody will read it.
        let text = "# Test strategy\n\n| Area | 40002 | P1 |\nsome kindly text 46001\n";
        assert!(
            scan("canvas.md", text).is_empty(),
            "{:?}",
            scan("canvas.md", text)
        );
    }

    #[test]
    fn clean_output_yields_no_findings() {
        let yaml = "trigger:\n  on: reaction_added\nsteps:\n  - action: send_message\n";
        assert!(scan("gate.yaml", yaml).is_empty());
        assert!(!has_errors(&[]));
    }
}
