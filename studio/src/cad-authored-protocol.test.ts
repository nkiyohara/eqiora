import { describe, expect, it } from "vitest";
import hostileCorpus from "../../verify/interfaces/studio-python-cad-round-trip/models/hostile.json";
import {
  CAD_AUTHORED_EXPORT_FILE_NAME,
  CAD_AUTHORED_EXPORT_PROTOCOL,
  CAD_AUTHORED_PROTOCOL,
  type CadAuthoredProjection,
  cadAuthoredBuildRequestSchema,
  cadAuthoredExportRenderSchema,
  cadAuthoredExportRequestSchema,
  cadAuthoredExportSaveSchema,
  cadAuthoredProjectionSchema,
  cadAuthoredSelectionRequestSchema,
  cadAuthoredSelectionResultSchema,
} from "./cad-authored-protocol";

/** Deterministic opaque token: ASCII bytes as even-length lowercase hex. */
function asciiHex(text: string): string {
  return [...text].map((char) => char.codePointAt(0)?.toString(16).padStart(2, "0")).join("");
}

// Every scientific constant below — scalars, digests, canonical byte counts,
// volumes, surface areas, per-face areas, and boundary loop counts — is copied
// verbatim from the accepted frozen cases
// `crates/eqiora-geometry/tests/cad_authored_circular_through_cut.rs` (v2) and
// `crates/eqiora-geometry/tests/cad_authored_rectangle_extrusion.rs` (v1).
// Canonical graph bytes and face handles stay opaque to the client, so the
// fixtures use schema-shaped placeholder tokens for them, never real bytes.
const V2_DIGEST = "00acb9494fc7dea8f1f2500d1316cb3315130a965a24179b3eb1b10345058b47";
const V2_CANONICAL_BYTE_COUNT = 1292;
const V2_FACES = [
  ["start-cap", 0.0037989380701702533, 2],
  ["end-cap", 0.0037989380701702533, 2],
  ["profile-x-lower", 0.001, 1],
  ["profile-x-upper", 0.001, 1],
  ["profile-y-lower", 0.0016, 1],
  ["profile-y-upper", 0.0016, 1],
  ["cut-wall", 0.001005309649148734, 2],
] as const;

const V1_DIGEST = "919545f70118840c04da9715829deb2da947460a51311ebabec6a34038c66f36";
const V1_CANONICAL_BYTE_COUNT = 731;
const V1_FACES = [
  ["start-cap", 15, 1],
  ["end-cap", 15, 1],
  ["profile-x-lower", 12, 1],
  ["profile-x-upper", 12, 1],
  ["profile-y-lower", 20, 1],
  ["profile-y-upper", 20, 1],
] as const;

function extrusionProjectionFixture(): CadAuthoredProjection {
  const corners: [number, number, number][] = [];
  for (const z of [0.5, 4.5]) {
    for (const [x, y] of [
      [-2, -1],
      [3, -1],
      [3, 2],
      [-2, 2],
    ] as const) {
      corners.push([x, y, z]);
    }
  }
  const handle = (key: string) => asciiHex(`handle:${key}`);
  return cadAuthoredProjectionSchema.parse({
    protocol: CAD_AUTHORED_PROTOCOL,
    graphDigest: V1_DIGEST,
    canonicalGraphHex: "ab".repeat(V1_CANONICAL_BYTE_COUNT),
    canonicalByteCount: V1_CANONICAL_BYTE_COUNT,
    history: [
      { kind: "sketch-plane", id: "sketch-plane", plane: "xy", zM: 0.5 },
      {
        kind: "rectangle-profile",
        id: "rectangle-profile",
        sketchPlane: "sketch-plane",
        constraint: "closed-by-construction",
        xBoundsM: [-2, 3],
        yBoundsM: [-1, 2],
      },
      { kind: "closed-face", id: "profile-face", profile: "rectangle-profile", regionCount: 1 },
      {
        kind: "positive-z-extrusion",
        id: "positive-z-extrusion",
        face: "profile-face",
        depthM: 4,
        repair: "none",
      },
    ],
    tolerances: {
      requestedModelingToleranceM: 1e-9,
      requestedBooleanToleranceM: null,
      repair: "none",
    },
    observations: {
      boundsM: [
        [-2, 3],
        [-1, 2],
        [0.5, 4.5],
      ],
      outerVerticesM: corners,
      vertexCount: 8,
      edgeCount: 12,
      faceCount: 6,
      closedShellCount: 1,
      bodyCount: 1,
      genus: 0,
      volumeM3: 60,
      surfaceAreaM2: 94,
    },
    faces: V1_FACES.map(([provenanceKey, areaM2, boundaryLoopCount]) => ({
      provenanceKey,
      handleHex: handle(provenanceKey),
      areaM2,
      boundaryLoopCount,
      centroidM: null,
      outwardUnitNormal: null,
      verticesM: null,
    })),
    build: {
      graphDigest: V1_DIGEST,
      providerProfile: "eqiora.cad.analytic-rectangle-extrusion-v1",
      requestedModelingToleranceM: 1e-9,
      requestedBooleanToleranceM: null,
      effectiveBooleanToleranceM: null,
      maximumPositionDiscrepancyM: 0,
      maximumAreaDiscrepancyM2: 0,
      maximumVolumeDiscrepancyM3: 0,
      repair: "none",
      lineage: {
        retainedUnchanged: [],
        retainedModified: [],
        created: V1_FACES.map(([key]) => ({ provenanceKey: key, handleHex: handle(key) })),
        deleted: [],
        split: [],
        merged: [],
      },
    },
  });
}

function cutProjectionFixture(): CadAuthoredProjection {
  const corners: [number, number, number][] = [];
  for (const z of [0, 0.02]) {
    for (const [x, y] of [
      [-0.04, -0.025],
      [0.04, -0.025],
      [0.04, 0.025],
      [-0.04, 0.025],
    ] as const) {
      corners.push([x, y, z]);
    }
  }
  const handle = (key: string) => asciiHex(`handle:${key}`);
  return cadAuthoredProjectionSchema.parse({
    protocol: CAD_AUTHORED_PROTOCOL,
    graphDigest: V2_DIGEST,
    canonicalGraphHex: "ab".repeat(V2_CANONICAL_BYTE_COUNT),
    canonicalByteCount: V2_CANONICAL_BYTE_COUNT,
    history: [
      { kind: "sketch-plane", id: "sketch-plane", plane: "xy", zM: 0 },
      {
        kind: "rectangle-profile",
        id: "rectangle-profile",
        sketchPlane: "sketch-plane",
        constraint: "closed-by-construction",
        xBoundsM: [-0.04, 0.04],
        yBoundsM: [-0.025, 0.025],
      },
      { kind: "closed-face", id: "profile-face", profile: "rectangle-profile", regionCount: 1 },
      {
        kind: "positive-z-extrusion",
        id: "positive-z-extrusion",
        face: "profile-face",
        depthM: 0.02,
        repair: "none",
      },
      { kind: "cut-sketch-plane", id: "cut-sketch-plane", face: "end-cap" },
      {
        kind: "circle-profile",
        id: "circle-profile",
        sketchPlane: "cut-sketch-plane",
        constraint: "closed-by-construction",
        centerM: [0.02, 0],
        radiusM: 0.008,
      },
      {
        kind: "closed-cut-face",
        id: "cut-profile-face",
        profile: "circle-profile",
        regionCount: 1,
      },
      {
        kind: "circular-through-cut",
        id: "circular-through-cut",
        target: "positive-z-extrusion",
        toolFace: "cut-profile-face",
        requestedBooleanToleranceM: 1e-9,
        repair: "none",
      },
    ],
    tolerances: {
      requestedModelingToleranceM: 1e-10,
      requestedBooleanToleranceM: 1e-9,
      repair: "none",
    },
    observations: {
      boundsM: [
        [-0.04, 0.04],
        [-0.025, 0.025],
        [0, 0.02],
      ],
      outerVerticesM: corners,
      vertexCount: null,
      edgeCount: null,
      faceCount: 7,
      closedShellCount: 1,
      bodyCount: 1,
      genus: 1,
      volumeM3: 7.597876140340507e-5,
      surfaceAreaM2: 0.01380318578948924,
    },
    faces: V2_FACES.map(([provenanceKey, areaM2, boundaryLoopCount]) => ({
      provenanceKey,
      handleHex: handle(provenanceKey),
      areaM2,
      boundaryLoopCount,
      centroidM: null,
      outwardUnitNormal: null,
      verticesM: null,
    })),
    build: {
      graphDigest: V2_DIGEST,
      providerProfile: "eqiora.cad.analytic-circular-through-cut-v1",
      requestedModelingToleranceM: 1e-10,
      requestedBooleanToleranceM: 1e-9,
      effectiveBooleanToleranceM: 1e-9,
      maximumPositionDiscrepancyM: 0,
      maximumAreaDiscrepancyM2: 0,
      maximumVolumeDiscrepancyM3: 0,
      repair: "none",
      lineage: {
        retainedUnchanged: [
          "profile-x-lower",
          "profile-x-upper",
          "profile-y-lower",
          "profile-y-upper",
        ].map((key) => ({ provenanceKey: key, handleHex: handle(key) })),
        retainedModified: ["start-cap", "end-cap"].map((key) => ({
          provenanceKey: key,
          handleHex: handle(key),
        })),
        created: [{ provenanceKey: "cut-wall", handleHex: handle("cut-wall") }],
        deleted: [],
        split: [],
        merged: [],
      },
    },
  });
}

type Mutable = ReturnType<typeof structuredClone<CadAuthoredProjection>>;

function mutated(change: (projection: Mutable) => void): unknown {
  const projection = structuredClone(cutProjectionFixture());
  change(projection);
  return projection;
}

describe("authored-CAD projection strict decode", () => {
  it("accepts the frozen cut projection and rejects a widened payload", () => {
    expect(cadAuthoredProjectionSchema.safeParse(cutProjectionFixture()).success).toBe(true);

    const widened = { ...cutProjectionFixture(), geometryDigest: V2_DIGEST };
    expect(cadAuthoredProjectionSchema.safeParse(widened).success).toBe(false);
  });

  it("never names the authored graph digest a Geometry identity", () => {
    const wire = JSON.stringify(cutProjectionFixture());
    expect(wire).toContain('"graphDigest"');
    expect(wire).not.toContain("graphDigestSha256");
    expect(wire).not.toContain("geometryDigest");
    expect(wire).not.toContain("meshDigest");
  });

  it("rejects each structural incoherence between history and evidence", () => {
    const rejects = [
      // Opaque byte count must agree with the opaque hex payload.
      mutated((p) => {
        p.canonicalByteCount = V2_CANONICAL_BYTE_COUNT - 1;
      }),
      // Cut history without its seventh face, genus, or Boolean tolerance.
      mutated((p) => {
        p.observations.genus = 0;
      }),
      mutated((p) => {
        p.observations.faceCount = 6;
      }),
      mutated((p) => {
        p.tolerances.requestedBooleanToleranceM = null;
      }),
      mutated((p) => {
        p.build.providerProfile = "eqiora.cad.analytic-rectangle-extrusion-v1";
      }),
      // The receipt must realize this exact authored graph identity.
      mutated((p) => {
        p.build.graphDigest = `1${V2_DIGEST.slice(1)}`;
      }),
      // Faces must repeat the owner's canonical inventory order exactly.
      mutated((p) => {
        p.faces.reverse();
      }),
      mutated((p) => {
        p.faces.pop();
      }),
      // A cut history is the complete frozen 8-step chain, never a summary.
      mutated((p) => {
        p.history.splice(4, 3);
      }),
      mutated((p) => {
        p.history.pop();
      }),
      // The receipt must echo the projected tolerances field-for-field.
      mutated((p) => {
        p.build.requestedModelingToleranceM = 2e-10;
      }),
      mutated((p) => {
        p.build.requestedBooleanToleranceM = 2e-9;
      }),
      // The accepted cut profile never substitutes the Boolean tolerance.
      mutated((p) => {
        p.build.effectiveBooleanToleranceM = 2e-9;
      }),
      mutated((p) => {
        p.build.effectiveBooleanToleranceM = null;
      }),
      // The history cut operation replays the projected Boolean tolerance.
      mutated((p) => {
        const cut = p.history[7];
        if (cut?.kind !== "circular-through-cut") throw new Error("fixture omitted cut operation");
        cut.requestedBooleanToleranceM = 2e-9;
      }),
      // A lineage member must carry the exact graph-bound handle of its face.
      mutated((p) => {
        const created = p.build.lineage.created[0];
        if (created === undefined) throw new Error("fixture omitted created lineage");
        created.handleHex = asciiHex("handle:foreign");
      }),
      // Opaque tokens are bounded lowercase hex.
      mutated((p) => {
        p.graphDigest = V2_DIGEST.toUpperCase();
      }),
      mutated((p) => {
        const face = p.faces[0];
        if (face === undefined) throw new Error("fixture omitted faces");
        face.handleHex = "abc";
      }),
    ];
    for (const [index, payload] of rejects.entries()) {
      expect(cadAuthoredProjectionSchema.safeParse(payload).success, `mutant ${index}`).toBe(false);
    }
  });

  it("keeps the cut-free receipt's effective Boolean tolerance fail-closed at null", () => {
    expect(cadAuthoredProjectionSchema.safeParse(extrusionProjectionFixture()).success).toBe(true);

    const substituted = structuredClone(extrusionProjectionFixture());
    substituted.build.effectiveBooleanToleranceM = 1e-9;
    expect(cadAuthoredProjectionSchema.safeParse(substituted).success).toBe(false);
  });

  it("represents the τ-only witness: same observations, different graph digest", () => {
    // Witness pattern copied from the accepted frozen case
    // `crates/eqiora-geometry/tests/cad_authored_rectangle_extrusion.rs`:
    // only the requested tolerance changes, so only identity may change.
    const first = cutProjectionFixture();
    const witnessDigest = `ff${V2_DIGEST.slice(2)}`;
    const witness = cadAuthoredProjectionSchema.parse(
      mutated((p) => {
        p.graphDigest = witnessDigest;
        p.build.graphDigest = witnessDigest;
        p.tolerances.requestedModelingToleranceM = 2e-10;
        p.build.requestedModelingToleranceM = 2e-10;
      }),
    );
    expect(witness.observations).toEqual(first.observations);
    expect(witness.graphDigest).not.toBe(first.graphDigest);
  });
});

describe("authored-CAD request schemas", () => {
  const request = {
    protocol: CAD_AUTHORED_PROTOCOL,
    sketch: { xBoundsM: [-0.04, 0.04], yBoundsM: [-0.025, 0.025], planeZM: 0 },
    extrusionDepthM: 0.02,
    requestedModelingToleranceM: 1e-10,
    cut: { centerM: [0.02, 0], radiusM: 0.008, requestedBooleanToleranceM: 1e-9 },
  };

  it("accepts the one closed shape with and without its optional cut", () => {
    expect(cadAuthoredBuildRequestSchema.safeParse(request).success).toBe(true);
    expect(cadAuthoredBuildRequestSchema.safeParse({ ...request, cut: null }).success).toBe(true);
  });

  it("rejects widened, degenerate, or unbounded requests", () => {
    for (const payload of [
      { ...request, operations: [] },
      { ...request, sketch: { ...request.sketch, xBoundsM: [0.04, -0.04] } },
      { ...request, extrusionDepthM: 0 },
      { ...request, requestedModelingToleranceM: -1e-10 },
      { ...request, sketch: { ...request.sketch, planeZM: Number.POSITIVE_INFINITY } },
      { ...request, cut: { ...request.cut, radiusM: Number.NaN } },
      { ...request, cut: { ...request.cut, provider: "truck" } },
    ]) {
      expect(cadAuthoredBuildRequestSchema.safeParse(payload).success).toBe(false);
    }
  });

  it("keeps selection replay closed to digest, opaque graph, and handle", () => {
    const selection = {
      protocol: CAD_AUTHORED_PROTOCOL,
      graphDigest: V2_DIGEST,
      canonicalGraphHex: "ab".repeat(V2_CANONICAL_BYTE_COUNT),
      handleHex: asciiHex("handle:cut-wall"),
    };
    expect(cadAuthoredSelectionRequestSchema.safeParse(selection).success).toBe(true);
    expect(
      cadAuthoredSelectionRequestSchema.safeParse({ ...selection, provenanceKey: "cut-wall" })
        .success,
    ).toBe(false);
    expect(
      cadAuthoredSelectionRequestSchema.safeParse({ ...selection, handleHex: "0z" }).success,
    ).toBe(false);

    const result = {
      protocol: CAD_AUTHORED_PROTOCOL,
      graphDigest: V2_DIGEST,
      provenanceKey: "cut-wall",
      handleHex: asciiHex("handle:cut-wall"),
      areaM2: 0.001005309649148734,
      boundaryLoopCount: 2,
      centroidM: null,
      outwardUnitNormal: null,
    };
    expect(cadAuthoredSelectionResultSchema.safeParse(result).success).toBe(true);
    expect(
      cadAuthoredSelectionResultSchema.safeParse({ ...result, geometryDigest: V2_DIGEST }).success,
    ).toBe(false);
  });
});

describe("authored-CAD Python export wire", () => {
  // The mutation ids and expected dispositions below are consumed from the
  // precommitted hostile corpus
  // `verify/interfaces/studio-python-cad-round-trip/models/hostile.json`.
  // Byte-exact source evidence is native-owner evidence; this suite freezes
  // only the closed client-side structure and bounded rejection rules.
  const request = {
    protocol: CAD_AUTHORED_EXPORT_PROTOCOL,
    canonicalGraphHex: "ab".repeat(V1_CANONICAL_BYTE_COUNT),
    graphDigest: V1_DIGEST,
  };

  // Schema-shaped stand-in for a native rendering: same layout and public
  // import as the frozen fixtures, never claimed byte-identical to them.
  const source =
    "# Generated by Eqiora Studio from an accepted authored CAD graph.\n" +
    "# Length arguments are coherent-SI metres.\n" +
    "import eqiora\n" +
    "\n" +
    "authored_graph = eqiora.geometry.CadAuthoredGraph.rectangle_extrusion(\n" +
    "    x_bounds=(-2.0, 3.0),\n" +
    "    y_bounds=(-1.0, 2.0),\n" +
    "    plane_z=0.5,\n" +
    "    depth=4.0,\n" +
    "    modeling_tolerance=1e-9,\n" +
    ")\n" +
    "\n" +
    "_expected_graph_digest = (\n" +
    `    "${V1_DIGEST}"\n` +
    ")\n" +
    "if authored_graph.graph_digest != _expected_graph_digest:\n" +
    '    raise RuntimeError("Eqiora authored CAD graph digest mismatch after reconstruction")\n';

  const render = {
    protocol: CAD_AUTHORED_EXPORT_PROTOCOL,
    graphDigest: V1_DIGEST,
    suggestedFileName: CAD_AUTHORED_EXPORT_FILE_NAME,
    sourceUtf8: source,
  };

  type HostileMutation = Readonly<{
    id: string;
    mutation: string;
    value?: unknown;
    field?: string;
    repeat?: number;
    needle?: string;
  }>;
  const bridgeMutants = hostileCorpus.bridgeRequestMutants as readonly HostileMutation[];
  const renderMutants = hostileCorpus.renderResponseMutants as readonly HostileMutation[];

  function requiredString(value: unknown, id: string): string {
    if (typeof value !== "string") throw new Error(`corpus mutant ${id} omitted a string value`);
    return value;
  }

  function mutateClientRequest(mutant: HostileMutation): unknown {
    if (mutant.mutation === "replace-protocol") {
      return { ...request, protocol: requiredString(mutant.value, mutant.id) };
    }
    if (mutant.mutation === "add-json-field") {
      if (mutant.field === undefined) throw new Error(`corpus mutant ${mutant.id} omitted a field`);
      return { ...request, [mutant.field]: mutant.value };
    }
    if (mutant.mutation === "replace-canonical-graph-hex") {
      return { ...request, canonicalGraphHex: requiredString(mutant.value, mutant.id) };
    }
    if (mutant.mutation === "replace-canonical-graph-hex-repeat") {
      if (mutant.repeat === undefined) {
        throw new Error(`corpus mutant ${mutant.id} omitted a repeat count`);
      }
      return {
        ...request,
        canonicalGraphHex: requiredString(mutant.value, mutant.id).repeat(mutant.repeat),
      };
    }
    return null;
  }

  function mutateRender(mutant: HostileMutation): unknown {
    switch (mutant.mutation) {
      case "replace-protocol":
        return { ...render, protocol: requiredString(mutant.value, mutant.id) };
      case "replace-digest-first-nibble":
        return {
          ...render,
          graphDigest: `${requiredString(mutant.value, mutant.id)}${render.graphDigest.slice(1)}`,
        };
      case "replace-suggested-filename":
        return { ...render, suggestedFileName: requiredString(mutant.value, mutant.id) };
      case "replace-source-repeat":
        if (mutant.repeat === undefined) {
          throw new Error(`corpus mutant ${mutant.id} omitted a repeat count`);
        }
        return {
          ...render,
          sourceUtf8: requiredString(mutant.value, mutant.id).repeat(mutant.repeat),
        };
      case "prepend-source":
        return { ...render, sourceUtf8: `${requiredString(mutant.value, mutant.id)}${source}` };
      case "append-source":
        return { ...render, sourceUtf8: `${source}${requiredString(mutant.value, mutant.id)}` };
      case "replace-source-substring":
        if (mutant.needle === undefined) {
          throw new Error(`corpus mutant ${mutant.id} omitted a replacement needle`);
        }
        return {
          ...render,
          sourceUtf8: source.replace(mutant.needle, requiredString(mutant.value, mutant.id)),
        };
      case "remove-final-source-newline":
        return { ...render, sourceUtf8: source.slice(0, -1) };
      default:
        throw new Error(`unsupported render corpus mutation ${mutant.mutation}`);
    }
  }

  it("matches the exact precommitted corpus wire", () => {
    expect(hostileCorpus.schema).toBe("eqiora.studio.cad-authored-python-export-hostiles/v1");
    expect(hostileCorpus.protocol).toBe(CAD_AUTHORED_EXPORT_PROTOCOL);
    expect(hostileCorpus.requestFields).toEqual(Object.keys(request));
    expect(hostileCorpus.renderResponseFields).toEqual(Object.keys(render));
    expect(hostileCorpus.saveResponseFields).toEqual(["protocol", "graphDigest", "status"]);
    expect(hostileCorpus.suggestedFileName).toBe(CAD_AUTHORED_EXPORT_FILE_NAME);
  });

  it("accepts the one closed request and rejects each widened or malformed one", () => {
    expect(cadAuthoredExportRequestSchema.safeParse(request).success).toBe(true);

    const clientMutants = bridgeMutants
      .map((mutant) => [mutant.id, mutateClientRequest(mutant)] as const)
      .filter((entry): entry is readonly [string, object] => entry[1] !== null);
    expect(clientMutants.map(([id]) => id)).toEqual([
      "unknown-protocol",
      "unknown-field",
      "client-path",
      "client-source",
      "odd-graph-hex",
      "uppercase-graph-hex",
      "oversized-graph-hex",
    ]);
    for (const [id, payload] of clientMutants) {
      expect(cadAuthoredExportRequestSchema.safeParse(payload).success, `mutant ${id}`).toBe(false);
    }
  });

  it("accepts the exact closed render response", () => {
    expect(cadAuthoredExportRenderSchema.safeParse(render).success).toBe(true);
  });

  it("rejects each hostile render response before preview publication", () => {
    const rejects = renderMutants
      .filter(({ id }) => id !== "wrong-digest")
      .map((mutant) => [mutant.id, mutateRender(mutant)] as const);
    for (const [id, payload] of rejects) {
      expect(cadAuthoredExportRenderSchema.safeParse(payload).success, `mutant ${id}`).toBe(false);
    }
    const wrongDigest = renderMutants.find(({ id }) => id === "wrong-digest");
    if (wrongDigest === undefined) throw new Error("corpus omitted wrong-digest");
    expect(cadAuthoredExportRenderSchema.safeParse(mutateRender(wrongDigest)).success).toBe(true);
  });

  it("keeps the save outcome closed to saved and cancelled", () => {
    const saved = {
      protocol: CAD_AUTHORED_EXPORT_PROTOCOL,
      graphDigest: V1_DIGEST,
      status: "saved",
    };
    expect(cadAuthoredExportSaveSchema.safeParse(saved).success).toBe(true);
    expect(cadAuthoredExportSaveSchema.safeParse({ ...saved, status: "cancelled" }).success).toBe(
      true,
    );

    for (const payload of [
      { ...saved, status: "failed" },
      { ...saved, status: "written" },
      { ...saved, path: "/tmp/out.py" },
      { ...saved, protocol: CAD_AUTHORED_PROTOCOL },
    ]) {
      expect(cadAuthoredExportSaveSchema.safeParse(payload).success).toBe(false);
    }
  });
});
