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
const documentDirMock = vi.fn();
const homeDirMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: (...args: unknown[]) => dialogOpenMock(...args),
}));

vi.mock("@tauri-apps/api/path", () => ({
  documentDir: (...args: unknown[]) => documentDirMock(...args),
  homeDir: (...args: unknown[]) => homeDirMock(...args),
}));

import type { StorageBackendConfig } from "../../api";
import PathField from "./PathField";
import BackendPicker from "./BackendPicker";
import InitStore from "./InitStore";
import ServiceSetup from "./ServiceSetup";

const STEP1_PATH = "/srv/lore/store";
const DOCUMENTS = "/home/you/Documents";

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
  documentDirMock.mockReset();
  documentDirMock.mockResolvedValue(DOCUMENTS);
  homeDirMock.mockReset();
  homeDirMock.mockResolvedValue("/home/you");
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

  /* SBAI-5841 --------------------------------------------------------- */

  it("Browse from an empty field starts at Documents, never the process CWD", async () => {
    render(
      <PathField
        id="pf"
        label="Local Storage Path"
        value=""
        onChange={vi.fn()}
        dialogTitle="Choose local storage directory"
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Browse…" }));

    await waitFor(() => expect(dialogOpenMock).toHaveBeenCalledTimes(1));
    expect(dialogOpenMock).toHaveBeenCalledWith(
      expect.objectContaining({ directory: true, defaultPath: DOCUMENTS }),
    );
  });

  it("Browse from a relative field starts at Documents, not that value", async () => {
    render(
      <PathField
        id="pf"
        label="Path"
        value="lore"
        onChange={vi.fn()}
        dialogTitle="Pick"
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Browse…" }));

    await waitFor(() => expect(dialogOpenMock).toHaveBeenCalledTimes(1));
    expect(dialogOpenMock).toHaveBeenCalledWith(
      expect.objectContaining({ defaultPath: DOCUMENTS }),
    );
  });

  it("Browse reuses the current value when it is already absolute", async () => {
    render(
      <PathField
        id="pf"
        label="Path"
        value={`  ${STEP1_PATH}  `}
        onChange={vi.fn()}
        dialogTitle="Pick"
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Browse…" }));

    await waitFor(() => expect(dialogOpenMock).toHaveBeenCalledTimes(1));
    expect(dialogOpenMock).toHaveBeenCalledWith(
      expect.objectContaining({ defaultPath: STEP1_PATH }),
    );
    expect(documentDirMock).not.toHaveBeenCalled();
  });

  it("refuses to open the picker when no trusted starting folder exists", async () => {
    // Both path lookups fail → defaultDialogPath() is undefined. Opening the
    // dialog anyway would let the OS start at the process CWD, so PathField
    // explains itself instead of opening (SBAI-5841 gap 5).
    documentDirMock.mockRejectedValue(new Error("unavailable"));
    homeDirMock.mockRejectedValue(new Error("unavailable"));
    const onChange = vi.fn();
    render(
      <PathField
        id="pf"
        label="Path"
        value=""
        onChange={onChange}
        dialogTitle="Pick"
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Browse…" }));

    const error = await screen.findByText(/no trusted starting folder/);
    expect(error).toHaveClass("server-config-field-error");
    expect(error.id).toBe("pf-browse-error");
    expect(screen.getByLabelText("Path").getAttribute("aria-describedby")).toContain(
      "pf-browse-error",
    );
    expect(dialogOpenMock).not.toHaveBeenCalled();
    expect(onChange).not.toHaveBeenCalled();
    // The button recovers — this is a refusal, not a stuck state.
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "Browse…" })).toBeEnabled(),
    );
  });

  it("clears the browse failure once the user types a path instead", async () => {
    documentDirMock.mockRejectedValue(new Error("unavailable"));
    homeDirMock.mockRejectedValue(new Error("unavailable"));
    render(
      <PathField
        id="pf"
        label="Path"
        value=""
        onChange={vi.fn()}
        dialogTitle="Pick"
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Browse…" }));
    await screen.findByText(/no trusted starting folder/);

    fireEvent.change(screen.getByLabelText("Path"), {
      target: { value: "/srv/lore" },
    });

    expect(screen.queryByText(/no trusted starting folder/)).toBeNull();
    expect(screen.getByLabelText("Path")).not.toHaveAttribute(
      "aria-describedby",
    );
  });

  it("does not start at a value that is absolute only on the other platform", async () => {
    // Union-acceptable, but not absolute on the host running the test.
    render(
      <PathField
        id="pf"
        label="Path"
        value={"C:\\LoreData"}
        onChange={vi.fn()}
        dialogTitle="Pick"
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Browse…" }));

    await waitFor(() => expect(dialogOpenMock).toHaveBeenCalledTimes(1));
    expect(dialogOpenMock).toHaveBeenCalledWith(
      expect.objectContaining({ defaultPath: DOCUMENTS }),
    );
  });

  it("manual relative entry shows an actionable error and marks the input invalid", () => {
    render(
      <PathField
        id="pf"
        label="Local Storage Path"
        value="lore"
        onChange={vi.fn()}
        dialogTitle="Pick"
      />,
    );

    const input = screen.getByLabelText("Local Storage Path");
    expect(input).toHaveAttribute("aria-invalid", "true");
    expect(input).toHaveAttribute("aria-describedby", "pf-path-error");

    const error = document.getElementById("pf-path-error");
    expect(error).not.toBeNull();
    expect(error).toHaveClass("server-config-field-error");
    expect(error?.textContent ?? "").toContain('"lore"');
    expect(error?.textContent ?? "").toContain("absolute path");
  });

  it("explains the Windows drive-relative case at the field", () => {
    render(
      <PathField
        id="pf"
        label="Path"
        value="C:lore"
        onChange={vi.fn()}
        dialogTitle="Pick"
      />,
    );

    expect(
      screen.getByText(/backslash after the drive letter/),
    ).toHaveClass("server-config-field-error");
  });

  it("shows no error for an absolute value or an untouched empty field", () => {
    const view = render(
      <PathField
        id="pf"
        label="Path"
        value={STEP1_PATH}
        onChange={vi.fn()}
        dialogTitle="Pick"
      />,
    );
    expect(document.querySelector(".server-config-field-error")).toBeNull();
    expect(screen.getByLabelText("Path")).not.toHaveAttribute("aria-invalid");

    view.rerender(
      <PathField
        id="pf"
        label="Path"
        value=""
        onChange={vi.fn()}
        dialogTitle="Pick"
      />,
    );
    expect(document.querySelector(".server-config-field-error")).toBeNull();

    view.rerender(
      <PathField
        id="pf"
        label="Path"
        value={"C:\\LoreData"}
        onChange={vi.fn()}
        dialogTitle="Pick"
      />,
    );
    expect(document.querySelector(".server-config-field-error")).toBeNull();
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

describe("BackendPicker gates on absolute lifecycle paths (SBAI-5841)", () => {
  it("keeps Prepare Store disabled for a relative local store path", () => {
    render(<BackendPicker />);
    const prepare = () => screen.getByRole("button", { name: "Prepare Store" });
    expect(prepare()).toBeDisabled();

    fireEvent.change(screen.getByLabelText("Local Storage Path"), {
      target: { value: "lore" },
    });
    expect(prepare()).toBeDisabled();
    expect(screen.getByText(/is a relative path/)).toHaveClass(
      "server-config-field-error",
    );

    fireEvent.change(screen.getByLabelText("Local Storage Path"), {
      target: { value: STEP1_PATH },
    });
    expect(prepare()).toBeEnabled();
    expect(document.querySelector(".server-config-field-error")).toBeNull();
    expect(invokeMock).not.toHaveBeenCalled();
  });

  it("keeps Prepare Store disabled for a relative mutable store path", () => {
    render(<BackendPicker />);
    fireEvent.change(screen.getByLabelText("Local Storage Path"), {
      target: { value: STEP1_PATH },
    });
    expect(screen.getByRole("button", { name: "Prepare Store" })).toBeEnabled();

    fireEvent.change(screen.getByLabelText("Mutable Store Path (optional)"), {
      target: { value: "C:mutable" },
    });
    expect(screen.getByRole("button", { name: "Prepare Store" })).toBeDisabled();
    expect(
      screen.getByText(/backslash after the drive letter/),
    ).toBeVisible();

    // Optional means optional: clearing it re-enables the step.
    fireEvent.change(screen.getByLabelText("Mutable Store Path (optional)"), {
      target: { value: "" },
    });
    expect(screen.getByRole("button", { name: "Prepare Store" })).toBeEnabled();
    expect(invokeMock).not.toHaveBeenCalled();
  });

  it("keeps Open Storage disabled for a relative S3 mutable store path", () => {
    render(<BackendPicker />);
    fireEvent.click(screen.getByRole("radio", { name: /S3-compatible/ }));
    fireEvent.change(screen.getByLabelText("Endpoint URL"), {
      target: { value: "https://s3.example.com" },
    });
    fireEvent.change(screen.getByLabelText("Bucket Name"), {
      target: { value: "lore" },
    });
    expect(screen.getByRole("button", { name: "Open Storage" })).toBeEnabled();

    fireEvent.change(screen.getByLabelText("Mutable Store Path (optional)"), {
      target: { value: "./mutable" },
    });
    expect(screen.getByRole("button", { name: "Open Storage" })).toBeDisabled();
    expect(invokeMock).not.toHaveBeenCalled();
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
