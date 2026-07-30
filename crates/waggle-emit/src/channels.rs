//! Emits `channel-templates.json` in the shape Buzz's template loader reads (FR-10,
//! FR-25, FR-26).
//!
//! **The substrate already provisions channels and canvases from a template store**
//! (`crates/buzz-cli/src/commands/channel_templates.rs`), and `--templates-file` overrides
//! the desktop app's default location — so waggle ships the store inside the compiled pack
//! and delegates the work rather than reimplementing it. Verified in `research-notes.md`
//! §8; that finding is what shrank Stories 2.7/2.8 into this one module.
//!
//! **AD-16: templates are data.** The input is `templates/<module>/channels.json`, and
//! nothing here branches on a module id. The agent roster is filled from the registry at
//! compile time, so a module's channels automatically list that module's agents.
//!
//! **Phase rooms (hive mirror):** set `stable_name: true` so the channel is named exactly
//! as authored (`planning`, not `bmm-planning`). Use `include_all_agents` for party/help.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// waggle's authored template, from `templates/<module>/channels.json`.
#[derive(Debug, Clone, Deserialize)]
pub struct ChannelTemplateSource {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default = "default_channel_type")]
    pub channel_type: String,
    #[serde(default = "default_visibility")]
    pub visibility: String,
    #[serde(default)]
    pub canvas_template: Option<String>,
    /// When true, emit `name` as-is (phase rooms). When false, prefix with `{module}-`.
    #[serde(default)]
    pub stable_name: bool,
    /// Fill the roster with every agent the module registers.
    #[serde(default)]
    pub include_module_agents: bool,
    /// Fill the roster with every agent the installation registers (party / help).
    #[serde(default)]
    pub include_all_agents: bool,
    /// Explicit additions, on top of include flags.
    #[serde(default)]
    pub personas: Vec<String>,
}

fn default_channel_type() -> String {
    "stream".to_string()
}

fn default_visibility() -> String {
    "open".to_string()
}

/// The wire shape Buzz reads. Field names and casing are fixed by its deserializer:
/// snake_case at the top level, camelCase inside `agents`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChannelTemplateRecord {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub channel_type: String,
    pub visibility: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub canvas_template: Option<String>,
    pub agents: AgentRoster,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentRoster {
    pub personas: Vec<PersonaEntry>,
    pub teams: Vec<TeamEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PersonaEntry {
    pub persona_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TeamEntry {
    pub team_id: String,
}

/// Build the store for one module.
pub fn build_store(
    module: &str,
    sources: &[ChannelTemplateSource],
    module_agent_ids: &[String],
    all_agent_ids: &[String],
) -> Vec<ChannelTemplateRecord> {
    sources
        .iter()
        .map(|s| {
            let mut personas: Vec<String> = Vec::new();
            if s.include_all_agents {
                personas.extend(all_agent_ids.iter().cloned());
            }
            if s.include_module_agents {
                for id in module_agent_ids {
                    if !personas.contains(id) {
                        personas.push(id.clone());
                    }
                }
            }
            for p in &s.personas {
                if !personas.contains(p) {
                    personas.push(p.clone());
                }
            }
            // Deterministic ordering (AD-4): the store is committed and diffed.
            personas.sort();
            personas.dedup();

            let name = if s.stable_name {
                s.name.clone()
            } else {
                format!("{module}-{}", s.name)
            };

            ChannelTemplateRecord {
                name,
                description: s.description.clone(),
                channel_type: s.channel_type.clone(),
                visibility: s.visibility.clone(),
                canvas_template: s.canvas_template.clone(),
                agents: AgentRoster {
                    personas: personas
                        .into_iter()
                        .map(|persona_id| PersonaEntry { persona_id })
                        .collect(),
                    teams: Vec::new(),
                },
            }
        })
        .collect()
}

/// Merge multiple template stores by channel name (case-insensitive).
///
/// Later records win on description/canvas/type; persona rosters are unioned.
/// Used by `waggle provision --all` for the hive-wide phase map.
pub fn merge_stores(stores: &[Vec<ChannelTemplateRecord>]) -> Vec<ChannelTemplateRecord> {
    let mut by_name: BTreeMap<String, ChannelTemplateRecord> = BTreeMap::new();
    for store in stores {
        for rec in store {
            let key = rec.name.to_ascii_lowercase();
            match by_name.get_mut(&key) {
                None => {
                    by_name.insert(key, rec.clone());
                }
                Some(existing) => {
                    if rec.description.is_some() {
                        existing.description = rec.description.clone();
                    }
                    // Prefer the richer canvas when both are present (help CSV seed vs stub).
                    match (&existing.canvas_template, &rec.canvas_template) {
                        (Some(a), Some(b)) if b.len() > a.len() => {
                            existing.canvas_template = Some(b.clone());
                        }
                        (_, Some(b)) => {
                            existing.canvas_template = Some(b.clone());
                        }
                        _ => {}
                    }
                    existing.channel_type = rec.channel_type.clone();
                    existing.visibility = rec.visibility.clone();
                    for p in &rec.agents.personas {
                        if !existing
                            .agents
                            .personas
                            .iter()
                            .any(|e| e.persona_id == p.persona_id)
                        {
                            existing.agents.personas.push(p.clone());
                        }
                    }
                    existing
                        .agents
                        .personas
                        .sort_by(|a, b| a.persona_id.cmp(&b.persona_id));
                }
            }
        }
    }
    by_name.into_values().collect()
}

/// Render the store as JSON, newline-terminated.
pub fn render(store: &[ChannelTemplateRecord]) -> Result<String, serde_json::Error> {
    let mut s = serde_json::to_string_pretty(store)?;
    s.push('\n');
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sources() -> Vec<ChannelTemplateSource> {
        serde_json::from_str(
            r##"[{
                "name": "test-strategy",
                "description": "d",
                "channel_type": "stream",
                "visibility": "open",
                "canvas_template": "# Test strategy\n",
                "include_module_agents": true
            }]"##,
        )
        .unwrap()
    }

    #[test]
    fn template_names_are_module_prefixed_by_default() {
        let store = build_store("tea", &sources(), &["bmad-tea".into()], &[]);
        assert_eq!(store[0].name, "tea-test-strategy");
    }

    #[test]
    fn stable_name_skips_module_prefix() {
        let mut s = sources();
        s[0].name = "planning".into();
        s[0].stable_name = true;
        let store = build_store("bmm", &s, &["bmad-agent-pm".into()], &[]);
        assert_eq!(store[0].name, "planning");
    }

    #[test]
    fn include_all_agents_fills_party_roster() {
        let mut s = sources();
        s[0].include_module_agents = false;
        s[0].include_all_agents = true;
        s[0].stable_name = true;
        s[0].name = "party".into();
        let store = build_store(
            "core",
            &s,
            &[],
            &["bmad-tea".into(), "bmad-agent-pm".into()],
        );
        let ids: Vec<_> = store[0]
            .agents
            .personas
            .iter()
            .map(|p| p.persona_id.as_str())
            .collect();
        assert_eq!(ids, vec!["bmad-agent-pm", "bmad-tea"]);
    }

    #[test]
    fn roster_is_filled_from_the_module_registry() {
        let store = build_store(
            "bmm",
            &sources(),
            &["bmad-agent-pm".into(), "bmad-agent-dev".into()],
            &[],
        );
        let ids: Vec<_> = store[0]
            .agents
            .personas
            .iter()
            .map(|p| p.persona_id.as_str())
            .collect();
        assert_eq!(ids, vec!["bmad-agent-dev", "bmad-agent-pm"]);
    }

    #[test]
    fn merge_stores_unions_personas() {
        let a = build_store(
            "bmm",
            &[ChannelTemplateSource {
                name: "planning".into(),
                description: Some("plan".into()),
                channel_type: "forum".into(),
                visibility: "open".into(),
                canvas_template: None,
                stable_name: true,
                include_module_agents: false,
                include_all_agents: false,
                personas: vec!["bmad-agent-pm".into()],
            }],
            &[],
            &[],
        );
        let b = build_store(
            "tea",
            &[ChannelTemplateSource {
                name: "planning".into(),
                description: None,
                channel_type: "forum".into(),
                visibility: "open".into(),
                canvas_template: Some("# x".into()),
                stable_name: true,
                include_module_agents: false,
                include_all_agents: false,
                personas: vec!["bmad-tea".into()],
            }],
            &[],
            &[],
        );
        let merged = merge_stores(&[a, b]);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].name, "planning");
        assert_eq!(merged[0].canvas_template.as_deref(), Some("# x"));
        let ids: Vec<_> = merged[0]
            .agents
            .personas
            .iter()
            .map(|p| p.persona_id.as_str())
            .collect();
        assert_eq!(ids, vec!["bmad-agent-pm", "bmad-tea"]);
    }

    #[test]
    fn wire_shape_matches_what_buzz_deserializes() {
        let store = build_store("tea", &sources(), &["bmad-tea".into()], &[]);
        let json = render(&store).unwrap();
        for key in [
            "\"channel_type\"",
            "\"visibility\"",
            "\"canvas_template\"",
            "\"personaId\"",
            "\"teams\"",
        ] {
            assert!(json.contains(key), "missing {key} in:\n{json}");
        }
        assert!(!json.contains("\"persona_id\""), "roster uses camelCase");
        assert!(
            !json.contains("\"channelType\""),
            "top level uses snake_case"
        );
    }

    #[test]
    fn rendering_is_deterministic() {
        let a = render(&build_store("tea", &sources(), &["bmad-tea".into()], &[])).unwrap();
        let b = render(&build_store("tea", &sources(), &["bmad-tea".into()], &[])).unwrap();
        assert_eq!(a, b);
    }
}
