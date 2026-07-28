# waggle

**Agentic method modules, compiled to a Buzz hive.**

Waggle is a self-hostable, Nostr-based team workspace where every agent from the
BMAD Method™ agent framework — across all official modules — runs as a first-class
member with its own keypair, and every artifact, handoff, and quality gate is a
signed event in one auditable log.

It is a **distribution**, not a fork. Waggle self-hosts stock
[Buzz](https://github.com/block/buzz) from upstream and adds a thin compilation and
configuration layer on top. The Buzz codebase is treated as an external service and
is never modified.

> **Not affiliated with BMad Code, LLC.** Waggle is an independent, community
> distribution that is *compatible with* the BMAD Method™. "BMad", "BMad Method",
> and related marks are trademarks of BMad Code, LLC. See [NOTICE](NOTICE).

---

## The three deliverables

Waggle is exactly three things layered over an unmodified Buzz relay.

**1. Installer / compiler (`waggle`)**
Reads BMAD `module.yaml` definitions (agents + workflows) and emits:
- Buzz **persona packs**, consumable by `buzz-persona` / `buzz-agent`
- Buzz **workflow YAML** for the relay's workflow engine (message / reaction /
  schedule / webhook triggers)

**2. Channel & canvas templates**, per module
BMM story channels · CIS brainstorm rooms · WDS design-spec canvases ·
GDS GDD canvases · TEA test-strategy canvases.

**3. Agent runtime config**
One `buzz-agent` session per method role, each with its own Nostr keypair (npub),
scoped channel memberships, and MCP config — driven via `buzz-cli` (JSON in / JSON out).

## Module coverage

All official method packs, not just core:

| Module | Role | Mapped onto Buzz as |
|---|---|---|
| `core` | Shared tasks + global config | Relay-level config and shared workflow tasks |
| `bmm` | Four-phase agile lifecycle | Phase channel categories; each story = a channel; SM→Dev→QA handoffs = signed events; Dev output = NIP-34 patch events |
| `bmb` | Module builder | Tooling to author new Buzz-native modules; *stretch:* publish/install community modules as signed Nostr events |
| `cis` | Ideation agents | Brainstorm channels + canvases; party mode = multiple agent npubs in one room |
| `tea` | Test Architect | P0–P3 priorities as event tags; release gates as reaction-triggered approval-gate workflows |
| `gds` | Game dev | BMM lifecycle with GDD canvases replacing PRDs |
| `wds` | Design-first UX | Design-spec canvases feeding the BMM PM agent |

**Quality gates are Buzz approval gates.** Every stage-transition checkpoint becomes a
workflow triggered by a human reaction on the artifact event. No custom UI.

**Pilot module: TEA.** The full `module.yaml → persona pack + workflow YAML` pipeline is
proven end-to-end on TEA before generalizing.

## Status

🚧 Pre-alpha. Phase 0 (scaffold) complete. See `docs/` for the product brief, PRD, and
architecture, and `docs/research-notes.md` for upstream research.

## Pinned versions

See [`BUZZ_VERSION`](BUZZ_VERSION). Currently targeting Buzz `v0.4.26` and
BMAD Method `6.10.0`.

## License

[Apache-2.0](LICENSE). See [NOTICE](NOTICE) for attribution and trademark statements.
