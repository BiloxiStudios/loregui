#!/usr/bin/env bash
#
# check-open-boundary.sh — EW.6 / SBAI-4236 boundary CI guard (R9 control).
#
# loregui is the OPEN (MIT) repo. The accounts security boundary
# (../CLAUDE.md "Security Boundary — Accounts Isolation"; ADR-0001 §3.2) forbids
# accounts/PII/proprietary code from ever landing in the open build:
#
#   * NO import of the accounts frontend (@biloxistudios/studiobrain-accounts-frontend)
#   * NO copied accounts / PII source (studiobrain_accounts, Stripe/Fernet secret material)
#   * NO IMPORT of the proprietary loregui-cloud overlay (the open core only ever
#     ships STUB seams that a loregui-cloud build replaces — it must never import
#     the overlay itself).
#
# This is a deny-list grep. It scans SOURCE only (not docs, not this script, not
# the deny-list) and distinguishes a documented seam (a comment/string that
# MENTIONS loregui-cloud to describe the overlay) from an actual LEAK (an
# import/require/from of forbidden code).
#
# Exit 0 = clean. Exit 1 = a boundary leak was found.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

# Directories that hold scannable open-build SOURCE.
SCAN_DIRS=(
  frontend/src
  src-tauri/src
  crates
  lore-mcp
  vscode-extension/src
  unreal-plugin
)

# File extensions to scan.
EXTS=(ts tsx js jsx mjs cjs rs)

# ---------------------------------------------------------------------------
# Deny rules. Each is "LABEL|||REGEX" (extended regex, case-insensitive).
# Keep these tight enough to avoid flagging the legitimate documented seam.
# ---------------------------------------------------------------------------
RULES=(
  # 1. Importing the accounts frontend package (any path under it).
  "accounts-frontend import|||(import|export|require)[^\n;]*['\"]@biloxistudios/studiobrain-accounts-frontend"
  # 2. Importing accounts source/UI by any specifier or relative path.
  "accounts source import|||(import|export|require|from)[^\n;]*['\"][^'\"]*(studiobrain-accounts|studiobrain_accounts|/accounts/src/)"
  # 3. IMPORTING the proprietary loregui-cloud overlay (NOT merely mentioning it).
  #    Matches an import/require/from whose module specifier contains loregui-cloud.
  "loregui-cloud overlay import|||(import|export|require|from)[^\n;]*['\"][^'\"]*loregui-cloud[^'\"]*['\"]"
  # 4. Hard-coded accounts/PII secret material that must never live in the open build.
  "secret material|||(JWT_PRIVATE_KEY|STRIPE_SECRET_KEY|STRIPE_WEBHOOK_SECRET|sk_live_|sk_test_|ACCOUNTS_SERVICE_SECRET|AUTH_DATABASE_URL)"
  # 5. Embedding accounts billing/identity UI components by name (bundling, not iframe).
  "accounts UI component|||(import|export)[^\n;]*\\b(BillingSettings|TeamSettings|ApiKeySettings|SsoSettings|StripeCheckout|PasswordReset|MfaEnroll)\\b[^\n;]*from"
)

# Paths to exclude even within scan dirs (generated, vendored, tests-of-this-guard).
EXCLUDE_RE='(/node_modules/|/dist/|/target/|/\.next/|/__generated__/|\.d\.ts$)'

# Build the find expression for extensions.
find_args=()
for d in "${SCAN_DIRS[@]}"; do
  [ -e "$d" ] && find_args+=("$d")
done
if [ ${#find_args[@]} -eq 0 ]; then
  echo "boundary-guard: no scan dirs present; nothing to check."
  exit 0
fi

# Collect candidate files.
mapfile -t FILES < <(
  find "${find_args[@]}" -type f \
    \( $(printf -- '-name *.%s -o ' "${EXTS[@]}" | sed 's/ -o $//') \) 2>/dev/null \
    | grep -vE "$EXCLUDE_RE" || true
)

if [ ${#FILES[@]} -eq 0 ]; then
  echo "boundary-guard: no source files matched; nothing to check."
  exit 0
fi

violations=0
for rule in "${RULES[@]}"; do
  label="${rule%%|||*}"
  regex="${rule##*|||}"
  # grep -nIE: line numbers, skip binary, extended regex; -i case-insensitive.
  if hits="$(grep -rnIiE "$regex" "${FILES[@]}" 2>/dev/null)"; then
    if [ -n "$hits" ]; then
      echo "::error::BOUNDARY LEAK [$label]"
      echo "$hits" | sed 's/^/    /'
      echo
      violations=$((violations + 1))
    fi
  fi
done

if [ "$violations" -gt 0 ]; then
  echo "boundary-guard: FAILED — $violations rule(s) matched. See ADR-0001 §3.2 / the accounts security boundary."
  echo "If a match is a legitimate documented seam (a comment describing the overlay, NOT an import),"
  echo "refactor it to a stub seam or narrow the rule — do not weaken the guard to pass a real leak."
  exit 1
fi

echo "boundary-guard: OK — no accounts/PII/proprietary leak in the open build (${#FILES[@]} files scanned)."
exit 0
