import { describe, expect, it } from "vitest";
import { BRIDGE_PROTOCOL } from "./protocol";
import {
  descriptorMatchesAcceptedResult,
  SCALAR_FIELD_MAX_CHUNK_COUNT,
  SCALAR_FIELD_VALUES_PER_CHUNK,
  SCALAR_FIELD_VIEW_PROTOCOL,
  type ScalarFieldDescriptor,
  scalarFieldChunkRequestSchema,
  scalarFieldChunkValueCount,
  scalarFieldDescriptorSchema,
  scalarFieldOpenEnvelopeSchema,
  scalarFieldOpenRequestSchema,
} from "./scalar-field-protocol";
import {
  MAX_SPATIAL_ENTITY_COUNT,
  type SpatialRunResult,
  spatialRunResultSchema,
} from "./spatial-protocol";

const RUN_ID = "00000000-0000-4000-8000-000000000001";
const MODEL_DIGEST = "sha256:0123456789abcdef";
const PLAN_KEY = "a".repeat(64);

function spatialResult(
  method: "finite-element" | "finite-volume" = "finite-element",
  spatialDimension = 2,
): SpatialRunResult {
  const cellsPerAxis = 64;
  const finiteElement = method === "finite-element";
  const axis = finiteElement ? cellsPerAxis + 1 : cellsPerAxis;
  return spatialRunResultSchema.parse({
    protocol: BRIDGE_PROTOCOL,
    runId: RUN_ID,
    digest: MODEL_DIGEST,
    plan: {
      protocol: BRIDGE_PROTOCOL,
      key: PLAN_KEY,
      modelDigest: MODEL_DIGEST,
      realizationRevision: 3,
      requirements: { spatialDimension, scalarType: "f64", vectorLayout: "replicated" },
      discretization: {
        method,
        space: finiteElement ? "continuous-lagrange" : "cell-constant",
        order: finiteElement ? 1 : null,
        mesh: "generated-cartesian",
        cellsPerAxis,
        cellCount: cellsPerAxis ** spatialDimension,
        quadrature: finiteElement ? "gauss-legendre" : "cell-centroid",
        pointsPerAxis: finiteElement ? 2 : null,
        fieldValueCount: axis ** spatialDimension,
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
      limits: { maximumEntityCount: MAX_SPATIAL_ENTITY_COUNT },
      acceptance: {
        algebraic: "independent-true-residual",
        continuous: "boundary-source-balance",
        independentTrueResidual: true,
      },
    },
    elapsedSeconds: 0.01,
    field: {
      location: finiteElement ? "vertex" : "cell-center",
      valueCount: axis ** spatialDimension,
      minimum: 0,
      maximum: 1,
    },
    balance: { boundaryTotal: 1, integratedSource: 1, relativeImbalance: 0 },
    assembly: {
      execution: { adapter: "eqiora.host.serial", topology: { kind: "host", workers: 1 } },
      packetCount: cellsPerAxis ** spatialDimension,
      targetCount: axis ** spatialDimension,
    },
    solve: {
      backend: "eqiora.reference.cg",
      execution: { adapter: "eqiora.host.serial", topology: { kind: "host", workers: 1 } },
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
  });
}

function descriptor(result = spatialResult()): ScalarFieldDescriptor {
  const axis =
    result.plan.discretization.method === "finite-element"
      ? result.plan.discretization.cellsPerAxis + 1
      : result.plan.discretization.cellsPerAxis;
  return scalarFieldDescriptorSchema.parse({
    protocol: SCALAR_FIELD_VIEW_PROTOCOL,
    modelDigest: result.digest,
    runId: result.runId,
    planKey: result.plan.key,
    field: {
      id: "Field:solution",
      name: "solution",
      dimension: "1",
      coherentSiUnit: "1",
      scalarType: "f64",
      location: result.field.location,
      valueCount: result.field.valueCount,
      minimum: result.field.minimum,
      maximum: result.field.maximum,
    },
    domain: {
      id: "Domain:unit-square",
      boundsM: [
        [0, 1],
        [0, 1],
      ],
    },
    grid: {
      kind: "uniform-cartesian-2d",
      logicalShape: [axis, axis],
      order: "row-major-last-axis-fastest",
    },
    transport: {
      kind: "explicit-owned-host-copy",
      encoding: "f64-le",
      valuesPerChunk: SCALAR_FIELD_VALUES_PER_CHUNK,
      chunkCount: Math.ceil(result.field.valueCount / SCALAR_FIELD_VALUES_PER_CHUNK),
    },
  });
}

describe("scalar-field data-plane protocol", () => {
  it("accepts only closed open and chunk requests", () => {
    expect(
      scalarFieldOpenRequestSchema.safeParse({
        protocol: SCALAR_FIELD_VIEW_PROTOCOL,
        modelDigest: MODEL_DIGEST,
        runId: RUN_ID,
        planKey: PLAN_KEY,
      }).success,
    ).toBe(true);
    expect(
      scalarFieldOpenRequestSchema.safeParse({
        protocol: SCALAR_FIELD_VIEW_PROTOCOL,
        modelDigest: MODEL_DIGEST,
        runId: RUN_ID,
        planKey: PLAN_KEY,
        range: [0, 10],
      }).success,
    ).toBe(false);
    expect(
      scalarFieldChunkRequestSchema.safeParse({
        protocol: SCALAR_FIELD_VIEW_PROTOCOL,
        modelDigest: MODEL_DIGEST,
        runId: RUN_ID,
        planKey: PLAN_KEY,
        chunkIndex: SCALAR_FIELD_MAX_CHUNK_COUNT,
      }).success,
    ).toBe(false);
  });

  it("checks shape, range, entity budget, and canonical chunk count", () => {
    const valid = descriptor();
    expect(scalarFieldDescriptorSchema.safeParse(valid).success).toBe(true);
    expect(
      scalarFieldDescriptorSchema.safeParse({
        ...valid,
        grid: { ...valid.grid, logicalShape: [64, 65] },
      }).success,
    ).toBe(false);
    expect(
      scalarFieldDescriptorSchema.safeParse({
        ...valid,
        field: { ...valid.field, maximum: -1 },
      }).success,
    ).toBe(false);
    expect(
      scalarFieldDescriptorSchema.safeParse({
        ...valid,
        field: { ...valid.field, valueCount: MAX_SPATIAL_ENTITY_COUNT + 1 },
      }).success,
    ).toBe(false);
    expect(
      scalarFieldDescriptorSchema.safeParse({
        ...valid,
        transport: { ...valid.transport, chunkCount: valid.transport.chunkCount + 1 },
      }).success,
    ).toBe(false);
  });

  it("binds descriptor presence to error-diagnostic absence in open envelopes", () => {
    const valid = descriptor();
    expect(
      scalarFieldOpenEnvelopeSchema.safeParse({
        protocol: SCALAR_FIELD_VIEW_PROTOCOL,
        result: valid,
        diagnostics: [],
      }).success,
    ).toBe(true);
    expect(
      scalarFieldOpenEnvelopeSchema.safeParse({
        protocol: SCALAR_FIELD_VIEW_PROTOCOL,
        result: valid,
        diagnostics: [
          {
            source: "studio",
            severity: "error",
            code: "STFIELD",
            message: "contradictory",
            graphPath: null,
            span: null,
          },
        ],
      }).success,
    ).toBe(false);
    expect(
      scalarFieldOpenEnvelopeSchema.safeParse({
        protocol: SCALAR_FIELD_VIEW_PROTOCOL,
        result: null,
        diagnostics: [],
      }).success,
    ).toBe(false);
  });

  it("binds FEM and FVM descriptors to exact accepted two-dimensional results", () => {
    const fem = spatialResult("finite-element");
    const fvm = spatialResult("finite-volume");
    expect(descriptorMatchesAcceptedResult(fem, descriptor(fem))).toBe(true);
    expect(descriptorMatchesAcceptedResult(fvm, descriptor(fvm))).toBe(true);
    expect(
      descriptorMatchesAcceptedResult(fem, {
        ...descriptor(fem),
        domain: {
          ...descriptor(fem).domain,
          boundsM: [
            [-2, 3],
            [4, 9],
          ],
        },
      }),
    ).toBe(true);
    expect(
      descriptorMatchesAcceptedResult(fem, {
        ...descriptor(fem),
        runId: "00000000-0000-4000-8000-000000000002",
      }),
    ).toBe(false);
    expect(
      descriptorMatchesAcceptedResult(fem, {
        ...descriptor(fem),
        field: { ...descriptor(fem).field, location: "cell-center" },
      }),
    ).toBe(false);
    expect(
      descriptorMatchesAcceptedResult(fem, {
        ...descriptor(fem),
        field: { ...descriptor(fem).field, maximum: 0.5 },
      }),
    ).toBe(false);
    expect(
      descriptorMatchesAcceptedResult(spatialResult("finite-element", 1), descriptor(fem)),
    ).toBe(false);
  });

  it("derives exact last-chunk counts without admitting arbitrary ranges", () => {
    const value = descriptor();
    expect(scalarFieldChunkValueCount(value, 0)).toBe(SCALAR_FIELD_VALUES_PER_CHUNK);
    expect(scalarFieldChunkValueCount(value, 1)).toBe(129);
    expect(scalarFieldChunkValueCount(value, -1)).toBeNull();
    expect(scalarFieldChunkValueCount(value, value.transport.chunkCount)).toBeNull();
  });
});
