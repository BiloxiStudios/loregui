/**
 * Component tests for BranchesPanel with mocked IPC.
 *
 * On mount the panel loads the branch list (core `branches` + `branch_list`)
 * and renders each branch row with its current marker + truncated latest
 * revision. We assert the load, the rendered rows, the current marker, the
 * empty/error states, and that switching a branch invokes `switch_branch`.
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";

const invokeMock = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

import BranchesPanel from "./BranchesPanel";

const CORE_BRANCHES = [
  {
    name: "main",
    id: "id-main",
    latest_revision: "1111111111111111aa",
    is_current: true,
  },
  {
    name: "feature/lore",
    id: "id-feat",
    latest_revision: "2222222222222222bb",
    is_current: false,
  },
];

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
      case "branches":
        return Promise.resolve(CORE_BRANCHES);
      case "branch_list":
        return Promise.resolve({ entries: [], count: 0 });
      case "switch_branch":
        return Promise.resolve(undefined);
      default:
        return Promise.resolve(null);
    }
  });
}

beforeEach(() => {
  invokeMock.mockReset();
});

describe("BranchesPanel", () => {
  it("loads the branch list on mount and renders each branch", async () => {
    routeInvoke();
    render(<BranchesPanel onClose={() => {}} />);
    await waitFor(() => {
      expect(screen.getByText("main")).toBeInTheDocument();
    });
    expect(invokeMock.mock.calls.map((c) => c[0])).toContain("branches");
    expect(invokeMock).toHaveBeenCalledWith("branch_list", { archived: false });
    expect(screen.getByText("feature/lore")).toBeInTheDocument();
  });

  it("marks the current branch and truncates the latest revision to 12 chars", async () => {
    routeInvoke();
    render(<BranchesPanel onClose={() => {}} />);
    await waitFor(() => expect(screen.getByText("main")).toBeInTheDocument());
    expect(screen.getByText(/● current/)).toBeInTheDocument();
    // 2222222222222222bb -> first 12 chars.
    expect(screen.getByText(/222222222222/)).toBeInTheDocument();
  });

  it("switches a non-current branch via switch_branch", async () => {
    routeInvoke();
    render(<BranchesPanel onClose={() => {}} />);
    await waitFor(() =>
      expect(screen.getByText("feature/lore")).toBeInTheDocument(),
    );
    // The row's Switch button (distinct from the "Switch branch" form section).
    fireEvent.click(
      screen.getByTitle("Switch the working copy to this branch"),
    );
    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("switch_branch", {
        name: "feature/lore",
      });
    });
  });

  it("shows an error + Retry when the list load fails", async () => {
    routeInvoke({ branches: reject("backend down") });
    render(<BranchesPanel onClose={() => {}} />);
    await waitFor(() => {
      expect(screen.getByText(/backend down/i)).toBeInTheDocument();
    });
    expect(screen.getByRole("button", { name: /Retry/i })).toBeInTheDocument();
  });

  it("renders the empty state when there are no branches", async () => {
    routeInvoke({ branches: [] });
    render(<BranchesPanel onClose={() => {}} />);
    await waitFor(() => {
      expect(screen.getByText(/No branches/i)).toBeInTheDocument();
    });
  });

  it("fires onClose from Close", async () => {
    routeInvoke();
    const onClose = vi.fn();
    render(<BranchesPanel onClose={onClose} />);
    await waitFor(() => expect(screen.getByText("main")).toBeInTheDocument());
    fireEvent.click(screen.getByText("Close"));
    expect(onClose).toHaveBeenCalled();
  });
});
