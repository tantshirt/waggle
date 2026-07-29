//! Signing and publishing events directly to the relay (FR-15, FR-17, FR-22, FR-24).
//!
//! **This is UP-07's consequence made concrete.** `buzz-cli` strips signatures from every
//! read and offers no way to attach typed tags on write, so anything that must carry
//! queryable tags — or be independently verified — cannot go through it. `waggle-hive`
//! therefore speaks the relay's own HTTP surface: NIP-98-authenticated `POST /events` to
//! publish, `POST /query` to read.
//!
//! **AD-2 still holds.** These are published substrate interfaces, documented in the
//! relay's own contributor guide. Nothing here modifies the substrate.
//!
//! **AD-14 still holds.** Secret key material is confined to this module: it is loaded,
//! used to sign, and never returned, logged, or formatted.

use std::path::Path;

use base64::Engine as _;
use nostr::util::JsonUtil;
use nostr::{EventBuilder, Keys, Kind, Tag};
use sha2::{Digest, Sha256};

use waggle_core::ArtifactEvent;

/// Standard NIP-29 group message. AD-8: standard kinds first.
const KIND_GROUP_MESSAGE: u16 = 9;

#[derive(Debug, thiserror::Error)]
pub enum EventError {
    #[error("no identity for role {role:?} — provision it first: waggle identity provision --role {role}")]
    NotProvisioned { role: String },

    #[error("identity for role {role:?} is malformed: {reason}")]
    BadIdentity { role: String, reason: String },

    #[error("could not build the event: {0}")]
    Build(String),

    #[error("relay at {url} rejected the event: HTTP {status} {body}")]
    Rejected {
        url: String,
        status: u16,
        body: String,
    },

    #[error("could not reach the relay at {url}: {source}")]
    Unreachable {
        url: String,
        #[source]
        source: reqwest::Error,
    },

    #[error("relay response was not the JSON we expected: {0}")]
    Unparseable(String),

    #[error("artifact is invalid: {0}")]
    Invalid(#[from] waggle_core::artifact::ArtifactError),

    #[error("artifact body is {bytes} bytes, over the relay's {limit}-byte content limit — the substrate's media store accepts images only, so it cannot be carried by reference (UP-16); split the artifact or shorten it")]
    TooLarge { bytes: usize, limit: usize },
}

/// Load a role's keys. Private: the secret never leaves this module (AD-14).
fn load_keys(project_root: &Path, role: &str) -> Result<Keys, EventError> {
    let path = project_root.join("keys").join(format!("{role}.nsec"));
    if !path.exists() {
        return Err(EventError::NotProvisioned {
            role: role.to_string(),
        });
    }
    let hex = std::fs::read_to_string(&path)
        .map_err(|e| EventError::BadIdentity {
            role: role.to_string(),
            reason: e.to_string(),
        })?
        .trim()
        .to_string();

    Keys::parse(&hex).map_err(|e| EventError::BadIdentity {
        role: role.to_string(),
        // Deliberately not echoing the key material into the error.
        reason: format!("not a valid secret key ({e})"),
    })
}

/// Build the `Authorization: Nostr <base64>` header for a NIP-98 request.
///
/// Tags follow the relay's own implementation: `u` (url), `method`, `nonce`, and
/// `payload` (sha256 of the body). The nonce is what allows two identical requests in
/// quick succession without one being treated as a replay.
fn nip98_header(
    keys: &Keys,
    method: &str,
    url: &str,
    body: &[u8],
    nonce: &str,
) -> Result<String, EventError> {
    let payload = hex_encode(&Sha256::digest(body));
    let tags = [
        Tag::parse(["u", url]),
        Tag::parse(["method", method]),
        Tag::parse(["nonce", nonce]),
        Tag::parse(["payload", &payload]),
    ];
    let mut built = Vec::new();
    for t in tags {
        built.push(t.map_err(|e| EventError::Build(e.to_string()))?);
    }

    let event = EventBuilder::new(Kind::Custom(27235), "")
        .tags(built)
        .sign_with_keys(keys)
        .map_err(|e| EventError::Build(e.to_string()))?;

    Ok(format!(
        "Nostr {}",
        base64::engine::general_purpose::STANDARD.encode(event.as_json().as_bytes())
    ))
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Size limits, discovered from the relay rather than hard-coded (AD-15).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// NIP-11 `max_message_length` — the whole event JSON.
    pub max_message: usize,
    /// Maximum `content` length the relay enforces.
    ///
    /// **Not advertised anywhere.** NIP-11 reports `max_message_length` (524288 on
    /// `v0.4.26`) but the relay separately rejects content over 262144 with
    /// `content exceeds maximum size of 262144`. The two disagree and only the larger is
    /// discoverable, so this is derived as half the advertised message length — which
    /// matches the observed value — and re-checked against the relay's own rejection.
    /// Logged as UP-15.
    pub max_content: usize,
}

impl Default for Limits {
    fn default() -> Self {
        // Values observed on v0.4.26, used only when NIP-11 is unavailable.
        Limits {
            max_message: 524_288,
            max_content: 262_144,
        }
    }
}

/// Read the relay's advertised limits (NIP-11). Falls back to observed defaults.
pub fn discover_limits(relay_url: &str) -> Limits {
    let client = reqwest::blocking::Client::new();
    let resp = client
        .get(relay_url.trim_end_matches('/'))
        .header("Accept", "application/nostr+json")
        .send();

    let Ok(resp) = resp else {
        return Limits::default();
    };
    let Ok(doc) = resp.json::<serde_json::Value>() else {
        return Limits::default();
    };

    let max_message = doc
        .get("limitation")
        .and_then(|l| l.get("max_message_length"))
        .and_then(serde_json::Value::as_u64)
        .map(|v| v as usize)
        .unwrap_or(Limits::default().max_message);

    Limits {
        max_message,
        max_content: max_message / 2,
    }
}

/// What the relay accepted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Published {
    pub event_id: String,
    /// The publishing identity, so callers can attribute without touching the secret.
    pub pubkey: String,
}

/// Sign and publish an artifact, handoff, verdict, or gate record.
///
/// The event is a standard kind:9 group message carrying waggle's typed tags, so a
/// third-party NIP-29 client still renders it (NFR-6).
pub fn publish_artifact(
    project_root: &Path,
    role: &str,
    relay_url: &str,
    artifact: &ArtifactEvent,
    nonce: &str,
) -> Result<Published, EventError> {
    publish_artifact_with_limits(project_root, role, relay_url, artifact, nonce, None)
}

/// As [`publish_artifact`], with limits supplied rather than discovered.
pub fn publish_artifact_with_limits(
    project_root: &Path,
    role: &str,
    relay_url: &str,
    artifact: &ArtifactEvent,
    nonce: &str,
    limits: Option<Limits>,
) -> Result<Published, EventError> {
    artifact.validate()?;

    // FR-16 / AD-15: check size *before* publishing, so an oversized artifact produces a
    // specific refusal rather than a relay 400 or, worse, a silent truncation.
    //
    // Reference-carrying is not available: the substrate's Blossom store accepts only
    // image MIME types (image/jpeg|png|gif|webp), so a large markdown artifact cannot be
    // stored there and referenced. Refusing loudly is the honest behaviour until an
    // alternative exists — see UP-16.
    let limits = limits.unwrap_or_else(|| discover_limits(relay_url));
    let len = artifact.body.len();
    if len > limits.max_content {
        return Err(EventError::TooLarge {
            bytes: len,
            limit: limits.max_content,
        });
    }
    let keys = load_keys(project_root, role)?;

    let mut tags = Vec::new();
    for t in artifact.tags() {
        tags.push(Tag::parse(t).map_err(|e| EventError::Build(e.to_string()))?);
    }

    let event = EventBuilder::new(Kind::Custom(KIND_GROUP_MESSAGE), artifact.body.clone())
        .tags(tags)
        .sign_with_keys(&keys)
        .map_err(|e| EventError::Build(e.to_string()))?;

    let event_id = event.id.to_hex();
    let pubkey = event.pubkey.to_hex();
    let body = serde_json::to_vec(&event).map_err(|e| EventError::Build(e.to_string()))?;

    let url = format!("{}/events", relay_url.trim_end_matches('/'));
    let auth = nip98_header(&keys, "POST", &url, &body, nonce)?;

    let client = reqwest::blocking::Client::new();
    let resp = client
        .post(&url)
        .header("Authorization", auth)
        .header("Content-Type", "application/json")
        .body(body)
        .send()
        .map_err(|source| EventError::Unreachable {
            url: url.clone(),
            source,
        })?;

    let status = resp.status();
    let text = resp.text().unwrap_or_default();
    if !status.is_success() {
        return Err(EventError::Rejected {
            url,
            status: status.as_u16(),
            body: text.chars().take(300).collect(),
        });
    }

    Ok(Published { event_id, pubkey })
}

/// Query the log by tag, returning **fully signed** events (FR-22).
///
/// This is the read path `buzz-cli` cannot provide: it strips signatures, so a caller
/// wanting to verify provenance must come here.
pub fn query_by_tag(
    project_root: &Path,
    role: &str,
    relay_url: &str,
    channel_id: &str,
    tag_letter: char,
    tag_value: &str,
    nonce: &str,
) -> Result<Vec<serde_json::Value>, EventError> {
    let keys = load_keys(project_root, role)?;

    // Two relay contract details, both learned from its error responses rather than docs:
    //   1. `kinds` must be explicit, or the query hits the p-gate and returns 403.
    //   2. the body is an ARRAY of filters — a bare filter object yields
    //      "invalid type: map, expected a sequence".
    let filter = serde_json::json!([{
        "kinds": [KIND_GROUP_MESSAGE],
        "#h": [channel_id],
        format!("#{tag_letter}"): [tag_value],
        "limit": 200
    }]);
    let body = serde_json::to_vec(&filter).map_err(|e| EventError::Build(e.to_string()))?;

    let url = format!("{}/query", relay_url.trim_end_matches('/'));
    let auth = nip98_header(&keys, "POST", &url, &body, nonce)?;

    let client = reqwest::blocking::Client::new();
    let resp = client
        .post(&url)
        .header("Authorization", auth)
        .header("Content-Type", "application/json")
        .body(body)
        .send()
        .map_err(|source| EventError::Unreachable {
            url: url.clone(),
            source,
        })?;

    let status = resp.status();
    let text = resp.text().unwrap_or_default();
    if !status.is_success() {
        return Err(EventError::Rejected {
            url,
            status: status.as_u16(),
            body: text.chars().take(300).collect(),
        });
    }

    let parsed: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| EventError::Unparseable(e.to_string()))?;

    Ok(parsed
        .as_array()
        .cloned()
        .or_else(|| parsed.get("events").and_then(|e| e.as_array()).cloned())
        .unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use waggle_core::{ArtifactKind, Priority};

    fn tmp_identity(name: &str) -> (std::path::PathBuf, Keys) {
        let root = std::env::temp_dir().join(format!("waggle-events-{name}"));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("keys")).unwrap();
        let keys = Keys::generate();
        std::fs::write(
            root.join("keys").join("tea.nsec"),
            keys.secret_key().to_secret_hex(),
        )
        .unwrap();
        (root, keys)
    }

    #[test]
    fn nip98_header_carries_the_tags_the_relay_checks() {
        let (_, keys) = tmp_identity("hdr");
        let header = nip98_header(&keys, "POST", "http://x/events", b"{}", "n1").unwrap();
        let b64 = header.strip_prefix("Nostr ").expect("scheme prefix");
        let json = base64::engine::general_purpose::STANDARD
            .decode(b64)
            .unwrap();
        let ev: serde_json::Value = serde_json::from_slice(&json).unwrap();

        assert_eq!(ev["kind"], 27235);
        let tags: Vec<(String, String)> = ev["tags"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| (t[0].as_str().unwrap().into(), t[1].as_str().unwrap().into()))
            .collect();
        let names: Vec<&str> = tags.iter().map(|(n, _)| n.as_str()).collect();
        for required in ["u", "method", "nonce", "payload"] {
            assert!(names.contains(&required), "missing {required} in {names:?}");
        }
        // payload must be the sha256 of the body, or the relay rejects it
        let payload = &tags.iter().find(|(n, _)| n == "payload").unwrap().1;
        assert_eq!(*payload, hex_encode(&Sha256::digest(b"{}")));
        // and the header must be signed
        assert!(ev["sig"].as_str().is_some_and(|s| !s.is_empty()));
    }

    #[test]
    fn the_nonce_changes_the_header_so_repeats_are_not_replays() {
        let (_, keys) = tmp_identity("nonce");
        let a = nip98_header(&keys, "POST", "http://x/events", b"{}", "n1").unwrap();
        let b = nip98_header(&keys, "POST", "http://x/events", b"{}", "n2").unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn a_missing_identity_says_how_to_fix_it() {
        let root = std::env::temp_dir().join("waggle-events-absent");
        let _ = std::fs::remove_dir_all(&root);
        let art = ArtifactEvent {
            kind_marker: ArtifactKind::Artifact,
            channel_id: "c".into(),
            artifact_type: None,
            module: None,
            story: None,
            priority: Some(Priority::P1),
            references: vec![],
            from_role: None,
            to_role: None,
            body: "b".into(),
        };
        let err = publish_artifact(&root, "tea", "http://localhost:1", &art, "n").unwrap_err();
        assert!(err.to_string().contains("waggle identity provision"));
    }

    #[test]
    fn an_invalid_artifact_is_rejected_before_any_network_call() {
        // Validation must precede I/O: pointing at an unreachable relay proves it.
        let (root, _) = tmp_identity("invalid");
        let bad = ArtifactEvent {
            kind_marker: ArtifactKind::Handoff,
            channel_id: "c".into(),
            artifact_type: None,
            module: None,
            story: None,
            priority: None,
            references: vec![], // handoff with no artifact
            from_role: None,
            to_role: None,
            body: "b".into(),
        };
        let err = publish_artifact(&root, "tea", "http://127.0.0.1:1", &bad, "n").unwrap_err();
        assert!(
            matches!(err, EventError::Invalid(_)),
            "expected validation failure, got {err}"
        );
    }
}

#[cfg(test)]
mod size_tests {
    use super::*;
    use waggle_core::{ArtifactKind, Priority};

    fn artifact(body: String) -> ArtifactEvent {
        ArtifactEvent {
            kind_marker: ArtifactKind::Artifact,
            channel_id: "c".into(),
            artifact_type: None,
            module: None,
            story: None,
            priority: Some(Priority::P1),
            references: vec![],
            from_role: None,
            to_role: None,
            body,
        }
    }

    fn identity(name: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!("waggle-size-{name}"));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("keys")).unwrap();
        let keys = Keys::generate();
        std::fs::write(
            root.join("keys").join("tea.nsec"),
            keys.secret_key().to_secret_hex(),
        )
        .unwrap();
        root
    }

    #[test]
    fn oversized_body_is_refused_before_any_network_call() {
        // Pointing at a dead port proves the size check runs first: a network attempt
        // would surface as Unreachable, not TooLarge.
        let root = identity("over");
        let limits = Limits {
            max_message: 1000,
            max_content: 500,
        };
        let err = publish_artifact_with_limits(
            &root,
            "tea",
            "http://127.0.0.1:1",
            &artifact("x".repeat(501)),
            "n",
            Some(limits),
        )
        .unwrap_err();

        match err {
            EventError::TooLarge { bytes, limit } => {
                assert_eq!((bytes, limit), (501, 500));
                // NFR-4: the message must say what to do, not merely that it failed.
                let msg = EventError::TooLarge { bytes, limit }.to_string();
                assert!(msg.contains("images only"), "{msg}");
                assert!(msg.contains("split"), "{msg}");
            }
            other => panic!("expected TooLarge, got {other}"),
        }
    }

    #[test]
    fn a_body_exactly_at_the_limit_is_allowed_through_the_check() {
        let root = identity("edge");
        let limits = Limits {
            max_message: 1000,
            max_content: 500,
        };
        let err = publish_artifact_with_limits(
            &root,
            "tea",
            "http://127.0.0.1:1",
            &artifact("x".repeat(500)),
            "n",
            Some(limits),
        )
        .unwrap_err();
        // It gets past the size gate and fails on the network instead.
        assert!(
            matches!(err, EventError::Unreachable { .. }),
            "expected the size check to pass, got {err}"
        );
    }

    #[test]
    fn limits_fall_back_to_observed_values_when_nip11_is_unavailable() {
        let l = discover_limits("http://127.0.0.1:1");
        assert_eq!(l, Limits::default());
        assert_eq!(l.max_content, 262_144);
    }
}
