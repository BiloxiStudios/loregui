#!/usr/bin/env bash
# Demonstrate that the required service-CWD proof fails closed on bad fixtures.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RUNNER="$ROOT/scripts/exact-pin-service-cwd-canary.sh"

if LOREVM_LORE_BIN="$ROOT/target/definitely-missing-lore" "$RUNNER" --verify-only; then
  echo "FAIL: missing explicit lore fixture was accepted" >&2
  exit 1
fi

if LOREVM_LORE_BIN=/bin/true "$RUNNER" --verify-only; then
  echo "FAIL: executable without exact-pin checkout provenance was accepted" >&2
  exit 1
fi

echo "exact-pin service-CWD negative fixture tests passed"
