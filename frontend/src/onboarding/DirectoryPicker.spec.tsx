import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";

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

function fieldByLabel(label: string): HTMLElement {
  const field = screen
    .getByLabelText(label)
    .closest<HTMLElement>(".onboarding-field");
  if (!field) throw new Error(`missing onboarding field for ${label}`);
  return field;
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

    const primaryField = fieldByLabel("Local Storage Path");
    fireEvent.click(within(primaryField).getByRole("button", { name: "Browse…" }));
    await waitFor(() =>
      expect(within(primaryField).getByText(WINDOWS_PATH)).toBeInTheDocument(),
    );
    fireEvent.click(within(primaryField).getByText("Advanced path entry"));
    expect(within(primaryField).getByLabelText("Local Storage Path")).toHaveValue(
      WINDOWS_PATH,
    );
    fireEvent.click(screen.getByRole("button", { name: "Prepare Store" }));

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("host_store_prepare", {
        path: WINDOWS_PATH,
        mutableStore: null,
      }),
    );
  });

  it("preserves BackendPicker's selected mutable-store directory in local IPC", async () => {
    invokeMock.mockImplementation((command: string, args: { path: string }) => {
      if (command === "host_store_prepare") return Promise.resolve(args.path);
      return Promise.reject(new Error(`unexpected command ${command}`));
    });
    render(<BackendPicker />);

    const primaryField = fieldByLabel("Local Storage Path");
    const mutableField = fieldByLabel("Mutable Store Path (optional)");

    fireEvent.click(within(primaryField).getByText("Advanced path entry"));
    fireEvent.change(within(primaryField).getByLabelText("Local Storage Path"), {
      target: { value: "C:\\primary" },
    });
    fireEvent.click(within(mutableField).getByRole("button", { name: "Browse…" }));
    await waitFor(() =>
      expect(within(mutableField).getByText(WINDOWS_PATH)).toBeInTheDocument(),
    );
    fireEvent.click(within(mutableField).getByText("Advanced path entry"));
    expect(
      within(mutableField).getByLabelText("Mutable Store Path (optional)"),
    ).toHaveValue(WINDOWS_PATH);
    fireEvent.click(screen.getByRole("button", { name: "Prepare Store" }));

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("host_store_prepare", {
        path: "C:\\primary",
        mutableStore: WINDOWS_PATH,
      }),
    );
  });

  it("preserves BackendPicker's selected mutable-store directory in S3 config", async () => {
    invokeMock.mockResolvedValue(undefined);
    render(<BackendPicker />);

    fireEvent.click(screen.getByRole("radio", { name: /S3-compatible/ }));
    fireEvent.change(screen.getByLabelText("Endpoint URL"), {
      target: { value: "https://s3.example.com" },
    });
    fireEvent.change(screen.getByLabelText("Bucket Name"), {
      target: { value: "lore" },
    });
    const mutableField = fieldByLabel("Mutable Store Path (optional)");

    fireEvent.click(within(mutableField).getByRole("button", { name: "Browse…" }));
    await waitFor(() =>
      expect(within(mutableField).getByText(WINDOWS_PATH)).toBeInTheDocument(),
    );
    fireEvent.click(screen.getByRole("button", { name: "Open Storage" }));

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("storage_open", {
        config: expect.objectContaining({ mutableStore: WINDOWS_PATH }),
      }),
    );
  });

  it("keeps the mutable-store value and invokes no backend action when cancelled", async () => {
    chooseDirectoryMock.mockResolvedValue(null);
    render(<BackendPicker />);
    const mutableField = fieldByLabel("Mutable Store Path (optional)");

    fireEvent.click(within(mutableField).getByText("Advanced path entry"));
    fireEvent.change(
      within(mutableField).getByLabelText("Mutable Store Path (optional)"),
      { target: { value: "D:\\mutable-existing" } },
    );
    fireEvent.click(within(mutableField).getByRole("button", { name: "Browse…" }));

    await waitFor(() => expect(chooseDirectoryMock).toHaveBeenCalledTimes(1));
    expect(
      within(mutableField).getByLabelText("Mutable Store Path (optional)"),
    ).toHaveValue("D:\\mutable-existing");
    expect(invokeMock).not.toHaveBeenCalled();
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
