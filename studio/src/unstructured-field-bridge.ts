import { invoke } from "@tauri-apps/api/core";
import {
  type UnstructuredFieldContext,
  type UnstructuredFieldDescriptor,
  type UnstructuredFieldStream,
  unstructuredDescriptorMatchesContext,
  unstructuredFieldChunkItemCount,
  unstructuredFieldChunkRequest,
  unstructuredFieldChunkRequestSchema,
  unstructuredFieldContextSchema,
  unstructuredFieldDescriptorSchema,
  unstructuredFieldFailureEnvelopeSchema,
  unstructuredFieldOpenEnvelopeSchema,
  unstructuredFieldOpenRequest,
  unstructuredFieldOpenRequestSchema,
} from "./unstructured-field-protocol";

export type UnstructuredFieldBridgeFailureCode =
  | "invalid-context"
  | "invalid-request"
  | "invalid-descriptor"
  | "descriptor-mismatch"
  | "chunk-out-of-range"
  | "invalid-chunk"
  | "nonfinite-chunk"
  | "bridge-rejected";

export type UnstructuredFieldBridgeFailure = Readonly<{
  code: UnstructuredFieldBridgeFailureCode;
  message: string;
}>;

export type UnstructuredFieldBridgeResult<T> =
  | Readonly<{ ok: true; value: T }>
  | Readonly<{ ok: false; failure: UnstructuredFieldBridgeFailure }>;

export type UnstructuredFieldChunk =
  | Readonly<{ stream: "coordinates"; values: Float64Array }>
  | Readonly<{ stream: "triangles"; values: Uint32Array }>
  | Readonly<{ stream: "values"; values: Float64Array }>;

export interface UnstructuredFieldDataBridge {
  open(
    context: UnstructuredFieldContext,
  ): Promise<UnstructuredFieldBridgeResult<UnstructuredFieldDescriptor>>;
  readChunk(
    descriptor: UnstructuredFieldDescriptor,
    stream: UnstructuredFieldStream,
    chunkIndex: number,
  ): Promise<UnstructuredFieldBridgeResult<UnstructuredFieldChunk>>;
}

function failure(
  code: UnstructuredFieldBridgeFailureCode,
  message: string,
): UnstructuredFieldBridgeResult<never> {
  return { ok: false, failure: { code, message } };
}

function rejectedMessage(error: unknown, fallback: string): string {
  const envelope = unstructuredFieldFailureEnvelopeSchema.safeParse(error);
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

export function decodeUnstructuredF64Chunk(
  payload: unknown,
  expectedItemCount: number,
  components: 1 | 2,
): UnstructuredFieldBridgeResult<Float64Array> {
  const scalarCount = expectedScalarCount(expectedItemCount, components);
  if (scalarCount === null) {
    return failure("invalid-chunk", "Unstructured f64 chunk has an invalid expected shape.");
  }
  const bytes = binaryBytes(payload);
  const expectedByteLength = scalarCount * Float64Array.BYTES_PER_ELEMENT;
  if (bytes === null || bytes.byteLength !== expectedByteLength) {
    return failure(
      "invalid-chunk",
      `Unstructured f64 chunk must contain exactly ${expectedByteLength} bytes.`,
    );
  }
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const values = new Float64Array(scalarCount);
  for (let index = 0; index < scalarCount; index += 1) {
    const value = view.getFloat64(index * Float64Array.BYTES_PER_ELEMENT, true);
    if (!Number.isFinite(value)) {
      return failure("nonfinite-chunk", "Unstructured f64 chunk contains a non-finite value.");
    }
    values[index] = value;
  }
  return { ok: true, value: values };
}

export function decodeUnstructuredU32Chunk(
  payload: unknown,
  expectedItemCount: number,
  components: 3,
): UnstructuredFieldBridgeResult<Uint32Array> {
  const scalarCount = expectedScalarCount(expectedItemCount, components);
  if (scalarCount === null) {
    return failure("invalid-chunk", "Unstructured u32 chunk has an invalid expected shape.");
  }
  const bytes = binaryBytes(payload);
  const expectedByteLength = scalarCount * Uint32Array.BYTES_PER_ELEMENT;
  if (bytes === null || bytes.byteLength !== expectedByteLength) {
    return failure(
      "invalid-chunk",
      `Unstructured u32 chunk must contain exactly ${expectedByteLength} bytes.`,
    );
  }
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const values = new Uint32Array(scalarCount);
  for (let index = 0; index < scalarCount; index += 1) {
    values[index] = view.getUint32(index * Uint32Array.BYTES_PER_ELEMENT, true);
  }
  return { ok: true, value: values };
}

export function encodeUnstructuredF64Chunk(values: ArrayLike<number>): ArrayBuffer {
  const buffer = new ArrayBuffer(values.length * Float64Array.BYTES_PER_ELEMENT);
  const view = new DataView(buffer);
  for (let index = 0; index < values.length; index += 1) {
    view.setFloat64(index * Float64Array.BYTES_PER_ELEMENT, values[index] ?? Number.NaN, true);
  }
  return buffer;
}

export function encodeUnstructuredU32Chunk(values: ArrayLike<number>): ArrayBuffer {
  const buffer = new ArrayBuffer(values.length * Uint32Array.BYTES_PER_ELEMENT);
  const view = new DataView(buffer);
  for (let index = 0; index < values.length; index += 1) {
    view.setUint32(index * Uint32Array.BYTES_PER_ELEMENT, values[index] ?? 0, true);
  }
  return buffer;
}

async function invokeNative(command: string, request: unknown): Promise<unknown> {
  return invoke(command, { request });
}

export const nativeUnstructuredFieldDataBridge: UnstructuredFieldDataBridge = {
  async open(context) {
    const accepted = unstructuredFieldContextSchema.safeParse(context);
    if (!accepted.success) {
      return failure(
        "invalid-context",
        "Unstructured field view requires one valid accepted P1 context.",
      );
    }
    const request = unstructuredFieldOpenRequest(accepted.data);
    if (!unstructuredFieldOpenRequestSchema.safeParse(request).success) {
      return failure("invalid-request", "Unstructured field open request is not canonical.");
    }
    try {
      const response = await invokeNative("open_unstructured_field_view", request);
      const envelope = unstructuredFieldOpenEnvelopeSchema.safeParse(response);
      if (!envelope.success) {
        return failure(
          "invalid-descriptor",
          "Native bridge returned an invalid unstructured field envelope.",
        );
      }
      if (envelope.data.result === null) {
        return failure(
          "bridge-rejected",
          envelope.data.diagnostics.find((diagnostic) => diagnostic.severity === "error")
            ?.message ?? "Native unstructured field open was rejected.",
        );
      }
      if (!unstructuredDescriptorMatchesContext(accepted.data, envelope.data.result)) {
        return failure(
          "descriptor-mismatch",
          "Native unstructured descriptor differs from the exact accepted context.",
        );
      }
      return { ok: true, value: envelope.data.result };
    } catch (error: unknown) {
      const detail = rejectedMessage(error, "Native unstructured field open was rejected.");
      return failure("bridge-rejected", `Native unstructured field open failed: ${detail}`);
    }
  },

  async readChunk(descriptor, stream, chunkIndex) {
    const checked = unstructuredFieldDescriptorSchema.safeParse(descriptor);
    if (!checked.success) {
      return failure("invalid-descriptor", "Unstructured field descriptor is not canonical.");
    }
    const expectedItems = unstructuredFieldChunkItemCount(checked.data, stream, chunkIndex);
    if (expectedItems === null) {
      return failure("chunk-out-of-range", "Unstructured field chunk index is out of range.");
    }
    const request = unstructuredFieldChunkRequest(checked.data, stream, chunkIndex);
    if (!unstructuredFieldChunkRequestSchema.safeParse(request).success) {
      return failure("invalid-request", "Unstructured field chunk request is not canonical.");
    }
    try {
      const response = await invokeNative("read_unstructured_field_chunk", request);
      switch (stream) {
        case "coordinates": {
          const decoded = decodeUnstructuredF64Chunk(response, expectedItems, 2);
          return decoded.ok ? { ok: true, value: { stream, values: decoded.value } } : decoded;
        }
        case "triangles": {
          const decoded = decodeUnstructuredU32Chunk(response, expectedItems, 3);
          return decoded.ok ? { ok: true, value: { stream, values: decoded.value } } : decoded;
        }
        case "values": {
          const decoded = decodeUnstructuredF64Chunk(response, expectedItems, 1);
          return decoded.ok ? { ok: true, value: { stream, values: decoded.value } } : decoded;
        }
      }
    } catch (error: unknown) {
      const detail = rejectedMessage(error, "Native unstructured field chunk read was rejected.");
      return failure("bridge-rejected", `Native unstructured field chunk read failed: ${detail}`);
    }
  },
};

function expectedScalarCount(itemCount: number, components: number): number | null {
  if (
    !Number.isSafeInteger(itemCount) ||
    itemCount <= 0 ||
    !Number.isSafeInteger(components) ||
    components <= 0
  ) {
    return null;
  }
  const count = itemCount * components;
  return Number.isSafeInteger(count) ? count : null;
}
