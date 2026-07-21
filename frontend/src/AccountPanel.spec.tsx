import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const invokeMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

import AccountPanel from "./AccountPanel";

/** Real EROS QA payload (SBAI-5478): identity loaders on auth-disabled server. */
const NO_AUTH_ENDPOINT = {
  kind: "CommandFailed",
  message: "No auth endpoint available",
} as const;

function routeCommands(opts: {
  loginError?: unknown;
  userInfo?: unknown;
  localInfo?: unknown;
  userInfoError?: unknown;
  localInfoError?: unknown;
}) {
  invokeMock.mockImplementation((command: string) => {
    if (command === "auth_user_info") {
      if (opts.userInfoError !== undefined) return Promise.reject(opts.userInfoError);
      return Promise.resolve(opts.userInfo ?? null);
    }
    if (command === "auth_local_user_info") {
      if (opts.localInfoError !== undefined) return Promise.reject(opts.localInfoError);
      return Promise.resolve(opts.localInfo ?? { users: [], tokens: [] });
    }
    if (command === "auth_login_interactive") {
      if (opts.loginError !== undefined) return Promise.reject(opts.loginError);
      return Promise.resolve({ id: "u1", name: "User" });
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
      loginError: {
        kind: "CommandFailed",
        message: "No authentication configured on server",
      },
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
      loginError: {
        kind: "CommandFailed",
        message:
          "Operation not supported: No authentication configured on server",
      },
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
      loginError: {
        kind: "CommandFailed",
        message: "Operation not supported: disk full",
      },
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

  it("rejects the bare NotSupported prefix without the auth operation", async () => {
    routeCommands({
      loginError: {
        kind: "CommandFailed",
        message: "Operation not supported",
      },
    });

    render(<AccountPanel onClose={vi.fn()} />);
    await screen.findByText(/Not signed in/);

    fireEvent.change(screen.getByLabelText("Server URL"), {
      target: { value: "lore://192.0.2.10/repo" },
    });
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Connect" }));
    });

    expect(screen.queryByText("Connected without authentication")).toBeNull();
    expect(screen.queryByText(/Operation not supported/)).not.toBeNull();
  });
});

/**
 * SBAI-5478 — real EROS Account dialog after successful no-auth connect showed
 * green Connect success while Signed in + This device rendered red raw
 * CommandFailed JSON ("No auth endpoint available"). Identity loaders must
 * treat no-auth signals as neutral empty states, not errors.
 */
describe("Account identity loaders on auth-disabled server (SBAI-5478)", () => {
  it("on open, No auth endpoint available is neutral on both surfaces (not red JSON)", async () => {
    routeCommands({
      userInfoError: NO_AUTH_ENDPOINT,
      localInfoError: NO_AUTH_ENDPOINT,
    });

    render(<AccountPanel onClose={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getByText(/Not signed in/)).toBeInTheDocument();
    });
    expect(screen.getByText(/No local identities on this device/)).toBeInTheDocument();
    expect(screen.queryByText(/CommandFailed/)).toBeNull();
    expect(screen.queryByText(/No auth endpoint available/)).toBeNull();
    // No red inline error containers for identity sections.
    expect(document.querySelectorAll(".storage-inline-error").length).toBe(0);
  });

  it("on open, legacy + nightly no-auth signals are neutral on both surfaces", async () => {
    routeCommands({
      userInfoError: {
        kind: "CommandFailed",
        message: "No authentication configured on server",
      },
      localInfoError: {
        kind: "CommandFailed",
        message:
          "Operation not supported: No authentication configured on server",
      },
    });

    render(<AccountPanel onClose={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getByText(/Not signed in/)).toBeInTheDocument();
    });
    expect(screen.getByText(/No local identities on this device/)).toBeInTheDocument();
    expect(screen.queryByText(/CommandFailed/)).toBeNull();
    expect(screen.queryByText(/No authentication configured/)).toBeNull();
  });

  it("successful no-auth connect keeps identity sections neutral (EROS contradiction)", async () => {
    routeCommands({
      userInfoError: NO_AUTH_ENDPOINT,
      localInfoError: NO_AUTH_ENDPOINT,
      loginError: {
        kind: "CommandFailed",
        message: "No authentication configured on server",
      },
    });

    render(<AccountPanel onClose={vi.fn()} />);
    await screen.findByText(/Not signed in/);

    fireEvent.change(screen.getByLabelText("Server URL"), {
      target: { value: "lore://10.15.1.167:41337/eros-noauth" },
    });
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Connect" }));
    });

    expect(
      screen.getByText("Connected without authentication"),
    ).toBeInTheDocument();
    // Both identity surfaces stay neutral — no red CommandFailed JSON.
    expect(screen.getByText(/Not signed in/)).toBeInTheDocument();
    expect(screen.getByText(/No local identities on this device/)).toBeInTheDocument();
    expect(screen.queryByText(/CommandFailed/)).toBeNull();
    expect(screen.queryByText(/No auth endpoint available/)).toBeNull();
  });

  it("unrelated identity loader errors remain fail-closed and visible", async () => {
    routeCommands({
      userInfoError: {
        kind: "CommandFailed",
        message: "network unreachable",
      },
      localInfoError: {
        kind: "CommandFailed",
        message: "disk I/O error",
      },
    });

    render(<AccountPanel onClose={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getByText(/network unreachable/)).toBeInTheDocument();
    });
    expect(screen.getByText(/disk I\/O error/)).toBeInTheDocument();
    expect(screen.queryByText("Connected without authentication")).toBeNull();
    // Unrelated failures must still surface as errors (not swallowed as no-auth).
    expect(document.querySelectorAll(".storage-inline-error").length).toBe(2);
  });
});
