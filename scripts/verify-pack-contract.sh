#!/usr/bin/env bash
# Round-trip test for the persona pack contract (Story 1.2).
# Asserts our pack validates AND that the validator rejects three specific defects.
# If this fails, either our pack drifted or the upstream contract changed — both matter.
set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BUZZ="${BUZZ_BIN:-$REPO_ROOT/vendor/buzz/target/release/buzz}"
PACK="${1:-$REPO_ROOT/packs/tea}"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

fail=0
pass() { echo "  PASS  $1"; }
bad()  { echo "  FAIL  $1"; fail=1; }

if [[ ! -x "$BUZZ" ]]; then
  echo "buzz binary not found at $BUZZ"
  echo "Build it:  cd vendor/buzz && cargo build --release -p buzz-cli"
  exit 2
fi

echo "Persona pack contract — $PACK"

# --- positive ---
if "$BUZZ" pack validate "$PACK" >/dev/null 2>&1; then
  pass "pack validates"
else
  bad "pack validates"; "$BUZZ" pack validate "$PACK" 2>&1 | head -3
fi

# --- negative: the validator must actually reject defects ---
neg() { # <label> <mutation-command>
  local label="$1"; shift
  rm -rf "$TMP/p"; cp -R "$PACK" "$TMP/p"
  ( cd "$TMP/p" && eval "$@" ) >/dev/null 2>&1
  if "$BUZZ" pack validate "$TMP/p" >/dev/null 2>&1; then
    bad "rejects: $label (it accepted a defective pack)"
  else
    pass "rejects: $label"
  fi
}

neg "missing required display_name" "perl -pi -e 's/^display_name: .*\$//' agents/*.persona.md"
# Anchor the mutation on `name:`, which every persona must have by definition. Anchoring
# on an optional field silently no-ops when the pack shape changes, and the check then
# passes for the wrong reason -- which is exactly what happened when the hand-built pack
# was replaced by compiler output.
neg "unknown frontmatter key"       "perl -pi -e 's/^(name: .*)\$/\$1\ntemprature: 0.5/' agents/*.persona.md"
neg "persona file listed but absent" "rm agents/*.persona.md"

# --- BMAD/Buzz skill format compatibility ---
missing=0
for d in "$PACK"/skills/*/; do
  [[ -f "$d/SKILL.md" ]] || { missing=1; continue; }
  grep -q '^name:' "$d/SKILL.md" && grep -q '^description:' "$d/SKILL.md" || missing=1
done
if [[ $missing -eq 0 ]]; then
  pass "every skill has required name+description frontmatter"
else
  bad "every skill has required name+description frontmatter"
fi

echo
if [[ $fail -eq 0 ]]; then echo "contract OK"; else echo "CONTRACT DRIFT"; fi
exit $fail
