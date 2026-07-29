import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import {
  decodeUnstructuredF64Chunk,
  decodeUnstructuredU32Chunk,
  encodeUnstructuredF64Chunk,
  encodeUnstructuredU32Chunk,
  type UnstructuredFieldChunk,
  type UnstructuredFieldDataBridge,
} from "./unstructured-field-bridge";
import {
  UNSTRUCTURED_FIELD_ITEMS_PER_CHUNK,
  UNSTRUCTURED_FIELD_VIEW_PROTOCOL,
  type UnstructuredFieldContext,
  type UnstructuredFieldDescriptor,
  type UnstructuredFieldStream,
  unstructuredDescriptorMatchesContext,
  unstructuredFieldContextSchema,
  unstructuredFieldDescriptorSchema,
} from "./unstructured-field-protocol";
import { drawUnstructuredP1Field, interpolateP1Triangle } from "./unstructured-field-renderer";
import { UnstructuredFieldDataSession } from "./unstructured-field-session";
import {
  nearestUnstructuredVertex,
  UnstructuredFieldWorkspace,
  unstructuredVertexCoordinates,
} from "./unstructured-field-workspace";

const coordinates = new Float64Array([0, 0, 1, 0, 1, 1, 0, 1]);
const triangles = new Uint32Array([0, 1, 2, 0, 2, 3]);
const values = new Float64Array([0, 1, 3, 2]);

function context(): UnstructuredFieldContext {
  return unstructuredFieldContextSchema.parse({
    modelDigest: "0".repeat(64),
    semanticRevision: "0",
    realizationDigest: "1".repeat(64),
    runDigest: "2".repeat(64),
    snapshotDigest: "3".repeat(64),
    meshDigest: "4".repeat(64),
    field: {
      id: "Field:01HZX3W0A1B2C3D4E5F6G7H8J9",
      dimension: "M·L^-1·T^-2",
      coherentSiUnit: "kg·m^-1·s^-2",
      valueCount: 4,
      minimum: 0,
      maximum: 3,
    },
    domain: {
      id: "Domain:01HZX3W0A1B2C3D4E5F6G7H8JA",
      boundsM: [
        [0, 1],
        [0, 1],
      ],
    },
    mesh: {
      kind: "affine-triangle-2d",
      vertexCount: 4,
      triangleCount: 2,
    },
  });
}

function descriptor(): UnstructuredFieldDescriptor {
  const accepted = context();
  return unstructuredFieldDescriptorSchema.parse({
    protocol: UNSTRUCTURED_FIELD_VIEW_PROTOCOL,
    modelDigest: accepted.modelDigest,
    semanticRevision: accepted.semanticRevision,
    realizationDigest: accepted.realizationDigest,
    runDigest: accepted.runDigest,
    snapshotDigest: accepted.snapshotDigest,
    meshDigest: accepted.meshDigest,
    field: {
      ...accepted.field,
      scalarType: "f64",
      location: "vertex",
    },
    domain: accepted.domain,
    mesh: accepted.mesh,
    transport: {
      kind: "explicit-owned-host-copy",
      coordinates: {
        encoding: "f64-le",
        components: 2,
        itemCount: 4,
        itemsPerChunk: UNSTRUCTURED_FIELD_ITEMS_PER_CHUNK,
        chunkCount: 1,
      },
      triangles: {
        encoding: "u32-le",
        components: 3,
        itemCount: 2,
        itemsPerChunk: UNSTRUCTURED_FIELD_ITEMS_PER_CHUNK,
        chunkCount: 1,
      },
      values: {
        encoding: "f64-le",
        components: 1,
        itemCount: 4,
        itemsPerChunk: UNSTRUCTURED_FIELD_ITEMS_PER_CHUNK,
        chunkCount: 1,
      },
    },
  });
}

function bridge(
  opened: UnstructuredFieldDescriptor = descriptor(),
  overrides: Partial<Record<UnstructuredFieldStream, Float64Array | Uint32Array>> = {},
): UnstructuredFieldDataBridge {
  return {
    async open() {
      return { ok: true, value: opened };
    },
    async readChunk(_descriptor, stream) {
      const streamValues =
        overrides[stream] ??
        (
          { coordinates, triangles, values } satisfies Record<
            UnstructuredFieldStream,
            Float64Array | Uint32Array
          >
        )[stream];
      return {
        ok: true,
        value: { stream, values: streamValues } as UnstructuredFieldChunk,
      };
    },
  };
}

describe("unstructured P1 scalar protocol", () => {
  it("binds one exact descriptor to its accepted lineage and counts", () => {
    expect(unstructuredDescriptorMatchesContext(context(), descriptor())).toBe(true);
    expect(
      unstructuredDescriptorMatchesContext(context(), {
        ...descriptor(),
        runDigest: "5".repeat(64),
      }),
    ).toBe(false);
    expect(
      unstructuredFieldDescriptorSchema.safeParse({
        ...descriptor(),
        transport: {
          ...descriptor().transport,
          triangles: {
            ...descriptor().transport.triangles,
            itemCount: 1,
          },
        },
      }).success,
    ).toBe(false);
    expect(
      unstructuredFieldContextSchema.safeParse({
        ...context(),
        field: { ...context().field, valueCount: 3 },
      }).success,
    ).toBe(false);
    expect(
      unstructuredFieldContextSchema.safeParse({
        ...context(),
        semanticRevision: "18446744073709551615",
      }).success,
    ).toBe(true);
    expect(
      unstructuredFieldContextSchema.safeParse({
        ...context(),
        semanticRevision: "18446744073709551616",
      }).success,
    ).toBe(false);
  });

  it("decodes exact little-endian stream shapes and rejects malformed f64 data", () => {
    expect(
      decodeUnstructuredF64Chunk(
        encodeUnstructuredF64Chunk(coordinates, "coordinates"),
        "coordinates",
        0,
        4,
        2,
      ),
    ).toEqual({
      ok: true,
      value: coordinates,
    });
    expect(decodeUnstructuredU32Chunk(encodeUnstructuredU32Chunk(triangles), 0, 2, 3)).toEqual({
      ok: true,
      value: triangles,
    });
    expect(decodeUnstructuredF64Chunk(new ArrayBuffer(7), "values", 0, 1, 1)).toMatchObject({
      ok: false,
      failure: { code: "invalid-chunk" },
    });
    expect(
      decodeUnstructuredF64Chunk(
        encodeUnstructuredF64Chunk([Number.POSITIVE_INFINITY], "values"),
        "values",
        0,
        1,
        1,
      ),
    ).toMatchObject({
      ok: false,
      failure: { code: "nonfinite-chunk" },
    });
    expect(
      decodeUnstructuredF64Chunk(
        encodeUnstructuredF64Chunk(values, "values", 0),
        "values",
        1,
        4,
        1,
      ),
    ).toMatchObject({
      ok: false,
      failure: { code: "invalid-chunk" },
    });
    expect(
      decodeUnstructuredF64Chunk(
        encodeUnstructuredF64Chunk(coordinates, "coordinates"),
        "values",
        0,
        8,
        1,
      ),
    ).toMatchObject({
      ok: false,
      failure: { code: "invalid-chunk" },
    });
  });
});

describe("unstructured P1 scalar session", () => {
  it("publishes only after coordinates, connectivity and values complete in order", async () => {
    const calls: string[] = [];
    const source = bridge();
    const recording: UnstructuredFieldDataBridge = {
      open: source.open,
      async readChunk(descriptor, stream, chunkIndex) {
        calls.push(`${stream}:${chunkIndex}`);
        return source.readChunk(descriptor, stream, chunkIndex);
      },
    };
    const state = await new UnstructuredFieldDataSession(recording).load(context());
    expect(calls).toEqual(["coordinates:0", "triangles:0", "values:0"]);
    expect(state).toMatchObject({ kind: "ready" });
    if (state.kind !== "ready") throw new Error("expected ready state");
    expect(state.coordinates).toEqual(coordinates);
    expect(state.triangles).toEqual(triangles);
    expect(state.values).toEqual(values);
  });

  it("fails closed on foreign descriptors, connectivity and final range drift", async () => {
    const foreign = { ...descriptor(), snapshotDigest: "5".repeat(64) };
    expect(await new UnstructuredFieldDataSession(bridge(foreign)).load(context())).toMatchObject({
      kind: "failed",
      failure: { code: "descriptor-mismatch" },
    });
    expect(
      await new UnstructuredFieldDataSession(
        bridge(descriptor(), { triangles: new Uint32Array([0, 1, 9, 0, 2, 3]) }),
      ).load(context()),
    ).toMatchObject({
      kind: "failed",
      failure: { code: "connectivity-invalid" },
    });
    expect(
      await new UnstructuredFieldDataSession(
        bridge(descriptor(), { triangles: new Uint32Array([0, 2, 1, 0, 2, 3]) }),
      ).load(context()),
    ).toMatchObject({
      kind: "failed",
      failure: { code: "connectivity-invalid" },
    });
    expect(
      await new UnstructuredFieldDataSession(
        bridge(descriptor(), { values: new Float64Array([0, 1, 2, 4]) }),
      ).load(context()),
    ).toMatchObject({
      kind: "failed",
      failure: { code: "field-range-mismatch" },
    });
  });

  it("rejects missing, reordered, short, non-finite and bounds-drifted streams", async () => {
    const missing: UnstructuredFieldDataBridge = {
      open: bridge().open,
      async readChunk() {
        return {
          ok: false,
          failure: { code: "bridge-rejected", message: "missing chunk" },
        };
      },
    };
    expect(await new UnstructuredFieldDataSession(missing).load(context())).toMatchObject({
      kind: "failed",
      failure: { code: "chunk-rejected" },
    });

    const reordered: UnstructuredFieldDataBridge = {
      open: bridge().open,
      async readChunk() {
        return {
          ok: true,
          value: { stream: "values", values },
        };
      },
    };
    expect(await new UnstructuredFieldDataSession(reordered).load(context())).toMatchObject({
      kind: "failed",
      failure: { code: "chunk-order-mismatch" },
    });

    expect(
      await new UnstructuredFieldDataSession(
        bridge(descriptor(), { coordinates: coordinates.slice(0, 6) }),
      ).load(context()),
    ).toMatchObject({
      kind: "failed",
      failure: { code: "chunk-size-mismatch" },
    });
    expect(
      await new UnstructuredFieldDataSession(
        bridge(descriptor(), {
          coordinates: new Float64Array([0, 0, 1, 0, 1, Number.NaN, 0, 1]),
        }),
      ).load(context()),
    ).toMatchObject({
      kind: "failed",
      failure: { code: "chunk-nonfinite" },
    });
    expect(
      await new UnstructuredFieldDataSession(
        bridge(descriptor(), {
          coordinates: new Float64Array([0, 0, 1, 0, 1, 2, 0, 1]),
        }),
      ).load(context()),
    ).toMatchObject({
      kind: "failed",
      failure: { code: "coordinate-bounds-mismatch" },
    });
  });

  it("drops stale asynchronous publication without mutating the new context", async () => {
    let resolveOpen:
      | ((value: Awaited<ReturnType<UnstructuredFieldDataBridge["open"]>>) => void)
      | undefined;
    const delayed: UnstructuredFieldDataBridge = {
      open: () =>
        new Promise((resolve) => {
          resolveOpen = resolve;
        }),
      readChunk: bridge().readChunk,
    };
    const session = new UnstructuredFieldDataSession(delayed);
    const pending = session.load(context());
    const next = {
      ...context(),
      runDigest: "5".repeat(64),
    };
    session.setContext(next);
    resolveOpen?.({ ok: true, value: descriptor() });
    await pending;
    expect(session.state).toEqual({ kind: "idle", context: next });
  });
});

describe("unstructured P1 scalar workspace", () => {
  it("interpolates the admitted vertex coefficients as one P1 triangle", () => {
    const points = [
      { x: 0, y: 0 },
      { x: 3, y: 0 },
      { x: 0, y: 3 },
    ] as const;
    const coefficients = [1, 4, 7] as const;
    expect(interpolateP1Triangle(points[0], points, coefficients)).toBe(1);
    expect(interpolateP1Triangle({ x: 1, y: 1 }, points, coefficients)).toBe(4);
    expect(interpolateP1Triangle({ x: 3, y: 3 }, points, coefficients)).toBeNull();
  });

  it("rejects excessive triangle-pixel work before allocating the raster", () => {
    vi.stubGlobal("window", { devicePixelRatio: 4 });
    const createImageData = vi.fn();
    const canvas = {
      clientHeight: 4_000,
      clientWidth: 4_000,
      getContext: () => ({ clearRect: vi.fn(), createImageData }),
      height: 0,
      width: 0,
    } as unknown as HTMLCanvasElement;
    const overlappingTriangles = new Uint32Array(8 * 3);
    for (let triangle = 0; triangle < 8; triangle += 1) {
      overlappingTriangles.set([0, 1, 2], triangle * 3);
    }
    const boundedDescriptor = {
      ...descriptor(),
      mesh: { ...descriptor().mesh, triangleCount: 8 },
    };

    try {
      expect(() =>
        drawUnstructuredP1Field(
          canvas,
          boundedDescriptor,
          coordinates,
          overlappingTriangles,
          values,
        ),
      ).toThrow("Triangle projection exceeds the bounded presentation work budget.");
      expect(canvas.width * canvas.height).toBeLessThanOrEqual(4_194_304);
      expect(createImageData).not.toHaveBeenCalled();
    } finally {
      vi.unstubAllGlobals();
    }
  });

  it("selects exact vertices and retains a keyboard/screen-reader table alternative", () => {
    expect(unstructuredVertexCoordinates(coordinates, 2)).toEqual({ xM: 1, yM: 1 });
    expect(nearestUnstructuredVertex(descriptor(), coordinates, 0.9, 0.9)).toBe(2);
    const html = renderToStaticMarkup(
      createElement(UnstructuredFieldWorkspace, {
        coordinates,
        descriptor: descriptor(),
        onSelect: () => {},
        selectedVertex: 2,
        stale: false,
        triangles,
        values,
      }),
    );
    expect(html).toContain("<table");
    expect(html).toContain("Exact P1 values in canonical mesh-vertex order");
    expect(html).toContain('aria-live="polite"');
    expect(html).toContain("Vertex 2");
    expect(html).toContain(context().field.id);
    expect(html).toContain(context().domain.id);
  });
});
