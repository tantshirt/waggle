#!/usr/bin/env bash
# End-to-end gate test (Stories 1.8/1.9, FR-19..FR-23) against a RUNNING relay.
#
# Proves the compiled workflow actually fires and that the resulting record is
# self-contained enough to reconstruct the decision from the log alone (FR-22).
#
# Requires: a relay on $RELAY, buzz-cli built, and a provisioned `tea` identity.
set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BUZZ="${BUZZ_BIN:-$REPO_ROOT/vendor/buzz/target/release/buzz}"
WAGGLE="${WAGGLE_BIN:-$REPO_ROOT/target/debug/waggle}"
RELAY="${RELAY:-http://localhost:3100}"

fail=0
pass() { echo "  PASS  $1"; }
bad()  { echo "  FAIL  $1"; fail=1; }

[[ -x "$BUZZ"   ]] || { echo "buzz-cli not built"; exit 3; }
[[ -f "$REPO_ROOT/keys/tea.nsec" ]] || { echo "no tea identity — waggle identity provision --role tea"; exit 3; }
curl -s -m 3 -o /dev/null "$RELAY" 2>/dev/null || true
if ! lsof -nP -iTCP:"${RELAY##*:}" -sTCP:LISTEN >/dev/null 2>&1; then
  echo "no relay listening at $RELAY — start it: (cd vendor/buzz && just relay)"
  exit 3
fi

export BUZZ_PRIVATE_KEY="$(cat "$REPO_ROOT/keys/tea.nsec")"
export BUZZ_RELAY_URL="$RELAY"

echo "waggle gate — end to end against $RELAY"

# The workflow under test is compiler output, not hand-written.
YAML="$REPO_ROOT/packs/tea/workflows/waggle-gate-tea.yaml"
[[ -f "$YAML" ]] || { echo "compile first: waggle compile --module tea --agent bmad-tea"; exit 3; }

CH=$("$BUZZ" channels create --name "gate-test-$$" --type stream --visibility open 2>/dev/null \
      | python3 -c "import sys,json;print(json.load(sys.stdin)['channel_id'])") || { bad "create channel"; exit 1; }

if "$BUZZ" workflows create --channel "$CH" --yaml "$(cat "$YAML")" >/dev/null 2>&1; then
  pass "relay accepts the compiled gate workflow"
else
  bad "relay accepts the compiled gate workflow"
  "$BUZZ" workflows create --channel "$CH" --yaml "$(cat "$YAML")" 2>&1 | tail -2
  exit 1
fi

VERDICT=$(printf 'waggle-gate-verdict\nverdict: CONCERNS\npriority: P1\nrationale: automated gate test')
EV=$("$BUZZ" messages send --channel "$CH" --content "$VERDICT" 2>/dev/null \
      | python3 -c "import sys,json;print(json.load(sys.stdin)['event_id'])")
[[ -n "$EV" ]] && pass "verdict published" || bad "verdict published"

"$BUZZ" reactions add --event "$EV" --emoji white_check_mark >/dev/null 2>&1 \
  && pass "approval reaction accepted" || bad "approval reaction accepted"

# The workflow engine fires asynchronously.
REC=""
for _ in $(seq 1 10); do
  sleep 1
  REC=$("$BUZZ" messages get --channel "$CH" 2>/dev/null | python3 -c "
import sys,json
for m in json.load(sys.stdin):
    if m['content'].startswith('waggle-gate-record'):
        print(json.dumps(m)); break
")
  [[ -n "$REC" ]] && break
done

if [[ -z "$REC" ]]; then
  bad "gate record published within 10s"
  exit 1
fi
pass "gate workflow fired and published a record"

# FR-22: the record alone must identify verdict, approver, and time.
# Exported explicitly: relying on the caller's environment made this check silently
# crash when invoked normally, while passing when invoked with a REC= prefix.
export REC
python3 - "$EV" <<'PY'
import json, sys, os
rec = json.loads(os.environ['REC'])
body = rec['content']
ev = sys.argv[1]
need = {
    'verdict-event: ' + ev: 'links the exact verdict event',
    'approver: ': 'names the approving identity',
    'approved-at: ': 'records when',
    'reaction: white_check_mark': 'records what triggered it',
}
missing = [d for k, d in need.items() if k not in body]
if missing:
    print('  FAIL  gate record is self-contained (missing: %s)' % ', '.join(missing))
    sys.exit(1)
print('  PASS  gate record is self-contained (FR-22)')
PY
[[ $? -eq 0 ]] || fail=1

echo
if [[ $fail -eq 0 ]]; then echo "gate OK"; else echo "GATE DRIFT"; fi
exit $fail
