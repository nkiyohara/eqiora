import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { protocolFailure } from "./bridge-contract";
import {
  CAD_AUTHORED_EXPORT_FILE_NAME,
  CAD_AUTHORED_EXPORT_PROTOCOL,
  CAD_AUTHORED_PROTOCOL,
  type CadAuthoredExportRender,
  type CadAuthoredProjection,
  type CadAuthoredSelectionRequest,
  type CadAuthoredSelectionResult,
  cadAuthoredProjectionSchema,
} from "./cad-authored-protocol";
import {
  type CadAuthoredBridge,
  type CadAuthoredExportState,
  CadAuthoredSession,
} from "./cad-authored-session";
import {
  CAD_AUTHORED_DEFAULT_FORM,
  CadAuthoredBuildReceiptPanel,
  CadAuthoredControls,
  CadAuthoredExportPanel,
  CadAuthoredFaceList,
  CadAuthoredIdentityPanel,
  cadAuthoredFormRequest,
} from "./cad-authored-workspace";
import { BRIDGE_PROTOCOL, type BridgeEnvelope } from "./protocol";

/** Deterministic opaque token: ASCII bytes as even-length lowercase hex. */
function asciiHex(text: string): string {
  return [...text].map((char) => char.codePointAt(0)?.toString(16).padStart(2, "0")).join("");
}

// Every scientific constant below is copied verbatim from the accepted frozen
// case `crates/eqiora-geometry/tests/cad_authored_circular_through_cut.rs`.
// Opaque graph bytes and handles are schema-shaped placeholder tokens; this
// suite treats them as the pure echoes they are. The fixture is deliberately
// repeated from `cad-authored-protocol.test.ts` so each suite stays
// self-contained instead of importing another test module.
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

function cutProjection(digest = V2_DIGEST): CadAuthoredProjection {
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
    graphDigest: digest,
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
      graphDigest: digest,
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

function selectionFor(
  projection: CadAuthoredProjection,
  handleHex: string,
): CadAuthoredSelectionResult {
  const face = projection.faces.find((candidate) => candidate.handleHex === handleHex);
  if (face === undefined) throw new Error("fixture face is absent");
  return {
    protocol: CAD_AUTHORED_PROTOCOL,
    graphDigest: projection.graphDigest,
    provenanceKey: face.provenanceKey,
    handleHex,
    areaM2: face.areaM2,
    boundaryLoopCount: face.boundaryLoopCount,
    centroidM: null,
    outwardUnitNormal: null,
  };
}

function envelope<T>(result: T): BridgeEnvelope<T> {
  return { protocol: BRIDGE_PROTOCOL, result, diagnostics: [] };
}

/** These suites exercise build and selection only; the session gates export
 * calls behind an accepted projection, so reaching one here is a defect. */
const exportNotCalled = {
  async renderPython(): Promise<BridgeEnvelope<never>> {
    throw new Error("renderPython must not run");
  },
  async savePython(): Promise<BridgeEnvelope<never>> {
    throw new Error("savePython must not run");
  },
};

function echoBridge(
  projection: CadAuthoredProjection,
  onResolve?: (request: CadAuthoredSelectionRequest) => void,
): CadAuthoredBridge {
  return {
    ...exportNotCalled,
    async build() {
      return envelope(projection);
    },
    async resolve(request) {
      onResolve?.(request);
      return envelope(selectionFor(projection, request.handleHex));
    },
  };
}

const BUILD_REQUEST = (() => {
  const parsed = cadAuthoredFormRequest(CAD_AUTHORED_DEFAULT_FORM);
  if (!parsed.ok) throw new Error(parsed.message);
  return parsed.request;
})();

describe("authored-CAD session build lifecycle", () => {
  it("publishes the accepted projection and fails closed on rejection", async () => {
    const transitions: string[] = [];
    const session = new CadAuthoredSession(echoBridge(cutProjection()), (state) =>
      transitions.push(state.build.kind),
    );
    const state = await session.build(BUILD_REQUEST);
    expect(state.build).toEqual({ kind: "ready", projection: cutProjection() });
    expect(transitions).toEqual(["building", "ready"]);

    const rejected = new CadAuthoredSession({
      ...exportNotCalled,
      async build() {
        return protocolFailure("The native owner rejected the authored history.");
      },
      async resolve() {
        throw new Error("resolve must not run");
      },
    });
    const failure = await rejected.build(BUILD_REQUEST);
    expect(failure.build).toMatchObject({
      kind: "failed",
      message: "The native owner rejected the authored history.",
    });
  });

  it("discards a superseded build response", async () => {
    const latest = cutProjection();
    const witnessDigest = `ff${V2_DIGEST.slice(2)}`;
    let calls = 0;
    let releaseFirst: (projection: CadAuthoredProjection) => void = () => {
      throw new Error("first request was not started");
    };
    const firstResponse = new Promise<CadAuthoredProjection>((resolve) => {
      releaseFirst = resolve;
    });
    const session = new CadAuthoredSession({
      ...exportNotCalled,
      async build() {
        calls += 1;
        return envelope(calls === 1 ? await firstResponse : latest);
      },
      async resolve() {
        throw new Error("resolve must not run");
      },
    });

    const stale = session.build(BUILD_REQUEST);
    expect((await session.build(BUILD_REQUEST)).build).toEqual({
      kind: "ready",
      projection: latest,
    });
    releaseFirst(cutProjection(witnessDigest));
    await stale;
    expect(session.state.build).toEqual({ kind: "ready", projection: latest });
  });
});

describe("authored-CAD session selection replay", () => {
  it("sends the digest-bound opaque handle and accepts only its exact echo", async () => {
    const projection = cutProjection();
    const requests: CadAuthoredSelectionRequest[] = [];
    const session = new CadAuthoredSession(
      echoBridge(projection, (request) => requests.push(request)),
    );
    await session.build(BUILD_REQUEST);
    const state = await session.select(handleFor("cut-wall"));
    expect(requests).toEqual([
      {
        protocol: CAD_AUTHORED_PROTOCOL,
        graphDigest: V2_DIGEST,
        canonicalGraphHex: projection.canonicalGraphHex,
        handleHex: handleFor("cut-wall"),
      },
    ]);
    expect(state.selection).toEqual({
      kind: "selected",
      selection: selectionFor(projection, handleFor("cut-wall")),
    });
  });

  it("submits the identical exact request from either selection modality", async () => {
    const projection = cutProjection();
    const requests: CadAuthoredSelectionRequest[] = [];
    const session = new CadAuthoredSession(
      echoBridge(projection, (request) => requests.push(request)),
    );
    await session.build(BUILD_REQUEST);

    // Modality one: the admitted face list row.
    const faceEntry = projection.faces.find((face) => face.provenanceKey === "cut-wall");
    // Modality two: the created-lineage chip on the build receipt.
    const lineageEntry = projection.build.lineage.created[0];
    if (faceEntry === undefined || lineageEntry === undefined) {
      throw new Error("fixture omitted the cut wall");
    }
    await session.select(faceEntry.handleHex);
    await session.select(lineageEntry.handleHex);
    expect(requests).toHaveLength(2);
    expect(requests[0]).toEqual(requests[1]);
  });

  it("suppresses a stale selection and rejects a mismatched echo", async () => {
    const projection = cutProjection();
    let releaseFirst: () => void = () => {
      throw new Error("first selection was not started");
    };
    const firstGate = new Promise<void>((resolve) => {
      releaseFirst = resolve;
    });
    let resolves = 0;
    const session = new CadAuthoredSession({
      ...exportNotCalled,
      async build() {
        return envelope(projection);
      },
      async resolve(request) {
        resolves += 1;
        if (resolves === 1) await firstGate;
        return envelope(selectionFor(projection, request.handleHex));
      },
    });
    await session.build(BUILD_REQUEST);
    const stale = session.select(handleFor("start-cap"));
    await session.select(handleFor("cut-wall"));
    releaseFirst();
    await stale;
    expect(session.state.selection).toEqual({
      kind: "selected",
      selection: selectionFor(projection, handleFor("cut-wall")),
    });

    // A digest echo from a foreign graph identity is never admitted.
    const witnessDigest = `ff${V2_DIGEST.slice(2)}`;
    const mismatched = new CadAuthoredSession({
      ...exportNotCalled,
      async build() {
        return envelope(projection);
      },
      async resolve(request) {
        return envelope({
          ...selectionFor(projection, request.handleHex),
          graphDigest: witnessDigest,
        });
      },
    });
    await mismatched.build(BUILD_REQUEST);
    const state = await mismatched.select(handleFor("cut-wall"));
    expect(state.selection.kind).toBe("failed");
  });

  it("rejects a semantic echo that differs from the current projection face", async () => {
    // Right digest and right handle, wrong meaning: each mutant swaps one
    // returned face observation for another admitted face's value.
    const projection = cutProjection();
    const startCap = projection.faces.find((face) => face.provenanceKey === "start-cap");
    if (startCap === undefined) throw new Error("fixture omitted the start cap");
    const mutants: ((result: CadAuthoredSelectionResult) => CadAuthoredSelectionResult)[] = [
      (result) => ({ ...result, provenanceKey: startCap.provenanceKey }),
      (result) => ({ ...result, areaM2: startCap.areaM2 }),
      (result) => ({ ...result, boundaryLoopCount: 1 }),
      (result) => ({ ...result, centroidM: [0, 0, 0] }),
      (result) => ({ ...result, outwardUnitNormal: [0, 0, 1] }),
    ];
    for (const mutate of mutants) {
      const session = new CadAuthoredSession({
        ...exportNotCalled,
        async build() {
          return envelope(projection);
        },
        async resolve(request) {
          return envelope(mutate(selectionFor(projection, request.handleHex)));
        },
      });
      await session.build(BUILD_REQUEST);
      const state = await session.select(handleFor("cut-wall"));
      expect(state.selection.kind).toBe("failed");
    }
  });

  it("refuses a handle absent from the current authored graph", async () => {
    const session = new CadAuthoredSession({
      ...exportNotCalled,
      async build() {
        return envelope(cutProjection());
      },
      async resolve() {
        throw new Error("a foreign handle must never reach the native bridge");
      },
    });
    await session.build(BUILD_REQUEST);
    const state = await session.select(asciiHex("handle:foreign"));
    expect(state.selection).toMatchObject({ kind: "failed" });
  });
});

describe("authored-CAD form authoring", () => {
  it("builds the frozen witness request from the default form", () => {
    expect(BUILD_REQUEST).toEqual({
      protocol: CAD_AUTHORED_PROTOCOL,
      sketch: { xBoundsM: [-0.04, 0.04], yBoundsM: [-0.025, 0.025], planeZM: 0 },
      extrusionDepthM: 0.02,
      requestedModelingToleranceM: 1e-10,
      cut: { centerM: [0.02, 0], radiusM: 0.008, requestedBooleanToleranceM: 1e-9 },
    });
    const withoutCut = cadAuthoredFormRequest({ ...CAD_AUTHORED_DEFAULT_FORM, cutEnabled: false });
    expect(withoutCut).toMatchObject({ ok: true, request: { cut: null } });
  });

  it("names the offending field instead of sending a malformed request", () => {
    const junk = cadAuthoredFormRequest({ ...CAD_AUTHORED_DEFAULT_FORM, extrusionDepthM: "deep" });
    expect(junk).toEqual({
      ok: false,
      message: "The extrusion depth field needs one finite scalar in metres.",
    });
    const ignored = cadAuthoredFormRequest({
      ...CAD_AUTHORED_DEFAULT_FORM,
      cutEnabled: false,
      cutRadiusM: "not-a-number",
    });
    expect(ignored.ok).toBe(true);
  });
});

describe("authored-CAD workspace accessibility and naming", () => {
  it("labels every scalar input and keeps controls keyboard-native", () => {
    const markup = renderToStaticMarkup(
      createElement(CadAuthoredControls, {
        busy: false,
        form: CAD_AUTHORED_DEFAULT_FORM,
        formError: null,
        onChange: () => {},
        onSubmit: () => {},
      }),
    );
    const inputIds = [...markup.matchAll(/<input[^>]*\sid="([^"]+)"/g)].map(([, id]) => id);
    const labelTargets = new Set(
      [...markup.matchAll(/<label[^>]*\sfor="([^"]+)"/g)].map(([, target]) => target),
    );
    expect(inputIds).toHaveLength(12);
    for (const id of inputIds) {
      expect(labelTargets.has(id ?? ""), `input ${id} needs a label`).toBe(true);
    }
    expect(markup).toContain('type="submit"');
    expect(markup).toContain("<legend>Circular through-cut</legend>");
  });

  it("presents faces and lineage chips as pressable buttons with shared selection state", () => {
    const projection = cutProjection();
    const selected = handleFor("cut-wall");
    const faceMarkup = renderToStaticMarkup(
      createElement(CadAuthoredFaceList, {
        faces: projection.faces,
        onSelect: () => {},
        selectedHandleHex: selected,
        selectionPending: false,
      }),
    );
    const receiptMarkup = renderToStaticMarkup(
      createElement(CadAuthoredBuildReceiptPanel, {
        build: projection.build,
        onSelect: () => {},
        selectedHandleHex: selected,
        selectionPending: false,
      }),
    );
    expect(faceMarkup.match(/type="button"/g)).toHaveLength(7);
    expect(faceMarkup.match(/aria-pressed="true"/g)).toHaveLength(1);
    expect(faceMarkup).toContain("Cut wall");
    expect(receiptMarkup.match(/aria-pressed="true"/g)).toHaveLength(1);
    expect(receiptMarkup).toContain("eqiora.cad.analytic-circular-through-cut-v1");
  });

  it("names the digest an authored graph identity, never a Geometry identity", () => {
    const markup = renderToStaticMarkup(
      createElement(CadAuthoredIdentityPanel, { projection: cutProjection() }),
    );
    expect(markup).toContain("Authored graph identity");
    expect(markup).toContain("Authored graph digest");
    expect(markup).toContain("not a Geometry identity");
    expect(markup).not.toContain("Geometry digest");
    expect(markup).toContain(V2_DIGEST.slice(0, 16));
  });
});

describe("authored-CAD Python export panel", () => {
  const idleExport: CadAuthoredExportState = { preview: { kind: "idle" }, save: { kind: "idle" } };
  // Schema-shaped placeholder source; byte-exact source is native evidence.
  const render: CadAuthoredExportRender = {
    protocol: CAD_AUTHORED_EXPORT_PROTOCOL,
    graphDigest: V2_DIGEST,
    suggestedFileName: CAD_AUTHORED_EXPORT_FILE_NAME,
    sourceUtf8:
      "# Generated by Eqiora Studio from an accepted authored CAD graph.\nimport eqiora\n",
  };

  function markupFor(state: CadAuthoredExportState): string {
    return renderToStaticMarkup(
      createElement(CadAuthoredExportPanel, {
        graphDigest: V2_DIGEST,
        onPreview: () => {},
        onSave: () => {},
        state,
      }),
    );
  }

  it("offers both export actions bound to the exact current graph digest", () => {
    const markup = markupFor(idleExport);
    expect(markup).toContain("Preview Python");
    expect(markup).toContain("Save Python file…");
    expect(markup.match(/type="button"/g)).toHaveLength(2);
    expect(markup).toContain(V2_DIGEST.slice(0, 16));
    expect(markup).not.toContain("<textarea");
  });

  it("shows the native source in a keyboard-reachable selectable code region", () => {
    const markup = markupFor({ ...idleExport, preview: { kind: "ready", render } });
    expect(markup).toContain('tabindex="0"');
    expect(markup).toContain('role="region"');
    expect(markup).toContain(
      `aria-label="Generated Python source ${CAD_AUTHORED_EXPORT_FILE_NAME}"`,
    );
    expect(markup).toContain("import eqiora");
    expect(markup).toContain(CAD_AUTHORED_EXPORT_FILE_NAME);
  });

  it("reports cancellation as a normal status, never as an error", () => {
    const markup = markupFor({ ...idleExport, save: { kind: "cancelled" } });
    expect(markup).toContain("nothing was written");
    expect(markup).toContain('role="status"');
    expect(markup).not.toContain('role="alert"');
  });

  it("reports the saved outcome and surfaces failures as alerts", () => {
    const saved = markupFor({ ...idleExport, save: { kind: "saved", graphDigest: V2_DIGEST } });
    expect(saved).toContain("Saved the exported Python file");

    const refusal =
      "The authored-CAD Python export is available only in native Studio; " +
      "browser preview does not fabricate Python source or a saved file.";
    const failed = markupFor({ ...idleExport, preview: { kind: "failed", message: refusal } });
    expect(failed).toContain('role="alert"');
    expect(failed).toContain("does not fabricate Python source or a saved file");
  });

  it("disables both actions while a render or save is in flight", () => {
    for (const state of [
      { ...idleExport, preview: { kind: "rendering" } } satisfies CadAuthoredExportState,
      { ...idleExport, save: { kind: "saving" } } satisfies CadAuthoredExportState,
    ]) {
      const markup = markupFor(state);
      expect(markup.match(/<button[^>]*disabled/g)).toHaveLength(2);
    }
  });
});
