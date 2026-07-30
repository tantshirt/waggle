#!/usr/bin/env bash
# Local gate for `waggle sync` / full-hive compile (Full BMAD Hive Mirror).
# Dry-run: compile --all, assert phase channel names, help catalog seed, core skills.
set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WAGGLE="${WAGGLE_BIN:-$REPO_ROOT/target/debug/waggle}"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

fail=0
pass() { echo "  PASS  $1"; }
bad()  { echo "  FAIL  $1"; fail=1; }

[[ -x "$WAGGLE" ]] || { echo "waggle not built — cargo build -p waggle-cli"; exit 3; }

echo "waggle sync — dry compile-all"

"$WAGGLE" --root "$REPO_ROOT" compile --all --out "$TMP/packs" >"$TMP/compile.txt" 2>&1 \
  && pass "compile --all" || { bad "compile --all"; tail -40 "$TMP/compile.txt"; }

# Phase rooms use stable names (not bmm-planning).
hive_json="$TMP/packs"
# Merge check: core help/party + bmm planning present under pack channel templates.
for mod_ch in "core:help" "core:party" "bmm:planning" "tea:gate" "cis:ideation"; do
  mod="${mod_ch%%:*}"
  ch="${mod_ch##*:}"
  f="$TMP/packs/$mod/channel-templates.json"
  if [[ -f "$f" ]] && grep -q "\"name\": \"$ch\"" "$f"; then
    pass "phase channel $ch in $mod"
  else
    bad "phase channel $ch in $mod"
  fi
done

# Help canvas: path chooser first, catalog appendix from bmad-help.csv when present.
help_csv="$REPO_ROOT/_bmad/_config/bmad-help.csv"
help_tpl="$TMP/packs/core/channel-templates.json"
if [[ -f "$help_csv" ]]; then
  if grep -q "Catalog appendix (from bmad-help.csv)" "$help_tpl" 2>/dev/null \
     || grep -q "Catalog (from bmad-help.csv)" "$help_tpl" 2>/dev/null; then
    pass "help canvas seeded from bmad-help.csv"
  else
    bad "help canvas seeded from bmad-help.csv"
  fi
  if grep -q "Choose a path" "$help_tpl" 2>/dev/null \
     && grep -q "Software" "$help_tpl" 2>/dev/null; then
    pass "help canvas path chooser"
  else
    bad "help canvas path chooser"
  fi
else
  echo "  SKIP  help csv absent (run installer / waggle sync once)"
fi

# Core help/party skills land in the core pack.
for skill in bmad-help bmad-party-mode; do
  if [[ -f "$TMP/packs/core/skills/$skill/SKILL.md" ]]; then
    pass "skill $skill in packs/core"
  else
    bad "skill $skill in packs/core"
  fi
done

# Shared instructions wire help/party behavior for every persona pack.
if grep -q 'bmad-help' "$TMP/packs/tea/instructions.md" \
   && grep -q 'bmad-party-mode' "$TMP/packs/tea/instructions.md" \
   && grep -q 'anytime' "$TMP/packs/tea/instructions.md"; then
  pass "instructions.md wires help/party skills"
else
  bad "instructions.md wires help/party skills"
fi

# Per-agent Preferred skills bias in compiled personas.
if grep -q '## Preferred skills' "$TMP/packs/tea/agents/bmad-tea.persona.md" \
   && grep -q 'bmad-help' "$TMP/packs/tea/agents/bmad-tea.persona.md" \
   && grep -q 'bmad-party-mode' "$TMP/packs/tea/agents/bmad-tea.persona.md"; then
  pass "persona Preferred skills bias"
else
  bad "persona Preferred skills bias"
fi

# Global skills publish into a temp home (never touch the developer's ~).
export CLAUDE_SKILLS_HOME="$TMP/claude-skills"
mkdir -p "$CLAUDE_SKILLS_HOME/01-cinematic"
echo '# user skill' >"$CLAUDE_SKILLS_HOME/01-cinematic/SKILL.md"
if "$WAGGLE" --root "$REPO_ROOT" sync --skip-install --offline --skip-global-skills >/dev/null 2>&1; then
  : # ensure sync still works with skip
fi
# Call publish via a tiny sync that only refreshes links: use sync without skip-global
# but offline + skip-install, writing into CLAUDE_SKILLS_HOME.
if "$WAGGLE" --root "$REPO_ROOT" sync --skip-install --offline \
     --human-pubkey 0000000000000000000000000000000000000000000000000000000000000001 \
     >"$TMP/sync-global.txt" 2>&1; then
  if [[ -L "$CLAUDE_SKILLS_HOME/bmad-help" ]] || [[ -e "$CLAUDE_SKILLS_HOME/.waggle-managed" ]]; then
    pass "global skills publish (CLAUDE_SKILLS_HOME)"
  else
    bad "global skills publish (CLAUDE_SKILLS_HOME)"
    tail -20 "$TMP/sync-global.txt"
  fi
  if [[ -d "$CLAUDE_SKILLS_HOME/01-cinematic" ]] && [[ ! -L "$CLAUDE_SKILLS_HOME/01-cinematic" ]]; then
    pass "global skills leave foreign dirs alone"
  else
    bad "global skills leave foreign dirs alone"
  fi
else
  bad "global skills publish (CLAUDE_SKILLS_HOME)"
  tail -20 "$TMP/sync-global.txt"
fi

# Pin file declares module set.
if grep -q '^BMAD_MODULES=' "$REPO_ROOT/BUZZ_VERSION"; then
  pass "BUZZ_VERSION declares BMAD_MODULES"
else
  bad "BUZZ_VERSION declares BMAD_MODULES"
fi

# Sync CLI surface exists.
if "$WAGGLE" sync --help >/dev/null 2>&1; then
  pass "waggle sync --help"
else
  bad "waggle sync --help"
fi

if "$WAGGLE" runtime supervisor --help >/dev/null 2>&1; then
  pass "waggle runtime supervisor --help"
else
  bad "waggle runtime supervisor --help"
fi

[[ "$fail" -eq 0 ]] && echo "verify-sync: OK" && exit 0
echo "verify-sync: FAILED"
exit 1
