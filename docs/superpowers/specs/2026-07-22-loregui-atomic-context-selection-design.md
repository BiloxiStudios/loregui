# LoreGUI Atomic Context Selection Design

**Ticket:** SBAI-5484, P1 Task 2  
**PR:** #424  
**Status:** Trio-approved design; implementation pending

## Problem

LoreGUI currently changes project context through two independent IPC calls:

1. `open_repository` validates a path, persists `AppSettings.active_repository`,
   and publishes `AppState.working_dir`.
2. `context_update` separately persists `ContextSettings`.

This split permits durable and runtime state to disagree. If the second call
fails, a new repository may remain active while the saved context and React
snapshot retain the previous selection. The first project has no prior path to
re-open, a rollback can itself fail, and frontend generations cannot undo a
Rust mutation. Concurrent selections can also allow an older request to mutate
Rust after a newer user intent.

There is no repository close/clear IPC. `repository_release` is a Lore
repository operation and is not a context lifecycle primitive.

## Decision

Add one backend-owned atomic context-selection command. It owns validation,
optimistic concurrency, persistence, and runtime publication for both project
and server selection.

Separate close/rollback IPC is rejected because it retains multi-call failure
windows. Frontend-only locking is rejected because it cannot undo mutations
that already crossed the IPC boundary.

## Contract

The command accepts:

- a complete `ContextSettings` candidate;
- a typed target containing either a `project_id` or `server_id`; and
- a positive monotonic `request_generation` allocated by the active frontend
  provider.

Raw repository paths are not command inputs. The backend validates the context
and resolves the selected project, repository, server, and local path from the
typed IDs.

The command returns an authoritative selection result containing the persisted
context, active repository path when applicable, and real repository status
when a project is selected.

## Concurrency and Transaction Semantics

The backend owns a serialized selection coordinator. Registration and commit
both take the coordinator lock. Registration records the newest request
generation before asynchronous work. Older generations are stale even if they
finish validation later.

For a project target:

1. Validate the complete context and resolve the project path server-side.
2. Validate the repository through the real backend `status` path without
   mutating application state.
3. Take the coordinator lock again and re-check that this request is still the
   newest generation.
4. While holding that lock, persist one `AppSettings` candidate
   containing both `ContextSettings` and `active_repository`.
5. Re-check generation immediately before publication. Registration uses the
   same coordinator lock, so a newer request cannot appear between the durable
   write and publication.
6. Publish `AppState.working_dir` last.
7. Return the authoritative context, path, and status.

For a server target:

1. Validate and resolve the server server-side.
2. Set the candidate context to the selected server with no active project.
3. Persist the context and `active_repository = None` in one settings update.
4. Re-check generation and clear `working_dir` last.
5. Return the authoritative server-only context.

Validation, stale-generation, or persistence failure leaves settings cache,
settings on disk, and runtime working directory unchanged. A stale request may
not persist or publish. Errors returned to the frontend are stable and
non-secret; raw paths, settings content, IPC payloads, credentials, and backend
details are not surfaced.

## Frontend Data Flow

`ContextProvider` remains the only allocator of selection generations. A
dedicated selection-generation ref is separate from refresh/view generations,
so an unrelated refresh cannot suppress frontend publication after a committed
backend selection. The provider no longer composes `open_repository` and
`context_update` for selections.

Project and server selections call the atomic command once and publish only the
returned authoritative result. React state is not changed on command failure.
Superseded callbacks are ignored, while the backend generation contract ensures
they also cannot mutate Rust state.

Restore remains unchanged: startup accepts a persisted project only when the
P0 `current_repository` path exactly matches and real `status` succeeds. No
process-CWD or AppData fallback is introduced.

## Tests and Evidence

Request-level Rust IPC tests and React tests must prove:

- a first-project persistence failure leaves no repository active;
- a persistence failure with a prior project retains the prior runtime and
  persisted selection without a rollback call;
- concurrent A then B selection rejects late A before persistence/publication;
- server selection atomically clears active repository and working directory;
- malformed target IDs, raw-path attempts, stale generations, validation
  failures, and persistence failures fail closed with redacted errors;
- successful project selection publishes only after the combined settings
  candidate is durable; and
- existing P0 restore, caller-root/CWD, boundary, and secret tests remain green.

Demonstrate enforcement by mutating out the generation re-check and the
publish-last ordering independently; each mutation must make a named regression
test fail.

## Scope

Authorized Task 2 expansion is limited to the command/API contract, `AppState`,
`SettingsManager`, Tauri registration, context frontend integration, and direct
Rust/React tests. Task 3 UI, repository browser, favorites, and unrelated
refactoring remain out of scope.
