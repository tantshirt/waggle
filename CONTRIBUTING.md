# Contributing to Waggle

Thanks for helping improve **Waggle — powered by BMAD**.

Waggle is an independent community distribution. It compiles BMAD Method modules into a Buzz hive. BMAD remains the method source of truth; Buzz remains an unmodified upstream substrate.

## Ground rules

1. **Do not modify Buzz.** The checkout under `vendor/buzz/` is an external service (gitignored). Never commit Buzz source or patch tracked files inside it. If you need a substrate change, document it in `docs/upstream-issues.md` as an upstream candidate.
2. **Do not invent parallel methods.** Prefer BMAD skills and sequencing. Waggle changes presentation (rooms, canvases, Help UX), compilation, and hive operations — not a second methodology.
3. **No secrets in git.** Agent keypairs live under `keys/` (gitignored). Never commit `.nsec`, `.env`, or personal override files with credentials.
4. **Keep the front door public.** No personal product names or private initiative docs in README / operator docs. Planning artifacts under `docs/planning-artifacts/` are method history, not the newcomer path.

## Setup

Follow [docs/dev-setup.md](docs/dev-setup.md) for a clean machine → local relay → first signed message.

Typical contributor loop:

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
./scripts/verify-sync.sh    # after touching templates, help, or sync
```

Pinned versions live in [`BUZZ_VERSION`](BUZZ_VERSION). Waggle refuses to operate outside the supported ranges.

### Local BMAD overrides

Optional personal BMAD config belongs in `_bmad/custom/config.user.toml` (gitignored via `_bmad/custom/.gitignore`). Do not commit user-specific values. Document defaults in `_bmad/config.toml` / `_bmad/custom/config.toml` when they are project-wide.

## What to work on

| Area | Where |
|---|---|
| Hive UX (paths, Help, canvases) | `templates/`, `crates/waggle-emit/src/help.rs`, `crates/waggle-cli/assets/instructions.md` |
| Compiler / packs | `crates/waggle-emit/`, `crates/waggle-method/` |
| Relay / provision / identity | `crates/waggle-hive/`, `crates/waggle-cli/` |
| Docs | `README.md`, `docs/`, this file |
| Experience design | `docs/planning-artifacts/ux-designs/` |

## Pull requests

- Keep PRs focused; separate UX copy from unrelated refactors when possible.
- Run the relevant `./scripts/verify-*.sh` gates locally.
- Update docs when you change operator-facing behavior (especially `#help` / paths / sync).
- Preserve stable channel names (`planning`, `help`, …). Renaming forks rooms on re-provision.
- Match existing Rust and markdown style; prefer clear operator language over jargon in canvases.

## Code of collaboration

Be respectful and constructive. Assume good faith. Prefer evidence (logs, event ids, failing tests) over vibes when debating gates or attribution.

## License

By contributing, you agree that your contributions are licensed under the Apache License, Version 2.0 (see [LICENSE](LICENSE)). Attribution and trademark notes are in [NOTICE](NOTICE).
