//! Renders a compiled [`PersonaPack`] to a Buzz persona pack directory.
//!
//! **AD-4: deterministic.** No timestamps, absolute paths, hostnames, or ordering that
//! depends on filesystem iteration. Two runs over unchanged input produce identical bytes,
//! so generated packs can be committed and reviewed in diffs.
//!
//! The output contract is `crates/buzz-persona/PERSONA_PACK_SPEC.md`, verified in Story 1.2
//! and recorded in `docs/persona-pack-contract.md`.

pub mod channels;
pub mod help;
pub mod workflow;

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use waggle_core::{MenuItem, PersonaPack};

#[derive(Debug, thiserror::Error)]
pub enum EmitError {
    #[error("could not create {path}: {source}")]
    Mkdir {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("could not write {path}: {source}")]
    Write {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("could not remove {path}: {source}")]
    Remove {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("could not replace pack directory {path}: {source}")]
    Replace {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("skill {skill:?} referenced by menu item {code:?} was not found at {searched} — the method installation and the pack would disagree")]
    SkillMissing {
        skill: String,
        code: String,
        searched: PathBuf,
    },

    #[error("could not copy skill {skill:?}: {source}")]
    SkillCopy {
        skill: String,
        #[source]
        source: std::io::Error,
    },
}

/// Everything a pack needs that does not come from the descriptor.
pub struct PackMeta<'a> {
    /// Module code, e.g. `tea`.
    pub module: &'a str,
    /// Module version as recorded by the installation, e.g. `v1.19.1`.
    pub module_version: &'a str,
    /// Directory holding materialized skill bodies, e.g. `.claude/skills` (AD-19).
    pub skills_source: &'a Path,
    /// Pack-level instructions, verbatim.
    pub instructions: &'a str,
    /// waggle's own template data for this module, if any
    /// (`templates/<module>/channels.json`). `None` means the module ships no
    /// channel templates — reported, not an error (AD-6).
    pub channel_templates: Option<&'a [channels::ChannelTemplateSource]>,
    /// Agent ids the module registers, for filling template rosters.
    pub module_agent_ids: &'a [String],
    /// Every agent the installation registers — for `include_all_agents` (party/help).
    pub all_agent_ids: &'a [String],
    /// Optional path to `_bmad/_config/bmad-help.csv` — seeds the `#help` canvas.
    pub help_csv: Option<&'a Path>,
    /// Skills always copied into the pack (core help/party), even with no agent menus.
    pub always_skills: &'a [String],
}

#[derive(Debug, Clone)]
pub struct EmitOutcome {
    pub pack_dir: PathBuf,
    pub files_written: Vec<PathBuf>,
    pub skills_copied: Vec<String>,
    /// Menu skills referenced but not materialized under the tool skills dir.
    /// Only populated when [`emit_pack`] is called with `allow_missing_skills = true`.
    pub skills_skipped: Vec<String>,
    /// Channel template names emitted, in order. Empty when the module ships none.
    pub channel_templates: Vec<String>,
}

/// Write the pack into a staging directory, validate, then atomically replace `out_dir/<module>`.
///
/// When `allow_missing_skills` is `false` (CI / sync default), a referenced skill without a
/// materialized `SKILL.md` is a hard error. When `true`, missing skills are recorded in
/// [`EmitOutcome::skills_skipped`] for local iteration.
pub fn emit_pack(
    out_dir: &Path,
    packs: &[PersonaPack],
    meta: &PackMeta<'_>,
    allow_missing_skills: bool,
) -> Result<EmitOutcome, EmitError> {
    let pack_dir = out_dir.join(meta.module);
    let staging = out_dir.join(format!(".{}-emit-staging", meta.module));
    let backup = out_dir.join(format!(".{}-emit-backup", meta.module));

    // Drop leftover staging/backup from a previous interrupted emit.
    remove_if_exists(&staging)?;
    remove_if_exists(&backup)?;

    let staged = match emit_into(&staging, packs, meta, allow_missing_skills) {
        Ok(o) => o,
        Err(e) => {
            let _ = remove_if_exists(&staging);
            return Err(e);
        }
    };

    // Swap: current → backup, staging → current, then drop backup.
    // Staging-then-rename removes stale agents/skills/workflows that a direct overwrite would leave.
    if pack_dir.exists() {
        std::fs::rename(&pack_dir, &backup).map_err(|source| EmitError::Replace {
            path: pack_dir.clone(),
            source,
        })?;
    }
    if let Err(source) = std::fs::rename(&staging, &pack_dir) {
        // Best-effort restore so a failed swap does not leave the pack missing.
        if backup.exists() && !pack_dir.exists() {
            let _ = std::fs::rename(&backup, &pack_dir);
        }
        let _ = remove_if_exists(&staging);
        return Err(EmitError::Replace {
            path: pack_dir,
            source,
        });
    }
    remove_if_exists(&backup)?;

    // Paths were recorded under staging; rewrite them to the final pack dir.
    let files_written = staged
        .files_written
        .into_iter()
        .map(|p| {
            let rel = p.strip_prefix(&staging).unwrap_or(&p);
            pack_dir.join(rel)
        })
        .collect();

    Ok(EmitOutcome {
        pack_dir,
        files_written,
        skills_copied: staged.skills_copied,
        skills_skipped: staged.skills_skipped,
        channel_templates: staged.channel_templates,
    })
}

fn remove_if_exists(path: &Path) -> Result<(), EmitError> {
    if path.exists() {
        std::fs::remove_dir_all(path).map_err(|source| EmitError::Remove {
            path: path.to_path_buf(),
            source,
        })?;
    }
    Ok(())
}

/// Render pack contents into `pack_dir` (caller supplies staging or final).
fn emit_into(
    pack_dir: &Path,
    packs: &[PersonaPack],
    meta: &PackMeta<'_>,
    allow_missing_skills: bool,
) -> Result<EmitOutcome, EmitError> {
    let mut files = Vec::new();

    for sub in [".plugin", "agents", "skills", "workflows"] {
        let d = pack_dir.join(sub);
        std::fs::create_dir_all(&d).map_err(|source| EmitError::Mkdir {
            path: d.clone(),
            source,
        })?;
    }

    // --- .plugin/plugin.json ---
    let manifest = build_manifest(packs, meta);
    let manifest_path = pack_dir.join(".plugin").join("plugin.json");
    let mut manifest_json = serde_json::to_string_pretty(&manifest)
        .expect("manifest is plain data and always serializes");
    manifest_json.push('\n');
    write(&manifest_path, &manifest_json)?;
    files.push(manifest_path);

    // --- agents/<id>.persona.md, one per registered agent ---
    for pack in packs {
        let persona_path = pack_dir
            .join("agents")
            .join(format!("{}.persona.md", pack.name));
        write(&persona_path, &render_persona(pack))?;
        files.push(persona_path);
    }

    // --- instructions.md ---
    let instructions_path = pack_dir.join("instructions.md");
    write(&instructions_path, meta.instructions)?;
    files.push(instructions_path);

    // --- workflows/<module>-gate.yaml ---
    // The gate is the module's release checkpoint (FR-4, FR-19).
    let gate = crate::workflow::gate_workflow(meta.module);
    let gate_yaml = crate::workflow::render(&gate)
        .expect("workflow definition is plain data and always serializes");
    let gate_path = pack_dir
        .join("workflows")
        .join(format!("{}.yaml", gate.name));
    write(&gate_path, &gate_yaml)?;
    files.push(gate_path);

    // --- channel-templates.json ---
    // Delegated provisioning: Buzz reads this store directly via --templates-file, so
    // waggle emits data rather than reimplementing channel and canvas creation.
    let mut channel_templates = Vec::new();
    if let Some(sources) = meta.channel_templates {
        let mut store = channels::build_store(
            meta.module,
            sources,
            meta.module_agent_ids,
            meta.all_agent_ids,
        );
        if let Some(csv_path) = meta.help_csv {
            let rows = help::load_csv(csv_path);
            if !rows.is_empty() {
                help::enrich_help_canvas(&mut store, &rows);
            }
        }
        channel_templates = store.iter().map(|t| t.name.clone()).collect();
        let json = channels::render(&store)
            .expect("channel templates are plain data and always serialize");
        let path = pack_dir.join("channel-templates.json");
        write(&path, &json)?;
        files.push(path);
    }

    // --- skills/ ---
    // Copied verbatim: BMAD skills and Buzz pack skills are the same format, so this is
    // placement rather than translation (verified in Story 1.2).
    // Union across every persona in the module, deduplicated and ordered (AD-4).
    let mut wanted: Vec<String> = Vec::new();
    for pack in packs {
        for s in pack.skill_ids() {
            if !wanted.contains(&s) {
                wanted.push(s);
            }
        }
    }
    for s in meta.always_skills {
        if !wanted.contains(s) {
            wanted.push(s.clone());
        }
    }
    wanted.sort();

    let mut skills_copied = Vec::new();
    let mut skills_skipped = Vec::new();
    for skill in wanted {
        let src = meta.skills_source.join(&skill);
        if !src.join("SKILL.md").exists() {
            if allow_missing_skills {
                // Local iteration: report and continue when skill ids disagree with disk.
                skills_skipped.push(skill);
                continue;
            }
            return Err(EmitError::SkillMissing {
                code: referring_menu_code(packs, &skill),
                skill: skill.clone(),
                searched: src,
            });
        }
        let dst = pack_dir.join("skills").join(&skill);
        copy_dir(&src, &dst).map_err(|source| EmitError::SkillCopy {
            skill: skill.clone(),
            source,
        })?;
        skills_copied.push(skill);
    }

    Ok(EmitOutcome {
        pack_dir: pack_dir.to_path_buf(),
        files_written: files,
        skills_copied,
        skills_skipped,
        channel_templates,
    })
}

fn referring_menu_code(packs: &[PersonaPack], skill: &str) -> String {
    for pack in packs {
        for item in &pack.menu {
            if let MenuItem::Dispatch { code, skill: s, .. } = item {
                if s == skill {
                    return code.clone();
                }
            }
        }
    }
    // always_skills / unattributed references
    String::new()
}

#[derive(serde::Serialize)]
struct Manifest {
    #[serde(rename = "$schema")]
    schema: &'static str,
    id: String,
    name: String,
    version: String,
    description: String,
    author: &'static str,
    license: &'static str,
    personas: Vec<String>,
    pack_instructions: &'static str,
    defaults: Defaults,
}

#[derive(serde::Serialize)]
struct Defaults {
    triggers: Triggers,
    subscribe: Vec<String>,
    thread_replies: bool,
    broadcast_replies: bool,
}

#[derive(serde::Serialize)]
struct Triggers {
    mentions: bool,
    keywords: Vec<String>,
    all_messages: bool,
}

fn build_manifest(packs: &[PersonaPack], meta: &PackMeta<'_>) -> Manifest {
    Manifest {
        schema: "https://open-plugin-spec.org/schema/v1/plugin.json",
        id: format!("dev.waggle.pack.{}", meta.module),
        name: format!("waggle — {}", meta.module),
        // Version tracks the module it was compiled from, so a pack's provenance is
        // legible without opening it.
        version: meta.module_version.trim_start_matches('v').to_string(),
        description: format!(
            "{} agent{} compiled from the {} module of a BMAD Method installation.",
            packs.len(),
            if packs.len() == 1 { "" } else { "s" },
            meta.module
        ),
        author: "The waggle contributors",
        license: "Apache-2.0",
        personas: packs
            .iter()
            .map(|p| format!("agents/{}.persona.md", p.name))
            .collect(),
        pack_instructions: "instructions.md",
        defaults: Defaults {
            triggers: Triggers {
                mentions: true,
                keywords: Vec::new(),
                all_messages: false,
            },
            subscribe: Vec::new(),
            thread_replies: true,
            broadcast_replies: false,
        },
    }
}

/// Render `agents/<id>.persona.md` — YAML frontmatter plus the persona prompt body.
fn render_persona(pack: &PersonaPack) -> String {
    let mut s = String::new();

    s.push_str("---\n");
    let _ = writeln!(s, "name: {}", yaml_str(&pack.name));
    let _ = writeln!(s, "display_name: {}", yaml_str(&pack.display_name));
    let _ = writeln!(s, "description: {}", yaml_str(&pack.description));

    let skills = pack.skill_ids();
    if !skills.is_empty() {
        s.push_str("skills:\n");
        for skill in &skills {
            let _ = writeln!(s, "  - \"./skills/{skill}/\"");
        }
    }
    s.push_str("---\n\n");

    // --- body: the [System] layer ---
    let _ = writeln!(s, "You are {}.\n", pack.display_name);

    if let Some(role) = &pack.role {
        let _ = writeln!(s, "## Role\n\n{role}\n");
    }
    if let Some(identity) = &pack.identity {
        let _ = writeln!(s, "## Identity\n\n{identity}\n");
    }
    if let Some(style) = &pack.communication_style {
        let _ = writeln!(s, "## Communication style\n\n{style}\n");
    }
    if !pack.principles.is_empty() {
        s.push_str("## Principles\n\n");
        for p in &pack.principles {
            let _ = writeln!(s, "- {p}");
        }
        s.push('\n');
    }

    let dispatch: Vec<_> = pack
        .menu
        .iter()
        .filter_map(|m| match m {
            MenuItem::Dispatch {
                code,
                description,
                skill,
            } => Some((code, description, skill)),
            MenuItem::Prompt { .. } => None,
        })
        .collect();

    if !dispatch.is_empty() {
        s.push_str("## Capabilities\n\nLoad a skill with `load(source: \"<skill-name>\")`.\n\n");
        s.push_str("| Code | Capability | Skill |\n|---|---|---|\n");
        for (code, description, skill) in &dispatch {
            let _ = writeln!(s, "| `{code}` | {description} | `{skill}` |");
        }
        s.push('\n');
    }

    // Bias toward this agent's menu skills + hive help/party (BMAD creator tip:
    // global skills under ~/.claude/skills + per-agent preference).
    s.push_str("## Preferred skills\n\n");
    s.push_str(
        "Bias toward these skills for your role (also available globally under \
         `~/.claude/skills` after `waggle sync`). Prefer loading them over improvising:\n\n",
    );
    if dispatch.is_empty() {
        s.push_str("- _(no menu skills registered for this persona)_\n");
    } else {
        for (_code, description, skill) in &dispatch {
            let _ = writeln!(s, "- `{skill}` — {description}");
        }
    }
    s.push_str(
        "\nHive surfaces (every agent):\n\
         - `bmad-help` — when mentioned in `#help` or asked what to do next in BMAD\n\
         - `bmad-party-mode` — when mentioned in `#party` or asked for a roundtable\n\n",
    );

    // AD-7: prompt-only items have no skill, so they live in the body as instructions.
    for item in &pack.menu {
        if let MenuItem::Prompt {
            code,
            description,
            prompt,
        } = item
        {
            let _ = writeln!(s, "## `{code}` — {description}\n");
            s.push_str(
                "This capability has **no skill**; it is a routing decision you make yourself.\n\n",
            );
            let _ = writeln!(s, "{prompt}\n");
        }
    }

    if !pack.persistent_facts.is_empty() {
        s.push_str("## Persistent context\n\n");
        s.push_str("Load these at activation and carry them for the session:\n\n");
        for f in &pack.persistent_facts {
            let _ = writeln!(s, "- `{f}`");
        }
        s.push('\n');
    }

    s
}

/// Quote a YAML scalar. Always quoted, so no value can accidentally parse as a bool,
/// number, or null.
fn yaml_str(v: &str) -> String {
    format!("\"{}\"", v.replace('\\', "\\\\").replace('"', "\\\""))
}

fn write(path: &Path, contents: &str) -> Result<(), EmitError> {
    std::fs::write(path, contents).map_err(|source| EmitError::Write {
        path: path.to_path_buf(),
        source,
    })
}

/// Recursively copy `src` → `dst`. Rejects symlinks (does not follow them) and surfaces
/// directory-entry I/O errors instead of silently dropping them.
fn copy_dir(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    // Sorted so the copy order is deterministic (AD-4), not filesystem-dependent.
    let mut entries = Vec::new();
    for entry in std::fs::read_dir(src)? {
        entries.push(entry?);
    }
    entries.sort_by_key(std::fs::DirEntry::file_name);

    for entry in entries {
        let from = entry.path();
        let meta = std::fs::symlink_metadata(&from)?;
        if meta.file_type().is_symlink() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("refusing to copy symlink: {}", from.display()),
            ));
        }
        let to = dst.join(entry.file_name());
        if meta.is_dir() {
            copy_dir(&from, &to)?;
        } else {
            std::fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use waggle_core::compile_persona;

    fn sample() -> PersonaPack {
        let d = toml::from_str::<toml::Value>(
            r#"
name = "Murat"
icon = "🧪"
role = "Test architect"
principles = ["Risk-based testing."]
persistent_facts = ["file:{project-root}/**/x.md"]

[[menu]]
code = "TD"
description = "Test Design"
skill = "s-td"

[[menu]]
code = "GATE"
description = "Release Gate"
prompt = "Route the gate."
"#,
        )
        .unwrap();
        compile_persona("bmad-tea", &d, "desc").unwrap().0
    }

    fn sample_no_skills() -> PersonaPack {
        let d = toml::from_str::<toml::Value>(
            r#"
name = "Murat"
icon = "🧪"
role = "Test architect"
principles = ["Risk-based testing."]

[[menu]]
code = "GATE"
description = "Release Gate"
prompt = "Route the gate."
"#,
        )
        .unwrap();
        compile_persona("bmad-tea", &d, "desc").unwrap().0
    }

    fn meta<'a>(
        module: &'a str,
        skills_source: &'a Path,
        always_skills: &'a [String],
    ) -> PackMeta<'a> {
        PackMeta {
            module,
            module_version: "v1.0.0",
            skills_source,
            instructions: "# instructions\n",
            channel_templates: None,
            module_agent_ids: &[],
            all_agent_ids: &[],
            help_csv: None,
            always_skills,
        }
    }

    fn write_skill(root: &Path, name: &str) {
        let dir = root.join(name);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("SKILL.md"), format!("# {name}\n")).unwrap();
    }

    #[test]
    fn frontmatter_has_the_three_required_fields() {
        let out = render_persona(&sample());
        assert!(out.starts_with("---\n"));
        for required in ["name:", "display_name:", "description:"] {
            assert!(out.contains(required), "missing {required}");
        }
    }

    #[test]
    fn prompt_only_items_reach_the_body_and_not_the_skills_list() {
        let out = render_persona(&sample());
        assert!(
            out.contains("Route the gate."),
            "GATE prompt must be in the body"
        );
        assert!(
            !out.contains("./skills/GATE"),
            "a prompt item must not become a skill"
        );
        assert!(
            out.contains("./skills/s-td/"),
            "dispatch item must be a skill"
        );
    }

    #[test]
    fn rendering_is_deterministic() {
        // AD-4
        assert_eq!(render_persona(&sample()), render_persona(&sample()));
    }

    #[test]
    fn yaml_scalars_are_always_quoted() {
        assert_eq!(yaml_str("plain"), "\"plain\"");
        assert_eq!(yaml_str(r#"has "quotes""#), r#""has \"quotes\"""#);
        // "no" would otherwise parse as boolean false in YAML
        assert_eq!(yaml_str("no"), "\"no\"");
    }

    #[test]
    fn persistent_facts_render_as_references() {
        let out = render_persona(&sample());
        assert!(out.contains("file:{project-root}/**/x.md"));
    }

    #[test]
    fn preferred_skills_bias_includes_menu_and_hive_surfaces() {
        let out = render_persona(&sample());
        assert!(out.contains("## Preferred skills"));
        assert!(out.contains("`s-td`"));
        assert!(out.contains("`bmad-help`"));
        assert!(out.contains("`bmad-party-mode`"));
    }

    #[test]
    fn copy_dir_rejects_symlinks() {
        let tmp = std::env::temp_dir().join(format!("waggle-emit-symlink-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();

        let src = tmp.join("src");
        let dst = tmp.join("dst");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("ok.txt"), "ok").unwrap();
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(src.join("ok.txt"), src.join("link.txt")).unwrap();
            let err = copy_dir(&src, &dst).expect_err("symlink must be rejected");
            assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
            assert!(
                err.to_string().contains("refusing to copy symlink"),
                "got: {err}"
            );
            assert!(!dst.join("link.txt").exists());
        }
        #[cfg(not(unix))]
        {
            // On non-unix, still exercise a plain copy path.
            copy_dir(&src, &dst).unwrap();
            assert!(dst.join("ok.txt").exists());
        }

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn emit_removes_stale_agents_skills_and_workflows() {
        let tmp = std::env::temp_dir().join(format!("waggle-emit-stale-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();

        let skills = tmp.join("skills-src");
        fs::create_dir_all(&skills).unwrap();
        // First emit with no menu skills.
        let packs = vec![sample_no_skills()];
        let always: Vec<String> = Vec::new();
        let m = meta("tea", &skills, &always);
        emit_pack(&tmp, &packs, &m, false).unwrap();

        let pack = tmp.join("tea");
        // Plant stale files that a naive overwrite would leave behind.
        fs::write(pack.join("agents").join("stale.persona.md"), "stale\n").unwrap();
        fs::create_dir_all(pack.join("skills").join("stale-skill")).unwrap();
        fs::write(
            pack.join("skills").join("stale-skill").join("SKILL.md"),
            "# stale\n",
        )
        .unwrap();
        fs::write(pack.join("workflows").join("stale.yaml"), "stale: true\n").unwrap();

        emit_pack(&tmp, &packs, &m, false).unwrap();

        assert!(
            !pack.join("agents").join("stale.persona.md").exists(),
            "stale agent must be removed"
        );
        assert!(
            !pack.join("skills").join("stale-skill").exists(),
            "stale skill must be removed"
        );
        assert!(
            !pack.join("workflows").join("stale.yaml").exists(),
            "stale workflow must be removed"
        );
        assert!(pack.join("agents").join("bmad-tea.persona.md").exists());
        assert!(!tmp.join(".tea-emit-staging").exists());
        assert!(!tmp.join(".tea-emit-backup").exists());

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn missing_skill_is_hard_error_by_default() {
        let tmp = std::env::temp_dir().join(format!("waggle-emit-missing-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();

        let skills = tmp.join("skills-src");
        fs::create_dir_all(&skills).unwrap();
        // s-td is referenced by the sample menu but not materialized.
        let packs = vec![sample()];
        let always: Vec<String> = Vec::new();
        let m = meta("tea", &skills, &always);

        let err = emit_pack(&tmp, &packs, &m, false).expect_err("missing skill must fail");
        match err {
            EmitError::SkillMissing { skill, code, .. } => {
                assert_eq!(skill, "s-td");
                assert_eq!(code, "TD");
            }
            other => panic!("expected SkillMissing, got {other}"),
        }
        // Failed emit must not leave a partial pack or staging residue.
        assert!(!tmp.join("tea").exists());
        assert!(!tmp.join(".tea-emit-staging").exists());

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn allow_missing_skills_records_skip_warning() {
        let tmp =
            std::env::temp_dir().join(format!("waggle-emit-allow-missing-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();

        let skills = tmp.join("skills-src");
        fs::create_dir_all(&skills).unwrap();
        let packs = vec![sample()];
        let always: Vec<String> = Vec::new();
        let m = meta("tea", &skills, &always);

        let outcome = emit_pack(&tmp, &packs, &m, true).unwrap();
        assert_eq!(outcome.skills_skipped, vec!["s-td".to_string()]);
        assert!(outcome.skills_copied.is_empty());
        assert!(tmp
            .join("tea")
            .join("agents")
            .join("bmad-tea.persona.md")
            .exists());

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn emit_copies_present_skills() {
        let tmp = std::env::temp_dir().join(format!("waggle-emit-copy-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();

        let skills = tmp.join("skills-src");
        write_skill(&skills, "s-td");
        let packs = vec![sample()];
        let always: Vec<String> = Vec::new();
        let m = meta("tea", &skills, &always);

        let outcome = emit_pack(&tmp, &packs, &m, false).unwrap();
        assert_eq!(outcome.skills_copied, vec!["s-td".to_string()]);
        assert!(outcome.skills_skipped.is_empty());
        assert!(tmp
            .join("tea")
            .join("skills")
            .join("s-td")
            .join("SKILL.md")
            .exists());

        let _ = fs::remove_dir_all(&tmp);
    }
}
