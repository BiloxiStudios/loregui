# LoreGUI Atomic Context Selection Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace split `open_repository` plus `context_update` project/server selection with one generation-guarded Rust transaction that persists context and repository identity together before publishing runtime state.

**Architecture:** `context_select` resolves a typed project/server ID from a validated `ContextSettings` payload. A coordinator shared by registration and commit rejects superseded generations; `SettingsManager` writes one candidate containing both `context` and `active_repository`, then Rust publishes or clears `working_dir` last. React calls this command once and publishes only its authoritative result.

**Tech Stack:** Rust, Tauri 2 IPC, serde, lore-vm `default_backend`, React 18, TypeScript, Vitest.

## Global Constraints

- Raw repository paths are never authoritative command inputs.
- `request_generation` is positive and monotonic within `ContextProvider`'s dedicated selection counter.
- Generation is checked before commit and immediately before runtime publication under the same coordinator lock.
- `ContextSettings` and `active_repository` are persisted in one candidate write before `working_dir` changes.
- Validation, stale generation, or persistence failure leaves disk, settings cache, and runtime repository unchanged.
- Project selection publishes `working_dir` last; server selection clears it last.
- IPC errors are stable and redacted; never return paths, credentials, payloads, or backend details.
- P0 restore/current-repository/CWD behavior is unchanged.
- Task 3 UI, browser, favorites, and unrelated refactoring are out of scope.

---

### Task 1: Atomic persistence and generation coordinator

**Files:**
- Modify: `src-tauri/src/settings.rs`
- Modify: `src-tauri/src/context.rs`
- Modify: `src-tauri/src/commands.rs`
- Test: `src-tauri/src/settings.rs`
- Test: `src-tauri/src/context.rs`

**Interfaces:**
- Produces: `SettingsManager::update_context_selection(ContextSettings, Option<PathBuf>) -> Result<(), String>`
- Produces: `ContextSelectionCoordinator::register(u64) -> Result<(), String>` and `ensure_current(u64) -> Result<(), String>`
- Consumes later: `AppState.context_selection: Mutex<ContextSelectionCoordinator>`

- [ ] **Step 1: Write failing transactional settings tests**

Add tests which persist a baseline project/context, call the new method, reload
from disk, and assert both fields changed together. Add a blocked-config test
that asserts both cache and disk retain the baseline:

```rust
fn complete_context() -> ContextSettings {
    let mut context = ContextSettings::default();
    context.servers.push(ServerProfile {
        id: "server-1".into(),
        alias: "Local Lore".into(),
        url: "lore://127.0.0.1:41337/repository-1".into(),
        source: ServerSource::Manual,
        favorite: false,
        auth_mode: AuthMode::NotRequired,
        credential_ref: None,
        last_seen_at: None,
    });
    context
}

#[test]
fn context_selection_persists_context_and_repository_as_one_candidate() {
    let tmp = tempfile::tempdir().expect("temp settings directory");
    let settings = SettingsManager::new(tmp.path().to_path_buf());
    let context = complete_context();
    let path = tmp.path().join("project-a");

    settings
        .update_context_selection(context.clone(), Some(path.clone()))
        .expect("atomic context selection");

    let reloaded = SettingsManager::new(tmp.path().to_path_buf()).get();
    assert_eq!(reloaded.context, context);
    assert_eq!(reloaded.active_repository, Some(path));
}

#[test]
fn failed_context_selection_retains_cache_and_disk() {
    let tmp = tempfile::tempdir().expect("temp root");
    let blocked = tmp.path().join("blocked-config");
    std::fs::write(&blocked, "not-a-directory").expect("blocking file");
    let settings = SettingsManager::new(blocked.clone());
    let before = settings.get();

    assert!(settings
        .update_context_selection(complete_context(), Some(tmp.path().join("candidate")))
        .is_err());
    let after = settings.get();
    assert_eq!(after.context, before.context);
    assert_eq!(after.active_repository, before.active_repository);
    assert_eq!(after.autostart_enabled, before.autostart_enabled);
    assert_eq!(after.close_to_tray, before.close_to_tray);
    assert_eq!(std::fs::read_to_string(blocked).unwrap(), "not-a-directory");
}
```

- [ ] **Step 2: Run the settings tests and record RED**

Run:

```bash
cargo test -p loregui settings::tests::context_selection -- --nocapture
```

Expected: compile failure because `update_context_selection` does not exist.

- [ ] **Step 3: Implement the one-candidate settings method**

Add to `SettingsManager`:

```rust
pub fn update_context_selection(
    &self,
    context: ContextSettings,
    active_repository: Option<PathBuf>,
) -> Result<(), String> {
    context.validate_for_persistence()?;
    self.update(move |settings| {
        settings.context = context;
        settings.active_repository = active_repository;
    })
}
```

- [ ] **Step 4: Write failing coordinator tests**

Define tests in `context.rs` before implementation:

```rust
#[test]
fn coordinator_rejects_zero_duplicate_and_superseded_generations() {
    let mut coordinator = ContextSelectionCoordinator::default();
    assert!(coordinator.register(0).is_err());
    coordinator.register(1).expect("generation one");
    assert!(coordinator.register(1).is_err());
    coordinator.register(2).expect("generation two");
    assert!(coordinator.ensure_current(1).is_err());
    coordinator.ensure_current(2).expect("current generation");
}
```

- [ ] **Step 5: Implement the coordinator and AppState field**

In `context.rs` add:

```rust
#[derive(Debug, Default)]
pub struct ContextSelectionCoordinator {
    latest_generation: u64,
}

impl ContextSelectionCoordinator {
    pub fn register(&mut self, generation: u64) -> Result<(), String> {
        if generation == 0 || generation <= self.latest_generation {
            return Err("context selection request is stale".into());
        }
        self.latest_generation = generation;
        Ok(())
    }

    pub fn ensure_current(&self, generation: u64) -> Result<(), String> {
        if generation != self.latest_generation {
            return Err("context selection request is stale".into());
        }
        Ok(())
    }
}
```

Add to `AppState` in `commands.rs`:

```rust
pub(crate) context_selection: Mutex<crate::context::ContextSelectionCoordinator>,
```

Initialize it with `Mutex::new(ContextSelectionCoordinator::default())` in the
production app and every test `AppState` constructor.

- [ ] **Step 6: Run focused tests and commit**

Run:

```bash
cargo test -p loregui settings::tests -- --nocapture
cargo test -p loregui context::tests::coordinator -- --nocapture
cargo fmt --all -- --check
```

Expected: all pass.

Commit:

```bash
git add src-tauri/src/settings.rs src-tauri/src/context.rs src-tauri/src/commands.rs src-tauri/src/lib.rs src-tauri/src/ipc_harness_tests.rs
git commit -m "feat(SBAI-5484): add atomic context selection state"
```

---

### Task 2: Backend-owned typed selection command

**Files:**
- Modify: `src-tauri/src/context.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/ipc_harness_tests.rs`

**Interfaces:**
- Consumes: `SettingsManager::update_context_selection`
- Consumes: `AppState.context_selection`
- Produces: Tauri command `context_select(context, target, request_generation) -> ContextSelectionResult`
- Produces: private `register_context_selection(&AppState, u64) -> Result<(), String>`
- Produces: private `commit_context_selection(&AppState, &SettingsManager, u64, ContextSettings, Option<PathBuf>, Option<RepoStatus>) -> Result<ContextSelectionResult, String>`

- [ ] **Step 1: Add typed request/response shapes and failing serde tests**

Add these exact public IPC types to `context.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ContextSelectionTarget {
    Project { project_id: String },
    Server { server_id: String },
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct ContextSelectionResult {
    pub context: ContextSettings,
    pub active_repository: Option<String>,
    pub status: Option<lore_vm::RepoStatus>,
}
```

Add a serde negative proving `{kind:"project", project_id:"p", path:"C:/raw"}`
is rejected and positive project/server round trips succeed.

- [ ] **Step 2: Run the serde tests and record RED**

Run:

```bash
cargo test -p loregui context::tests::selection_target -- --nocapture
```

Expected: compile failure because the IPC types do not exist.

- [ ] **Step 3: Write failing state-transition tests**

Using `create_offline_fixture_repository`, add tests that directly exercise
the command or its private commit helper:

```rust
#[test]
fn stale_a_cannot_commit_after_b_registers() {
    let app = build_app();
    register_context_selection(&app.state::<AppState>(), 1).unwrap();
    register_context_selection(&app.state::<AppState>(), 2).unwrap();
    let error = commit_context_selection(
        &app.state::<AppState>(),
        &app.state::<SettingsManager>(),
        1,
        ContextSettings::default(),
        None,
        None,
    )
    .unwrap_err();
    assert_eq!(error, "context selection request is stale");
    assert_eq!(commands::current_repository(app.state()), None);
}
```

Add direct tests for:

- blocked persistence with no prior repository leaves runtime/cache empty;
- blocked persistence with an existing repository leaves the prior runtime and
  saved context unchanged;
- successful server selection persists `active_repository = None` and clears
  `working_dir`;
- unknown target IDs and invalid context fail before any state mutation; and
- errors equal stable redacted strings and do not contain fixture paths.

- [ ] **Step 4: Implement target normalization, registration, validation, and commit**

Implement `context_select` with this control flow:

```rust
#[tauri::command]
pub async fn context_select(
    state: State<'_, AppState>,
    settings: State<'_, SettingsManager>,
    context: ContextSettings,
    target: ContextSelectionTarget,
    request_generation: u64,
) -> Result<ContextSelectionResult, String> {
    let normalized = normalize_selection(context, &target)?;
    register_context_selection(&state, request_generation)?;

    let (active_repository, status) = match &target {
        ContextSelectionTarget::Project { project_id } => {
            let project = normalized.projects.iter().find(|item| &item.id == project_id)
                .ok_or_else(|| "selected project is unavailable".to_string())?;
            let path = PathBuf::from(&project.local_path);
            let status = default_backend(path.clone()).status().await
                .map_err(|_| "selected project could not be opened".to_string())?;
            (Some(path), Some(status))
        }
        ContextSelectionTarget::Server { .. } => (None, None),
    };

    commit_context_selection(
        &state,
        &settings,
        request_generation,
        normalized,
        active_repository,
        status,
    )
}
```

Implement the helpers so registration and the synchronous persist/publish
commit use the same coordinator lock:

```rust
fn register_context_selection(state: &AppState, generation: u64) -> Result<(), String> {
    state
        .context_selection
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .register(generation)
}

fn commit_context_selection(
    state: &AppState,
    settings: &SettingsManager,
    generation: u64,
    context: ContextSettings,
    active_repository: Option<PathBuf>,
    status: Option<lore_vm::RepoStatus>,
) -> Result<ContextSelectionResult, String> {
    let coordinator = state
        .context_selection
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    coordinator.ensure_current(generation)?;
    settings
        .update_context_selection(context.clone(), active_repository.clone())
        .map_err(|_| "selected context could not be saved".to_string())?;
    coordinator.ensure_current(generation)?;
    *state.working_dir.lock().unwrap_or_else(|error| error.into_inner()) =
        active_repository.clone();
    Ok(ContextSelectionResult {
        context,
        active_repository: active_repository.map(|path| path.to_string_lossy().into_owned()),
        status,
    })
}
```

`normalize_selection` must derive `active.project_id`, `active.server_id`, and
identity retention from IDs already present in the validated context. It must
call `validate_for_persistence` after normalization. Do not accept a path from
the target.

- [ ] **Step 5: Register the command in production and MockRuntime handlers**

Add `context_select` beside `context_get/context_validate/context_update` in
`src-tauri/src/lib.rs` and the MockRuntime handler. Add a request-level IPC test
that sends the typed target and asserts the returned context, path, and status.
Add a request containing a raw `path` field and assert deserialization fails.

- [ ] **Step 6: Demonstrate generation and publish-last enforcement**

Run the stale-A test against the implementation and record PASS. Temporarily
remove `coordinator.ensure_current(request_generation)?` before the settings
write; rerun and record that `stale_a_cannot_commit_after_b_registers` FAILS.
Restore the guard.

Temporarily assign `working_dir` before `update_context_selection`; rerun the
blocked-persistence test and record that it FAILS because the candidate becomes
visible. Restore publish-last.

- [ ] **Step 7: Run Rust gates and commit**

Run:

```bash
cargo test -p loregui context::tests -- --nocapture
cargo test -p loregui ipc_harness_tests -- --nocapture
cargo check -p loregui
cargo fmt --all -- --check
```

Expected: all pass after both mutations are restored.

Commit:

```bash
git add src-tauri/src/context.rs src-tauri/src/lib.rs src-tauri/src/ipc_harness_tests.rs
git commit -m "feat(SBAI-5484): select Lore context atomically"
```

---

### Task 3: Frontend consumes authoritative atomic selection

**Files:**
- Modify: `frontend/src/context/types.ts`
- Modify: `frontend/src/context/api.ts`
- Modify: `frontend/src/context/ContextProvider.tsx`
- Modify: `frontend/src/context/ContextProvider.spec.tsx`

**Interfaces:**
- Consumes: `context_select` IPC and `ContextSelectionResult`
- Produces: unchanged `useLoreContext()` public API

- [ ] **Step 1: Extend the five existing provider regressions with RED race/failure cases**

Add deferred mocks that assert:

```typescript
it("keeps the prior snapshot when atomic selection persistence fails", async () => {
  // Restore project A, select B, reject context_select, then assert A remains
  // visible and no open_repository/context_update command was issued.
});

it("lets B win when deferred A resolves after B", async () => {
  // select A with generation 1, select B with generation 2, resolve B then A;
  // assert B is the only published project and generations [1, 2] were sent.
});

it("server selection closes the public repository snapshot", async () => {
  // Return an authoritative server-only context from context_select and assert
  // project/repository/branch are null while the selected server remains.
});
```

Assert every selection makes exactly one `context_select` call and never calls
`open_repository` or `context_update`.

- [ ] **Step 2: Run focused Vitest and record RED**

Run:

```bash
cd frontend
npx vitest run src/context/ContextProvider.spec.tsx
```

Expected: the new tests fail because the provider still composes split IPC.

- [ ] **Step 3: Add exact frontend IPC types and client**

In `types.ts` add:

```typescript
export type ContextSelectionTarget =
  | { kind: "project"; project_id: string }
  | { kind: "server"; server_id: string };

export interface ContextSelectionResult {
  context: ContextSettings;
  active_repository: string | null;
  status: RepoStatus | null;
}
```

In `api.ts` add:

```typescript
select: (
  context: ContextSettings,
  target: ContextSelectionTarget,
  requestGeneration: number,
) =>
  invoke<ContextSelectionResult>("context_select", {
    context,
    target,
    requestGeneration,
  }),
```

- [ ] **Step 4: Replace split selection calls and separate generations**

Keep the existing view/refresh `operation` ref. Add:

```typescript
const selectionGeneration = useRef(0);
```

For both selection functions increment only `selectionGeneration`, call
`contextApi.select` once, and publish only when the returned generation is
still current. Project publication uses returned `status` and resolved records;
server publication uses the returned server-only context. Do not call
`openRepository`, `currentRepository`, `status`, or `update` during selection.
Catch only to set stable generic UI errors; retain prior React state.

- [ ] **Step 5: Run frontend gates and commit**

Run:

```bash
cd frontend
npx vitest run src/context/ContextProvider.spec.tsx src/App.spec.tsx
npm run typecheck
npm run test
npm run build
```

Expected: all pass.

Commit:

```bash
git add frontend/src/context/types.ts frontend/src/context/api.ts frontend/src/context/ContextProvider.tsx frontend/src/context/ContextProvider.spec.tsx
git commit -m "feat(SBAI-5484): consume atomic context selection"
```

---

### Task 4: Full verification and immutable review handoff

**Files:**
- Verify only: all Task 1-3 files and existing guard scripts

**Interfaces:**
- Produces: frozen PR head with reproducible evidence; no merge or Jira transition

- [ ] **Step 1: Run full repository gates**

Run:

```bash
cargo test -p loregui
cargo check --workspace
cargo fmt --all -- --check
cd frontend && npm run test && npm run typecheck && npm run build && cd ..
node scripts/check-boundaries.mjs
node scripts/check-palette-parity.mjs
git diff --check origin/main...HEAD
```

Expected: every command succeeds.

- [ ] **Step 2: Verify scope and forbidden calls**

Run:

```bash
git diff --name-only 692f9ea...HEAD
rg -n 'openRepository|contextApi\.update' frontend/src/context/ContextProvider.tsx
rg -n 'context_select' src-tauri/src/lib.rs src-tauri/src/ipc_harness_tests.rs frontend/src/context
```

Expected: no Task 3/UI files; no split selection calls remain in the provider;
the new command is registered and tested in Rust and TypeScript.

- [ ] **Step 3: Push the frozen head and route exact-SHA reviews**

```bash
git push origin worker/sbai-5484-p1
gh pr view 424 --repo BiloxiStudios/loregui --json headRefOid,state,mergeStateStatus,url
```

Record the exact SHA, the RED/GREEN and mutation-proof evidence, and all gate
results. Route independent spec/quality review and sb-secure review. Do not
merge, transition Jira, or begin Task 3.
