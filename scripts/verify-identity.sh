#!/usr/bin/env bash
# Secret-hygiene and idempotence tests for `waggle identity` (Story 1.5, AD-14 + NFR-7).
#
# NFR-7 has no remediation if it fails once, so it gets an end-to-end test against the
# real binary rather than only unit tests on the library.
set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WAGGLE="${WAGGLE_BIN:-$REPO_ROOT/target/debug/waggle}"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

fail=0
pass() { echo "  PASS  $1"; }
bad()  { echo "  FAIL  $1"; fail=1; }

if [[ ! -x "$WAGGLE" ]]; then
  echo "waggle binary not found at $WAGGLE — run: cargo build --workspace"
  exit 3
fi

echo "waggle identity — secret hygiene"

"$WAGGLE" --root "$TMP" identity provision --role tea >"$TMP/out" 2>"$TMP/err"
[[ $? -eq 0 ]] && pass "provision succeeds" || bad "provision succeeds"

SECRET="$(tr -d '\n' < "$TMP/keys/tea.nsec")"
[[ -n "$SECRET" ]] || bad "secret file is non-empty"

# The one that matters.
if grep -qF "$SECRET" "$TMP/out" "$TMP/err" 2>/dev/null; then
  bad "secret must not appear on stdout or stderr"
else
  pass "secret never printed to stdout or stderr"
fi

for fmt in text json; do
  if "$WAGGLE" --format "$fmt" --root "$TMP" identity list 2>&1 | grep -qF "$SECRET"; then
    bad "secret must not appear in '$fmt' listing"
  else
    pass "secret absent from '$fmt' listing"
  fi
done

if [[ "$(uname)" != "Darwin" && "$(uname)" != "Linux" ]]; then
  echo "  SKIP  permission check (non-unix)"
else
  mode="$(stat -f '%Lp' "$TMP/keys/tea.nsec" 2>/dev/null || stat -c '%a' "$TMP/keys/tea.nsec")"
  [[ "$mode" == "600" ]] && pass "secret file is 0600" || bad "secret file is 0600 (got $mode)"
fi

# Idempotence: re-provisioning must not silently orphan a key's history.
BEFORE="$(cat "$TMP/keys/tea.pub")"
"$WAGGLE" --root "$TMP" identity provision --role tea >/dev/null 2>&1
code=$?
AFTER="$(cat "$TMP/keys/tea.pub")"
[[ $code -eq 1 ]] && pass "re-provision refuses with USER exit" || bad "re-provision refuses with USER exit (got $code)"
[[ "$BEFORE" == "$AFTER" ]] && pass "existing key untouched" || bad "existing key untouched"

"$WAGGLE" --root "$TMP" identity provision --role tea --force >/dev/null 2>&1
FORCED="$(cat "$TMP/keys/tea.pub")"
[[ "$BEFORE" != "$FORCED" ]] && pass "--force replaces the key" || bad "--force replaces the key"

# Distinct roles must never share a key.
"$WAGGLE" --root "$TMP" identity provision --role dev >/dev/null 2>&1
[[ "$(cat "$TMP/keys/dev.pub")" != "$FORCED" ]] && pass "roles get distinct keys" || bad "roles get distinct keys"

# Path traversal via role name.
if "$WAGGLE" --root "$TMP" identity provision --role "../escape" >/dev/null 2>&1; then
  bad "rejects role names that escape the key dir"
else
  pass "rejects role names that escape the key dir"
fi

echo
if [[ $fail -eq 0 ]]; then echo "identity OK"; else echo "IDENTITY DRIFT"; fi
exit $fail
