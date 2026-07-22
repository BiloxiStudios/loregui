/**
 * First-run / no-repo robustness tests for the app shell (loregui #331).
 *
 * Regression target: on a fresh install no repository is active, so
 * `current_repository` resolves to `null` and `status` rejects with the exact
 * `{ kind: "NoRepository", message: "no repository is open" }` contract. Before the
 * fix this uncaught error crash-closed the React tree to a blank window. These
 * tests pin the new behavior:
 *   1. fresh install (no `loregui.onboarded`)        -> onboarding renders, no crash
 *   2. previously onboarded but no repo open          -> usable shell + "Set Up
 *                                                        Repository", no crash
 *   3. an UNEXPECTED throw on the shell path          -> ErrorBoundary recovery,
 *                                                        not a blank close
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

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

// Lore's "no repository open" signal, exactly as it reaches the frontend.
const NOT_A_REPO = {
  kind: "NoRepository",
  message: "no repository is open",
};

const BACKEND_REPOSITORY_NOT_FOUND = {
  kind: "CommandFailed",
  message: "Repository not found: C:/missing/lore-repository",
};

const VALID_STATUS = {
  repo_id: "repo-123",
  branch: "main",
  revision: "abc123",
  changes: [],
  ahead: 0,
  behind: 0,
};

const REPOSITORY_ACTIONS = [
  "Branches",
  "History",
  "Locks",
  "Manage",
  "Dependencies",
  "Sync",
  "Push",
  "Verify",
  "Flush",
  "GC",
  "Metadata",
] as const;

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
        return Promise.resolve(null);
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
      case "lan_discover_browse":
      case "lan_discover_refresh":
        return Promise.resolve([]);
      case "lan_discover_stop":
        return Promise.resolve();
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

    // The expected typed startup signal must not leak into the UI.
    expect(screen.queryByText(/NoRepository/)).toBeNull();
    expect(screen.queryByText(/no repository is open/)).toBeNull();
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
    expect(screen.queryByText(/^no repository is open$/)).toBeNull();
  });

  it("surfaces CommandFailed repository-not-found errors instead of classifying them as startup", async () => {
    localStorage.setItem("loregui.onboarded", "true");
    routeInvoke({ status: { __reject: BACKEND_REPOSITORY_NOT_FOUND } });
    render(<App />);

    expect(
      await screen.findByText(BACKEND_REPOSITORY_NOT_FOUND.message),
    ).toBeInTheDocument();
  });

  it("surfaces current_repository failures instead of treating them as empty state", async () => {
    const currentRepositoryFailure = {
      kind: "CommandFailed",
      message: "current repository state is unavailable",
    };
    localStorage.setItem("loregui.onboarded", "true");
    routeInvoke({
      current_repository: { __reject: currentRepositoryFailure },
    });
    render(<App />);

    expect(
      await screen.findByText(currentRepositoryFailure.message),
    ).toBeInTheDocument();
    expect(
      invokeMock.mock.calls.some(([command]) => command === "status"),
    ).toBe(false);
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

describe("repository action guard", () => {
  it("renders a guided project hub and blocks every visible repository action without validated context", async () => {
    localStorage.setItem("loregui.onboarded", "true");
    routeInvoke();
    const user = userEvent.setup();
    render(<App />);

    expect(
      await screen.findByRole("heading", { name: "Choose a project" }),
    ).toBeVisible();
    for (const action of ["Open existing", "Create local", "Connect", "Host"]) {
      expect(screen.getByRole("button", { name: action })).toBeEnabled();
    }
    expect(screen.getByRole("button", { name: "Lock requests" })).toBeEnabled();
    expect(screen.getByRole("button", { name: "List Repos" })).toBeEnabled();

    await waitFor(() => {
      for (const action of REPOSITORY_ACTIONS) {
        expect(screen.getByRole("button", { name: action })).toBeDisabled();
      }
    });

    invokeMock.mockClear();
    for (const action of REPOSITORY_ACTIONS) {
      await user.click(screen.getByRole("button", { name: action }));
    }

    expect(invokeMock).not.toHaveBeenCalled();
    expect(screen.queryByRole("dialog", { name: "Branches" })).toBeNull();
    expect(screen.queryByRole("dialog", { name: "History" })).toBeNull();
    expect(screen.queryByRole("dialog", { name: "Locks" })).toBeNull();
    expect(screen.queryByRole("dialog", { name: "Dependencies" })).toBeNull();
    expect(
      screen.queryByRole("dialog", { name: "Repository management" }),
    ).toBeNull();
    expect(screen.queryByText(/AppData/i)).toBeNull();
    expect(screen.queryByText(/process (current )?working directory/i)).toBeNull();
  });

  it("fails closed when status is valid but the current repository path is absent", async () => {
    localStorage.setItem("loregui.onboarded", "true");
    routeInvoke({ status: VALID_STATUS });
    render(<App />);

    expect(
      await screen.findByRole("heading", { name: "Choose a project" }),
    ).toBeVisible();
    expect(screen.getByRole("button", { name: "Sync" })).toBeDisabled();
  });

  it.each([
    ["Open existing", "Open Working Tree"],
    ["Create local", "Create Local Project"],
    ["Connect", "Connect to Server"],
    ["Host", "Choose Storage Backend"],
  ])(
    "%s opens its distinct %s flow",
    async (action, destination) => {
      localStorage.setItem("loregui.onboarded", "true");
      routeInvoke();
      const user = userEvent.setup();
      render(<App />);

      await user.click(
        await screen.findByRole("button", { name: action }),
      );

      expect(
        await screen.findByRole("heading", { name: destination }),
      ).toBeVisible();
    },
  );

  it("Create local creates and validates the chosen project before enabling the shell", async () => {
    localStorage.setItem("loregui.onboarded", "true");
    routeInvoke({
      repository_create: {
        id: "repo-new",
        name: "world-bible",
        path: "C:/projects/world-bible",
      },
      open_repository: null,
    });
    const user = userEvent.setup();
    render(<App />);

    await user.click(
      await screen.findByRole("button", { name: "Create local" }),
    );
    await user.type(screen.getByLabelText("Project name"), "world-bible");
    await user.type(
      screen.getByLabelText("Local project path"),
      "C:/projects/world-bible",
    );
    await user.click(screen.getByRole("button", { name: "Create project" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith(
        "repository_create",
        expect.objectContaining({
          path: "C:/projects/world-bible",
          repositoryUrl: "lore://localhost/world-bible",
        }),
      );
      expect(invokeMock).toHaveBeenCalledWith("open_repository", {
        path: "C:/projects/world-bible",
      });
    });
  });

  it("enables repository IPC and panel launchers only with a path plus validated status", async () => {
    localStorage.setItem("loregui.onboarded", "true");
    routeInvoke({
      current_repository: "C:/projects/world-bible",
      status: VALID_STATUS,
      revision_sync: {
        files: [],
        revisions: [],
        files_updated: 0,
        files_deleted: 0,
      },
    });
    const user = userEvent.setup();
    render(<App />);

    const sync = await screen.findByRole("button", { name: "Sync" });
    expect(sync).toBeEnabled();
    await user.click(sync);
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith(
        "revision_sync",
        expect.any(Object),
      ),
    );

    const manage = screen.getByRole("button", { name: "Manage" });
    expect(manage).toBeEnabled();
    await user.click(manage);
    expect(
      await screen.findByRole("dialog", { name: "Repository management" }),
    ).toBeVisible();
  });
});
