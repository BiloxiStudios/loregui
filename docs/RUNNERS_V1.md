# RUNNERS_V1 — CI runner policy (self-hosted-first, Actions failover)

Versioned policy. Changes ship as `RUNNERS_V2`, never as silent edits.
Tracking: SBAI-5460 (T1). Adopted by lorecrew lead ruling, 2026-07-21.

## The rule

CI runs **self-hosted-first** with GitHub-hosted Actions as failover — except
that **workflow runs triggered by fork pull requests always run GitHub-hosted**.
loregui is a public repository; fork PRs execute untrusted code, and untrusted
code never reaches studio infrastructure. No exceptions, no overrides.

## Mechanism

Every migrated job selects its runner with this expression:

```yaml
runs-on: ${{ (github.event_name == 'pull_request' && github.event.pull_request.head.repo.full_name != github.repository) && 'ubuntu-latest' || fromJSON(vars.LOREGUI_LINUX_RUNNER || '["ubuntu-latest"]') }}
```

- **Fork-PR events** (head repo ≠ this repo) → `ubuntu-latest`, unconditionally.
- **Trusted events** (push, same-repo PR, schedule, workflow_dispatch) → the
  JSON label array in the repo variable `LOREGUI_LINUX_RUNNER`; if the variable
  is unset, the hosted default `["ubuntu-latest"]`.

**Failover is a variable flip, not a commit:** set `LOREGUI_LINUX_RUNNER` to
`["ubuntu-latest"]` (or delete it) and every migrated workflow is back on
GitHub-hosted immediately. Restore `["self-hosted","linux","proxmox"]` to
return. Variable changes are audit-logged by GitHub.

`.github/workflows/runner-policy-preflight.yml` evaluates the same expression
on every PR and push, prints the event-context truth table, and fails the run
if a fork-PR context ever resolves to self-hosted.

## Fork-safety: defense in depth (four layers)

1. The `runs-on` expression above (workflow level) — fork PRs → hosted, always.
2. Org runner group keeps **“allow public repositories” OFF**; loregui uses the
   group only for trusted-event runs (org-admin action, applied by infra).
3. Repo setting **“require approval for all outside collaborators”** for fork
   PR workflows; `GITHUB_TOKEN` stays read-only for fork PRs (repo-admin action).
4. Never `pull_request_target` with a checkout of the fork head — and never on
   self-hosted. This combination is the canonical RCE and is banned outright.

## Tiers and current state

| Tier | Scope | Target labels | State |
|------|-------|---------------|-------|
| T1 | Linux plain: auto-release, boundary-guard, ci, frontend-test, integration, licenses, remote-qa, upstream-parity, vscode-test | `["self-hosted","linux","proxmox"]` (CT145–148, pve1) | Mechanism landed; variable stays at hosted default until org runner-group admission is confirmed by infra |
| T2 | Linux GUI/Tauri: tauri-e2e, build-crossplatform (linux), release (linux) | same as T1 (Tauri v2 deps verified on all four runners) | Pending T1; stagger matrix concurrency — runners are shared with model-manager CI |
| T3 | Windows: windows-build, release (win), publish-vscode (win32) | `["self-hosted","Windows","X64"]` (bx-w11-build01) | Pending; single runner → serialized. Never use the retired `Windows, proxmox` label (dead VM150) |
| T4 | macOS: release, build-crossplatform, publish-vscode (darwin) | TBD | Last; blocked on macOS runner health + signing keychain (macOS was deliberately moved off self-hosted before — see release.yml header). GitHub-hosted remains the supported path until proven |

## Rollout / operations

1. T1 lands with the **hosted default** — behavior is unchanged until the flip.
2. Infra confirms org runner-group admission for this repo (group policy: public
   repos OFF), then sets `LOREGUI_LINUX_RUNNER='["self-hosted","linux","proxmox"]'`.
3. Failover drill (required before relying on self-hosted): drain or stop a
   runner, flip the variable back to hosted, re-run a workflow, confirm green.
4. Each subsequent tier repeats this pattern with its own variable
   (`LOREGUI_WINDOWS_RUNNER`, `LOREGUI_MACOS_RUNNER`) and its own drill.
