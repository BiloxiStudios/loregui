import type { OpManifest } from "../../types";

const manifest: OpManifest = {
  id: "auth.logout",
  domain: "auth",
  op: "logout",
  label: "Auth: Sign out",
  description: "Sign out of a server, clearing its stored session.",
  command: "auth_logout",
  args: [
    { name: "authUrl", kind: "text", label: "Server URL", required: true },
    { name: "resource", kind: "text", label: "Resource", required: false },
    { name: "userId", kind: "text", label: "User ID", required: false },
  ],
  resultKind: "void",
  keywords: ["logout", "signout", "auth", "session"],
};

export default manifest;
