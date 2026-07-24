import type { DocumentProjection, StudioDiagnostic } from "./protocol";
import type {
  RunCancellation,
  RunOutcome,
  RunPlan,
  RunProgress,
  RunResult,
} from "./reference-run-protocol";
import type { RunConfiguration, ValidatedRunConfiguration } from "./run-configuration";
import type { ValueEditEvidence, ValueEditPlan, ValueEditResult } from "./value-edit-protocol";

export type Point = Readonly<{ x: number; y: number }>;
export type NodeLayout = Readonly<Record<string, Point>>;

export type CompileStatus =
  | Readonly<{ kind: "idle" }>
  | Readonly<{ kind: "compiling"; requestId: number }>
  | Readonly<{ kind: "ready" }>
  | Readonly<{ kind: "failed" }>;

export type RunStatus =
  | Readonly<{ kind: "idle" }>
  | Readonly<{
      kind: "running";
      requestId: number;
      runId: string;
      digest: string;
      configuration: ValidatedRunConfiguration;
      progress: RunProgress | null;
    }>
  | Readonly<{
      kind: "cancelling";
      requestId: number;
      runId: string;
      digest: string;
      configuration: ValidatedRunConfiguration;
      progress: RunProgress | null;
    }>
  | Readonly<{
      kind: "complete";
      runId: string;
      digest: string;
      configuration: ValidatedRunConfiguration;
    }>
  | Readonly<{
      kind: "cancelled";
      runId: string;
      digest: string;
      configuration: ValidatedRunConfiguration;
      cancellation: RunCancellation;
    }>
  | Readonly<{ kind: "failed" }>;

export type CompletedRun = Readonly<{
  result: RunResult;
  configuration: ValidatedRunConfiguration;
}>;

export type RunPlanStatus =
  | Readonly<{ kind: "idle" }>
  | Readonly<{ kind: "previewing"; requestId: number }>
  | Readonly<{
      kind: "ready";
      plan: RunPlan;
      configuration: ValidatedRunConfiguration;
      digest: string;
    }>
  | Readonly<{ kind: "failed" }>;

export type CommandPaletteState =
  | Readonly<{ kind: "closed" }>
  | Readonly<{ kind: "open"; query: string }>;

export type ValueEditStatus =
  | Readonly<{ kind: "idle" }>
  | Readonly<{ kind: "previewing"; requestId: number }>
  | Readonly<{
      kind: "ready";
      plan: ValueEditPlan;
      input: string;
      digest: string;
      targetId: string;
    }>
  | Readonly<{ kind: "committing"; requestId: number; plan: ValueEditPlan }>
  | Readonly<{ kind: "failed" }>;

export type RevisionEntry = Readonly<{
  document: DocumentProjection;
  editEvidence: ValueEditEvidence | null;
}>;

export const MAX_REVISION_LINEAGE = 24;

export interface StudioState {
  readonly source: string;
  readonly compiledSource: string | null;
  readonly sourceDigest: string | null;
  readonly document: DocumentProjection | null;
  readonly compileStatus: CompileStatus;
  readonly compileDiagnostics: readonly StudioDiagnostic[];
  readonly compileDiagnosticSource: string | null;
  readonly selectedNodeId: string | null;
  readonly layoutsByDigest: Readonly<Record<string, NodeLayout>>;
  readonly runConfiguration: RunConfiguration;
  readonly runPlanStatus: RunPlanStatus;
  readonly runStatus: RunStatus;
  readonly latestRun: CompletedRun | null;
  readonly runPlanDiagnostics: readonly StudioDiagnostic[];
  readonly runDiagnostics: readonly StudioDiagnostic[];
  readonly commandPalette: CommandPaletteState;
  readonly valueEditInput: string;
  readonly valueEditStatus: ValueEditStatus;
  readonly valueEditDiagnostics: readonly StudioDiagnostic[];
  readonly revisionLineage: readonly RevisionEntry[];
  readonly revisionIndex: number;
}

export type StudioAction =
  | Readonly<{ type: "source-edited"; source: string }>
  | Readonly<{ type: "compile-started"; requestId: number }>
  | Readonly<{
      type: "compile-finished";
      requestId: number;
      compiledSource: string;
      document: DocumentProjection | null;
      workspaceLayout: NodeLayout | null;
      diagnostics: readonly StudioDiagnostic[];
    }>
  | Readonly<{ type: "node-selected"; nodeId: string | null }>
  | Readonly<{ type: "node-moved"; nodeId: string; position: Point }>
  | Readonly<{ type: "layout-reset" }>
  | Readonly<{ type: "value-edit-input-edited"; value: string }>
  | Readonly<{ type: "value-edit-preview-started"; requestId: number }>
  | Readonly<{
      type: "value-edit-preview-finished";
      requestId: number;
      digest: string;
      targetId: string;
      input: string;
      plan: ValueEditPlan | null;
      diagnostics: readonly StudioDiagnostic[];
    }>
  | Readonly<{ type: "value-edit-commit-started"; requestId: number; plan: ValueEditPlan }>
  | Readonly<{
      type: "value-edit-commit-finished";
      requestId: number;
      result: ValueEditResult | null;
      diagnostics: readonly StudioDiagnostic[];
    }>
  | Readonly<{ type: "revision-undo" }>
  | Readonly<{ type: "revision-redo" }>
  | Readonly<{
      type: "run-input-edited";
      field: keyof RunConfiguration;
      value: string;
    }>
  | Readonly<{ type: "run-preview-started"; requestId: number }>
  | Readonly<{
      type: "run-preview-finished";
      requestId: number;
      digest: string;
      configuration: ValidatedRunConfiguration;
      plan: RunPlan | null;
      diagnostics: readonly StudioDiagnostic[];
    }>
  | Readonly<{
      type: "run-started";
      requestId: number;
      runId: string;
      digest: string;
      configuration: ValidatedRunConfiguration;
    }>
  | Readonly<{ type: "run-progressed"; requestId: number; progress: RunProgress }>
  | Readonly<{ type: "run-cancel-requested"; requestId: number }>
  | Readonly<{
      type: "run-cancel-failed";
      requestId: number;
      diagnostics: readonly StudioDiagnostic[];
    }>
  | Readonly<{
      type: "run-finished";
      requestId: number;
      outcome: RunOutcome | null;
      diagnostics: readonly StudioDiagnostic[];
    }>
  | Readonly<{ type: "command-palette-opened" }>
  | Readonly<{ type: "command-palette-closed" }>
  | Readonly<{ type: "command-query-edited"; query: string }>;

const KIND_COLUMNS = new Map([
  ["activation", 0],
  ["connection", 0],
  ["relation", 1],
  ["field", 2],
  ["parameter", 2],
  ["port", 2],
  ["domain", 3],
  ["representation", 3],
  ["clock-domain", 3],
]);

export function defaultLayout(document: DocumentProjection): NodeLayout {
  const rows = [0, 0, 0, 0];
  return Object.fromEntries(
    document.nodes.map((node) => {
      const column = KIND_COLUMNS.get(node.kind) ?? 0;
      const row = rows[column] ?? 0;
      rows[column] = row + 1;
      return [node.id, { x: 48 + column * 280, y: 50 + row * 138 }];
    }),
  );
}

export function initialStudioState(source: string): StudioState {
  return {
    source,
    compiledSource: null,
    sourceDigest: null,
    document: null,
    compileStatus: { kind: "idle" },
    compileDiagnostics: [],
    compileDiagnosticSource: null,
    selectedNodeId: null,
    layoutsByDigest: {},
    runConfiguration: { endTime: "4", maxStep: "0.1" },
    runPlanStatus: { kind: "idle" },
    runStatus: { kind: "idle" },
    latestRun: null,
    runPlanDiagnostics: [],
    runDiagnostics: [],
    commandPalette: { kind: "closed" },
    valueEditInput: "",
    valueEditStatus: { kind: "idle" },
    valueEditDiagnostics: [],
    revisionLineage: [],
    revisionIndex: -1,
  };
}

export function currentLayout(state: StudioState): NodeLayout {
  if (state.document === null) {
    return {};
  }
  return state.layoutsByDigest[state.document.digest] ?? defaultLayout(state.document);
}

function reconciledLayout(document: DocumentProjection, saved: NodeLayout | null): NodeLayout {
  const fallback = defaultLayout(document);
  return Object.fromEntries(
    document.nodes.map((node) => [
      node.id,
      saved?.[node.id] ?? fallback[node.id] ?? { x: 0, y: 0 },
    ]),
  );
}

function nodeValueInput(document: DocumentProjection, nodeId: string | null): string {
  const value = document.nodes.find((node) => node.id === nodeId)?.value;
  return value === null || value === undefined ? "" : value.toString();
}

function moveToRevision(state: StudioState, index: number): StudioState {
  const entry = state.revisionLineage[index];
  if (
    entry === undefined ||
    state.runStatus.kind === "running" ||
    state.runStatus.kind === "cancelling" ||
    state.valueEditStatus.kind === "committing"
  ) {
    return state;
  }
  const selectedNodeId = entry.document.nodes.some((node) => node.id === state.selectedNodeId)
    ? state.selectedNodeId
    : (entry.document.nodes[0]?.id ?? null);
  return {
    ...state,
    document: entry.document,
    revisionIndex: index,
    selectedNodeId,
    valueEditInput: nodeValueInput(entry.document, selectedNodeId),
    valueEditStatus: { kind: "idle" },
    valueEditDiagnostics: [],
    runPlanStatus: { kind: "idle" },
    runPlanDiagnostics: [],
  };
}

export function studioReducer(state: StudioState, action: StudioAction): StudioState {
  switch (action.type) {
    case "source-edited":
      return { ...state, source: action.source };
    case "compile-started":
      return {
        ...state,
        compileStatus: { kind: "compiling", requestId: action.requestId },
        compileDiagnostics: [],
      };
    case "compile-finished": {
      if (
        state.compileStatus.kind !== "compiling" ||
        state.compileStatus.requestId !== action.requestId
      ) {
        return state;
      }
      if (action.document === null) {
        return {
          ...state,
          compileStatus: { kind: "failed" },
          compileDiagnostics: action.diagnostics,
          compileDiagnosticSource: action.compiledSource,
        };
      }
      const layout =
        state.layoutsByDigest[action.document.digest] ??
        reconciledLayout(action.document, action.workspaceLayout);
      return {
        ...state,
        compiledSource: action.compiledSource,
        sourceDigest: action.document.digest,
        document: action.document,
        compileStatus: { kind: "ready" },
        compileDiagnostics: action.diagnostics,
        compileDiagnosticSource: action.compiledSource,
        selectedNodeId: action.document.nodes[0]?.id ?? null,
        layoutsByDigest: {
          ...state.layoutsByDigest,
          [action.document.digest]: layout,
        },
        runPlanStatus: { kind: "idle" },
        runStatus: { kind: "idle" },
        latestRun: null,
        runPlanDiagnostics: [],
        runDiagnostics: [],
        valueEditInput: nodeValueInput(action.document, action.document.nodes[0]?.id ?? null),
        valueEditStatus: { kind: "idle" },
        valueEditDiagnostics: [],
        revisionLineage: [{ document: action.document, editEvidence: null }],
        revisionIndex: 0,
      };
    }
    case "node-selected":
      return {
        ...state,
        selectedNodeId: action.nodeId,
        valueEditInput:
          state.document === null ? "" : nodeValueInput(state.document, action.nodeId),
        valueEditStatus: { kind: "idle" },
        valueEditDiagnostics: [],
      };
    case "node-moved": {
      if (state.document === null) {
        return state;
      }
      const digest = state.document.digest;
      return {
        ...state,
        layoutsByDigest: {
          ...state.layoutsByDigest,
          [digest]: {
            ...currentLayout(state),
            [action.nodeId]: action.position,
          },
        },
      };
    }
    case "layout-reset": {
      if (state.document === null) {
        return state;
      }
      return {
        ...state,
        layoutsByDigest: {
          ...state.layoutsByDigest,
          [state.document.digest]: defaultLayout(state.document),
        },
      };
    }
    case "value-edit-input-edited":
      return {
        ...state,
        valueEditInput: action.value,
        valueEditStatus: { kind: "idle" },
        valueEditDiagnostics: [],
      };
    case "value-edit-preview-started":
      return {
        ...state,
        valueEditStatus: { kind: "previewing", requestId: action.requestId },
        valueEditDiagnostics: [],
      };
    case "value-edit-preview-finished":
      if (
        state.valueEditStatus.kind !== "previewing" ||
        state.valueEditStatus.requestId !== action.requestId ||
        state.document?.digest !== action.digest ||
        state.selectedNodeId !== action.targetId ||
        state.valueEditInput !== action.input
      ) {
        return state;
      }
      return {
        ...state,
        valueEditStatus:
          action.plan === null
            ? { kind: "failed" }
            : {
                kind: "ready",
                plan: action.plan,
                input: action.input,
                digest: action.digest,
                targetId: action.targetId,
              },
        valueEditDiagnostics: action.diagnostics,
      };
    case "value-edit-commit-started":
      return {
        ...state,
        valueEditStatus: {
          kind: "committing",
          requestId: action.requestId,
          plan: action.plan,
        },
        valueEditDiagnostics: [],
      };
    case "value-edit-commit-finished": {
      if (
        state.valueEditStatus.kind !== "committing" ||
        state.valueEditStatus.requestId !== action.requestId
      ) {
        return state;
      }
      if (action.result === null) {
        return {
          ...state,
          valueEditStatus: { kind: "failed" },
          valueEditDiagnostics: action.diagnostics,
        };
      }
      if (state.document?.digest !== action.result.evidence.plan.baseDigest) return state;
      const document = action.result.document;
      let lineage = [
        ...state.revisionLineage.slice(0, state.revisionIndex + 1),
        { document, editEvidence: action.result.evidence },
      ];
      if (lineage.length > MAX_REVISION_LINEAGE) {
        lineage = lineage.slice(lineage.length - MAX_REVISION_LINEAGE);
      }
      const selectedNodeId = document.nodes.some((node) => node.id === state.selectedNodeId)
        ? state.selectedNodeId
        : (document.nodes[0]?.id ?? null);
      const layout =
        state.layoutsByDigest[document.digest] ?? reconciledLayout(document, currentLayout(state));
      return {
        ...state,
        document,
        selectedNodeId,
        layoutsByDigest: { ...state.layoutsByDigest, [document.digest]: layout },
        valueEditInput: nodeValueInput(document, selectedNodeId),
        valueEditStatus: { kind: "idle" },
        valueEditDiagnostics: action.diagnostics,
        revisionLineage: lineage,
        revisionIndex: lineage.length - 1,
        runPlanStatus: { kind: "idle" },
        runPlanDiagnostics: [],
      };
    }
    case "revision-undo":
      return moveToRevision(state, state.revisionIndex - 1);
    case "revision-redo":
      return moveToRevision(state, state.revisionIndex + 1);
    case "run-input-edited":
      return {
        ...state,
        runConfiguration: {
          ...state.runConfiguration,
          [action.field]: action.value,
        },
        runPlanStatus: { kind: "idle" },
        runPlanDiagnostics: [],
      };
    case "run-preview-started":
      return {
        ...state,
        runPlanStatus: { kind: "previewing", requestId: action.requestId },
        runPlanDiagnostics: [],
      };
    case "run-preview-finished":
      if (
        state.runPlanStatus.kind !== "previewing" ||
        state.runPlanStatus.requestId !== action.requestId
      ) {
        return state;
      }
      return {
        ...state,
        runPlanStatus:
          action.plan === null
            ? { kind: "failed" }
            : {
                kind: "ready",
                plan: action.plan,
                configuration: action.configuration,
                digest: action.digest,
              },
        runPlanDiagnostics: action.diagnostics,
      };
    case "run-started":
      return {
        ...state,
        runStatus: {
          kind: "running",
          requestId: action.requestId,
          runId: action.runId,
          digest: action.digest,
          configuration: action.configuration,
          progress: null,
        },
        runDiagnostics: [],
      };
    case "run-progressed": {
      if (
        (state.runStatus.kind !== "running" && state.runStatus.kind !== "cancelling") ||
        state.runStatus.requestId !== action.requestId ||
        state.runStatus.runId !== action.progress.runId ||
        state.runStatus.configuration.endTime !== action.progress.endTime
      ) {
        return state;
      }
      const previous = state.runStatus.progress;
      if (
        previous !== null &&
        (action.progress.acceptedSteps < previous.acceptedSteps ||
          action.progress.modelTime < previous.modelTime)
      ) {
        return state;
      }
      return {
        ...state,
        runStatus: { ...state.runStatus, progress: action.progress },
      };
    }
    case "run-cancel-requested":
      if (state.runStatus.kind !== "running" || state.runStatus.requestId !== action.requestId) {
        return state;
      }
      return {
        ...state,
        runStatus: { ...state.runStatus, kind: "cancelling" },
      };
    case "run-cancel-failed":
      if (state.runStatus.kind !== "cancelling" || state.runStatus.requestId !== action.requestId) {
        return state;
      }
      return {
        ...state,
        runStatus: { ...state.runStatus, kind: "running" },
        runDiagnostics: action.diagnostics,
      };
    case "run-finished":
      if (
        (state.runStatus.kind !== "running" && state.runStatus.kind !== "cancelling") ||
        state.runStatus.requestId !== action.requestId
      ) {
        return state;
      }
      if (action.outcome?.kind === "completed") {
        if (action.outcome.result.digest !== state.runStatus.digest) return state;
        return {
          ...state,
          runStatus: {
            kind: "complete",
            runId: state.runStatus.runId,
            digest: state.runStatus.digest,
            configuration: state.runStatus.configuration,
          },
          latestRun: {
            result: action.outcome.result,
            configuration: state.runStatus.configuration,
          },
          runDiagnostics: action.diagnostics,
        };
      }
      if (action.outcome?.kind === "cancelled") {
        if (action.outcome.cancellation.runId !== state.runStatus.runId) return state;
        return {
          ...state,
          runStatus: {
            kind: "cancelled",
            runId: state.runStatus.runId,
            digest: state.runStatus.digest,
            configuration: state.runStatus.configuration,
            cancellation: action.outcome.cancellation,
          },
          runDiagnostics: action.diagnostics,
        };
      }
      return {
        ...state,
        runStatus: { kind: "failed" },
        runDiagnostics: action.diagnostics,
      };
    case "command-palette-opened":
      return { ...state, commandPalette: { kind: "open", query: "" } };
    case "command-palette-closed":
      return state.commandPalette.kind === "closed"
        ? state
        : { ...state, commandPalette: { kind: "closed" } };
    case "command-query-edited":
      return state.commandPalette.kind === "open"
        ? { ...state, commandPalette: { kind: "open", query: action.query } }
        : state;
  }
}
