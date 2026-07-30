# Dev setup — clean machine to first signed message

Story 1.1's walking skeleton. Verified end to end on macOS 15 (Darwin 25.4.0),
2026-07-28, against Buzz `v0.4.26`.

**What you get:** a stock Buzz relay running locally, one agent Nostr keypair, and one
signed `kind:9` message in a channel whose event id and author you can verify yourself.

**Time:** about 25 minutes, most of it the first Rust build.

---

## Prerequisites

Only two, because Buzz's own environment tooling (Hermit) supplies the rest.

| Need | Why |
|---|---|
| **Docker Desktop**, running | Postgres, Redis, MinIO, Prometheus |
| **git** | Cloning the substrate |

**You do not need Rust, Node, pnpm, or `just` installed.** Hermit provides them, pinned.
This machine had `rustc 1.79.0` — far below Buzz's requirement — and it did not matter.

> **OQ-6 resolved.** Hermit supplies cargo **1.95.0** (matching Buzz's
> `rust-toolchain.toml`), Node **24.14.0**, pnpm **11.4.0**, and `just` **1.46.0**.
> Contributors do **not** need to upgrade their system toolchain. Note that Buzz's README
> says "Rust 1.88+" but `rust-toolchain.toml` actually pins **1.95.0** — the pin is
> authoritative.

## 1. Clone the substrate

```bash
mkdir -p vendor
git clone --depth 1 --branch v0.4.26 https://github.com/block/buzz.git vendor/buzz
```

`vendor/` is gitignored. **The checkout is an external service — never edit a tracked file
in it** (AD-2). See "Substrate integrity" below for what is and is not allowed.

## 2. Start Docker

Docker Desktop must be *running*, not merely installed. `just setup` fails fast with
`Docker daemon is not running` otherwise.

```bash
open -a Docker          # macOS
docker info             # should succeed before continuing
```

> **Docker Desktop's Compose tab may say "The Compose app is no longer running" while the
> stack is perfectly healthy.** That panel is unreliable. Trust the CLI:
> ```bash
> cd vendor/buzz && docker compose ps
> ```

## 3. Run setup

```bash
cd vendor/buzz
cp -n .env.example .env
. ./bin/activate-hermit    # or prefix commands with ./bin/
just setup
```

This starts the container stack, applies migrations, and installs desktop/web deps.
Expect roughly:

```
postgres     Up (healthy)
redis        Up (healthy)
minio        Up (healthy)
adminer      Up
prometheus   Up
keycloak     Up (unhealthy)   ← expected, see below
```

> **`keycloak` reports unhealthy and that is fine.** It exists for local OAuth testing.
> The relay does not depend on it, and the walking skeleton does not touch it.

## 4. Pick a free port

**Buzz defaults to port 3000, which collides with almost every JS dev server.** On this
machine a `next-server` from an unrelated project already held it, and the relay died with:

```
Error: Failed to bind 0.0.0.0:3000: Address already in use (os error 48)
```

Check first, and move Buzz rather than killing the other process:

```bash
lsof -nP -iTCP:3000 -sTCP:LISTEN     # anything listed means pick another port
```

If occupied, edit `vendor/buzz/.env`:

```ini
BUZZ_BIND_ADDR=0.0.0.0:3100
RELAY_URL=ws://localhost:3100
```

The relay re-seeds its local dev community hosts on next start, so `localhost:3100`
becomes a valid community host automatically.

## 5. Start the relay

```bash
just relay
```

First run compiles the workspace — several minutes. Ready when you see:

```json
{"level":"INFO","message":"buzz-relay TCP listening","addr":"0.0.0.0:3100"}
```

Verify:

```bash
lsof -nP -iTCP:3100 -sTCP:LISTEN
```

## 6. Build the CLI

In a second terminal:

```bash
cd vendor/buzz
. ./bin/activate-hermit
cargo build --release -p buzz-cli     # ~3 minutes
```

Binary lands at `vendor/buzz/target/release/buzz`.

## 7. Create an agent identity

```bash
cd <repo-root>
mkdir -p keys
cd vendor/buzz
cargo run --release -q -p buzz-admin -- generate-key > ../../keys/tea-agent.key
```

`buzz-admin generate-key` prints `Public key:` and `Secret key:`.

> **`keys/` is gitignored (NFR-7).** Never commit, print, or log the secret key. Extract
> only the public half for anything you intend to share:
> ```bash
> grep 'Public key' keys/tea-agent.key | awk '{print $3}' > keys/tea-agent.pub
> ```

Confirm the ignore rule is actually in effect before going further:

```bash
git check-ignore -v keys/tea-agent.key    # must print a matching rule
```

## 8. Post a signed message

```bash
export BUZZ_PRIVATE_KEY=$(grep 'Secret key' keys/tea-agent.key | awk '{print $3}')
export BUZZ_RELAY_URL=http://localhost:3100

cd vendor/buzz
./target/release/buzz channels create \
  --name waggle-walking-skeleton --type stream --visibility open

./target/release/buzz messages send \
  --channel <channel_id> --content "first signed message from an agent identity."
```

Both return JSON:

```json
{"accepted":true,"channel_id":"2699ad63-…","event_id":"a3bcdbcb…"}
{"accepted":true,"event_id":"968ac19a…"}
```

No relay-member registration was needed — the relay's pubkey allowlist is optional and is
not enabled by default, so NIP-42 authentication accepts a fresh keypair.

## 9. Verify it

```bash
./target/release/buzz --format json messages get --channel <channel_id>
```

```json
[{ "id": "968ac19a…", "kind": 9, "pubkey": "775208d3…",
   "tags": [["h", "2699ad63-…"]],
   "content": "first signed message from an agent identity." }]
```

Kind `9` with an `h` tag carrying the channel UUID, exactly as `NOSTR.md` documents.

Recompute the event id yourself — NIP-01 defines it as
`sha256([0, pubkey, created_at, kind, tags, content])`:

```bash
./target/release/buzz --format json messages get --channel <channel_id> | python3 -c "
import sys, json, hashlib
ev = json.load(sys.stdin)[0]
ser = json.dumps([0, ev['pubkey'], ev['created_at'], ev['kind'], ev['tags'], ev['content']],
                 separators=(',', ':'), ensure_ascii=False)
print('MATCH:', hashlib.sha256(ser.encode()).hexdigest() == ev['id'])
print('AUTHOR:', ev['pubkey'])
"
```

Expect `MATCH: True` and an author equal to your `keys/tea-agent.pub`.

### ⚠️ What this does and does not prove

`buzz-cli` **strips signatures from every read, in both `json` and `compact` formats.**
There is no flag to include them.

So the check above proves:

- the relay stored exactly the bytes we sent (id recomputes), and
- the event is bound to our agent's pubkey.

It does **not** directly verify the Schnorr signature. The relay verifies signatures at
ingest — stage 5 of its event pipeline, per `ARCHITECTURE.md` — so an event that is stored
was signature-checked. But that is the relay vouching, not us.

> **This matters for waggle's architecture, not just this document.** FR-22 requires a gate
> record to be independently verifiable from the log alone. That is **not achievable through
> `buzz-cli`.** Getting raw signed events requires either a WebSocket `REQ` with NIP-42 auth
> or `POST /query` with NIP-98 auth — `POST /query` without auth returns
> `401 {"error":"missing Nostr auth"}`. `waggle-hive` must therefore speak the relay
> protocol directly rather than shelling out to `buzz-cli`. Logged as **UP-07**.

## Substrate integrity (AD-2)

AD-2 forbids modifying the substrate. Step 4 edits `vendor/buzz/.env`, which looks like a
violation but is not: `.env` is listed in **Buzz's own `.gitignore`** (line 10). It is local
runtime config the substrate's setup script generates, not source.

The invariant is **no tracked file is modified**. Verify at any time:

```bash
cd vendor/buzz && git status --porcelain     # must be empty
```

Empty output means AD-2 holds. This is the check CI enforces.

## Worked example from the verified run

| Item | Value |
|---|---|
| Buzz tag | `v0.4.26` |
| Relay bind | `0.0.0.0:3100` |
| Community | `f7f426e6-6e87-4d3c-a4cb-6083d16471af` |
| Channel | `2699ad63-f3ee-41e3-bef9-99ad6f34ac46` |
| Agent pubkey | `775208d3a8f1bfd838a2af86a5a43505785e4874c6a80ccb36254b14eeefed2f` |
| Message event | `968ac19a78d0bf3b3063a71643955572fbbd5a20754bb62defaaf6dc61f2566b` |
| Result | id recomputed ✅ · author matched ✅ |

The keypair above is a throwaway used only against a local relay.

## Full hive (Desktop + lazy ACP)

After the relay is up and `cargo build -p waggle-cli` has succeeded:

```bash
# From the waggle repo root — install/compile all modules, provision phase rooms
./target/debug/waggle sync \
  --relay http://localhost:3100 \
  --buzz-cli ./vendor/buzz/target/debug/buzz

# Terminal A — lazy ACP supervisor (wake agents on @mention)
./target/debug/waggle runtime supervisor \
  --relay ws://localhost:3100 \
  --agent-owner <your-desktop-pubkey-hex>

# Terminal B — Buzz Desktop pointed at Local Dev (same relay)
```

Do **not** keep always-on `buzz-acp` processes for every agent; the supervisor spawns on
mention and relies on `BUZZ_ACP_IDLE_TIMEOUT` (default 320s) to exit. Cap is
`--max-concurrent` (default 4).

See the README **Full hive** section for the phase-room map (`#help` → `#gate`).

## Troubleshooting

| Symptom | Cause | Fix |
|---|---|---|
| `Docker daemon is not running` | Docker Desktop not started | `open -a Docker`, wait for `docker info` |
| `Failed to bind 0.0.0.0:3000: Address already in use` | Another dev server on 3000 | Step 4 — move Buzz, don't kill theirs |
| `"The Compose app is no longer running"` in the GUI | Docker Desktop panel bug | Ignore; use `docker compose ps` |
| `keycloak Up (unhealthy)` | Expected | Ignore |
| `401 {"error":"missing Nostr auth"}` on `POST /query` | Endpoint needs NIP-98 | Use `buzz-cli`, or implement NIP-98 |
| `BUZZ_RELAY_PRIVATE_KEY is required` from `add-member` | Relay signing key unset | Not needed — the allowlist is off by default |
| CLI: `unrecognized subcommand 'list'` | It is `messages get`, not `list` | `buzz messages get --channel …` |
| Relay query returns 403 | Filter omitted `kinds` | Always pass explicit `kinds` |

## Teardown

```bash
cd vendor/buzz
docker compose down          # stop, keep data
./scripts/dev-reset.sh       # wipe and start fresh
```
