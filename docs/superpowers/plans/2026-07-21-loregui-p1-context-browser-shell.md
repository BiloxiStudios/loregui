# LoreGUI P1 Context, Browser, and Shell Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Delivery status:** Approved remaining scope for SBAI-5484; not implemented by
P0/#420. The P0 prerequisite is on durable main at `06a4378c`. This plan owns
the saved/recent/favorite server catalog, server repository browser, versioned
non-secret context records, and persistent Server / Repository / Local path /
Branch / Identity status shell. Its checkboxes are future implementation work,
not runtime-completion claims.

**Goal:** Build a persistent, standalone-first project hub and shell that makes the active server, repository, local path, branch, identity/auth mode, and operation status unambiguous.

**Architecture:** Store versioned non-secret server, repository, project, hosted-server, and active-context records in the existing settings manager; keep credentials as opaque OS-store references. A React context controller owns validated selection and feeds a server/repository browser, a persistent context chain, and a reduced sidebar-based shell.

**Tech Stack:** Rust, serde, Tauri v2, React 19, TypeScript, Vitest, WebDriverIO.

## Global Constraints

- P0 repository-state correctness is merged at `06a4378c`; do not duplicate or
  weaken its validated-repository and no-CWD-fallback boundary.
- StudioBrain Premium is optional and secondary; it never owns standalone projects.
- Server, Repository, Local path, Branch, and Identity/no-auth are fixed customer terms.
- Secrets never enter `settings.json`, React localStorage, screenshots, or IPC logs.
- Unknown or malformed settings fail closed and preserve a recoverable backup.
- Major user-facing changes require local-Qwen docs/website parity evidence.

---

### Task 1: Persist versioned non-secret context records

**Files:**
- Modify: `src-tauri/src/settings.rs`
- Create: `src-tauri/src/context.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/src/settings.rs`
- Test: `src-tauri/src/context.rs`

**Interfaces:**
- Produces: `ContextSettings`, `ServerProfile`, `RepositoryBookmark`, `LocalProject`, `HostedServerProfile`, and `ActiveContext`.
- Produces: `context_get`, `context_update`, and `context_validate` Tauri commands.

- [ ] **Step 1: Write serde round-trip, migration, and secret-rejection tests**

Test empty legacy settings, a complete context, duplicate IDs, a missing active
project, and JSON containing `token`, `password`, `secret`, or raw credential
values. The latter must return a validation error and not persist.

- [ ] **Step 2: Verify tests fail before context types exist**

Run `cargo test -p loregui context -- --nocapture`. Expected: FAIL.

- [ ] **Step 3: Implement the versioned model**

Use snake_case serde fields and explicit enums:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AuthMode { NotRequired, Required, Unknown }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextSettings {
    pub schema_version: u32,
    pub servers: Vec<ServerProfile>,
    pub repositories: Vec<RepositoryBookmark>,
    pub projects: Vec<LocalProject>,
    pub hosted_servers: Vec<HostedServerProfile>,
    pub active: ActiveContext,
}
```

Implement `Default` explicitly with `schema_version: 1` and empty collections.
Validate referential integrity before update. Store
only `credential_ref: Option<String>` and reject suspicious secret field names
recursively before disk write.

- [ ] **Step 4: Register commands and run tests**

Run `cargo test -p loregui context settings -- --nocapture` and
`cargo check -p loregui`. Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/context.rs src-tauri/src/settings.rs src-tauri/src/lib.rs
git commit -m "feat(SBAI-5484): persist non-secret LoreGUI context"
```

### Task 2: Add a validated frontend context controller

**Files:**
- Create: `frontend/src/context/types.ts`
- Create: `frontend/src/context/api.ts`
- Create: `frontend/src/context/ContextProvider.tsx`
- Create: `frontend/src/context/ContextProvider.spec.tsx`
- Modify: `frontend/src/main.tsx`

**Interfaces:**
- Produces: `useLoreContext()` with `snapshot`, `selectProject`, `selectServer`, `refresh`, and `validationError`.
- Consumes: Tauri context commands and P0 `current_repository`/`status`.

- [ ] **Step 1: Write restore and stale-project tests**

Assert a valid last project is opened and validated, while a missing local path
leaves repository actions closed and marks only that project unavailable.

- [ ] **Step 2: Implement exact TypeScript mirrors and provider**

```ts
export interface ActiveContextSnapshot {
  server: ServerProfile | null;
  repository: RepositoryBookmark | null;
  project: LocalProject | null;
  branch: string | null;
  authMode: "not_required" | "required" | "unknown";
  connection: "local" | "connected" | "reconnecting" | "offline" | "auth_required";
}
```

Selection is transactional: validate/open first, persist second, publish state
last. On failure retain the previous active context.

- [ ] **Step 3: Run tests and commit**

```bash
cd frontend
npm run test:vitest -- src/context
npm run typecheck
cd ..
git add frontend/src/context frontend/src/main.tsx
git commit -m "feat(SBAI-5484): add validated active-context controller"
```

### Task 3: Build the saved-server and repository browser

**Files:**
- Create: `frontend/src/servers/ServerHub.tsx`
- Create: `frontend/src/servers/ServerCard.tsx`
- Create: `frontend/src/servers/RepositoryBrowser.tsx`
- Create: `frontend/src/servers/ServerHub.spec.tsx`
- Modify: `frontend/src/RepositoryPanel.tsx`
- Modify: `frontend/src/App.tsx`

**Interfaces:**
- Consumes: saved/favorite/recent/LAN/hosted profiles and existing repository-list command.
- Produces: Open Existing, Clone, Favorite, Details, Add Server, Retry, and Connect actions.

- [ ] **Step 1: Write browser behavior tests**

Cover favorites first, duplicate URL normalization, LAN-to-saved promotion,
health reasons, no-auth badge, repository-list error with Retry, and row actions.
Assert `window.prompt` is never called.

- [ ] **Step 2: Run and verify current raw prompt fails**

Run `cd frontend && npm run test:vitest -- src/servers src/RepositoryPanel.spec.tsx`.
Expected: FAIL because the server hub does not exist.

- [ ] **Step 3: Implement the hub and browser**

Server cards display alias, URL tooltip, source, favorite, last used, health,
and auth mode. Selecting a card loads its repositories. Row actions invoke the
P0 native picker and validated context controller. Delete the top-bar
`window.prompt` path and route Browse server to this hub.

- [ ] **Step 4: Run tests and commit**

```bash
cd frontend
npm run test:vitest -- src/servers src/RepositoryPanel.spec.tsx src/App.spec.tsx
npm run typecheck
cd ..
git add frontend/src/servers frontend/src/RepositoryPanel.tsx frontend/src/App.tsx
git commit -m "feat(SBAI-5484): add saved-server repository browser"
```

### Task 4: Add the persistent context chain and operation status

**Files:**
- Create: `frontend/src/context/ContextBar.tsx`
- Create: `frontend/src/context/ContextBar.spec.tsx`
- Create: `frontend/src/activity/ActivityDrawer.tsx`
- Create: `frontend/src/activity/ActivityDrawer.spec.tsx`
- Modify: `frontend/src/App.tsx`
- Modify: `frontend/src/styles.css`

**Interfaces:**
- Consumes: `ActiveContextSnapshot` and current operation state.
- Produces: accessible switchers for Server/Local, Project/Repository, Local path, Branch, and Status.

- [ ] **Step 1: Write exact-state rendering tests**

Assert the bar renders:

```text
EROS Lore · No auth -> Game Lore -> E:\lore\game-lore -> main -> Up to date
```

Cover Local only, Offline, Reconnecting, Auth required, Dirty, Incoming,
Outgoing, Conflict, and Operation failed. Each segment must have an accessible
button name.

- [ ] **Step 2: Implement the bar and persistent activity drawer**

Status errors remain until dismissed or superseded. Operation rows include
target project, local path, started/completed time, outcome, and recovery
action; credential data is redacted before state storage.

- [ ] **Step 3: Run tests and commit**

```bash
cd frontend
npm run test:vitest -- src/context/ContextBar.spec.tsx src/activity
npm run typecheck
cd ..
git add frontend/src/context frontend/src/activity frontend/src/App.tsx frontend/src/styles.css
git commit -m "feat(SBAI-5484): show persistent project and connection context"
```

### Task 5: Restructure the shell around daily domains

**Files:**
- Create: `frontend/src/shell/AppSidebar.tsx`
- Create: `frontend/src/shell/AppSidebar.spec.tsx`
- Create: `frontend/src/shell/ManageMenu.tsx`
- Modify: `frontend/src/App.tsx`
- Modify: `frontend/src/styles.css`
- Modify: `docs/INFORMATION-ARCHITECTURE.md`

**Interfaces:**
- Produces: Files, Changes, Branches, History, Locks, and Activity routes.
- Produces: Manage entries for Verify, GC, Flush, Metadata, Dependencies, Hosted Servers, Settings, and Integrations.

- [ ] **Step 1: Write navigation and duplication tests**

Assert only context, status, one primary action, palette, and overflow remain in
the top bar. Assert each daily domain appears once and admin actions appear only
under Manage or palette.

- [ ] **Step 2: Implement sidebar and Manage menu**

Use existing panels without changing their backend contracts. Branches and
History become main views rather than duplicate overlays. Rename Sync to
`Get latest`; keep Push separate and context-gated.

- [ ] **Step 3: Run tests and commit**

```bash
cd frontend
npm run test:vitest -- src/shell src/App.spec.tsx
npm run typecheck
cd ..
git add frontend/src/shell frontend/src/App.tsx frontend/src/styles.css docs/INFORMATION-ARCHITECTURE.md
git commit -m "feat(SBAI-5484): align LoreGUI shell with context-first IA"
```

### Task 6: Add standalone-first onboarding with optional Premium

**Files:**
- Modify: `frontend/src/onboarding/ModeSelect.tsx`
- Modify: `frontend/src/onboarding/OnboardingFlow.tsx`
- Create: `frontend/src/onboarding/ModeSelect.spec.tsx`
- Create: `frontend/src/onboarding/StudioBrainPremiumCard.tsx`
- Test: `frontend/src/onboarding/ModeSelect.spec.tsx`

**Interfaces:**
- Produces: modes `local`, `client`, `host`, and `studiobrain_premium`.
- Consumes: existing premium entitlement/login seam without moving it into core Lore context.

- [ ] **Step 1: Write ordering and independence tests**

Assert Local, Connect, and Host are primary choices; Premium is fourth and
labeled optional. Assert local/no-auth completion never invokes StudioBrain
auth and signing out does not remove local projects.

- [ ] **Step 2: Implement approved copy and routing**

Use:

```text
Connect StudioBrain (Premium)
Add managed servers, federation, and StudioBrain integrations. Local and LAN
Lore projects do not require an account.
```

The Premium card opens the existing premium integration flow, not a new core
credential store.

- [ ] **Step 3: Run tests and commit**

```bash
cd frontend
npm run test:vitest -- src/onboarding
npm run typecheck
cd ..
git add frontend/src/onboarding
git commit -m "feat(SBAI-5484): make StudioBrain an optional premium onboarding path"
```

### Task 7: Prove persistence, switching, and documentation parity

**Files:**
- Modify: `frontend/e2e/smoke.e2e.ts`
- Create: `frontend/e2e/context-switch.e2e.ts`
- Modify: `README.md`
- Modify: `website/src/app/page.tsx`
- Modify: `website/src/app/guide/page.tsx`
- Modify: `website/src/app/docs/connect/page.mdx`
- Modify: `website/src/app/docs/host/page.mdx`
- Modify: `website/src/app/docs/panels/page.mdx`
- Modify: `website/src/app/premium/page.tsx`
- Modify: `website/src/components/mockups/AppWindow.tsx`

**Interfaces:**
- Consumes: complete P1 context model and UI.
- Produces: real restart/switch evidence and matching public copy.

- [ ] **Step 1: Add restart and context-switch E2E cases**

Create two fixture-owned servers and projects. Favorite one, switch between
them, restart, and assert exact server/repository/local path/branch/auth state.
Take an operation after each switch and assert the IPC arguments and filesystem
diff target only the selected project.

- [ ] **Step 2: Run the local-Qwen parity audit**

Give the exact product head and the file scope from the design spec to the
standing `lore-docs-sync` worker. Require a stale-claim inventory before edits
and exact changed-file/check evidence after edits.

- [ ] **Step 3: Update docs, website, mockups, and real screenshots**

Remove StudioBrain-as-prerequisite copy and raw URL-prompt instructions. Show
the persistent context chain and optional Premium card. Screenshots must come
from the reviewed build and contain no placeholder text.

- [ ] **Step 4: Run the full P1 gate**

```bash
cd frontend
npm test
npm run typecheck
npm run build
cd ../website
npm test --if-present
npm run build
cd ..
cargo test -p loregui --lib
cargo fmt --all -- --check
git diff --check
```

Expected: every command succeeds.

- [ ] **Step 5: Commit**

```bash
git add frontend/e2e README.md website
git commit -m "docs(SBAI-5484): align LoreGUI product copy with context-first UX"
```
