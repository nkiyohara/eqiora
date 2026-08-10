import { describe, expect, it } from "vitest";
import { decodeMeshPayload } from "./mesh-contract";

const PROFILE = "circular-hole-chordal-reference-50/v1";
const MESH_DIGEST = "148e2fb4f3d5c801eaa4e3a376f0b8ec547abdcfebc1108cf0577e5c952a946a";
const VERTEX_COUNT = 104;
const TRIANGLE_COUNT = 104;
const COORDINATE_BYTES = 1_664;
const TRIANGLE_BYTES = 1_248;

// Exact little-endian projections replayed from the already accepted native
// interfaces.python-circular-hole-chordal-mesh producer at the frozen base.
// These are transport fixtures, not newly derived scientific expectations.
const COORDINATES_F64_LE =
  "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";
const TRIANGLES_U32_LE =
  "MgAAADMAAAABAAAAMgAAAAEAAAAAAAAAMgAAAGcAAAAzAAAAMwAAADQAAAACAAAAMwAAAAIAAAABAAAANAAAADUAAAADAAAANAAAAAMAAAACAAAANQAAADYAAAAEAAAANQAAAAQAAAADAAAANgAAADcAAAAFAAAANgAAAAUAAAAEAAAANwAAADgAAAAGAAAANwAAAAYAAAAFAAAAOAAAADkAAAAHAAAAOAAAAAcAAAAGAAAAOQAAADoAAAAIAAAAOQAAAAgAAAAHAAAAOgAAADsAAAAJAAAAOgAAAAkAAAAIAAAAOwAAADwAAAAKAAAAOwAAAAoAAAAJAAAAPAAAAD0AAAALAAAAPAAAAAsAAAAKAAAAPQAAAD4AAAAMAAAAPQAAAAwAAAALAAAAPgAAAD8AAAANAAAAPgAAAA0AAAAMAAAAPwAAAEAAAAAOAAAAPwAAAA4AAAANAAAAQAAAAEEAAAAPAAAAQAAAAA8AAAAOAAAAQQAAAEIAAAAQAAAAQQAAABAAAAAPAAAAQgAAAEMAAAARAAAAQgAAABEAAAAQAAAAQwAAAEQAAAASAAAAQwAAABIAAAARAAAARAAAAEUAAAATAAAARAAAABMAAAASAAAARAAAAGUAAABFAAAARQAAAEYAAAAUAAAARQAAABQAAAATAAAARgAAAEcAAAAVAAAARgAAABUAAAAUAAAARwAAAEgAAAAWAAAARwAAABYAAAAVAAAASAAAAEkAAAAXAAAASAAAABcAAAAWAAAASQAAAEoAAAAYAAAASQAAABgAAAAXAAAASgAAAEsAAAAZAAAASgAAABkAAAAYAAAASwAAAEwAAAAaAAAASwAAABoAAAAZAAAATAAAAE0AAAAbAAAATAAAABsAAAAaAAAATQAAAE4AAAAcAAAATQAAABwAAAAbAAAATgAAAE8AAAAdAAAATgAAAB0AAAAcAAAATwAAAFAAAAAeAAAATwAAAB4AAAAdAAAAUAAAAFEAAAAfAAAAUAAAAB8AAAAeAAAAUQAAAFIAAAAgAAAAUQAAACAAAAAfAAAAUQAAAGQAAABSAAAAUgAAAFMAAAAhAAAAUgAAACEAAAAgAAAAUwAAAFQAAAAiAAAAUwAAACIAAAAhAAAAVAAAAFUAAAAjAAAAVAAAACMAAAAiAAAAVQAAAFYAAAAkAAAAVQAAACQAAAAjAAAAVgAAAFcAAAAlAAAAVgAAACUAAAAkAAAAVwAAAFgAAAAmAAAAVwAAACYAAAAlAAAAWAAAAFkAAAAnAAAAWAAAACcAAAAmAAAAWQAAAFoAAAAoAAAAWQAAACgAAAAnAAAAWgAAAFsAAAApAAAAWgAAACkAAAAoAAAAWwAAAFwAAAAqAAAAWwAAACoAAAApAAAAXAAAAF0AAAArAAAAXAAAACsAAAAqAAAAXQAAAF4AAAAsAAAAXQAAACwAAAArAAAAXgAAAF8AAAAtAAAAXgAAAC0AAAAsAAAAXwAAAGAAAAAuAAAAXwAAAC4AAAAtAAAAYAAAAGEAAAAvAAAAYAAAAC8AAAAuAAAAYQAAAGIAAAAwAAAAYQAAADAAAAAvAAAAYgAAAGMAAAAxAAAAYgAAADEAAAAwAAAAYwAAADIAAAAAAAAAYwAAAAAAAAAxAAAAYwAAAGYAAAAyAAAA";

type Payload = {
  profile: string;
  mesh_digest: string;
  vertex_count: number;
  triangle_count: number;
  coordinates_f64_le: DataView;
  triangles_u32_le: DataView;
};

function bytes(encoded: string): Uint8Array {
  const decoded = atob(encoded);
  return Uint8Array.from(decoded, (character) => character.charCodeAt(0));
}

function view(values: Uint8Array): DataView {
  const copy = values.slice();
  return new DataView(copy.buffer, copy.byteOffset, copy.byteLength);
}

function fixture(): Payload {
  return {
    profile: PROFILE,
    mesh_digest: MESH_DIGEST,
    vertex_count: VERTEX_COUNT,
    triangle_count: TRIANGLE_COUNT,
    coordinates_f64_le: view(bytes(COORDINATES_F64_LE)),
    triangles_u32_le: view(bytes(TRIANGLES_U32_LE)),
  };
}

function mutatedBytes(input: DataView, mutate: (copy: DataView) => void): DataView {
  const copy = new Uint8Array(input.buffer, input.byteOffset, input.byteLength).slice();
  const result = new DataView(copy.buffer);
  mutate(result);
  return result;
}

function expectRejected(payload: unknown): void {
  expect(() => decodeMeshPayload(payload)).toThrow();
}

describe("the exact N1 Mesh transport contract", () => {
  it("accepts only the existing literal identity and exact accepted buffers", () => {
    const payload = fixture();
    const decoded = decodeMeshPayload(payload);
    expect(decoded.coordinates).toBeInstanceOf(Float64Array);
    expect(decoded.triangles).toBeInstanceOf(Uint32Array);
    expect(decoded.coordinates).toHaveLength(VERTEX_COUNT * 2);
    expect(decoded.triangles).toHaveLength(TRIANGLE_COUNT * 3);

    const coordinates = payload.coordinates_f64_le;
    for (let index = 0; index < decoded.coordinates.length; index += 1) {
      expect(decoded.coordinates[index]).toBe(coordinates.getFloat64(index * 8, true));
    }
    const triangles = payload.triangles_u32_le;
    for (let index = 0; index < decoded.triangles.length; index += 1) {
      expect(decoded.triangles[index]).toBe(triangles.getUint32(index * 4, true));
    }
  });

  it("rejects profile, digest, count, closed-member, and valid-hex authority drift", () => {
    expectRejected({ ...fixture(), profile: "circular-hole-chordal-reference-50/v2" });
    expectRejected({ ...fixture(), mesh_digest: "f".repeat(64) });
    expectRejected({ ...fixture(), mesh_digest: MESH_DIGEST.toUpperCase() });
    expectRejected({ ...fixture(), vertex_count: VERTEX_COUNT - 1 });
    expectRejected({ ...fixture(), triangle_count: TRIANGLE_COUNT + 1 });
    expectRejected({ ...fixture(), mode: "surface" });
  });

  it("rejects truncated, appended, native-endian-swapped, and same-size coordinate drift", () => {
    const accepted = fixture();
    const coordinateBytes = new Uint8Array(
      accepted.coordinates_f64_le.buffer,
      accepted.coordinates_f64_le.byteOffset,
      accepted.coordinates_f64_le.byteLength,
    );
    expect(coordinateBytes).toHaveLength(COORDINATE_BYTES);
    expectRejected({ ...accepted, coordinates_f64_le: view(coordinateBytes.slice(0, -1)) });
    expectRejected({
      ...accepted,
      coordinates_f64_le: view(Uint8Array.from([...coordinateBytes, 0])),
    });
    expectRejected({
      ...accepted,
      coordinates_f64_le: mutatedBytes(accepted.coordinates_f64_le, (copy) => {
        const original = copy.getUint8(0);
        copy.setUint8(0, original ^ 1);
      }),
    });
    expectRejected({
      ...accepted,
      coordinates_f64_le: mutatedBytes(accepted.coordinates_f64_le, (copy) => {
        for (let offset = 0; offset < copy.byteLength; offset += 8) {
          const group = Array.from({ length: 8 }, (_, index) => copy.getUint8(offset + index));
          for (let index = 0; index < 8; index += 1) {
            copy.setUint8(offset + index, group[7 - index]);
          }
        }
      }),
    });
  });

  it("rejects non-finite coordinates before renderer construction", () => {
    const accepted = fixture();
    expectRejected({
      ...accepted,
      coordinates_f64_le: mutatedBytes(accepted.coordinates_f64_le, (copy) => {
        copy.setFloat64(0, Number.POSITIVE_INFINITY, true);
      }),
    });
    expectRejected({
      ...accepted,
      coordinates_f64_le: mutatedBytes(accepted.coordinates_f64_le, (copy) => {
        copy.setFloat64(8, Number.NaN, true);
      }),
    });
  });

  it("rejects connectivity byte drift, wrong endian, range, incompleteness, and degeneracy", () => {
    const accepted = fixture();
    const triangleBytes = new Uint8Array(
      accepted.triangles_u32_le.buffer,
      accepted.triangles_u32_le.byteOffset,
      accepted.triangles_u32_le.byteLength,
    );
    expect(triangleBytes).toHaveLength(TRIANGLE_BYTES);
    expectRejected({ ...accepted, triangles_u32_le: view(triangleBytes.slice(0, -4)) });
    expectRejected({
      ...accepted,
      triangles_u32_le: mutatedBytes(accepted.triangles_u32_le, (copy) => {
        copy.setUint8(copy.byteLength - 1, copy.getUint8(copy.byteLength - 1) ^ 1);
      }),
    });
    expectRejected({
      ...accepted,
      triangles_u32_le: mutatedBytes(accepted.triangles_u32_le, (copy) => {
        for (let offset = 0; offset < copy.byteLength; offset += 4) {
          const group = Array.from({ length: 4 }, (_, index) => copy.getUint8(offset + index));
          for (let index = 0; index < 4; index += 1) {
            copy.setUint8(offset + index, group[3 - index]);
          }
        }
      }),
    });
    expectRejected({
      ...accepted,
      triangles_u32_le: mutatedBytes(accepted.triangles_u32_le, (copy) => {
        copy.setUint32(0, VERTEX_COUNT, true);
      }),
    });
    expectRejected({
      ...accepted,
      triangles_u32_le: mutatedBytes(accepted.triangles_u32_le, (copy) => {
        copy.setUint32(4, copy.getUint32(0, true), true);
      }),
    });
  });
});
