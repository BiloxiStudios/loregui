import { act, fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const invokeMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

import AccountPanel from "./AccountPanel";

function routeCommands(authError: unknown) {
  invokeMock.mockImplementation((command: string) => {
    if (command === "auth_user_info") return Promise.resolve(null);
    if (command === "auth_local_user_info") {
      return Promise.resolve({ users: [], tokens: [] });
    }
    if (command === "auth_login_interactive") {
      return Promise.reject(authError);
    }
    return Promise.reject(new Error(`unexpected command ${command}`));
  });
}

beforeEach(() => {
  invokeMock.mockReset();
});

describe("auth-disabled connection from the account panel (#404)", () => {
  it("shows a successful no-auth connection for v0.8.5 legacy error", async () => {
    routeCommands({
      kind: "CommandFailed",
      message: "No authentication configured on server",
    });

    render(<AccountPanel onClose={vi.fn()} />);
    await screen.findByText(/Not signed in/);

    fireEvent.change(screen.getByLabelText("Server URL"), {
      target: { value: "lore://192.0.2.10/repo" },
    });
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Connect" }));
    });

    expect(
      screen.getByText("Connected without authentication"),
    ).toBeInTheDocument();
    expect(screen.queryByText(/CommandFailed/)).toBeNull();
    expect(screen.queryByText(/No authentication configured/)).toBeNull();
  });

  it("shows a successful no-auth connection for nightly (f20ef0d7d+) NotSupported code 18", async () => {
    routeCommands({
      kind: "CommandFailed",
      message: "Operation not supported",
    });

    render(<AccountPanel onClose={vi.fn()} />);
    await screen.findByText(/Not signed in/);

    fireEvent.change(screen.getByLabelText("Server URL"), {
      target: { value: "lore://192.0.2.10/repo" },
    });
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Connect" }));
    });

    expect(
      screen.getByText("Connected without authentication"),
    ).toBeInTheDocument();
    expect(screen.queryByText(/CommandFailed/)).toBeNull();
  });

  it("rejects near-miss NotSupported messages with qualifiers", async () => {
    routeCommands({
      kind: "CommandFailed",
      message: "Operation not supported: disk full",
    });

    render(<AccountPanel onClose={vi.fn()} />);
    await screen.findByText(/Not signed in/);

    fireEvent.change(screen.getByLabelText("Server URL"), {
      target: { value: "lore://192.0.2.10/repo" },
    });
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Connect" }));
    });

    // Should show the error, NOT the "Connected without authentication" success
    expect(screen.queryByText("Connected without authentication")).toBeNull();
    expect(screen.queryByText(/disk full/)).not.toBeNull();
  });
});
