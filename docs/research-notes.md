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

---

## 6. Corrections from Story 1.1 (2026-07-28)

The walking skeleton put the research above in contact with the real substrate. Four things
we asserted were wrong, and one mapping turned out far better than assumed. Recorded here
rather than silently edited, because the *reason* we got them wrong is reusable.

### 6.1 The persona pack schema is documented — we missed it `[BUZZ]`

**We said:** undocumented upstream (O-1, UP-04), "the largest unknown in the plan."

**Reality:** `crates/buzz-persona/PERSONA_PACK_SPEC.md` is a complete 16-section
specification. Our research pass read repo-root docs (`README`, `ARCHITECTURE`, `NOSTR`,
`VISION_*`) and *listed* crate names without opening in-crate documentation.

**Method lesson:** for a Rust workspace, in-crate `*.md` is first-class documentation. Grep
the whole tree for docs before declaring anything undocumented.

**What the spec actually gives us:**

| Concept | Detail |
|---|---|
| Pack format | Superset of the [Open Plugin Spec](https://open-plugin-spec.org) — every valid pack is a valid OPS package |
| Manifest | `.plugin/plugin.json` — `personas[]`, `defaults{}`, `pack_instructions`, `mcp_config`, `hooks_config` |
| Persona file | `agents/<name>.persona.md` — YAML frontmatter (identity + behavioral config) + markdown body as the persona prompt |
| Required persona fields | `name`, `display_name`, `description` |
| Behavioral config | `subscribe`, `triggers{mentions,keywords,all_messages}`, `model`, `temperature`, `max_context_tokens`, `thread_replies`, `broadcast_replies` |
| Precedence | 5 levels: operator env > desktop UI > persona frontmatter > pack defaults > built-in |
| Merge semantics | **Shallow replacement.** Objects and arrays replace whole; no deep merge, no sub-key inheritance |
| Null semantics | `null` = absent (falls through); `[]` / `{}` = present (overrides) |
| Skills | `skills/<name>/SKILL.md`, **`name:` and `description:` both required**, silently skipped if malformed |
| Validation | `buzz pack validate` exists and is implemented |
| Distribution | `.buzzpack` zip with a **mandatory** `.sha256`; git installs; `pack.lock` |

### 6.2 The BMAD → Buzz mapping is much closer than the architecture assumed `[WAGGLE]`

The single most consequential finding. BMAD skills and Buzz pack skills are **the same
format**: `SKILL.md` with required `name:` and `description:` frontmatter. BMAD installs to
`.claude/skills/<id>/SKILL.md`; the pack spec's own skill discovery list includes
`$AGENT_CWD/.claude/skills/<skill-name>/SKILL.md`.

So the "compiler" is a smaller transform than the PRD imagined — closer to a manifest
generator plus a directory copy than a translator:

| BMAD | Buzz pack |
|---|---|
| module | one pack |
| `[agents.<id>]` descriptor + `customize.toml` `[agent]` | `agents/<id>.persona.md` frontmatter |
| `role` + `identity` + `communication_style` + `principles` | persona markdown body |
| `.claude/skills/<id>/SKILL.md` | `skills/<id>/SKILL.md` — **already compatible** |
| menu item → `skill` | a skill in the pack |
| menu item → `prompt` | text in the persona body (confirms AD-7) |

This *reduces* scope and strengthens SM-C1: there is even less excuse for module-specific
compiler branches.

### 6.3 Signatures are not readable through `buzz-cli` `[BUZZ]`

Reads are sig-stripped in every format, with no opt-in flag. FR-22's independent
verifiability cannot be built on the CLI. See **UP-07**; this is the finding with the
largest architectural consequence.

### 6.4 Toolchain: the pin is 1.95.0, and Hermit supplies it `[BUZZ]`

`README.md` says Rust 1.88+; `rust-toolchain.toml` pins **1.95.0**. The pin wins. Hermit
provides cargo 1.95.0, Node 24.14.0, pnpm 11.4.0, `just` 1.46.0 — so a machine with
`rustc 1.79.0` builds Buzz fine. **OQ-6 resolved; contributors need no system upgrade.**

### 6.5 Channel templates already exist upstream `[BUZZ]`

`buzz channels create --template <name>` "supplies default type/visibility/description/canvas,
and resolves its agent roster against the relay to add as members," reading a
`channel-templates.json`. Deliverable 2 (channel and canvas templates, FR-10/FR-25/FR-26)
may be substantially *configuration of an existing feature* rather than new construction.
Worth investigating before Epic 2 estimates are trusted.

### 6.6 Smaller corrections

- **`buzz-admin`** provides `generate-key`, `add-member`, `remove-member`, `list-members` —
  FR-11/FR-12's provisioning procedure exists and is documented in CLI help.
- **The pubkey allowlist is off by default**, so a fresh keypair can authenticate via NIP-42
  and publish without registration. `add-member` requires `BUZZ_RELAY_PRIVATE_KEY`.
- **`buzz-cli` exit codes** are `0=ok 1=input 2=relay/network 3=auth 4=other 5=write conflict`
  — a concrete precedent for AD-20's taxonomy.
- **`--format compact` is a global flag**, before the subcommand.
- **Relay queries must include explicit `kinds`** or hit the p-gate with 403.
- **Crates we had not catalogued:** `sprig` (all-in-one ACP + agent + dev-MCP harness),
  `buzz-conformance`, `buzz-relay-mesh`, `buzz-pair-relay`, `buzz-ws-client`.
- **AD-2 clarification:** `.env` is gitignored *by Buzz itself*, so editing it is
  configuration, not modification. The enforceable invariant is "no **tracked** file
  modified" — `git status --porcelain` empty.

---

## 7. Workflow engine contract, verified (Story 1.8)

Established by compiling a gate and creating it on a running relay. Schema source is
`crates/buzz-workflow/src/schema.rs` at `v0.4.26`. `[BUZZ]`

### 7.1 Shape

```yaml
name: "waggle-gate-tea"          # required, non-empty
description: "..."               # optional
enabled: true                    # defaults true
trigger:
  on: "reaction_added"           # internally tagged by `on`
  emoji: "white_check_mark"      # optional filter
steps:
  - id: "publish_gate_record"    # see 7.2
    name: "..."
    action: "send_message"       # action tag is FLATTENED onto the step
    text: "..."
```

**Triggers:** `message_posted` · `reaction_added` · `diff_posted` · `schedule` · `webhook`.
**Actions:** `send_message` · `send_dm` · `set_channel_topic` · `add_reaction` ·
`call_webhook` · `request_approval` · `delay`.

**Template variables** resolved at fire time: `{{trigger.text}}`, `{{trigger.author}}`,
`{{trigger.channel_id}}`, `{{trigger.timestamp}}`, `{{trigger.emoji}}`,
`{{trigger.message_id}}`, and `{{steps.<id>.output.<field>}}`. In `if` expressions the
same values appear flattened as `trigger_text`, `trigger_author`, and so on.

### 7.2 ⚠️ Step ids reject dashes — undocumented

`step id 'publish-gate-record' is invalid: must contain only alphanumeric characters and
underscores`

This constraint is **not stated in the schema source's doc comments**; it surfaced only at
`workflows create` time against a live relay. Anything generating workflow YAML must
sanitize ids to `[A-Za-z0-9_]`. waggle asserts it in a unit test so the emitter cannot
regress.

**Method lesson, again:** the schema type is not the contract. Validation lives in
`validate()` and in the relay, not in the struct definition. Create against a real relay
before believing a generated document is acceptable.

### 7.3 Why waggle does not emit `request_approval`

The action exists and would be the obvious way to build a gate. We do not use it:
upstream marks runs reaching an approval step as **failed** rather than suspended
(UP-01). A gate built on it would report failure for every approval.

Instead the **reaction is the approval**, and the workflow's only job is to write a
signed record into the channel. That satisfies FR-22 (reconstructible from the log alone)
directly rather than depending on substrate run state, which is what AD-10 requires. If
UP-01 is never fixed, waggle is still correct.

### 7.4 Verified gate chain

| Step | Event | Kind |
|---|---|---|
| Test Architect publishes a verdict | `waggle-gate-verdict` + verdict/priority/rationale | `9` |
| Human approves | reaction `white_check_mark` on the verdict event | `7` |
| Workflow fires | `waggle-gate-record` naming verdict event, approver, time, reaction | `9` |

All on standard kinds with typed body markers — no custom kind claimed, satisfying NFR-6
and AD-8. A third-party NIP-29 client can read the whole chain.

---

## 8. Channel and canvas templates already exist upstream (2.7/2.8 rescope)

Investigated 2026-07-29 against a live relay before writing any code, because
`buzz channels create --template` looked like it might already do most of FR-10 /
FR-25 / FR-26. It does — but not all of it, and the gaps are specific. `[BUZZ]`

### 8.1 The upstream mechanism

`buzz channels create --template <name> [--templates-file <path>]` reads a JSON store:

```json
[{
  "name": "tea-test-strategy",
  "description": "...",
  "channel_type": "stream",
  "visibility": "open",
  "canvas_template": "# Test strategy\n...",
  "agents": { "personas": [{"personaId": "bmad-tea"}], "teams": [{"teamId": "..."}] }
}]
```

Source: `crates/buzz-cli/src/commands/channel_templates.rs`.

**`--templates-file` always wins over the default path.** That is the finding that
matters: the default is the *desktop app's* app-data directory
(`<data>/xyz.block.buzz.app/templates/channel-templates.json`), which would have made
this desktop-coupled and useless to us. The override means **waggle can ship its own
template store inside the compiled pack** and never touch the desktop app.

### 8.2 Verified behaviour

| Capability | Result |
|---|---|
| Create channel from a waggle-supplied template file | ✅ headless |
| Apply `canvas_template` to the new channel | ✅ `"canvas_applied": true`, content round-trips byte-exact |
| Resolve the agent roster to members | ❌ `skipped: [{persona_id: "bmad-tea", reason: "no live instances"}]` |
| Idempotent by channel name | ❌ **creates a duplicate** |
| Degrade gracefully when the roster cannot resolve | ✅ channel + canvas still succeed; skips are reported, not silent |

### 8.3 The two real gaps

**Roster membership needs a managed-agent *record* with `persona_id` — not a Desktop
session.** Resolution scans managed-agent events (kind:30177) whose `content.persona_id`
matches, to find pubkeys. Early research treated `buzz agents draft-create` (a Desktop
human-in-the-loop flow) as the only path; that was wrong (review F-12). Kind 30177 is an
ordinary self-authored event publishable via `POST /events`. waggle ships
`waggle runtime publish-agent --persona <id>` for this. A *running* ACP/LLM process is
still required for a live conversational turn, but channel roster membership is not
blocked on Desktop.

**Provisioning is not idempotent.** Creating twice with the same name yields two
channels. FR-25 requires re-running to produce no duplicates, and NFR-2 requires
idempotence generally, so **waggle must check before creating** — Buzz will not do it.

### 8.4 Rescope

Stories 2.7 and 2.8 shrink substantially and merge into one piece of work:

| Was | Now |
|---|---|
| Build a channel-template format and a provisioner | **Emit `channel-templates.json` into each compiled pack** — the template *data* is the deliverable |
| Build a canvas-template mechanism | **Covered upstream.** Canvases come from `canvas_template` in the same file |
| Provision channels and add agents | **Thin wrapper** that checks for an existing channel first, then delegates to `buzz channels create --templates-file` |
| Agent roster membership | **Headless via kind:30177** (`waggle runtime publish-agent`); live ACP session remains an operator credential residual |

Net: 2.7 and 2.8 become mostly template authoring plus an existence check, not
mechanism-building. This is the second time Epic 2 has turned out smaller than written
(the first being that BMAD skills need no transformation at all, §6.2).

### 8.5 Team events, unexplored but promising `[WAGGLE]`

The template roster also accepts `teams: [{"teamId": ...}]`, resolved from kind `30176`
events carrying `persona_ids`. Every BMAD agent descriptor already declares
`team = "software-development"`. A single team event per module would let one template
line pull in a whole module's agents rather than listing them individually. Not pursued
yet; worth it once agent instances can run.
