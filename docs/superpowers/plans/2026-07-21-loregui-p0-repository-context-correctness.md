# LoreGUI P0 Repository Context Correctness Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Prevent LoreGUI from treating its launch directory as a repository and make host/connect onboarding end in an explicit, validated local-project state.

**Architecture:** Represent the active repository as optional fail-closed runtime state, validate every transition before storing it, and keep server storage separate from a client local path. Add a shared native-directory picker and lift step completion into the onboarding state machine so Continue and Finish reflect real backend success.

**Tech Stack:** Rust, Tauri v2, React 19, TypeScript, Vitest, Tauri MockRuntime, WebDriverIO.

## Global Constraints

- LoreGUI remains standalone Tauri v2 + React; no Dioxus rewrite.
- Local, LAN-discovered, and manually entered Lore servers require no StudioBrain account.
- A server store path is never inferred to be a client repository path.
- Process current directory is never an active-repository fallback.
- Repository-scoped commands fail closed with `LoreError::NoRepository("no repository is open")`.
- Folder selection uses `@tauri-apps/plugin-dialog`; manual text entry remains an Advanced affordance.
- Every behavior change is test-first and every task ends in a focused commit.

---

### Task 1: Make active-repository state optional and fail closed

**Files:**
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/ipc_harness_tests.rs`
- Test: `src-tauri/src/ipc_harness_tests.rs`

**Interfaces:**
- Produces: `AppState::dir() -> Result<PathBuf, LoreError>` and `current_repository() -> Option<String>`.
- Produces: `open_repository` validates status before updating `working_dir`.
- Consumes: existing `LoreBackend::status`, `create_repository`, and `clone` lifecycle methods.

- [ ] **Step 1: Write failing no-repository IPC tests**

Add tests that construct `AppState { working_dir: Mutex::new(None), ... }` and assert:

```rust
assert_eq!(current_repository(state.clone()), None);
let error = status(state.clone()).await.unwrap_err();
assert!(matches!(error, LoreError::NoRepository(message) if message == "no repository is open"));
```

Add a second test that passes a temporary non-repository directory to
`open_repository` and asserts both the returned `NoRepository` and that
`current_repository` remains `None`.

- [ ] **Step 2: Run the tests and verify the old CWD behavior fails them**

Run:

```bash
cargo test -p loregui --lib ipc_harness_tests::no_repository -- --nocapture
```

Expected: FAIL because `working_dir` is a required `PathBuf` and status attempts
the configured path.

- [ ] **Step 3: Implement the optional state and shared guard**

Change the state and accessor to:

```rust
pub struct AppState {
    pub working_dir: Mutex<Option<PathBuf>>,
    // existing fields unchanged
}

impl AppState {
    pub(crate) fn dir(&self) -> Result<PathBuf, LoreError> {
        self.working_dir
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
            .ok_or_else(|| LoreError::NoRepository("no repository is open".into()))
    }
}
```

Initialize it with `Mutex::new(None)` in `lib.rs`; delete `initial_dir`.
Change repository-scoped call sites from `state.dir()` to `state.dir()?`.
For `flush_app_state`, return without flushing when `dir()` is `Err`.
For lifecycle create/clone operations, build their backend from the target's
parent directory instead of calling `state.dir()` before a repository exists:

```rust
fn lifecycle_root(target: &std::path::Path) -> PathBuf {
    target.parent().unwrap_or(target).to_path_buf()
}
```

Only store `Some(path)` after create/clone/open succeeds. Implement:

```rust
#[tauri::command]
pub async fn open_repository(
    state: State<'_, AppState>,
    path: String,
) -> Result<(), LoreError> {
    let candidate = PathBuf::from(path);
    default_backend(candidate.clone()).status().await?;
    *state.working_dir.lock().unwrap_or_else(|e| e.into_inner()) = Some(candidate);
    Ok(())
}

#[tauri::command]
pub fn current_repository(state: State<'_, AppState>) -> Option<String> {
    state.dir().ok().map(|path| path.to_string_lossy().into_owned())
}
```

- [ ] **Step 4: Compile until every repository-scoped command uses the guard**

Run:

```bash
cargo check -p loregui
```

Expected: PASS and `rg 'state\.dir\(\)' src-tauri/src/commands.rs` shows only
guarded `?`, explicit `match`, or `if let Ok` use.

- [ ] **Step 5: Run Rust regression tests**

Run:

```bash
cargo test -p loregui --lib ipc_harness_tests -- --nocapture
```

Expected: PASS, including no-repository and invalid-open cases.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/commands.rs src-tauri/src/lib.rs src-tauri/src/ipc_harness_tests.rs
git commit -m "fix(SBAI-5483): fail closed without an active repository"
```

### Task 2: Gate every repository action and render a guided empty state

**Files:**
- Modify: `frontend/src/api.ts`
- Modify: `frontend/src/App.tsx`
- Modify: `frontend/src/App.spec.tsx`
- Modify: `frontend/src/styles.css`
- Test: `frontend/src/App.spec.tsx`

**Interfaces:**
- Consumes: `api.currentRepository() -> Promise<string | null>`.
- Produces: `RepositoryActionGuard` derived only from validated `RepoStatus`.
- Produces: empty-state actions `Open existing`, `Create local`, `Connect`, and `Host`.

- [ ] **Step 1: Write failing shell tests**

Mock `current_repository` as `null` and `status` as `NoRepository`. Assert that
the guided empty state is visible and that clicking labels for Sync, Push,
Verify, GC, Metadata, Branches, History, Locks, and Dependencies cannot invoke
their IPC commands. Assert the AppData string does not render.

```tsx
expect(screen.getByRole("heading", { name: "Choose a project" })).toBeVisible();
expect(screen.getByRole("button", { name: "Open existing" })).toBeEnabled();
expect(invoke).not.toHaveBeenCalledWith("sync", expect.anything());
```

- [ ] **Step 2: Verify the existing always-enabled top bar fails**

Run:

```bash
cd frontend
npm run test:vitest -- src/App.spec.tsx
```

Expected: FAIL because repository actions remain available without `repoOpen`.

- [ ] **Step 3: Implement one repository-action policy**

Change `currentRepository` to `invoke<string | null>`. Derive:

```ts
const repoOpen = Boolean(status?.repo_id && currentRepository);
const repoActionDisabledReason = repoOpen
  ? null
  : "Open or create a local project before running repository actions.";
```

Use one helper for every repository-scoped button:

```tsx
function RepoActionButton(props: ButtonHTMLAttributes<HTMLButtonElement>) {
  const disabled = !repoOpen || props.disabled;
  return <button {...props} disabled={disabled} title={disabled ? repoActionDisabledReason ?? undefined : props.title} />;
}
```

When `repoOpen` is false, render the guided project hub instead of Changes,
Branches, and History data. Keep Account and Settings reachable.

- [ ] **Step 4: Run focused tests and typecheck**

Run:

```bash
cd frontend
npm run test:vitest -- src/App.spec.tsx
npm run typecheck
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add frontend/src/api.ts frontend/src/App.tsx frontend/src/App.spec.tsx frontend/src/styles.css
git commit -m "fix(SBAI-5483): gate repository actions behind validated context"
```

### Task 3: Add one tested native directory-picker adapter

**Files:**
- Create: `frontend/src/platform/directoryPicker.ts`
- Create: `frontend/src/platform/directoryPicker.spec.ts`
- Modify: `frontend/src/onboarding/ClientClone.tsx`
- Modify: `frontend/src/onboarding/server/BackendPicker.tsx`
- Modify: `frontend/src/onboarding/server/InitStore.tsx`
- Modify: `frontend/src/onboarding/server/ServiceSetup.tsx`
- Test: `frontend/src/platform/directoryPicker.spec.ts`

**Interfaces:**
- Produces: `chooseDirectory(options: DirectoryPickerOptions): Promise<string | null>`.
- Consumes: `open` from `@tauri-apps/plugin-dialog`.

- [ ] **Step 1: Write adapter tests for selection and cancellation**

```ts
it("returns one Windows directory", async () => {
  vi.mocked(open).mockResolvedValue("E:\\lore");
  await expect(chooseDirectory({ title: "Choose server storage" }))
    .resolves.toBe("E:\\lore");
});

it("returns null when cancelled", async () => {
  vi.mocked(open).mockResolvedValue(null);
  await expect(chooseDirectory({ title: "Choose" })).resolves.toBeNull();
});
```

- [ ] **Step 2: Run and verify the missing adapter fails**

Run `cd frontend && npm run test:vitest -- src/platform/directoryPicker.spec.ts`.
Expected: FAIL because the module does not exist.

- [ ] **Step 3: Implement the adapter**

```ts
import { open } from "@tauri-apps/plugin-dialog";

export interface DirectoryPickerOptions {
  title: string;
  defaultPath?: string;
}

export async function chooseDirectory(options: DirectoryPickerOptions) {
  const selected = await open({ directory: true, multiple: false, ...options });
  return typeof selected === "string" ? selected : null;
}
```

Add a `Browse…` button beside each local path. Cancellation preserves the
current value. Selected Windows paths are passed verbatim. Manual editing moves
under an `Advanced path entry` disclosure.

- [ ] **Step 4: Add component tests for all four consumers**

For each component, mock `chooseDirectory` to return `E:\\lore`, click
`Browse…`, and assert its input and subsequent IPC arguments contain exactly
`E:\\lore`. Add one cancellation assertion.

- [ ] **Step 5: Run frontend tests and commit**

```bash
cd frontend
npm run test:vitest -- src/platform/directoryPicker.spec.ts src/onboarding
npm run typecheck
cd ..
git add frontend/src/platform frontend/src/onboarding
git commit -m "feat(SBAI-5483): add native directory selection"
```

### Task 4: Make onboarding completion state explicit

**Files:**
- Modify: `frontend/src/onboarding/OnboardingFlow.tsx`
- Modify: `frontend/src/onboarding/ClientConnect.tsx`
- Modify: `frontend/src/onboarding/ClientClone.tsx`
- Modify: `frontend/src/onboarding/server/BackendPicker.tsx`
- Modify: `frontend/src/onboarding/server/ValidateConnectivity.tsx`
- Modify: `frontend/src/onboarding/server/InitStore.tsx`
- Modify: `frontend/src/onboarding/server/ServiceSetup.tsx`
- Create: `frontend/src/onboarding/OnboardingFlow.spec.tsx`

**Interfaces:**
- Produces: `StepResult<T> = { status: "idle" | "working" | "success" | "error"; value?: T }`.
- Produces: each child calls `onStateChange(result)`.
- Consumes: selected server URL flows from Connect into Clone/Browse.

- [ ] **Step 1: Write failing navigation-state tests**

Assert Continue is disabled before backend preparation/connect success,
remains disabled on error, enables on success, and Finish cannot call
`onComplete` until repository open or explicit `manage-server-only` selection.

- [ ] **Step 2: Verify unconditional navigation fails the tests**

Run `cd frontend && npm run test:vitest -- src/onboarding/OnboardingFlow.spec.tsx`.
Expected: FAIL because Continue and Finish are always enabled.

- [ ] **Step 3: Implement the shared result contract**

```ts
export type StepStatus = "idle" | "working" | "success" | "error";
export interface StepResult<T = void> {
  status: StepStatus;
  value?: T;
  message?: string;
}
```

Store one result per step in `OnboardingFlow`. Set `disabled` and an explanatory
`title` from the current step result. Pass the successful Connect URL into the
clone/browser step rather than asking again.

- [ ] **Step 4: Implement honest host completion choices**

After server start success, replace bare Finish with:

```ts
type HostNextAction =
  | "browse-repositories"
  | "create-repository"
  | "open-existing"
  | "manage-server-only";
```

Only `manage-server-only` may finish without an active repository, and its copy
must say repository actions will remain unavailable.

- [ ] **Step 5: Run tests and commit**

```bash
cd frontend
npm run test:vitest -- src/onboarding
npm run typecheck
cd ..
git add frontend/src/onboarding
git commit -m "fix(SBAI-5483): gate onboarding on verified step state"
```

### Task 5: Keep hosted-server context visible after onboarding

**Files:**
- Create: `frontend/src/HostedServerCard.tsx`
- Create: `frontend/src/HostedServerCard.spec.tsx`
- Modify: `frontend/src/App.tsx`
- Modify: `frontend/src/api.ts`

**Interfaces:**
- Consumes: existing `host_server_status`, `host_server_stop`, and advertised URL fields.
- Produces: a session-durable card with name, URL, store, auth mode, PID/health, Browse, Copy URL, Restart, and Stop.

- [ ] **Step 1: Write a failing card behavior test**

Mock a running no-auth server and assert `Hosted on this device`, the exact
store `E:\\lore`, URL, `Authentication: Not required`, and Browse/Stop actions
remain visible after onboarding unmounts.

- [ ] **Step 2: Implement the card and mount it in the guided hub/Manage**

Poll only while the card is mounted. Render stopped and error states without
discarding the last configuration. Browse opens the repository browser with
the hosted URL preselected.

- [ ] **Step 3: Run tests and commit**

```bash
cd frontend
npm run test:vitest -- src/HostedServerCard.spec.tsx src/App.spec.tsx
npm run typecheck
cd ..
git add frontend/src/HostedServerCard.tsx frontend/src/HostedServerCard.spec.tsx frontend/src/App.tsx frontend/src/api.ts
git commit -m "feat(SBAI-5483): preserve hosted-server context in the shell"
```

### Task 6: Prove the installed flow and update immediate documentation

**Files:**
- Modify: `frontend/e2e/smoke.e2e.ts`
- Modify: `src-tauri/src/ipc_harness_tests.rs`
- Modify: `docs/INFORMATION-ARCHITECTURE.md`
- Modify: `website/src/app/docs/connect/page.mdx`
- Modify: `website/src/app/docs/host/page.mdx`

**Interfaces:**
- Consumes: P0 guarded state and native picker seam.
- Produces: regression proof that no repository command targets AppData.

- [ ] **Step 1: Add an E2E case that starts with no repository**

Launch with an isolated app profile and explicit non-repository CWD. Assert the
project hub appears, repository operations are disabled, and the IPC event log
contains no repository-scoped command.

- [ ] **Step 2: Add the host-to-project journey**

Use a fixture-owned server store and client local path. Host, choose Create or
Open, validate the visible context, restart the app, and assert the same local
path is restored. A command failure must fail the case instead of returning
success.

- [ ] **Step 3: Update docs and website copy**

Document the distinction between server store and local path, native Browse,
no-auth status, and the four next actions. Remove any instruction that says
List Repos opens a URL prompt.

- [ ] **Step 4: Run the full P0 gate**

```bash
cd frontend
npm test
npm run typecheck
npm run build
cd ..
cargo test -p loregui --lib
cargo fmt --all -- --check
git diff --check
```

Expected: every command succeeds.

- [ ] **Step 5: Produce EROS evidence and commit**

Record installer SHA, installed executable SHA, selected server store, selected
local path, visible context, redacted IPC log, before/after screenshots, and
restart persistence under the ticket evidence directory. Then:

```bash
git add frontend/e2e/smoke.e2e.ts src-tauri/src/ipc_harness_tests.rs docs/INFORMATION-ARCHITECTURE.md website/src/app/docs/connect/page.mdx website/src/app/docs/host/page.mdx
git commit -m "test(SBAI-5483): prove repository context on installed Windows"
```

