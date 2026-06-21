# LoreGUI — Command-Palette Parity Plan

**Status:** active · **Epic:** SBAI-3865 · **Created:** 2026-06-20
**Depends on:** `docs/IMPLEMENTATION-PLAN.md` (§3–§6 define the palette + manifest model)

## 1. Goal

Make **every** lore op reachable, searchable, and runnable from the GUI through a
single **Ctrl/Cmd-K command palette** that renders a form from a per-op manifest
and invokes the matching Tauri command. This is the "GUI affordance" layer the
implementation plan calls for (line 16: *"Every operation gets a binding, GUI
affordance, and test"*) — delivered uniformly instead of hand-building ~136
bespoke panels.

This is **not** a competing design to the original 124-op fan-out. It is the same
goal, finishing the **GUI layer**:

| Layer | Artifact | Owner |
|---|---|---|
| 1. op binding | `crates/lore-vm/src/ops/<domain>/<op>.rs` | done (~136) |
| 2. command | `#[tauri::command]` + `api.ts` binding | ~71/136 (gap: ~65) |
| 3. **GUI affordance** | **`palette/manifest/<domain>/<op>.ts`** | **this epic** |
| + test | per-op integration test | per-op |

## 2. Current coverage (gap)

- **136** op bindings (engine reachable).
- **76** registered Tauri commands.
- **3** palette manifest entries (the Phase 0 references) → **~73** registered
  commands not yet exposed; **~65** ops have no command at all.
- Only **~29** ops had any GUI affordance before the palette.

## 3. Architecture

```
frontend/src/palette/
├── types.ts            FieldSpec + OpManifest (the per-op contract)
├── form.tsx            FieldSpec[] -> generated form
├── result.tsx          void / text / json result rendering
├── CommandPalette.tsx  Ctrl-K overlay: fuzzy search -> form -> invoke
├── manifest/
│   ├── index.ts        registry — auto-discovers entries via import.meta.glob
│   └── <domain>/<op>.ts  ONE entry per op (the fan-out unit)
└── README.md           how to add an op
```

**Merge-safe fan-out:** entries are auto-discovered with Vite `import.meta.glob`,
so adding an op is **one new file, zero shared-file edits** — no index to append,
no registry conflicts. (The command/`api.ts` layer still has shared appends; the
integration manager serializes those.)

A manifest entry is declarative:

```ts
const manifest: OpManifest = {
  id: "branch.create", domain: "branch", op: "create",
  label: "Branch: Create", description: "Create a new branch.",
  command: "create_branch",                 // a registered Tauri command
  args: [{ name: "name", kind: "text", label: "Branch name", required: true }],
  resultKind: "json",
};
export default manifest;
```

`FieldSpec.kind` ∈ `text | number | boolean | enum | string-list`.

## 4. Phases

- **Phase 0 — infra (SBAI-3866, done):** palette + manifest model + form/result +
  Ctrl-K + 3 reference entries + parity ratchet + this doc.
- **Phase 1 — command/binding gap (~65 ops):** add the `#[tauri::command]` +
  `api.ts` binding for ops that lack one. Fan out by domain.
- **Phase 2 — manifest fan-out (136 entries):** one `manifest/<domain>/<op>.ts`
  per op; ops from Phase 1 get entry + binding together. Merged continuously.
- **Phase 3 — polish:** typed result renderers for high-value ops (status/diff/
  history reuse existing panels), arg validation, keyboard UX; resolve the
  deferred `cherry_pick`/`bisect` spike (upstream may not export a fn yet).

**Tickets:** SBAI-3865 (epic) → SBAI-3866 (infra) + 14 domain stories
(SBAI-3867…3879) → 136 per-op subtasks (SBAI-3880…4015). One file per op; do not
edit shared registries (manager merges).

## 5. Parity enforcement (the ratchet)

`frontend/scripts/palette-parity.mjs` (CI job `palette-parity` in `ci.yml`):

- Parses registered commands from `src-tauri/src/lib.rs` `generate_handler!`.
- Parses `command:` from every `palette/manifest/**/*.ts`.
- **Fails** if a registered command is neither exposed nor in
  `palette-parity-allowlist.json` (`deferred` backlog or permanent `excluded`).
- **Fails** if a `deferred` entry is already covered or no longer a command — the
  allowlist only shrinks, so it trends to empty = **full enforced parity**.

Effect: a new lore op cannot be wired without either exposing it in the GUI or
consciously deferring it. As the fan-out lands manifest entries, each is removed
from `deferred`; when `deferred` is empty the GUI is provably complete and stays
that way.

## 6. Upstream parity (keeping up with Epic's lore)

`scripts/upstream-lore-parity.mjs` enumerates the op surface (`pub async fn`) of
the **pinned** upstream `lore` source and diffs it against our bindings. At the
current pin it reports **0 new / 0 orphaned** — full parity.

`.github/workflows/upstream-parity.yml` (scheduled weekly + manual) runs the same
detector against upstream **HEAD** and, when Epic adds ops we don't bind, opens/
updates a tracking issue listing them — the **notice** that new functions need
building. Turning that notice into dispatched work:

1. Detector finds new upstream ops → tracking issue (and/or, on BRAINZ where Jira
   creds live, SBAI subtasks filed under SBAI-3865).
2. The pipeline dispatches an agent per new op: bind in `lore-vm` → add command →
   add palette manifest entry → test.
3. The `palette-parity` ratchet guarantees the new command is exposed before
   merge. Parity restored automatically.

This closes the loop: **as the lore API grows, the GUI is forced to grow with it.**

## 7. Mirror

Canonical: this file in the repo. Mirror under
`/srv/studiobrain-dev/plans/loregui/COMMAND-PALETTE-PLAN.md` (keep in sync).
