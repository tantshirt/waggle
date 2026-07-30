---
title: "Product Brief Addendum: waggle"
status: draft
created: 2026-07-28
updated: 2026-07-28
---

# Addendum — waggle product brief

Depth that earned a place but does not belong in a 2-page brief. Downstream consumers:
the **PRD** (§1, §2, §5) and the **architecture doc** (§2, §3, §4).

Traceability tags follow `docs/research-notes.md`: `[BUZZ]` `[NOSTR]` `[BMAD]` `[WAGGLE]`.

---

## 1. Module coverage — full mapping (→ PRD)

The brief says "all seven official modules." This is the owner-supplied mapping, verbatim
in intent, for the PRD to turn into epics.

| Module | Method role | Mapped onto the workspace as |
|---|---|---|
| `core` | Shared tasks + global config | Relay-level config and shared workflow tasks |
| `bmm` | Four-phase agile lifecycle | Phase channel categories; each story is a channel; SM→Dev→QA handoffs are signed events; developer output is a patch event |
| `bmb` | Module builder | Tooling to author new native modules. *Stretch:* publish/install community modules as signed events |
| `cis` | Ideation agents | Brainstorm channels + canvases; party mode is several agent npubs in one room |
| `tea` | Test Architect | P0–P3 priorities as event tags; release gates as reaction-triggered approval workflows |
| `gds` | Game dev | The `bmm` lifecycle with GDD canvases replacing PRDs |
| `wds` | Design-first UX | Design-spec canvases feeding the `bmm` PM agent |

Installed and compiled into the full hive mirror: `core` 6.10.0, `bmm` 6.10.0,
`bmb` v2.1.0, `tea` v1.19.1, `cis` v0.2.1, `gds` v0.6.0, `wds` v0.4.3 — phase rooms,
`waggle sync`, and lazy ACP supervisor. `[BMAD]`

## 2. Event kind selection — candidates (→ architecture)

Not decided here. This is the shortlist the architecture doc must choose from and justify.

**Available and safe** `[BUZZ]` `[NOSTR]`

| Kind | Meaning | Candidate waggle use |
|---|---|---|
| `9` | Group chat message, `#h <channel-uuid>` | Artifact posts, handoffs |
| `7` | Reaction (NIP-25) | **Human gate trigger** |
| `9007` / `9008` | Create / delete group | Story-channel provisioning |
| `9000`–`9002` | Add / remove member, edit metadata | Agent channel membership |
| `39000`–`39002` | Relay-signed metadata / admin / member lists | Roster discovery |
| `9030`–`9033` | NIP-43 relay admin events | Agent npub provisioning |
| `1617` | NIP-34 patch | **Developer output** |
| `1621` | NIP-34 issue | Method-surfaced defects |
| `1630`–`1633` | NIP-34 status: Open / Applied / Closed / Draft | **Gate outcome state machine** |
| `1059` | Gift-wrapped DM (NIP-17) | Private agent coordination |

**Reserved by the host, do not collide** `[BUZZ]`: workflows `46001–46012`, job dispatch
`43001–43006`, audit `48001`.

**Avoid** `[BUZZ]`: kinds `40002` / `40003` are host-only. They work on the wire but no
standard NIP-29 client understands them, which breaks the portability claim in the brief.
Worth a compiler lint.

**Hard constraints the architecture must design around** `[BUZZ]`
- Kinds `20000–29999` are ephemeral and **never stored** — nothing auditable may live there.
- Historical `REQ` queries are capped at **500 results per filter**.
- Frame limit is **65,536 bytes**; 1024 max subscriptions per connection.
- Search excludes privacy-sensitive kinds `1059`, `30300`, `30622`.
- Client-submitted `44100` / `44101` are rejected; only the relay keypair may sign them.

## 3. Persona pack mapping — field-level seed (→ architecture)

The compiler's core transform. Source is `.claude/skills/<id>/customize.toml` `[agent]`.
`[BMAD]` → `[WAGGLE]`

| Method field | Type | Proposed persona-pack target |
|---|---|---|
| `name` | scalar, non-configurable | Nostr profile `name` (kind:0) |
| `title` | scalar, non-configurable | Profile `about` headline |
| `icon` | scalar | Profile emoji / display prefix |
| `role` | scalar | System prompt — responsibility scope |
| `identity` | scalar | System prompt — expertise |
| `communication_style` | scalar | System prompt — voice |
| `principles` | array, appends | System prompt — value system |
| `persistent_facts` | array, appends; `file:` entries are globs | Preloaded context / MCP file reads |
| `menu` | array of tables keyed by `code`; each item has exactly one of `skill` or `prompt` | Command surface → workflow trigger map |
| `activation_steps_prepend` / `_append` | arrays, append | Pre/post activation hooks |

**Merge semantics the compiler must reproduce exactly** (base → team → user): scalars
override; tables deep-merge; arrays of tables keyed by `code`/`id` replace matching entries
and append new ones; all other arrays append.

The method ships `_bmad/scripts/resolve_customization.py`, which already implements this.
Reusing it avoids drift; porting it risks silent divergence. **Open question O-4.**

**The non-uniformity, concretely.** Of TEA's ten menu items, nine carry `skill` and one
(`GATE`) carries `prompt`. A `prompt` item has no workflow to compile — it becomes an
agent-side instruction. Any claim that "menu items compile to workflows" is already false in
the pilot module. `[BMAD]`

## 4. TEA pilot detail (→ architecture, epic 1)

Agent `bmad-tea` = **Murat** 🧪, Master Test Architect and Quality Advisor. `[BMAD]`

| Code | Capability | Dispatch |
|---|---|---|
| `TMT` | Teach Me Testing (7 sessions) | `bmad-teach-me-testing` |
| `TD` | Test Design — risk assessment, NFR planning, coverage | `bmad-testarch-test-design` |
| `TF` | Test Framework — initialize architecture | `bmad-testarch-framework` |
| `CI` | Continuous Integration — scaffold quality pipeline | `bmad-testarch-ci` |
| `AT` | ATDD — failing acceptance tests + checklist | `bmad-testarch-atdd` |
| `TA` | Test Automation — prioritized tests, fixtures, DoD | `bmad-testarch-automate` |
| **`GATE`** | **Release Gate — routes audit / NFR / trace** | **`prompt` (router)** |
| `RV` | Review Tests | `bmad-testarch-test-review` |
| `NR` | NFR Evidence Audit | `bmad-testarch-nfr` |
| `TR` | Trace Coverage — requirements→tests, then gate decision | `bmad-testarch-trace` |

The gate decision is `bmad-testarch-trace` **Phase 2**, verdict vocabulary
`PASS` / `CONCERNS` / `FAIL` / `WAIVED`. `risk_threshold` defaults to `p1`, confirming the
P0–P3 priority scale the owner's locked decisions call for as event tags.

## 5. Rejected alternatives and their rationale

**Fork the substrate instead of distributing it.** Rejected by the owner as a locked
decision, and the research supports it: the upstream project is under active development
(six releases in the six days before this brief), so a fork would be stale within a week and
every upstream fix would need re-application by hand.

**Name candidates rejected.** `bmad-hive` — the placeholder — is the exact pattern the
method's trademark policy prohibits, alongside their own cited bad examples "BMadFlow" and
"BMad Studio." `apiary` — viable, but has collisions with existing infrastructure tooling.
`forager` — good agent metaphor, weaker on the auditable-log idea that is the actual thesis.
`honeycomb` was never considered; it is an established observability vendor.

**MIT for our license.** Viable and it would match the method. Rejected for Apache-2.0
because Apache matches the substrate we ship alongside, adds a patent grant, and — the
deciding factor — its section 6 explicitly withholds any trademark license, which states our
posture toward the method's marks in the license text itself rather than only in `NOTICE`.

## 6. Deferred to post-pilot

- `cis`, `gds`, `wds` module compilation
- Module publishing/installation as signed events (the `bmb` stretch goal)
- Multi-community / multi-tenant operation
- Any hosted offering
