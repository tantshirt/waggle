---
title: "Product Brief: waggle"
status: draft
created: 2026-07-28
updated: 2026-07-28
---

# Product Brief: waggle

> **Input mode.** The mission and architecture of this project were supplied as locked
> decisions by the project owner. This brief records and pressure-tests them; it does not
> re-derive them. Sections marked `[ASSUMPTION]` are inferences the author made to fill a
> gap and are the parts most worth challenging on review.

## Executive Summary

Teams running an agentic development method today do it inside tools that were never built
for it. The agents live in a chat client that treats them as bots, their artifacts land in a
docs folder or a wiki, their handoffs are prose someone has to read and act on, and the
quality gates are a human remembering to check. Nothing is signed. Nothing is one log. When
something ships wrong, reconstructing who decided what, on what evidence, and who approved
it means reading three systems and trusting all of them.

**waggle** is a self-hostable workspace where that entire method runs as first-class
infrastructure. Every method agent — the analyst, the PM, the architect, the developer, the
test architect — is a real member of the workspace with its own cryptographic keypair. Every
artifact it produces, every handoff it makes, and every quality gate a human approves is a
signed event in one append-only, tamper-evident log. The audit trail is not a feature bolted
on afterward; it is the substrate.

waggle gets there by not building most of it. It is a **distribution**, not a product from
scratch: it self-hosts stock [Buzz](https://github.com/block/buzz) from Block, unmodified,
and adds a thin compilation layer that turns method module definitions into things Buzz
already knows how to run — persona packs, workflow YAML, channels, canvases. Buzz already
believes agents are first-class members with their own keypairs and audit trails. The method
already defines the agents and the gates. Nobody has connected them. That connection is the
whole product.

## The Problem

An agentic development method gives a team a real structure: named agent roles with distinct
expertise, a phased lifecycle, artifacts that feed forward, and quality gates between stages.
It works. The problem is where it runs.

**The method has no home.** Agents are invoked one at a time in an IDE assistant. Each
invocation is a fresh session in one developer's terminal. The "team" is a metaphor — there
is no room they share, no way for the test architect to see what the developer just shipped
without a human copy-pasting it.

**Handoffs are lossy and unattributable.** When the scrum master hands a story to the
developer, the artifact is a markdown file and the handoff is a human deciding to act. Six
weeks later there is no record of *when* the handoff happened, what the story said at that
moment, or which agent produced which claim. Git history captures the code, not the reasoning
that produced it.

**Gates are honor-system.** The method defines real gate decisions — the test architect's
release gate returns `PASS`, `CONCERNS`, `FAIL`, or `WAIVED`. That verdict currently lives in
a generated markdown file. Nothing enforces it. Nothing records who accepted a `CONCERNS` and
why. For a team that needs to demonstrate its process to an auditor, a customer, or a
regulator, "we ran the workflow and it wrote a file" is not evidence.

**Every team rebuilds the plumbing.** Anyone who wants agents in a shared room today writes
their own bot bridge, their own state store, their own approval flow. That work is
undifferentiated and it is where the security mistakes live.

The cost of the status quo is not that the method fails. It is that the method's output is
untrustworthy at exactly the moment it matters — when someone asks how a decision was made.

## The Solution

waggle compiles a method into a running workspace. Three deliverables, layered over an
unmodified Buzz relay.

**1. The compiler.** waggle reads the installed method's own machine-readable definitions and
emits two things Buzz consumes natively: **persona packs** for the agent runtime, and
**workflow YAML** for the relay's workflow engine. This is the load-bearing piece. The method's
agent descriptors already carry name, title, icon, role, identity, communication style, and
principles as structured data — compiling them into a persona is close to a field rename. The
menu of capabilities each agent exposes becomes the map from agent command to relay workflow.

**2. Channel and canvas templates.** Each module gets the room shape it needs: story channels
for the agile lifecycle, brainstorm rooms for ideation, and canvases — Buzz's live co-edited
documents — for design specs, game design docs, and test strategies.

**3. Agent runtime config.** One agent session per method role, each with its own keypair and
npub, scoped channel memberships, and its own tool configuration. An agent is a member, not an
integration.

**Gates are the payoff.** A method quality gate becomes a Buzz approval gate: a human reacts to
an artifact event, that reaction triggers a workflow, and the workflow records the approval.
The gate verdict, the approver, the artifact it applied to, and the timestamp are all signed
events in the same log as the artifact itself. No custom UI is built, because none is needed.

## What Makes This Different

**We are not building a platform.** The differentiator is restraint. Buzz supplies the relay,
the signed event log, the hash-chained audit trail, the workflow engine with approval actions,
the agent harness, and the media storage. The method supplies the agents, the lifecycle, and
the gate semantics. waggle is the compiler and the configuration between them. The honest
moat is that this integration is genuinely non-obvious and nobody has done it — not that any
individual piece is hard.

**Signed by default, not logged by default.** Most tools that promise auditability produce a
log that the tool itself writes and could rewrite. Here, each agent holds its own key and
signs its own output, and the relay maintains a SHA-256 hash chain. Tampering with an entry
breaks every entry after it. That is a categorically stronger claim, and we get it for free by
picking the right substrate.

**Portable, not captive.** Artifacts are Nostr events on standard kinds, and developer output
uses the standard git-over-Nostr event kinds. A NIP-34 client that has never heard of waggle
can still read the repository, the patches, and their status. The audit trail outlives the tool
that produced it. This constrains us — we have to avoid the host's convenient proprietary event
kinds — and we accept that constraint deliberately.

**Self-hostable, single-tenant.** For the teams most likely to need a signed method trail —
regulated, security-conscious, or air-gapped — a SaaS is a non-starter. `docker compose up` is
the distribution model. `[ASSUMPTION]`

## Who This Serves

**Primary — the method-adopting engineering team (5–30 people).** They already run an agentic
method and feel its ceiling: work that lives in one person's terminal, handoffs by copy-paste,
gates by trust. They have a platform-minded engineer willing to run a Docker Compose stack.
Success for them is that the method becomes something the whole team participates in rather
than something each developer does alone, and that "what happened on this story" is one query.

**Primary — the quality and compliance owner.** Often the person playing the test architect
role. They own the gate decision and are accountable for it. Today they produce a verdict that
nothing enforces. Success is that their gate is a real checkpoint with a signed, attributable
record, and that producing evidence for an audit is an export rather than an archaeology
project.

**Secondary — the method-module author.** Builds custom agents and workflows for their own
domain. waggle gives them a distribution channel: a module they publish as signed events that
another team can verify and install. This is a stretch goal, not a launch requirement.

**Explicitly not served at launch:** solo developers (the coordination problem does not exist
at n=1), and teams wanting a hosted SaaS. `[ASSUMPTION]`

## Success Criteria

**The pilot is the proof.** waggle succeeds or fails on one question first: can a real method
module compile into a working persona pack and a working gate workflow against a stock,
unmodified relay? Everything else is generalization.

| # | Signal | Target |
|---|---|---|
| 1 | The pilot module compiles end-to-end to a persona pack and gate workflow, running against a stock relay | Binary — it works or it does not |
| 2 | Zero modifications to the upstream substrate | Enforced; any need becomes a logged upstream candidate |
| 3 | A gate decision is reconstructible from the signed log alone — verdict, approver, artifact, timestamp — with no other system consulted | Binary |
| 4 | Time from clean machine to a running hive with one agent posting a signed message | Under 30 minutes, documented `[ASSUMPTION]` |
| 5 | Coverage across all official method modules once the pattern generalizes | All 7 modules `[ASSUMPTION: sequenced after pilot]` |

**Leading indicator that the thesis is wrong:** if compiling the pilot module requires
special-casing rather than a general rule, the "compiler" is really a hand-written config with
extra steps, and the whole approach needs rethinking before scaling to seven modules. We
already know one such hazard exists in the pilot — see Risks.

## Scope

**In, for the first version:**
- The compiler, proven on the pilot module: module definitions → persona pack + workflow YAML
- A Docker Compose bundle that stands up a pinned, stock relay
- Keypair provisioning for agent identities
- The gate layer, behind a thin interface, with a reaction-triggered approval workflow
- Channel and canvas templates for the pilot module
- Documented setup path from clean machine to first signed message

**Explicitly out:**
- Any fork or vendoring of the substrate — it is an external service, always
- Custom UI of any kind; we use what the host already renders
- Hosted or multi-tenant operation
- Module publishing as signed events (stretch, post-pilot)
- The remaining six modules until the pilot proves the pattern generalizes

## Risks and Open Questions

Stated plainly because they are the parts most likely to move the plan.

**The substrate's approval gates are incomplete upstream.** Runs that reach an approval step
are currently marked failed rather than suspended. This lands directly on our central promise.
The mitigation — a thin gate interface so the workaround lives in exactly one place — is
load-bearing from day one rather than defensive design. *(Tracked as UP-01.)*

**The persona pack format is undocumented.** The substrate's agent vision describes the
runtime but never specifies the pack schema, keypair provisioning, or the channel-join
procedure. This is the compiler's output contract and we are inferring it from source. It is
the single largest unknown in the plan. *(Tracked as UP-04 / O-1.)*

**The compiler's input is not what we assumed.** The method ships no per-module manifest file;
the real contract is a generated config plus a skill manifest plus per-skill customization
files. This was caught during research and is a net improvement — structured data rather than
markdown scraping — but it means the input contract is installer-generated and can change
between method versions. Pinning is mandatory.

**Not every agent capability compiles to a workflow.** In the pilot module, the release-gate
menu item is a routing prompt rather than a dispatchable skill. The compiler cannot assume a
uniform mapping, and the exception appears in the very first module we compile. This is the
clearest early test of whether "compiler" is the right word for what we are building.

**Artifacts may not fit in a single event.** The relay enforces a 64 KB frame limit, and
planning artifacts routinely exceed it. Large artifacts likely need content-addressed storage
with the event carrying a reference. Unresolved. *(Tracked as O-2.)*

**Trademark constraint.** The method's marks are owned by a third party and may not be used in
our name or branding. The project name and all naming are descriptive-compatibility only. This
is settled, but it constrains marketing and discoverability permanently — people will search
for the method's name and not find us.

## Vision

If this works, the method stops being a set of prompts a developer runs and becomes the
operating system of the team.

A story opens as a channel. The analyst, PM, architect, developer, and test architect are all
in it, each with their own identity, each signing their own contributions. The developer's
output arrives as a portable patch event. The test architect posts a gate verdict backed by
evidence. A human reacts to approve, and that reaction is the gate — recorded, attributed,
permanent. Six months later, "why did we ship this" is a single query against one log, and the
answer is cryptographically verifiable.

Beyond a single team: modules become distributable as signed events, so a team can publish a
domain-specific method — regulated medical device development, game production, design-first
product work — and another team can verify its provenance and install it. The method becomes a
market rather than a monolith.

The furthest version is the boring one, and it is the one worth building toward: agentic
development becomes auditable enough to be uncontroversial. Not because a vendor promises it,
but because the log signs itself.
