# How Waggle works

**Waggle — powered by BMAD.** This page explains the system at a high level so you can operate or contribute without reading every crate first.

## One sentence

Waggle reads a BMAD Method installation and compiles it into a Buzz hive: agents with keypairs, phase rooms with canvases, skills, and human-approved quality gates.

## Pieces

| Piece | Role |
|---|---|
| **BMAD Method** | Source of truth for agents, skills, phases, and Help catalog (`_bmad/`, including `bmad-help.csv`) |
| **Waggle CLI** | Install/sync BMAD, compile packs, provision identities/channels, run gate recording, emit runtime config |
| **Buzz** | Unmodified substrate: relay, Desktop client, channels, memberships, workflows, reactions |
| **Persona packs** | Compiled output under `packs/<module>/` — agents, skills, channel templates, instructions |
| **Hive** | Live Buzz workspace operators use day to day |

Waggle never vendors or patches Buzz source. See `BUZZ_VERSION` for pins and supported ranges.

## Main commands

| Command | What it does |
|---|---|
| `waggle sync` | Install/refresh BMAD (optional), compile all modules, provision hive pieces, link global skills |
| `waggle compile` | BMAD → persona packs + workflows (deterministic) |
| `waggle provision` | Create/update channels and canvases from templates (idempotent) |
| `waggle identity …` | Provision/register/publish agent identities (secrets never printed) |
| `waggle gate` | Record human ✅ approvals with correct attribution |
| `waggle runtime supervisor` | Lazy ACP: spawn agents on first `@mention` |

## Compile pipeline

```text
templates/<module>/channels.json  ──┐
_bmad/ agents + skills + help.csv ──┼──► waggle-emit ──► packs/<module>/
crate assets/instructions.md     ──┘         │
                                             ▼
                              channel-templates.json
                              agents/*.persona.md
                              skills/
                              instructions.md
                              workflows/
```

Important UX detail: `#help`'s canvas is generated from `bmad-help.csv` with a **path chooser first** and the full catalog as an appendix (`crates/waggle-emit/src/help.rs`).

## Hive UX model

Humans should not experience the hive as a random chat dump.

1. **`#help`** — choose Software / Game / Creative / Builder / Testing, or state a goal.
2. Work in that path's rooms (stable names: `planning`, `gds-design`, …).
3. **Anytime:** `@mention` an agent and ask what's next — they load `bmad-help` (same idea as BMAD Help in CLI/IDE).
4. **`#party`** — multi-agent roundtable when you want the cast together.
5. **`#gate`** — agents publish verdicts; humans approve with ✅; `waggle gate` writes the authoritative record.

See the experience spine under `planning-artifacts/ux-designs/ux-waggle-2026-07-30/`.

## Gates (why attribution matters)

A quality gate is not "the bot said PASS." It is:

1. Agent publishes a verdict event.
2. A human reacts ✅.
3. `waggle gate` reads reactions, takes the approver from the **signature-bound pubkey**, checks admin membership, and publishes a record under Waggle's agent identity.

Tests that only check "a record exists" are insufficient — they must ask **who signed it**.

## Skills discovery

`waggle sync` can symlink project `.claude/skills` into `~/.claude/skills` (or `$CLAUDE_SKILLS_HOME`) so Claude ACP discovers BMAD skills. Personas list **Preferred skills** so agents bias toward their menu. CI should use `--skip-global-skills`.

## Architecture (crates)

| Crate | Role |
|---|---|
| `waggle-core` | Pure domain — no I/O |
| `waggle-method` | Read BMAD install |
| `waggle-emit` | Deterministic pack/workflow/help rendering |
| `waggle-hive` | Relay adapters |
| `waggle-cli` | Composition root |

Decisions AD-1…AD-20 live under `planning-artifacts/architecture/`.

## After you change templates or Help

```bash
cargo build -p waggle-cli
./scripts/verify-sync.sh
# Against a live relay — --refresh rewrites description + canvas on existing rooms:
./target/debug/waggle provision --all --refresh \
  --relay http://localhost:3100 \
  --buzz-cli ./vendor/buzz/target/debug/buzz
```

Without `--refresh`, provision is create-only: existing channels stay on their old description/canvas (Desktop looks unchanged). Stable channel *names* are intentional — renaming would fork rooms.
