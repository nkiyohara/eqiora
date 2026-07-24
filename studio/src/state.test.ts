import { describe, expect, it } from "vitest";
import { BRIDGE_PROTOCOL, type DocumentProjection } from "./protocol";
import type { RunPlan, RunProgress, RunResult } from "./reference-run-protocol";
import type { ValueEditPlan, ValueEditResult } from "./value-edit-protocol";
import { currentLayout, initialStudioState, studioReducer } from "./state";

const document: DocumentProjection = {
  protocol: BRIDGE_PROTOCOL,
  digest: "sha256:0123456789abcdef",
  revision: 1,
  modelId: "Model:01",
  nodes: [
    {
      id: "Field:01",
      name: "temperature",
      kind: "field",
      summary: "Scalar field",
      dimension: "Θ",
      value: 300,
    },
  ],
  edges: [],
  workflows: { scalarElliptic: null },
};

const plan: RunPlan = {
  protocol: BRIDGE_PROTOCOL,
  key: "eqiora.reference-plan/v1:0000000000000000:0000000000000000",
  adapter: { id: "eqiora.reference", version: "0.1.0" },
  placement: { kind: "host", workers: 1 },
  integration: { method: "backward-euler", endTime: 4, maxStep: 0.1 },
  nonlinear: {
    method: "dense-finite-difference-newton",
    absoluteTolerance: 1e-10,
    relativeTolerance: 1e-10,
    maximumIterations: 32,
  },
  events: {
    timeTolerance: 1e-10,
    guardTolerance: 1e-10,
    maximumLocalizationIterations: 80,
    maximumZeroTimeEvents: 64,
  },
  limits: { maximumSteps: 1_000_000 },
  acceptance: { kind: "semantic-oracle", independentVerifier: false },
};

const runId = "00000000-0000-4000-8000-000000000001";
const configuration = { endTime: 4, maxStep: 0.1 } as const;
const progress: RunProgress = {
  protocol: BRIDGE_PROTOCOL,
  runId,
  modelTime: 1,
  endTime: 4,
  acceptedSteps: 10,
  maximumSteps: 1_000_000,
  elapsedSeconds: 0.2,
};
const runResult: RunResult = {
  protocol: BRIDGE_PROTOCOL,
  digest: document.digest,
  evidence: { plan, elapsedSeconds: 0.5, fieldCount: 1, sampleCount: 2 },
  series: [
    {
      fieldId: "Field:01",
      name: "temperature",
      dimension: "Θ",
      time: [0, 4],
      values: [300, 301],
    },
  ],
};

const editPlan: ValueEditPlan = {
  protocol: BRIDGE_PROTOCOL,
  key: `eqiora.value-edit-plan/v1:${"a".repeat(64)}`,
  baseDigest: document.digest,
  baseRevision: 1,
  targetId: "Field:01",
  before: { value: 300, dimension: "Θ" },
  after: { value: 310, dimension: "Θ" },
  transactionDigest: "a".repeat(64),
};

const childDocument: DocumentProjection = {
  ...document,
  digest: "sha256:fedcba9876543210",
  revision: 2,
  nodes: document.nodes.map((node) => ({ ...node, value: 310 })),
};

const editResult: ValueEditResult = {
  protocol: BRIDGE_PROTOCOL,
  document: childDocument,
  evidence: {
    plan: editPlan,
    resultDigest: childDocument.digest,
    resultRevision: 2,
  },
};

describe("studio reducer boundaries", () => {
  it("ignores an obsolete compile response", () => {
    let state = initialStudioState("source A");
    state = studioReducer(state, { type: "compile-started", requestId: 2 });
    const next = studioReducer(state, {
      type: "compile-finished",
      requestId: 1,
      compiledSource: "source A",
      document,
      workspaceLayout: null,
      diagnostics: [],
    });
    expect(next).toBe(state);
  });

  it("keeps graph layout outside the canonical document", () => {
    let state = initialStudioState("source");
    state = studioReducer(state, { type: "compile-started", requestId: 1 });
    state = studioReducer(state, {
      type: "compile-finished",
      requestId: 1,
      compiledSource: "source",
      document,
      workspaceLayout: null,
      diagnostics: [],
    });
    const canonicalBefore = state.document;
    state = studioReducer(state, {
      type: "node-moved",
      nodeId: "Field:01",
      position: { x: 800, y: 240 },
    });
    expect(state.document).toBe(canonicalBefore);
    expect(currentLayout(state)["Field:01"]).toEqual({ x: 800, y: 240 });
  });

  it("reconciles saved workspace positions against canonical entity identity", () => {
    let state = initialStudioState("source");
    state = studioReducer(state, { type: "compile-started", requestId: 1 });
    state = studioReducer(state, {
      type: "compile-finished",
      requestId: 1,
      compiledSource: "source",
      document,
      workspaceLayout: {
        "Field:01": { x: 123, y: 456 },
        "Field:removed": { x: 9, y: 9 },
      },
      diagnostics: [],
    });
    expect(currentLayout(state)).toEqual({ "Field:01": { x: 123, y: 456 } });
  });

  it("retains the exact failed source for diagnostic provenance", () => {
    let state = initialStudioState("accepted source");
    state = studioReducer(state, { type: "compile-started", requestId: 1 });
    state = studioReducer(state, {
      type: "compile-finished",
      requestId: 1,
      compiledSource: "field ;",
      document: null,
      workspaceLayout: null,
      diagnostics: [],
    });
    expect(state.compileDiagnosticSource).toBe("field ;");
    expect(state.compileStatus.kind).toBe("failed");
  });

  it("keeps incomplete run input representable until validation", () => {
    const state = studioReducer(initialStudioState("source"), {
      type: "run-input-edited",
      field: "maxStep",
      value: "",
    });
    expect(state.runConfiguration.maxStep).toBe("");
  });

  it("accepts only the latest native capability preview", () => {
    let state = initialStudioState("source");
    state = studioReducer(state, { type: "run-preview-started", requestId: 2 });
    const obsolete = studioReducer(state, {
      type: "run-preview-finished",
      requestId: 1,
      digest: document.digest,
      configuration: { endTime: 4, maxStep: 0.1 },
      plan,
      diagnostics: [],
    });
    expect(obsolete).toBe(state);

    const ready = studioReducer(state, {
      type: "run-preview-finished",
      requestId: 2,
      digest: document.digest,
      configuration: { endTime: 4, maxStep: 0.1 },
      plan,
      diagnostics: [],
    });
    expect(ready.runPlanStatus).toEqual({
      kind: "ready",
      plan,
      configuration: { endTime: 4, maxStep: 0.1 },
      digest: document.digest,
    });
  });

  it("invalidates a preview when editable input changes", () => {
    let state = initialStudioState("source");
    state = studioReducer(state, { type: "run-preview-started", requestId: 1 });
    state = studioReducer(state, {
      type: "run-preview-finished",
      requestId: 1,
      digest: document.digest,
      configuration: { endTime: 4, maxStep: 0.1 },
      plan,
      diagnostics: [],
    });
    state = studioReducer(state, {
      type: "run-input-edited",
      field: "maxStep",
      value: "0.2",
    });
    expect(state.runPlanStatus).toEqual({ kind: "idle" });
    expect(state.runConfiguration.maxStep).toBe("0.2");
  });

  it("accepts only monotone progress for the exact active run", () => {
    let state = studioReducer(initialStudioState("source"), {
      type: "run-started",
      requestId: 7,
      runId,
      digest: document.digest,
      configuration,
    });
    state = studioReducer(state, { type: "run-progressed", requestId: 7, progress });
    expect(state.runStatus.kind).toBe("running");
    if (state.runStatus.kind !== "running") throw new Error("run must be active");
    expect(state.runStatus.progress).toBe(progress);

    const misrouted = studioReducer(state, {
      type: "run-progressed",
      requestId: 7,
      progress: { ...progress, runId: "00000000-0000-4000-8000-000000000002" },
    });
    expect(misrouted).toBe(state);
    const regressed = studioReducer(state, {
      type: "run-progressed",
      requestId: 7,
      progress: { ...progress, modelTime: 0.5, acceptedSteps: 5 },
    });
    expect(regressed).toBe(state);
  });

  it("keeps the last completed evidence visible through a cancelled successor", () => {
    let state = studioReducer(initialStudioState("source"), {
      type: "run-started",
      requestId: 1,
      runId,
      digest: document.digest,
      configuration,
    });
    state = studioReducer(state, {
      type: "run-finished",
      requestId: 1,
      outcome: { kind: "completed", result: runResult },
      diagnostics: [],
    });
    expect(state.latestRun?.result).toBe(runResult);
    expect(state.runStatus.kind).toBe("complete");

    const successorId = "00000000-0000-4000-8000-000000000002";
    state = studioReducer(state, {
      type: "run-started",
      requestId: 2,
      runId: successorId,
      digest: document.digest,
      configuration,
    });
    expect(state.latestRun?.result).toBe(runResult);
    state = studioReducer(state, { type: "run-cancel-requested", requestId: 2 });
    expect(state.runStatus.kind).toBe("cancelling");
    state = studioReducer(state, {
      type: "run-finished",
      requestId: 2,
      outcome: {
        kind: "cancelled",
        cancellation: {
          protocol: BRIDGE_PROTOCOL,
          runId: successorId,
          plan,
          elapsedSeconds: 0.3,
          progress: { ...progress, runId: successorId },
        },
      },
      diagnostics: [],
    });
    expect(state.runStatus.kind).toBe("cancelled");
    expect(state.latestRun?.result).toBe(runResult);
  });

  it("keeps command search ephemeral and closed by default", () => {
    let state = initialStudioState("source");
    expect(state.commandPalette).toEqual({ kind: "closed" });
    expect(studioReducer(state, { type: "command-query-edited", query: "run" })).toBe(state);

    state = studioReducer(state, { type: "command-palette-opened" });
    state = studioReducer(state, { type: "command-query-edited", query: "run" });
    expect(state.commandPalette).toEqual({ kind: "open", query: "run" });
    state = studioReducer(state, { type: "command-palette-closed" });
    expect(state.commandPalette).toEqual({ kind: "closed" });
  });

  it("commits one child revision and navigates lineage without inverse transactions", () => {
    let state = initialStudioState("source");
    state = studioReducer(state, { type: "compile-started", requestId: 1 });
    state = studioReducer(state, {
      type: "compile-finished",
      requestId: 1,
      compiledSource: "source",
      document,
      workspaceLayout: null,
      diagnostics: [],
    });
    state = studioReducer(state, { type: "value-edit-input-edited", value: "310" });
    state = studioReducer(state, { type: "value-edit-preview-started", requestId: 2 });
    state = studioReducer(state, {
      type: "value-edit-preview-finished",
      requestId: 2,
      digest: document.digest,
      targetId: "Field:01",
      input: "310",
      plan: editPlan,
      diagnostics: [],
    });
    state = studioReducer(state, {
      type: "value-edit-commit-started",
      requestId: 3,
      plan: editPlan,
    });
    state = studioReducer(state, {
      type: "value-edit-commit-finished",
      requestId: 3,
      result: editResult,
      diagnostics: [],
    });

    expect(state.document).toBe(childDocument);
    expect(state.sourceDigest).toBe(document.digest);
    expect(state.revisionLineage.map((entry) => entry.document.digest)).toEqual([
      document.digest,
      childDocument.digest,
    ]);
    expect(state.revisionIndex).toBe(1);
    expect(state.valueEditInput).toBe("310");

    state = studioReducer(state, { type: "revision-undo" });
    expect(state.document).toBe(document);
    expect(state.valueEditInput).toBe("300");
    state = studioReducer(state, { type: "revision-redo" });
    expect(state.document).toBe(childDocument);
    expect(state.valueEditInput).toBe("310");

    state = studioReducer(state, {
      type: "value-edit-commit-started",
      requestId: 4,
      plan: editPlan,
    });
    expect(studioReducer(state, { type: "revision-undo" })).toBe(state);
  });

  it("suppresses an edit preview after its target input changed", () => {
    let state = initialStudioState("source");
    state = studioReducer(state, { type: "compile-started", requestId: 1 });
    state = studioReducer(state, {
      type: "compile-finished",
      requestId: 1,
      compiledSource: "source",
      document,
      workspaceLayout: null,
      diagnostics: [],
    });
    state = studioReducer(state, { type: "value-edit-input-edited", value: "310" });
    state = studioReducer(state, { type: "value-edit-preview-started", requestId: 2 });
    state = studioReducer(state, { type: "value-edit-input-edited", value: "320" });
    const next = studioReducer(state, {
      type: "value-edit-preview-finished",
      requestId: 2,
      digest: document.digest,
      targetId: "Field:01",
      input: "310",
      plan: editPlan,
      diagnostics: [],
    });
    expect(next).toBe(state);
  });
});
