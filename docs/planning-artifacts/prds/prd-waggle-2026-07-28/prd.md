---
title: waggle
status: final
created: 2026-07-28
updated: 2026-07-28
---

# PRD: waggle

## 0. Document Purpose

This PRD is for the downstream architecture and epic/story workflows, and for anyone
evaluating whether waggle's scope is coherent before implementation starts. It builds on
two documents and does not duplicate them: the **product brief**
(`docs/planning-artifacts/briefs/brief-waggle-2026-07-28/brief.md`) for problem framing and
audience, and its **addendum** for the module mapping, event-kind shortlist, and
field-level persona mapping. Upstream research with per-concept source traceability lives
in `docs/research-notes.md`; known upstream defects live in `docs/upstream-issues.md`.

Structure: Glossary-anchored vocabulary (§3), features grouped with globally-numbered FRs
nested under them (§4), cross-cutting NFRs separated out (§10), assumptions tagged inline
and indexed at the end (§16). Requirements state capabilities, not implementation — the
mechanism and transport decisions live in this run's `addendum.md` and are the architecture
doc's job to settle.

**Scope discipline note.** The mission and architecture were supplied as locked decisions by
the project owner. This PRD specifies them; it does not reopen them.

## 1. Vision

waggle turns an agentic development method into running infrastructure. Every agent the
method defines becomes a real member of a self-hosted workspace with its own cryptographic
keypair. Every artifact it produces, every handoff between roles, and every quality gate a
human approves becomes a signed event in one append-only, tamper-evident log.

It gets there by building as little as possible. waggle self-hosts **Buzz** — Block's
Nostr-based workspace — unmodified, from a pinned upstream release, and adds a thin
compilation layer that turns BMAD Method module definitions into artifacts Buzz already
knows how to consume: persona packs, workflow YAML, channels, and canvases. Buzz already
treats agents as first-class members with their own keypairs and audit trails. The BMAD
Method already defines the agent roster, the lifecycle, and the gate semantics. waggle is
the compiler and configuration between them.

The outcome a team should feel: the method stops being a set of prompts each developer runs
alone and becomes something the whole team participates in, where "how was this decided, by
whom, on what evidence, and who approved it" is one query against one cryptographically
verifiable log.

## 2. Target User

### 2.1 Jobs To Be Done

- **Run the method as a team, not as n solo sessions.** Give every method role a persistent
  identity in one shared workspace so handoffs stop being copy-paste between terminals.
- **Make a quality gate mean something.** Turn a generated verdict file into an enforced
  checkpoint with an attributable, signed approval record.
- **Answer "why did we ship this" without archaeology.** Reconstruct any decision from a
  single log rather than correlating chat, docs, and git.
- **Stand the whole thing up without becoming a platform team.** One compose bundle, a
  pinned upstream release, no forked substrate to maintain.
- **Adopt the method's newer modules without rebuilding the plumbing.** Compile a module,
  get agents and gates.

### 2.2 Non-Users (v1)

- **Solo developers.** The coordination problem waggle solves does not exist at n=1.
- **Teams wanting a hosted product.** v1 is self-host only; there is no managed offering.
  `[ASSUMPTION: self-host-only is acceptable for the launch audience]`
- **Teams not already running the BMAD Method.** waggle compiles an existing method
  installation; it is not an on-ramp to the method itself.
- **Anyone needing a custom UI.** waggle deliberately renders nothing of its own.

### 2.3 Key User Journeys

- **UJ-1. Sam stands up a hive before lunch.**
  - **Persona + context:** Sam, the platform-minded engineer on a 12-person product team,
    has been asked to "make the method work for all of us" and has half a day.
  - **Entry state:** Clean laptop, Docker installed, the team's repo already has a BMAD
    Method installation committed.
  - **Path:** Runs the compose bundle, which pulls the pinned Buzz release. Runs one waggle
    command to provision agent identities. Runs a second to compile the installed modules.
    Opens the workspace and sees the method's agents present as members.
  - **Climax:** Sam posts a message in a channel, an agent replies signed under its own
    npub, and the event is visible in the log with a verifiable signature.
  - **Resolution:** The hive is running. Sam commits the generated config and hands the
    workspace to the team.
  - **Edge case:** the pinned Buzz release fails to start because a required service is
    unhealthy — waggle reports which service and the exact upstream command to inspect it,
    rather than failing opaquely.

- **UJ-2. Murat's gate finally has teeth.**
  - **Persona + context:** Murat plays the Test Architect role. He owns the release gate and
    is accountable for it, but today his verdict is a markdown file nothing enforces.
  - **Entry state:** A story channel exists with implementation evidence already posted as
    signed events.
  - **Path:** He runs the trace workflow. It posts a gate verdict — `CONCERNS`, with the
    specific NFR evidence gaps cited — as a signed event in the story channel. The workspace
    shows the artifact awaiting approval. A human tech lead reads it and reacts to approve.
  - **Climax:** The reaction fires the gate workflow. The approval is recorded as its own
    signed event referencing the verdict event, the approver's npub, and the timestamp.
  - **Resolution:** The story's gate state is now a fact in the log, not a claim in a file.
    Six months later the `CONCERNS` acceptance is still attributable.
  - **Edge case:** the gate verdict is `FAIL` and nobody approves. The story simply does not
    advance; there is no silent bypass, and the un-actioned gate is visible.

- **UJ-3. Amelia's work lands as a portable patch.**
  - **Persona + context:** Amelia is the Senior Engineer role, working a story.
  - **Entry state:** A story channel with an accepted story spec, handed off from the Scrum
    Master role as a signed event.
  - **Path:** She implements against the spec, and her output is published to the story
    channel as a standard git-over-Nostr patch event rather than as chat text or an
    attachment.
  - **Climax:** A reviewer — and, crucially, a third-party client that has never heard of
    waggle — can read the repository, the patch, and its status.
  - **Resolution:** The patch's status advances through the standard status kinds as review
    proceeds, and the whole chain sits in the same log as the reasoning that produced it.

- **UJ-4. Dana compiles a module the pilot never anticipated.**
  - **Persona + context:** Dana adopts the ideation module six months after launch.
  - **Entry state:** A running hive; Dana installs the new module into the method
    installation.
  - **Path:** Runs the same compile command. waggle reads the module's descriptors, emits
    persona packs and workflows, and reports what it could not compile.
  - **Climax:** The new agents appear as members with their own identities, without waggle
    having shipped a line of module-specific code.
  - **Resolution:** Where a capability could not be mechanically compiled, Dana gets a named,
    specific report — not a silent omission.

## 3. Glossary

Downstream workflows and readers use these terms exactly. Introducing a synonym anywhere in
this PRD is a discipline violation.

- **Hive** — one self-hosted deployment: a pinned stock Buzz relay plus its supporting
  services plus the waggle-generated configuration applied to it. One hive, one community,
  one event log.
- **Substrate** — the unmodified upstream Buzz deployment inside a hive. Always external,
  never forked, never edited. Pinned by release tag.
- **Method installation** — a BMAD Method installation inside the user's repository. waggle's
  compiler input. Read-only to waggle.
- **Module** — one BMAD Method module (e.g. the test-architecture module). Contributes agent
  descriptors and workflow definitions to the method installation.
- **Agent descriptor** — the structured record defining one method agent: name, title, icon,
  role, identity, communication style, principles, persistent facts, and menu.
- **Menu item** — one capability an agent exposes. Carries exactly one of a **dispatchable
  reference** (names a workflow) or a **prompt** (inline instruction text, no workflow).
- **Persona pack** — waggle's compiled output describing one agent to the substrate's agent
  runtime: identity, system prompt material, and tool configuration.
- **Compiled workflow** — waggle's compiled output describing one automation to the
  substrate's workflow engine, as workflow YAML.
- **Compile** — the transform from method installation to persona packs and compiled
  workflows. Deterministic and idempotent.
- **Agent identity** — one Nostr keypair belonging to one method role in one hive. Its public
  half is the **npub**; its secret half is the **nsec**, which never leaves the operator's
  control and is never committed.
- **Artifact event** — a signed event carrying a method artifact (brief, PRD, story, test
  design, gate verdict) or a reference to one held in content-addressed storage.
- **Handoff** — a signed event marking transfer of work from one method role to another,
  referencing the artifact event it hands over.
- **Story channel** — a channel in the hive scoped to exactly one story, holding that story's
  artifact events, handoffs, and gate records.
- **Canvas** — a live co-edited document in the substrate, editable by humans and agents.
- **Gate** — a method quality checkpoint, realized as a reaction-triggered approval workflow.
- **Verdict** — a gate's decision. Vocabulary is exactly `PASS`, `CONCERNS`, `FAIL`,
  `WAIVED`.
- **Gate record** — the signed event recording an approval: the verdict it applies to, the
  approving npub, and the timestamp.
- **Gate interface** — waggle's single internal abstraction over the substrate's approval
  mechanism. The one place upstream API churn is absorbed.
- **Priority tag** — a `P0`–`P3` risk priority carried as a tag on an artifact event.

## 4. Features

### 4.1 The compiler

**Description:** waggle's core transform. It reads a method installation and emits persona
packs and compiled workflows. The compiler treats the method installation as strictly
read-only, because the installer regenerates parts of it on every install; waggle's own
settings live in the method's designated override location instead. Compilation is
deterministic — the same input produces byte-identical output — so generated configuration
can be committed and diffed. Realizes UJ-1, UJ-4.

The compiler's central risk is that its input contract is installer-generated and can change
between method versions. It therefore validates what it reads before compiling, and refuses
rather than guessing.

**Functional Requirements:**

#### FR-1: Read the method installation

An operator can point waggle at a method installation and have it enumerate every installed
module, every agent descriptor, and every workflow definition. Realizes UJ-4.

**Consequences (testable):**
- Enumerates all installed modules with their versions and provenance (source, package,
  commit reference where the module is externally sourced).
- Enumerates every agent descriptor with all its fields.
- Enumerates every workflow definition with its owning module.
- Resolves each agent's and workflow's behavioral body, following the method's own
  materialization rules rather than assuming a fixed path.
- Fails with a named, specific error if the installation is absent, unreadable, or of an
  unsupported method version — never a partial or silent read.

**Out of Scope:**
- Writing anything into the method installation.

#### FR-2: Compile agent descriptors to persona packs

waggle can compile every agent descriptor into a persona pack consumable by the substrate's
agent runtime. Realizes UJ-1, UJ-4.

**Consequences (testable):**
- Every descriptor field is either mapped into the persona pack or explicitly reported as
  dropped — no field is silently discarded.
- The pack carries the agent's display identity (name, title, icon) and its behavioral
  material (role, identity, communication style, principles).
- Persistent-fact file references are preserved as references, resolved at agent runtime
  rather than inlined at compile time, so facts stay current as the repository changes.
- A pack is produced for every agent in every installed module without module-specific code.

#### FR-3: Reproduce the method's override merge semantics exactly

The compiler resolves an agent's effective descriptor across the method's layered override
files with the same result the method itself would produce. Realizes UJ-4.

**Consequences (testable):**
- Layers resolve in base → team → user order.
- Scalars override; tables deep-merge; arrays of tables keyed by a stable identifier replace
  matching entries and append new ones; all other arrays append.
- A differential test asserts waggle's resolved descriptor equals the method's own resolver
  output for every installed agent.
- If the method's resolver is available, waggle prefers it over an independent
  implementation. `[ASSUMPTION: reuse is preferable to porting; see Open Question OQ-1]`

#### FR-4: Compile dispatchable menu items to workflows

waggle can compile each menu item that names a workflow into a compiled workflow for the
substrate's workflow engine. Realizes UJ-2, UJ-4.

**Consequences (testable):**
- Each dispatchable menu item yields exactly one compiled workflow.
- The compiled workflow declares its trigger and its actions using only trigger and action
  types the pinned substrate actually implements.
- Compilation refuses, with a named error, if a required action type is unimplemented in the
  pinned substrate version.
- Generated workflow identifiers are stable across recompiles of unchanged input.

#### FR-5: Handle non-dispatchable menu items without silent loss

waggle can compile a menu item that carries a prompt rather than a workflow reference into
agent-side instruction material, and reports it as such. Realizes UJ-4.

**Consequences (testable):**
- A prompt-carrying menu item produces no compiled workflow.
- Its instruction text is carried into the owning agent's persona pack.
- The compile report names every menu item handled this way, per agent.
- The compile does not fail on encountering one — this is expected input, not an error. The
  pilot module contains one.

#### FR-6: Report and lint the compile

An operator can see exactly what a compile produced, dropped, and flagged. Realizes UJ-4.

**Consequences (testable):**
- The report lists, per module: persona packs emitted, workflows emitted, prompt-only items
  carried, and fields dropped.
- The compile emits a portability warning when generated output would depend on a
  substrate-proprietary event kind that standard third-party clients cannot read.
- The report is available as machine-readable output for scripting.
- A compile that produced zero output for an installed module is a reported warning, not a
  silent success.

#### FR-7: Deterministic, idempotent output

Recompiling unchanged input produces byte-identical output. Realizes UJ-1.

**Consequences (testable):**
- Two compiles of an unchanged method installation produce identical bytes, including
  ordering of any generated collections.
- No timestamps, absolute paths, or machine-specific values appear in generated artifacts.
- Generated output is safe to commit and diff.

### 4.2 Hive provisioning

**Description:** The compose bundle and the commands that take an operator from a clean
machine to a running hive. The substrate is pulled at a pinned version and run as-is. waggle
applies configuration to it over its normal interfaces; it never edits its files. Realizes
UJ-1.

**Functional Requirements:**

#### FR-8: Stand up a pinned substrate

An operator can bring up a complete hive from a single compose bundle at a version pinned by
waggle. Realizes UJ-1.

**Consequences (testable):**
- The substrate version is pinned in a single declared location; no floating tags.
- Bringing the bundle up yields a reachable relay and its required supporting services.
- Service health is verified before waggle reports success.
- On failure, waggle names the unhealthy service and the upstream command to inspect it.

#### FR-9: Never modify the substrate

waggle applies no change to the substrate's source or files. Realizes UJ-1.

**Consequences (testable):**
- An integrity check asserts the substrate checkout or image is unmodified after any waggle
  operation.
- Every configuration change waggle applies goes through a documented substrate interface.
- Any capability requiring a substrate change is recorded as a logged upstream candidate,
  not implemented locally.

#### FR-10: Provision the hive's channel structure

An operator can provision a hive's channels and categories from waggle's per-module
templates. Realizes UJ-1, UJ-2.

**Consequences (testable):**
- Provisioning is idempotent — re-running does not duplicate channels.
- Channel structure reflects the installed modules, not a fixed hard-coded set.
- A story channel can be created on demand for a single story.
- Provisioning failures name the specific channel and cause.

### 4.3 Agent identity and runtime

**Description:** Each method role gets its own keypair and runs as its own session with
scoped channel membership. Identity is scoped to a single hive: the same keypair may join
another hive independently, inheriting nothing. Secret key material is the most sensitive
thing waggle handles and is treated accordingly. Realizes UJ-1.

**Functional Requirements:**

#### FR-11: Provision agent identities

An operator can generate one keypair per method role for a hive. Realizes UJ-1.

**Consequences (testable):**
- One distinct keypair per role; no shared identities.
- Secret key material is never written to a location tracked by version control, and the
  repository's ignore rules enforce this.
- Secret key material never appears in logs, compile reports, or generated configuration.
- Re-running provisioning does not silently regenerate an existing identity; overwriting
  requires an explicit destructive flag.
- The operator can export the public identities for review without exposing secrets.

#### FR-12: Register agent identities with the hive

An operator can register provisioned agent identities as members of the hive with
appropriate scope. Realizes UJ-1.

**Consequences (testable):**
- Each agent is registered with its role-appropriate membership scope.
- Registration is idempotent.
- An agent's channel memberships are scoped — an agent is not a member of every channel by
  default.
- Registration uses the substrate's own membership mechanism, satisfying FR-9.

#### FR-13: Emit agent runtime configuration

waggle emits, per role, the configuration needed to run one agent session against the hive.
Realizes UJ-1, UJ-4.

**Consequences (testable):**
- One runtime configuration per role, referencing that role's persona pack and identity.
- Tool configuration is declared per role rather than globally.
- Concurrency is bounded by configuration, so a runaway agent cannot saturate the hive.
- Configuration is generated, not hand-written, and is regenerated by recompiling.

#### FR-14: Publish agent presence and profile

Each agent's public profile in the hive reflects its persona pack. Realizes UJ-1.

**Consequences (testable):**
- An agent's displayed name, title, and icon in the hive match its descriptor.
- The profile is published under the agent's own identity, not a shared service identity.
- Recompiling an agent with a changed descriptor updates its published profile.

### 4.4 Artifacts and handoffs

**Description:** Method artifacts and role-to-role handoffs become signed events in the
story channel. Developer output uses the standard git-over-Nostr representation so the chain
stays readable by third-party clients. Artifacts that exceed the substrate's single-event
size limit are carried by reference rather than truncated. Realizes UJ-2, UJ-3.

**Functional Requirements:**

#### FR-15: Publish artifacts as signed events

An agent can publish a method artifact into a story channel as an event signed by its own
identity. Realizes UJ-2, UJ-3.

**Consequences (testable):**
- The event is signed by the producing agent's identity, not a service identity.
- The event identifies the artifact's type, its owning story, and the method module that
  produced it.
- Artifact events carry priority tags where the method assigns a priority.
- Published artifacts are retrievable and signature-verifiable from the log alone.

#### FR-16: Carry oversized artifacts by reference

An artifact exceeding the substrate's single-event size limit is published as a reference to
content-addressed storage, not truncated or dropped. Realizes UJ-2.

**Consequences (testable):**
- An artifact larger than the substrate's limit publishes successfully.
- The event carries a content hash that verifies the retrieved artifact.
- Retrieval from the reference returns the exact bytes published.
- The size threshold is derived from the pinned substrate's actual limit, not hard-coded
  independently. `[ASSUMPTION: content-addressed reference is the chosen mechanism over
  chunking; see OQ-2]`

#### FR-17: Record role-to-role handoffs

An agent can hand work to another role such that the transfer is a distinct, signed,
queryable event. Realizes UJ-2, UJ-3.

**Consequences (testable):**
- The handoff event names the originating role, the receiving role, and the artifact event
  being handed over.
- A handoff is a separate event from the artifact it transfers.
- The full handoff chain for a story is reconstructible in order from the log alone.

#### FR-18: Publish developer output as portable patch events

The developer role's output is published using the standard git-over-Nostr patch
representation. Realizes UJ-3.

**Consequences (testable):**
- Patches use the standard patch event kind and its required tags.
- Patch status advances using the standard status kinds.
- A third-party client with no knowledge of waggle can read the repository, its patches, and
  their statuses.
- Patch events are linked to the story channel and to the artifact events that motivated
  them.

### 4.5 Quality gates

**Description:** The feature the product's core promise rests on. A method gate becomes a
reaction-triggered approval workflow: a human reacts to a verdict event, the reaction fires
the workflow, and the approval is recorded as its own signed event. Everything about the
gate — verdict, approver, artifact, time — is reconstructible from the log with no other
system consulted.

This feature is built behind a single internal interface because the substrate's approval
mechanism is known to be incomplete upstream: runs that reach an approval step are currently
marked failed rather than suspended. waggle must therefore own gate state and reconcile
against the substrate, rather than reading gate state out of substrate run status — and must
be able to stop doing so, in one place, when upstream lands the fix. Realizes UJ-2.

**Functional Requirements:**

#### FR-19: Isolate the substrate's approval mechanism behind one interface

All gate behavior reaches the substrate's approval mechanism through a single internal
interface. Realizes UJ-2.

**Consequences (testable):**
- Exactly one module in the codebase calls the substrate's approval mechanism; a structural
  test enforces this.
- The interface has at least two implementations — one for current upstream behavior, one
  assuming upstream approval suspension works — selectable by configuration.
- Changing implementations requires no change to gate-consuming code.
- The pinned substrate version's known approval limitations are asserted by a test that will
  fail when upstream behavior changes, prompting a deliberate re-pin.

#### FR-20: Fire a gate from a human reaction

A human can approve a gate by reacting to the verdict event in the hive, with no custom UI.
Realizes UJ-2.

**Consequences (testable):**
- A reaction of the designated type on a verdict event triggers the gate workflow.
- Reactions by non-authorized identities do not advance the gate.
- A reaction on a non-verdict event does not fire a gate.
- The triggering reaction event is referenced by the resulting gate record.

#### FR-21: Constrain verdicts to the method's vocabulary

A gate verdict is exactly one of `PASS`, `CONCERNS`, `FAIL`, `WAIVED`. Realizes UJ-2.

**Consequences (testable):**
- A verdict event carrying a value outside the vocabulary is rejected at publish time.
- Each verdict value has defined downstream consequences for whether work advances.
- `WAIVED` and `CONCERNS` require an accompanying rationale; a bare waiver is rejected.

#### FR-22: Make the gate record self-contained

A gate decision is fully reconstructible from the log alone. Realizes UJ-2.

**Consequences (testable):**
- The gate record identifies the verdict event, the approving identity, and the time.
- A gate record's signature verifies independently.
- Reconstruction requires no database, file, or service outside the log.
- Tampering with any earlier record is detectable.

#### FR-23: Behave safely while upstream approval suspension is incomplete

While the substrate cannot durably suspend a run at an approval step, waggle does not report
a gate as passed on the strength of substrate run status. Realizes UJ-2.

**Consequences (testable):**
- A substrate run marked failed at an approval step is not interpreted as a gate failure.
- Gate state is owned by waggle and reconciled against the log, not read from run status.
- The operator is told, once and clearly, that degraded gate mode is active and why.
- A test asserts the degraded path is exercised, so it cannot rot while upstream lags.

#### FR-24: Carry risk priorities as event tags

Method-assigned risk priorities travel with artifacts as queryable tags. Realizes UJ-2.

**Consequences (testable):**
- Priority values are constrained to `P0`–`P3`.
- Artifacts can be filtered by priority from the log.
- A gate can reference the priority of what it is gating.

### 4.6 Channel and canvas templates

**Description:** Per-module room shapes. The agile module gets story channels and phase
categories; the ideation module gets brainstorm rooms; the design, game, and test modules get
canvas templates for their respective specification documents. Templates are data, not code,
so adding a module's templates does not require changing the compiler. Realizes UJ-1, UJ-4.

**Functional Requirements:**

#### FR-25: Provide per-module channel templates

Each supported module contributes a channel template describing the rooms its workflow needs.
Realizes UJ-1.

**Consequences (testable):**
- A template declares channel names, purposes, visibility, and which agent identities are
  members.
- Templates are declarative data files, not code paths.
- An unsupported or template-less module provisions nothing and is reported, not failed on.

#### FR-26: Provide per-module canvas templates

Each supported module contributes canvas templates for its specification artifacts. Realizes
UJ-4.

**Consequences (testable):**
- A canvas created from a template is co-editable by both humans and agent identities.
- Template content is versioned with waggle and identified by version in the created canvas.
- Creating a canvas from a template is idempotent per story or per scope.

### 4.7 Command-line surface

**Description:** waggle's entire interface. Every command is scriptable, and every command
that produces structured results can emit machine-readable output, because the primary
consumers are automation and agents rather than humans reading a terminal. Realizes UJ-1,
UJ-4.

**Functional Requirements:**

#### FR-27: Provide a scriptable command surface

An operator or an automation can drive every waggle capability from the command line.
Realizes UJ-1, UJ-4.

**Consequences (testable):**
- Commands cover: compile, provision identities, register identities, provision channels,
  bring the hive up, and report status.
- Every command supports machine-readable output.
- Exit codes distinguish success, user error, and system failure.
- No command requires interactive input to complete; interactivity is optional
  ergonomics.

#### FR-28: Verify version compatibility before acting

waggle verifies that the substrate and method installation match what it was pinned against,
before performing any operation that depends on their contracts. Realizes UJ-1, UJ-4.

**Consequences (testable):**
- A substrate version outside the supported range produces a clear refusal naming both the
  found and the expected version.
- A method installation version outside the supported range produces the same.
- The check can be run standalone as a preflight command.
- A compatibility override exists but is explicit and warns loudly.

## 5. Non-Goals (Explicit)

- **We are not forking or vendoring the substrate.** Ever. If it needs to change, that is an
  upstream contribution, logged as such.
- **We are not building a UI.** No web app, no desktop client, no dashboard. If a capability
  seems to need one, it is out of scope or it belongs upstream.
- **We are not becoming a hosted service.** No multi-tenant operation, no managed offering,
  no billing.
- **We are not reimplementing the method.** waggle compiles a method installation; it does
  not embed, replace, or reinterpret the method's own logic.
- **We are not building a general-purpose Nostr client.** waggle reads and writes the
  specific event shapes its features require.
- **We are not a CI system.** Gates coordinate approvals; they do not run test suites.
- **We are not shipping module-specific code paths.** A module that needs bespoke compiler
  logic is a signal the compiler abstraction is wrong.

## 6. MVP Scope

### 6.1 In Scope

- The compiler, proven end-to-end on the **test-architecture module as the pilot**: method
  installation → persona packs + compiled workflows (FR-1 through FR-7).
- A compose bundle standing up a pinned stock substrate (FR-8, FR-9).
- Agent identity provisioning and registration (FR-11, FR-12).
- Agent runtime configuration for the pilot module's agent (FR-13, FR-14).
- The gate layer behind its interface, with a working reaction-triggered approval and the
  degraded-mode behavior upstream currently forces (FR-19 through FR-23).
- Channel and canvas templates for the pilot module (FR-25, FR-26).
- The command surface and version preflight (FR-27, FR-28).
- Documented path from clean machine to first signed message.

### 6.2 Out of Scope for MVP

- **The other six modules.** Deferred until the pilot proves the pattern generalizes without
  module-specific code. This is the whole point of picking a pilot.
- **Full artifact and handoff features (FR-15 through FR-18).** The pilot needs artifact
  publication for gate verdicts; the general handoff chain and portable patch publication
  follow once the compiler is proven. `[NOTE FOR PM] FR-18 (portable patch events) is
  emotionally load-bearing — it is the clearest demonstration of the portability claim in the
  brief. Revisit for MVP inclusion if the pilot lands early.`
- **Module publishing and installation as signed events.** The module-builder stretch goal.
- **Multi-hive or multi-community operation.** One hive per deployment in v1.
- **Any managed or hosted offering.**
- **Migration tooling from an existing non-waggle method installation's history.** New work
  only; we do not backfill.

## 7. Success Metrics

**Primary**

- **SM-1: Pilot compiles clean.** The pilot module compiles to persona packs and compiled
  workflows against a stock, unmodified pinned substrate, with zero module-specific code
  paths in the compiler. Binary. Validates FR-1 through FR-7.
- **SM-2: Gate is reconstructible from the log alone.** For any completed gate, verdict,
  approving identity, gated artifact, and timestamp are all recoverable from the event log
  with no other system consulted, and signatures verify. Binary. Validates FR-19 through
  FR-22.
- **SM-3: Substrate integrity.** Zero modifications to the substrate across the full test
  suite and documented setup path; every needed change is instead a logged upstream
  candidate. Binary. Validates FR-9.

**Secondary**

- **SM-4: Time to first signed message.** Clean machine to a running hive with one agent
  identity posting one verifiable signed message: under 30 minutes following the documented
  path. `[ASSUMPTION: 30 minutes is the right bar]` Validates FR-8, FR-11, FR-27.
- **SM-5: Generalization without code.** Compiling a module the compiler was not developed
  against requires no compiler change — only templates and configuration. Validates FR-2,
  FR-4, FR-5.
- **SM-6: Nothing dropped silently.** Every descriptor field and menu item is either mapped
  or named in the compile report. Zero silent omissions. Validates FR-6.

**Counter-metrics (do not optimize)**

- **SM-C1: Compiler special-casing count.** Number of module-specific branches in the
  compiler. Should trend to zero and never be traded for pilot velocity. Counterbalances
  SM-1: making the pilot compile by special-casing it defeats the purpose of having a pilot.
- **SM-C2: Feature count in waggle's own surface.** Capability added to waggle that could
  instead live upstream, or be omitted, is a cost — not progress. Counterbalances SM-5:
  generality achieved by absorbing substrate responsibilities makes us a fork by other means.
- **SM-C3: Setup speed at the expense of key hygiene.** Do not optimize SM-4 by defaulting to
  a shared identity, committing secret material, or weakening key handling. A slower setup
  with sound key custody is the correct trade.

## 8. Integration and Dependencies

- **Substrate (Buzz).** Consumed as an unmodified pinned upstream release. Depended on for:
  the relay and event log, the tamper-evident audit chain, the workflow engine with its
  trigger and action types, the agent runtime, membership and channel management, canvases,
  and content-addressed media storage. **This is the deepest dependency in the product and
  the largest source of external risk.**
- **Method installation (BMAD Method).** Consumed read-only. Depended on for: agent
  descriptors, workflow definitions, module registry and provenance, and the override
  resolution semantics waggle must reproduce.
- **Nostr protocol.** Depended on for event format and signing, group and channel semantics,
  reactions, relay authentication, and the git-over-Nostr representation that makes developer
  output portable.
- **Container runtime.** The compose bundle assumes a working Docker-compatible runtime on
  the operator's machine.

**Dependency policy:** dependencies outside these three ecosystems require explicit owner
approval before adoption.

## 9. Audit Trail and Decision Provenance

The product's central promise, stated as requirements rather than aspiration.

- Every artifact, handoff, and gate record is signed by the identity that produced it. No
  shared service identity produces content attributable to a role.
- The log is append-only and tamper-evident; modifying any earlier record is detectable.
- Provenance for a decision includes: which agent produced it, under which module and module
  version, at what time, and which artifact events it referenced.
- Compile provenance is recorded: which method version, which module versions, and which
  substrate version produced a given generated configuration.
- Secret key material never enters the log, generated configuration, compile reports, or
  version control.
- Reconstruction of any gate decision requires the log and nothing else.

## 10. Cross-Cutting NFRs

- **NFR-1 Determinism.** All generated output is reproducible byte-for-byte from the same
  inputs.
- **NFR-2 Idempotence.** All provisioning operations are safely re-runnable without
  duplication or destructive side effects.
- **NFR-3 Substrate integrity.** No waggle operation modifies substrate files. Verified, not
  assumed.
- **NFR-4 Fail loud, fail specific.** Every failure names the specific artifact, module,
  service, or version involved. Silent partial success is a defect.
- **NFR-5 Version pinning.** Substrate release, method version, and language toolchains are
  all pinned in-repo. No floating versions anywhere.
- **NFR-6 Portability of the record.** Generated events avoid substrate-proprietary kinds
  where a standard equivalent exists, so third-party clients can read the log.
- **NFR-7 Secret hygiene.** Secret key material is never committed, logged, printed, or
  embedded in generated output.
- **NFR-8 Bounded concurrency.** Agent session concurrency is bounded by configuration, since
  the substrate's own rate limiting is not yet implemented.
- **NFR-9 Machine-first interface.** Every command that produces structured results can emit
  machine-readable output.
- **NFR-10 Upstream churn isolation.** Substrate contracts known to be in flux — approvals
  above all — are reached through a single interface.

## 11. Constraints and Guardrails

**Safety**
- Never modify the substrate. Log upstream candidates instead.
- No destructive operation without an explicit flag; identity overwrite in particular.
- Degraded modes are announced, never silent.

**Privacy**
- Agent identities are scoped to a single hive and inherit no state across hives.
- Private channel membership visibility follows the substrate's rules; waggle does not widen
  them.
- waggle does not exfiltrate repository content to any external service.

**Cost**
- Self-hosted; the operator bears infrastructure cost directly. Defaults should be modest
  enough to run on a single machine for a small team.

**Legal and trademark**
- The method's marks belong to a third party and may not be used in waggle's name, branding,
  or domain. References are descriptive-compatibility only. Attribution and the
  no-endorsement statement are carried in `NOTICE`.

## 12. Public Surface and Compatibility Policy

- **Command surface** is the public contract. Command names, their machine-readable output
  shape, and exit-code semantics are treated as a compatibility surface.
- **Generated artifact formats** (persona packs, compiled workflows) are treated as internal
  and may change with the substrate's contracts — they are regenerated, not hand-edited.
- **Event shapes** waggle publishes are a compatibility surface, because third parties may
  read the log. Changing an event's meaning is a breaking change.
- **Substrate compatibility** is declared as a supported version range, not a single tag.
  waggle refuses to operate outside it rather than degrading unpredictably.
- **Breaking changes** require a major version increment and a migration note.

## 13. Language, Runtime, and Toolchain Targets

- Toolchain versions for the substrate build path, and for waggle's own implementation, are
  pinned in-repo alongside the substrate release tag.
- The developer environment must reproduce the substrate's required toolchain versions
  regardless of what is installed globally on the machine. `[ASSUMPTION: the substrate's own
  environment tooling covers this; unverified — see OQ-6]`
- waggle's implementation language is not decided in this PRD. It is an architecture
  decision, constrained by: must parse the method's configuration formats, must be
  distributable as a single command, must not add an ecosystem outside the three named in §8.

## 14. Risks and Mitigations

| # | Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|---|
| R-1 | Upstream approval gates remain incomplete, so gates cannot durably suspend | High — known present today | **Critical**; it is the core promise | FR-19 interface + FR-23 degraded mode; assert upstream behavior with a test that fails when it changes |
| R-2 | Persona pack schema is undocumented upstream and inferred from source | High | **Critical**; it is the compiler's output contract | Resolve by reading substrate source in the first story; contribute a documentation PR upstream |
| R-3 | Method installation format changes between versions, breaking the compiler's input | Medium | High | FR-28 version preflight; pin method version; refuse rather than guess |
| R-4 | Compiler requires module-specific special-casing, invalidating the "compiler" framing | Medium — one non-uniformity already known in the pilot | High | SM-C1 tracks it explicitly; FR-5 makes the known case a first-class path rather than an exception |
| R-5 | Substrate churn generally — it is under very active development | High | Medium | Pin a release; declare a supported range; isolate volatile contracts (NFR-10) |
| R-6 | Secret key material leaks into version control or logs | Low | **Critical** | NFR-7 plus enforced ignore rules plus a test asserting no secret material in generated output |
| R-7 | Artifacts exceed event size limits and are silently truncated | Medium | High | FR-16 reference-carrying with content-hash verification |
| R-8 | Trademark constraint suppresses discoverability | Certain | Low-Medium | Accepted; descriptive compatibility language in documentation and metadata |
| R-9 | Substrate rate limiting is unimplemented, so a runaway agent can saturate a hive | Medium | Medium | NFR-8 bounded concurrency on waggle's side |

## 15. Open Questions

1. **OQ-1.** Does waggle reuse the method's own override resolver, or reimplement the merge?
   Reuse avoids drift but couples us to an internal script's interface. *(Blocks FR-3.
   Architecture decision.)*
2. **OQ-2.** How are oversized artifacts carried — content-addressed reference, chunking, or
   both depending on size? *(Blocks FR-16. Architecture decision.)*
3. **OQ-3.** What exactly is the substrate's persona pack schema, keypair provisioning
   procedure, tool configuration schema, and channel-join procedure? Undocumented upstream.
   *(Blocks FR-2, FR-11, FR-12, FR-13. Resolve by reading substrate source.)*
4. **OQ-4.** Which event kinds does waggle publish for artifact events, handoffs, and gate
   records — reusing standard kinds with tags, or claiming kinds in an unreserved range?
   *(Blocks FR-15, FR-17, FR-22. Architecture decision; constrained by NFR-6 and by the
   substrate's reserved ranges.)*
5. **OQ-5.** Which identities are authorized to fire a gate, and how is that authorization
   expressed and checked? *(Blocks FR-20.)*
6. **OQ-6.** Does the substrate's environment tooling fully provision its required toolchain,
   or must operators upgrade their machines? Locally observed toolchain is well below the
   substrate's stated requirement. *(Blocks FR-8 and SM-4.)*
7. **OQ-7.** Does the method's body-materialization rule hold across all supported agent
   tools, or only the one installed here? *(Blocks FR-1.)*
8. **OQ-8.** What is waggle's implementation language? *(Blocks §13 and all of §4.)*

## 16. Assumptions Index

Every `[ASSUMPTION]` in this document, surfaced for confirmation:

- **§2.2** — Self-host-only is acceptable to the launch audience; no hosted offering is
  needed for adoption.
- **§4.1 / FR-3** — Reusing the method's own resolver is preferable to porting the merge
  logic. Tied to OQ-1.
- **§4.4 / FR-16** — Content-addressed reference is the mechanism for oversized artifacts,
  rather than chunking. Tied to OQ-2.
- **§7 / SM-4** — 30 minutes is the right bar for clean machine to first signed message.
- **§13** — The substrate's own environment tooling provisions its required toolchain
  versions, so operators need not upgrade globally. Unverified; tied to OQ-6.

Inherited from the product brief and still unconfirmed: the non-served audience boundary
(solo developers, SaaS seekers) and the sequencing of all-seven-module coverage after the
pilot.
