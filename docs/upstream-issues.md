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

## UP-04 — Persona pack format and agent keypair provisioning are undocumented

- **Status:** `observed`
- **Source:** Buzz `VISION_AGENT.md` covers ACP, session limits, and community-scoped
  identity, but not keypair generation, persona pack schema, MCP config schema, or the
  channel-join procedure.
- **Impact on waggle:** **Critical.** This is the input contract for deliverable 1 and
  the whole of deliverable 3.
- **Our mitigation:** read `crates/buzz-persona` and `crates/buzz-cli` directly in
  Story 1.1. Tracked as research open question **O-1**.
- **Candidate contribution:** a documentation PR specifying the persona pack schema
  would be a genuinely useful first upstream contribution.

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
