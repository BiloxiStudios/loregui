#!/usr/bin/env bash
# Regenerate ALL third-party license attribution bundles for the distributed
# LoreGUI binary, and stage in-app copies under frontend/public/licenses/.
#
#   scripts/gen-licenses.sh
#
# Produces / refreshes:
#   THIRD-PARTY-LICENSES-RUST.md        (cargo-about: about.toml + about.hbs)
#   THIRD-PARTY-LICENSES-FRONTEND.md    (frontend/scripts/gen-third-party-licenses.mjs)
#   frontend/public/licenses/rust.md        (in-app copy)
#   frontend/public/licenses/frontend.md    (in-app copy)
#
# Both underlying generators FAIL if a non-permissive (GPL/AGPL/LGPL/SSPL/...)
# dependency is introduced, so running this is also the license-policy gate.
# CI (.github/workflows/licenses.yml) runs this and diffs the result to prevent
# drift.
set -euo pipefail

cd "$(dirname "$0")/.."
ROOT="$(pwd)"

echo "==> Rust licenses (cargo-about)"
if ! command -v cargo-about >/dev/null 2>&1; then
  echo "cargo-about not found; installing (cargo install cargo-about)..."
  cargo install cargo-about --locked
fi
cargo about generate --all-features about.hbs -o "$ROOT/THIRD-PARTY-LICENSES-RUST.md"

echo "==> Frontend licenses (license-checker)"
# Ensure frontend deps are present for the scan.
if [ ! -d "$ROOT/frontend/node_modules" ]; then
  echo "frontend/node_modules missing; running npm ci..."
  npm --prefix "$ROOT/frontend" ci
fi
node "$ROOT/frontend/scripts/gen-third-party-licenses.mjs"

echo "==> Staging in-app copies under frontend/public/licenses/"
mkdir -p "$ROOT/frontend/public/licenses"
cp "$ROOT/THIRD-PARTY-LICENSES-RUST.md" "$ROOT/frontend/public/licenses/rust.md"
# frontend.md is already written by the .mjs generator; re-copy defensively.
cp "$ROOT/THIRD-PARTY-LICENSES-FRONTEND.md" "$ROOT/frontend/public/licenses/frontend.md"

echo "==> Done. Bundles regenerated."
