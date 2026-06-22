import type { OpManifest } from "../../types";

/**
 * Palette manifest for `lock.request_checkin` (SBAI-4044).
 *
 * Asks the current holder of a file's lock to check it in / release it. The
 * holder receives a tray notification + an inbox entry on their client. See
 * `docs/lock-messaging-spike.md` for the transport: the core build delivers
 * locally; cross-network delivery is the premium relay (SBAI-4072).
 */
const manifest: OpManifest = {
  id: "lock.request_checkin",
  domain: "lock",
  op: "request_checkin",
  label: "Lock: Request Check-In",
  description:
    "Ask the holder of a locked file to check it in. They get a tray notification and an inbox entry with Release / Dismiss.",
  command: "lock_request_checkin",
  args: [
    {
      name: "path",
      kind: "text",
      label: "File path",
      description: "The locked file you want released.",
      required: true,
      placeholder: "Content/Characters/hero.uasset",
    },
    {
      name: "branch",
      kind: "text",
      label: "Branch",
      description: "Branch the lock is on; leave empty for the current branch.",
      required: false,
      placeholder: "e.g. main",
    },
    {
      name: "from",
      kind: "text",
      label: "Your name",
      description: "Who the request is from (shown to the holder).",
      required: true,
      placeholder: "e.g. you@example.com",
    },
    {
      name: "holder",
      kind: "text",
      label: "Holder",
      description: "Display name of the current lock holder.",
      required: true,
      placeholder: "e.g. teammate@example.com",
    },
    {
      name: "toUserId",
      kind: "text",
      label: "Holder user ID",
      description: "User id of the holder (from the lock's owner field).",
      required: true,
      placeholder: "e.g. 12345",
    },
    {
      name: "note",
      kind: "text",
      label: "Note",
      description: "Optional message to the holder.",
      required: false,
      placeholder: "Need this for the merge — thanks!",
    },
  ],
  resultKind: "json",
  keywords: ["lock", "request", "check-in", "checkin", "nudge", "ask", "release"],
};

export default manifest;
