---
name: Waggle hive
status: final
sources:
  - docs/planning-artifacts/ux-designs/ux-waggle-2026-07-30/EXPERIENCE.md
updated: 2026-07-30
---

# Waggle — Design Spine

> Thin visual identity for Waggle-owned hive content. Buzz Desktop owns chrome, theme, and layout. This spine documents canvas/copy conventions only.

## Brand & Style

**Line:** Waggle — powered by BMAD.

Independent community distribution compatible with the BMAD Method. Not an official BMad product (see root `NOTICE`).

Tone in canvases: calm, method-faithful, operator-clear. Prefer plain path labels over acronym soup in headings; BMAD skill names and menu codes appear in tables and appendix, not as the hero.

## Colors

Inherited from Buzz Desktop. No Waggle color system. Do not invent purple gradient or cream-editorial themes in canvases.

## Typography

Inherited from Buzz Desktop markdown rendering. Canvas headings:

- `#` — room title
- `##` — Continue / path sections
- `###` — catalog phases (appendix only)

## Layout & Spacing

Canvas structure (every path room):

1. Title (`# Room`)
2. Continue strip (`## Continue` — path, purpose, next, Help)
3. Room working body (tables, checklists)
4. No marketing hero blocks, badge piles, or card chrome in markdown

`#help` structure:

1. Title + one-line powered-by note
2. How to get Help (anytime)
3. Choose a path
4. Hubs
5. Catalog appendix

## Elevation & Depth

None in canvases — flat markdown. Buzz provides UI elevation.

## Shapes

N/A (Buzz chrome).

## Components

| Component | Spec |
|---|---|
| Continue strip | Four short lines or a tiny table: Path · Purpose · Usually next · Help |
| Path chooser table | Path \| Do this \| Rooms |
| Catalog appendix | Phase heading + Code/Skill/Name/Module table |

## Do's and Don'ts

**Do**

- Lead with path and intent
- Keep Continue strips short
- Point Help at `@mention` + goal language
- Preserve stable channel slugs

**Don't**

- Put the full skill catalog above the path chooser
- Add decorative emoji rows as navigation
- Imply official BMad affiliation
- Redesign Buzz sidebar chrome in docs as if Waggle owns it
