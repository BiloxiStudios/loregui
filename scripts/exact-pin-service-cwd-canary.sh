#!/usr/bin/env bash
# Required exact-pin Epic Lore service/caller-CWD compatibility proof (SBAI-5488).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

REV="$(grep -oE 'rev = "[0-9a-f]{40}"' Cargo.toml | head -1 | grep -oE '[0-9a-f]{40}')"
if [[ ! "$REV" =~ ^[0-9a-f]{40}$ ]]; then
  echo "FATAL: could not read exact Epic Lore pin from Cargo.toml" >&2
  exit 1
fi
SHORT="${REV:0:7}"

# An explicit fixture is a contract, not a hint: missing input must not fall
# through to a cached checkout artifact and accidentally green the proof.
if [[ -n "${LOREVM_LORE_BIN:-}" && ! -x "$LOREVM_LORE_BIN" ]]; then
  echo "FATAL: required LOREVM_LORE_BIN is missing or not executable: $LOREVM_LORE_BIN" >&2
  exit 1
fi

cargo fetch --locked
CARGO_HOME_DIR="${CARGO_HOME:-$HOME/.cargo}"
CHECKOUT=""
while IFS= read -r candidate; do
  if [[ "$(git -C "$candidate" rev-parse HEAD 2>/dev/null || true)" == "$REV" ]]; then
    CHECKOUT="$candidate"
    break
  fi
done < <(find "$CARGO_HOME_DIR/git/checkouts" -mindepth 2 -maxdepth 2 -type d -name "$SHORT" 2>/dev/null | sort)

if [[ -z "$CHECKOUT" ]]; then
  echo "FATAL: exact Epic Lore checkout $REV was not provisioned by cargo fetch" >&2
  exit 1
fi

if [[ -n "${LOREVM_LORE_BIN:-}" ]]; then
  BIN="$LOREVM_LORE_BIN"
else
  cargo build --manifest-path "$CHECKOUT/Cargo.toml" -p lore-client --bin lore
  BIN="$CHECKOUT/target/debug/lore"
fi

BIN_REAL="$(realpath "$BIN")"
CHECKOUT_REAL="$(realpath "$CHECKOUT")"
case "$BIN_REAL" in
  "$CHECKOUT_REAL"/target/debug/lore|"$CHECKOUT_REAL"/target/release/lore) ;;
  *)
    echo "FATAL: lore binary lacks exact-pin checkout provenance: $BIN_REAL" >&2
    exit 1
    ;;
esac

ACTUAL_REV="$(git -C "$CHECKOUT_REAL" rev-parse HEAD)"
if [[ "$ACTUAL_REV" != "$REV" ]]; then
  echo "FATAL: lore binary checkout is $ACTUAL_REV, expected $REV" >&2
  exit 1
fi
VERSION="$($BIN_REAL --version)"
if [[ "$VERSION" != lore\ 0.8.6-nightly* ]]; then
  echo "FATAL: unexpected lore binary version for $REV: $VERSION" >&2
  exit 1
fi

echo "exact-pin lore fixture: rev=$ACTUAL_REV binary=$BIN_REAL version=$VERSION"
if [[ "${1:-}" == "--verify-only" ]]; then
  exit 0
fi
if [[ $# -ne 0 ]]; then
  echo "FATAL: unsupported argument: $1" >&2
  exit 1
fi

LOREVM_LORE_BIN="$BIN_REAL" \
  cargo test -p lore-vm --features integration-tests \
    --test service_unix_smoke \
    unix_service_resolves_relative_repository_against_caller_root \
    -- --exact --nocapture
