#!/usr/bin/env bash
# Exit-code taxonomy test for `waggle preflight` (Story 1.4, AD-18 + AD-20).
#
# The taxonomy is a public contract (PRD section 12), so it gets a test. In particular
# this pins the clap collision: clap exits 2 on usage errors by default, which would
# otherwise be indistinguishable from our "upstream contract error".
set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WAGGLE="${WAGGLE_BIN:-$REPO_ROOT/target/debug/waggle}"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

fail=0
check() { # <expected-code> <label> <args...>
  local want="$1"; local label="$2"; shift 2
  "$WAGGLE" "$@" >/dev/null 2>&1
  local got=$?
  if [[ "$got" == "$want" ]]; then
    echo "  PASS  [$got] $label"
  else
    echo "  FAIL  [want $want, got $got] $label"; fail=1
  fi
}

if [[ ! -x "$WAGGLE" ]]; then
  echo "waggle binary not found at $WAGGLE — run: cargo build --workspace"
  exit 3
fi

# A root whose declared range cannot match the checked-out substrate.
mkdir -p "$TMP/narrow"
cp -R "$REPO_ROOT/_bmad" "$TMP/narrow/"
sed -E 's/^BUZZ_SUPPORTED=.*/BUZZ_SUPPORTED=>=99.0.0,<100.0.0/' \
  "$REPO_ROOT/BUZZ_VERSION" > "$TMP/narrow/BUZZ_VERSION"

echo "waggle preflight — exit-code taxonomy"
check 0 "happy path"                         --root "$REPO_ROOT" preflight
check 0 "--help is a request, not a failure" --help
check 1 "unknown flag is USER, not clap's 2" preflight --nonsense
check 1 "missing pins file is USER"          --root "$TMP" preflight
check 2 "unsupported version is UPSTREAM"    --root "$TMP/narrow" preflight --substrate "$REPO_ROOT/vendor/buzz"
check 0 "--allow-unsupported overrides"      --root "$TMP/narrow" preflight --substrate "$REPO_ROOT/vendor/buzz" --allow-unsupported
check 2 "absent substrate is UPSTREAM"       --root "$TMP/narrow" preflight --substrate "$TMP/nope"

# The override must stay visible in machine output, not silently succeed.
if "$WAGGLE" --format json --root "$TMP/narrow" preflight \
     --substrate "$REPO_ROOT/vendor/buzz" --allow-unsupported 2>/dev/null \
     | grep -q '"overridden": true'; then
  echo "  PASS  override is reported in json output"
else
  echo "  FAIL  override is reported in json output"; fail=1
fi

echo
if [[ $fail -eq 0 ]]; then echo "taxonomy OK"; else echo "TAXONOMY DRIFT"; fi
exit $fail
