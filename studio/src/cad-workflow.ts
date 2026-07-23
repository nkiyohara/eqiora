import {
  CAD_VIEW_PROTOCOL,
  type CadProjection,
  type CadSelectionRequest,
  type CadSelectionResult,
  type CadSemanticEntity,
} from "./cad-protocol";

type CadSelectionContext = Readonly<{
  modelDigest: string;
  planKey: string;
  geometryDigest: string;
}>;

export interface CadSelectionState {
  readonly context: CadSelectionContext | null;
  readonly accepted: CadSelectionRequest | null;
  readonly status:
    | Readonly<{ kind: "idle" }>
    | Readonly<{ kind: "resolving"; requestId: number; request: CadSelectionRequest }>
    | Readonly<{ kind: "failed" }>;
}

export type CadSelectionAction =
  | Readonly<{ type: "context-changed"; projection: CadProjection | null }>
  | Readonly<{ type: "selection-started"; requestId: number; request: CadSelectionRequest }>
  | Readonly<{
      type: "selection-finished";
      requestId: number;
      result: CadSelectionResult | null;
    }>;

export function initialCadSelectionState(): CadSelectionState {
  return { context: null, accepted: null, status: { kind: "idle" } };
}

function contextOf(projection: CadProjection): CadSelectionContext {
  return {
    modelDigest: projection.modelDigest,
    planKey: projection.planKey,
    geometryDigest: projection.geometryDigest,
  };
}

function requestMatchesContext(
  request: CadSelectionRequest,
  context: CadSelectionContext | null,
): boolean {
  return (
    context !== null &&
    request.modelDigest === context.modelDigest &&
    request.planKey === context.planKey &&
    request.geometryDigest === context.geometryDigest
  );
}

/**
 * Admit native selection only when its complete asynchronous lineage still
 * matches the active Model, plan, Geometry revision, and requested Domain.
 */
export function cadSelectionReducer(
  state: CadSelectionState,
  action: CadSelectionAction,
): CadSelectionState {
  switch (action.type) {
    case "context-changed": {
      const context = action.projection === null ? null : contextOf(action.projection);
      if (
        context !== null &&
        state.context?.modelDigest === context.modelDigest &&
        state.context.planKey === context.planKey &&
        state.context.geometryDigest === context.geometryDigest
      ) {
        return state;
      }
      return { context, accepted: null, status: { kind: "idle" } };
    }
    case "selection-started":
      if (!requestMatchesContext(action.request, state.context)) return state;
      return {
        ...state,
        status: { kind: "resolving", requestId: action.requestId, request: action.request },
      };
    case "selection-finished": {
      if (state.status.kind !== "resolving" || state.status.requestId !== action.requestId) {
        return state;
      }
      const request = state.status.request;
      const result = action.result;
      if (
        result === null ||
        !requestMatchesContext(request, state.context) ||
        result.modelDigest !== request.modelDigest ||
        result.planKey !== request.planKey ||
        result.geometryDigest !== request.geometryDigest ||
        result.domainId !== request.domainId ||
        result.entity.domainId !== request.domainId
      ) {
        return { ...state, status: { kind: "failed" } };
      }
      return { ...state, accepted: request, status: { kind: "idle" } };
    }
  }
}

export type CadSelectionResolution =
  | Readonly<{ kind: "none" }>
  | Readonly<{ kind: "stale" }>
  | Readonly<{ kind: "missing" }>
  | Readonly<{ kind: "selected"; entity: CadSemanticEntity }>;

/**
 * Create an exact semantic request from either the table or the viewport.
 * Renderer primitive ranks and geometry-local entity indices cannot enter it.
 */
export function cadSelectionRequest(
  projection: CadProjection,
  domain: string,
): CadSelectionRequest | null {
  if (!projection.entities.some((entity) => entity.domainId === domain)) return null;
  return {
    protocol: CAD_VIEW_PROTOCOL,
    modelDigest: projection.modelDigest,
    planKey: projection.planKey,
    geometryDigest: projection.geometryDigest,
    domainId: domain,
  };
}

/** Resolve accepted application selection without trusting presentation state. */
export function resolveCadSelection(
  projection: CadProjection,
  selection: CadSelectionRequest | null,
): CadSelectionResolution {
  if (selection === null) return { kind: "none" };
  if (selection.geometryDigest !== projection.geometryDigest) return { kind: "stale" };
  if (
    selection.modelDigest !== projection.modelDigest ||
    selection.planKey !== projection.planKey
  ) {
    return { kind: "stale" };
  }
  const entity = projection.entities.find((candidate) => candidate.domainId === selection.domainId);
  return entity === undefined ? { kind: "missing" } : { kind: "selected", entity };
}

export function cadEntityLabel(entity: CadSemanticEntity): string {
  if (entity.name !== null) return entity.name;
  if (entity.axis !== null && entity.side !== null) {
    return `${["x", "y", "z"][entity.axis]} ${entity.side}`;
  }
  return entity.kind === "body" ? "Body" : "Boundary";
}

export function cadAxisSideLabel(entity: CadSemanticEntity): string | null {
  if (entity.axis === null || entity.side === null) return null;
  const axis = ["X", "Y", "Z"][entity.axis];
  return `${axis} ${entity.side} · parent-outward`;
}
