import type { OpManifest } from "../../types";

/**
 * Command-palette manifest for file.reset_to_last_merged. Exercises the
 * `string-list`, `text`, and `boolean` field kinds; the result is rendered
 * as pretty-printed JSON.
 */
const manifest: OpManifest = {
  id: "file.reset_to_last_merged",
  domain: "file",
  op: "reset_to_last_merged",
  label: "File: Reset to Last Merged",
  description: "Reset one or more files to the state they were in at the last merged revision on a given branch.",
  command: "file_reset_to_last_merged",
  args: [
    {
      name: "paths",
      kind: "string-list",
      label: "Paths",
      description: "Repository-relative paths to reset. One path per line.",
      required: true,
      placeholder: "src/main.rs\nREADME.md",
    },
    {
      name: "branch",
      kind: "text",
      label: "Branch",
      description: "Branch whose last merged revision to reset to.",
      required: true,
      placeholder: "main",
    },
    {
      name: "purge",
      kind: "boolean",
      label: "Purge untracked files",
      description: "Whether to purge untracked files.",
      required: false,
      default: false,
    },
  ],
  resultKind: "json",
  keywords: ["reset", "revert", "merge", "branch"],
};

export default manifest;
