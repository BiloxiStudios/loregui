/**
 * Component tests for RepositoryPanel with mocked IPC.
 *
 * The panel loads the current-repo identity on mount (current_repository +
 * status) and renders loading / error / empty / populated states. We mock
 * `@tauri-apps/api/core` so the panel's `api.*` calls resolve against canned
 * data, and assert the rendered identity + that Close/Esc fire onClose.
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";

const invokeMock = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

import RepositoryPanel from "./RepositoryPanel";

// A rejection marker: lore command errors arrive as plain strings, so tests
// model failures by rejecting with a raw string (not an Error object).
function reject(message: string) {
  return { __reject: message };
}

// Route invoke by command name so mount-time Promise.all resolves correctly.
function routeInvoke(overrides: Record<string, unknown> = {}) {
  invokeMock.mockImplementation((cmd: string) => {
    if (cmd in overrides) {
      const v = overrides[cmd] as { __reject?: string };
      return v && v.__reject !== undefined
        ? Promise.reject(v.__reject)
        : Promise.resolve(v);
    }
    switch (cmd) {
      case "current_repository":
        return Promise.resolve("/disk/world-bible");
      case "status":
        return Promise.resolve({
          repo_id: "abcdef0123456789aa",
          branch: "main",
          revision: "deadbeefcafef00d11",
          changes: [],
          ahead: 0,
          behind: 0,
        });
      default:
        return Promise.resolve(null);
    }
  });
}

beforeEach(() => {
  invokeMock.mockReset();
});

describe("RepositoryPanel", () => {
  it("loads + renders the current repository identity on mount", async () => {
    routeInvoke();
    render(<RepositoryPanel onClose={() => {}} />);

    await waitFor(() => {
      expect(screen.getByText("/disk/world-bible")).toBeInTheDocument();
    });
    const called = invokeMock.mock.calls.map((c) => c[0]);
    expect(called).toContain("current_repository");
    expect(called).toContain("status");
    // Repo id is truncated to 16 chars by the panel.
    expect(screen.getByText("abcdef0123456789")).toBeInTheDocument();
    expect(screen.getByText("main")).toBeInTheDocument();
  });

  it("shows an error + Retry when identity load fails", async () => {
    routeInvoke({ current_repository: reject("no repo open") });
    render(<RepositoryPanel onClose={() => {}} />);
    await waitFor(() => {
      expect(
        screen.getByRole("button", { name: /Retry/i }),
      ).toBeInTheDocument();
    });
  });

  it("fires onClose from the Close button", async () => {
    routeInvoke();
    const onClose = vi.fn();
    render(<RepositoryPanel onClose={onClose} />);
    await waitFor(() =>
      expect(screen.getByText("/disk/world-bible")).toBeInTheDocument(),
    );
    fireEvent.click(screen.getByTitle(/Close/i));
    expect(onClose).toHaveBeenCalled();
  });

  it("fires onClose on Escape", async () => {
    routeInvoke();
    const onClose = vi.fn();
    render(<RepositoryPanel onClose={onClose} />);
    await waitFor(() =>
      expect(screen.getByText("/disk/world-bible")).toBeInTheDocument(),
    );
    fireEvent.keyDown(window, { key: "Escape" });
    expect(onClose).toHaveBeenCalled();
  });

  it("has an accessible dialog role + label", async () => {
    routeInvoke();
    render(<RepositoryPanel onClose={() => {}} />);
    expect(
      screen.getByRole("dialog", { name: /Repository management/i }),
    ).toBeInTheDocument();
    // Await the mount-time identity load so it doesn't fire act() warnings.
    await waitFor(() => expect(screen.queryByText("Loading…")).not.toBeInTheDocument());
  });
});
