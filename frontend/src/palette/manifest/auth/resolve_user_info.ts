import type { OpManifest } from "../../types";

/**
 * Command-palette manifest for auth.resolve_user_info.
 *
 * Resolves the currently authenticated user via the remote authentication
 * service. Returns user ID and display name if authenticated, or null if
 * no user session exists.
 */
const manifest: OpManifest = {
  id: "auth.resolve_user_info",
  domain: "auth",
  op: "resolve_user_info",
  label: "Auth: Resolve User Info",
  description: "Resolve the currently authenticated user's ID and display name.",
  command: "auth_user_info",
  requiresRepository: false,
  args: [],
  resultKind: "json",
  keywords: ["user", "whoami", "identity", "profile", "auth", "login"],
};

export default manifest;
