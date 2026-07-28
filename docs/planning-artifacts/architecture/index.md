# Architecture — waggle

The architecture is an **architecture spine**: invariants and rules only, with structural
detail deliberately kept minimal (the code owns it once it exists).

| Document | Contents |
|---|---|
| [`spine-waggle-2026-07-28/ARCHITECTURE-SPINE.md`](spine-waggle-2026-07-28/ARCHITECTURE-SPINE.md) | Design paradigm, AD-1 … AD-20, consistency conventions, stack, structural seed, capability→architecture map, deferred |
| [`spine-waggle-2026-07-28/.memlog.md`](spine-waggle-2026-07-28/.memlog.md) | Rationale for every decision. The spine records *what*; the memlog records *why*. |

**Status:** final, 2026-07-28. Paradigm: hexagonal (ports and adapters). Altitude: initiative.

Binds `FR-1`–`FR-28` and `NFR-1`–`NFR-10` from
[`../prds/prd-waggle-2026-07-28/prd.md`](../prds/prd-waggle-2026-07-28/prd.md).
