import type { OpManifest } from "../../types";

/**
 * Manifest entry for auth.local_user_info.
 *
 * Resolves user identities from locally cached JWT tokens. Returns user info
 * and optionally token details for identities with cached credentials.
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
      label: "Auth Endpoint",
      required: false,
      placeholder: "ucs-auth://auth.example.com",
    },
    {
      name: "userIds",
      kind: "string-list",
      label: "User IDs",
      required: false,
      default: [],
    },
    {
      name: "withToken",
      kind: "boolean",
      label: "With Token",
      required: false,
      default: false,
    },
  ],
  resultKind: "json",
  keywords: ["auth", "user", "local", "jwt", "token"],
};

export default manifest;
