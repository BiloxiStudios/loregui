/**
 * SBAI-5560: the Host-a-server flow asks for the store path EXACTLY ONCE — in
 * step 1 (Choose Storage Backend), with a native folder picker. Steps 3
 * (Initialize server) and 4 (Host server) show the step-1 path as a read-only
 * summary with a clear role label and pass it unchanged to
 * `host_store_prepare` / `host_server_start`.
 */
import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";

const invokeMock = vi.fn();
const dialogOpenMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: (...args: unknown[]) => dialogOpenMock(...args),
}));

import type { StorageBackendConfig } from "../../api";
import PathField from "./PathField";
import BackendPicker from "./BackendPicker";
import InitStore from "./InitStore";
import ServiceSetup from "./ServiceSetup";

const STEP1_PATH = "/srv/lore/store";

function fieldByLabel(label: string): HTMLElement {
  const field = screen
    .getByLabelText(label)
    .closest<HTMLElement>(".onboarding-field");
  if (!field) throw new Error(`missing onboarding field for ${label}`);
  return field;
}

beforeEach(() => {
  invokeMock.mockReset();
  dialogOpenMock.mockReset();
  dialogOpenMock.mockResolvedValue(STEP1_PATH);
});

describe("PathField", () => {
  it("Browse opens a native directory picker and fills the input", async () => {
    const onChange = vi.fn();
    render(
      <PathField
        id="pf"
        label="Local Storage Path"
        value=""
        onChange={onChange}
        dialogTitle="Choose local storage directory"
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Browse…" }));

    await waitFor(() => expect(onChange).toHaveBeenCalledWith(STEP1_PATH));
    expect(dialogOpenMock).toHaveBeenCalledTimes(1);
    expect(dialogOpenMock).toHaveBeenCalledWith(
      expect.objectContaining({ directory: true, multiple: false }),
    );
  });

  it("keeps the current value when the picker is cancelled", async () => {
    dialogOpenMock.mockResolvedValue(null);
    const onChange = vi.fn();
    render(
      <PathField
        id="pf"
        label="Path"
        value="/keep"
        onChange={onChange}
        dialogTitle="Pick"
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Browse…" }));

    await waitFor(() => expect(dialogOpenMock).toHaveBeenCalledTimes(1));
    expect(onChange).not.toHaveBeenCalled();
    expect(screen.getByLabelText("Path")).toHaveValue("/keep");
  });

  it("disables input and button while disabled, and shows a browsing state", async () => {
    let resolveOpen!: (value: string | null) => void;
    dialogOpenMock.mockReturnValue(
      new Promise<string | null>((res) => {
        resolveOpen = res;
      }),
    );
    const onChange = vi.fn();
    const view = render(
      <PathField
        id="pf"
        label="Path"
        value=""
        onChange={onChange}
        dialogTitle="Pick"
        disabled
      />,
    );
    expect(screen.getByLabelText("Path")).toBeDisabled();
    expect(screen.getByRole("button", { name: "Browse…" })).toBeDisabled();

    view.rerender(
      <PathField
        id="pf"
        label="Path"
        value=""
        onChange={onChange}
        dialogTitle="Pick"
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Browse…" }));
    expect(
      await screen.findByRole("button", { name: "Browsing…" }),
    ).toBeDisabled();

    resolveOpen(null);
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "Browse…" })).toBeEnabled(),
    );
    expect(onChange).not.toHaveBeenCalled();
  });

  it("read-only summary renders the path as static text with its role label", () => {
    render(
      <PathField
        id="pf"
        label="Shared store — created in step 1"
        value={STEP1_PATH}
        readOnly
      />,
    );

    expect(screen.getByText("Shared store — created in step 1")).toBeVisible();
    expect(screen.getByText(STEP1_PATH)).toBeVisible();
    expect(screen.queryByRole("textbox")).toBeNull();
    expect(screen.queryByRole("button")).toBeNull();
  });
});

describe("host flow asks for the store path exactly once (SBAI-5560)", () => {
  it("step 1 holds the only editable store-path asks, both with native pickers", async () => {
    invokeMock.mockImplementation((command: string, args: { path: string }) => {
      if (command === "host_store_prepare") return Promise.resolve(args.path);
      return Promise.reject(new Error(`unexpected command ${command}`));
    });
    const onConfigured = vi.fn();
    render(<BackendPicker onConfigured={onConfigured} />);

    // The required store path plus the optional mutable store — both pickers,
    // and the only two Browse buttons in the whole flow.
    expect(screen.getByLabelText("Local Storage Path")).toBeInTheDocument();
    expect(
      screen.getByLabelText("Mutable Store Path (optional)"),
    ).toBeInTheDocument();
    expect(screen.getAllByRole("button", { name: "Browse…" })).toHaveLength(2);

    const primaryField = fieldByLabel("Local Storage Path");
    fireEvent.click(within(primaryField).getByRole("button", { name: "Browse…" }));
    await waitFor(() =>
      expect(screen.getByLabelText("Local Storage Path")).toHaveValue(
        STEP1_PATH,
      ),
    );
    fireEvent.click(screen.getByRole("button", { name: "Prepare Store" }));

    await waitFor(() => expect(onConfigured).toHaveBeenCalledTimes(1));
    const config = onConfigured.mock.calls[0][0] as StorageBackendConfig;
    expect(config.kind).toBe("local");
    expect(config.path).toBe(STEP1_PATH);
  });

  it("step 3 shows the step-1 path read-only and prepares exactly that path", async () => {
    invokeMock.mockImplementation((command: string, args: { path: string }) => {
      if (command === "host_store_prepare") return Promise.resolve(args.path);
      return Promise.reject(new Error(`unexpected command ${command}`));
    });
    const config: StorageBackendConfig = { kind: "local", path: STEP1_PATH };
    render(<InitStore config={config} />);

    // Read-only summary with a clear role label — no picker, no path input.
    expect(screen.getByText("Shared store — created in step 1")).toBeVisible();
    expect(screen.getByText(STEP1_PATH)).toBeVisible();
    expect(screen.queryByRole("button", { name: "Browse…" })).toBeNull();
    // The only textbox left is the optional repository name.
    expect(screen.getAllByRole("textbox")).toHaveLength(1);
    expect(screen.getByLabelText("Repository Name (optional)")).toBeVisible();

    fireEvent.click(screen.getByRole("button", { name: "Create Store" }));

    await waitFor(() =>
      expect(screen.getByText("Store ready")).toBeInTheDocument(),
    );
    expect(invokeMock).toHaveBeenCalledWith("host_store_prepare", {
      path: STEP1_PATH,
      mutableStore: null,
    });
  });

  it("step 4 shows the step-1 path read-only and serves exactly that path", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "host_server_status") {
        return Promise.resolve({ running: false });
      }
      if (command === "host_server_start") {
        return Promise.resolve({
          running: true,
          url: "lore://localhost/project",
          storeDir: STEP1_PATH,
        });
      }
      return Promise.reject(new Error(`unexpected command ${command}`));
    });
    render(<ServiceSetup storePath={STEP1_PATH} repoName="project" />);

    // Read-only summary with a clear role label — no picker, no path input.
    expect(screen.getByText("Serving store")).toBeVisible();
    expect(screen.getByText(STEP1_PATH)).toBeVisible();
    expect(screen.queryByRole("button", { name: "Browse…" })).toBeNull();
    expect(screen.queryByLabelText("Store directory to serve")).toBeNull();

    fireEvent.click(await screen.findByRole("button", { name: "Start Hosting" }));

    await waitFor(() =>
      expect(screen.getByText("Server is hosting")).toBeInTheDocument(),
    );
    expect(invokeMock).toHaveBeenCalledWith(
      "host_server_start",
      expect.objectContaining({ storeDir: STEP1_PATH }),
    );
  });
});
