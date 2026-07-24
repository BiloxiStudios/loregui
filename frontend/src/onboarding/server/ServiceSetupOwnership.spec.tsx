import { useState } from "react";
import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const invokeMock = vi.fn();
const getRelayControlMock = vi.fn();
const isEntitledMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

vi.mock("../../commercial/relay-registry", () => ({
  getRelayControl: () => getRelayControlMock(),
}));

vi.mock("../../commercial/entitlement", () => ({
  isEntitled: (...args: unknown[]) => isEntitledMock(...args),
}));

import ServiceSetup from "./ServiceSetup";
import type { StepResult } from "../stepResult";
import type { RelayControlProps } from "../../commercial/relay-registry";

const OWNERSHIP_ERROR =
  "A Lore server is already running from /other, not this flow's store /store. Stop it before continuing.";
let relayRefreshCallback: (() => void) | undefined;

function RelayProbe({ onAdvertisedUrlChange }: RelayControlProps) {
  relayRefreshCallback = onAdvertisedUrlChange;
  return <div>Relay control</div>;
}

function enableRelay() {
  isEntitledMock.mockReturnValue(true);
  getRelayControlMock.mockReturnValue({
    id: "relay-test",
    feature: "lore_relay",
    label: "Relay test",
    component: RelayProbe,
  });
}

function invokeRelayRefresh() {
  if (!relayRefreshCallback) throw new Error("Relay refresh callback was missing");
  act(() => relayRefreshCallback?.());
}

function mockRunningThenRefresh(refresh: Promise<unknown>) {
  let statusCalls = 0;
  invokeMock.mockImplementation((command: string) => {
    if (command === "host_server_status") {
      statusCalls += 1;
      return statusCalls === 1
        ? Promise.resolve({
            running: true,
            url: "lore://localhost/a",
            storeDir: "/store-a",
          })
        : refresh;
    }
    if (command === "host_server_stop") return Promise.resolve();
    return Promise.reject(new Error(`unexpected command ${command}`));
  });
}

function expectFinishBlocked(onComplete: ReturnType<typeof vi.fn>) {
  expect(screen.queryByText("Server is hosting")).toBeNull();
  const finish = screen.getByRole("button", { name: "Finish" });
  expect(finish).toBeDisabled();
  finish.removeAttribute("disabled");
  fireEvent.click(finish);
  expect(onComplete).not.toHaveBeenCalled();
}

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

beforeEach(() => {
  invokeMock.mockReset();
  getRelayControlMock.mockReset();
  getRelayControlMock.mockReturnValue(null);
  isEntitledMock.mockReset();
  isEntitledMock.mockReturnValue(false);
  relayRefreshCallback = undefined;
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

  it("ignores start A after the store prop changes A to B and back to A", async () => {
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
    const view = render(
      <ServiceSetup
        storePath="/store-a"
        onStateChange={(result) => states.push(result)}
      />,
    );
    fireEvent.click(await screen.findByRole("button", { name: "Start Hosting" }));
    view.rerender(
      <ServiceSetup
        storePath="/store-b"
        onStateChange={(result) => states.push(result)}
      />,
    );
    view.rerender(
      <ServiceSetup
        storePath="/store-a"
        onStateChange={(result) => states.push(result)}
      />,
    );

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

  it("ignores a late start error after the store prop changes", async () => {
    const start = deferred<never>();
    invokeMock.mockImplementation((command: string) =>
      command === "host_server_status"
        ? Promise.resolve({ running: false })
        : start.promise,
    );
    const states: StepResult<string>[] = [];
    const view = render(
      <ServiceSetup
        storePath="/store-a"
        onStateChange={(result) => states.push(result)}
      />,
    );
    fireEvent.click(await screen.findByRole("button", { name: "Start Hosting" }));
    view.rerender(
      <ServiceSetup
        storePath="/store-b"
        onStateChange={(result) => states.push(result)}
      />,
    );

    await act(async () => start.reject(new Error("late start failure")));
    expect(screen.queryByText("late start failure")).toBeNull();
    expect(states.some((state) => state.status === "error")).toBe(false);
    expect(states[states.length - 1]?.status).toBe("idle");
  });

  it("ignores a late initial status after the store prop changes", async () => {
    const status = deferred<{
      running: boolean;
      url: string;
      storeDir: string;
    }>();
    // First fetch is the stale one; a store prop change correctly re-fetches.
    let statusCalls = 0;
    invokeMock.mockImplementation(() =>
      ++statusCalls === 1
        ? status.promise
        : Promise.resolve({ running: false }),
    );
    const states: StepResult<string>[] = [];
    const view = render(
      <ServiceSetup
        storePath="/store-a"
        onStateChange={(result) => states.push(result)}
      />,
    );
    view.rerender(
      <ServiceSetup
        storePath="/store-b"
        onStateChange={(result) => states.push(result)}
      />,
    );

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

  it("applies an exact-store relay refresh and reports the refreshed success", async () => {
    enableRelay();
    invokeMock
      .mockResolvedValueOnce({
        running: true,
        url: "lore://localhost/a",
        storeDir: "/store-a",
      })
      .mockResolvedValueOnce({
        running: true,
        url: "lore://localhost/a",
        advertisedUrl: "lore://relay.example/a",
        storeDir: "/store-a",
      });
    const states: StepResult<string>[] = [];
    render(
      <ServiceSetup
        storePath="/store-a"
        onStateChange={(result) => states.push(result)}
      />,
    );
    expect(await screen.findByText("Relay control")).toBeVisible();

    invokeRelayRefresh();

    await waitFor(() =>
      expect(screen.getByLabelText(/Connection URL/)).toHaveValue(
        "lore://relay.example/a",
      ),
    );
    expect(states[states.length - 1]).toEqual({
      status: "success",
      value: "lore://relay.example/a",
    });
  });

  it.each([
    ["not running", { running: false }, null],
    [
      "a mismatched store",
      { running: true, url: "lore://localhost/other", storeDir: "/other" },
      "A Lore server is already running from /other, not this flow's store /store-a. Stop it before continuing.",
    ],
    [
      "a missing store",
      { running: true, url: "lore://localhost/a" },
      "A Lore server is already running from an unknown store, not this flow's store /store-a. Stop it before continuing.",
    ],
    ["an error", new Error("refresh status failed"), "refresh status failed"],
  ])(
    "clears prior success when relay refresh reports %s",
    async (_caseName, refreshResult, expectedError) => {
      enableRelay();
      invokeMock.mockResolvedValueOnce({
        running: true,
        url: "lore://localhost/a",
        storeDir: "/store-a",
      });
      if (refreshResult instanceof Error) {
        invokeMock.mockRejectedValueOnce(refreshResult);
      } else {
        invokeMock.mockResolvedValueOnce(refreshResult);
      }
      const onComplete = vi.fn();
      render(<FinishHarness onComplete={onComplete} />);
      expect(await screen.findByText("Relay control")).toBeVisible();
      expect(screen.getByRole("button", { name: "Finish" })).toBeEnabled();

      invokeRelayRefresh();

      await waitFor(() =>
        expect(screen.queryByText("Server is hosting")).toBeNull(),
      );
      if (expectedError) {
        expect(screen.getByText(expectedError)).toBeVisible();
      } else {
        expect(screen.getByRole("button", { name: "Start Hosting" })).toBeVisible();
      }
      const finish = screen.getByRole("button", { name: "Finish" });
      expect(finish).toBeDisabled();
      finish.removeAttribute("disabled");
      fireEvent.click(finish);
      expect(onComplete).not.toHaveBeenCalled();
    },
  );

  it("does not let relay refresh cancel or override a pending start", async () => {
    enableRelay();
    const start = deferred<{
      running: boolean;
      url: string;
      storeDir: string;
    }>();
    let statusCalls = 0;
    invokeMock.mockImplementation((command: string) => {
      if (command === "host_server_status") {
        statusCalls += 1;
        return Promise.resolve({
          running: true,
          url: "lore://localhost/a",
          storeDir: "/store-a",
        });
      }
      if (command === "host_server_stop") return Promise.resolve();
      if (command === "host_server_start") return start.promise;
      return Promise.reject(new Error(`unexpected command ${command}`));
    });
    render(<ServiceSetup storePath="/store-a" />);
    expect(await screen.findByText("Relay control")).toBeVisible();
    const capturedRefresh = relayRefreshCallback;
    fireEvent.click(screen.getByRole("button", { name: "Stop Hosting" }));
    expect(await screen.findByRole("button", { name: "Start Hosting" })).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: "Start Hosting" }));

    act(() => capturedRefresh?.());
    expect(statusCalls).toBe(1);
    await act(async () =>
      start.resolve({
        running: true,
        url: "lore://localhost/started",
        storeDir: "/store-a",
      }),
    );
    expect(screen.getByText("Server is hosting")).toBeVisible();
    expect(screen.getByLabelText(/Connection URL/)).toHaveValue(
      "lore://localhost/started",
    );
  });

  it("does not let relay refresh cancel or override a pending stop", async () => {
    enableRelay();
    const stop = deferred<void>();
    let statusCalls = 0;
    invokeMock.mockImplementation((command: string) => {
      if (command === "host_server_status") {
        statusCalls += 1;
        return Promise.resolve({
          running: true,
          url: "lore://localhost/a",
          storeDir: "/store-a",
        });
      }
      if (command === "host_server_stop") return stop.promise;
      return Promise.reject(new Error(`unexpected command ${command}`));
    });
    render(<ServiceSetup storePath="/store-a" />);
    expect(await screen.findByText("Relay control")).toBeVisible();
    const capturedRefresh = relayRefreshCallback;
    fireEvent.click(screen.getByRole("button", { name: "Stop Hosting" }));

    act(() => capturedRefresh?.());
    expect(statusCalls).toBe(1);
    await act(async () => stop.resolve());
    expect(screen.getByRole("button", { name: "Start Hosting" })).toBeVisible();
    expect(screen.queryByText("Server is hosting")).toBeNull();
  });

  it("ignores a refresh that began before a newer start", async () => {
    enableRelay();
    const refresh = deferred<{ running: boolean }>();
    const start = deferred<{
      running: boolean;
      url: string;
      storeDir: string;
    }>();
    let statusCalls = 0;
    invokeMock.mockImplementation((command: string) => {
      if (command === "host_server_status") {
        statusCalls += 1;
        return statusCalls === 1
          ? Promise.resolve({
              running: true,
              url: "lore://localhost/a",
              storeDir: "/store-a",
            })
          : refresh.promise;
      }
      if (command === "host_server_stop") return Promise.resolve();
      if (command === "host_server_start") return start.promise;
      return Promise.reject(new Error(`unexpected command ${command}`));
    });
    render(<ServiceSetup storePath="/store-a" />);
    expect(await screen.findByText("Relay control")).toBeVisible();
    const capturedRefresh = relayRefreshCallback;
    fireEvent.click(screen.getByRole("button", { name: "Stop Hosting" }));
    expect(await screen.findByRole("button", { name: "Start Hosting" })).toBeVisible();
    act(() => capturedRefresh?.());
    fireEvent.click(screen.getByRole("button", { name: "Start Hosting" }));

    await act(async () => refresh.resolve({ running: false }));
    expect(screen.getByRole("button", { name: "Starting…" })).toBeDisabled();
    await act(async () =>
      start.resolve({
        running: true,
        url: "lore://localhost/started",
        storeDir: "/store-a",
      }),
    );
    expect(screen.getByText("Server is hosting")).toBeVisible();
  });

  it("ignores a refresh that began before a newer stop", async () => {
    enableRelay();
    const refresh = deferred<{ running: boolean }>();
    const stop = deferred<void>();
    let statusCalls = 0;
    invokeMock.mockImplementation((command: string) => {
      if (command === "host_server_status") {
        statusCalls += 1;
        return statusCalls === 1
          ? Promise.resolve({
              running: true,
              url: "lore://localhost/a",
              storeDir: "/store-a",
            })
          : refresh.promise;
      }
      if (command === "host_server_stop") return stop.promise;
      return Promise.reject(new Error(`unexpected command ${command}`));
    });
    render(<ServiceSetup storePath="/store-a" />);
    expect(await screen.findByText("Relay control")).toBeVisible();
    invokeRelayRefresh();
    fireEvent.click(screen.getByRole("button", { name: "Stop Hosting" }));

    await act(async () => refresh.resolve({ running: false }));
    expect(screen.getByRole("button", { name: "Stopping…" })).toBeDisabled();
    await act(async () => stop.resolve());
    expect(screen.getByRole("button", { name: "Start Hosting" })).toBeVisible();
  });

  it("ignores a relay refresh invalidated by a store prop change", async () => {
    enableRelay();
    const refresh = deferred<{
      running: boolean;
      url: string;
      storeDir: string;
    }>();
    let statusCalls = 0;
    const states: StepResult<string>[] = [];
    invokeMock.mockImplementation((command: string) => {
      if (command === "host_server_status") {
        statusCalls += 1;
        return statusCalls === 1
          ? Promise.resolve({
              running: true,
              url: "lore://localhost/a",
              storeDir: "/store-a",
            })
          : refresh.promise;
      }
      if (command === "host_server_stop") return Promise.resolve();
      return Promise.reject(new Error(`unexpected command ${command}`));
    });
    const view = render(
      <ServiceSetup
        storePath="/store-a"
        onStateChange={(result) => states.push(result)}
      />,
    );
    expect(await screen.findByText("Relay control")).toBeVisible();
    const capturedRefresh = relayRefreshCallback;
    fireEvent.click(screen.getByRole("button", { name: "Stop Hosting" }));
    expect(await screen.findByRole("button", { name: "Start Hosting" })).toBeVisible();
    act(() => capturedRefresh?.());
    view.rerender(
      <ServiceSetup
        storePath="/store-b"
        onStateChange={(result) => states.push(result)}
      />,
    );
    const successCount = states.filter((state) => state.status === "success").length;

    await act(async () =>
      refresh.resolve({
        running: true,
        url: "lore://localhost/stale",
        storeDir: "/store-a",
      }),
    );
    expect(screen.queryByText("Server is hosting")).toBeNull();
    expect(screen.getByText("/store-b")).toBeVisible();
    expect(states.filter((state) => state.status === "success")).toHaveLength(
      successCount,
    );
  });

  it("ignores a deferred refresh error after a Basic-to-Expert mode change", async () => {
    enableRelay();
    const refresh = deferred<never>();
    mockRunningThenRefresh(refresh.promise);
    const onComplete = vi.fn();
    render(<FinishHarness onComplete={onComplete} />);
    expect(await screen.findByText("Relay control")).toBeVisible();
    const capturedRefresh = relayRefreshCallback;
    fireEvent.click(screen.getByRole("button", { name: "Stop Hosting" }));
    expect(await screen.findByRole("button", { name: "Start Hosting" })).toBeVisible();
    act(() => capturedRefresh?.());

    fireEvent.click(screen.getByRole("tab", { name: "Expert" }));
    await act(async () => refresh.reject(new Error("stale refresh failure")));

    expect(screen.queryByText("stale refresh failure")).toBeNull();
    expectFinishBlocked(onComplete);
  });

  it("ignores a deferred refresh after an advanced-field edit", async () => {
    enableRelay();
    const refresh = deferred<{
      running: boolean;
      url: string;
      storeDir: string;
    }>();
    mockRunningThenRefresh(refresh.promise);
    const onComplete = vi.fn();
    render(<FinishHarness onComplete={onComplete} />);
    expect(await screen.findByText("Relay control")).toBeVisible();
    const capturedRefresh = relayRefreshCallback;
    fireEvent.click(screen.getByRole("button", { name: "Stop Hosting" }));
    expect(await screen.findByRole("button", { name: "Start Hosting" })).toBeVisible();
    fireEvent.click(screen.getByRole("tab", { name: "Expert" }));
    fireEvent.click(
      screen.getByRole("button", {
        name: /Network: bind host, QUIC, gRPC, HTTP/,
      }),
    );
    act(() => capturedRefresh?.());

    fireEvent.change(screen.getByLabelText("Bind host"), {
      target: { value: "0.0.0.0" },
    });
    await act(async () =>
      refresh.resolve({
        running: true,
        url: "lore://localhost/stale",
        storeDir: "/store-a",
      }),
    );

    expect(screen.getByLabelText("Bind host")).toHaveValue("0.0.0.0");
    expectFinishBlocked(onComplete);
  });

  it("ignores a relay refresh invalidated by a prop reset", async () => {
    enableRelay();
    const refresh = deferred<{
      running: boolean;
      url: string;
      storeDir: string;
    }>();
    let statusCalls = 0;
    invokeMock.mockImplementation((command: string) => {
      if (command !== "host_server_status") {
        return Promise.reject(new Error(`unexpected command ${command}`));
      }
      statusCalls += 1;
      if (statusCalls === 1) {
        return Promise.resolve({
          running: true,
          url: "lore://localhost/a",
          storeDir: "/store-a",
        });
      }
      if (statusCalls === 2) return refresh.promise;
      return Promise.resolve({ running: false });
    });
    const view = render(<ServiceSetup storePath="/store-a" />);
    expect(await screen.findByText("Relay control")).toBeVisible();
    invokeRelayRefresh();
    view.rerender(<ServiceSetup storePath="/store-b" />);
    expect(await screen.findByText("/store-b")).toBeVisible();

    await act(async () =>
      refresh.resolve({
        running: true,
        url: "lore://localhost/stale",
        storeDir: "/store-a",
      }),
    );
    expect(screen.queryByText("Server is hosting")).toBeNull();
    expect(screen.getByText("/store-b")).toBeVisible();
  });

  it("ignores a relay refresh that rejects after unmount", async () => {
    enableRelay();
    const refresh = deferred<never>();
    invokeMock
      .mockResolvedValueOnce({
        running: true,
        url: "lore://localhost/a",
        storeDir: "/store-a",
      })
      .mockReturnValueOnce(refresh.promise);
    const states: StepResult<string>[] = [];
    const view = render(
      <ServiceSetup
        storePath="/store-a"
        onStateChange={(result) => states.push(result)}
      />,
    );
    expect(await screen.findByText("Relay control")).toBeVisible();
    invokeRelayRefresh();
    const stateCount = states.length;
    view.unmount();

    await act(async () => refresh.reject(new Error("late refresh failure")));
    expect(states).toHaveLength(stateCount);
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
