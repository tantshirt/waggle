//! The compile transform: a resolved BMAD descriptor becomes a Buzz persona pack.
//!
//! **AD-4: pure.** No clock, no environment, no paths, no randomness — the same descriptor
//! always produces the same pack, byte for byte, so generated config can be committed and
//! reviewed in diffs.
//!
//! **AD-6: nothing is dropped silently.** Every field of the descriptor is mapped, carried
//! into the persona body, or explicitly reported as dropped with a reason. A field we do
//! not recognize is reported as *unknown* rather than ignored — that is the difference
//! between "waggle doesn't support this yet" and "your agent quietly lost a capability".
//!
//! **AD-7: menu items are a sum type.** A dispatchable item names a skill; a prompt item
//! carries instruction text and produces no workflow. Both are normal.
//!
//! **AD-16: no module-specific branches.** Nothing here may test for a module id.

use std::collections::BTreeSet;

use serde::Serialize;
use toml::Value;

/// Descriptor keys waggle understands. Anything outside this set is reported as unknown
/// (AD-6) rather than silently discarded.
const KNOWN_KEYS: [&str; 10] = [
    "name",
    "title",
    "icon",
    "role",
    "identity",
    "communication_style",
    "principles",
    "persistent_facts",
    "menu",
    "activation_steps_prepend",
];

/// One capability the agent exposes. AD-7's sum type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MenuItem {
    /// Names a workflow skill. Compiles to a pack skill entry.
    Dispatch {
        code: String,
        description: String,
        skill: String,
    },
    /// Carries instruction text. Produces **no** skill and no workflow — it becomes part
    /// of the persona body. TEA's `GATE` is one of these.
    Prompt {
        code: String,
        description: String,
        prompt: String,
    },
}

impl MenuItem {
    pub fn code(&self) -> &str {
        match self {
            MenuItem::Dispatch { code, .. } | MenuItem::Prompt { code, .. } => code,
        }
    }
}

/// The compiled persona, before rendering to files.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PersonaPack {
    /// Machine name — the BMAD agent id, passed through unchanged (AD-3: never re-derive
    /// an id the method already owns).
    pub name: String,
    /// `"<Name> <icon>"`, e.g. `Murat 🧪`.
    pub display_name: String,
    pub description: String,
    pub role: Option<String>,
    pub identity: Option<String>,
    pub communication_style: Option<String>,
    pub principles: Vec<String>,
    /// Preserved as *references*, never inlined — facts must stay current as the
    /// repository changes.
    pub persistent_facts: Vec<String>,
    pub menu: Vec<MenuItem>,
}

impl PersonaPack {
    /// Skill ids this persona needs, in menu order, deduplicated.
    pub fn skill_ids(&self) -> Vec<String> {
        let mut seen = BTreeSet::new();
        self.menu
            .iter()
            .filter_map(|m| match m {
                MenuItem::Dispatch { skill, .. } => Some(skill.clone()),
                MenuItem::Prompt { .. } => None,
            })
            .filter(|s| seen.insert(s.clone()))
            .collect()
    }
}

/// What a compile did, so a human can trust it without reading the output (AD-6, FR-6).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct CompileReport {
    pub agent_id: String,
    /// Descriptor keys mapped into the pack.
    pub mapped: Vec<String>,
    /// Menu item codes carried into the persona body instead of becoming skills (AD-7).
    pub prompt_only: Vec<String>,
    /// Keys present in the descriptor that waggle does not understand.
    pub unknown: Vec<String>,
    /// Keys understood but deliberately not carried, with the reason.
    pub dropped: Vec<Dropped>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Dropped {
    pub field: String,
    pub reason: String,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum CompileError {
    #[error("descriptor for {agent_id} is not a table")]
    NotATable { agent_id: String },

    #[error("descriptor for {agent_id} is missing required field {field:?}")]
    MissingField { agent_id: String, field: String },

    #[error("menu item {code:?} in {agent_id} has neither `skill` nor `prompt` — every item must have exactly one")]
    MenuItemUnclassifiable { agent_id: String, code: String },

    #[error("menu item {code:?} in {agent_id} has BOTH `skill` and `prompt` — every item must have exactly one")]
    MenuItemAmbiguous { agent_id: String, code: String },
}

/// Compile a resolved `[agent]` descriptor into a persona pack plus a report.
///
/// `agent_id` is the BMAD id (e.g. `bmad-tea`); `description` comes from the installation
/// registry, which holds it separately from the customize block.
pub fn compile_persona(
    agent_id: &str,
    descriptor: &Value,
    description: &str,
) -> Result<(PersonaPack, CompileReport), CompileError> {
    let table = descriptor.as_table().ok_or(CompileError::NotATable {
        agent_id: agent_id.to_string(),
    })?;

    let mut report = CompileReport {
        agent_id: agent_id.to_string(),
        ..Default::default()
    };

    // AD-6: account for every key present, before doing anything else.
    for key in table.keys() {
        if KNOWN_KEYS.contains(&key.as_str()) {
            report.mapped.push(key.clone());
        } else if key == "activation_steps_append" {
            report.dropped.push(Dropped {
                field: key.clone(),
                reason: "no persona-pack equivalent; Buzz lifecycle hooks are parsed but \
                         not executed at the pinned version"
                    .to_string(),
            });
        } else {
            report.unknown.push(key.clone());
        }
    }
    report.mapped.sort();
    report.unknown.sort();

    let name = str_field(table, "name").ok_or_else(|| CompileError::MissingField {
        agent_id: agent_id.to_string(),
        field: "name".into(),
    })?;
    let icon = str_field(table, "icon");

    // Buzz requires `display_name`; BMAD splits it across name + icon.
    let display_name = match &icon {
        Some(i) if !i.is_empty() => format!("{name} {i}"),
        _ => name.clone(),
    };

    let principles = str_array(table, "principles");
    let persistent_facts = str_array(table, "persistent_facts");

    if principles.is_empty() {
        report
            .warnings
            .push("descriptor declares no principles".to_string());
    }

    let menu = compile_menu(agent_id, table, &mut report)?;

    Ok((
        PersonaPack {
            // AD-3: the method owns this id.
            name: agent_id.to_string(),
            display_name,
            description: description.to_string(),
            role: str_field(table, "role"),
            identity: str_field(table, "identity"),
            communication_style: str_field(table, "communication_style"),
            principles,
            persistent_facts,
            menu,
        },
        report,
    ))
}

fn compile_menu(
    agent_id: &str,
    table: &toml::Table,
    report: &mut CompileReport,
) -> Result<Vec<MenuItem>, CompileError> {
    let Some(items) = table.get("menu").and_then(Value::as_array) else {
        return Ok(Vec::new());
    };

    let mut out = Vec::with_capacity(items.len());
    for item in items {
        let code = item
            .get("code")
            .and_then(Value::as_str)
            .unwrap_or("<uncoded>")
            .to_string();
        let description = item
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();

        let skill = item.get("skill").and_then(Value::as_str);
        let prompt = item.get("prompt").and_then(Value::as_str);

        match (skill, prompt) {
            (Some(_), Some(_)) => {
                return Err(CompileError::MenuItemAmbiguous {
                    agent_id: agent_id.to_string(),
                    code,
                })
            }
            (None, None) => {
                return Err(CompileError::MenuItemUnclassifiable {
                    agent_id: agent_id.to_string(),
                    code,
                })
            }
            (Some(skill), None) => out.push(MenuItem::Dispatch {
                code,
                description,
                skill: skill.to_string(),
            }),
            (None, Some(prompt)) => {
                // AD-7: normal control flow, not an error path.
                report.prompt_only.push(code.clone());
                out.push(MenuItem::Prompt {
                    code,
                    description,
                    prompt: prompt.to_string(),
                });
            }
        }
    }
    Ok(out)
}

fn str_field(table: &toml::Table, key: &str) -> Option<String> {
    table.get(key).and_then(Value::as_str).map(str::to_string)
}

fn str_array(table: &toml::Table, key: &str) -> Vec<String> {
    table
        .get(key)
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn descriptor(s: &str) -> Value {
        toml::from_str::<Value>(s).unwrap()
    }

    const TEA_LIKE: &str = r#"
name = "Murat"
title = "Master Test Architect"
icon = "🧪"
role = "Test architect"
identity = "Specializes in risk-based testing"
communication_style = "Strong opinions, weakly held"
principles = ["Risk-based testing.", "Gates backed by data."]
persistent_facts = ["file:{project-root}/**/project-context.md"]

[[menu]]
code = "TD"
description = "Test Design"
skill = "bmad-testarch-test-design"

[[menu]]
code = "GATE"
description = "Release Gate"
prompt = "Route the release gate path."
"#;

    #[test]
    fn maps_identity_and_builds_display_name_from_name_plus_icon() {
        let (pack, _) = compile_persona("bmad-tea", &descriptor(TEA_LIKE), "desc").unwrap();
        assert_eq!(pack.name, "bmad-tea", "the method's id passes through");
        assert_eq!(pack.display_name, "Murat 🧪");
        assert_eq!(
            pack.communication_style.as_deref(),
            Some("Strong opinions, weakly held")
        );
        assert_eq!(pack.principles.len(), 2);
    }

    #[test]
    fn menu_splits_into_dispatch_and_prompt() {
        let (pack, report) = compile_persona("bmad-tea", &descriptor(TEA_LIKE), "d").unwrap();
        assert_eq!(pack.menu.len(), 2);
        assert!(matches!(pack.menu[0], MenuItem::Dispatch { .. }));
        assert!(matches!(pack.menu[1], MenuItem::Prompt { .. }));
        // AD-7: the prompt item must be reported, not silently absorbed.
        assert_eq!(report.prompt_only, vec!["GATE"]);
        // and it must NOT become a skill
        assert_eq!(pack.skill_ids(), vec!["bmad-testarch-test-design"]);
    }

    #[test]
    fn persistent_facts_stay_references_and_are_not_inlined() {
        let (pack, _) = compile_persona("bmad-tea", &descriptor(TEA_LIKE), "d").unwrap();
        assert_eq!(
            pack.persistent_facts,
            vec!["file:{project-root}/**/project-context.md"],
            "facts must stay references so they track the repo"
        );
    }

    #[test]
    fn unknown_keys_are_reported_not_ignored() {
        // Prepend, not append: a key written after `[[menu]]` would land *inside* the
        // last table rather than at the document root.
        let d = descriptor(&format!("some_new_bmad_field = \"x\"\n{TEA_LIKE}"));
        let (_, report) = compile_persona("bmad-tea", &d, "d").unwrap();
        assert_eq!(report.unknown, vec!["some_new_bmad_field"]);
    }

    #[test]
    fn deliberately_dropped_fields_carry_a_reason() {
        let d = descriptor(&format!("activation_steps_append = [\"x\"]\n{TEA_LIKE}"));
        let (_, report) = compile_persona("bmad-tea", &d, "d").unwrap();
        assert_eq!(report.dropped.len(), 1);
        assert_eq!(report.dropped[0].field, "activation_steps_append");
        assert!(
            !report.dropped[0].reason.is_empty(),
            "a drop needs a reason"
        );
    }

    #[test]
    fn a_menu_item_with_both_skill_and_prompt_is_an_error() {
        let d = descriptor(
            r#"
name = "X"
[[menu]]
code = "BAD"
skill = "s"
prompt = "p"
"#,
        );
        assert_eq!(
            compile_persona("a", &d, "d").unwrap_err(),
            CompileError::MenuItemAmbiguous {
                agent_id: "a".into(),
                code: "BAD".into()
            }
        );
    }

    #[test]
    fn a_menu_item_with_neither_is_an_error() {
        let d = descriptor(
            r#"
name = "X"
[[menu]]
code = "BAD"
description = "nothing to dispatch"
"#,
        );
        assert!(matches!(
            compile_persona("a", &d, "d").unwrap_err(),
            CompileError::MenuItemUnclassifiable { .. }
        ));
    }

    #[test]
    fn missing_name_is_an_error_not_a_default() {
        let d = descriptor(r#"icon = "🧪""#);
        assert!(matches!(
            compile_persona("a", &d, "d").unwrap_err(),
            CompileError::MissingField { .. }
        ));
    }

    #[test]
    fn compile_is_deterministic() {
        // AD-4: same input, identical output, including collection ordering.
        let d = descriptor(TEA_LIKE);
        let a = compile_persona("bmad-tea", &d, "d").unwrap();
        let b = compile_persona("bmad-tea", &d, "d").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn an_agent_with_no_icon_still_gets_a_display_name() {
        let d = descriptor(r#"name = "Plain""#);
        let (pack, report) = compile_persona("a", &d, "d").unwrap();
        assert_eq!(pack.display_name, "Plain");
        assert!(report.warnings.iter().any(|w| w.contains("principles")));
    }
}
