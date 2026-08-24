const PROFILE = "fixed-mesh-scalar-trajectory-2d/v1";
const VERTEX_COUNT = 9;
const TRIANGLE_COUNT = 8;
const STATE_COUNT = 2;
const SUPPORT_COUNT = 6;

export interface TrajectoryModel {
	get(name: string): unknown;
}

export interface DecodedTrajectory {
	readonly trajectoryDigest: string;
	readonly meshDigest: string;
	readonly fieldId: string;
	readonly dimension: readonly number[];
	readonly frame: "invariant";
	readonly coordinates: Float64Array;
	readonly triangles: Uint32Array;
	readonly support: Uint32Array;
	readonly steps: readonly bigint[];
	readonly times: Float64Array;
	readonly values: Float64Array;
}

function fail(message: string): never {
	throw new Error(`Invalid Eqiora Trajectory presentation payload: ${message}`);
}

function exactString(value: unknown, expected: string, name: string): string {
	if (typeof value !== "string" || value !== expected) fail(`${name} changed`);
	return value;
}

function digest(value: unknown, name: string): string {
	if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) {
		fail(`${name} is not a lowercase SHA-256 digest`);
	}
	return value;
}

function count(value: unknown, expected: number, name: string): number {
	if (!Number.isSafeInteger(value) || value !== expected) fail(`${name} changed`);
	return value as number;
}

function bytes(value: unknown, length: number, name: string): Uint8Array<ArrayBuffer> {
	if (!(value instanceof DataView) || value.byteLength !== length) {
		fail(`${name} has the wrong binary type or byte length`);
	}
	const copy = new Uint8Array(new ArrayBuffer(length));
	copy.set(new Uint8Array(value.buffer, value.byteOffset, value.byteLength));
	return copy;
}

function rotateRight(value: number, places: number): number {
	return (value >>> places) | (value << (32 - places));
}

const INITIAL = new Uint32Array([
	0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
	0x5be0cd19,
]);
const ROUNDS = new Uint32Array([
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

function sha256(input: Uint8Array<ArrayBuffer>): string {
	const bitLength = input.byteLength * 8;
	const paddedLength = Math.ceil((input.byteLength + 9) / 64) * 64;
	const padded = new Uint8Array(new ArrayBuffer(paddedLength));
	padded.set(input);
	padded[input.byteLength] = 0x80;
	const view = new DataView(padded.buffer);
	view.setUint32(paddedLength - 8, Math.floor(bitLength / 0x1_0000_0000), false);
	view.setUint32(paddedLength - 4, bitLength >>> 0, false);
	const state = new Uint32Array(INITIAL);
	const words = new Uint32Array(64);
	for (let offset = 0; offset < paddedLength; offset += 64) {
		for (let index = 0; index < 16; index += 1)
			words[index] = view.getUint32(offset + index * 4, false);
		for (let index = 16; index < 64; index += 1) {
			const a = words[index - 15],
				b = words[index - 2];
			words[index] =
				(words[index - 16] +
					(rotateRight(a, 7) ^ rotateRight(a, 18) ^ (a >>> 3)) +
					words[index - 7] +
					(rotateRight(b, 17) ^ rotateRight(b, 19) ^ (b >>> 10))) >>>
				0;
		}
		let [a, b, c, d, e, f, g, h] = state;
		for (let index = 0; index < 64; index += 1) {
			const t1 =
				(h +
					(rotateRight(e, 6) ^ rotateRight(e, 11) ^ rotateRight(e, 25)) +
					((e & f) ^ (~e & g)) +
					ROUNDS[index] +
					words[index]) >>>
				0;
			const t2 =
				((rotateRight(a, 2) ^ rotateRight(a, 13) ^ rotateRight(a, 22)) +
					((a & b) ^ (a & c) ^ (b & c))) >>>
				0;
			[h, g, f, e, d, c, b, a] = [g, f, e, (d + t1) >>> 0, c, b, a, (t1 + t2) >>> 0];
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
	return Array.from(state, (value) => value.toString(16).padStart(8, "0")).join("");
}

function verified(
	model: TrajectoryModel,
	name: string,
	length: number,
): Uint8Array<ArrayBuffer> {
	const value = bytes(model.get(name), length, name);
	if (
		sha256(value) !==
		digest(model.get(name.replace(/_(?:f64|u32|u64)_le$/, "_sha256")), `${name} hash`)
	) {
		fail(`${name} digest disagrees with its bytes`);
	}
	return value;
}

function floats(bytes: Uint8Array<ArrayBuffer>): Float64Array {
	const result = new Float64Array(bytes.byteLength / 8);
	const view = new DataView(bytes.buffer);
	for (let i = 0; i < result.length; i += 1) result[i] = view.getFloat64(i * 8, true);
	return result;
}

function uints(bytes: Uint8Array<ArrayBuffer>): Uint32Array {
	const result = new Uint32Array(bytes.byteLength / 4);
	const view = new DataView(bytes.buffer);
	for (let i = 0; i < result.length; i += 1) result[i] = view.getUint32(i * 4, true);
	return result;
}

export function decodeTrajectoryContract(model: TrajectoryModel): DecodedTrajectory {
	exactString(model.get("profile"), PROFILE, "profile");
	count(model.get("vertex_count"), VERTEX_COUNT, "vertex_count");
	count(model.get("triangle_count"), TRIANGLE_COUNT, "triangle_count");
	count(model.get("state_count"), STATE_COUNT, "state_count");
	for (const name of ["state_digests", "snapshot_digests"]) {
		const values = model.get(name);
		if (
			typeof values !== "string" ||
			values.split(",").length !== STATE_COUNT ||
			!values.split(",").every((value) => /^[0-9a-f]{64}$/.test(value))
		)
			fail(`${name} changed`);
	}
	const dimension = model.get("dimension");
	if (typeof dimension !== "string" || !/^-?\d+(,-?\d+){6}$/.test(dimension))
		fail("dimension changed");
	const dimensionValues = dimension.split(",").map(Number);
	if (!dimensionValues.every(Number.isSafeInteger)) fail("dimension changed");
	const coordinates = floats(
		verified(model, "coordinates_f64_le", VERTEX_COUNT * 2 * 8),
	);
	const triangles = uints(verified(model, "triangles_u32_le", TRIANGLE_COUNT * 3 * 4));
	const support = uints(verified(model, "support_u32_le", SUPPORT_COUNT * 4));
	const stepBytes = verified(model, "steps_u64_le", STATE_COUNT * 8);
	const times = floats(verified(model, "times_f64_le", STATE_COUNT * 8));
	const values = floats(
		verified(model, "values_f64_le", STATE_COUNT * SUPPORT_COUNT * 8),
	);
	const stepView = new DataView(stepBytes.buffer);
	const steps = Array.from({ length: STATE_COUNT }, (_, i) =>
		stepView.getBigUint64(i * 8, true),
	);
	if (
		!coordinates.every(Number.isFinite) ||
		!times.every(Number.isFinite) ||
		!values.every(Number.isFinite)
	)
		fail("non-finite numeric member");
	if (times[1] <= times[0] || steps[1] <= steps[0])
		fail("states are not strictly ordered");
	if (
		triangles.some((value) => value >= VERTEX_COUNT) ||
		support.some((value) => value >= VERTEX_COUNT) ||
		new Set(support).size !== SUPPORT_COUNT
	)
		fail("topology or support is invalid");
	return {
		trajectoryDigest: digest(model.get("trajectory_digest"), "trajectory_digest"),
		meshDigest: digest(model.get("mesh_digest"), "mesh_digest"),
		fieldId: exactString(
			model.get("field_id"),
			model.get("field_id") as string,
			"field_id",
		),
		dimension: Object.freeze(dimensionValues),
		frame: exactString(model.get("frame"), "invariant", "frame") as "invariant",
		coordinates,
		triangles,
		support,
		steps: Object.freeze(steps),
		times,
		values,
	};
}
