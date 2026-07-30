//! Channel provisioning (FR-10, FR-25, FR-26).
//!
//! **AD-2:** provisioning goes through `buzz channels create --templates-file`, a published
//! substrate interface. waggle does not reimplement channel or canvas creation — the
//! substrate already does both, verified in `research-notes.md` §8.
//!
//! **What waggle must add: idempotence.** Creating twice with the same name yields two
//! channels upstream (UP-10), while FR-25 requires no duplicates and NFR-2 requires
//! idempotence generally. So this module checks before creating.
//!
//! The check is inherently racy against a concurrent creator. That is acceptable for a
//! provisioning command run by an operator, and the honest alternative needs relay-side
//! support — which is why UP-10 proposes it upstream rather than pretending we solved it.

use std::path::Path;
use std::process::Command;

#[derive(Debug, thiserror::Error)]
pub enum ChannelError {
    #[error("could not run the substrate CLI at {path}: {source}")]
    CliUnavailable {
        path: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("substrate CLI failed listing channels: {0}")]
    ListFailed(String),

    #[error("substrate CLI failed creating channel {name:?} from template {template:?}: {stderr}")]
    CreateFailed {
        name: String,
        template: String,
        stderr: String,
    },

    #[error("could not parse the substrate CLI response: {0}")]
    Unparseable(String),
}

/// What happened to one channel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Provisioned {
    /// Newly created; carries the channel id and whether its canvas was applied.
    Created { id: String, canvas_applied: bool },
    /// Already present, left alone — the idempotent path (FR-25, NFR-2).
    AlreadyExists { id: String },
    /// Already present; description and/or canvas were rewritten from the template.
    Refreshed {
        id: String,
        description_updated: bool,
        canvas_applied: bool,
    },
}

/// Result of refreshing an existing channel's description / canvas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefreshResult {
    pub description_updated: bool,
    pub canvas_applied: bool,
}

/// Names of channels that already exist, lowercased for case-insensitive comparison.
///
/// Buzz matches template names case-insensitively, so channel-name comparison follows.
pub fn existing_channel_names(
    buzz_cli: &Path,
    relay_url: &str,
    secret_hex: &str,
) -> Result<Vec<(String, String)>, ChannelError> {
    let out = Command::new(buzz_cli)
        .env("BUZZ_PRIVATE_KEY", secret_hex)
        .env("BUZZ_RELAY_URL", relay_url)
        .args(["--format", "compact", "channels", "list"])
        .output()
        .map_err(|source| ChannelError::CliUnavailable {
            path: buzz_cli.to_path_buf(),
            source,
        })?;

    if !out.status.success() {
        return Err(ChannelError::ListFailed(
            String::from_utf8_lossy(&out.stderr).trim().to_string(),
        ));
    }

    let parsed: serde_json::Value = serde_json::from_slice(&out.stdout)
        .map_err(|e| ChannelError::Unparseable(e.to_string()))?;

    Ok(parsed
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|c| {
                    let name = c.get("name")?.as_str()?.to_ascii_lowercase();
                    let id = c
                        .get("id")
                        .or_else(|| c.get("channel_id"))
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string();
                    Some((name, id))
                })
                .collect()
        })
        .unwrap_or_default())
}

/// Create one channel from a template, unless a channel of that name already exists.
pub fn provision_channel(
    buzz_cli: &Path,
    relay_url: &str,
    secret_hex: &str,
    templates_file: &Path,
    template_name: &str,
    channel_name: &str,
    existing: &[(String, String)],
) -> Result<Provisioned, ChannelError> {
    if let Some((_, id)) = existing
        .iter()
        .find(|(n, _)| n == &channel_name.to_ascii_lowercase())
    {
        return Ok(Provisioned::AlreadyExists { id: id.clone() });
    }

    let out = Command::new(buzz_cli)
        .env("BUZZ_PRIVATE_KEY", secret_hex)
        .env("BUZZ_RELAY_URL", relay_url)
        .args(["channels", "create", "--name", channel_name])
        .args(["--template", template_name])
        .arg("--templates-file")
        .arg(templates_file)
        .output()
        .map_err(|source| ChannelError::CliUnavailable {
            path: buzz_cli.to_path_buf(),
            source,
        })?;

    if !out.status.success() {
        return Err(ChannelError::CreateFailed {
            name: channel_name.to_string(),
            template: template_name.to_string(),
            stderr: String::from_utf8_lossy(&out.stderr).trim().to_string(),
        });
    }

    // The CLI may emit warnings on stdout before the result; take the last JSON object.
    let stdout = String::from_utf8_lossy(&out.stdout);
    let last = stdout
        .lines()
        .rev()
        .find(|l| l.trim_start().starts_with('{'))
        .unwrap_or_default();
    let parsed: serde_json::Value =
        serde_json::from_str(last).map_err(|e| ChannelError::Unparseable(e.to_string()))?;

    Ok(Provisioned::Created {
        id: parsed
            .get("channel_id")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        canvas_applied: parsed
            .get("canvas_applied")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
    })
}

/// Rewrite description and/or canvas on an existing channel (Buzz `channels update` + `canvas set`).
///
/// Used when templates change after first provision — create-only idempotence otherwise leaves
/// Desktop showing stale copy.
pub fn refresh_channel(
    buzz_cli: &Path,
    relay_url: &str,
    secret_hex: &str,
    channel_id: &str,
    description: Option<&str>,
    canvas: Option<&str>,
) -> Result<RefreshResult, ChannelError> {
    let mut description_updated = false;
    let mut canvas_applied = false;

    if let Some(desc) = description {
        let out = Command::new(buzz_cli)
            .env("BUZZ_PRIVATE_KEY", secret_hex)
            .env("BUZZ_RELAY_URL", relay_url)
            .args([
                "channels",
                "update",
                "--channel",
                channel_id,
                "--description",
                desc,
            ])
            .output()
            .map_err(|source| ChannelError::CliUnavailable {
                path: buzz_cli.to_path_buf(),
                source,
            })?;
        if !out.status.success() {
            return Err(ChannelError::CreateFailed {
                name: channel_id.to_string(),
                template: "channels update".into(),
                stderr: String::from_utf8_lossy(&out.stderr).trim().to_string(),
            });
        }
        description_updated = true;
    }

    if let Some(content) = canvas {
        // `--content -` reads markdown from stdin (avoids huge argv / quoting issues).
        let mut child = Command::new(buzz_cli)
            .env("BUZZ_PRIVATE_KEY", secret_hex)
            .env("BUZZ_RELAY_URL", relay_url)
            .args(["canvas", "set", "--channel", channel_id, "--content", "-"])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|source| ChannelError::CliUnavailable {
                path: buzz_cli.to_path_buf(),
                source,
            })?;
        if let Some(mut stdin) = child.stdin.take() {
            use std::io::Write;
            stdin
                .write_all(content.as_bytes())
                .map_err(|e| ChannelError::CreateFailed {
                    name: channel_id.to_string(),
                    template: "canvas set".into(),
                    stderr: e.to_string(),
                })?;
        }
        let out = child.wait_with_output().map_err(|e| ChannelError::CreateFailed {
            name: channel_id.to_string(),
            template: "canvas set".into(),
            stderr: e.to_string(),
        })?;
        if !out.status.success() {
            return Err(ChannelError::CreateFailed {
                name: channel_id.to_string(),
                template: "canvas set".into(),
                stderr: String::from_utf8_lossy(&out.stderr).trim().to_string(),
            });
        }
        canvas_applied = true;
    }

    Ok(RefreshResult {
        description_updated,
        canvas_applied,
    })
}

/// Add a pubkey to a channel (idempotent best-effort). Returns Ok even when already a member.
pub fn add_member(
    buzz_cli: &Path,
    relay_url: &str,
    secret_hex: &str,
    channel_id: &str,
    pubkey: &str,
    role: &str,
) -> Result<(), ChannelError> {
    let out = Command::new(buzz_cli)
        .env("BUZZ_PRIVATE_KEY", secret_hex)
        .env("BUZZ_RELAY_URL", relay_url)
        .args([
            "channels",
            "add-member",
            "--channel",
            channel_id,
            "--pubkey",
            pubkey,
            "--role",
            role,
        ])
        .output()
        .map_err(|source| ChannelError::CliUnavailable {
            path: buzz_cli.to_path_buf(),
            source,
        })?;

    if out.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&out.stderr).to_ascii_lowercase();
    let stdout = String::from_utf8_lossy(&out.stdout).to_ascii_lowercase();
    let combined = format!("{stdout}{stderr}");
    // Already-a-member / duplicate paths vary by substrate version.
    if combined.contains("already")
        || combined.contains("exists")
        || combined.contains("duplicate")
        || combined.contains("missing p tag")
    {
        return Ok(());
    }
    Err(ChannelError::CreateFailed {
        name: channel_id.to_string(),
        template: "add-member".into(),
        stderr: String::from_utf8_lossy(&out.stderr).trim().to_string(),
    })
}

/// Ensure every pubkey is a member of the channel. Reports failures without aborting.
pub fn ensure_members(
    buzz_cli: &Path,
    relay_url: &str,
    secret_hex: &str,
    channel_id: &str,
    pubkeys: &[String],
) -> Vec<(String, String)> {
    let mut failed = Vec::new();
    for pk in pubkeys {
        if let Err(e) = add_member(buzz_cli, relay_url, secret_hex, channel_id, pk, "member") {
            failed.push((pk.clone(), e.to_string()));
        }
    }
    failed
}

/// Personas the substrate could not resolve to a live agent, from a create response.
///
/// Surfaced rather than swallowed (AD-6): "no live instances" is the expected state until
/// agent runtimes exist, and an operator should see which agents did not join.
pub fn skipped_personas(create_stdout: &str) -> Vec<(String, String)> {
    let Some(last) = create_stdout
        .lines()
        .rev()
        .find(|l| l.trim_start().starts_with('{'))
    else {
        return Vec::new();
    };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(last) else {
        return Vec::new();
    };
    v.get("skipped")
        .and_then(|s| s.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|s| {
                    Some((
                        s.get("persona_id")?.as_str()?.to_string(),
                        s.get("reason")
                            .and_then(|r| r.as_str())
                            .unwrap_or("unknown")
                            .to_string(),
                    ))
                })
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn existing_channel_short_circuits_creation() {
        // No CLI is invoked at all when the name is taken — that is the idempotence
        // guarantee, and it must not depend on the substrate behaving well.
        let existing = vec![("story-42".to_string(), "abc-123".to_string())];
        let got = provision_channel(
            Path::new("/nonexistent/buzz"),
            "http://localhost:1",
            "deadbeef",
            Path::new("/nonexistent/templates.json"),
            "bmm-story",
            "story-42",
            &existing,
        )
        .expect("must not touch the CLI for an existing channel");
        assert_eq!(
            got,
            Provisioned::AlreadyExists {
                id: "abc-123".into()
            }
        );
    }

    #[test]
    fn name_comparison_is_case_insensitive() {
        let existing = vec![("story-42".to_string(), "abc".to_string())];
        let got = provision_channel(
            Path::new("/nonexistent/buzz"),
            "http://localhost:1",
            "deadbeef",
            Path::new("/nope.json"),
            "bmm-story",
            "STORY-42",
            &existing,
        )
        .unwrap();
        assert!(matches!(got, Provisioned::AlreadyExists { .. }));
    }

    #[test]
    fn skipped_personas_are_extracted_with_reasons() {
        let stdout = r#"{"warning":"noise"}
{"status":"ok","channel_id":"x","skipped":[{"persona_id":"bmad-tea","reason":"no live instances"}]}"#;
        assert_eq!(
            skipped_personas(stdout),
            vec![("bmad-tea".to_string(), "no live instances".to_string())]
        );
    }

    #[test]
    fn skipped_personas_tolerates_absent_or_malformed_output() {
        assert!(skipped_personas("").is_empty());
        assert!(skipped_personas("not json").is_empty());
        assert!(skipped_personas(r#"{"status":"ok"}"#).is_empty());
    }
}
