import type { OpManifest } from "../../types";

/**
 * Manifest entry for `auth::clear`.
 *
 * Clears all stored authentication identities and tokens.
 * No arguments required; returns void on success.
 */
const manifest: OpManifest = {
  id: "auth.clear",
  domain: "auth",
  op: "clear",
  label: "Auth: Clear",
  description: "Clear all stored authentication identities and tokens.",
  command: "auth_clear",
  args: [],
  resultKind: "void",
  keywords: ["clear", "reset", "remove", "delete", "auth"],
};

export default manifest;
