# Waggle — powered by BMAD

**Agentic method modules, compiled to a Buzz hive.**

Waggle is a self-hostable, [Nostr](https://github.com/nostr-protocol/nips)-based team workspace where every agent from the [BMAD Method](https://github.com/bmad-code-org/BMAD-METHOD) runs as a first-class member with its own keypair, and every artifact, handoff, and quality gate is a signed event in one auditable log.

It is a **distribution**, not a fork. Waggle self-hosts stock [Buzz](https://github.com/block/buzz) from upstream and adds a thin compilation and configuration layer on top. The Buzz codebase is treated as an external service and is never modified. See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for the authoritative architecture pointer.

> **Not affiliated with BMad Code, LLC.** Waggle is an independent, community distribution that is *compatible with* the BMAD Method™. "BMad", "BMad Method", and related marks are trademarks of BMad Code, LLC. See [NOTICE](NOTICE).

---

## Who it's for

- **Operators** who want BMAD's full method (software, game, creative, builder, testing) as Buzz Desktop rooms and agents — without building a custom UI.
- **Contributors** who want a clear path to improve the compiler, channel UX, docs, or gates while keeping BMAD as the method source of truth.

If you already use BMAD Help in a CLI or IDE, Waggle is the same orientation model in a team hive: choose a path, work in rooms, ask what's next anytime.

## How it works

```text
BMAD Method install (_bmad/)
        │
        ▼
  waggle sync / compile
        │
        ├── persona packs (agents + skills)
        ├── channel + canvas templates
        ├── gate workflows
        └── help catalog → #help canvas
        │
        ▼
  Buzz Desktop hive
  (@mention agents · phase rooms · human ✅ gates)
```

1. Install BMAD modules (or refresh with `waggle sync`).
2. Waggle compiles them into Buzz persona packs and provisions phase rooms.
3. You work in Buzz Desktop: `@mention` agents, follow path rooms, approve gates with ✅.

Details: [docs/how-waggle-works.md](docs/how-waggle-works.md).

## Hive UX — hubs and paths

Open **`#help` first**. It is the Desktop equivalent of slash-command `bmad-help`: pick a path, or describe your goal and `@mention` any agent.

**Always-on Help:** In *any* room, ask an agent "what's next?" / "continue" / **BH**. Agents load `bmad-help`, continue from where you left off, and should not dump the full catalog.

| Hub / path | Plain label | Rooms |
|---|---|---|
| `#help` | Path chooser + BMAD Help | — |
| `#party` | Multi-agent roundtable | — |
| **Software** | Build a product | `#planning` → `#architecture` → `#ux-design` → `#story` → `#implementation` → Testing |
| **Game** | Build a game | `#gds-design` → `#gds-production` |
| **Creative** | Ideate / brainstorm | `#ideation` (winners → `#planning`) |
| **Builder** | Extend the method | `#bmb-workshop` |
| **Testing** | Prove and gate | `#test-strategy` → `#gate` |

Optional: group these into Buzz Desktop sidebar sections to match the table. Channel *names* stay stable so re-provision does not fork rooms.

Experience design: [docs/planning-artifacts/ux-designs/ux-waggle-2026-07-30/](docs/planning-artifacts/ux-designs/ux-waggle-2026-07-30/).

## Quick start

Full walkthrough (clean machine → first signed message): [`docs/dev-setup.md`](docs/dev-setup.md).

```bash
# Clone this repo (waggle is the publishable root)
git clone <your-waggle-remote> waggle && cd waggle

# Substrate (gitignored; never modify tracked files inside)
git clone --depth 1 --branch v0.4.26 https://github.com/block/buzz.git vendor/buzz
cd vendor/buzz && cp -n .env.example .env
. ./bin/activate-hermit && just setup && just relay    # terminal 1

cargo build --workspace                                 # terminal 2 (repo root)
./target/debug/waggle sync --relay http://localhost:3100 \
  --buzz-cli ./vendor/buzz/target/debug/buzz

./target/debug/waggle runtime supervisor \
  --relay ws://localhost:3100 \
  --agent-owner <your-desktop-pubkey-hex>
```

Then open Buzz Desktop against the local relay and start in `#help`.

**Global skills:** `waggle sync` symlinks project `.claude/skills` into `~/.claude/skills` (or `$CLAUDE_SKILLS_HOME`) so Claude ACP can discover them. Use `--skip-global-skills` in CI/sandboxes.

## Status

The pilot works end to end against an unmodified Buzz relay: compile, provision, signed agents, and human-approved gates with correct attribution. Live agent replies still need Anthropic/Claude credentials in Desktop.

Honest gap tracking: [`docs/implementation-artifacts/sprint-status.yaml`](docs/implementation-artifacts/sprint-status.yaml). Longer pilot notes live under [`docs/`](docs/) — not in this front door.

## Architecture

Hexagonal — ports and adapters:

| Crate | Role |
|---|---|
| `waggle-core` | Domain + pure transforms. **Zero I/O.** |
| `waggle-method` | Reads a BMAD installation. Read-only. |
| `waggle-emit` | Renders packs and workflow YAML. Deterministic. |
| `waggle-hive` | Talks to the relay. Never modifies it. |
| `waggle-cli` | Wires the others |

Binding decisions: [`docs/planning-artifacts/architecture/`](docs/planning-artifacts/architecture/). **AD-5:** override merge is reimplemented in Rust and differentially tested against BMAD's Python resolver.

## Module coverage

| Module | What you get |
|---|---|
| `core` | `#help` + `#party`; help/party skills |
| `bmm` | Software path rooms + 6 agents |
| `wds` | Design agents; merges into `#ux-design` |
| `cis` | Creative path `#ideation` + ideation agents |
| `gds` | Game path `#gds-*` + game agents |
| `bmb` | Builder path `#bmb-workshop` |
| `tea` | Testing path `#test-strategy` + `#gate` |

**Quality gates are Buzz approval gates.** A human ✅ on a verdict event is recorded by `waggle gate` under the correct agent identity (not the relay).

## Verification

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings

./scripts/verify-preflight.sh
./scripts/verify-identity.sh
./scripts/verify-pack-contract.sh
./scripts/verify-compile.sh
./scripts/verify-sync.sh
./scripts/verify-cli.sh
# Needs a running relay:
./scripts/verify-gate.sh
./scripts/verify-provision.sh
./scripts/verify-trail.sh
./scripts/verify-gate-attribution.sh
```

## Documentation

| Document | Contents |
|---|---|
| [docs/README.md](docs/README.md) | Docs index |
| [docs/how-waggle-works.md](docs/how-waggle-works.md) | High-level system explanation |
| [docs/dev-setup.md](docs/dev-setup.md) | Clean machine → first signed message |
| [docs/dev-loop.md](docs/dev-loop.md) | How stories get built |
| [CONTRIBUTING.md](CONTRIBUTING.md) | How to contribute |
| [SECURITY.md](SECURITY.md) | How to report vulnerabilities |

## Pinned versions

See [`BUZZ_VERSION`](BUZZ_VERSION) — Buzz `v0.4.26`, BMAD Method `6.10.0`, Rust `1.95.0`. Waggle refuses to operate outside the supported ranges.

## Contributing

Contributions welcome. Start with [CONTRIBUTING.md](CONTRIBUTING.md): setup, tests, PR expectations, and the hard rule **do not modify Buzz**.

## License

[Apache-2.0](LICENSE). See [NOTICE](NOTICE) for attribution and trademark statements.
