#!/usr/bin/env bash
# Channel + canvas provisioning (Story 2.7, FR-10/FR-25/FR-26) against a RUNNING relay.
#
# The substrate does the provisioning; waggle supplies the template store and adds the
# idempotence the substrate lacks (UP-10). Both halves are tested here.
set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WAGGLE="${WAGGLE_BIN:-$REPO_ROOT/target/debug/waggle}"
BUZZ="${BUZZ_BIN:-$REPO_ROOT/vendor/buzz/target/release/buzz}"
RELAY="${RELAY:-http://localhost:3100}"
ROLE="${ROLE:-tea}"

fail=0
pass() { echo "  PASS  $1"; }
bad()  { echo "  FAIL  $1"; fail=1; }

[[ -x "$WAGGLE" ]] || { echo "waggle not built — cargo build --workspace"; exit 3; }
[[ -f "$REPO_ROOT/keys/$ROLE.nsec" ]] || { echo "no $ROLE identity — waggle identity provision --role $ROLE"; exit 3; }
lsof -nP -iTCP:"${RELAY##*:}" -sTCP:LISTEN >/dev/null 2>&1 || {
  echo "no relay at $RELAY — start it: (cd vendor/buzz && just relay)"; exit 3; }

echo "waggle provision — channels and canvases"

# The pack must actually carry a template store.
STORE="$REPO_ROOT/packs/tea/channel-templates.json"
[[ -f "$STORE" ]] && pass "compiled pack ships channel-templates.json" \
  || { bad "compiled pack ships channel-templates.json"; exit 1; }

# Wire shape: Buzz reads snake_case at top level, camelCase inside agents.
python3 - "$STORE" <<'PY'
import json, sys
store = json.load(open(sys.argv[1]))
t = store[0]
for k in ("name", "channel_type", "visibility", "canvas_template", "agents"):
    assert k in t, f"missing {k}"
assert "personas" in t["agents"] and "teams" in t["agents"]
for p in t["agents"]["personas"]:
    assert "personaId" in p, f"roster must use camelCase, got {list(p)}"
PY
[[ $? -eq 0 ]] && pass "store matches the shape Buzz deserializes" || bad "store matches the shape Buzz deserializes"

"$WAGGLE" --root "$REPO_ROOT" provision --module tea --role "$ROLE" --relay "$RELAY" >/dev/null 2>&1 \
  && pass "provision succeeds" || bad "provision succeeds"

# Idempotence — the reason this wrapper exists at all (UP-10, FR-25, NFR-2).
OUT="$("$WAGGLE" --format json --root "$REPO_ROOT" provision --module tea --role "$ROLE" --relay "$RELAY" 2>/dev/null)"
if echo "$OUT" | python3 -c "
import json,sys
d=json.load(sys.stdin)
assert d['channels'], 'no channels reported'
assert all(c['outcome']=='already-exists' for c in d['channels']), d['channels']
assert all(c.get('id') for c in d['channels']), 'existing channel must report its id'
" 2>/dev/null; then
  pass "re-provisioning creates nothing and reports existing ids"
else
  bad "re-provisioning creates nothing and reports existing ids"
fi

# And prove it against the relay, not just our own report.
export BUZZ_PRIVATE_KEY="$(cat "$REPO_ROOT/keys/$ROLE.nsec")"
export BUZZ_RELAY_URL="$RELAY"
if "$BUZZ" --format compact channels list 2>/dev/null | python3 -c "
import sys,json
from collections import Counter
names=[c['name'] for c in json.load(sys.stdin) if c.get('name','').startswith(('tea-','bmm-'))]
dupes={n:c for n,c in Counter(names).items() if c>1}
sys.exit(1 if dupes else 0)
"; then
  pass "relay holds no duplicate templated channels"
else
  bad "relay holds no duplicate templated channels"
fi

# The canvas is the half most likely to silently not apply.
CID="$("$BUZZ" --format compact channels list 2>/dev/null | python3 -c "
import sys,json
print(next((c['channel_id'] for c in json.load(sys.stdin) if c.get('name')=='tea-test-strategy'), ''))")"
if [[ -n "$CID" ]] && "$BUZZ" canvas get --channel "$CID" 2>/dev/null | grep -q "Test strategy"; then
  pass "canvas was applied from the template"
else
  bad "canvas was applied from the template"
fi

# A module with no templates must report, not fail (AD-6).
if "$WAGGLE" --root "$REPO_ROOT" provision --module core --relay "$RELAY" 2>&1 | grep -q "nothing to provision"; then
  pass "module without templates reports rather than failing"
else
  bad "module without templates reports rather than failing"
fi

echo
if [[ $fail -eq 0 ]]; then echo "provision OK"; else echo "PROVISION DRIFT"; fi
exit $fail
