import { describe, expect, it } from "vitest";
import { readFileSync } from "node:fs";
import { compileSurfaceMap, type SurfaceMapFile } from "./compile";
import type { SurfaceAction, SurfaceCaseVariant } from "./types";

const action: SurfaceAction = {
  schema: 1,
  app_commit: "test-head",
  surface_id: "server.repository.clone",
  state_id: "local-host-no-auth",
  element_id: "server.repository.clone",
  selector: { strategy: "qa", value: "server.repository.clone" },
  visible_name: "Clone repository",
  preconditions: ["fixture-owned-server", "repository-listed"],
  risk: "write_reversible",
  expected_ipc: [{ command: "repository_clone", args_match: {} }],
  oracles: ["dom", "ipc", "state", "filesystem"],
  cleanup: "delete fixture-owned clone root",
  platform: ["linux", "windows", "macos"],
};

const allVariants: SurfaceCaseVariant[] = ["success", "error", "cancel"];

function validFile(): SurfaceMapFile {
  return {
    file: "core.yaml",
    inventory: [action],
    cases: [
      {
        action_id: "server.repository.clone",
        state_id: "local-host-no-auth",
        selector_matches: 1,
        variants: [...allVariants],
      },
    ],
  };
}

describe("compileSurfaceMap", () => {
  it("compiles the checked-in P0/P1 core inventory", () => {
    const coreMap = JSON.parse(
      readFileSync(new URL("./map/core.yaml", import.meta.url), "utf8"),
    ) as Omit<SurfaceMapFile, "file">;

    const compiled = compileSurfaceMap([{ file: "map/core.yaml", ...coreMap }]);

    expect(compiled.actions.map((entry) => entry.surface_id)).toEqual([
      "onboarding.local.open-existing",
      "onboarding.client.connect",
      "server.repository.clone",
      "settings.server.remove",
    ]);
  });

  it("compiles a uniquely resolved inventory into action and case indexes", () => {
    const compiled = compileSurfaceMap([validFile()]);

    expect(compiled.actions).toHaveLength(1);
    expect(compiled.actions[0]?.surface_id).toBe("server.repository.clone");
    expect(compiled.case_by_action_id.get("server.repository.clone")?.selector_matches).toBe(1);
  });

  it("rejects duplicate action ids before an executor can select the wrong action", () => {
    const file = validFile();
    file.inventory.push({ ...action });

    expect(() => compileSurfaceMap([file])).toThrow(/duplicate action id: server\.repository\.clone/i);
  });

  it.each([0, 2])("rejects selectors that resolve to %i elements", (selector_matches) => {
    const file = validFile();
    file.cases[0]!.selector_matches = selector_matches;

    expect(() => compileSurfaceMap([file])).toThrow(
      new RegExp(`selector for server\\.repository\\.clone resolved to ${selector_matches} elements`, "i"),
    );
  });

  it.each(allVariants)(
    "rejects a case without its %s variant",
    (missingVariant) => {
      const file = validFile();
      file.cases[0]!.variants = allVariants.filter(
        (variant) => variant !== missingVariant,
      );

      expect(() => compileSurfaceMap([file])).toThrow(
        new RegExp(`missing ${missingVariant} variant for server\\.repository\\.clone`, "i"),
      );
    },
  );

  it("rejects destructive actions unless a fixture owner and cleanup are both declared", () => {
    const file = validFile();
    file.inventory[0] = { ...action, risk: "destructive", cleanup: null };

    expect(() => compileSurfaceMap([file])).toThrow(
      /destructive action server\.repository\.clone requires fixture ownership and cleanup/i,
    );
  });

  it("rejects actions that leave deterministic oracles unspecified", () => {
    const file = validFile();
    file.inventory[0] = { ...action, oracles: [] };

    expect(() => compileSurfaceMap([file])).toThrow(
      /action server\.repository\.clone requires at least one oracle/i,
    );
  });

  it("rejects inventory entries that cannot be exercised by a case", () => {
    const file = validFile();
    file.cases = [];

    expect(() => compileSurfaceMap([file])).toThrow(
      /inventory action server\.repository\.clone has no case/i,
    );
  });
});
