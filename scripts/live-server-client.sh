#!/usr/bin/env bash
# =============================================================================
# live-server-client.sh — SBAI-4064 spike
#
# Boots a REAL lore QUIC/gRPC server (upstream `loreserver`) on localhost with
# local in-process stores, the shipped test certs, and NO auth, then drives the
# full networked client loop end-to-end:
#
#     connect → create → commit → push  (client A)  →  clone → verify (client B)
#
# Client B clones from the SERVER into its own separate store, so a successful
# verify proves a genuine network round trip (not a shared-local-store shortcut).
#
# LOCAL-ONLY. Binds 127.0.0.1 exclusively. Not wired into CI — the lore server
# is built from the pinned upstream `lore` git checkout, which CI does not have.
#
# Usage:   scripts/live-server-client.sh
# Env:     LORE_PORT (default 41337)   KEEP_TMP=1 to leave temp dirs for inspection
# =============================================================================
set -euo pipefail

PORT="${LORE_PORT:-41337}"
HTTP_PORT=$((PORT + 2))
REPO_NAME="spikerepo-$$"
REPO_URL="lore://127.0.0.1:${PORT}/${REPO_NAME}"
SPIKE_DIR="$(mktemp -d /tmp/lore-spike.XXXXXX)"
SERVER_LOG="${SPIKE_DIR}/server.log"
SERVER_PID=""

# ---- locate the pinned upstream lore checkout (for the loreserver binary) ---
# loregui pins `lore` by rev in Cargo.toml; cargo unpacks it under the git cache.
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LORE_REV="$(grep -oE 'rev = "[0-9a-f]{40}"' "${REPO_ROOT}/Cargo.toml" | head -1 | grep -oE '[0-9a-f]{40}')"
if [[ -z "${LORE_REV}" ]]; then
  echo "FATAL: could not read pinned lore rev from Cargo.toml" >&2
  exit 1
fi
SHORT_REV="${LORE_REV:0:7}"
LORE_CHECKOUT="$(find "${CARGO_HOME:-$HOME/.cargo}/git/checkouts" -maxdepth 2 -type d -name "${SHORT_REV}" 2>/dev/null | head -1)"
if [[ -z "${LORE_CHECKOUT}" ]]; then
  echo "FATAL: lore checkout for rev ${SHORT_REV} not found under cargo git cache." >&2
  echo "       Run a build that fetches the dep first (e.g. cargo fetch)." >&2
  exit 1
fi
echo "lore checkout: ${LORE_CHECKOUT}"

cleanup() {
  if [[ -n "${SERVER_PID}" ]] && kill -0 "${SERVER_PID}" 2>/dev/null; then
    echo "stopping server (pid ${SERVER_PID})"
    kill "${SERVER_PID}" 2>/dev/null || true
    wait "${SERVER_PID}" 2>/dev/null || true
  fi
  if [[ "${KEEP_TMP:-0}" == "1" ]]; then
    echo "KEEP_TMP=1 — leaving ${SPIKE_DIR} (server log: ${SERVER_LOG})"
  else
    rm -rf "${SPIKE_DIR}"
  fi
}
trap cleanup EXIT INT TERM

# ---- 1. build the server binary + the client example -----------------------
echo "==> building loreserver (upstream, from pinned checkout)"
( cd "${LORE_CHECKOUT}" && cargo build -p lore-server --bin loreserver )
LORESERVER="${LORE_CHECKOUT}/target/debug/loreserver"

echo "==> building lore-vm live_server_client example"
( cd "${REPO_ROOT}" && cargo build -p lore-vm --example live_server_client )
# Resolve the example binary regardless of workspace target dir layout.
EXAMPLE="$(find "${REPO_ROOT}/target/debug/examples" -maxdepth 1 -name live_server_client -type f 2>/dev/null | head -1)"
if [[ -z "${EXAMPLE}" ]]; then
  echo "FATAL: built example not found under target/debug/examples" >&2
  exit 1
fi

# ---- 2. write a localhost-only server config --------------------------------
# Single-node, local stores under SPIKE_DIR/store, shipped test certs for QUIC,
# NO [server.auth] block → JWT verification disabled (auth: None).
mkdir -p "${SPIKE_DIR}/config" "${SPIKE_DIR}/store"
TEST_CERT="${LORE_CHECKOUT}/lore-server/src/protocol/test_data/test_cert.pem"
TEST_KEY="${LORE_CHECKOUT}/lore-server/src/protocol/test_data/test_key.pem"
cat > "${SPIKE_DIR}/config/local.toml" <<EOF
[server.quic]
host = "127.0.0.1"
port = ${PORT}
[server.quic.certificate]
cert_file = "${TEST_CERT}"
pkey_file = "${TEST_KEY}"

[server.grpc]
host = "127.0.0.1"
port = ${PORT}

[server.http]
host = "127.0.0.1"
port = ${HTTP_PORT}

[immutable_store.local]
path = "${SPIKE_DIR}/store"
[mutable_store.local]
path = "${SPIKE_DIR}/store"

[telemetry.logger]
format = "ansi"

[topology]
provider = "none"
EOF

# ---- 3. boot the server -----------------------------------------------------
echo "==> starting loreserver on 127.0.0.1:${PORT} (gRPC+QUIC), http ${HTTP_PORT}"
# `exec` so the subshell becomes the server process itself — then $! is the real
# server pid and the EXIT trap can reliably reap it (no orphaned grandchild).
( cd "${SPIKE_DIR}" && exec env LORE_CONFIG_PATH="${SPIKE_DIR}/config" LORE_ENV=local \
    "${LORESERVER}" > "${SERVER_LOG}" 2>&1 ) &
SERVER_PID=$!

# wait for the gRPC + QUIC sockets to come up (or the process to die)
echo -n "==> waiting for server to listen"
for _ in $(seq 1 60); do
  if ! kill -0 "${SERVER_PID}" 2>/dev/null; then
    echo; echo "FATAL: server exited during startup. Log tail:" >&2
    tail -30 "${SERVER_LOG}" >&2
    exit 1
  fi
  if ss -tln 2>/dev/null | grep -q "127.0.0.1:${PORT}" \
     && ss -uln 2>/dev/null | grep -q "127.0.0.1:${PORT}"; then
    echo " — up (tcp+udp ${PORT})"
    break
  fi
  echo -n "."
  sleep 0.5
done

# ---- 4. drive the networked client loop -------------------------------------
echo "==> running client loop: ${REPO_URL}"
CLIENT_A="${SPIKE_DIR}/clientA"
CLIENT_B="${SPIKE_DIR}/clientB"
mkdir -p "${CLIENT_A}" "${CLIENT_B}"

set +e
"${EXAMPLE}" "${REPO_URL}" "${CLIENT_A}" "${CLIENT_B}"
RC=$?
set -e

echo
if [[ ${RC} -eq 0 ]]; then
  echo "RESULT: SUCCESS — live networked round trip verified."
else
  echo "RESULT: FAILURE (exit ${RC}). Server log tail:"
  tail -40 "${SERVER_LOG}"
fi
exit ${RC}
