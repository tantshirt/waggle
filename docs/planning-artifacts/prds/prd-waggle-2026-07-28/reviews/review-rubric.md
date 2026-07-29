---
title: "PRD Quality Review — waggle"
reviewer: independent reviewer-gate subagent (not an author)
prd: docs/planning-artifacts/prds/prd-waggle-2026-07-28/prd.md
reviewed: 2026-07-29
basis: prd.md, addendum.md, .memlog.md, brief.md, prd-validation-checklist.md,
  plus post-hoc evidence — sprint-status.yaml, research-notes.md §6/§7/§8,
  upstream-issues.md UP-01..UP-17, persona-pack-contract.md, epics.md
---

# PRD Quality Review — waggle

> **Standing of this review.** The PRD's own Finalize step called for a reviewer gate; the
> addendum §5 records it as **"Not run — subagent use is disabled for this session"**, and
> `.memlog.md` line 16 logs it as an `(override)`. The document was nonetheless flipped to
> `status: final` and owner-approved **"as-is, no changes requested"** (memlog line 18). This
> is that skipped gate, run late. It has the unfair advantage of ~6 months of implementation
> evidence, and it uses it — the interesting question is not "is this PRD well-formed" (it
> largely is) but "which of its sentences were true."

## Overall verdict

As a *document* this PRD is above average: a real thesis, a glossary that downstream actually
used, 28 contiguous FRs that mapped 28-of-28 into epics with no orphans, and testable
consequences under nearly every FR. As a *specification of a system built on someone else's
substrate* it is unfounded in a specific, repeating way: it states substrate capabilities as
fact, tags none of them as assumptions, and assigns no requirement anywhere the obligation to
verify one before building against it. Four such statements were wrong — FR-16's size limit
and its carrier mechanism, FR-2's persistent-fact resolution, R-2/OQ-3's "undocumented"
persona schema, and the unstated premise under all of §4.3 that an agent instance can be
created headlessly — and the last of those has three stories (1.5, 1.7, 1.9) still open in
`sprint-status.yaml` against an epic-1 that is the whole thesis. §16 is a clean roundtrip of
the five inline tags and is nonetheless the least honest section in the document, because it
indexes preferences ("30 minutes is the right bar") and omits every capability claim that
actually broke.

## Decision-readiness — adequate

A decision-maker could act on this. The locked decisions are stated as locked (§0 "The mission
and architecture were supplied as locked decisions… This PRD specifies them; it does not
reopen them"), the pilot-first sequencing is argued rather than asserted (§6.2 "This is the
whole point of picking a pilot"), and the counter-metrics in §7 name real sacrifices —
SM-C3's "A slower setup with sound key custody is the correct trade" is a genuine trade-off
with a loser named. §14's nine-row register is not decoration; R-1 correctly identifies the
upstream approval defect as **"Critical; it is the core promise"** and R-4 admits the
generalization hazard is already present in the pilot. That is more nerve than most PRDs show.

Where it fails is at the two decisions that were actually load-bearing and got no callout.
First: **`[NOTE FOR PM]` appears exactly once in 821 lines**, at §6.2 on FR-18 — a deferral
nobody disputed, already agreed in the brief's scope section, and re-listed in addendum §4
with an owner and a revisit condition. The rubric names this failure mode by name
("`[NOTE FOR PM]` callouts at real tensions, not at safe checkpoints"). The real tensions went
unflagged: OQ-8 declares the implementation language undecided and **"Blocks §13 and all of
§4"** — a PRD that marks itself `final` while conceding an open question blocks its entire
requirements section should say so out loud, and does not. Second: §4.5 rests the product's
"core promise" on a substrate defect (UP-01) with no `[NOTE FOR PM]` asking the owner whether
a promise that depends on someone else's unfixed bug is a promise worth making in v1.

Second failure: the risk register is complete about the risks that were already known and
silent about the class that bit. R-1 through R-9 contain **no row for "a substrate capability
we assumed exists does not exist or does not work headlessly."** That is precisely the class
that produced UP-07, UP-10, UP-14, UP-16, UP-17 and the stalled agent-runtime stories. R-5
("Substrate churn generally") is not that row — churn is change over time; this was wrongness
at the pin.

### Findings

- **critical** No requirement, risk, or assumption covers headless agent instantiation (§4.3, UJ-1, §14) — §4.3's description asserts "Each method role gets its own keypair and **runs as its own session**", FR-13 requires "the configuration needed to run one agent session against the hive", FR-14 requires a published profile, and UJ-1's climax is "an agent replies signed under its own npub". None of this is reachable: per `research-notes.md` §8.3, agent instances come from `buzz agents draft-create`, which *"opens a prefilled create-agent form in the owner's Buzz Desktop"* — **a human-in-the-loop desktop flow with no headless equivalent.** This collides head-on with §5's "We are not building a UI" and FR-27's "No command requires interactive input to complete." `sprint-status.yaml` shows the consequence: `1-5`, `1-7`, `1-9` all `in-progress` while epics 2 and 3 are `done` — the pilot epic that *is* the thesis is the one blocked. *Fix:* add an FR or explicit `[ASSUMPTION]` that agent session instantiation has a headless path, make it a Story-1.1 verification obligation, and add a risk row with a fallback (contribute a headless `agents create` upstream, or scope v1 to a human-instantiated agent).
- **high** R-2 rates a Critical risk on an unverified negative that was false (§14 R-2, §15 OQ-3) — "Persona pack schema is undocumented upstream and inferred from source… **Critical**; it is the compiler's output contract." It was never undocumented. `crates/buzz-persona/PERSONA_PACK_SPEC.md` is a complete 16-section spec (`upstream-issues.md` UP-04, now `withdrawn`; `research-notes.md` §6.1). The research pass read repo-root docs and *listed* crate names without opening in-crate documentation. A PRD is entitled to record an unknown; it is not entitled to promote "we did not find it" to "it does not exist" and then rate that Critical. *Fix:* require negative claims about a dependency to name the search performed ("grepped the full tree for `*.md`"), and downgrade any unverified negative to an `[ASSUMPTION]` rather than a risk-register fact.
- **medium** OQ-8 blocks all of §4 while the document is `final` (§15 OQ-8, §13) — "*(Blocks §13 and all of §4.)*" is a remarkable sentence to leave un-callouted in an approved artifact. *Fix:* either scope the block honestly (it blocks *implementation*, not requirement acceptance) or carry a `[NOTE FOR PM]`.
- **low** The only `[NOTE FOR PM]` sits at the safest available decision (§6.2, FR-18) — duplicated in addendum §4 with owner and revisit condition, so it carries no information the addendum lacks.

## Substance over theater — adequate

Very little furniture here, and that is worth saying plainly. §1's vision would not swap into
another PRD — "waggle is the compiler and configuration between them" is a specific and
falsifiable claim about this product. The four UJs each carry a named protagonist with entry
state, climax, and an edge case, and UJ-2's edge case ("the gate verdict is `FAIL` and nobody
approves. The story simply does not advance; there is no silent bypass") does real work. The
NFRs are not boilerplate: NFR-8 justifies itself against a named upstream gap ("since the
substrate's own rate limiting is not yet implemented", = UP-03, which held), and NFR-6
constrains a real choice rather than asserting a virtue. §5's non-goals mostly bite.

Three places are theater, in decreasing severity.

**FR-19's second implementation is mandated speculative generality.** The FR requires "at
least two implementations — one for current upstream behavior, one assuming upstream approval
suspension works — selectable by configuration," and addendum §2 names them
`LogReconciledGate` and `SubstrateNativeGate`. But `research-notes.md` §7.3 — written *after*
verifying the workflow engine — concludes waggle does not emit `request_approval` at all: "the
**reaction is the approval**, and the workflow's only job is to write a signed record into the
channel… **If UP-01 is never fixed, waggle is still correct.**" The dormant implementation is
therefore not waiting on a fix; it is waiting on a need that will not arrive, because the
design that satisfies FR-22 does not consult run status even when run status works. An FR that
mandates building an adapter for a path the architecture has already abandoned is exactly what
SM-C2 ("capability added to waggle that could instead live upstream, or be omitted, is a cost")
was written to catch, and SM-C2 did not catch it because SM-C2 is unmeasurable (below).

**The counter-metrics are earned in concept and unmeasurable in practice.** SM-C1 is a count
and could be a real gate — but "Should trend to zero and never be traded for pilot velocity"
sets no threshold, no cadence, and no owner. SM-C2 and SM-C3 are not metrics at all; they are
policies phrased as metrics ("is a cost — not progress"). Nothing about SM-C2 would have fired
when FR-25/FR-26 turned out to duplicate an upstream feature, or when UP-07 forced waggle to
implement its own relay-protocol client.

**§5's non-goals include one that cannot be violated.** "We are not building a general-purpose
Nostr client. waggle reads and writes the specific event shapes its features require" —
the qualifier makes the goal unfalsifiable. As it happens, UP-07 and UP-13 forced
`waggle-hive` to speak the relay protocol directly with NIP-42/NIP-98 auth and full tag
control; whether that crossed the line is unanswerable against the text as written.

### Findings

- **high** FR-19 requires a second gate implementation with no reachable purpose (§4.5 FR-19) — `research-notes.md` §7.3 establishes the reaction-as-approval design is correct independent of UP-01, so `SubstrateNativeGate` is dead code the PRD contractually obliges. *Fix:* restate FR-19 as "exactly one module calls the substrate's approval mechanism" (its first consequence, which is the valuable one) and drop the two-implementations consequence, or condition it on an explicit trigger.
- **medium** Counter-metrics have no thresholds, cadence, or owner (§7 SM-C1..SM-C3) — SM-C2's job was to catch scope waggle should not have absorbed; FR-25/FR-26 (upstream already did it) and the `waggle-hive` relay client (forced by UP-07/UP-13) both slipped past it. *Fix:* give SM-C1 a number and a review point ("reviewed at each epic retrospective; >0 requires written justification"); convert SM-C2/SM-C3 into guardrails in §11 where policies belong.
- **low** One non-goal is written so it cannot be falsified (§5, "general-purpose Nostr client").

## Strategic coherence — strong

This is the PRD's best dimension and it should be defended. There is a stated thesis —
"It gets there by building as little as possible" (§1) — and the document is organized to bet
on it: the pilot exists to falsify the compiler framing before seven modules are attempted
(§6.2), SM-1 is binary and tests exactly that, SM-C1 exists to stop the team from faking SM-1,
and §5's non-goals fence the thesis on all four sides (no fork, no UI, no SaaS, no
reimplementation of the method). The scope kind is coherently *platform/capability*, and the
MVP cut matches: prove the transform, defer the record types. This is not a backlog with
section headings.

The one strategic gap follows directly from the thesis and is worth naming precisely: **the
thesis "build as little as possible" was never turned into an obligation on any requirement.**
No FR says "before specifying a capability, establish that the substrate does not already
provide it." So the thesis governed the *shape* of the PRD but not its *content*, and the
predictable happened in two places. FR-26 ("Provide per-module canvas templates") specifies a
mechanism that `research-notes.md` §8 found already exists: `canvas_template` is a field in
the same upstream `channel-templates.json` that FR-25's channel templates use, applied by
`buzz channels create --template`, with content that "round-trips byte-exact." Story 2.8's
entire scope collapsed — `sprint-status.yaml` line 36 records it bluntly: *"2-8 absorbed into
2-7 (canvases are a field in the same template file)."* Separately, `persona-pack-contract.md`
§4 found that "**BMAD skills are Buzz pack skills, byte-for-byte**… copied… **with no
modification**", so §4.1's framing of the compiler as a "transform" over-describes what the
skills path needed — the contract doc's own conclusion is "The compiler is smaller than
scoped. Epic 2 should be re-estimated downward." Twice, in one epic.

This is not a fatal flaw — under-scoping upstream capability is the *safe* direction of error
for a project whose whole discipline is restraint, and both cases resolved by deleting work.
But the PRD claims restraint as its differentiator, and restraint that is not checked against
the substrate is aspiration.

### Findings

- **medium** No requirement obliges checking upstream before specifying a capability (§4 generally; §4.6 FR-25/FR-26 concretely) — the thesis is "build as little as possible" but the compile-time discipline for it exists only as SM-C2, which is not measurable. FR-26 specified a mechanism upstream already shipped; Story 2.8 was absorbed wholesale. *Fix:* add a standing consequence pattern — each feature block carries a line naming what the substrate already provides and what waggle adds on top — and make "verify against a running relay before specifying" an explicit precondition in §0.
- **low** §4.1's "compiler"/"transform" framing over-describes the skills path (§4.1 description, FR-2) — skills need placement, not translation. Harmless to the architecture, but it inflated Epic 2's estimate.

## Done-ness clarity — thin

The rubric says be unforgiving here, so: the consequence blocks are genuinely good writing —
concrete, mostly testable, and several are excellent ("Two compiles of an unchanged method
installation produce identical bytes, including ordering of any generated collections",
FR-7; "Exactly one module in the codebase calls the substrate's approval mechanism; a
structural test enforces this", FR-19). There is almost no "handles X gracefully" prose.
Downstream leaned on this hard and it held: `epics.md` produced Given/When/Then for 23 of 23
stories directly from these consequences.

But the dimension is *thin*, not adequate, because several consequences are untestable or
false, and the failures cluster on the FRs that mattered most.

**FR-16 is broken end to end and its first consequence is now unachievable.** The requirement
states a mechanism in its title line — "published as a **reference to content-addressed
storage**" — which violates §0's own discipline ("Requirements state capabilities, not
implementation — the mechanism and transport decisions live in this run's `addendum.md`"). The
PRD broke its stated rule exactly once, on the one FR whose mechanism turned out impossible.
Three separate falsifications land here:

| PRD text | Reality |
|---|---|
| "exceeding the substrate's single-event size limit" (FR-16) — 65,536 per addendum §OQ-2 | **UP-14**: 65,536 is the *WebSocket frame* limit and does not govern the HTTP publish path. Real content ceiling is **262,144**. "a documented number was applied to a transport it did not govern" |
| "published as a reference to content-addressed storage" | **UP-16**: `ALLOWED_MIME_TYPES` is `["image/jpeg","image/png","image/gif","image/webp"]`. The media store **cannot hold a markdown document**, so there is nothing to reference |
| "An artifact larger than the substrate's limit **publishes successfully**" | Not achievable. What shipped (UP-16 mitigation) is a **refusal** with a specific error. Story `3-2-publish-artifacts-larger-than-one-event` is marked **`done`** in `sprint-status.yaml` against a consequence it does not meet |
| "The size threshold is **derived** from the pinned substrate's actual limit, not hard-coded" | **UP-15**: the only derivable number is NIP-11's `max_message_length: 524288`, which is *wrong*. waggle reads it and halves it — "a coincidence that should not be relied on" |

That last row is the subtlest and the most instructive: the consequence was written to be
*rigorous* ("derived, not hard-coded") and the rigor is what made it unsatisfiable, because
nobody checked whether the substrate advertises a derivable number at all.

**FR-2 mandates a runtime mechanism the substrate does not have.** "Persistent-fact file
references are preserved as references, resolved at agent runtime rather than inlined at
compile time, so facts stay current as the repository changes." `persona-pack-contract.md` §6:
"**`persistent_facts`.** BMAD supports `file:` glob entries loaded as facts. **The pack spec
has no equivalent**". This consequence was carried verbatim into Story 1.6's acceptance
criteria ("**Then** they are preserved as references rather than inlined **And** are resolved
at agent runtime") — an AC that cannot pass, in a story marked `done`.

**FR-20 has a consequence that OQ-5 makes untestable.** "Reactions by non-authorized
identities do not advance the gate" — while OQ-5 asks "Which identities are authorized to fire
a gate, and how is that authorization expressed and checked?" You cannot write a test for
"non-authorized" when authorization is undefined; the FR and the OQ are in direct tension and
the PRD does not acknowledge it.

**Two consequences are adjectival where the rubric demands bounds.** FR-12: "Each agent is
registered with its **role-appropriate** membership scope" — appropriate by whose rule? FR-13:
"Concurrency is bounded by configuration" — bounded by *what* configuration, and at what
default? `research-notes.md` §1.2/§1.7 records the substrate already bounds itself (buzz-acp
"spawns 1–32 agent subprocesses"; buzz-agent "orchestrates up to 8 concurrent sessions"), so
NFR-8's premise that waggle must supply the bound deserved a number and a justification.

**FR-18 has no size consequence at all**, and needed one: **UP-17** found kind-`1617` patch
content capped at **61,440** bytes — *four times smaller* than the message limit — and "the
first patch we tried was 83 KB and was rejected." A requirement whose whole point is
portability of real developer output specified nothing about whether real developer output
fits.

### Findings

- **critical** FR-16 is falsified on premise, mechanism, and verifiability (§4.4 FR-16; §14 R-7; §16) — wrong limit (UP-14), impossible carrier (UP-16), underivable threshold (UP-15), and a top-line consequence ("publishes successfully") that the shipped behavior contradicts while Story 3.2 is marked `done`. R-7's mitigation ("FR-16 reference-carrying with content-hash verification") is void with it. *Fix:* rewrite as a capability — "an artifact that cannot be published whole fails with a specific error naming size, limit, and reason; it is never truncated or silently dropped" — move the carrier choice to the addendum, and add a consequence requiring per-kind limits be measured against a running relay rather than read from NIP-11.
- **high** FR-2's persistent-fact consequence specifies a substrate mechanism that does not exist (§4.1 FR-2, 3rd consequence) — no pack-spec equivalent (`persona-pack-contract.md` §6); the AC derived from it in Story 1.6 is unpassable. *Fix:* restate as "persistent-fact references are either carried into `instructions.md`/the persona body or reported as dropped per FR-6" — which is what FR-6 already guarantees, and would have made this a non-event.
- **medium** FR-20's authorization consequence is untestable while OQ-5 is open (§4.4 FR-20 vs §15 OQ-5) — the FR and its own blocking OQ contradict each other with no acknowledgement. *Fix:* mark the consequence `[BLOCKED BY OQ-5]` inline, or specify a default (channel admin per kind `39001`, per addendum §OQ-5) that the architecture may narrow.
- **medium** FR-18 specifies no size behavior for patch events (§4.4 FR-18) — UP-17's 61,440-byte patch ceiling rejected the first real patch attempted. *Fix:* add "publishing refuses with the relay's own limit named when a patch exceeds the patch-kind ceiling; fixtures do not assume any commit fits."
- **medium** FR-2/FR-3 are silent on the merge-direction inversion between input and output (§4.1) — FR-3 correctly describes BMAD's semantics ("tables deep-merge; arrays… append"), but `persona-pack-contract.md` §5 warns the *target* is the opposite: "**Merge is shallow replacement**… **This is the opposite of BMAD's merge semantics**… Getting this backwards would **silently drop principles and menu items**." Nothing in FR-2 requires emitting a flat, fully-resolved persona. *Fix:* add a consequence to FR-2: "emitted persona configuration is fully resolved and flat; the compiler never relies on the substrate to complete a merge."
- **medium** Adjectival consequences where bounds are required (§4.3 FR-12 "role-appropriate membership scope", FR-13 "bounded by configuration") — the rubric's exact red flag. *Fix:* define the scope rule (agents join only channels their module's template names) and state a default concurrency ceiling with rationale against buzz-acp's own 1–32 / 8-session bounds.
- **low** FR-9/NFR-3's "unmodified" had to be redefined during implementation (§4.2 FR-9, §10 NFR-3) — `research-notes.md` §6.6: "`.env` is gitignored *by Buzz itself*, so editing it is configuration, not modification. The enforceable invariant is 'no **tracked** file modified'." The PRD's flat "the substrate checkout or image is unmodified" was too strong to test. *Fix:* adopt the tracked-file formulation, which is what SM-3 actually measures.
- **low** NFR-2/FR-25 assert idempotence without a concurrency caveat (§10 NFR-2, §4.6 FR-25) — UP-10: upstream channel creation is not idempotent, so waggle check-then-creates, "inherently racy against concurrent creators." *Fix:* scope NFR-2 to sequential re-runs and say so.

## Scope honesty — thin

Deferrals are handled well. §6.2 names what is out and why, addendum §4 gives each deferral an
owner and a revisit condition, and the FR-18 deferral is flagged rather than quietly dropped.
§5's non-goals do real work. Open-items density (8 OQs, 5 assumptions, 1 PM note) is
proportionate for a green-light-to-build PRD on a substrate this volatile — arguably *low*
for one.

And that is the problem. **§16 is a perfect mechanical roundtrip — five inline `[ASSUMPTION]`
tags, five index entries, no orphans in either direction — and it indexes the wrong five.**
Look at what got tagged: self-host-only is acceptable (§2.2), resolver reuse is preferable to
porting (FR-3), reference-over-chunking (FR-16), 30 minutes is the right bar (SM-4), env
tooling covers the toolchain (§13). Four of the five are *preferences and product judgments*.
The fifth (§13 toolchain) is a capability claim, is the only one marked "Unverified", and is
also the only one that resolved cleanly — `research-notes.md` §6.4: "Hermit provides cargo
1.95.0… **OQ-6 resolved; contributors need no system upgrade.**" The tagging instinct was
pointed at the wrong category.

Here is what was asserted as fact, untagged, and later proved wrong or unavailable:

| Untagged claim in the PRD | Location | Outcome |
|---|---|---|
| The persona pack schema is undocumented upstream | §14 R-2, §15 OQ-3 | **False** — UP-04 withdrawn; a 16-section spec existed |
| The substrate has a "single-event size limit" waggle must design around | §4.4 desc, FR-16 | **False transport** — UP-14; 4× the assumed headroom |
| "content-addressed media storage" is a dependency waggle can use for artifacts | §8, glossary "Artifact event" | **False** — UP-16; images only |
| An agent session can be run/registered/profiled headlessly | §4.3 desc, FR-13, FR-14, UJ-1 | **Not available** — desktop human-in-the-loop; 1.5/1.7/1.9 stalled |
| Persistent-fact references resolve at agent runtime | FR-2 | **No such mechanism** — pack spec has no equivalent |
| Reads from the substrate carry verifiable signatures | FR-22, NFR-9, §12 | **False** — UP-07: "`buzz-cli` strips signatures from every read, in every format"; forced waggle to build its own relay client |
| Channel/canvas templating is waggle's to provide | FR-25, FR-26 | **Already upstream** — Story 2.8 absorbed |
| Provisioning primitives are idempotent | NFR-2, FR-25 | **False** — UP-10; check-then-create required |
| Skills require compilation/transformation | §4.1 | **False** — byte-for-byte compatible |

Nine capability claims; zero tags. The Assumptions Index is honest about its contents and
blind about its category — it surfaces what the author *chose*, not what the author *believed
about someone else's code*. For a PRD whose §8 says of the substrate "**This is the deepest
dependency in the product and the largest source of external risk**", that is the central
scope-honesty failure of the document.

One structural aggravator: the PRD nowhere distinguishes claims *verified against a running
substrate* from claims *read in upstream docs*. `upstream-issues.md` maintains exactly that
distinction (`observed` vs `confirmed`) and it is the single most useful convention in the
repo. It postdates the PRD. It should not have.

### Findings

- **high** §16 indexes preferences and omits every substrate-capability assumption that broke (§16, §4 throughout) — five tags, four of them product judgments; nine untagged capability claims, at least six falsified (table above). *Fix:* introduce a second tag class — `[ASSUMPTION: substrate-capability, unverified]` — mandatory on any statement about what the substrate does, and require each to name its verification story before the PRD can be marked final.
- **high** The PRD does not distinguish doc-read claims from relay-verified claims (§0, §8, §14) — the `observed` / `confirmed` discipline that later made `upstream-issues.md` trustworthy is absent from the artifact that drove the build. *Fix:* adopt that vocabulary in §8 and §14 and carry a provenance marker on every substrate claim.
- **medium** §8 lists an unusable capability as a dependency (§8) — "content-addressed media storage" is depended on for the FR-16 path and cannot store the artifacts in question (UP-16). *Fix:* qualify it ("media storage, images only at the pin") so the constraint is visible where dependencies are chosen.
- **low** Inherited-assumption paragraph at the end of §16 is unstructured — "the non-served audience boundary… and the sequencing of all-seven-module coverage" are carried as prose without IDs or owners, so nothing downstream can track their resolution. *Fix:* give them IDs and revisit conditions like addendum §4 does for deferrals.

## Downstream usability — strong

This dimension is close to exemplary and the evidence is not hypothetical: `epics.md`
source-extracted from it cleanly. Its validation record reports "Every FR appears in the
inventory: 28 of 28", "Every FR appears in the coverage map: 28 of 28", "Every NFR declared:
10 of 10", with story numbering contiguous and Given/When/Then on 23 of 23 stories. The FR→epic
map (`epics.md` lines 115–142) resolves every ID without invention. That does not happen by
luck; it happens because §3's glossary is real, the FR IDs are contiguous and globally
numbered rather than per-feature, and each FR names the UJ it realizes so the epic author could
reconstruct intent.

Section-level independence holds too — §9 (Audit Trail) reads standalone, §12 (Public Surface)
reads standalone, and cross-references go through glossary terms rather than "see above." The
glossary carried into implementation intact: "hive", "substrate", "story channel", "gate
record", "verdict" all appear unchanged in `research-notes.md`, `upstream-issues.md`, and
`persona-pack-contract.md` six months later. Vocabulary discipline that survives contact with
code is the strongest possible evidence for this dimension.

The one blemish is that the glossary embeds a falsified mechanism: **"Artifact event** — a
signed event carrying a method artifact… **or a reference to one held in content-addressed
storage**" (§3). Because the glossary is the canonical vocabulary, that clause propagated the
FR-16 error into the definition layer, where it is hardest to dislodge.

### Findings

- **low** Glossary definition of "Artifact event" hard-codes the FR-16 mechanism (§3) — definitions should be mechanism-free for the same reason §0 says requirements are. *Fix:* "…carrying a method artifact, or a reference to one where the artifact cannot be carried inline."
- **low** OQ→FR references are one-directional (§15 vs §4) — OQ-5 says "*Blocks FR-20*" but FR-20 does not cite OQ-5; same for OQ-7/FR-1. A reader working forward from an FR cannot see it is blocked. *Fix:* inline `[BLOCKED BY OQ-n]` at the affected consequence.

## Shape fit — adequate

The shape is broadly right. waggle is chain-top (PRD → architecture → epics → stories), so the
traceability investment is correct and paid off. It is also a two-audience product — the
platform-minded operator and the quality owner — which justifies UJs rather than a bare
capability spec, and §7's binary success metrics (SM-1, SM-2, SM-3) are appropriately
operational rather than user-facing. Adapt-in cluster selection (addendum §5) is defensible;
dropping monetization, IA, rollout, and compliance-as-subject for a self-hosted no-UI tool is
right, and promoting Audit Trail to a top-level §9 rather than a compliance subsection is a
genuinely good judgment call — it is the product's thesis.

Mild over-formalization at the edges. **UJ-3 (Amelia) is a UJ whose entire payload is deferred**
— it realizes FR-18, which §6.2 puts out of MVP scope, so the journey's climax ("a third-party
client that has never heard of waggle can read the repository, the patch, and its status")
describes something MVP does not do. The PRD flags FR-18's deferral but not that a whole
key user journey goes with it. **UJ-4 (Dana)** is similarly forward-looking ("six months after
launch") and drives Epic 2. Both are legitimate as vision-carriers; neither is labeled as
post-MVP, so a reader sizing MVP from §2.3 sizes it wrong.

The `[ASSUMPTION]` mechanism itself is a mild shape mismatch for this product: it is designed
for inferences about *users*, and waggle's real uncertainty is about *someone else's Rust
workspace*. The PRD used the mechanism as designed and the uncertainty went untagged — see
Scope honesty.

### Findings

- **low** UJ-3's climax describes deferred functionality without saying so (§2.3 UJ-3, §6.2) — FR-18 is out of MVP; the journey is not marked. *Fix:* annotate UJ-3 and UJ-4 `[post-MVP]`.

## Mechanical notes

- **ID continuity — clean.** FR-1..FR-28 contiguous, no gaps or duplicates. NFR-1..NFR-10,
  UJ-1..UJ-4, SM-1..SM-6, SM-C1..SM-C3, OQ-1..OQ-8, R-1..R-9 all contiguous. Every FR names at
  least one UJ it realizes. No dangling references found.
- **Assumptions Index roundtrip — passes mechanically, fails substantively.** Five inline
  `[ASSUMPTION]` tags (§2.2, FR-3, FR-16, SM-4, §13); five index entries; exact match both
  directions. See Scope honesty for why this is the wrong five.
- **Glossary drift — none detected.** Terms are used identically across §2, §4, §7, §9, and
  survived into the implementation docs unchanged. "Hive", "substrate", "method installation",
  "persona pack", "compiled workflow", "gate record", "verdict", "priority tag" are all stable.
  Minor: §4.5 uses "the substrate's approval mechanism" where §3 defines "Gate interface" as
  the abstraction over it — consistent, but the mechanism itself has no glossary entry.
- **Cross-refs to sibling docs resolve.** brief.md, brief addendum, `research-notes.md`,
  `upstream-issues.md`, `NOTICE` all exist at the cited paths.
- **§14 R-7's mitigation is now void** ("FR-16 reference-carrying with content-hash
  verification") and R-2's is moot (the schema was documented) — the register needs a pass.
- **UJ protagonists** — all four named with inline context (Sam, Murat, Amelia, Dana). Passes.
- **Stale-by-implementation sections** worth marking rather than silently leaving: §4.4
  description (size limit), FR-16, FR-2 3rd consequence, §8 (media storage), §14 R-2 and R-7,
  §15 OQ-2/OQ-3/OQ-6 (all three now resolved — OQ-6 by `research-notes.md` §6.4, OQ-3 by
  UP-04's withdrawal, OQ-2 by UP-16 forcing a different answer than either option offered).

## What this PRD should have done differently, in one paragraph

Everything that went wrong shares one shape: **a sentence about the substrate, written from
documentation, stated as fact, with no verification obligation attached.** The document already
had the machinery to prevent it — `[ASSUMPTION]` tags, an Open Questions section, a risk
register, and §0's discipline that "requirements state capabilities, not implementation." It
applied all four to the things the *author* was choosing and none of them to the things the
author was *believing about someone else's code*, which is the only place a distribution
project can be wrong. One rule would have caught six of the nine falsifications: no statement
about the substrate enters the PRD without a provenance marker and a named story that will
verify it against a running relay before anything is built on it.
