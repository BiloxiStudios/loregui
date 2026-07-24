import type {
  CompiledSurfaceMap,
  SurfaceAction,
  SurfaceCase,
  SurfaceCaseVariant,
  SurfaceMapFile,
} from "./types";

export type { CompiledSurfaceMap, SurfaceMapFile } from "./types";

const REQUIRED_VARIANTS: readonly SurfaceCaseVariant[] = [
  "success",
  "error",
  "cancel",
];

const RISKS = new Set<SurfaceAction["risk"]>([
  "read",
  "write_reversible",
  "destructive",
  "external",
]);
const ORACLES = new Set<SurfaceAction["oracles"][number]>([
  "dom",
  "ipc",
  "state",
  "filesystem",
  "process",
  "network",
  "accessibility",
  "screenshot",
]);
const PLATFORMS = new Set<SurfaceAction["platform"][number]>([
  "linux",
  "windows",
  "macos",
]);
const SELECTOR_STRATEGIES = new Set<SurfaceAction["selector"]["strategy"]>([
  "qa",
  "role",
  "uia",
  "manifest",
]);

/**
 * Validates already-parsed surface-map files and builds immutable lookup
 * indexes. File parsing and fixture probing are deliberately outside this
 * function: the compiler only trusts the deterministic evidence it receives.
 */
export function compileSurfaceMap(
  files: readonly unknown[],
): CompiledSurfaceMap {
  const actionById = new Map<string, SurfaceAction>();
  const caseByActionId = new Map<string, SurfaceCase>();

  for (const [fileIndex, rawFile] of files.entries()) {
    const file = parseSurfaceMapFile(rawFile, fileIndex);
    for (const action of file.inventory) {
      validateAction(action);

      if (actionById.has(action.surface_id)) {
        throw new Error(`duplicate action id: ${action.surface_id}`);
      }
      actionById.set(action.surface_id, action);
    }

    for (const surfaceCase of file.cases) {
      if (caseByActionId.has(surfaceCase.action_id)) {
        throw new Error(`duplicate case for action: ${surfaceCase.action_id}`);
      }
      caseByActionId.set(surfaceCase.action_id, surfaceCase);
    }
  }

  for (const [actionId, action] of actionById) {
    const surfaceCase = caseByActionId.get(actionId);
    if (!surfaceCase) {
      throw new Error(`inventory action ${actionId} has no case`);
    }
    if (surfaceCase.state_id !== action.state_id) {
      throw new Error(`case for ${actionId} targets state ${surfaceCase.state_id}, expected ${action.state_id}`);
    }
    validateCase(action, surfaceCase);
  }

  for (const actionId of caseByActionId.keys()) {
    if (!actionById.has(actionId)) {
      throw new Error(`case references unknown action: ${actionId}`);
    }
  }

  return {
    actions: [...actionById.values()],
    cases: [...caseByActionId.values()],
    action_by_id: actionById,
    case_by_action_id: caseByActionId,
  };
}

function validateAction(action: SurfaceAction): void {
  if (!action.oracles.length) {
    throw new Error(`action ${action.surface_id} requires at least one oracle`);
  }
  if (!action.oracles.some((oracle) => oracle !== "screenshot")) {
    throw new Error(
      `action ${action.surface_id} requires at least one authoritative nonvisual oracle`,
    );
  }
}

function validateCase(action: SurfaceAction, surfaceCase: SurfaceCase): void {
  if (surfaceCase.selector_matches !== 1) {
    throw new Error(
      `selector for ${action.surface_id} resolved to ${surfaceCase.selector_matches} elements`,
    );
  }

  for (const variant of REQUIRED_VARIANTS) {
    if (!surfaceCase.variants.includes(variant)) {
      throw new Error(`missing ${variant} variant for ${action.surface_id}`);
    }
  }

  if (action.risk === "destructive" || action.risk === "external") {
    const ownership = surfaceCase.fixture_ownership;
    const hasOwnership = Boolean(
      ownership?.token_ref &&
        (ownership.owned_paths.length > 0 || ownership.loopback_endpoints.length > 0),
    );
    if (!hasOwnership || !action.cleanup) {
      throw new Error(
        `${action.risk} action ${action.surface_id} requires fixture ownership and cleanup`,
      );
    }
    if (
      action.expected_ipc.length === 0 ||
      action.expected_ipc.some((ipc) => Object.keys(ipc.args_match).length === 0)
    ) {
      throw new Error(
        `${action.risk} action ${action.surface_id} requires expected IPC arguments`,
      );
    }
  }
}

function parseSurfaceMapFile(rawFile: unknown, fileIndex: number): SurfaceMapFile {
  const file = object(rawFile, `surface map file ${fileIndex}`);
  const name = string(file.file, `surface map file ${fileIndex} file`);
  const inventory = array(file.inventory, `surface map file ${name} inventory`).map(
    parseAction,
  );
  const cases = array(file.cases, `surface map file ${name} cases`).map(parseCase);

  return { file: name, inventory, cases };
}

function parseAction(rawAction: unknown): SurfaceAction {
  const action = object(rawAction, "action");
  const surfaceId = string(action.surface_id, "action surface_id");
  const label = `action ${surfaceId}`;

  if (action.schema !== 1) {
    throw new Error(`${label} has unsupported schema: ${String(action.schema)}`);
  }
  const risk = enumValue(action.risk, RISKS, `${label} has invalid risk`);
  const selector = object(action.selector, `${label} selector`);
  const selectorStrategy = enumValue(
    selector.strategy,
    SELECTOR_STRATEGIES,
    `${label} has invalid selector strategy`,
  );
  const expectedIpc = array(action.expected_ipc, `${label} has invalid expected_ipc`).map(
    (rawIpc) => {
      const ipc = object(rawIpc, `${label} has invalid expected_ipc entry`);
      return {
        command: string(ipc.command, `${label} has invalid expected_ipc command`),
        args_match: object(
          ipc.args_match,
          `${label} has invalid expected_ipc args_match`,
        ),
      };
    },
  );
  const oracles = array(action.oracles, `${label} has invalid oracles`).map((oracle) =>
    enumValue(oracle, ORACLES, `${label} has invalid oracle`),
  );
  const platform = array(action.platform, `${label} has invalid platform`).map((entry) =>
    enumValue(entry, PLATFORMS, `${label} has invalid platform`),
  );

  return {
    schema: 1,
    app_commit: string(action.app_commit, `${label} app_commit`),
    surface_id: surfaceId,
    state_id: string(action.state_id, `${label} state_id`),
    element_id: string(action.element_id, `${label} element_id`),
    selector: {
      strategy: selectorStrategy,
      value: string(selector.value, `${label} selector value`),
    },
    visible_name: string(action.visible_name, `${label} visible_name`),
    preconditions: array(action.preconditions, `${label} preconditions`).map((entry) =>
      string(entry, `${label} precondition`),
    ),
    risk,
    expected_ipc: expectedIpc,
    oracles,
    cleanup: nullableString(action.cleanup, `${label} cleanup`),
    platform,
  };
}

function parseCase(rawCase: unknown): SurfaceCase {
  const surfaceCase = object(rawCase, "surface case");
  const actionId = string(surfaceCase.action_id, "surface case action_id");
  const label = `case for ${actionId}`;
  const variants = array(surfaceCase.variants, `${label} variants`).map((variant) =>
    enumValue(variant, new Set(REQUIRED_VARIANTS), `${label} has invalid variant`),
  );
  const selectorMatches = surfaceCase.selector_matches;
  if (!Number.isInteger(selectorMatches) || (selectorMatches as number) < 0) {
    throw new Error(`${label} has invalid selector_matches`);
  }

  const ownership = surfaceCase.fixture_ownership;
  return {
    action_id: actionId,
    state_id: string(surfaceCase.state_id, `${label} state_id`),
    selector_matches: selectorMatches as number,
    variants,
    fixture_ownership:
      ownership === undefined
        ? undefined
        : parseFixtureOwnership(ownership, label),
  };
}

function parseFixtureOwnership(
  rawOwnership: unknown,
  label: string,
): NonNullable<SurfaceCase["fixture_ownership"]> {
  const ownership = object(rawOwnership, `${label} has invalid fixture ownership`);
  const endpoints = array(
    ownership.loopback_endpoints,
    `${label} has invalid fixture ownership endpoints`,
  ).map((endpoint) => {
    const value = string(endpoint, `${label} has invalid fixture ownership endpoint`);
    if (!isLoopbackEndpoint(value)) {
      throw new Error(`${label} has non-loopback fixture ownership endpoint`);
    }
    return value;
  });

  return {
    token_ref: string(ownership.token_ref, `${label} has invalid fixture ownership token`),
    owned_paths: array(
      ownership.owned_paths,
      `${label} has invalid fixture ownership paths`,
    ).map((path) => string(path, `${label} has invalid fixture ownership path`)),
    loopback_endpoints: endpoints,
  };
}

function object(value: unknown, message: string): Record<string, unknown> {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new Error(message);
  }
  return value as Record<string, unknown>;
}

function array(value: unknown, message: string): unknown[] {
  if (!Array.isArray(value)) throw new Error(message);
  return value;
}

function string(value: unknown, message: string): string {
  if (typeof value !== "string" || !value.trim()) throw new Error(message);
  return value;
}

function nullableString(value: unknown, message: string): string | null {
  if (value === null) return null;
  return string(value, message);
}

function enumValue<T extends string>(
  value: unknown,
  values: ReadonlySet<T>,
  message: string,
): T {
  if (typeof value !== "string" || !values.has(value as T)) throw new Error(message);
  return value as T;
}

function isLoopbackEndpoint(value: string): boolean {
  try {
    const endpoint = new URL(value);
    return (
      ["http:", "https:", "lore:"].includes(endpoint.protocol) &&
      ["127.0.0.1", "localhost", "[::1]"].includes(endpoint.hostname)
    );
  } catch {
    return false;
  }
}
