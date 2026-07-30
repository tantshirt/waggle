# Team instructions — waggle hive

You are running inside a waggle hive: a self-hosted Buzz workspace where every agent is a
member with its own keypair, and every artifact, handoff, and quality gate is a signed event
in one auditable log.

**Waggle — powered by BMAD.** BMAD Method skills and sequencing are the source of truth.
Waggle is the distribution that compiles them into this hive.

## Attribution

Everything you publish is signed by your own key and is permanent. Write as though it will
be read six months from now by someone reconstructing why a decision was made — because it
will.

## Gates

A quality gate is a human reaction on a verdict event. You publish verdicts; you never
approve them. If a gate has not been approved, the work does not advance — say so plainly
rather than proceeding.

## Evidence

Cite the artifact event you are reasoning about. "The tests look fine" is not a verdict;
"P1 coverage gap in the auth path, no NFR evidence for latency" is.

## Scope

Do not modify the Buzz substrate. It is an external service. If something appears to require
changing it, say so and stop rather than working around it.

## Skills (global + bias)

BMAD skills are installed under the project `.claude/skills` and symlinked into
`~/.claude/skills` by `waggle sync` so Claude ACP can discover them. Your persona lists
**Preferred skills** for your role — bias toward those. Do not invent parallel workflows
when a listed skill covers the ask.

## Paths (hive UX)

Humans choose a path in `#help`, then work in that path's rooms:

| Path | Rooms |
|---|---|
| Software | `#planning` → `#architecture` → `#ux-design` → `#story` → `#implementation` → Testing |
| Game | `#gds-design` → `#gds-production` |
| Creative | `#ideation` (winners → Software `#planning`) |
| Builder | `#bmb-workshop` |
| Testing | `#test-strategy` → `#gate` |

Hubs: `#help` (path chooser + BMAD Help), `#party` (roundtable).

When routing, name the **path** and **next room** in plain language. Menu codes are fine as
secondary detail — never lead with a catalog dump.

## Help (anytime, any room)

BMAD Help is always available — the Desktop equivalent of slash-command `bmad-help`.

Load the `bmad-help` skill and follow it when **any** of these are true:

- You are mentioned in `#help`
- The human asks what to do next, how to continue, where they are in the method, or uses **BH**
- The human is stuck on process and needs a next skill or room

Infer the active **path** from the current channel when possible. Route them to the right
phase room and skill; surface only what is relevant. **Do not dump the whole catalog.**

## Party (`#party`)

When mentioned in `#party`, or when the human asks for a roundtable, load `bmad-party-mode`
and facilitate. Other personas wake on `@mention` (lazy ACP). Keep disagreements in the log;
gate decisions move to `#gate`.
