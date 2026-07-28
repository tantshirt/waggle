---
title: "PRD Addendum: waggle"
status: final
created: 2026-07-28
updated: 2026-07-28
---

# PRD Addendum — waggle

Mechanism, transport, and technical-how deliberately kept out of the PRD. **Primary
consumer: the architecture doc.** Nothing here is decided — this is the option space, with
the constraints that narrow it.

Does not duplicate the brief's addendum
(`docs/planning-artifacts/briefs/brief-waggle-2026-07-28/addendum.md`), which already holds
the seven-module mapping, the event-kind shortlist with host-reserved ranges, the field-level
persona mapping, and the pilot module's menu. Read that first.

Traceability tags follow `docs/research-notes.md`: `[BUZZ]` `[NOSTR]` `[BMAD]` `[WAGGLE]`.

---

## 1. Open questions → architecture decisions, with option space

### OQ-1 — Reuse or port the override resolver? (blocks FR-3)

| Option | For | Against |
|---|---|---|
| **A. Shell out to the method's own resolver** | Zero drift by construction; the method owns its semantics | Couples us to an internal script's CLI and its runtime dependency; breaks if the method reorganizes |
| **B. Port the merge into waggle** | No external runtime; full control; testable in isolation | Silent divergence risk — a merge-rule change upstream produces wrong personas with no error |
| **C. Port, plus a differential test against the method's resolver** | Independence with a drift alarm | Requires the method's runtime present in CI anyway; more code |

Constraint: FR-3 already mandates a differential test regardless of choice, which makes C
close to free once B exists. `[BMAD]`

### OQ-2 — Oversized artifact transport (blocks FR-16)

Substrate frame limit is 65,536 bytes; media storage caps at 50 MB. `[BUZZ]`

| Option | For | Against |
|---|---|---|
| **A. Always content-addressed reference** | One code path; content hash gives verification for free; matches how the substrate handles media | Small artifacts pay an indirection they do not need; retrieval needs the media service alive |
| **B. Inline under threshold, reference above** | Small artifacts stay fully self-contained in the log | Two paths, two failure modes; the threshold becomes a compatibility surface |
| **C. Chunk across multiple events** | Everything stays in the log proper; no media dependency | Reassembly logic, ordering, partial-write states; no upstream precedent to lean on |

Note the interaction with the 500-result historical query cap `[BUZZ]` — option C makes a
single artifact consume many result slots.

### OQ-3 — Substrate agent contracts (blocks FR-2, FR-11, FR-12, FR-13)

Not an option-space question. It is a **research task**: read `crates/buzz-persona`,
`crates/buzz-agent`, `crates/buzz-cli`, and `crates/buzz-admin` at the pinned tag and write
down the actual schemas. The vision doc covers ACP, session limits, and community-scoped
identity, but never the pack schema, keypair provisioning, tool config, or channel-join
procedure. `[BUZZ]` Tracked as UP-04.

Deliverable: a schema appendix in the architecture doc, and a candidate documentation PR
upstream.

### OQ-4 — Event kinds for artifacts, handoffs, and gate records (blocks FR-15, FR-17, FR-22)

| Option | For | Against |
|---|---|---|
| **A. Standard kinds + waggle tags** (e.g. group message with typed tags) | Maximum portability — any NIP-29 client renders it; nothing to reserve | Semantics live in tags, so a naive client shows a wall of messages; weaker type safety |
| **B. Claim kinds in an unreserved range** | Clean typing; filterable by kind, which is O(1) in the substrate's subscription index | Portability loss; collision risk; must avoid host-reserved `46001–46012`, `43001–43006`, `48001` |
| **C. Standard kinds where one exists, claimed kinds only for genuine gaps** | The substrate's own stated philosophy | Requires a case-by-case ruling per event type |

NFR-6 pushes hard toward A or C. Developer output is already settled by locked decision:
NIP-34 patch kind `1617`, with status kinds `1630`–`1633` available for gate outcome state.
`[NOSTR]`

Hard constraints: nothing auditable may use ephemeral kinds `20000–29999` (never stored);
avoid host-only `40002`/`40003`; `44100`/`44101` are relay-signed only. `[BUZZ]`

### OQ-5 — Gate authorization (blocks FR-20)

Candidates: channel admin/owner role from the substrate's own admin lists (`39001`);
an explicit waggle-maintained approver set published as an event; or method-role-derived
(whoever holds a given role's npub). Interacts with FR-21 — `WAIVED` plausibly needs a
higher authorization bar than `PASS`.

### OQ-6 — Toolchain provisioning (blocks FR-8, SM-4)

Substrate requires Rust 1.88+, Node 24+, pnpm 10+, `just`, and ships environment tooling
(Hermit) intended to supply them. Locally observed: `rustc 1.79.0`, Node 26, npm 11, Docker
28.0.4, `git` 2.54. `[BUZZ]` `[WAGGLE]`

If we consume a published container image rather than building from source, this question
mostly evaporates for operators and remains only for contributors. **That trade — image vs.
source build — is itself an architecture decision and probably the higher-leverage one.**

### OQ-8 — Implementation language

| Option | For | Against |
|---|---|---|
| **A. Rust** | Same ecosystem as the substrate; single static binary; could one day share types or contribute upstream directly | Heaviest to write; TOML/CSV parsing is fine but YAML emission is fiddly; contributor pool for a config compiler is narrower |
| **B. TypeScript / Node** | Same ecosystem as the method's own installer; contributors overlap with method users; fast to write | Needs a runtime or a bundling step to be "a single command"; another ecosystem in the deployment |
| **C. Python** | The method already ships Python helper scripts and the repo already depends on `uv`; trivial reuse of the method's resolver (OQ-1 option A) | Distribution as a single command needs care; third ecosystem |

Constraint from §8: no ecosystem outside substrate / method / Nostr without owner approval.
All three options sit inside that boundary. Note C is the only one where OQ-1 option A is
free rather than awkward.

## 2. Gate layer — degraded mode sketch (→ architecture, FR-19/FR-23)

The shape the interface has to support, given that upstream marks approval-step runs as
failed rather than suspended. `[BUZZ]` UP-01.

**Two implementations behind one interface:**

- `SubstrateNativeGate` — assumes upstream durable suspension works. Reads gate state from
  substrate run status. Not selectable today; exists so the migration is a config change.
- `LogReconciledGate` — waggle owns gate state, derived by reading the event log: find
  verdict events, find approval reaction events referencing them, and pair them. Substrate
  run status is treated as advisory only. This is the MVP path.

**The alarm that keeps it honest:** FR-19 requires a test asserting the pinned substrate's
*current* (broken) approval behavior. When upstream fixes it, that test fails, which forces a
deliberate decision to re-pin and switch implementations — rather than the degraded path
quietly outliving its reason to exist.

**Why reconciling from the log is defensible and not a hack:** FR-22 already requires gate
records be fully reconstructible from the log alone. `LogReconciledGate` is that requirement's
implementation, not a workaround bolted beside it. If upstream never fixes UP-01, waggle is
still correct.

## 3. FR → architecture obligation map

What the architecture doc must specify, per the PRD's own requirement that it cover the
mapping spec, event-kind choices, keypair management, and gate firing.

| Architecture obligation | Serves |
|---|---|
| Compiler input contract: exact files read, validation rules, version gate | FR-1, FR-28 |
| Persona pack schema + field-by-field mapping table | FR-2, FR-3, FR-14 |
| Override resolution strategy + differential test design | FR-3 |
| Workflow emission: trigger/action mapping, identifier stability | FR-4, FR-7 |
| Prompt-only menu item handling | FR-5 |
| Compile report + lint schema | FR-6 |
| Determinism rules (ordering, no timestamps/paths) | FR-7, NFR-1 |
| Deployment topology, pinning mechanism, image-vs-source decision | FR-8, NFR-5, OQ-6 |
| Substrate integrity verification mechanism | FR-9, NFR-3 |
| Channel/canvas template format | FR-10, FR-25, FR-26 |
| **Keypair generation, storage, custody, and rotation** | FR-11, NFR-7 |
| Membership registration flow + scoping model | FR-12 |
| Agent runtime config schema + concurrency bounds | FR-13, NFR-8 |
| **Event-kind selection with rationale per event type** | FR-15, FR-17, FR-18, FR-22, NFR-6 |
| Oversized artifact transport | FR-16 |
| **Gate interface definition + both implementations** | FR-19, FR-23, NFR-10 |
| Reaction trigger wiring + authorization model | FR-20, OQ-5 |
| Verdict validation + rationale enforcement | FR-21 |
| Priority tag encoding | FR-24 |
| Command surface + machine-readable output schema + exit codes | FR-27, NFR-9 |
| Implementation language + distribution | §13, OQ-8 |

## 4. Deferred with owner and revisit condition

| Item | Owner | Revisit when |
|---|---|---|
| FR-15/17/18 full artifact + handoff chain | Project owner | Pilot compiles clean (SM-1) |
| FR-18 portable patch events specifically | Project owner | Pilot lands early — flagged `[NOTE FOR PM]` in PRD §6.2 as the clearest demonstration of the portability claim |
| `cis`, `gds`, `wds` module support | Project owner | SM-5 demonstrated on a second module |
| Module publish/install as signed events | Project owner | Module-builder epic reached |
| Multi-hive operation | Project owner | Second deployment demanded by a real user |
| Upstream documentation PR for persona pack schema | Project owner | OQ-3 research complete |

## 5. Reviewer-gate note

The PRD's Finalize step calls for parallel reviewer subagents (rubric walker, structural and
prose passes). **Not run** — subagent use is disabled for this session by standing
instruction. The PRD was instead written against the rubric's structure directly, and the
Essential Spine plus the Adapt-In clusters that apply (cross-cutting NFRs, constraints and
guardrails, audit trail and decision provenance, integration and dependencies, risk register,
public surface, versioning, runtime targets) are all present. Clusters deliberately dropped:
aesthetic/tone, information architecture, monetization, platform, stakeholders and approvals,
ROI, operational SLA/on-call, rollout and change management, data governance, compliance and
regulatory, hardware. Rationale: no UI, no revenue model, no hosted operation, and waggle
*enables* compliance evidence rather than being subject to a regime itself.

Worth re-running the reviewer gate before implementation if subagents become available.
