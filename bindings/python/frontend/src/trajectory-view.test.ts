import { describe, expect, it } from "vitest";

import type { TrajectoryModel } from "./trajectory-contract";
import { render } from "./trajectory-view";

class ElementStub {
	readonly children: ElementStub[] = [];
	readonly attributes = new Map<string, string>();
	readonly dataset: Record<string, string> = {};
	readonly listeners = new Map<string, Array<() => void>>();
	className = "";
	textContent = "";
	type = "";
	value = "";
	min = "";
	max = "";
	step = "";
	disabled = false;
	selected = false;
	width = 0;
	height = 0;

	constructor(readonly tagName: string) {}

	append(child: ElementStub): void {
		this.children.push(child);
		if (this.tagName === "select" && child.tagName === "option") {
			if (this.children.length === 1 || child.selected) this.value = child.value;
		}
	}

	setAttribute(name: string, value: string): void {
		this.attributes.set(name, value);
	}

	addEventListener(name: string, listener: () => void): void {
		const listeners = this.listeners.get(name) ?? [];
		listeners.push(listener);
		this.listeners.set(name, listeners);
	}

	dispatch(name: string): void {
		for (const listener of this.listeners.get(name) ?? []) listener();
	}

	replaceChildren(): void {
		this.children.length = 0;
	}

	getContext(): object {
		return {
			beginPath() {},
			clearRect() {},
			closePath() {},
			fill() {},
			lineTo() {},
			moveTo() {},
			stroke() {},
			fillStyle: "",
			strokeStyle: "",
		};
	}
}

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

function fixture(): TrajectoryModel {
	const payload = new Map<string, unknown>([
		["profile", "fixed-mesh-scalar-trajectory-2d/v1"],
		["trajectory_digest", "a".repeat(64)],
		["mesh_digest", "b".repeat(64)],
		["vertex_count", 9],
		["triangle_count", 8],
		["state_count", 2],
		["state_digests", `${"c".repeat(64)},${"d".repeat(64)}`],
		["snapshot_digests", `${"e".repeat(64)},${"f".repeat(64)}`],
		["field_id", "temperature"],
		["dimension", "0,0,0,1,0,0,0"],
		["frame", "invariant"],
		[
			"coordinates_f64_le",
			view([0, 0, 1, 0, 2, 0, 0, 1, 1, 1, 2, 1, 0, 2, 1, 2, 2, 2], "f64"),
		],
		[
			"triangles_u32_le",
			view(
				[0, 1, 4, 0, 4, 3, 1, 2, 5, 1, 5, 4, 3, 4, 7, 3, 7, 6, 4, 5, 8, 4, 8, 7],
				"u32",
			),
		],
		["support_u32_le", view([0, 1, 2, 3, 4, 5], "u32")],
		["steps_u64_le", view([1, 2], "u64")],
		["times_f64_le", view([0.05, 0.1], "f64")],
		["values_f64_le", view([1, 2, 3, 4, 5, 6, 6, 5, 4, 3, 2, 1], "f64")],
	]);
	for (const [name, hash] of Object.entries({
		coordinates: "d14c40e37814fef8de4af256f4af14916a3fb6759c8948773d1f6d7dbac29ef4",
		triangles: "aa6b372beee3efc1358fd8549e7b69415a821b65b36dbe1cbc98dd23c3e1f1ca",
		support: "cd9a54ed1f18bf97db08914e280ea7349e11ca2c4885a4d8052552ceba84208d",
		steps: "0c730b69905c5ef7a4ca5269f72365400bde2dd2c04eaf9bbb3d1c4a265a0131",
		times: "316036b7b4cdd18925380a3187bcbc3cc0f621862ca65a69440b91b251d963b5",
		values: "b853634323f4a5d87426649faf4b57745e80ebbd1f1dd1d4114adada10cf15d7",
	}))
		payload.set(`${name}_sha256`, hash);
	return { get: (name) => payload.get(name) };
}

describe("Trajectory visible controls", () => {
	it("updates visible metadata and discrete control state", () => {
		const previousDocument = globalThis.document;
		const previousWindow = globalThis.window;
		Object.assign(globalThis, {
			document: {
				createElement: (tagName: string) => new ElementStub(tagName),
			},
			window: {
				clearInterval() {},
				setInterval: () => 1,
			},
		});
		try {
			const host = new ElementStub("div");
			const cleanup = render({ model: fixture(), el: host as unknown as HTMLElement });
			const root = host.children[0];
			const canvas = root.children[0];
			const metadata = root.children[1];
			const controls = root.children[2];
			const [previous, , next, slider] = controls.children;

			expect(canvas.attributes.get("role")).toBe("img");
			expect(canvas.attributes.get("aria-label")).toBe(
				`Eqiora Trajectory ${"a".repeat(64)}; field temperature; coherent-SI dimension [0, 0, 0, 1, 0, 0, 0]; invariant frame; 2 stored states.`,
			);
			expect(canvas.textContent).toBe(canvas.attributes.get("aria-label"));
			expect(metadata.textContent).toContain("state 1/2 · step 1 · t=0.05 s");
			expect(slider.value).toBe("0");
			expect(previous.disabled).toBe(true);
			expect(next.disabled).toBe(false);

			next.dispatch("click");
			expect(metadata.textContent).toContain("state 2/2 · step 2 · t=0.1 s");
			expect(slider.value).toBe("1");
			expect(previous.disabled).toBe(false);
			expect(next.disabled).toBe(true);

			previous.dispatch("click");
			expect(metadata.textContent).toContain("state 1/2 · step 1 · t=0.05 s");
			expect(slider.value).toBe("0");
			cleanup();
			expect(host.children).toHaveLength(0);
		} finally {
			Object.assign(globalThis, { document: previousDocument, window: previousWindow });
		}
	});
});
