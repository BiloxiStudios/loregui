import type { OpManifest } from "../../types";

/**
 * Manifest entry for auth.logout.
 *
 * Removes stored authentication and authorization tokens. Behavior depends
 * on which arguments are provided:
 * - auth_url empty: resolved from the current repository's remote config.
 * - user_id empty: removes all identities for the auth URL.
 * - user_id set, resource empty: removes the user's authentication token
 *   and all authorization tokens for the auth URL.
 * - user_id + resource set: removes only that specific authorization token.
 */
const manifest: OpManifest = {
  id: "auth.logout",
  domain: "auth",
  op: "logout",
  label: "Auth: Logout",
  description:
    "Remove stored authentication and authorization tokens for a remote.",
  command: "auth_logout",
  args: [
    {
      name: "authUrl",
      kind: "text",
      label: "Auth URL",
      description:
        "Auth service URL (e.g. ucs-auth://auth.example.com). Leave empty to resolve from repo config.",
      required: false,
      placeholder: "ucs-auth://auth.example.com",
    },
    {
      name: "resource",
      kind: "text",
      label: "Resource",
      description:
        "Resource ID (e.g. urc-{id}). Leave empty to remove all tokens for the auth URL.",
      required: false,
      placeholder: "urc-abc123",
    },
    {
      name: "userId",
      kind: "text",
      label: "User ID",
      description:
        "User identity to remove. Leave empty to remove all identities for the auth URL.",
      required: false,
      placeholder: "",
    },
  ],
  resultKind: "void",
  keywords: ["auth", "logout", "sign out", "token", "remove", "identity"],
};

export default manifest;
