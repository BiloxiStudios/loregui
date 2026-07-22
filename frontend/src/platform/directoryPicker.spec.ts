import { beforeEach, describe, expect, it, vi } from "vitest";
import { open } from "@tauri-apps/plugin-dialog";
import { chooseDirectory } from "./directoryPicker";

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(),
}));

describe("chooseDirectory", () => {
  beforeEach(() => {
    vi.mocked(open).mockReset();
  });

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
  });

  it("returns null when cancelled", async () => {
    vi.mocked(open).mockResolvedValue(null);

    await expect(chooseDirectory({ title: "Choose" })).resolves.toBeNull();
  });
});
