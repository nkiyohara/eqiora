import { describe, expect, it } from "vitest";

import { decodeTrajectoryContract, type TrajectoryModel } from "./trajectory-contract";

function view(values: ArrayLike<number>, kind: "f64" | "u32" | "u64"): DataView {
	const width = kind === "u32" ? 4 : 8;
	const buffer = new ArrayBuffer(values.length * width);
	const output = new DataView(buffer);
	for (let index = 0; index < values.length; index += 1) {
		if (kind === "f64") output.setFloat64(index * width, Number(values[index]), true);
		else if (kind === "u32")
			output.setUint32(index * width, Number(values[index]), true);
		else output.setBigUint64(index * width, BigInt(values[index]), true);
	}
	return output;
}

function fixture(): Map<string, unknown> {
	const coordinates = view(
		[0, 0, 1, 0, 2, 0, 0, 1, 1, 1, 2, 1, 0, 2, 1, 2, 2, 2],
		"f64",
	);
	const triangles = view(
		[0, 1, 4, 0, 4, 3, 1, 2, 5, 1, 5, 4, 3, 4, 7, 3, 7, 6, 4, 5, 8, 4, 8, 7],
		"u32",
	);
	const support = view([0, 1, 2, 3, 4, 5], "u32");
	const steps = view([1, 4], "u64");
	const times = view([0.05, 0.17], "f64");
	const values = view([1, 2, 3, 4, 5, 6, 6, 5, 4, 3, 2, 1], "f64");
	const payload = new Map<string, unknown>([
		["profile", "fixed-mesh-scalar-trajectory-2d/v1"],
		["trajectory_digest", "a".repeat(64)],
		["mesh_digest", "b".repeat(64)],
		["vertex_count", 9],
		["triangle_count", 8],
		["state_count", 2],
		["state_digests", `${"c".repeat(64)},${"d".repeat(64)}`],
		["snapshot_digests", `${"e".repeat(64)},${"f".repeat(64)}`],
		["field_id", "01JFIELD000000000000000000"],
		["dimension", "1,-1,-2,0,0,0,0"],
		["frame", "invariant"],
		["coordinates_f64_le", coordinates],
		["triangles_u32_le", triangles],
		["support_u32_le", support],
		["steps_u64_le", steps],
		["times_f64_le", times],
		["values_f64_le", values],
	]);
	for (const [name, hash] of Object.entries({
		coordinates: "d14c40e37814fef8de4af256f4af14916a3fb6759c8948773d1f6d7dbac29ef4",
		triangles: "aa6b372beee3efc1358fd8549e7b69415a821b65b36dbe1cbc98dd23c3e1f1ca",
		support: "cd9a54ed1f18bf97db08914e280ea7349e11ca2c4885a4d8052552ceba84208d",
		steps: "0fb84472ffe2b1591c692c9d25d94d8c28050b69e42dfc3823eb373446684311",
		times: "2b05df4a98782f9246fd81d4a5b212b57623382107eb34d7926284139a6e2802",
		values: "b853634323f4a5d87426649faf4b57745e80ebbd1f1dd1d4114adada10cf15d7",
	}))
		payload.set(`${name}_sha256`, hash);
	return payload;
}

function model(payload: Map<string, unknown>): TrajectoryModel {
	return { get: (name) => payload.get(name) };
}

describe("decodeTrajectoryContract", () => {
	it("accepts exact bytes and preserves discrete nonuniform states", () => {
		const decoded = decodeTrajectoryContract(model(fixture()));
		expect(decoded.steps).toEqual([1n, 4n]);
		expect([...decoded.times]).toEqual([0.05, 0.17]);
		expect(decoded.values).toHaveLength(12);
	});

	it("rejects a byte mutant even when shape remains valid", () => {
		const payload = fixture();
		const values = payload.get("values_f64_le") as DataView;
		values.setUint8(0, values.getUint8(0) ^ 1);
		expect(() => decodeTrajectoryContract(model(payload))).toThrow(/digest disagrees/);
	});

	it("rejects interpolation-like duplicate or reordered state metadata", () => {
		const payload = fixture();
		payload.set("steps_u64_le", view([4, 1], "u64"));
		payload.set(
			"steps_sha256",
			"181d9408cee887a97d4c8d97f2f846ab0edc8d9f2c803793daaa119a16fbd824",
		);
		expect(() => decodeTrajectoryContract(model(payload))).toThrow(/strictly ordered/);
	});
});
