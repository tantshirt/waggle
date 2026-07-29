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
    /// Fill the roster with every agent the module registers. Keeps the template free of
    /// hard-coded agent ids that would drift as a module gains or loses agents.
    #[serde(default)]
    pub include_module_agents: bool,
    /// Explicit additions, on top of `include_module_agents`.
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
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
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

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentRoster {
    pub personas: Vec<PersonaEntry>,
    pub teams: Vec<TeamEntry>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PersonaEntry {
    pub persona_id: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TeamEntry {
    pub team_id: String,
}

/// Build the store for one module.
///
/// Template names are prefixed with the module code so two modules cannot collide in a
/// single store — Buzz matches templates by name, case-insensitively.
pub fn build_store(
    module: &str,
    sources: &[ChannelTemplateSource],
    module_agent_ids: &[String],
) -> Vec<ChannelTemplateRecord> {
    sources
        .iter()
        .map(|s| {
            let mut personas: Vec<String> = Vec::new();
            if s.include_module_agents {
                personas.extend(module_agent_ids.iter().cloned());
            }
            for p in &s.personas {
                if !personas.contains(p) {
                    personas.push(p.clone());
                }
            }
            // Deterministic ordering (AD-4): the store is committed and diffed.
            personas.sort();
            personas.dedup();

            ChannelTemplateRecord {
                name: format!("{module}-{}", s.name),
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
        // r##"…"## because the canvas content contains `"#`, which would close r#"…"# early.
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
    fn template_names_are_module_prefixed() {
        let store = build_store("tea", &sources(), &["bmad-tea".into()]);
        assert_eq!(store[0].name, "tea-test-strategy");
    }

    #[test]
    fn roster_is_filled_from_the_module_registry() {
        let store = build_store(
            "bmm",
            &sources(),
            &["bmad-agent-pm".into(), "bmad-agent-dev".into()],
        );
        let ids: Vec<_> = store[0]
            .agents
            .personas
            .iter()
            .map(|p| p.persona_id.as_str())
            .collect();
        // sorted, so the committed store does not churn on registry ordering
        assert_eq!(ids, vec!["bmad-agent-dev", "bmad-agent-pm"]);
    }

    #[test]
    fn opting_out_of_module_agents_leaves_the_roster_empty() {
        let mut s = sources();
        s[0].include_module_agents = false;
        let store = build_store("tea", &s, &["bmad-tea".into()]);
        assert!(store[0].agents.personas.is_empty());
    }

    #[test]
    fn explicit_personas_do_not_duplicate_module_agents() {
        let mut s = sources();
        s[0].personas = vec!["bmad-tea".into(), "outsider".into()];
        let store = build_store("tea", &s, &["bmad-tea".into()]);
        let ids: Vec<_> = store[0]
            .agents
            .personas
            .iter()
            .map(|p| p.persona_id.as_str())
            .collect();
        assert_eq!(ids, vec!["bmad-tea", "outsider"]);
    }

    #[test]
    fn wire_shape_matches_what_buzz_deserializes() {
        // Buzz reads snake_case at the top level and camelCase inside `agents`.
        // Getting this wrong yields a template that parses to defaults and silently
        // provisions the wrong thing.
        let store = build_store("tea", &sources(), &["bmad-tea".into()]);
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
        let a = render(&build_store("tea", &sources(), &["bmad-tea".into()])).unwrap();
        let b = render(&build_store("tea", &sources(), &["bmad-tea".into()])).unwrap();
        assert_eq!(a, b);
    }
}
