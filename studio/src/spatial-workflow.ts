import type { StudioDiagnostic } from "./protocol";
import {
  MAX_SPATIAL_ENTITY_COUNT,
  type SpatialRunPlan,
  type SpatialRunResult,
} from "./spatial-protocol";

export type SpatialMethod = "finite-element" | "finite-volume";

export type SpatialConfiguration = Readonly<{
  method: SpatialMethod;
  cellsPerAxis: string;
  workers: string;
}>;

export type ValidatedSpatialConfiguration = Readonly<{
  method: SpatialMethod;
  cellsPerAxis: number;
  workers: number;
  cellCount: number;
  fieldValueCount: number;
}>;

export type SpatialConfigurationValidation = Readonly<{
  value: ValidatedSpatialConfiguration | null;
  errors: Readonly<{
    cellsPerAxis: string | null;
    workers: string | null;
  }>;
}>;

export type SpatialPlanStatus =
  | Readonly<{ kind: "idle" }>
  | Readonly<{ kind: "previewing"; requestId: number }>
  | Readonly<{
      kind: "ready";
      plan: SpatialRunPlan;
      digest: string;
      realizationRevision: number;
      configuration: ValidatedSpatialConfiguration;
    }>
  | Readonly<{ kind: "failed" }>;

export type SpatialRunStatus =
  | Readonly<{ kind: "idle" }>
  | Readonly<{
      kind: "running";
      requestId: number;
      runId: string;
      digest: string;
      realizationRevision: number;
      configuration: ValidatedSpatialConfiguration;
    }>
  | Readonly<{
      kind: "complete";
      runId: string;
      digest: string;
      realizationRevision: number;
      configuration: ValidatedSpatialConfiguration;
    }>
  | Readonly<{ kind: "failed" }>;

export interface SpatialWorkflowState {
  readonly contextDigest: string | null;
  readonly configuration: SpatialConfiguration;
  readonly realizationRevision: number;
  readonly planStatus: SpatialPlanStatus;
  readonly runStatus: SpatialRunStatus;
  readonly latestResult: SpatialRunResult | null;
  readonly planDiagnostics: readonly StudioDiagnostic[];
  readonly runDiagnostics: readonly StudioDiagnostic[];
}

export type SpatialWorkflowAction =
  | Readonly<{ type: "context-changed"; digest: string | null }>
  | Readonly<{ type: "input-edited"; field: "method"; value: SpatialMethod }>
  | Readonly<{
      type: "input-edited";
      field: "cellsPerAxis" | "workers";
      value: string;
    }>
  | Readonly<{ type: "preview-started"; requestId: number }>
  | Readonly<{
      type: "preview-finished";
      requestId: number;
      digest: string;
      realizationRevision: number;
      configuration: ValidatedSpatialConfiguration;
      plan: SpatialRunPlan | null;
      diagnostics: readonly StudioDiagnostic[];
    }>
  | Readonly<{
      type: "run-started";
      requestId: number;
      runId: string;
      digest: string;
      realizationRevision: number;
      configuration: ValidatedSpatialConfiguration;
    }>
  | Readonly<{
      type: "run-finished";
      requestId: number;
      result: SpatialRunResult | null;
      diagnostics: readonly StudioDiagnostic[];
    }>;

export function initialSpatialWorkflowState(): SpatialWorkflowState {
  return {
    contextDigest: null,
    configuration: { method: "finite-element", cellsPerAxis: "16", workers: "1" },
    realizationRevision: 1,
    planStatus: { kind: "idle" },
    runStatus: { kind: "idle" },
    latestResult: null,
    planDiagnostics: [],
    runDiagnostics: [],
  };
}

function positiveInteger(value: string): number | null {
  if (!/^[1-9][0-9]*$/.test(value)) return null;
  const parsed = Number(value);
  return Number.isSafeInteger(parsed) ? parsed : null;
}

function checkedPower(base: number, exponent: number): number | null {
  let value = 1;
  for (let index = 0; index < exponent; index += 1) {
    value *= base;
    if (!Number.isSafeInteger(value) || value > MAX_SPATIAL_ENTITY_COUNT) return null;
  }
  return value;
}

export function validateSpatialConfiguration(
  configuration: SpatialConfiguration,
  spatialDimension: number,
  maximumHostWorkers: number,
): SpatialConfigurationValidation {
  const cellsPerAxis = positiveInteger(configuration.cellsPerAxis);
  const workers = positiveInteger(configuration.workers);
  let cellsError =
    cellsPerAxis === null ? "Enter a positive whole number of cells per axis." : null;
  const workersError =
    workers === null
      ? "Enter a positive whole number of workers."
      : workers > maximumHostWorkers
        ? `This Studio session admits at most ${maximumHostWorkers} workers.`
        : null;

  let cellCount: number | null = null;
  let fieldValueCount: number | null = null;
  if (cellsPerAxis !== null) {
    cellCount = checkedPower(cellsPerAxis, spatialDimension);
    const fieldAxis = configuration.method === "finite-element" ? cellsPerAxis + 1 : cellsPerAxis;
    fieldValueCount = checkedPower(fieldAxis, spatialDimension);
    if (cellCount === null || fieldValueCount === null) {
      cellsError = `The requested mesh exceeds the ${MAX_SPATIAL_ENTITY_COUNT.toLocaleString()}-entity Studio boundary.`;
    }
  }

  if (
    cellsPerAxis === null ||
    workers === null ||
    cellsError !== null ||
    workersError !== null ||
    cellCount === null ||
    fieldValueCount === null
  ) {
    return { value: null, errors: { cellsPerAxis: cellsError, workers: workersError } };
  }
  return {
    value: {
      method: configuration.method,
      cellsPerAxis,
      workers,
      cellCount,
      fieldValueCount,
    },
    errors: { cellsPerAxis: null, workers: null },
  };
}

function sameConfiguration(
  left: ValidatedSpatialConfiguration,
  right: ValidatedSpatialConfiguration,
): boolean {
  return (
    left.method === right.method &&
    left.cellsPerAxis === right.cellsPerAxis &&
    left.workers === right.workers
  );
}

export function spatialWorkflowReducer(
  state: SpatialWorkflowState,
  action: SpatialWorkflowAction,
): SpatialWorkflowState {
  switch (action.type) {
    case "context-changed":
      if (state.contextDigest === action.digest) return state;
      return {
        ...initialSpatialWorkflowState(),
        contextDigest: action.digest,
      };
    case "input-edited":
      if (state.configuration[action.field] === action.value) return state;
      return {
        ...state,
        configuration: { ...state.configuration, [action.field]: action.value },
        realizationRevision: state.realizationRevision + 1,
        planStatus: { kind: "idle" },
        planDiagnostics: [],
      };
    case "preview-started":
      return {
        ...state,
        planStatus: { kind: "previewing", requestId: action.requestId },
        planDiagnostics: [],
      };
    case "preview-finished":
      if (
        state.planStatus.kind !== "previewing" ||
        state.planStatus.requestId !== action.requestId ||
        state.contextDigest !== action.digest ||
        state.realizationRevision !== action.realizationRevision
      ) {
        return state;
      }
      return {
        ...state,
        planStatus:
          action.plan === null
            ? { kind: "failed" }
            : {
                kind: "ready",
                plan: action.plan,
                digest: action.digest,
                realizationRevision: action.realizationRevision,
                configuration: action.configuration,
              },
        planDiagnostics: action.diagnostics,
      };
    case "run-started":
      return {
        ...state,
        runStatus: {
          kind: "running",
          requestId: action.requestId,
          runId: action.runId,
          digest: action.digest,
          realizationRevision: action.realizationRevision,
          configuration: action.configuration,
        },
        runDiagnostics: [],
      };
    case "run-finished":
      if (state.runStatus.kind !== "running" || state.runStatus.requestId !== action.requestId) {
        return state;
      }
      if (
        action.result === null ||
        action.result.runId !== state.runStatus.runId ||
        action.result.digest !== state.runStatus.digest ||
        action.result.plan.realizationRevision !== state.runStatus.realizationRevision ||
        !sameConfiguration(state.runStatus.configuration, {
          method: action.result.plan.discretization.method,
          cellsPerAxis: action.result.plan.discretization.cellsPerAxis,
          workers: action.result.plan.placement.workers,
          cellCount: action.result.plan.discretization.cellCount,
          fieldValueCount: action.result.plan.discretization.fieldValueCount,
        })
      ) {
        return {
          ...state,
          runStatus: { kind: "failed" },
          runDiagnostics: action.diagnostics,
        };
      }
      return {
        ...state,
        runStatus: {
          kind: "complete",
          runId: state.runStatus.runId,
          digest: state.runStatus.digest,
          realizationRevision: state.runStatus.realizationRevision,
          configuration: state.runStatus.configuration,
        },
        latestResult: action.result,
        runDiagnostics: action.diagnostics,
      };
  }
}

export function spatialPlanIsCurrent(
  state: SpatialWorkflowState,
  digest: string,
  configuration: ValidatedSpatialConfiguration,
): boolean {
  return (
    state.planStatus.kind === "ready" &&
    state.planStatus.digest === digest &&
    state.planStatus.realizationRevision === state.realizationRevision &&
    sameConfiguration(state.planStatus.configuration, configuration)
  );
}
