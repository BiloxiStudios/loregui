import type { OpManifest } from "../../types";

/**
 * Manifest entry for `auth::login_with_token`.
 *
 * Authenticates against a remote using a provided token (e.g., JWT).
 * Returns user identity info on success.
 */
const manifest: OpManifest = {
  id: "auth.login_with_token",
  domain: "auth",
  op: "login_with_token",
  label: "Auth: Login With Token",
  description: "Authenticate against a remote using a provided token.",
  command: "auth_login_with_token",
  args: [
    {
      name: "remoteUrl",
      kind: "text",
      label: "Remote URL",
      description: "Remote URL; empty falls back to the repository config.",
      default: "",
    },
    {
      name: "token",
      kind: "text",
      label: "Token",
      description: "Authentication token (e.g., JWT).",
      required: true,
    },
    {
      name: "tokenType",
      kind: "text",
      label: "Token type",
      description: "Token type (e.g., 'Bearer', 'JWT').",
      default: "Bearer",
    },
    {
      name: "authUrl",
      kind: "text",
      label: "Auth URL",
      description: "Auth service URL with scheme; used directly when non-empty, required when no remote URL is available.",
      default: "",
    },
  ],
  resultKind: "json",
  keywords: ["login", "token", "authenticate", "jwt", "bearer", "auth", "signin"],
};

export default manifest;
