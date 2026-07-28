//! Quality gates (FR-19 … FR-23).
//!
//! Pure: this module decides *what* a gate is. Talking to the substrate is `waggle-hive`'s
//! job, and exactly one crate may do it (AD-10).
//!
//! **AD-10: the event log is the authority on gate state.** Upstream currently marks a
//! workflow run that reaches an approval step as *failed* rather than suspended (UP-01), so
//! run status cannot be trusted. The gate record is therefore a signed event published into
//! the channel, and gate state is derived by pairing verdicts with the approval reactions
//! that reference them. If upstream never fixes UP-01, waggle is still correct — FR-22
//! already required log-only reconstruction, so this is that requirement's implementation
//! rather than a workaround bolted beside it.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// A gate decision. Closed set — the vocabulary comes from the method's own trace
/// workflow, and anything outside it is rejected at publish time (AD-12).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Verdict {
    Pass,
    Concerns,
    Fail,
    Waived,
}

impl Verdict {
    pub const ALL: [Verdict; 4] = [
        Verdict::Pass,
        Verdict::Concerns,
        Verdict::Fail,
        Verdict::Waived,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Verdict::Pass => "PASS",
            Verdict::Concerns => "CONCERNS",
            Verdict::Fail => "FAIL",
            Verdict::Waived => "WAIVED",
        }
    }

    /// Whether a bare verdict is acceptable, or a rationale is mandatory.
    ///
    /// `CONCERNS` and `WAIVED` are the two that let work proceed despite known problems,
    /// so an unexplained one is exactly the record that is useless six months later.
    pub const fn requires_rationale(self) -> bool {
        matches!(self, Verdict::Concerns | Verdict::Waived)
    }

    /// Whether work may advance past this gate once approved.
    pub const fn advances(self) -> bool {
        matches!(self, Verdict::Pass | Verdict::Concerns | Verdict::Waived)
    }

    /// Minimum authorization required to approve this verdict (AD-13).
    ///
    /// `WAIVED` needs an owner: waiving is the one action that discards a failing signal
    /// entirely.
    pub const fn required_role(self) -> Role {
        match self {
            Verdict::Waived => Role::Owner,
            _ => Role::Admin,
        }
    }
}

impl fmt::Display for Verdict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Verdict {
    type Err = GateError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Case-sensitive on purpose: the method writes these in uppercase, and accepting
        // "pass" invites a second spelling into the log.
        Verdict::ALL
            .into_iter()
            .find(|v| v.as_str() == s)
            .ok_or_else(|| GateError::UnknownVerdict { got: s.to_string() })
    }
}

/// Authorization level, read from the relay-signed admin list (AD-13).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Member,
    Admin,
    Owner,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum GateError {
    #[error("{got:?} is not a gate verdict — expected one of PASS, CONCERNS, FAIL, WAIVED")]
    UnknownVerdict { got: String },

    #[error(
        "a {verdict} verdict requires a rationale — a bare {verdict} is not an auditable record"
    )]
    RationaleRequired { verdict: Verdict },

    #[error("{role:?} cannot approve a {verdict} verdict; {required:?} or higher is required")]
    Unauthorized {
        verdict: Verdict,
        role: Role,
        required: Role,
    },
}

/// A verdict about to be published.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GateVerdict {
    pub verdict: Verdict,
    /// Required for `CONCERNS` and `WAIVED`.
    pub rationale: Option<String>,
    /// Risk priority of the artifact being gated (`P0`–`P3`).
    pub priority: Option<String>,
    /// Event id of the artifact this verdict applies to.
    pub artifact_event: Option<String>,
}

impl GateVerdict {
    /// Validate before publishing (AD-12). Rejecting here rather than at read time means
    /// an unusable record never enters the log in the first place.
    pub fn validate(&self) -> Result<(), GateError> {
        if self.verdict.requires_rationale()
            && self
                .rationale
                .as_ref()
                .map(|r| r.trim().is_empty())
                .unwrap_or(true)
        {
            return Err(GateError::RationaleRequired {
                verdict: self.verdict,
            });
        }
        Ok(())
    }
}

/// Decide whether a reaction from `role` may approve `verdict` (AD-13).
pub fn authorize(verdict: Verdict, role: Role) -> Result<(), GateError> {
    let required = verdict.required_role();
    if role >= required {
        Ok(())
    } else {
        Err(GateError::Unauthorized {
            verdict,
            role,
            required,
        })
    }
}

/// The emoji whose reaction fires a gate.
///
/// One value, in one place: the compiled workflow's trigger and the reconciler that reads
/// the log must agree, or approvals silently never register.
pub const APPROVAL_EMOJI: &str = "white_check_mark";

/// Marker line opening a gate record event, so records are findable in the log without a
/// custom event kind (AD-8: standard kinds first).
pub const GATE_RECORD_MARKER: &str = "waggle-gate-record";

/// Marker line opening a verdict event.
pub const VERDICT_MARKER: &str = "waggle-gate-verdict";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_vocabulary_is_closed() {
        for v in Verdict::ALL {
            assert_eq!(v.as_str().parse::<Verdict>().unwrap(), v);
        }
        for bad in ["pass", "Pass", "APPROVED", "OK", "", "PASSED"] {
            assert!(
                bad.parse::<Verdict>().is_err(),
                "{bad:?} must not parse as a verdict"
            );
        }
    }

    #[test]
    fn concerns_and_waived_require_a_rationale() {
        for v in [Verdict::Concerns, Verdict::Waived] {
            let bare = GateVerdict {
                verdict: v,
                rationale: None,
                priority: None,
                artifact_event: None,
            };
            assert_eq!(
                bare.validate().unwrap_err(),
                GateError::RationaleRequired { verdict: v }
            );

            // whitespace is not a rationale
            let blank = GateVerdict {
                rationale: Some("   ".into()),
                ..bare.clone()
            };
            assert!(blank.validate().is_err());

            let good = GateVerdict {
                rationale: Some("P1 coverage gap in the auth path".into()),
                ..bare
            };
            assert!(good.validate().is_ok());
        }
    }

    #[test]
    fn pass_and_fail_need_no_rationale() {
        for v in [Verdict::Pass, Verdict::Fail] {
            let g = GateVerdict {
                verdict: v,
                rationale: None,
                priority: None,
                artifact_event: None,
            };
            assert!(g.validate().is_ok());
        }
    }

    #[test]
    fn waiving_requires_an_owner() {
        assert!(authorize(Verdict::Waived, Role::Owner).is_ok());
        assert!(authorize(Verdict::Waived, Role::Admin).is_err());
        assert!(authorize(Verdict::Waived, Role::Member).is_err());
    }

    #[test]
    fn ordinary_verdicts_need_admin_and_members_cannot_approve() {
        for v in [Verdict::Pass, Verdict::Concerns, Verdict::Fail] {
            assert!(authorize(v, Role::Admin).is_ok());
            assert!(authorize(v, Role::Owner).is_ok());
            assert!(
                authorize(v, Role::Member).is_err(),
                "a plain member must not advance a {v} gate"
            );
        }
    }

    #[test]
    fn fail_does_not_advance_work() {
        assert!(!Verdict::Fail.advances());
        assert!(Verdict::Pass.advances());
        // CONCERNS and WAIVED advance, which is exactly why they need a rationale.
        assert!(Verdict::Concerns.advances());
        assert!(Verdict::Waived.advances());
    }
}
