import type { OpManifest } from "../../types";

/** Generation-safe restart of the backend-owned hosted-server session. */
const manifest: OpManifest = {
  id: "service.host_server_restart",
  domain: "service",
  op: "host_server_restart",
  label: "Host Server: Restart",
  description:
    "Restart the active hosted server from its private backend launch recipe. Requires the generation reported by Host Server: Status.",
  command: "host_server_restart",
  args: [
    {
      name: "expectedGeneration",
      kind: "number",
      label: "Expected generation",
      description:
        "Exact backend generation from Host Server: Status; stale values fail closed.",
      required: true,
    },
  ],
  resultKind: "json",
  keywords: ["host", "server", "restart", "generation", "loreserver"],
};

export default manifest;
