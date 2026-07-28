//! Reading and resolving BMAD agent descriptors (FR-1, FR-3).
//!
//! **AD-3: read-only.** **AD-19: body materialization is discovered, not assumed** — the
//! skill manifest records logical paths that do not exist on disk, so the real bodies are
//! resolved through the tool directory recorded in the installation manifest.
//!
//! The layering, lowest precedence first:
//!
//! 1. `<tool-dir>/<agent-id>/customize.toml` — shipped defaults
//! 2. `_bmad/custom/<agent-id>.toml` — team overrides, committed
//! 3. `_bmad/custom/<agent-id>.user.toml` — personal overrides, gitignored
//!
//! Merging is [`waggle_core::merge`] (AD-5), and a differential test in this module
//! asserts our result matches BMAD's own resolver for every installed agent.

use std::path::{Path, PathBuf};

use toml::Value;

use crate::MethodError;

/// Where an agent's shipped `customize.toml` lives, for a given tool directory.
///
/// AD-19: the tool directory comes from the installation manifest's `ides` list, never
/// hard-coded to `.claude/skills`.
pub fn customize_path(project_root: &Path, tool_dir: &str, agent_id: &str) -> PathBuf {
    project_root
        .join(tool_dir)
        .join(agent_id)
        .join("customize.toml")
}

fn team_override_path(project_root: &Path, agent_id: &str) -> PathBuf {
    project_root
        .join("_bmad")
        .join("custom")
        .join(format!("{agent_id}.toml"))
}

fn user_override_path(project_root: &Path, agent_id: &str) -> PathBuf {
    project_root
        .join("_bmad")
        .join("custom")
        .join(format!("{agent_id}.user.toml"))
}

/// Tool directories BMAD may have materialized skill bodies into, in manifest order.
///
/// Derived from the installation manifest's `ides` list (AD-19). The mapping from tool id
/// to directory mirrors the installer's own table.
pub fn tool_dirs(ides: &[String]) -> Vec<String> {
    ides.iter()
        .map(|ide| match ide.as_str() {
            "claude-code" => ".claude/skills".to_string(),
            "cline" => ".cline/skills".to_string(),
            "kiro" => ".kiro/skills".to_string(),
            "junie" => ".junie/skills".to_string(),
            "qoder" => ".qoder/skills".to_string(),
            "antigravity" => ".agent/skills".to_string(),
            // The installer's default for most tools.
            _ => ".agents/skills".to_string(),
        })
        .collect()
}

/// Resolve one agent's effective `[agent]` block across all three layers.
///
/// Returns the merged table. A missing override layer is skipped, not an error — most
/// agents have no overrides at all.
pub fn resolve_agent(
    project_root: &Path,
    tool_dir: &str,
    agent_id: &str,
) -> Result<Value, MethodError> {
    let base_path = customize_path(project_root, tool_dir, agent_id);
    if !base_path.exists() {
        return Err(MethodError::NotInstalled(
            project_root.to_path_buf(),
            base_path,
        ));
    }

    let mut layers = Vec::new();
    for path in [
        base_path.clone(),
        team_override_path(project_root, agent_id),
        user_override_path(project_root, agent_id),
    ] {
        if !path.exists() {
            continue;
        }
        let raw = std::fs::read_to_string(&path).map_err(|source| MethodError::Unreadable {
            path: path.clone(),
            source,
        })?;
        let value: Value = toml::from_str(&raw).map_err(|e| MethodError::UnparseableToml {
            path: path.clone(),
            reason: e.to_string(),
        })?;
        layers.push(value);
    }

    let merged = waggle_core::merge_layers(layers)
        .ok_or_else(|| MethodError::NotInstalled(project_root.to_path_buf(), base_path.clone()))?;

    // The `[agent]` table is the descriptor; a customize.toml without one is malformed.
    merged
        .get("agent")
        .cloned()
        .ok_or(MethodError::MissingAgentBlock { path: base_path })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo_root() -> PathBuf {
        // CARGO_MANIFEST_DIR is <root>/crates/waggle-method
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf()
    }

    #[test]
    fn tool_dirs_map_from_the_manifest_not_a_hardcoded_path() {
        assert_eq!(tool_dirs(&["claude-code".into()]), vec![".claude/skills"]);
        assert_eq!(tool_dirs(&["codex".into()]), vec![".agents/skills"]);
        assert_eq!(tool_dirs(&["kiro".into()]), vec![".kiro/skills"]);
    }

    #[test]
    fn resolves_the_pilot_agent_from_the_real_installation() {
        let root = repo_root();
        let agent = resolve_agent(&root, ".claude/skills", "bmad-tea")
            .expect("the TEA agent should resolve");

        assert_eq!(agent.get("name").and_then(|v| v.as_str()), Some("Murat"));
        assert_eq!(agent.get("icon").and_then(|v| v.as_str()), Some("🧪"));

        // All seven principles must survive the merge. Losing one would be silent.
        let principles = agent["principles"].as_array().expect("principles array");
        assert_eq!(principles.len(), 7, "principles were dropped by the merge");

        // Ten menu items: nine dispatchable, one prompt-only (AD-7).
        let menu = agent["menu"].as_array().expect("menu array");
        assert_eq!(menu.len(), 10);
        let prompts = menu.iter().filter(|m| m.get("prompt").is_some()).count();
        let skills = menu.iter().filter(|m| m.get("skill").is_some()).count();
        assert_eq!((skills, prompts), (9, 1), "TEA is 9 skills + GATE prompt");
    }

    /// **AD-5, mandatory and non-skippable.**
    ///
    /// Rust cannot reuse BMAD's resolver, so this asserts our reimplementation agrees with
    /// it for every installed agent. If BMAD changes its merge rules, this fails loudly
    /// instead of producing quietly-wrong personas.
    #[test]
    fn differential_against_bmad_resolver() {
        let root = repo_root();
        let script = root.join("_bmad/scripts/resolve_customization.py");
        if !script.exists() {
            panic!(
                "AD-5 differential test cannot run: {} is missing. \
                 This test may not be skipped — install BMAD or fix the path.",
                script.display()
            );
        }

        let skills_dir = root.join(".claude/skills");
        let mut checked = 0;

        for entry in std::fs::read_dir(&skills_dir).expect("skills dir should be readable") {
            let entry = entry.expect("dir entry");
            let skill_dir = entry.path();
            if !skill_dir.join("customize.toml").exists() {
                continue;
            }
            let agent_id = entry.file_name().to_string_lossy().to_string();

            // BMAD's own answer.
            let out = std::process::Command::new("uv")
                .current_dir(&root)
                .args(["run", "_bmad/scripts/resolve_customization.py", "--skill"])
                .arg(&skill_dir)
                .args(["--key", "agent"])
                .output()
                .expect("uv run should execute");

            if !out.status.success() {
                // Not every skill exposes an `agent` key; workflows use `workflow`.
                continue;
            }
            let theirs: serde_json::Value = match serde_json::from_slice(&out.stdout) {
                Ok(v) => v,
                Err(_) => continue,
            };
            let Some(theirs) = theirs.get("agent") else {
                continue;
            };

            // Ours.
            let ours = resolve_agent(&root, ".claude/skills", &agent_id)
                .unwrap_or_else(|e| panic!("waggle failed to resolve {agent_id}: {e}"));
            let ours: serde_json::Value = serde_json::to_value(&ours)
                .unwrap_or_else(|e| panic!("could not convert {agent_id} to json: {e}"));

            assert_eq!(
                &ours, theirs,
                "AD-5 VIOLATED for {agent_id}: waggle's resolved descriptor differs from \
                 BMAD's own resolver. A persona compiled from this would be silently wrong."
            );
            checked += 1;
        }

        assert!(
            checked > 0,
            "the differential test compared zero agents — it is not actually running"
        );
        eprintln!("AD-5 differential: {checked} agents matched BMAD's resolver");
    }
}
