function e(e) {
	throw Error(`Invalid Eqiora Trajectory presentation payload: ${e}`);
}
function t(t, n, r) {
	return (typeof t != "string" || t !== n) && e(`${r} changed`), t;
}
function n(t, n) {
	return (typeof t != "string" || !/^[0-9a-f]{64}$/.test(t)) && e(`${n} is not a lowercase SHA-256 digest`), t;
}
function r(t, n) {
	return (typeof t != "string" || t.length === 0) && e(`${n} is empty or invalid`), t;
}
function i(t, n, r) {
	return (!Number.isSafeInteger(t) || t !== n) && e(`${r} changed`), t;
}
function a(t, n, r) {
	(!(t instanceof DataView) || t.byteLength !== n) && e(`${r} has the wrong binary type or byte length`);
	let i = new Uint8Array(new ArrayBuffer(n));
	return i.set(new Uint8Array(t.buffer, t.byteOffset, t.byteLength)), i;
}
function o(e, t) {
	return e >>> t | e << 32 - t;
}
var s = new Uint32Array([
	1779033703,
	3144134277,
	1013904242,
	2773480762,
	1359893119,
	2600822924,
	528734635,
	1541459225
]), c = new Uint32Array([
	1116352408,
	1899447441,
	3049323471,
	3921009573,
	961987163,
	1508970993,
	2453635748,
	2870763221,
	3624381080,
	310598401,
	607225278,
	1426881987,
	1925078388,
	2162078206,
	2614888103,
	3248222580,
	3835390401,
	4022224774,
	264347078,
	604807628,
	770255983,
	1249150122,
	1555081692,
	1996064986,
	2554220882,
	2821834349,
	2952996808,
	3210313671,
	3336571891,
	3584528711,
	113926993,
	338241895,
	666307205,
	773529912,
	1294757372,
	1396182291,
	1695183700,
	1986661051,
	2177026350,
	2456956037,
	2730485921,
	2820302411,
	3259730800,
	3345764771,
	3516065817,
	3600352804,
	4094571909,
	275423344,
	430227734,
	506948616,
	659060556,
	883997877,
	958139571,
	1322822218,
	1537002063,
	1747873779,
	1955562222,
	2024104815,
	2227730452,
	2361852424,
	2428436474,
	2756734187,
	3204031479,
	3329325298
]);
function l(e) {
	let t = e.byteLength * 8, n = Math.ceil((e.byteLength + 9) / 64) * 64, r = new Uint8Array(new ArrayBuffer(n));
	r.set(e), r[e.byteLength] = 128;
	let i = new DataView(r.buffer);
	i.setUint32(n - 8, Math.floor(t / 4294967296), !1), i.setUint32(n - 4, t >>> 0, !1);
	let a = new Uint32Array(s), l = /* @__PURE__ */ new Uint32Array(64);
	for (let e = 0; e < n; e += 64) {
		for (let t = 0; t < 16; t += 1) l[t] = i.getUint32(e + t * 4, !1);
		for (let e = 16; e < 64; e += 1) {
			let t = l[e - 15], n = l[e - 2];
			l[e] = l[e - 16] + (o(t, 7) ^ o(t, 18) ^ t >>> 3) + l[e - 7] + (o(n, 17) ^ o(n, 19) ^ n >>> 10) >>> 0;
		}
		let [t, n, r, s, u, d, f, p] = a;
		for (let e = 0; e < 64; e += 1) {
			let i = p + (o(u, 6) ^ o(u, 11) ^ o(u, 25)) + (u & d ^ ~u & f) + c[e] + l[e] >>> 0, a = (o(t, 2) ^ o(t, 13) ^ o(t, 22)) + (t & n ^ t & r ^ n & r) >>> 0;
			[p, f, d, u, s, r, n, t] = [
				f,
				d,
				u,
				s + i >>> 0,
				r,
				n,
				t,
				i + a >>> 0
			];
		}
		a[0] = a[0] + t >>> 0, a[1] = a[1] + n >>> 0, a[2] = a[2] + r >>> 0, a[3] = a[3] + s >>> 0, a[4] = a[4] + u >>> 0, a[5] = a[5] + d >>> 0, a[6] = a[6] + f >>> 0, a[7] = a[7] + p >>> 0;
	}
	return Array.from(a, (e) => e.toString(16).padStart(8, "0")).join("");
}
function u(t, r, i) {
	let o = a(t.get(r), i, r);
	return l(o) !== n(t.get(r.replace(/_(?:f64|u32|u64)_le$/, "_sha256")), `${r} hash`) && e(`${r} digest disagrees with its bytes`), o;
}
function d(e) {
	let t = new Float64Array(e.byteLength / 8), n = new DataView(e.buffer);
	for (let e = 0; e < t.length; e += 1) t[e] = n.getFloat64(e * 8, !0);
	return t;
}
function f(e) {
	let t = new Uint32Array(e.byteLength / 4), n = new DataView(e.buffer);
	for (let e = 0; e < t.length; e += 1) t[e] = n.getUint32(e * 4, !0);
	return t;
}
function p(a) {
	t(a.get("profile"), "fixed-mesh-scalar-trajectory-2d/v1", "profile"), i(a.get("vertex_count"), 9, "vertex_count"), i(a.get("triangle_count"), 8, "triangle_count"), i(a.get("state_count"), 2, "state_count");
	for (let t of ["state_digests", "snapshot_digests"]) {
		let n = a.get(t);
		(typeof n != "string" || n.split(",").length !== 2 || !n.split(",").every((e) => /^[0-9a-f]{64}$/.test(e))) && e(`${t} changed`);
	}
	let o = a.get("dimension");
	(typeof o != "string" || !/^-?\d+(,-?\d+){6}$/.test(o)) && e("dimension changed");
	let s = o.split(",").map(Number);
	s.every(Number.isSafeInteger) || e("dimension changed");
	let c = d(u(a, "coordinates_f64_le", 144)), l = f(u(a, "triangles_u32_le", 96)), p = f(u(a, "support_u32_le", 24)), m = u(a, "steps_u64_le", 16), h = d(u(a, "times_f64_le", 16)), g = d(u(a, "values_f64_le", 96)), _ = new DataView(m.buffer), v = Array.from({ length: 2 }, (e, t) => _.getBigUint64(t * 8, !0));
	return (!c.every(Number.isFinite) || !h.every(Number.isFinite) || !g.every(Number.isFinite)) && e("non-finite numeric member"), (h[1] <= h[0] || v[1] <= v[0]) && e("states are not strictly ordered"), (l.some((e) => e >= 9) || p.some((e) => e >= 9) || new Set(p).size !== 6) && e("topology or support is invalid"), {
		trajectoryDigest: n(a.get("trajectory_digest"), "trajectory_digest"),
		meshDigest: n(a.get("mesh_digest"), "mesh_digest"),
		fieldId: r(a.get("field_id"), "field_id"),
		dimension: Object.freeze(s),
		frame: t(a.get("frame"), "invariant", "frame"),
		coordinates: c,
		triangles: l,
		support: p,
		steps: Object.freeze(v),
		times: h,
		values: g
	};
}
//#endregion
//#region src/trajectory-view.ts
function m(e, t) {
	let n = document.createElement(e);
	return t.append(n), n;
}
function h(e, t, n) {
	let r = n === t ? .5 : Math.max(0, Math.min(1, (e - t) / (n - t))), i = [
		[
			38,
			63,
			143
		],
		[
			57,
			168,
			189
		],
		[
			243,
			211,
			91
		],
		[
			181,
			40,
			53
		]
	], a = r * (i.length - 1), o = Math.min(i.length - 2, Math.floor(a)), s = a - o;
	return `rgb(${i[o].map((e, t) => Math.round(e + (i[o + 1][t] - e) * s)).join(",")})`;
}
function g({ model: e, el: t }) {
	let n;
	try {
		n = p(e);
	} catch {
		return t.className = "eqiora-trajectory-error", t.textContent = "Eqiora could not validate this Trajectory view. The exact text representation remains available.", () => {
			t.replaceChildren();
		};
	}
	let r = m("div", t);
	r.className = "eqiora-trajectory", r.dataset.eqioraTrajectoryDigest = n.trajectoryDigest;
	let i = m("canvas", r);
	i.width = 960, i.height = 480;
	let a = `Eqiora Trajectory ${n.trajectoryDigest}; field ${n.fieldId}; coherent-SI dimension [${n.dimension.join(", ")}]; ${n.frame} frame; ${n.steps.length} stored states.`;
	i.setAttribute("role", "img"), i.setAttribute("aria-label", a), i.textContent = a;
	let o = m("div", r);
	o.className = "eqiora-trajectory-meta";
	let s = m("div", r);
	s.className = "eqiora-trajectory-controls";
	let c = m("button", s);
	c.type = "button", c.textContent = "Previous";
	let l = m("button", s);
	l.type = "button", l.textContent = "Play";
	let u = m("button", s);
	u.type = "button", u.textContent = "Next";
	let d = m("input", s);
	d.type = "range", d.min = "0", d.max = String(n.steps.length - 1), d.step = "1", d.value = "0", d.setAttribute("aria-label", "Trajectory state");
	let f = m("select", s);
	f.setAttribute("aria-label", "Playback speed");
	for (let e of [
		.5,
		1,
		2
	]) {
		let t = m("option", f);
		t.value = String(e), t.textContent = `${e}×`, e === 1 && (t.selected = !0);
	}
	let g = m("span", s);
	g.className = "eqiora-trajectory-swatch";
	let _ = 0, v, y = !1, b = /* @__PURE__ */ new Map();
	n.support.forEach((e, t) => {
		b.set(e, t);
	});
	let x = [];
	for (let e = 0; e < n.triangles.length; e += 3) {
		let t = [
			n.triangles[e],
			n.triangles[e + 1],
			n.triangles[e + 2]
		];
		t.every((e) => b.has(e)) && x.push(t);
	}
	let S = Array.from(n.support, (e) => n.coordinates[e * 2]), C = Array.from(n.support, (e) => n.coordinates[e * 2 + 1]), w = Math.min(...S), T = Math.max(...S), E = Math.min(...C), D = Math.max(...C), O = Math.min(860 / (T - w || 1), 400 / (D - E || 1)), k = (e) => [50 + (n.coordinates[e * 2] - w) * O, 440 - (n.coordinates[e * 2 + 1] - E) * O];
	function A() {
		let e = i.getContext("2d");
		if (!e) return;
		e.clearRect(0, 0, i.width, i.height);
		let t = _ * n.support.length, r = n.values.slice(t, t + n.support.length), a = Math.min(...r), s = Math.max(...r);
		for (let [t, n, i] of x) {
			let o = [
				t,
				n,
				i
			], c = o.reduce((e, t) => e + r[b.get(t)], 0) / 3;
			e.beginPath(), o.forEach((t, n) => {
				let [r, i] = k(t);
				n === 0 ? e.moveTo(r, i) : e.lineTo(r, i);
			}), e.closePath(), e.fillStyle = h(c, a, s), e.fill(), e.strokeStyle = "rgba(20,31,52,.45)", e.stroke();
		}
		o.textContent = `state ${_ + 1}/${n.steps.length} · step ${n.steps[_]} · t=${n.times[_]} s · field ${n.fieldId} · dimension [${n.dimension.join(", ")}] · ${n.frame} · range ${a}…${s}`, d.value = String(_), c.disabled = _ === 0, u.disabled = _ === n.steps.length - 1;
	}
	function j() {
		v !== void 0 && window.clearInterval(v), v = void 0, l.textContent = "Play";
	}
	function M() {
		j(), l.textContent = "Pause";
		let e = () => {
			_ = (_ + 1) % n.steps.length, A();
		};
		e(), v = window.setInterval(e, 1e3 / Number(f.value));
	}
	c.addEventListener("click", () => {
		j(), _ = Math.max(0, _ - 1), A();
	}), u.addEventListener("click", () => {
		j(), _ = Math.min(n.steps.length - 1, _ + 1), A();
	}), l.addEventListener("click", () => v === void 0 ? M() : j()), d.addEventListener("input", () => {
		j(), _ = Number(d.value), A();
	}), f.addEventListener("change", () => {
		v !== void 0 && M();
	}), A();
	let N = () => {
		y || (y = !0, j(), e.off?.("destroy", N), e.off?.("comm:close", N), t.replaceChildren());
	};
	return e.on?.("destroy", N), e.on?.("comm:close", N), N;
}
//#endregion
//#region src/mesh-view.ts
function _(e) {
	return e.model.get("profile") === "fixed-mesh-scalar-trajectory-2d/v1" ? g(e) : (e.el.replaceChildren(), () => void 0);
}
var v = { render: _ };
//#endregion
export { v as default };
