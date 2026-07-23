import type { SpatialRunResult } from "./protocol";
import type { ScalarFieldBridgeFailure, ScalarFieldDataBridge } from "./scalar-field-bridge";
import {
  descriptorMatchesAcceptedResult,
  type ScalarFieldDescriptor,
  scalarFieldChunkValueCount,
} from "./scalar-field-protocol";

export type ScalarFieldSessionFailureCode =
  | "open-rejected"
  | "descriptor-mismatch"
  | "chunk-rejected"
  | "chunk-order-mismatch"
  | "chunk-size-mismatch"
  | "chunk-nonfinite"
  | "field-range-mismatch";

export type ScalarFieldSessionFailure = Readonly<{
  code: ScalarFieldSessionFailureCode;
  message: string;
  cause: ScalarFieldBridgeFailure | null;
}>;

type ScalarFieldContext = SpatialRunResult;

export type ScalarFieldSessionState =
  | Readonly<{ kind: "idle"; context: ScalarFieldContext | null }>
  | Readonly<{
      kind: "opening";
      context: ScalarFieldContext;
      requestId: number;
    }>
  | Readonly<{
      kind: "streaming";
      context: ScalarFieldContext;
      descriptor: ScalarFieldDescriptor;
      nextChunkIndex: number;
      receivedValueCount: number;
      observedMinimum: number | null;
      observedMaximum: number | null;
      inFlight: Readonly<{ requestId: number; chunkIndex: number }> | null;
    }>
  | Readonly<{
      kind: "ready";
      context: ScalarFieldContext;
      descriptor: ScalarFieldDescriptor;
      values: Float64Array;
    }>
  | Readonly<{
      kind: "failed";
      context: ScalarFieldContext;
      failure: ScalarFieldSessionFailure;
    }>;

const bufferedChunks = Symbol("scalar-field-buffered-chunks");
type StreamingState = Extract<ScalarFieldSessionState, { kind: "streaming" }>;
type InternalStreamingState = StreamingState & {
  readonly [bufferedChunks]: readonly Float64Array[];
};

export type ScalarFieldSessionAction =
  | Readonly<{ type: "context-changed"; result: SpatialRunResult | null }>
  | Readonly<{
      type: "open-started";
      requestId: number;
      result: SpatialRunResult;
    }>
  | Readonly<{
      type: "open-succeeded";
      requestId: number;
      result: SpatialRunResult;
      descriptor: ScalarFieldDescriptor;
    }>
  | Readonly<{
      type: "open-failed";
      requestId: number;
      result: SpatialRunResult;
      failure: ScalarFieldBridgeFailure;
    }>
  | Readonly<{
      type: "chunk-started";
      requestId: number;
      descriptor: ScalarFieldDescriptor;
      chunkIndex: number;
    }>
  | Readonly<{
      type: "chunk-succeeded";
      requestId: number;
      chunkIndex: number;
      values: Float64Array;
    }>
  | Readonly<{
      type: "chunk-failed";
      requestId: number;
      chunkIndex: number;
      failure: ScalarFieldBridgeFailure;
    }>;

export function initialScalarFieldSessionState(
  context: SpatialRunResult | null = null,
): ScalarFieldSessionState {
  return { kind: "idle", context };
}

function sameContext(left: SpatialRunResult, right: SpatialRunResult): boolean {
  return (
    left.digest === right.digest &&
    left.runId === right.runId &&
    left.plan.key === right.plan.key &&
    left.plan.requirements.spatialDimension === right.plan.requirements.spatialDimension &&
    left.plan.discretization.method === right.plan.discretization.method &&
    left.plan.discretization.cellsPerAxis === right.plan.discretization.cellsPerAxis &&
    left.field.location === right.field.location &&
    left.field.valueCount === right.field.valueCount &&
    Object.is(left.field.minimum, right.field.minimum) &&
    Object.is(left.field.maximum, right.field.maximum)
  );
}

function sameDescriptor(left: ScalarFieldDescriptor, right: ScalarFieldDescriptor): boolean {
  return (
    left.modelDigest === right.modelDigest &&
    left.runId === right.runId &&
    left.planKey === right.planKey &&
    left.field.id === right.field.id &&
    left.field.name === right.field.name &&
    left.field.dimension === right.field.dimension &&
    left.field.coherentSiUnit === right.field.coherentSiUnit &&
    left.field.scalarType === right.field.scalarType &&
    left.field.location === right.field.location &&
    left.field.valueCount === right.field.valueCount &&
    Object.is(left.field.minimum, right.field.minimum) &&
    Object.is(left.field.maximum, right.field.maximum) &&
    left.domain.id === right.domain.id &&
    left.domain.boundsM[0][0] === right.domain.boundsM[0][0] &&
    left.domain.boundsM[0][1] === right.domain.boundsM[0][1] &&
    left.domain.boundsM[1][0] === right.domain.boundsM[1][0] &&
    left.domain.boundsM[1][1] === right.domain.boundsM[1][1] &&
    left.grid.kind === right.grid.kind &&
    left.grid.logicalShape[0] === right.grid.logicalShape[0] &&
    left.grid.logicalShape[1] === right.grid.logicalShape[1] &&
    left.grid.order === right.grid.order &&
    left.transport.kind === right.transport.kind &&
    left.transport.encoding === right.transport.encoding &&
    left.transport.valuesPerChunk === right.transport.valuesPerChunk &&
    left.transport.chunkCount === right.transport.chunkCount
  );
}

function sessionFailure(
  context: SpatialRunResult,
  code: ScalarFieldSessionFailureCode,
  message: string,
  cause: ScalarFieldBridgeFailure | null = null,
): ScalarFieldSessionState {
  return { kind: "failed", context, failure: { code, message, cause } };
}

function internalStreaming(state: StreamingState): InternalStreamingState {
  return state as InternalStreamingState;
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

function joinChunks(chunks: readonly Float64Array[], valueCount: number): Float64Array | null {
  const values = new Float64Array(valueCount);
  let offset = 0;
  for (const chunk of chunks) {
    if (offset + chunk.length > valueCount) return null;
    values.set(chunk, offset);
    offset += chunk.length;
  }
  return offset === valueCount ? values : null;
}

export function scalarFieldSessionReducer(
  state: ScalarFieldSessionState,
  action: ScalarFieldSessionAction,
): ScalarFieldSessionState {
  switch (action.type) {
    case "context-changed":
      if (action.result === null && state.kind === "idle" && state.context === null) {
        return state;
      }
      if (
        action.result !== null &&
        state.context !== null &&
        sameContext(state.context, action.result)
      ) {
        return state;
      }
      return initialScalarFieldSessionState(action.result);

    case "open-started":
      if (state.context === null || !sameContext(state.context, action.result)) return state;
      return {
        kind: "opening",
        context: state.context,
        requestId: action.requestId,
      };

    case "open-succeeded": {
      if (
        state.kind !== "opening" ||
        state.requestId !== action.requestId ||
        !sameContext(state.context, action.result)
      ) {
        return state;
      }
      if (!descriptorMatchesAcceptedResult(state.context, action.descriptor)) {
        return sessionFailure(
          state.context,
          "descriptor-mismatch",
          "Scalar-field descriptor differs from the exact accepted result.",
        );
      }
      const streaming: InternalStreamingState = {
        kind: "streaming",
        context: state.context,
        descriptor: action.descriptor,
        nextChunkIndex: 0,
        receivedValueCount: 0,
        observedMinimum: null,
        observedMaximum: null,
        inFlight: null,
        [bufferedChunks]: [],
      };
      return streaming;
    }

    case "open-failed":
      if (
        state.kind !== "opening" ||
        state.requestId !== action.requestId ||
        !sameContext(state.context, action.result)
      ) {
        return state;
      }
      return sessionFailure(
        state.context,
        "open-rejected",
        "Scalar-field descriptor could not be opened.",
        action.failure,
      );

    case "chunk-started":
      if (state.kind !== "streaming") return state;
      if (!sameDescriptor(state.descriptor, action.descriptor)) {
        return sessionFailure(
          state.context,
          "descriptor-mismatch",
          "Scalar-field chunk request names a foreign descriptor.",
        );
      }
      if (state.inFlight !== null) {
        return sessionFailure(
          state.context,
          "chunk-order-mismatch",
          "Only one scalar-field chunk may be in flight.",
        );
      }
      if (action.chunkIndex !== state.nextChunkIndex) {
        return sessionFailure(
          state.context,
          "chunk-order-mismatch",
          "Scalar-field chunks must be requested once in canonical order.",
        );
      }
      return {
        ...state,
        inFlight: { requestId: action.requestId, chunkIndex: action.chunkIndex },
      };

    case "chunk-succeeded": {
      if (state.kind !== "streaming") return state;
      if (state.inFlight === null || state.inFlight.requestId !== action.requestId) {
        return sessionFailure(
          state.context,
          "chunk-order-mismatch",
          "Scalar-field chunk response does not match the one current request.",
        );
      }
      if (
        state.inFlight.chunkIndex !== action.chunkIndex ||
        action.chunkIndex !== state.nextChunkIndex
      ) {
        return sessionFailure(
          state.context,
          "chunk-order-mismatch",
          "Scalar-field chunk response arrived out of canonical order.",
        );
      }
      const expectedValueCount = scalarFieldChunkValueCount(state.descriptor, action.chunkIndex);
      if (expectedValueCount === null || action.values.length !== expectedValueCount) {
        return sessionFailure(
          state.context,
          "chunk-size-mismatch",
          "Scalar-field chunk length differs from its descriptor.",
        );
      }
      const extrema = finiteExtrema(action.values);
      if (extrema === null) {
        return sessionFailure(
          state.context,
          "chunk-nonfinite",
          "Scalar-field chunk contains a non-finite value.",
        );
      }

      const chunks = [...internalStreaming(state)[bufferedChunks], action.values.slice()];
      const receivedValueCount = state.receivedValueCount + action.values.length;
      const observedMinimum =
        state.observedMinimum === null
          ? extrema.minimum
          : Math.min(state.observedMinimum, extrema.minimum);
      const observedMaximum =
        state.observedMaximum === null
          ? extrema.maximum
          : Math.max(state.observedMaximum, extrema.maximum);
      const nextChunkIndex = state.nextChunkIndex + 1;

      if (nextChunkIndex < state.descriptor.transport.chunkCount) {
        const streaming: InternalStreamingState = {
          ...state,
          nextChunkIndex,
          receivedValueCount,
          observedMinimum,
          observedMaximum,
          inFlight: null,
          [bufferedChunks]: chunks,
        };
        return streaming;
      }

      if (
        receivedValueCount !== state.descriptor.field.valueCount ||
        !Object.is(observedMinimum, state.descriptor.field.minimum) ||
        !Object.is(observedMaximum, state.descriptor.field.maximum)
      ) {
        return sessionFailure(
          state.context,
          "field-range-mismatch",
          "Completed scalar-field values differ from the accepted range or value count.",
        );
      }
      const values = joinChunks(chunks, state.descriptor.field.valueCount);
      if (values === null) {
        return sessionFailure(
          state.context,
          "chunk-size-mismatch",
          "Completed scalar-field chunks do not form the declared field.",
        );
      }
      return {
        kind: "ready",
        context: state.context,
        descriptor: state.descriptor,
        values,
      };
    }

    case "chunk-failed":
      if (state.kind !== "streaming") return state;
      if (state.inFlight === null || state.inFlight.requestId !== action.requestId) {
        return sessionFailure(
          state.context,
          "chunk-order-mismatch",
          "Scalar-field chunk failure does not match the one current request.",
        );
      }
      if (state.inFlight.chunkIndex !== action.chunkIndex) {
        return sessionFailure(
          state.context,
          "chunk-order-mismatch",
          "Scalar-field chunk failure was routed from another request.",
        );
      }
      return sessionFailure(
        state.context,
        "chunk-rejected",
        "Scalar-field chunk could not be read.",
        action.failure,
      );
  }
}

export type ScalarFieldSessionObserver = (state: ScalarFieldSessionState) => void;

/**
 * Serial loader for one current result. Changing context invalidates every outstanding await.
 * The observer can only obtain materialized values from the terminal `ready` state.
 */
export class ScalarFieldDataSession {
  readonly #bridge: ScalarFieldDataBridge;
  readonly #observer: ScalarFieldSessionObserver;
  #generation = 0;
  #requestSequence = 0;
  #state: ScalarFieldSessionState = initialScalarFieldSessionState();

  constructor(bridge: ScalarFieldDataBridge, observer: ScalarFieldSessionObserver = () => {}) {
    this.#bridge = bridge;
    this.#observer = observer;
  }

  get state(): ScalarFieldSessionState {
    return this.#state;
  }

  setContext(result: SpatialRunResult | null): void {
    this.#generation += 1;
    this.#transition({ type: "context-changed", result });
  }

  clear(): void {
    this.setContext(null);
  }

  async load(result: SpatialRunResult): Promise<ScalarFieldSessionState> {
    const generation = ++this.#generation;
    this.#transition({ type: "context-changed", result });
    const openRequestId = ++this.#requestSequence;
    this.#transition({ type: "open-started", requestId: openRequestId, result });
    const opened = await this.#bridge.open(result);
    if (generation !== this.#generation) return this.#state;
    if (!opened.ok) {
      this.#transition({
        type: "open-failed",
        requestId: openRequestId,
        result,
        failure: opened.failure,
      });
      return this.#state;
    }
    this.#transition({
      type: "open-succeeded",
      requestId: openRequestId,
      result,
      descriptor: opened.value,
    });

    while (this.#state.kind === "streaming") {
      const descriptor = this.#state.descriptor;
      const chunkIndex = this.#state.nextChunkIndex;
      const chunkRequestId = ++this.#requestSequence;
      this.#transition({
        type: "chunk-started",
        requestId: chunkRequestId,
        descriptor,
        chunkIndex,
      });
      const chunk = await this.#bridge.readChunk(descriptor, chunkIndex);
      if (generation !== this.#generation) return this.#state;
      if (!chunk.ok) {
        this.#transition({
          type: "chunk-failed",
          requestId: chunkRequestId,
          chunkIndex,
          failure: chunk.failure,
        });
        return this.#state;
      }
      this.#transition({
        type: "chunk-succeeded",
        requestId: chunkRequestId,
        chunkIndex,
        values: chunk.value,
      });
    }
    return this.#state;
  }

  #transition(action: ScalarFieldSessionAction): void {
    const next = scalarFieldSessionReducer(this.#state, action);
    if (next === this.#state) return;
    this.#state = next;
    this.#observer(next);
  }
}
