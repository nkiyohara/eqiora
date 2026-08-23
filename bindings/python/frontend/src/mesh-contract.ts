const PROFILE = "circular-hole-gmsh-4.15.2/v1";
export const MESH_DIGEST =
	"5962836788fa785fd0761813c542e9078523796409787d86ad8a006dfef5b62b";
const VERTEX_COUNT = 662;
const TRIANGLE_COUNT = 1_210;
const COORDINATE_BYTES = 10_592;
const TRIANGLE_BYTES = 14_520;
const COORDINATE_SHA256 =
	"42ea585f3facdc21fadf66435f37f1127bf926e6159c5ff1e4a345ba7268db3d";
const TRIANGLE_SHA256 =
	"05a68c5630e68ed091e7da3bff07516a9ddf9345bc8319db108ac4004a7c6642";
const PAYLOAD_FIELDS = [
	"profile",
	"mesh_digest",
	"vertex_count",
	"triangle_count",
	"coordinates_f64_le",
	"triangles_u32_le",
] as const;

const SHA256_INITIAL = new Uint32Array([
	0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c,
	0x1f83d9ab, 0x5be0cd19,
]);
const SHA256_ROUNDS = new Uint32Array([
	0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1,
	0x923f82a4, 0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3,
	0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786,
	0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
	0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147,
	0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13,
	0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
	0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
	0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a,
	0x5b9cca4f, 0x682e6ff3, 0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208,
	0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
]);

const MODES = ["surface", "wireframe", "points"] as const;
export type RepresentationMode = (typeof MODES)[number];

export interface MeshModel {
	get(name: string): unknown;
	on?(event: string, callback: () => void): void;
	off?(event: string, callback: () => void): void;
}

export interface DecodedMesh {
	readonly digest: typeof MESH_DIGEST;
	readonly coordinates: Float64Array;
	readonly triangles: Uint32Array;
}

interface MeshPayload {
	readonly profile: unknown;
	readonly mesh_digest: unknown;
	readonly vertex_count: unknown;
	readonly triangle_count: unknown;
	readonly coordinates_f64_le: unknown;
	readonly triangles_u32_le: unknown;
}

function fail(message: string): never {
	throw new Error(`Invalid Eqiora Mesh presentation payload: ${message}`);
}

function exactString(value: unknown, expected: string, name: string): string {
	if (typeof value !== "string" || value !== expected) {
		fail(`${name} is not the accepted literal`);
	}
	return value;
}

function exactCount(value: unknown, expected: number, name: string): number {
	if (!Number.isSafeInteger(value) || value !== expected) {
		fail(`${name} disagrees with the accepted digest`);
	}
	return value as number;
}

function exactBytes(
	value: unknown,
	length: number,
	name: string,
): Uint8Array<ArrayBuffer> {
	if (!(value instanceof DataView) || value.byteLength !== length) {
		fail(`${name} has the wrong binary type or byte length`);
	}
	const owned = new Uint8Array(new ArrayBuffer(length));
	owned.set(new Uint8Array(value.buffer, value.byteOffset, value.byteLength));
	return owned;
}

function rotateRight(value: number, places: number): number {
	return (value >>> places) | (value << (32 - places));
}

function sha256(bytes: Uint8Array<ArrayBuffer>): string {
	const bitLength = bytes.byteLength * 8;
	const paddedLength = Math.ceil((bytes.byteLength + 9) / 64) * 64;
	const padded = new Uint8Array(new ArrayBuffer(paddedLength));
	padded.set(bytes);
	padded[bytes.byteLength] = 0x80;
	const paddedView = new DataView(padded.buffer);
	paddedView.setUint32(
		paddedLength - 8,
		Math.floor(bitLength / 0x1_0000_0000),
		false,
	);
	paddedView.setUint32(paddedLength - 4, bitLength >>> 0, false);

	const state = new Uint32Array(SHA256_INITIAL);
	const words = new Uint32Array(64);
	for (let offset = 0; offset < paddedLength; offset += 64) {
		for (let index = 0; index < 16; index += 1) {
			words[index] = paddedView.getUint32(offset + index * 4, false);
		}
		for (let index = 16; index < 64; index += 1) {
			const previous = words[index - 15];
			const earlier = words[index - 2];
			const sigma0 =
				rotateRight(previous, 7) ^ rotateRight(previous, 18) ^ (previous >>> 3);
			const sigma1 =
				rotateRight(earlier, 17) ^ rotateRight(earlier, 19) ^ (earlier >>> 10);
			words[index] =
				(words[index - 16] + sigma0 + words[index - 7] + sigma1) >>> 0;
		}

		let a = state[0];
		let b = state[1];
		let c = state[2];
		let d = state[3];
		let e = state[4];
		let f = state[5];
		let g = state[6];
		let h = state[7];
		for (let index = 0; index < 64; index += 1) {
			const choose = (e & f) ^ (~e & g);
			const majority = (a & b) ^ (a & c) ^ (b & c);
			const sum0 = rotateRight(a, 2) ^ rotateRight(a, 13) ^ rotateRight(a, 22);
			const sum1 = rotateRight(e, 6) ^ rotateRight(e, 11) ^ rotateRight(e, 25);
			const temporary1 =
				(h + sum1 + choose + SHA256_ROUNDS[index] + words[index]) >>> 0;
			const temporary2 = (sum0 + majority) >>> 0;
			h = g;
			g = f;
			f = e;
			e = (d + temporary1) >>> 0;
			d = c;
			c = b;
			b = a;
			a = (temporary1 + temporary2) >>> 0;
		}
		state[0] = (state[0] + a) >>> 0;
		state[1] = (state[1] + b) >>> 0;
		state[2] = (state[2] + c) >>> 0;
		state[3] = (state[3] + d) >>> 0;
		state[4] = (state[4] + e) >>> 0;
		state[5] = (state[5] + f) >>> 0;
		state[6] = (state[6] + g) >>> 0;
		state[7] = (state[7] + h) >>> 0;
	}
	return Array.from(state, (word) => word.toString(16).padStart(8, "0")).join(
		"",
	);
}

function exactPayload(value: unknown): MeshPayload {
	if (typeof value !== "object" || value === null || Array.isArray(value)) {
		fail("payload is not one closed object");
	}
	const keys = Reflect.ownKeys(value);
	if (
		keys.length !== PAYLOAD_FIELDS.length ||
		keys.some(
			(key) =>
				typeof key !== "string" ||
				!PAYLOAD_FIELDS.includes(key as (typeof PAYLOAD_FIELDS)[number]),
		)
	) {
		fail("payload members are not the exact closed set");
	}
	return value as MeshPayload;
}

export function validateRepresentationMode(value: unknown): RepresentationMode {
	if (
		typeof value !== "string" ||
		!MODES.includes(value as RepresentationMode)
	) {
		fail("representation mode is unknown");
	}
	return value as RepresentationMode;
}

export function decodeMeshPayload(value: unknown): DecodedMesh {
	const payload = exactPayload(value);
	exactString(payload.profile, PROFILE, "profile");
	const digest = exactString(payload.mesh_digest, MESH_DIGEST, "mesh_digest");
	const vertexCount = exactCount(
		payload.vertex_count,
		VERTEX_COUNT,
		"vertex_count",
	);
	const triangleCount = exactCount(
		payload.triangle_count,
		TRIANGLE_COUNT,
		"triangle_count",
	);
	const coordinateBytes = exactBytes(
		payload.coordinates_f64_le,
		COORDINATE_BYTES,
		"coordinates_f64_le",
	);
	const triangleBytes = exactBytes(
		payload.triangles_u32_le,
		TRIANGLE_BYTES,
		"triangles_u32_le",
	);

	const coordinateHash = sha256(coordinateBytes);
	const triangleHash = sha256(triangleBytes);
	if (
		coordinateHash !== COORDINATE_SHA256 ||
		triangleHash !== TRIANGLE_SHA256
	) {
		fail("same-size array bytes do not belong to the accepted digest");
	}

	const coordinates = new Float64Array(vertexCount * 2);
	const coordinateView = new DataView(
		coordinateBytes.buffer,
		coordinateBytes.byteOffset,
		coordinateBytes.byteLength,
	);
	for (let index = 0; index < coordinates.length; index += 1) {
		const value = coordinateView.getFloat64(index * 8, true);
		if (!Number.isFinite(value)) {
			fail("coordinates contain a non-finite value");
		}
		coordinates[index] = value;
	}

	const triangles = new Uint32Array(triangleCount * 3);
	const triangleView = new DataView(
		triangleBytes.buffer,
		triangleBytes.byteOffset,
		triangleBytes.byteLength,
	);
	for (let index = 0; index < triangles.length; index += 1) {
		const vertex = triangleView.getUint32(index * 4, true);
		if (vertex >= vertexCount) {
			fail("triangle connectivity contains an out-of-range index");
		}
		triangles[index] = vertex;
	}

	for (let cell = 0; cell < triangleCount; cell += 1) {
		const a = triangles[cell * 3];
		const b = triangles[cell * 3 + 1];
		const c = triangles[cell * 3 + 2];
		if (a === b || b === c || c === a) {
			fail("triangle connectivity is degenerate");
		}
		const ax = coordinates[a * 2];
		const ay = coordinates[a * 2 + 1];
		const bx = coordinates[b * 2];
		const by = coordinates[b * 2 + 1];
		const cx = coordinates[c * 2];
		const cy = coordinates[c * 2 + 1];
		const twiceArea = (bx - ax) * (cy - ay) - (by - ay) * (cx - ax);
		if (!Number.isFinite(twiceArea) || twiceArea === 0) {
			fail("triangle geometry is degenerate");
		}
	}

	return {
		digest: digest as typeof MESH_DIGEST,
		coordinates,
		triangles,
	};
}

export async function decodeMeshContract(
	model: MeshModel,
): Promise<DecodedMesh> {
	return decodeMeshPayload({
		profile: model.get("profile"),
		mesh_digest: model.get("mesh_digest"),
		vertex_count: model.get("vertex_count"),
		triangle_count: model.get("triangle_count"),
		coordinates_f64_le: model.get("coordinates_f64_le"),
		triangles_u32_le: model.get("triangles_u32_le"),
	});
}
