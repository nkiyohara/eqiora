import { describe, expect, it } from "vitest";
import {
  CYLINDER_DEMO_ID,
  CYLINDER_DEMO_PROTOCOL,
  type CylinderDemoResult,
  cylinderDemoResultSchema,
} from "./cylinder-demo-protocol";
import { CylinderDemoSession } from "./cylinder-demo-session";
import { BRIDGE_PROTOCOL } from "./protocol";
import type { UnstructuredFieldDataBridge } from "./unstructured-field-bridge";
import {
  UNSTRUCTURED_FIELD_ITEMS_PER_CHUNK,
  UNSTRUCTURED_FIELD_VIEW_PROTOCOL,
  type UnstructuredFieldContext,
  type UnstructuredFieldDescriptor,
  unstructuredFieldContextSchema,
  unstructuredFieldDescriptorSchema,
} from "./unstructured-field-protocol";

const coordinates = new Float64Array([0, 0, 1, 0, 0, 1]);
const triangles = new Uint32Array([0, 1, 2]);
const values = new Float64Array([0, 1, 2]);

function context(): UnstructuredFieldContext {
  return unstructuredFieldContextSchema.parse({
    modelDigest: "0".repeat(64),
    semanticRevision: "132",
    realizationDigest: "1".repeat(64),
    runDigest: "2".repeat(64),
    snapshotDigest: "3".repeat(64),
    meshDigest: "4".repeat(64),
    field: {
      id: "Field:pressure",
      dimension: "M·L^-1·T^-2",
      coherentSiUnit: "kg·m^-1·s^-2",
      valueCount: 3,
      minimum: 0,
      maximum: 2,
    },
    domain: {
      id: "Domain:fluid",
      boundsM: [
        [0, 1],
        [0, 1],
      ],
    },
    mesh: {
      kind: "affine-triangle-2d",
      vertexCount: 3,
      triangleCount: 1,
    },
  });
}

function descriptor(): UnstructuredFieldDescriptor {
  const accepted = context();
  return unstructuredFieldDescriptorSchema.parse({
    protocol: UNSTRUCTURED_FIELD_VIEW_PROTOCOL,
    modelDigest: accepted.modelDigest,
    semanticRevision: accepted.semanticRevision,
    realizationDigest: accepted.realizationDigest,
    runDigest: accepted.runDigest,
    snapshotDigest: accepted.snapshotDigest,
    meshDigest: accepted.meshDigest,
    field: {
      ...accepted.field,
      scalarType: "f64",
      location: "vertex",
    },
    domain: accepted.domain,
    mesh: accepted.mesh,
    transport: {
      kind: "explicit-owned-host-copy",
      coordinates: {
        encoding: "f64-le",
        components: 2,
        itemCount: 3,
        itemsPerChunk: UNSTRUCTURED_FIELD_ITEMS_PER_CHUNK,
        chunkCount: 1,
      },
      triangles: {
        encoding: "u32-le",
        components: 3,
        itemCount: 1,
        itemsPerChunk: UNSTRUCTURED_FIELD_ITEMS_PER_CHUNK,
        chunkCount: 1,
      },
      values: {
        encoding: "f64-le",
        components: 1,
        itemCount: 3,
        itemsPerChunk: UNSTRUCTURED_FIELD_ITEMS_PER_CHUNK,
        chunkCount: 1,
      },
    },
  });
}

function fieldBridge(): UnstructuredFieldDataBridge {
  return {
    async open() {
      return { ok: true, value: descriptor() };
    },
    async readChunk(_descriptor, stream) {
      switch (stream) {
        case "coordinates":
          return { ok: true, value: { stream, values: coordinates } };
        case "triangles":
          return { ok: true, value: { stream, values: triangles } };
        case "values":
          return { ok: true, value: { stream, values } };
      }
    },
  };
}

function acceptedResult(): CylinderDemoResult {
  return cylinderDemoResultSchema.parse({
    protocol: CYLINDER_DEMO_PROTOCOL,
    exampleId: CYLINDER_DEMO_ID,
    context: context(),
    geometry: {
      exactSourceDigest: "5".repeat(64),
      realizedGeometryDigest: "6".repeat(64),
      requestedMaxBoundaryErrorM: 1e-4,
      boundaryEvaluationAllowanceM: 1e-8,
      boundaryErrorBoundM: 8e-5,
      circleSegments: 50,
    },
    cylinderReaction: {
      convention: "constraint-force-on-fluid",
      forceOnFluidNM: [2, 3],
    },
    fluxBalance: {
      convention: "physical-parent-outward",
      inletM2S: -1,
      outletM2S: 0.9,
      netM2S: -1 + 0.9,
    },
    momentumBalance: {
      constrainedReactionNM: [2, 3],
      integratedBodyForceNM: [0, 0],
      integratedTractionNM: [-2, -3],
      closureNM: [0, 0],
    },
    solver: {
      algorithm: "sparse-lu",
      preconditioner: "identity",
      reduction: "fast",
      relativeTolerance: 1e-6,
      absoluteTolerance: 1e-13,
      completedIterations: 1,
      residualTarget: 1e-8,
      trueResidualNorm: 1e-9,
      continuityResidualNorm: 2e-10,
    },
  });
}

describe("exact-cylinder Studio protocol", () => {
  it("accepts the closed result and rejects relational evidence drift", () => {
    const accepted = acceptedResult();
    expect(cylinderDemoResultSchema.safeParse(accepted).success).toBe(true);

    expect(
      cylinderDemoResultSchema.safeParse({
        ...accepted,
        fluxBalance: { ...accepted.fluxBalance, netM2S: 0 },
      }).success,
    ).toBe(false);
    expect(
      cylinderDemoResultSchema.safeParse({
        ...accepted,
        momentumBalance: { ...accepted.momentumBalance, closureNM: [1, 0] },
      }).success,
    ).toBe(false);
    expect(
      cylinderDemoResultSchema.safeParse({
        ...accepted,
        solver: { ...accepted.solver, trueResidualNorm: 2e-8 },
      }).success,
    ).toBe(false);
    expect(
      cylinderDemoResultSchema.safeParse({
        ...accepted,
        context: {
          ...accepted.context,
          field: { ...accepted.context.field, coherentSiUnit: "Pa" },
        },
      }).success,
    ).toBe(false);
  });
});

describe("exact-cylinder Studio session", () => {
  it("publishes ready state only after command evidence and all field streams agree", async () => {
    const transitions: string[] = [];
    const result = acceptedResult();
    const session = new CylinderDemoSession(
      {
        async runCylinderDemo(request) {
          expect(request).toEqual({ protocol: BRIDGE_PROTOCOL });
          return { protocol: BRIDGE_PROTOCOL, result, diagnostics: [] };
        },
      },
      fieldBridge(),
      (state) => transitions.push(state.kind),
    );

    const terminal = await session.run();
    expect(transitions).toEqual([
      "solving",
      "loading-field",
      "loading-field",
      "loading-field",
      "loading-field",
      "loading-field",
      "loading-field",
      "loading-field",
      "ready",
    ]);
    expect(terminal).toMatchObject({
      kind: "ready",
      result: {
        protocol: CYLINDER_DEMO_PROTOCOL,
        context: { snapshotDigest: result.context.snapshotDigest },
      },
    });
    if (terminal.kind !== "ready") throw new Error("expected ready cylinder demo");
    expect(terminal.field.coordinates).toEqual(coordinates);
    expect(terminal.field.triangles).toEqual(triangles);
    expect(terminal.field.values).toEqual(values);
  });

  it("fails closed when the native command or exact field binding is rejected", async () => {
    const rejectedCommand = new CylinderDemoSession(
      {
        async runCylinderDemo() {
          return {
            protocol: BRIDGE_PROTOCOL,
            result: null,
            diagnostics: [
              {
                source: "studio",
                severity: "error",
                code: "studio.cylinder.native_required",
                message: "Native runtime required.",
                graphPath: [],
                span: null,
                patch: null,
              },
            ],
          };
        },
      },
      fieldBridge(),
    );
    expect(await rejectedCommand.run()).toMatchObject({
      kind: "failed",
      result: null,
      message: "Native runtime required.",
    });

    const foreignField: UnstructuredFieldDataBridge = {
      ...fieldBridge(),
      async open() {
        return {
          ok: true,
          value: { ...descriptor(), snapshotDigest: "7".repeat(64) },
        };
      },
    };
    const result = acceptedResult();
    const rejectedField = new CylinderDemoSession(
      {
        async runCylinderDemo() {
          return { protocol: BRIDGE_PROTOCOL, result, diagnostics: [] };
        },
      },
      foreignField,
    );
    expect(await rejectedField.run()).toMatchObject({
      kind: "failed",
      result: { context: { snapshotDigest: result.context.snapshotDigest } },
      message: "Unstructured descriptor differs from the exact accepted context.",
    });
  });

  it("discards a superseded native command response before field publication", async () => {
    let commandCalls = 0;
    let releaseFirst: () => void = () => {
      throw new Error("first command was not started");
    };
    const transitions: string[] = [];
    const result = acceptedResult();
    const session = new CylinderDemoSession(
      {
        async runCylinderDemo() {
          commandCalls += 1;
          if (commandCalls === 1) {
            await new Promise<void>((resolve) => {
              releaseFirst = resolve;
            });
            return { protocol: BRIDGE_PROTOCOL, result: null, diagnostics: [] };
          }
          return { protocol: BRIDGE_PROTOCOL, result, diagnostics: [] };
        },
      },
      fieldBridge(),
      (state) => transitions.push(state.kind),
    );

    const superseded = session.run();
    const current = await session.run();
    expect(current.kind).toBe("ready");

    releaseFirst();
    expect(await superseded).toBe(current);
    expect(session.state).toBe(current);
    expect(transitions).not.toContain("failed");
  });
});
