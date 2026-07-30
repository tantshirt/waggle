# Waggle architecture (authoritative)

**Canonical specification:** [`docs/planning-artifacts/architecture/spine-waggle-2026-07-28/ARCHITECTURE-SPINE.md`](docs/planning-artifacts/architecture/spine-waggle-2026-07-28/ARCHITECTURE-SPINE.md)

That spine is the single authoritative architecture document for build and review.
Historical review notes, rubrics, and unverified-claims audits live under
`docs/planning-artifacts/**/reviews/` and are **not** normative.

## Product shape

Waggle is a **distribution/compiler layer**: BMAD agents, skills, and workflows
become Buzz-native identities, rooms, packs, and signed Nostr artifacts — without
forking Buzz. Buzz owns identity, transport, channels, and audit history; BMAD is
the behavioral source.

## Ownership model (NIP-AP)

| Kind | Author | `d` tag | Role |
|------|--------|---------|------|
| 30175 | hive **owner** | persona slug | Definition (`system_prompt` lives here) |
| 30177 | hive **owner** | agent pubkey | Slim instance (`persona_id` link; no prompt fields) |

Agent role `.nsec` files are for runtime ACP sessions and kind:0 profiles — not for
authoring 30175/30177.

## Release artifacts vs sources

- `_bmad/` + compiler inputs are the **source**.
- `packs/` are **generated release artifacts** (emit may be strict about missing skills).
- `.claude/skills/` may mirror installed skills for local tools; do not treat both
  packs and `.claude` as independent sources of truth.

## Sync state

`_bmad/custom/waggle-sync-state.json` records versions and a `content_hash`. Prefer
hash/version fields for CI diffs — not wall-clock timestamps.

## Reproducibility

Pinned components are declared in `BUZZ_VERSION`. Floating container tags or npm
ranges, if any remain, must be called out explicitly in that file or deploy docs —
do not claim reproducibility for unpinned images.
