import type { OpManifest } from "../../types";

/**
 * Command-palette manifest entry for `service: discover servers on LAN`
 * (SBAI-4073).
 *
 * Open-core, NOT gated: dynamic mDNS/zeroconf discovery (the same pattern
 * studiobrain-model-manager uses for gateway clustering). Starts (or reuses) a
 * background browse for lore servers advertised on the local network and returns
 * the list seen so far — each entry carries a friendly name, repo, host, and the
 * `lore://host:port/<repo>` URL a client connects with. The connect/onboarding
 * flow ("Servers on your network") is the rich surface; this palette row is the
 * universal way to trigger a scan from anywhere (Ctrl-K). No arguments.
 */
const manifest: OpManifest = {
  id: "service.lan_discover_browse",
  domain: "service",
  op: "lan_discover_browse",
  label: "Discover Servers on Network",
  description:
    "Scan the local network for lore servers (mDNS) and list any found, with the lore:// URL to connect. Open-core; the manual server URL always works too.",
  command: "lan_discover_browse",
  requiresRepository: false,
  args: [],
  resultKind: "json",
  keywords: [
    "lan",
    "discover",
    "discovery",
    "mdns",
    "zeroconf",
    "bonjour",
    "network",
    "servers",
    "browse",
    "find",
    "nearby",
  ],
};

export default manifest;
