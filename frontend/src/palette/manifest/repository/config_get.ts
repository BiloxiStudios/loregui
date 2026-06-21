import type { OpManifest } from "../../types";

/**
 * Palette manifest for `repository.config_get`.
 *
 * Reads a configuration value (e.g. `remote_url`, `identity`) from the
 * repository config. Single-field form + JSON result.
 */
const manifest: OpManifest = {
  id: "repository.config_get",
  domain: "repository",
  op: "config_get",
  label: "Repository: Get Config",
  description: "Read a configuration value from the repository config.",
  command: "config_get",
  args: [
    {
      name: "key",
      kind: "text",
      label: "Config Key",
      description:
        "The configuration key to read (e.g. remote_url, identity).",
      required: true,
      placeholder: "remote_url",
    },
  ],
  resultKind: "json",
  keywords: ["config", "configuration", "setting", "get", "read", "key"],
};

export default manifest;
