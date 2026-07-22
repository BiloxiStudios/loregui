# Task 5 Report — Persistent Hosted-Server Context

Base: `62861d37762b758d785ef70fef5e983230b50818`

## Result

- Added a shell-level `HostedServerCard` to both the guided project hub and the
  active-project management surface. It is independent of repository state,
  process CWD, and StudioBrain identity.
- Polling exists only while the card is mounted. Polls, Stop, and clipboard
  actions use independent generations; unmount and lifecycle actions invalidate
  older completions.
- Running display uses only backend status: configured name or `Unnamed server`,
  preferred advertised client URL, separately labelled local URL, exact store,
  auth requirement, PID, and `Process running`. It never says `Healthy` because
  this task did not add or prove a `/health_check` probe.
- The Rust-owned live server now retains only the non-secret launch fields that
  status needs: normalized repository/server name and the resolved auth boolean.
  `HostStatus` returns those fields only while running. Stopped status fabricates
  neither field, and serialization tests prove credential/secret keys are absent.
- Stopped, incomplete, and error status clear running/PID claims while retaining
  the last non-secret context in memory for the app session. No context is written
  to disk or local storage.
- Browse repositories sends the exact displayed hosted URL directly through the
  existing `repository_list` backend and shell results surface. The hosted path
  never opens a URL prompt; the independent manual `List Repos` action still does.
- Copy URL reports visible success or failure. Stop applies only its current real
  backend result. Browse, Copy, and Stop are disabled when the server is not
  truthfully running.

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

### GREEN

- Focused card + shell: 2 files, 26 tests passed.
- Card race suite: 11 tests passed, covering running no-auth/auth-required,
  session remount, direct Browse, clipboard success/failure, newer stopped/error
  polls beating old running polls, malformed/error clearing, stop/poll races in
  both completion orders, unmount poll/action invalidation, retained context,
  and disabled stopped actions.
- Rust status tests: 2 passed.

## Final verification

- `cd frontend && npm test`
  - 25/25 Node tests passed.
  - 243/243 Vitest tests passed across 23 files.
- `cd frontend && npm run typecheck` — passed.
- `cd frontend && npm run build` — passed; 237 modules transformed. Vite emitted
  only the existing advisory for a generated chunk over 500 kB.
- `cargo test -p loregui --lib server_host::tests -- --nocapture`
  - 38 passed, 0 failed, 1 existing live-server smoke ignored.
- `cargo test -p loregui --lib ipc_harness_tests -- --nocapture`
  - 13 passed, 0 failed, 1 existing reachable-server test ignored.
- `cargo test -p loregui --lib`
  - 64 passed, 0 failed, 2 explicitly ignored.
- `cargo check -p loregui` — passed.
- `cargo fmt --all -- --check` — passed.
- `git diff --check` — passed before the report; rerun in the final commit gate.

## Scope

No push, PR, Jira mutation, or E2E smoke edit was performed. In particular,
`frontend/e2e/specs/smoke.e2e.ts` remains untouched for Task 6 ownership.
