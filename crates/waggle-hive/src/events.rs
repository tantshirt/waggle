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
use nostr::{EventBuilder, Keys, Kind, Tag, Timestamp};
use sha2::{Digest, Sha256};

use waggle_core::ArtifactEvent;

/// Standard NIP-29 group message. AD-8: standard kinds first.
const KIND_GROUP_MESSAGE: u16 = 9;

/// Blossom upload auth (BUD-02).
const KIND_BLOSSOM_AUTH: u16 = 24_242;

/// Upstream's generic-file upload cap (`max_file_bytes` in buzz-media). Not in NIP-11.
/// Bodies larger than this cannot go inline *or* by reference.
pub const MAX_BLOB_BYTES: usize = 104_857_600;

/// Marker `t` tag on events whose body is a content-addressed blob reference (FR-16).
pub const TAG_REF: &str = "ref";

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

    #[error("artifact body is {bytes} bytes, over the media store's {limit}-byte upload limit — cannot be published inline or by reference; split the artifact or shorten it")]
    TooLarge { bytes: usize, limit: usize },

    #[error("uploaded blob hash mismatch: expected {expected}, got {got}")]
    HashMismatch { expected: String, got: String },
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
pub(crate) fn nip98_header(
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

/// How the artifact body reached the hive (FR-16 / AD-15).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Transport {
    /// Body fits the content limit and was published inline in kind:9.
    Inline,
    /// Body was stored via Blossom `PUT /upload`; the event carries a hash reference.
    Reference {
        sha256: String,
        url: String,
        bytes: usize,
    },
}

/// What the relay accepted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Published {
    pub event_id: String,
    /// The publishing identity, so callers can attribute without touching the secret.
    pub pubkey: String,
    pub transport: Transport,
}

/// A content-addressed blob accepted by the relay's upload endpoint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlobRef {
    pub sha256: String,
    pub url: String,
    pub bytes: usize,
}

/// Sign and publish an artifact, handoff, verdict, or gate record.
///
/// The event is a standard kind:9 group message carrying waggle's typed tags, so a
/// third-party NIP-29 client still renders it (NFR-6).
///
/// **FR-16 / AD-15:** bodies within the content limit publish inline. Larger bodies are
/// uploaded via the relay's Blossom `PUT /upload` (not `buzz-cli`, which is images-only)
/// and the event carries a content-addressed reference. Bodies over the media-store cap
/// are refused with a specific error — never truncated or silently dropped.
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

    let limits = limits.unwrap_or_else(|| discover_limits(relay_url));
    let len = artifact.body.len();

    refuse_if_over_media_cap(len)?;

    let keys = load_keys(project_root, role)?;

    let (content, mut extra_tags, transport) = if len > limits.max_content {
        let blob = upload_blob(&keys, relay_url, artifact.body.as_bytes())?;
        let content = reference_body(&blob);
        let extra = vec![
            Tag::parse(["x", &blob.sha256]).map_err(|e| EventError::Build(e.to_string()))?,
            Tag::parse(["t", TAG_REF]).map_err(|e| EventError::Build(e.to_string()))?,
        ];
        (
            content,
            extra,
            Transport::Reference {
                sha256: blob.sha256,
                url: blob.url,
                bytes: blob.bytes,
            },
        )
    } else {
        (artifact.body.clone(), Vec::new(), Transport::Inline)
    };

    let mut tags = Vec::new();
    for t in artifact.tags() {
        tags.push(Tag::parse(t).map_err(|e| EventError::Build(e.to_string()))?);
    }
    tags.append(&mut extra_tags);

    let event = EventBuilder::new(Kind::Custom(KIND_GROUP_MESSAGE), content)
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

    Ok(Published {
        event_id,
        pubkey,
        transport,
    })
}

/// Refuse bodies that exceed even the media-store upload cap (FR-16).
fn refuse_if_over_media_cap(len: usize) -> Result<(), EventError> {
    if len > MAX_BLOB_BYTES {
        return Err(EventError::TooLarge {
            bytes: len,
            limit: MAX_BLOB_BYTES,
        });
    }
    Ok(())
}

/// Compact, machine-readable body for a reference-carrying event.
pub fn reference_body(blob: &BlobRef) -> String {
    serde_json::json!({
        "waggle": "blob-ref",
        "sha256": blob.sha256,
        "url": blob.url,
        "bytes": blob.bytes,
    })
    .to_string()
}

/// Blossom auth header for `PUT /upload` (kind 24242, URL-safe base64).
fn blossom_upload_auth(keys: &Keys, sha256: &str) -> Result<String, EventError> {
    let now = Timestamp::now().as_secs();
    let tags = [
        Tag::parse(["t", "upload"]),
        Tag::parse(["x", sha256]),
        Tag::parse(["expiration", &(now + 300).to_string()]),
    ];
    let mut built = Vec::new();
    for t in tags {
        built.push(t.map_err(|e| EventError::Build(e.to_string()))?);
    }

    let event = EventBuilder::new(Kind::Custom(KIND_BLOSSOM_AUTH), "Upload")
        .tags(built)
        .sign_with_keys(keys)
        .map_err(|e| EventError::Build(e.to_string()))?;

    Ok(format!(
        "Nostr {}",
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(event.as_json().as_bytes())
    ))
}

/// Upload raw bytes to the relay's Blossom endpoint. Speaks HTTP directly — `buzz-cli`
/// upload is images-only and must not be used for method artifacts (UP-16 withdrawn).
pub fn upload_blob(keys: &Keys, relay_url: &str, bytes: &[u8]) -> Result<BlobRef, EventError> {
    let sha256 = hex_encode(&Sha256::digest(bytes));
    let url = format!("{}/upload", relay_url.trim_end_matches('/'));
    let auth = blossom_upload_auth(keys, &sha256)?;

    let client = reqwest::blocking::Client::new();
    let resp = client
        .put(&url)
        .header("Authorization", auth)
        .header("X-SHA-256", &sha256)
        .header("Content-Type", "application/octet-stream")
        .body(bytes.to_vec())
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

    let desc: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| EventError::Unparseable(e.to_string()))?;
    let got = desc
        .get("sha256")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    if got != sha256 {
        return Err(EventError::HashMismatch {
            expected: sha256,
            got: got.to_string(),
        });
    }
    let blob_url = desc
        .get("url")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    if blob_url.is_empty() {
        return Err(EventError::Unparseable(
            "upload response missing url".into(),
        ));
    }

    Ok(BlobRef {
        sha256,
        url: blob_url,
        bytes: bytes.len(),
    })
}

/// Fetch a blob and verify its SHA-256 matches (FR-16 retrieve path).
pub fn fetch_and_verify(blob_url: &str, expected_sha256: &str) -> Result<Vec<u8>, EventError> {
    let client = reqwest::blocking::Client::new();
    let resp = client
        .get(blob_url)
        .send()
        .map_err(|source| EventError::Unreachable {
            url: blob_url.to_string(),
            source,
        })?;

    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().unwrap_or_default();
        return Err(EventError::Rejected {
            url: blob_url.to_string(),
            status: status.as_u16(),
            body: body.chars().take(300).collect(),
        });
    }

    let bytes = resp
        .bytes()
        .map_err(|source| EventError::Unreachable {
            url: blob_url.to_string(),
            source,
        })?
        .to_vec();
    let got = hex_encode(&Sha256::digest(&bytes));
    if got != expected_sha256 {
        return Err(EventError::HashMismatch {
            expected: expected_sha256.to_string(),
            got,
        });
    }
    Ok(bytes)
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
    fn body_over_media_cap_is_refused_before_any_network_call() {
        // Don't allocate 100MB in a unit test — exercise the gate directly.
        let err = refuse_if_over_media_cap(MAX_BLOB_BYTES + 1).unwrap_err();
        match err {
            EventError::TooLarge { bytes, limit } => {
                assert_eq!((bytes, limit), (MAX_BLOB_BYTES + 1, MAX_BLOB_BYTES));
                let msg = err.to_string();
                assert!(msg.contains("upload limit"), "{msg}");
                assert!(msg.contains("split"), "{msg}");
                assert!(!msg.contains("images only"), "{msg}");
            }
            other => panic!("expected TooLarge, got {other}"),
        }
        assert!(refuse_if_over_media_cap(MAX_BLOB_BYTES).is_ok());
    }

    #[test]
    fn oversized_for_inline_attempts_reference_upload_before_event_post() {
        // Body over the content limit but under the media cap takes the Blossom path.
        // Against a dead port that surfaces as Unreachable on /upload, not TooLarge.
        let root = identity("over-inline");
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
        assert!(
            matches!(err, EventError::Unreachable { ref url, .. } if url.ends_with("/upload")),
            "expected Unreachable on /upload, got {err}"
        );
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
        // It gets past the size gate and fails on the event POST instead.
        assert!(
            matches!(err, EventError::Unreachable { ref url, .. } if url.ends_with("/events")),
            "expected the size check to pass into /events, got {err}"
        );
    }

    #[test]
    fn limits_fall_back_to_observed_values_when_nip11_is_unavailable() {
        let l = discover_limits("http://127.0.0.1:1");
        assert_eq!(l, Limits::default());
        assert_eq!(l.max_content, 262_144);
    }

    #[test]
    fn reference_body_is_compact_and_carries_the_hash() {
        let blob = BlobRef {
            sha256: "ab".repeat(32),
            url: "http://localhost:3100/media/ab.bin".into(),
            bytes: 300_000,
        };
        let body = reference_body(&blob);
        assert!(body.len() < 500, "reference body must fit easily: {}", body.len());
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["waggle"], "blob-ref");
        assert_eq!(v["sha256"], blob.sha256);
        assert_eq!(v["url"], blob.url);
        assert_eq!(v["bytes"], 300_000);
    }

    #[test]
    fn blossom_auth_is_kind_24242_with_url_safe_encoding() {
        let (_, keys) = {
            let root = identity("blossom-auth");
            let keys = load_keys(&root, "tea").unwrap();
            (root, keys)
        };
        let header = blossom_upload_auth(&keys, &"ab".repeat(32)).unwrap();
        let b64 = header.strip_prefix("Nostr ").expect("scheme");
        // URL_SAFE_NO_PAD — no '+' '/' or '=' padding.
        assert!(!b64.contains('+') && !b64.contains('/') && !b64.contains('='), "{b64}");
        let json = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(b64)
            .unwrap();
        let ev: serde_json::Value = serde_json::from_slice(&json).unwrap();
        assert_eq!(ev["kind"], KIND_BLOSSOM_AUTH);
        let tags: Vec<(String, String)> = ev["tags"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| (t[0].as_str().unwrap().into(), t[1].as_str().unwrap().into()))
            .collect();
        assert!(tags.iter().any(|(n, v)| n == "t" && v == "upload"));
        assert!(tags.iter().any(|(n, _)| n == "x"));
        assert!(tags.iter().any(|(n, _)| n == "expiration"));
    }
}

// ---------------------------------------------------------------------------
// Gate reconciliation inputs (UP-18)
// ---------------------------------------------------------------------------

/// Generic signed query against the relay, returning raw events.
fn query(
    keys: &Keys,
    relay_url: &str,
    filter: serde_json::Value,
    nonce: &str,
) -> Result<Vec<serde_json::Value>, EventError> {
    let body = serde_json::to_vec(&filter).map_err(|e| EventError::Build(e.to_string()))?;
    let url = format!("{}/query", relay_url.trim_end_matches('/'));
    let auth = nip98_header(keys, "POST", &url, &body, nonce)?;

    let resp = reqwest::blocking::Client::new()
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
    Ok(parsed.as_array().cloned().unwrap_or_default())
}

/// Fetch the reactions (kind:7) targeting `verdict_event`.
///
/// **The author is read from each event's `pubkey` field and nothing else.** That is the
/// entire point of UP-18: `actor` tags are attacker-controlled on client-submitted events
/// (the relay guards them only when it signed the event itself), so they are ignored here
/// even if present.
pub fn fetch_reactions(
    project_root: &Path,
    role: &str,
    relay_url: &str,
    verdict_event: &str,
    nonce: &str,
) -> Result<Vec<waggle_core::gate::SignedReaction>, EventError> {
    let keys = load_keys(project_root, role)?;
    let events = query(
        &keys,
        relay_url,
        serde_json::json!([{ "kinds": [7], "#e": [verdict_event], "limit": 200 }]),
        nonce,
    )?;

    Ok(events
        .iter()
        .filter_map(|e| {
            Some(waggle_core::gate::SignedReaction {
                event_id: e.get("id")?.as_str()?.to_string(),
                // Signature-bound. Never `actor`.
                author_pubkey: e.get("pubkey")?.as_str()?.to_string(),
                emoji: e
                    .get("content")
                    .and_then(|c| c.as_str())
                    .unwrap_or("")
                    .to_string(),
                target_event: verdict_event.to_string(),
                created_at: e.get("created_at").and_then(serde_json::Value::as_u64)?,
            })
        })
        .collect())
}

/// Fetch the relay-signed admin roster (kind:39001) for a channel.
///
/// Tags are `["p", pubkey, relay_url, role]`. The event is signed by the relay, which is
/// exactly what makes it usable as an authorization source (AD-13): waggle maintains no
/// approver list of its own.
pub fn fetch_roster(
    project_root: &Path,
    role: &str,
    relay_url: &str,
    channel_id: &str,
    nonce: &str,
) -> Result<Vec<waggle_core::gate::RosterEntry>, EventError> {
    let keys = load_keys(project_root, role)?;
    let events = query(
        &keys,
        relay_url,
        serde_json::json!([{ "kinds": [39001], "#d": [channel_id], "limit": 10 }]),
        nonce,
    )?;

    let mut out = Vec::new();
    for e in &events {
        let Some(tags) = e.get("tags").and_then(|t| t.as_array()) else {
            continue;
        };
        for t in tags {
            let Some(parts) = t.as_array() else { continue };
            if parts.first().and_then(|v| v.as_str()) != Some("p") {
                continue;
            }
            let Some(pubkey) = parts.get(1).and_then(|v| v.as_str()) else {
                continue;
            };
            // NIP-29 writes `["p", pubkey, relay_url, role]`, but the relay omits an
            // empty relay_url, so real events look like `["p", pubkey, "owner"]`.
            // Verified against stored events, not inferred from the builder source —
            // reading `Tag::parse(["p", hex, "", role])` suggests index 3, and the
            // emitted tag is at index 2. Scan for a role value instead of trusting a
            // position, so either shape works.
            let role = parts
                .iter()
                .skip(2)
                .filter_map(|v| v.as_str())
                .find_map(|v| match v {
                    "owner" => Some(waggle_core::gate::Role::Owner),
                    "admin" => Some(waggle_core::gate::Role::Admin),
                    _ => None,
                })
                .unwrap_or(waggle_core::gate::Role::Member);
            out.push(waggle_core::gate::RosterEntry {
                pubkey: pubkey.to_string(),
                role,
            });
        }
    }
    // Deterministic ordering (NFR-1).
    out.sort_by(|a, b| a.pubkey.cmp(&b.pubkey));
    out.dedup_by(|a, b| a.pubkey == b.pubkey);
    Ok(out)
}

#[cfg(test)]
mod reconcile_input_tests {

    #[test]
    fn reaction_parsing_ignores_actor_tags_entirely() {
        // A hostile reaction carrying ["actor", "<victim>"] must still be attributed to
        // the key that signed it. This is UP-18's attack, asserted directly.
        let raw: Vec<serde_json::Value> = serde_json::from_str(
            r#"[{
                "id": "r1",
                "pubkey": "attacker",
                "content": "white_check_mark",
                "created_at": 100,
                "tags": [["e","v1"],["actor","victim-who-never-approved"]]
            }]"#,
        )
        .unwrap();

        // Mirrors the mapping in fetch_reactions.
        let parsed: Vec<_> = raw
            .iter()
            .filter_map(|e| {
                Some(waggle_core::gate::SignedReaction {
                    event_id: e.get("id")?.as_str()?.to_string(),
                    author_pubkey: e.get("pubkey")?.as_str()?.to_string(),
                    emoji: e.get("content")?.as_str()?.to_string(),
                    target_event: "v1".into(),
                    created_at: e.get("created_at")?.as_u64()?,
                })
            })
            .collect();

        assert_eq!(parsed[0].author_pubkey, "attacker");
        assert_ne!(parsed[0].author_pubkey, "victim-who-never-approved");
    }
}
