import { describe, expect, it } from "vitest";
import { BRIDGE_PROTOCOL } from "./protocol";
import {
  MAX_SPATIAL_ENTITY_COUNT,
  type SpatialRunResult,
  spatialRunResultSchema,
} from "./spatial-protocol";
import {
  createPreviewScalarFieldDataBridge,
  decodeScalarFieldChunk,
  encodeScalarFieldChunk,
} from "./scalar-field-bridge";
import { SCALAR_FIELD_VALUES_PER_CHUNK } from "./scalar-field-protocol";

function acceptedResult(
  method: "finite-element" | "finite-volume" = "finite-element",
): SpatialRunResult {
  const cellsPerAxis = 64;
  const finiteElement = method === "finite-element";
  const axis = finiteElement ? cellsPerAxis + 1 : cellsPerAxis;
  return spatialRunResultSchema.parse({
    protocol: BRIDGE_PROTOCOL,
    runId: "00000000-0000-4000-8000-000000000001",
    digest: "sha256:0123456789abcdef",
    plan: {
      protocol: BRIDGE_PROTOCOL,
      key: "a".repeat(64),
      modelDigest: "sha256:0123456789abcdef",
      realizationRevision: 1,
      requirements: { spatialDimension: 2, scalarType: "f64", vectorLayout: "replicated" },
      discretization: {
        method,
        space: finiteElement ? "continuous-lagrange" : "cell-constant",
        order: finiteElement ? 1 : null,
        mesh: "generated-cartesian",
        cellsPerAxis,
        cellCount: cellsPerAxis ** 2,
        quadrature: finiteElement ? "gauss-legendre" : "cell-centroid",
        pointsPerAxis: finiteElement ? 2 : null,
        fieldValueCount: axis ** 2,
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
      valueCount: axis ** 2,
      minimum: -2,
      maximum: 3,
    },
    balance: { boundaryTotal: 1, integratedSource: 1, relativeImbalance: 0 },
    assembly: {
      execution: { adapter: "eqiora.host.serial", topology: { kind: "host", workers: 1 } },
      packetCount: cellsPerAxis ** 2,
      targetCount: axis ** 2,
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

describe("scalar-field binary bridge", () => {
  it("round-trips explicit f64 little-endian bytes", () => {
    const decoded = decodeScalarFieldChunk(encodeScalarFieldChunk([-2.5, 0, 1 / 3, 9.25]), 4);
    expect(decoded.ok).toBe(true);
    if (decoded.ok) expect([...decoded.value]).toEqual([-2.5, 0, 1 / 3, 9.25]);
  });

  it("rejects short, long, non-binary, and non-finite chunks", () => {
    expect(decodeScalarFieldChunk(new ArrayBuffer(7), 1)).toMatchObject({
      ok: false,
      failure: { code: "invalid-chunk" },
    });
    expect(decodeScalarFieldChunk(new ArrayBuffer(16), 1)).toMatchObject({
      ok: false,
      failure: { code: "invalid-chunk" },
    });
    expect(decodeScalarFieldChunk([1], 1)).toMatchObject({
      ok: false,
      failure: { code: "invalid-chunk" },
    });
    expect(decodeScalarFieldChunk(encodeScalarFieldChunk([Number.NaN]), 1)).toMatchObject({
      ok: false,
      failure: { code: "nonfinite-chunk" },
    });
    expect(
      decodeScalarFieldChunk(encodeScalarFieldChunk([Number.POSITIVE_INFINITY]), 1),
    ).toMatchObject({
      ok: false,
      failure: { code: "nonfinite-chunk" },
    });
    expect(
      decodeScalarFieldChunk(
        new ArrayBuffer((SCALAR_FIELD_VALUES_PER_CHUNK + 1) * 8),
        SCALAR_FIELD_VALUES_PER_CHUNK + 1,
      ),
    ).toMatchObject({ ok: false, failure: { code: "invalid-chunk" } });
  });

  for (const method of ["finite-element", "finite-volume"] as const) {
    it(`streams the ${method} preview through the same decoder with exact extrema`, async () => {
      const bridge = createPreviewScalarFieldDataBridge();
      const opened = await bridge.open(acceptedResult(method));
      expect(opened.ok).toBe(true);
      if (!opened.ok) return;

      const chunks: Float64Array[] = [];
      for (let index = 0; index < opened.value.transport.chunkCount; index += 1) {
        const chunk = await bridge.readChunk(opened.value, index);
        expect(chunk.ok).toBe(true);
        if (chunk.ok) chunks.push(chunk.value);
      }
      const values = chunks.flatMap((chunk) => [...chunk]);
      expect(values).toHaveLength(opened.value.field.valueCount);
      expect(Math.min(...values)).toBe(opened.value.field.minimum);
      expect(Math.max(...values)).toBe(opened.value.field.maximum);
    });
  }

  it("rejects descriptors which were not opened in that preview session", async () => {
    const first = createPreviewScalarFieldDataBridge();
    const second = createPreviewScalarFieldDataBridge();
    const opened = await first.open(acceptedResult());
    expect(opened.ok).toBe(true);
    if (!opened.ok) return;
    await expect(second.readChunk(opened.value, 0)).resolves.toMatchObject({
      ok: false,
      failure: { code: "descriptor-mismatch" },
    });
    await expect(
      first.readChunk({ ...opened.value, runId: "00000000-0000-4000-8000-000000000002" }, 0),
    ).resolves.toMatchObject({
      ok: false,
      failure: { code: "descriptor-mismatch" },
    });
  });

  it("rejects out-of-range chunks before allocating values", async () => {
    const bridge = createPreviewScalarFieldDataBridge();
    const opened = await bridge.open(acceptedResult());
    expect(opened.ok).toBe(true);
    if (!opened.ok) return;
    await expect(
      bridge.readChunk(opened.value, opened.value.transport.chunkCount),
    ).resolves.toMatchObject({
      ok: false,
      failure: { code: "chunk-out-of-range" },
    });
  });
});
