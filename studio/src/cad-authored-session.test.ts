import { describe, expect, it } from "vitest";
import hostileCorpus from "../../verify/interfaces/studio-python-cad-round-trip/models/hostile.json";
import { protocolFailure } from "./bridge-contract";
import {
  CAD_AUTHORED_EXPORT_FILE_NAME,
  CAD_AUTHORED_EXPORT_PROTOCOL,
  CAD_AUTHORED_PROTOCOL,
  type CadAuthoredBuildRequest,
  type CadAuthoredExportRender,
  type CadAuthoredExportRequest,
  type CadAuthoredExportSave,
  type CadAuthoredProjection,
  cadAuthoredProjectionSchema,
} from "./cad-authored-protocol";
import {
  type CadAuthoredBridge,
  CadAuthoredSession,
  cadAuthoredBridge,
} from "./cad-authored-session";
import { BRIDGE_PROTOCOL, type BridgeEnvelope } from "./protocol";

/** Deterministic opaque token: ASCII bytes as even-length lowercase hex. */
function asciiHex(text: string): string {
  return [...text].map((char) => char.codePointAt(0)?.toString(16).padStart(2, "0")).join("");
}

// Every scientific constant below is copied verbatim from the accepted frozen
// cases `crates/eqiora-geometry/tests/cad_authored_rectangle_extrusion.rs`
// (v1) and `crates/eqiora-geometry/tests/cad_authored_circular_through_cut.rs`
// (v2). Opaque graph bytes, handles, and Python source are schema-shaped
// placeholder tokens; byte-exact source evidence belongs to the native Rust
// renderer, never to this suite. The fixtures are deliberately repeated from
// `cad-authored-protocol.test.ts` so each suite stays self-contained.
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

function handleFor(key: string): string {
  return asciiHex(`handle:${key}`);
}

function extrusionProjection(): CadAuthoredProjection {
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
      handleHex: handleFor(provenanceKey),
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
        created: V1_FACES.map(([key]) => ({ provenanceKey: key, handleHex: handleFor(key) })),
        deleted: [],
        split: [],
        merged: [],
      },
    },
  });
}

function cutProjection(): CadAuthoredProjection {
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
      handleHex: handleFor(provenanceKey),
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
        ].map((key) => ({ provenanceKey: key, handleHex: handleFor(key) })),
        retainedModified: ["start-cap", "end-cap"].map((key) => ({
          provenanceKey: key,
          handleHex: handleFor(key),
        })),
        created: [{ provenanceKey: "cut-wall", handleHex: handleFor("cut-wall") }],
        deleted: [],
        split: [],
        merged: [],
      },
    },
  });
}

// Scalar requests replayed by the fake owner below; the projection each fake
// build answers with is fixed per test, so only the shape matters here.
const V1_BUILD_REQUEST: CadAuthoredBuildRequest = {
  protocol: CAD_AUTHORED_PROTOCOL,
  sketch: { xBoundsM: [-2, 3], yBoundsM: [-1, 2], planeZM: 0.5 },
  extrusionDepthM: 4,
  requestedModelingToleranceM: 1e-9,
  cut: null,
};
const V2_BUILD_REQUEST: CadAuthoredBuildRequest = {
  protocol: CAD_AUTHORED_PROTOCOL,
  sketch: { xBoundsM: [-0.04, 0.04], yBoundsM: [-0.025, 0.025], planeZM: 0 },
  extrusionDepthM: 0.02,
  requestedModelingToleranceM: 1e-10,
  cut: { centerM: [0.02, 0], radiusM: 0.008, requestedBooleanToleranceM: 1e-9 },
};

/** Schema-shaped placeholder source; exact bytes are native-owner evidence. */
function sourceFor(digest: string): string {
  return `# Generated by Eqiora Studio from an accepted authored CAD graph.\nimport eqiora\n\n# graph ${digest}\n`;
}

function renderFor(digest: string): CadAuthoredExportRender {
  return {
    protocol: CAD_AUTHORED_EXPORT_PROTOCOL,
    graphDigest: digest,
    suggestedFileName: CAD_AUTHORED_EXPORT_FILE_NAME,
    sourceUtf8: sourceFor(digest),
  };
}

function saveOutcomeFor(digest: string, status: "saved" | "cancelled"): CadAuthoredExportSave {
  return { protocol: CAD_AUTHORED_EXPORT_PROTOCOL, graphDigest: digest, status };
}

function envelope<T>(result: T): BridgeEnvelope<T> {
  return { protocol: BRIDGE_PROTOCOL, result, diagnostics: [] };
}

interface ExportBridgeOptions {
  readonly projections?: readonly CadAuthoredProjection[];
  readonly renderPython?: (
    request: CadAuthoredExportRequest,
  ) => Promise<BridgeEnvelope<CadAuthoredExportRender>>;
  readonly savePython?: (
    request: CadAuthoredExportRequest,
  ) => Promise<BridgeEnvelope<CadAuthoredExportSave>>;
}

/** Fake owner: each build answers the next fixed projection in order. */
function exportBridge(options: ExportBridgeOptions): CadAuthoredBridge {
  const projections = [...(options.projections ?? [cutProjection()])];
  return {
    async build() {
      const projection = projections.length > 1 ? projections.shift() : projections[0];
      if (projection === undefined) throw new Error("fixture omitted a projection");
      return envelope(projection);
    },
    async resolve() {
      throw new Error("resolve must not run in export tests");
    },
    async renderPython(request) {
      if (options.renderPython === undefined) throw new Error("renderPython must not run");
      return options.renderPython(request);
    },
    async savePython(request) {
      if (options.savePython === undefined) throw new Error("savePython must not run");
      return options.savePython(request);
    },
  };
}

describe("authored-CAD session Python render binding", () => {
  it("covers the complete precommitted session and save mutant inventory", () => {
    expect(hostileCorpus.sessionMutants).toEqual([
      {
        id: "stale-render-after-rebuild",
        start: "v1",
        current: "v2",
        response: "v1-render-success",
        expect: "discard-stale",
      },
      {
        id: "stale-save-after-rebuild",
        start: "v1",
        current: "v2",
        response: "v1-save-success",
        expect: "discard-stale",
      },
    ]);
    expect(hostileCorpus.saveMutants).toEqual([
      {
        id: "dialog-cancelled",
        base: "v1",
        dialog: "cancelled",
        expectedWrites: 0,
        expect: "cancelled",
      },
      {
        id: "write-error",
        base: "v1",
        writer: "returns-error",
        expect: "bounded-write-error-not-saved",
      },
    ]);
  });
  it("sends the exact digest-bound opaque request and publishes the native render", async () => {
    const projection = cutProjection();
    const requests: CadAuthoredExportRequest[] = [];
    const session = new CadAuthoredSession(
      exportBridge({
        projections: [projection],
        renderPython: async (request) => {
          requests.push(request);
          return envelope(renderFor(request.graphDigest));
        },
      }),
    );
    await session.build(V2_BUILD_REQUEST);
    const state = await session.renderPython();
    expect(requests).toEqual([
      {
        protocol: CAD_AUTHORED_EXPORT_PROTOCOL,
        canonicalGraphHex: projection.canonicalGraphHex,
        graphDigest: V2_DIGEST,
      },
    ]);
    expect(state.export.preview).toEqual({ kind: "ready", render: renderFor(V2_DIGEST) });
    expect(state.export.save).toEqual({ kind: "idle" });
  });

  it("refuses to render or save before one accepted projection exists", async () => {
    const session = new CadAuthoredSession(exportBridge({}));
    expect((await session.renderPython()).export.preview).toEqual({ kind: "idle" });
    expect((await session.savePython()).export.save).toEqual({ kind: "idle" });
  });

  // Hostile corpus render-response mutant `wrong-digest`: a schema-valid
  // render bound to a foreign graph identity never publishes as a preview.
  it("rejects a render echo bound to a different graph identity", async () => {
    const witnessDigest = `ff${V2_DIGEST.slice(2)}`;
    const session = new CadAuthoredSession(
      exportBridge({
        renderPython: async () => envelope(renderFor(witnessDigest)),
      }),
    );
    await session.build(V2_BUILD_REQUEST);
    const state = await session.renderPython();
    expect(state.export.preview.kind).toBe("failed");
  });

  it("discards a superseded render in favour of the latest one", async () => {
    let releaseFirst: () => void = () => {
      throw new Error("first render was not started");
    };
    const firstGate = new Promise<void>((resolve) => {
      releaseFirst = resolve;
    });
    let renders = 0;
    const session = new CadAuthoredSession(
      exportBridge({
        renderPython: async (request) => {
          renders += 1;
          if (renders === 1) await firstGate;
          return envelope(renderFor(request.graphDigest));
        },
      }),
    );
    await session.build(V2_BUILD_REQUEST);
    const stale = session.renderPython();
    await session.renderPython();
    releaseFirst();
    await stale;
    expect(session.state.export.preview).toEqual({ kind: "ready", render: renderFor(V2_DIGEST) });
    expect(renders).toBe(2);
  });

  it("lets independent render and save calls finish without stranding either state", async () => {
    const session = new CadAuthoredSession(
      exportBridge({
        renderPython: async (request) => envelope(renderFor(request.graphDigest)),
        savePython: async (request) => envelope(saveOutcomeFor(request.graphDigest, "saved")),
      }),
    );
    await session.build(V2_BUILD_REQUEST);
    await Promise.all([session.renderPython(), session.savePython()]);
    expect(session.state.export.preview).toEqual({ kind: "ready", render: renderFor(V2_DIGEST) });
    expect(session.state.export.save).toEqual({ kind: "saved", graphDigest: V2_DIGEST });
  });
});

describe("authored-CAD session export staleness", () => {
  // Hostile corpus session mutant `stale-render-after-rebuild`: a v1 render
  // success arriving after the graph became v2 must not publish.
  it("discards a stale render response after the graph is rebuilt", async () => {
    let releaseFirst: () => void = () => {
      throw new Error("first render was not started");
    };
    const firstGate = new Promise<void>((resolve) => {
      releaseFirst = resolve;
    });
    const session = new CadAuthoredSession(
      exportBridge({
        projections: [extrusionProjection(), cutProjection()],
        renderPython: async (request) => {
          if (request.graphDigest === V1_DIGEST) await firstGate;
          return envelope(renderFor(request.graphDigest));
        },
      }),
    );
    await session.build(V1_BUILD_REQUEST);
    const stale = session.renderPython();
    await session.build(V2_BUILD_REQUEST);
    releaseFirst();
    await stale;
    expect(session.state.build).toMatchObject({ kind: "ready" });
    expect(session.state.export).toEqual({ preview: { kind: "idle" }, save: { kind: "idle" } });
  });

  // Hostile corpus session mutant `stale-save-after-rebuild`: a v1 saved
  // outcome arriving after the graph became v2 must not report saved state.
  it("discards a stale save response after the graph is rebuilt", async () => {
    let releaseFirst: () => void = () => {
      throw new Error("first save was not started");
    };
    const firstGate = new Promise<void>((resolve) => {
      releaseFirst = resolve;
    });
    const session = new CadAuthoredSession(
      exportBridge({
        projections: [extrusionProjection(), cutProjection()],
        savePython: async (request) => {
          if (request.graphDigest === V1_DIGEST) await firstGate;
          return envelope(saveOutcomeFor(request.graphDigest, "saved"));
        },
      }),
    );
    await session.build(V1_BUILD_REQUEST);
    const stale = session.savePython();
    await session.build(V2_BUILD_REQUEST);
    releaseFirst();
    await stale;
    expect(session.state.export).toEqual({ preview: { kind: "idle" }, save: { kind: "idle" } });
  });

  it("clears the previous preview and save outcome when the graph changes", async () => {
    const session = new CadAuthoredSession(
      exportBridge({
        projections: [cutProjection(), extrusionProjection()],
        renderPython: async (request) => envelope(renderFor(request.graphDigest)),
        savePython: async (request) => envelope(saveOutcomeFor(request.graphDigest, "saved")),
      }),
    );
    await session.build(V2_BUILD_REQUEST);
    await session.renderPython();
    await session.savePython();
    expect(session.state.export.preview.kind).toBe("ready");
    expect(session.state.export.save.kind).toBe("saved");

    await session.build(V1_BUILD_REQUEST);
    expect(session.state.export).toEqual({ preview: { kind: "idle" }, save: { kind: "idle" } });

    session.clear();
    expect(session.state.export).toEqual({ preview: { kind: "idle" }, save: { kind: "idle" } });
  });
});

describe("authored-CAD session save outcomes", () => {
  // Hostile corpus save mutant `dialog-cancelled`: cancellation is a normal
  // explicit outcome, never a failure and never a saved report.
  it("reports dialog cancellation as a normal outcome that keeps the preview", async () => {
    const session = new CadAuthoredSession(
      exportBridge({
        renderPython: async (request) => envelope(renderFor(request.graphDigest)),
        savePython: async (request) => envelope(saveOutcomeFor(request.graphDigest, "cancelled")),
      }),
    );
    await session.build(V2_BUILD_REQUEST);
    await session.renderPython();
    const state = await session.savePython();
    expect(state.export.save).toEqual({ kind: "cancelled" });
    expect(state.export.preview.kind).toBe("ready");
  });

  it("accepts a saved outcome only for the exact bound graph", async () => {
    const session = new CadAuthoredSession(
      exportBridge({
        savePython: async (request) => envelope(saveOutcomeFor(request.graphDigest, "saved")),
      }),
    );
    await session.build(V2_BUILD_REQUEST);
    const state = await session.savePython();
    expect(state.export.save).toEqual({ kind: "saved", graphDigest: V2_DIGEST });

    const witnessDigest = `ff${V2_DIGEST.slice(2)}`;
    const foreign = new CadAuthoredSession(
      exportBridge({
        savePython: async () => envelope(saveOutcomeFor(witnessDigest, "saved")),
      }),
    );
    await foreign.build(V2_BUILD_REQUEST);
    const rejected = await foreign.savePython();
    expect(rejected.export.save.kind).toBe("failed");
  });

  // Hostile corpus save mutant `write-error`: a bounded diagnostic, surfaced
  // as failure and never as a saved file.
  it("surfaces a native write error without reporting a saved file", async () => {
    const session = new CadAuthoredSession(
      exportBridge({
        savePython: async () =>
          protocolFailure("failed to write the exported Python file: PermissionDenied"),
      }),
    );
    await session.build(V2_BUILD_REQUEST);
    const state = await session.savePython();
    expect(state.export.save).toEqual({
      kind: "failed",
      message: "failed to write the exported Python file: PermissionDenied",
    });
  });
});

describe("authored-CAD browser preview refusal", () => {
  it("refuses to fabricate Python source or a successful save outside native Studio", async () => {
    // Under vitest there is no Tauri host, so the exported bridge is the
    // browser-preview bridge; both export calls must refuse explicitly.
    const request: CadAuthoredExportRequest = {
      protocol: CAD_AUTHORED_EXPORT_PROTOCOL,
      canonicalGraphHex: "ab".repeat(V2_CANONICAL_BYTE_COUNT),
      graphDigest: V2_DIGEST,
    };
    for (const response of [
      await cadAuthoredBridge.renderPython(request),
      await cadAuthoredBridge.savePython(request),
    ]) {
      expect(response.result).toBeNull();
      expect(response.diagnostics[0]?.message).toContain(
        "does not fabricate Python source or a saved file",
      );
    }
  });
});
