import type { OpManifest } from "../../types";

/** Authenticate against a remote using a bearer token (non-interactive). */
const manifest: OpManifest = {
  id: "auth.login_with_token",
  domain: "auth",
  op: "login_with_token",
  label: "Auth: Login with Token",
  description:
    "Authenticate against a remote using a bearer token, without a browser; returns the signed-in user.",
  command: "auth_login_with_token",
  args: [
    {
      name: "remoteUrl",
      kind: "text",
      label: "Remote URL",
      description: "Remote API URL to authenticate against.",
      required: true,
      placeholder: "https://api.example.com",
    },
    {
      name: "token",
      kind: "text",
      label: "Token",
      description: "Bearer token to authenticate with.",
      required: true,
      placeholder: "paste your token",
    },
  ],
  resultKind: "json",
  keywords: ["login", "auth", "token", "bearer", "sign-in", "headless"],
};

export default manifest;
