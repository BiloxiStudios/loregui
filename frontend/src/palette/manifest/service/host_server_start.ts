import type { OpManifest } from "../../types";

/**
 * Command-palette manifest entry for `host server: start` (SBAI-4065).
 *
 * Launches a real standalone `loreserver` process serving a local store over
 * QUIC + gRPC on loopback, with auth disabled (the local host case). Returns the
 * `lore://host:port/<repo>` URL that clients connect to. Unlike `service.start`
 * (an upstream stub that hosts nothing), this actually hosts a server.
 */
const manifest: OpManifest = {
  id: "service.host_server_start",
  domain: "service",
  op: "host_server_start",
  label: "Host Server: Start",
  description:
    "Launch a real Lore server (loreserver) over a local store on 127.0.0.1. Returns the lore:// URL to give to clients.",
  command: "host_server_start",
  requiresRepository: false,
  args: [
    {
      name: "storeDir",
      kind: "text",
      label: "Store directory",
      description:
        "Directory backing the served stores. Use the same shared-store path your repository was created in.",
      required: true,
      placeholder: "/path/to/shared/store",
    },
    {
      name: "port",
      kind: "number",
      label: "Port",
      description: "QUIC/gRPC port. HTTP is served on port + 2. Defaults to 41337.",
      required: false,
      placeholder: "41337",
    },
    {
      name: "repositoryName",
      kind: "text",
      label: "Repository name",
      description:
        "Optional. Embedded in the advertised lore://host:port/<name> URL clients clone.",
      required: false,
    },
  ],
  resultKind: "json",
  keywords: ["host", "server", "loreserver", "serve", "start", "quic", "grpc"],
};

export default manifest;
