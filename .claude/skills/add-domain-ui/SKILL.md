---
name: add-domain-ui
description: Build a whole domain's UI surface coherently — its panel, nav entry, palette entries for every op, and help — per the information architecture. Use to bring a domain from palette-only to a first-class part of the app.
---

# Add a domain's UI

Bring one domain (e.g. branch, storage, lock) to first-class coherence.

## Steps

1. **Map.** List the domain's ops (`crates/lore-vm/src/ops/<domain>/`). Spawn the
   domain expert for behavior/state machines. Per the IA, classify each op:
   panel-primary / row-menu / palette-only.
2. **Spec.** Spawn `loregui-ux-designer` for the panel layout (reusing components),
   the nav entry, every op's placement + copy, and the four states.
3. **Commands.** Ensure every op has a registered `#[tauri::command]`; add the
   missing ones (`integrate-endpoint` step 4).
4. **Palette.** Add a manifest entry for **every** op (`palette-entry` skill).
5. **Panel + nav.** Build the domain panel and add its **navigation entry**
   (sidebar for daily domains, Settings/Manage for admin) per the IA. Wire routing.
   Reuse existing panels' structure (`.section`, cards, `OpForm`, `OpResult`).
6. **Help.** `loregui-docs-writer`: op descriptions, the panel's empty state, and a
   tutorial for any multi-step flow (e.g. merge, server setup).
7. **Review + gate.** `design-review`; then `cargo check -p loregui`,
   `cargo fmt --all --check`, `npm build`, `palette-parity` (+ IA/help) all green.
8. **Update the IA doc** if you added/moved a nav entry.

## Done when
The domain has a navigable panel, every op is reachable (palette + its prescribed
surface), help exists, it's themed and consistent with sibling domains, gates green,
design review passed. This is the bar every domain must reach (Epic SBAI-4024).
