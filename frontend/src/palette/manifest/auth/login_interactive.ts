import type { OpManifest } from "../../types";

/**
 * Auth manifest entry — authenticate against a remote via browser-based flow.
 *
 * The palette renders a single optional `remoteUrl` field; when submitted,
 * it invokes `auth_login_interactive` which opens a browser window for OAuth.
 * On success, the result contains the user's identity ID and display name.
 */
const manifest: OpManifest = {
  id: "auth.login_interactive",
  domain: "auth",
  op: "login_interactive",
  label: "Auth: Login (Interactive)",
  description:
    "Authenticate against a remote via browser-based OAuth flow. Returns user identity on success.",
  command: "login_interactive",
  args: [
    {
      name: "remoteUrl",
      kind: "text",
      label: "Remote URL",
      description:
        "Remote API URL; empty resolves from the repository config.",
      required: false,
      placeholder: "https://api.example.com",
    },
  ],
  resultKind: "json",
  keywords: ["login", "auth", "sign-in", "browser", "oauth"],
};

export default manifest;
