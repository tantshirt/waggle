//! Agent runtime configuration (FR-13) and managed-agent publication (FR-12/14).
//!
//! Emits the config a session runner needs: pack path, identity paths, concurrency
//! bounds, relay URL. Does **not** start an agent process — that requires an ACP
//! runtime and LLM credentials on the operator's machine (Story 1.7 residual).
//!
//! Publishing kind:30175 (persona definition) and kind:30177 (managed-agent instance)
//! is headless and owner-authored (NIP-AP). Secrets never appear in the projection.

use std::fs;
use std::path::{Path, PathBuf};

use nostr::{EventBuilder, Keys, Kind, Tag};
use serde::{Deserialize, Serialize};

use crate::events::{nip98_header, EventError, Published, Transport};
use crate::identity::{self, IdentityError};

/// Default session concurrency ceiling (NFR-8). Bounded; matches buzz-acp's
/// documented comfort range rather than its hard max of 32.
pub const DEFAULT_MAX_SESSIONS: u32 = 8;

const KIND_PERSONA: u16 = 30_175;
const KIND_MANAGED_AGENT: u16 = 30_177;

/// Valid Buzz `respond_to` wire values (NIP-AP).
pub const RESPOND_TO_VALUES: &[&str] = &["owner-only", "allowlist", "anyone"];

#[derive(Debug, thiserror::Error)]
pub enum RuntimeError {
    #[error(transparent)]
    Identity(#[from] IdentityError),

    #[error(transparent)]
    Event(#[from] EventError),

    #[error("pack directory {0} does not exist — compile first: waggle compile --module <module>")]
    PackMissing(PathBuf),

    #[error("could not write runtime config to {path}: {source}")]
    Unwritable {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("could not read persona from {path}: {reason}")]
    BadPack { path: PathBuf, reason: String },

    #[error("buzz-cli failed adding channel member: {0}")]
    ChannelMemberFailed(String),

    #[error(
        "owner secret required for publishing personas/managed-agents — set BUZZ_PRIVATE_KEY or WAGGLE_OWNER_NSEC"
    )]
    OwnerSecretMissing,

    #[error("owner secret is not a valid secret key: {0}")]
    OwnerSecretMalformed(String),

    #[error("invalid respond_to {0:?} — expected one of: owner-only, allowlist, anyone")]
    InvalidRespondTo(String),
}

/// Machine-readable runtime configuration for one role (FR-13).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeConfig {
    pub role: String,
    pub npub: String,
    pub public_key_hex: String,
    /// Path to the secret file — never the secret itself (AD-14).
    pub secret_key_path: String,
    pub pack_dir: String,
    pub persona_file: String,
    pub relay_url: String,
    pub max_sessions: u32,
    /// Env var the ACP runner should set to the agent command.
    pub acp_agent_command_env: String,
    /// Documented env vars the operator must supply for a live turn.
    pub required_env: Vec<String>,
}

/// Parsed pack persona used for owner-authored 30175/30177 publication.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackPersona {
    pub display_name: String,
    pub description: String,
    /// Markdown body after the closing `---` — the real system prompt.
    pub body: String,
}

/// Emit runtime configuration for `role` against a compiled pack.
pub fn emit_config(
    project_root: &Path,
    role: &str,
    pack_dir: &Path,
    persona_id: &str,
    relay_url: &str,
    max_sessions: u32,
) -> Result<(RuntimeConfig, PathBuf), RuntimeError> {
    if !pack_dir.is_dir() {
        return Err(RuntimeError::PackMissing(pack_dir.to_path_buf()));
    }

    let id = identity::load_public(project_root, role)?;
    let persona_file = pack_dir
        .join("agents")
        .join(format!("{persona_id}.persona.md"));
    if !persona_file.is_file() {
        return Err(RuntimeError::BadPack {
            path: persona_file,
            reason: "persona file missing".into(),
        });
    }

    let secret_key_path = identity::key_dir(project_root).join(format!("{role}.nsec"));
    let cfg = RuntimeConfig {
        role: role.to_string(),
        npub: id.npub,
        public_key_hex: id.public_key_hex,
        secret_key_path: secret_key_path.display().to_string(),
        pack_dir: pack_dir.display().to_string(),
        persona_file: persona_file.display().to_string(),
        relay_url: relay_url.to_string(),
        max_sessions: max_sessions.max(1),
        acp_agent_command_env: "BUZZ_ACP_AGENT_COMMAND".into(),
        required_env: vec![
            "BUZZ_ACP_AGENT_COMMAND".into(),
            "GOOSE_PROVIDER".into(),
            "GOOSE_MODEL".into(),
            "BUZZ_PRIVATE_KEY".into(),
            "BUZZ_RELAY_URL".into(),
        ],
    };

    let out_dir = project_root.join("keys").join("runtime");
    fs::create_dir_all(&out_dir).map_err(|source| RuntimeError::Unwritable {
        path: out_dir.clone(),
        source,
    })?;
    let out_path = out_dir.join(format!("{role}.json"));
    let json = serde_json::to_vec_pretty(&cfg).map_err(|e| RuntimeError::Unwritable {
        path: out_path.clone(),
        source: std::io::Error::other(e.to_string()),
    })?;
    fs::write(&out_path, json).map_err(|source| RuntimeError::Unwritable {
        path: out_path.clone(),
        source,
    })?;

    Ok((cfg, out_path))
}

/// Resolve the hive owner secret used to author 30175/30177.
///
/// Prefers `WAGGLE_OWNER_NSEC`, then `BUZZ_PRIVATE_KEY` — the same principal used
/// as `BUZZ_PRIVATE_KEY` for channel provision.
pub fn load_owner_keys() -> Result<Keys, RuntimeError> {
    let secret = std::env::var("WAGGLE_OWNER_NSEC")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            std::env::var("BUZZ_PRIVATE_KEY")
                .ok()
                .filter(|s| !s.trim().is_empty())
        })
        .ok_or(RuntimeError::OwnerSecretMissing)?;
    Keys::parse(secret.trim()).map_err(|e| RuntimeError::OwnerSecretMalformed(e.to_string()))
}

/// Validate a Buzz `respond_to` wire value.
pub fn validate_respond_to(value: &str) -> Result<&str, RuntimeError> {
    if RESPOND_TO_VALUES.contains(&value) {
        Ok(value)
    } else {
        Err(RuntimeError::InvalidRespondTo(value.to_string()))
    }
}

/// Slim definition-linked kind:30177 content (NIP-AP). Omits definition-level fields.
pub fn managed_agent_content(
    display_name: &str,
    persona_id: &str,
    max_sessions: u32,
    respond_to: &str,
) -> Result<String, RuntimeError> {
    let respond_to = validate_respond_to(respond_to)?;
    Ok(serde_json::json!({
        "name": display_name,
        "persona_id": persona_id,
        "parallelism": max_sessions.max(1),
        "respond_to": respond_to,
    })
    .to_string())
}

/// Kind:30175 persona definition content. `system_prompt` is the compiled body.
pub fn persona_definition_content(display_name: &str, system_prompt: &str) -> String {
    serde_json::json!({
        "display_name": display_name,
        "system_prompt": system_prompt,
    })
    .to_string()
}

/// Read display_name, description, and markdown body from a pack persona file.
pub fn read_pack_persona_file(path: &Path) -> Result<PackPersona, RuntimeError> {
    let text = fs::read_to_string(path).map_err(|e| RuntimeError::BadPack {
        path: path.to_path_buf(),
        reason: e.to_string(),
    })?;

    let mut lines = text.lines();
    let first = lines.next().unwrap_or("").trim();
    if first != "---" {
        return Err(RuntimeError::BadPack {
            path: path.to_path_buf(),
            reason: "missing opening frontmatter delimiter".into(),
        });
    }

    let mut display_name = None;
    let mut description = None;
    let mut closed = false;
    let mut body_lines = Vec::new();
    for line in lines {
        if !closed {
            if line.trim() == "---" {
                closed = true;
                continue;
            }
            if let Some(v) = line.strip_prefix("display_name:") {
                display_name = Some(v.trim().trim_matches('"').to_string());
            } else if let Some(v) = line.strip_prefix("description:") {
                description = Some(v.trim().trim_matches('"').to_string());
            }
        } else {
            body_lines.push(line);
        }
    }
    if !closed {
        return Err(RuntimeError::BadPack {
            path: path.to_path_buf(),
            reason: "missing closing frontmatter delimiter".into(),
        });
    }

    Ok(PackPersona {
        display_name: display_name.ok_or_else(|| RuntimeError::BadPack {
            path: path.to_path_buf(),
            reason: "missing display_name".into(),
        })?,
        description: description.unwrap_or_default(),
        body: body_lines.join("\n").trim().to_string(),
    })
}

fn post_signed_event(
    keys: &Keys,
    event: nostr::Event,
    relay_url: &str,
    nonce: &str,
) -> Result<Published, RuntimeError> {
    let event_id = event.id.to_hex();
    let pubkey = event.pubkey.to_hex();
    let body = serde_json::to_vec(&event).map_err(|e| EventError::Build(e.to_string()))?;
    let url = format!("{}/events", relay_url.trim_end_matches('/'));
    let auth = nip98_header(keys, "POST", &url, &body, nonce)?;

    let resp = crate::events::http_client()
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
    let text = crate::events::response_text_capped(resp, &url, crate::events::MAX_HTTP_JSON_BYTES)?;
    if !status.is_success() {
        return Err(EventError::Rejected {
            url,
            status: status.as_u16(),
            body: text.chars().take(300).collect(),
        }
        .into());
    }

    Ok(Published {
        event_id,
        pubkey,
        transport: Transport::Inline,
    })
}

/// Publish a kind:30175 persona definition, signed by the hive owner.
pub fn publish_persona_definition(
    owner: &Keys,
    relay_url: &str,
    persona_id: &str,
    display_name: &str,
    system_prompt: &str,
    nonce: &str,
) -> Result<Published, RuntimeError> {
    let content = persona_definition_content(display_name, system_prompt);
    let secret_hex = owner.secret_key().to_secret_hex();
    if content.contains(&secret_hex) {
        return Err(RuntimeError::BadPack {
            path: PathBuf::from("persona"),
            reason: "persona projection would embed secret key material".into(),
        });
    }

    let event = EventBuilder::new(Kind::Custom(KIND_PERSONA), content)
        .tags(vec![
            Tag::parse(["d", persona_id]).map_err(|e| EventError::Build(e.to_string()))?
        ])
        .sign_with_keys(owner)
        .map_err(|e| EventError::Build(e.to_string()))?;

    post_signed_event(owner, event, relay_url, nonce)
}

/// Publish a kind:30177 managed-agent instance, signed by the hive **owner**.
///
/// The `d` tag is the agent's pubkey. Content is slim / definition-linked when
/// `persona_id` is set (NIP-AP).
#[allow(clippy::too_many_arguments)]
pub fn publish_managed_agent(
    project_root: &Path,
    agent_role: &str,
    owner: &Keys,
    relay_url: &str,
    display_name: &str,
    persona_id: &str,
    max_sessions: u32,
    respond_to: &str,
    nonce: &str,
) -> Result<Published, RuntimeError> {
    let id = identity::load_public(project_root, agent_role)?;
    let content = managed_agent_content(display_name, persona_id, max_sessions, respond_to)?;
    let secret_hex = owner.secret_key().to_secret_hex();
    if content.contains(&secret_hex) {
        return Err(RuntimeError::BadPack {
            path: project_root.to_path_buf(),
            reason: "managed-agent projection would embed secret key material".into(),
        });
    }

    let event = EventBuilder::new(Kind::Custom(KIND_MANAGED_AGENT), content)
        .tags(vec![
            Tag::parse(["d", &id.public_key_hex]).map_err(|e| EventError::Build(e.to_string()))?
        ])
        .sign_with_keys(owner)
        .map_err(|e| EventError::Build(e.to_string()))?;

    // Coordinate is (30177, owner_pubkey, d=agent_pubkey) — required for Buzz
    // `authors: [owner]` roster resolution.
    debug_assert_eq!(event.pubkey, owner.public_key());

    post_signed_event(owner, event, relay_url, nonce)
}

/// Publish 30175 then slim 30177 for one pack persona / agent role.
#[allow(clippy::too_many_arguments)]
pub fn publish_persona_and_agent(
    project_root: &Path,
    agent_role: &str,
    pack_dir: &Path,
    persona_id: &str,
    relay_url: &str,
    max_sessions: u32,
    respond_to: &str,
    nonce: &str,
) -> Result<(Published, Published), RuntimeError> {
    let owner = load_owner_keys()?;
    let persona_path = pack_dir
        .join("agents")
        .join(format!("{persona_id}.persona.md"));
    let persona = read_pack_persona_file(&persona_path)?;
    let prompt = if persona.body.is_empty() {
        persona.description.clone()
    } else {
        persona.body.clone()
    };
    let def = publish_persona_definition(
        &owner,
        relay_url,
        persona_id,
        &persona.display_name,
        &prompt,
        nonce,
    )?;
    let agent = publish_managed_agent(
        project_root,
        agent_role,
        &owner,
        relay_url,
        &persona.display_name,
        persona_id,
        max_sessions,
        respond_to,
        &format!("{nonce}-agent"),
    )?;
    Ok((def, agent))
}

/// Add the role's pubkey to a channel roster via `buzz channels add-member`.
pub fn add_channel_member(
    buzz_cli: &Path,
    relay_url: &str,
    secret_hex: &str,
    channel_id: &str,
    pubkey_hex: &str,
    channel_role: &str,
) -> Result<String, RuntimeError> {
    let out = std::process::Command::new(buzz_cli)
        .env("BUZZ_PRIVATE_KEY", secret_hex)
        .env("BUZZ_RELAY_URL", relay_url)
        .args([
            "channels",
            "add-member",
            "--channel",
            channel_id,
            "--pubkey",
            pubkey_hex,
            "--role",
            channel_role,
        ])
        .output()
        .map_err(|source| RuntimeError::Unwritable {
            path: buzz_cli.to_path_buf(),
            source,
        })?;

    if !out.status.success() {
        return Err(RuntimeError::ChannelMemberFailed(
            String::from_utf8_lossy(&out.stderr).trim().to_string(),
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::provision;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("waggle-runtime-{name}"));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn emit_config_writes_public_data_only() {
        let root = tmp("emit");
        let id = provision(&root, "tea", false).unwrap();
        let pack = root.join("packs/tea");
        fs::create_dir_all(pack.join("agents")).unwrap();
        fs::write(
            pack.join("agents/bmad-tea.persona.md"),
            "---\nname: bmad-tea\ndisplay_name: Murat\ndescription: x\n---\nbody\n",
        )
        .unwrap();

        let (cfg, path) = emit_config(
            &root,
            "tea",
            &pack,
            "bmad-tea",
            "http://localhost:3100",
            DEFAULT_MAX_SESSIONS,
        )
        .unwrap();

        assert_eq!(cfg.npub, id.npub);
        assert_eq!(cfg.max_sessions, 8);
        let rendered = fs::read_to_string(&path).unwrap();
        let secret = fs::read_to_string(root.join("keys/tea.nsec")).unwrap();
        assert!(
            !rendered.contains(secret.trim()),
            "runtime config must not embed the secret"
        );
        assert!(rendered.contains("BUZZ_ACP_AGENT_COMMAND"));
    }

    #[test]
    fn emit_config_refuses_a_missing_pack() {
        let root = tmp("missing-pack");
        provision(&root, "tea", false).unwrap();
        let err = emit_config(
            &root,
            "tea",
            &root.join("nope"),
            "bmad-tea",
            "http://localhost:3100",
            8,
        )
        .unwrap_err();
        assert!(matches!(err, RuntimeError::PackMissing(_)));
    }

    #[test]
    fn managed_agent_content_is_slim_and_valid() {
        let raw = managed_agent_content("Murat", "bmad-tea", 4, "anyone").unwrap();
        let v: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(v["persona_id"], "bmad-tea");
        assert_eq!(v["respond_to"], "anyone");
        assert_eq!(v["parallelism"], 4);
        assert!(v.get("system_prompt").is_none());
        assert!(v.get("model").is_none());
        assert!(v.get("provider").is_none());
    }

    #[test]
    fn managed_agent_content_rejects_mentions() {
        let err = managed_agent_content("x", "y", 1, "mentions").unwrap_err();
        assert!(matches!(err, RuntimeError::InvalidRespondTo(_)));
    }

    #[test]
    fn owner_signed_30177_uses_owner_pubkey_and_agent_d_tag() {
        let root = tmp("owner-sign");
        let agent = provision(&root, "tea", false).unwrap();
        let owner = Keys::generate();

        let content = managed_agent_content("Murat", "bmad-tea", 8, "owner-only").unwrap();
        let event = EventBuilder::new(Kind::Custom(KIND_MANAGED_AGENT), content)
            .tags(vec![Tag::parse(["d", &agent.public_key_hex]).unwrap()])
            .sign_with_keys(&owner)
            .unwrap();

        assert_eq!(event.pubkey.to_hex(), owner.public_key().to_hex());
        assert_ne!(event.pubkey.to_hex(), agent.public_key_hex);
        let d = event
            .tags
            .iter()
            .find_map(|t| {
                let s = t.clone().to_vec();
                if s.first().map(String::as_str) == Some("d") {
                    s.get(1).cloned()
                } else {
                    None
                }
            })
            .unwrap();
        assert_eq!(d, agent.public_key_hex);

        // Buzz-style authors=[owner] filter would match this event.
        assert_eq!(event.pubkey, owner.public_key());
    }

    #[test]
    fn read_pack_persona_uses_body_as_prompt() {
        let root = tmp("persona-body");
        let path = root.join("x.persona.md");
        fs::write(
            &path,
            "---\ndisplay_name: Murat\ndescription: short blurb\n---\n# Real prompt\n\nDo the work.\n",
        )
        .unwrap();
        let p = read_pack_persona_file(&path).unwrap();
        assert_eq!(p.display_name, "Murat");
        assert_eq!(p.description, "short blurb");
        assert!(p.body.contains("Real prompt"));
        assert!(!p.body.contains("display_name"));
    }

    #[test]
    fn persona_definition_content_carries_system_prompt() {
        let raw = persona_definition_content("Murat", "# body");
        let v: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(v["display_name"], "Murat");
        assert_eq!(v["system_prompt"], "# body");
    }
}
