# RUNNERS_V1 — CI runner policy (self-hosted-first, Actions failover)

Versioned policy. Changes ship as `RUNNERS_V2`, never as silent edits.
Tracking: SBAI-5460 (T1). Adopted by lorecrew lead ruling, 2026-07-21.

## The rule

CI runs **self-hosted-first** with GitHub-hosted Actions as failover — except
that **only canonical-main events (push / schedule / workflow_dispatch on
`refs/heads/main`) may consult the self-hosted variable. Everything else runs
GitHub-hosted**: every `pull_request` of any origin, and every non-main ref
(branch push, tag push, branch dispatch). The reason is structural, not about
who triggered the run: non-main runs execute **that ref's workflow
definitions**, so any PR or branch can mutate a workflow file (including
`runs-on` itself) — and the runner group is pinned to workflows@main, which
would deny non-main jobs anyway (a queue deadlock, not a safety event, but
still wrong). loregui is a public repository; unreviewed code never reaches
studio infrastructure. No exceptions, no overrides.

Tooling note: the truth-table validator depends on `@actions/expressions`
(MIT, GitHub's own expression engine), pinned by `.github/scripts/package-lock.json`.
It is CI-only tooling — it is not part of the distributed LoreGUI binary, so it
does not belong in the THIRD-PARTY-LICENSES bundles, whose scope is shipped code.

## Mechanism

Every migrated job selects its runner with this expression:

```yaml
runs-on: ${{ (github.event_name == 'pull_request' || github.ref != 'refs/heads/main') && 'ubuntu-latest' || fromJSON(vars.LOREGUI_LINUX_RUNNER || '["ubuntu-latest"]') }}
```

This block is drift-checked: the static truth-table validator fails CI if it
ever differs from the expression in the workflows.

- **Any `pull_request` event, or any non-main ref** (branch push, tag,
  branch dispatch) → `ubuntu-latest`, unconditionally (see The rule).
- **Canonical-main events** (push/schedule/workflow_dispatch on
  `refs/heads/main`) → the JSON label array in the repo variable
  `LOREGUI_LINUX_RUNNER`; if the variable is unset, the hosted default
  `["ubuntu-latest"]`.

**Failover is a variable flip, not a commit:** set `LOREGUI_LINUX_RUNNER` to
`["ubuntu-latest"]` (or delete it) and every migrated workflow is back on
GitHub-hosted immediately. Restore `["self-hosted","linux","proxmox"]` to
return. Variable changes are audit-logged by GitHub.

`.github/workflows/runner-policy-preflight.yml` evaluates the same expression
on every PR and push, prints the event-context truth table, and fails the run
if an untrusted PR context (fork or Dependabot) ever resolves to self-hosted.

## T1-exempt workflows (deliberate, hosted-only)

Some workflows are **permanently hosted-only** and are *not* runner-selection
subjects, so they are deliberately **absent from `T1_WORKFLOWS`** in the truth
table. The omission IS the exemption — it is authoritative only because it is
recorded here. Any workflow in this category must be listed here in the same
edit that lands it; an unlisted omission is a gate bug, not an exemption.

- `release-supply-chain.yml` (SBAI-426, split out of `boundary-guard.yml` in
  SBAI-5505) — the supply-chain contract test. Static GitHub-hosted OS matrix
  (`ubuntu-latest` / `macos-latest` / `windows-latest`); never consults
  `LOREGUI_LINUX_RUNNER`; must stay hosted-only permanently (the SBOM /
  checksum / provenance contract is verified against GitHub-hosted runners).

## Fork-safety: defense in depth (four layers)

1. The `runs-on` expression above (workflow level) — every `pull_request`
   and every non-main ref → hosted, always. Proven two ways: the live
   preflight assertion, and the static truth table
   (`.github/scripts/runner-policy-truth-table.mjs`) which evaluates the real,
   drift-checked expression with GitHub's own expression engine across every
   event/ref/variable combination — fork, Dependabot, same-repo PR, branch
   push, tag, and branch-dispatch rows all asserted hosted.
2. A **dedicated org runner group for this repository alone**, with
   **selected-workflows restriction pinned to this repo's workflows at
   `refs/heads/main`** — this is REQUIRED, not optional (lead security
   ruling): it makes a workflow-mutation attack inert at the group level,
   because a PR-ref workflow that hardcodes self-hosted labels is not on the
   group's allowlist and cannot acquire its runners. GitHub blocks public
   repositories from runner groups unless the group opts in
   (`allows_public_repositories=true`) — which is exactly why the group must
   contain **only loregui** and is never a shared pool. **Live**: group
   `loregui-public` (id 7) exists (org-API-verified 2026-07-21); the
   main-pinned workflow restriction and the **ephemeral-runner conversion**
   (residual-risk control: each job gets a fresh runner instance, so nothing
   persists across jobs) are being applied by infra and are drill
   prerequisites.
3. Repo setting **“require approval for all outside collaborators”** for fork
   PR workflows — **set and verified by the lead (2026-07-21,
   `all_external_contributors`)**; `GITHUB_TOKEN` stays read-only for fork PRs.
4. Never `pull_request_target` with a checkout of the fork head — and never on
   self-hosted. This combination is the canonical RCE and is banned outright.

## Tiers and current state

| Tier | Scope | Target labels | State |
|------|-------|---------------|-------|
| T1 | Linux plain: auto-release, boundary-guard, ci, frontend-test, integration, licenses, remote-qa, upstream-parity, vscode-test | `["self-hosted","linux","proxmox"]` — matches the two **dedicated loregui runners `actions-linux-5` (id 32) and `actions-linux-6` (id 33)**, labels `[self-hosted, Linux, X64, proxmox, vm3]`, in org runner group **`loregui-public` (id 7)**: `allows_public_repositories=true`, visibility=selected → this repo only. Hosts: CT158/159 on **vm3** (16 vCPU / 32G each) — additive capacity per owner directive, deliberately OFF the CPU-saturated pve1; the earlier 2/2-split plan is superseded and model-manager's four pve1 runners are untouched | Mechanism landed; runners live and group verified on the org API (lorecrew record 2026-07-21). Toolchain verified on both runners (full Tauri v2 set + rust/node/cmake/protoc — infra record 2026-07-21). Variable stays UNSET (hosted default) until the failover drill passes and the lead signs off. Both runners carry the `vm3` label, so it does not isolate one — draining a single runner means stopping that named runner's service on its CT |
| T2 | Linux GUI/Tauri: tauri-e2e, build-crossplatform (linux), release (linux) | same as T1 (dedicated runners) | Pending T1. Toolchain proof on actions-linux-5/6 specifically is delivered (webkit2gtk-4.1 + gtk3 + xvfb + full Tauri v2 dep set + rust/node/cmake/protoc — infra record 2026-07-21), so T2 is dep-ready on the dedicated runners. NOT shared with model-manager; stagger only against each other (two runners) |
| T3 | Windows: windows-build, release (win), publish-vscode (win32) | `["self-hosted","Windows","X64"]` (bx-w11-build01) | Pending; single runner → serialized. Never use the retired `Windows, proxmox` label (dead VM150) |
| T4 | macOS: release, build-crossplatform, publish-vscode (darwin) | TBD | Last; blocked on macOS runner health + signing keychain (macOS was deliberately moved off self-hosted before — see release.yml header). GitHub-hosted remains the supported path until proven |

## Rollout / operations

1. T1 lands with the **hosted default** — behavior is unchanged until the flip.
2. Infra provisions the dedicated group + runners (done: `loregui-public` id 7,
   `actions-linux-5/6` on vm3) and confirms build toolchain on both runners.
3. **Seeded-violation proof** (gated on infra completing the main-pinned
   workflow restriction + ephemeral conversion, and on security sign-off;
   planned, not yet launched): a real fork PR edits a workflow's `runs-on` to
   hardcode the self-hosted labels. Expected: the mutated PR-ref workflow is
   not on group 7's main-pinned allowlist and **cannot acquire its runners**
   (queues indefinitely / fails), the unmutated workflows run hosted, and the
   org audit shows zero job assignments to actions-linux-5/6 from the PR.
   Rollback: close the PR — the variable is never involved.
4. **Failover drill** (required before relying on self-hosted): set
   `LOREGUI_LINUX_RUNNER='["self-hosted","linux","proxmox"]'`, dispatch a light
   **main-ref** workflow (workflow_dispatch) and confirm it lands on
   actions-linux-5/6; drain one runner mid-queue and confirm the other picks
   up; flip the variable back to hosted and confirm green on GitHub-hosted;
   then unset until the lead signs off.
5. Decommission/rollback: remove the runner from the org + destroy CT158/159 on
   vm3; deleting group 7 rolls membership back to Default. The variable flip is
   always the fastest escape hatch.
6. Each subsequent tier repeats this pattern with its own variable
   (`LOREGUI_WINDOWS_RUNNER`, `LOREGUI_MACOS_RUNNER`) and its own drill.
