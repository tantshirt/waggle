---
stepsCompleted: [1, 2, 3, 4]
inputDocuments:
  - docs/planning-artifacts/prds/prd-waggle-2026-07-28/prd.md
  - docs/planning-artifacts/prds/prd-waggle-2026-07-28/addendum.md
  - docs/planning-artifacts/architecture/spine-waggle-2026-07-28/ARCHITECTURE-SPINE.md
  - docs/planning-artifacts/briefs/brief-waggle-2026-07-28/brief.md
  - docs/research-notes.md
  - docs/upstream-issues.md
---

# waggle - Epic Breakdown

## Overview

This document provides the complete epic and story breakdown for waggle, decomposing the
requirements from the PRD and the Architecture spine into implementable stories.

**No UX design contract exists and none is needed** — waggle deliberately renders no UI
(PRD §5, Non-Goals). The UX Design Requirements section is therefore empty by design, not by
omission.

**No starter template.** The architecture specifies a Rust workspace of six crates
(spine §Structural Seed) rather than a third-party starter, so Epic 1 Story 1 is not a
scaffold-from-template story.

## Requirements Inventory

### Functional Requirements

FR-1: Read the method installation — enumerate every module, agent descriptor, and workflow definition.
FR-2: Compile agent descriptors to persona packs.
FR-3: Reproduce the method's override merge semantics exactly.
FR-4: Compile dispatchable menu items to workflows.
FR-5: Handle non-dispatchable menu items without silent loss.
FR-6: Report and lint the compile.
FR-7: Produce deterministic, idempotent output.
FR-8: Stand up a pinned substrate.
FR-9: Never modify the substrate.
FR-10: Provision the hive's channel structure.
FR-11: Provision agent identities.
FR-12: Register agent identities with the hive.
FR-13: Emit agent runtime configuration.
FR-14: Publish agent presence and profile.
FR-15: Publish artifacts as signed events.
FR-16: Carry oversized artifacts by reference.
FR-17: Record role-to-role handoffs.
FR-18: Publish developer output as portable patch events.
FR-19: Isolate the substrate's approval mechanism behind one interface.
FR-20: Fire a gate from a human reaction.
FR-21: Constrain verdicts to the method's vocabulary.
FR-22: Make the gate record self-contained.
FR-23: Behave safely while upstream approval suspension is incomplete.
FR-24: Carry risk priorities as event tags.
FR-25: Provide per-module channel templates.
FR-26: Provide per-module canvas templates.
FR-27: Provide a scriptable command surface.
FR-28: Verify version compatibility before acting.

### NonFunctional Requirements

NFR-1: Determinism — all generated output reproducible byte-for-byte from the same inputs.
NFR-2: Idempotence — all provisioning operations safely re-runnable.
NFR-3: Substrate integrity — no waggle operation modifies substrate files; verified, not assumed.
NFR-4: Fail loud, fail specific — every failure names the artifact, module, service, or version involved.
NFR-5: Version pinning — substrate release, method version, and toolchains all pinned in-repo.
NFR-6: Portability of the record — generated events avoid substrate-proprietary kinds where a standard equivalent exists.
NFR-7: Secret hygiene — secret key material never committed, logged, printed, or embedded in output.
NFR-8: Bounded concurrency — agent session concurrency bounded by configuration.
NFR-9: Machine-first interface — every command producing structured results can emit machine-readable output.
NFR-10: Upstream churn isolation — volatile substrate contracts reached through a single interface.

### Additional Requirements

From the Architecture spine (AD ids are binding and cited by stories):

- **AD-1** `waggle-core` performs no I/O; it depends on nothing of ours.
- **AD-2** The substrate is external and immutable; CI asserts the checkout is byte-unchanged.
- **AD-3** The method installation is read-only; waggle writes only to `_bmad/custom/` and its own output directory.
- **AD-4** Compile is a pure function — no clock, env var, absolute path, or random source.
- **AD-5** The override merge is ported natively **and** a differential test against the method's own `resolve_customization.py` is mandatory and non-skippable in CI.
- **AD-6** Nothing is dropped silently; unknown fields are reported as unknown.
- **AD-7** Menu items are a sum type: dispatchable → one workflow; prompt → persona instruction material, no workflow.
- **AD-8** Event kinds standard-first; custom claims require written rationale in a committed registry; `40002`/`40003` forbidden; reserved ranges `43001`–`43006`, `46001`–`46012`, `48001` never used.
- **AD-9** Auditable state never rides an ephemeral kind (`20000`–`29999`).
- **AD-10** The event log is the authority on gate state; substrate run status is advisory. Exactly one crate may call the approval mechanism.
- **AD-11** `GateBackend` port with two adapters, plus a test asserting the pinned substrate's *current* approval behavior as a tripwire.
- **AD-12** Verdict is a closed enum; `CONCERNS` and `WAIVED` require a non-empty rationale, rejected at publish time.
- **AD-13** Gate authorization reads the relay-signed admin list (kind `39001`); `WAIVED` requires owner.
- **AD-14** Secret key material never crosses the identity port; held in a type with no debug/display formatting.
- **AD-15** Artifact transport chosen by size against the substrate's real frame limit (65,536 bytes at `v0.4.26`).
- **AD-16** Templates are data; no module-specific conditional in `waggle-core` or `waggle-emit`.
- **AD-17** The substrate image is built in waggle CI from upstream's unmodified `Dockerfile` at the pinned tag; compose references it by immutable digest.
- **AD-18** Refuse outside the supported version range; explicit, warning override only.
- **AD-19** Body materialization is discovered via the installation manifest's tool list, never a hard-coded path.
- **AD-20** Every command is machine-first with a fixed exit-code taxonomy.

Infrastructure and integration requirements:

- Upstream publishes **no relay container image** — releases are desktop binaries only. waggle CI must build one.
- Substrate build requires Rust 1.88+; waggle itself pins Rust 1.97.1. Local machine currently has 1.79.0.
- Substrate stack: Postgres 17, Redis 7, MinIO (Blossom/S3), all from upstream's own compose.
- The `serde_yaml` crate is deprecated; `serde_norway` 0.9.42 is bound instead.

Open research obligations:

- **OQ-3 / UP-04** The substrate's persona pack schema, keypair provisioning, tool config schema, and channel-join procedure are **undocumented upstream**. This is the largest unknown and blocks FR-2, FR-11, FR-12, FR-13. Resolved in Epic 1.

### UX Design Requirements

None. waggle renders no UI (PRD §5). No UX design contract was produced or required.

### FR Coverage Map

FR-1: Epic 1 (pilot module subset) → Epic 2 (full enumeration, all modules)
FR-2: Epic 1 (Murat's pack) → Epic 2 (every agent, every module)
FR-3: Epic 2 - Override merge ported with mandatory differential test
FR-4: Epic 1 (TEA gate workflow) → Epic 2 (all dispatchable items)
FR-5: Epic 1 (TEA `GATE` prompt item) → Epic 2 (general sum-type handling)
FR-6: Epic 2 - Compile report and portability lint
FR-7: Epic 2 - Deterministic, byte-identical output
FR-8: Epic 1 - Pinned hive bundle from CI-built image
FR-9: Epic 1 - Substrate integrity check in CI
FR-10: Epic 2 - Channel provisioning from templates
FR-11: Epic 1 - Agent identity provisioning
FR-12: Epic 1 - Identity registration with the hive
FR-13: Epic 1 - Agent runtime configuration
FR-14: Epic 1 - Agent profile publication
FR-15: Epic 1 (gate verdict events only) → Epic 3 (general artifact publication)
FR-16: Epic 3 - Oversized artifact transport by reference
FR-17: Epic 3 - Role-to-role handoff events
FR-18: Epic 3 - Portable patch events
FR-19: Epic 1 - `GateBackend` port and adapters
FR-20: Epic 1 - Reaction-triggered gate firing
FR-21: Epic 1 - Verdict vocabulary enforcement
FR-22: Epic 1 - Self-contained gate record
FR-23: Epic 1 - Degraded-mode gate behavior
FR-24: Epic 3 - Risk priority tags
FR-25: Epic 2 - Per-module channel templates
FR-26: Epic 2 - Per-module canvas templates
FR-27: Epic 1 (pilot commands) → Epic 2 (full surface, machine output)
FR-28: Epic 1 - Version preflight

Every FR is mapped. Where an FR appears in two epics, the **first** epic delivers it for the
pilot's vertical slice and the second generalizes it; the second never reworks the first.

## Epic List

### Epic 1: TEA pilot — a hive where Murat gates a story

Compile the TEA module to a working persona pack and gate workflow against a locally running
stock Buzz relay. After this epic an operator can stand up a hive, see the Test Architect
present as a member with its own npub, receive a signed gate verdict, approve it with a
reaction, and reconstruct the whole decision from the log alone.

**FRs covered:** FR-1 (subset), FR-2, FR-4 (subset), FR-5 (subset), FR-8, FR-9, FR-11, FR-12,
FR-13, FR-14, FR-15 (verdicts only), FR-19, FR-20, FR-21, FR-22, FR-23, FR-27 (subset), FR-28

**Why this is the first epic:** it is the whole thesis in one vertical slice. If the pilot
cannot compile without module-specific code, the "compiler" framing is wrong and everything
downstream needs rethinking. It also front-loads the largest unknown (OQ-3).

### Epic 2: Any module compiles, with no new code

Generalize the pilot into a real compiler: read any method installation, resolve overrides
with proven fidelity, compile every agent and every menu item, report what happened, and
provision rooms from template data. Proven by compiling a second module the compiler was not
developed against.

**FRs covered:** FR-1, FR-2, FR-3, FR-4, FR-5, FR-6, FR-7, FR-10, FR-25, FR-26, FR-27

**Why it is standalone:** Epic 1's hive keeps working unchanged. This epic widens what can be
compiled into it and never reworks the pilot's output.

### Epic 3: The signed trail

Turn the hive from "agents that gate" into "a complete auditable record": artifacts published
as signed events, oversized artifacts carried by verified reference, role-to-role handoffs as
first-class events, and developer output as portable git-over-Nostr patches readable by
clients that have never heard of waggle.

**FRs covered:** FR-15, FR-16, FR-17, FR-18, FR-24

**Why it is standalone:** Epics 1 and 2 remain fully functional without it. This epic adds
record types; it changes no existing behavior.

> **Beyond PRD scope.** Module authoring and publishing modules as signed events (the
> module-builder stretch goal) are explicitly out of MVP and carry no PRD FRs. They become an
> epic only after Epic 2 demonstrates SM-5.

## Validation Record

Run 2026-07-28, against the step-4 checklist.

| Check | Result |
|---|---|
| Every FR appears in the inventory | 28 of 28 |
| Every FR appears in the coverage map | 28 of 28 |
| Every NFR declared | 10 of 10 |
| Story numbering contiguous per epic | Pass — 9 / 9 / 5, no gaps |
| User-story format on every story | Pass — 23 of 23 |
| Given/When/Then on every story | Pass — 23 of 23, 3–6 ACs each |
| Template placeholders replaced | Pass — none remaining |
| Starter-template story required | N/A — architecture specifies a Rust workspace, not a starter |
| Entities created only where needed | Pass — no upfront-schema story; each story creates only what it uses |
| Forward dependencies within an epic | None — see the ordering audit below |
| Epic independence | Pass — Epics 2 and 3 each build on earlier output and neither requires a later epic |

### Story ordering audit

Each story depends only on stories before it.

- **Epic 1.** 1.1 needs nothing. 1.2 needs a running relay (1.1). 1.3 replaces 1.1's manual
  bring-up with the pinned bundle and does not depend on 1.2. 1.4 is independent. 1.5 needs a
  hive (1.1 or 1.3). 1.6 needs the schema from 1.2. 1.7 needs the identity (1.5) and the pack
  (1.6). 1.8 needs the compiler path established in 1.6. 1.9 needs the compiled gate workflow
  (1.8) and a running agent (1.7).
- **Epic 2.** Strictly linear: enumerate (2.1) → resolve (2.2) → agents (2.3) → menu items
  (2.4) → report (2.5) → determinism (2.6) → channels (2.7) → canvases (2.8) → command surface
  (2.9).
- **Epic 3.** 3.1 establishes artifact publication; 3.2–3.5 each extend it independently.

### Two judgment calls worth recording

**File churn between Epics 1 and 2 is deliberate.** Both touch `waggle-core` and
`waggle-emit`, which the checklist flags as a consolidation candidate. Consolidation was
considered and rejected: the split sits on a genuine risk boundary. Epic 1 exists to answer
whether the pilot compiles without module-specific code (SM-1, SM-C1). If the answer is no,
Epic 2's entire direction changes. Merging them would remove the feedback loop that is the
whole reason for having a pilot. Epic 2 extends Epic 1's output and never reworks it.

**Story 2.9 is a consolidation story, and that is intentional.** Earlier Epic 2 stories each
introduce the command they need, so 2.9 does not introduce the command surface from nothing —
it standardizes the envelope, the exit-code taxonomy, and non-interactivity across all of
them. Story 2.5 introduces the structured envelope for the compile report specifically; 2.9
makes one envelope shape uniform. That overlap is real and accepted rather than hidden.

## Epic 1: TEA pilot — a hive where Murat gates a story

Compile the TEA module to a working persona pack and gate workflow against a locally running
stock Buzz relay, proving the full pipeline end to end before generalizing.

### Story 1.1: Stand up a hive and post one signed message

As a waggle contributor,
I want a documented path from a clean machine to a running Buzz relay with one agent keypair posting a verifiable signed message,
So that every later story has a real substrate to build against instead of assumptions.

**Acceptance Criteria:**

**Given** a clean machine with Docker and the upstream toolchain requirements met
**When** I follow `docs/dev-setup.md` from top to bottom
**Then** a stock Buzz relay from the pinned tag `v0.4.26` is running locally
**And** no file in the Buzz checkout has been modified at any point.

**Given** a running relay
**When** I generate one Nostr keypair for an agent identity and post a message via `buzz-cli`
**Then** the message appears in the relay's event log
**And** its Schnorr signature verifies against the agent's npub
**And** the npub is recorded in `docs/dev-setup.md` as the worked example.

**Given** the documented path
**When** a second contributor follows it without assistance
**Then** they reach a signed message without needing undocumented steps
**And** any step that required improvisation is added to the document.

**Given** the local toolchain is below the substrate's stated requirement of Rust 1.88+
**When** setup runs
**Then** the document states explicitly whether upstream's environment tooling supplies the toolchain or the contributor must upgrade
**And** the resolution of OQ-6 is recorded.

### Story 1.2: Validate the persona pack contract against a real pack

> **Rescoped 2026-07-28, during implementation.** Originally "write down the substrate's
> *undocumented* agent contracts." The premise was false: the contract is fully specified in
> `crates/buzz-persona/PERSONA_PACK_SPEC.md`, which our research missed by reading only
> repo-root docs. UP-04 is withdrawn. The story became validation rather than discovery —
> the same risk, retired a different way.

As a waggle contributor,
I want the documented persona pack contract proven against a real pack and a real validator,
So that the compiler targets verified behavior rather than a spec we have only read.

**Acceptance Criteria:**

**Given** the pack specification at the pinned tag
**When** a persona pack is hand-built for the pilot module
**Then** `buzz pack validate` accepts it
**And** `buzz pack inspect` reports the expected persona, display name, triggers, and skills.

**Given** the validator accepts our pack
**When** three specific defects are injected — a missing required field, an unknown frontmatter key, and a persona file listed in the manifest but absent
**Then** each is rejected with a specific error
**And** all of this runs from one committed script, so "valid" cannot mean "the validator does nothing."

**Given** the method installation's skills
**When** they are placed in the pack unmodified
**Then** every one satisfies the substrate's required `name:` and `description:` frontmatter
**And** the format compatibility is asserted by the same script.

**Given** the verified contract
**When** the schema appendix is written
**Then** it records the authoritative field list, the BMAD-to-pack mapping, the behavioral-config precedence and merge rules, and every field not yet mapped
**And** any disagreement between the spec and the implementation is logged as an upstream issue.

### Story 1.3: Bring up a hive from a pinned upstream image

As an operator,
I want a single compose bundle that starts a hive from the publicly published Buzz
relay image, pinned by immutable digest,
So that I need only a container runtime, not the substrate's full build toolchain.

> **Rescoped 2026-07-29.** UP-08 ("no relay container image") was withdrawn —
> `ghcr.io/block/buzz` is public. AD-17's "waggle CI builds the image" pipeline is
> therefore redundant. This story pulls by digest; it does not build.

**Acceptance Criteria:**

**Given** the public image at `ghcr.io/block/buzz`
**When** waggle pins a digest in `deploy/compose/` and `BUZZ_VERSION`
**Then** the compose bundle references that digest, not a floating tag
**And** no Dockerfile build step is required of the operator.

**Given** the pinned image
**When** I run the compose bundle in `deploy/compose/`
**Then** the relay and its Postgres, Redis, and object-storage services all reach a healthy state
**And** the bundle references the relay image by immutable digest, not a floating tag.

**Given** a service that fails to become healthy
**When** bring-up is attempted
**Then** waggle names the specific unhealthy service and the upstream command to inspect it
**And** does not report success.

**Given** the full test suite has run against a hive
**When** CI checks substrate integrity
**Then** the substrate checkout is byte-unchanged
**And** the build fails if it is not.

### Story 1.4: Refuse to run against unsupported versions

As an operator,
I want waggle to check the substrate and method versions before doing anything that depends on their contracts,
So that I get a clear refusal instead of plausible but wrong output.

**Acceptance Criteria:**

**Given** a supported version range declared in one committed location
**When** I run the preflight command
**Then** it reports the found and expected versions for both the substrate and the method installation
**And** exits with the success code when both are in range.

**Given** a substrate or method version outside the supported range
**When** I run any command that depends on those contracts
**Then** waggle refuses, names both the found and the expected version, and exits with the upstream-contract error code
**And** performs no partial work.

**Given** the explicit override flag
**When** I run a command outside the supported range
**Then** the command proceeds
**And** emits a prominent warning naming the risk.

**Given** the repository
**When** it is scanned for version references
**Then** no floating tag or unpinned version reference exists anywhere.

### Story 1.5: Give the Test Architect its own identity in the hive

As an operator,
I want to provision a keypair for the Test Architect role and register it as a hive member,
So that its output is attributable to it rather than to a shared service account.

**Acceptance Criteria:**

**Given** a running hive
**When** I run the identity provisioning command for the TEA role
**Then** one distinct keypair is generated
**And** the secret key material is written only to a path covered by the repository ignore rules
**And** the command prints the npub but never the nsec.

**Given** an identity that already exists
**When** I re-run provisioning without a destructive flag
**Then** the existing identity is left untouched and the command reports it as already present
**And** overwriting requires the explicit destructive flag.

**Given** a provisioned identity
**When** I run the registration command
**Then** the npub is registered as a hive member through the substrate's own membership mechanism
**And** re-running registration produces no duplicate membership.

**Given** any generated artifact, log line, or command output
**When** it is scanned for secret key material
**Then** none is found
**And** an automated test enforces this.

### Story 1.6: Compile Murat's descriptor into a persona pack

As an operator,
I want the Test Architect's method descriptor compiled into a persona pack the substrate accepts,
So that the agent's identity and behavior come from the method rather than from hand-written configuration.

**Acceptance Criteria:**

**Given** the installed TEA module
**When** I run the compile command scoped to it
**Then** a persona pack is emitted conforming to the schema appendix from Story 1.2
**And** it carries name `Murat`, title, icon, role, identity, communication style, and all seven principles.

**Given** the descriptor's persistent-fact file references
**When** the pack is emitted
**Then** they are preserved as references rather than inlined
**And** are resolved at agent runtime, so facts stay current as the repository changes.

**Given** the descriptor's ten menu items
**When** the pack is emitted
**Then** each is accounted for in the compile output
**And** no field present in the descriptor is silently absent from the pack.

**Given** a descriptor field the compiler does not recognize
**When** compiling
**Then** it is reported as unknown rather than ignored.

### Story 1.7: See the Test Architect present as a hive member

As a team member,
I want the Test Architect running as its own session with its own npub and scoped channel membership,
So that I can talk to it in the workspace like any other member.

**Acceptance Criteria:**

**Given** a compiled persona pack and a registered identity
**When** I run the command that emits agent runtime configuration
**Then** one runtime configuration is produced for the TEA role, referencing that role's pack and identity
**And** its tool configuration is declared per role rather than globally
**And** its session concurrency is bounded by configuration.

**Given** the runtime configuration
**When** the agent session starts and authenticates to the relay
**Then** the agent appears in the hive as a member under its own npub
**And** its displayed name, title, and icon match the descriptor.

**Given** an agent present in a channel
**When** I address it there
**Then** it replies with an event signed by its own npub, not a shared identity
**And** the reply's signature verifies.

**Given** a changed descriptor
**When** I recompile and restart the session
**Then** the published profile reflects the change.

### Story 1.8: Compile the TEA release gate into a workflow

As an operator,
I want the Test Architect's release gate compiled into a Buzz workflow behind waggle's gate interface,
So that the gate is real automation rather than a generated markdown file.

**Acceptance Criteria:**

**Given** the TEA module's menu
**When** I compile it
**Then** each of the nine dispatchable menu items yields exactly one compiled workflow
**And** the `GATE` item, which carries a prompt rather than a workflow reference, yields no workflow and is instead carried into the persona pack as instruction material
**And** the compile report names `GATE` as handled that way.

**Given** a compiled workflow
**When** it is inspected
**Then** it declares only trigger and action types the pinned substrate actually implements
**And** its identifier is stable across recompiles of unchanged input.

**Given** a menu item requiring an action type the pinned substrate does not implement
**When** compiling
**Then** waggle refuses with a named error identifying the item and the missing action type.

**Given** the codebase
**When** a structural test scans for calls to the substrate's approval mechanism
**Then** exactly one crate, `waggle-gate`, contains them
**And** the test fails if any other crate does.

### Story 1.9: Approve a gate with a reaction and reconstruct it from the log

As a tech lead,
I want to approve a gate verdict by reacting to it in the workspace, and have that approval be a signed, self-contained record,
So that the gate decision is attributable and verifiable months later without consulting any other system.

**Acceptance Criteria:**

**Given** the Test Architect has run its trace workflow
**When** it publishes a gate verdict
**Then** the verdict event is signed by the agent's own npub and is one of `PASS`, `CONCERNS`, `FAIL`, `WAIVED`
**And** a verdict of `CONCERNS` or `WAIVED` without a non-empty rationale is rejected at publish time
**And** a value outside the vocabulary is rejected at publish time.

**Given** a published verdict event small enough to fit the substrate frame limit
**When** it is published
**Then** it is published inline rather than behind a storage reference, so a third-party client can read it directly.

**Given** a published verdict event
**When** an identity on the channel's relay-signed admin list reacts to it
**Then** the gate workflow fires
**And** a gate record is published identifying the verdict event, the approving npub, and the timestamp.

**Given** a reaction from an identity not on the admin list
**When** it is received
**Then** the reaction is recorded but the gate does not advance
**And** a `WAIVED` verdict requires an identity with the owner role specifically.

**Given** the substrate marks the approval-step workflow run as failed, per the known upstream defect
**When** waggle reports gate state
**Then** it derives that state from the event log rather than from run status
**And** does not report the gate as failed on the strength of run status
**And** the operator is told once, clearly, that degraded gate mode is active and why.

**Given** only the event log and no other system
**When** a completed gate is reconstructed
**Then** the verdict, the approving identity, the gated artifact, and the timestamp are all recoverable
**And** every signature verifies
**And** the degraded path is exercised by an automated test so it cannot rot while upstream lags.

## Epic 2: Any module compiles, with no new code

Generalize the pilot into a real compiler, proven by compiling a module it was not developed
against without adding compiler code.

### Story 2.1: Enumerate any method installation

As an operator,
I want waggle to read a whole method installation and enumerate every module, agent, and workflow,
So that compilation is not limited to the module the pilot was built around.

**Acceptance Criteria:**

**Given** a method installation
**When** I run the enumerate command
**Then** every installed module is listed with its version and provenance, including source, package, and commit reference for externally sourced modules
**And** every agent descriptor is listed with all its fields
**And** every workflow definition is listed with its owning module.

**Given** the installation's own manifest records which agent tool directories are in use
**When** waggle resolves an agent's or workflow's behavioral body
**Then** it resolves through the recorded tool directory rather than a hard-coded path
**And** treats the skill manifest's path field as a logical identifier.

**Given** a body that cannot be resolved
**When** enumeration runs
**Then** waggle reports a named error identifying the agent, the module, and the tool directory searched.

**Given** an absent, unreadable, or unsupported-version installation
**When** enumeration runs
**Then** waggle fails with a named, specific error
**And** performs no partial read.

**Given** any operation
**When** waggle writes to disk
**Then** it writes only under `_bmad/custom/` and its own output directory
**And** a test fails if any other path under the method installation is written.

### Story 2.2: Resolve overrides exactly as the method does

As an operator,
I want waggle's override resolution to produce the same result the method's own resolver produces,
So that a customized agent is never silently compiled into the wrong persona.

**Acceptance Criteria:**

**Given** base, team, and user override layers
**When** waggle resolves a descriptor
**Then** layers resolve in base then team then user order
**And** scalars override, tables deep-merge, arrays of tables keyed by a stable identifier replace matching entries and append new ones, and all other arrays append.

**Given** every agent in the installation
**When** the differential test runs
**Then** waggle's resolved descriptor equals the output of the method's own `resolve_customization.py` for each one
**And** the test fails the build on any inequality.

**Given** CI configuration
**When** it is inspected
**Then** the differential test cannot be skipped, excluded, or marked allowed-to-fail.

**Given** a team override that adds a menu item with a new code and replaces one with an existing code
**When** resolution runs
**Then** the new item is appended and the existing one is replaced in place
**And** neither is duplicated.

### Story 2.3: Compile every agent in every module

As an operator,
I want a persona pack for every agent in every installed module,
So that adopting a new module gives me its agents without waiting for waggle to support it.

**Acceptance Criteria:**

**Given** an installation with multiple modules
**When** I compile
**Then** one persona pack is emitted for every agent in every module
**And** no compiler code path names a specific module.

**Given** a module the compiler was not developed against
**When** it is compiled
**Then** packs are produced without any change to compiler code
**And** the only additions required are template data and configuration.

**Given** any descriptor field
**When** compiling
**Then** it is mapped into the pack, carried as instruction material, or explicitly reported as dropped with a reason
**And** no field is silently discarded.

**Given** a module that produces zero output
**When** compiling completes
**Then** it is reported as a warning
**And** is not treated as a silent success.

### Story 2.4: Compile both kinds of menu item

As an operator,
I want every agent capability to compile correctly whether it dispatches a workflow or carries a prompt,
So that no capability is lost and no capability is misrepresented as automation.

**Acceptance Criteria:**

**Given** a menu item that names a workflow
**When** it is compiled
**Then** exactly one workflow is emitted
**And** its identifier is stable across recompiles of unchanged input.

**Given** a menu item that carries a prompt
**When** it is compiled
**Then** no workflow is emitted
**And** its instruction text is carried into the owning agent's persona pack
**And** the compile does not fail — this is normal control flow, not an error.

**Given** the compile report
**When** it is read
**Then** it names every prompt-carrying menu item, per agent.

**Given** a menu item carrying neither a workflow reference nor a prompt, or both
**When** it is compiled
**Then** waggle reports a named error identifying the agent and the item code.

### Story 2.5: See exactly what a compile did

As an operator,
I want a report of everything a compile produced, carried, dropped, and flagged,
So that I can trust the output without reading the generated files.

**Acceptance Criteria:**

**Given** a completed compile
**When** I read the report
**Then** it lists, per module, the persona packs emitted, workflows emitted, prompt-only items carried, and fields dropped with reasons.

**Given** generated output that would depend on a substrate-proprietary event kind
**When** the compile runs
**Then** a portability warning is emitted naming the kind and the affected output.

**Given** output using a kind in a substrate-reserved range
**When** the compile runs
**Then** the compile fails with a named error.

**Given** any compile
**When** I request machine-readable output
**Then** the report is emitted in a structured, versioned envelope suitable for scripting.

### Story 2.6: Get byte-identical output from unchanged input

As an operator,
I want recompiling unchanged input to produce identical bytes,
So that I can commit generated configuration and review it in diffs.

**Acceptance Criteria:**

**Given** an unchanged method installation
**When** I compile twice
**Then** the two outputs are byte-identical, including the ordering of every generated collection.

**Given** any generated artifact
**When** it is inspected
**Then** it contains no timestamp, absolute path, hostname, or other machine-specific value.

**Given** two different machines with the same inputs
**When** each compiles
**Then** both produce the same bytes
**And** a snapshot test asserts this in CI.

**Given** the compile transform
**When** its dependencies are inspected
**Then** it reads no clock, environment variable, or random source.

### Story 2.7: Emit and apply channel + canvas templates

> **Rescoped 2026-07-29 after investigation, before writing code.** Stories 2.7 and 2.8
> assumed waggle would build a channel-template format *and* a canvas mechanism *and* a
> provisioner. Buzz already ships all three: `buzz channels create --template` reads a
> JSON store, and `--templates-file` lets waggle supply its own, so nothing is coupled to
> the desktop app. Verified on a live relay — channel creation and canvas application both
> work headlessly, and the canvas round-trips byte-exact. See `docs/research-notes.md` §8.
>
> **Story 2.8 is absorbed into this one.** Canvases are a field in the same template file;
> there is no separate mechanism to build.
>
> Two gaps remain waggle's job: Buzz is **not idempotent** by channel name (UP-10), and
> roster membership needs a **live managed agent** (same blocker as Story 1.7).

As an operator,
I want each module's channels and canvases created from templates waggle ships,
So that adopting a module gives me its rooms without hand-building any of them.

**Acceptance Criteria:**

**Given** a compiled module pack
**When** I inspect it
**Then** it contains a `channel-templates.json` in the shape Buzz's template loader reads
**And** each template declares name, description, channel type, visibility, and a canvas.

**Given** that template file
**When** I provision a module's channels
**Then** waggle delegates to the substrate's own template mechanism via `--templates-file`
**And** the created channel's canvas matches the template exactly
**And** nothing depends on the Buzz desktop application.

**Given** a channel that already exists for the requested name
**When** provisioning runs again
**Then** waggle reports the existing channel and creates no duplicate
**And** this check is waggle's own, because the substrate does not deduplicate (UP-10).

**Given** a template whose agent roster cannot resolve because no agent instance is running
**When** provisioning runs
**Then** the channel and canvas are still created
**And** each unresolved persona is reported by name with its reason, never silently dropped.

**Given** a module with no template file
**When** provisioning runs
**Then** nothing is provisioned for it and the omission is reported
**And** the command does not fail.

### Story 2.9: Drive every capability from the command line

As an automation author,
I want every waggle capability available as a scriptable command with machine-readable output,
So that agents and CI can drive waggle without a human reading a terminal.

**Acceptance Criteria:**

**Given** the command surface
**When** I list it
**Then** it covers compile, provision identities, register identities, provision channels, bring the hive up, preflight, and report status.

**Given** any command producing structured results
**When** I pass the machine-readable output flag
**Then** structured output goes to stdout and diagnostics go to stderr
**And** the output uses one versioned envelope shape shared across commands.

**Given** any command
**When** it is run without a terminal attached
**Then** it completes without requiring interactive input.

**Given** a command that fails
**When** it exits
**Then** its exit code distinguishes user error, upstream-contract error, and system failure
**And** the taxonomy is documented.

## Epic 3: The signed trail

Turn the hive into a complete auditable record of the method's work.

### Story 3.1: Publish any method artifact as a signed event

As a team member,
I want each method artifact published into its story channel as an event signed by the agent that produced it,
So that every artifact is attributable and verifiable.

**Acceptance Criteria:**

**Given** an agent producing an artifact
**When** it publishes into a story channel
**Then** the event is signed by that agent's own npub, not a service identity
**And** it identifies the artifact type, the owning story, and the producing module.

**Given** a published artifact
**When** it is retrieved from the log
**Then** its signature verifies
**And** the producing module's version is recoverable from the event.

**Given** any artifact, handoff, or gate event
**When** its kind is inspected
**Then** it is not in the ephemeral range `20000`–`29999`
**And** it is not a substrate-proprietary kind.

**Given** a kind claimed by waggle rather than reused from a standard
**When** the kind registry is inspected
**Then** that kind has a committed written rationale
**And** it falls outside every substrate-reserved range.

### Story 3.2: Publish artifacts larger than one event

As a team member,
I want artifacts too large for a single event published by verified reference,
So that a long PRD or architecture document is never truncated or silently dropped.

**Acceptance Criteria:**

**Given** an artifact whose serialized event exceeds the substrate's frame limit
**When** it is published
**Then** it is published as a content-addressed reference carrying a hash
**And** publication succeeds.

**Given** a published reference
**When** the artifact is retrieved and hashed
**Then** the hash matches the one in the event
**And** the retrieved bytes are exactly the bytes published.

**Given** an artifact that fits within the frame limit
**When** it is published
**Then** it is published inline rather than by reference.

**Given** the size threshold
**When** it is inspected
**Then** it is derived from the pinned substrate's actual limit
**And** is not independently hard-coded.

### Story 3.3: Record a handoff between roles

As a team member,
I want each transfer of work between method roles recorded as its own signed event,
So that the chain of custody for a story is reconstructible in order.

**Acceptance Criteria:**

**Given** one role handing work to another
**When** the handoff is published
**Then** it is a separate event from the artifact it transfers
**And** it names the originating role, the receiving role, and the artifact event being handed over.

**Given** a story's full history
**When** its handoffs are queried from the log
**Then** the complete chain is reconstructible in order
**And** requires no system outside the log.

**Given** a handoff referencing an artifact event that does not exist
**When** it is published
**Then** it is rejected with a named error.

### Story 3.4: Publish developer output as a portable patch

As a reviewer,
I want developer output published as a standard git-over-Nostr patch,
So that even a client that has never heard of waggle can read the repository, the patch, and its status.

**Acceptance Criteria:**

**Given** developer output for a story
**When** it is published
**Then** it uses the standard patch event kind with its required tags
**And** it is linked to the story channel and to the artifact events that motivated it.

**Given** a published patch
**When** its review progresses
**Then** its status advances using the standard status kinds.

**Given** a third-party client with no knowledge of waggle
**When** it reads the relay
**Then** it can resolve the repository, its patches, and their statuses.

### Story 3.5: Filter the log by risk priority

As a quality owner,
I want method-assigned risk priorities carried as tags on artifact events,
So that I can find everything at a given priority without reading each artifact.

**Acceptance Criteria:**

**Given** an artifact the method assigned a priority
**When** it is published
**Then** the event carries that priority as a tag
**And** the value is one of `P0`, `P1`, `P2`, `P3`.

**Given** a priority value outside that set
**When** publication is attempted
**Then** it is rejected with a named error.

**Given** a hive with artifacts at several priorities
**When** I query the log filtered by priority
**Then** only artifacts at that priority are returned.

**Given** a gate
**When** its record is inspected
**Then** it can reference the priority of the artifact it gated.
