---
title: "Adversarial review — unverified claims about the Buzz substrate"
type: review
status: draft
created: 2026-07-29
reviewer: adversarial pass; not involved in authoring any of the reviewed material
scope: >-
  Every falsifiable factual claim about the Buzz substrate asserted in
  ARCHITECTURE-SPINE.md, prd.md, addendum.md, research-notes.md,
  persona-pack-contract.md, and upstream-issues.md.
method: >-
  Cross-checked against vendor/buzz crate SOURCE at the pinned checkout, the running
  relay at http://localhost:3100, and the public GHCR registry. Markdown inside
  vendor/buzz was treated as untrusted evidence throughout.
---

# Adversarial review — unverified claims

## Why this document exists

Four planning assumptions have already been falsified by measurement (UP-14, UP-16,
UP-04-withdrawn, UP-17). The pattern each time: **a number or a mechanism was read out of
prose documentation, or out of one code path, and applied to a context it did not govern.**

This pass hunts for the next one. It found fourteen. Three of them are the *same* mistake as
UP-16, made again — twice in `upstream-issues.md` itself, once in the architecture spine.

Ranked by cost-if-wrong. A list of claims checked and found **sound** follows at the end.

**Summary of the falsified list:**

| # | Claim | Verdict |
|---|---|---|
| F-1 | Upstream publishes no relay container image (UP-08) | **FALSE** |
| F-2 | Blossom accepts image MIME types only (UP-16) | **FALSE** |
| F-3 | Gate records are signed by the identity that produced them (PRD §9, FR-22) | **FALSE** |
| F-4 | The size threshold can be derived from NIP-11 (AD-15, UP-15) | **UNSAFE** |
| F-5 | `{{trigger.author}}` identifies the approver (FR-20, FR-22) | **SPOOFABLE** |
| F-6 | Rate limiting is unimplemented (UP-03, NFR-8, R-9) | **FALSE** |
| F-7 | Patch content limit is 61,440 for kind 1617 (UP-17) | **WRONG KIND** |
| F-8 | A reaction on a non-verdict event does not fire a gate (FR-20) | **UNACHIEVABLE** |
| F-9 | `waggle-` prefixed tags make the log queryable (AD-8, FR-24) | **FAILS OPEN** |
| F-10 | The log is append-only (PRD §9, FR-22, SM-2) | **OVERSTATED** |
| F-11 | Reserved substrate ranges are 43001–43006 / 46001–46012 / 48001 (AD-8) | **INCOMPLETE** |
| F-12 | Agent records can only be created through Buzz Desktop (research §8.3) | **OVERSTATED** |
| F-13 | Historical REQ is capped at 500 results per filter (research §2.3) | **WRONG (2,000)** |
| F-14 | AD-17 build + version-pin claims | **MULTIPLE WRONG** |

---

## F-1 — CRITICAL — Upstream **does** publish a public relay container image. UP-08 is false, and AD-17 builds a pipeline for a solved problem.

**The claim.** `docs/upstream-issues.md`, UP-08, status **`confirmed`**:

> **UP-08 — Buzz publishes no relay container image**
> **Status:** `confirmed` — checked GitHub releases and `orgs/block/packages`, 2026-07-28.
> release assets for `v0.4.26` are desktop binaries only … **No relay image is published to
> GHCR or any public registry.** … per `AGENTS.md` the relay image is built by an internal
> Block pipeline (`squareup/sprout-oss`) and pushed to a private ECR.
> **Candidate contribution:** publishing an official relay image to GHCR … **Probably the
> highest-value upstream ask on this list.**

`ARCHITECTURE-SPINE.md`, AD-17 rests entirely on it:

> **Prevents:** every operator needing the substrate's full build toolchain, **which no
> published image currently relieves them of — upstream ships desktop binaries only** `[BUZZ]`

**What the source actually says.** `vendor/buzz/.github/workflows/docker.yml:1-40`:

```
name: Docker image
# Builds and publishes the public Buzz relay image as ghcr.io/block/buzz.
# Versioning: the relay is versioned independently of the desktop app via
# its own `relay-v*` tags … Desktop `v*` tags and agent `sprig-v*` tags do NOT
# publish this image — only `relay-v*` does
# Triggers:
#   - push to main           → :main + :sha-<7>
#   - push tags relay-v*.*.* → :{version} + :{major}.{minor} + :{major} (+ :latest for stable)
```

`vendor/buzz/deploy/compose/compose.yml:5` — upstream's own deploy bundle pulls it:
`image: ${BUZZ_IMAGE:-ghcr.io/block/buzz:main}`

**Measured against the live registry** (anonymous pull token, 2026-07-29):

```
GET https://ghcr.io/v2/block/buzz/tags/list
→ {"name":"block/buzz","tags":["main","latest","0.3.20-rc.win.2","0.1.0","0.1.1","0.1","0",
                               "sha-1fa63ba", … ]}
GET https://ghcr.io/v2/block/buzz/manifests/main   → 200
```

Public, anonymously pullable, multi-arch, semver-tagged.

**Why the mistake happened — the UP-14 pattern exactly.** The research checked release assets
for tag `v0.4.26`. `v0.4.26` is the **desktop** release line; relay images publish on `relay-v*`
tags, a separate version axis. Checking the wrong tag produced a "confirmed" negative.
`AGENTS.md`'s ecosystem table (which does describe `sprout-oss` → private ECR) was read as
exhaustive; the workflow file sitting next to it contradicts it.

**Cost if wrong.** AD-17 is an entire deliverable — a CI image build, a waggle registry, image
provenance naming upstream tag and commit, digest pinning in the compose bundle — carrying FR-8,
NFR-5, and OQ-6's resolution. All redundant. It also means the "highest-value upstream
contribution on this list" proposes something upstream already ships.

---

## F-2 — CRITICAL — Blossom **does** accept markdown. UP-16's evidence is a client-side allowlist in `buzz-cli`. FR-16's mechanism was abandoned for no reason.

**The claim.** `docs/upstream-issues.md`, UP-16, status **`confirmed`**:

> **Status:** `confirmed` — `buzz upload file --file x.md` returns
> `unsupported file type: application/octet-stream`. `ALLOWED_MIME_TYPES` in
> `buzz-media/src/validation.rs` is `["image/jpeg", "image/png", "image/gif", "image/webp"]`.
> **Impact:** **this blocks FR-16's chosen mechanism.** … the substrate's media store cannot
> hold a markdown document, so there is nothing to reference.
> **Our mitigation:** refuse oversized artifacts with a specific error…

**What the source actually says.**

**1. The quoted error is emitted by the CLI, before any HTTP request is made.**
`vendor/buzz/crates/buzz-cli/src/client.rs:64-70, 1116-1117`:

```rust
/// MIME types accepted for upload.
const ALLOWED_MIMES: &[&str] = &["image/jpeg","image/png","image/gif","image/webp","video/mp4"];
…
if !ALLOWED_MIMES.contains(&mime.as_str()) {
    return Err(CliError::Usage(format!("unsupported file type: {mime}")));
}
```

`CliError::Usage` — the relay never saw the file.

**2. `ALLOWED_MIME_TYPES` in `buzz-media` governs the *image* pipeline only.**
`vendor/buzz/crates/buzz-media/src/validation.rs:11-15`:

> `/// Accepted MIME types for the **image upload path**.`
> `/// video/mp4 is intentionally excluded — video uploads use a separate pipeline`

**3. A third, generic-file pipeline explicitly accepts markdown.**
`vendor/buzz/crates/buzz-media/src/validation.rs:153-156`:

> `/// Files with no detectable signature (plain text, CSV, source code, JSON —`
> `/// none of which have magic bytes) are accepted as `application/octet-stream`.`
> `/// They are always served as downloads…`

**4. The relay routes to it based on which endpoint you hit.**
`vendor/buzz/crates/buzz-relay/src/api/media.rs:54-58` and `:369-399`:

```rust
fn upload_route_mode(path: &str) -> Result<UploadRouteMode, MediaError> {
    match path {
        "/upload"       => Ok(UploadRouteMode::Upload),        // Blossom
        "/media/upload" => Ok(UploadRouteMode::LegacyMedia),   // images only
```
```rust
if is_image { process_upload(…) }
else if auth.route_mode == UploadRouteMode::LegacyMedia {
    return Err(MediaError::DisallowedContentType(mime));   // ← the "images only" behaviour
} else {
    process_file_upload(…)                                 // ← markdown lands here
}
```

Both routes registered at `crates/buzz-relay/src/router.rs:39-40`.

**So:** `PUT /upload` with Blossom (kind 24242) auth accepts an arbitrary markdown artifact up
to `max_file_bytes` — **100 MB** default (`crates/buzz-relay/src/config.rs:630-645`) — stores it
content-addressed by SHA-256, and serves it at `/media/{sha256_ext}`. That is exactly the
content-addressed reference with hash verification AD-15 specified.

**Cost if wrong.** FR-16, AD-15's resolution of OQ-2, and R-7's mitigation were all written off.
The stated mitigation — "refuse oversized artifacts" — is a deliberate capability regression
adopted on false evidence.

**Note the recursion.** UP-07 and UP-13 already established *"`buzz-cli` is not the substrate;
speak the relay protocol directly."* UP-16 was filed **after both** and failed to apply it. That
lesson needs to be a checklist item, not a paragraph.

---

## F-3 — CRITICAL — The gate record is signed by the **relay keypair**, not by any agent or the approver, and can carry no tags. This breaks PRD §9, FR-22, FR-24, AD-8, and SM-2 at once.

**The claim.** `prd.md` §9:

> Every artifact, handoff, and gate record is signed by the identity that produced it.
> **No shared service identity produces content attributable to a role.**

`prd.md` FR-22: *"A gate record's signature verifies independently."*
`ARCHITECTURE-SPINE.md` gate-firing diagram: `Gate->>Relay: publish gate record (signed)`.
`research-notes.md` §7.4 records the chain as verified without naming the signer.

**What the source actually says.** `vendor/buzz/crates/buzz-workflow/src/action_sink.rs:57-62`:

> `/// - author_pubkey: hex-encoded pubkey of the workflow owner (used for`
> `///   the p attribution tag; **the relay keypair signs the event**)`

`vendor/buzz/crates/buzz-relay/src/workflow_sink.rs:253-267, 301-305`:

```rust
// 3. Build kind:9 Nostr event
//    - Signed by relay keypair (event.pubkey = relay pubkey)
//    - `p` tag attributes the message to the workflow owner
//    - `h` tag scopes to the channel …
//    - `buzz:workflow` tag prevents recursive workflow triggering
let mut tags = vec![ p(author), h(channel), ["buzz:workflow","true"] ];
…
let event = EventBuilder::new(kind, &text).tags(tags)
    .sign_with_keys(&state.relay_keypair)?;
```

`vendor/buzz/crates/buzz-workflow/src/executor.rs:559` — the `p` tag is the **workflow owner**
(whoever published the kind:30620 definition), not the approver:
`let owner_pubkey_hex = hex::encode(&workflow.owner_pubkey);`

Three consequences:

1. **Signer.** Every gate record reads `pubkey = <relay>`. UP-07 already ruled that the relay
   vouching for content is *"the relay vouching, not independent verification"* and unacceptable
   for FR-22. This is exactly that, structurally.
2. **Tags.** The tag set is hard-coded with no extension parameter. A workflow `send_message`
   cannot emit an `e` tag to the verdict event, a classification tag, or a `P0`–`P3` priority
   tag. AD-8's "typed tags" and FR-24 are unreachable on this path — UP-13 again, on the gate
   path rather than the CLI path.
3. **Attribution.** The approver appears nowhere structurally. waggle already ships the
   workaround — `crates/waggle-emit/src/workflow.rs:56-59` interpolates it into free text:
   `verdict-event: {{trigger.message_id}}` / `approver: {{trigger.author}}` / … That is prose in
   a `content` field, not a queryable, verifiable record. And see F-5: it is forgeable.

**Cost if wrong.** SM-2 (*"Gate is reconstructible from the log alone … and signatures verify"*)
is a **binary primary** success metric and fails on this path. The fix is architectural: waggle
must publish the gate record itself, under an agent identity, via `POST /events` after observing
the reaction — treating the substrate workflow as a notification at most. That is a different
design from the one AD-10/AD-11 describe.

---

## F-4 — HIGH — AD-15 derives its size threshold from a number the operator can change, while the real ceiling is a hard constant. UP-15's "halve `max_message_length`" heuristic works only by coincidence at default config.

**The claim.** `ARCHITECTURE-SPINE.md` AD-15: *"The threshold is read from the pinned
substrate's actual limit … never hard-coded independently."* `upstream-issues.md` UP-15:

> AD-15 requires the threshold be *derived* from the substrate rather than hard-coded; the only
> derivable number is the wrong one. **waggle reads `max_message_length` and halves it, which
> matches the observed value — a coincidence that should not be relied on indefinitely.**

waggle implements exactly that: `crates/waggle-hive/src/events.rs:127-167`.

**What the source actually says.** The two numbers are unrelated quantities, and only one of
them moves:

- `max_message_length` **is the WebSocket frame limit, and it is env-tunable.**
  `crates/buzz-relay/src/nip11.rs:96-103` takes it as a parameter; the unit test
  `nip11.rs:456-459` is literally named `max_message_length_uses_configured_frame_limit`. The
  source value is `config.max_frame_bytes` — `crates/buzz-relay/src/config.rs:14`
  `DEFAULT_MAX_FRAME_BYTES: usize = 512 * 1024`, overridable via **`BUZZ_MAX_FRAME_BYTES`**
  (`config.rs:464-468`).
- The content ceiling is a **hard `const` that no configuration touches.**
  `crates/buzz-relay/src/handlers/ingest.rs:1516-1521`:

  ```rust
  const MAX_EVENT_CONTENT_BYTES: usize = 256 * 1024; // 256 KB
  if event.content.len() > MAX_EVENT_CONTENT_BYTES {
      return Err(IngestError::Rejected(format!(
          "invalid: content exceeds maximum size of {} bytes (got {})", …)));
  ```

512 KB ÷ 2 = 256 KB is arithmetic, not a relationship. Set `BUZZ_MAX_FRAME_BYTES=1048576` and
waggle's derived threshold becomes 512 KB — every artifact between 256 KB and 512 KB is then
published inline and **rejected by the relay**. Lower it and waggle silently under-uses capacity.

**Also correct the frame number itself.** UP-14 is titled *"The 64 KB frame limit is not an
event size limit."* 65,536 is not the frame limit either — the default is **512 KB**. The
65,536 in `ARCHITECTURE.md` (and thus in `research-notes.md` §2.3 and the spine's AD-15) matches
nothing this relay enforces.

**Cost if wrong.** A silent, config-dependent publish failure on the exact path FR-16 governs.
Fix: hard-code 262,144 with a comment naming `ingest.rs`'s constant and a test that fails when
the pinned substrate's constant moves — the honest version of AD-15's intent, since the number
genuinely is not derivable.

---

## F-5 — HIGH — `{{trigger.author}}` is client-spoofable. Gate approver attribution is forgeable.

**The claim.** Implied by FR-20 (*"Reactions by non-authorized identities do not advance the
gate"*), AD-13, and FR-22 (*"identifies … the approving identity"*). waggle uses
`{{trigger.author}}` as the approver (F-3).

**What the source actually says.** `vendor/buzz/crates/buzz-workflow/src/lib.rs:888-901` —
`build_trigger_context` prefers an `actor` **tag** over the event's own pubkey, on *any* event:

```rust
let author = event.event.tags.iter()
    .find_map(|tag| if tag.kind().to_string() == "actor" { tag.content().map(…) } else { None })
    .unwrap_or_else(|| event.event.pubkey.to_hex());
```

The relay's ingest path applies the same convention **only for relay-signed events**, which is
the correct guard — `vendor/buzz/crates/buzz-relay/src/handlers/ingest.rs:729-733`:

```rust
pub(crate) fn effective_message_author(event: &Event, relay_pubkey: &nostr::PublicKey) -> Vec<u8> {
    if event.pubkey == *relay_pubkey {
        // Workflow-generated or legacy relay-signed attributed event — real author
        // in "actor" or "p" tag.
```

`buzz-workflow` has no such guard. A user-signed `kind:7` carrying
`["actor","<someone else's pubkey>"]` makes `{{trigger.author}}` report that other pubkey.

**Cost if wrong.** The approver line in every gate record is attacker-controlled, inside an
event signed by the relay (F-3) — so it *looks* authoritative. Authorization must read the
reaction event's real `pubkey` via `POST /query`, never the template variable. Also a clean
upstream bug report.

---

## F-6 — HIGH — UP-03 is false: the rate limiter is fully implemented, wired, and returns `429`. NFR-8 and R-9 are framed backwards.

**The claim.** `docs/upstream-issues.md` UP-03 (source: `ARCHITECTURE.md`, *"currently
unstub"*): *"Rate limiter is a trait with only a test stub … a runaway agent loop has **nothing
throttling it**."* Repeated at `research-notes.md` §1.4 #3. `prd.md` NFR-8: *"…since **the
substrate's own rate limiting is not yet implemented**."* R-9 likewise.

**What the source actually says.** A production Redis-backed implementation exists and is wired
into relay state:

- `vendor/buzz/crates/buzz-pubsub/src/rate_limiter.rs:3` — *"Implements the [RateLimiter] trait
  from buzz-auth"*; `:92-99` `impl RateLimiter for RedisRateLimiter`.
- `vendor/buzz/crates/buzz-relay/src/state.rs:712` —
  `let admission_rate_limiter = Arc::new(RedisRateLimiter::new(redis_pool.clone()));`
- `AlwaysAllowRateLimiter` (`buzz-auth/src/rate_limit.rs:222`) is documented as *"A no-op …
  provided for **unit tests**"* — the stub the claim mistook for the whole story.

Enforced defaults (`buzz-auth/src/rate_limit.rs:110-160`; `.env.example:60-66`):

| Limit | Default |
|---|---|
| `human_messages_per_min` | 60 |
| `human_api_calls_per_min` | **300** |
| `human_ws_events_per_sec` | 10 |
| `agent_standard_messages_per_min` | 120 |
| `agent_standard_api_calls_per_min` | 600 |

And the HTTP bridge — the path waggle chose — returns `429`
(`vendor/buzz/crates/buzz-relay/src/api/bridge.rs:29-47`):

```rust
let limit = state.auth.config().rate_limits.human_api_calls_per_min;
… LimitType::ApiCalls, 60, limit …
Err(AdmissionError::Exceeded { reset_in_secs }) =>
    Err(api_error(StatusCode::TOO_MANY_REQUESTS,
        &format!("rate-limited: quota exceeded; retry in {reset_in_secs}s")))
```

WebSocket publishes hit `LimitType::Messages` (`connection.rs:612-642`); media uploads and
invite claims have their own scoped limiters.

**Cost if wrong.** This inverts a whole risk. The danger is not "nothing throttles us" but
**"we will be throttled and have designed no response."** UP-07 + UP-13 concentrated *all* of
waggle's signed-trail traffic onto `POST /events` and `POST /query` — capped at 300 calls/min
per pubkey with a hard `429`. Nothing in the PRD, addendum, or architecture mentions backoff,
retry, `Retry-After`, or `429` as an error class in AD-20's exit-code taxonomy. A compile that
publishes one artifact event per agent per story will reach it. NFR-8's mitigation (bound
concurrency our side) is still right, for the opposite reason.

---

## F-7 — HIGH — UP-17's 61,440 limit governs kind **40008**, not NIP-34 kind **1617**. Patches actually get 262,144, and the "small commit" mitigation is unnecessary.

**The claim.** `docs/upstream-issues.md` UP-17, status `confirmed`:

> | kind:1617 patch content | 61,440 | `content exceeds maximum size of 61440 bytes (got 83562)` |
>
> **Impact:** A patch limit four times smaller than the message limit is genuinely surprising,
> and a real `git format-patch` of a documentation-heavy commit exceeds it easily…
> **Our mitigation:** … Test fixtures deliberately select a small commit rather than assuming
> any commit fits.

**What the source actually says.** The 61,440 check exists in exactly one place and is gated on
one kind — `vendor/buzz/crates/buzz-relay/src/handlers/ingest.rs:895-902`:

```rust
/// Validate kind:40008 diff event metadata tags.
fn validate_diff_event(event: &Event) -> Result<(), String> {
    // Content max 60KB
    if event.content.len() > 61_440 {
        return Err(format!("diff content exceeds 60KB limit (got {} bytes)", …));
```

invoked only at `ingest.rs:1998-2000`:

```rust
if kind_u32 == KIND_STREAM_MESSAGE_DIFF {   // 40008 — a Buzz-proprietary kind
    validate_diff_event(&event) …
```

Note the error string does not match either: the diff validator says `"diff content exceeds
60KB limit (got N bytes)"`, whereas the text quoted in UP-17 is the **global** gate's wording
from `ingest.rs:1518`. There is **no** kind-specific limit for `1617` anywhere; it falls under
the single 256 KB global gate.

waggle publishes kind 1617 (`crates/waggle-hive/src/patches.rs:36`, *"Send a `git format-patch`
file as a NIP-34 kind:1617 event"*), so the 83 KB patch that was rejected was not a 1617 — it
implies something on the diff path (40008) was exercised instead, which AD-8/NFR-6 would in any
case forbid as substrate-proprietary.

**Cost if wrong.** FR-18 was scoped around a 61,440 ceiling that does not apply to it — a 4×
under-estimate on the product's flagship portability feature, plus a permanent test-fixture
constraint adopted for no reason. Re-measure by publishing an oversized **kind 1617** over
`POST /events` before changing anything.

---

## F-8 — HIGH — The reaction trigger cannot filter on *what was reacted to*. FR-20's stated consequence is not achievable from the substrate.

**The claim.** `prd.md` FR-20, Consequences (testable):

> - A reaction of the designated type on a verdict event triggers the gate workflow.
> - **A reaction on a non-verdict event does not fire a gate.**

**What the source actually says.** `vendor/buzz/crates/buzz-workflow/src/schema.rs:38-53` —
`ReactionAdded` is the only trigger variant with no `filter` field:

```rust
MessagePosted { filter: Option<String> },   // has a filter
ReactionAdded { emoji: Option<String> },    // ← emoji only, no filter
DiffPosted    { filter: Option<String> },   // has a filter
```

`lib.rs:806-823` confirms only the emoji is compared. `lib.rs:884-950`
(`build_trigger_context`) shows everything a reaction exposes: `text` (= the emoji), `author`,
`channel_id`, `timestamp`, `emoji`, `message_id` (the target's id). **Nothing about the target
event's kind, content, tags, or author.** A step `if` cannot look it up either — `evalexpr`
runs with a 100 ms hard timeout and no I/O (`executor.rs:341-370`).

**Cost if wrong.** Every ✅ anywhere in a story channel fires the gate workflow, which then
publishes a relay-signed "gate record" (F-3) referencing a non-verdict event. FR-20's second and
third consequences cannot be satisfied by configuration. The log fills with
authoritative-looking junk, which is worse for SM-2 than an outright failure.

---

## F-9 — HIGH — Filters on `waggle-`prefixed tags are **silently discarded** by the relay's filter parser. They fail open, returning everything.

**The claim.** `ARCHITECTURE-SPINE.md` AD-8: *"group message `9` with typed tags for artifacts
and handoffs."* Consistency Conventions table:

> | Event tags | Lowercase kebab-case tag names, `waggle-` prefixed where waggle-specific… |

`prd.md` FR-24: *"Artifacts can be filtered by priority from the log."* Vision §1: *"one query
against one cryptographically verifiable log."*

**What the source actually says.** The relay uses `nostr` 0.44's `Filter`, whose generic tag map
is keyed by `SingleLetterTag`. Its deserializer **discards** any `#…` key that is not exactly
one character — `nostr-0.44.6/src/filter.rs:986-995`:

```rust
if let (Some('#'), Some(ch), None) = (chars.next(), chars.next(), chars.next()) { … }
else { map.next_value::<serde::de::IgnoredAny>()?; }   // ← silently dropped
```

So `{"kinds":[9],"#waggle-gate-verdict":["FAIL"]}` is parsed as `{"kinds":[9]}` and returns
**every kind-9 event in scope, with a success response.** No error, no warning — the query fails
*open*, which is the dangerous direction.

**Two mitigating facts, and a residual defect:**

- The implementation already avoids the trap: `crates/waggle-core/src/artifact.rs:136-152` puts
  classification and priority on **`t`** tags and references on **`e`**, with multi-char
  `waggle-*` tags kept explicitly descriptive. So the *code* is right; the **planning documents
  are not**, and the spine's tag convention as written would produce an unqueryable log.
- Residual: `#t` is **not** pushed into SQL. `crates/buzz-relay/src/handlers/req.rs:782-812`
  (`filter_fully_pushable`) documents that `#t` and `#a` are post-filtered in memory, and
  `crates/buzz-db/src/event.rs:347-348` applies `LIMIT` **before** that post-filter. A `#t: P0`
  query in a busy channel can therefore silently return an incomplete set. FR-24's
  *"Artifacts can be filtered by priority from the log"* is not reliably true at scale.

Filters that are genuinely index-backed: `#e` (JSONB containment, `event.rs:447-465`), `#p`
(via `event_mentions`, `req.rs:926-933`), `#d` (only when every kind in the filter is
parameterized-replaceable, `req.rs:943-965`), `#h` (the `channel_id` column, `req.rs:966-999`).

**Cost if wrong.** A wrong-but-successful query is the worst failure mode for an audit product.
Fix the spine's tag convention to say "single-letter `t`/`e`/`h` for anything queryable;
`waggle-*` names are descriptive only, never filtered", and add a test that a `#waggle-*` filter
is never constructed. Restate FR-24 in terms of `#t` plus explicit paging.

---

## F-10 — MEDIUM — "The log is append-only" is overstated: kind 5 soft-deletes, and every read path hides deleted events.

**The claim.** `prd.md` §9: *"The log is append-only and tamper-evident; modifying any earlier
record is detectable."* FR-22: *"Tampering with any earlier record is detectable."*
`research-notes.md` §2.1 lists kind `5` as available for *"Retractions."*

**What the source actually says.** Deletion is implemented and effective, though **not
advertised in NIP-11** (see F-14):

- `crates/buzz-relay/src/handlers/side_effects.rs:150` → `handle_standard_deletion_event`, →
  `:2145-2153` → `db.soft_delete_event_and_update_thread`.
- `crates/buzz-db/src/event.rs:806-812` — `UPDATE events SET deleted_at = NOW() …`
- Every read path filters it, e.g. `crates/buzz-db/src/event.rs:372` — `AND deleted_at IS NULL`.
- Authorship is enforced (`side_effects.rs:179-232`: `target_author == actor_bytes` or agent
  owner), and exactly one target is required (`ingest.rs:1972-1983`).

Rows are retained (`get_event_by_id_including_deleted` exists), so this is hide-not-erase — but
a normal reader, including waggle's own `POST /query` reconciliation, cannot see a deleted
event and gets no signal that one existed.

**Cost if wrong.** An agent can retract its own artifact or verdict event and a
log-reconciled gate (AD-10) will simply not find it. "Reconstruction of any gate decision
requires the log and nothing else" (PRD §9) holds only if nothing was deleted, and waggle has
no way to detect that from the relay's read surface. Either state the caveat explicitly, or
cross-check against `buzz-audit`'s hash chain (kind 48001) — which the docs cite as *"the
auditable log our mission promises"* but which no requirement actually reads.

---

## F-11 — MEDIUM — AD-8's "reserved substrate ranges" list is materially incomplete; a claimed custom kind can collide.

**The claim.** `ARCHITECTURE-SPINE.md` AD-8: *"Reserved substrate ranges `43001`–`43006`,
`46001`–`46012`, `48001` are never used."* Same list in `research-notes.md` §1.6 and
`addendum.md` OQ-4.

**What the source actually says.** `vendor/buzz/crates/buzz-core/src/kind.rs` registers far more.
Buzz-proprietary kinds **outside** the three stated ranges include:

`8000`–`8003`, `9035`/`9036`, `9040`–`9044`, `13534`/`13535`, `24200`, `24810`, `28936`,
`30174`–`30177`, `30300`, `30350`, `30620`, `30622`, `39003`, `39005`, `39006`,
`40004`–`40008`, `40099`, `40100`, `40901`, `40902`, `41001`, `41010`–`41012`, `42000`,
`44100`/`44101`, `44200`, `45001`–`45003`, **`46020`, `46030`, `46031`**, **`48100`–`48106`**,
`49001`.

`46020`/`46030`/`46031` (workflow trigger, approval grant, approval deny) and `48100`–`48106`
(huddles) sit immediately outside the ranges the docs call reserved — the two places a reader
following AD-8's rule would most plausibly reach.

Two further constraints AD-8 omits:
- **Kinds are u16-bounded.** `kind.rs:770-780`: `event_kind_u32` is `event.kind.as_u16() as u32`
  and `event_kind_i32`'s comment reads *"all Buzz kinds fit in i32 (max 65535)"*.
- `ALL_KINDS` (`kind.rs:566`) is the authoritative list, with a duplicate-detection test
  (`kind.rs:828-835`). It, not any prose doc, is the thing to diff against.

**Cost if wrong.** AD-8 permits claiming a custom kind with written rationale. A claim landing
on `46020` or `48100` silently aliases a substrate kind and inherits its routing, filtering, or
loop-suppression behaviour. Cheap to prevent now; expensive after the log has events in it,
because PRD §12 makes published event shapes a compatibility surface.

---

## F-12 — MEDIUM — Agent records **can** be created headlessly. Research §8.3's blocker is narrower than recorded, and Stories 1.7 / 2.7-2.8 may be under-scoped as a result.

**The claim.** `research-notes.md` §8.3:

> **Roster membership needs a live managed agent.** … Those are created by
> `buzz agents draft-create`, which *"opens a prefilled create-agent form in the owner's Buzz
> Desktop"* — a human-in-the-loop desktop flow **with no headless equivalent**. This is the
> **same blocker as Story 1.7**: without a running agent instance there is nothing to add to a
> channel.

**What the source actually says.** The description of `draft-create` is accurate — it publishes
an *ephemeral NIP-44 observer frame* for owner review, not a record
(`crates/buzz-cli/src/commands/agents.rs:31-41`: *"Draft sent to Buzz Desktop for owner review.
Nothing changes until the owner saves it."*, `"saved": false`). But the conclusion does not follow:

- **Kind 30177 (`KIND_MANAGED_AGENT`) is an ordinary self-authored event, publishable via
  `POST /events`.** `crates/buzz-relay/src/handlers/ingest.rs:200-205` maps
  `KIND_PERSONA | KIND_TEAM | KIND_MANAGED_AGENT` to `Scope::UsersWrite` — that is the *only*
  authorization requirement. No owner role, no desktop gate. It is stored through the generic
  NIP-33 path (`ingest.rs:2401-2412`), constrained only by `D_TAG_MAX_LEN = 1024`. There is no
  `KIND_MANAGED_AGENT` arm in `handle_side_effects` at all (`side_effects.rs:148-171`).
- **Kind 30175 (persona)** has one purely structural validator —
  `validate_persona_envelope` (`ingest.rs:1034-1080`): one non-empty `d` tag matching
  `^[a-z0-9][a-z0-9_-]{0,63}$`, at most one `["shared","true"]`. No owner check.
- **Roster membership is headless today.** `crates/buzz-cli/src/lib.rs:646-658` —
  `channels add-member --channel <uuid> --pubkey <hex> --role <owner|admin|member|guest|bot>`,
  a plain kind-9000 NIP-29 put-user.

The genuine residual gap is narrow and worth stating precisely: publishing 30177 creates a
**record**, not a **running process**, and the record is explicitly a redacted projection —
`crates/buzz-core/src/kind.rs:251-258`: *"Content is an explicit opt-IN allowlist projection …
it MUST never carry the agent's secret key, NIP-OA auth tag, env vars, or runtime fields."*

**Cost if wrong.** "Roster membership deferred with Story 1.7" defers work that is achievable
now, and the §8.4 rescope table lists agent roster as blocked when only the agent *runtime* is.
Re-test by publishing a 30177 over `POST /events` and re-running `channels create --template`.

---

## F-13 — MEDIUM — "Historical REQ queries are hard-capped at 500 results per filter" is wrong. It is 2,000. NIP-11 advertises 10,000.

**The claim.** `research-notes.md` §2.3: *"Historical REQ queries are hard-capped at **500
results per filter**."* Used in `addendum.md` OQ-2 to argue against chunking:

> Note the interaction with the **500-result historical query cap** `[BUZZ]` — option C makes a
> single artifact consume many result slots.

**What the source actually says.** `vendor/buzz/crates/buzz-relay/src/handlers/req.rs:25`:

```rust
const MAX_HISTORICAL_LIMIT: i64 = 2_000;
```

applied at `req.rs:538-539` and `:881-882`. The 500s that exist are unrelated:
`BRIDGE_THREAD_MAX_LIMIT` (`api/bridge.rs:252`), `MODERATION_READ_LIMIT` (`:2075`), a user-search
clamp (`buzz-db/src/user.rs:238`). NIP-11 advertises a third number —
`crates/buzz-relay/src/nip11.rs:106`: `max_limit: Some(10_000)`, matching the live relay.

**This is the third advertised-vs-enforced mismatch**, alongside UP-15 and UP-17. UP-15 should
be widened: *no* limit this relay advertises matches what it enforces, and `max_limit` is a
static literal that is not even derived from the enforcing constant.

**Cost if wrong.** Low direct cost, but it was one of two inputs to an architecture decision —
AD-15 resolved OQ-2 partly by discounting option C on this number. With 2,000 slots (4× the
assumed budget) and F-2 restoring the reference mechanism, OQ-2 should be re-decided on correct
inputs rather than inherited.

---

## F-14 — MEDIUM — AD-17's build claims are overstated on three axes, and the version pin is on the wrong version axis entirely.

**The claims.** `ARCHITECTURE-SPINE.md` AD-17: *"…**The build is reproducible from the pinned
tag alone** … **Operators need only a container runtime**."* Stack table: *"Postgres / Redis /
MinIO — as pinned by the substrate's own compose"*, under NFR-5 *"No floating versions
anywhere."* `research-notes.md` §2.1: *"NIPs supported: 01, 04/44, 05, 09, 10, 11, 17, 25, 29,
42, 50, 70."*

| Sub-claim | Verdict | Evidence |
|---|---|---|
| The `Dockerfile` builds only the relay | overstated | `Dockerfile:67-72` also builds `buzz-admin` and `buzz-pair-relay`; `:77-112` runs a full pnpm/vite stage for `web` and `admin-web` |
| Rust build needs no live Postgres | **SOUND** | Zero `sqlx::query!` macros repo-wide; `buzz-db/src/lib.rs:10` states the invariant; sqlx `macros` feature off (`Cargo.toml:52-54`); no `.sqlx/`, no `build.rs`. UP-05's "Low" impact holds |
| "Reproducible from the pinned tag alone" | **WRONG** | `Dockerfile:54-61,127-133` run unpinned `apt-get install`; base images float on minor (`rust:1.95-bookworm`, `node:24-bookworm-slim`, `debian:bookworm-slim`) |
| "Operators need only a container runtime" | **WRONG for the build** | needs egress to crates.io, npmjs, github.com; `Cargo.toml:171` pulls a git dep from a personal fork (`github.com/tlongwell-block/rust-s3`). Moot for operators anyway — see F-1 |
| Compose pins PG/Redis/MinIO | **WRONG** | `docker-compose.yml:104,133,57,152` — `minio/minio:latest`, `minio/mc:latest`, `adminer:latest`, `prom/prometheus:latest`. PG/Redis are major-only (`postgres:17-alpine`, `redis:7-alpine`) |
| Substrate "requires Rust 1.88+" (Stack prose) | understated | `Cargo.toml:37` `rust-version = "1.88.0"` is the MSRV floor; `rust-toolchain.toml` pins **1.95.0**; `Dockerfile:14` hard-codes `ARG RUST_VERSION=1.95`. `BUZZ_VERSION` already records this correctly; the spine's prose does not |
| Supported NIPs list | **WRONG** | `crates/buzz-relay/src/nip11.rs:15` — `SUPPORTED_NIPS = &[1,2,10,11,16,17,23,25,29,33,38,42,50,56]` (+43 conditionally, `:147-150`). 04, 05, 09, 70 are **not** advertised; 2, 16, 23, 33, 38, 56 are advertised and absent from the note. NIP-09 *is* implemented despite not being advertised (F-10); NIP-70 is required on specific kinds but has no generic enforcement |

**And the version axis.** `crates/buzz-relay/Cargo.toml:7` → `version = "0.2.0"`. The **live
relay's NIP-11 reports `"version":"0.2.0"`**. GHCR's relay tags are `0.1.0`, `0.1.1`, `0.1`,
`0`, `0.3.20-rc.win.2`. Meanwhile `BUZZ_VERSION` declares:

```
BUZZ_VERSION=v0.4.26
BUZZ_SUPPORTED=>=0.4.0,<0.5.0
```

`v0.4.26` is the **desktop** release line. AD-18/FR-28's supported range therefore constrains
nothing about the relay contracts waggle depends on, and **would reject every published relay
image**. Compounding it, `crates/waggle-hive/src/lib.rs:60-80` detects the version by running
`git describe --tags --exact-match` against a **source checkout**, which an operator following
FR-8/AD-17 ("only a container runtime") does not have. The preflight cannot run at all against
an image-based hive.

**Cost if wrong.** FR-28 is the primary guardrail against *"compiling against contracts that
have moved."* Today it guards the wrong number and cannot execute in the target deployment
shape. Fix: read `version` from NIP-11 (already served at `GET /`), re-pin the range against
relay versions, keep the checkout path as a fallback.

---

## Smaller corrections (worth fixing, low rework)

- **"media storage caps at 50 MB"** (`research-notes.md` §1.2, `addendum.md` OQ-2) is one of
  four caps. `crates/buzz-media/src/config.rs:34-43` + `crates/buzz-relay/src/config.rs:630-645`:
  image **50 MB** (`BUZZ_MAX_IMAGE_BYTES`), animated GIF **10 MB**, video **500 MB**, generic
  file **100 MB** (`BUZZ_MAX_FILE_BYTES`). The one that governs a markdown artifact (F-2) is
  100 MB.
- **±15-minute timestamp gate, undocumented anywhere in the plan.**
  `crates/buzz-relay/src/handlers/ingest.rs:1506-1513` —
  `MAX_TIMESTAMP_DRIFT_SECS: i64 = 900`; an event whose `created_at` is outside ±15 min of
  server time is rejected outright. Relevant to any batch publish, replay, or artifact carrying
  a method-assigned timestamp, and to clock skew on an operator machine. Should be an FR-15
  consequence and an AD-20 error class.
- **`restricted_writes: true` and `auth_required: true` in NIP-11 are hardcoded literals**
  (`crates/buzz-relay/src/nip11.rs:104-113`), not derived from config, and carry no information
  about the allowlist. They do **not** contradict research §6.6's finding that
  `BUZZ_PUBKEY_ALLOWLIST` defaults to `false` (`config.rs:479-481`, with a test pinning the
  default at `:949-951`). Worth a note so the live NIP-11 is not mistaken for a refutation.

---

## Doc drift — the spine no longer matches its own upstream-issues log

Not new substrate claims, but the same failure mode one layer up. `ARCHITECTURE-SPINE.md` is
`status: final` and is the document an implementer reads.

- **AD-15 still says 65,536.** *"The threshold is read from the pinned substrate's actual limit
  (65,536 bytes at `v0.4.26` `[BUZZ]`)"* — retracted by UP-14, refined by UP-17, and wrong again
  per F-4 (the frame default is 512 KB; the content ceiling is 262,144).
- **AD-15 still mandates content-addressed reference**, which UP-16 declared impossible and F-2
  now restores. Two retractions in opposite directions, neither reflected.
- **`waggle-gate` does not exist.** AD-11 and FR-19 require a `waggle-gate` crate with a
  `GateBackend` port, two adapters, and a structural test asserting *"exactly one crate may call
  the substrate's approval mechanism."* The workspace (`Cargo.toml` members) is `waggle-core`,
  `waggle-method`, `waggle-hive`, `waggle-emit`, `waggle-cli`; gate logic lives in
  `waggle-core/src/gate.rs`. The non-optional structural test has no boundary to enforce.
- **`deploy/compose/` does not exist** in waggle, though the spine's source tree and FR-8
  require it. F-1 changes what belongs in it.
- **The Consistency Conventions tag rule is unqueryable as written** (F-9); the implementation
  already deviates from it correctly.

---

## Claims checked and found SOUND

Listed so this is an audit rather than a complaint. Each was verified against source or the
live relay, not against `vendor/buzz` markdown.

| # | Claim | Where asserted | Verification |
|---|---|---|---|
| S-1 | Approval-step runs are marked `Failed`, not `WaitingApproval` | UP-01 (`observed`), AD-10, AD-11, FR-23, R-1 | **Now confirmed in source.** `buzz-workflow/src/lib.rs:191-215`: *"Approval gates are not yet implemented (WF-08). Fail explicitly rather than creating unreachable WaitingApproval rows."* `RunStatus::WaitingApproval` exists (`buzz-db/src/workflow.rs:84`) and the relay has resume/deny paths (`command_executor.rs:1197,1253`), but nothing ever writes that status. Upgrade `observed` → `confirmed` |
| S-2 | `send_dm` and `set_channel_topic` return `NotImplemented` | UP-02, research §1.4 | `buzz-workflow/src/executor.rs:580-589` — both return `WorkflowError::NotImplemented`. Sound |
| S-3 | Ephemeral kinds 20000–29999 are never stored | AD-9, research §2.3 | `kind.rs:697` `is_ephemeral`; short-circuits at `buzz-db/src/event.rs:268`, `:1058`, and `buzz-relay/src/handlers/event.rs:698`. Sound |
| S-4 | Gate authorization is readable from relay-signed kind `39001`, and owner is distinguishable from admin | AD-13, resolves OQ-5 | `side_effects.rs:954-1050` — *"Emit NIP-29 group discovery events (39000, 39001, 39002) signed by the relay keypair"*; 39001 carries `["p", pubkey, role]` filtered to `role == "owner" \|\| role == "admin"`. **AD-13's owner-only-for-WAIVED rule is expressible.** Caveat: channel-scoped, so it arrives via historical REQ, not live global subscription |
| S-5 | NIP-34 kinds `1617` and `1630`–`1633` are registered and usable | AD-8, addendum OQ-4, FR-18 | `kind.rs:549-563` — all five present in `ALL_KINDS`. Sound (but see F-7 on their limits) |
| S-6 | `40002`/`40003` are Buzz-proprietary and unportable | UP-06, AD-8, NFR-6 | `kind.rs:421-423` `KIND_STREAM_MESSAGE_V2` / `_EDIT`. Sound — though the forbidden list is incomplete (F-11) |
| S-7 | Workflow engine shape: 5 triggers, 7 actions, `evalexpr`, 100 ms timeout, step ids reject dashes | research §7.1–7.2 | `schema.rs:35-115` — triggers and actions exactly as listed; `executor.rs:342` `EVAL_TIMEOUT = 100ms`. Sound and unusually well-verified |
| S-8 | Workflow run concurrency is semaphore-bounded | research §1.3 | `buzz-workflow/src/lib.rs:111` `Semaphore::new(permits)` — bounded, though `permits` is configurable rather than the flat "100" stated |
| S-9 | Presence TTL is 90 s | research §1.2 | `buzz-pubsub/src/presence.rs:16` `PRESENCE_TTL_SECS: u64 = 90`. Sound |
| S-10 | Relay queries must include explicit `kinds` or hit the p-gate with 403 | research §6.6 | `req.rs:184-187` — `"restricted: p-gated events require #p matching your pubkey"`; `p_gated_filters_authorized` at `:1038-1045` treats a kindless filter as unable to exclude p-gated kinds. Sound |
| S-11 | The pubkey allowlist is off by default | research §6.6 | `crates/buzz-relay/src/config.rs:479-481` → `unwrap_or(false)`, pinned by test at `:949-951`; enforced only at `handlers/auth.rs:186-213` when enabled. `BUZZ_REQUIRE_RELAY_MEMBERSHIP` and `BUZZ_REQUIRE_AUTH_TOKEN` also default false. Sound |
| S-12 | Substrate build needs no live Postgres (UP-05 impact is Low) | UP-05 | Zero compile-time `sqlx::query!` macros; sqlx `macros` feature off. Sound |
| S-13 | Rust toolchain pin 1.95.0 matches the substrate's | Stack table, `BUZZ_VERSION`, `rust-toolchain.toml` | Both files pin `1.95.0`. Sound; `BUZZ_VERSION`'s note that 1.88.0 is MSRV-not-pin is correct |
| S-14 | `nostr` 0.44.x matches the substrate's | Stack table | waggle `Cargo.toml:29` `nostr = "0.44.6"`; buzz `Cargo.toml:61` `nostr = { version = "0.44", … }`. Same release line. Sound |
| S-15 | Kind `30176` team events exist and carry `persona_ids` | research §8.5 | `kind.rs:250` `KIND_TEAM: u32 = 30176`. Sound |
| S-16 | Persona frontmatter accepts an undocumented `runtime` field | UP-09, persona-pack-contract §3 | Not re-derived; the original finding quotes the parser's own `deny_unknown_fields` rejection message, which is authoritative. Accepted |
| S-17 | The persona pack spec exists and is complete | UP-04 (withdrawn), persona-pack-contract | `crates/buzz-persona/PERSONA_PACK_SPEC.md` present; the contract doc records a live `buzz pack validate` round-trip plus three negative tests. Well-evidenced |
| S-18 | Channel creation from a template is not idempotent | UP-10 | Reproduced live by the original author against a running relay; not re-run here. Accepted |
| S-19 | `POST /query` requires an array of filters | UP-12 | Reproduced live; error string quoted. Accepted |
| S-20 | `buzz-cli` strips signatures from all reads | UP-07 | Reproduced live; field list quoted. Accepted — and F-2 shows the *general* lesson was still under-applied |
| S-21 | `buzz-cli` exit codes are `0/1/2/3/4/5` | research §6.6, AD-20 precedent | Stated verbatim in `vendor/buzz/AGENTS.md` and consistent with `crates/buzz-cli` error types. Accepted |
| S-22 | Relay serves NIP-11 at `localhost:3100` | dev setup | `GET /` → 200, `"software":"https://github.com/block/buzz"`, `"version":"0.2.0"`. Sound — and that version string is itself F-14's evidence |

---

## Recommended actions, in order

1. **Withdraw UP-08; delete AD-17.** Pull `ghcr.io/block/buzz` by digest. Re-pin
   `BUZZ_SUPPORTED` against **relay** versions read from NIP-11, not a desktop git tag. (F-1, F-14)
2. **Withdraw UP-16; reopen FR-16 / OQ-2.** `PUT /upload` with Blossom auth accepts markdown up
   to 100 MB, content-addressed. Record the reason plainly: *the error came from `buzz-cli`'s
   client-side allowlist, not the relay.* (F-2)
3. **Redesign the gate record path.** waggle publishes it under an agent identity via
   `POST /events` with real tags; the substrate workflow becomes a notification at most. Never
   trust `{{trigger.author}}`. Re-derive AD-10/AD-11 from that. (F-3, F-5, F-8)
4. **Replace AD-15's NIP-11 derivation with the pinned constant 262,144**, plus a tripwire test.
   Correct UP-14/UP-15/UP-17's numbers. Re-measure kind 1617 before trusting F-7's implication.
   (F-4, F-7)
5. **Design for `429`.** Add rate-limit handling and an exit-code class to AD-20; rewrite UP-03
   and re-motivate NFR-8/R-9 as "we will be throttled", not "nothing throttles us". (F-6)
6. **Fix the tag convention in the spine** to match what the code already does, and restate
   FR-24 in terms of `#t` with explicit paging. Add a test that no `#waggle-*` filter is ever
   constructed. (F-9)
7. **Pin the kind registry.** Snapshot `buzz-core::kind::ALL_KINDS` into the committed kind
   registry AD-8 already requires; make the "is this kind free?" check mechanical. (F-11)
8. **Re-test headless agent-record creation** before deferring Story 1.7 work again. (F-12)
9. **Reconcile the spine with `upstream-issues.md`**, and create `waggle-gate` and
   `deploy/compose/` or amend the architecture to match reality.
10. **Adopt a standing rule:** *"A capability is not absent until the **relay** has refused it
    over HTTP. `buzz-cli` refusing it, or a doc omitting it, proves nothing."* UP-07 and UP-13
    taught this; UP-16 was filed afterwards and still got it wrong, twice.
</content>
