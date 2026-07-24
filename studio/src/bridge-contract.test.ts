import { describe, expect, it } from "vitest";
import {
  checkedRequest,
  outcomeMatchesRunRequest,
  spatialResultMatchesRequest,
} from "./bridge-contract";
import { BRIDGE_PROTOCOL } from "./protocol";
import {
  type RunPlan,
  type RunRequest,
  runCancellationSchema,
  runPlanSchema,
  runRequestSchema,
} from "./reference-run-protocol";
import {
  type SpatialRealizationRunRequest,
  type SpatialRunPlan,
  spatialRunPlanSchema,
  spatialRunResultSchema,
} from "./spatial-protocol";
import { valueEditPlanSchema } from "./value-edit-protocol";

const RUN_ID = "00000000-0000-4000-8000-000000000001";
const OTHER_RUN_ID = "00000000-0000-4000-8000-000000000002";
const PLAN: RunPlan = {
  protocol: BRIDGE_PROTOCOL,
  key: "eqiora.reference-plan/v1:key",
  adapter: { id: "eqiora.reference", version: "0.1.0" },
  placement: { kind: "host", workers: 1 },
  integration: { method: "backward-euler", endTime: 1, maxStep: 0.1 },
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
const RUN_REQUEST: RunRequest = {
  protocol: BRIDGE_PROTOCOL,
  digest: "sha256:0123456789abcdef",
  endTime: 1,
  maxStep: 0.1,
  runId: RUN_ID,
  planKey: PLAN.key,
};
const SPATIAL_PLAN: SpatialRunPlan = {
  protocol: BRIDGE_PROTOCOL,
  key: "a".repeat(64),
  modelDigest: RUN_REQUEST.digest,
  realizationRevision: 3,
  requirements: { spatialDimension: 2, scalarType: "f64", vectorLayout: "replicated" },
  discretization: {
    method: "finite-element",
    space: "continuous-lagrange",
    order: 1,
    mesh: "generated-cartesian",
    cellsPerAxis: 8,
    cellCount: 64,
    quadrature: "gauss-legendre",
    pointsPerAxis: 2,
    fieldValueCount: 81,
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
    adapter: "eqiora.rayon",
    workers: 2,
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
const SPATIAL_REQUEST: SpatialRealizationRunRequest = {
  protocol: BRIDGE_PROTOCOL,
  digest: RUN_REQUEST.digest,
  realizationRevision: 3,
  method: "finite-element",
  cellsPerAxis: 8,
  workers: 2,
  runId: RUN_ID,
  planKey: SPATIAL_PLAN.key,
};
const SPATIAL_RESULT = {
  protocol: BRIDGE_PROTOCOL,
  runId: RUN_ID,
  digest: RUN_REQUEST.digest,
  plan: SPATIAL_PLAN,
  elapsedSeconds: 0.01,
  field: { location: "vertex", valueCount: 81, minimum: 0, maximum: 0.1 },
  balance: { boundaryTotal: 1, integratedSource: 1, relativeImbalance: 0 },
  assembly: {
    execution: { adapter: "eqiora.rayon", topology: { kind: "host", workers: 2 } },
    packetCount: 64,
    targetCount: 81,
  },
  solve: {
    backend: "eqiora.reference.cg",
    execution: { adapter: "eqiora.rayon", topology: { kind: "host", workers: 2 } },
    verification: { adapter: "eqiora.reference", topology: { kind: "host", workers: 1 } },
    algorithm: "conjugate-gradient",
    preconditioner: "identity",
    reduction: "reproducible",
    reason: "residual-tolerance-satisfied",
    completedIterations: 12,
    initialResidualNorm: 1,
    reportedResidualNorm: 1e-12,
    trueResidualNorm: 2e-12,
    residualTarget: 1e-10,
  },
} as const;

describe("Studio bridge request boundary", () => {
  it("returns a structured diagnostic instead of throwing for malformed input", () => {
    const checked = checkedRequest(
      runRequestSchema,
      {
        protocol: BRIDGE_PROTOCOL,
        digest: "sha256:0123456789abcdef",
        endTime: 1,
        maxStep: Number.NaN,
        runId: RUN_ID,
      },
      "Run",
    );

    expect(checked).toEqual({
      ok: false,
      failure: {
        protocol: BRIDGE_PROTOCOL,
        result: null,
        diagnostics: [
          expect.objectContaining({
            source: "studio",
            severity: "error",
            code: "ST0002",
          }),
        ],
      },
    });
  });

  it("enforces the bounded-step wire contract before allocation", () => {
    const checked = checkedRequest(
      runRequestSchema,
      {
        protocol: BRIDGE_PROTOCOL,
        digest: "sha256:0123456789abcdef",
        endTime: 10,
        maxStep: 1e-7,
        runId: RUN_ID,
      },
      "Run",
    );
    expect(checked.ok).toBe(false);
  });

  it("does not let a reference result claim an independent verifier", () => {
    const decoded = runPlanSchema.safeParse({
      ...PLAN,
      acceptance: { kind: "semantic-oracle", independentVerifier: true },
    });

    expect(decoded.success).toBe(false);
  });

  it("does not let a value edit change dimension across the bridge", () => {
    const decoded = valueEditPlanSchema.safeParse({
      protocol: BRIDGE_PROTOCOL,
      key: `eqiora.value-edit-plan/v1:${"a".repeat(64)}`,
      baseDigest: "sha256:0123456789abcdef",
      baseRevision: 1,
      targetId: "Parameter:01",
      before: { value: 1, dimension: "T^-1" },
      after: { value: 2, dimension: "L" },
      transactionDigest: "a".repeat(64),
    });
    expect(decoded.success).toBe(false);
  });

  it("rejects cancellation evidence routed from another run", () => {
    const decoded = runCancellationSchema.safeParse({
      protocol: BRIDGE_PROTOCOL,
      runId: RUN_ID,
      plan: PLAN,
      elapsedSeconds: 0.2,
      progress: {
        protocol: BRIDGE_PROTOCOL,
        runId: OTHER_RUN_ID,
        modelTime: 0.5,
        endTime: 1,
        acceptedSteps: 5,
        maximumSteps: 1_000_000,
        elapsedSeconds: 0.1,
      },
    });
    expect(decoded.success).toBe(false);
  });

  it("binds a terminal outcome to the exact submitted request", () => {
    const cancellation = runCancellationSchema.parse({
      protocol: BRIDGE_PROTOCOL,
      runId: RUN_ID,
      plan: PLAN,
      elapsedSeconds: 0.2,
      progress: {
        protocol: BRIDGE_PROTOCOL,
        runId: RUN_ID,
        modelTime: 0.5,
        endTime: 1,
        acceptedSteps: 5,
        maximumSteps: 1_000_000,
        elapsedSeconds: 0.1,
      },
    });
    expect(outcomeMatchesRunRequest(RUN_REQUEST, { kind: "cancelled", cancellation })).toBe(true);
    expect(
      outcomeMatchesRunRequest(RUN_REQUEST, {
        kind: "cancelled",
        cancellation: {
          ...cancellation,
          runId: OTHER_RUN_ID,
          progress: { ...cancellation.progress, runId: OTHER_RUN_ID },
        },
      }),
    ).toBe(false);
    expect(
      outcomeMatchesRunRequest(RUN_REQUEST, {
        kind: "cancelled",
        cancellation: { ...cancellation, plan: { ...PLAN, key: "another-plan" } },
      }),
    ).toBe(false);

    const completed = {
      protocol: BRIDGE_PROTOCOL,
      digest: RUN_REQUEST.digest,
      evidence: { plan: PLAN, elapsedSeconds: 0.2, fieldCount: 0, sampleCount: 0 },
      series: [],
    };
    expect(outcomeMatchesRunRequest(RUN_REQUEST, { kind: "completed", result: completed })).toBe(
      true,
    );
    expect(
      outcomeMatchesRunRequest(RUN_REQUEST, {
        kind: "completed",
        result: { ...completed, digest: "sha256:fedcba9876543210" },
      }),
    ).toBe(false);
  });

  it("rejects an incoherent spatial method contract", () => {
    expect(
      spatialRunPlanSchema.safeParse({
        ...SPATIAL_PLAN,
        discretization: {
          ...SPATIAL_PLAN.discretization,
          method: "finite-volume",
        },
      }).success,
    ).toBe(false);
  });

  it("binds host adapter identity to the admitted worker count", () => {
    expect(
      spatialRunPlanSchema.safeParse({
        ...SPATIAL_PLAN,
        placement: { ...SPATIAL_PLAN.placement, adapter: "eqiora.host.serial" },
      }).success,
    ).toBe(false);
  });

  it("requires spatial acceptance evidence to pass the true-residual target", () => {
    expect(
      spatialRunResultSchema.safeParse({
        ...SPATIAL_RESULT,
        solve: { ...SPATIAL_RESULT.solve, trueResidualNorm: 2e-8 },
      }).success,
    ).toBe(false);
  });

  it("binds a spatial result to the exact submitted Realization", () => {
    const result = spatialRunResultSchema.parse(SPATIAL_RESULT);
    expect(spatialResultMatchesRequest(SPATIAL_REQUEST, result)).toBe(true);
    expect(
      spatialResultMatchesRequest(
        { ...SPATIAL_REQUEST, realizationRevision: SPATIAL_REQUEST.realizationRevision + 1 },
        result,
      ),
    ).toBe(false);
    expect(spatialResultMatchesRequest({ ...SPATIAL_REQUEST, workers: 1 }, result)).toBe(false);
  });
});
