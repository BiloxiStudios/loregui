import type { OpManifest } from "../../types";

/**
 * Manifest entry for `auth::local_user_info`.
 *
 * Resolves user identities from locally cached JWT tokens.
 * Returns user info and optionally token details for identities with cached tokens.
 */
const manifest: OpManifest = {
  id: "auth.local_user_info",
  domain: "auth",
  op: "local_user_info",
  label: "Auth: Local User Info",
  description: "Resolve user identities from locally cached JWT tokens.",
  command: "auth_local_user_info",
  args: [
    {
      name: "authEndpoint",
      kind: "text",
      label: "Auth endpoint",
      description: "Auth service endpoint URL; empty resolves from repository remote config.",
      default: "",
    },
    {
      name: "userIds",
      kind: "string-list",
      label: "User IDs",
      description: "User IDs to resolve; empty resolves the current user.",
      default: [],
      placeholder: "user-123\nuser-456",
    },
    {
      name: "withToken",
      kind: "boolean",
      label: "Include tokens",
      description: "When true, emit token details for identities with a locally cached token.",
      default: false,
    },
  ],
  resultKind: "json",
  keywords: ["local", "user", "info", "jwt", "cache", "auth"],
};

export default manifest;
