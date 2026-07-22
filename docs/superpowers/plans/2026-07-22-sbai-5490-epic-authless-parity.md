# SBAI-5490 Epic Authless Parity Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Advance LoreGUI to Epic Lore `826ad5d20ff4f5814101c946df127cef8253ada3` and prove that remote user-info resolution against an authless server returns typed `NotSupported` code 18, never `NotAuthenticated` code 12.

**Architecture:** Keep LoreGUI as a thin exact-pin consumer. The root manifest and lockfile select the upstream implementation; a small exact-pin contract checker validates both dependency pins, lock sources, and the upstream source semantics. The existing live `loreserver` harness exercises both the public C-ABI `lore_auth_user_info` status and LoreGUI's Rust wrapper/wire rendering while frontend compatibility tests preserve the legacy and prior-nightly contracts.

**Tech Stack:** Rust workspace, Epic Lore git dependencies, Bash live harness, Node contract tests, Vitest/React frontend tests, cargo-about license generation.

## Global Constraints

- Both `lore` and `[patch.crates-io].quinn-proto` must use exact revision `826ad5d20ff4f5814101c946df127cef8253ada3`.
- The live C-ABI call must return `18` and explicitly prove it is not `12`; the gate may not skip when its artifacts are absent.
- The Rust wrapper must render exactly `Operation not supported: No authentication configured on server`.
- Preserve the exact legacy `No authentication configured on server`, prior-nightly rendered NotSupported, and `No auth endpoint available` classifiers.
- Unrelated and near-miss authentication errors remain visible and fail closed.
- Regenerate `Cargo.lock`, root/public Rust attribution bundles, and audit every pin consumer/comment.
- Do not modify or rebase `worker/sbai-5484-p1`; do not merge or transition Jira.

---

### Task 1: Exact dual-pin contract and negative fixtures

**Files:**
- Create: `scripts/exact-pin-authless-contract.mjs`
- Create: `scripts/exact-pin-authless-contract.test.mjs`
- Modify: `scripts/live-server-client.sh`
- Modify: `.github/workflows/integration.yml`

**Interfaces:**
- Produces: `verifyExactPin(repoRoot, expectedRev)` which throws on a missing/mismatched manifest pin, lock source, checkout provenance, or upstream auth semantic.
- Consumes: root `Cargo.toml`, `Cargo.lock`, and Cargo's exact git checkout for Epic Lore.

- [ ] **Step 1: Write failing contract fixtures**

  Add executable tests with temporary manifests/locks for the exact dual pin, a missing quinn pin, a wrong lore pin, a wrong lock source, and a target checkout whose exchange source does not contain the canonical typed NotSupported operation.

- [ ] **Step 2: Verify RED**

  Run `node --test scripts/exact-pin-authless-contract.test.mjs`.
  Expected: failure because `scripts/exact-pin-authless-contract.mjs` does not exist.

- [ ] **Step 3: Implement the minimal contract checker**

  Parse both manifest entries independently, require exact full-SHA equality, require `lore` and `quinn-proto` lock sources at that revision, resolve the Cargo checkout by full `git rev-parse HEAD`, and require both auth exchange branches plus all three user-info forwarders from the target source.

- [ ] **Step 4: Make the live harness consume the checker**

  Run the checker before building any artifact. Keep the live harness non-skippable and add it as a required integration workflow step with all checker/harness/example paths in the workflow trigger.

- [ ] **Step 5: Verify GREEN**

  Run `node --test scripts/exact-pin-authless-contract.test.mjs scripts/upstream-lore-parity.test.mjs`.
  Expected: all fixtures pass, including asserted non-zero exits for missing/wrong pins.

### Task 2: Public C-ABI and Rust wire canaries

**Files:**
- Modify: `crates/lore-vm/examples/live_server_client.rs`
- Test: `scripts/live-server-client.sh`

**Interfaces:**
- Consumes: `lore::interface::lore_auth_user_info` and `lore_vm::ops::auth::resolve_user_info::resolve_user_info`.
- Produces: live assertions for status `18 != 12` and canonical wire text.

- [ ] **Step 1: Write the failing live assertions**

  After creating the remote-backed repository, call the public C-ABI entry point with a non-current user and assert `status == 18` and `status != 12`. Call the LoreGUI Rust wrapper with the same identity and assert the exact `CommandFailed` message.

- [ ] **Step 2: Verify RED at the old pin**

  Run `scripts/live-server-client.sh` on `9179c6dc7cd14931af5b66beb3b2e186907f6360`.
  Expected: the C-ABI assertion reports code 12 instead of 18, proving the regression canary distinguishes the upstream change.

- [ ] **Step 3: Verify GREEN at the target pin**

  Run `scripts/live-server-client.sh` after the dual pin update.
  Expected: C-ABI `authUserInfo` prints code 18 and not 12; Rust wrapper prints the canonical wire string; the full network create/push/clone round trip remains green.

### Task 3: Compatibility classifier preservation

**Files:**
- Modify: `frontend/src/api.ts`
- Modify: `frontend/src/api.spec.ts`
- Modify: `frontend/src/AccountPanel.spec.tsx`
- Modify: `frontend/src/onboarding/ClientConnect.spec.tsx`
- Modify: `crates/lore-vm/examples/live_server_client.rs`

**Interfaces:**
- Consumes: three exact compatibility strings from #404, #414, and SBAI-5478.
- Produces: narrow predicates that accept only those exact strings.

- [ ] **Step 1: Add target-pin regression cases before comment updates**

  Assert the target-pin user-info error is neutral in identity loaders, while suffixed/prefixed NotSupported messages, bare NotSupported, connection failures, and I/O errors stay visible.

- [ ] **Step 2: Verify focused tests**

  Run `npm --prefix frontend run test:vitest -- src/api.spec.ts src/AccountPanel.spec.tsx src/onboarding/ClientConnect.spec.tsx`.
  Expected: exact legacy/nightly/target strings pass and unrelated cases remain rejected.

- [ ] **Step 3: Update only stale provenance comments**

  Record `826ad5d20` as the pin establishing forwarded user-info/exchange semantics; do not broaden predicate matching.

### Task 4: Pin, lock, and attribution regeneration

**Files:**
- Modify: `Cargo.toml`
- Modify: `Cargo.lock`
- Modify: `THIRD-PARTY-LICENSES-RUST.md`
- Modify: `frontend/public/licenses/rust.md`
- Modify: pin-specific comments found by repository-wide search

**Interfaces:**
- Produces: one dependency graph whose lore-family and quinn-proto sources resolve to the target full SHA.

- [ ] **Step 1: Update both exact pins**

  Change only the two Epic Lore revisions and their provenance comments.

- [ ] **Step 2: Regenerate the lockfile**

  Run `cargo update -p lore --precise 826ad5d20ff4f5814101c946df127cef8253ada3` and verify every Epic Lore/quinn source ends in `#826ad5d20ff4f5814101c946df127cef8253ada3`.

- [ ] **Step 3: Regenerate licenses**

  Run `bash scripts/gen-licenses.sh`; verify only dependency-derived Rust attribution changes are produced.

- [ ] **Step 4: Sweep all consumers**

  Search for `9179c6d`, stale pin labels, dual-pin extractors, and authless operation strings. Classify every remaining old SHA reference as intentional history or update it.

### Task 5: Scoped and full verification

**Files:**
- Verify all files changed above.

- [ ] **Step 1: Focused schema/boundary/parity/license gates**

  Run exact-pin contract tests, `scripts/upstream-lore-parity.mjs --json`, `scripts/check-open-boundary.sh`, license regeneration followed by `git diff --exit-code` on generated bundles, and `cargo metadata --locked`.

- [ ] **Step 2: Rust surface gates**

  Run `cargo test -p lore-vm`, `cargo test -p lorevm-cli`, `cargo test -p lorevm-ffi`, and the feature-scoped lore-vm integration tests required by `.github/workflows/integration.yml`.

- [ ] **Step 3: Frontend compatibility gates**

  Run focused auth tests, frontend typecheck, and the full frontend test suite.

- [ ] **Step 4: Live authless proof**

  Run `scripts/live-server-client.sh` with no skip flags and retain output proving public C-ABI code 18, inequality to 12, canonical Rust wire text, and network round trip.

- [ ] **Step 5: Adversarial audit and freeze**

  Run `cargo fmt --all --check`, `git diff --check`, inspect the complete base-to-head diff, verify `Cargo.lock --locked` behavior, and re-run the exact canary after deliberately exercising its wrong-pin fixture.

- [ ] **Step 6: Publish without merge**

  Commit with SBAI-5490, push `worker/sbai-5490`, create a non-draft PR titled `SBAI-5490: advance Epic authless user-info parity to 826ad5d20`, and report the immutable head plus exact evidence. Do not merge or transition Jira.
