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

const CHUNK_MAGIC = [0x45, 0x51, 0x50, 0x31] as const;
const CHUNK_HEADER_BYTES = 16;

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
  stream: "coordinates" | "values",
  chunkIndex: number,
  expectedItemCount: number,
  components: 1 | 2,
): UnstructuredFieldBridgeResult<Float64Array> {
  const scalarCount = expectedScalarCount(expectedItemCount, components);
  if (scalarCount === null) {
    return failure("invalid-chunk", "Unstructured f64 chunk has an invalid expected shape.");
  }
  const payloadBytes = checkedChunkPayload(
    payload,
    stream,
    chunkIndex,
    expectedItemCount,
    scalarCount * Float64Array.BYTES_PER_ELEMENT,
  );
  if (!payloadBytes.ok) {
    return failure(
      "invalid-chunk",
      `Unstructured f64 chunk is not the exact requested ${stream} chunk.`,
    );
  }
  const bytes = payloadBytes.value;
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
  chunkIndex: number,
  expectedItemCount: number,
  components: 3,
): UnstructuredFieldBridgeResult<Uint32Array> {
  const scalarCount = expectedScalarCount(expectedItemCount, components);
  if (scalarCount === null) {
    return failure("invalid-chunk", "Unstructured u32 chunk has an invalid expected shape.");
  }
  const payloadBytes = checkedChunkPayload(
    payload,
    "triangles",
    chunkIndex,
    expectedItemCount,
    scalarCount * Uint32Array.BYTES_PER_ELEMENT,
  );
  if (!payloadBytes.ok) {
    return failure(
      "invalid-chunk",
      "Unstructured u32 chunk is not the exact requested triangle chunk.",
    );
  }
  const bytes = payloadBytes.value;
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const values = new Uint32Array(scalarCount);
  for (let index = 0; index < scalarCount; index += 1) {
    values[index] = view.getUint32(index * Uint32Array.BYTES_PER_ELEMENT, true);
  }
  return { ok: true, value: values };
}

export function encodeUnstructuredF64Chunk(
  values: ArrayLike<number>,
  stream: "coordinates" | "values",
  chunkIndex = 0,
): ArrayBuffer {
  const components = stream === "coordinates" ? 2 : 1;
  const itemCount = values.length / components;
  if (!Number.isInteger(itemCount)) throw new Error("Chunk values do not form complete items.");
  const buffer = new ArrayBuffer(
    CHUNK_HEADER_BYTES + values.length * Float64Array.BYTES_PER_ELEMENT,
  );
  const view = new DataView(buffer);
  writeChunkHeader(view, stream, chunkIndex, itemCount);
  for (let index = 0; index < values.length; index += 1) {
    view.setFloat64(
      CHUNK_HEADER_BYTES + index * Float64Array.BYTES_PER_ELEMENT,
      values[index] ?? Number.NaN,
      true,
    );
  }
  return buffer;
}

export function encodeUnstructuredU32Chunk(values: ArrayLike<number>, chunkIndex = 0): ArrayBuffer {
  const itemCount = values.length / 3;
  if (!Number.isInteger(itemCount)) throw new Error("Chunk values do not form complete triangles.");
  const buffer = new ArrayBuffer(
    CHUNK_HEADER_BYTES + values.length * Uint32Array.BYTES_PER_ELEMENT,
  );
  const view = new DataView(buffer);
  writeChunkHeader(view, "triangles", chunkIndex, itemCount);
  for (let index = 0; index < values.length; index += 1) {
    view.setUint32(
      CHUNK_HEADER_BYTES + index * Uint32Array.BYTES_PER_ELEMENT,
      values[index] ?? 0,
      true,
    );
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
          const decoded = decodeUnstructuredF64Chunk(
            response,
            stream,
            chunkIndex,
            expectedItems,
            2,
          );
          return decoded.ok ? { ok: true, value: { stream, values: decoded.value } } : decoded;
        }
        case "triangles": {
          const decoded = decodeUnstructuredU32Chunk(response, chunkIndex, expectedItems, 3);
          return decoded.ok ? { ok: true, value: { stream, values: decoded.value } } : decoded;
        }
        case "values": {
          const decoded = decodeUnstructuredF64Chunk(
            response,
            stream,
            chunkIndex,
            expectedItems,
            1,
          );
          return decoded.ok ? { ok: true, value: { stream, values: decoded.value } } : decoded;
        }
      }
    } catch (error: unknown) {
      const detail = rejectedMessage(error, "Native unstructured field chunk read was rejected.");
      return failure("bridge-rejected", `Native unstructured field chunk read failed: ${detail}`);
    }
  },
};

function checkedChunkPayload(
  payload: unknown,
  stream: UnstructuredFieldStream,
  chunkIndex: number,
  itemCount: number,
  payloadByteLength: number,
): UnstructuredFieldBridgeResult<Uint8Array> {
  const bytes = binaryBytes(payload);
  if (
    bytes === null ||
    bytes.byteLength !== CHUNK_HEADER_BYTES + payloadByteLength ||
    !Number.isSafeInteger(chunkIndex) ||
    chunkIndex < 0 ||
    !Number.isSafeInteger(itemCount) ||
    itemCount <= 0
  ) {
    return failure("invalid-chunk", "Unstructured chunk byte shape or identity is invalid.");
  }
  const header = new DataView(bytes.buffer, bytes.byteOffset, CHUNK_HEADER_BYTES);
  if (
    CHUNK_MAGIC.some((byte, index) => header.getUint8(index) !== byte) ||
    header.getUint8(4) !== streamCode(stream) ||
    header.getUint8(5) !== 0 ||
    header.getUint8(6) !== 0 ||
    header.getUint8(7) !== 0 ||
    header.getUint32(8, true) !== chunkIndex ||
    header.getUint32(12, true) !== itemCount
  ) {
    return failure("invalid-chunk", "Unstructured chunk header differs from its exact request.");
  }
  return {
    ok: true,
    value: new Uint8Array(bytes.buffer, bytes.byteOffset + CHUNK_HEADER_BYTES, payloadByteLength),
  };
}

function writeChunkHeader(
  view: DataView,
  stream: UnstructuredFieldStream,
  chunkIndex: number,
  itemCount: number,
): void {
  for (const [index, byte] of CHUNK_MAGIC.entries()) view.setUint8(index, byte);
  view.setUint8(4, streamCode(stream));
  view.setUint8(5, 0);
  view.setUint8(6, 0);
  view.setUint8(7, 0);
  view.setUint32(8, chunkIndex, true);
  view.setUint32(12, itemCount, true);
}

function streamCode(stream: UnstructuredFieldStream): number {
  switch (stream) {
    case "coordinates":
      return 0;
    case "triangles":
      return 1;
    case "values":
      return 2;
  }
}

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
