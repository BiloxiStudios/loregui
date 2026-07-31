import { beforeEach, describe, expect, it, vi } from "vitest";
import { open } from "@tauri-apps/plugin-dialog";
import { documentDir, homeDir } from "@tauri-apps/api/path";
import { chooseDirectory, defaultDialogPath } from "./directoryPicker";

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(),
}));

vi.mock("@tauri-apps/api/path", () => ({
  documentDir: vi.fn(),
  homeDir: vi.fn(),
}));

const DOCUMENTS = "/home/you/Documents";
const HOME = "/home/you";

beforeEach(() => {
  vi.mocked(open).mockReset();
  vi.mocked(documentDir).mockReset();
  vi.mocked(homeDir).mockReset();
  vi.mocked(documentDir).mockResolvedValue(DOCUMENTS);
  vi.mocked(homeDir).mockResolvedValue(HOME);
});

describe("chooseDirectory", () => {
  it("returns one Windows directory and forwards the exact dialog options", async () => {
    vi.mocked(open).mockResolvedValue("E:\\lore");

    await expect(
      chooseDirectory({
        title: "Choose server storage",
        defaultPath: "D:\\existing",
      }),
    ).resolves.toBe("E:\\lore");
    expect(open).toHaveBeenCalledWith({
      directory: true,
      multiple: false,
      title: "Choose server storage",
      defaultPath: "D:\\existing",
    });
    // An absolute current value is used as-is — no path API call needed.
    expect(documentDir).not.toHaveBeenCalled();
  });

  it("returns null when cancelled", async () => {
    vi.mocked(open).mockResolvedValue(null);

    await expect(chooseDirectory({ title: "Choose" })).resolves.toBeNull();
  });

  /* SBAI-5841: the dialog must never be opened with no starting directory,
     because the OS then falls back to the process CWD (System32 for a Start
     Menu launch on Windows). */

  it("starts at Documents when no defaultPath is supplied", async () => {
    vi.mocked(open).mockResolvedValue(DOCUMENTS);

    await chooseDirectory({ title: "Choose" });

    expect(open).toHaveBeenCalledWith(
      expect.objectContaining({ directory: true, defaultPath: DOCUMENTS }),
    );
  });

  it.each(["", "   ", "lore", "./x", "C:foo", "\\\\server"])(
    "starts at Documents instead of inheriting the CWD for defaultPath %j",
    async (relative) => {
      vi.mocked(open).mockResolvedValue(null);

      await chooseDirectory({ title: "Choose", defaultPath: relative });

      expect(open).toHaveBeenCalledWith(
        expect.objectContaining({ defaultPath: DOCUMENTS }),
      );
    },
  );

  it("falls back to the home directory when documentDir throws", async () => {
    vi.mocked(documentDir).mockRejectedValue(new Error("no Documents"));
    vi.mocked(open).mockResolvedValue(null);

    await chooseDirectory({ title: "Choose", defaultPath: "lore" });

    expect(homeDir).toHaveBeenCalledTimes(1);
    expect(open).toHaveBeenCalledWith(
      expect.objectContaining({ defaultPath: HOME }),
    );
  });

  it("passes undefined only when the path API itself is unavailable", async () => {
    vi.mocked(documentDir).mockRejectedValue(new Error("unavailable"));
    vi.mocked(homeDir).mockRejectedValue(new Error("unavailable"));
    vi.mocked(open).mockResolvedValue(null);

    await chooseDirectory({ title: "Choose" });

    expect(open).toHaveBeenCalledWith(
      expect.objectContaining({ defaultPath: undefined }),
    );
  });

  it("keeps state unchanged on cancel even with a fallback default", async () => {
    vi.mocked(open).mockResolvedValue(null);

    await expect(
      chooseDirectory({ title: "Choose", defaultPath: "lore" }),
    ).resolves.toBeNull();
  });
});

describe("defaultDialogPath", () => {
  it("prefers Documents", async () => {
    await expect(defaultDialogPath()).resolves.toBe(DOCUMENTS);
    expect(homeDir).not.toHaveBeenCalled();
  });

  it("falls back to home when Documents is unavailable", async () => {
    vi.mocked(documentDir).mockRejectedValue(new Error("nope"));
    await expect(defaultDialogPath()).resolves.toBe(HOME);
  });

  it("returns undefined when both are unavailable", async () => {
    vi.mocked(documentDir).mockRejectedValue(new Error("nope"));
    vi.mocked(homeDir).mockRejectedValue(new Error("nope"));
    await expect(defaultDialogPath()).resolves.toBeUndefined();
  });

  it("never offers a non-absolute directory it was handed", async () => {
    // A path API that somehow answers with a relative value is no better than
    // the CWD — skip it rather than start the dialog there.
    vi.mocked(documentDir).mockResolvedValue("Documents");
    await expect(defaultDialogPath()).resolves.toBe(HOME);
  });
});
