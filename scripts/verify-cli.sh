#!/usr/bin/env bash
# Command-surface contract (Story 2.9, FR-27, AD-20, NFR-9).
#
# The surface is a public compatibility contract (PRD section 12), so it is tested rather
# than assumed: every command emits a versioned envelope on stdout, diagnostics go to
# stderr, nothing requires a terminal, and exit codes follow one taxonomy.
set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WAGGLE="${WAGGLE_BIN:-$REPO_ROOT/target/debug/waggle}"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

fail=0
pass() { echo "  PASS  $1"; }
bad()  { echo "  FAIL  $1"; fail=1; }

[[ -x "$WAGGLE" ]] || { echo "waggle not built — cargo build --workspace"; exit 3; }

echo "waggle — command surface"

# Every capability the PRD names must exist.
HELP="$("$WAGGLE" --help 2>&1)"
for cmd in preflight identity compile modules provision; do
  echo "$HELP" | grep -q "  $cmd" && pass "command present: $cmd" || bad "command present: $cmd"
done

# Machine-readable output is a versioned envelope, on stdout, parseable in isolation.
for spec in "preflight:--root $REPO_ROOT preflight" \
            "modules:--root $REPO_ROOT modules" \
            "identity.list:--root $REPO_ROOT identity list"; do
  name="${spec%%:*}"; args="${spec#*:}"
  out="$("$WAGGLE" --format json $args 2>/dev/null)"
  if echo "$out" | python3 -c "
import json,sys
d=json.load(sys.stdin)
assert d.get('schema')=='waggle.v1', d.get('schema')
assert 'command' in d and 'ok' in d
" 2>/dev/null; then
    pass "$name emits a versioned envelope"
  else
    bad "$name emits a versioned envelope"
  fi
done

# stdout must stay parseable even when diagnostics are produced.
"$WAGGLE" --format json --root "$REPO_ROOT" compile --module tea --out "$TMP/p" >"$TMP/o" 2>"$TMP/e"
if python3 -c "import json;json.load(open('$TMP/o'))" 2>/dev/null; then
  pass "stdout stays parseable with stderr diagnostics present"
else
  bad "stdout stays parseable with stderr diagnostics present"
fi

# No command may require a terminal (AD-20). stdin closed, no tty.
if "$WAGGLE" --format json --root "$REPO_ROOT" modules </dev/null >/dev/null 2>&1; then
  pass "runs with stdin closed and no tty"
else
  bad "runs with stdin closed and no tty"
fi

# Exit-code taxonomy, end to end.
"$WAGGLE" --help >/dev/null 2>&1;                            [[ $? -eq 0 ]] && pass "0 = success" || bad "0 = success"
"$WAGGLE" --nonsense >/dev/null 2>&1;                        [[ $? -eq 1 ]] && pass "1 = user error" || bad "1 = user error"
"$WAGGLE" --root "$TMP" preflight >/dev/null 2>&1;           [[ $? -eq 1 ]] && pass "1 = missing input is user error" || bad "1 = missing input is user error"
"$WAGGLE" --root "$REPO_ROOT" compile --module nope --out "$TMP/x" >/dev/null 2>&1
[[ $? -eq 1 ]] && pass "1 = unknown module is user error" || bad "1 = unknown module is user error"

# An unknown module must say what IS available rather than just failing.
# Output is captured first: under `set -o pipefail` the command's non-zero exit would
# propagate through a pipe and fail the `if` even when grep matched.
msg="$("$WAGGLE" --root "$REPO_ROOT" compile --module nope --out "$TMP/x" 2>&1)"
if echo "$msg" | grep -q "Known modules"; then
  pass "unknown module names the known ones (NFR-4)"
else
  bad "unknown module names the known ones (NFR-4)"
fi

echo
if [[ $fail -eq 0 ]]; then echo "cli OK"; else echo "CLI DRIFT"; fi
exit $fail
