import { describe, expect, it } from "vitest";
import {
  BRIDGE_PROTOCOL,
  MAX_SPATIAL_ENTITY_COUNT,
  type SpatialRunResult,
  spatialRunResultSchema,
} from "./protocol";
import type { ScalarFieldBridgeResult, ScalarFieldDataBridge } from "./scalar-field-bridge";
import {
  SCALAR_FIELD_VALUES_PER_CHUNK,
  SCALAR_FIELD_VIEW_PROTOCOL,
  type ScalarFieldDescriptor,
  scalarFieldDescriptorSchema,
} from "./scalar-field-protocol";
import {
  initialScalarFieldSessionState,
  ScalarFieldDataSession,
  type ScalarFieldSessionState,
  scalarFieldSessionReducer,
} from "./scalar-field-session";

const RUN_ID = "00000000-0000-4000-8000-000000000001";
const OTHER_RUN_ID = "00000000-0000-4000-8000-000000000002";
const MODEL_DIGEST = "sha256:0123456789abcdef";
const PLAN_KEY = "a".repeat(64);

function result(runId = RUN_ID): SpatialRunResult {
  const cellsPerAxis = 64;
  const axis = cellsPerAxis + 1;
  return spatialRunResultSchema.parse({
    protocol: BRIDGE_PROTOCOL,
    runId,
    digest: MODEL_DIGEST,
    plan: {
      protocol: BRIDGE_PROTOCOL,
      key: PLAN_KEY,
      modelDigest: MODEL_DIGEST,
      realizationRevision: 1,
      requirements: { spatialDimension: 2, scalarType: "f64", vectorLayout: "replicated" },
      discretization: {
        method: "finite-element",
        space: "continuous-lagrange",
        order: 1,
        mesh: "generated-cartesian",
        cellsPerAxis,
        cellCount: cellsPerAxis ** 2,
        quadrature: "gauss-legendre",
        pointsPerAxis: 2,
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
    field: { location: "vertex", valueCount: axis ** 2, minimum: 0, maximum: 1 },
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

function descriptor(context = result()): ScalarFieldDescriptor {
  return scalarFieldDescriptorSchema.parse({
    protocol: SCALAR_FIELD_VIEW_PROTOCOL,
    modelDigest: context.digest,
    runId: context.runId,
    planKey: context.plan.key,
    field: {
      id: "Field:solution",
      name: "solution",
      dimension: "1",
      coherentSiUnit: "1",
      scalarType: "f64",
      location: "vertex",
      valueCount: 65 ** 2,
      minimum: 0,
      maximum: 1,
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
      logicalShape: [65, 65],
      order: "row-major-last-axis-fastest",
    },
    transport: {
      kind: "explicit-owned-host-copy",
      encoding: "f64-le",
      valuesPerChunk: SCALAR_FIELD_VALUES_PER_CHUNK,
      chunkCount: 2,
    },
  });
}

function streamingState(): ScalarFieldSessionState {
  const context = result();
  let state = initialScalarFieldSessionState(context);
  state = scalarFieldSessionReducer(state, {
    type: "open-started",
    requestId: 1,
    result: context,
  });
  return scalarFieldSessionReducer(state, {
    type: "open-succeeded",
    requestId: 1,
    result: context,
    descriptor: descriptor(context),
  });
}

function startChunk(
  state: ScalarFieldSessionState,
  requestId: number,
  chunkIndex: number,
): ScalarFieldSessionState {
  if (state.kind !== "streaming") throw new Error("test requires a streaming state");
  return scalarFieldSessionReducer(state, {
    type: "chunk-started",
    requestId,
    descriptor: state.descriptor,
    chunkIndex,
  });
}

function firstChunk(): Float64Array {
  const values = new Float64Array(SCALAR_FIELD_VALUES_PER_CHUNK);
  values.fill(0.25);
  values[0] = 0;
  return values;
}

function finalChunk(): Float64Array {
  const values = new Float64Array(129);
  values.fill(0.75);
  values[128] = 1;
  return values;
}

describe("scalar-field session reducer", () => {
  it("keeps partial chunks outside the public state and joins only once at readiness", () => {
    let state = startChunk(streamingState(), 2, 0);
    state = scalarFieldSessionReducer(state, {
      type: "chunk-succeeded",
      requestId: 2,
      chunkIndex: 0,
      values: firstChunk(),
    });
    expect(state.kind).toBe("streaming");
    expect("values" in state).toBe(false);
    expect(Object.keys(state)).not.toContain("bufferedChunks");

    state = startChunk(state, 3, 1);
    state = scalarFieldSessionReducer(state, {
      type: "chunk-succeeded",
      requestId: 3,
      chunkIndex: 1,
      values: finalChunk(),
    });
    expect(state.kind).toBe("ready");
    if (state.kind === "ready") {
      expect(state.values).toBeInstanceOf(Float64Array);
      expect(state.values).toHaveLength(65 ** 2);
      expect(state.values[0]).toBe(0);
      expect(state.values.at(-1)).toBe(1);
    }
  });

  it("fails closed for skipped, duplicate, and concurrent chunk requests", () => {
    const skipped = startChunk(streamingState(), 2, 1);
    expect(skipped).toMatchObject({
      kind: "failed",
      failure: { code: "chunk-order-mismatch" },
    });

    const inFlight = startChunk(streamingState(), 2, 0);
    const concurrent = startChunk(inFlight, 3, 0);
    expect(concurrent).toMatchObject({
      kind: "failed",
      failure: { code: "chunk-order-mismatch" },
    });

    const wrongResponse = scalarFieldSessionReducer(inFlight, {
      type: "chunk-succeeded",
      requestId: 2,
      chunkIndex: 1,
      values: firstChunk(),
    });
    expect(wrongResponse).toMatchObject({
      kind: "failed",
      failure: { code: "chunk-order-mismatch" },
    });

    const completedFirst = scalarFieldSessionReducer(inFlight, {
      type: "chunk-succeeded",
      requestId: 2,
      chunkIndex: 0,
      values: firstChunk(),
    });
    const duplicate = scalarFieldSessionReducer(completedFirst, {
      type: "chunk-succeeded",
      requestId: 2,
      chunkIndex: 0,
      values: firstChunk(),
    });
    expect(duplicate).toMatchObject({
      kind: "failed",
      failure: { code: "chunk-order-mismatch" },
    });
  });

  it("fails closed for foreign descriptors, short or long chunks, and non-finite values", () => {
    const context = result();
    const foreign = scalarFieldSessionReducer(streamingState(), {
      type: "chunk-started",
      requestId: 2,
      descriptor: {
        ...descriptor(context),
        runId: OTHER_RUN_ID,
      },
      chunkIndex: 0,
    });
    expect(foreign).toMatchObject({
      kind: "failed",
      failure: { code: "descriptor-mismatch" },
    });

    for (const values of [
      new Float64Array(SCALAR_FIELD_VALUES_PER_CHUNK - 1),
      new Float64Array(SCALAR_FIELD_VALUES_PER_CHUNK + 1),
    ]) {
      const invalidSize = scalarFieldSessionReducer(startChunk(streamingState(), 2, 0), {
        type: "chunk-succeeded",
        requestId: 2,
        chunkIndex: 0,
        values,
      });
      expect(invalidSize).toMatchObject({
        kind: "failed",
        failure: { code: "chunk-size-mismatch" },
      });
    }

    const nonfiniteValues = firstChunk();
    nonfiniteValues[12] = Number.NaN;
    const nonfinite = scalarFieldSessionReducer(startChunk(streamingState(), 2, 0), {
      type: "chunk-succeeded",
      requestId: 2,
      chunkIndex: 0,
      values: nonfiniteValues,
    });
    expect(nonfinite).toMatchObject({
      kind: "failed",
      failure: { code: "chunk-nonfinite" },
    });
  });

  it("drops every partial chunk when final extrema differ from the accepted summary", () => {
    let state = startChunk(streamingState(), 2, 0);
    const first = new Float64Array(SCALAR_FIELD_VALUES_PER_CHUNK);
    first.fill(0.25);
    state = scalarFieldSessionReducer(state, {
      type: "chunk-succeeded",
      requestId: 2,
      chunkIndex: 0,
      values: first,
    });
    state = startChunk(state, 3, 1);
    const last = new Float64Array(129);
    last.fill(0.75);
    state = scalarFieldSessionReducer(state, {
      type: "chunk-succeeded",
      requestId: 3,
      chunkIndex: 1,
      values: last,
    });
    expect(state).toMatchObject({
      kind: "failed",
      failure: { code: "field-range-mismatch" },
    });
    expect("values" in state).toBe(false);
  });

  it("ignores stale responses after the accepted context changes", () => {
    const inFlight = startChunk(streamingState(), 2, 0);
    const changed = scalarFieldSessionReducer(inFlight, {
      type: "context-changed",
      result: result(OTHER_RUN_ID),
    });
    const stale = scalarFieldSessionReducer(changed, {
      type: "chunk-succeeded",
      requestId: 2,
      chunkIndex: 0,
      values: firstChunk(),
    });
    expect(stale).toBe(changed);
    expect(stale).toMatchObject({ kind: "idle", context: { runId: OTHER_RUN_ID } });
  });
});

describe("ScalarFieldDataSession loader", () => {
  it("issues exactly one chunk request at a time in descriptor order", async () => {
    const context = result();
    const fieldDescriptor = descriptor(context);
    let inFlight = 0;
    let maximumInFlight = 0;
    const requested: number[] = [];
    const bridge: ScalarFieldDataBridge = {
      async open() {
        return { ok: true, value: fieldDescriptor };
      },
      async readChunk(_descriptor, chunkIndex) {
        requested.push(chunkIndex);
        inFlight += 1;
        maximumInFlight = Math.max(maximumInFlight, inFlight);
        await Promise.resolve();
        inFlight -= 1;
        return {
          ok: true,
          value: chunkIndex === 0 ? firstChunk() : finalChunk(),
        };
      },
    };
    const observed: ScalarFieldSessionState[] = [];
    const session = new ScalarFieldDataSession(bridge, (state) => observed.push(state));
    const terminal = await session.load(context);
    expect(terminal.kind).toBe("ready");
    expect(requested).toEqual([0, 1]);
    expect(maximumInFlight).toBe(1);
    expect(observed.filter((state) => state.kind === "ready")).toHaveLength(1);
  });

  it("ignores a late open after a new context replaces it", async () => {
    const firstContext = result();
    const secondContext = result(OTHER_RUN_ID);
    let resolveFirst: ((value: ScalarFieldBridgeResult<ScalarFieldDescriptor>) => void) | undefined;
    const firstOpen = new Promise<ScalarFieldBridgeResult<ScalarFieldDescriptor>>((resolve) => {
      resolveFirst = resolve;
    });
    const bridge: ScalarFieldDataBridge = {
      open(context) {
        return context.runId === RUN_ID
          ? firstOpen
          : Promise.resolve({ ok: true, value: descriptor(secondContext) });
      },
      async readChunk(_fieldDescriptor, chunkIndex) {
        return {
          ok: true,
          value: chunkIndex === 0 ? firstChunk() : finalChunk(),
        };
      },
    };
    const session = new ScalarFieldDataSession(bridge);
    const staleLoad = session.load(firstContext);
    const currentLoad = session.load(secondContext);
    resolveFirst?.({ ok: true, value: descriptor(firstContext) });
    await staleLoad;
    const terminal = await currentLoad;
    expect(terminal).toMatchObject({ kind: "ready", context: { runId: OTHER_RUN_ID } });
  });

  it("maps a bridge rejection to a closed terminal failure", async () => {
    const bridge: ScalarFieldDataBridge = {
      async open() {
        return {
          ok: false,
          failure: { code: "bridge-rejected", message: "unavailable" },
        };
      },
      async readChunk() {
        throw new Error("must not be called");
      },
    };
    const terminal = await new ScalarFieldDataSession(bridge).load(result());
    expect(terminal).toMatchObject({
      kind: "failed",
      failure: {
        code: "open-rejected",
        cause: { code: "bridge-rejected" },
      },
    });
  });
});
