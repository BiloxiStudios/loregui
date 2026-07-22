# RUNNERS_V1 — Self-hosted-first CI runners with fork-safety gate

**Status:** Adopted (2026-07-22) · **Decision:** sb-lore ruling, PR-only, merge held for lead review  
**Ticket:** SBAI-5460 · **Scope:** 9 Linux-plain workflows (T1)

## Problem

loregui is a **public** repository. Workflow runs triggered by fork PRs execute
attacker-controlled code. That code must **never** reach self-hosted runners
(org infrastructure, shared secrets, network access beyond the sandbox).

At the same time, the team wants to default to org self-hosted runners for all
trusted events (push to main, same-repo PRs, schedules, dispatches) to benefit
from cached builds, GPU access, and faster CI — with an instant failover back
to GitHub-hosted runners by flipping a repo variable (zero commits).

## Mechanism

Every T1 workflow job selects its runner via this expression:

```yaml
runs-on: ${{ github.event_name == 'pull_request' && github.event.pull_request.head.repo.full_name != github.repository && 'ubuntu-latest' || fromJSON(vars.LOREGUI_LINUX_RUNNER || '["ubuntu-latest"]') }}
```

### Fork detection

```
github.event.pull_request.head.repo.full_name != github.repository
```

- **Fork PR** → always `ubuntu-latest` (GitHub-hosted, sandboxed)
- **Same-repo PR** → resolves from `vars.LOREGUI_LINUX_RUNNER`
- **Push / schedule / workflow_dispatch** → resolves from `vars.LOREGUI_LINUX_RUNNER`

### Repo variable

`LOREGUI_LINUX_RUNNER` — a GitHub Actions repository variable (JSON array string):

| Value | Effect |
|-------|--------|
| *(unset)* | All jobs use `ubuntu-latest` (GitHub-hosted default) |
| `["self-hosted","linux","proxmox"]` | Trusted jobs use org self-hosted runners (CT145–CT148) |

**Failover:** Flip the variable. Zero commits required.

## Truth table

| Event Context | LOREGUI_LINUX_RUNNER | Resolved runs-on | Safe? |
|---|---|---|---|
| push to main | unset | `ubuntu-latest` | ✓ |
| push to main | `["self-hosted","linux","proxmox"]` | self-hosted | ✓ (trusted) |
| push to feature | any | `ubuntu-latest` | ✓ (non-canonical ref) |
| schedule | unset | `ubuntu-latest` | ✓ |
| schedule | `["self-hosted","linux","proxmox"]` | self-hosted | ✓ (trusted) |
| workflow_dispatch | unset | `ubuntu-latest` | ✓ |
| workflow_dispatch | `["self-hosted","linux","proxmox"]` | self-hosted | ✓ (trusted) |
| PR (same repo) | unset | `ubuntu-latest` | ✓ |
| PR (same repo) | `["self-hosted","linux","proxmox"]` | self-hosted | ✓ (trusted) |
| **PR from FORK** | unset | `ubuntu-latest` | ✓ |
| **PR from FORK** | `["self-hosted","linux","proxmox"]` | `ubuntu-latest` | ✓ **fork blocked** |

## T1 workflows (migrated)

1. `auto-release.yml` — 1 job (`cut`)
2. `boundary-guard.yml` — 1 job (`boundary`; `release-supply-chain` matrix excluded)
3. `ci.yml` — 2 jobs (`core-check`, `palette-parity`)
4. `frontend-test.yml` — 1 job (`test`)
5. `integration.yml` — 1 job (`integration`)
6. `licenses.yml` — 1 job (`attribution`)
7. `remote-qa.yml` — 1 job (`remote-multiuser`)
8. `upstream-parity.yml` — 2 jobs (`detect`, `canary`)
9. `vscode-test.yml` — 2 jobs (`cli-contract`, `e2e`)

**Total:** 12 `runs-on` sites migrated.

## T2/T3 (out of scope for this PR)

- **T2:** `tauri-e2e.yml`, `build-crossplatform.yml` (cross-platform builds)
- **T3:** `windows-build.yml` (Windows-specific, self-hosted Windows runners)
- **T4:** macOS workflows (last)

## Preflight verification

Every PR and push to main runs `runner-policy-preflight.yml` which:

1. **Live assertion** — evaluates the actual expression in the running event
   context and hard-fails if a fork-PR event resolves to self-hosted.
2. **Static truth table** — evaluates the canonical expression across the full
   event/variable matrix (including fork rows same-repo PRs can never exercise)
   using GitHub's own `@actions/expressions` engine. Drifts in any workflow's
   `runs-on` expression cause immediate failure.

## Merge gates

Per sb-lore ruling:
- [ ] Lead review approved
- [ ] Demonstrated hosted failover (flip variable → CI still green)
- [ ] Preflight truth-table evidence in CI logs
- [ ] Repo setting "require approval for ALL outside collaborators" enabled (parallel)
