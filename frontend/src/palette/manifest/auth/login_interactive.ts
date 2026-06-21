import type { OpManifest } from "../../types";

/**
 * Manifest entry for `auth::login_interactive`.
 *
 * Authenticates against a remote via browser-based flow.
 * Returns user info and optionally the login URL (in no-browser mode).
 */
const manifest: OpManifest = {
  id: "auth.login_interactive",
  domain: "auth",
  op: "login_interactive",
  label: "Auth: Login Interactive",
  description: "Authenticate against a remote via browser-based interactive login.",
  command: "auth_login_interactive",
  args: [
    {
      name: "remoteUrl",
      kind: "text",
      label: "Remote URL",
      description: "Remote URL; empty resolves from the repository config.",
      default: "",
    },
    {
      name: "noBrowser",
      kind: "boolean",
      label: "No browser",
      description: "Emit the login URL instead of opening a browser.",
      default: false,
    },
  ],
  resultKind: "json",
  keywords: ["login", "authenticate", "browser", "interactive", "auth", "signin"],
};

export default manifest;
