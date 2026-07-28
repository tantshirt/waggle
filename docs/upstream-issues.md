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
