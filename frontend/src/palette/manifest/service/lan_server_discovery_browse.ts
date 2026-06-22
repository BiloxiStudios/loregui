import type { OpManifest } from "../../types";

/**
 * Command-palette manifest entry for `lan server discovery: browse` (SBAI-4073).
 *
 * List LoreGUI-hosted lore servers discovered on the local network via
 * multicast LAN advertisements.
 */
const manifest: OpManifest = {
  id: "service.lan_server_discovery_browse",
  domain: "service",
  op: "lan_server_discovery_browse",
  label: "LAN Servers: Discover",
  description:
    "Browse the local network for LoreGUI-hosted lore servers and return their lore:// URLs.",
  command: "lan_server_discovery_browse",
  args: [
    {
      name: "timeoutMs",
      kind: "number",
      label: "Browse timeout (ms)",
      description: "Optional listen window before returning results (200-10000ms).",
      required: false,
      placeholder: "1200",
    },
  ],
  resultKind: "json",
  keywords: ["lan", "discover", "discovery", "mdns", "local network", "server"],
};

export default manifest;
