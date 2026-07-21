import { beforeEach, describe, expect, it, vi } from "vitest";
import { act, fireEvent, render, screen } from "@testing-library/react";

const invokeMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: () => Promise.resolve(() => {}),
}));

import ClientConnect from "./ClientConnect";

function routeAuthFailure(error: unknown) {
  invokeMock.mockImplementation((command: string) => {
    if (command === "lan_discover_browse") return Promise.resolve([]);
    if (command === "lan_discover_stop") return Promise.resolve();
    if (command === "auth_login_interactive") return Promise.reject(error);
    return Promise.reject(new Error(`unexpected command ${command}`));
  });
}

async function connect() {
  render(<ClientConnect />);
  await screen.findByText(/No servers found yet/);
  fireEvent.change(screen.getByLabelText("Remote Server URL"), {
    target: { value: "lore://192.0.2.10/repo" },
  });
  await act(async () => {
    fireEvent.click(screen.getByRole("button", { name: "Connect" }));
  });
}

beforeEach(() => {
  invokeMock.mockReset();
});

describe("auth-disabled server connection (#404)", () => {
  it("accepts a reachable server that explicitly has no authentication configured (v0.8.5)", async () => {
    routeAuthFailure({
      kind: "CommandFailed",
      message: "No authentication configured on server",
    });

    await connect();

    expect(
      await screen.findByText("Connected without authentication"),
    ).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Retry" })).toBeNull();
    expect(screen.queryByText(/CommandFailed/)).toBeNull();
  });

  it("accepts the nightly (f20ef0d7d+) NotSupported code 18 authless signal", async () => {
    routeAuthFailure({
      kind: "CommandFailed",
      message:
        "Operation not supported: No authentication configured on server",
    });

    await connect();

    expect(
      await screen.findByText("Connected without authentication"),
    ).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Retry" })).toBeNull();
    expect(screen.queryByText(/CommandFailed/)).toBeNull();
  });

  it("continues to reject unrelated authentication failures", async () => {
    routeAuthFailure({
      kind: "CommandFailed",
      message: "Server refused the connection",
    });

    await connect();

    expect(await screen.findByRole("button", { name: "Retry" })).toBeInTheDocument();
    expect(screen.queryByText("Connected without authentication")).toBeNull();
  });

  it("rejects near-miss NotSupported messages with qualifiers", async () => {
    routeAuthFailure({
      kind: "CommandFailed",
      message: "Operation not supported: disk full",
    });

    await connect();

    expect(await screen.findByRole("button", { name: "Retry" })).toBeInTheDocument();
    expect(screen.queryByText("Connected without authentication")).toBeNull();
  });

  it("rejects the bare NotSupported prefix without the auth operation", async () => {
    routeAuthFailure({
      kind: "CommandFailed",
      message: "Operation not supported",
    });

    await connect();

    expect(await screen.findByRole("button", { name: "Retry" })).toBeInTheDocument();
    expect(screen.queryByText("Connected without authentication")).toBeNull();
  });
});
