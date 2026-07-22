# LoreGUI P2 Deterministic Qwen Surface QA Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace false-green exploratory clicking with a deterministic, fixture-owned LoreGUI surface test system that lets local Qwen classify visual results without inventing actions or destructive targets.

**Architecture:** A versioned test-map compiler inventories state-dependent actions and validates selectors, risks, oracles, and cleanup. A deterministic executor performs DOM or Windows UI Automation actions, captures IPC/state/filesystem/process/network evidence, and gives Qwen only bounded action IDs and screenshots for anomaly classification.

**Tech Stack:** TypeScript, JSON Schema, WebDriverIO/tauri-driver, Windows UI Automation, Rust Tauri test commands, Qwen 3.6-VL.

## Global Constraints

- P0 and P1 context APIs are the system under test; Qwen does not define product state.
- Qwen cannot invent selectors, coordinates, paths, URLs, credentials, or commands.
- Destructive and external actions default-deny.
- Fixture-owned paths and loopback endpoints carry per-run ownership tokens.
- Screenshots support but never replace IPC, state, filesystem, process, network, and remote-client oracles.
- Required-lane skips are reported as non-green SKIP, never success.

---

### Task 1: Define and compile the machine-readable surface map

**Files:**
- Create: `frontend/e2e/surface/schema.json`
- Create: `frontend/e2e/surface/types.ts`
- Create: `frontend/e2e/surface/compile.ts`
- Create: `frontend/e2e/surface/compile.spec.ts`
- Create: `frontend/e2e/surface/map/core.yaml`

**Interfaces:**
- Produces: `CompiledSurfaceMap` and `compileSurfaceMap(files): CompiledSurfaceMap`.
- Consumes: semantic `data-qa` IDs and command-palette manifest IDs.

- [ ] **Step 1: Write compiler rejection tests**

Reject duplicate action IDs, selectors resolving to zero or multiple elements,
missing success/error/cancel variants, destructive actions without ownership and
cleanup, missing oracles, and inventory entries with no case.

- [ ] **Step 2: Implement the schema**

Require:

```ts
export interface SurfaceAction {
  schema: 1;
  app_commit: string;
  surface_id: string;
  state_id: string;
  element_id: string;
  selector: { strategy: "qa" | "role" | "uia" | "manifest"; value: string };
  visible_name: string;
  preconditions: string[];
  risk: "read" | "write_reversible" | "destructive" | "external";
  expected_ipc: Array<{ command: string; args_match: Record<string, unknown> }>;
  oracles: Array<"dom" | "ipc" | "state" | "filesystem" | "process" | "network" | "accessibility" | "screenshot">;
  cleanup: string | null;
  platform: Array<"linux" | "windows" | "macos">;
}
```

- [ ] **Step 3: Compile the first P0/P1 inventory and commit**

Run `cd frontend && npx vitest run e2e/surface/compile.spec.ts` and expect PASS.
Then commit the schema, compiler, tests, and core map.

### Task 2: Add stable selectors, IPC trace, and state snapshot seams

**Files:**
- Create: `frontend/src/testing/qa.ts`
- Modify: `frontend/src/App.tsx`
- Modify: `frontend/src/onboarding/ModeSelect.tsx`
- Modify: `frontend/src/onboarding/OnboardingFlow.tsx`
- Modify: `frontend/src/onboarding/ClientConnect.tsx`
- Modify: `frontend/src/onboarding/ClientClone.tsx`
- Modify: `frontend/src/onboarding/server/BackendPicker.tsx`
- Modify: `frontend/src/onboarding/server/ValidateConnectivity.tsx`
- Modify: `frontend/src/onboarding/server/InitStore.tsx`
- Modify: `frontend/src/onboarding/server/ServiceSetup.tsx`
- Modify: `frontend/src/servers/ServerHub.tsx`
- Modify: `frontend/src/servers/ServerCard.tsx`
- Modify: `frontend/src/servers/RepositoryBrowser.tsx`
- Create: `src-tauri/src/e2e.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/src/ipc_harness_tests.rs`

**Interfaces:**
- Produces: semantic `data-qa` selectors in test and production builds.
- Produces test-only `e2e_reset`, `e2e_snapshot_state`, `e2e_seed`, and `e2e_set_fault` commands.
- Produces redacted IPC events `{command, args, started_at, completed_at, result_kind}`.

- [ ] **Step 1: Write tests proving snapshot/reset and redaction**

Seed a project/server, snapshot, reset, and assert empty state. Invoke an auth
command with token-like input and assert the IPC log contains `[REDACTED]` and
not the secret.

- [ ] **Step 2: Implement feature-gated E2E commands**

Compile them only under `test` or the existing E2E feature/config. Restrict all
paths to the run root passed at process launch. Fault names are a closed enum,
not arbitrary strings.

- [ ] **Step 3: Add selectors from the generated map and commit**

The compiler must prove every selector resolves exactly once in its fixture
state. Run frontend tests, Rust harness tests, typecheck, and commit.

### Task 3: Build deterministic fixtures and ownership safety

**Files:**
- Create: `frontend/e2e/fixtures/manager.ts`
- Create: `frontend/e2e/fixtures/manager.spec.ts`
- Create: `frontend/e2e/fixtures/states.ts`
- Reuse: `scripts/remote-multiuser-qa.sh`

**Interfaces:**
- Produces: `createFixture(stateId)`, `snapshotFixture`, and `destroyFixture`.
- Produces: per-run `ownershipToken`, isolated app profile, explicit CWD, ports, roots, and credential cache.

- [ ] **Step 1: Write ownership escape and cleanup tests**

Reject parent traversal, symlink escape, non-loopback external URLs, missing
ownership token, and deletion outside the fresh temp root. Prove cleanup leaves
no process, port, file, or credential artifact.

- [ ] **Step 2: Implement required fixtures**

Implement `fresh_no_repo_no_auth`, `shell_no_repo`, `local_repo_clean`,
`local_repo_dirty`, `local_host_no_auth`, `remote_two_user_no_auth`,
`remote_auth`, `host_running`, `corrupt_repo`, `network_down_mid_action`,
`lock_inbox_pending`, and `settings_persisted`.

- [ ] **Step 3: Run fixture tests and commit**

Run `cd frontend && npx vitest run e2e/fixtures/manager.spec.ts`. Expected: PASS.
Commit fixture manager and state definitions.

### Task 4: Implement the bounded executor and Qwen protocol

**Files:**
- Create: `frontend/e2e/surface/executor.ts`
- Create: `frontend/e2e/surface/qwenProtocol.ts`
- Create: `frontend/e2e/surface/executor.spec.ts`
- Create: `frontend/e2e/surface/run.ts`

**Interfaces:**
- Produces: executor input `{action_id, parameters}` and bounded observation output.
- Consumes: compiled map, fixture manager, WebDriver, IPC/state trace, and local Qwen CLI.

- [ ] **Step 1: Write protocol rejection tests**

Reject unknown action IDs, extra parameter keys, coordinates, selectors, paths,
URLs, and observation codes not declared by the action.

- [ ] **Step 2: Implement strict JSON protocol**

Qwen receives only allowed actions and fixture parameter keys. Require output:

```json
{"action_id":"server.repository.clone","parameters":{"destination_ref":"clone_root"},"observation_code":"expected"}
```

The executor, not Qwen, resolves `clone_root`, performs the action, waits for
declared state, captures oracles, and decides pass/fail. Qwen may return only a
declared visual observation code.

- [ ] **Step 3: Prove the oracle can fail**

Seed one wrong IPC command, one wrong filesystem diff, and one mismatched visual
state. Each must fail the case. Restore the correct oracle and pass.

- [ ] **Step 4: Run tests and commit**

Run `cd frontend && npx vitest run e2e/surface`. Expected: PASS. Commit the
executor, protocol, runner, and tests.

### Task 5: Add Windows native-surface coverage

**Files:**
- Create: `frontend/e2e/windows/LoreGui.Native.Tests.ps1`
- Create: `frontend/e2e/windows/README.md`
- Modify: `.github/workflows/vscode-test.yml`

**Interfaces:**
- Produces: UIA coverage for directory dialogs, tray, notifications, installer, and updater confirmation.
- Consumes: UIA AutomationId mirrored from surface-map `element_id`.

- [ ] **Step 1: Write UIA discovery and uniqueness assertions**

Launch the exact installer/build in an isolated Windows account/profile. Assert
every required AutomationId exists once. Missing UIA capability must mark the
case SKIP and the required lane non-green.

- [ ] **Step 2: Implement safe native journeys**

Cover picker select/cancel, tray menu, notification appearance, installer
repair/uninstall in a disposable VM snapshot, and updater no-update/error using
a local fixture feed. Never install from a real external endpoint.

- [ ] **Step 3: Add the Windows CI job and commit**

Route only trusted canonical workflows to the existing Windows runner boundary.
Run the PowerShell suite and upload its evidence bundle. Commit workflow,
script, and README.

### Task 6: Standardize evidence and activate the standing docs drift gate

**Files:**
- Create: `frontend/e2e/surface/evidence.ts`
- Create: `frontend/e2e/surface/evidence.spec.ts`
- Create: `scripts/check-docs-website-parity.mjs`
- Modify: `.github/workflows/frontend-test.yml`
- Modify: `docs/INFORMATION-ARCHITECTURE.md`

**Interfaces:**
- Produces: `artifacts/loregui-surface/<commit>/<platform>/<run-id>/` evidence.
- Produces: deterministic major-change classification and docs/website checklist consumed by `lore-docs-sync`.

- [ ] **Step 1: Write evidence completeness tests**

Require `run.json`, `events.jsonl`, `ipc.redacted.jsonl`, before/after state,
accessibility data, screenshots, filesystem manifest, process/network logs, and
SHA256 hashes. A case missing a declared oracle artifact must fail.

- [ ] **Step 2: Implement the docs parity classifier**

Classify changes to onboarding, shell, context, connection/auth, server/repo,
install/version, tier/premium, and screenshots as major. For a major change,
require either matching docs/website changes or a checked exemption with
reviewer identity and reason. Reject placeholder screenshot text and claims
that StudioBrain authentication is required for standalone use.

- [ ] **Step 3: Run a local Qwen audit against the exact head**

The standing `lore-docs-sync` worker reports stale files and proposed copy;
deterministic checks decide pass/fail. Qwen output alone never satisfies the
gate.

- [ ] **Step 4: Run the full P2 gate and commit**

```bash
cd frontend
npm test
npm run typecheck
npx vitest run e2e/surface e2e/fixtures
cd ..
node scripts/check-docs-website-parity.mjs --base origin/main --head HEAD
cargo test -p loregui --lib
git diff --check
```

Expected: every command succeeds and a complete evidence bundle is produced.
Commit the evidence writer, parity gate, workflow, and IA documentation.
