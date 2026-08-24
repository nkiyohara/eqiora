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
const SELECTION_MEMBERSHIP_DOMAIN = new TextEncoder().encode(
	"eqiora.mesh-selection-membership/v1\0",
);
const SELECTION_NAMES = ["cylinder", "inlet", "outlet", "walls", "fluid"] as const;
const SELECTION_DIMENSIONS = [1, 1, 1, 1, 2] as const;
const SELECTION_COUNTS = [50, 14, 2, 48, TRIANGLE_COUNT] as const;
const PAYLOAD_FIELDS = [
	"profile",
	"mesh_digest",
	"vertex_count",
	"triangle_count",
	"coordinates_f64_le",
	"triangles_u32_le",
	"correspondence_digest",
	"selection_membership",
	"selection_membership_sha256",
] as const;

const SHA256_INITIAL = new Uint32Array([
	0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
	0x5be0cd19,
]);
const SHA256_ROUNDS = new Uint32Array([
	0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
	0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
	0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
	0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
	0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
	0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
	0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
	0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
	0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
	0xc67178f2,
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
	readonly correspondenceDigest: string;
	readonly coordinates: Float64Array;
	readonly triangles: Uint32Array;
	readonly selections: readonly DecodedSelection[];
	readonly selectionMembershipDigest: string;
}

export type SelectionName = (typeof SELECTION_NAMES)[number];

export interface DecodedSelection {
	readonly name: SelectionName;
	readonly dimension: 1 | 2;
	readonly entityIndices: Uint32Array;
	readonly vertexIndices: Uint32Array;
}

interface MeshPayload {
	readonly profile: unknown;
	readonly mesh_digest: unknown;
	readonly vertex_count: unknown;
	readonly triangle_count: unknown;
	readonly coordinates_f64_le: unknown;
	readonly triangles_u32_le: unknown;
	readonly correspondence_digest: unknown;
	readonly selection_membership: unknown;
	readonly selection_membership_sha256: unknown;
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

function digestString(value: unknown, name: string): string {
	if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) {
		fail(`${name} is not one canonical SHA-256 digest`);
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

function ownedBytes(value: unknown, name: string): Uint8Array<ArrayBuffer> {
	if (!(value instanceof DataView) || value.byteLength === 0) {
		fail(`${name} has the wrong binary type or is empty`);
	}
	const owned = new Uint8Array(new ArrayBuffer(value.byteLength));
	owned.set(new Uint8Array(value.buffer, value.byteOffset, value.byteLength));
	return owned;
}

function rotateRight(value: number, places: number): number {
	return (value >>> places) | (value << (32 - places));
}

export function sha256ForTransport(bytes: Uint8Array<ArrayBuffer>): string {
	const bitLength = bytes.byteLength * 8;
	const paddedLength = Math.ceil((bytes.byteLength + 9) / 64) * 64;
	const padded = new Uint8Array(new ArrayBuffer(paddedLength));
	padded.set(bytes);
	padded[bytes.byteLength] = 0x80;
	const paddedView = new DataView(padded.buffer);
	paddedView.setUint32(paddedLength - 8, Math.floor(bitLength / 0x1_0000_0000), false);
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
			words[index] = (words[index - 16] + sigma0 + words[index - 7] + sigma1) >>> 0;
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
	return Array.from(state, (word) => word.toString(16).padStart(8, "0")).join("");
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
	if (typeof value !== "string" || !MODES.includes(value as RepresentationMode)) {
		fail("representation mode is unknown");
	}
	return value as RepresentationMode;
}

function decodeSelectionMembership(
	encoded: Uint8Array<ArrayBuffer>,
	expectedDigest: string,
	triangles: Uint32Array,
): readonly DecodedSelection[] {
	if (encoded.byteLength > 100_000) {
		fail("selection_membership exceeds the private presentation bound");
	}
	let offset = 0;
	const take = (length: number): Uint8Array<ArrayBuffer> => {
		if (
			!Number.isSafeInteger(length) ||
			length < 0 ||
			offset + length > encoded.byteLength
		) {
			fail("selection_membership is truncated or has an invalid length");
		}
		const value = encoded.slice(offset, offset + length);
		offset += length;
		return value;
	};
	const u32 = (): number => {
		const value = take(4);
		return new DataView(value.buffer, value.byteOffset, value.byteLength).getUint32(
			0,
			true,
		);
	};
	const literal = take(SELECTION_MEMBERSHIP_DOMAIN.byteLength);
	if (!literal.every((value, index) => value === SELECTION_MEMBERSHIP_DOMAIN[index])) {
		fail("selection_membership has an unknown domain");
	}
	const embeddedDigest = new TextDecoder("ascii", { fatal: true }).decode(take(64));
	if (embeddedDigest !== expectedDigest) {
		fail("selection_membership belongs to another correspondence");
	}
	if (u32() !== SELECTION_NAMES.length) {
		fail("selection_membership has the wrong selection inventory");
	}

	const edgeMap = new Map<
		string,
		{ vertices: readonly [number, number]; incidence: number }
	>();
	for (let cell = 0; cell < TRIANGLE_COUNT; cell += 1) {
		const triangle = [
			triangles[cell * 3],
			triangles[cell * 3 + 1],
			triangles[cell * 3 + 2],
		] as const;
		for (const [left, right] of [
			[triangle[0], triangle[1]],
			[triangle[0], triangle[2]],
			[triangle[1], triangle[2]],
		] as const) {
			const vertices: readonly [number, number] =
				left < right ? [left, right] : [right, left];
			const key = `${vertices[0]}:${vertices[1]}`;
			const current = edgeMap.get(key);
			edgeMap.set(key, { vertices, incidence: (current?.incidence ?? 0) + 1 });
		}
	}
	const canonicalEdges = Array.from(edgeMap.values()).sort(
		(left, right) =>
			left.vertices[0] - right.vertices[0] || left.vertices[1] - right.vertices[1],
	);
	const selectedBoundaryEntities = new Set<number>();
	const decoder = new TextDecoder("utf-8", { fatal: true });
	const selections: DecodedSelection[] = [];
	for (
		let selectionIndex = 0;
		selectionIndex < SELECTION_NAMES.length;
		selectionIndex += 1
	) {
		let name: string;
		try {
			name = decoder.decode(take(u32()));
		} catch {
			fail("selection_membership contains an invalid UTF-8 name");
		}
		const expectedName = SELECTION_NAMES[selectionIndex];
		const expectedDimension = SELECTION_DIMENSIONS[selectionIndex];
		const dimension = u32();
		const entityCount = u32();
		if (
			name !== expectedName ||
			dimension !== expectedDimension ||
			entityCount !== SELECTION_COUNTS[selectionIndex]
		) {
			fail(
				"selection_membership name, dimension, or count is not the accepted inventory",
			);
		}
		const entityIndices = new Uint32Array(entityCount);
		const vertexIndices = new Uint32Array(entityCount * (dimension + 1));
		let previous = -1;
		for (let member = 0; member < entityCount; member += 1) {
			const entity = u32();
			const vertexCount = u32();
			if (entity <= previous || vertexCount !== dimension + 1) {
				fail("selection_membership is not canonical or has the wrong entity closure");
			}
			previous = entity;
			entityIndices[member] = entity;
			const vertices: number[] = [];
			for (let local = 0; local < vertexCount; local += 1) {
				const vertex = u32();
				if (vertex >= VERTEX_COUNT || vertices.includes(vertex)) {
					fail("selection_membership has a repeated or out-of-range vertex");
				}
				vertices.push(vertex);
				vertexIndices[member * vertexCount + local] = vertex;
			}
			if (dimension === 1) {
				const edge = canonicalEdges[entity];
				if (
					edge === undefined ||
					edge.incidence !== 1 ||
					edge.vertices[0] !== vertices[0] ||
					edge.vertices[1] !== vertices[1] ||
					selectedBoundaryEntities.has(entity)
				) {
					fail("selection_membership does not identify canonical boundary entities");
				}
				selectedBoundaryEntities.add(entity);
			} else if (
				entity >= TRIANGLE_COUNT ||
				vertices.some((vertex, local) => triangles[entity * 3 + local] !== vertex)
			) {
				fail("selection_membership does not identify canonical cells");
			}
		}
		selections.push({
			name: expectedName,
			dimension: expectedDimension,
			entityIndices,
			vertexIndices,
		});
	}
	if (offset !== encoded.byteLength) {
		fail("selection_membership has trailing bytes");
	}
	const boundaryCount = canonicalEdges.filter((edge) => edge.incidence === 1).length;
	if (selectedBoundaryEntities.size !== boundaryCount) {
		fail("selection_membership does not partition the canonical boundary");
	}
	return selections;
}

export function decodeMeshPayload(value: unknown): DecodedMesh {
	const payload = exactPayload(value);
	exactString(payload.profile, PROFILE, "profile");
	const digest = exactString(payload.mesh_digest, MESH_DIGEST, "mesh_digest");
	const vertexCount = exactCount(payload.vertex_count, VERTEX_COUNT, "vertex_count");
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

	const coordinateHash = sha256ForTransport(coordinateBytes);
	const triangleHash = sha256ForTransport(triangleBytes);
	if (coordinateHash !== COORDINATE_SHA256 || triangleHash !== TRIANGLE_SHA256) {
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
	const correspondenceDigest = digestString(
		payload.correspondence_digest,
		"correspondence_digest",
	);
	const selectionMembership = ownedBytes(
		payload.selection_membership,
		"selection_membership",
	);
	const selectionMembershipDigest = digestString(
		payload.selection_membership_sha256,
		"selection_membership_sha256",
	);
	if (sha256ForTransport(selectionMembership) !== selectionMembershipDigest) {
		fail("selection_membership bytes disagree with their transport digest");
	}
	const selections = decodeSelectionMembership(
		selectionMembership,
		correspondenceDigest,
		triangles,
	);

	return {
		digest: digest as typeof MESH_DIGEST,
		correspondenceDigest,
		coordinates,
		triangles,
		selections,
		selectionMembershipDigest,
	};
}

export async function decodeMeshContract(model: MeshModel): Promise<DecodedMesh> {
	return decodeMeshPayload({
		profile: model.get("profile"),
		mesh_digest: model.get("mesh_digest"),
		vertex_count: model.get("vertex_count"),
		triangle_count: model.get("triangle_count"),
		coordinates_f64_le: model.get("coordinates_f64_le"),
		triangles_u32_le: model.get("triangles_u32_le"),
		correspondence_digest: model.get("correspondence_digest"),
		selection_membership: model.get("selection_membership"),
		selection_membership_sha256: model.get("selection_membership_sha256"),
	});
}
