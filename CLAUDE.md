# CLAUDE.md — LoreGUI

Cross-platform **Tauri v2 + React 19 + TypeScript** desktop GUI for Epic Games'
[`lore`](https://github.com/EpicGames/lore) VCS. Public, MIT. Binds the upstream
`lore` crate **in-process** (never shells out to the CLI). Canonical checkout on
BRAINZ at `/srv/studiobrain-dev/loregui`. Jira project **SBAI**.

## Architecture (one op, four layers)

```
crates/lore-vm/src/ops/<domain>/<op>.rs   1. binding — calls lore's async fn, collects events → typed result
src-tauri/src/commands.rs (+ lib.rs)      2. #[tauri::command] — thin wrapper, registered in generate_handler!
frontend/src/api.ts                       3. typed invoke wrappers
frontend/src/palette/ + panels/views      4. GUI — command palette (universal) + panels/menus (rich)
```

- **Plans:** `docs/IMPLEMENTATION-PLAN.md` (full-parity), `docs/COMMAND-PALETTE-PLAN.md` (palette parity, Epic SBAI-3865), `docs/EXPERT-AGENTS.md` (this system, Epic SBAI-4024).
- **CI gates:** `ci.yml` (`core-check` = fmt+clippy+test on lore-vm; `palette-parity` = GUI coverage ratchet), `integration.yml`, `windows-build.yml`. `main` is not branch-protected.

## One binding, two consumption modes (NOT a contradiction)

"Binds `lore` in-process, never shells out" is about **this GUI** — and it still
holds. There are two distinct surfaces over the *same* in-process `lore-vm`
binding; don't read them as conflicting:

- **GUI / Tauri path (in-process, typed):** the app binds `lore-vm`'s ops as ~140
  typed `#[tauri::command]`s (the four-layer pattern above). It calls the ops
  in-process and **never shells out and never goes through FFI.** This is the
  rule, unchanged.
- **External-driver contract (`lorevm` CLI + `lorevm-ffi`):** the deliberate seam
  for drivers that are *not* this GUI — `lore-mcp` and the planned VS Code
  extension (shell `lorevm <domain>.<op> --args '<json>'`), and the Unreal Engine
  plugin (link `lorevm-ffi`'s C ABI for the hot path). These are *supposed* to
  shell / FFI in; that's their job.

Both surfaces ride **one canonical dispatch**: `lore_vm::dispatch(&api, op_id,
args) -> Result<Value, LoreError>` in `crates/lore-vm/src/dispatch.rs`, with
`lore_vm::supported_ops()` listing the routable ids. `lorevm-cli` and
`lorevm-ffi` both call it (no re-typed op match — SBAI-4081), so the CLI, the FFI
bridge, and any future external driver cannot drift. The GUI's typed Tauri
commands *could* also route through `dispatch` instead of binding each op
directly, but that ~140-command refactor is **out of scope** — the GUI stays
as-is for now; this is just a note, not a TODO.

See `docs/ue-lorevm-bridge-spike.md` for the UE bridge design that motivated the
shared seam.

## The coherence mandate (READ BEFORE ANY UI WORK)

Exposing an op is **not** just adding a command-palette row. Every endpoint must
land in the **full** app coherently. When you touch any op or domain you MUST:

1. Read `docs/DESIGN-SYSTEM.md` and `docs/INFORMATION-ARCHITECTURE.md` and the
   relevant `docs/domains/<domain>.md`.
2. Use the theme **semantic surface tokens** (`--surface-*`) — never hardcode
   colors (see DESIGN-SYSTEM). The app is fully themeable; your UI must re-theme.
3. Decide the op's **surface** per the IA: a rich **panel**, a **menu/nav** entry,
   and/or a **command-palette** entry. Add a palette manifest entry at minimum
   (the parity gate requires it).
4. Add **help**: a clear `description` on the palette entry; for multi-step flows,
   a tutorial / in-app help (use the `write-tutorial` skill / `docs-writer`).
5. Get a **design review** (`design-review` skill / `loregui-ux-designer`) before
   opening the PR — buttons, labels, empty/error states, and placement must make
   sense to a real user.

This applies to autonomous pipeline workers too — there is no human to catch an
incoherent UI. Acceptance for any UI ticket: **palette-parity + IA + help gates
green, and design review passed.**

## Expert agents & skills

`docs/EXPERT-AGENTS.md` lists them. Spawn the domain expert (`loregui-storage-expert`,
`loregui-auth-expert`, `loregui-vcs-domain-expert`) for behavior, `loregui-frontend-engineer`
for implementation, `loregui-ux-designer` for review, `loregui-docs-writer` for help.
Skills: `integrate-endpoint`, `add-domain-ui`, `design-review`, `write-tutorial`,
`palette-entry`.

## Conventions

- One op = one file per layer. Manifest entries auto-discover via `import.meta.glob`
  (no index edits). Don't reformat or touch files outside your op.
- Verify before PR: `cargo check -p loregui`, `cargo fmt --all --check`,
  `npm --prefix frontend run build`, `node frontend/scripts/palette-parity.mjs`.
- Every commit/PR references its SBAI ticket. Never use plan mode in autonomous runs.
- Pipeline workers: claim via `/opt/BrainMon/monitoring/lib/claim-ticket.sh SBAI-XXXX`.
