import { fireEvent, render, screen, waitFor } from "@testing-library/react";
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

  it("validates and opens a project before persisting and publishing it", async () => {
    let runtimePath: string | null = null;
    invokeMock.mockImplementation((command: string, args?: { context?: ContextSettings; path?: string }) => {
      if (command === "context_get") return Promise.resolve(fixture(null));
      if (command === "context_validate") return Promise.resolve(args?.context);
      if (command === "open_repository") {
        runtimePath = args?.path ?? null;
        return Promise.resolve();
      }
      if (command === "current_repository") return Promise.resolve(runtimePath);
      if (command === "status") return Promise.resolve(status("selected-main"));
      if (command === "context_update") return Promise.resolve(args?.context);
      return Promise.reject(new Error(`unexpected command: ${command}`));
    });

    render(
      <ContextProvider>
        <Probe />
      </ContextProvider>,
    );
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("context_get"));
    fireEvent.click(screen.getByRole("button", { name: "Select project one" }));

    await waitFor(() => expect(snapshot().project).toBe("project-1"));
    expect(snapshot().branch).toBe("selected-main");
    const commands = invokeMock.mock.calls.map(([command]) => command);
    expect(commands).toEqual([
      "context_get",
      "context_validate",
      "open_repository",
      "current_repository",
      "status",
      "context_update",
    ]);
  });

  it("retains the previous active context and redacts IPC details when persistence fails", async () => {
    let runtimePath: string | null = projectOnePath;
    invokeMock.mockImplementation((command: string, args?: { context?: ContextSettings; path?: string }) => {
      if (command === "context_get") return Promise.resolve(fixture());
      if (command === "context_validate") return Promise.resolve(args?.context);
      if (command === "open_repository") {
        runtimePath = args?.path ?? null;
        return Promise.resolve();
      }
      if (command === "current_repository") return Promise.resolve(runtimePath);
      if (command === "status") return Promise.resolve(status());
      if (command === "context_update") {
        return Promise.reject("failed with ghp_should-never-render");
      }
      return Promise.reject(new Error(`unexpected command: ${command}`));
    });

    render(
      <ContextProvider>
        <Probe />
      </ContextProvider>,
    );
    await waitFor(() => expect(snapshot().project).toBe("project-1"));
    fireEvent.click(screen.getByRole("button", { name: "Select project two" }));

    await waitFor(() =>
      expect(screen.getByTestId("validation-error")).toHaveTextContent(
        "Could not save selected project",
      ),
    );
    expect(snapshot().project).toBe("project-1");
    expect(document.body).not.toHaveTextContent("ghp_should-never-render");
  });

  it("persists a server selection before publishing it without opening a repository", async () => {
    invokeMock.mockImplementation((command: string, args?: { context?: ContextSettings }) => {
      if (command === "context_get") return Promise.resolve(fixture(null));
      if (command === "context_validate") return Promise.resolve(args?.context);
      if (command === "context_update") return Promise.resolve(args?.context);
      return Promise.reject(new Error(`unexpected command: ${command}`));
    });

    render(
      <ContextProvider>
        <Probe />
      </ContextProvider>,
    );
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("context_get"));
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
    expect(invokeMock.mock.calls.map(([command]) => command)).toEqual([
      "context_get",
      "context_validate",
      "context_update",
    ]);
  });
});
