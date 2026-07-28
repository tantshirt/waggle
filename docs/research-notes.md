# Research notes

Findings from the three reference repos, gathered before Phase 1 planning.

**Traceability legend** — every concept below is tagged with its source:

| Tag | Source |
|---|---|
| `[BUZZ]` | https://github.com/block/buzz @ `v0.4.26` |
| `[NOSTR]` | https://github.com/nostr-protocol/nips (NIP-01, NIP-25, NIP-29, NIP-34, NIP-42) |
| `[BMAD]` | https://github.com/bmad-code-org/BMAD-METHOD @ `6.10.0`, as installed into this repo |
| `[WAGGLE]` | Original to this project — no upstream equivalent |

Date gathered: 2026-07-28. Pins recorded in [`BUZZ_VERSION`](../BUZZ_VERSION).

---

## 1. Buzz — the substrate

### 1.1 What it is `[BUZZ]`

A self-hosted workspace where humans and AI agents collaborate in shared rooms.
It *is* a Nostr relay: every message, review, workflow step, and git event is a
signed entry in an immutable log. Agents are first-class members with their own
keypairs and audit trails, not bots with scoped permissions. Apache-2.0,
maintained by Block, Inc.

This is the single most important reason Buzz is the right substrate for us: the
"agent as first-class member with its own identity" premise is already the
product's thesis, so we are not fighting the grain.

### 1.2 Crate map `[BUZZ]`

| Crate | Role | Relevance to waggle |
|---|---|---|
| `buzz-core` | Zero-I/O shared types, signature verification, kind registry | Defines which event kinds are legal |
| `buzz-relay` | Axum WebSocket server; the only crate importing all others | The service we self-host, unmodified |
| `buzz-db` | Postgres event persistence, monthly range partitioning, channel/member CRUD, workflow defs | Stores our artifacts |
| `buzz-auth` | NIP-42 / NIP-98 auth, scope enforcement | How agent npubs authenticate |
| `buzz-pubsub` | Redis pub/sub fan-out, presence (90s TTL), typing | — |
| `buzz-search` | Postgres `tsvector` + GIN full-text search | Makes artifacts searchable |
| `buzz-audit` | SHA-256 hash-chain append-only log, `pg_advisory_lock` single writer | **The auditable log our mission promises** |
| `buzz-workflow` | YAML-as-code engine: 4 trigger types, 7 action types | **Deliverable 1 output target** |
| `buzz-acp` | Agent Client Protocol harness; spawns 1–32 agent subprocesses, batches @mention events per channel | **Deliverable 3 runtime** |
| `buzz-cli` | Agent-first CLI | **Deliverable 3 driver (JSON in / JSON out)** |
| `buzz-admin` | Operator CLI for membership + key management | How we provision agent npubs |
| `buzz-sdk` | Typed Nostr event builders | — |
| `buzz-media` | Blossom/S3 media storage, 50 MB limit | Artifact attachments |

### 1.3 Workflow engine — our gate substrate `[BUZZ]`

YAML-defined. Four triggers: **message**, **reaction**, **schedule**, **webhook**.
Seven actions: message, DM, topic, reaction, webhook, **approval**, delay.
Template resolution is single-pass via `evalexpr` with custom string functions and
a 100 ms timeout. Concurrency capped by an `Arc<Semaphore>` with 100 permits. Cron
scheduler evaluates window-based matching every 60 s.

The **reaction trigger + approval action** pair is exactly the shape of a method
quality gate: a human reacts to an artifact event, and the gate fires. No custom UI
is needed, which is why the locked decision to build gates this way holds up.

### 1.4 Known upstream gaps that constrain us `[BUZZ]`

These are the reason the locked decision to hide gates behind a thin interface is
correct. Logged in [`upstream-issues.md`](upstream-issues.md).

1. **Approval gate suspension is incomplete (`WF-08`).** Runs that hit an approval
   step are marked `Failed` instead of `WaitingApproval`. This directly hits our
   pilot. Our gate interface must not assume durable suspension works yet.
2. `send_dm` and `set_channel_topic` workflow actions return `NotImplemented`.
3. Rate limiter trait is defined but only a test stub is implemented.
4. No sqlx offline query cache (runtime queries only).
5. No REST endpoint for typing-state queries.

Approval tokens themselves are sound: UUID from a CSPRNG, stored as SHA-256,
single-use enforced in Postgres.

### 1.5 Local setup `[BUZZ]`

```bash
. ./bin/activate-hermit
just setup && just build
just dev            # relay + desktop; or `just relay` / `just desktop-dev` separately
```

Requires Docker and Hermit, or Rust 1.88+ / Node 24+ / pnpm 10+ / just.
Docker Compose supplies Postgres 17, Redis 7, MinIO, Prometheus, Adminer.

> ⚠️ **Local blocker for Story 1.1.** This machine has `rustc 1.79.0`; Buzz needs
> 1.88+. Hermit should provide the correct toolchain, but if we bypass Hermit we
> must bump Rust first.

### 1.6 Projects vision — canvases and the forge `[BUZZ]`

Branch channels, project channels, forum/issues, and a releases channel. **Canvases**
are living documents that humans and agents co-edit via MCP tools, and they update
when the underlying code changes. That is the natural home for our per-module canvas
templates (deliverable 2). Git runs over standard Smart HTTP with content negotiation.

Buzz's own reserved kind ranges, beyond NIP-34: workflows `46001–46012`, job dispatch
`43001–43006`, audit `48001`.

### 1.7 Agent vision `[BUZZ]`

Design principle: *"A coding agent should be small enough to hold in your head."*
Two independent binaries over standard protocols, not a shared library.
`buzz-agent` speaks ACP and orchestrates up to 8 concurrent sessions, each with
isolated MCP servers. `buzz-dev-mcp` supplies shell and file editing.

Identity is **community-scoped**: an agent's profile, presence, DMs, memories, jobs,
channel memberships, and audit trail are scoped to the community behind the relay URL.
One keypair may join several communities independently, with no state inherited across
hosts.

> ⚠️ `VISION_AGENT.md` does **not** document keypair generation, persona-pack format,
> MCP config schema, or channel-join procedure. Deliverable 3 therefore has an
> unspecified upstream contract. **Open question O-1** — resolve by reading
> `crates/buzz-persona` and `crates/buzz-cli` source directly during Story 1.1
> rather than trusting the vision doc.

---

## 2. Nostr — the protocol

### 2.1 Kinds Buzz already uses `[BUZZ]` `[NOSTR]`

| Kind | Meaning | Waggle use |
|---|---|---|
| `9` | Group chat message, `#h <channel-uuid>` | Artifact posts, handoffs |
| `7` | Reaction (NIP-25); channel derived from target's `#e` | **Human gate trigger** |
| `5` | Deletion (NIP-09), self-authored only | Retractions |
| `9007` / `9008` | Create / delete group | Story-channel provisioning |
| `9000` / `9001` / `9002` | Add / remove member, edit metadata | Agent channel membership |
| `9021` / `9022` | Join request / leave | — |
| `39000`–`39002` | Relay-signed group metadata / admin list / member list | Roster discovery |
| `44100` / `44101` | Member added / removed (relay-signed only) | Audit |
| `20001` / `20002` | Presence / typing (ephemeral, 20000–29999 not stored) | Agent liveness |
| `1059` | Gift-wrapped DM (NIP-17) | Private agent coordination |
| `13534` | Membership roster (NIP-70 protected) | — |
| `9030`–`9033` | NIP-43 relay admin events | Provisioning agent npubs |
| `40002` / `40003` | Rich content / edits — **Buzz-only, not portable** | Avoid; breaks interop |

NIPs supported: 01, 04/44, 05, 09, 10, 11, 17, 25, 29, 42, 50, 70.

### 2.2 NIP-34 — git events `[NOSTR]`

| Kind | Purpose | Required tags |
|---|---|---|
| `30617` | Repository announcement | `d` |
| `30618` | Repository state (branches/tags) | `d`, `refs` |
| `1617` | **Patch** | `a`, `r` |
| `1618` | Pull request | `a`, `c`, `clone` |
| `1619` | PR update | `a`, `c`, `E` |
| `1621` | Issue | `a`, `p` |
| `1630`–`1633` | Status: Open / Applied-Merged / Closed / Draft | `e`, `p` |
| `10317` | User grasp-server list | `g` |

Kind `1617` is the target for BMM Dev-agent output, per the locked decisions.
Kinds `1630`–`1633` are a ready-made state machine for gate outcomes.

### 2.3 Constraints that will bite `[BUZZ]`

- Ephemeral kinds `20000–29999` are **never stored** — nothing auditable may live there.
- Historical REQ queries are hard-capped at **500 results per filter**.
- Frame limit 65,536 bytes; 1024 max subscriptions per connection. **Large artifacts
  (PRD, architecture doc) exceed one frame** and must go to Blossom/S3 media with the
  event carrying a reference, not inline content. **Open question O-2.**
- Search deliberately excludes privacy-sensitive kinds `1059`, `30300`, `30622`.
- Global subscriptions are excluded from channel-scoped events as a security boundary.
- Client-submitted `44100`/`44101` are rejected; only the relay keypair may sign them.

---

## 3. BMAD Method — the methodology

### 3.1 Installed state `[BMAD]`

`npx bmad-method@6.10.0 install --modules bmm,bmb,tea --tools claude-code --all-stable`

| Module | Version | Source | Provenance |
|---|---|---|---|
| `core` | 6.10.0 | built-in | — |
| `bmm` | 6.10.0 | built-in | — |
| `bmb` | v2.1.0 | external | `bmad-builder` @ `d54981a` |
| `tea` | v1.19.1 | external | `bmad-method-test-architecture-enterprise` @ `74cf6e6` |

61 skills materialized into `.claude/skills/`.

### 3.2 ⚠️ Correction: there is no `module.yaml` `[BMAD]`

The project kickoff assumed the compiler's input was `module.yaml`. **BMAD v6.10 ships
no such file.** The actual machine-readable contract is:

| File | Content | Compiler role |
|---|---|---|
| `_bmad/config.toml` | `[core]`, `[modules.<code>]`, and `[agents.<id>]` descriptors — installer-managed, regenerated on every install, explicitly read-only | **Primary registry input** |
| `_bmad/custom/config.toml` | Team overrides, committed, never touched by installer | **Our override seam** |
| `_bmad/custom/config.user.toml` | Personal overrides, gitignored | Local only |
| `_bmad/<module>/config.yaml` | Per-module resolved config | Module settings |
| `_bmad/_config/manifest.yaml` | Install versions, module sources, SHAs, IDE list | Provenance for signed events |
| `_bmad/_config/skill-manifest.csv` | `canonicalId,name,description,module,path` — every agent and workflow | **Workflow enumeration input** |
| `.claude/skills/<id>/customize.toml` | Structured `[agent]` persona block | **Persona pack input** |
| `.claude/skills/<id>/SKILL.md` | Activation protocol prose | Behavior body |

This correction is good news. `config.toml` `[agents.*]` and `customize.toml` `[agent]`
are already structured records, so the compiler parses TOML/CSV rather than scraping
markdown.

> Note: `skill-manifest.csv` `path` values point at `_bmad/<module>/agents/<id>/SKILL.md`,
> which does **not** exist on disk — the installer materializes bodies into the IDE
> directory instead. The compiler must resolve bodies at
> `.claude/skills/<canonicalId>/SKILL.md` and treat the manifest `path` as a logical id.
> **Open question O-3:** confirm this holds for non-`claude-code` tool targets.

### 3.3 The persona contract `[BMAD]` → `[WAGGLE]`

`.claude/skills/bmad-tea/customize.toml` `[agent]` maps almost 1:1 onto a Buzz persona pack:

| BMAD field | Type | Proposed Buzz persona-pack mapping `[WAGGLE]` |
|---|---|---|
| `name` | scalar, non-configurable | Nostr profile `name` (kind:0) |
| `title` | scalar, non-configurable | Profile `about` headline |
| `icon` | scalar | Profile emoji / display prefix |
| `role` | scalar | System prompt — responsibility scope |
| `identity` | scalar | System prompt — expertise |
| `communication_style` | scalar | System prompt — voice |
| `principles` | array, **appends** on override | System prompt — value system |
| `persistent_facts` | array, appends; `file:` entries are globs loaded as facts | Preloaded context; MCP file reads |
| `menu` | array of tables keyed by `code`; each item has exactly one of `skill` or `prompt` | **Command surface → workflow trigger map** |
| `activation_steps_prepend` / `_append` | arrays, append | Pre/post activation hooks |

Merge rules the compiler must reproduce exactly, base → team → user:
**scalars override; tables deep-merge; arrays of tables keyed by `code`/`id` replace
matching entries and append new ones; all other arrays append.**

BMAD ships `_bmad/scripts/resolve_customization.py` to do this resolution. Reusing it
is preferable to reimplementing the merge and drifting. **Open question O-4:** decide
whether the compiler shells out to it or ports it.

### 3.4 TEA — the pilot module `[BMAD]`

Agent `bmad-tea` = **Murat**, 🧪, Master Test Architect and Quality Advisor.

Ten menu items, nine of which dispatch a registered skill and one (`GATE`) which is a
routing prompt:

| Code | Description | Dispatch |
|---|---|---|
| `TMT` | Teach Me Testing (7 progressive sessions) | `bmad-teach-me-testing` |
| `TD` | Test Design — risk assessment, NFR planning, coverage strategy | `bmad-testarch-test-design` |
| `TF` | Test Framework — initialize framework architecture | `bmad-testarch-framework` |
| `CI` | Continuous Integration — scaffold CI/CD quality pipeline | `bmad-testarch-ci` |
| `AT` | ATDD — failing acceptance tests + implementation checklist | `bmad-testarch-atdd` |
| `TA` | Test Automation — prioritized API/E2E tests, fixtures, DoD | `bmad-testarch-automate` |
| **`GATE`** | **Release Gate — route final audit, NFR evidence audit, trace gate decision** | **`prompt` (router, no skill)** |
| `RV` | Review Tests — quality check against written tests | `bmad-testarch-test-review` |
| `NR` | NFR Evidence Audit | `bmad-testarch-nfr` |
| `TR` | Trace Coverage — requirements→tests (Ph1), gate decision (Ph2) | `bmad-testarch-trace` |

**The gate decision is `bmad-testarch-trace` Phase 2, and its verdict vocabulary is
`PASS` / `CONCERNS` / `FAIL` / `WAIVED`.** This is the exact enum our Buzz approval-gate
layer must emit and consume. `risk_threshold` defaults to `p1`, confirming the P0–P3
priority scale the locked decisions call for as event tags.

Note that `GATE` carries a `prompt`, not a `skill` — so the compiler cannot assume
every menu item maps to a workflow. Prompt-only items compile to an agent-side
instruction, not a Buzz workflow YAML. **This is the first real generalization
hazard, and it appears in the pilot module.**

TEA config keys of interest: `risk_threshold: p1`, `test_design_output`,
`test_review_output`, `trace_output`, plus `ci_platform` / `test_framework` /
`test_stack_type` all defaulting to `auto`.

### 3.5 Other modules `[BMAD]`

`bmm` supplies the software-development team: Mary (Analyst 📊), Paige (Tech Writer 📚),
John (PM 📋), Sally (UX 🎨), Winston (Architect 🏗️), Amelia (Senior Engineer 💻).
Every one carries the same `[agents.*]` descriptor shape as `bmad-tea`, which is
strong evidence the persona mapping generalizes. `bmb` is the module builder.
`cis`, `gds`, and `wds` are **not installed yet** — they arrive in later epics.

Note all agent descriptors share `team = "software-development"`. That is a natural
seed for Buzz channel-category grouping. `[WAGGLE]`

---

## 4. Synthesis — what this means for the three deliverables

### Deliverable 1: installer/compiler `[WAGGLE]`

Input is **`_bmad/config.toml` + `_bmad/_config/skill-manifest.csv` + per-skill
`customize.toml`**, not `module.yaml`. Output is persona packs plus Buzz workflow YAML.

Firm findings:
- Persona generation is mostly a field rename (§3.3) — low risk.
- Workflow generation is **not** uniform: menu items may carry `skill` *or* `prompt`,
  and only `skill` items have a workflow to compile (§3.4).
- The compiler must reproduce BMAD's three-layer merge semantics exactly, or prefer
  reusing `resolve_customization.py` (O-4).
- Because `_bmad/config.toml` is regenerated on every install, the compiler must
  **read** it and **write** nothing into it. Our own settings belong in
  `_bmad/custom/config.toml`.

### Deliverable 2: channel & canvas templates `[WAGGLE]`

Canvases are a real Buzz concept with MCP-based co-editing (§1.6). Channels are created
via kind `9007` with `visibility` and `channel_type` tags. Large artifacts must not be
inlined — 65,536-byte frame limit (§2.3, O-2).

### Deliverable 3: agent runtime config `[WAGGLE]`

`buzz-agent` + `buzz-cli`, one session per role, each with its own npub. Identity is
community-scoped (§1.7). **The upstream contract for keypair provisioning and persona
pack format is undocumented (O-1) and must be resolved by reading source in Story 1.1.**

### Gates `[WAGGLE]`

Buzz reaction trigger (kind `7`) → workflow → approval action. TEA supplies the verdict
enum `PASS`/`CONCERNS`/`FAIL`/`WAIVED`. NIP-34 kinds `1630`–`1633` supply a portable
status state machine. **Upstream `WF-08` means durable approval suspension does not yet
work** (§1.4), so the thin gate interface is load-bearing from day one, not a
nice-to-have.

---

## 5. Open questions carried into Phase 1

| ID | Question | Resolve by |
|---|---|---|
| **O-1** | Actual `buzz-persona` pack schema, keypair provisioning, MCP config, channel-join procedure — undocumented in `VISION_AGENT.md` | Read `crates/buzz-persona` + `crates/buzz-cli` source during Story 1.1 |
| **O-2** | How to carry artifacts larger than the 65,536-byte frame limit — Blossom/S3 reference vs chunking | Architecture doc |
| **O-3** | Does the `.claude/skills/` body-resolution rule hold for non-`claude-code` tool targets? | Architecture doc |
| **O-4** | Compiler shells out to `resolve_customization.py`, or ports the merge? | Architecture doc |
| **O-5** | How do we track upstream `WF-08` so gates become durable when it lands? | `upstream-issues.md` + gate interface design |
| **O-6** | Local Rust is 1.79.0; Buzz needs 1.88+. Does Hermit fully cover this? | Story 1.1 |
