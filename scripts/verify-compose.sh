#!/usr/bin/env bash
# Story 1.3 — the hive bundle pins an immutable digest and never builds the image.
set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
COMPOSE="$REPO_ROOT/deploy/compose"
fail=0
pass() { echo "  PASS  $1"; }
bad()  { echo "  FAIL  $1"; fail=1; }

echo "waggle compose — pinned pull, no build"

[[ -f "$COMPOSE/compose.yml" ]] && pass "compose.yml present" || bad "compose.yml present"
[[ -f "$COMPOSE/.env.example" ]] && pass ".env.example present" || bad ".env.example present"
[[ -x "$COMPOSE/run.sh" ]] && pass "run.sh executable" || bad "run.sh executable"

# Must pin by digest, not a floating tag.
if grep -E 'image:.*ghcr\.io/block/buzz@sha256:[0-9a-f]{64}' "$COMPOSE/compose.yml" >/dev/null; then
  pass "relay image pinned by sha256 digest"
else
  bad "relay image pinned by sha256 digest"
fi

# Must not instruct operators to build.
if grep -Eiq 'docker build|dockerfile' "$COMPOSE/README.md" "$COMPOSE/compose.yml"; then
  bad "bundle must pull, not build"
else
  pass "bundle documents pull, not build"
fi

# BUZZ_VERSION must record the same digest.
DIGEST="$(grep -oE 'sha256:[0-9a-f]{64}' "$COMPOSE/compose.yml" | head -1)"
if grep -qF "$DIGEST" "$REPO_ROOT/BUZZ_VERSION"; then
  pass "BUZZ_VERSION records the pinned digest"
else
  bad "BUZZ_VERSION records the pinned digest ($DIGEST)"
fi

# Validate compose file parses (does not start anything).
# compose.yml references `env_file: .env`. Never overwrite an operator's real .env.
VERIFY_ENV="$COMPOSE/.env.waggle-verify"
sed 's/CHANGE_ME_[A-Z0-9_]*/deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef/g' \
  "$COMPOSE/.env.example" >"$VERIFY_ENV"
# Temporarily point compose at the throwaway file without touching a real .env.
# Prefer --env-file; also satisfy env_file: .env via a symlink only when .env is absent.
RESTORE_ENV=0
if [[ ! -e "$COMPOSE/.env" ]]; then
  ln -s .env.waggle-verify "$COMPOSE/.env"
  RESTORE_ENV=1
fi
trap 'rm -f "$COMPOSE/.env.waggle-verify"; if [[ "$RESTORE_ENV" -eq 1 ]]; then rm -f "$COMPOSE/.env"; fi' EXIT
if docker compose -f "$COMPOSE/compose.yml" --env-file "$VERIFY_ENV" config >/dev/null 2>&1 \
   || docker compose -f "$COMPOSE/compose.yml" config >/dev/null 2>&1; then
  pass "compose config validates (with placeholder secrets)"
else
  bad "compose config validates"
  docker compose -f "$COMPOSE/compose.yml" --env-file "$VERIFY_ENV" config 2>&1 | tail -8
fi
rm -f "$COMPOSE/.env.waggle-verify"
if [[ "$RESTORE_ENV" -eq 1 ]]; then
  rm -f "$COMPOSE/.env"
fi
trap - EXIT
echo
if [[ $fail -eq 0 ]]; then echo "compose OK"; else echo "COMPOSE DRIFT"; fi
exit $fail
