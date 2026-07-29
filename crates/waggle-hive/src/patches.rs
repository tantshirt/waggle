//! Portable developer output as NIP-34 patch events (FR-18).
//!
//! **AD-2:** patch publication goes through `buzz patches`, a published substrate
//! interface implementing NIP-34 — waggle does not reimplement git-over-Nostr.
//!
//! **AD-8 / NFR-6:** the kinds are entirely standard — `30617` repository announcement,
//! `1617` patch, `1630`–`1633` status — so a third-party NIP-34 client
//! (gitworkshop.dev, ngit) can read the repository, its patches, and their statuses
//! without knowing waggle exists. That portability is the point of the requirement.
//!
//! waggle's own contribution is the **link**: FR-18 requires a patch be tied to the story
//! channel and to the artifact events that motivated it, which NIP-34 alone does not do.

use std::path::Path;
use std::process::Command;

#[derive(Debug, thiserror::Error)]
pub enum PatchError {
    #[error("could not run the substrate CLI at {path}: {source}")]
    CliUnavailable {
        path: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("patch file {0} does not exist")]
    NoPatchFile(std::path::PathBuf),

    #[error("substrate rejected the patch: {0}")]
    Rejected(String),

    #[error("could not parse the substrate response: {0}")]
    Unparseable(String),
}

/// Send a `git format-patch` file as a NIP-34 kind:1617 event.
///
/// `euc` is the repository's earliest unique commit, which NIP-34 uses to identify a
/// repository across relays — `git rev-list --max-parents=0 HEAD | tail -1`.
#[allow(clippy::too_many_arguments)]
pub fn send_patch(
    buzz_cli: &Path,
    relay_url: &str,
    secret_hex: &str,
    repo_owner: &str,
    repo_id: &str,
    patch_file: &Path,
    euc: &str,
    is_root: bool,
) -> Result<String, PatchError> {
    if !patch_file.exists() {
        return Err(PatchError::NoPatchFile(patch_file.to_path_buf()));
    }

    let mut cmd = Command::new(buzz_cli);
    cmd.env("BUZZ_PRIVATE_KEY", secret_hex)
        .env("BUZZ_RELAY_URL", relay_url)
        .args(["patches", "send", "--repo-owner", repo_owner])
        .args(["--repo-id", repo_id])
        .arg("--patch-file")
        .arg(patch_file)
        .args(["--euc", euc]);
    if is_root {
        cmd.arg("--root");
    }

    let out = cmd.output().map_err(|source| PatchError::CliUnavailable {
        path: buzz_cli.to_path_buf(),
        source,
    })?;

    let stdout = String::from_utf8_lossy(&out.stdout);
    let last = stdout
        .lines()
        .rev()
        .find(|l| l.trim_start().starts_with('{'))
        .unwrap_or_default();

    let parsed: serde_json::Value =
        serde_json::from_str(last).map_err(|e| PatchError::Unparseable(e.to_string()))?;

    // The substrate reports its own errors as JSON on stdout with exit 0 in some paths,
    // so success is decided by the payload rather than the exit status alone.
    if let Some(msg) = parsed.get("message").and_then(|m| m.as_str()) {
        if parsed.get("error").is_some() {
            return Err(PatchError::Rejected(msg.to_string()));
        }
    }
    if !out.status.success() {
        return Err(PatchError::Rejected(
            String::from_utf8_lossy(&out.stderr).trim().to_string(),
        ));
    }

    parsed
        .get("event_id")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .ok_or_else(|| PatchError::Unparseable(format!("no event_id in {last}")))
}

/// Set a patch's status using the standard NIP-34 status kinds.
pub fn set_status(
    buzz_cli: &Path,
    relay_url: &str,
    secret_hex: &str,
    root_event: &str,
    status: &str,
) -> Result<String, PatchError> {
    let out = Command::new(buzz_cli)
        .env("BUZZ_PRIVATE_KEY", secret_hex)
        .env("BUZZ_RELAY_URL", relay_url)
        .args([
            "patches", "status", "--root", root_event, "--status", status,
        ])
        .output()
        .map_err(|source| PatchError::CliUnavailable {
            path: buzz_cli.to_path_buf(),
            source,
        })?;

    let stdout = String::from_utf8_lossy(&out.stdout);
    let last = stdout
        .lines()
        .rev()
        .find(|l| l.trim_start().starts_with('{'))
        .unwrap_or_default();
    let parsed: serde_json::Value =
        serde_json::from_str(last).map_err(|e| PatchError::Unparseable(e.to_string()))?;

    if parsed.get("error").is_some() {
        return Err(PatchError::Rejected(
            parsed
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("unknown")
                .to_string(),
        ));
    }

    parsed
        .get("event_id")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .ok_or_else(|| PatchError::Unparseable(format!("no event_id in {last}")))
}

/// Valid NIP-34 status values, mapping to kinds 1630–1633.
pub const STATUSES: [&str; 4] = ["open", "merged", "closed", "draft"];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_patch_file_is_caught_before_invoking_anything() {
        let err = send_patch(
            Path::new("/nonexistent/buzz"),
            "http://127.0.0.1:1",
            "deadbeef",
            "owner",
            "repo",
            Path::new("/nonexistent/x.patch"),
            "euc",
            true,
        )
        .unwrap_err();
        assert!(
            matches!(err, PatchError::NoPatchFile(_)),
            "expected the file check first, got {err}"
        );
    }

    #[test]
    fn statuses_cover_the_nip34_range() {
        // 1630 open, 1631 applied/merged, 1632 closed, 1633 draft.
        assert_eq!(STATUSES.len(), 4);
        for s in ["open", "merged", "closed", "draft"] {
            assert!(STATUSES.contains(&s));
        }
    }
}
