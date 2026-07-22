# LoreGUI P0 Repository Context Correctness Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Delivery status:** Product-code acceptance delivered by #420 at
`06a4378c83012fe43bf4307a6ecfd54fc629c482`. This file preserves the reviewed
plan and its test requirements; unchecked plan syntax is not a current runtime
status ledger. Tasks 1-5 and the documentation portion of Task 6 are on durable
main. The installed-Windows/EROS restart, screenshots, selected-path proof, and
redacted action evidence remain the SBAI-5483 operational close gate.

**Boundary to P1:** P0 provides one validated active repository, native path
selection, fail-closed actions, honest onboarding, and hosted-server context.
It does not implement SBAI-5484's saved/recent/favorite server catalog, server
repository browser, or persistent Server / Repository / Local path / Branch /
Identity context shell.

**Goal:** Prevent LoreGUI from treating its launch directory as a repository and make host/connect onboarding end in an explicit, validated local-project state.

**Architecture:** Represent the active repository as optional fail-closed runtime state, validate every transition before storing it, and keep server storage separate from a client local path. Add a shared native-directory picker and lift step completion into the onboarding state machine so Continue and Finish reflect real backend success.

**Tech Stack:** Rust, Tauri v2, React 19, TypeScript, Vitest, Tauri MockRuntime, WebDriverIO.

## Global Constraints

- LoreGUI remains standalone Tauri v2 + React; no Dioxus rewrite.
- Local, LAN-discovered, and manually entered Lore servers require no StudioBrain account.
- A server store path is never inferred to be a client repository path.
- Process current directory is never an active-repository fallback.
- Repository-scoped commands fail closed with `LoreError::NoRepository("no repository is open")`.
- Pre-repository lifecycle commands do not consume active-repository state:
  create/clone/shared-store creation use their destination, storage operations
  retain a storage-session root, and auth/service commands use stable app-config
  lifecycle roots.
- Folder selection uses `@tauri-apps/plugin-dialog`; manual text entry remains an Advanced affordance.
- Every behavior change is test-first and every task ends in a focused commit.

---

### Task 1: Make active-repository state optional and fail closed

**Files:**
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/ipc_harness_tests.rs`
- Modify: `frontend/src/api.ts`
- Modify: `frontend/src/api.spec.ts`
- Modify: `frontend/src/App.tsx`
- Modify: `frontend/src/RepositoryPanel.tsx`
- Modify: `frontend/e2e/specs/smoke.e2e.ts`
- Test: `src-tauri/src/ipc_harness_tests.rs`

**Interfaces:**
- Produces: `AppState::dir() -> Result<PathBuf, LoreError>` and `current_repository() -> Option<String>`.
- Produces: `open_repository` validates status before updating `working_dir`.
- Produces: `api.currentRepository() -> Promise<string | null>` and an E2E
  startup contract that expects `null` until create/clone/open succeeds.
- Preserves: storage and all authentication operations before a repository is
  selected, without using process CWD as their backend root.
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

Do not apply the active-repository guard mechanically to lifecycle operations.
Storage open/put/get/close and handle-based storage commands must retain an
explicit storage-session root. Shared-store creation uses its destination
parent. Interactive/token login, local/current-user lookup, logout, and clear
must use a dedicated `<app_config_dir>/auth` root. Global service start/stop use
a dedicated `<app_config_dir>/service` root. Resolve app lifecycle roots from
`AppHandle`, with a deterministic temp fallback. None of these operations may
read process CWD or require `working_dir`.

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

Update the TypeScript wrapper to `invoke<string | null>` and update frontend
state consumers. In the WebDriver smoke flow, assert `null` at startup; create
or clone first, then validate/open the resulting repository.

Classify the empty state only from the exact typed frontend error contract:
`{ kind: "NoRepository", message: "no repository is open" }`. Never infer it
from message substrings such as `"Repository not found"`; unrelated
`CommandFailed` errors must remain visible.

`currentRepository()` represents its valid empty state as `null`; it has no
expected error state. Surface and stop refresh on every rejection from that
call. In WebDriver, parse the serialized JSON payload from the invoke wrapper
and deep-compare `{ kind, message }`; substring matching is not evidence of a
typed contract.

- [ ] **Step 4: Compile until every repository-scoped command uses the guard**

Run:

```bash
cargo check -p loregui
```

Expected: PASS and `rg 'state\.dir\(\)' src-tauri/src/commands.rs` shows only
guarded `?`, explicit `match`, or `if let Ok` use on repository-scoped commands.
Audit every `auth_*` and `storage_*` command separately: none may use
`state.dir()`. Audit every frontend onboarding API wrapper as well: lifecycle
wrappers must not require a repository, while genuine repository configuration
and VCS actions must retain the guard.

- [ ] **Step 5: Run Rust regression tests**

Run:

```bash
cargo test -p loregui --lib ipc_harness_tests -- --nocapture
```

Expected: PASS, including no-repository and invalid-open cases.

Also require regressions proving:

- storage open/put/get/obliterate round-trips with `current_repository == None`;
- token login reaches its auth backend rather than returning `NoRepository`;
- local identity lookup and auth clear do not require a repository;
- shared-store creation and global service lifecycle commands get past the
  repository guard and report only their own backend result;
- create and clone start from `working_dir == None`, reach their own backend,
  leave state `None` after a backend failure, and activate state only after
  success; reachable-server tests must not pre-seed `working_dir`;
- startup `status`, `log`, and `branches` each reject with the exact structured
  `NoRepository` contract while unrelated/transport errors fail the test;
- no test accepts `Ok || Err` tautologically or swallows arbitrary create,
  clone, auth, transport, or repository errors.

Every deliberately unavailable backend test must pin the exact observed
`{ kind, message }` result. Assertions such as “not `NoRepository`” are too weak
because lifecycle-root and other pre-backend failures would also pass.

Run the frontend contract gates as well:

```bash
cd frontend
npm test
npm run typecheck
npm run build
cd e2e && npx tsc --noEmit
```

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/commands.rs src-tauri/src/lib.rs src-tauri/src/ipc_harness_tests.rs \
  frontend/src/api.ts frontend/src/api.spec.ts frontend/src/App.tsx \
  frontend/src/RepositoryPanel.tsx frontend/e2e/specs/smoke.e2e.ts
git commit -m "fix(SBAI-5483): fail closed without an active repository"
```

### Task 2: Gate every repository action and render a guided empty state

**Files:**
- Modify: `frontend/src/api.ts`
- Modify: `frontend/src/App.tsx`
- Modify: `frontend/src/App.spec.tsx`
- Modify: `frontend/src/styles.css`
- Modify: `src-tauri/src/commands.rs`
- Test: `src-tauri/src/ipc_harness_tests.rs`
- Modify: `frontend/src/onboarding/OnboardingFlow.tsx`
- Modify: `frontend/src/onboarding/ClientClone.tsx`
- Modify: `frontend/src/palette/types.ts`
- Modify: `frontend/src/palette/CommandPalette.tsx`
- Modify: audited pre-repository-safe entries under `frontend/src/palette/manifest/**`
- Test: `frontend/src/onboarding/OnboardingFlow.spec.tsx`
- Test: `frontend/src/onboarding/ClientClone.spec.tsx`
- Test: `frontend/src/palette/CommandPalette.spec.tsx`
- Test: `frontend/src/App.spec.tsx`

**Interfaces:**
- Consumes: `api.currentRepository() -> Promise<string | null>`.
- Produces: `RepositoryActionGuard` derived only from validated `RepoStatus`.
- Produces: typed setup intents for `Open existing`, `Create local`, `Connect`,
  and `Host`, each routed to its real destination.

- [ ] **Step 1: Write failing shell tests**

Mock `current_repository` as `null` and `status` as `NoRepository`. Assert that
the guided empty state is visible and that clicking labels for Sync, Push,
Verify, GC, Metadata, Branches, History, Locks, and Dependencies cannot invoke
their IPC commands. Assert the AppData string does not render.

Open the command palette while the guard is closed. Assert repository-scoped
commands such as branch create, file obliterate, and repository GC cannot
invoke. Assert an explicitly audited pre-repository command such as repository
list remains usable. With a validated repository, assert branch create invokes
normally. The palette shell stays reachable; its operation boundary is guarded.

Do not prove this only by clicking disabled DOM buttons: disabled controls
suppress click handlers and would leave internal guard regressions undetected.
Exercise both enforcement layers independently:

- with no repository, highlight a repository-required command and press Enter;
  assert the selection guard prevents opening its form or invoking it;
- select a repository-required command while the repository is valid, rerender
  with the guard closed, then submit Run; assert the execution guard reports the
  exact disabled reason and does not invoke.

Click each empty-state action and assert its distinct destination:

- Open existing → `ClientClone` open-working-tree mode;
- Create local → a real name/path form that calls `repository_create`, then
  validates/opens the created path;
- Connect → client connect step;
- Host → storage backend step.

Four labels that all reopen the generic mode selector are not wired actions.

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

Pass a typed setup intent into `OnboardingFlow`. Add an initial mode to
`ClientClone` so open and create routes land on their actual forms. Path input
may remain manual in this task; Task 3 replaces it with the shared native
directory picker. Do not change Task 4's Continue/Finish completion gates here.

Add a typed manifest repository-context requirement. It is fail-closed by
default: an operation requires the validated active repository unless its
individual manifest was audited against the Rust command and explicitly marks
itself pre-repository-safe. Do not infer safety from a whole domain. Pass the
central guard into `CommandPalette`; a closed guard must make invoking any
repository-required manifest impossible while leaving audited auth, service,
storage/shared-store lifecycle, clone, and remote-list operations usable.

`repository_list` is a remote discovery operation and must work before a local
repository exists. Its Rust command currently calls `state.dir()` despite not
using a local repository. Prove the correction from `working_dir=None`: invoke
it against a valid unreachable `lore://` URL and assert the exact upstream
transport failure is returned, not `NoRepository`. Run it from a dedicated app
lifecycle root. Do not make palette `repository_create` or
`repository_create_with_metadata` pre-repository-safe: those forms do not carry
an explicit target path and therefore still depend on active repository state.

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
git add frontend/src/api.ts frontend/src/App.tsx frontend/src/App.spec.tsx frontend/src/styles.css \
  frontend/src/onboarding/OnboardingFlow.tsx frontend/src/onboarding/OnboardingFlow.spec.tsx \
  frontend/src/onboarding/ClientClone.tsx frontend/src/onboarding/ClientClone.spec.tsx \
  frontend/src/palette/types.ts frontend/src/palette/CommandPalette.tsx \
  frontend/src/palette/CommandPalette.spec.tsx frontend/src/palette/manifest
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

Add a `Browse…` button beside each local directory path: ClientClone clone
destination, open-existing path, and create-local path; BackendPicker local
storage path and optional mutable-store path; InitStore store path; and
ServiceSetup store directory. Do not
mistake S3 endpoint/bucket or telemetry log-file fields for directory inputs.
Cancellation preserves the current value. Selected Windows paths are passed
verbatim. Manual editing moves under an `Advanced path entry` disclosure.

- [ ] **Step 4: Add component tests for all four consumers**

For all seven real directory fields across the four components, mock
`chooseDirectory` to return `E:\\lore`, click `Browse…`, and assert its input
and subsequent IPC arguments contain exactly `E:\\lore`. Add one cancellation
assertion.

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
Exercise navigation callbacks, not only the button's disabled attribute. Prove
success is cleared when a child later reports error, the user switches mode,
or navigation moves backward.

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
clone/browser step rather than asking again: render the exact trimmed URL
prefilled and prove clone IPC uses that exact value.

- [ ] **Step 4: Implement honest host completion choices**

After server start success, replace bare Finish with:

```ts
type HostNextAction =
  | "browse-repositories"
  | "create-repository"
  | "open-existing"
  | "manage-server-only";
```

Only an explicit visible `manage-server-only` selection after ServiceSetup
success may finish without an active repository; service-running success alone
must not finish. Its copy must say repository actions will remain unavailable.

Switching among `browse-repositories`, `create-repository`, and `open-existing`
must remount/reset the repository form so the visible form always matches the
selected action. Test switching in both directions; a component that consumes
`initialMode` only once must not be reused across choices.

An already-running server may satisfy ServiceSetup only when its reported
`storeDir` exactly matches this flow's selected store path. A running server for
another store is an explicit visible error and cannot unlock Finish. Test both
matching and mismatched status.

Apply the same ownership check to the `host_server_start` response: success
requires `running=true` and an exact selected-store `storeDir`. Missing or
mismatched ownership is a visible error and cannot unlock Finish.

After repository success, any mode/input/directory edit must clear both the
shell result and ClientClone's local `Repository ready` banner. Connectivity
errors must preserve native `Error.message` rather than rendering `{}`.

Late async completion must not resurrect invalidated state. Use attempt tokens
inside repository actions and a route generation at the onboarding shell. An
edit/reset/unmount invalidates the child attempt; Back, mode/step movement, and
host-action switching invalidate old child reporter closures. Prove by resolving
deferred IPC/callbacks only after invalidation and asserting no success/banner,
no navigation, and no Finish unlock.

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
