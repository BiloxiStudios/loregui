# RUNNERS_V1 — CI runner policy (self-hosted-first, Actions failover)

Versioned policy. Changes ship as `RUNNERS_V2`, never as silent edits.
Tracking: SBAI-5460 (T1). Adopted by lorecrew lead ruling, 2026-07-21.

## The rule

CI runs **self-hosted-first** with GitHub-hosted Actions as failover — except
that **untrusted pull-request runs always run GitHub-hosted**: fork PRs
(attacker-controlled code) and Dependabot PRs (same head repo, but GitHub runs
them with fork-like trust and they execute freshly-updated dependency code).
loregui is a public repository; untrusted code never reaches studio
infrastructure. No exceptions, no overrides.

Tooling note: the truth-table validator depends on `@actions/expressions`
(MIT, GitHub's own expression engine), pinned by `.github/scripts/package-lock.json`.
It is CI-only tooling — it is not part of the distributed LoreGUI binary, so it
does not belong in the THIRD-PARTY-LICENSES bundles, whose scope is shipped code.

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
   Proven two ways: the live preflight assertion, and the static truth table
   (`.github/scripts/runner-policy-truth-table.mjs`) which evaluates the real,
   drift-checked expression with GitHub's own expression engine across every
   event/variable combination — fork rows included.
2. A **dedicated org runner group for this repository alone**. GitHub blocks
   public repositories from runner groups unless the group opts in, so the
   group must set `allows_public_repositories=true` — which is exactly why it
   must contain **only loregui** (selected-repositories) and, where the org
   plan supports it, be restricted to **selected workflows** pinned to this
   repo's workflow files. The group is never a shared pool: opting a shared
   group into public repos would expose every repo's runners. Containment for
   fork code itself is layers 1, 3, and 4 — the group scoping bounds the blast
   radius to runners this repo was explicitly granted. Live org-admin state
   and group creation: infra (brain-chat), pending admin credential unlock.
3. Repo setting **“require approval for all outside collaborators”** for fork
   PR workflows — **set and verified by the lead (2026-07-21,
   `all_external_contributors`)**; `GITHUB_TOKEN` stays read-only for fork PRs.
4. Never `pull_request_target` with a checkout of the fork head — and never on
   self-hosted. This combination is the canonical RCE and is banned outright.

## Tiers and current state

| Tier | Scope | Target labels | State |
|------|-------|---------------|-------|
| T1 | Linux plain: auto-release, boundary-guard, ci, frontend-test, integration, licenses, remote-qa, upstream-parity, vscode-test | `["self-hosted","linux","proxmox"]` — dedicated group gets **CT147/148** (2/2 split with model-manager per infra capacity call: pve1 is CPU-saturated, so runners are repartitioned, never added there; all four CT145–148 verified Tauri-v2-ready — lorecrew board record 2026-07-21) | Mechanism landed; variable stays UNSET (hosted default) until the dedicated runner group exists, fork-safety proof and failover drill pass, and the lead signs off. Two runners ⇒ cap/stagger matrices and avoid overlapping model-manager release builds; durable contention fix is runners on a different host |
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
