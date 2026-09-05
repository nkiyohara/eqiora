import * as THREE from "three";
import { OrbitControls } from "three/examples/jsm/controls/OrbitControls.js";
import {
	decodeScene,
	type BinaryInput,
	type DecodedScene,
	type GeometryLayer,
	type MeshLayer,
	type ScalarFieldLayer,
	type SelectionLayer,
} from "./contract";
import "./viewer.css";

interface RenderedTarget {
	readonly layer: GeometryLayer | MeshLayer;
	readonly group: THREE.Group;
	readonly coordinates: Float64Array;
	readonly connectivity: Uint32Array;
	readonly surface: THREE.Mesh | null;
	readonly edges: THREE.LineSegments;
	readonly triangleToCell: readonly number[];
}

interface ViewerState {
	readonly scene: THREE.Scene;
	readonly renderer: THREE.WebGLRenderer;
	readonly camera: THREE.PerspectiveCamera;
	readonly controls: OrbitControls;
	readonly targets: Map<string, RenderedTarget>;
	readonly disposables: Array<{ dispose(): void }>;
	highlight: THREE.Object3D | null;
	fieldSurface: THREE.Mesh | null;
	selectedField: ScalarFieldLayer | null;
	selectedSelection: SelectionLayer | null;
	selectionIsolated: boolean;
	surfacesVisible: boolean;
	edgesVisible: boolean;
	frame: number;
	cleaned: boolean;
}

export interface ViewerMount {
	readonly cleanup: () => void;
}

function element<K extends keyof HTMLElementTagNameMap>(
	name: K,
	className: string,
	parent: HTMLElement,
): HTMLElementTagNameMap[K] {
	const value = document.createElement(name);
	value.className = className;
	parent.append(value);
	return value;
}

function button(
	label: string,
	parent: HTMLElement,
	action: () => void,
): HTMLButtonElement {
	const value = element("button", "eqiora-viewer__button", parent);
	value.type = "button";
	value.textContent = label;
	value.setAttribute("aria-label", label);
	value.addEventListener("click", action);
	return value;
}

function f64(
	scene: DecodedScene,
	reference: { readonly buffer: number },
): Float64Array {
	const value = scene.buffers[reference.buffer];
	if (!(value instanceof Float64Array))
		throw new Error("viewer expected float64 buffer");
	return value;
}

function u32(scene: DecodedScene, reference: { readonly buffer: number }): Uint32Array {
	const value = scene.buffers[reference.buffer];
	if (!(value instanceof Uint32Array)) throw new Error("viewer expected uint32 buffer");
	return value;
}

function positions3(coordinates: Float64Array): Float32Array {
	const result = new Float32Array((coordinates.length / 2) * 3);
	for (let vertex = 0; vertex < coordinates.length / 2; vertex += 1) {
		result[vertex * 3] = coordinates[vertex * 2];
		result[vertex * 3 + 1] = coordinates[vertex * 2 + 1];
		result[vertex * 3 + 2] = 0;
	}
	return result;
}

function expandedSegments(
	positions: Float32Array,
	segments: Uint32Array,
): Float32Array {
	const result = new Float32Array((segments.length / 2) * 6);
	for (let segment = 0; segment < segments.length / 2; segment += 1) {
		for (let endpoint = 0; endpoint < 2; endpoint += 1) {
			const source = segments[segment * 2 + endpoint];
			const target = segment * 6 + endpoint * 3;
			result[target] = positions[source * 3];
			result[target + 1] = positions[source * 3 + 1];
			result[target + 2] = positions[source * 3 + 2];
		}
	}
	return result;
}

function exactCellEdges(cells: Uint32Array, width: number): Uint32Array {
	const result = new Uint32Array((cells.length / width) * width * 2);
	let output = 0;
	for (let cell = 0; cell < cells.length / width; cell += 1) {
		for (let edge = 0; edge < width; edge += 1) {
			result[output++] = cells[cell * width + edge];
			result[output++] = cells[cell * width + ((edge + 1) % width)];
		}
	}
	return result;
}

function triangleIndex(
	cells: Uint32Array,
	width: number,
): {
	readonly indices: Uint32Array;
	readonly triangleToCell: readonly number[];
} {
	if (width === 3) {
		return {
			indices: new Uint32Array(cells),
			triangleToCell: Array.from({ length: cells.length / 3 }, (_, index) => index),
		};
	}
	const count = cells.length / 4;
	const indices = new Uint32Array(count * 6);
	const triangleToCell: number[] = [];
	for (let cell = 0; cell < count; cell += 1) {
		const [a, b, c, d] = cells.subarray(cell * 4, cell * 4 + 4);
		indices.set([a, b, c, a, c, d], cell * 6);
		triangleToCell.push(cell, cell);
	}
	return { indices, triangleToCell };
}

function lineObject(
	positions: Float32Array,
	segments: Uint32Array,
	color: THREE.ColorRepresentation,
	state: Pick<ViewerState, "disposables">,
): THREE.LineSegments {
	const geometry = new THREE.BufferGeometry();
	geometry.setAttribute(
		"position",
		new THREE.BufferAttribute(expandedSegments(positions, segments), 3),
	);
	const material = new THREE.LineBasicMaterial({ color });
	state.disposables.push(geometry, material);
	return new THREE.LineSegments(geometry, material);
}

function addGeometryTarget(
	scene: DecodedScene,
	layer: GeometryLayer,
	state: ViewerState,
): void {
	const coordinates = f64(scene, layer.positions);
	const segments = u32(scene, layer.segments);
	const positions = positions3(coordinates);
	const group = new THREE.Group();
	const edges = lineObject(positions, segments, 0x25334a, state);
	group.add(edges);
	state.scene.add(group);
	state.targets.set(layer.id, {
		layer,
		group,
		coordinates,
		connectivity: segments,
		surface: null,
		edges,
		triangleToCell: [],
	});
}

function addMeshTarget(
	scene: DecodedScene,
	layer: MeshLayer,
	state: ViewerState,
): void {
	const coordinates = f64(scene, layer.coordinates);
	const cells = u32(scene, layer.connectivity);
	const positions = positions3(coordinates);
	const width = layer.cell_kind === "triangle" ? 3 : 4;
	const triangles = triangleIndex(cells, width);
	const geometry = new THREE.BufferGeometry();
	geometry.setAttribute("position", new THREE.BufferAttribute(positions, 3));
	geometry.setIndex(new THREE.BufferAttribute(triangles.indices, 1));
	const material = new THREE.MeshBasicMaterial({
		color: 0x77a5d8,
		side: THREE.DoubleSide,
		transparent: true,
		opacity: 0.82,
	});
	const surface = new THREE.Mesh(geometry, material);
	surface.userData.eqioraTarget = layer.id;
	const edges = lineObject(positions, exactCellEdges(cells, width), 0x23354d, state);
	const group = new THREE.Group();
	group.add(surface, edges);
	state.disposables.push(geometry, material);
	state.scene.add(group);
	state.targets.set(layer.id, {
		layer,
		group,
		coordinates,
		connectivity: cells,
		surface,
		edges,
		triangleToCell: triangles.triangleToCell,
	});
}

function rangeColor(value: number, minimum: number, maximum: number): THREE.Color {
	const unit =
		maximum === minimum
			? 0.5
			: Math.min(1, Math.max(0, (value - minimum) / (maximum - minimum)));
	return new THREE.Color().setHSL((2 / 3) * (1 - unit), 0.78, 0.52);
}

function fieldMesh(
	scene: DecodedScene,
	field: ScalarFieldLayer,
	target: RenderedTarget,
): THREE.Mesh {
	if (target.layer.kind !== "mesh") throw new Error("field target is not a MeshLayer");
	const values = f64(scene, field.values);
	const width = target.layer.cell_kind === "triangle" ? 3 : 4;
	const triangles = triangleIndex(target.connectivity, width).indices;
	const expandedPositions = new Float32Array(triangles.length * 3);
	const colors = new Float32Array(triangles.length * 3);
	for (let corner = 0; corner < triangles.length; corner += 1) {
		const vertex = triangles[corner];
		expandedPositions.set(
			[target.coordinates[vertex * 2], target.coordinates[vertex * 2 + 1], 0],
			corner * 3,
		);
		const cell = Math.floor(corner / 3);
		const acceptedCell = target.triangleToCell[cell];
		const acceptedValue =
			field.association === "vertex" ? values[vertex] : values[acceptedCell];
		const color = rangeColor(acceptedValue, field.scale.minimum, field.scale.maximum);
		colors.set(color.toArray(), corner * 3);
	}
	const geometry = new THREE.BufferGeometry();
	geometry.setAttribute("position", new THREE.BufferAttribute(expandedPositions, 3));
	geometry.setAttribute("color", new THREE.BufferAttribute(colors, 3));
	const material = new THREE.MeshBasicMaterial({
		vertexColors: true,
		side: THREE.DoubleSide,
	});
	const mesh = new THREE.Mesh(geometry, material);
	mesh.userData.eqioraTarget = target.layer.id;
	mesh.userData.eqioraField = field.id;
	return mesh;
}

function disposeObject(object: THREE.Object3D | null): void {
	if (object === null) return;
	object.removeFromParent();
	const mesh = object as THREE.Mesh;
	mesh.geometry?.dispose();
	const materials = Array.isArray(mesh.material)
		? mesh.material
		: mesh.material === undefined
			? []
			: [mesh.material];
	for (const material of materials) material.dispose();
}

function applyField(
	scene: DecodedScene,
	state: ViewerState,
	field: ScalarFieldLayer | null,
): void {
	disposeObject(state.fieldSurface);
	state.fieldSurface = null;
	state.selectedField = field;
	for (const target of state.targets.values()) {
		if (target.surface !== null) target.surface.visible = state.surfacesVisible;
	}
	if (field === null) return;
	const target = state.targets.get(field.target_layer);
	if (target === undefined || target.surface === null)
		throw new Error("field target disappeared");
	target.surface.visible = false;
	state.fieldSurface = fieldMesh(scene, field, target);
	state.fieldSurface.visible = state.surfacesVisible && !state.selectionIsolated;
	state.scene.add(state.fieldSurface);
}

function selectionObject(
	scene: DecodedScene,
	state: ViewerState,
	selection: SelectionLayer,
	color: string,
): THREE.Object3D {
	const target = state.targets.get(selection.target_layer);
	if (target === undefined || selection.entity_indices === null)
		throw new Error("selection target disappeared");
	const positions = positions3(target.coordinates);
	let connectivity: Uint32Array;
	if (selection.connectivity !== null) {
		connectivity = u32(scene, selection.connectivity);
	} else {
		const primitives = u32(scene, selection.entity_indices);
		connectivity = new Uint32Array(primitives.length * 2);
		primitives.forEach((primitive, index) => {
			connectivity[index * 2] = target.connectivity[primitive * 2];
			connectivity[index * 2 + 1] = target.connectivity[primitive * 2 + 1];
		});
	}
	if (selection.dimension === 1)
		return lineObject(positions, connectivity, color, state);
	const width = connectivity.length / u32(scene, selection.entity_indices).length;
	const triangles = triangleIndex(connectivity, width).indices;
	const geometry = new THREE.BufferGeometry();
	geometry.setAttribute("position", new THREE.BufferAttribute(positions, 3));
	geometry.setIndex(new THREE.BufferAttribute(triangles, 1));
	const material = new THREE.MeshBasicMaterial({
		color,
		side: THREE.DoubleSide,
		transparent: true,
		opacity: 0.9,
		depthTest: false,
	});
	const highlight = new THREE.Mesh(geometry, material);
	highlight.renderOrder = 3;
	return highlight;
}

function applySelection(
	scene: DecodedScene,
	state: ViewerState,
	selection: SelectionLayer | null,
	color: string,
	isolate: boolean,
): void {
	disposeObject(state.highlight);
	state.highlight = null;
	state.selectedSelection = selection;
	state.selectionIsolated = Boolean(selection?.available && isolate);
	for (const target of state.targets.values()) target.group.visible = true;
	if (state.fieldSurface !== null) {
		state.fieldSurface.visible = state.surfacesVisible && !state.selectionIsolated;
	}
	if (selection === null || !selection.available) return;
	state.highlight = selectionObject(scene, state, selection, color);
	state.scene.add(state.highlight);
	if (state.selectionIsolated) {
		for (const target of state.targets.values()) target.group.visible = false;
	}
}

function fitCamera(state: ViewerState): {
	readonly position: THREE.Vector3;
	readonly target: THREE.Vector3;
} {
	const box = new THREE.Box3().setFromObject(state.scene);
	if (box.isEmpty()) throw new Error("viewer scene has no visible bounds");
	const center = box.getCenter(new THREE.Vector3());
	const size = box.getSize(new THREE.Vector3());
	const extent = Math.max(size.x, size.y, size.z, 1e-9);
	const position = center
		.clone()
		.add(new THREE.Vector3(extent * 1.35, -extent * 1.35, extent * 1.75));
	state.camera.position.copy(position);
	state.controls.target.copy(center);
	state.controls.update();
	return { position, target: center };
}

function requestDraw(state: ViewerState): void {
	if (state.cleaned || state.frame !== 0) return;
	state.frame = requestAnimationFrame(() => {
		state.frame = 0;
		if (!state.cleaned) state.renderer.render(state.scene, state.camera);
	});
}

function unitLabel(field: ScalarFieldLayer): string {
	const exponents = field.dimension.map(([numerator, denominator]) =>
		denominator === 1 ? `${numerator}` : `${numerator}/${denominator}`,
	);
	return `${field.unit} [${exponents.join(",")}] · ${field.frame}`;
}

function pickAcceptedValue(
	event: PointerEvent,
	canvas: HTMLCanvasElement,
	state: ViewerState,
	scene: DecodedScene,
	output: HTMLOutputElement,
): void {
	const bounds = canvas.getBoundingClientRect();
	const pointer = new THREE.Vector2(
		((event.clientX - bounds.left) / bounds.width) * 2 - 1,
		-((event.clientY - bounds.top) / bounds.height) * 2 + 1,
	);
	const raycaster = new THREE.Raycaster();
	raycaster.setFromCamera(pointer, state.camera);
	const surfaces = [
		state.fieldSurface,
		...Array.from(state.targets.values(), (target) => target.surface),
	].filter((value): value is THREE.Mesh => value?.visible === true);
	const hit = raycaster.intersectObjects(surfaces, false)[0];
	if (hit === undefined || hit.faceIndex === undefined || hit.faceIndex === null) {
		output.value = "No exact accepted primitive selected";
		return;
	}
	const targetId = textData(hit.object.userData.eqioraTarget);
	const target = state.targets.get(targetId);
	if (target?.layer.kind !== "mesh") {
		output.value = "Geometry line picking is unavailable in this projection";
		return;
	}
	const triangle = hit.faceIndex;
	const cell = target.triangleToCell[triangle];
	const field = state.selectedField;
	if (field === null || field.target_layer !== targetId) {
		output.value = `Accepted Mesh cell ${cell}`;
		return;
	}
	const values = f64(scene, field.values);
	if (field.association === "cell") {
		output.value = `Accepted cell ${cell}: ${values[cell]} ${unitLabel(field)}`;
		return;
	}
	const width = target.layer.cell_kind === "triangle" ? 3 : 4;
	const vertices = target.connectivity.subarray(cell * width, cell * width + width);
	let closest = vertices[0];
	let distance = Number.POSITIVE_INFINITY;
	for (const vertex of vertices) {
		const dx = target.coordinates[vertex * 2] - hit.point.x;
		const dy = target.coordinates[vertex * 2 + 1] - hit.point.y;
		const candidate = dx * dx + dy * dy;
		if (candidate < distance) {
			distance = candidate;
			closest = vertex;
		}
	}
	output.value = `Accepted vertex ${closest}: ${values[closest]} ${unitLabel(field)}`;
}

function textData(value: unknown): string {
	return typeof value === "string" ? value : "";
}

function buildShell(
	host: HTMLElement,
	scene: DecodedScene,
): {
	readonly root: HTMLElement;
	readonly toolbar: HTMLElement;
	readonly viewport: HTMLElement;
	readonly inspector: HTMLElement;
} {
	const root = element("section", "eqiora-viewer", host);
	root.setAttribute("aria-label", "Eqiora semantic scene viewer");
	root.dataset.sceneSchema = scene.metadata.schema;
	const heading = element("div", "eqiora-viewer__heading", root);
	const title = element("strong", "eqiora-viewer__title", heading);
	title.textContent = "Eqiora Viewer";
	const summary = element("span", "eqiora-viewer__summary", heading);
	summary.textContent = `${scene.metadata.layers.length} typed layers · presentation only`;
	const toolbar = element("div", "eqiora-viewer__toolbar", root);
	toolbar.setAttribute("role", "toolbar");
	toolbar.setAttribute("aria-label", "Viewer controls");
	const viewport = element("div", "eqiora-viewer__viewport", root);
	const inspector = element("aside", "eqiora-viewer__inspector", root);
	inspector.setAttribute("aria-label", "Accepted value inspector");
	return { root, toolbar, viewport, inspector };
}

function addControls(
	scene: DecodedScene,
	state: ViewerState,
	toolbar: HTMLElement,
	inspector: HTMLElement,
	reset: { readonly position: THREE.Vector3; readonly target: THREE.Vector3 },
): void {
	const camera = element("div", "eqiora-viewer__control-group", toolbar);
	camera.setAttribute("aria-label", "Camera controls");
	const move = (offset: THREE.Vector3) => {
		state.camera.position.add(offset);
		state.controls.update();
		requestDraw(state);
	};
	button("Orbit", camera, () => {
		const offset = state.camera.position.clone().sub(state.controls.target);
		move(new THREE.Vector3(-offset.z - offset.x, 0, offset.x - offset.z));
	});
	button("Pan", camera, () => move(new THREE.Vector3(0.1, 0, 0)));
	button("Zoom in", camera, () => {
		state.camera.position.lerp(state.controls.target, 0.2);
		requestDraw(state);
	});
	button("Zoom out", camera, () => {
		state.camera.position
			.sub(state.controls.target)
			.multiplyScalar(1.25)
			.add(state.controls.target);
		requestDraw(state);
	});
	button("Reset", camera, () => {
		state.camera.position.copy(reset.position);
		state.controls.target.copy(reset.target);
		state.controls.update();
		requestDraw(state);
	});

	const visibility = element("fieldset", "eqiora-viewer__control-group", toolbar);
	const visibilityLegend = element("legend", "eqiora-viewer__legend", visibility);
	visibilityLegend.textContent = "Visibility";
	for (const [label, property] of [
		["Surfaces", "surface"],
		["Edges", "edges"],
	] as const) {
		const wrapper = element("label", "eqiora-viewer__toggle", visibility);
		const input = element("input", "", wrapper);
		input.type = "checkbox";
		input.checked = true;
		wrapper.append(label);
		input.addEventListener("change", () => {
			for (const target of state.targets.values()) {
				if (property === "surface" && target.surface !== null) {
					target.surface.visible = input.checked;
				}
				if (property === "edges") target.edges.visible = input.checked;
			}
			if (property === "surface") {
				state.surfacesVisible = input.checked;
				if (state.fieldSurface !== null) {
					state.fieldSurface.visible = input.checked && !state.selectionIsolated;
				}
			} else {
				state.edgesVisible = input.checked;
			}
			requestDraw(state);
		});
	}

	const selections = scene.metadata.layers.filter(
		(layer): layer is SelectionLayer => layer.kind === "selection",
	);
	const selectionLabel = element("label", "eqiora-viewer__label", toolbar);
	selectionLabel.append("Selection");
	const selectionSelect = element("select", "eqiora-viewer__select", selectionLabel);
	const noSelection = element("option", "", selectionSelect);
	noSelection.value = "";
	noSelection.textContent = "None";
	for (const selection of selections) {
		const option = element("option", "", selectionSelect);
		option.value = selection.id;
		option.textContent = `${selection.name} · ${selection.target_layer.startsWith("mesh:") ? "Mesh" : "Geometry"}`;
		option.disabled = !selection.available;
		if (!selection.available)
			option.title = selection.unavailable_reason ?? "Unavailable";
	}
	const colorLabel = element("label", "eqiora-viewer__label", toolbar);
	colorLabel.append("Selection colour");
	const color = element("input", "eqiora-viewer__color", colorLabel);
	color.type = "color";
	color.value = "#f26b38";
	const isolateLabel = element("label", "eqiora-viewer__toggle", toolbar);
	const isolate = element("input", "", isolateLabel);
	isolate.type = "checkbox";
	isolateLabel.append("Isolate exact selection");
	const updateSelection = () => {
		const selection =
			selections.find((candidate) => candidate.id === selectionSelect.value) ?? null;
		applySelection(scene, state, selection, color.value, isolate.checked);
		requestDraw(state);
	};
	selectionSelect.addEventListener("change", updateSelection);
	color.addEventListener("input", updateSelection);
	isolate.addEventListener("change", updateSelection);

	const fields = scene.metadata.layers.filter(
		(layer): layer is ScalarFieldLayer => layer.kind === "scalar-field",
	);
	const fieldLabel = element("label", "eqiora-viewer__label", toolbar);
	fieldLabel.append("Scalar field");
	const fieldSelect = element("select", "eqiora-viewer__select", fieldLabel);
	const noField = element("option", "", fieldSelect);
	noField.value = "";
	noField.textContent = "None";
	for (const field of fields) {
		const option = element("option", "", fieldSelect);
		option.value = field.id;
		option.textContent = `${field.field_id} · ${field.association}`;
	}
	const legend = element("output", "eqiora-viewer__field-legend", inspector);
	legend.setAttribute("aria-live", "polite");
	fieldSelect.addEventListener("change", () => {
		const field =
			fields.find((candidate) => candidate.id === fieldSelect.value) ?? null;
		applyField(scene, state, field);
		legend.value =
			field === null
				? "No scalar field selected"
				: `${field.field_id}: ${field.scale.minimum} to ${field.scale.maximum} ${unitLabel(field)}; ${field.scale.provenance}`;
		requestDraw(state);
	});
	legend.value = "No scalar field selected";
}

function createState(viewport: HTMLElement): ViewerState {
	const renderer = new THREE.WebGLRenderer({ antialias: true, alpha: false });
	renderer.setClearColor(0xf4f7fb, 1);
	renderer.setPixelRatio(Math.min(globalThis.devicePixelRatio || 1, 2));
	const canvas = renderer.domElement;
	canvas.className = "eqiora-viewer__canvas";
	canvas.tabIndex = 0;
	canvas.setAttribute("role", "img");
	canvas.setAttribute(
		"aria-label",
		"Interactive read-only Eqiora scene. Picking reports only exact accepted entity values.",
	);
	viewport.append(canvas);
	const camera = new THREE.PerspectiveCamera(42, 1, 1e-6, 1e9);
	camera.up.set(0, 1, 0);
	const controls = new OrbitControls(camera, canvas);
	controls.enablePan = true;
	controls.enableRotate = true;
	controls.enableZoom = true;
	return {
		scene: new THREE.Scene(),
		renderer,
		camera,
		controls,
		targets: new Map(),
		disposables: [],
		highlight: null,
		fieldSurface: null,
		selectedField: null,
		selectedSelection: null,
		selectionIsolated: false,
		surfacesVisible: true,
		edgesVisible: true,
		frame: 0,
		cleaned: false,
	};
}

export function mountEqioraViewer(
	host: HTMLElement,
	metadataJson: string,
	buffers: readonly BinaryInput[],
): ViewerMount {
	const scene = decodeScene(metadataJson, buffers);
	host.replaceChildren();
	const shell = buildShell(host, scene);
	const state = createState(shell.viewport);
	for (const layer of scene.metadata.layers) {
		if (layer.kind === "geometry") addGeometryTarget(scene, layer, state);
		if (layer.kind === "mesh") addMeshTarget(scene, layer, state);
	}
	const reset = fitCamera(state);
	addControls(scene, state, shell.toolbar, shell.inspector, reset);
	const picked = element("output", "eqiora-viewer__picked", shell.inspector);
	picked.setAttribute("aria-live", "polite");
	picked.value = "Pick a visible primitive to inspect an exact accepted value";
	const canvas = state.renderer.domElement;
	const pick = (event: PointerEvent) => {
		pickAcceptedValue(event, canvas, state, scene, picked);
	};
	canvas.addEventListener("pointerdown", pick);
	const changed = () => requestDraw(state);
	state.controls.addEventListener("change", changed);
	const resize = new ResizeObserver(() => {
		const bounds = shell.viewport.getBoundingClientRect();
		const width = Math.max(1, Math.floor(bounds.width));
		const height = Math.max(1, Math.floor(bounds.height));
		state.renderer.setSize(width, height, false);
		state.camera.aspect = width / height;
		state.camera.updateProjectionMatrix();
		requestDraw(state);
	});
	resize.observe(shell.viewport);
	requestDraw(state);

	const cleanup = () => {
		if (state.cleaned) return;
		state.cleaned = true;
		resize.disconnect();
		canvas.removeEventListener("pointerdown", pick);
		state.controls.removeEventListener("change", changed);
		state.controls.dispose();
		if (state.frame !== 0) cancelAnimationFrame(state.frame);
		disposeObject(state.highlight);
		disposeObject(state.fieldSurface);
		for (const disposable of state.disposables) disposable.dispose();
		state.scene.clear();
		state.renderer.dispose();
		state.renderer.forceContextLoss();
		shell.root.remove();
	};
	return { cleanup };
}

export function showViewerFailure(host: HTMLElement, error: unknown): void {
	const diagnostic = document.createElement("p");
	diagnostic.className = "eqiora-viewer__diagnostic";
	diagnostic.setAttribute("role", "alert");
	diagnostic.textContent = `Eqiora Viewer unavailable: ${error instanceof Error ? error.message : String(error)}`;
	host.replaceChildren(diagnostic);
}
