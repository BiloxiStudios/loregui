import type { OpManifest } from "../../types";

/**
 * Manifest entry for `auth::resolve_user_info`.
 *
 * Resolves user IDs to display names using the remote authentication service.
 * Returns a list of resolved users with their display names.
 */
const manifest: OpManifest = {
  id: "auth.resolve_user_info",
  domain: "auth",
  op: "resolve_user_info",
  label: "Auth: Resolve User Info",
  description: "Resolve user IDs to display names using the remote authentication service.",
  command: "auth_user_info",
  args: [
    {
      name: "userIds",
      kind: "string-list",
      label: "User IDs",
      description: "User IDs to resolve; empty resolves the current user locally.",
      default: [],
      placeholder: "user-123\nuser-456",
    },
  ],
  resultKind: "json",
  keywords: ["resolve", "user", "info", "lookup", "auth"],
};

export default manifest;
