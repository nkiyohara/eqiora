import * as THREE from "three";
import { OrbitControls } from "three/addons/controls/OrbitControls.js";

import {
	type DecodedMesh,
	decodeMeshContract,
	MESH_DIGEST,
	type MeshModel,
	type RepresentationMode,
	validateRepresentationMode,
} from "./mesh-contract";

interface RenderContext {
	model: MeshModel;
	el: HTMLElement;
}

const FAILURE_MESSAGE =
	"Eqiora could not create the WebGL Mesh view. The exact text representation remains available.";

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
		positions[vertex * 3] =
			(2 * (mesh.coordinates[vertex * 2] - centerX)) / extent;
		positions[vertex * 3 + 1] =
			(2 * (mesh.coordinates[vertex * 2 + 1] - centerY)) / extent;
		positions[vertex * 3 + 2] = 0;
	}
	return positions;
}

function renderMesh(mesh: DecodedMesh, context: RenderContext): () => void {
	const listeners: Array<() => void> = [];
	const root = document.createElement("section");
	root.className = "eqiora-mesh-view";
	root.setAttribute("aria-label", `Mesh ${mesh.digest}`);

	const toolbar = document.createElement("div");
	toolbar.className = "eqiora-mesh-toolbar";
	toolbar.setAttribute("role", "toolbar");
	toolbar.setAttribute("aria-label", "Mesh view controls");
	root.append(toolbar);

	const viewport = document.createElement("div");
	viewport.className = "eqiora-mesh-viewport";
	root.append(viewport);
	context.el.replaceChildren(root);

	let renderer: THREE.WebGLRenderer;
	try {
		renderer = new THREE.WebGLRenderer({ antialias: true, alpha: false });
	} catch (error) {
		root.remove();
		throw error;
	}
	renderer.setClearColor(0xf7f8fa, 1);
	renderer.setPixelRatio(Math.min(globalThis.devicePixelRatio || 1, 2));
	const canvas = renderer.domElement;
	canvas.tabIndex = 0;
	canvas.className = "eqiora-mesh-canvas";
	canvas.setAttribute(
		"aria-label",
		`Mesh interactive view, digest ${MESH_DIGEST}`,
	);
	viewport.append(canvas);

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
	const initialPosition = camera.position.clone();
	const initialTarget = controls.target.clone();
	const initialUp = camera.up.clone();

	let cleaned = false;
	let contextFailed = false;
	let frame = 0;
	const draw = () => {
		frame = 0;
		if (!cleaned && !contextFailed) {
			renderer.render(scene, camera);
		}
	};
	const requestDraw = () => {
		if (!cleaned && !contextFailed && frame === 0) {
			frame = requestAnimationFrame(draw);
		}
	};
	controls.addEventListener("change", requestDraw);
	listeners.push(() => controls.removeEventListener("change", requestDraw));

	const applyCamera = (position: THREE.Vector3, nextTarget: THREE.Vector3) => {
		camera.position.copy(position);
		controls.target.copy(nextTarget);
		camera.lookAt(nextTarget);
		controls.update();
		requestDraw();
	};
	const orbit = (direction: number) => {
		const offset = camera.position.clone().sub(controls.target);
		offset.applyAxisAngle(camera.up, direction * Math.PI * 0.08);
		applyCamera(controls.target.clone().add(offset), controls.target.clone());
	};
	const pan = (direction: number) => {
		const view = controls.target.clone().sub(camera.position).normalize();
		const displacement = view
			.clone()
			.cross(camera.up)
			.normalize()
			.multiplyScalar(
				direction * camera.position.distanceTo(controls.target) * 0.08,
			);
		applyCamera(
			camera.position.clone().add(displacement),
			controls.target.clone().add(displacement),
		);
	};
	const zoom = (factor: number) => {
		const offset = camera.position
			.clone()
			.sub(controls.target)
			.multiplyScalar(factor);
		applyCamera(controls.target.clone().add(offset), controls.target.clone());
	};
	const reset = () => {
		camera.up.copy(initialUp);
		applyCamera(initialPosition.clone(), initialTarget.clone());
	};
	const top = () => {
		const distance = camera.position.distanceTo(controls.target);
		camera.up.copy(initialUp);
		applyCamera(
			controls.target.clone().add(new THREE.Vector3(0, 0, distance)),
			controls.target.clone(),
		);
	};
	const isometric = () => {
		const distance = camera.position.distanceTo(controls.target);
		const component = distance / Math.sqrt(3);
		camera.up.copy(initialUp);
		applyCamera(
			controls.target
				.clone()
				.add(new THREE.Vector3(component, -component, component)),
			controls.target.clone(),
		);
	};

	button("Orbit left", toolbar, listeners, () => orbit(-1));
	button("Pan right", toolbar, listeners, () => pan(1));
	button("Zoom in", toolbar, listeners, () => zoom(0.8));
	button("Zoom out", toolbar, listeners, () => zoom(1.25));
	button("Reset view", toolbar, listeners, reset);
	const topButton = button("Top view", toolbar, listeners, top);
	const isometricButton = button(
		"Isometric view",
		toolbar,
		listeners,
		isometric,
	);

	const modeButtons = new Map<RepresentationMode, HTMLButtonElement>();
	const setMode = (candidate: unknown) => {
		const mode = validateRepresentationMode(candidate);
		surface.visible = mode === "surface";
		wireframe.visible = mode === "wireframe";
		points.visible = mode === "points";
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

	const updateNamedView = (name: "top" | "isometric" | null) => {
		topButton.setAttribute("aria-pressed", String(name === "top"));
		isometricButton.setAttribute("aria-pressed", String(name === "isometric"));
	};
	topButton.setAttribute("aria-pressed", "false");
	isometricButton.setAttribute("aria-pressed", "true");
	const topClick = () => updateNamedView("top");
	const isometricClick = () => updateNamedView("isometric");
	topButton.addEventListener("click", topClick);
	isometricButton.addEventListener("click", isometricClick);
	listeners.push(() => topButton.removeEventListener("click", topClick));
	listeners.push(() =>
		isometricButton.removeEventListener("click", isometricClick),
	);

	const keydown = (event: KeyboardEvent) => {
		let handled = true;
		if (event.key === "ArrowLeft" && event.shiftKey) {
			pan(-1);
		} else if (event.key === "ArrowRight" && event.shiftKey) {
			pan(1);
		} else if (event.key === "ArrowLeft") {
			orbit(-1);
		} else if (event.key === "ArrowRight") {
			orbit(1);
		} else if (event.key === "+" || event.key === "=") {
			zoom(0.8);
		} else if (event.key === "-") {
			zoom(1.25);
		} else if (event.key.toLowerCase() === "r") {
			reset();
			updateNamedView("isometric");
		} else if (event.key.toLowerCase() === "t") {
			top();
			updateNamedView("top");
		} else if (event.key.toLowerCase() === "i") {
			isometric();
			updateNamedView("isometric");
		} else {
			handled = false;
		}
		if (handled) {
			event.preventDefault();
		}
	};
	canvas.addEventListener("keydown", keydown);
	listeners.push(() => canvas.removeEventListener("keydown", keydown));

	const resize = new ResizeObserver((entries) => {
		const entry = entries.find((candidate) => candidate.target === viewport);
		if (entry === undefined || cleaned) {
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

	const contextLost = (event: Event) => {
		event.preventDefault();
		contextFailed = true;
		const diagnostic = document.createElement("p");
		diagnostic.className = "eqiora-mesh-diagnostic";
		diagnostic.setAttribute("role", "alert");
		diagnostic.textContent = FAILURE_MESSAGE;
		viewport.replaceChildren(diagnostic);
	};
	canvas.addEventListener("webglcontextlost", contextLost);
	listeners.push(() =>
		canvas.removeEventListener("webglcontextlost", contextLost),
	);

	const cleanup = () => {
		if (cleaned) {
			return;
		}
		cleaned = true;
		resize.disconnect();
		if (frame !== 0) {
			cancelAnimationFrame(frame);
			frame = 0;
		}
		for (const remove of listeners.splice(0)) {
			remove();
		}
		controls.dispose();
		geometry.deleteAttribute("position");
		geometry.setIndex(null);
		geometry.dispose();
		surfaceMaterial.dispose();
		wireframeMaterial.dispose();
		pointsMaterial.dispose();
		scene.clear();
		renderer.dispose();
		if (!contextFailed) {
			renderer.forceContextLoss();
		}
		root.remove();
	};
	context.model.on?.("destroy", cleanup);
	context.model.on?.("comm:close", cleanup);
	listeners.push(() => context.model.off?.("destroy", cleanup));
	listeners.push(() => context.model.off?.("comm:close", cleanup));

	const bounds = viewport.getBoundingClientRect();
	renderer.setSize(
		Math.max(1, bounds.width),
		Math.max(1, bounds.height),
		false,
	);
	camera.aspect = Math.max(1, bounds.width) / Math.max(1, bounds.height);
	camera.updateProjectionMatrix();
	requestDraw();
	return cleanup;
}

function showFailure(el: HTMLElement): void {
	const diagnostic = document.createElement("p");
	diagnostic.className = "eqiora-mesh-diagnostic";
	diagnostic.setAttribute("role", "alert");
	diagnostic.textContent = FAILURE_MESSAGE;
	el.replaceChildren(diagnostic);
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
