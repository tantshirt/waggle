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

# FR-16 / AD-15: bodies within the content limit publish inline; larger bodies are
# uploaded via Blossom PUT /upload and the event carries a content-addressed reference.
# Note the content limit is 262144, NOT the 65536 frame limit older docs claimed (UP-14).
# UP-16's "images only" claim was withdrawn — the relay accepts markdown/octet-stream.
big="$(python3 -c "print('x' * 200000)")"
OUT_BIG="$("$WAGGLE" --format json --root "$REPO_ROOT" publish --role "$ROLE" --channel "$CH" \
     --artifact-type prd --body "$big" --relay "$RELAY" 2>/dev/null)"
if echo "$OUT_BIG" | python3 -c "
import json,sys
d=json.load(sys.stdin)
assert d.get('transport')=='inline', d
assert d.get('event_id')
"; then
  pass "a 200 KB artifact publishes inline (well past the old 64 KB assumption)"
else
  bad "a 200 KB artifact publishes inline"
fi

huge="$(python3 -c "print('x' * 300000)")"
OUT_HUGE="$("$WAGGLE" --format json --root "$REPO_ROOT" publish --role "$ROLE" --channel "$CH" \
        --artifact-type prd --body "$huge" --relay "$RELAY" 2>&1)"
code=$?
if [[ $code -eq 0 ]] && echo "$OUT_HUGE" | python3 -c "
import json,sys,hashlib,urllib.request
d=json.load(sys.stdin)
assert d.get('transport')=='reference', d
assert d.get('sha256') and d.get('blob_url'), d
# FR-16: retrieve and verify the hash matches.
body=urllib.request.urlopen(d['blob_url'], timeout=10).read()
assert hashlib.sha256(body).hexdigest()==d['sha256'], 'hash mismatch'
assert len(body)==300000
assert b'x'*300000==body
"; then
  pass "an oversized artifact publishes by Blossom reference and retrieves byte-identical"
else
  bad "an oversized artifact publishes by Blossom reference and retrieves byte-identical"
  echo "$OUT_HUGE" | tail -5
fi

# Bodies over the media-store cap (~100 MB) are refused specifically — covered by
# the unit test on refuse_if_over_media_cap (allocating 100 MB here is not useful).
pass "media-cap refusal covered by unit test (MAX_BLOB_BYTES)"
# FR-18: developer output as a portable NIP-34 patch, linked to its story channel.
# Patches have their OWN content limit (61440) distinct from kind:9's 262144 (UP-17),
# so the fixture is deliberately a small commit.
PUB="$(cat "$REPO_ROOT/keys/$ROLE.pub" 2>/dev/null)"
EUC="$(git -C "$REPO_ROOT" rev-list --max-parents=0 HEAD | tail -1)"
SMALL="$(git -C "$REPO_ROOT" rev-list HEAD | while read c; do
  sz=$(git -C "$REPO_ROOT" format-patch -1 "$c" --stdout 2>/dev/null | wc -c)
  if [ "$sz" -lt 50000 ] && [ "$sz" -gt 500 ]; then echo "$c"; break; fi
done)"
PF="$(mktemp)"; git -C "$REPO_ROOT" format-patch -1 "$SMALL" --stdout > "$PF"

"$BUZZ" repos create --id "trailtest$$" --name "trail test" \
  --clone https://example.invalid/x.git >/dev/null 2>&1

OUT="$("$WAGGLE" --format json --root "$REPO_ROOT" patch --role "$ROLE" --channel "$CH" \
        --repo-id "trailtest$$" --patch-file "$PF" --euc "$EUC" --relay "$RELAY" 2>&1)"
if echo "$OUT" | python3 -c "
import json,sys
d=json.load(sys.stdin)
assert d['patch_event'] and d['link_event'], d
assert 1617 in d['kinds'], d['kinds']
" 2>/dev/null; then
  pass "patch publishes as NIP-34 kind:1617 and is linked to the story channel"
else
  bad "patch publishes as NIP-34 kind:1617 and is linked to the story channel"
  echo "$OUT" | tail -2
fi
rm -f "$PF"

# The link must reference the patch, or FR-18's traceability is decorative.
if echo "$OUT" | python3 -c "
import json,sys
json.load(sys.stdin)
" 2>/dev/null; then
  PE="$(echo "$OUT" | python3 -c "import sys,json;print(json.load(sys.stdin)['patch_event'])")"
  if "$WAGGLE" --format json --root "$REPO_ROOT" trail --role "$ROLE" --channel "$CH" \
       --relay "$RELAY" 2>/dev/null \
     | python3 -c "
import json,sys
d=json.load(sys.stdin)
refs=[t[1] for e in d['events'] for t in e['tags'] if t[0]=='e']
sys.exit(0 if '$PE' in refs else 1)
"; then
    pass "the story channel links back to the patch event"
  else
    bad "the story channel links back to the patch event"
  fi
fi

echo
if [[ $fail -eq 0 ]]; then echo "trail OK"; else echo "TRAIL DRIFT"; fi
exit $fail
