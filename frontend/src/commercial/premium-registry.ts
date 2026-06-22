import type { ComponentType } from "react";
import type { Feature } from "./entitlement";

/**
 * Premium panel registry — the seam between the open core and commercial
 * overlays (SBAI-4061 / SBAI-4068).
 *
 * LoreGUI is open core (MIT). Premium surfaces (the first being the Reporting &
 * Insights add-on) are NOT shipped in the public repo — their implementations
 * live in the proprietary `loregui-cloud` overlay. To keep the open core free of
 * any premium UI while still letting a commercial build slot it in, the core
 * ships only this tiny registry plus an EMPTY overlay entry point
 * (`overlay-entry.ts`). A commercial build replaces that entry point with one
 * that imports the overlay's premium modules, each of which calls
 * {@link registerPremiumPanel} at import time.
 *
 * The open core never registers anything here, so {@link getPremiumPanels}
 * returns `[]` and `App.tsx` renders zero premium nav entries / panels. The gate
 * (`entitlement.ts`) still ships in core: a registered premium panel is only
 * shown when `isEntitled(panel.feature)` is true.
 *
 * This module is intentionally dependency-light (just React's `ComponentType`
 * and the `Feature` id type) so overlays can register synchronously at module
 * load, before React mounts.
 */

/** Props every premium panel receives. Panels are modal overlays that close. */
export interface PremiumPanelProps {
  onClose: () => void;
}

/** A premium panel contributed by a commercial overlay. */
export interface PremiumPanel {
  /** Stable id, e.g. "reporting". Also used as the React key. */
  id: string;
  /** Nav-button label, e.g. "Reporting". */
  label: string;
  /**
   * Tooltip/title for the nav button when ENTITLED. The locked tooltip is
   * derived by the host. Optional.
   */
  title?: string;
  /** The entitlement feature this panel is gated behind. */
  feature: Feature;
  /** The panel component, mounted when its nav entry is activated. */
  component: ComponentType<PremiumPanelProps>;
}

/** Registry, keyed by id so a re-register (HMR / double import) is idempotent. */
const registry = new Map<string, PremiumPanel>();

/**
 * Register a premium panel. Called at import time by commercial overlay modules
 * (e.g. `loregui-cloud/frontend-overlay/reporting/index.ts`). Idempotent per id.
 *
 * No-op in the open core: nothing imports a module that calls this, so the
 * registry stays empty and no premium UI renders.
 */
export function registerPremiumPanel(panel: PremiumPanel): void {
  registry.set(panel.id, panel);
}

/**
 * All registered premium panels, in registration order. The open core gets `[]`.
 * `App.tsx` filters this by `isEntitled(panel.feature)` to decide what to mount;
 * a registered-but-unentitled panel still shows a locked upsell nav entry, and
 * the panel component itself re-checks entitlement defensively.
 */
export function getPremiumPanels(): PremiumPanel[] {
  return [...registry.values()];
}

/** @internal — for tests only. Clear the registry. */
export function __resetPremiumRegistryForTests(): void {
  registry.clear();
}
