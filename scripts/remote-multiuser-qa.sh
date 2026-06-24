#!/usr/bin/env bash
# =============================================================================
# remote-multiuser-qa.sh — REMOTE / multi-user networked QA harness
#
# The companion runner for `crates/lore-vm/tests/remote_multiuser.rs`. Where
# `scripts/live-server-client.sh` (SBAI-4064 spike) proves a single happy-path
# round trip, THIS drives the full Perforce-class multi-user surface end to end:
#
#     sync (file UPDATE + DELETE propagation), push, a push CONFLICT,
#     two-user lock acquire / query / contention / release, and a file CONFLICT
#     state — TWO independent clones through ONE real `loreserver`, over the wire.
#
# The test (`tests/remote_multiuser.rs`, feature `remote-integration-tests`) is
# self-contained: it boots its OWN `loreserver` per test run from a resolved
# binary. This script's only job is to RESOLVE / BUILD that binary once and hand
# it to the test via `LOREVM_SERVER_BIN`, so the heavy upstream build happens
# here (and is cacheable) rather than implicitly inside the test.
#
# LOCAL-ONLY by design. The server binds 127.0.0.1 exclusively and runs auth
# disabled (no `[server.auth]` block). The test picks its own free loopback port.
#
# Usage:   scripts/remote-multiuser-qa.sh
# Env:
#   LOREVM_SERVER_BIN   pre-built loreserver to use (skips the build entirely)
#   SKIP_BUILD=1        do not build; require a resolvable binary or fail
#   CARGO_PROFILE       release|debug for the loreserver build (default: release)
# =============================================================================
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PROFILE="${CARGO_PROFILE:-release}"

# ---- resolve the loreserver binary -----------------------------------------
resolve_or_build_server() {
  # 1. explicit override wins.
  if [[ -n "${LOREVM_SERVER_BIN:-}" ]]; then
    if [[ -x "${LOREVM_SERVER_BIN}" ]]; then
      echo "${LOREVM_SERVER_BIN}"
      return 0
    fi
    echo "FATAL: LOREVM_SERVER_BIN=${LOREVM_SERVER_BIN} is not executable" >&2
    exit 1
  fi

  # 2. locate the pinned upstream lore checkout (loregui pins lore by rev).
  local rev short checkout bin
  rev="$(grep -oE 'rev = "[0-9a-f]{40}"' "${REPO_ROOT}/Cargo.toml" | head -1 | grep -oE '[0-9a-f]{40}')"
  if [[ -z "${rev}" ]]; then
    echo "FATAL: could not read pinned lore rev from Cargo.toml" >&2
    exit 1
  fi
  short="${rev:0:7}"
  checkout="$(find "${CARGO_HOME:-$HOME/.cargo}/git/checkouts" -maxdepth 2 -type d -name "${short}" 2>/dev/null | head -1)"
  if [[ -z "${checkout}" ]]; then
    # populate the cargo git cache so the checkout exists.
    ( cd "${REPO_ROOT}" && cargo fetch )
    checkout="$(find "${CARGO_HOME:-$HOME/.cargo}/git/checkouts" -maxdepth 2 -type d -name "${short}" 2>/dev/null | head -1)"
  fi
  if [[ -z "${checkout}" ]]; then
    echo "FATAL: lore checkout for rev ${short} not found under cargo git cache" >&2
    exit 1
  fi

  # 3. already built?
  for p in release debug; do
    bin="${checkout}/target/${p}/loreserver"
    [[ -x "${bin}" ]] && { echo "${bin}"; return 0; }
  done

  if [[ "${SKIP_BUILD:-0}" == "1" ]]; then
    echo "FATAL: no pre-built loreserver and SKIP_BUILD=1" >&2
    exit 1
  fi

  # 4. build it (slow first run — ~1 GB from the pinned checkout).
  echo "==> building loreserver (${PROFILE}) from pinned checkout: ${checkout}" >&2
  if [[ "${PROFILE}" == "release" ]]; then
    ( cd "${checkout}" && cargo build --release -p lore-server --bin loreserver ) >&2
    bin="${checkout}/target/release/loreserver"
  else
    ( cd "${checkout}" && cargo build -p lore-server --bin loreserver ) >&2
    bin="${checkout}/target/debug/loreserver"
  fi
  [[ -x "${bin}" ]] || { echo "FATAL: built loreserver not found at ${bin}" >&2; exit 1; }
  echo "${bin}"
}

SERVER_BIN="$(resolve_or_build_server)"
echo "==> loreserver: ${SERVER_BIN}"

# ---- run the feature-gated multi-user suite --------------------------------
# The test boots its own server from this binary, picks a free loopback port,
# and drives both clones. --nocapture surfaces the per-phase [A]/[B] trace.
echo "==> running remote_multiuser suite"
cd "${REPO_ROOT}"
LOREVM_SERVER_BIN="${SERVER_BIN}" \
  cargo test -p lore-vm --features remote-integration-tests \
    --test remote_multiuser -- --nocapture
RC=$?

echo
if [[ ${RC} -eq 0 ]]; then
  echo "RESULT: SUCCESS — remote multi-user QA verified (sync/delete/push-conflict/locks/conflict)."
else
  echo "RESULT: FAILURE (exit ${RC})."
fi
exit ${RC}
