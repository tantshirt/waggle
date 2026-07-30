//! Publish BMAD skills into the Claude global skills directory (creator tip:
//! agents discover skills under `~/.claude/skills`).
//!
//! Project `.claude/skills` remains the sync source of truth. This module only
//! maintains symlinks — it never copies skill bodies and never clobbers
//! unrelated user skills already present under `~`.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub const MANAGED_MARKER: &str = ".waggle-managed";

#[derive(Debug, thiserror::Error)]
pub enum SkillsError {
    #[error("skills source missing: {0}")]
    NoSource(PathBuf),

    #[error("could not create {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}

#[derive(Debug, Clone, Default)]
pub struct PublishReport {
    pub target_dir: PathBuf,
    pub linked: Vec<String>,
    pub skipped: Vec<(String, String)>,
    pub removed: Vec<String>,
}

/// Resolve the global skills home: `$CLAUDE_SKILLS_HOME` or `~/.claude/skills`.
pub fn global_skills_home() -> PathBuf {
    if let Ok(p) = std::env::var("CLAUDE_SKILLS_HOME") {
        let p = p.trim();
        if !p.is_empty() {
            return PathBuf::from(p);
        }
    }
    dirs_home()
        .map(|h| h.join(".claude").join("skills"))
        .unwrap_or_else(|| PathBuf::from(".claude/skills"))
}

fn dirs_home() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

/// Symlink every skill under `project_skills` into `target_dir`.
///
/// Skips real directories that are not listed in `.waggle-managed`.
pub fn publish_global(project_skills: &Path, target_dir: &Path) -> Result<PublishReport, SkillsError> {
    if !project_skills.is_dir() {
        return Err(SkillsError::NoSource(project_skills.to_path_buf()));
    }

    fs::create_dir_all(target_dir).map_err(|source| SkillsError::Io {
        path: target_dir.to_path_buf(),
        source,
    })?;

    let marker = target_dir.join(MANAGED_MARKER);
    let mut managed = read_managed(&marker);
    let mut report = PublishReport {
        target_dir: target_dir.to_path_buf(),
        ..Default::default()
    };

    let mut wanted = Vec::new();
    let mut entries: Vec<_> = fs::read_dir(project_skills)
        .map_err(|source| SkillsError::Io {
            path: project_skills.to_path_buf(),
            source,
        })?
        .filter_map(|e| e.ok())
        .collect();
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        let src = entry.path();
        if !src.is_dir() || !src.join("SKILL.md").is_file() {
            continue;
        }
        wanted.push(name.clone());
        let dst = target_dir.join(&name);
        match ensure_symlink(&src, &dst, managed.contains(&name)) {
            Ok(EnsureOutcome::Linked) => {
                if !managed.contains(&name) {
                    managed.push(name.clone());
                }
                report.linked.push(name);
            }
            Ok(EnsureOutcome::Already) => {
                if !managed.contains(&name) {
                    managed.push(name.clone());
                }
                report.linked.push(name);
            }
            Ok(EnsureOutcome::Skipped(reason)) => {
                report.skipped.push((name, reason));
            }
            Err(e) => {
                report.skipped.push((name, e.to_string()));
            }
        }
    }

    // Drop managed links that no longer exist in the project install.
    let stale: Vec<String> = managed
        .iter()
        .filter(|n| !wanted.contains(n))
        .cloned()
        .collect();
    for name in &stale {
        let dst = target_dir.join(name);
        if dst.is_symlink() {
            let _ = fs::remove_file(&dst);
            report.removed.push(name.clone());
        }
        managed.retain(|n| n != name);
    }

    managed.sort();
    managed.dedup();
    write_managed(&marker, &managed)?;

    Ok(report)
}

#[derive(Debug)]
enum EnsureOutcome {
    Linked,
    Already,
    Skipped(String),
}

fn ensure_symlink(src: &Path, dst: &Path, was_managed: bool) -> Result<EnsureOutcome, SkillsError> {
    let src_canon = fs::canonicalize(src).unwrap_or_else(|_| src.to_path_buf());

    if dst.exists() || dst.is_symlink() {
        if dst.is_symlink() {
            let target = fs::read_link(dst).map_err(|source| SkillsError::Io {
                path: dst.to_path_buf(),
                source,
            })?;
            let resolved = if target.is_absolute() {
                target
            } else {
                dst.parent()
                    .unwrap_or_else(|| Path::new("."))
                    .join(target)
            };
            let resolved = fs::canonicalize(&resolved).unwrap_or(resolved);
            if resolved == src_canon {
                return Ok(EnsureOutcome::Already);
            }
            if was_managed {
                fs::remove_file(dst).map_err(|source| SkillsError::Io {
                    path: dst.to_path_buf(),
                    source,
                })?;
            } else {
                return Ok(EnsureOutcome::Skipped(
                    "existing symlink points elsewhere (not waggle-managed)".into(),
                ));
            }
        } else if dst.is_dir() {
            if was_managed {
                // Previously managed but somehow became a real dir — refuse to rm -rf.
                return Ok(EnsureOutcome::Skipped(
                    "path is a directory previously marked managed — remove manually".into(),
                ));
            }
            return Ok(EnsureOutcome::Skipped(
                "real directory exists (not clobbering user skill)".into(),
            ));
        } else {
            return Ok(EnsureOutcome::Skipped(
                "path exists and is not a symlink".into(),
            ));
        }
    }

    symlink_dir(&src_canon, dst)?;
    Ok(EnsureOutcome::Linked)
}

fn symlink_dir(src: &Path, dst: &Path) -> Result<(), SkillsError> {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(src, dst).map_err(|source| SkillsError::Io {
            path: dst.to_path_buf(),
            source,
        })
    }
    #[cfg(not(unix))]
    {
        // Best-effort on non-unix: directory junction via std is unavailable;
        // surface a clear skip rather than panicking.
        Err(SkillsError::Io {
            path: dst.to_path_buf(),
            source: io::Error::new(
                io::ErrorKind::Unsupported,
                "symlink publish requires unix",
            ),
        })
    }
}

fn read_managed(path: &Path) -> Vec<String> {
    let Ok(raw) = fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut names: Vec<String> = raw
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(str::to_string)
        .collect();
    names.sort();
    names.dedup();
    names
}

fn write_managed(path: &Path, names: &[String]) -> Result<(), SkillsError> {
    let mut body = String::from("# Skills symlinked by waggle sync. Do not edit by hand.\n");
    for n in names {
        body.push_str(n);
        body.push('\n');
    }
    fs::write(path, body).map_err(|source| SkillsError::Io {
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publishes_symlinks_and_skips_foreign_dirs() {
        let tmp = std::env::temp_dir().join(format!("waggle-skills-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        let src = tmp.join("src");
        let dst = tmp.join("dst");
        fs::create_dir_all(src.join("bmad-help")).unwrap();
        fs::write(src.join("bmad-help/SKILL.md"), "# help\n").unwrap();
        fs::create_dir_all(src.join("bmad-tea-skill")).unwrap();
        fs::write(src.join("bmad-tea-skill/SKILL.md"), "# tea\n").unwrap();

        fs::create_dir_all(&dst).unwrap();
        fs::create_dir_all(dst.join("01-cinematic")).unwrap();
        fs::write(dst.join("01-cinematic/SKILL.md"), "# user\n").unwrap();

        let report = publish_global(&src, &dst).unwrap();
        assert!(report.linked.contains(&"bmad-help".into()));
        assert!(report.linked.contains(&"bmad-tea-skill".into()));
        assert!(dst.join("bmad-help").is_symlink());
        assert!(dst.join("01-cinematic").is_dir());
        assert!(!dst.join("01-cinematic").is_symlink());

        // Idempotent second run.
        let report2 = publish_global(&src, &dst).unwrap();
        assert!(report2.linked.len() >= 2);

        // Remove a skill from source → managed symlink removed.
        fs::remove_dir_all(src.join("bmad-tea-skill")).unwrap();
        let report3 = publish_global(&src, &dst).unwrap();
        assert!(report3.removed.contains(&"bmad-tea-skill".into()));
        assert!(!dst.join("bmad-tea-skill").exists());

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn global_skills_home_respects_env() {
        std::env::set_var("CLAUDE_SKILLS_HOME", "/tmp/waggle-claude-skills-test");
        assert_eq!(
            global_skills_home(),
            PathBuf::from("/tmp/waggle-claude-skills-test")
        );
        std::env::remove_var("CLAUDE_SKILLS_HOME");
    }
}
