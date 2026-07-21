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
  it("shows a successful no-auth connection instead of the raw command error", async () => {
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
});
