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

/// Marker for the relay-signed advisory notice the workflow posts.
///
/// Deliberately distinct from [`GATE_RECORD_MARKER`]: a reader must be able to tell a
/// relay-signed advisory apart from the agent-signed record at a glance, because only one
/// of them carries trustworthy attribution (UP-18).
pub const GATE_NOTICE_MARKER: &str = "waggle-gate-notice";

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

// ---------------------------------------------------------------------------
// Reconciliation (UP-18)
// ---------------------------------------------------------------------------
//
// The relay's workflow engine signs its output with the RELAY keypair and resolves
// `{{trigger.author}}` from an `actor` tag with no relay-pubkey guard. A record produced
// that way attests only that the relay said so, and names an approver anyone can forge.
//
// So waggle does not treat the workflow's output as the record. It reads the reaction
// events itself and derives the approver from the one field a signature actually binds:
// the event's `pubkey`. Everything below is pure; fetching is `waggle-hive`'s job.

/// A reaction, reduced to what a signature actually guarantees.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedReaction {
    pub event_id: String,
    /// Author, taken from the event's `pubkey` field — **never** from an `actor` tag.
    pub author_pubkey: String,
    /// Emoji content of the kind:7 event.
    pub emoji: String,
    /// Event this reacts to.
    pub target_event: String,
    pub created_at: u64,
}

/// One entry of the relay-signed admin list (kind 39001).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RosterEntry {
    pub pubkey: String,
    pub role: Role,
}

/// The outcome of reconciling one verdict against the reactions that reference it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum GateOutcome {
    /// An authorized identity approved it.
    Approved {
        verdict: Verdict,
        approver: String,
        approved_at: u64,
        reaction_event: String,
    },
    /// Reactions exist, but none from an identity permitted to approve this verdict.
    Unauthorized {
        verdict: Verdict,
        /// Who reacted anyway — recorded, never silently dropped (AD-6).
        attempted_by: Vec<String>,
        required: Role,
    },
    /// No approving reaction yet.
    Pending { verdict: Verdict },
}

/// Reconcile a verdict against reactions and the roster.
///
/// `roster` comes from the relay-signed admin list, so authorization is decided by the
/// relay's own record of membership rather than anything waggle maintains (AD-13).
///
/// The earliest authorized reaction wins: a gate is decided once, and a later approver
/// cannot overwrite the record of who actually decided it.
pub fn reconcile(
    verdict: Verdict,
    reactions: &[SignedReaction],
    roster: &[RosterEntry],
) -> GateOutcome {
    let required = verdict.required_role();

    let mut approving: Vec<&SignedReaction> = reactions
        .iter()
        .filter(|r| r.emoji == APPROVAL_EMOJI)
        .collect();
    // Deterministic: earliest first, event id breaking ties.
    approving.sort_by(|a, b| {
        a.created_at
            .cmp(&b.created_at)
            .then_with(|| a.event_id.cmp(&b.event_id))
    });

    if approving.is_empty() {
        return GateOutcome::Pending { verdict };
    }

    for r in &approving {
        let role = roster
            .iter()
            .find(|e| e.pubkey == r.author_pubkey)
            .map(|e| e.role)
            .unwrap_or(Role::Member);

        if authorize(verdict, role).is_ok() {
            return GateOutcome::Approved {
                verdict,
                approver: r.author_pubkey.clone(),
                approved_at: r.created_at,
                reaction_event: r.event_id.clone(),
            };
        }
    }

    GateOutcome::Unauthorized {
        verdict,
        attempted_by: approving.iter().map(|r| r.author_pubkey.clone()).collect(),
        required,
    }
}

/// Render the gate record body waggle publishes under its OWN identity.
///
/// Every field here is derived from a signed event, so the record's own signature makes
/// the whole chain checkable: waggle asserts it, and the reaction it cites can be fetched
/// and verified independently.
pub fn render_gate_record(outcome: &GateOutcome, verdict_event: &str) -> Option<String> {
    match outcome {
        GateOutcome::Approved {
            verdict,
            approver,
            approved_at,
            reaction_event,
        } => Some(format!(
            "{GATE_RECORD_MARKER}\n\
             verdict: {verdict}\n\
             verdict-event: {verdict_event}\n\
             approver: {approver}\n\
             approved-at: {approved_at}\n\
             reaction-event: {reaction_event}\n\
             approver-source: reaction event pubkey (signature-bound)"
        )),
        _ => None,
    }
}

#[cfg(test)]
mod reconcile_tests {
    use super::*;

    fn reaction(author: &str, at: u64, emoji: &str) -> SignedReaction {
        SignedReaction {
            event_id: format!("r-{author}-{at}"),
            author_pubkey: author.into(),
            emoji: emoji.into(),
            target_event: "v1".into(),
            created_at: at,
        }
    }

    fn roster() -> Vec<RosterEntry> {
        vec![
            RosterEntry {
                pubkey: "owner1".into(),
                role: Role::Owner,
            },
            RosterEntry {
                pubkey: "admin1".into(),
                role: Role::Admin,
            },
        ]
    }

    #[test]
    fn an_admin_can_approve_an_ordinary_verdict() {
        let out = reconcile(
            Verdict::Pass,
            &[reaction("admin1", 100, APPROVAL_EMOJI)],
            &roster(),
        );
        assert_eq!(
            out,
            GateOutcome::Approved {
                verdict: Verdict::Pass,
                approver: "admin1".into(),
                approved_at: 100,
                reaction_event: "r-admin1-100".into(),
            }
        );
    }

    #[test]
    fn a_stranger_cannot_approve_and_is_recorded_not_dropped() {
        // The UP-18 case: someone not on the relay-signed roster reacts.
        let out = reconcile(
            Verdict::Pass,
            &[reaction("stranger", 100, APPROVAL_EMOJI)],
            &roster(),
        );
        match out {
            GateOutcome::Unauthorized {
                attempted_by,
                required,
                ..
            } => {
                assert_eq!(attempted_by, vec!["stranger"]);
                assert_eq!(required, Role::Admin);
            }
            other => panic!("a non-member must not approve, got {other:?}"),
        }
    }

    #[test]
    fn waiving_requires_an_owner_even_from_an_admin() {
        let out = reconcile(
            Verdict::Waived,
            &[reaction("admin1", 100, APPROVAL_EMOJI)],
            &roster(),
        );
        assert!(matches!(out, GateOutcome::Unauthorized { .. }));

        let out = reconcile(
            Verdict::Waived,
            &[reaction("owner1", 100, APPROVAL_EMOJI)],
            &roster(),
        );
        assert!(matches!(out, GateOutcome::Approved { .. }));
    }

    #[test]
    fn the_earliest_authorized_reaction_decides_the_gate() {
        // A later approver must not overwrite who actually decided.
        let out = reconcile(
            Verdict::Pass,
            &[
                reaction("owner1", 200, APPROVAL_EMOJI),
                reaction("admin1", 100, APPROVAL_EMOJI),
            ],
            &roster(),
        );
        match out {
            GateOutcome::Approved { approver, .. } => assert_eq!(approver, "admin1"),
            other => panic!("expected approval, got {other:?}"),
        }
    }

    #[test]
    fn an_unauthorized_reaction_does_not_block_a_later_authorized_one() {
        let out = reconcile(
            Verdict::Pass,
            &[
                reaction("stranger", 50, APPROVAL_EMOJI),
                reaction("admin1", 100, APPROVAL_EMOJI),
            ],
            &roster(),
        );
        match out {
            GateOutcome::Approved { approver, .. } => assert_eq!(approver, "admin1"),
            other => panic!("expected approval, got {other:?}"),
        }
    }

    #[test]
    fn other_emoji_do_not_approve() {
        let out = reconcile(
            Verdict::Pass,
            &[reaction("owner1", 100, "thumbsup")],
            &roster(),
        );
        assert_eq!(
            out,
            GateOutcome::Pending {
                verdict: Verdict::Pass
            }
        );
    }

    #[test]
    fn no_reactions_means_pending_not_approved() {
        assert_eq!(
            reconcile(Verdict::Fail, &[], &roster()),
            GateOutcome::Pending {
                verdict: Verdict::Fail
            }
        );
    }

    #[test]
    fn the_record_names_where_the_approver_came_from() {
        // The whole point of UP-18's fix: the record must be explicit that the approver
        // is signature-bound, not lifted from a spoofable tag.
        let out = reconcile(
            Verdict::Concerns,
            &[reaction("admin1", 100, APPROVAL_EMOJI)],
            &roster(),
        );
        let body = render_gate_record(&out, "v1").expect("approved gates render a record");
        assert!(body.contains("verdict: CONCERNS"));
        assert!(body.contains("approver: admin1"));
        assert!(body.contains("reaction-event: r-admin1-100"));
        assert!(body.contains("signature-bound"), "{body}");
    }

    #[test]
    fn unapproved_gates_render_no_record_at_all() {
        assert!(render_gate_record(
            &GateOutcome::Pending {
                verdict: Verdict::Pass
            },
            "v1"
        )
        .is_none());
        assert!(render_gate_record(
            &GateOutcome::Unauthorized {
                verdict: Verdict::Pass,
                attempted_by: vec!["x".into()],
                required: Role::Admin
            },
            "v1"
        )
        .is_none());
    }
}
