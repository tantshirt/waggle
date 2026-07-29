#!/usr/bin/env bash
# The signed trail (Epic 3: FR-15, FR-17, FR-22, FR-24) against a RUNNING relay.
#
# This is the path buzz-cli cannot serve: it strips signatures on read and cannot attach
# typed tags on write (UP-07). waggle publishes and queries the relay directly, so the
# tests here check what that buys — queryable tags and verifiable provenance.
set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WAGGLE="${WAGGLE_BIN:-$REPO_ROOT/target/debug/waggle}"
BUZZ="${BUZZ_BIN:-$REPO_ROOT/vendor/buzz/target/release/buzz}"
RELAY="${RELAY:-http://localhost:3100}"
ROLE="${ROLE:-tea}"

fail=0
pass() { echo "  PASS  $1"; }
bad()  { echo "  FAIL  $1"; fail=1; }

[[ -x "$WAGGLE" ]] || { echo "waggle not built"; exit 3; }
[[ -f "$REPO_ROOT/keys/$ROLE.nsec" ]] || { echo "no $ROLE identity"; exit 3; }
lsof -nP -iTCP:"${RELAY##*:}" -sTCP:LISTEN >/dev/null 2>&1 || {
  echo "no relay at $RELAY — start it: (cd vendor/buzz && just relay)"; exit 3; }

export BUZZ_PRIVATE_KEY="$(cat "$REPO_ROOT/keys/$ROLE.nsec")"
export BUZZ_RELAY_URL="$RELAY"

echo "waggle trail — signed artifacts, handoffs, priorities"

CH="$("$BUZZ" channels create --name "trail-test-$$" --type stream --visibility open 2>/dev/null \
      | python3 -c "import sys,json;print(json.load(sys.stdin)['channel_id'])")"
[[ -n "$CH" ]] || { bad "create channel"; exit 1; }

pub() { # <priority> <body> -> event id
  "$WAGGLE" --format json --root "$REPO_ROOT" publish --role "$ROLE" --channel "$CH" \
    --artifact-type prd --module bmm --priority "$1" --body "$2" --relay "$RELAY" 2>/dev/null \
    | python3 -c "import sys,json;print(json.load(sys.stdin)['event_id'])"
}

P0="$(pub P0 'critical')"; P2="$(pub P2 'moderate')"
[[ -n "$P0" && -n "$P2" ]] && pass "artifacts publish with typed tags" || bad "artifacts publish with typed tags"

# FR-17: a handoff must reference the artifact it transfers.
if "$WAGGLE" --root "$REPO_ROOT" publish --role "$ROLE" --channel "$CH" --marker handoff \
     --from-role sm --to-role dev --ref "$P0" --body "ready" --relay "$RELAY" >/dev/null 2>&1; then
  pass "handoff publishes with a reference"
else
  bad "handoff publishes with a reference"
fi

# ...and must be rejected without one, before any network call.
"$WAGGLE" --root "$REPO_ROOT" publish --role "$ROLE" --channel "$CH" --marker handoff \
  --from-role sm --to-role dev --body "no ref" --relay "$RELAY" >/dev/null 2>&1
[[ $? -eq 1 ]] && pass "handoff without an artifact is rejected" || bad "handoff without an artifact is rejected"

# FR-24: priority filtering must be exact, not approximate.
count_for() {
  "$WAGGLE" --format json --root "$REPO_ROOT" trail --role "$ROLE" --channel "$CH" \
    --priority "$1" --relay "$RELAY" 2>/dev/null \
    | python3 -c "import sys,json;print(json.load(sys.stdin)['count'])"
}
[[ "$(count_for P0)" == "1" ]] && pass "P0 filter returns exactly the P0 artifact" || bad "P0 filter returns exactly the P0 artifact"
[[ "$(count_for P2)" == "1" ]] && pass "P2 filter returns exactly the P2 artifact" || bad "P2 filter returns exactly the P2 artifact"
[[ "$(count_for P1)" == "0" ]] && pass "unused priority returns nothing" || bad "unused priority returns nothing"

# An invalid priority is refused rather than silently matching nothing.
"$WAGGLE" --root "$REPO_ROOT" trail --role "$ROLE" --channel "$CH" --priority P9 --relay "$RELAY" >/dev/null 2>&1
[[ $? -eq 1 ]] && pass "invalid priority is refused" || bad "invalid priority is refused"

# FR-22: events come back signed, and the id recomputes from the canonical form.
if "$WAGGLE" --format json --root "$REPO_ROOT" trail --role "$ROLE" --channel "$CH" \
     --priority P0 --relay "$RELAY" 2>/dev/null | python3 -c "
import sys, json, hashlib
d = json.load(sys.stdin)
ev = d['events'][0]
assert ev.get('sig'), 'no signature — buzz-cli behaviour, not what we want'
ser = json.dumps([0, ev['pubkey'], ev['created_at'], ev['kind'], ev['tags'], ev['content']],
                 separators=(',', ':'), ensure_ascii=False)
assert hashlib.sha256(ser.encode()).hexdigest() == ev['id'], 'id does not recompute'
"; then
  pass "returned events are signed and their ids recompute (FR-22)"
else
  bad "returned events are signed and their ids recompute (FR-22)"
fi

# NFR-6: the trail rides standard kind:9, so third-party clients can read it.
if "$WAGGLE" --format json --root "$REPO_ROOT" trail --role "$ROLE" --channel "$CH" \
     --relay "$RELAY" 2>/dev/null | python3 -c "
import sys, json
d = json.load(sys.stdin)
assert d['count'] >= 3, d['count']
assert all(e['kind'] == 9 for e in d['events']), set(e['kind'] for e in d['events'])
"; then
  pass "whole trail is one query, all on standard kind:9 (NFR-6)"
else
  bad "whole trail is one query, all on standard kind:9 (NFR-6)"
fi

echo
if [[ $fail -eq 0 ]]; then echo "trail OK"; else echo "TRAIL DRIFT"; fi
exit $fail
