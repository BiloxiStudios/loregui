/**
 * First-run / no-repo robustness tests for the app shell (loregui #331).
 *
 * Regression target: on a fresh install no repository is active, so
 * `current_repository` resolves to `null`. The shell must stop there and never
 * issue `status`/`branches`/`log` against an implicit process CWD. Before the
 * fix that probe could target AppData/System32 and crash-close the React tree.
 * These tests pin the new behavior:
 *   1. fresh install (no `loregui.onboarded`)        -> onboarding renders, no crash
 *   2. previously onboarded but no repo open          -> usable shell + "Set Up
 *                                                        Repository", no crash
 *   3. an UNEXPECTED throw on the shell path          -> ErrorBoundary recovery,
 *                                                        not a blank close
 */
import { afterEach, describe, it, expect, vi, beforeEach } from "vitest";
import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
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
      case "host_server_status":
        return Promise.resolve({ running: false });
      default:
        return Promise.resolve(null);
    }
  });
}

beforeEach(() => {
  localStorage.clear();
  invokeMock.mockReset();
});

afterEach(() => {
  vi.useRealTimers();
});

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

describe("App first-run / no-repo handling (#331)", () => {
  it("renders onboarding without invoking repository-scoped IPC when no repository path exists", async () => {
    routeInvoke();
    render(<App />);

    // The onboarding mode-select must appear — the app stayed alive.
    expect(
      await screen.findByText(/Choose Your Setup Mode/i),
    ).toBeInTheDocument();

    // The expected typed startup signal must not leak into the UI.
    expect(screen.queryByText(/NoRepository/)).toBeNull();
    expect(screen.queryByText(/no repository is open/)).toBeNull();

    const commands = invokeMock.mock.calls.map(([command]) => command);
    expect(commands).toContain("current_repository");
    expect(commands).not.toContain("status");
    expect(commands).not.toContain("branches");
    expect(commands).not.toContain("log");
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
    routeInvoke({
      current_repository: "C:/missing/lore-repository",
      status: { __reject: BACKEND_REPOSITORY_NOT_FOUND },
    });
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
  it("browses the hosted URL through the real repository-list surface without prompting", async () => {
    localStorage.setItem("loregui.onboarded", "true");
    const hostedUrl = "lore://192.168.1.8:41337/world-bible";
    routeInvoke({
      host_server_status: {
        running: true,
        pid: 4242,
        port: 41337,
        httpPort: 41339,
        url: "lore://127.0.0.1:41337/world-bible",
        advertisedUrl: hostedUrl,
        storeDir: "E:\\lore",
        serverName: "world-bible",
        authRequired: false,
      },
      repository_list: {
        url: hostedUrl,
        entries: [{ id: "repo-world-bible", name: "world-bible" }],
      },
    });
    const promptSpy = vi.spyOn(window, "prompt");
    const user = userEvent.setup();
    render(<App />);

    await user.click(
      await screen.findByRole("button", { name: "Browse repositories" }),
    );

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("repository_list", {
        url: hostedUrl,
      }),
    );
    expect(promptSpy).not.toHaveBeenCalled();
    expect(await screen.findByText(`Repositories at ${hostedUrl}`)).toBeVisible();
    expect(screen.getByText("repo-world-b")).toBeVisible();
    expect(screen.getAllByText(/world-bible/).length).toBeGreaterThan(1);
    expect(
      screen.getByRole("button", { name: "Close repository list" }),
    ).toBeVisible();
    promptSpy.mockRestore();
  });

  it.each(["stopped", "url-changed"] as const)(
    "suppresses deferred hosted Browse results after the server becomes %s",
    async (nextKind) => {
      vi.useFakeTimers();
      localStorage.setItem("loregui.onboarded", "true");
      const hostedUrl = "lore://192.168.1.8:41337/world-bible";
      const nextUrl = "lore://192.168.1.9:41337/world-bible";
      const list = deferred<{
        url: string;
        entries: Array<{ id: string; name: string }>;
      }>();
      let statusCalls = 0;
      invokeMock.mockImplementation((command: string) => {
        if (command === "current_repository") return Promise.resolve(null);
        if (command === "status") return Promise.reject(NOT_A_REPO);
        if (command === "host_server_status") {
          statusCalls += 1;
          if (statusCalls === 1) {
            return Promise.resolve({
              running: true,
              pid: 4242,
              url: "lore://127.0.0.1:41337/world-bible",
              advertisedUrl: hostedUrl,
              storeDir: "E:\\lore",
              serverName: "world-bible",
              authRequired: false,
            });
          }
          return Promise.resolve(
            nextKind === "stopped"
              ? { running: false }
              : {
                  running: true,
                  pid: 4243,
                  url: "lore://127.0.0.1:41337/world-bible",
                  advertisedUrl: nextUrl,
                  storeDir: "E:\\lore",
                  serverName: "world-bible",
                  authRequired: false,
                },
          );
        }
        if (command === "repository_list") return list.promise;
        if (command === "branches" || command === "log") return Promise.resolve([]);
        if (command === "tray_sync_state") return Promise.resolve();
        if (command === "lock_messaging_inbox_list") return Promise.resolve([]);
        return Promise.resolve(null);
      });
      render(<App />);
      await act(async () => {
        await Promise.resolve();
      });
      fireEvent.click(screen.getByRole("button", { name: "Browse repositories" }));

      await act(async () => {
        await vi.advanceTimersByTimeAsync(5_000);
        list.resolve({
          url: hostedUrl,
          entries: [{ id: "stale-repo", name: "stale-world" }],
        });
        await Promise.resolve();
      });
      expect(screen.queryByLabelText("Remote repository browser")).toBeNull();
      expect(screen.queryByText("stale-world")).toBeNull();
      vi.useRealTimers();
    },
  );

  it("keeps the manual List Repos prompt for an operator-entered remote", async () => {
    localStorage.setItem("loregui.onboarded", "true");
    routeInvoke({
      repository_list: { url: "lore://manual/repo", entries: [] },
    });
    const promptSpy = vi
      .spyOn(window, "prompt")
      .mockReturnValue("lore://manual/repo");
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "List Repos" }));
    expect(promptSpy).toHaveBeenCalledTimes(1);
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("repository_list", {
        url: "lore://manual/repo",
      }),
    );
    promptSpy.mockRestore();
  });

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
    expect(screen.queryByLabelText("Hosted server")).toBeNull();
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
    const dialog = await screen.findByRole("dialog", {
      name: "Repository management",
    });
    expect(dialog).toBeVisible();
    expect(
      await within(dialog).findByLabelText("Hosted server"),
    ).toBeVisible();
  });
});
