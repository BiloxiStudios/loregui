import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const invokeMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: () => Promise.resolve(() => {}),
}));

vi.mock("../platform/directoryPicker", () => ({
  chooseDirectory: () => Promise.resolve(null),
}));

import type { StorageBackendConfig } from "../api";
import ClientClone from "./ClientClone";
import ClientConnect from "./ClientConnect";
import BackendPicker from "./server/BackendPicker";
import InitStore from "./server/InitStore";
import ServiceSetup from "./server/ServiceSetup";
import ValidateConnectivity from "./server/ValidateConnectivity";
import type { StepResult } from "./stepResult";

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

function lastStatus(states: StepResult<unknown>[]) {
  return states[states.length - 1]?.status;
}

beforeEach(() => {
  invokeMock.mockReset();
});

describe("onboarding children report backend truth", () => {
  it("ClientConnect reports idle, working, then exact trimmed URL success", async () => {
    const login = deferred<{ id: string; name: string }>();
    invokeMock.mockImplementation((command: string) => {
      if (command === "lan_discover_browse") return Promise.resolve([]);
      if (command === "lan_discover_stop") return Promise.resolve();
      if (command === "auth_login_interactive") return login.promise;
      return Promise.reject(new Error(`unexpected command ${command}`));
    });
    const states: StepResult<string>[] = [];
    render(<ClientConnect onStateChange={(result) => states.push(result)} />);
    await screen.findByText(/No servers found yet/);
    expect(lastStatus(states)).toBe("idle");

    fireEvent.change(screen.getByLabelText("Remote Server URL"), {
      target: { value: "  lore://server.example/team  " },
    });
    fireEvent.click(screen.getByRole("button", { name: "Connect" }));
    expect(lastStatus(states)).toBe("working");
    expect(states.some((state) => state.status === "success")).toBe(false);

    login.resolve({ id: "person", name: "Person" });
    await waitFor(() => expect(lastStatus(states)).toBe("success"));
    expect(states[states.length - 1]?.value).toBe("lore://server.example/team");
    expect(invokeMock).toHaveBeenCalledWith("auth_login_interactive", {
      remoteUrl: "lore://server.example/team",
    });
  });

  it("ClientConnect reports an IPC rejection as error", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "lan_discover_browse") return Promise.resolve([]);
      if (command === "lan_discover_stop") return Promise.resolve();
      if (command === "auth_login_interactive") return Promise.reject("refused");
      return Promise.reject(new Error(`unexpected command ${command}`));
    });
    const states: StepResult<string>[] = [];
    render(<ClientConnect onStateChange={(result) => states.push(result)} />);
    await screen.findByText(/No servers found yet/);
    fireEvent.change(screen.getByLabelText("Remote Server URL"), {
      target: { value: "lore://server/team" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Connect" }));
    await waitFor(() => expect(lastStatus(states)).toBe("error"));
    expect(states[states.length - 1]?.message).toBe("refused");
  });

  it("ClientClone uses the exact forwarded URL and waits for clone plus open", async () => {
    const clone = deferred<void>();
    const opened = deferred<void>();
    invokeMock.mockImplementation((command: string) => {
      if (command === "repository_clone") return clone.promise;
      if (command === "open_repository") return opened.promise;
      return Promise.reject(new Error(`unexpected command ${command}`));
    });
    const states: StepResult<string>[] = [];
    render(
      <ClientClone
        initialMode="clone"
        initialCloneUrl="lore://server.example/team"
        onStateChange={(result) => states.push(result)}
      />,
    );
    expect(screen.getByLabelText("Repository URL")).toHaveValue(
      "lore://server.example/team",
    );
    fireEvent.click(screen.getByText("Advanced path entry"));
    fireEvent.change(screen.getByLabelText("Destination Path"), {
      target: { value: "/local/team" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Clone" }));
    expect(lastStatus(states)).toBe("working");
    expect(invokeMock).toHaveBeenCalledWith("repository_clone", {
      url: "lore://server.example/team",
      dest: "/local/team",
    });

    clone.resolve();
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("open_repository", {
        path: "/local/team",
      }),
    );
    expect(lastStatus(states)).toBe("working");
    opened.resolve();
    await waitFor(() => expect(lastStatus(states)).toBe("success"));
    expect(states[states.length - 1]?.value).toBe("/local/team");
  });

  it("ClientClone reports open rejection as error", async () => {
    invokeMock.mockRejectedValue("not a repository");
    const states: StepResult<string>[] = [];
    render(
      <ClientClone initialMode="open" onStateChange={(result) => states.push(result)} />,
    );
    fireEvent.click(screen.getByText("Advanced path entry"));
    fireEvent.change(screen.getByLabelText("Repository Path"), {
      target: { value: "/bad" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Open" }));
    await waitFor(() => expect(lastStatus(states)).toBe("error"));
  });

  it("ClientClone invalidates visible success when repository inputs change", async () => {
    invokeMock.mockResolvedValue(undefined);
    const states: StepResult<string>[] = [];
    render(
      <ClientClone initialMode="open" onStateChange={(result) => states.push(result)} />,
    );
    fireEvent.click(screen.getByText("Advanced path entry"));
    fireEvent.change(screen.getByLabelText("Repository Path"), {
      target: { value: "/repo" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Open" }));
    await screen.findByText("✓ Repository ready");
    expect(lastStatus(states)).toBe("success");

    fireEvent.change(screen.getByLabelText("Repository Path"), {
      target: { value: "/different" },
    });
    expect(screen.queryByText("✓ Repository ready")).toBeNull();
    expect(lastStatus(states)).toBe("idle");
  });

  it("ClientClone ignores a pending repository result after an input edit", async () => {
    const opened = deferred<void>();
    invokeMock.mockReturnValue(opened.promise);
    const states: StepResult<string>[] = [];
    render(
      <ClientClone initialMode="open" onStateChange={(result) => states.push(result)} />,
    );
    fireEvent.click(screen.getByText("Advanced path entry"));
    fireEvent.change(screen.getByLabelText("Repository Path"), {
      target: { value: "/repo" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Open" }));
    expect(lastStatus(states)).toBe("working");
    fireEvent.change(screen.getByLabelText("Repository Path"), {
      target: { value: "/different" },
    });
    expect(lastStatus(states)).toBe("idle");

    await act(async () => opened.resolve());
    expect(screen.queryByText("✓ Repository ready")).toBeNull();
    expect(states.some((state) => state.status === "success")).toBe(false);
    expect(lastStatus(states)).toBe("idle");
  });

  it("ClientClone ignores a pending repository result after unmount", async () => {
    const opened = deferred<void>();
    invokeMock.mockReturnValue(opened.promise);
    const states: StepResult<string>[] = [];
    const view = render(
      <ClientClone initialMode="open" onStateChange={(result) => states.push(result)} />,
    );
    fireEvent.click(screen.getByText("Advanced path entry"));
    fireEvent.change(screen.getByLabelText("Repository Path"), {
      target: { value: "/repo" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Open" }));
    view.unmount();

    await act(async () => opened.reject(new Error("late failure")));
    expect(states.some((state) => state.status === "success")).toBe(false);
    expect(states.some((state) => state.status === "error")).toBe(false);
  });

  it("ClientClone does not open a clone invalidated while clone IPC is pending", async () => {
    const cloning = deferred<void>();
    invokeMock.mockImplementation((command: string) => {
      if (command === "repository_clone") return cloning.promise;
      if (command === "open_repository") return Promise.resolve();
      return Promise.reject(new Error(`unexpected command ${command}`));
    });
    const states: StepResult<string>[] = [];
    render(
      <ClientClone
        initialMode="clone"
        initialCloneUrl="lore://server/team"
        onStateChange={(result) => states.push(result)}
      />,
    );
    fireEvent.click(screen.getByText("Advanced path entry"));
    fireEvent.change(screen.getByLabelText("Destination Path"), {
      target: { value: "/repo" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Clone" }));
    fireEvent.change(screen.getByLabelText("Destination Path"), {
      target: { value: "/different" },
    });

    await act(async () => cloning.resolve());
    expect(invokeMock).not.toHaveBeenCalledWith("open_repository", expect.anything());
    expect(states.some((state) => state.status === "success")).toBe(false);
    expect(lastStatus(states)).toBe("idle");
  });

  it("BackendPicker reports idle, working, success, and rejection", async () => {
    const prepare = deferred<string>();
    invokeMock.mockReturnValueOnce(prepare.promise);
    const states: StepResult<StorageBackendConfig>[] = [];
    const view = render(
      <BackendPicker onStateChange={(result) => states.push(result)} />,
    );
    expect(lastStatus(states)).toBe("idle");
    fireEvent.click(screen.getAllByText("Advanced path entry")[0]);
    fireEvent.change(screen.getByLabelText("Local Storage Path"), {
      target: { value: "/store" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Prepare Store" }));
    expect(lastStatus(states)).toBe("working");
    prepare.resolve("/resolved/store");
    await waitFor(() => expect(lastStatus(states)).toBe("success"));
    view.unmount();

    invokeMock.mockRejectedValueOnce("disk full");
    const errors: StepResult<StorageBackendConfig>[] = [];
    render(<BackendPicker onStateChange={(result) => errors.push(result)} />);
    fireEvent.click(screen.getAllByText("Advanced path entry")[0]);
    fireEvent.change(screen.getByLabelText("Local Storage Path"), {
      target: { value: "/store" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Prepare Store" }));
    await waitFor(() => expect(lastStatus(errors)).toBe("error"));
  });

  it("ValidateConnectivity reports idle, working, success, and rejection", async () => {
    const probe = deferred<void>();
    invokeMock.mockReturnValueOnce(probe.promise);
    const config: StorageBackendConfig = { kind: "local", path: "/store" };
    const states: StepResult[] = [];
    const view = render(
      <ValidateConnectivity config={config} onStateChange={(result) => states.push(result)} />,
    );
    expect(lastStatus(states)).toBe("idle");
    fireEvent.click(screen.getByRole("button", { name: "Run Connectivity Test" }));
    expect(lastStatus(states)).toBe("working");
    probe.resolve();
    await waitFor(() => expect(lastStatus(states)).toBe("success"));
    view.unmount();

    invokeMock.mockRejectedValueOnce("offline");
    const errors: StepResult[] = [];
    render(
      <ValidateConnectivity config={config} onStateChange={(result) => errors.push(result)} />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Run Connectivity Test" }));
    await waitFor(() => expect(lastStatus(errors)).toBe("error"));
  });

  it("ValidateConnectivity preserves Error.message in UI and reported state", async () => {
    invokeMock.mockRejectedValueOnce(new Error("probe failed"));
    const states: StepResult[] = [];
    render(
      <ValidateConnectivity
        config={{ kind: "local", path: "/store" }}
        onStateChange={(result) => states.push(result)}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Run Connectivity Test" }));

    expect(await screen.findByText("probe failed")).toBeVisible();
    expect(states[states.length - 1]).toEqual({
      status: "error",
      message: "probe failed",
    });
  });

  it("InitStore reports idle, working, success, and rejection", async () => {
    const prepare = deferred<string>();
    invokeMock.mockReturnValueOnce(prepare.promise);
    const config: StorageBackendConfig = { kind: "local", path: "/store" };
    const states: StepResult<{ storePath: string; repoName: string }>[] = [];
    const view = render(
      <InitStore config={config} onStateChange={(result) => states.push(result)} />,
    );
    expect(lastStatus(states)).toBe("idle");
    fireEvent.click(screen.getByRole("button", { name: "Create Store" }));
    expect(lastStatus(states)).toBe("working");
    prepare.resolve("/resolved/store");
    await waitFor(() => expect(lastStatus(states)).toBe("success"));
    view.unmount();

    invokeMock.mockRejectedValueOnce("permission denied");
    const errors: StepResult<{ storePath: string; repoName: string }>[] = [];
    render(<InitStore config={config} onStateChange={(result) => errors.push(result)} />);
    fireEvent.click(screen.getByRole("button", { name: "Create Store" }));
    await waitFor(() => expect(lastStatus(errors)).toBe("error"));
  });

  it("ServiceSetup reports idle, working, success URL, and rejection", async () => {
    const start = deferred<{ running: boolean; url: string; storeDir: string }>();
    invokeMock.mockImplementation((command: string) => {
      if (command === "host_server_status") return Promise.resolve({ running: false });
      if (command === "host_server_start") return start.promise;
      return Promise.reject(new Error(`unexpected command ${command}`));
    });
    const states: StepResult<string>[] = [];
    const view = render(
      <ServiceSetup storePath="/store" onStateChange={(result) => states.push(result)} />,
    );
    expect(lastStatus(states)).toBe("idle");
    fireEvent.click(await screen.findByRole("button", { name: "Start Hosting" }));
    expect(lastStatus(states)).toBe("working");
    start.resolve({
      running: true,
      url: "lore://localhost/team",
      storeDir: "/store",
    });
    await waitFor(() => expect(lastStatus(states)).toBe("success"));
    expect(states[states.length - 1]?.value).toBe("lore://localhost/team");
    view.unmount();

    invokeMock.mockImplementation((command: string) =>
      command === "host_server_status"
        ? Promise.resolve({ running: false })
        : Promise.reject("port in use"),
    );
    const errors: StepResult<string>[] = [];
    render(
      <ServiceSetup storePath="/store" onStateChange={(result) => errors.push(result)} />,
    );
    fireEvent.click(await screen.findByRole("button", { name: "Start Hosting" }));
    await waitFor(() => expect(lastStatus(errors)).toBe("error"));
  });

  it("reports idle again when editable child inputs change", async () => {
    const cloneStates: StepResult<string>[] = [];
    const clone = render(
      <ClientClone
        initialMode="open"
        onStateChange={(result) => cloneStates.push(result)}
      />,
    );
    cloneStates.length = 0;
    fireEvent.click(screen.getByText("Advanced path entry"));
    fireEvent.change(screen.getByLabelText("Repository Path"), {
      target: { value: "/changed" },
    });
    expect(lastStatus(cloneStates)).toBe("idle");
    clone.unmount();

    const backendStates: StepResult<StorageBackendConfig>[] = [];
    const backend = render(
      <BackendPicker onStateChange={(result) => backendStates.push(result)} />,
    );
    backendStates.length = 0;
    fireEvent.click(screen.getAllByText("Advanced path entry")[0]);
    fireEvent.change(screen.getByLabelText("Local Storage Path"), {
      target: { value: "/changed" },
    });
    expect(lastStatus(backendStates)).toBe("idle");
    backend.unmount();

    const initStates: StepResult<{ storePath: string; repoName: string }>[] = [];
    const initialized = render(
      <InitStore onStateChange={(result) => initStates.push(result)} />,
    );
    initStates.length = 0;
    fireEvent.change(screen.getByLabelText("Repository Name (optional)"), {
      target: { value: "changed" },
    });
    expect(lastStatus(initStates)).toBe("idle");
    initialized.unmount();

    invokeMock.mockImplementation((command: string) =>
      command === "host_server_status"
        ? Promise.resolve({ running: false })
        : Promise.reject(new Error(`unexpected command ${command}`)),
    );
    const serviceStates: StepResult<string>[] = [];
    render(
      <ServiceSetup onStateChange={(result) => serviceStates.push(result)} />,
    );
    serviceStates.length = 0;
    fireEvent.click(screen.getByText("Advanced path entry"));
    fireEvent.change(screen.getByLabelText("Store directory to serve"), {
      target: { value: "/changed" },
    });
    expect(lastStatus(serviceStates)).toBe("idle");
  });
});
