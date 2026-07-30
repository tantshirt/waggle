//! Agent runtime configuration (FR-13) and managed-agent publication (FR-12/14).
//!
//! Emits the config a session runner needs: pack path, identity paths, concurrency
//! bounds, relay URL. Does **not** start an agent process — that requires an ACP
//! runtime and LLM credentials on the operator's machine (Story 1.7 residual).
//!
//! Publishing a kind:30177 managed-agent record is headless and does not need a
//! live session (see review F-12). Secrets never appear in the projection.

use std::fs;
use std::path::{Path, PathBuf};

use nostr::{EventBuilder, Keys, Kind, Tag};
use serde::{Deserialize, Serialize};

use crate::events::{nip98_header, EventError, Published, Transport};
use crate::identity::{self, IdentityError};

/// Default session concurrency ceiling (NFR-8). Bounded; matches buzz-acp's
/// documented comfort range rather than its hard max of 32.
pub const DEFAULT_MAX_SESSIONS: u32 = 8;

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

/// Publish a kind:30177 managed-agent record for the role (headless; F-12).
///
/// Content is the opt-IN projection only — never secrets, env vars, or runtime
/// blobs. The `d` tag is the agent's 64-hex pubkey.
pub fn publish_managed_agent(
    project_root: &Path,
    role: &str,
    relay_url: &str,
    display_name: &str,
    system_prompt: &str,
    persona_id: &str,
    max_sessions: u32,
    nonce: &str,
) -> Result<Published, RuntimeError> {
    let id = identity::load_public(project_root, role)?;
    let sec_path = identity::key_dir(project_root).join(format!("{role}.nsec"));
    if !sec_path.exists() {
        return Err(IdentityError::NotProvisioned {
            role: role.to_string(),
        }
        .into());
    }
    let secret = fs::read_to_string(&sec_path)
        .map_err(|e| IdentityError::Malformed {
            path: sec_path.clone(),
            reason: e.to_string(),
        })?
        .trim()
        .to_string();
    let keys = Keys::parse(&secret).map_err(|e| IdentityError::Malformed {
        path: sec_path,
        reason: format!("not a valid secret key ({e})"),
    })?;

    let content = serde_json::json!({
        "name": display_name,
        "persona_id": persona_id,
        "system_prompt": system_prompt,
        "parallelism": max_sessions.max(1),
        "respond_to": "mentions",
    })
    .to_string();

    if content.contains(&secret) {
        return Err(RuntimeError::BadPack {
            path: project_root.to_path_buf(),
            reason: "managed-agent projection would embed secret key material".into(),
        });
    }

    const KIND_MANAGED_AGENT: u16 = 30_177;
    let event = EventBuilder::new(Kind::Custom(KIND_MANAGED_AGENT), content)
        .tags(vec![
            Tag::parse(["d", &id.public_key_hex]).map_err(|e| EventError::Build(e.to_string()))?,
        ])
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
        }
        .into());
    }

    Ok(Published {
        event_id,
        pubkey,
        transport: Transport::Inline,
    })
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
            "---\nname: bmad-tea\ndisplay_name: Murat\ndescription: x\n---\n",
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
}
