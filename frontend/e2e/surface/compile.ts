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

/**
 * Validates already-parsed surface-map files and builds immutable lookup
 * indexes. File parsing and fixture probing are deliberately outside this
 * function: the compiler only trusts the deterministic evidence it receives.
 */
export function compileSurfaceMap(
  files: readonly SurfaceMapFile[],
): CompiledSurfaceMap {
  const actionById = new Map<string, SurfaceAction>();
  const caseByActionId = new Map<string, SurfaceCase>();

  for (const file of files) {
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
  if (action.schema !== 1) {
    throw new Error(`action ${action.surface_id} has unsupported schema: ${action.schema}`);
  }
  if (!action.surface_id) {
    throw new Error("action id must not be empty");
  }
  if (!action.oracles.length) {
    throw new Error(`action ${action.surface_id} requires at least one oracle`);
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
  }
}
