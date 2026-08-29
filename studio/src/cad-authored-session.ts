import { invoke } from "@tauri-apps/api/core";
import { protocolFailure } from "./bridge-contract";
import {
  CAD_AUTHORED_EXPORT_PROTOCOL,
  CAD_AUTHORED_PROTOCOL,
  type CadAuthoredExportRender,
  type CadAuthoredExportRequest,
  type CadAuthoredExportSave,
  type CadAuthoredProjection,
  type CadAuthoredSelectionRequest,
  type CadAuthoredSelectionResult,
  cadAuthoredBuildRequestSchema,
  cadAuthoredExportRenderSchema,
  cadAuthoredExportRequestSchema,
  cadAuthoredExportSaveSchema,
  cadAuthoredProjectionSchema,
  cadAuthoredSelectionRequestSchema,
  cadAuthoredSelectionResultSchema,
  type GeometryBuildReceiptRequest,
} from "./cad-authored-protocol";
import { type BridgeEnvelope, bridgeEnvelopeSchema, type StudioDiagnostic } from "./protocol";

/** Native command names for the later thin Tauri wrappers. */
export const CAD_AUTHORED_BUILD_COMMAND = "build_cad_authored_graph";
export const CAD_AUTHORED_RESOLVE_COMMAND = "resolve_cad_authored_face";
export const CAD_AUTHORED_RENDER_PYTHON_COMMAND = "render_cad_authored_python";
export const CAD_AUTHORED_SAVE_PYTHON_COMMAND = "save_cad_authored_python";

/** Transport boundary: exactly four closed native calls, all replayable. */
export interface CadAuthoredBridge {
  build(request: GeometryBuildReceiptRequest): Promise<BridgeEnvelope<CadAuthoredProjection>>;
  resolve(
    request: CadAuthoredSelectionRequest,
  ): Promise<BridgeEnvelope<CadAuthoredSelectionResult>>;
  renderPython(request: CadAuthoredExportRequest): Promise<BridgeEnvelope<CadAuthoredExportRender>>;
  savePython(request: CadAuthoredExportRequest): Promise<BridgeEnvelope<CadAuthoredExportSave>>;
}

async function checkedInvoke<T>(
  command: string,
  args: Record<string, unknown>,
  schema: ReturnType<typeof bridgeEnvelopeSchema>,
): Promise<BridgeEnvelope<T>> {
  try {
    const response: unknown = await invoke(command, args);
    const decoded = schema.safeParse(response);
    return decoded.success
      ? (decoded.data as BridgeEnvelope<T>)
      : protocolFailure(`Native bridge returned an invalid ${command} response.`);
  } catch (error: unknown) {
    const detail = error instanceof Error ? error.message : String(error);
    return protocolFailure(`Native bridge call ${command} failed: ${detail}`);
  }
}

/**
 * The projection must echo the exact requested scalars through its complete
 * frozen history and identity-bearing tolerances, and carry the matching cut
 * parity: four steps without a cut, the full eight-step chain with one.
 */
export function cadAuthoredProjectionMatchesRequest(
  request: GeometryBuildReceiptRequest,
  projection: CadAuthoredProjection,
): boolean {
  const sketchPlane = projection.history[0];
  const rectangle = projection.history[1];
  const extrusion = projection.history[3];
  if (
    sketchPlane?.kind !== "sketch-plane" ||
    rectangle?.kind !== "rectangle-profile" ||
    extrusion?.kind !== "positive-z-extrusion"
  ) {
    return false;
  }
  const sketchMatches =
    sketchPlane.zM === request.sketch.planeZM &&
    rectangle.xBoundsM[0] === request.sketch.xBoundsM[0] &&
    rectangle.xBoundsM[1] === request.sketch.xBoundsM[1] &&
    rectangle.yBoundsM[0] === request.sketch.yBoundsM[0] &&
    rectangle.yBoundsM[1] === request.sketch.yBoundsM[1] &&
    extrusion.depthM === request.extrusionDepthM &&
    projection.tolerances.requestedModelingToleranceM === request.requestedModelingToleranceM;
  if (!sketchMatches) return false;
  if (request.cut === null) return projection.history.length === 4;
  const circleProfile = projection.history[5];
  const cut = projection.history[7];
  return (
    projection.history.length === 8 &&
    circleProfile?.kind === "circle-profile" &&
    cut?.kind === "circular-through-cut" &&
    circleProfile.centerM[0] === request.cut.centerM[0] &&
    circleProfile.centerM[1] === request.cut.centerM[1] &&
    circleProfile.radiusM === request.cut.radiusM &&
    cut.requestedBooleanToleranceM === request.cut.requestedBooleanToleranceM
  );
}

function selectionMatchesRequest(
  request: CadAuthoredSelectionRequest,
  result: CadAuthoredSelectionResult,
): boolean {
  return result.graphDigest === request.graphDigest && result.handleHex === request.handleHex;
}

const nativeCadAuthoredBridge: CadAuthoredBridge = {
  async build(request) {
    const checked = cadAuthoredBuildRequestSchema.safeParse(request);
    if (!checked.success) return protocolFailure("Authored-CAD build request is not canonical.");
    const response = await checkedInvoke<CadAuthoredProjection>(
      CAD_AUTHORED_BUILD_COMMAND,
      { request: checked.data },
      bridgeEnvelopeSchema(cadAuthoredProjectionSchema),
    );
    return response.result !== null &&
      !cadAuthoredProjectionMatchesRequest(checked.data, response.result)
      ? protocolFailure("Native authored-CAD projection differs from the exact request.")
      : response;
  },

  async resolve(request) {
    const checked = cadAuthoredSelectionRequestSchema.safeParse(request);
    if (!checked.success) {
      return protocolFailure("Authored-CAD selection request is not canonical.");
    }
    const response = await checkedInvoke<CadAuthoredSelectionResult>(
      CAD_AUTHORED_RESOLVE_COMMAND,
      { request: checked.data },
      bridgeEnvelopeSchema(cadAuthoredSelectionResultSchema),
    );
    return response.result !== null && !selectionMatchesRequest(checked.data, response.result)
      ? protocolFailure("Native authored-CAD selection differs from the exact request.")
      : response;
  },

  async renderPython(request) {
    const checked = cadAuthoredExportRequestSchema.safeParse(request);
    if (!checked.success) {
      return protocolFailure("Authored-CAD Python export request is not canonical.");
    }
    const response = await checkedInvoke<CadAuthoredExportRender>(
      CAD_AUTHORED_RENDER_PYTHON_COMMAND,
      { request: checked.data },
      bridgeEnvelopeSchema(cadAuthoredExportRenderSchema),
    );
    return response.result !== null && response.result.graphDigest !== checked.data.graphDigest
      ? protocolFailure("Native Python rendering is bound to a different graph identity.")
      : response;
  },

  async savePython(request) {
    const checked = cadAuthoredExportRequestSchema.safeParse(request);
    if (!checked.success) {
      return protocolFailure("Authored-CAD Python export request is not canonical.");
    }
    const response = await checkedInvoke<CadAuthoredExportSave>(
      CAD_AUTHORED_SAVE_PYTHON_COMMAND,
      { request: checked.data },
      bridgeEnvelopeSchema(cadAuthoredExportSaveSchema),
    );
    return response.result !== null && response.result.graphDigest !== checked.data.graphDigest
      ? protocolFailure("Native Python save outcome is bound to a different graph identity.")
      : response;
  },
};

const PREVIEW_REFUSAL =
  "The authored-CAD owner replay is available only in native Studio; " +
  "browser preview does not fabricate canonical bytes or graph identities.";

/**
 * Browser preview refuses to fabricate Python source or a saved file: only
 * the native owner replay may produce either.
 */
const EXPORT_PREVIEW_REFUSAL =
  "The authored-CAD Python export is available only in native Studio; " +
  "browser preview does not fabricate Python source or a saved file.";

const previewCadAuthoredBridge: CadAuthoredBridge = {
  async build() {
    await Promise.resolve();
    return protocolFailure(PREVIEW_REFUSAL);
  },
  async resolve() {
    await Promise.resolve();
    return protocolFailure(PREVIEW_REFUSAL);
  },
  async renderPython() {
    await Promise.resolve();
    return protocolFailure(EXPORT_PREVIEW_REFUSAL);
  },
  async savePython() {
    await Promise.resolve();
    return protocolFailure(EXPORT_PREVIEW_REFUSAL);
  },
};

export const cadAuthoredBridge: CadAuthoredBridge =
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window
    ? nativeCadAuthoredBridge
    : previewCadAuthoredBridge;

export type GeometryBuildReceiptState =
  | Readonly<{ kind: "idle" }>
  | Readonly<{ kind: "building" }>
  | Readonly<{ kind: "ready"; projection: CadAuthoredProjection }>
  | Readonly<{
      kind: "failed";
      message: string;
      diagnostics: readonly StudioDiagnostic[];
    }>;

export type CadAuthoredSelectionState =
  | Readonly<{ kind: "idle" }>
  | Readonly<{ kind: "resolving"; handleHex: string }>
  | Readonly<{ kind: "selected"; selection: CadAuthoredSelectionResult }>
  | Readonly<{ kind: "failed"; message: string }>;

export type CadAuthoredExportPreviewState =
  | Readonly<{ kind: "idle" }>
  | Readonly<{ kind: "rendering" }>
  | Readonly<{ kind: "ready"; render: CadAuthoredExportRender }>
  | Readonly<{ kind: "failed"; message: string }>;

export type CadAuthoredExportSaveState =
  | Readonly<{ kind: "idle" }>
  | Readonly<{ kind: "saving" }>
  | Readonly<{ kind: "saved"; graphDigest: string }>
  | Readonly<{ kind: "cancelled" }>
  | Readonly<{ kind: "failed"; message: string }>;

/** Preview and save outcomes for the current accepted graph only. */
export type CadAuthoredExportState = Readonly<{
  preview: CadAuthoredExportPreviewState;
  save: CadAuthoredExportSaveState;
}>;

export type CadAuthoredSessionState = Readonly<{
  build: GeometryBuildReceiptState;
  selection: CadAuthoredSelectionState;
  export: CadAuthoredExportState;
}>;

export type CadAuthoredSessionObserver = (state: CadAuthoredSessionState) => void;

const IDLE_EXPORT_STATE: CadAuthoredExportState = {
  preview: { kind: "idle" },
  save: { kind: "idle" },
};

const IDLE_STATE: CadAuthoredSessionState = {
  build: { kind: "idle" },
  selection: { kind: "idle" },
  export: IDLE_EXPORT_STATE,
};

/**
 * Own the asynchronous authored-CAD application boundary.
 *
 * State is separated from transport: the bridge is injected and this class
 * never touches the native layer directly. Generation guards discard every
 * superseded build response; a selection is admitted only while the exact
 * projection it was requested against is still the current one, and only when
 * the returned selection repeats that projection's face field-for-field.
 */
export class CadAuthoredSession {
  readonly #bridge: CadAuthoredBridge;
  readonly #observer: CadAuthoredSessionObserver;
  #buildGeneration = 0;
  #selectionGeneration = 0;
  #renderGeneration = 0;
  #saveGeneration = 0;
  #state: CadAuthoredSessionState = IDLE_STATE;

  constructor(bridge: CadAuthoredBridge, observer: CadAuthoredSessionObserver = () => {}) {
    this.#bridge = bridge;
    this.#observer = observer;
  }

  get state(): CadAuthoredSessionState {
    return this.#state;
  }

  clear(): void {
    this.#buildGeneration += 1;
    this.#selectionGeneration += 1;
    this.#renderGeneration += 1;
    this.#saveGeneration += 1;
    this.#transition(IDLE_STATE);
  }

  /** Replay one closed scalar request in the native owner. */
  async build(request: GeometryBuildReceiptRequest): Promise<CadAuthoredSessionState> {
    const generation = ++this.#buildGeneration;
    this.#selectionGeneration += 1;
    // Rebuilding discards the previous graph's export state entirely; an
    // in-flight render or save response for it can no longer publish.
    this.#renderGeneration += 1;
    this.#saveGeneration += 1;
    this.#transition({
      build: { kind: "building" },
      selection: { kind: "idle" },
      export: IDLE_EXPORT_STATE,
    });
    const response = await this.#bridge.build(request);
    if (generation !== this.#buildGeneration) return this.#state;
    if (response.result !== null) {
      this.#transition({
        build: { kind: "ready", projection: response.result },
        selection: { kind: "idle" },
        export: IDLE_EXPORT_STATE,
      });
      return this.#state;
    }
    this.#transition({
      build: {
        kind: "failed",
        message: firstError(
          response.diagnostics,
          "The native owner rejected the authored history.",
        ),
        diagnostics: response.diagnostics,
      },
      selection: { kind: "idle" },
      export: IDLE_EXPORT_STATE,
    });
    return this.#state;
  }

  /**
   * Send one opaque graph-bound handle back for native replay/resolve. The
   * handle must belong to the current projection's admitted face inventory.
   */
  async select(handleHex: string): Promise<CadAuthoredSessionState> {
    const build = this.#state.build;
    if (build.kind !== "ready") return this.#state;
    const projection = build.projection;
    const admitted =
      projection.faces.some((face) => face.handleHex === handleHex) ||
      lineageHandles(projection).includes(handleHex);
    if (!admitted) {
      this.#transition({
        build,
        selection: {
          kind: "failed",
          message: "The requested face handle is absent from this authored graph.",
        },
        export: this.#state.export,
      });
      return this.#state;
    }
    const generation = ++this.#selectionGeneration;
    this.#transition({
      build,
      selection: { kind: "resolving", handleHex },
      export: this.#state.export,
    });
    const response = await this.#bridge.resolve({
      protocol: CAD_AUTHORED_PROTOCOL,
      graphDigest: projection.graphDigest,
      canonicalGraphHex: projection.canonicalGraphHex,
      handleHex,
    });
    if (generation !== this.#selectionGeneration) return this.#state;
    const current = this.#state.build;
    if (current.kind !== "ready" || current.projection.graphDigest !== projection.graphDigest) {
      return this.#state;
    }
    if (
      response.result === null ||
      !selectionMatchesProjectionFace(projection, handleHex, response.result)
    ) {
      this.#transition({
        build: current,
        selection: {
          kind: "failed",
          message: firstError(
            response.diagnostics,
            "The native owner did not replay this face selection.",
          ),
        },
        export: this.#state.export,
      });
      return this.#state;
    }
    this.#transition({
      build: current,
      selection: { kind: "selected", selection: response.result },
      export: this.#state.export,
    });
    return this.#state;
  }

  /**
   * Ask the native owner to render the current accepted graph as Python.
   * The request carries only the opaque canonical bytes and exact digest;
   * a response for a superseded graph or generation is discarded.
   */
  async renderPython(): Promise<CadAuthoredSessionState> {
    const request = this.#currentExportRequest();
    if (request === null) return this.#state;
    const generation = ++this.#renderGeneration;
    this.#transitionExport({ preview: { kind: "rendering" } });
    const response = await this.#bridge.renderPython(request);
    if (generation !== this.#renderGeneration || !this.#exportGraphIsCurrent(request.graphDigest)) {
      return this.#state;
    }
    if (response.result === null || response.result.graphDigest !== request.graphDigest) {
      this.#transitionExport({
        preview: {
          kind: "failed",
          message: firstError(
            response.diagnostics,
            "The native owner did not render this authored graph as Python.",
          ),
        },
      });
      return this.#state;
    }
    this.#transitionExport({ preview: { kind: "ready", render: response.result } });
    return this.#state;
  }

  /**
   * Ask the native save boundary to write the current accepted graph's
   * rendering through its own dialog. Cancellation is a normal outcome;
   * a stale outcome cannot replace the current graph's export state.
   */
  async savePython(): Promise<CadAuthoredSessionState> {
    const request = this.#currentExportRequest();
    if (request === null) return this.#state;
    const generation = ++this.#saveGeneration;
    this.#transitionExport({ save: { kind: "saving" } });
    const response = await this.#bridge.savePython(request);
    if (generation !== this.#saveGeneration || !this.#exportGraphIsCurrent(request.graphDigest)) {
      return this.#state;
    }
    if (response.result === null || response.result.graphDigest !== request.graphDigest) {
      this.#transitionExport({
        save: {
          kind: "failed",
          message: firstError(
            response.diagnostics,
            "The native owner did not save the exported Python file.",
          ),
        },
      });
      return this.#state;
    }
    this.#transitionExport(
      response.result.status === "saved"
        ? { save: { kind: "saved", graphDigest: response.result.graphDigest } }
        : { save: { kind: "cancelled" } },
    );
    return this.#state;
  }

  /** The exact digest-bound request for the current accepted projection. */
  #currentExportRequest(): CadAuthoredExportRequest | null {
    const build = this.#state.build;
    if (build.kind !== "ready") return null;
    return {
      protocol: CAD_AUTHORED_EXPORT_PROTOCOL,
      canonicalGraphHex: build.projection.canonicalGraphHex,
      graphDigest: build.projection.graphDigest,
    };
  }

  /** A response publishes only while its exact graph remains current. */
  #exportGraphIsCurrent(graphDigest: string): boolean {
    const current = this.#state.build;
    return current.kind === "ready" && current.projection.graphDigest === graphDigest;
  }

  #transitionExport(patch: Partial<CadAuthoredExportState>): void {
    this.#transition({ ...this.#state, export: { ...this.#state.export, ...patch } });
  }

  #transition(state: CadAuthoredSessionState): void {
    this.#state = state;
    this.#observer(state);
  }
}

/**
 * A selection is admitted only when the owner's echo agrees with the current
 * projection on every semantic field: graph digest, opaque handle, provenance
 * key, and each returned face observation. A resolve that returns a different
 * face's meaning under the right handle fails closed.
 */
function selectionMatchesProjectionFace(
  projection: CadAuthoredProjection,
  handleHex: string,
  result: CadAuthoredSelectionResult,
): boolean {
  const face = projection.faces.find((candidate) => candidate.handleHex === handleHex);
  return (
    face !== undefined &&
    result.graphDigest === projection.graphDigest &&
    result.handleHex === handleHex &&
    result.provenanceKey === face.provenanceKey &&
    result.areaM2 === face.areaM2 &&
    result.boundaryLoopCount === face.boundaryLoopCount &&
    nullableVec3Equals(result.centroidM, face.centroidM) &&
    nullableVec3Equals(result.outwardUnitNormal, face.outwardUnitNormal)
  );
}

function nullableVec3Equals(
  left: readonly [number, number, number] | null,
  right: readonly [number, number, number] | null,
): boolean {
  if (left === null || right === null) return left === right;
  return left[0] === right[0] && left[1] === right[1] && left[2] === right[2];
}

function lineageHandles(projection: CadAuthoredProjection): string[] {
  const lineage = projection.build.lineage;
  return [
    ...lineage.retainedUnchanged,
    ...lineage.retainedModified,
    ...lineage.created,
    ...lineage.deleted,
    ...lineage.split,
    ...lineage.merged,
  ].map((entry) => entry.handleHex);
}

function firstError(diagnostics: readonly StudioDiagnostic[], fallback: string): string {
  return diagnostics.find((diagnostic) => diagnostic.severity === "error")?.message ?? fallback;
}
