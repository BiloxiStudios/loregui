import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const invokeMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

vi.mock("../../platform/directoryPicker", () => ({
  chooseDirectory: () => Promise.resolve(null),
}));

import ServiceSetup from "./ServiceSetup";
import type { StepResult } from "../stepResult";

const OWNERSHIP_ERROR =
  "A Lore server is already running from /other, not this flow's store /store. Stop it before continuing.";

beforeEach(() => {
  invokeMock.mockReset();
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
});
