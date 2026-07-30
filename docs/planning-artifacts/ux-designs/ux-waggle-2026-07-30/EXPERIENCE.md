---
name: Waggle hive
status: final
sources:
  - docs/planning-artifacts/prds/prd-waggle-2026-07-28/prd.md
updated: 2026-07-30
---

# Waggle — Experience Spine

> Buzz Desktop hive UX for Waggle (powered by BMAD). Visual chrome inherits Buzz Desktop; this spine owns information architecture, journeys, and room behavior. Paired with `DESIGN.md`. Spines win on conflict with canvases or mocks.

## Foundation

**Form factor:** Buzz Desktop team workspace (Nostr-backed hive) — channels, canvases, `@mention` agents, reaction gates. Not a custom web UI.

**UI system:** Buzz Desktop. Waggle owns channel names (stable), descriptions, canvas markdown, and agent instructions that compile into persona packs.

**Product framing:** Waggle is a distribution that compiles BMAD Method modules into a Buzz hive. BMAD skills and sequencing remain authoritative; Waggle presents paths and always-on Help so users are not left in a flat list of rooms.

## Information Architecture

### Hubs (always available)

| Surface | Reached from | Purpose |
|---|---|---|
| `#help` | Sidebar / first open | Path chooser + BMAD Help (Desktop equivalent of slash-command `bmad-help`) |
| `#party` | Sidebar / agent invite | Multi-agent roundtable (`bmad-party-mode`) |

### Paths (choose in `#help`, then use that path’s rooms)

| Path | Plain label | Rooms (stable names) |
|---|---|---|
| Software | Build a product | `#planning` → `#architecture` → `#ux-design` → `#story` → `#implementation` → Testing |
| Game | Build a game | `#gds-design` → `#gds-production` |
| Creative | Ideate / brainstorm | `#ideation` → winners to Software `#planning` |
| Builder | Extend the method | `#bmb-workshop` |
| Testing | Prove and gate | `#test-strategy` → `#gate` |

Buzz channel templates do not provision sidebar categories. Operators may group rooms into Desktop sidebar sections manually; Waggle documents the recommended grouping.

### Room roles

| Room | Path | Purpose |
|---|---|---|
| `#planning` | Software | Brief, PRD, analysis |
| `#architecture` | Software | Solutioning and trade-offs |
| `#ux-design` | Software (+ WDS) | UX / design specs |
| `#story` | Software | One story end to end |
| `#implementation` | Software | Build, patches, review |
| `#ideation` | Creative | Brainstorm / innovation |
| `#gds-design` | Game | GDD and game design |
| `#gds-production` | Game | Game production track |
| `#bmb-workshop` | Builder | Custom agents, workflows, modules |
| `#test-strategy` | Testing | Risk-based test strategy |
| `#gate` | Testing | Verdicts + human ✅ approval |

## Voice and Tone

Microcopy for canvases and agent replies in the hive.

| Do | Don't |
|---|---|
| "Choose a path, or say your goal." | "Welcome to the BMAD Method™ mega catalog!" |
| "What's next?" / "Continue from where you left off." | Dump the full skill CSV |
| "You're in Software · Planning." | Jargon-only room titles with no path context |
| "Stuck? `@mention` any agent and ask for BMAD Help." | "Load skill BH with args…" as the first line |
| Name the next room in plain language | Assume the user knows menu codes |

## Component Patterns

Behavioral patterns for Waggle-owned surfaces (canvases + agent chat). Visual chrome is Buzz.

| Pattern | Use | Behavioral rules |
|---|---|---|
| Path chooser | `#help` canvas top | Five paths with plain labels and room sequences. No catalog above the fold. |
| Continue strip | Every path room canvas | Path name, room purpose, usual next room, Help affordance. |
| BMAD Help (anytime) | Any room + `#help` | On goal / continue / "what's next" / BH: load `bmad-help`, infer path from channel, recommend next skill + room. Never dump the catalog. |
| Party facilitation | `#party` | Load `bmad-party-mode`; wake others on `@mention`. Gate decisions move to `#gate`. |
| Catalog appendix | `#help` bottom | Full `bmad-help.csv` grouping for power users; secondary. |

## State Patterns

| State | Surface | Treatment |
|---|---|---|
| First open | `#help` | Path chooser + "or describe your goal and `@mention` an agent" |
| Mid-project continue | Any room | Agent runs `bmad-help` against artifacts / phase; points to next room |
| Path switch | `#help` | User picks another path; no need to delete old rooms |
| Empty catalog | `#help` | Note to run `waggle sync` so `_bmad/_config/bmad-help.csv` exists |
| Gate pending | `#gate` | Verdict visible; human ✅ required; agents never self-approve |
| Idle agents | All rooms | Offline until `@mention` (lazy ACP) |

## Interaction Primitives

- **Enter a room** — open channel from sidebar.
- **Wake an agent** — `@mention` (required for replies).
- **BMAD Help** — in any room: ask what's next / continue / BH; or go to `#help`.
- **Party** — `#party` + mention facilitators / cast.
- **Approve a gate** — ✅ reaction on the verdict event (human only).

## Accessibility Floor

- Canvas markdown stays structured (headings, tables) for screen-reader scan order.
- Room descriptions state purpose in plain language (sidebar truncation-safe first clause).
- Agents restate the recommended next step in prose, not only menu codes.
- Do not rely on emoji alone for path meaning.

## Key Flows

### Jordan — first day in the hive

1. Jordan opens Buzz Desktop against a synced hive; sidebar shows many rooms.
2. They open `#help` and see five paths, not a spreadsheet of skills.
3. They choose **Software** and move to `#planning`, `@mention` Mary or John with a goal.
4. **Climax:** An agent routes them to the right BMAD skill and confirms the next room — they feel oriented, not lost in chats.

### Sam — continue mid-project

1. Sam returns weeks later in `#implementation` with a half-done story.
2. They `@mention` Amelia: "what's next in BMAD?"
3. Amelia loads `bmad-help`, uses channel + artifacts as context.
4. **Climax:** Sam gets a single next action (e.g. code review skill → then Testing `#test-strategy`) without re-reading the catalog.

### Riley — game path

1. Riley opens `#help`, picks **Game**.
2. Works in `#gds-design` with the game cast, then `#gds-production`.
3. **Climax:** When ready to prove quality, Help routes them to Testing rooms without forcing the full Software journey.

### Avery — gate

1. Murat publishes a CONCERNS verdict in `#gate`.
2. Avery reviews evidence, reacts ✅.
3. **Climax:** `waggle gate` records approval under the correct identity; work may advance.
