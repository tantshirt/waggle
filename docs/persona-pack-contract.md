# Persona pack contract

waggle's compiler output contract. **The upstream authority is
`vendor/buzz/crates/buzz-persona/PERSONA_PACK_SPEC.md`** at Buzz `v0.4.26`; this document
records what we verified against a running relay, what the BMAD Method maps onto, and where
the spec and the implementation disagree.

Verified 2026-07-28 (Story 1.2) by hand-building `packs/tea` and round-tripping it through
`buzz pack validate` and `buzz pack inspect`.

> **Story 1.2 was rescoped.** It was written as "discover the undocumented schema." The
> schema turned out to be fully documented (see `docs/upstream-issues.md` UP-04, withdrawn),
> so the story became "validate the documented contract against reality." That is what this
> document is.

---

## 1. Verified result

```
$ buzz pack validate ./packs/tea
Valid.

$ buzz pack inspect ./packs/tea
Pack: waggle — Test Architect (TEA) (dev.waggle.pack.tea)
Personas: 1
  bmad-tea
    Display: Murat 🧪
    Triggers: mentions + keywords ["gate", "test design", "coverage", "traceability", "NFR"]
    Skills: 9 skills
    System prompt: 3844 chars
```

The validator is not a rubber stamp. Three negative tests each fail with a specific error:

| Mutation | Result |
|---|---|
| Remove `display_name` | `missing required field: display_name` |
| Add unknown key `temprature` | `unknown field 'temprature', expected one of …` |
| Delete a persona file listed in the manifest | `persona file not found: …` |

Reproduce with `scripts/verify-pack-contract.sh`.

## 2. Pack layout waggle emits

```
packs/<module>/
├── .plugin/plugin.json          # OPS-superset manifest
├── agents/<agent-id>.persona.md # one per module agent
├── skills/<skill-id>/SKILL.md   # copied verbatim from the method installation
├── instructions.md              # hive-wide team instructions
└── README.md                    # provenance + attribution
```

## 3. Persona frontmatter — authoritative field list

The spec's §4 table omits one field the parser accepts. This list came out of the parser's
own `deny_unknown_fields` error and is therefore authoritative:

```
name, display_name, avatar, description, version, author, skills,
mcp_servers, subscribe, respond_to, triggers, model, runtime,
temperature, max_context_tokens, thread_replies, broadcast_replies, hooks
```

> ⚠️ **Spec drift: `runtime`.** Accepted by the parser, absent from
> `PERSONA_PACK_SPEC.md` §4's field reference. Undocumented semantics. waggle does not emit
> it. Logged as **UP-09**.

**Required:** `name`, `display_name`, `description`. Everything else is optional.

**`name` must be lowercase with no spaces** and unique within the pack. waggle uses the
method's own agent id (e.g. `bmad-tea`) unchanged — AD-3's "never re-derive an id the
method already owns."

## 4. BMAD → pack mapping, as verified

| BMAD source | Pack target | Notes |
|---|---|---|
| module (e.g. `tea`) | one pack | `id` = `dev.waggle.pack.<module>` |
| agent id (`bmad-tea`) | persona `name` | passed through unchanged |
| `customize.toml` `name` + `icon` | `display_name` (`"Murat 🧪"`) | icon becomes part of the display name |
| `config.toml` `[agents.*].description` | `description` | |
| `role`, `identity`, `communication_style`, `principles[]` | **markdown body** | prose sections, not frontmatter |
| menu item with `skill` | entry in `skills[]` + `skills/<id>/` | 9 of TEA's 10 |
| menu item with `prompt` | **markdown body section** | 1 of TEA's 10 (`GATE`) — confirms **AD-7** |
| `persistent_facts` | *not yet mapped* | see §6 |
| module version + provenance | manifest `version`, README | |

### The finding that matters

**BMAD skills are Buzz pack skills, byte-for-byte.** Both are `SKILL.md` with required
`name:` and `description:` frontmatter. All 9 TEA workflow skills were copied from
`.claude/skills/` into `packs/tea/skills/` **with no modification** and validated.

The pack spec's own skill discovery list already includes
`$AGENT_CWD/.claude/skills/<skill-name>/SKILL.md` — the exact path BMAD installs to.

So the "compiler" is smaller than the PRD assumed: **a manifest generator, a persona-file
renderer, and a directory copy.** It is not a translator. This strengthens SM-C1 — there is
even less justification for module-specific branches.

## 5. Behavioral config

Applies identically in `plugin.json` `defaults` and in persona frontmatter.

| Field | Type | Built-in default |
|---|---|---|
| `subscribe` | `string[]` | `[]` — `#` prefix is display-only, stripped before relay calls |
| `triggers.mentions` | bool | `true` |
| `triggers.keywords` | `string[]` | `[]` — case-insensitive |
| `triggers.all_messages` | bool | `false` |
| `model` | string | none — `"provider:model-id"`, split on first `:` |
| `temperature` | float | `0.7` |
| `max_context_tokens` | int | none |
| `thread_replies` | bool | `true` |
| `broadcast_replies` | bool | `false` |

**Precedence (highest first):** operator env vars → desktop UI per-agent → persona
frontmatter → pack `defaults` → built-in.

**Merge is shallow replacement — there is no deep merge.** A persona's `triggers` replaces
the pack default object entirely; sub-keys are not inherited. Same for `subscribe` arrays.

**Null vs empty:** `null` = absent (falls through). `[]` and `{}` = present (override).

> ⚠️ **This is the opposite of BMAD's merge semantics.** BMAD appends arrays and deep-merges
> tables; Buzz replaces wholesale. The compiler must resolve BMAD's layers **fully** (AD-5)
> and emit a **flat, already-resolved** persona — never rely on Buzz to finish a merge.
> Getting this backwards would silently drop principles and menu items.

## 6. Not yet mapped

- **`persistent_facts`.** BMAD supports `file:` glob entries loaded as facts. The pack spec
  has no equivalent; the nearest options are `instructions.md` or the persona body. Deferred
  to Story 1.6.
- **`activation_steps_prepend` / `_append`.** No pack equivalent. Buzz has lifecycle
  `hooks` (`on_start`, `on_stop`, `on_message`) but the spec states hooks are **parsed and
  validated but not yet executed**. Do not depend on them.
- **MCP config.** `.mcp.json` and per-persona `mcp_servers` are specified; waggle emits
  neither yet. Note `${VAR}` interpolation is **not implemented** — values pass through as
  literals. Only `stdio` and `streamable_http` transports; SSE is rejected.
- **Avatars.** `avatar` is a pack-relative path; BMAD supplies only an emoji icon.

## 7. Distribution

- Zip packs are `.buzzpack` and **must** ship `<name>-<version>.buzzpack.sha256`.
  buzz-acp refuses on mismatch.
- Git installs are supported; `pack.lock` records the resolved commit SHA.
- Installed packs live at `~/.buzz/packs/<pack-id>/`.
- `engines.buzz` gates the minimum Buzz version; buzz-acp rejects packs that require newer.

This is the natural carrier for the `bmb` stretch goal — publishing modules as signed,
verifiable artifacts — and it already has integrity checking built in.

## 8. Consequences for the architecture

1. **AD-7 is confirmed by real data.** TEA has 10 menu items; 9 compile to skills, 1
   (`GATE`) has no skill and must land in the persona body. The sum type is correct.
2. **AD-5 becomes more load-bearing, not less.** Because Buzz replaces rather than merges,
   BMAD's layered overrides must be fully resolved *before* emission. A partial resolution
   silently loses data with no error anywhere.
3. **The compiler is smaller than scoped.** Epic 2 should be re-estimated downward.
4. **Skills need no transformation at all** — only placement. FR-2's "compile" is mostly
   file assembly.
