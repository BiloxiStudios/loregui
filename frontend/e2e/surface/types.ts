/** A stable selector consumed by deterministic surface tests. */
export interface SurfaceSelector {
  strategy: "qa" | "role" | "uia" | "manifest";
  value: string;
}

export type SurfaceRisk =
  | "read"
  | "write_reversible"
  | "destructive"
  | "external";

export type SurfaceOracle =
  | "dom"
  | "ipc"
  | "state"
  | "filesystem"
  | "process"
  | "network"
  | "accessibility"
  | "screenshot";

export type SurfacePlatform = "linux" | "windows" | "macos";

/**
 * One state-dependent, fixture-owned interaction in the surface inventory.
 * `surface_id` is the action id accepted by later deterministic executors.
 */
export interface SurfaceAction {
  schema: 1;
  app_commit: string;
  surface_id: string;
  state_id: string;
  element_id: string;
  selector: SurfaceSelector;
  visible_name: string;
  preconditions: string[];
  risk: SurfaceRisk;
  expected_ipc: Array<{
    command: string;
    args_match: Record<string, unknown>;
  }>;
  oracles: SurfaceOracle[];
  cleanup: string | null;
  platform: SurfacePlatform[];
}

export type SurfaceCaseVariant = "success" | "error" | "cancel";

/**
 * Evidence supplied by a fixture after resolving an inventory selector. This
 * keeps selector uniqueness deterministic and separate from visual judgement.
 */
export interface SurfaceCase {
  action_id: string;
  state_id: string;
  selector_matches: number;
  variants: SurfaceCaseVariant[];
  fixture_ownership?: {
    token_ref: string;
    owned_paths: string[];
    loopback_endpoints: string[];
  };
}

/** One pre-parsed surface-map file. YAML parsing deliberately lives at the IO boundary. */
export interface SurfaceMapFile {
  file: string;
  inventory: SurfaceAction[];
  cases: SurfaceCase[];
}

export interface CompiledSurfaceMap {
  actions: readonly SurfaceAction[];
  cases: readonly SurfaceCase[];
  action_by_id: ReadonlyMap<string, SurfaceAction>;
  case_by_action_id: ReadonlyMap<string, SurfaceCase>;
}
