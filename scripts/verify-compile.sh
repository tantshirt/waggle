#!/usr/bin/env bash
# Compiler guarantees for `waggle compile` (Story 1.6).
# Asserts AD-4 determinism, AD-6 full accounting, AD-7 sum-type handling, and that the
# GENERATED pack (not a hand-written one) passes Buzz's own validator.
set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WAGGLE="${WAGGLE_BIN:-$REPO_ROOT/target/debug/waggle}"
BUZZ="${BUZZ_BIN:-$REPO_ROOT/vendor/buzz/target/release/buzz}"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

fail=0
pass() { echo "  PASS  $1"; }
bad()  { echo "  FAIL  $1"; fail=1; }

[[ -x "$WAGGLE" ]] || { echo "waggle not built — cargo build --workspace"; exit 3; }

echo "waggle compile — pilot module"

"$WAGGLE" --root "$REPO_ROOT" compile --module tea --agent bmad-tea --out "$TMP/a" >"$TMP/a.txt" 2>&1 \
  && pass "compiles" || { bad "compiles"; cat "$TMP/a.txt"; }

# AD-4: byte-identical across runs, including skill copy order.
"$WAGGLE" --root "$REPO_ROOT" compile --module tea --agent bmad-tea --out "$TMP/b" >/dev/null 2>&1
if diff -r "$TMP/a" "$TMP/b" >/dev/null 2>&1; then
  pass "AD-4 deterministic: byte-identical across runs"
else
  bad "AD-4 deterministic"; diff -r "$TMP/a" "$TMP/b" | head -5
fi

# No absolute paths or timestamps leaked into generated artifacts.
if grep -rqF "$TMP" "$TMP/a/tea/agents" "$TMP/a/tea/.plugin" 2>/dev/null; then
  bad "generated files must not embed absolute paths"
else
  pass "no absolute paths in generated artifacts"
fi

# AD-7: GATE is prompt-only — body yes, skills list no.
persona="$TMP/a/tea/agents/bmad-tea.persona.md"
grep -q 'GATE' "$persona" && pass "AD-7 prompt item reaches the persona body" \
  || bad "AD-7 prompt item reaches the persona body"
grep -q './skills/GATE' "$persona" && bad "prompt item must not become a skill" \
  || pass "AD-7 prompt item is not emitted as a skill"

# Nine dispatch items become nine skills.
n=$(ls "$TMP/a/tea/skills" | wc -l | tr -d ' ')
[[ "$n" == "9" ]] && pass "9 skills copied" || bad "9 skills copied (got $n)"

# AD-6: the report accounts for everything, in machine-readable form.
"$WAGGLE" --format json --root "$REPO_ROOT" compile --module tea --agent bmad-tea --out "$TMP/c" \
  > "$TMP/c.json" 2>/dev/null
if python3 -c "
import json,sys
d=json.load(open('$TMP/c.json'))
r=next(x for x in d['reports'] if x['agent_id']=='bmad-tea')
assert r['prompt_only']==['GATE'], r['prompt_only']
assert not r['unknown'], r['unknown']
assert any(x['field']=='activation_steps_append' and x['reason'] for x in r['dropped']), r['dropped']
assert 'principles' in r['mapped'] and 'menu' in r['mapped'], r['mapped']
" 2>/dev/null; then
  pass "AD-6 report accounts for mapped, prompt-only, dropped, unknown"
else
  bad "AD-6 report accounts for mapped, prompt-only, dropped, unknown"
fi

# The point of the whole exercise: generated output satisfies the real validator.
if [[ -x "$BUZZ" ]]; then
  "$BUZZ" pack validate "$TMP/a/tea" >/dev/null 2>&1 \
    && pass "generated pack passes buzz pack validate" \
    || bad "generated pack passes buzz pack validate"
else
  echo "  SKIP  buzz pack validate (buzz-cli not built)"
fi

# --- SM-5: generality. A module the compiler was not developed against must compile
# --- with no compiler change: only registry data and skill placement.
echo
echo "waggle compile — generality (SM-5)"

"$WAGGLE" --root "$REPO_ROOT" compile --module bmm --out "$TMP/bmm" >"$TMP/bmm.txt" 2>&1 \
  && pass "second module compiles" || { bad "second module compiles"; cat "$TMP/bmm.txt"; }

n=$(ls "$TMP/bmm/bmm/agents" 2>/dev/null | wc -l | tr -d ' ')
[[ "$n" -ge 6 ]] && pass "all $n registered agents emitted" || bad "all registered agents emitted (got $n)"

if [[ -x "$BUZZ" ]]; then
  "$BUZZ" pack validate "$TMP/bmm/bmm" >/dev/null 2>&1 \
    && pass "second module's pack passes buzz pack validate" \
    || bad "second module's pack passes buzz pack validate"
fi

# SM-C1 made structural: AD-16 forbids a module-id conditional in the pure layers.
if grep -rnE '"(tea|bmm|bmb|cis|gds|wds)"' \
     "$REPO_ROOT/crates/waggle-core/src/compile.rs" \
     "$REPO_ROOT/crates/waggle-core/src/merge.rs" \
     "$REPO_ROOT/crates/waggle-emit/src/lib.rs" 2>/dev/null \
   | grep -vE "TEA_LIKE|bmad-tea|^\s*//|test" | grep -q .; then
  bad "AD-16: no module-id conditional in waggle-core or waggle-emit"
else
  pass "AD-16: no module-id conditional in the pure layers"
fi

# The registry is authoritative: a non-persona [agent] block must be reported, not compiled.
if "$WAGGLE" --format json --root "$REPO_ROOT" modules 2>/dev/null \
   | python3 -c "import json,sys; d=json.load(sys.stdin); sys.exit(0 if d['unregistered_agent_blocks'] else 1)"; then
  pass "non-persona [agent] blocks are surfaced, not compiled (AD-6)"
else
  bad "non-persona [agent] blocks are surfaced, not compiled (AD-6)"
fi

echo
if [[ $fail -eq 0 ]]; then echo "compile OK"; else echo "COMPILE DRIFT"; fi
exit $fail
