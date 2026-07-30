//! Seed `#help` canvases from BMAD's assembled help catalog (`bmad-help.csv`).
//!
//! The canvas leads with path chooser + anytime BMAD Help. The full CSV catalog
//! is an appendix for power users — never the first screen.

use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HelpRow {
    pub module: String,
    pub skill: String,
    pub display_name: String,
    pub menu_code: String,
    pub description: String,
    pub phase: String,
}

/// Parse `_bmad/_config/bmad-help.csv`. Skips `_meta` rows and blank skills.
pub fn parse_csv(raw: &str) -> Vec<HelpRow> {
    let mut rows = Vec::new();
    let mut lines = raw.lines();
    let _header = lines.next();
    for line in lines {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let cols = split_csv_line(line);
        if cols.len() < 8 {
            continue;
        }
        let skill = cols[1].trim();
        if skill.is_empty() || skill == "_meta" {
            continue;
        }
        rows.push(HelpRow {
            module: cols[0].trim().to_string(),
            skill: skill.to_string(),
            display_name: cols[2].trim().to_string(),
            menu_code: cols[3].trim().to_string(),
            description: cols[4].trim().to_string(),
            phase: cols[7].trim().to_string(),
        });
    }
    rows
}

/// Load and parse the catalog from disk. Missing file → empty.
pub fn load_csv(path: &Path) -> Vec<HelpRow> {
    match std::fs::read_to_string(path) {
        Ok(raw) => parse_csv(&raw),
        Err(_) => Vec::new(),
    }
}

/// Markdown canvas for `#help`: path chooser first, CSV catalog as appendix.
pub fn render_help_canvas(rows: &[HelpRow]) -> String {
    let mut out = String::from(
        "# BMAD Help\n\
         \n\
         **Waggle — powered by BMAD.** This room is the Desktop equivalent of slash-command `bmad-help`.\n\
         \n\
         ## Get help anytime\n\
         \n\
         In **any** room — not only here — `@mention` an agent with your goal, or ask\n\
         \"what's next?\" / \"continue\" / **BH**. Agents load the `bmad-help` skill, pick up\n\
         from where you left off, and route you to the next skill and room. They should\n\
         **not** dump the full catalog.\n\
         \n\
         | Code | Skill | Purpose |\n\
         |---|---|---|\n\
         | BH | bmad-help | What's next in the method |\n\
         | PM | bmad-party-mode | Multi-agent roundtable in `#party` |\n\
         \n\
         ## Choose a path\n\
         \n\
         Pick the journey that matches what you are doing. Then open that path's rooms.\n\
         \n\
         | Path | Do this | Rooms |\n\
         |---|---|---|\n\
         | **Software** | Build a product | `#planning` → `#architecture` → `#ux-design` → `#story` → `#implementation` → Testing |\n\
         | **Game** | Build a game | `#gds-design` → `#gds-production` |\n\
         | **Creative** | Ideate / brainstorm | `#ideation` (winners → Software `#planning`) |\n\
         | **Builder** | Extend the method | `#bmb-workshop` |\n\
         | **Testing** | Prove and gate | `#test-strategy` → `#gate` |\n\
         \n\
         ## Hubs\n\
         \n\
         - `#help` — path chooser + BMAD Help (you are here)\n\
         - `#party` — bring the cast together for a roundtable\n\
         \n\
         Or skip the table: describe your goal and `@mention` any agent.\n\
         \n",
    );

    if rows.is_empty() {
        out.push_str(
            "## Catalog appendix (from bmad-help.csv)\n\
             \n\
             _Catalog empty — run `waggle sync` so `_bmad/_config/bmad-help.csv` is present._\n",
        );
        return out;
    }

    let mut by_phase: BTreeMap<&str, Vec<&HelpRow>> = BTreeMap::new();
    for row in rows {
        let phase = if row.phase.is_empty() {
            "anytime"
        } else {
            row.phase.as_str()
        };
        by_phase.entry(phase).or_default().push(row);
    }

    out.push_str(
        "## Catalog appendix (from bmad-help.csv)\n\
         \n\
         Power-user reference. Prefer path chooser + asking an agent what's next.\n\
         \n",
    );
    for (phase, phase_rows) in by_phase {
        out.push_str(&format!("### {phase}\n\n"));
        out.push_str("| Code | Skill | Name | Module |\n|---|---|---|---|\n");
        for r in phase_rows {
            let code = if r.menu_code.is_empty() {
                "—"
            } else {
                r.menu_code.as_str()
            };
            let name = if r.display_name.is_empty() {
                r.skill.as_str()
            } else {
                r.display_name.as_str()
            };
            out.push_str(&format!(
                "| {code} | {} | {name} | {} |\n",
                r.skill, r.module
            ));
        }
        out.push('\n');
    }

    out
}

/// Replace or set the `help` channel's canvas from the CSV catalog.
pub fn enrich_help_canvas(store: &mut [crate::channels::ChannelTemplateRecord], rows: &[HelpRow]) {
    let canvas = render_help_canvas(rows);
    for rec in store.iter_mut() {
        if rec.name.eq_ignore_ascii_case("help") {
            rec.canvas_template = Some(canvas);
            return;
        }
    }
}

fn split_csv_line(line: &str) -> Vec<String> {
    let mut cols = Vec::new();
    let mut cur = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                if in_quotes && chars.peek() == Some(&'"') {
                    cur.push('"');
                    chars.next();
                } else {
                    in_quotes = !in_quotes;
                }
            }
            ',' if !in_quotes => {
                cols.push(std::mem::take(&mut cur));
            }
            _ => cur.push(c),
        }
    }
    cols.push(cur);
    cols
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_skips_meta() {
        let raw = "\
module,skill,display-name,menu-code,description,action,args,phase,preceded-by,followed-by,required,output-location,outputs
BMad Method,_meta,,,,,,,,,false,https://docs.example,,,
BMad Method,bmad-prd,Create PRD,PRD,desc,,,2-planning,,,true,planning_artifacts,prd
";
        let rows = parse_csv(raw);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].skill, "bmad-prd");
        assert_eq!(rows[0].menu_code, "PRD");
        assert_eq!(rows[0].phase, "2-planning");
    }

    #[test]
    fn canvas_leads_with_paths_and_groups_catalog() {
        let rows = vec![HelpRow {
            module: "BMad Method".into(),
            skill: "bmad-prd".into(),
            display_name: "Create PRD".into(),
            menu_code: "PRD".into(),
            description: "d".into(),
            phase: "2-planning".into(),
        }];
        let md = render_help_canvas(&rows);
        let path_idx = md.find("## Choose a path").expect("path chooser");
        let catalog_idx = md
            .find("Catalog appendix (from bmad-help.csv)")
            .expect("catalog appendix");
        assert!(path_idx < catalog_idx, "paths must appear before catalog");
        assert!(md.contains("| **Software** |"));
        assert!(md.contains("| **Game** |"));
        assert!(md.contains("### 2-planning"));
        assert!(md.contains("| PRD | bmad-prd |"));
        assert!(md.contains("bmad-help"));
        assert!(md.contains("Get help anytime"));
    }

    #[test]
    fn empty_catalog_still_shows_paths() {
        let md = render_help_canvas(&[]);
        assert!(md.contains("## Choose a path"));
        assert!(md.contains("Catalog appendix (from bmad-help.csv)"));
        assert!(md.contains("Catalog empty"));
    }
}
