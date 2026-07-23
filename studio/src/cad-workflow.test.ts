import { describe, expect, it } from "vitest";
import { CAD_VIEW_PROTOCOL, type CadProjection, cadProjectionSchema } from "./cad-protocol";
import {
  cadSelectionReducer,
  cadSelectionRequest,
  initialCadSelectionState,
  resolveCadSelection,
} from "./cad-workflow";

const digest = (digit: string) => digit.repeat(64);
const roles = [
  [0, "lower"],
  [0, "upper"],
  [1, "lower"],
  [1, "upper"],
  [2, "lower"],
  [2, "upper"],
] as const;
const faceTriangles = [
  [
    [0, 2, 6],
    [0, 6, 4],
  ],
  [
    [1, 5, 7],
    [1, 7, 3],
  ],
  [
    [0, 4, 5],
    [0, 5, 1],
  ],
  [
    [2, 3, 7],
    [2, 7, 6],
  ],
  [
    [0, 1, 3],
    [0, 3, 2],
  ],
  [
    [4, 6, 7],
    [4, 7, 5],
  ],
] as const;

function projection(): CadProjection {
  const boundaryDomains = roles.map(([axis, side]) => `Domain:${axis}-${side}`);
  return {
    protocol: CAD_VIEW_PROTOCOL,
    planKey: digest("1"),
    modelDigest: digest("2"),
    geometryDigest: digest("3"),
    meshDigest: digest("4"),
    design: {
      sourceUnit: "millimetre",
      importedStockBoundsM: [
        [-1, 1],
        [-1, 1],
        [-1, 1],
      ],
      sketch: {
        xBoundsM: [-0.5, 0.5],
        yBoundsM: [-0.5, 0.5],
        planeZM: -0.5,
        remainingDegreesOfFreedom: 0,
      },
      extrusion: { direction: "positive-z", depthM: 1 },
      boolean: "intersection",
      resultBoundsM: [
        [-0.5, 0.5],
        [-0.5, 0.5],
        [-0.5, 0.5],
      ],
    },
    build: {
      adapter: "eqiora.truck",
      adapterVersion: "1",
      kernel: "truck",
      kernelVersion: "0.6.0",
      repair: "none",
      importedStock: { solidCount: 1, closedShellCount: 1, planarFaceCount: 6 },
      extrudedTool: { solidCount: 1, closedShellCount: 1, planarFaceCount: 6 },
      intersection: { solidCount: 1, closedShellCount: 1, planarFaceCount: 6 },
    },
    verticesM: [
      [-0.5, -0.5, -0.5],
      [0.5, -0.5, -0.5],
      [-0.5, 0.5, -0.5],
      [0.5, 0.5, -0.5],
      [-0.5, -0.5, 0.5],
      [0.5, -0.5, 0.5],
      [-0.5, 0.5, 0.5],
      [0.5, 0.5, 0.5],
    ],
    triangles: boundaryDomains.flatMap((domainId, face) =>
      (faceTriangles[face] ?? []).map((vertexIndices) => ({
        domainId,
        vertexIndices: [...vertexIndices],
      })),
    ),
    entities: [
      {
        domainId: "Domain:body",
        name: "body",
        kind: "body",
        parentDomainId: null,
        axis: null,
        side: null,
        meshEntityCount: 6,
        relationIds: [],
        portIds: [],
      },
      ...roles.map(([axis, side]) => ({
        domainId: `Domain:${axis}-${side}`,
        name: `${axis}_${side}`,
        kind: "boundary" as const,
        parentDomainId: "Domain:body",
        axis,
        side,
        meshEntityCount: 2,
        relationIds: side === "upper" ? ["Relation:wall"] : [],
        portIds: axis === 0 && side === "upper" ? ["Port:mechanical"] : [],
      })),
    ],
  };
}

describe("CAD projection boundary", () => {
  it("accepts one coherent exact projection and rejects unowned fields", () => {
    const value = projection();
    expect(cadProjectionSchema.parse(value)).toEqual(value);
    expect(cadProjectionSchema.safeParse({ ...value, kernelFace: 17 }).success).toBe(false);
  });

  it("rejects renderer Domains not present as semantic boundaries", () => {
    const value = projection();
    const first = value.triangles[0];
    expect(first).toBeDefined();
    const invalid = {
      ...value,
      triangles: [{ ...first, domainId: "Domain:body" }, ...value.triangles.slice(1)],
    };
    expect(cadProjectionSchema.safeParse(invalid).success).toBe(false);
  });

  it("uses one exact request for both table and viewport modalities", () => {
    const value = projection();
    const boundary = value.entities[1];
    const triangle = value.triangles[0];
    expect(boundary).toBeDefined();
    expect(triangle).toBeDefined();
    const tableRequest = cadSelectionRequest(value, boundary?.domainId ?? "");
    const viewportRequest = cadSelectionRequest(value, triangle?.domainId ?? "");
    expect(viewportRequest).toEqual(tableRequest);
    expect(resolveCadSelection(value, tableRequest)).toEqual({
      kind: "selected",
      entity: boundary,
    });
  });

  it("does not resolve a request against a different exact plan", () => {
    const value = projection();
    const request = cadSelectionRequest(value, value.entities[1]?.domainId ?? "");
    expect(request).not.toBeNull();
    expect(resolveCadSelection({ ...value, planKey: digest("8") }, request)).toEqual({
      kind: "stale",
    });
  });

  it("admits only the matching native response into application selection state", () => {
    const value = projection();
    const entity = value.entities[1];
    const request = cadSelectionRequest(value, entity?.domainId ?? "");
    expect(request).not.toBeNull();
    if (request === null || entity === undefined) return;

    const contextual = cadSelectionReducer(initialCadSelectionState(), {
      type: "context-changed",
      projection: value,
    });
    const resolving = cadSelectionReducer(contextual, {
      type: "selection-started",
      requestId: 7,
      request,
    });
    const stale = cadSelectionReducer(resolving, {
      type: "selection-finished",
      requestId: 6,
      result: {
        protocol: CAD_VIEW_PROTOCOL,
        modelDigest: value.modelDigest,
        planKey: value.planKey,
        geometryDigest: value.geometryDigest,
        domainId: entity.domainId,
        entity,
      },
    });
    expect(stale).toBe(resolving);

    const mismatched = cadSelectionReducer(resolving, {
      type: "selection-finished",
      requestId: 7,
      result: {
        protocol: CAD_VIEW_PROTOCOL,
        modelDigest: value.modelDigest,
        planKey: value.planKey,
        geometryDigest: digest("9"),
        domainId: entity.domainId,
        entity,
      },
    });
    expect(mismatched.accepted).toBeNull();
    expect(mismatched.status).toEqual({ kind: "failed" });

    const accepted = cadSelectionReducer(resolving, {
      type: "selection-finished",
      requestId: 7,
      result: {
        protocol: CAD_VIEW_PROTOCOL,
        modelDigest: value.modelDigest,
        planKey: value.planKey,
        geometryDigest: value.geometryDigest,
        domainId: entity.domainId,
        entity,
      },
    });
    expect(accepted.accepted).toEqual(request);
    expect(accepted.status).toEqual({ kind: "idle" });
  });
});
