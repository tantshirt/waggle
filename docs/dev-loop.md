# The waggle dev loop

How stories get built. One story per iteration, stopping for a human accept each time.

## The contract

| Setting | Value |
|---|---|
| Scope per iteration | **One story**, start to accepted |
| Stop condition | After each story — never auto-advances |
| Trigger | The user says `next` (or names a story). No timer, no scheduled wake-ups. |
| Subagents | **Allowed** for the dev loop, including `bmad-dev-auto`'s parallel reviewers |
| Work queue | `docs/implementation-artifacts/sprint-status.yaml` |

## One iteration

1. **Pick** the next story whose status is `backlog`, in id order, from `sprint-status.yaml`.
   Epic order is 1 → 2 → 3; within an epic, strictly ascending.
2. **Create the story file** — `bmad-create-story` writes a context-complete spec into
   `docs/implementation-artifacts/`. Status → `ready-for-dev`.
3. **Implement** — `bmad-dev-story` (attended) or `bmad-dev-auto` (unattended iteration).
   Status → `in-progress`, then `review`.
4. **Review** — `bmad-code-review` runs adversarial parallel review layers in fresh context.
5. **Land** — update `sprint-status.yaml`, conventional-commit, push to `origin main`.
6. **Stop** and report: what now verifiably works, and the single next action.

Steps 5 and 6 are enforced by the `on_complete` override in
`_bmad/custom/bmad-dev-auto.toml` and `_bmad/custom/bmad-dev-story.toml`, so the loop cannot
silently run on into the next story.

Accept happens in conversation. Only then does the story move to `done`.

## Status vocabulary

`backlog` → `ready-for-dev` → `in-progress` → `review` → `done`

`backlog` means the story exists only in `epics.md`. `ready-for-dev` means a story file exists.
Do not mark a story `ready-for-dev` before its file is written — the distinction is what makes
the queue trustworthy.

## What every loop invocation carries

Team overrides in `_bmad/custom/*.toml` append to each skill's persistent facts, so the binding
constraints travel with every invocation rather than living only in the spine:

- `project-context.md` (skill default) — the 15 rules most easily violated
- `ARCHITECTURE-SPINE.md` — AD-1 … AD-20, binding
- `epics.md` — the story and its acceptance criteria
- `upstream-issues.md` — known substrate defects, so the loop does not "fix" one locally
- Four verbatim guardrails: never write to `vendor/buzz/`; the method installation is read-only;
  never emit an `nsec`; surface AD conflicts instead of working around them

Arrays append under BMAD's merge rules, so these add to the defaults rather than replacing them.

## Ordering constraints the loop must respect

From the epics validation record — each story depends only on earlier ones:

- **1.1 → 1.2** — the schema read needs a running relay.
- **1.2 → 1.6** — the persona pack cannot be compiled before its target schema is known.
- **1.5 + 1.6 → 1.7** — a running agent needs both an identity and a pack.
- **1.7 + 1.8 → 1.9** — the gate needs a live agent and a compiled workflow.

Stories 1.3 and 1.4 are independent and can be reordered freely.

## When the loop must stop early

Stop and surface, rather than working around, any of:

- A task that appears to require modifying `vendor/buzz/` (log it in `docs/upstream-issues.md`).
- A conflict with a binding AD.
- A discovery that invalidates a downstream story — **most likely in Story 1.2**, if Buzz's real
  persona pack schema differs materially from what the architecture inferred. That would put
  stories 1.6–1.9 on a wrong assumption, and is exactly why 1.2 is sequenced second.
- A needed dependency outside the Buzz / BMAD / Nostr ecosystems.

## Deferred review debt

The PRD and the architecture spine were both finalized with their reviewer gates **skipped**,
because subagents were unavailable at the time. Both are logged as overrides in their memlogs.
Now that subagents are permitted, re-running those two gates is worthwhile before Epic 2 — the
spine especially, since AD-1 … AD-20 bind every story after it.

---

## Where the loop stopped (2026-07-29)

Epic 1 ran to the end of what is reachable without external credentials or a publishing
decision. Current state is in `docs/implementation-artifacts/sprint-status.yaml`; this
section records *why* each unfinished story is unfinished, so the next session does not
re-derive it.

| Story | State | Why |
|---|---|---|
| 1.1 walking skeleton | **done** | — |
| 1.2 pack contract | **done** | Rescoped mid-flight; the contract was documented, not missing |
| 1.3 CI-built image | backlog | **Needs a decision:** publishing images to a public registry is outward-facing |
| 1.4 version preflight | **done** | — |
| 1.5 agent identity | in-progress | Provisioning + profile done; `add-member` needs a stable relay signing key |
| 1.6 compile persona pack | **done** | SM-1 achieved |
| 1.7 agent as hive member | in-progress | Profile done; a live turn needs an agent runtime + LLM credentials |
| 1.8 compile the gate | **done** | Fires end to end on a real relay |
| 1.9 approve + reconstruct | in-progress | Mechanics proven; the verdict was published by hand, not generated |

### The three unblocks

1. **LLM provider credentials** + an ACP runtime (`goose` or a built `buzz-agent`) on
   PATH. Finishes 1.7 and 1.9 properly. `buzz-acp` reads `BUZZ_ACP_AGENT_COMMAND`;
   model selection projects to `GOOSE_PROVIDER` / `GOOSE_MODEL`.
2. **A decision on publishing container images.** Unblocks 1.3 and AD-17.
3. **`BUZZ_RELAY_PRIVATE_KEY`** set on the relay, then restart. Unblocks 1.5's
   `add-member`. Low urgency: the pubkey allowlist is off by default, so agents
   authenticate and publish without it.

### What is genuinely next, needing nothing

**Epic 2.** The compiler already has no module-specific branches (AD-16, asserted), so
compiling a second module is the cheapest way to test SM-5 — and Story 1.2's finding
that BMAD skills *are* Buzz pack skills means Epic 2 should be re-estimated downward
before it is scheduled.

Also outstanding: the **reviewer gates skipped on the PRD and the architecture spine**
while subagents were unavailable. Both are logged as overrides in their memlogs. The
spine's gate is worth running before Epic 2, since `AD-1`…`AD-20` bind everything after
it — and this project has already had two tests pass for the wrong reason, which is
exactly the class of thing an independent reviewer catches.
