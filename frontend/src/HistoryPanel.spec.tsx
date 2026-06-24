/**
 * Component tests for HistoryPanel with mocked IPC.
 *
 * On mount the panel loads the revision history of the current branch
 * (`revision_history`) and renders each revision row with its truncated hash +
 * revision number + merge marker. Selecting a revision loads its info + diff.
 * We assert the mount load, the rendered rows, the empty/error states, and that
 * selecting a revision fans out to `revision_info` + `revision_diff`.
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";

const invokeMock = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

import HistoryPanel from "./HistoryPanel";

const HISTORY = {
  entries: [
    { revision: "aaaaaaaaaaaaaaaa11", revision_number: 2, parents: ["p0", "p1"] },
    { revision: "bbbbbbbbbbbbbbbb22", revision_number: 1, parents: [] },
  ],
};

// lore command errors arrive as plain strings; model failures by rejecting
// with a raw string rather than an Error object.
function reject(message: string) {
  return { __reject: message };
}

function routeInvoke(overrides: Record<string, unknown> = {}) {
  invokeMock.mockImplementation((cmd: string) => {
    if (cmd in overrides) {
      const v = overrides[cmd] as { __reject?: string };
      return v && v.__reject !== undefined
        ? Promise.reject(v.__reject)
        : Promise.resolve(v);
    }
    switch (cmd) {
      case "revision_history":
        return Promise.resolve(HISTORY);
      case "revision_info":
        return Promise.resolve({ info: null, deltas: [], metadata: [] });
      case "revision_diff":
        return Promise.resolve({ files: [] });
      default:
        return Promise.resolve(null);
    }
  });
}

beforeEach(() => {
  invokeMock.mockReset();
});

describe("HistoryPanel", () => {
  it("loads + renders the revision history on mount", async () => {
    routeInvoke();
    render(<HistoryPanel onClose={() => {}} />);
    await waitFor(() => {
      expect(screen.getByText("aaaaaaaaaaaa")).toBeInTheDocument();
    });
    expect(invokeMock).toHaveBeenCalledWith(
      "revision_history",
      expect.objectContaining({ branch: "", onlyBranch: false }),
    );
    // Revision number + the merge marker (2 parents).
    expect(screen.getByText(/#2 · merge/)).toBeInTheDocument();
    expect(screen.getByText(/#1$/)).toBeInTheDocument();
  });

  it("selecting a revision loads its info + diff", async () => {
    routeInvoke();
    render(<HistoryPanel onClose={() => {}} />);
    await waitFor(() =>
      expect(screen.getByText("aaaaaaaaaaaa")).toBeInTheDocument(),
    );
    fireEvent.click(screen.getAllByRole("button", { name: /Details/i })[0]);
    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith(
        "revision_info",
        expect.objectContaining({ revision: "aaaaaaaaaaaaaaaa11" }),
      );
    });
    expect(invokeMock).toHaveBeenCalledWith(
      "revision_diff",
      expect.objectContaining({ revisionSource: "aaaaaaaaaaaaaaaa11" }),
    );
  });

  it("renders the empty state when there are no revisions", async () => {
    routeInvoke({ revision_history: { entries: [] } });
    render(<HistoryPanel onClose={() => {}} />);
    await waitFor(() => {
      expect(screen.getByText(/No revisions yet/i)).toBeInTheDocument();
    });
  });

  it("shows an error when the history load fails", async () => {
    routeInvoke({ revision_history: reject("walk failed") });
    render(<HistoryPanel onClose={() => {}} />);
    await waitFor(() => {
      expect(screen.getByText(/walk failed/i)).toBeInTheDocument();
    });
  });

  it("fires onClose on Escape", async () => {
    routeInvoke();
    const onClose = vi.fn();
    render(<HistoryPanel onClose={onClose} />);
    await waitFor(() =>
      expect(screen.getByText("aaaaaaaaaaaa")).toBeInTheDocument(),
    );
    fireEvent.keyDown(window, { key: "Escape" });
    expect(onClose).toHaveBeenCalled();
  });
});
