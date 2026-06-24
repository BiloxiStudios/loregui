/**
 * Component tests for the CommandPalette overlay: it stays closed until opened
 * (Ctrl-K or the launcher event), filters the registry by query, drills into an
 * op's generated form, and runs the selected command through `invoke`, surfacing
 * the result or error.
 *
 * `@tauri-apps/api/core` is mocked so command execution is observable without a
 * Tauri runtime.
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";

const invokeMock = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

import CommandPalette, { OPEN_PALETTE_EVENT } from "./CommandPalette";

beforeEach(() => {
  invokeMock.mockReset();
  invokeMock.mockResolvedValue("ok");
});

function openViaEvent() {
  fireEvent(window, new Event(OPEN_PALETTE_EVENT));
}

describe("CommandPalette", () => {
  it("renders nothing until opened", () => {
    render(<CommandPalette />);
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("opens on the launcher event and shows the search box", () => {
    render(<CommandPalette />);
    openViaEvent();
    expect(screen.getByRole("dialog")).toBeInTheDocument();
    expect(
      screen.getByPlaceholderText(/Run a command/i),
    ).toBeInTheDocument();
  });

  it("opens on Ctrl-K", () => {
    render(<CommandPalette />);
    fireEvent.keyDown(window, { key: "k", ctrlKey: true });
    expect(screen.getByRole("dialog")).toBeInTheDocument();
  });

  it("filters the command list by the typed query", () => {
    render(<CommandPalette />);
    openViaEvent();
    const search = screen.getByPlaceholderText(/Run a command/i);
    fireEvent.change(search, { target: { value: "branch create" } });
    // A matching op is shown...
    expect(screen.getByText("Branch: Create")).toBeInTheDocument();
    // ...and the footer count is > 0 (some commands matched).
    expect(
      screen.getByText(/command(s)? · ↑↓ to navigate/i),
    ).toBeInTheDocument();
  });

  it("shows an empty state when nothing matches", () => {
    render(<CommandPalette />);
    openViaEvent();
    fireEvent.change(screen.getByPlaceholderText(/Run a command/i), {
      target: { value: "zzz-no-such-command-zzz" },
    });
    expect(screen.getByText(/No commands match/i)).toBeInTheDocument();
  });

  it("drills into an op's form, runs it, and shows the result", async () => {
    invokeMock.mockResolvedValueOnce({ name: "feature/x", is_commit: false });
    render(<CommandPalette />);
    openViaEvent();
    fireEvent.change(screen.getByPlaceholderText(/Run a command/i), {
      target: { value: "branch create" },
    });
    fireEvent.click(screen.getByText("Branch: Create"));

    // The selected op's command name is shown in the detail header.
    expect(screen.getByText("branch_create")).toBeInTheDocument();

    // Fill the required field then Run.
    fireEvent.change(screen.getByLabelText(/Branch name/i), {
      target: { value: "feature/x" },
    });
    fireEvent.click(screen.getByRole("button", { name: /^Run$/i }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith(
        "branch_create",
        expect.objectContaining({ branch: "feature/x" }),
      );
    });
  });

  it("surfaces a command error in the detail view", async () => {
    invokeMock.mockRejectedValueOnce("boom: branch exists");
    render(<CommandPalette />);
    openViaEvent();
    fireEvent.change(screen.getByPlaceholderText(/Run a command/i), {
      target: { value: "branch create" },
    });
    fireEvent.click(screen.getByText("Branch: Create"));
    fireEvent.change(screen.getByLabelText(/Branch name/i), {
      target: { value: "dup" },
    });
    fireEvent.click(screen.getByRole("button", { name: /^Run$/i }));

    await waitFor(() => {
      expect(screen.getByText(/boom: branch exists/i)).toBeInTheDocument();
    });
  });
});
