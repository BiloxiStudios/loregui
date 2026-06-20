# Command Palette

A single Ctrl/Cmd-K palette that can invoke any lore-vm op via a manifest-driven
generated form. This is the host for **full GUI parity**: one manifest entry per
op makes that op reachable, searchable, and runnable.

## Architecture

| File | Role |
|---|---|
| `types.ts` | `FieldSpec` + `OpManifest` model (the per-op contract). |
| `form.tsx` | Renders a generated form from `OpManifest.args` and runs it. |
| `result.tsx` | Renders the command result per `resultKind` (`void`/`text`/`json`). |
| `CommandPalette.tsx` | The Ctrl-K overlay: fuzzy search → form → `invoke`. |
| `manifest/index.ts` | The registry. **Append-only** — the only shared file. |
| `manifest/<domain>/<op>.ts` | One entry per op. The unit of parity work. |

## Adding an op (the fan-out unit)

1. Create `manifest/<domain>/<op>.ts` exporting a default `OpManifest`:

   ```ts
   import type { OpManifest } from "../../types";

   const manifest: OpManifest = {
     id: "branch.create",
     domain: "branch",
     op: "create",
     label: "Branch: Create",
     description: "Create a new branch.",
     command: "create_branch",        // the registered Tauri command name
     args: [
       { name: "name", kind: "text", label: "Branch name", required: true },
     ],
     resultKind: "json",
     keywords: ["branch", "new"],
   };

   export default manifest;
   ```

   - `command` must be a registered `#[tauri::command]` (add the wrapper + an
     `api.ts` binding if the op lacks one).
   - `FieldSpec.name` is the **camelCase** arg key the command expects (Tauri v2
     maps camelCase → snake_case), matching the `api.ts` call site.
   - `kind`: `text | number | boolean | enum | string-list`. `enum` needs
     `options`. `string-list` collects one value per line.

2. Append one `import` + one array element (sorted by `id`) to
   `manifest/index.ts`. **Do not** edit any other op's file.

3. Acceptance: the op appears in Ctrl-K, its generated form invokes the command,
   and the result renders.

See the three Phase 0 reference entries: `repository/status` (no args, JSON),
`file/stage` (string-list, void), `revision/commit` (required text, text result).
