import type { OpManifest } from "../../types";

/**
 * Command-palette manifest for revision.activity_report.
 *
 * The data op behind the commercial **Reporting & Insights** add-on (SBAI-4061
 * under epic SBAI-4068). Produces an aggregated "who did what when" rollup over
 * the revision chain: per-revision author, message, timestamp, and changed
 * files, with optional contributor / date-window / file filters.
 *
 * NOTE: this palette entry is required by the parity ratchet (every registered
 * command must be reachable). The rich, gated experience lives in the Reporting
 * panel (ReportingPanel.tsx), which is dark unless `isEntitled("reporting")`.
 * Running this op from the palette only returns data (the same read-only history
 * already exposed by revision.history/info), so it is not separately locked here;
 * the label flags it as a premium surface.
 */
const manifest: OpManifest = {
  id: "revision.activity_report",
  domain: "revision",
  op: "activity_report",
  label: "Revision: Activity Report (Premium)",
  description:
    "Who-did-what-when rollup over the revision history — powers the premium Reporting & Insights add-on.",
  command: "revision_activity_report",
  surface: "panel",
  args: [
    {
      name: "revision",
      kind: "text",
      label: "Revision",
      description: "Start from this revision; empty for current HEAD.",
      required: false,
      placeholder: "e.g. abc123def",
    },
    {
      name: "branch",
      kind: "text",
      label: "Branch",
      description: "Restrict to this branch; empty for current.",
      required: false,
      placeholder: "e.g. main",
    },
    {
      name: "length",
      kind: "number",
      label: "Max revisions",
      description: "Maximum number of revisions to scan; 0 for unlimited.",
      required: false,
      default: 200,
    },
    {
      name: "author",
      kind: "text",
      label: "Contributor",
      description:
        "Only include revisions by an author whose name contains this substring.",
      required: false,
      placeholder: "e.g. bob",
    },
    {
      name: "dateFrom",
      kind: "number",
      label: "From (Unix seconds)",
      description: "Only include revisions at or after this timestamp; 0 = unbounded.",
      required: false,
      default: 0,
    },
    {
      name: "dateTo",
      kind: "number",
      label: "To (Unix seconds)",
      description: "Only include revisions at or before this timestamp; 0 = unbounded.",
      required: false,
      default: 0,
    },
    {
      name: "filePath",
      kind: "text",
      label: "File path",
      description: "Only include revisions that touched this file path.",
      required: false,
      placeholder: "e.g. Content/Maps/main.umap",
    },
  ],
  resultKind: "json",
  keywords: [
    "activity",
    "report",
    "insights",
    "who did what",
    "rollup",
    "contributor",
    "audit",
    "premium",
  ],
};

export default manifest;
