import type {
  UnstructuredFieldBridgeFailure,
  UnstructuredFieldChunk,
  UnstructuredFieldDataBridge,
} from "./unstructured-field-bridge";
import {
  type UnstructuredFieldContext,
  type UnstructuredFieldDescriptor,
  type UnstructuredFieldStream,
  unstructuredDescriptorMatchesContext,
  unstructuredFieldChunkItemCount,
  unstructuredFieldContextsEqual,
  unstructuredFieldDescriptorsEqual,
} from "./unstructured-field-protocol";

export type UnstructuredFieldSessionFailureCode =
  | "open-rejected"
  | "descriptor-mismatch"
  | "chunk-rejected"
  | "chunk-order-mismatch"
  | "chunk-size-mismatch"
  | "chunk-nonfinite"
  | "coordinate-bounds-mismatch"
  | "connectivity-invalid"
  | "field-range-mismatch";

export type UnstructuredFieldSessionFailure = Readonly<{
  code: UnstructuredFieldSessionFailureCode;
  message: string;
  cause: UnstructuredFieldBridgeFailure | null;
}>;

type StreamingState = Readonly<{
  kind: "streaming";
  context: UnstructuredFieldContext;
  descriptor: UnstructuredFieldDescriptor;
  stream: UnstructuredFieldStream;
  nextChunkIndex: number;
  receivedItemCount: number;
  inFlight: Readonly<{
    requestId: number;
    stream: UnstructuredFieldStream;
    chunkIndex: number;
  }> | null;
}>;

export type UnstructuredFieldSessionState =
  | Readonly<{ kind: "idle"; context: UnstructuredFieldContext | null }>
  | Readonly<{
      kind: "opening";
      context: UnstructuredFieldContext;
      requestId: number;
    }>
  | StreamingState
  | Readonly<{
      kind: "ready";
      context: UnstructuredFieldContext;
      descriptor: UnstructuredFieldDescriptor;
      coordinates: Float64Array;
      triangles: Uint32Array;
      values: Float64Array;
    }>
  | Readonly<{
      kind: "failed";
      context: UnstructuredFieldContext;
      failure: UnstructuredFieldSessionFailure;
    }>;

const bufferedStreams = Symbol("unstructured-field-buffered-streams");
type StreamBuffers = Readonly<{
  coordinates: readonly Float64Array[];
  triangles: readonly Uint32Array[];
  values: readonly Float64Array[];
}>;
type InternalStreamingState = StreamingState & {
  readonly [bufferedStreams]: StreamBuffers;
};

export type UnstructuredFieldSessionAction =
  | Readonly<{ type: "context-changed"; context: UnstructuredFieldContext | null }>
  | Readonly<{
      type: "open-started";
      requestId: number;
      context: UnstructuredFieldContext;
    }>
  | Readonly<{
      type: "open-succeeded";
      requestId: number;
      context: UnstructuredFieldContext;
      descriptor: UnstructuredFieldDescriptor;
    }>
  | Readonly<{
      type: "open-failed";
      requestId: number;
      context: UnstructuredFieldContext;
      failure: UnstructuredFieldBridgeFailure;
    }>
  | Readonly<{
      type: "chunk-started";
      requestId: number;
      descriptor: UnstructuredFieldDescriptor;
      stream: UnstructuredFieldStream;
      chunkIndex: number;
    }>
  | Readonly<{
      type: "chunk-succeeded";
      requestId: number;
      chunk: UnstructuredFieldChunk;
      chunkIndex: number;
    }>
  | Readonly<{
      type: "chunk-failed";
      requestId: number;
      stream: UnstructuredFieldStream;
      chunkIndex: number;
      failure: UnstructuredFieldBridgeFailure;
    }>;

const streamOrder = ["coordinates", "triangles", "values"] as const;

export function initialUnstructuredFieldSessionState(
  context: UnstructuredFieldContext | null = null,
): UnstructuredFieldSessionState {
  return { kind: "idle", context };
}

export function unstructuredFieldSessionReducer(
  state: UnstructuredFieldSessionState,
  action: UnstructuredFieldSessionAction,
): UnstructuredFieldSessionState {
  switch (action.type) {
    case "context-changed":
      if (action.context === null && state.kind === "idle" && state.context === null) return state;
      if (
        action.context !== null &&
        state.context !== null &&
        sameContext(state.context, action.context)
      ) {
        return state;
      }
      return initialUnstructuredFieldSessionState(action.context);

    case "open-started":
      if (state.context === null || !sameContext(state.context, action.context)) return state;
      return { kind: "opening", context: state.context, requestId: action.requestId };

    case "open-succeeded": {
      if (
        state.kind !== "opening" ||
        state.requestId !== action.requestId ||
        !sameContext(state.context, action.context)
      ) {
        return state;
      }
      if (!unstructuredDescriptorMatchesContext(state.context, action.descriptor)) {
        return sessionFailure(
          state.context,
          "descriptor-mismatch",
          "Unstructured descriptor differs from the exact accepted context.",
        );
      }
      return streamingState(state.context, action.descriptor, emptyBuffers());
    }

    case "open-failed":
      if (
        state.kind !== "opening" ||
        state.requestId !== action.requestId ||
        !sameContext(state.context, action.context)
      ) {
        return state;
      }
      return sessionFailure(
        state.context,
        "open-rejected",
        "Unstructured field descriptor could not be opened.",
        action.failure,
      );

    case "chunk-started":
      if (state.kind !== "streaming") return state;
      if (!sameDescriptor(state.descriptor, action.descriptor)) {
        return sessionFailure(
          state.context,
          "descriptor-mismatch",
          "Unstructured chunk request names a foreign descriptor.",
        );
      }
      if (
        state.inFlight !== null ||
        action.stream !== state.stream ||
        action.chunkIndex !== state.nextChunkIndex
      ) {
        return sessionFailure(
          state.context,
          "chunk-order-mismatch",
          "Unstructured chunks must be requested once in canonical stream order.",
        );
      }
      return {
        ...state,
        inFlight: {
          requestId: action.requestId,
          stream: action.stream,
          chunkIndex: action.chunkIndex,
        },
      };

    case "chunk-succeeded":
      return acceptChunk(state, action);

    case "chunk-failed":
      if (state.kind !== "streaming") return state;
      if (
        state.inFlight === null ||
        state.inFlight.requestId !== action.requestId ||
        state.inFlight.stream !== action.stream ||
        state.inFlight.chunkIndex !== action.chunkIndex
      ) {
        return sessionFailure(
          state.context,
          "chunk-order-mismatch",
          "Unstructured chunk failure belongs to another request.",
        );
      }
      return sessionFailure(
        state.context,
        "chunk-rejected",
        "Unstructured field chunk could not be read.",
        action.failure,
      );
  }
}

export type UnstructuredFieldSessionObserver = (state: UnstructuredFieldSessionState) => void;

/** Serial fail-closed loader for one exact unstructured projection. */
export class UnstructuredFieldDataSession {
  readonly #bridge: UnstructuredFieldDataBridge;
  readonly #observer: UnstructuredFieldSessionObserver;
  #generation = 0;
  #requestSequence = 0;
  #state: UnstructuredFieldSessionState = initialUnstructuredFieldSessionState();

  constructor(
    bridge: UnstructuredFieldDataBridge,
    observer: UnstructuredFieldSessionObserver = () => {},
  ) {
    this.#bridge = bridge;
    this.#observer = observer;
  }

  get state(): UnstructuredFieldSessionState {
    return this.#state;
  }

  setContext(context: UnstructuredFieldContext | null): void {
    this.#generation += 1;
    this.#transition({ type: "context-changed", context });
  }

  clear(): void {
    this.setContext(null);
  }

  async load(context: UnstructuredFieldContext): Promise<UnstructuredFieldSessionState> {
    const generation = ++this.#generation;
    this.#transition({ type: "context-changed", context });
    const openRequestId = ++this.#requestSequence;
    this.#transition({ type: "open-started", requestId: openRequestId, context });
    const opened = await this.#bridge.open(context);
    if (generation !== this.#generation) return this.#state;
    if (!opened.ok) {
      this.#transition({
        type: "open-failed",
        requestId: openRequestId,
        context,
        failure: opened.failure,
      });
      return this.#state;
    }
    this.#transition({
      type: "open-succeeded",
      requestId: openRequestId,
      context,
      descriptor: opened.value,
    });

    while (this.#state.kind === "streaming") {
      const descriptor = this.#state.descriptor;
      const stream = this.#state.stream;
      const chunkIndex = this.#state.nextChunkIndex;
      const requestId = ++this.#requestSequence;
      this.#transition({
        type: "chunk-started",
        requestId,
        descriptor,
        stream,
        chunkIndex,
      });
      const chunk = await this.#bridge.readChunk(descriptor, stream, chunkIndex);
      if (generation !== this.#generation) return this.#state;
      if (!chunk.ok) {
        this.#transition({
          type: "chunk-failed",
          requestId,
          stream,
          chunkIndex,
          failure: chunk.failure,
        });
        return this.#state;
      }
      this.#transition({
        type: "chunk-succeeded",
        requestId,
        chunkIndex,
        chunk: chunk.value,
      });
    }
    return this.#state;
  }

  #transition(action: UnstructuredFieldSessionAction): void {
    const next = unstructuredFieldSessionReducer(this.#state, action);
    if (next === this.#state) return;
    this.#state = next;
    this.#observer(next);
  }
}

function acceptChunk(
  state: UnstructuredFieldSessionState,
  action: Extract<UnstructuredFieldSessionAction, { type: "chunk-succeeded" }>,
): UnstructuredFieldSessionState {
  if (state.kind !== "streaming") return state;
  if (
    state.inFlight === null ||
    state.inFlight.requestId !== action.requestId ||
    state.inFlight.stream !== action.chunk.stream ||
    state.inFlight.chunkIndex !== action.chunkIndex ||
    state.stream !== action.chunk.stream ||
    state.nextChunkIndex !== action.chunkIndex
  ) {
    return sessionFailure(
      state.context,
      "chunk-order-mismatch",
      "Unstructured chunk response arrived outside canonical stream order.",
    );
  }
  const expectedItems = unstructuredFieldChunkItemCount(
    state.descriptor,
    state.stream,
    action.chunkIndex,
  );
  const components = state.descriptor.transport[state.stream].components;
  if (expectedItems === null || action.chunk.values.length !== expectedItems * components) {
    return sessionFailure(
      state.context,
      "chunk-size-mismatch",
      "Unstructured chunk length differs from its descriptor.",
    );
  }
  if (
    action.chunk.values instanceof Float64Array &&
    action.chunk.values.some((value) => !Number.isFinite(value))
  ) {
    return sessionFailure(
      state.context,
      "chunk-nonfinite",
      "Unstructured f64 chunk contains a non-finite value.",
    );
  }
  if (
    action.chunk.stream === "triangles" &&
    action.chunk.values.some((vertex) => vertex >= state.descriptor.mesh.vertexCount)
  ) {
    return sessionFailure(
      state.context,
      "connectivity-invalid",
      "Unstructured triangle connectivity references a foreign vertex.",
    );
  }

  const buffers = appendBuffer(
    internalStreaming(state)[bufferedStreams],
    action.chunk.stream,
    action.chunk.values,
  );
  const receivedItemCount = state.receivedItemCount + expectedItems;
  const nextChunkIndex = state.nextChunkIndex + 1;
  const contract = state.descriptor.transport[state.stream];
  if (nextChunkIndex < contract.chunkCount) {
    return streamingState(
      state.context,
      state.descriptor,
      buffers,
      state.stream,
      nextChunkIndex,
      receivedItemCount,
    );
  }
  if (receivedItemCount !== contract.itemCount) {
    return sessionFailure(
      state.context,
      "chunk-size-mismatch",
      "Completed unstructured stream differs from its declared item count.",
    );
  }

  const validation = validateCompletedStream(state.descriptor, state.stream, buffers);
  if (validation !== null)
    return sessionFailure(state.context, validation.code, validation.message);

  const streamIndex = streamOrder.indexOf(state.stream);
  const nextStream = streamOrder[streamIndex + 1];
  if (nextStream !== undefined) {
    return streamingState(state.context, state.descriptor, buffers, nextStream);
  }

  const coordinates = joinF64(
    buffers.coordinates,
    state.descriptor.mesh.vertexCount * state.descriptor.transport.coordinates.components,
  );
  const triangles = joinU32(
    buffers.triangles,
    state.descriptor.mesh.triangleCount * state.descriptor.transport.triangles.components,
  );
  const values = joinF64(buffers.values, state.descriptor.field.valueCount);
  if (coordinates === null || triangles === null || values === null) {
    return sessionFailure(
      state.context,
      "chunk-size-mismatch",
      "Complete unstructured streams cannot be materialized at their declared shapes.",
    );
  }
  return {
    kind: "ready",
    context: state.context,
    descriptor: state.descriptor,
    coordinates,
    triangles,
    values,
  };
}

function validateCompletedStream(
  descriptor: UnstructuredFieldDescriptor,
  stream: UnstructuredFieldStream,
  buffers: StreamBuffers,
): Readonly<{
  code: UnstructuredFieldSessionFailureCode;
  message: string;
}> | null {
  if (stream === "coordinates") {
    const coordinates = joinF64(buffers.coordinates, descriptor.mesh.vertexCount * 2);
    if (coordinates === null) {
      return {
        code: "chunk-size-mismatch",
        message: "Coordinate chunks do not form the declared vertex array.",
      };
    }
    const bounds = finiteCoordinateBounds(coordinates);
    if (
      bounds === null ||
      bounds[0][0] !== descriptor.domain.boundsM[0][0] ||
      bounds[0][1] !== descriptor.domain.boundsM[0][1] ||
      bounds[1][0] !== descriptor.domain.boundsM[1][0] ||
      bounds[1][1] !== descriptor.domain.boundsM[1][1]
    ) {
      return {
        code: "coordinate-bounds-mismatch",
        message: "Completed coordinates differ from the accepted mesh bounds.",
      };
    }
  }
  if (stream === "triangles") {
    const triangles = joinU32(buffers.triangles, descriptor.mesh.triangleCount * 3);
    const coordinates = joinF64(buffers.coordinates, descriptor.mesh.vertexCount * 2);
    if (triangles === null || coordinates === null) {
      return {
        code: "chunk-size-mismatch",
        message: "Connectivity and coordinate chunks do not form the declared mesh.",
      };
    }
    for (let triangle = 0; triangle < descriptor.mesh.triangleCount; triangle += 1) {
      const a = triangles[triangle * 3];
      const b = triangles[triangle * 3 + 1];
      const c = triangles[triangle * 3 + 2];
      if (
        a === undefined ||
        b === undefined ||
        c === undefined ||
        a === b ||
        b === c ||
        c === a ||
        !(orientedAreaTwice(coordinates, a, b, c) > 0)
      ) {
        return {
          code: "connectivity-invalid",
          message: "Completed connectivity is not a positive affine-triangle mesh.",
        };
      }
    }
  }
  if (stream === "values") {
    const values = joinF64(buffers.values, descriptor.field.valueCount);
    const extrema = values === null ? null : finiteExtrema(values);
    if (
      extrema === null ||
      extrema.minimum !== descriptor.field.minimum ||
      extrema.maximum !== descriptor.field.maximum
    ) {
      return {
        code: "field-range-mismatch",
        message: "Completed values differ from the accepted scalar range.",
      };
    }
  }
  return null;
}

function streamingState(
  context: UnstructuredFieldContext,
  descriptor: UnstructuredFieldDescriptor,
  buffers: StreamBuffers,
  stream: UnstructuredFieldStream = "coordinates",
  nextChunkIndex = 0,
  receivedItemCount = 0,
): InternalStreamingState {
  return {
    kind: "streaming",
    context,
    descriptor,
    stream,
    nextChunkIndex,
    receivedItemCount,
    inFlight: null,
    [bufferedStreams]: buffers,
  };
}

function emptyBuffers(): StreamBuffers {
  return { coordinates: [], triangles: [], values: [] };
}

function appendBuffer(
  buffers: StreamBuffers,
  stream: UnstructuredFieldStream,
  values: Float64Array | Uint32Array,
): StreamBuffers {
  switch (stream) {
    case "coordinates":
      return {
        ...buffers,
        coordinates: [...buffers.coordinates, (values as Float64Array).slice()],
      };
    case "triangles":
      return { ...buffers, triangles: [...buffers.triangles, (values as Uint32Array).slice()] };
    case "values":
      return { ...buffers, values: [...buffers.values, (values as Float64Array).slice()] };
  }
}

function internalStreaming(state: StreamingState): InternalStreamingState {
  return state as InternalStreamingState;
}

function joinF64(chunks: readonly Float64Array[], scalarCount: number): Float64Array | null {
  const values = new Float64Array(scalarCount);
  return joinTyped(chunks, values);
}

function joinU32(chunks: readonly Uint32Array[], scalarCount: number): Uint32Array | null {
  const values = new Uint32Array(scalarCount);
  return joinTyped(chunks, values);
}

function joinTyped<T extends Float64Array | Uint32Array>(
  chunks: readonly T[],
  values: T,
): T | null {
  let offset = 0;
  for (const chunk of chunks) {
    if (offset + chunk.length > values.length) return null;
    values.set(chunk, offset);
    offset += chunk.length;
  }
  return offset === values.length ? values : null;
}

function finiteCoordinateBounds(
  coordinates: Float64Array,
): [[number, number], [number, number]] | null {
  const bounds: [[number, number], [number, number]] = [
    [Number.POSITIVE_INFINITY, Number.NEGATIVE_INFINITY],
    [Number.POSITIVE_INFINITY, Number.NEGATIVE_INFINITY],
  ];
  for (let index = 0; index < coordinates.length; index += 2) {
    const x = coordinates[index];
    const y = coordinates[index + 1];
    if (x === undefined || y === undefined || !Number.isFinite(x) || !Number.isFinite(y)) {
      return null;
    }
    bounds[0][0] = Math.min(bounds[0][0], x);
    bounds[0][1] = Math.max(bounds[0][1], x);
    bounds[1][0] = Math.min(bounds[1][0], y);
    bounds[1][1] = Math.max(bounds[1][1], y);
  }
  return bounds;
}

function finiteExtrema(values: Float64Array): { minimum: number; maximum: number } | null {
  let minimum = Number.POSITIVE_INFINITY;
  let maximum = Number.NEGATIVE_INFINITY;
  for (const value of values) {
    if (!Number.isFinite(value)) return null;
    minimum = Math.min(minimum, value);
    maximum = Math.max(maximum, value);
  }
  return { minimum, maximum };
}

function orientedAreaTwice(coordinates: Float64Array, a: number, b: number, c: number): number {
  const ax = coordinates[a * 2];
  const ay = coordinates[a * 2 + 1];
  const bx = coordinates[b * 2];
  const by = coordinates[b * 2 + 1];
  const cx = coordinates[c * 2];
  const cy = coordinates[c * 2 + 1];
  if (
    ax === undefined ||
    ay === undefined ||
    bx === undefined ||
    by === undefined ||
    cx === undefined ||
    cy === undefined
  ) {
    return Number.NaN;
  }
  return (bx - ax) * (cy - ay) - (by - ay) * (cx - ax);
}

function sameContext(left: UnstructuredFieldContext, right: UnstructuredFieldContext): boolean {
  return unstructuredFieldContextsEqual(left, right);
}

function sameDescriptor(
  left: UnstructuredFieldDescriptor,
  right: UnstructuredFieldDescriptor,
): boolean {
  return unstructuredFieldDescriptorsEqual(left, right);
}

function sessionFailure(
  context: UnstructuredFieldContext,
  code: UnstructuredFieldSessionFailureCode,
  message: string,
  cause: UnstructuredFieldBridgeFailure | null = null,
): UnstructuredFieldSessionState {
  return { kind: "failed", context, failure: { code, message, cause } };
}
