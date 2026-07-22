# Task 5 Report — Persistent Hosted-Server Context

Base: `62861d37762b758d785ef70fef5e983230b50818`

## Result

- Added a shell-level `HostedServerCard` to the guided project hub and inside
  the real `RepositoryPanel` dialog opened by Manage. It is not mounted globally
  above active-repository content. It remains independent of process CWD and
  StudioBrain identity.
- Polling exists only while the card is mounted. Polls, Stop, and clipboard
  actions use independent generations; unmount and lifecycle actions invalidate
  older completions.
- Running display uses only backend status: configured name or `Unnamed server`,
  preferred advertised client URL, separately labelled local URL, exact store,
  auth requirement, PID, and `Process running`. It never says `Healthy` because
  this task did not add or prove a `/health_check` probe.
- The Rust-owned live server retains only the non-secret launch fields status
  needs: normalized repository/server name and the effective resolved auth
  boolean. Local auth-enabled hosting is not implemented: `auth:true` now fails
  closed at the shared resolver before config preparation or process spawn with
  the exact error `authenticated hosting is not implemented for local loreserver
  launches`. Every successful current local launch therefore truthfully reports
  `authRequired:false`. Stopped status fabricates neither field, and serialization
  tests prove credential/secret keys are absent.
- Stopped, incomplete, and error status clear running/PID claims while retaining
  the last non-secret context in memory for the app session. No context is written
  to disk or local storage.
- Browse repositories sends the exact displayed hosted URL directly through the
  existing `repository_list` backend and shell results surface. Each Browse owns
  an `AbortSignal`; stopped/error/URL-changed status, Stop, unmount, or a newer
  Browse aborts it, and the shell suppresses late results by signal plus request
  generation. The hosted path never opens a URL prompt; manual `List Repos`
  remains unsignaled and still prompts. The result close button is named `Close
  repository list`.
- Copy and Stop have separate generations. Stopped/error/changed context
  invalidates pending Copy feedback without canceling Stop. Browse and Copy
  disable immediately when Stop starts; a late Copy cannot supersede the real
  Stop result.

## Restart decision / in-lane follow-up

Restart is deliberately disabled with this exact visible explanation:

`Restart is unavailable because the backend does not retain a secret-free full launch configuration.`

The live backend owns a child plus resolved URL/store/process metadata, but it
does not retain a replayable full `HostServerOptions`. That input can include S3
credentials. Reconstructing start arguments from the frontend card would be
incomplete and unsafe, while retaining credentials in `HostStatus` or frontend
memory would violate this task. A future backend restart command needs a
credential-safe, backend-owned launch-config lifecycle before Restart can be
enabled. No fake stop/start path was added.

Authenticated local hosting remains separately deferred. Task 6 / SBAI-5470
must provide real authenticated-server evidence before the frontend's future-
compatible `Authentication: Required` rendering can be claimed for a launch.

## TDD evidence

### RED

Frontend command:

`cd frontend && npm run test:vitest -- src/HostedServerCard.spec.tsx`

Result: failed before collection because `./HostedServerCard` did not exist.

Rust command:

`cargo test -p loregui --lib server_host::tests::hosted_status_uses_owned_launch_name_and_auth_without_secrets -- --nocapture`

Result: compilation failed on the expected missing backend contract:
`HostedServer` had no `server_name` / `auth_required` fields and `HostStatus`
had no corresponding fields.

The first integration run also proved a real product gap: hosted Browse invoked
`repository_list`, but its existing result UI was nested inside the active-local-
repository branch and therefore invisible in the guided hub. The result surface
was lifted to shell scope and the test now asserts command, exact args, visible
results, and zero prompt calls.

Review-hold RED commands:

- `cargo test -p loregui --lib server_host::tests::auth_enabled_local_launch_is_rejected_before_config -- --nocapture`
  failed because `auth:true` returned an auth-disabled TOML instead of an error.
- Focused card/App tests failed on 20 direct assertions before correction:
  pending Copy leaked late success/error after stopped/error/URL change, Copy
  stayed enabled during Stop, Browse received no `AbortSignal`, URL-A repository
  results appeared after stopped/URL-B state, the active card was outside Manage,
  and the result close control was only named `x`.

### GREEN

- Focused card + shell/Manage: 3 files, 43 tests passed.
- Card race suite: 21 tests passed, covering running no-auth/future-auth payload,
  session remount, direct Browse, clipboard success/failure, newer stopped/error
  polls beating old running polls, malformed/error clearing, stop/poll races,
  independent Stop/Copy ownership, Copy invalidation on status changes, and
  Browse abort on stopped/error/URL change/Stop/unmount/newer Browse.
- Rust auth/status tests prove exact auth rejection, no store creation during
  rejected prepare, effective successful `authRequired:false`, and secret-free
  serialization.

## Final verification

- `cd frontend && npm test`
  - 25/25 Node tests passed.
  - 255/255 Vitest tests passed across 23 files.
- `cd frontend && npm run typecheck` — passed.
- `cd frontend && npm run build` — passed; 237 modules transformed. Vite emitted
  only the existing advisory for a generated chunk over 500 kB.
- `cargo test -p loregui --lib server_host::tests -- --nocapture`
  - 40 passed, 0 failed, 1 existing live-server smoke ignored.
- `cargo test -p loregui --lib ipc_harness_tests -- --nocapture`
  - 13 passed, 0 failed, 1 existing reachable-server test ignored.
- `cargo test -p loregui --lib`
  - 66 passed, 0 failed, 2 explicitly ignored.
- `cargo check -p loregui` — passed.
- `cargo fmt --all -- --check` — passed.
- `git diff --check` — passed before the report; rerun in the final commit gate.

## Scope

No push, PR, Jira mutation, or E2E smoke edit was performed. In particular,
`frontend/e2e/specs/smoke.e2e.ts` remains untouched for Task 6 ownership.
