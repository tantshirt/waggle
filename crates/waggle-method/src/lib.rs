//! Adapter for reading a BMAD Method installation.
//!
//! **AD-3: the installation is read-only.** Nothing here writes. waggle's own settings
//! belong in `_bmad/custom/`, which the installer never regenerates.

pub mod descriptors;

use std::path::{Path, PathBuf};

use serde::Deserialize;
use waggle_core::Version;

#[derive(Debug, thiserror::Error)]
pub enum MethodError {
    /// NFR-4: name the specific path, never a bare "not found".
    #[error("no BMAD Method installation at {0} (expected the manifest at {1})")]
    NotInstalled(PathBuf, PathBuf),

    #[error("could not read the method manifest at {path}: {source}")]
    Unreadable {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("could not parse the method manifest at {path}: {source}")]
    Unparseable {
        path: PathBuf,
        #[source]
        source: serde_norway::Error,
    },

    #[error(
        "method manifest at {path} reports version {raw:?}, which is not a version we understand"
    )]
    UnparseableVersion { path: PathBuf, raw: String },

    #[error("could not parse TOML at {path}: {reason}")]
    UnparseableToml { path: PathBuf, reason: String },

    #[error("{path} has no [agent] block — this is an agent descriptor, so one is required")]
    MissingAgentBlock { path: PathBuf },
}

/// The installer-generated manifest at `_bmad/_config/manifest.yaml`.
///
/// Only the fields we actually use are modeled. The manifest is regenerated on every
/// `bmad-method install`, so treating it as read-only is not optional (AD-3).
#[derive(Debug, Deserialize)]
struct Manifest {
    installation: Installation,
    #[serde(default)]
    modules: Vec<ManifestModule>,
    /// Tool directories BMAD materialized skill bodies into (AD-19).
    #[serde(default)]
    ides: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct Installation {
    version: String,
}

#[derive(Debug, Deserialize)]
struct ManifestModule {
    name: String,
    version: String,
    #[serde(default)]
    source: Option<String>,
    #[serde(default)]
    sha: Option<String>,
}

/// One installed module, with the provenance FR-1 asks us to surface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledModule {
    pub name: String,
    /// Raw version string as recorded (e.g. `v1.19.1` or `6.10.0`).
    pub version_raw: String,
    pub source: Option<String>,
    /// Commit reference, present for externally sourced modules.
    pub sha: Option<String>,
}

#[derive(Debug, Clone)]
pub struct MethodInstallation {
    pub root: PathBuf,
    pub manifest_path: PathBuf,
    pub version: Version,
    pub version_raw: String,
    pub modules: Vec<InstalledModule>,
    /// Tool ids from the manifest. Resolve to directories with [`descriptors::tool_dirs`].
    pub ides: Vec<String>,
}

/// Path of the manifest relative to the project root.
pub fn manifest_path(project_root: &Path) -> PathBuf {
    project_root
        .join("_bmad")
        .join("_config")
        .join("manifest.yaml")
}

/// Read the installation. **Reads only** (AD-3).
pub fn detect(project_root: &Path) -> Result<MethodInstallation, MethodError> {
    let path = manifest_path(project_root);
    if !path.exists() {
        return Err(MethodError::NotInstalled(project_root.to_path_buf(), path));
    }

    let raw = std::fs::read_to_string(&path).map_err(|source| MethodError::Unreadable {
        path: path.clone(),
        source,
    })?;

    let manifest: Manifest =
        serde_norway::from_str(&raw).map_err(|source| MethodError::Unparseable {
            path: path.clone(),
            source,
        })?;

    let version_raw = manifest.installation.version;
    let version = Version::parse(&version_raw).ok_or_else(|| MethodError::UnparseableVersion {
        path: path.clone(),
        raw: version_raw.clone(),
    })?;

    let modules = manifest
        .modules
        .into_iter()
        .map(|m| InstalledModule {
            name: m.name,
            version_raw: m.version,
            source: m.source,
            sha: m.sha,
        })
        .collect();

    Ok(MethodInstallation {
        root: project_root.to_path_buf(),
        manifest_path: path,
        version,
        version_raw,
        modules,
        ides: manifest.ides,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_installation_names_both_paths() {
        let dir = std::env::temp_dir().join("waggle-method-absent-test");
        let err = detect(&dir).unwrap_err();
        let msg = err.to_string();
        // NFR-4: the message must be actionable, not "not found".
        assert!(msg.contains("manifest.yaml"), "unhelpful message: {msg}");
        assert!(matches!(err, MethodError::NotInstalled(..)));
    }

    #[test]
    fn manifest_path_is_installer_owned_location() {
        let p = manifest_path(Path::new("/proj"));
        assert!(p.ends_with("_bmad/_config/manifest.yaml"), "got {p:?}");
    }
}
