/**
 * Fresh local-FS "Host a server" wizard path (loregui host-wizard bug fix).
 *
 * A brand-new user hosting a LOCAL filesystem server must get through steps
 * 1 (Choose Storage Backend), 2 (Validate connectivity) and 3 (Initialize
 * server / Create store) with NO errors, ending at the working host step.
 *
 * The bug: step 1 called `storage_open` (which requires an existing `.lore`
 * repository → "missing .lore") and step 3 called `shared_store_create` (which
 * requires a remote URL → "no remote URL"). Both are the wrong abstraction for a
 * local host store — that store is a plain directory the loreserver fills at
 * host time. These tests pin the fix: the local path now drives the
 * `host_store_prepare` / `host_store_probe` filesystem commands and succeeds
 * without any `.lore` repo or remote URL.
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";

const invokeMock = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

import type { StorageBackendConfig } from "../../api";
import BackendPicker from "./BackendPicker";
import ValidateConnectivity from "./ValidateConnectivity";
import InitStore, { type InitStoreResult } from "./InitStore";

/** All invoked (name, args) pairs, in order. */
function calls(): Array<[string, Record<string, unknown> | undefined]> {
  return invokeMock.mock.calls.map((c) => [
    c[0] as string,
    c[1] as Record<string, unknown> | undefined,
  ]);
}
function names(): string[] {
  return calls().map(([n]) => n);
}

beforeEach(() => {
  invokeMock.mockReset();
});

describe("step 1 — Choose Storage Backend (local FS)", () => {
  it("prepares the store dir via host_store_prepare, never opens a .lore repo", async () => {
    // host_store_prepare echoes back the resolved absolute path.
    invokeMock.mockImplementation((cmd: string, args: Record<string, unknown>) => {
      if (cmd === "host_store_prepare") return Promise.resolve(args.path);
      return Promise.reject(`unexpected command ${cmd}`);
    });

    const onConfigured = vi.fn();
    render(<BackendPicker onConfigured={onConfigured} />);

    fireEvent.change(screen.getByLabelText("Local Storage Path"), {
      target: { value: "C:/loredata" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Prepare Store" }));

    await waitFor(() => expect(onConfigured).toHaveBeenCalledTimes(1));

    // It used the local-FS prepare command, NOT the repository storage_open that
    // produced "missing .lore".
    expect(names()).toContain("host_store_prepare");
    expect(names()).not.toContain("storage_open");
    expect(calls()[0]).toEqual([
      "host_store_prepare",
      { path: "C:/loredata", mutableStore: null },
    ]);

    const cfg = onConfigured.mock.calls[0][0] as StorageBackendConfig;
    expect(cfg.kind).toBe("local");
    expect(cfg.path).toBe("C:/loredata");
    expect(screen.getByText(/Storage opened/)).toBeInTheDocument();
  });
});

describe("step 2 — Validate connectivity (local FS)", () => {
  it("round-trips via host_store_probe, not the content-store put/get", async () => {
    invokeMock.mockResolvedValue(undefined);
    const config: StorageBackendConfig = { kind: "local", path: "C:/loredata" };
    render(<ValidateConnectivity config={config} />);

    fireEvent.click(
      screen.getByRole("button", { name: "Run Connectivity Test" }),
    );

    await waitFor(() =>
      expect(screen.getByText(/backend is reachable/)).toBeInTheDocument(),
    );
    expect(calls()).toEqual([["host_store_probe", { path: "C:/loredata" }]]);
    expect(names()).not.toContain("storage_put");
    expect(names()).not.toContain("storage_open");
  });
});

describe("step 3 — Initialize server / Create store (local FS)", () => {
  it("creates the store locally, never calling shared_store_create / remote", async () => {
    invokeMock.mockImplementation((cmd: string, args: Record<string, unknown>) => {
      if (cmd === "host_store_prepare") return Promise.resolve(args.path);
      return Promise.reject(`unexpected command ${cmd}`);
    });

    let result: InitStoreResult | null = null;
    const config: StorageBackendConfig = { kind: "local", path: "C:/loredata" };
    render(
      <InitStore config={config} onInitialized={(r) => (result = r)} />,
    );

    // Store path is prefilled from step 1's config.
    const pathInput = screen.getByLabelText("Store Path") as HTMLInputElement;
    expect(pathInput.value).toBe("C:/loredata");

    fireEvent.change(screen.getByLabelText("Repository Name (optional)"), {
      target: { value: "my-repo" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Create Store" }));

    await waitFor(() =>
      expect(screen.getByText("Store ready")).toBeInTheDocument(),
    );

    expect(names()).toEqual(["host_store_prepare"]);
    expect(names()).not.toContain("shared_store_create");
    expect(names()).not.toContain("repository_create");
    expect(result).toEqual({ storePath: "C:/loredata", repoName: "my-repo" });
  });
});
