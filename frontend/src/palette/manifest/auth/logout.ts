import type { OpManifest } from "../../types";

/**
 * Manifest entry for `auth::logout`.
 *
 * Removes stored authentication and authorization tokens.
 * All arguments are optional with intelligent defaults for selective removal.
 */
const manifest: OpManifest = {
  id: "auth.logout",
  domain: "auth",
  op: "logout",
  label: "Auth: Logout",
  description: "Remove stored authentication and authorization tokens.",
  command: "auth_logout",
  args: [
    {
      name: "authUrl",
      kind: "text",
      label: "Auth URL",
      description: "Auth service URL (e.g., 'ucs-auth://auth.example.com'); empty resolves from the current repository's remote config.",
      default: "",
    },
    {
      name: "resource",
      kind: "text",
      label: "Resource",
      description: "Resource ID (e.g., 'urc-{id}'); empty removes all tokens for the auth URL.",
      default: "",
    },
    {
      name: "userId",
      kind: "text",
      label: "User ID",
      description: "User identity to remove; empty removes all identities for the auth URL.",
      default: "",
    },
  ],
  resultKind: "void",
  keywords: ["logout", "remove", "delete", "signout", "auth"],
};

export default manifest;
