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
| T1 | Linux plain: auto-release, boundary-guard, ci, frontend-test, integration, licenses, remote-qa, upstream-parity, vscode-test | `["self-hosted","linux","proxmox"]` — matches the two **dedicated loregui runners `actions-linux-5` (id 32) and `actions-linux-6` (id 33)**, labels `[self-hosted, Linux, X64, proxmox, vm3]`, in org runner group **`loregui-public` (id 7)**: `allows_public_repositories=true`, visibility=selected → this repo only. Hosts: CT158/159 on **vm3** (16 vCPU / 32G each) — additive capacity per owner directive, deliberately OFF the CPU-saturated pve1; the earlier 2/2-split plan is superseded and model-manager's four pve1 runners are untouched | Mechanism landed; runners live and group verified on the org API (lorecrew record 2026-07-21). Variable stays UNSET (hosted default) until the Tauri toolchain install completes on both CTs, the failover drill passes, and the lead signs off. The extra `vm3` label lets the drill drain a specific runner |
| T2 | Linux GUI/Tauri: tauri-e2e, build-crossplatform (linux), release (linux) | same as T1 (Tauri v2 deps verified on all four runners) | Pending T1; stagger matrix concurrency — runners are shared with model-manager CI |
| T3 | Windows: windows-build, release (win), publish-vscode (win32) | `["self-hosted","Windows","X64"]` (bx-w11-build01) | Pending; single runner → serialized. Never use the retired `Windows, proxmox` label (dead VM150) |
| T4 | macOS: release, build-crossplatform, publish-vscode (darwin) | TBD | Last; blocked on macOS runner health + signing keychain (macOS was deliberately moved off self-hosted before — see release.yml header). GitHub-hosted remains the supported path until proven |

## Rollout / operations

1. T1 lands with the **hosted default** — behavior is unchanged until the flip.
2. Infra provisions the dedicated group + runners (done: `loregui-public` id 7,
   `actions-linux-5/6` on vm3) and confirms build toolchain on both runners.
3. **Failover drill** (required before relying on self-hosted): set
   `LOREGUI_LINUX_RUNNER='["self-hosted","linux","proxmox"]'`, dispatch a light
   workflow and confirm it lands on actions-linux-5/6; drain one runner
   mid-queue and confirm the other picks up; flip the variable back to hosted
   and confirm green on GitHub-hosted; then unset until the lead signs off.
4. Decommission/rollback: remove the runner from the org + destroy CT158/159 on
   vm3; deleting group 7 rolls membership back to Default. The variable flip is
   always the fastest escape hatch.
5. Each subsequent tier repeats this pattern with its own variable
   (`LOREGUI_WINDOWS_RUNNER`, `LOREGUI_MACOS_RUNNER`) and its own drill.
