---
name: design-review
description: Run the LoreGUI coherence checklist over a UI diff — pass/fail with specific fixes. Use before merging any UI PR (the ux-designer's gate). Checks tokens, placement, copy, states, consistency, accessibility, help.
---

# Design review

Review a diff (default: `git diff main...HEAD`) against the checklist. Read
`docs/DESIGN-SYSTEM.md` + `docs/INFORMATION-ARCHITECTURE.md` first. For each item:
PASS, or FAIL with `file:line` + the concrete fix.

## Checklist

- [ ] **Tokens** — no hardcoded hex/rgb/font-size/shadow; uses `--surface-*` or the
  legacy aliases. Renders correctly in **both** light and dark themes.
- [ ] **Placement** — op is in the surface(s) the IA prescribes; correct nav
  location; not duplicated or buried.
- [ ] **Copy** — label is a clear verb; `description` is one plain sentence;
  matches lore terminology; jargon has a tooltip.
- [ ] **States** — empty (helpful + next action), loading (disabled + "…"), error
  (`--surface-error-*`, real message, retry), success all handled.
- [ ] **Destructive** — obliterate/delete/reset confirm and explain consequences.
- [ ] **Consistency** — looks/behaves like sibling domains; reuses `OpForm`/
  `OpResult`/existing panels; one primary action per view.
- [ ] **A11y** — semantic `button`/`label[htmlFor]`/`role=dialog`; keyboard-
  reachable; visible focus; meaning not by color alone.
- [ ] **Help** — non-trivial flow has help/tutorial; first use is discoverable.
- [ ] **Gates** — `palette-parity` (+ IA/help) green; build green.

## Output
`DESIGN REVIEW: PASS` or `FAIL` + a numbered list of fixes (file:line, what, why).
Be specific and actionable; do not approve incoherent or unthemed UI.
