---
name: palette-entry
description: Add one op's command-palette manifest entry (the mechanical Phase-2 unit). Use when an op already has a registered Tauri command and just needs to be exposed in the palette. Formalizes frontend/src/palette/README.md.
---

# Add a palette manifest entry

1. Confirm the op's **registered command** name in `src-tauri/src/lib.rs`
   `generate_handler!`. If none exists, this isn't a pure palette-entry task — use
   `integrate-endpoint` to add the command first.
2. Read the command's signature in `src-tauri/src/commands.rs` to get the exact
   parameter names + types.
3. Create `frontend/src/palette/manifest/<domain>/<op>.ts`:
   ```ts
   import type { OpManifest } from "../../types";
   const manifest: OpManifest = {
     id: "<domain>.<op>", domain: "<domain>", op: "<op>",
     label: "<Domain>: <Op>",            // verb-led, Title Case
     description: "<one plain sentence: what it does + effect>",
     command: "<registered_command_name>",
     args: [ /* one FieldSpec per command param */ ],
     resultKind: "void" | "text" | "json",   // by the command's return type
     keywords: ["<search terms>"],
     // surface?: "panel" | "menu" | "palette"  // per docs/INFORMATION-ARCHITECTURE.md
   };
   export default manifest;
   ```
   - `FieldSpec.name` = the **camelCase** command param (Tauri maps to snake_case).
   - `kind`: `text|number|boolean|enum|string-list`; `enum` needs `options`.
   - Mark `required` where the op needs a non-empty value.
   - `description` is mandatory (help gate).
4. Do **not** edit the manifest index (auto-globbed) or the allowlist — the gate is
   contention-free.
5. Verify: `npm --prefix frontend run build` and
   `node frontend/scripts/palette-parity.mjs` (OK).

Reference entries: `manifest/{repository/status,file/stage,revision/commit}.ts`
and `manifest/service/{start,stop}.ts`.
