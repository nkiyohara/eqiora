import { describe, expect, it } from "vitest";
import { BRIDGE_PROTOCOL } from "./protocol";
import type { SpatialRunPlan } from "./spatial-protocol";
import {
  initialSpatialWorkflowState,
  spatialPlanIsCurrent,
  spatialWorkflowReducer,
  validateSpatialConfiguration,
} from "./spatial-workflow";

const PLAN: SpatialRunPlan = {
  protocol: BRIDGE_PROTOCOL,
  key: "a".repeat(64),
  modelDigest: "sha256:0123456789abcdef",
  realizationRevision: 1,
  requirements: { spatialDimension: 2, scalarType: "f64", vectorLayout: "replicated" },
  discretization: {
    method: "finite-element",
    space: "continuous-lagrange",
    order: 1,
    mesh: "generated-cartesian",
    cellsPerAxis: 16,
    cellCount: 256,
    quadrature: "gauss-legendre",
    pointsPerAxis: 2,
    fieldValueCount: 289,
  },
  solver: {
    adapter: "eqiora.reference",
    algorithm: "conjugate-gradient",
    preconditioner: "identity",
    reduction: "reproducible",
    relativeTolerance: 1e-10,
    absoluteTolerance: 1e-12,
    maximumIterations: 1_000,
  },
  placement: {
    kind: "host",
    adapter: "eqiora.host.serial",
    workers: 1,
    maximumWorkers: 8,
    budgetSource: "studio-session-budget",
  },
  limits: { maximumEntityCount: 250_000 },
  acceptance: {
    algebraic: "independent-true-residual",
    continuous: "boundary-source-balance",
    independentTrueResidual: true,
  },
};

describe("spatial Realization workflow", () => {
  it("checks dimensional entity counts before bridge allocation", () => {
    expect(
      validateSpatialConfiguration(
        { method: "finite-element", cellsPerAxis: "16", workers: "2" },
        2,
        8,
      ).value,
    ).toEqual({
      method: "finite-element",
      cellsPerAxis: 16,
      workers: 2,
      cellCount: 256,
      fieldValueCount: 289,
    });
    expect(
      validateSpatialConfiguration(
        { method: "finite-element", cellsPerAxis: "64", workers: "1" },
        3,
        8,
      ).value,
    ).toBeNull();
  });

  it("distinguishes the Studio session budget from requested placement", () => {
    const validation = validateSpatialConfiguration(
      { method: "finite-volume", cellsPerAxis: "8", workers: "9" },
      2,
      8,
    );
    expect(validation.value).toBeNull();
    expect(validation.errors.workers).toContain("Studio session admits at most 8");
  });

  it("invalidates an accepted plan when Realization intent changes", () => {
    let state = initialSpatialWorkflowState();
    state = spatialWorkflowReducer(state, {
      type: "context-changed",
      digest: PLAN.modelDigest,
    });
    const configuration = validateSpatialConfiguration(state.configuration, 2, 8).value;
    expect(configuration).not.toBeNull();
    if (configuration === null) return;
    state = spatialWorkflowReducer(state, { type: "preview-started", requestId: 1 });
    state = spatialWorkflowReducer(state, {
      type: "preview-finished",
      requestId: 1,
      digest: PLAN.modelDigest,
      realizationRevision: 1,
      configuration,
      plan: PLAN,
      diagnostics: [],
    });
    expect(spatialPlanIsCurrent(state, PLAN.modelDigest, configuration)).toBe(true);

    state = spatialWorkflowReducer(state, {
      type: "input-edited",
      field: "method",
      value: "finite-volume",
    });
    expect(state.realizationRevision).toBe(2);
    expect(state.planStatus.kind).toBe("idle");
  });

  it("ignores a preview from an obsolete Realization revision", () => {
    let state = initialSpatialWorkflowState();
    state = spatialWorkflowReducer(state, {
      type: "context-changed",
      digest: PLAN.modelDigest,
    });
    const configuration = validateSpatialConfiguration(state.configuration, 2, 8).value;
    expect(configuration).not.toBeNull();
    if (configuration === null) return;
    state = spatialWorkflowReducer(state, { type: "preview-started", requestId: 1 });
    state = spatialWorkflowReducer(state, {
      type: "input-edited",
      field: "cellsPerAxis",
      value: "17",
    });
    const next = spatialWorkflowReducer(state, {
      type: "preview-finished",
      requestId: 1,
      digest: PLAN.modelDigest,
      realizationRevision: 1,
      configuration,
      plan: PLAN,
      diagnostics: [],
    });
    expect(next).toBe(state);
  });
});
