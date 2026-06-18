# LoreGUI — Pipeline Integration & Repo Layout Guide

For the BrainMon dispatch pipeline and any agent (pipeline-dispatched or interactive) working LoreGUI tickets.

## 1. Repo routing (ACTION REQUIRED on BrainMon)

LoreGUI tickets live in the **SBAI** Jira project but target a **new repo not in the StudioBrain routing table**:

| Selector | Repo | Checkout on BRAINZ |
|---|---|---|
| SBAI ticket has label **`repo-loregui`** | `BiloxiStudios/loregui` | `/srv/studiobrain-dev/loregui` |

**BrainMon change needed:** add a routing rule so that any SBAI issue labeled `repo-loregui` is dispatched to clone/work in `BiloxiStudios/loregui` (NOT studiobrain-core/cloud/app). All LoreGUI Epic children carry this label. Until this rule exists, do not auto-dispatch these tickets — they would land in the wrong repo.

The existing routing table (see `studiobrain-core/CLAUDE.md` → "Repo routing") is keyed by content type; LoreGUI adds a label-keyed override that takes precedence.

## 2. Dispatch gate (DO NOT dispatch yet)

Every op subtask carries label **`blocked-foundation`** and a "BLOCKED BY SBAI-3685" note. The Foundation Story (**SBAI-3685**) must be merged to `main` first — it provides the crate binding, the `collect`/`global`/`api`/`model` infra, and the one-file-per-op stub each agent fills in.

**Release procedure (integration manager):**
1. Land SBAI-3685; CI green on `main`.
2. Bulk-remove the `blocked-foundation` label from the 136 op subtasks (`scripts/release_pipeline.py`, TODO).
3. BrainMon may then dispatch. Recommended concurrency cap: start ~20–30, scale up once merge throughput is proven.

## 3. Repo layout (monorepo)

```
loregui/
├── crates/lore-vm/        Reusable, GUI-agnostic core. Binds the upstream `lore` crate.
│   └── src/ops/<domain>/<op>.rs   ← ONE FILE PER OPERATION (the unit of work)
├── src-tauri/             Tauri v2 desktop shell. src/commands/<domain>.rs (one cmd per op).
├── frontend/              GUI (Vite + React + TS). Per-domain panels + command palette.
├── website/              Marketing landing (Next.js) for loregui.com.  ← see §5
├── docs/                  IMPLEMENTATION-PLAN.md, this guide, jira-subtasks.tsv (op→ticket map).
├── scripts/              Jira/automation helpers.
└── .github/workflows/    windows-build.yml (tauri-action → NSIS/MSI installer).
```

## 4. Per-ticket workflow (every op subtask)

1. **Claim** the SBAI ticket via the BrainMon claim protocol (same as StudioBrain).
2. **Branch:** `SBAI-<n>-<domain>-<op>` off `main`.
3. **Implement, one file per layer** (no shared-file edits — see below):
   - `crates/lore-vm/src/ops/<domain>/<op>.rs` — bind `lore::<domain>::<op>` per IMPLEMENTATION-PLAN.md §4 (event-collector → typed view-model). NO CLI shelling.
   - `src-tauri/src/commands/<domain>.rs` — add the `#[tauri::command]` (this file is per-domain; coordinate within a domain or the manager merges).
   - `frontend/` — add a **command-manifest entry** (data), not a hand-edited switch, + the panel affordance.
   - test: integration test against a temp repo + shared-store (`lore-vm` test support `test_repo()`).
4. **Do NOT touch** the shared registries — the integration manager owns these and merges them in a controlled order:
   - `crates/lore-vm/src/ops/<domain>/mod.rs` (`pub mod <op>;` — append-only)
   - `src-tauri/src/lib.rs` `generate_handler![...]`
   - the frontend command-manifest aggregator
5. **PR:** title `SBAI-<n>: <domain> <op>`, never draft, one PR per ticket. CI must be green.
6. **Acceptance:** compiles; command registered (via manifest); GUI can invoke it; integration test passes; no files outside the op touched.

## 5. Should the website be its own repo? — Decision: NO (monorepo for now)

Keep `website/` inside the `loregui` monorepo for the launch.
- **Why:** single source of truth, one clone for pipeline agents, shared release cadence with the app's download links, simplest for a one-day ship. Vercel deploys a subdirectory cleanly via **Root Directory = `website`**.
- StudioBrain keeps its landing in a separate repo (`studiobrain-landing`) because it predates and deploys independently of the core product; LoreGUI has no such history.
- **Revisit later** only if the site gains an independent release cadence or external web contributors — then split to `BiloxiStudios/loregui-web` (tracked as a follow-up, not now).

Website is tracked under the Epic via its own task (add `SBAI` task labeled `loregui-website` if separate tracking is wanted; currently folded into Foundation/scaffold which already shipped it).

## 6. Build & test (CI gates)

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test -p lore-vm
cargo check -p loregui                 # the Tauri crate
npm --prefix frontend run build
# Windows installer: CI only (windows-latest, tauri-action). Cross-compiling from Linux is not supported.
```

## 7. Notifying the pipeline

This guide documents the required BrainMon change (§1) and the dispatch gate (§2). The actual pipeline config edit is owned by the BrainMon/`studiobrain-pipeline` operators. Options to apply it:
- Hand this doc's §1–§2 to the BrainMon maintainer.
- Relay via the inter-agent tmux channel to the `studiobrain-pipeline` / `brain-chat` session to apply the label-routing rule.
- Integration manager performs the bulk label-release (§2 step 2) only after Foundation merges.
