//! Agent identity provisioning (FR-11, FR-12).
//!
//! **AD-14: secret key material never crosses the port.** [`AgentIdentity`] carries only
//! public data. The secret exists inside [`provision`] just long enough to be written to
//! disk with restrictive permissions, and is never returned, logged, or formatted.
//!
//! There is deliberately no `Debug`/`Display`/`Serialize` on anything holding a secret, so
//! a careless `{:?}` cannot leak one. NFR-7 has no remediation if it fails once.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use nostr::key::Keys;
use nostr::nips::nip19::ToBech32;

#[derive(Debug, thiserror::Error)]
pub enum IdentityError {
    #[error("identity for role {role:?} already exists at {path} — pass --force to replace it (this destroys the existing key and any history signed with it)")]
    AlreadyExists { role: String, path: PathBuf },

    #[error("could not create the key directory {path}: {source}")]
    KeyDirUnwritable {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("could not write the identity for role {role:?} to {path}: {source}")]
    Unwritable {
        role: String,
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("no identity for role {role:?} — provision it first: waggle identity provision --role {role}")]
    NotProvisioned { role: String },

    #[error("identity file {path} is malformed: {reason}")]
    Malformed { path: PathBuf, reason: String },

    #[error("role names must be lowercase alphanumeric with dashes; got {0:?}")]
    InvalidRole(String),

    #[error(
        "BUZZ_RELAY_PRIVATE_KEY is not set — buzz-admin needs a stable relay signing key to \
         publish the membership roster (kind:13534). Generate one with \
         `openssl rand -hex 32`, set it on the relay and in this shell, then re-run."
    )]
    RelayKeyMissing,

    #[error("could not run buzz-admin at {path}: {source}")]
    AdminUnavailable {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("buzz-admin failed registering {role:?}: {stderr}")]
    RegisterFailed { role: String, stderr: String },
}

/// The public half of one agent identity. Safe to print, log, and serialize.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentIdentity {
    pub role: String,
    /// 32-byte x-only public key, hex — the form the relay and event `pubkey` use.
    pub public_key_hex: String,
    /// NIP-19 `npub1…` form, for humans.
    pub npub: String,
}

/// Directory holding secret key material. Gitignored (`/keys/`).
pub fn key_dir(project_root: &Path) -> PathBuf {
    project_root.join("keys")
}

fn secret_path(project_root: &Path, role: &str) -> PathBuf {
    key_dir(project_root).join(format!("{role}.nsec"))
}

fn public_path(project_root: &Path, role: &str) -> PathBuf {
    key_dir(project_root).join(format!("{role}.pub"))
}

fn validate_role(role: &str) -> Result<(), IdentityError> {
    let ok = !role.is_empty()
        && role
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        && !role.starts_with('-')
        && !role.ends_with('-');
    if ok {
        Ok(())
    } else {
        Err(IdentityError::InvalidRole(role.to_string()))
    }
}

/// Generate a keypair for `role`.
///
/// Idempotent by default: an existing identity is left untouched and reported, because
/// silently regenerating would orphan every event the previous key ever signed. `force`
/// is the explicit destructive path (FR-11).
pub fn provision(
    project_root: &Path,
    role: &str,
    force: bool,
) -> Result<AgentIdentity, IdentityError> {
    validate_role(role)?;

    let dir = key_dir(project_root);
    let sec_path = secret_path(project_root, role);

    if sec_path.exists() && !force {
        return Err(IdentityError::AlreadyExists {
            role: role.to_string(),
            path: sec_path,
        });
    }

    fs::create_dir_all(&dir).map_err(|source| IdentityError::KeyDirUnwritable {
        path: dir.clone(),
        source,
    })?;

    let keys = Keys::generate();
    let public_key_hex = keys.public_key().to_hex();
    let npub = keys
        .public_key()
        .to_bech32()
        .map_err(|e| IdentityError::Malformed {
            path: sec_path.clone(),
            reason: format!("could not encode npub: {e}"),
        })?;

    // The secret's only appearance. Written 0600 (temp + rename), then dropped.
    // On --force, the previous secret is kept as `{role}.nsec.bak` before rotate.
    let secret_hex = keys.secret_key().to_secret_hex();
    write_secret(&sec_path, role, &secret_hex)?;

    // Public half is written separately so tooling never needs to open the secret file.
    fs::write(
        public_path(project_root, role),
        format!("{public_key_hex}\n"),
    )
    .map_err(|source| IdentityError::Unwritable {
        role: role.to_string(),
        path: public_path(project_root, role),
        source,
    })?;

    Ok(AgentIdentity {
        role: role.to_string(),
        public_key_hex,
        npub,
    })
}

fn secret_backup_path(path: &Path) -> PathBuf {
    // tea.nsec → tea.nsec.bak (keep previous on force rotate).
    PathBuf::from(format!("{}.bak", path.display()))
}

fn secret_temp_path(path: &Path) -> PathBuf {
    PathBuf::from(format!("{}.tmp", path.display()))
}

fn map_unwritable(role: &str, path: &Path, source: std::io::Error) -> IdentityError {
    IdentityError::Unwritable {
        role: role.to_string(),
        path: path.to_path_buf(),
        source,
    }
}

/// Persist secret material: backup existing → write temp 0600 → rename → chmod 0600.
///
/// `OpenOptions::mode` only applies on create, so a `--force` truncate of a world-readable
/// file would leave unsafe perms. Always chmod after write, and rotate via temp + rename.
#[cfg(unix)]
fn write_secret(path: &Path, role: &str, secret_hex: &str) -> Result<(), IdentityError> {
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

    if path.exists() {
        let bak = secret_backup_path(path);
        fs::copy(path, &bak).map_err(|source| map_unwritable(role, &bak, source))?;
        fs::set_permissions(&bak, fs::Permissions::from_mode(0o600))
            .map_err(|source| map_unwritable(role, &bak, source))?;
    }

    let tmp = secret_temp_path(path);
    {
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&tmp)
            .map_err(|source| map_unwritable(role, &tmp, source))?;

        f.write_all(secret_hex.as_bytes())
            .and_then(|()| f.write_all(b"\n"))
            .and_then(|()| f.sync_all())
            .map_err(|source| map_unwritable(role, &tmp, source))?;
    }

    // Always enforce owner-only after write (covers --force over a loose mode).
    fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600))
        .map_err(|source| map_unwritable(role, &tmp, source))?;
    fs::rename(&tmp, path).map_err(|source| map_unwritable(role, path, source))?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .map_err(|source| map_unwritable(role, path, source))?;
    Ok(())
}

#[cfg(not(unix))]
fn write_secret(path: &Path, role: &str, secret_hex: &str) -> Result<(), IdentityError> {
    if path.exists() {
        let bak = secret_backup_path(path);
        fs::copy(path, &bak).map_err(|source| map_unwritable(role, &bak, source))?;
    }

    let tmp = secret_temp_path(path);
    fs::write(&tmp, format!("{secret_hex}\n"))
        .map_err(|source| map_unwritable(role, &tmp, source))?;
    fs::rename(&tmp, path).map_err(|source| map_unwritable(role, path, source))?;
    Ok(())
}

/// Load the public half of an identity. **Never reads the secret file.**
pub fn load_public(project_root: &Path, role: &str) -> Result<AgentIdentity, IdentityError> {
    validate_role(role)?;
    let pub_path = public_path(project_root, role);
    if !pub_path.exists() {
        return Err(IdentityError::NotProvisioned {
            role: role.to_string(),
        });
    }

    let hex = fs::read_to_string(&pub_path)
        .map_err(|e| IdentityError::Malformed {
            path: pub_path.clone(),
            reason: e.to_string(),
        })?
        .trim()
        .to_string();

    let pk = nostr::PublicKey::from_hex(&hex).map_err(|e| IdentityError::Malformed {
        path: pub_path.clone(),
        reason: format!("not a public key: {e}"),
    })?;

    Ok(AgentIdentity {
        role: role.to_string(),
        public_key_hex: hex,
        npub: pk.to_bech32().map_err(|e| IdentityError::Malformed {
            path: pub_path,
            reason: format!("could not encode npub: {e}"),
        })?,
    })
}

/// Every provisioned identity, by role. Public data only.
pub fn list(project_root: &Path) -> Vec<AgentIdentity> {
    let dir = key_dir(project_root);
    let Ok(entries) = fs::read_dir(&dir) else {
        return Vec::new();
    };

    let mut out: Vec<AgentIdentity> = entries
        .filter_map(Result::ok)
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            let role = name.strip_suffix(".pub")?;
            load_public(project_root, role).ok()
        })
        .collect();
    out.sort_by(|a, b| a.role.cmp(&b.role)); // deterministic output (NFR-1)
    out
}

/// Publish an agent's profile (kind:0) to the hive under its own key (FR-14).
///
/// **AD-14 boundary:** the secret is read here, inside the adapter, and handed to the
/// substrate CLI through the child process environment. It is never returned to the
/// caller, never logged, and never placed on a command line where `ps` could see it.
///
/// **AD-2:** this goes through `buzz-cli`, a published substrate interface, rather than
/// reimplementing event signing.
pub fn publish_profile(
    project_root: &Path,
    role: &str,
    buzz_cli: &Path,
    relay_url: &str,
    display_name: &str,
    about: &str,
) -> Result<String, IdentityError> {
    validate_role(role)?;
    let sec_path = secret_path(project_root, role);
    if !sec_path.exists() {
        return Err(IdentityError::NotProvisioned {
            role: role.to_string(),
        });
    }

    let secret = fs::read_to_string(&sec_path)
        .map_err(|e| IdentityError::Malformed {
            path: sec_path.clone(),
            reason: e.to_string(),
        })?
        .trim()
        .to_string();

    let out = std::process::Command::new(buzz_cli)
        // Env, not argv: process arguments are world-readable via `ps`.
        .env("BUZZ_PRIVATE_KEY", &secret)
        .env("BUZZ_RELAY_URL", relay_url)
        .args([
            "users",
            "set-profile",
            "--name",
            display_name,
            "--about",
            about,
        ])
        .output()
        .map_err(|source| IdentityError::Unwritable {
            role: role.to_string(),
            path: buzz_cli.to_path_buf(),
            source,
        })?;

    if !out.status.success() {
        return Err(IdentityError::Malformed {
            path: buzz_cli.to_path_buf(),
            // stderr may quote our input but never the key, which was passed via env.
            reason: String::from_utf8_lossy(&out.stderr).trim().to_string(),
        });
    }

    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Outcome of registering a role with the hive membership list (FR-12).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Registered {
    /// Newly added to the relay membership list.
    Added { pubkey: String },
    /// Already present — idempotent success (FR-12, NFR-2).
    AlreadyMember { pubkey: String },
}

/// Register a provisioned identity as a relay member via `buzz-admin add-member`.
///
/// **AD-2:** goes through the substrate's published admin CLI. Requires
/// `DATABASE_URL`, `RELAY_URL`, and `BUZZ_RELAY_PRIVATE_KEY` in the environment
/// (the last signs the kind:13534 roster event). The agent's secret is never
/// read — only the public key is passed.
pub fn register_member(
    project_root: &Path,
    role: &str,
    buzz_admin: &Path,
    member_role: &str,
) -> Result<Registered, IdentityError> {
    validate_role(role)?;
    let id = load_public(project_root, role)?;

    if std::env::var_os("BUZZ_RELAY_PRIVATE_KEY").is_none() {
        return Err(IdentityError::RelayKeyMissing);
    }

    let out = std::process::Command::new(buzz_admin)
        .args([
            "add-member",
            "--pubkey",
            &id.public_key_hex,
            "--role",
            member_role,
        ])
        .output()
        .map_err(|source| IdentityError::AdminUnavailable {
            path: buzz_admin.to_path_buf(),
            source,
        })?;

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    let combined = format!("{stdout}{stderr}");

    if !out.status.success() {
        return Err(IdentityError::RegisterFailed {
            role: role.to_string(),
            stderr: combined.trim().to_string(),
        });
    }

    if combined.contains("already a member") {
        Ok(Registered::AlreadyMember {
            pubkey: id.public_key_hex,
        })
    } else {
        Ok(Registered::Added {
            pubkey: id.public_key_hex,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("waggle-identity-{name}"));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn provisions_a_distinct_keypair_per_role() {
        let root = tmp("distinct");
        let a = provision(&root, "tea", false).unwrap();
        let b = provision(&root, "dev", false).unwrap();
        assert_ne!(
            a.public_key_hex, b.public_key_hex,
            "roles must not share a key"
        );
        assert!(a.npub.starts_with("npub1"));
        assert_eq!(a.public_key_hex.len(), 64);
    }

    #[test]
    fn is_idempotent_without_force() {
        let root = tmp("idempotent");
        let first = provision(&root, "tea", false).unwrap();
        let err = provision(&root, "tea", false).unwrap_err();
        assert!(matches!(err, IdentityError::AlreadyExists { .. }));
        // and the original survived untouched
        assert_eq!(load_public(&root, "tea").unwrap(), first);
        // the error must tell you the consequence, not just "exists"
        assert!(err.to_string().contains("--force"));
    }

    #[test]
    fn force_replaces_the_key() {
        let root = tmp("force");
        let first = provision(&root, "tea", false).unwrap();
        let first_secret = fs::read_to_string(root.join("keys/tea.nsec")).unwrap();
        let second = provision(&root, "tea", true).unwrap();
        assert_ne!(first.public_key_hex, second.public_key_hex);
        let bak = fs::read_to_string(root.join("keys/tea.nsec.bak")).unwrap();
        assert_eq!(
            bak, first_secret,
            "force rotate must keep the previous secret as .nsec.bak"
        );
    }

    #[test]
    #[cfg(unix)]
    fn secret_file_is_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let root = tmp("perms");
        provision(&root, "tea", false).unwrap();
        let mode = fs::metadata(root.join("keys/tea.nsec"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600, "secret key must be owner-only, got {mode:o}");
    }

    #[test]
    #[cfg(unix)]
    fn force_reapplies_owner_only_even_when_existing_mode_was_loose() {
        use std::os::unix::fs::PermissionsExt;
        let root = tmp("force-perms");
        provision(&root, "tea", false).unwrap();
        let sec = root.join("keys/tea.nsec");
        let mut loose = fs::metadata(&sec).unwrap().permissions();
        loose.set_mode(0o644);
        fs::set_permissions(&sec, loose).unwrap();
        provision(&root, "tea", true).unwrap();
        let mode = fs::metadata(&sec).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "force rotate must chmod 0600, got {mode:o}");
        let bak_mode = fs::metadata(root.join("keys/tea.nsec.bak"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(bak_mode, 0o600, "backup must also be owner-only");
    }

    #[test]
    fn public_data_never_contains_the_secret() {
        // AD-14 / NFR-7: whatever we hand across the port must not embed the secret.
        let root = tmp("nosecret");
        let id = provision(&root, "tea", false).unwrap();
        let secret = fs::read_to_string(root.join("keys/tea.nsec")).unwrap();
        let secret = secret.trim();
        assert!(!secret.is_empty());

        let rendered = format!("{id:?} {} {} {}", id.role, id.public_key_hex, id.npub);
        assert!(
            !rendered.contains(secret),
            "secret key material leaked into public identity output"
        );
    }

    #[test]
    fn list_is_sorted_and_public_only() {
        let root = tmp("list");
        provision(&root, "zeta", false).unwrap();
        provision(&root, "alpha", false).unwrap();
        let roles: Vec<_> = list(&root).into_iter().map(|i| i.role).collect();
        assert_eq!(
            roles,
            vec!["alpha", "zeta"],
            "listing must be deterministic"
        );
    }

    #[test]
    fn rejects_role_names_that_would_escape_the_key_dir() {
        let root = tmp("roles");
        for bad in ["../evil", "Tea", "with space", "", "-lead", "trail-"] {
            assert!(
                provision(&root, bad, false).is_err(),
                "should reject role {bad:?}"
            );
        }
    }

    #[test]
    fn load_public_tells_you_how_to_fix_a_missing_identity() {
        let root = tmp("missing");
        let err = load_public(&root, "tea").unwrap_err();
        assert!(err.to_string().contains("waggle identity provision"));
    }
}
