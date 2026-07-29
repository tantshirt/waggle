//! Emits Buzz workflow YAML for a method quality gate (FR-4, FR-19).
//!
//! The schema is `crates/buzz-workflow/src/schema.rs` at the pinned tag: a workflow has a
//! name, a tagged `trigger`, and ordered `steps` whose action is flattened onto the step.
//!
//! **We deliberately do not emit a `request_approval` step.** Upstream marks runs that
//! reach one as failed rather than suspended (UP-01), so a gate built on it would report
//! failure for every approval. Instead the reaction *is* the approval, and the workflow's
//! job is to write a signed gate record into the log — which is what AD-10 requires and
//! what FR-22 needs for log-only reconstruction.

use serde::Serialize;
use waggle_core::gate::{APPROVAL_EMOJI, GATE_NOTICE_MARKER};

#[derive(Debug, Serialize)]
pub struct WorkflowDef {
    pub name: String,
    pub description: String,
    pub trigger: Trigger,
    pub steps: Vec<Step>,
    pub enabled: bool,
}

#[derive(Debug, Serialize)]
#[serde(tag = "on", rename_all = "snake_case")]
pub enum Trigger {
    ReactionAdded {
        #[serde(skip_serializing_if = "Option::is_none")]
        emoji: Option<String>,
    },
}

#[derive(Debug, Serialize)]
pub struct Step {
    pub id: String,
    pub name: String,
    #[serde(flatten)]
    pub action: Action,
}

#[derive(Debug, Serialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum Action {
    SendMessage { text: String },
}

/// Build the gate workflow for a module.
///
/// The emitted name is stable across recompiles of unchanged input (AD-4/FR-4).
pub fn gate_workflow(module: &str) -> WorkflowDef {
    // Template variables are resolved by the engine at fire time:
    // trigger.message_id, trigger.author, trigger.timestamp, trigger.emoji.
    // NOT the gate record. UP-18: the relay signs workflow output with its own keypair,
    // and resolves {{trigger.author}} from an `actor` tag with no relay-pubkey guard, so
    // anything written here is both mis-attributed and forgeable. This notice exists only
    // to make the approval visible in the channel; the authoritative record is published
    // by `waggle gate`, signed by an agent identity, with the approver derived from the
    // reaction's signature-bound pubkey.
    let notice = format!(
        "{GATE_NOTICE_MARKER}\n\
         module: {module}\n\
         verdict-event: {{{{trigger.message_id}}}}\n\
         reaction: {{{{trigger.emoji}}}}\n\
         note: advisory only — this message is relay-signed and its author field is not \
         trustworthy. Run `waggle gate --verdict-event <id>` to produce the signed record."
    );

    WorkflowDef {
        name: format!("waggle-gate-{module}"),
        description: format!(
            "Release gate notice for the {module} module. A human reaction on a verdict \
             event posts an advisory notice. The authoritative, agent-signed gate record is \
             produced by `waggle gate`, which derives the approver from the reaction's \
             signature-bound pubkey and checks it against the relay-signed admin list."
        ),
        trigger: Trigger::ReactionAdded {
            emoji: Some(APPROVAL_EMOJI.to_string()),
        },
        steps: vec![Step {
            // Buzz validates step ids as alphanumeric + underscore only; a dash is
            // rejected at create time with "step id ... is invalid". Discovered against
            // the real engine, not the schema source.
            id: "publish_gate_record".to_string(),
            name: "Post an advisory approval notice (not the record — see UP-18)".to_string(),
            action: Action::SendMessage { text: notice },
        }],
        enabled: true,
    }
}

/// Render to YAML.
pub fn render(def: &WorkflowDef) -> Result<String, serde_json::Error> {
    // Round-tripped through JSON so the output uses exactly the field names Buzz's serde
    // tags produce, rather than anything a YAML serializer might normalize differently.
    let v: serde_json::Value = serde_json::to_value(def)?;
    Ok(json_to_yaml(&v, 0))
}

fn json_to_yaml(v: &serde_json::Value, indent: usize) -> String {
    let pad = "  ".repeat(indent);
    match v {
        serde_json::Value::Object(map) => map
            .iter()
            .map(|(k, val)| match val {
                serde_json::Value::Object(_) | serde_json::Value::Array(_) => {
                    format!("{pad}{k}:\n{}", json_to_yaml(val, indent + 1))
                }
                _ => format!("{pad}{k}: {}\n", scalar(val)),
            })
            .collect(),
        serde_json::Value::Array(items) => items
            .iter()
            .map(|item| match item {
                serde_json::Value::Object(map) => {
                    let mut it = map.iter();
                    let first = it
                        .next()
                        .map(|(k, val)| format!("{pad}- {k}: {}\n", scalar(val)))
                        .unwrap_or_default();
                    let rest: String = it
                        .map(|(k, val)| format!("{pad}  {k}: {}\n", scalar(val)))
                        .collect();
                    format!("{first}{rest}")
                }
                _ => format!("{pad}- {}\n", scalar(item)),
            })
            .collect(),
        _ => format!("{pad}{}\n", scalar(v)),
    }
}

/// Always block- or quote-encode scalars so multi-line text and colons survive.
fn scalar(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => {
            // JSON string escaping is valid YAML double-quoted style, including \n.
            serde_json::to_string(s).unwrap_or_else(|_| format!("{s:?}"))
        }
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Null => "null".to_string(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trigger_is_a_reaction_on_the_agreed_emoji() {
        let y = render(&gate_workflow("tea")).unwrap();
        assert!(y.contains("on: \"reaction_added\""), "got:\n{y}");
        assert!(y.contains(APPROVAL_EMOJI));
    }

    #[test]
    fn does_not_use_request_approval() {
        // UP-01: a request_approval step would mark every run failed.
        let y = render(&gate_workflow("tea")).unwrap();
        assert!(
            !y.contains("request_approval"),
            "gate must not depend on the broken upstream approval step"
        );
        assert!(y.contains("send_message"));
    }

    #[test]
    fn step_ids_satisfy_buzz_id_rules() {
        // Buzz rejects anything outside [A-Za-z0-9_] in a step id.
        for step in &gate_workflow("tea").steps {
            assert!(
                step.id
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_'),
                "step id {:?} would be rejected by the workflow engine",
                step.id
            );
        }
    }

    #[test]
    fn notice_cites_the_verdict_but_never_claims_an_approver() {
        // UP-18: {{trigger.author}} is spoofable via an unguarded `actor` tag, so the
        // workflow must not name an approver at all. Naming one would put a forgeable
        // claim into the log under the relay's signature.
        let y = render(&gate_workflow("tea")).unwrap();
        assert!(y.contains(GATE_NOTICE_MARKER));
        assert!(y.contains("{{trigger.message_id}}"));
        assert!(
            !y.contains("{{trigger.author}}"),
            "the workflow must not attribute an approver:\n{y}"
        );
        assert!(
            !y.contains(waggle_core::gate::GATE_RECORD_MARKER),
            "the workflow must not masquerade as the gate record:\n{y}"
        );
        assert!(
            y.contains("advisory"),
            "the notice must say it is advisory:\n{y}"
        );
    }

    #[test]
    fn workflow_name_is_stable_for_the_same_module() {
        assert_eq!(gate_workflow("tea").name, "waggle-gate-tea");
        assert_eq!(
            render(&gate_workflow("tea")).unwrap(),
            render(&gate_workflow("tea")).unwrap()
        );
    }
}
