import { describe, expect, test } from "vitest";
import { BRIDGE_PROTOCOL, type DocumentProjection } from "./protocol";
import { initialStudioState, studioReducer } from "./state";
import type { ValueEditPlan, ValueEditResult } from "./value-edit-protocol";

const document: DocumentProjection = {
  protocol: BRIDGE_PROTOCOL,
  digest: "0123456789abcdef",
  revision: 1,
  modelId: "Model:test",
  nodes: [
    { id: "Parameter:x", name: "x", kind: "parameter", summary: "x", dimension: "1", value: 1 },
  ],
  edges: [],
};

const editPlan: ValueEditPlan = {
  protocol: BRIDGE_PROTOCOL,
  key: `eqiora.value-edit-plan/v1:${"a".repeat(64)}`,
  baseDigest: document.digest,
  baseRevision: 1,
  targetId: "Parameter:x",
  before: { value: 1, dimension: "1" },
  after: { value: 2, dimension: "1" },
  transactionDigest: "a".repeat(64),
};
const childDocument: DocumentProjection = {
  ...document,
  digest: "fedcba9876543210",
  revision: 2,
  nodes: document.nodes.map((node) => ({ ...node, value: 2 })),
};
const editResult: ValueEditResult = {
  protocol: BRIDGE_PROTOCOL,
  document: childDocument,
  evidence: {
    plan: editPlan,
    resultDigest: childDocument.digest,
    resultRevision: childDocument.revision,
  },
};

function compiledState() {
  let state = initialStudioState("source");
  state = studioReducer(state, { type: "compile-started", requestId: 1 });
  return studioReducer(state, {
    type: "compile-finished",
    requestId: 1,
    compiledSource: "source",
    document,
    workspaceLayout: null,
    diagnostics: [],
  });
}

function committedState() {
  let state = compiledState();
  state = studioReducer(state, { type: "value-edit-input-edited", value: "2" });
  state = studioReducer(state, { type: "value-edit-preview-started", requestId: 2 });
  state = studioReducer(state, {
    type: "value-edit-preview-finished",
    requestId: 2,
    digest: document.digest,
    targetId: "Parameter:x",
    input: "2",
    plan: editPlan,
    diagnostics: [],
  });
  state = studioReducer(state, { type: "value-edit-commit-started", requestId: 3, plan: editPlan });
  return studioReducer(state, {
    type: "value-edit-commit-finished",
    requestId: 3,
    result: editResult,
    diagnostics: [],
  });
}

describe("Studio compiler state", () => {
  test("accepts a compiled document and owns layout", () => {
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
    expect(state.document?.digest).toBe(document.digest);
    state = studioReducer(state, {
      type: "node-moved",
      nodeId: "Parameter:x",
      position: { x: 4, y: 5 },
    });
    expect(state.layoutsByDigest[document.digest]?.["Parameter:x"]).toEqual({ x: 4, y: 5 });
  });
  test("suppresses stale compile responses", () => {
    const state = studioReducer(initialStudioState("source"), {
      type: "compile-finished",
      requestId: 1,
      compiledSource: "source",
      document,
      workspaceLayout: null,
      diagnostics: [],
    });
    expect(state.document).toBeNull();
  });
  test("owns command palette state", () =>
    expect(
      studioReducer(initialStudioState("source"), { type: "command-palette-opened" }).commandPalette
        .kind,
    ).toBe("open"));

  test("retains the exact failed source for diagnostic provenance", () => {
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

  test("suppresses a preview after its target input changes", () => {
    let state = compiledState();
    state = studioReducer(state, { type: "value-edit-input-edited", value: "2" });
    state = studioReducer(state, { type: "value-edit-preview-started", requestId: 2 });
    state = studioReducer(state, { type: "value-edit-input-edited", value: "3" });
    const next = studioReducer(state, {
      type: "value-edit-preview-finished",
      requestId: 2,
      digest: document.digest,
      targetId: "Parameter:x",
      input: "2",
      plan: editPlan,
      diagnostics: [],
    });
    expect(next).toBe(state);
  });

  test("accepts only the current exact value-edit preview", () => {
    let state = compiledState();
    state = studioReducer(state, { type: "value-edit-input-edited", value: "2" });
    state = studioReducer(state, { type: "value-edit-preview-started", requestId: 2 });
    state = studioReducer(state, {
      type: "value-edit-preview-finished",
      requestId: 2,
      digest: document.digest,
      targetId: "Parameter:x",
      input: "2",
      plan: editPlan,
      diagnostics: [],
    });
    expect(state.valueEditStatus).toEqual({
      kind: "ready",
      plan: editPlan,
      input: "2",
      digest: document.digest,
      targetId: "Parameter:x",
    });
  });

  test("commits one child and navigates without inverse transactions", () => {
    let state = committedState();
    expect(state.document).toBe(childDocument);
    expect(state.sourceDigest).toBe(document.digest);
    expect(state.revisionLineage.map((entry) => entry.document.digest)).toEqual([
      document.digest,
      childDocument.digest,
    ]);
    state = studioReducer(state, { type: "revision-undo" });
    expect(state.document).toBe(document);
    expect(state.valueEditInput).toBe("1");
    state = studioReducer(state, { type: "revision-redo" });
    expect(state.document).toBe(childDocument);
    expect(state.valueEditInput).toBe("2");
    state = studioReducer(state, {
      type: "value-edit-commit-started",
      requestId: 4,
      plan: editPlan,
    });
    expect(studioReducer(state, { type: "revision-undo" })).toBe(state);
  });

  test("a new child after undo replaces the abandoned forward branch", () => {
    let state = studioReducer(committedState(), { type: "revision-undo" });
    const branchPlan: ValueEditPlan = {
      ...editPlan,
      key: `eqiora.value-edit-plan/v1:${"b".repeat(64)}`,
      after: { value: 3, dimension: "1" },
      transactionDigest: "b".repeat(64),
    };
    const branchDocument: DocumentProjection = {
      ...document,
      digest: "aaaaaaaaaaaaaaaa",
      revision: 2,
      nodes: document.nodes.map((node) => ({ ...node, value: 3 })),
    };
    state = studioReducer(state, {
      type: "value-edit-commit-started",
      requestId: 5,
      plan: branchPlan,
    });
    state = studioReducer(state, {
      type: "value-edit-commit-finished",
      requestId: 5,
      result: {
        protocol: BRIDGE_PROTOCOL,
        document: branchDocument,
        evidence: {
          plan: branchPlan,
          resultDigest: branchDocument.digest,
          resultRevision: branchDocument.revision,
        },
      },
      diagnostics: [],
    });
    expect(state.revisionLineage.map((entry) => entry.document.digest)).toEqual([
      document.digest,
      branchDocument.digest,
    ]);
    expect(studioReducer(state, { type: "revision-redo" })).toBe(state);
  });
});
