//! Adapter for the Buzz substrate.
//!
//! **AD-2: the substrate is external and immutable.** Nothing here writes to the checkout.
//! Detection reads the checked-out tag; it does not run `git` against anything else, and it
//! never mutates.
//!
//! Why the git tag and not `Cargo.toml`: Buzz's workspace version is `0.1.0` on every
//! release, so it carries no release information. The tag is the only local source of truth
//! about which Buzz you actually have.

pub mod identity;

pub use identity::{AgentIdentity, IdentityError};

use std::path::{Path, PathBuf};
use std::process::Command;

use waggle_core::Version;

#[derive(Debug, thiserror::Error)]
pub enum HiveError {
    #[error("no substrate checkout at {0} — clone it with: git clone --depth 1 --branch <tag> https://github.com/block/buzz.git {0}")]
    NotCheckedOut(PathBuf),

    #[error("could not run git against {path}: {source}")]
    GitUnavailable {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("substrate checkout at {path} is not on a release tag — waggle pins by tag, so detach onto one (git checkout {expected})")]
    NotOnATag { path: PathBuf, expected: String },

    #[error(
        "substrate checkout at {path} reports tag {raw:?}, which is not a version we understand"
    )]
    UnparseableVersion { path: PathBuf, raw: String },
}

#[derive(Debug, Clone)]
pub struct SubstrateCheckout {
    pub path: PathBuf,
    pub version: Version,
    /// The tag as written upstream, e.g. `v0.4.26`.
    pub version_raw: String,
}

/// Default location of the substrate checkout, relative to the project root.
///
/// Gitignored. Treated as an external service.
pub fn default_path(project_root: &Path) -> PathBuf {
    project_root.join("vendor").join("buzz")
}

/// Detect which substrate release is checked out. **Reads only** (AD-2).
pub fn detect(checkout: &Path, expected_hint: &str) -> Result<SubstrateCheckout, HiveError> {
    if !checkout.join(".git").exists() {
        return Err(HiveError::NotCheckedOut(checkout.to_path_buf()));
    }

    let out = Command::new("git")
        .arg("-C")
        .arg(checkout)
        .args(["describe", "--tags", "--exact-match"])
        .output()
        .map_err(|source| HiveError::GitUnavailable {
            path: checkout.to_path_buf(),
            source,
        })?;

    if !out.status.success() {
        return Err(HiveError::NotOnATag {
            path: checkout.to_path_buf(),
            expected: expected_hint.to_string(),
        });
    }

    let version_raw = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let version = Version::parse(&version_raw).ok_or_else(|| HiveError::UnparseableVersion {
        path: checkout.to_path_buf(),
        raw: version_raw.clone(),
    })?;

    Ok(SubstrateCheckout {
        path: checkout.to_path_buf(),
        version,
        version_raw,
    })
}

/// Assert the substrate checkout has no modified tracked files.
///
/// This is the enforceable form of **AD-2**. Note the invariant is *tracked* files:
/// `.env` is gitignored by Buzz itself, so configuring it is not a modification.
/// Returns the offending porcelain lines when dirty.
pub fn verify_integrity(checkout: &Path) -> Result<Vec<String>, HiveError> {
    if !checkout.join(".git").exists() {
        return Err(HiveError::NotCheckedOut(checkout.to_path_buf()));
    }

    let out = Command::new("git")
        .arg("-C")
        .arg(checkout)
        .args(["status", "--porcelain"])
        .output()
        .map_err(|source| HiveError::GitUnavailable {
            path: checkout.to_path_buf(),
            source,
        })?;

    Ok(String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::to_string)
        .filter(|l| !l.trim().is_empty())
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absent_checkout_tells_you_how_to_fix_it() {
        let dir = std::env::temp_dir().join("waggle-hive-absent-test");
        let err = detect(&dir, "v0.4.26").unwrap_err();
        let msg = err.to_string();
        // NFR-4: the error is the instruction.
        assert!(msg.contains("git clone"), "unhelpful message: {msg}");
    }

    #[test]
    fn default_path_is_the_gitignored_vendor_dir() {
        let p = default_path(Path::new("/proj"));
        assert!(p.ends_with("vendor/buzz"), "got {p:?}");
    }
}
