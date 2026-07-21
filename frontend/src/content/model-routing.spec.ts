/**
 * Routing tests for USD preview (SBAI-5433): kind detection and the
 * ModelViewer loader dispatch. The tinyusdz adapter is mocked here — the
 * real-engine behavioral coverage lives in ./usd/usdAdapter.spec.ts.
 */
import { describe, expect, it, vi } from "vitest";
import { kindOf } from "./kinds";

vi.mock("./usd/usdAdapter", () => ({
  loadUsdToThree: vi.fn(async () => {
    const THREE = await import("three");
    return { object: new THREE.Group() };
  }),
  USD_EXTERNAL_REFS_HINT: "this USD file references external assets",
}));

const { loadModelObject } = await import("./ModelViewer");
const { loadUsdToThree } = await import("./usd/usdAdapter");

describe("kindOf routing for USD extensions", () => {
  it.each(["usdz", "usd", "usda", "usdc"])("routes .%s to the model kind", (ext) => {
    expect(kindOf(`content/hero.${ext}`)).toBe("model");
  });

  it("keeps existing model extensions routed", () => {
    for (const ext of ["gltf", "glb", "fbx", "obj"]) {
      expect(kindOf(`content/hero.${ext}`)).toBe("model");
    }
  });
});

describe("loadModelObject dispatch", () => {
  it.each(["usdz", "usd", "usda", "usdc"] as const)(
    "sends .%s through the quarantined TinyUSDZ adapter",
    async (ext) => {
      const object = await loadModelObject(ext, "blob:mock-url");
      expect(object).toBeTruthy();
      expect(loadUsdToThree).toHaveBeenCalledWith("blob:mock-url");
    },
  );

  it("rejects an unknown model extension instead of guessing a loader", async () => {
    await expect(
      // @ts-expect-error -- deliberately invalid ext to prove the guard
      loadModelObject("step", "blob:mock-url"),
    ).rejects.toThrow("no loader");
  });
});
