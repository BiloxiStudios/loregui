# LoreGUI — Design System

The single visual contract. All UI consumes **semantic theme tokens** so it
re-themes with any user theme (build/save/share is shipped). Never hardcode a
color, font, or shadow.

## Tokens (CSS custom properties on `:root`)

Written by `frontend/src/theme/theme.ts::applyTheme`. 12 surfaces × 7 slots:

```
--surface-<name>-bg | -text | -text-secondary | -border | -hover | -active | -shadow
```

| Surface | Use for |
|---|---|
| `base` | page background, primary content, body text |
| `elevated` | cards, panels, raised content (`--shadow-md`) |
| `overlay` | modals, dropdowns, the command palette, tooltips (`--shadow-lg`) |
| `primary` | primary buttons / CTAs |
| `secondary` | secondary buttons / neutral actions |
| `accent` | highlights, badges, emphasis |
| `success` `warning` `error` `info` | status: confirmations, cautions, errors, info |
| `navigation` | top bar, sidebars, menus |
| `input` | text fields, selects, textareas |

Also: `--shadow-sm/-md/-lg`, `--base-font-size`, `--font-family`.

**Rules**
- Use `var(--surface-<x>-bg)` etc. In plain-CSS files prefer the legacy aliases
  (`--bg`, `--panel`, `--accent`, `--text`, `--muted`, `--green/--red/--amber`)
  which are mapped to surfaces in `styles.css`. In inline styles (palette, theme
  editor) reference the `--surface-*` vars directly.
- Buttons: primary action → `--surface-primary-*`; everything else → default
  button (secondary). One primary action per view.
- Status colors are semantic only — success/warning/error/info, never decorative.
- Respect `--base-font-size`/`--font-family`; don't set absolute px font sizes on
  body text.

## Components (reuse, don't reinvent)

- **Button** — base `button` (secondary); `--surface-primary-*` for the primary.
- **Field** — `.onboarding-field` pattern (label + input/select/textarea using
  `--surface-input-*`) or the palette `OpForm` generated fields.
- **Card / panel** — `--surface-elevated-bg` + 1px `--surface-elevated-border` +
  8px radius + `--shadow-md`. See `.branch-info-panel`, `.file-info-panel`.
- **Modal / overlay** — `--surface-overlay-*` + `--shadow-lg`, click-scrim to
  close, `Esc` to dismiss. See the theme modal and `CommandPalette`.
- **Command palette** — `frontend/src/palette/` (Ctrl/Cmd-K). The universal way to
  run any op via a generated form.
- **Generated form** — `palette/form.tsx` renders `FieldSpec[]`. Reuse its field
  kinds (`text|number|boolean|enum|string-list`) instead of bespoke inputs.
- **Result view** — `palette/result.tsx` (`void|text|json`). Rich domains get a
  typed renderer (reuse existing status/diff/history panels).

## Interaction & states

Every view handles the **four states**: empty (helpful, points to the next
action), loading (disabled control + "…"), error (`--surface-error-*`, the real
message, a retry), success. Destructive actions confirm. Keyboard: `Esc` closes
overlays; the palette is `Ctrl/Cmd-K`; forms submit on Enter.

## Copy voice

Concise, imperative, lowercase-tech-terms. Labels are verbs for actions
("Stage", "Commit", "Acquire lock"). Descriptions are one plain sentence of what
the op does + its effect. No jargon without a tooltip. Match lore's own
terminology (revision, branch, fragment, partition, shared store).

## Accessibility

Semantic elements (`button`, `label`+`htmlFor`, `role="dialog"`/`aria-modal`).
Keyboard-reachable; visible focus. Don't encode meaning in color alone (pair an
icon/word). Contrast must hold across themes — rely on the paired `-text` slot of
a surface, never a guessed color.
