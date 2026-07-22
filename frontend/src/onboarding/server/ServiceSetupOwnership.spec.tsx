import { useState } from "react";
import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const invokeMock = vi.fn();
const chooseDirectoryMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

vi.mock("../../platform/directoryPicker", () => ({
  chooseDirectory: (...args: unknown[]) => chooseDirectoryMock(...args),
}));

import ServiceSetup from "./ServiceSetup";
import type { StepResult } from "../stepResult";

const OWNERSHIP_ERROR =
  "A Lore server is already running from /other, not this flow's store /store. Stop it before continuing.";

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

function FinishHarness({ onComplete }: { onComplete: () => void }) {
  const [result, setResult] = useState<StepResult<string>>({ status: "idle" });
  return (
    <>
      <ServiceSetup storePath="/store-a" onStateChange={setResult} />
      <button
        disabled={result.status !== "success"}
        onClick={() => {
          if (result.status === "success") onComplete();
        }}
      >
        Finish
      </button>
    </>
  );
}

function editDisabledInput(label: string, value: string) {
  const input = screen.getByLabelText(label);
  input.removeAttribute("disabled");
  fireEvent.change(input, { target: { value } });
}

function invokeDisabledButton(name: string) {
  const button = screen.getByRole("button", { name });
  const propsKey = Object.keys(button).find((key) =>
    key.startsWith("__reactProps$"),
  );
  if (!propsKey) throw new Error("React button props were not available");
  const props = (
    button as unknown as Record<string, { onClick?: () => void }>
  )[propsKey];
  if (!props.onClick) throw new Error("React button click handler was missing");
  act(() => props.onClick?.());
}

beforeEach(() => {
  invokeMock.mockReset();
  chooseDirectoryMock.mockReset();
  chooseDirectoryMock.mockResolvedValue(null);
});

describe("ServiceSetup running-host ownership", () => {
  it("reports a visible error and keeps Finish blocked for a different store", async () => {
    invokeMock.mockResolvedValue({
      running: true,
      url: "lore://localhost/other",
      storeDir: "/other",
    });
    const states: StepResult<string>[] = [];
    render(
      <ServiceSetup
        storePath="/store"
        onStateChange={(result) => states.push(result)}
      />,
    );

    expect(await screen.findByText(OWNERSHIP_ERROR)).toBeVisible();
    expect(states[states.length - 1]).toEqual({
      status: "error",
      message: OWNERSHIP_ERROR,
    });
    expect(states.some((state) => state.status === "success")).toBe(false);
  });

  it("accepts an already-running host only for the exact current-flow store", async () => {
    invokeMock.mockResolvedValue({
      running: true,
      url: "lore://localhost/team",
      storeDir: "/store",
    });
    const states: StepResult<string>[] = [];
    render(
      <ServiceSetup
        storePath="/store"
        onStateChange={(result) => states.push(result)}
      />,
    );

    await waitFor(() =>
      expect(states[states.length - 1]).toEqual({
        status: "success",
        value: "lore://localhost/team",
      }),
    );
    expect(screen.getByText("Server is hosting")).toBeVisible();
  });

  it.each([
    [
      "a mismatched start store",
      { running: true, url: "lore://localhost/other", storeDir: "/other" },
      "Lore server did not start for this flow's store /store (reported /other).",
    ],
    [
      "a missing start store",
      { running: true, url: "lore://localhost/team" },
      "Lore server did not start for this flow's store /store (reported an unknown store).",
    ],
    [
      "a non-running start response",
      { running: false, url: "lore://localhost/team", storeDir: "/store" },
      "Lore server did not start for this flow's store /store (server is not running).",
    ],
  ])("rejects %s", async (_caseName, startResponse, expectedMessage) => {
    invokeMock.mockImplementation((command: string) =>
      command === "host_server_status"
        ? Promise.resolve({ running: false })
        : Promise.resolve(startResponse),
    );
    const states: StepResult<string>[] = [];
    render(
      <ServiceSetup
        storePath="/store"
        onStateChange={(result) => states.push(result)}
      />,
    );
    fireEvent.click(await screen.findByRole("button", { name: "Start Hosting" }));

    expect(await screen.findByText(expectedMessage)).toBeVisible();
    expect(states[states.length - 1]).toEqual({
      status: "error",
      message: expectedMessage,
    });
    expect(states.some((state) => state.status === "success")).toBe(false);
  });

  it("ignores start A after the store changes A to B and back to A", async () => {
    const start = deferred<{
      running: boolean;
      url: string;
      storeDir: string;
    }>();
    invokeMock.mockImplementation((command: string) =>
      command === "host_server_status"
        ? Promise.resolve({ running: false })
        : start.promise,
    );
    const onComplete = vi.fn();
    render(<FinishHarness onComplete={onComplete} />);
    fireEvent.click(screen.getByText("Advanced path entry"));
    fireEvent.click(await screen.findByRole("button", { name: "Start Hosting" }));
    editDisabledInput("Store directory to serve", "/store-b");
    editDisabledInput("Store directory to serve", "/store-a");

    await act(async () =>
      start.resolve({
        running: true,
        url: "lore://localhost/a",
        storeDir: "/store-a",
      }),
    );
    expect(screen.queryByText("Server is hosting")).toBeNull();
    const finish = screen.getByRole("button", { name: "Finish" });
    expect(finish).toBeDisabled();
    finish.removeAttribute("disabled");
    fireEvent.click(finish);
    expect(onComplete).not.toHaveBeenCalled();
  });

  it("ignores a late start error after the store changes", async () => {
    const start = deferred<never>();
    invokeMock.mockImplementation((command: string) =>
      command === "host_server_status"
        ? Promise.resolve({ running: false })
        : start.promise,
    );
    const states: StepResult<string>[] = [];
    render(
      <ServiceSetup
        storePath="/store-a"
        onStateChange={(result) => states.push(result)}
      />,
    );
    fireEvent.click(screen.getByText("Advanced path entry"));
    fireEvent.click(await screen.findByRole("button", { name: "Start Hosting" }));
    editDisabledInput("Store directory to serve", "/store-b");

    await act(async () => start.reject(new Error("late start failure")));
    expect(screen.queryByText("late start failure")).toBeNull();
    expect(states.some((state) => state.status === "error")).toBe(false);
    expect(states[states.length - 1]?.status).toBe("idle");
  });

  it("ignores a late initial status after the store is edited", async () => {
    const status = deferred<{
      running: boolean;
      url: string;
      storeDir: string;
    }>();
    invokeMock.mockReturnValue(status.promise);
    const states: StepResult<string>[] = [];
    render(
      <ServiceSetup
        storePath="/store-a"
        onStateChange={(result) => states.push(result)}
      />,
    );
    fireEvent.click(screen.getByText("Advanced path entry"));
    fireEvent.change(screen.getByLabelText("Store directory to serve"), {
      target: { value: "/store-b" },
    });

    await act(async () =>
      status.resolve({
        running: true,
        url: "lore://localhost/a",
        storeDir: "/store-a",
      }),
    );
    expect(screen.queryByText("Server is hosting")).toBeNull();
    expect(states.some((state) => state.status === "success")).toBe(false);
    expect(states[states.length - 1]?.status).toBe("idle");
  });

  it("invalidates a pending start when Browse selects a different store", async () => {
    const directory = deferred<string | null>();
    const start = deferred<{
      running: boolean;
      url: string;
      storeDir: string;
    }>();
    chooseDirectoryMock.mockReturnValue(directory.promise);
    invokeMock.mockImplementation((command: string) =>
      command === "host_server_status"
        ? Promise.resolve({ running: false })
        : start.promise,
    );
    const states: StepResult<string>[] = [];
    render(
      <ServiceSetup
        storePath="/store-a"
        onStateChange={(result) => states.push(result)}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Browse…" }));
    fireEvent.click(await screen.findByRole("button", { name: "Start Hosting" }));
    await act(async () => directory.resolve("/store-b"));
    await act(async () =>
      start.resolve({
        running: true,
        url: "lore://localhost/a",
        storeDir: "/store-a",
      }),
    );

    expect(screen.getByText("/store-b")).toBeVisible();
    expect(screen.queryByText("Server is hosting")).toBeNull();
    expect(states.some((state) => state.status === "success")).toBe(false);
  });

  it("ignores a pending Browse result after a newer manual edit", async () => {
    const directory = deferred<string | null>();
    chooseDirectoryMock.mockReturnValue(directory.promise);
    invokeMock.mockResolvedValue({ running: false });
    render(<ServiceSetup storePath="/store-a" />);
    fireEvent.click(screen.getByText("Advanced path entry"));
    fireEvent.click(screen.getByRole("button", { name: "Browse…" }));
    fireEvent.change(screen.getByLabelText("Store directory to serve"), {
      target: { value: "/store-manual" },
    });

    await act(async () => directory.resolve("/store-stale-picker"));

    expect(screen.getByText("/store-manual")).toBeVisible();
    expect(screen.queryByText("/store-stale-picker")).toBeNull();
  });

  it("invalidates a pending start as soon as Browse opens", async () => {
    const directory = deferred<string | null>();
    const start = deferred<{
      running: boolean;
      url: string;
      storeDir: string;
    }>();
    chooseDirectoryMock.mockReturnValue(directory.promise);
    invokeMock.mockImplementation((command: string) =>
      command === "host_server_status"
        ? Promise.resolve({ running: false })
        : start.promise,
    );
    const states: StepResult<string>[] = [];
    render(
      <ServiceSetup
        storePath="/store-a"
        onStateChange={(result) => states.push(result)}
      />,
    );
    fireEvent.click(await screen.findByRole("button", { name: "Start Hosting" }));
    invokeDisabledButton("Browse…");
    expect(chooseDirectoryMock).toHaveBeenCalledTimes(1);

    await act(async () =>
      start.resolve({
        running: true,
        url: "lore://localhost/a",
        storeDir: "/store-a",
      }),
    );
    expect(screen.queryByText("Server is hosting")).toBeNull();
    expect(states.some((state) => state.status === "success")).toBe(false);
    expect(states[states.length - 1]?.status).toBe("idle");

    await act(async () => directory.resolve(null));
  });

  it("invalidates a pending start on detail-mode and advanced edits", async () => {
    const start = deferred<{
      running: boolean;
      url: string;
      storeDir: string;
    }>();
    invokeMock.mockImplementation((command: string) =>
      command === "host_server_status"
        ? Promise.resolve({ running: false })
        : start.promise,
    );
    const states: StepResult<string>[] = [];
    render(
      <ServiceSetup
        storePath="/store-a"
        onStateChange={(result) => states.push(result)}
      />,
    );
    fireEvent.click(screen.getByRole("tab", { name: "Expert" }));
    fireEvent.click(
      screen.getByRole("button", {
        name: /Network: bind host, QUIC, gRPC, HTTP/,
      }),
    );
    fireEvent.click(screen.getByRole("button", { name: "Start Hosting" }));
    const basic = screen.getByRole("tab", { name: "Basic" });
    basic.removeAttribute("disabled");
    fireEvent.click(basic);
    const expert = screen.getByRole("tab", { name: "Expert" });
    expert.removeAttribute("disabled");
    fireEvent.click(expert);
    editDisabledInput("Bind host", "0.0.0.0");

    await act(async () =>
      start.resolve({
        running: true,
        url: "lore://localhost/a",
        storeDir: "/store-a",
      }),
    );
    expect(screen.queryByText("Server is hosting")).toBeNull();
    expect(states.some((state) => state.status === "success")).toBe(false);
    expect(states[states.length - 1]?.status).toBe("idle");
  });

  it("ignores late start and stop results after unmount", async () => {
    const start = deferred<{
      running: boolean;
      url: string;
      storeDir: string;
    }>();
    invokeMock.mockImplementation((command: string) =>
      command === "host_server_status"
        ? Promise.resolve({ running: false })
        : start.promise,
    );
    const startStates: StepResult<string>[] = [];
    const startView = render(
      <ServiceSetup
        storePath="/store-a"
        onStateChange={(result) => startStates.push(result)}
      />,
    );
    fireEvent.click(await screen.findByRole("button", { name: "Start Hosting" }));
    startView.unmount();
    await act(async () => start.reject(new Error("late unmounted start")));
    expect(startStates.some((state) => state.status === "error")).toBe(false);

    const stop = deferred<void>();
    invokeMock.mockImplementation((command: string) => {
      if (command === "host_server_status") {
        return Promise.resolve({
          running: true,
          url: "lore://localhost/a",
          storeDir: "/store-a",
        });
      }
      if (command === "host_server_stop") return stop.promise;
      return Promise.reject(new Error(`unexpected command ${command}`));
    });
    const stopStates: StepResult<string>[] = [];
    const stopView = render(
      <ServiceSetup
        storePath="/store-a"
        onStateChange={(result) => stopStates.push(result)}
      />,
    );
    fireEvent.click(await screen.findByRole("button", { name: "Stop Hosting" }));
    stopView.unmount();
    await act(async () => stop.reject(new Error("late unmounted stop")));
    expect(
      stopStates.some(
        (state) => state.status === "error" && state.message === "late unmounted stop",
      ),
    ).toBe(false);
  });
});
