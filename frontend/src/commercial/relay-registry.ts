import type { ComponentType } from "react";
import type { Feature } from "./entitlement";
import type { HostStatus } from "../api";

/**
 * Cross-network relay seam — the open-core hook a premium overlay uses to add a
 * "Make reachable across networks (relay)" control to the Host-a-server flow
 * (SBAI-4072).
 *
 * LoreGUI is open core (MIT). It hosts a `loreserver` on `127.0.0.1` only
 * (SBAI-4065) — reachable on the LAN at best. Making that server reachable
 * across networks with NO VPN (via the StudioBrain *bore* relay) is a PREMIUM,
 * PROPRIETARY capability that lives in the `loregui-cloud` overlay. To keep the
 * open core free of any relay/tunnel logic while still letting a commercial
 * build slot the control in, the core ships only this tiny registry plus the
 * generic advertised-URL hooks on {@link api} (`hostServerSetAdvertisedUrl` /
 * `hostServerClearAdvertisedUrl`).
 *
 * This mirrors {@link import("./premium-registry")} exactly, but for a control
 * *embedded inside the host flow* rather than a standalone nav panel: the relay
 * toggle only makes sense next to a running hosted server, so it renders within
 * `ServiceSetup`, not the top-bar nav.
 *
 * The open core registers nothing here, so {@link getRelayControl} returns
 * `null` and `ServiceSetup` renders zero relay UI. A commercial build's overlay
 * entry imports the relay module, which calls {@link registerRelayControl} at
 * import time. The control is still gated: `ServiceSetup` only mounts it when
 * `isEntitled(control.feature)` (`"relay"`) is true, otherwise it shows a locked
 * upsell affordance.
 *
 * Dependency-light (just React's `ComponentType` + the `Feature` id + the
 * `HostStatus` shape) so the overlay can register synchronously at module load.
 */

/** Props the host flow passes to a registered relay control. */
export interface RelayControlProps {
  /**
   * Live status of the hosted `loreserver`. The control reads `port` (the
   * local QUIC/gRPC port to tunnel) and `running`; it must render inert /
   * disabled when the server is not running.
   */
  status: HostStatus;
  /**
   * Ask the host flow to re-fetch `host_server_status` so a freshly-registered
   * `advertisedUrl` (the public relay URL) is reflected in the UI. The control
   * calls this after it opens or closes a tunnel.
   */
  onAdvertisedUrlChange: () => void;
}

/** A relay control contributed by a commercial overlay. */
export interface RelayControl {
  /** Stable id, e.g. "relay". */
  id: string;
  /** The entitlement feature this control is gated behind (e.g. "relay"). */
  feature: Feature;
  /** Short label for the locked-upsell affordance, e.g. "Cross-network relay". */
  label: string;
  /** The control component, mounted inside the host flow when entitled. */
  component: ComponentType<RelayControlProps>;
}

/** At most one relay control; a re-register (HMR / double import) replaces it. */
let registered: RelayControl | null = null;

/**
 * Register the cross-network relay control. Called at import time by the
 * commercial overlay's relay module
 * (`loregui-cloud/frontend-overlay/relay/index.ts`). Idempotent.
 *
 * No-op in the open core: nothing imports a module that calls this, so the
 * registry stays empty and no relay UI renders.
 */
export function registerRelayControl(control: RelayControl): void {
  registered = control;
}

/**
 * The registered relay control, or `null` in the open core. `ServiceSetup`
 * gates this by `isEntitled(control.feature)` to decide whether to mount it or
 * show a locked upsell; an open-core build gets `null` and renders neither.
 */
export function getRelayControl(): RelayControl | null {
  return registered;
}

/** @internal — for tests only. Clear the registry. */
export function __resetRelayRegistryForTests(): void {
  registered = null;
}
