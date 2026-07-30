//! `waggle gate` — verified verdict + roster reconciliation.

use std::process::ExitCode;

use serde::Serialize;

use crate::common::uuid_like;
use crate::exit;
use crate::output::{emit, Format};

#[derive(Serialize)]
struct GateReport {
    verdict_event: String,
    outcome: waggle_core::gate::GateOutcome,
    #[serde(skip_serializing_if = "Option::is_none")]
    record_event: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    record_signed_by: Option<String>,
    roster_size: usize,
    reactions_seen: usize,
    roster: Vec<RosterView>,
}

#[derive(Serialize)]
struct RosterView {
    pubkey: String,
    role: String,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn run_gate(
    root: &std::path::Path,
    role: &str,
    channel: &str,
    verdict_event: &str,
    verdict: &str,
    dry_run: bool,
    relay: &str,
    format: Format,
) -> ExitCode {
    let verdict: waggle_core::Verdict = match verdict.parse() {
        Ok(v) => v,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(exit::USER);
        }
    };

    let verdict = match waggle_hive::events::prove_verdict_claim(
        root,
        role,
        relay,
        verdict_event,
        verdict,
        &uuid_like(),
    ) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(exit::UPSTREAM);
        }
    };

    let reactions = match waggle_hive::events::fetch_reactions(
        root,
        role,
        relay,
        verdict_event,
        &uuid_like(),
    ) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(exit::UPSTREAM);
        }
    };

    let roster = match waggle_hive::events::fetch_roster(root, role, relay, channel, &uuid_like()) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(exit::UPSTREAM);
        }
    };

    let outcome = waggle_core::gate::reconcile(verdict, &reactions, &roster);

    let mut record_event = None;
    let mut record_signed_by = None;

    if !dry_run {
        if let Some(body) = waggle_core::gate::render_gate_record(&outcome, verdict_event) {
            let record = waggle_core::ArtifactEvent {
                kind_marker: waggle_core::ArtifactKind::GateRecord,
                channel_id: channel.to_string(),
                artifact_type: Some("gate-record".into()),
                module: None,
                story: None,
                priority: None,
                references: vec![verdict_event.to_string()],
                from_role: Some(role.to_string()),
                to_role: None,
                body,
            };
            match waggle_hive::events::publish_artifact(root, role, relay, &record, &uuid_like()) {
                Ok(p) => {
                    record_event = Some(p.event_id);
                    record_signed_by = Some(p.pubkey);
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    return ExitCode::from(exit::UPSTREAM);
                }
            }
        }
    }

    let report = GateReport {
        verdict_event: verdict_event.to_string(),
        outcome,
        record_event,
        record_signed_by,
        roster_size: roster.len(),
        reactions_seen: reactions.len(),
        roster: roster
            .iter()
            .map(|r| RosterView {
                pubkey: r.pubkey.clone(),
                role: format!("{:?}", r.role).to_lowercase(),
            })
            .collect(),
    };

    match format {
        Format::Json => emit(format, "gate", true, &report),
        Format::Text => match &report.outcome {
            waggle_core::gate::GateOutcome::Approved {
                verdict, approver, ..
            } => {
                println!("{verdict} approved by {approver}");
                match &report.record_event {
                    Some(id) => println!(
                        "record {id}
  signed by {} (waggle identity, not the relay)",
                        report.record_signed_by.clone().unwrap_or_default()
                    ),
                    None => println!("(dry run — no record published)"),
                }
            }
            waggle_core::gate::GateOutcome::Unauthorized {
                verdict,
                attempted_by,
                required,
            } => {
                println!(
                    "{verdict} NOT approved — {} reaction(s) from identities below {required:?}",
                    attempted_by.len()
                );
                for a in attempted_by {
                    println!("  unauthorized: {a}");
                }
            }
            waggle_core::gate::GateOutcome::Pending { verdict } => {
                println!("{verdict} pending — no approving reaction yet");
            }
        },
    }

    ExitCode::from(exit::OK)
}
