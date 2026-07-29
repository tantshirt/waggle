//! Artifact, handoff, and priority tagging (FR-15, FR-17, FR-24).
//!
//! Pure: builds the tag set and validates it. Signing and publishing are `waggle-hive`'s.
//!
//! **AD-8: standard kinds first.** Everything here rides kind `9` — the standard NIP-29
//! group message — with typed tags, rather than claiming a custom kind. A third-party
//! NIP-29 client renders these as ordinary messages, which is the portability NFR-6 asks
//! for.
//!
//! **The single-letter rule is why the tags look the way they do.** NIP-01 only indexes
//! single-letter tags for `#<letter>` filter queries; a `waggle-priority` tag would be
//! stored but *not* queryable, so FR-24's "filter the log by priority" would quietly
//! degrade to fetching everything and filtering client-side. Priorities and types
//! therefore ride `t` tags, and references ride `e`.

use serde::Serialize;

/// Marker `t` tag present on every waggle-published event, so the whole trail is one query.
pub const TAG_WAGGLE: &str = "waggle";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ArtifactKind {
    /// A method artifact: brief, PRD, architecture, story, test design.
    Artifact,
    /// Transfer of work between two method roles.
    Handoff,
    /// A gate decision awaiting approval.
    Verdict,
    /// The signed record of an approval.
    GateRecord,
}

impl ArtifactKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            ArtifactKind::Artifact => "artifact",
            ArtifactKind::Handoff => "handoff",
            ArtifactKind::Verdict => "verdict",
            ArtifactKind::GateRecord => "gate-record",
        }
    }
}

/// `P0`–`P3`. Any other value is rejected rather than stored (FR-24).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub enum Priority {
    P0,
    P1,
    P2,
    P3,
}

impl Priority {
    pub const ALL: [Priority; 4] = [Priority::P0, Priority::P1, Priority::P2, Priority::P3];

    pub const fn as_str(self) -> &'static str {
        match self {
            Priority::P0 => "P0",
            Priority::P1 => "P1",
            Priority::P2 => "P2",
            Priority::P3 => "P3",
        }
    }

    /// Lowercase form used in `t` tags. Nostr tag matching is case-sensitive, so one
    /// casing must be chosen and used everywhere; lowercase matches hashtag convention.
    pub const fn tag_value(self) -> &'static str {
        match self {
            Priority::P0 => "p0",
            Priority::P1 => "p1",
            Priority::P2 => "p2",
            Priority::P3 => "p3",
        }
    }

    pub fn parse(s: &str) -> Result<Self, ArtifactError> {
        Priority::ALL
            .into_iter()
            .find(|p| p.as_str().eq_ignore_ascii_case(s))
            .ok_or_else(|| ArtifactError::BadPriority { got: s.to_string() })
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ArtifactError {
    #[error("{got:?} is not a risk priority — expected one of P0, P1, P2, P3")]
    BadPriority { got: String },

    #[error("a handoff must name the artifact event it transfers")]
    HandoffWithoutArtifact,

    #[error("a handoff must name both the originating and receiving role")]
    HandoffWithoutRoles,

    #[error("channel id is required — every artifact belongs to a story channel")]
    MissingChannel,
}

/// An event waggle is about to publish.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ArtifactEvent {
    pub kind_marker: ArtifactKind,
    pub channel_id: String,
    /// Method artifact type, e.g. `prd`, `story`, `test-design`.
    pub artifact_type: Option<String>,
    pub module: Option<String>,
    pub story: Option<String>,
    pub priority: Option<Priority>,
    /// Events this one references — the transferred artifact, the gated verdict.
    pub references: Vec<String>,
    pub from_role: Option<String>,
    pub to_role: Option<String>,
    pub body: String,
}

impl ArtifactEvent {
    /// Validate before publishing. Rejecting here keeps unusable records out of the log.
    pub fn validate(&self) -> Result<(), ArtifactError> {
        if self.channel_id.trim().is_empty() {
            return Err(ArtifactError::MissingChannel);
        }
        if self.kind_marker == ArtifactKind::Handoff {
            if self.references.is_empty() {
                return Err(ArtifactError::HandoffWithoutArtifact);
            }
            if self.from_role.is_none() || self.to_role.is_none() {
                return Err(ArtifactError::HandoffWithoutRoles);
            }
        }
        Ok(())
    }

    /// The Nostr tag set, in a deterministic order (NFR-1).
    ///
    /// Single-letter tags (`h`, `t`, `e`) are the queryable ones; the rest are descriptive
    /// and readable but not indexed.
    pub fn tags(&self) -> Vec<Vec<String>> {
        let mut tags: Vec<Vec<String>> = Vec::new();

        // Channel scoping — required by the relay for kind:9.
        tags.push(vec!["h".into(), self.channel_id.clone()]);

        // Queryable classification.
        tags.push(vec!["t".into(), TAG_WAGGLE.into()]);
        tags.push(vec!["t".into(), self.kind_marker.as_str().into()]);
        if let Some(p) = self.priority {
            tags.push(vec!["t".into(), p.tag_value().into()]);
        }
        if let Some(m) = &self.module {
            tags.push(vec!["t".into(), format!("module-{m}")]);
        }

        // References.
        for r in &self.references {
            tags.push(vec!["e".into(), r.clone()]);
        }

        // Descriptive, not indexed.
        if let Some(a) = &self.artifact_type {
            tags.push(vec!["waggle-artifact".into(), a.clone()]);
        }
        if let Some(s) = &self.story {
            tags.push(vec!["waggle-story".into(), s.clone()]);
        }
        if let Some(f) = &self.from_role {
            tags.push(vec!["waggle-from".into(), f.clone()]);
        }
        if let Some(t) = &self.to_role {
            tags.push(vec!["waggle-to".into(), t.clone()]);
        }

        tags
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> ArtifactEvent {
        ArtifactEvent {
            kind_marker: ArtifactKind::Artifact,
            channel_id: "chan-1".into(),
            artifact_type: Some("prd".into()),
            module: Some("bmm".into()),
            story: Some("1.8".into()),
            priority: Some(Priority::P1),
            references: vec![],
            from_role: None,
            to_role: None,
            body: "body".into(),
        }
    }

    fn tag_values<'a>(tags: &'a [Vec<String>], name: &str) -> Vec<&'a str> {
        tags.iter()
            .filter(|t| t[0] == name)
            .map(|t| t[1].as_str())
            .collect()
    }

    #[test]
    fn priority_parsing_is_closed_and_case_insensitive() {
        assert_eq!(Priority::parse("P1").unwrap(), Priority::P1);
        assert_eq!(Priority::parse("p1").unwrap(), Priority::P1);
        for bad in ["P4", "high", "", "1", "PP1"] {
            assert!(Priority::parse(bad).is_err(), "{bad:?} must be rejected");
        }
    }

    #[test]
    fn queryable_facets_use_single_letter_tags() {
        // The whole point: NIP-01 only indexes single-letter tags, so anything we expect
        // to filter on must live in `t` or `e`, never in `waggle-*`.
        let tags = base().tags();
        let t = tag_values(&tags, "t");
        assert!(t.contains(&"waggle"), "{t:?}");
        assert!(t.contains(&"artifact"), "{t:?}");
        assert!(t.contains(&"p1"), "priority must be queryable: {t:?}");
        assert!(t.contains(&"module-bmm"), "{t:?}");
        assert_eq!(tag_values(&tags, "h"), vec!["chan-1"]);
    }

    #[test]
    fn descriptive_facets_do_not_masquerade_as_queryable() {
        let tags = base().tags();
        assert_eq!(tag_values(&tags, "waggle-artifact"), vec!["prd"]);
        assert_eq!(tag_values(&tags, "waggle-story"), vec!["1.8"]);
        // and they are not duplicated into `t`, which would double-index noise
        assert!(!tag_values(&tags, "t").contains(&"prd"));
    }

    #[test]
    fn absent_priority_emits_no_priority_tag() {
        let mut e = base();
        e.priority = None;
        let tags = e.tags();
        let t = tag_values(&tags, "t");
        assert!(
            !t.iter().any(|v| v.starts_with('p') && v.len() == 2),
            "{t:?}"
        );
    }

    #[test]
    fn a_handoff_must_name_its_artifact_and_both_roles() {
        let mut e = base();
        e.kind_marker = ArtifactKind::Handoff;
        assert_eq!(
            e.validate().unwrap_err(),
            ArtifactError::HandoffWithoutArtifact
        );

        e.references = vec!["abc".into()];
        assert_eq!(
            e.validate().unwrap_err(),
            ArtifactError::HandoffWithoutRoles
        );

        e.from_role = Some("sm".into());
        e.to_role = Some("dev".into());
        assert!(e.validate().is_ok());

        let tags = e.tags();
        assert_eq!(tag_values(&tags, "e"), vec!["abc"]);
        assert_eq!(tag_values(&tags, "waggle-from"), vec!["sm"]);
        assert_eq!(tag_values(&tags, "waggle-to"), vec!["dev"]);
    }

    #[test]
    fn an_artifact_without_a_channel_is_rejected() {
        let mut e = base();
        e.channel_id = "  ".into();
        assert_eq!(e.validate().unwrap_err(), ArtifactError::MissingChannel);
    }

    #[test]
    fn tag_order_is_deterministic() {
        assert_eq!(base().tags(), base().tags());
    }

    #[test]
    fn every_waggle_event_carries_the_marker_so_the_trail_is_one_query() {
        for k in [
            ArtifactKind::Artifact,
            ArtifactKind::Handoff,
            ArtifactKind::Verdict,
            ArtifactKind::GateRecord,
        ] {
            let mut e = base();
            e.kind_marker = k;
            let tags = e.tags();
            assert!(tag_values(&tags, "t").contains(&TAG_WAGGLE));
        }
    }
}
