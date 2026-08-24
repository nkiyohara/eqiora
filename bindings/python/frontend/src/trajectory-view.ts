import {
	decodeTrajectoryContract,
	type DecodedTrajectory,
	type TrajectoryModel,
} from "./trajectory-contract";

interface Context {
	model: TrajectoryModel;
	el: HTMLElement;
}
interface OracleElement extends HTMLElement {
	__eqioraN3Oracle?: { snapshot(): object };
}

function control<K extends keyof HTMLElementTagNameMap>(
	tag: K,
	parent: HTMLElement,
): HTMLElementTagNameMap[K] {
	const element = document.createElement(tag);
	parent.append(element);
	return element;
}

function color(value: number, minimum: number, maximum: number): string {
	const t =
		maximum === minimum
			? 0.5
			: Math.max(0, Math.min(1, (value - minimum) / (maximum - minimum)));
	const stops = [
		[38, 63, 143],
		[57, 168, 189],
		[243, 211, 91],
		[181, 40, 53],
	];
	const scaled = t * (stops.length - 1),
		index = Math.min(stops.length - 2, Math.floor(scaled)),
		f = scaled - index;
	return `rgb(${stops[index].map((entry, channel) => Math.round(entry + (stops[index + 1][channel] - entry) * f)).join(",")})`;
}

export function render({ model, el }: Context): () => void {
	let trajectory: DecodedTrajectory;
	try {
		trajectory = decodeTrajectoryContract(model);
	} catch {
		el.className = "eqiora-trajectory-error";
		el.textContent =
			"Eqiora could not validate this Trajectory view. The exact text representation remains available.";
		return () => {
			el.replaceChildren();
		};
	}
	const root = control("div", el) as OracleElement;
	root.className = "eqiora-trajectory";
	root.dataset.eqioraTrajectoryDigest = trajectory.trajectoryDigest;
	const canvas = control("canvas", root);
	canvas.width = 960;
	canvas.height = 480;
	const meta = control("div", root);
	meta.className = "eqiora-trajectory-meta";
	const controls = control("div", root);
	controls.className = "eqiora-trajectory-controls";
	const previous = control("button", controls);
	previous.type = "button";
	previous.textContent = "Previous";
	const play = control("button", controls);
	play.type = "button";
	play.textContent = "Play";
	const next = control("button", controls);
	next.type = "button";
	next.textContent = "Next";
	const slider = control("input", controls);
	slider.type = "range";
	slider.min = "0";
	slider.max = String(trajectory.steps.length - 1);
	slider.step = "1";
	slider.value = "0";
	slider.setAttribute("aria-label", "Trajectory state");
	const speed = control("select", controls);
	speed.setAttribute("aria-label", "Playback speed");
	for (const value of [0.5, 1, 2]) {
		const option = control("option", speed);
		option.value = String(value);
		option.textContent = `${value}×`;
		if (value === 1) option.selected = true;
	}
	const swatch = control("span", controls);
	swatch.className = "eqiora-trajectory-swatch";
	let stateIndex = 0,
		timer: number | undefined,
		disposed = false;
	const supportMap = new Map<number, number>();
	trajectory.support.forEach((vertex, index) => {
		supportMap.set(vertex, index);
	});
	const drawable: Array<[number, number, number]> = [];
	for (let i = 0; i < trajectory.triangles.length; i += 3) {
		const tri: [number, number, number] = [
			trajectory.triangles[i],
			trajectory.triangles[i + 1],
			trajectory.triangles[i + 2],
		];
		if (tri.every((vertex) => supportMap.has(vertex))) drawable.push(tri);
	}
	const xs = Array.from(
		trajectory.support,
		(vertex) => trajectory.coordinates[vertex * 2],
	);
	const ys = Array.from(
		trajectory.support,
		(vertex) => trajectory.coordinates[vertex * 2 + 1],
	);
	const minX = Math.min(...xs),
		maxX = Math.max(...xs),
		minY = Math.min(...ys),
		maxY = Math.max(...ys),
		scale = Math.min(860 / (maxX - minX || 1), 400 / (maxY - minY || 1));
	const point = (vertex: number): [number, number] => [
		50 + (trajectory.coordinates[vertex * 2] - minX) * scale,
		440 - (trajectory.coordinates[vertex * 2 + 1] - minY) * scale,
	];
	function draw(): void {
		const context = canvas.getContext("2d");
		if (!context) return;
		context.clearRect(0, 0, canvas.width, canvas.height);
		const offset = stateIndex * trajectory.support.length;
		const stateValues = trajectory.values.slice(
			offset,
			offset + trajectory.support.length,
		);
		const minimum = Math.min(...stateValues),
			maximum = Math.max(...stateValues);
		for (const [a, b, c] of drawable) {
			const vertices = [a, b, c],
				average =
					vertices.reduce(
						(sum, vertex) => sum + stateValues[supportMap.get(vertex) as number],
						0,
					) / 3;
			context.beginPath();
			vertices.forEach((vertex, index) => {
				const [x, y] = point(vertex);
				if (index === 0) context.moveTo(x, y);
				else context.lineTo(x, y);
			});
			context.closePath();
			context.fillStyle = color(average, minimum, maximum);
			context.fill();
			context.strokeStyle = "rgba(20,31,52,.45)";
			context.stroke();
		}
		meta.textContent = `state ${stateIndex + 1}/${trajectory.steps.length} · step ${trajectory.steps[stateIndex]} · t=${trajectory.times[stateIndex]} s · field ${trajectory.fieldId} · dimension [${trajectory.dimension.join(", ")}] · ${trajectory.frame} · range ${minimum}…${maximum}`;
		slider.value = String(stateIndex);
		previous.disabled = stateIndex === 0;
		next.disabled = stateIndex === trajectory.steps.length - 1;
	}
	function stop(): void {
		if (timer !== undefined) window.clearInterval(timer);
		timer = undefined;
		play.textContent = "Play";
	}
	function start(): void {
		stop();
		play.textContent = "Pause";
		timer = window.setInterval(() => {
			stateIndex = (stateIndex + 1) % trajectory.steps.length;
			draw();
		}, 1000 / Number(speed.value));
	}
	previous.addEventListener("click", () => {
		stop();
		stateIndex = Math.max(0, stateIndex - 1);
		draw();
	});
	next.addEventListener("click", () => {
		stop();
		stateIndex = Math.min(trajectory.steps.length - 1, stateIndex + 1);
		draw();
	});
	play.addEventListener("click", () => (timer === undefined ? start() : stop()));
	slider.addEventListener("input", () => {
		stop();
		stateIndex = Number(slider.value);
		draw();
	});
	speed.addEventListener("change", () => {
		if (timer !== undefined) start();
	});
	root.__eqioraN3Oracle = {
		snapshot: () => ({
			stateIndex,
			step: String(trajectory.steps[stateIndex]),
			timeS: trajectory.times[stateIndex],
			playing: timer !== undefined,
			disposed,
			drawableTriangles: drawable.length,
		}),
	};
	draw();
	return () => {
		disposed = true;
		stop();
		delete root.__eqioraN3Oracle;
		el.replaceChildren();
	};
}
