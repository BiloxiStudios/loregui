import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";

const invokeMock = vi.fn();
const chooseDirectoryMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

vi.mock("../platform/directoryPicker", () => ({
  chooseDirectory: (...args: unknown[]) => chooseDirectoryMock(...args),
}));

import ClientClone from "./ClientClone";
import BackendPicker from "./server/BackendPicker";
import InitStore from "./server/InitStore";
import ServiceSetup from "./server/ServiceSetup";

const WINDOWS_PATH = "E:\\lore";

function revealAdvancedPathEntry() {
  fireEvent.click(screen.getByText("Advanced path entry"));
}

async function chooseWindowsDirectory() {
  fireEvent.click(screen.getByRole("button", { name: "Browse…" }));
  await waitFor(() => expect(screen.getByText(WINDOWS_PATH)).toBeInTheDocument());
  revealAdvancedPathEntry();
}

beforeEach(() => {
  invokeMock.mockReset();
  chooseDirectoryMock.mockReset();
  chooseDirectoryMock.mockResolvedValue(WINDOWS_PATH);
});

describe("ClientClone native directory selection", () => {
  it("preserves the selected clone destination in clone and open IPC", async () => {
    invokeMock.mockResolvedValue(undefined);
    render(<ClientClone initialMode="clone" />);

    fireEvent.change(screen.getByLabelText("Repository URL"), {
      target: { value: "lore://example/project" },
    });
    await chooseWindowsDirectory();

    expect(screen.getByLabelText("Destination Path")).toHaveValue(WINDOWS_PATH);
    fireEvent.click(screen.getByRole("button", { name: "Clone" }));

    await waitFor(() => expect(invokeMock).toHaveBeenCalledTimes(2));
    expect(invokeMock.mock.calls[0]).toEqual([
      "repository_clone",
      { url: "lore://example/project", dest: WINDOWS_PATH },
    ]);
    expect(invokeMock.mock.calls[1]).toEqual([
      "open_repository",
      { path: WINDOWS_PATH },
    ]);
  });

  it("preserves the selected open-existing path in IPC", async () => {
    invokeMock.mockResolvedValue(undefined);
    render(<ClientClone initialMode="open" />);

    await chooseWindowsDirectory();

    expect(screen.getByLabelText("Repository Path")).toHaveValue(WINDOWS_PATH);
    fireEvent.click(screen.getByRole("button", { name: "Open" }));

    await waitFor(() => expect(invokeMock).toHaveBeenCalledTimes(1));
    expect(invokeMock).toHaveBeenCalledWith("open_repository", {
      path: WINDOWS_PATH,
    });
  });

  it("preserves the selected create-local path in create and open IPC", async () => {
    invokeMock.mockImplementation((command: string) =>
      Promise.resolve(command === "repository_create" ? { id: "repo-id" } : undefined),
    );
    render(<ClientClone initialMode="create" />);

    fireEvent.change(screen.getByLabelText("Project name"), {
      target: { value: "world-bible" },
    });
    await chooseWindowsDirectory();

    expect(screen.getByLabelText("Local project path")).toHaveValue(WINDOWS_PATH);
    fireEvent.click(screen.getByRole("button", { name: "Create project" }));

    await waitFor(() => expect(invokeMock).toHaveBeenCalledTimes(2));
    expect(invokeMock.mock.calls[0]).toEqual([
      "repository_create",
      {
        path: WINDOWS_PATH,
        repositoryUrl: "lore://localhost/world-bible",
        description: "",
        id: "",
        useSharedStore: false,
        sharedStorePath: "",
      },
    ]);
    expect(invokeMock.mock.calls[1]).toEqual([
      "open_repository",
      { path: WINDOWS_PATH },
    ]);
  });

  it("keeps the existing value and invokes no backend action when cancelled", async () => {
    chooseDirectoryMock.mockResolvedValue(null);
    render(<ClientClone initialMode="open" />);
    revealAdvancedPathEntry();
    fireEvent.change(screen.getByLabelText("Repository Path"), {
      target: { value: "C:\\existing" },
    });

    fireEvent.click(screen.getByRole("button", { name: "Browse…" }));

    await waitFor(() => expect(chooseDirectoryMock).toHaveBeenCalledTimes(1));
    expect(screen.getByLabelText("Repository Path")).toHaveValue("C:\\existing");
    expect(invokeMock).not.toHaveBeenCalled();
  });
});

describe("server onboarding native directory selection", () => {
  it("preserves BackendPicker's selected local path in IPC", async () => {
    invokeMock.mockImplementation((command: string, args: { path: string }) => {
      if (command === "host_store_prepare") return Promise.resolve(args.path);
      return Promise.reject(new Error(`unexpected command ${command}`));
    });
    render(<BackendPicker />);

    await chooseWindowsDirectory();
    expect(screen.getByLabelText("Local Storage Path")).toHaveValue(WINDOWS_PATH);
    fireEvent.click(screen.getByRole("button", { name: "Prepare Store" }));

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("host_store_prepare", {
        path: WINDOWS_PATH,
        mutableStore: null,
      }),
    );
  });

  it("preserves InitStore's selected local path in IPC", async () => {
    invokeMock.mockImplementation((command: string, args: { path: string }) => {
      if (command === "host_store_prepare") return Promise.resolve(args.path);
      return Promise.reject(new Error(`unexpected command ${command}`));
    });
    render(<InitStore />);

    await chooseWindowsDirectory();
    expect(screen.getByLabelText("Store Path")).toHaveValue(WINDOWS_PATH);
    fireEvent.click(screen.getByRole("button", { name: "Create Store" }));

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("host_store_prepare", {
        path: WINDOWS_PATH,
        mutableStore: null,
      }),
    );
  });

  it("preserves ServiceSetup's selected local path in IPC", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "host_server_status") {
        return Promise.resolve({ running: false });
      }
      if (command === "host_server_start") {
        return Promise.resolve({ running: true, url: "lore://localhost/repo" });
      }
      return Promise.reject(new Error(`unexpected command ${command}`));
    });
    render(<ServiceSetup repoName="project" />);

    await chooseWindowsDirectory();
    expect(screen.getByLabelText("Store directory to serve")).toHaveValue(
      WINDOWS_PATH,
    );
    fireEvent.click(screen.getByRole("button", { name: "Start Hosting" }));

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith(
        "host_server_start",
        expect.objectContaining({ storeDir: WINDOWS_PATH }),
      ),
    );
  });
});
