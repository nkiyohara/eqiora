import { describe, expect, it } from "vitest";
import {
  SCALAR_FIELD_VALUES_PER_CHUNK,
  SCALAR_FIELD_VIEW_PROTOCOL,
  scalarFieldDescriptorSchema,
} from "./scalar-field-protocol";
import {
  scalarFieldCoordinates,
  scalarFieldIndices,
  scalarFieldOrdinal,
  scalarFieldOrdinalAtPoint,
} from "./scalar-field-workspace";

function descriptor(location: "vertex" | "cell-center") {
  return scalarFieldDescriptorSchema.parse({
    protocol: SCALAR_FIELD_VIEW_PROTOCOL,
    modelDigest: "sha256:0123456789abcdef",
    runId: "00000000-0000-4000-8000-000000000001",
    planKey: "a".repeat(64),
    field: {
      id: "Field:temperature",
      name: "temperature",
      dimension: "Θ",
      coherentSiUnit: "K",
      scalarType: "f64",
      location,
      valueCount: 6,
      minimum: 0,
      maximum: 5,
    },
    domain: {
      id: "Domain:plate",
      boundsM: [
        [10, 20],
        [100, 160],
      ],
    },
    grid: {
      kind: "uniform-cartesian-2d",
      logicalShape: [2, 3],
      order: "row-major-last-axis-fastest",
    },
    transport: {
      kind: "explicit-owned-host-copy",
      encoding: "f64-le",
      valuesPerChunk: SCALAR_FIELD_VALUES_PER_CHUNK,
      chunkCount: 1,
    },
  });
}

describe("scalar-field workspace canonical selection", () => {
  it("round-trips exact last-axis-fastest ordinals", () => {
    const shape = [2, 3] as const;
    expect(scalarFieldOrdinal(shape, 0, 0)).toBe(0);
    expect(scalarFieldOrdinal(shape, 0, 2)).toBe(2);
    expect(scalarFieldOrdinal(shape, 1, 0)).toBe(3);
    expect(scalarFieldOrdinal(shape, 1, 2)).toBe(5);
    expect(scalarFieldIndices(shape, 3)).toEqual({ i: 1, j: 0 });
    expect(scalarFieldIndices(shape, 6)).toBeNull();
  });

  it("maps cell-centred pointer selection to the same exact ordinal and coordinate", () => {
    const field = descriptor("cell-center");
    const ordinal = scalarFieldOrdinalAtPoint(field, 0.99, 0.01);
    expect(ordinal).toBe(3);
    expect(scalarFieldCoordinates(field, ordinal)).toEqual({ xM: 17.5, yM: 110 });
  });

  it("maps vertex selection without interpolation", () => {
    const field = descriptor("vertex");
    const ordinal = scalarFieldOrdinalAtPoint(field, 1, 0.5);
    expect(ordinal).toBe(4);
    expect(scalarFieldCoordinates(field, ordinal)).toEqual({ xM: 20, yM: 130 });
  });
});
