#!/usr/bin/env python3
"""Route diagnostic: the same assembly on a straight channel with no hole.

This is **not** part of the frozen route and **not** a claim of the frozen contract. It
exists to answer one question a reader of ``result.json`` will ask: the pressure
and cylinder reaction recovered on the RFC 0082 chordal mesh are three to four
orders of magnitude above their physical scales, so is the assembly wrong?

It is not. On a straight ``2.2 m x 0.41 m`` channel with the same viscosity,
the same parabolic inlet, the same no-slip walls and the same zero outlet
traction, this module's assembly recovers the analytic Poiseuille pressure drop

    dp = 8 mu Umax Lx / H^2

the analytic centreline speed ``Umax`` and the analytic flux ``2 Umax H / 3``,
converging as the mesh refines. The contract numbers are a property of the RFC 0082
reference topology, which has no interior velocity vertices at all, not of this
route's algebra. The exact-reproduction patch test inside ``oracle.py`` is the
hard check; this script is the magnitude corroboration.

    python3 reference_channel.py

Requires ``mpmath``.
"""

from __future__ import annotations

import pathlib
import sys
import time

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

import mpmath  # noqa: E402

mpmath.mp.dps = 20

import stokes  # noqa: E402

MU = mpmath.mpf(0.001)
H = mpmath.mpf(0.41)
UMAX = mpmath.mpf(0.3)
LX = mpmath.mpf("2.2")


def inlet_profile(y):
    return 4 * UMAX * y * (H - y) / H**2


def structured_channel(nx: int, ny: int):
    vertices, index = [], {}
    for i in range(nx + 1):
        for j in range(ny + 1):
            index[(i, j)] = len(vertices)
            vertices.append((LX * i / nx, H * j / ny))
    cells, inlet, outlet, walls = [], [], [], []
    for i in range(nx):
        for j in range(ny):
            a = index[(i, j)]
            b = index[(i + 1, j)]
            c = index[(i + 1, j + 1)]
            d = index[(i, j + 1)]
            cells += [(a, b, c), (a, c, d)]
    for j in range(ny):
        inlet.append((index[(0, j + 1)], index[(0, j)]))
        outlet.append((index[(nx, j)], index[(nx, j + 1)]))
    for i in range(nx):
        walls.append((index[(i, 0)], index[(i + 1, 0)]))
        walls.append((index[(i + 1, ny)], index[(i, ny)]))
    return vertices, cells, inlet, outlet, walls


def run(nx: int, ny: int) -> dict:
    vertices, cells, inlet, outlet, walls = structured_channel(nx, ny)
    system = stokes.assemble(vertices, cells, MU, stokes.Formulation())
    prescribed = {}
    for v in sorted({k for pair in inlet + walls for k in pair}):
        _, y = vertices[v]
        ux = mpmath.mpf(0) if (y == 0 or y == H) else inlet_profile(y)
        prescribed[system.vel_p1(v, 0)] = ux
        prescribed[system.vel_p1(v, 1)] = mpmath.mpf(0)
    stokes.add_facet_traction(system, vertices, outlet, (mpmath.mpf(0), mpmath.mpf(0)))
    x = stokes.condensed_solve(system, prescribed)

    p_in = [x[system.pressure(v)] for v in range(len(vertices)) if vertices[v][0] == 0]
    p_out = [
        x[system.pressure(v)] for v in range(len(vertices)) if vertices[v][0] == LX
    ]
    drop = sum(p_in) / len(p_in) - sum(p_out) / len(p_out)

    best, best_d = 0, None
    for c, cell in enumerate(cells):
        bx = sum(vertices[k][0] for k in cell) / 3
        by = sum(vertices[k][1] for k in cell) / 3
        d = (bx - LX / 2) ** 2 + (by - H / 2) ** 2
        if best_d is None or d < best_d:
            best, best_d = c, d
    cell = cells[best]
    centreline = (
        sum(x[system.vel_p1(k, 0)] for k in cell) / 3 + x[system.vel_bubble(best, 0)]
    )

    flux = mpmath.mpf(0)
    for a, b in outlet:
        pa, pb = vertices[a], vertices[b]
        flux += (x[system.vel_p1(a, 0)] + x[system.vel_p1(b, 0)]) / 2 * (pb[1] - pa[1])
        flux += (x[system.vel_p1(a, 1)] + x[system.vel_p1(b, 1)]) / 2 * -(pb[0] - pa[0])

    return {
        "drop": drop,
        "drop_exact": 8 * MU * UMAX * LX / H**2,
        "centreline": centreline,
        "centreline_exact": UMAX,
        "flux": flux,
        "flux_exact": 2 * UMAX * H / 3,
    }


def main() -> int:
    print(
        f"straight channel {float(LX)} m x {float(H)} m, mu = {float(MU)} Pa s, Umax = {float(UMAX)} m/s"
    )
    for nx, ny in ((6, 3), (10, 5), (14, 7)):
        started = time.perf_counter()
        r = run(nx, ny)
        print(
            f"  {nx:2d}x{ny}: dp = {mpmath.nstr(r['drop'], 8)} Pa "
            f"(analytic {mpmath.nstr(r['drop_exact'], 8)}, ratio {mpmath.nstr(r['drop'] / r['drop_exact'], 6)})"
            f"  u_centre = {mpmath.nstr(r['centreline'], 6)} (analytic {mpmath.nstr(r['centreline_exact'], 6)})"
            f"  flux = {mpmath.nstr(r['flux'], 6)} (analytic {mpmath.nstr(r['flux_exact'], 6)})"
            f"  [{time.perf_counter() - started:.1f}s]",
            flush=True,
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
