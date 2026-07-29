# waggle

**Agentic method modules, compiled to a Buzz hive.**

Waggle is a self-hostable, Nostr-based team workspace where every agent from the
BMAD Method™ agent framework — across all official modules — runs as a first-class
member with its own keypair, and every artifact, handoff, and quality gate is a
signed event in one auditable log.

It is a **distribution**, not a fork. Waggle self-hosts stock
[Buzz](https://github.com/block/buzz) from upstream and adds a thin compilation and
configuration layer on top. The Buzz codebase is treated as an external service and
is never modified — CI asserts the checkout is byte-clean.

> **Not affiliated with BMad Code, LLC.** Waggle is an independent, community
> distribution that is *compatible with* the BMAD Method™. "BMad", "BMad Method",
> and related marks are trademarks of BMad Code, LLC. See [NOTICE](NOTICE).

---

## Status: the pilot works

The core thesis is proven end to end against a real, unmodified Buzz relay.

```
$ waggle compile --module tea --agent bmad-tea
compiled bmad-tea -> packs/tea
  skills       9 copied
  prompt-only  GATE (carried into the persona body, no skill)
  dropped      activation_steps_append — no persona-pack equivalent

$ buzz pack validate packs/tea
Valid.
```

And the gate — the reason the project exists — fires for real. **But see the correction
below: the gate *mechanism* works and its *attribution* does not.**

```
verdict (CONCERNS, P1, with rationale)
  → human reaction ✅
    → compiled workflow fires
      → waggle-gate-record
           verdict-event: ae98ddf4…
           approver:      47e6e1db…
           approved-at:   1785264314
```

**FR-22 is satisfied — but only after a reviewer gate caught that it wasn't.**
An earlier version of this README claimed it, wrongly: gate records were being signed by the
**relay keypair**, and the approver name came from an `actor` tag that any client can forge.
Every test passed, because none of them asked *who signed it*.

The fix (UP-18): `waggle gate` reads the reactions itself, takes the approver from each
reaction's **signature-bound pubkey**, checks it against the relay-signed admin list, and
publishes the record under waggle's own agent identity. The compiled workflow was demoted to
an advisory *notice* that names no approver at all.

```
$ waggle gate --role tea --channel <ch> --verdict-event <id> --verdict CONCERNS
CONCERNS approved by 47e6e1db…
record 4dc6742c…
  signed by 47e6e1db… (waggle identity, not the relay)
```

Verified in the log: signed by the agent, not the relay's `79be667e…`, and the id recomputes.
Guarded by `scripts/verify-gate-attribution.sh`.

### What works today

| Capability | Command |
|---|---|
| Version preflight, refuses outside the supported range | `waggle preflight` |
| Agent identity provisioning, secrets never printed | `waggle identity provision --role tea` |
| List identities (public data only) | `waggle identity list` |
| Enumerate modules, agents, and provenance | `waggle modules` |
| Compile a module to a validated persona pack + gate workflow | `waggle compile --module tea` |
| Provision a module's channels and canvases, idempotently | `waggle provision --module tea` |
| Publish an agent's profile under its own key | `waggle identity publish-profile --role tea --pack packs/tea` |

### What is not done

Honest accounting — see `docs/implementation-artifacts/sprint-status.yaml`.

| Gap | Blocked on |
|---|---|
| A live agent generating its own verdict (1.7, 1.9) | LLM provider credentials; `buzz-acp` needs an agent runtime on PATH |
| ~~CI-built relay image (1.3)~~ | **Unblocked** — `ghcr.io/block/buzz` is public; pull it. AD-17's build pipeline is redundant (UP-08 withdrawn) |
| Relay membership registration (1.5) | A stable relay signing key; the pubkey allowlist is off by default, so nothing needs it yet |
| Modules beyond the pilot | **Done.** `bmm` compiles — 6 agents, zero compiler changes (SM-5) |
| Agent roster membership in channels | Same live-agent blocker: rosters report `no live instances` |

Stories 1.8/1.9 were exercised with a verdict published by hand rather than generated
by an agent. The gate mechanics and the approver's attribution are both real; only the
verdict's *author* was simulated.

## Quick start

Full detail, including the port-conflict and Docker gotchas, is in
[`docs/dev-setup.md`](docs/dev-setup.md).

```bash
git clone --depth 1 --branch v0.4.26 https://github.com/block/buzz.git vendor/buzz
cd vendor/buzz && cp -n .env.example .env
. ./bin/activate-hermit && just setup && just relay    # terminal 1

cargo build --workspace                                 # terminal 2
./target/debug/waggle preflight
./target/debug/waggle identity provision --role tea
./target/debug/waggle compile --module tea --agent bmad-tea
```

You do **not** need Rust, Node, or pnpm installed — Buzz's Hermit supplies them, pinned.

## The three deliverables

**1. Installer / compiler (`waggle`)** — reads a BMAD installation and emits Buzz
persona packs and workflow YAML. Deterministic and byte-reproducible.

**2. Channel & canvas templates**, per module — `templates/<module>/channels.json`
compiles into the pack, and `waggle provision` delegates creation to the substrate while
adding the idempotence it lacks (UP-10). Canvases come free in the same file.

**3. Agent runtime config** — one keypair per role, scoped memberships, MCP config.
*Identity and profile publication work; live sessions need credentials.*

## Module coverage

| Module | Status |
|---|---|
| `tea` | **Compiles, gate fires, channels + canvas provision** |
| `bmm` | **Compiles — 6 agents, 2 channels. Proved SM-5: no compiler changes needed** |
| `core`, `bmb` | Installed; `bmb` ships no persona agents |
| `cis`, `gds`, `wds` | Not installed; deferred until the pattern generalizes |

**Quality gates are Buzz approval gates.** A human reaction on an artifact event fires
a workflow. No custom UI. Notably the gate does *not* use Buzz's `request_approval`
step — upstream marks such runs failed rather than suspended (UP-01), so waggle owns
gate state and derives it from the log instead.

## Architecture

Hexagonal — ports and adapters. Six crates:

| Crate | Role |
|---|---|
| `waggle-core` | Domain + pure transforms. **Zero I/O.** |
| `waggle-method` | Reads a BMAD installation. Read-only. |
| `waggle-emit` | Renders packs and workflow YAML. Deterministic. |
| `waggle-hive` | Talks to the relay. Never modifies it. |
| `waggle-gate` | *(gate logic currently lives in `waggle-core::gate`)* |
| `waggle-cli` | The only crate that wires the others |

The binding decisions are `AD-1`…`AD-20` in
[`docs/planning-artifacts/architecture/`](docs/planning-artifacts/architecture/). The
load-bearing one is **AD-5**: because waggle is Rust and BMAD's resolver is Python, the
override merge is reimplemented — so a differential test compares waggle's resolved
descriptor against BMAD's own resolver for every installed agent. It is mandatory and
non-skippable.

## Verification

```bash
cargo test --workspace                  # 84 tests
cargo clippy --workspace --all-targets -- -D warnings

./scripts/verify-preflight.sh           # exit-code taxonomy
./scripts/verify-identity.sh            # secret hygiene (greps the real key)
./scripts/verify-pack-contract.sh       # pack validates AND validator rejects defects
./scripts/verify-compile.sh             # determinism, full accounting, AD-7
./scripts/verify-gate.sh                # end-to-end gate (needs a running relay)
./scripts/verify-provision.sh           # channels + canvases, idempotence (needs a relay)
./scripts/verify-cli.sh                 # command surface + exit-code taxonomy
./scripts/verify-trail.sh               # signed artifacts, handoffs, priorities (needs a relay)
./scripts/verify-gate-attribution.sh    # WHO SIGNED IT — the assertion everything else missed
```

Each negative-tests itself: a check that only asserts the happy path can pass while
the thing it guards is broken. **Three times** in this project a test passed or failed
for a reason unrelated to what it claimed to test, caught only by deliberately breaking
the input. Tests were not enough on their own: the independent reviewer gates in
`docs/planning-artifacts/*/reviews/` found a critical defect (UP-18) that every green
test had missed, because the tests verified the mechanism and never asked who signed it. The portability lint, for example, is verified by poisoning a template with a
forbidden kind and requiring the compile to fail.

## Documentation

| Document | Contents |
|---|---|
| [`docs/dev-setup.md`](docs/dev-setup.md) | Clean machine to first signed message |
| [`docs/dev-loop.md`](docs/dev-loop.md) | How stories get built |
| [`docs/persona-pack-contract.md`](docs/persona-pack-contract.md) | The compiler's output contract, verified |
| [`docs/research-notes.md`](docs/research-notes.md) | Upstream research + §6 corrections |
| [`docs/upstream-issues.md`](docs/upstream-issues.md) | 18 upstream findings — **three withdrawn as our own error**, one a security defect |
| [`docs/planning-artifacts/*/reviews/`](docs/planning-artifacts/) | Independent reviewer-gate reports |
| [`docs/planning-artifacts/`](docs/planning-artifacts/) | Brief, PRD, architecture spine, epics |

## Pinned versions

See [`BUZZ_VERSION`](BUZZ_VERSION) — Buzz `v0.4.26`, BMAD Method `6.10.0`, Rust `1.95.0`.
Ranges, not just pins: waggle refuses to operate outside them.

## License

[Apache-2.0](LICENSE). See [NOTICE](NOTICE) for attribution and trademark statements.
