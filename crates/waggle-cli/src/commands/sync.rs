//! `waggle sync` — fail-closed hive bootstrap.

use std::process::ExitCode;

use crate::common::{role_for_agent, uuid_like};
use crate::exit;
use crate::output::{emit, Format};
use crate::{run_compile_all, run_provision_all_opts};

#[allow(clippy::too_many_arguments)]
pub(crate) fn run_sync(
    root: &std::path::Path,
    bmad_version: Option<&str>,
    modules: Option<&str>,
    skip_install: bool,
    allow_unsupported: bool,
    relay: &str,
    offline: bool,
    buzz_cli: &std::path::Path,
    human_pubkey: Option<&str>,
    skip_global_skills: bool,
    refresh: bool,
    format: Format,
) -> ExitCode {
    let mut steps: Vec<serde_json::Value> = Vec::new();
    let fail = |steps: Vec<serde_json::Value>, msg: String, code: u8| -> ExitCode {
        eprintln!("error: {msg}");
        if matches!(format, Format::Json) {
            emit(
                format,
                "sync",
                false,
                &serde_json::json!({
                    "error": msg,
                    "steps": steps,
                }),
            );
        }
        ExitCode::from(code)
    };

    let pins_raw = match std::fs::read_to_string(root.join("BUZZ_VERSION")) {
        Ok(s) => s,
        Err(e) => {
            return fail(
                steps,
                format!("cannot read BUZZ_VERSION: {e}"),
                exit::SYSTEM,
            );
        }
    };
    let pins = waggle_core::pins::parse_pins(&pins_raw);
    let version = bmad_version
        .or_else(|| pins.get("BMAD_METHOD_VERSION").map(String::as_str))
        .unwrap_or("6.10.0")
        .to_string();
    let module_list = modules
        .or_else(|| pins.get("BMAD_MODULES").map(String::as_str))
        .unwrap_or("bmm,bmb,tea,cis,gds,wds")
        .to_string();

    if let Some(range) = waggle_core::pins::range(&pins, "BMAD_SUPPORTED") {
        if let Some(v) = waggle_core::Version::parse(&version) {
            if !range.check(v).is_supported() && !allow_unsupported {
                return fail(
                    steps,
                    format!(
                        "BMAD {version} is outside BMAD_SUPPORTED ({range}). \
                         Pass --allow-unsupported to override."
                    ),
                    exit::USER,
                );
            }
        }
    }

    if !skip_install {
        if matches!(format, Format::Text) {
            eprintln!("sync: installing bmad-method@{version} modules={module_list}");
        }
        let output = std::process::Command::new("npx")
            .args([
                "--yes",
                &format!("bmad-method@{version}"),
                "install",
                "--directory",
                &root.display().to_string(),
                "--modules",
                &module_list,
                "--tools",
                "claude-code",
                "--all-stable",
                "--action",
                "update",
                "--yes",
            ])
            .current_dir(root)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output();
        match output {
            Ok(o) if o.status.success() => {
                let err = String::from_utf8_lossy(&o.stderr);
                if !err.trim().is_empty() {
                    eprint!("{err}");
                }
                steps.push(serde_json::json!({"step":"install","ok":true}));
            }
            Ok(o) => {
                eprint!("{}", String::from_utf8_lossy(&o.stderr));
                return fail(
                    steps,
                    format!("bmad-method install exited {}", o.status),
                    exit::UPSTREAM,
                );
            }
            Err(e) => {
                return fail(
                    steps,
                    format!("could not run npx bmad-method: {e}"),
                    exit::SYSTEM,
                );
            }
        }
    }

    let out_dir = root.join("packs");
    // Nested compile must not emit its own JSON envelope when sync owns stdout.
    let compile_format = match format {
        Format::Json => Format::Text,
        Format::Text => Format::Text,
    };
    // Sync / CI: missing skills are hard errors (no --allow-missing-skills).
    let compile_code = run_compile_all(root, &out_dir, compile_format, false);
    if compile_code != ExitCode::from(exit::OK) {
        return fail(steps, "compile failed".into(), exit::UPSTREAM);
    }
    steps.push(serde_json::json!({"step":"compile","ok":true}));

    if !skip_global_skills {
        let project_skills = root.join(".claude").join("skills");
        let target = waggle_hive::skills::global_skills_home();
        match waggle_hive::skills::publish_global(&project_skills, &target) {
            Ok(report) => {
                if matches!(format, Format::Text) {
                    eprintln!(
                        "global skills {} linked, {} skipped, {} removed → {}",
                        report.linked.len(),
                        report.skipped.len(),
                        report.removed.len(),
                        report.target_dir.display()
                    );
                }
                steps.push(serde_json::json!({
                    "step":"global_skills","ok":true,
                    "linked": report.linked.len(),
                    "skipped": report.skipped.len(),
                }));
            }
            Err(e) => {
                return fail(steps, format!("global skills publish: {e}"), exit::SYSTEM);
            }
        }
    }

    let registry = match waggle_method::registry::read(root) {
        Ok(r) => r,
        Err(e) => {
            return fail(steps, e.to_string(), exit::UPSTREAM);
        }
    };

    let mut agents_ok = 0usize;
    for (agent_id, agent) in &registry {
        let role = role_for_agent(agent_id);
        match waggle_hive::identity::provision(root, &role, false) {
            Ok(_) | Err(waggle_hive::IdentityError::AlreadyExists { .. }) => {}
            Err(e) => {
                return fail(steps, format!("identity {role}: {e}"), exit::SYSTEM);
            }
        }
        let pack = root.join("packs").join(&agent.module);
        if let Err(e) = waggle_hive::runtime::emit_config(
            root,
            &role,
            &pack,
            agent_id,
            relay,
            waggle_hive::runtime::DEFAULT_MAX_SESSIONS,
        ) {
            return fail(steps, format!("runtime emit {role}: {e}"), exit::SYSTEM);
        }
        if !offline {
            let admin = root.join("vendor/buzz/target/debug/buzz-admin");
            let admin = if admin.exists() {
                admin
            } else {
                root.join("vendor/buzz/target/release/buzz-admin")
            };
            if admin.exists() {
                if let Err(e) =
                    waggle_hive::identity::register_member(root, &role, &admin, "member")
                {
                    return fail(
                        steps,
                        format!("register_member {role}: {e}"),
                        exit::UPSTREAM,
                    );
                }
            }
            let persona_path = pack.join("agents").join(format!("{agent_id}.persona.md"));
            if persona_path.is_file() {
                let persona = match waggle_hive::runtime::read_pack_persona_file(&persona_path) {
                    Ok(p) => p,
                    Err(e) => {
                        return fail(steps, format!("persona {agent_id}: {e}"), exit::USER);
                    }
                };
                if let Err(e) = waggle_hive::runtime::publish_persona_and_agent(
                    root,
                    &role,
                    &pack,
                    agent_id,
                    relay,
                    waggle_hive::runtime::DEFAULT_MAX_SESSIONS,
                    "anyone",
                    &uuid_like(),
                ) {
                    return fail(
                        steps,
                        format!("publish agent {agent_id}: {e}"),
                        exit::UPSTREAM,
                    );
                }
                let secret_path = root.join("keys").join(format!("{role}.nsec"));
                if let Ok(secret) = std::fs::read_to_string(&secret_path) {
                    let status = std::process::Command::new(buzz_cli)
                        .env("BUZZ_PRIVATE_KEY", secret.trim())
                        .env("BUZZ_RELAY_URL", relay)
                        .args([
                            "users",
                            "set-profile",
                            "--name",
                            &persona.display_name,
                            "--about",
                            &persona.description,
                        ])
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::piped())
                        .status();
                    match status {
                        Ok(s) if s.success() => {}
                        Ok(s) => {
                            return fail(
                                steps,
                                format!("set-profile {role} exited {s}"),
                                exit::UPSTREAM,
                            );
                        }
                        Err(e) => {
                            return fail(steps, format!("set-profile {role}: {e}"), exit::SYSTEM);
                        }
                    }
                }
            }
        }
        agents_ok += 1;
    }
    steps.push(serde_json::json!({
        "step":"agents","ok":true,"count": agents_ok
    }));

    if !offline {
        let provision_code = run_provision_all_opts(
            root,
            "tea",
            relay,
            buzz_cli,
            human_pubkey,
            refresh,
            Format::Text,
            false, // fold into sync envelope — do not emit nested JSON
        );
        if provision_code != ExitCode::from(exit::OK) {
            return fail(steps, "channel provision failed".into(), exit::UPSTREAM);
        }
        steps.push(serde_json::json!({"step":"provision","ok":true}));
    }

    let content_hash = {
        use sha2::{Digest, Sha256};
        let mut h = Sha256::new();
        h.update(version.as_bytes());
        h.update(module_list.as_bytes());
        h.update(agents_ok.to_string().as_bytes());
        format!("{:x}", h.finalize())
    };

    let state = serde_json::json!({
        "bmad_method_version": version,
        "modules_requested": module_list,
        "content_hash": content_hash,
        "steps": steps,
        "agents_provisioned": agents_ok,
    });
    let custom = root.join("_bmad").join("custom");
    if let Err(e) = std::fs::create_dir_all(&custom) {
        return fail(steps, format!("sync state dir: {e}"), exit::SYSTEM);
    }
    if let Err(e) = std::fs::write(
        custom.join("waggle-sync-state.json"),
        serde_json::to_string_pretty(&state).unwrap_or_default() + "\n",
    ) {
        return fail(steps, format!("sync state write: {e}"), exit::SYSTEM);
    }

    match format {
        Format::Text => {
            println!("sync complete — restart: waggle runtime supervisor");
        }
        Format::Json => emit(format, "sync", true, &state),
    }
    ExitCode::from(exit::OK)
}
