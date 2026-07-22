import type { OpManifest } from "../../types";

const manifest: OpManifest = {
  id: "auth.clear",
  domain: "auth",
  op: "clear",
  label: "Auth: Clear all sessions",
  description: "Clear all locally stored auth sessions on this device.",
  command: "auth_clear",
  requiresRepository: false,
  args: [],
  resultKind: "void",
  keywords: ["clear", "signout", "auth", "sessions", "reset"],
};

export default manifest;
