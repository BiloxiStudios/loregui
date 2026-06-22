/**
 * Commercial overlay entry point — open-core stub (SBAI-4061 / SBAI-4068).
 *
 * This module is imported ONCE at bootstrap (see `main.tsx`) for its side
 * effects: a commercial overlay's entry registers its premium panels into the
 * {@link import("./premium-registry").registerPremiumPanel} registry here.
 *
 * In the OPEN CORE this is an EMPTY stub — it registers nothing, so
 * `getPremiumPanels()` returns `[]` and the app renders with zero premium UI.
 * The public repo ships exactly this file.
 *
 * A commercial build (the `loregui-cloud` overlay) REPLACES this file with one
 * that imports the overlay's premium modules (e.g. `_overlay/reporting/index.ts`),
 * each of which calls `registerPremiumPanel(...)` at import time. See
 * `loregui-cloud/frontend-overlay/overlay-entry.ts` and its `scripts/overlay.mjs`
 * compose step.
 *
 * Keep this file side-effect-free in the open core. Do NOT import any premium
 * module here — that would defeat the open/commercial split.
 */

export {};
