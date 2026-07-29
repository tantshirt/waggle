#!/usr/bin/env bash
# Gate attribution (UP-18) against a RUNNING relay.
#
# This script exists because every other test in this project passed while the gate was
# broken. They asserted that a record appeared and that it was self-contained. None asked
# WHO SIGNED IT — and the answer was the relay, not the approver, with a spoofable
# `actor` tag deciding the name. That is the assertion below.
set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WAGGLE="${WAGGLE_BIN:-$REPO_ROOT/target/debug/waggle}"
BUZZ="${BUZZ_BIN:-$REPO_ROOT/vendor/buzz/target/release/buzz}"
RELAY="${RELAY:-http://localhost:3100}"
ROLE="${ROLE:-tea}"

# The relay's default dev signing key (secp256k1 generator x-coordinate). A gate record
# signed by this is the exact defect UP-18 describes.
RELAY_KEY="79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"

fail=0
pass() { echo "  PASS  $1"; }
bad()  { echo "  FAIL  $1"; fail=1; }

[[ -x "$WAGGLE" ]] || { echo "waggle not built"; exit 3; }
[[ -f "$REPO_ROOT/keys/$ROLE.nsec" ]] || { echo "no $ROLE identity"; exit 3; }
lsof -nP -iTCP:"${RELAY##*:}" -sTCP:LISTEN >/dev/null 2>&1 || {
  echo "no relay at $RELAY — start it: (cd vendor/buzz && just relay)"; exit 3; }

export BUZZ_PRIVATE_KEY="$(cat "$REPO_ROOT/keys/$ROLE.nsec")"
export BUZZ_RELAY_URL="$RELAY"
AGENT="$(cat "$REPO_ROOT/keys/$ROLE.pub")"

echo "waggle gate — attribution (UP-18)"

CH="$("$BUZZ" channels create --name "attr-$$" --type stream --visibility open 2>/dev/null \
      | python3 -c "import sys,json;print(json.load(sys.stdin)['channel_id'])")"
V="$("$WAGGLE" --format json --root "$REPO_ROOT" publish --role "$ROLE" --channel "$CH" \
      --marker verdict --body "waggle-gate-verdict
verdict: CONCERNS
rationale: attribution test" --relay "$RELAY" 2>/dev/null \
      | python3 -c "import sys,json;print(json.load(sys.stdin)['event_id'])")"
[[ -n "$V" ]] && pass "verdict published" || { bad "verdict published"; exit 1; }

# Fail closed: no reaction means no record.
out="$("$WAGGLE" --format json --root "$REPO_ROOT" gate --role "$ROLE" --channel "$CH" \
        --verdict-event "$V" --verdict CONCERNS --relay "$RELAY" 2>/dev/null)"
if echo "$out" | python3 -c "
import json,sys
d=json.load(sys.stdin)
assert 'Pending' in json.dumps(d['outcome']), d['outcome']
assert d.get('record_event') is None, 'a record was published with no approval'
"; then
  pass "no approving reaction means no record (fails closed)"
else
  bad "no approving reaction means no record"
fi

"$BUZZ" reactions add --event "$V" --emoji white_check_mark >/dev/null 2>&1

out="$("$WAGGLE" --format json --root "$REPO_ROOT" gate --role "$ROLE" --channel "$CH" \
        --verdict-event "$V" --verdict CONCERNS --relay "$RELAY" 2>/dev/null)"

# THE assertion. Signed by the agent, never the relay.
if echo "$out" | python3 -c "
import json,sys
d=json.load(sys.stdin)
signer = d.get('record_signed_by')
assert signer == '$AGENT', f'record signed by {signer}, expected the agent'
assert signer != '$RELAY_KEY', 'record signed by the RELAY — UP-18 has regressed'
"; then
  pass "gate record is signed by the agent identity, not the relay"
else
  bad "gate record is signed by the agent identity, not the relay"
  echo "$out" | head -c 300
fi

# The approver must be the reaction's signing key.
if echo "$out" | python3 -c "
import json,sys
d=json.load(sys.stdin)
o=d['outcome']['Approved']
assert o['approver'] == '$AGENT', o['approver']
assert o['reaction_event'], 'record must cite the reaction it derived the approver from'
"; then
  pass "approver is the reaction's signature-bound pubkey, and is cited"
else
  bad "approver is the reaction's signature-bound pubkey, and is cited"
fi

# And confirm it in the log itself, not just our own report.
if "$WAGGLE" --format json --root "$REPO_ROOT" trail --role "$ROLE" --channel "$CH" \
     --relay "$RELAY" 2>/dev/null | python3 -c "
import json, sys, hashlib
d = json.load(sys.stdin)
rec = [e for e in d['events'] if e['content'].startswith('waggle-gate-record')]
assert rec, 'no gate record in the log'
e = rec[0]
assert e['pubkey'] == '$AGENT', f\"log says signed by {e['pubkey']}\"
assert e['pubkey'] != '$RELAY_KEY', 'log shows a relay-signed record'
assert e.get('sig'), 'no signature'
ser = json.dumps([0, e['pubkey'], e['created_at'], e['kind'], e['tags'], e['content']],
                 separators=(',', ':'), ensure_ascii=False)
assert hashlib.sha256(ser.encode()).hexdigest() == e['id'], 'id does not recompute'
assert 'signature-bound' in e['content'], 'record must state where the approver came from'
"; then
  pass "the log confirms it: agent-signed, verifiable, FR-22 satisfied"
else
  bad "the log confirms it: agent-signed, verifiable, FR-22 satisfied"
fi

echo
if [[ $fail -eq 0 ]]; then echo "attribution OK"; else echo "ATTRIBUTION DRIFT"; fi
exit $fail
