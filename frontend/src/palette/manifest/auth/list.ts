import type { OpManifest } from "../../types";

/**
 * Manifest entry for `auth::list`.
 *
 * Lists all stored authentication identities across all auth endpoints.
 * Optional boolean flag to include decrypted cached tokens.
 */
const manifest: OpManifest = {
  id: "auth.list",
  domain: "auth",
  op: "list",
  label: "Auth: List",
  description: "List all stored authentication identities.",
  command: "auth_list",
  args: [
    {
      name: "withToken",
      kind: "boolean",
      label: "Include tokens",
      description: "When true, include decrypted cached tokens in each identity entry.",
      default: false,
    },
  ],
  resultKind: "json",
  keywords: ["list", "show", "identities", "auth"],
};

export default manifest;
