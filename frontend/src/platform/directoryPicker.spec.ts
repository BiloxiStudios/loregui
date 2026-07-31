import { beforeEach, describe, expect, it, vi } from "vitest";
import { open } from "@tauri-apps/plugin-dialog";
import { documentDir, homeDir } from "@tauri-apps/api/path";
import {
  chooseDirectory,
  defaultDialogPath,
  NO_TRUSTED_START_MESSAGE,
} from "./directoryPicker";
import { isWindowsPlatform } from "./pathPolicy";

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
  it("returns the chosen directory and forwards the exact dialog options", async () => {
    vi.mocked(open).mockResolvedValue("E:\\lore");

    await expect(
      chooseDirectory({
        title: "Choose server storage",
        defaultPath: "/srv/existing",
      }),
    ).resolves.toBe("E:\\lore");
    expect(open).toHaveBeenCalledWith({
      directory: true,
      multiple: false,
      title: "Choose server storage",
      defaultPath: "/srv/existing",
    });
    // A natively-absolute current value is used as-is — no path API call.
    expect(documentDir).not.toHaveBeenCalled();
  });

  it("does not trust a foreign-platform absolute path as a starting folder", async () => {
    // SBAI-5841 / gap 6: `D:\…` is union-acceptable (so the inline field error
    // stays quiet) but it is not absolute on THIS platform, so it must never
    // be handed to this platform's dialog.
    expect(isWindowsPlatform()).toBe(false);
    vi.mocked(open).mockResolvedValue(null);

    await chooseDirectory({ title: "Choose", defaultPath: "D:\\existing" });

    expect(open).toHaveBeenCalledWith(
      expect.objectContaining({ defaultPath: DOCUMENTS }),
    );
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

  it("fails closed — never opens the dialog with no trusted start", async () => {
    // Opening with defaultPath undefined is precisely the CWD inheritance this
    // whole change exists to prevent, so refuse instead of guessing.
    vi.mocked(documentDir).mockRejectedValue(new Error("unavailable"));
    vi.mocked(homeDir).mockRejectedValue(new Error("unavailable"));

    await expect(chooseDirectory({ title: "Choose" })).rejects.toThrow(
      NO_TRUSTED_START_MESSAGE,
    );
    expect(open).not.toHaveBeenCalled();
  });

  it("fails closed for a relative defaultPath when no fallback resolves", async () => {
    vi.mocked(documentDir).mockRejectedValue(new Error("unavailable"));
    vi.mocked(homeDir).mockResolvedValue("Documents");

    await expect(
      chooseDirectory({ title: "Choose", defaultPath: "lore" }),
    ).rejects.toThrow(/enter an absolute path manually/);
    expect(open).not.toHaveBeenCalled();
  });

  it("uses the resolved Documents value when the field holds a relative path", async () => {
    vi.mocked(open).mockResolvedValue("/picked");

    await expect(
      chooseDirectory({ title: "Choose", defaultPath: "./relative" }),
    ).resolves.toBe("/picked");
    expect(open).toHaveBeenCalledWith(
      expect.objectContaining({ defaultPath: DOCUMENTS }),
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

  it("vets candidates natively, not by the lenient union", async () => {
    // A `C:\…` answer on a POSIX host is absolute for the *other* family only;
    // it is no more usable here than a relative one.
    expect(isWindowsPlatform()).toBe(false);
    vi.mocked(documentDir).mockResolvedValue("C:\\Users\\you\\Documents");
    await expect(defaultDialogPath()).resolves.toBe(HOME);
  });
});
