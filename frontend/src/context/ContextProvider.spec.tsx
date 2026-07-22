import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const invokeMock = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

import {
  ContextProvider,
  useLoreContext,
} from "./ContextProvider";
import type { ContextSettings } from "./types";

interface Deferred<T> {
  promise: Promise<T>;
  resolve: (value: T) => void;
  reject: (reason?: unknown) => void;
}

function deferred<T>(): Deferred<T> {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

const projectOnePath = "C:/projects/game-lore";
const projectTwoPath = "C:/projects/cinematic-lore";

function fixture(activeProjectId: string | null = "project-1"): ContextSettings {
  return {
    schema_version: 1,
    servers: [
      {
        id: "server-1",
        alias: "EROS Lore",
        url: "lore://eros",
        source: "manual",
        favorite: true,
        auth_mode: "required",
        credential_ref: "windows-credential-manager://loregui/server/server-1",
        last_seen_at: "2026-07-21T12:00:00Z",
      },
      {
        id: "server-2",
        alias: "Local Lore",
        url: "lore://localhost",
        source: "hosted",
        favorite: false,
        auth_mode: "not_required",
        credential_ref: null,
        last_seen_at: null,
      },
    ],
    repositories: [
      {
        id: "repository-1",
        server_id: "server-1",
        display_name: "Game Lore",
        url: "lore://eros/game",
        favorite: true,
      },
      {
        id: "repository-2",
        server_id: "server-2",
        display_name: "Cinematic Lore",
        url: "lore://localhost/cinematic",
        favorite: false,
      },
    ],
    projects: [
      {
        id: "project-1",
        repository_id: "repository-1",
        display_name: "Game Lore",
        local_path: projectOnePath,
        branch: "persisted-branch",
        favorite: true,
        last_opened_at: "2026-07-21T12:00:00Z",
      },
      {
        id: "project-2",
        repository_id: "repository-2",
        display_name: "Cinematic Lore",
        local_path: projectTwoPath,
        branch: "cinematic",
        favorite: false,
        last_opened_at: "2026-07-21T12:00:00Z",
      },
    ],
    hosted_servers: [],
    active: {
      project_id: activeProjectId,
      server_id: activeProjectId === null ? null : "server-1",
      identity_ref: null,
    },
  };
}

function status(branch = "validated-main") {
  return {
    repo_id: "repository-id",
    branch,
    revision: "revision-id",
    changes: [],
    ahead: 0,
    behind: 0,
  };
}

function selectedProjectContext(
  projectId: "project-1" | "project-2",
): ContextSettings {
  const context = fixture(projectId);
  const project = context.projects.find((item) => item.id === projectId)!;
  const repository = context.repositories.find(
    (item) => item.id === project.repository_id,
  )!;
  return {
    ...context,
    active: {
      project_id: projectId,
      server_id: repository.server_id,
      identity_ref: null,
    },
  };
}

function selectedServerContext(serverId: string): ContextSettings {
  const context = fixture();
  return {
    ...context,
    active: {
      project_id: null,
      server_id: serverId,
      identity_ref: null,
    },
  };
}

function Probe() {
  const context = useLoreContext();
  return (
    <>
      <output data-testid="snapshot">
        {JSON.stringify({
          server: context.snapshot.server?.id ?? null,
          repository: context.snapshot.repository?.id ?? null,
          project: context.snapshot.project?.id ?? null,
          branch: context.snapshot.branch,
          authMode: context.snapshot.authMode,
          connection: context.snapshot.connection,
        })}
      </output>
      <output data-testid="unavailable">
        {[...context.unavailableProjectIds].sort().join(",")}
      </output>
      <output data-testid="validation-error">
        {context.validationError ?? ""}
      </output>
      <button onClick={() => void context.selectProject("project-1")}>
        Select project one
      </button>
      <button onClick={() => void context.selectProject("project-2")}>
        Select project two
      </button>
      <button onClick={() => void context.selectServer("server-2")}>
        Select server two
      </button>
      <button onClick={() => void context.refresh()}>Refresh context</button>
    </>
  );
}

function snapshot() {
  return JSON.parse(screen.getByTestId("snapshot").textContent ?? "{}") as {
    server: string | null;
    repository: string | null;
    project: string | null;
    branch: string | null;
    authMode: string;
    connection: string;
  };
}

beforeEach(() => {
  invokeMock.mockReset();
});

describe("ContextProvider", () => {
  it("restores a saved project only after P0 path and status validation", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "context_get") return Promise.resolve(fixture());
      if (command === "current_repository") return Promise.resolve(projectOnePath);
      if (command === "status") return Promise.resolve(status());
      return Promise.reject(new Error(`unexpected command: ${command}`));
    });

    render(
      <ContextProvider>
        <Probe />
      </ContextProvider>,
    );

    await waitFor(() => expect(snapshot().project).toBe("project-1"));
    expect(snapshot()).toEqual({
      server: "server-1",
      repository: "repository-1",
      project: "project-1",
      branch: "validated-main",
      authMode: "required",
      connection: "offline",
    });
    expect(screen.getByTestId("unavailable")).toHaveTextContent("");
    expect(invokeMock.mock.calls.map(([command]) => command)).toEqual([
      "context_get",
      "current_repository",
      "status",
    ]);
  });

  it("keeps repository actions closed and marks only the stale project unavailable", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "context_get") return Promise.resolve(fixture());
      if (command === "current_repository") return Promise.resolve(null);
      return Promise.reject(new Error(`must not probe ${command}`));
    });

    render(
      <ContextProvider>
        <Probe />
      </ContextProvider>,
    );

    await waitFor(() =>
      expect(screen.getByTestId("unavailable")).toHaveTextContent("project-1"),
    );
    expect(snapshot()).toEqual({
      server: null,
      repository: null,
      project: null,
      branch: null,
      authMode: "unknown",
      connection: "offline",
    });
    expect(screen.getByTestId("unavailable")).not.toHaveTextContent("project-2");
    expect(screen.getByTestId("validation-error")).toHaveTextContent(
      "Saved project is unavailable",
    );
    expect(invokeMock.mock.calls.some(([command]) => command === "status")).toBe(false);
    expect(invokeMock.mock.calls.some(([command]) => command === "open_repository")).toBe(false);
  });

  it("publishes a project from one authoritative atomic selection", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "context_get") return Promise.resolve(fixture(null));
      if (command === "context_select") {
        return Promise.resolve({
          context: selectedProjectContext("project-1"),
          active_repository: projectOnePath,
          status: status("selected-main"),
        });
      }
      return Promise.reject(new Error(`unexpected command: ${command}`));
    });

    render(
      <ContextProvider>
        <Probe />
      </ContextProvider>,
    );
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("context_get"));
    invokeMock.mockClear();
    fireEvent.click(screen.getByRole("button", { name: "Select project one" }));

    await waitFor(() => expect(snapshot().project).toBe("project-1"));
    expect(snapshot().branch).toBe("selected-main");
    expect(invokeMock).toHaveBeenCalledTimes(1);
    expect(invokeMock).toHaveBeenCalledWith("context_select", {
      target: { kind: "project", project_id: "project-1" },
      requestGeneration: 1,
    });
    expect(
      invokeMock.mock.calls.some(
        ([command]) => command === "open_repository" || command === "context_update",
      ),
    ).toBe(false);
  });

  it("keeps the prior snapshot when atomic selection persistence fails", async () => {
    const failedSelection = deferred<never>();
    invokeMock.mockImplementation((command: string) => {
      if (command === "context_get") return Promise.resolve(fixture());
      if (command === "current_repository") return Promise.resolve(projectOnePath);
      if (command === "status") return Promise.resolve(status());
      if (command === "context_select") return failedSelection.promise;
      return Promise.reject(new Error(`unexpected command: ${command}`));
    });

    render(
      <ContextProvider>
        <Probe />
      </ContextProvider>,
    );
    await waitFor(() => expect(snapshot().project).toBe("project-1"));
    invokeMock.mockClear();
    fireEvent.click(screen.getByRole("button", { name: "Select project two" }));
    await waitFor(() =>
      expect(
        invokeMock.mock.calls.filter(([command]) => command === "context_select"),
      ).toHaveLength(1),
    );
    await act(async () => {
      failedSelection.reject("failed with ghp_should-never-render");
    });

    await waitFor(() =>
      expect(screen.getByTestId("validation-error")).toHaveTextContent(
        "Could not save selected project",
      ),
    );
    expect(snapshot().project).toBe("project-1");
    expect(document.body).not.toHaveTextContent("ghp_should-never-render");
    expect(invokeMock.mock.calls.map(([command]) => command)).toEqual([
      "context_select",
    ]);
    expect(
      invokeMock.mock.calls.some(
        ([command]) => command === "open_repository" || command === "context_update",
      ),
    ).toBe(false);
  });

  it("lets B win when deferred A resolves after B", async () => {
    const selectionA = deferred<unknown>();
    const selectionB = deferred<unknown>();
    invokeMock.mockImplementation(
      (command: string, args?: { target?: { project_id?: string } }) => {
        if (command === "context_get") return Promise.resolve(fixture(null));
        if (command === "context_select") {
          return args?.target?.project_id === "project-1"
            ? selectionA.promise
            : selectionB.promise;
        }
        return Promise.reject(new Error(`unexpected command: ${command}`));
      },
    );

    render(
      <ContextProvider>
        <Probe />
      </ContextProvider>,
    );
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("context_get"));
    invokeMock.mockClear();
    fireEvent.click(screen.getByRole("button", { name: "Select project one" }));
    fireEvent.click(screen.getByRole("button", { name: "Select project two" }));

    await waitFor(() => expect(invokeMock).toHaveBeenCalledTimes(2));
    await act(async () => {
      selectionB.resolve({
        context: selectedProjectContext("project-2"),
        active_repository: projectTwoPath,
        status: status("branch-b"),
      });
      await selectionB.promise;
    });
    expect(snapshot().project).toBe("project-2");
    expect(snapshot().branch).toBe("branch-b");

    await act(async () => {
      selectionA.resolve({
        context: selectedProjectContext("project-1"),
        active_repository: projectOnePath,
        status: status("late-branch-a"),
      });
      await selectionA.promise;
    });
    expect(snapshot().project).toBe("project-2");
    expect(snapshot().branch).toBe("branch-b");
    expect(
      invokeMock.mock.calls.map(([, args]) => args.requestGeneration),
    ).toEqual([1, 2]);
    expect(
      invokeMock.mock.calls.some(
        ([command]) => command === "open_repository" || command === "context_update",
      ),
    ).toBe(false);
  });

  it("server selection closes the public repository snapshot", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "context_get") return Promise.resolve(fixture());
      if (command === "current_repository") return Promise.resolve(projectOnePath);
      if (command === "status") return Promise.resolve(status());
      if (command === "context_select") {
        return Promise.resolve({
          context: selectedServerContext("server-2"),
          active_repository: null,
          status: null,
        });
      }
      return Promise.reject(new Error(`unexpected command: ${command}`));
    });

    render(
      <ContextProvider>
        <Probe />
      </ContextProvider>,
    );
    await waitFor(() => expect(snapshot().project).toBe("project-1"));
    invokeMock.mockClear();
    fireEvent.click(screen.getByRole("button", { name: "Select server two" }));

    await waitFor(() => expect(snapshot().server).toBe("server-2"));
    expect(snapshot()).toEqual({
      server: "server-2",
      repository: null,
      project: null,
      branch: null,
      authMode: "not_required",
      connection: "offline",
    });
    expect(invokeMock).toHaveBeenCalledTimes(1);
    expect(invokeMock).toHaveBeenCalledWith("context_select", {
      target: { kind: "server", server_id: "server-2" },
      requestGeneration: 1,
    });
    expect(
      invokeMock.mock.calls.some(
        ([command]) => command === "open_repository" || command === "context_update",
      ),
    ).toBe(false);
  });
});
