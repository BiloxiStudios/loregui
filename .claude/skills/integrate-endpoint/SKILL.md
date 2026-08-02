---
name: integrate-endpoint
description: Integrate one lore op into the FULL LoreGUI app coherently — binding → #[tauri::command] → palette entry → panel/menu if warranted → help/tutorial → design review. Use for any ticket that exposes or changes an op. Ensures it's not just a palette row.
---

# Integrate an endpoint (full vertical)

The end-to-end procedure to add/expose one op so it lands coherently in the app.

## Steps

1. **Context.** Read `CLAUDE.md`, `docs/DESIGN-SYSTEM.md`,
   `docs/INFORMATION-ARCHITECTURE.md`, `docs/domains/<domain>.md`.
2. **Behavior.** Spawn the domain expert (`loregui-storage-expert` /
   `loregui-auth-expert` / `loregui-vcs-domain-expert`) for the correct op, args,
   state machine, destructiveness, and gotchas.
3. **Surface decision.** Per the IA rule, decide: panel? menu? palette-only?
   (Every op gets at least a palette entry.)
4. **Command layer.** If the op has no `#[tauri::command]`, add a thin wrapper in
   `src-tauri/src/commands.rs` (read the op's Args/Result; `LoreApi::new(state.dir())`;
   build Args from params; return Result) and register in `lib.rs` `generate_handler!`.
   Add an `api.ts` wrapper only if a panel needs it.
5. **Palette entry.** Use the `palette-entry` skill: `palette/manifest/<domain>/<op>.ts`.
6. **Panel/menu.** If the IA calls for it, build/extend the domain panel or add the
   row/menu action (spawn `loregui-frontend-engineer` to the ux-designer spec).
7. **Help.** Spawn `loregui-docs-writer` for the op `description`, empty/error copy,
   and a tutorial if the flow is multi-step.
8. **Review.** Run the `design-review` skill (`loregui-ux-designer`). Fix findings.
9. **Gate.** All green: `cargo check -p loregui`, `cargo fmt --all --check`,
   `npm --prefix frontend run build`, `node frontend/scripts/palette-parity.mjs`.
10. **PR.** One op = one file per layer. Reference the SBAI ticket. Don't touch the
    manifest index or unrelated registries.

## Done when
The op is invokable from ⌘K with a generated form **and** sits in its correct
panel/menu with help, themed, all states handled, design review passed, gates green.
