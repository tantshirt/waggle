# Upstream issues

**Guardrail:** we never modify files inside the self-hosted Buzz checkout. It is an
external service. Anything Buzz needs to change is logged here as a candidate upstream
PR or issue against https://github.com/block/buzz.

Pinned at Buzz `v0.4.26`. Status values: `observed` (read in upstream docs, not yet
reproduced locally) · `confirmed` (reproduced here) · `filed` (issue/PR open upstream)
· `fixed` (landed upstream; bump `BUZZ_VERSION` and remove our workaround).

---

## UP-01 — Approval gate runs are marked `Failed` instead of `WaitingApproval`

- **Status:** `observed`
- **Source:** Buzz `ARCHITECTURE.md`, tracked upstream as `WF-08`
- **Impact on waggle:** **Critical.** Every quality gate is an approval gate. Without
  durable suspension, a workflow that reaches a gate is indistinguishable from a
  failed run, so gate state cannot be trusted as the source of truth.
- **Our mitigation:** keep the gate layer behind a thin interface
  (`GateBackend`-style) so the workaround lives in exactly one place. Until this
  lands, treat gate state as waggle-owned and reconcile against Buzz, rather than
  reading it out of Buzz run status.
- **Action:** confirm against `v0.4.26` source during Story 1.1, then file upstream if
  still present.

## UP-02 — `send_dm` and `set_channel_topic` workflow actions return `NotImplemented`

- **Status:** `observed`
- **Source:** Buzz `ARCHITECTURE.md`
- **Impact on waggle:** Medium. Rules out DM-based agent handoff notifications and
  auto-updating a story channel's topic with current phase/gate state.
- **Our mitigation:** use channel messages (kind `9`) for handoffs instead of DMs;
  carry phase/gate state in the artifact event's tags rather than the channel topic.

## UP-03 — Rate limiter is a trait with only a test stub

- **Status:** `observed`
- **Source:** Buzz `ARCHITECTURE.md` ("currently unstub")
- **Impact on waggle:** Medium, and it grows with us. A hive runs many agent sessions
  against one relay; a runaway agent loop has nothing throttling it.
- **Our mitigation:** cap concurrency on our side in the agent runtime config.

## UP-04 — ~~Persona pack format and agent keypair provisioning are undocumented~~

- **Status:** `withdrawn` — **this issue was wrong.** Resolved 2026-07-28 during Story 1.1.
- **What we got wrong:** we concluded the persona pack schema was undocumented because
  `README.md` and `VISION_AGENT.md` do not describe it. They do not — but
  **`crates/buzz-persona/PERSONA_PACK_SPEC.md` is a complete, 16-section specification**
  that we had not read. Our research pass covered the repo-root docs and skimmed crate
  names; it did not open in-crate documentation.
- **What is actually specified:** pack layout, `.plugin/plugin.json` manifest (an Open
  Plugin Spec superset), `.persona.md` frontmatter with a full field reference, the
  five-level config precedence model, merge semantics, skills, MCP config and its merge
  rules, lifecycle hooks, distribution and integrity, and a planned-features list.
- **Keypair provisioning is also solved:** `buzz-admin generate-key` and
  `buzz-admin add-member` exist and are documented in the crate's CLI help.
- **Lesson for later research:** read in-crate docs, not only repo-root docs. Applied to
  `docs/research-notes.md` §5.
- **No upstream contribution needed.** A docs cross-link from `VISION_AGENT.md` to
  `PERSONA_PACK_SPEC.md` would still help discoverability — a small, friendly first PR.

## UP-05 — No sqlx offline query cache

- **Status:** `observed`
- **Source:** Buzz `ARCHITECTURE.md`
- **Impact on waggle:** Low. Affects building Buzz from source in CI without a live
  Postgres. Prefer the published release artifact / container image.

## UP-06 — Kinds `40002` / `40003` are Buzz-only

- **Status:** `observed`
- **Source:** Buzz `NOSTR.md` — "works on the wire but Buzz-only", no standard NIP-29
  client support.
- **Impact on waggle:** Low, but a portability trap. Our artifacts should stay on
  standard kinds so a NIP-34 client (gitworkshop.dev, ngit-cli) can still read the log.
- **Our mitigation:** avoid `40002`/`40003` in emitted workflows; note it as a
  compiler lint.

## UP-07 — `buzz-cli` strips signatures from every read, in every format

- **Status:** `confirmed` — reproduced locally against `v0.4.26`, 2026-07-28.
- **Source:** `AGENTS.md` states "All reads return sig-stripped JSON arrays." Confirmed:
  `buzz --format json messages get` returns exactly
  `['content','created_at','id','kind','pubkey','tags']`. There is no `sig`, and no flag
  to include one.
- **Impact on waggle:** **Critical, and it changes the architecture.** FR-22 requires a gate
  record to be independently verifiable from the log alone. A consumer cannot verify a
  Schnorr signature it is never given. Anything in waggle that must *prove* provenance —
  rather than trust the relay — cannot go through `buzz-cli`.
- **What still works without it:** the event id recomputes from
  `sha256([0,pubkey,created_at,kind,tags,content])` per NIP-01, which proves the relay
  stored exactly the submitted bytes bound to the claimed pubkey. The relay verifies
  signatures at ingest (event pipeline stage 5, `ARCHITECTURE.md`). That is the relay
  vouching, not independent verification.
- **Our mitigation:** `waggle-hive` speaks the relay protocol directly — WebSocket `REQ`
  with NIP-42 auth, or `POST /query` with NIP-98 auth — for any path requiring signature
  verification. `buzz-cli` remains fine for provisioning and publishing.
  `POST /query` unauthenticated returns `401 {"error":"missing Nostr auth"}`.
- **Architecture consequence:** AD-20's "machine-first command surface" must not be read as
  "waggle shells out to `buzz-cli`". Recorded against FR-22 and FR-15.
- **Candidate contribution:** an opt-in `--include-sig` flag on read commands would make
  `buzz-cli` sufficient for verifiable audit consumers. Worth proposing upstream.

## UP-08 — Buzz publishes no relay container image

- **Status:** `confirmed` — checked GitHub releases and `orgs/block/packages`, 2026-07-28.
- **Detail:** release assets for `v0.4.26` are desktop binaries only — `.dmg`, `.deb`,
  `.AppImage`, `.exe`. No relay image is published to GHCR or any public registry. The
  repo does ship a working `Dockerfile` and `docker-compose.yml`; per `AGENTS.md` the relay
  image is built by an internal Block pipeline (`squareup/sprout-oss`) and pushed to a
  private ECR.
- **Impact on waggle:** medium, and already designed around. AD-17 has waggle CI build the
  image from upstream's unmodified `Dockerfile` at the pinned tag.
- **Candidate contribution:** publishing an official relay image to GHCR would remove that
  burden from every self-hoster, not just us. Probably the highest-value upstream ask on
  this list.

## UP-09 — `runtime` persona field is accepted but undocumented

- **Status:** `confirmed` — observed 2026-07-28 against `v0.4.26`.
- **Detail:** the persona frontmatter parser uses `deny_unknown_fields`, and its rejection
  message enumerates the accepted set:
  `name, display_name, avatar, description, version, author, skills, mcp_servers, subscribe,
  respond_to, triggers, model, runtime, temperature, max_context_tokens, thread_replies,
  broadcast_replies, hooks`.
  **`runtime` is in that list but appears nowhere in `PERSONA_PACK_SPEC.md` §4's field
  reference table.** Its type, valid values, default, and precedence behavior are all
  unspecified.
- **Impact on waggle:** low today — waggle does not emit `runtime`. But it is a field that
  presumably selects the agent runtime, which would matter a great deal for FR-13. We cannot
  use it without guessing.
- **Our mitigation:** do not emit `runtime`. Recorded in `docs/persona-pack-contract.md` §3.
- **Candidate contribution:** document `runtime` in the spec's §4 table, or remove it from
  the accepted set if it is vestigial. Small, concrete, and easy for a maintainer to confirm.

## UP-10 — Channel creation from a template is not idempotent

- **Status:** `confirmed` — reproduced against `v0.4.26`, 2026-07-29.
- **Detail:** `buzz channels create --name X --template T` run twice produces **two
  channels named `X`**, each with its own UUID and its own canvas. There is no
  name-uniqueness check and no "already exists" response.
- **Impact on waggle:** medium. FR-25 requires re-running provisioning to produce no
  duplicates and NFR-2 requires idempotence generally, so waggle must query for an
  existing channel before creating one.
- **Our mitigation:** check-then-create in `waggle-hive`. Note this is inherently racy
  against concurrent creators; acceptable for a provisioning command, and the honest
  alternative would need relay-side support.
- **Candidate contribution:** an `--if-not-exists` flag, or returning the existing
  channel when the name matches within a community. Small and self-contained.

## UP-11 — Relay info document is missing the `self` field, so archive filtering is untrusted

- **Status:** `confirmed` — observed 2026-07-29 on a local `v0.4.26` relay.
- **Detail:** template-based channel creation emits
  `archived-identities snapshot untrusted, proceeding without archive filtering: relay
  info document missing 'self' field`. The relay's NIP-11 document does not advertise
  `self`, so the CLI cannot verify the kind `13535` archive snapshot is relay-signed and
  proceeds without filtering archived identities.
- **Impact on waggle:** low today — we add no archived agents. It would matter once
  agent identities are rotated or retired, since an archived agent could silently be
  re-added to a channel.
- **Our mitigation:** none needed yet. Recorded so the warning is not mistaken for noise.
- **Candidate contribution:** populate `self` in the relay's NIP-11 document, or
  document why it is absent in local dev.

## UP-12 — `POST /query` requires an array of filters, undocumented

- **Status:** `confirmed` — reproduced against `v0.4.26`, 2026-07-29.
- **Detail:** posting a bare Nostr filter object to `/query` returns
  `400 {"error":"invalid filters: invalid type: map, expected a sequence"}`. The endpoint
  wants a JSON **array** of filters. `buzz-cli` wraps it internally, so the requirement is
  invisible to anyone reading the CLI or the endpoint description ("Nostr REQ filters over
  HTTP") rather than the deserializer.
- **Impact on waggle:** none now that it is known — one line. Recorded because it cost a
  round trip and would cost the next implementer the same.
- **Candidate contribution:** state the array requirement in the HTTP surface
  documentation, or accept a bare object for convenience.

## UP-13 — `buzz-cli` cannot attach arbitrary tags when sending a message

- **Status:** `confirmed` — `messages send` exposes `--kind`, `--reply-to`, `--broadcast`,
  and `--file`, but no way to set a tag.
- **Impact on waggle:** **high, and it compounds UP-07.** Nostr only indexes single-letter
  tags for `#<letter>` filter queries, so a queryable trail needs typed tags at publish
  time. Without them FR-24 ("filter the log by priority") would degrade to fetching
  everything and filtering client-side.
- **Our mitigation:** `waggle-hive` publishes directly to `POST /events` with NIP-98 auth
  and full control of the tag set. Together with UP-07 this is why waggle speaks the relay
  protocol rather than shelling out for anything on the signed-trail path.
- **Candidate contribution:** a repeatable `--tag name=value` flag on `messages send`
  would make `buzz-cli` sufficient for structured publishing.

## UP-14 — The 64 KB frame limit is not an event size limit

- **Status:** `confirmed` — measured against `v0.4.26`, 2026-07-29.
- **What we had wrong:** `ARCHITECTURE.md` documents a 65,536-byte frame limit, and waggle's
  own research notes, PRD (FR-16), and architecture (AD-15, OQ-2) all treated it as a
  ceiling on artifact size. **It is the WebSocket frame limit.** Publishing over HTTP
  `POST /events` is unaffected: a 200,000-byte event was accepted without complaint.
- **The real ceiling is content length, at 262,144 bytes**, enforced with
  `invalid: content exceeds maximum size of 262144`.
- **Impact on waggle:** it *relaxes* a constraint. Artifacts have 4× the assumed headroom,
  so almost every method artifact fits inline and the reference-carrying machinery FR-16
  described is needed far less often than the PRD assumed.
- **Recorded because** the mistake was ours: a documented number was applied to a transport
  it did not govern, and nobody would have caught it without measuring.

## UP-15 — NIP-11 advertises a message limit that disagrees with the enforced content limit

- **Status:** `confirmed` — `v0.4.26`, 2026-07-29.
- **Detail:** the relay's NIP-11 document reports `limitation.max_message_length: 524288`,
  but content over `262144` is rejected. The enforced content ceiling is advertised
  nowhere, so a client can only discover it by being refused.
- **Impact on waggle:** low but awkward. AD-15 requires the threshold be *derived* from the
  substrate rather than hard-coded; the only derivable number is the wrong one. waggle
  reads `max_message_length` and halves it, which matches the observed value — a
  coincidence that should not be relied on indefinitely.
- **Candidate contribution:** advertise the content limit in NIP-11, or reconcile the two.

## UP-16 — Blossom media accepts image MIME types only

- **Status:** `confirmed` — `buzz upload file --file x.md` returns
  `unsupported file type: application/octet-stream`. `ALLOWED_MIME_TYPES` in
  `buzz-media/src/validation.rs` is `["image/jpeg", "image/png", "image/gif", "image/webp"]`.
- **Impact on waggle:** **this blocks FR-16's chosen mechanism.** AD-15 resolved OQ-2 in
  favour of content-addressed reference for oversized artifacts, but the substrate's media
  store cannot hold a markdown document, so there is nothing to reference.
- **Our mitigation:** refuse oversized artifacts with a specific error naming the size, the
  limit, and why reference-carrying is unavailable. FR-16 forbids truncating or silently
  dropping; it does not require accepting the unstorable. Mitigated further by UP-14 — the
  real limit is 4× what we planned for, so this is rare.
- **Candidate contribution:** allow `text/markdown` and `text/plain` in the Blossom
  allowlist. Small change, and it would unblock document-carrying for every consumer, not
  just us.

## UP-17 — Content size limits differ per event kind, and none are advertised

- **Status:** `confirmed` — measured on `v0.4.26`, 2026-07-29.
- **Detail:** three different ceilings, discoverable only by hitting them:

  | Path | Limit | How it surfaces |
  |---|---|---|
  | WebSocket frame | 65,536 | documented in `ARCHITECTURE.md` |
  | kind:9 content (HTTP publish) | 262,144 | `content exceeds maximum size of 262144` |
  | kind:1617 patch content | 61,440 | `content exceeds maximum size of 61440 bytes (got 83562)` |

  NIP-11 advertises only `max_message_length: 524288`, which matches none of them.
- **Impact on waggle:** medium. A patch limit four times smaller than the message limit is
  genuinely surprising, and a real `git format-patch` of a documentation-heavy commit
  exceeds it easily — the first patch we tried was 83 KB and was rejected.
- **Our mitigation:** size-check before publishing and surface the relay's own numbers.
  Test fixtures deliberately select a small commit rather than assuming any commit fits.
- **Candidate contribution:** advertise per-kind limits in NIP-11, or at least document
  them. A client cannot currently know whether a patch will be accepted without trying.
