# waggle — TEA persona pack

A Buzz persona pack for the Test Architect role, derived from a BMAD Method TEA module
installation (`bmad-method-test-architecture-enterprise` v1.19.1).

Hand-built for Story 1.2 to validate the pack contract against
`crates/buzz-persona/PERSONA_PACK_SPEC.md`. Story 1.6 generates this same output from the
module definition automatically; this pack is the target the compiler must reproduce.

**Compatible with the BMAD Method™. Not affiliated with, or endorsed by, BMad Code, LLC.**
See the repository `NOTICE`.

## Contents

- `.plugin/plugin.json` — OPS-superset manifest
- `agents/bmad-tea.persona.md` — Murat 🧪
- `skills/` — 9 workflow skills, copied verbatim from the BMAD installation
- `instructions.md` — hive-wide team instructions

## Validate

```bash
buzz pack validate ./packs/tea
buzz pack inspect  ./packs/tea
```
