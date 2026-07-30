//! Shared CLI helpers.

use std::path::Path;

pub fn role_for_agent(agent_id: &str) -> String {
    if agent_id == "bmad-tea" {
        return "tea".into();
    }
    if let Some(rest) = agent_id.strip_prefix("bmad-agent-") {
        return rest.to_string();
    }
    if let Some(rest) = agent_id.strip_prefix("bmad-cis-agent-") {
        return format!("cis-{rest}");
    }
    if let Some(rest) = agent_id.strip_prefix("gds-agent-") {
        return format!("gds-{rest}");
    }
    if let Some(rest) = agent_id.strip_prefix("wds-agent-") {
        return format!("wds-{rest}");
    }
    if let Some(rest) = agent_id.strip_prefix("bmad-") {
        return rest.to_string();
    }
    agent_id.to_string()
}

pub fn resolve_human_pubkey(explicit: Option<&str>) -> Option<String> {
    explicit
        .map(str::to_string)
        .or_else(|| std::env::var("WAGGLE_HUMAN_PUBKEY").ok())
        .or_else(|| std::env::var("BUZZ_ACP_AGENT_OWNER").ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

pub fn roster_pubkeys(root: &Path, human: Option<&str>) -> Vec<String> {
    let mut keys = Vec::new();
    if let Some(h) = human {
        keys.push(h.to_ascii_lowercase());
    }
    let runtime = root.join("keys").join("runtime");
    if let Ok(rd) = std::fs::read_dir(&runtime) {
        for entry in rd.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let Ok(raw) = std::fs::read_to_string(&path) else {
                continue;
            };
            let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) else {
                continue;
            };
            if let Some(pk) = v.get("public_key_hex").and_then(|x| x.as_str()) {
                let pk = pk.to_ascii_lowercase();
                if !keys.contains(&pk) {
                    keys.push(pk);
                }
            }
        }
    }
    keys.sort();
    keys.dedup();
    keys
}

pub fn uuid_like() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let n = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    format!("waggle-{n}")
}
