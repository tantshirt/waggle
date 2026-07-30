# Hive bring-up — pinned upstream image

Story 1.3 / FR-8. Operators need only Docker Compose — not the substrate's Rust
toolchain.

**We pull. We do not build.** Upstream publishes `ghcr.io/block/buzz` publicly
(UP-08 withdrawn). AD-17's CI image-build pipeline is therefore redundant; this
bundle pins an immutable digest instead.

## Quick start

**Production (closed auth, loopback bind — default):**

```bash
cd deploy/compose
cp .env.example .env
# Replace every CHANGE_ME — generate with: openssl rand -hex 32
./run.sh start
```

**Local open hive (dev overlay):**

```bash
cd deploy/compose
cp .env.dev.example .env
# Replace every CHANGE_ME
BUZZ_COMPOSE_DEV=true ./run.sh start
```

`.env.example` / `compose.yml` require `BUZZ_REQUIRE_AUTH_TOKEN` and
`BUZZ_REQUIRE_RELAY_MEMBERSHIP` (default `true`). The relay host port binds to
`127.0.0.1` unless you set `BUZZ_HTTP_HOST` deliberately. `compose.dev.yml`
overrides auth open for local bring-up only.

Wait for healthy services, then:

```bash
./run.sh status
curl -fsS "http://127.0.0.1:$(grep -E '^BUZZ_HTTP_PORT=' .env | cut -d= -f2-)/_liveness"
```

If a service is unhealthy, `./run.sh status` names it. Inspect with:

```bash
./run.sh logs <service>     # relay | postgres | redis | minio
docker compose --env-file .env -f compose.yml ps
```

## Image pin

The default `BUZZ_IMAGE` in `.env.example` and `compose.yml` is a content digest,
not a floating tag:

```
ghcr.io/block/buzz@sha256:6cf2db58ee1607a99bd7651277c62d1cc41e05f96483b37bba9a4ecabf4cf6cb
```

Resolved from `:main` on 2026-07-29. Recorded also in the repo-root `BUZZ_VERSION`.

**Desktop vs relay versioning.** Upstream versions the relay image independently
of desktop releases (`relay-v*` tags and `:main` / `:sha-<7>`). Desktop tag
`v0.4.26` does **not** publish a matching image. Local Hermit development still
builds the relay from the `v0.4.26` source checkout; operators use this digest.

## Integrity

The substrate checkout under `vendor/buzz/` (Hermit path) remains immutable
(AD-2). `waggle preflight` asserts it is byte-unchanged. This compose bundle
never writes into that checkout.

## Source

Compose service definitions are adapted from upstream's
`deploy/compose/` at the pinned desktop tag, with the image reference changed
from a floating `:main` default to the digest above.
