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
  expected_ipc: [
    {
      command: "repository_clone",
      args_match: {
        url: "lore://127.0.0.1:7177/fixture-repository",
        dest: "$fixture.clone_root",
      },
    },
  ],
  oracles: ["dom", "ipc", "state", "filesystem"],
  cleanup: "delete fixture-owned clone root",
  platform: ["linux", "windows", "macos"],
};

const allVariants: SurfaceCaseVariant[] = ["success", "error", "cancel"];

const fixtureOwnership = {
  token_ref: "run.ownership_token",
  owned_paths: ["fixture-profile"],
  loopback_endpoints: ["lore://127.0.0.1:7177/fixture-repository"],
};

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
      readFileSync("e2e/surface/map/core.yaml", "utf8"),
    ) as Omit<SurfaceMapFile, "file">;

    const compiled = compileSurfaceMap([{ file: "map/core.yaml", ...coreMap }]);

    expect(compiled.actions.map((entry) => entry.surface_id)).toEqual([
      "onboarding.local.open-existing",
      "onboarding.client.connect",
      "server.repository.clone",
      "repository.delete",
    ]);
    expect(compiled.actions.map((entry) => entry.expected_ipc[0]?.command)).toEqual([
      "open_repository",
      "auth_login_interactive",
      "repository_clone",
      "repository_delete",
    ]);
    expect(compiled.actions[1]?.expected_ipc[0]?.args_match).toEqual({
      remoteUrl: "lore://127.0.0.1:7177/fixture-repository",
    });
    expect(compiled.actions[3]?.expected_ipc[0]?.args_match).toEqual({
      repositoryUrl: "lore://127.0.0.1:7177/fixture-repository",
    });
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

  it("rejects screenshot-only evidence because visual classification cannot decide pass or fail", () => {
    const file = validFile();
    file.inventory[0] = { ...action, oracles: ["screenshot"] };

    expect(() => compileSurfaceMap([file])).toThrow(
      /action server\.repository\.clone requires at least one authoritative oracle/i,
    );
  });

  it.each(["dom", "accessibility"] as const)(
    "rejects %s-only evidence because it is not an authoritative pass/fail oracle",
    (oracle) => {
      const file = validFile();
      file.inventory[0] = { ...action, oracles: [oracle] };

      expect(() => compileSurfaceMap([file])).toThrow(
        /action server\.repository\.clone requires at least one authoritative oracle/i,
      );
    },
  );

  it("rejects malformed risk values before they can bypass elevated-risk safety checks", () => {
    const file = validFile();
    file.inventory[0] = { ...action, risk: "unsafe" } as unknown as SurfaceAction;

    expect(() => compileSurfaceMap([file])).toThrow(
      /action server\.repository\.clone has invalid risk/i,
    );
  });

  it("rejects malformed IPC argument patterns before elevated-risk validation", () => {
    const file = validFile();
    file.inventory[0] = {
      ...action,
      risk: "external",
      cleanup: "disconnect fixture-owned loopback client",
      expected_ipc: [
        { command: "auth_login_interactive", args_match: null },
      ] as unknown as SurfaceAction["expected_ipc"],
    };
    file.cases[0]!.fixture_ownership = fixtureOwnership;

    expect(() => compileSurfaceMap([file])).toThrow(
      /action server\.repository\.clone has invalid expected_ipc args_match/i,
    );
  });

  it.each(["external", "destructive"] as const)(
    "rejects %s actions whose IPC pattern has no bound fixture arguments",
    (risk) => {
      const file = validFile();
      file.inventory[0] = {
        ...action,
        risk,
        cleanup: "reset fixture-owned state",
        expected_ipc: [{ command: "repository_delete", args_match: {} }],
      };
      file.cases[0]!.fixture_ownership = fixtureOwnership;

      expect(() => compileSurfaceMap([file])).toThrow(
        new RegExp(`${risk} action server\\.repository\\.clone requires expected IPC arguments`, "i"),
      );
    },
  );

  it("rejects an external action whose URL points outside the declared fixture ownership", () => {
    const file = validFile();
    file.inventory[0] = {
      ...action,
      risk: "external",
      cleanup: "disconnect fixture-owned loopback client",
      expected_ipc: [
        {
          command: "auth_login_interactive",
          args_match: { remoteUrl: "https://production.example" },
        },
      ],
    };
    file.cases[0]!.fixture_ownership = fixtureOwnership;

    expect(() => compileSurfaceMap([file])).toThrow(
      /external action server\.repository\.clone has unowned target remoteUrl/i,
    );
  });

  it("accepts a token-backed fixture target for an external action", () => {
    const file = validFile();
    file.inventory[0] = {
      ...action,
      risk: "external",
      cleanup: "disconnect fixture-owned loopback client",
      expected_ipc: [
        {
          command: "auth_login_interactive",
          args_match: { remoteUrl: "$fixture.remote_url" },
        },
      ],
    };
    file.cases[0]!.fixture_ownership = fixtureOwnership;

    expect(() => compileSurfaceMap([file])).not.toThrow();
  });

  it("rejects inventory entries that cannot be exercised by a case", () => {
    const file = validFile();
    file.cases = [];

    expect(() => compileSurfaceMap([file])).toThrow(
      /inventory action server\.repository\.clone has no case/i,
    );
  });
});
