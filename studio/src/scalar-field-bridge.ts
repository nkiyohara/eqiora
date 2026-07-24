import { invoke } from "@tauri-apps/api/core";
import {
  descriptorMatchesAcceptedResult,
  SCALAR_FIELD_VALUES_PER_CHUNK,
  SCALAR_FIELD_VIEW_PROTOCOL,
  type ScalarFieldDescriptor,
  scalarFieldChunkRequest,
  scalarFieldChunkRequestSchema,
  scalarFieldChunkValueCount,
  scalarFieldDescriptorSchema,
  scalarFieldFailureEnvelopeSchema,
  scalarFieldOpenEnvelopeSchema,
  scalarFieldOpenRequest,
  scalarFieldOpenRequestSchema,
} from "./scalar-field-protocol";
import { type SpatialRunResult, spatialRunResultSchema } from "./spatial-protocol";

export type ScalarFieldBridgeFailureCode =
  | "invalid-result"
  | "invalid-request"
  | "invalid-descriptor"
  | "descriptor-mismatch"
  | "chunk-out-of-range"
  | "invalid-chunk"
  | "nonfinite-chunk"
  | "bridge-rejected";

export type ScalarFieldBridgeFailure = Readonly<{
  code: ScalarFieldBridgeFailureCode;
  message: string;
}>;

export type ScalarFieldBridgeResult<T> =
  | Readonly<{ ok: true; value: T }>
  | Readonly<{ ok: false; failure: ScalarFieldBridgeFailure }>;

export interface ScalarFieldDataBridge {
  open(result: SpatialRunResult): Promise<ScalarFieldBridgeResult<ScalarFieldDescriptor>>;
  readChunk(
    descriptor: ScalarFieldDescriptor,
    chunkIndex: number,
  ): Promise<ScalarFieldBridgeResult<Float64Array>>;
}

function failure(
  code: ScalarFieldBridgeFailureCode,
  message: string,
): ScalarFieldBridgeResult<never> {
  return { ok: false, failure: { code, message } };
}

function rejectedMessage(error: unknown, fallback: string): string {
  const envelope = scalarFieldFailureEnvelopeSchema.safeParse(error);
  if (envelope.success) {
    return (
      envelope.data.diagnostics.find((diagnostic) => diagnostic.severity === "error")?.message ??
      fallback
    );
  }
  return error instanceof Error ? error.message : fallback;
}

function binaryBytes(payload: unknown): Uint8Array | null {
  if (payload instanceof ArrayBuffer) return new Uint8Array(payload);
  if (payload instanceof Uint8Array) {
    return new Uint8Array(payload.buffer, payload.byteOffset, payload.byteLength);
  }
  return null;
}

/** Decode one exact little-endian f64 chunk, with no implicit coercion or truncation. */
export function decodeScalarFieldChunk(
  payload: unknown,
  expectedValueCount: number,
): ScalarFieldBridgeResult<Float64Array> {
  if (
    !Number.isSafeInteger(expectedValueCount) ||
    expectedValueCount <= 0 ||
    expectedValueCount > SCALAR_FIELD_VALUES_PER_CHUNK
  ) {
    return failure("invalid-chunk", "Scalar-field chunk has an invalid expected value count.");
  }
  const bytes = binaryBytes(payload);
  const expectedByteLength = expectedValueCount * Float64Array.BYTES_PER_ELEMENT;
  if (bytes === null || bytes.byteLength !== expectedByteLength) {
    return failure(
      "invalid-chunk",
      `Scalar-field chunk must contain exactly ${expectedByteLength} bytes.`,
    );
  }

  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const values = new Float64Array(expectedValueCount);
  for (let index = 0; index < expectedValueCount; index += 1) {
    const value = view.getFloat64(index * Float64Array.BYTES_PER_ELEMENT, true);
    if (!Number.isFinite(value)) {
      return failure("nonfinite-chunk", "Scalar-field chunk contains a non-finite value.");
    }
    values[index] = value;
  }
  return { ok: true, value: values };
}

/** Encode preview values through the same f64 little-endian boundary used by native data. */
export function encodeScalarFieldChunk(values: ArrayLike<number>): ArrayBuffer {
  const buffer = new ArrayBuffer(values.length * Float64Array.BYTES_PER_ELEMENT);
  const view = new DataView(buffer);
  for (let index = 0; index < values.length; index += 1) {
    view.setFloat64(index * Float64Array.BYTES_PER_ELEMENT, values[index] ?? Number.NaN, true);
  }
  return buffer;
}

async function invokeNative(command: string, request: unknown): Promise<unknown> {
  return invoke(command, { request });
}

export const nativeScalarFieldDataBridge: ScalarFieldDataBridge = {
  async open(result) {
    const accepted = spatialRunResultSchema.safeParse(result);
    if (!accepted.success) {
      return failure("invalid-result", "Scalar-field view requires a valid accepted run result.");
    }
    const request = scalarFieldOpenRequest(accepted.data);
    if (!scalarFieldOpenRequestSchema.safeParse(request).success) {
      return failure("invalid-request", "Scalar-field open request is not canonical.");
    }

    try {
      const response = await invokeNative("open_scalar_field_view", request);
      const envelope = scalarFieldOpenEnvelopeSchema.safeParse(response);
      if (!envelope.success) {
        return failure(
          "invalid-descriptor",
          "Native field-view bridge returned an invalid open envelope.",
        );
      }
      if (envelope.data.result === null) {
        return failure(
          "bridge-rejected",
          envelope.data.diagnostics.find((diagnostic) => diagnostic.severity === "error")
            ?.message ?? "Native scalar-field open was rejected.",
        );
      }
      if (!descriptorMatchesAcceptedResult(accepted.data, envelope.data.result)) {
        return failure(
          "descriptor-mismatch",
          "Native field descriptor differs from the exact accepted run result.",
        );
      }
      return { ok: true, value: envelope.data.result };
    } catch (error: unknown) {
      const detail = rejectedMessage(error, "Native scalar-field open was rejected.");
      return failure("bridge-rejected", `Native scalar-field open failed: ${detail}`);
    }
  },

  async readChunk(descriptor, chunkIndex) {
    const checkedDescriptor = scalarFieldDescriptorSchema.safeParse(descriptor);
    if (!checkedDescriptor.success) {
      return failure("invalid-descriptor", "Scalar-field descriptor is not canonical.");
    }
    const expectedValueCount = scalarFieldChunkValueCount(checkedDescriptor.data, chunkIndex);
    if (expectedValueCount === null) {
      return failure("chunk-out-of-range", "Scalar-field chunk index is out of range.");
    }
    const request = scalarFieldChunkRequest(checkedDescriptor.data, chunkIndex);
    if (!scalarFieldChunkRequestSchema.safeParse(request).success) {
      return failure("invalid-request", "Scalar-field chunk request is not canonical.");
    }

    try {
      const response = await invokeNative("read_scalar_field_chunk", request);
      return decodeScalarFieldChunk(response, expectedValueCount);
    } catch (error: unknown) {
      const detail = rejectedMessage(error, "Native scalar-field chunk read was rejected.");
      return failure("bridge-rejected", `Native scalar-field chunk read failed: ${detail}`);
    }
  },
};

const previewFieldId = "Field:solution";
const previewDomainId = "Domain:unit-square";

function previewDescriptor(result: SpatialRunResult): ScalarFieldDescriptor | null {
  const cellsPerAxis = result.plan.discretization.cellsPerAxis;
  const finiteElement = result.plan.discretization.method === "finite-element";
  const axis = finiteElement ? cellsPerAxis + 1 : cellsPerAxis;
  const decoded = scalarFieldDescriptorSchema.safeParse({
    protocol: SCALAR_FIELD_VIEW_PROTOCOL,
    modelDigest: result.digest,
    runId: result.runId,
    planKey: result.plan.key,
    field: {
      id: previewFieldId,
      name: "solution",
      dimension: "1",
      coherentSiUnit: "1",
      scalarType: "f64",
      location: finiteElement ? "vertex" : "cell-center",
      valueCount: result.field.valueCount,
      minimum: result.field.minimum,
      maximum: result.field.maximum,
    },
    domain: {
      id: previewDomainId,
      boundsM: [
        [0, 1],
        [0, 1],
      ],
    },
    grid: {
      kind: "uniform-cartesian-2d",
      logicalShape: [axis, axis],
      order: "row-major-last-axis-fastest",
    },
    transport: {
      kind: "explicit-owned-host-copy",
      encoding: "f64-le",
      valuesPerChunk: SCALAR_FIELD_VALUES_PER_CHUNK,
      chunkCount: Math.ceil(result.field.valueCount / SCALAR_FIELD_VALUES_PER_CHUNK),
    },
  });
  return decoded.success && descriptorMatchesAcceptedResult(result, decoded.data)
    ? decoded.data
    : null;
}

function previewDescriptorKey(descriptor: ScalarFieldDescriptor): string {
  return `${descriptor.modelDigest}\0${descriptor.runId}\0${descriptor.planKey}`;
}

function samePreviewDescriptor(left: ScalarFieldDescriptor, right: ScalarFieldDescriptor): boolean {
  return JSON.stringify(left) === JSON.stringify(right);
}

function axisExtrema(axisSize: number, location: "vertex" | "cell-center") {
  let minimum = Number.POSITIVE_INFINITY;
  let maximum = Number.NEGATIVE_INFINITY;
  for (let index = 0; index < axisSize; index += 1) {
    const coordinate = location === "vertex" ? index / (axisSize - 1) : (index + 0.5) / axisSize;
    const value = Math.sin(Math.PI * coordinate);
    minimum = Math.min(minimum, value);
    maximum = Math.max(maximum, value);
  }
  return { minimum, maximum };
}

function previewValue(
  descriptor: ScalarFieldDescriptor,
  flatIndex: number,
  rawMinimum: number,
  rawMaximum: number,
): number {
  const [rows, columns] = descriptor.grid.logicalShape;
  const row = Math.floor(flatIndex / columns);
  const column = flatIndex % columns;
  const coordinate = (index: number, size: number) =>
    descriptor.field.location === "vertex" ? index / (size - 1) : (index + 0.5) / size;
  const raw =
    Math.sin(Math.PI * coordinate(row, rows)) * Math.sin(Math.PI * coordinate(column, columns));
  if (Object.is(raw, rawMinimum)) return descriptor.field.minimum;
  if (Object.is(raw, rawMaximum)) return descriptor.field.maximum;
  if (Object.is(rawMinimum, rawMaximum)) return descriptor.field.minimum;
  const normalized = (raw - rawMinimum) / (rawMaximum - rawMinimum);
  return (
    descriptor.field.minimum + normalized * (descriptor.field.maximum - descriptor.field.minimum)
  );
}

/** Create an isolated browser-preview bridge, including its exact opened-descriptor set. */
export function createPreviewScalarFieldDataBridge(): ScalarFieldDataBridge {
  const openedDescriptors = new Map<string, ScalarFieldDescriptor>();
  return {
    async open(result) {
      const accepted = spatialRunResultSchema.safeParse(result);
      if (!accepted.success || accepted.data.plan.requirements.spatialDimension !== 2) {
        return failure(
          "invalid-result",
          "Browser field preview requires an accepted two-dimensional scalar result.",
        );
      }
      const descriptor = previewDescriptor(accepted.data);
      if (descriptor === null) {
        return failure(
          "descriptor-mismatch",
          "Browser field preview cannot describe this accepted result exactly.",
        );
      }
      const key = previewDescriptorKey(descriptor);
      openedDescriptors.delete(key);
      openedDescriptors.set(key, descriptor);
      while (openedDescriptors.size > 2) {
        const oldest = openedDescriptors.keys().next().value;
        if (oldest === undefined) break;
        openedDescriptors.delete(oldest);
      }
      await Promise.resolve();
      return { ok: true, value: descriptor };
    },

    async readChunk(descriptor, chunkIndex) {
      const decoded = scalarFieldDescriptorSchema.safeParse(descriptor);
      if (!decoded.success) {
        return failure("invalid-descriptor", "Scalar-field descriptor is not canonical.");
      }
      const opened = openedDescriptors.get(previewDescriptorKey(decoded.data));
      if (opened === undefined || !samePreviewDescriptor(opened, decoded.data)) {
        return failure(
          "descriptor-mismatch",
          "Scalar-field descriptor is foreign to this browser-preview session.",
        );
      }
      const expectedValueCount = scalarFieldChunkValueCount(decoded.data, chunkIndex);
      if (expectedValueCount === null) {
        return failure("chunk-out-of-range", "Scalar-field chunk index is out of range.");
      }

      const [rows, columns] = decoded.data.grid.logicalShape;
      const rowExtrema = axisExtrema(rows, decoded.data.field.location);
      const columnExtrema = axisExtrema(columns, decoded.data.field.location);
      const rawMinimum = rowExtrema.minimum * columnExtrema.minimum;
      const rawMaximum = rowExtrema.maximum * columnExtrema.maximum;
      const offset = chunkIndex * decoded.data.transport.valuesPerChunk;
      const values = Array.from({ length: expectedValueCount }, (_, localIndex) =>
        previewValue(decoded.data, offset + localIndex, rawMinimum, rawMaximum),
      );
      await Promise.resolve();
      return decodeScalarFieldChunk(encodeScalarFieldChunk(values), expectedValueCount);
    },
  };
}

export const previewScalarFieldDataBridge = createPreviewScalarFieldDataBridge();

export const scalarFieldDataBridge: ScalarFieldDataBridge =
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window
    ? nativeScalarFieldDataBridge
    : previewScalarFieldDataBridge;
