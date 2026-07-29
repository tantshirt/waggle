---
review: rubric-walker
target: docs/planning-artifacts/architecture/spine-waggle-2026-07-28/ARCHITECTURE-SPINE.md
reviewer: independent (not the author)
date: 2026-07-29
verdict: FAIL — do not hand off as `status: final`
---

# Reviewer Gate — rubric walker

Judged against the good-spine checklist in
`.claude/skills/bmad-architecture/references/reviewer-gate.md`:

> "it fixes the real divergence points for the level below and misses none; every `AD`'s Rule
> is enforceable and actually prevents its stated divergence; nothing under Deferred could let
> two units diverge; named tech is verified-current; **it ratifies rather than contradicts a
> brownfield codebase**; if a spec drove it, it covers that spec's capabilities; … and every
> dimension the altitude owns is decided, deferred, or an open question."

## Verdict

The spine is well-written and its *reasoning* is unusually honest, but it is marked
`status: final` while (a) three ADs rest on measurements that have since been falsified,
(b) it mandates a crate, a port, and two adapters that do not exist in the repository it
governs, (c) one whole cross-cutting NFR (idempotence) and one whole structural dimension
(build/CI/supply chain, named as the enforcement mechanism for four separate Rules) are
silent, and (d) three `Deferred` entries defer questions that measurement already answered,
which now *permits* divergence rather than deferring it.

Line references are to `ARCHITECTURE-SPINE.md` as read on 2026-07-29.

---

## Critical

### C-1 · AD-15 — the Rule mandates a mechanism the substrate cannot provide, against a number that governs a different transport

Lines 194–203. The Rule:

> "An artifact whose serialized event fits the substrate frame limit is published inline. One
> that does not is **published as a content-addressed reference** carrying a hash that
> verifies the retrieved bytes. The threshold is read from the pinned substrate's actual limit
> (**65,536 bytes at `v0.4.26`** `[BUZZ]`), never hard-coded independently."

Three separate defects, all now measured:

1. **The number governs the wrong transport.** UP-14: *"It is the WebSocket frame limit.
   Publishing over HTTP `POST /events` is unaffected: a 200,000-byte event was accepted
   without complaint. The real ceiling is content length, at 262,144 bytes."* waggle does not
   use the WebSocket publish path — UP-07 and UP-13 forced it onto HTTP `POST /events`
   (`crates/waggle-hive/src/events.rs`). AD-15 therefore names a limit that never applies to
   waggle's own code path.
2. **The chosen mechanism does not exist.** UP-16: *"`ALLOWED_MIME_TYPES` in
   `buzz-media/src/validation.rs` is `["image/jpeg", "image/png", "image/gif", "image/webp"]`
   … **this blocks FR-16's chosen mechanism.** … the substrate's media store cannot hold a
   markdown document, so there is nothing to reference."* AD-15 resolves OQ-2 in favour of a
   capability the pinned substrate does not have. The implementation already diverges from the
   spine: `waggle-hive/src/events.rs:58` returns `EventError::TooLarge` with a message that
   explicitly cites UP-16. The spine mandates reference-carrying; the code refuses. A unit
   built to this spine builds the wrong thing.
3. **A single threshold cannot model per-kind limits.** UP-17 measured three ceilings —
   frame 65,536, kind:9 content 262,144, kind:1617 patch content 61,440 — and *"a real
   `git format-patch` of a documentation-heavy commit exceeds it easily — the first patch we
   tried was 83 KB and was rejected."* AD-15 speaks of "the substrate frame limit" as one
   value; the map (line 450) routes only FR-16 through it, and FR-18 (patches) is governed by
   AD-8 alone. `crates/waggle-hive/src/patches.rs` performs **no size check at all**. The
   spine's size discipline has a hole exactly where the smallest limit lives.

**Fix:** rewrite AD-15 as a per-kind, relay-derived size *policy* whose failure mode is a
named refusal (matching what is built), and record that reference-carrying is unavailable at
`v0.4.26` and is a precondition on UP-16 landing upstream. Delete the 65,536 figure.

### C-2 · AD-13 — gate authorization is aspirational; no mechanism can execute it, and nothing does

Lines 173–181:

> "Authorization to fire a gate is read from the relay-signed admin list (kind `39001`) for the
> channel. `PASS`, `CONCERNS`, and `FAIL` require admin or owner; `WAIVED` requires owner. …
> **A reaction from an unauthorized identity is recorded but does not advance the gate.**"

The Rule states a policy but never names where the check runs, and in the architecture as
actually verified there is no place it *can* run:

- The gate record is published by the **substrate's** workflow engine, not by waggle.
  research-notes §7.4 and `crates/waggle-emit/src/workflow.rs` show the emitted workflow's
  entire body is one `send_message` step fired by `reaction_added`. The verified action set
  (research §7.1) is `send_message · send_dm · set_channel_topic · add_reaction ·
  call_webhook · request_approval · delay` — there is no action that reads kind `39001` and
  no conditional that could gate on it. Any reaction with the configured emoji, from any
  identity, publishes a gate record.
- `waggle_core::gate::authorize` exists (`gate.rs:150`) and has **zero production callers** —
  `grep -rn "authorize" crates/` returns only the definition, the re-export, and unit tests.
  `grep -rn "39001" crates/` returns nothing outside doc comments. The rule is asserted in
  three comments and implemented nowhere.
- Consequently "recorded but does not advance the gate" is undefined. The record *is* in the
  log, signed, indistinguishable from an authorized one. Two units will diverge on whether an
  unauthorized-but-published record counts, because the spine gives no discriminator.

**Related and equally silent: who signs the gate record.** The sequence diagram (lines
380–383) reads `Gate->>Relay: publish gate record (signed)` as if waggle signs it. It does
not — the workflow engine does, under whatever identity created the workflow. PRD §9 requires
*"Every artifact, handoff, and gate record is signed by the identity that produced it. No
shared service identity produces content attributable to a role."* FR-22 requires *"A gate
record's signature verifies independently"*, and UP-07 confirms *"`buzz-cli` strips
signatures from every read, in every format"*. SM-2 — the second of three binary primary
success metrics — rides entirely on this. The spine decides none of it.

**Fix:** either (a) make `waggle-gate` the publisher (poll reactions, check `39001`, sign and
publish under a waggle identity — and then say so, including whose npub signs), or (b) accept
that the relay-side workflow publishes unconditionally and add an AD stating that
authorization is a *read-side reconciliation* rule, defining precisely how a reader
distinguishes an advancing record from a recorded-but-void one. Silence permits both.

### C-3 · NFR-2 (idempotence) has no AD, no map row, and no mention anywhere in the spine

Frontmatter line 11 claims `binds: [FR-1..FR-28, NFR-1..NFR-10]`. Searching the spine for
`idempot` returns **zero hits**; NFR-2 appears in no AD and the Capability → Architecture Map
(lines 431–461) contains no NFR rows at all. Yet:

- PRD NFR-2: *"All provisioning operations are safely re-runnable without duplication or
  destructive side effects."*
- FR-10, FR-12, FR-25, FR-26 each restate it as a testable consequence.
- UP-10 (confirmed 2026-07-29): *"`buzz channels create --name X --template T` run twice
  produces **two channels named `X`** … There is no name-uniqueness check and no 'already
  exists' response."* research §8.2 confirms: *"Idempotent by channel name — ❌ **creates a
  duplicate**."*

So the substrate is actively hostile to the requirement, the requirement is load-bearing for
four FRs, and the spine says nothing. The one implementation
(`waggle-hive/src/channels.rs:96–115`, check-then-create against a lowercased name list) is
undocumented in the spine and, per UP-10, *"inherently racy against concurrent creators"* —
a trade-off the spine should have ratified and did not. FR-11's *"Re-running provisioning does
not silently regenerate an existing identity"* is a second, different idempotence rule with a
third possible strategy.

This is the checklist's canonical failure: two units built independently will each invent
their own idempotence strategy (check-then-create / deterministic-name-as-id / do nothing),
and two of the three are wrong.

**Fix:** add an AD binding NFR-2 that states the strategy (check-then-create), names the race
as accepted, and states what "already exists" must return so callers can be written against
it.

---

## High

### H-1 · The spine contradicts the codebase it is supposed to govern

The checklist requires a spine to *"ratify rather than contradict a brownfield codebase."*
Per `docs/implementation-artifacts/sprint-status.yaml`, epics 2 and 3 are `done` and epic 1 is
`in-progress` — roughly 6,400 lines of Rust exist. The spine mandates structures that are not
in it:

| Spine says | Repository has |
|---|---|
| `waggle-gate` crate (lines 42, 152, 159, 422, 426) | **Does not exist.** `Cargo.toml` members: core, method, hive, emit, cli. |
| AD-10: *"Exactly one crate, `waggle-gate`, may call the substrate's approval mechanism; a structural test enforces the boundary."* | Gate logic is split across `waggle-core/src/gate.rs`, `waggle-emit/src/workflow.rs`, and `waggle-cli/src/main.rs`. No structural test exists. |
| AD-11: *"`waggle-gate` defines one `GateBackend` port with two implementations: `LogReconciledGate` … and `SubstrateNativeGate`"* | `grep -rn "GateBackend\|LogReconciled\|SubstrateNative" crates/` → **no matches.** Neither the port nor either adapter exists. |
| AD-11: *"A test asserts the pinned substrate's *current* approval behavior"* | No such test. |
| Source tree line 426: `deploy/compose/` | **Does not exist.** |
| Stack line 316: `insta` (snapshot testing) 1.48.0; AD-4 *"asserted by a snapshot test"* | `insta` appears in **no** manifest and **not in `Cargo.lock`**. Determinism is asserted by hand-rolled `assert_eq!(compile(x), compile(x))` (e.g. `compile.rs:417`), which proves the function is deterministic *in-process* but not that output is byte-stable across versions — a weaker property than AD-4 claims. |

A unit handed this spine creates `crates/waggle-gate` and a `GateBackend` port, duplicating
working code. Either the spine ratifies what exists (gate domain in `waggle-core`, gate
emission in `waggle-emit`) or the code is refactored — but the spine cannot be `final` while
it silently disagrees with the thing it governs. The memlog records no awareness of this.

### H-2 · Four Rules name "CI" as their enforcement mechanism; there is no CI, and the spine never decides one

- AD-2, line 68: *"**CI asserts** the substrate checkout is byte-unchanged after the full test
  suite."*
- AD-5, line 98: *"**This test is not optional and may not be skipped in CI**"* — the memlog
  calls this *"the single largest correctness risk the language choice introduced."*
- AD-17, line 220: *"**waggle CI builds** the relay image from upstream's own `Dockerfile` at
  the pinned release tag … and publishes it to **waggle's registry**."*
- AD-11's tripwire is only meaningful as a scheduled/CI check.

The repository has no `.github/`, no CI configuration of any kind, and no `deploy/`. The spine
never names a CI platform, a container registry, a release trigger, or who owns the signing of
the published image — yet AD-17 explicitly *"takes on image-publishing and supply-chain
responsibility"* (memlog line 11). That is an entire structural dimension the checklist calls
out by name ("deployment & environments, infra/provider strategy, operations") left neither
decided nor deferred. The `Deferred` list contains *"Release and versioning mechanics for
waggle's own binary"* — which is a different, smaller question, and does not cover the
substrate image pipeline that AD-17 depends on.

Rules whose enforcement mechanism does not exist are aspirational by definition.

### H-3 · AD-2's integrity assertion is already known to be unachievable as written

Line 68: *"CI asserts the substrate checkout is **byte-unchanged** after the full test suite."*

research-notes §6.6 already corrected this: *"**AD-2 clarification:** `.env` is gitignored *by
Buzz itself*, so editing it is configuration, not modification. The enforceable invariant is
'no **tracked** file modified' — `git status --porcelain` empty."* Bringing the substrate up
writes `.env`, `.buzz-data/`, and Postgres/MinIO volumes. A byte-unchanged assertion fails on
first run, and the predictable outcome is that whoever hits it weakens or disables the check —
which is precisely the divergence AD-2 exists to prevent (SM-3 is a binary primary metric).

**Fix:** restate the invariant as "no tracked file modified in the substrate checkout, asserted
by `git status --porcelain` being empty," which is both true and testable.

### H-4 · AD-2 permits two substrate transports with no rule for choosing, and one of them silently loses signature verifiability

Lines 65–67: *"All substrate interaction goes through `waggle-hive` using published
interfaces — **relay WebSocket, REST, its own CLIs**."*

That sentence permits all three. Measurement says two of them are unusable on the audit path:

- UP-07 (confirmed): *"`buzz-cli` strips signatures from every read, in every format … A
  consumer cannot verify a Schnorr signature it is never given. Anything in waggle that must
  *prove* provenance … cannot go through `buzz-cli`."* It adds an explicit architecture
  consequence: *"AD-20's 'machine-first command surface' must not be read as 'waggle shells out
  to `buzz-cli`'."* **That consequence appears nowhere in the spine.**
- UP-13 (confirmed): *"`buzz-cli` cannot attach arbitrary tags when sending a message …
  Without them FR-24 ('filter the log by priority') would degrade to fetching everything and
  filtering client-side."*

The code has silently drawn the line by hand: `events.rs` speaks HTTP `POST /events` with
NIP-98; `channels.rs`, `identity.rs`, and `patches.rs` shell out to the `buzz` binary. No AD
states which path a new capability must take. `patches.rs` — the FR-18 portable-patch path,
described by the PRD as *"emotionally load-bearing … the clearest demonstration of the
portability claim"* — goes through the CLI, which means its published events cannot be
sig-verified on read-back and its tags cannot be set. Two units will diverge here on their
first day.

**Fix:** an AD stating "anything on the signed trail — publish or read — uses the relay HTTP/WS
protocol directly; `buzz-cli` is permitted only for provisioning operations whose output is not
part of the audit record," with the UP-07/UP-13 rationale attached.

### H-5 · AD-17's operator promise is false: waggle requires the `buzz` client binary, and the spine never says where it comes from

Line 224: *"Operators need only a container runtime."* Line 323: *"Contributors building the
substrate locally need it; operators do not."*

But `waggle-hive` invokes an operator-supplied `buzz` executable for identity provisioning
(`identity.rs:261`), channel listing and creation (`channels.rs:58`, `:113`), and patch
publication (`patches.rs:55`, `:110`). UP-08 (confirmed) records that upstream's *"release
assets for `v0.4.26` are desktop binaries only — `.dmg`, `.deb`, `.AppImage`, `.exe`"*, and
AD-17 solves distribution only for the **relay image**. Nothing in the spine decides how an
operator obtains `buzz-cli` / `buzz-admin`: build from source (reintroducing the Rust 1.95
toolchain requirement AD-17 exists to remove), extract from a desktop package, or run it inside
the CI-built image. SM-4 ("under 30 minutes, clean machine to first signed message") depends on
the answer.

**Fix:** decide the client-binary distribution story in AD-17, or add an AD for it. It is not
deferrable — every provisioning command depends on it today.

### H-6 · The largest `Deferred` item was already resolved before the spine was finalized, and the real risk it uncovered has no AD

Lines 466–471:

> "**Persona pack and agent-runtime schemas.** The substrate does not document them; they must
> be read from `crates/buzz-persona` … **This is Story 1.1's output and the largest remaining
> unknown** (OQ-3 / **UP-04**)."

UP-04's status is **`withdrawn` — "this issue was wrong"** (resolved 2026-07-28, the spine's own
creation date): *"`crates/buzz-persona/PERSONA_PACK_SPEC.md` **is a complete, 16-section
specification** that we had not read."* `docs/persona-pack-contract.md` records the contract
verified against a running relay, including `buzz pack validate` passing on a hand-built pack.
The spine's single largest declared unknown is stale, and it cites a withdrawn issue as its
justification.

That would be merely embarrassing if the *actual* finding had landed somewhere. It has not.
`persona-pack-contract.md` §5 states, in bold:

> "**Merge is shallow replacement — there is no deep merge.** … **This is the opposite of
> BMAD's merge semantics.** BMAD appends arrays and deep-merges tables; Buzz replaces
> wholesale. The compiler must resolve BMAD's layers **fully** (AD-5) and emit a **flat,
> already-resolved** persona — never rely on Buzz to finish a merge. **Getting this backwards
> would silently drop principles and menu items.**"

No AD says this. AD-5 governs the *input-side* BMAD merge only; nothing in the spine forbids a
unit from emitting layered persona defaults and expecting `plugin.json` `defaults` to be
deep-merged with persona frontmatter. The failure mode — silently dropped principles and menu
items — is exactly what AD-6 ("nothing is dropped silently") exists to prevent, and AD-6 does
not reach it because the drop happens in the substrate at load time, not in the compiler.

**Fix:** replace the stale Deferred entry with an AD: "personas are emitted fully resolved and
flat; waggle never relies on substrate-side merge, whose semantics are shallow replacement and
opposite to the method's." Also fold in `[BUZZ]`'s null-vs-empty rule (`null` = absent, `[]`/`{}`
= override), which is a second silent-divergence trap.

### H-7 · Agent instantiation — the thing every agent-facing FR depends on — is presented as solved and is neither decided nor deferred

The container topology (lines 347–351) shows `buzz-agent · one per method role · own npub`
authenticating to the relay, and the map assigns FR-12, FR-13, and FR-14 to `waggle-hive` as
though provisioning is the whole story. research-notes §8.3 says otherwise:

> "**Roster membership needs a live managed agent.** Resolution scans managed-agent events
> whose `content.persona_id` matches, to find pubkeys. Those are created by `buzz agents
> draft-create`, which *'opens a prefilled create-agent form in the owner's Buzz Desktop'* — a
> **human-in-the-loop desktop flow with no headless equivalent.** This is the **same blocker as
> Story 1.7**: without a running agent instance there is nothing to add to a channel."

`sprint-status.yaml` confirms the consequence: `1-5` (identity), `1-7` (agent present as a hive
member), and `1-9` (approve a gate) are all still `in-progress` while epics 2 and 3 are `done`.
UJ-1's climax — *"an agent replies signed under its own npub"* — and FR-14's *"An agent's
displayed name, title, and icon in the hive match its descriptor"* both sit behind this.

The spine's only nearby entry is *"Agent session concurrency defaults"* (lines 477–478), which
defers a **number** while the **mechanism** is missing and undiscussed. There is no AD, no open
question, and no Deferred entry for how an agent process is instantiated, bound to its npub, and
made visible to roster resolution — nor any statement that this is blocked on a desktop flow
with no headless path. A whole runtime dimension is silent, and the diagram actively implies it
is settled.

**Fix:** either an AD for the agent-instantiation seam (and an upstream issue for the missing
headless `draft-create`), or an explicit Deferred entry naming the blocker. It must not stay
implied-solved in a diagram.

---

## Medium

### M-1 · AD-15's "derived, never hard-coded" is satisfied by a coincidence the team has already flagged

UP-15: *"the relay's NIP-11 document reports `limitation.max_message_length: 524288`, but
content over `262144` is rejected. The enforced content ceiling is advertised nowhere … waggle
reads `max_message_length` and **halves it**, which matches the observed value — **a
coincidence that should not be relied on indefinitely**."* `events.rs:174`:
`max_content: max_message / 2`.

AD-15's *"never hard-coded independently"* is satisfied in letter by a magic `/2`. The spine
should state the halving explicitly, name it as a coincidence, and attach a tripwire in AD-11's
style so an upstream change to `max_message_length` is caught rather than silently doubling the
accepted size.

### M-2 · The Stack table is not the dependency set — three phantom entries, six omissions

Lines 307–320. Listed but present in **no** manifest and not in `Cargo.lock`:
`nostr-sdk 0.44.1`, `csv 1.4.0`, `insta 1.48.0`. Present in `Cargo.toml` but absent from the
Stack: `reqwest`, `sha2`, `base64`, `uuid`, `thiserror`, `serde_json`.

This matters beyond bookkeeping. PRD §8: *"**Dependency policy:** dependencies outside these
three ecosystems [substrate / method / Nostr] require explicit owner approval before adoption."*
`reqwest`, `sha2`, `base64`, and `uuid` are outside all three, and the spine — the document
whose Stack table is the record of that decision — does not mention them. `reqwest` in
particular is now the primary substrate transport (H-4) and carries a TLS stack; it deserves a
row and a rationale. Conversely, `insta` being listed while AD-4 leans on *"a snapshot test"*
that does not exist overstates the determinism guarantee.

**Verification caveat:** this reviewer had no network access, so the *currency* of the pinned
versions (`nostr 0.44.6`, `serde_norway 0.9.42`, `clap 4.6.4`, `toml 1.1.3`, `reqwest 0.13`)
could not be independently confirmed. The memlog's claim of verification on 2026-07-28 is taken
at face value; the internal inconsistencies above are provable without network.

### M-3 · AD-18's "no floating version reference exists anywhere in the repository" is violated by the repository

Line 234. `Cargo.toml` `[workspace.dependencies]` uses caret ranges throughout:
`clap = { version = "4.6", … }`, `reqwest = "0.13"`, `thiserror = "2"`, `uuid = "1"`,
`sha2 = "0.10"`, `base64 = "0.22"`, `serde_json = "1.0"`. `Cargo.lock` is committed, which is
the real pinning mechanism — but the Rule as written says something stronger and false. Either
say "the lockfile is the pin and is committed" or require exact `=` versions. As written, a
unit reading AD-18 literally would rewrite every manifest.

### M-4 · Deferring the kind registry now permits divergence, because the answer already exists in code

Lines 472–474: *"Which kinds are actually claimed cannot be settled before the artifact and
handoff event shapes are designed, which is post-pilot scope."*

They are designed. research §7.4 records the verified gate chain (verdict kind `9`, reaction
`7`, gate record `9`, *"no custom kind claimed"*), and `waggle-core/src/artifact.rs` already
fixes a whole tag vocabulary — `t:waggle`, `t:artifact|handoff|verdict|gate-record`,
`t:p0..p3`, `t:module-<m>`, `h:<channel>`, `e:<ref>`, plus descriptive `waggle-artifact`,
`waggle-story`, `waggle-from`, `waggle-to`. `gate.rs` fixes `APPROVAL_EMOJI`,
`GATE_RECORD_MARKER`, `VERDICT_MARKER`.

None of this is in the spine, and the constraint that *forced* it is in a source comment only:

> "NIP-01 only indexes single-letter tags for `#<letter>` filter queries; a `waggle-priority`
> tag would be stored but *not* queryable, so FR-24's 'filter the log by priority' would
> quietly degrade to fetching everything and filtering client-side."

That is precisely an AD-8-shaped invariant — a rule whose violation is silent and whose
correction is expensive. Leaving it deferred means a second unit adds `waggle-priority` and
FR-24 degrades with no error anywhere. Also note the PRD treats event shapes as a
**compatibility surface** (§12: *"Changing an event's meaning is a breaking change"*), which
makes an undeclared vocabulary worse than an incomplete one.

### M-5 · The canvas-template deferral is stale and would send a unit to build something that exists

Line 475: *"**Canvas template format.** Depends on the substrate's canvas MCP contract,
unread."*

research §8.1–§8.2 (2026-07-29) read it: canvases come from a `canvas_template` **string field
in the same `channel-templates.json`**, verified *"`canvas_applied": true`, content round-trips
byte-exact"*, and `templates/tea/channels.json` in this repo already ships one. §8.4 concludes
*"Build a canvas-template mechanism → **Covered upstream**."* A unit reading this Deferred entry
would go investigate an MCP contract that is irrelevant to the answer.

The same section carries a second correction the spine never absorbed: `--templates-file`
overrides the **desktop app's** app-data path, and *"That is the finding that matters"* — it is
the only reason this design is headless at all. AD-16 says templates are *"declarative data files
loaded at runtime"* without recording that the loader is upstream's, that the file must be the
upstream `channel-templates.json` shape, or that the `--templates-file` override is load-bearing.

### M-6 · Observability and operations are deferred as a question that leaves the dimension silent

Lines 486–488: *"**Observability envelope.** The substrate ships Prometheus `[BUZZ]`; whether
waggle emits its own metrics is unanswered and not blocking."*

Metrics are genuinely deferrable. What is not: the Conventions table's single row (*"Logging:
Structured, leveled, on stderr"*) does not decide log **format** (JSON vs. human), does not say
whether a compile run is correlatable to the events it later produces (PRD §9 requires *"Compile
provenance is recorded: which method version, which module versions, and which substrate version
produced a given generated configuration"* — no AD binds this), and does not cover FR-23's
*"The operator is told, **once and clearly**, that degraded gate mode is active and why."*
"Once and clearly" is a stateful behaviour with no home in the spine.

### M-7 · AD-6 and FR-2 conflict over `persistent_facts`, and the spine does not notice

AD-6 (lines 101–109) permits a field to be *"explicitly dropped with a reason."* FR-2's
consequence is stricter: *"Persistent-fact file references are preserved as references, resolved
at agent runtime rather than inlined at compile time, so facts stay current as the repository
changes."* `persona-pack-contract.md` §6 records `persistent_facts` as *"not yet mapped"* with no
pack equivalent, and `activation_steps_prepend/_append` as having none either (Buzz `hooks` are
*"parsed and validated but not yet executed"* — *"Do not depend on them"*).

So a compliant compiler can satisfy AD-6 by reporting the drop while failing FR-2 outright, and
the map (line 436) shows FR-2 governed by AD-4 and AD-6 only. The spine needs either an AD
carrying FR-2's reference-preservation requirement, or an explicit acknowledgement that FR-2 is
partially unimplementable at `v0.4.26` and an upstream issue to match.

### M-8 · AD-11's tripwire cannot fire from waggle's own code path, and one of its two adapters has no consumer

Lines 154–163: *"A test asserts the pinned substrate's *current* approval behavior; when upstream
fixes UP-01 that test fails, forcing a deliberate re-pin."*

But research §7.3 records that *"waggle does not emit `request_approval`"* — deliberately.
Nothing waggle produces ever reaches an approval step, so the tripwire must be a bespoke probe
that creates a throwaway `request_approval` workflow against a live relay and observes the run
status. That is a meaningfully different (and CI-and-relay-dependent) test from what the Rule
implies, and it does not exist. Meanwhile `SubstrateNativeGate` is specified as a dormant adapter
with no consumer and no test — dead structure the spine mandates on the strength of a fix that
may never land. Given AD-10's own (correct) argument that log reconciliation *is* FR-22's
implementation rather than a workaround, the second adapter may not be worth specifying at all.

---

## Low

### L-1 · Key-file layout is a real divergence point the custody deferral does not cover

*"**Key custody beyond local files.** AD-14 fixes the boundary. Whether secrets live in files, an
OS keychain, or an external manager is an operator-facing choice that does not change the
boundary."* (lines 479–481)

True about the boundary; silent about the on-disk contract, which two units will get differently.
Today `events.rs:66` reads `keys/<role>.nsec` and parses its contents as **hex**, while
`identity.rs` writes `<role>.key` / `<role>.pub` and `keys/` in this repo contains both
`tea.nsec`/`tea.pub` and `tea-agent.key`/`tea-agent.pub`. A `.nsec` extension holding hex rather
than bech32 is a trap for anyone who has read NIP-19. AD-14's Rule also says secrets *"are written
only to paths covered by the repository ignore rules"* without naming the path; `.gitignore` covers
`*.nsec` and `/keys/` but not `*.key`, which `identity.rs` writes.

### L-2 · AD-20's exit-code taxonomy silently diverges from the precedent it wraps

Line 253: *"Exit codes are a fixed taxonomy: success, user error, upstream-contract error, system
failure."* research §6.6 records the substrate's own five: *"`buzz-cli` exit codes are
`0=ok 1=input 2=relay/network 3=auth 4=other 5=write conflict` — **a concrete precedent for AD-20's
taxonomy**."* Since `waggle-hive` shells out to that CLI (H-4), every wrapper must map five codes
onto four, and the spine gives no mapping — notably for `3=auth` and `5=write conflict`, which fit
none of waggle's four categories cleanly. Two units will map them differently, and exit codes are
declared a **public compatibility surface** by PRD §12.

### L-3 · "Verified current at authoring" overstates what the Stack row means

Line 305 heads the Stack *"Seed. Verified current at authoring, 2026-07-28"*, but the first row
pins Rust 1.95.0 while the memlog records *"Verified current 2026-07-28: Rust stable 1.97.1"*. The
pin is correct (matched to the substrate) and the reasoning is sound — but the heading implies every
row is latest-stable, which the load-bearing row deliberately is not. One clarifying clause.

### L-4 · Output paths are written but never named

AD-3 (lines 74–77) permits writing to *"`_bmad/custom/` … and to its own output directory"* without
naming it. In practice waggle writes `packs/`, `templates/`, and `keys/` at the repository root.
These are conventions two units would guess differently, and `packs/<module>/` in particular is the
compiler's product path referenced by `persona-pack-contract.md` §2. One row in Consistency
Conventions.

---

## What the spine gets right (so the fixes do not undo it)

- **AD-7** is confirmed by real data, not assumption: TEA has 10 menu items, 9 skills + 1 prompt
  (`persona-pack-contract.md` §8). The sum type is correct and load-bearing.
- **AD-10**'s argument — that log reconciliation is FR-22's *implementation* rather than a
  workaround for UP-01 — is the strongest reasoning in the document and survives every measurement
  since.
- **AD-16** holds in the code: `grep` for module literals in `waggle-core`/`waggle-emit` returns
  hits only inside `#[cfg(test)]`. SM-C1 is currently zero.
- **AD-5**'s non-skippable differential test exists and `panic!`s rather than skipping when the
  method's resolver is absent (`waggle-method/src/descriptors.rs:160–175`) — the rule is genuinely
  enforced, which is rare in this document.
- **AD-8/AD-9** correctly anticipated the kind selection that measurement later confirmed
  (research §7.4).

## Coverage check against the PRD

FR-1…FR-28 all appear in the Capability → Architecture Map. Gaps found by inspection rather than
by absence from the table:

| Requirement | Status |
|---|---|
| FR-2 (persistent facts as references) | Partially uncovered — M-7 |
| FR-14 (published profile matches descriptor) | Blocked on an undecided mechanism — H-7 |
| FR-16 (oversized artifacts) | Mandated mechanism is unavailable — C-1 |
| FR-18 (patch events) | No size discipline; published via a sig-stripping transport — C-1, H-4 |
| FR-20 (unauthorized reactions do not advance) | No enforcement locus — C-2 |
| FR-22 (independently verifiable gate record) | Signer undecided; reads are sig-stripped — C-2, H-4 |
| **NFR-2** (idempotence) | **No AD, no map row, zero mentions** — C-3 |
| **NFR-8** (bounded concurrency) | Mentioned only in Deferred as a number; no AD, no owning crate |

`NFR-1, 3, 4, 5, 6, 7, 9, 10` are each named by at least one AD and adequately covered.

---

## Recommended disposition

**Do not hand off at `status: final`.** Minimum before implementation continues:

1. Rewrite **AD-15** against measured reality (C-1) and add per-kind size discipline.
2. Decide **who signs a gate record** and **where authorization is enforced** (C-2) — this is SM-2.
3. Add an **idempotence AD** binding NFR-2 (C-3).
4. Reconcile the spine with the existing crate layout, or record the refactor as a decision (H-1).
5. Decide the **CI / image-publishing / client-binary** dimension (H-2, H-5) or defer it explicitly.
6. Add an AD for **flat, fully-resolved persona emission** (H-6) and one for **agent instantiation**
   (H-7).
7. Refresh the three stale `Deferred` entries (H-6, M-4, M-5) — each currently permits divergence
   rather than deferring a question.

The memlog's own closing note anticipated this gate: *"Reviewer-gate subagents not dispatched …
Worth re-running with independent reviewers before implementation."* It was right to.
