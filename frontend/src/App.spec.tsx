/**
 * First-run / no-repo robustness tests for the app shell (loregui #331).
 *
 * Regression target: on a fresh Windows install the working dir is the app-data
 * folder, which is NOT a lore repo, so `status` rejects with
 * `{ kind: "CommandFailed", message: "...Repository not found..." }`. Before the
 * fix this uncaught error crash-closed the React tree to a blank window. These
 * tests pin the new behavior:
 *   1. fresh install (no `loregui.onboarded`)        -> onboarding renders, no crash
 *   2. previously onboarded but no repo open          -> usable shell + "Set Up
 *                                                        Repository", no crash
 *   3. an UNEXPECTED throw on the shell path          -> ErrorBoundary recovery,
 *                                                        not a blank close
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";

const invokeMock = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

// App subscribes to tray/lock events on mount; give listen() a no-op unlisten.
vi.mock("@tauri-apps/api/event", () => ({
  listen: () => Promise.resolve(() => {}),
}));

import App from "./App";
import ErrorBoundary from "./ErrorBoundary";

// lore's "no repository here" signal, exactly as it reaches the frontend: a
// serialized LoreError::CommandFailed carrying "Repository not found".
const NOT_A_REPO = {
  kind: "CommandFailed",
  message:
    "`lore status` exited 1: [Error] Repository not found: C:/Users/MyUser/AppData/Local/LoreGUI",
};

/** Route invoke() by command name; `status` rejects as a non-repo by default. */
function routeInvoke(overrides: Record<string, unknown> = {}) {
  invokeMock.mockImplementation((cmd: string) => {
    if (cmd in overrides) {
      const v = overrides[cmd] as { __reject?: unknown };
      return v && typeof v === "object" && "__reject" in v
        ? Promise.reject(v.__reject)
        : Promise.resolve(v);
    }
    switch (cmd) {
      case "current_repository":
        return Promise.resolve("C:/Users/MyUser/AppData/Local/LoreGUI");
      case "status":
        return Promise.reject(NOT_A_REPO);
      case "branches":
        return Promise.resolve([]);
      case "log":
        return Promise.resolve([]);
      case "tray_sync_state":
        return Promise.resolve();
      case "lock_messaging_inbox_list":
        return Promise.resolve([]);
      default:
        return Promise.resolve(null);
    }
  });
}

beforeEach(() => {
  localStorage.clear();
  invokeMock.mockReset();
});

describe("App first-run / no-repo handling (#331)", () => {
  it("renders onboarding (not a crash) on a fresh install where status is not-a-repo", async () => {
    routeInvoke();
    render(<App />);

    // The onboarding mode-select must appear — the app stayed alive.
    expect(
      await screen.findByText(/Choose Your Setup Mode/i),
    ).toBeInTheDocument();

    // The raw CommandFailed JSON must NOT leak into the UI.
    expect(screen.queryByText(/CommandFailed/)).toBeNull();
    expect(screen.queryByText(/Repository not found/)).toBeNull();
  });

  it("keeps a usable shell with a re-entry path when onboarded but no repo is open", async () => {
    localStorage.setItem("loregui.onboarded", "true");
    routeInvoke();
    render(<App />);

    // Shell renders; the topbar shows no-repo state and an explicit way back to
    // setup — the user is never locked out.
    expect(await screen.findByText("no repository open")).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: /Set Up Repository/i }),
    ).toBeInTheDocument();
    // Settings is always reachable even with no repo.
    expect(screen.getByRole("button", { name: /^Settings$/ })).toBeInTheDocument();
    // No fatal error banner from the expected not-a-repo case.
    expect(screen.queryByText(/Repository not found/)).toBeNull();
  });

  it("the ErrorBoundary degrades an unexpected throw to a recovery state, not a blank close", () => {
    function Boom(): never {
      throw { kind: "CommandFailed", message: "boom from the shell" };
    }
    // Silence the expected React error log for this case.
    const spy = vi.spyOn(console, "error").mockImplementation(() => {});
    render(
      <ErrorBoundary>
        <Boom />
      </ErrorBoundary>,
    );
    expect(screen.getByText(/Something went wrong/i)).toBeInTheDocument();
    // The message is humanized, not raw JSON.
    expect(screen.getByText("boom from the shell")).toBeInTheDocument();
    expect(screen.queryByText(/"kind"/)).toBeNull();
    spy.mockRestore();
  });
});
