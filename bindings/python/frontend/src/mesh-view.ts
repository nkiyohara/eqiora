import * as THREE from "three";
import { OrbitControls } from "three/addons/controls/OrbitControls.js";

import {
	type DecodedMesh,
	decodeMeshContract,
	type MeshModel,
	type RepresentationMode,
	validateRepresentationMode,
} from "./mesh-contract";

type Vector3 = [number, number, number];
type CameraPreset = "initial" | "orbit" | "pan" | "zoom" | "top" | "isometric";

interface RenderModel extends MeshModel {
	send(
		content: unknown,
		callbacks?: unknown,
		buffers?: ArrayBuffer[] | ArrayBufferView[],
	): void;
}

interface RenderContext {
	model: RenderModel;
	el: HTMLElement;
}

interface TransportObservation {
	modelSetCalls: number;
	saveChangesCalls: number;
	sendCalls: number;
	customMessageHandlers: number;
}

interface LifecycleObservation {
	cleanupCount: number;
	controlsDisposed: boolean;
	geometryDisposed: boolean;
	materialsDisposed: boolean;
	rendererDisposed: boolean;
}

interface ViewState {
	mode: RepresentationMode;
	preset: CameraPreset;
	cleaned: boolean;
	contextFailed: boolean;
	frame: number;
	readonly transport: TransportObservation;
	readonly lifecycle: LifecycleObservation;
}

interface CameraState {
	readonly camera: THREE.PerspectiveCamera;
	readonly controls: OrbitControls;
	readonly initialPosition: THREE.Vector3;
	readonly initialTarget: THREE.Vector3;
	readonly initialUp: THREE.Vector3;
}

interface SceneState {
	readonly geometry: THREE.BufferGeometry;
	readonly materials: readonly [
		THREE.MeshBasicMaterial,
		THREE.MeshBasicMaterial,
		THREE.PointsMaterial,
	];
	readonly surface: THREE.Mesh;
	readonly wireframe: THREE.Mesh;
	readonly points: THREE.Points;
	readonly scene: THREE.Scene;
}

interface CameraActions {
	orbit(): void;
	pan(): void;
	zoom(): void;
	zoomOut(): void;
	reset(): void;
	top(): void;
	isometric(): void;
}

interface ViewSnapshot {
	modelId: string;
	viewId: string;
	mode: RepresentationMode;
	transport: TransportObservation;
	camera: {
		position: Vector3;
		target: Vector3;
		initialPosition: Vector3;
		initialTarget: Vector3;
		preset: CameraPreset;
	};
	lifecycle: LifecycleObservation & {
		activeListeners: number;
		pendingAnimationFrames: number;
	};
}

interface ViewOracle {
	snapshot(): ViewSnapshot;
	closeComm(): void;
}

interface OracleElement extends HTMLElement {
	__eqioraN1Oracle?: ViewOracle;
}

const FAILURE_MESSAGE =
	"Eqiora could not create the WebGL Mesh view. The exact text representation remains available.";
const ORACLE_CLOSE_MESSAGE = "eqiora:n1-oracle-close";
let nextOracleViewId = 0;
const oracleCloseRequests = new Set<string>();

function button(
	label: string,
	parent: HTMLElement,
	listeners: Array<() => void>,
	action: () => void,
): HTMLButtonElement {
	const control = document.createElement("button");
	control.type = "button";
	control.className = "eqiora-mesh-button";
	control.textContent = label;
	control.setAttribute("aria-label", label);
	control.addEventListener("click", action);
	listeners.push(() => control.removeEventListener("click", action));
	parent.append(control);
	return control;
}

function normalizedPositions(mesh: DecodedMesh): Float32Array {
	let minimumX = Number.POSITIVE_INFINITY;
	let minimumY = Number.POSITIVE_INFINITY;
	let maximumX = Number.NEGATIVE_INFINITY;
	let maximumY = Number.NEGATIVE_INFINITY;
	for (let index = 0; index < mesh.coordinates.length; index += 2) {
		minimumX = Math.min(minimumX, mesh.coordinates[index]);
		maximumX = Math.max(maximumX, mesh.coordinates[index]);
		minimumY = Math.min(minimumY, mesh.coordinates[index + 1]);
		maximumY = Math.max(maximumY, mesh.coordinates[index + 1]);
	}
	const extent = Math.max(maximumX - minimumX, maximumY - minimumY);
	if (!Number.isFinite(extent) || extent <= 0) {
		throw new Error("the accepted Mesh has no finite presentation extent");
	}
	const centerX = (minimumX + maximumX) / 2;
	const centerY = (minimumY + maximumY) / 2;
	const positions = new Float32Array((mesh.coordinates.length / 2) * 3);
	for (let vertex = 0; vertex < mesh.coordinates.length / 2; vertex += 1) {
		positions[vertex * 3] = (2 * (mesh.coordinates[vertex * 2] - centerX)) / extent;
		positions[vertex * 3 + 1] =
			(2 * (mesh.coordinates[vertex * 2 + 1] - centerY)) / extent;
		positions[vertex * 3 + 2] = 0;
	}
	return positions;
}

function createRoot(
	mesh: DecodedMesh,
	context: RenderContext,
): {
	root: OracleElement;
	toolbar: HTMLDivElement;
	viewport: HTMLDivElement;
} {
	const root: OracleElement = document.createElement("section");
	root.className = "eqiora-mesh-view";
	root.setAttribute("aria-label", `Mesh ${mesh.digest}`);
	root.setAttribute("data-eqiora-mesh-digest", mesh.digest);

	const toolbar = document.createElement("div");
	toolbar.className = "eqiora-mesh-toolbar";
	toolbar.setAttribute("role", "toolbar");
	toolbar.setAttribute("aria-label", "Mesh view controls");
	root.append(toolbar);

	const viewport = document.createElement("div");
	viewport.className = "eqiora-mesh-viewport";
	root.append(viewport);
	context.el.replaceChildren(root);
	return { root, toolbar, viewport };
}

function createScene(mesh: DecodedMesh): SceneState {
	const geometry = new THREE.BufferGeometry();
	geometry.setAttribute(
		"position",
		new THREE.BufferAttribute(normalizedPositions(mesh), 3),
	);
	geometry.setIndex(new THREE.BufferAttribute(mesh.triangles, 1));
	geometry.computeBoundingSphere();

	const surfaceMaterial = new THREE.MeshBasicMaterial({
		color: 0x4f7fbb,
		side: THREE.DoubleSide,
		transparent: true,
		opacity: 0.9,
	});
	const wireframeMaterial = new THREE.MeshBasicMaterial({
		color: 0x16324f,
		side: THREE.DoubleSide,
		wireframe: true,
	});
	const pointsMaterial = new THREE.PointsMaterial({
		color: 0xa43d31,
		size: 0.035,
		sizeAttenuation: true,
	});
	const surface = new THREE.Mesh(geometry, surfaceMaterial);
	const wireframe = new THREE.Mesh(geometry, wireframeMaterial);
	const points = new THREE.Points(geometry, pointsMaterial);
	const scene = new THREE.Scene();
	scene.add(surface, wireframe, points);
	return {
		geometry,
		materials: [surfaceMaterial, wireframeMaterial, pointsMaterial],
		surface,
		wireframe,
		points,
		scene,
	};
}

function createCamera(canvas: HTMLCanvasElement): CameraState {
	const camera = new THREE.PerspectiveCamera(40, 1, 0.01, 100);
	camera.up.set(0, 1, 0);
	const target = new THREE.Vector3(0, 0, 0);
	camera.position.set(2.6, -2.6, 2.6);
	camera.lookAt(target);
	const controls = new OrbitControls(camera, canvas);
	controls.target.copy(target);
	controls.enableDamping = false;
	controls.enablePan = true;
	controls.enableRotate = true;
	controls.enableZoom = true;
	controls.screenSpacePanning = true;
	controls.update();
	controls.saveState();
	return {
		camera,
		controls,
		initialPosition: camera.position.clone(),
		initialTarget: controls.target.clone(),
		initialUp: camera.up.clone(),
	};
}

function createDrawScheduler(
	renderer: THREE.WebGLRenderer,
	scene: THREE.Scene,
	camera: THREE.PerspectiveCamera,
	state: ViewState,
): () => void {
	const draw = () => {
		state.frame = 0;
		if (!state.cleaned && !state.contextFailed) {
			renderer.render(scene, camera);
		}
	};
	return () => {
		if (!state.cleaned && !state.contextFailed && state.frame === 0) {
			state.frame = requestAnimationFrame(draw);
		}
	};
}

function createCameraActions(
	cameraState: CameraState,
	state: ViewState,
	requestDraw: () => void,
): CameraActions {
	const { camera, controls, initialPosition, initialTarget, initialUp } = cameraState;
	const apply = (
		position: THREE.Vector3,
		target: THREE.Vector3,
		preset: CameraPreset,
	) => {
		camera.position.copy(position);
		controls.target.copy(target);
		camera.lookAt(target);
		controls.update();
		state.preset = preset;
		requestDraw();
	};
	const scaleFromTarget = (factor: number, preset: CameraPreset) => {
		const offset = camera.position.clone().sub(controls.target).multiplyScalar(factor);
		apply(controls.target.clone().add(offset), controls.target.clone(), preset);
	};
	return {
		orbit() {
			const offset = camera.position.clone().sub(controls.target);
			const rotated = new THREE.Vector3(-offset.z, offset.y, offset.x);
			apply(controls.target.clone().add(rotated), controls.target.clone(), "orbit");
		},
		pan() {
			const displacement = new THREE.Vector3(
				camera.position.x - controls.target.x,
				0,
				0,
			);
			apply(
				camera.position.clone().add(displacement),
				controls.target.clone().add(displacement),
				"pan",
			);
		},
		zoom: () => scaleFromTarget(0.5, "zoom"),
		zoomOut: () => scaleFromTarget(1.25, "zoom"),
		reset: () => {
			camera.up.copy(initialUp);
			apply(initialPosition.clone(), initialTarget.clone(), "initial");
		},
		top: () => {
			const distance = camera.position.distanceTo(controls.target);
			camera.up.copy(initialUp);
			apply(
				controls.target.clone().add(new THREE.Vector3(0, 0, distance)),
				controls.target.clone(),
				"top",
			);
		},
		isometric: () => {
			const distance = camera.position.distanceTo(controls.target);
			const component = distance / Math.sqrt(3);
			camera.up.copy(initialUp);
			apply(
				controls.target
					.clone()
					.add(new THREE.Vector3(component, -component, component)),
				controls.target.clone(),
				"isometric",
			);
		},
	};
}

function addToolbar(
	toolbar: HTMLElement,
	scene: SceneState,
	state: ViewState,
	actions: CameraActions,
	requestDraw: () => void,
	listeners: Array<() => void>,
): void {
	button("Orbit camera", toolbar, listeners, actions.orbit);
	button("Pan camera", toolbar, listeners, actions.pan);
	button("Zoom camera", toolbar, listeners, actions.zoom);
	button("Zoom out", toolbar, listeners, actions.zoomOut);
	button("Reset camera", toolbar, listeners, actions.reset);
	button("Top view", toolbar, listeners, actions.top);
	button("Isometric view", toolbar, listeners, actions.isometric);

	const modeButtons = new Map<RepresentationMode, HTMLButtonElement>();
	const setMode = (candidate: unknown) => {
		const mode = validateRepresentationMode(candidate);
		state.mode = mode;
		scene.surface.visible = mode === "surface";
		scene.wireframe.visible = mode === "wireframe";
		scene.points.visible = mode === "points";
		for (const [name, control] of modeButtons) {
			control.setAttribute("aria-pressed", String(name === mode));
			control.dataset.current = String(name === mode);
		}
		requestDraw();
	};
	for (const mode of ["surface", "wireframe", "points"] as const) {
		const control = button(
			`${mode[0].toUpperCase()}${mode.slice(1)}`,
			toolbar,
			listeners,
			() => setMode(mode),
		);
		control.setAttribute("aria-pressed", "false");
		modeButtons.set(mode, control);
	}
	setMode("surface");
}

function addKeyboardControls(
	canvas: HTMLCanvasElement,
	actions: CameraActions,
	listeners: Array<() => void>,
): void {
	const keydown = (event: KeyboardEvent) => {
		let handled = true;
		if (event.key === "ArrowLeft") {
			actions.orbit();
		} else if (event.key === "ArrowRight" && event.shiftKey) {
			actions.pan();
		} else if (event.key === "+" || event.key === "=") {
			actions.zoom();
		} else if (event.key === "-") {
			actions.zoomOut();
		} else if (event.key.toLowerCase() === "r") {
			actions.reset();
		} else if (event.key.toLowerCase() === "t") {
			actions.top();
		} else if (event.key.toLowerCase() === "i") {
			actions.isometric();
		} else {
			handled = false;
		}
		if (handled) {
			event.preventDefault();
		}
	};
	canvas.addEventListener("keydown", keydown);
	listeners.push(() => canvas.removeEventListener("keydown", keydown));
}

function observeViewport(
	viewport: HTMLElement,
	renderer: THREE.WebGLRenderer,
	camera: THREE.PerspectiveCamera,
	state: ViewState,
	requestDraw: () => void,
): ResizeObserver {
	const resize = new ResizeObserver((entries) => {
		const entry = entries.find((candidate) => candidate.target === viewport);
		if (entry === undefined || state.cleaned) {
			return;
		}
		const width = Math.max(1, Math.floor(entry.contentRect.width));
		const height = Math.max(1, Math.floor(entry.contentRect.height));
		renderer.setSize(width, height, false);
		camera.aspect = width / height;
		camera.updateProjectionMatrix();
		requestDraw();
	});
	resize.observe(viewport);
	return resize;
}

function showFailure(el: HTMLElement, digest?: string): void {
	const diagnostic = document.createElement("p");
	diagnostic.className = "eqiora-mesh-diagnostic";
	diagnostic.setAttribute("role", "alert");
	diagnostic.textContent =
		digest === undefined
			? FAILURE_MESSAGE
			: `${FAILURE_MESSAGE} Mesh digest ${digest}.`;
	el.replaceChildren(diagnostic);
}

function attachOracle(
	root: OracleElement,
	context: RenderContext,
	state: ViewState,
	cameraState: CameraState,
	listeners: Array<() => void>,
): void {
	const modelId = context.model.get("_eqiora_n1_model_id");
	if (typeof modelId !== "string" || modelId.length === 0) {
		throw new Error("Eqiora Mesh delegate omitted its private model identity");
	}
	const viewId = `${modelId}:${++nextOracleViewId}`;
	root.__eqioraN1Oracle = {
		snapshot: () => ({
			modelId,
			viewId,
			mode: state.mode,
			transport: { ...state.transport },
			camera: {
				position: cameraState.camera.position.toArray() as Vector3,
				target: cameraState.controls.target.toArray() as Vector3,
				initialPosition: cameraState.initialPosition.toArray() as Vector3,
				initialTarget: cameraState.initialTarget.toArray() as Vector3,
				preset: state.preset,
			},
			lifecycle: {
				...state.lifecycle,
				activeListeners: listeners.length,
				pendingAnimationFrames: state.frame === 0 ? 0 : 1,
			},
		}),
		closeComm: () => {
			if (!oracleCloseRequests.has(modelId)) {
				oracleCloseRequests.add(modelId);
				state.transport.sendCalls += 1;
				context.model.send({ kind: ORACLE_CLOSE_MESSAGE });
			}
		},
	};
}

function makeCleanup(
	root: HTMLElement,
	resize: ResizeObserver,
	cameraState: CameraState,
	scene: SceneState,
	renderer: THREE.WebGLRenderer,
	state: ViewState,
	listeners: Array<() => void>,
): () => void {
	return () => {
		if (state.cleaned) {
			return;
		}
		state.cleaned = true;
		state.lifecycle.cleanupCount += 1;
		resize.disconnect();
		if (state.frame !== 0) {
			cancelAnimationFrame(state.frame);
			state.frame = 0;
		}
		for (const remove of listeners.splice(0)) {
			remove();
		}
		cameraState.controls.dispose();
		state.lifecycle.controlsDisposed = true;
		scene.geometry.deleteAttribute("position");
		scene.geometry.setIndex(null);
		scene.geometry.dispose();
		state.lifecycle.geometryDisposed = true;
		for (const material of scene.materials) {
			material.dispose();
		}
		state.lifecycle.materialsDisposed = true;
		scene.scene.clear();
		renderer.dispose();
		state.lifecycle.rendererDisposed = true;
		if (!state.contextFailed) {
			renderer.forceContextLoss();
		}
		root.remove();
	};
}

function renderMesh(mesh: DecodedMesh, context: RenderContext): () => void {
	const listeners: Array<() => void> = [];
	const { root, toolbar, viewport } = createRoot(mesh, context);
	let renderer: THREE.WebGLRenderer;
	try {
		renderer = new THREE.WebGLRenderer({ antialias: true, alpha: false });
	} catch {
		showFailure(viewport, mesh.digest);
		return () => root.remove();
	}
	renderer.setClearColor(0xf7f8fa, 1);
	renderer.setPixelRatio(Math.min(globalThis.devicePixelRatio || 1, 2));
	const canvas = renderer.domElement;
	canvas.tabIndex = 0;
	canvas.className = "eqiora-mesh-canvas";
	canvas.setAttribute("role", "img");
	canvas.setAttribute("aria-label", `Mesh interactive view, digest ${mesh.digest}`);
	viewport.append(canvas);

	const scene = createScene(mesh);
	const cameraState = createCamera(canvas);
	const state: ViewState = {
		mode: "surface",
		preset: "initial",
		cleaned: false,
		contextFailed: false,
		frame: 0,
		transport: {
			modelSetCalls: 0,
			saveChangesCalls: 0,
			sendCalls: 0,
			customMessageHandlers: 0,
		},
		lifecycle: {
			cleanupCount: 0,
			controlsDisposed: false,
			geometryDisposed: false,
			materialsDisposed: false,
			rendererDisposed: false,
		},
	};
	const requestDraw = createDrawScheduler(
		renderer,
		scene.scene,
		cameraState.camera,
		state,
	);
	cameraState.controls.addEventListener("change", requestDraw);
	listeners.push(() => cameraState.controls.removeEventListener("change", requestDraw));
	const actions = createCameraActions(cameraState, state, requestDraw);
	addToolbar(toolbar, scene, state, actions, requestDraw, listeners);
	addKeyboardControls(canvas, actions, listeners);
	const resize = observeViewport(
		viewport,
		renderer,
		cameraState.camera,
		state,
		requestDraw,
	);
	const contextLost = (event: Event) => {
		event.preventDefault();
		state.contextFailed = true;
		showFailure(viewport, mesh.digest);
	};
	canvas.addEventListener("webglcontextlost", contextLost);
	listeners.push(() => canvas.removeEventListener("webglcontextlost", contextLost));

	const cleanup = makeCleanup(
		root,
		resize,
		cameraState,
		scene,
		renderer,
		state,
		listeners,
	);
	context.model.on?.("destroy", cleanup);
	context.model.on?.("comm:close", cleanup);
	listeners.push(() => context.model.off?.("destroy", cleanup));
	listeners.push(() => context.model.off?.("comm:close", cleanup));
	attachOracle(root, context, state, cameraState, listeners);

	const bounds = viewport.getBoundingClientRect();
	renderer.setSize(Math.max(1, bounds.width), Math.max(1, bounds.height), false);
	cameraState.camera.aspect = Math.max(1, bounds.width) / Math.max(1, bounds.height);
	cameraState.camera.updateProjectionMatrix();
	requestDraw();
	return cleanup;
}

function render(context: RenderContext): () => void {
	let cleanup: (() => void) | undefined;
	let cancelled = false;
	void decodeMeshContract(context.model)
		.then((mesh) => {
			if (!cancelled) {
				cleanup = renderMesh(mesh, context);
			}
		})
		.catch(() => {
			if (!cancelled) {
				showFailure(context.el);
			}
		});
	return () => {
		cancelled = true;
		cleanup?.();
		cleanup = undefined;
	};
}

export default { render };
