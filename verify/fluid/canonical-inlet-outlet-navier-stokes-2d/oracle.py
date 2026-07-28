#!/usr/bin/env python3
"""Open-boundary convection identity oracle (pre-committed, non-implementing lane).

Issue nkiyohara/eqiora#124. This file is evidence, not implementation. It was
written without reading any Rust source, Rust test, or existing numerical
fixture, and it freezes the expected values that the implementation lane must
reproduce. The implementation lane must not edit the witnesses, the frozen
values, the tolerances (there are none -- every comparison is exact), or the
falsifiers. An implementer who believes a frozen value is wrong stops and
returns the proof.

Frozen public claim
-------------------
Constant density ``rho`` on an affine, bounded, polygonal 2D domain ``Omega``
meshed by affine triangles, with a continuous P1 velocity ``u = (u_0, u_1)``
and a scalar P1 test function ``phi``. Per test row ``phi`` and velocity
component ``i``, define

    A = int_Omega rho (u . grad) u_i phi dx        advective row
    C = - int_Omega rho (u . grad phi) u_i dx      conservative row, volume only
    S = (A + C) / 2                                energy-skew row
    B = int_dOmega rho (u . n) u_i phi ds          parent-outward boundary flux
    D = int_Omega rho (div u) u_i phi dx           divergence defect

The claim under audit is

    S - C = B/2 - D/2.

Derivation (owned here, independent of any implementation)
---------------------------------------------------------
Apply the divergence theorem to the vector field ``rho u u_i phi``:

    int_Omega div(rho u u_i phi) dx = int_dOmega rho (u . n) u_i phi ds = B.

Expanding the divergence of the product, with rho constant,

    div(rho u u_i phi) = rho (div u) u_i phi + rho (u . grad u_i) phi
                       + rho u_i (u . grad phi).

Integrating termwise gives ``D + A + (-C) = B``, because the third term
integrates to ``-C`` by the sign in the definition of ``C``. Hence

    A - C = B - D,  and  S - C = (A + C)/2 - C = (A - C)/2 = B/2 - D/2.

The sign is therefore fixed by the derivation, not chosen: the boundary term
enters with ``+1/2`` under the parent-outward normal and the divergence term
with ``-1/2``. Reversing the normal or dropping either term changes the row by
a quantity this oracle computes explicitly.

Why every quantity is exactly rational
--------------------------------------
Arclength and unit normals are irrational in general, but they only ever occur
as the product ``n ds``. On an edge from ``P`` to ``Q`` with ``d = Q - P``,
parametrised by ``x(t) = P + t d`` for ``t`` in ``[0, 1]``, the unit normal is
``(d_y, -d_x)/|d|`` (up to sign) and ``ds = |d| dt``, so

    n ds = (d_y, -d_x) dt,

which is rational in the vertex coordinates. Every integrand below is a
polynomial with rational coefficients over a rational domain, so the whole
audit is exact in ``fractions.Fraction`` with no tolerance anywhere.

Polynomial degrees, and why degree three is the facet requirement
-----------------------------------------------------------------
With P1 velocity on an affine triangle, ``u_i`` and ``phi`` are degree 1 and
``grad u_i``, ``grad phi`` and ``div u`` are constant. So ``A``, ``C`` and
``D`` have degree-2 volume integrands, while the boundary integrand
``(u . n) u_i phi`` is a product of three traces, each linear along the edge:
degree 3 in the edge parameter. A facet rule that is only degree-1 exact
(midpoint, trapezoid) under-integrates it. This is the ``under-integration``
falsifier, and it is why the volume rules and the facet rules are certified
separately below.

Two independent routes, no shared evaluation kernel
---------------------------------------------------
Route 1, ``AnalyticRoute`` -- symbolic. P1 coefficients come from an exact
3x3 Cramer solve of ``lambda_k(V_j) = delta_kj``. Fields are built as explicit
bivariate polynomial coefficient dictionaries in Cartesian ``(x, y)``;
gradients are coefficient-level differentiation. A triangle integral is taken
by composing the polynomial with the affine pullback to the reference triangle
and summing ``m! n! / (m+n+2)!`` per reference monomial, times ``|det J|``. An
edge integral composes the polynomial with the line parametrisation and sums
``1/(k+1)`` per power of ``t``. Outward normals use the CCW-orientation rule
``(d_y, -d_x)``, and CCW listing is checked per element.

Route 2, ``QuadratureRoute`` -- numerical. No polynomial coefficients exist
anywhere in it. Values come from barycentric interpolation at quadrature
points; gradients come from the reference chain rule ``grad lambda = J^-T
ghat`` with ``ghat`` the constant reference gradients. Volume integrals use a
degree-3 rational triangle rule (centroid weight ``-27/48``, three points
``(3/5, 1/5, 1/5)`` weight ``25/48``); facet integrals use rational Newton-
Cotes rules on ``[0, 1]``. Outward normals are derived geometrically by the
opposite-vertex sign test, never from vertex ordering.

The two routes therefore share only the mesh data and the rational number
type. They agree exactly on every quantity, for every row, in every witness,
and both agree with the frozen table.

Witnesses
---------
w1  Single affine rational triangle, open boundary. Nonzero boundary flux and
    nonzero divergence, with no cancellation between them.
w2  Two-triangle quadrilateral, open boundary, one interior facet. Also proves
    that interior facet contributions cancel exactly, so the assembled
    boundary term is over dOmega only.
w3  Closed unit square, four triangles, complete zero velocity trace. Proves
    the zero-normal-flux reduction ``S - C = -D/2`` with ``B = 0`` exactly,
    while ``D`` stays nonzero, so the reduction is not vacuous.
w4  Single triangle, exactly divergence-free P1 velocity, nonzero boundary
    flux. Isolates the boundary term: ``S - C = B/2`` with ``D = 0``.

All three P1 basis rows and both velocity components are covered in w1 and w4;
w2 and w3 cover all global nodal rows and both components.

Falsifiers (each must be caught by at least one witness)
--------------------------------------------------------
omit_boundary        RHS taken as ``-D/2``.
reversed_normal      RHS taken as ``-B/2 - D/2``.
omit_divergence      RHS taken as ``B/2``.
midpoint_boundary    ``B`` recomputed with the degree-1 midpoint facet rule.
trapezoid_boundary   ``B`` recomputed with the degree-1 trapezoid facet rule.

Non-claims
----------
P1 velocity and P1 test rows only: the MINI bubble enrichment is not a witness
here, and a bubble-bearing velocity raises the facet integrand above degree 3.
No time stepping, solver, pressure, gauge, mesh admission, or wire behaviour
is audited. Nothing here asserts what the Rust assembly currently computes;
this file states only what it must compute to satisfy the frozen claim.

Usage
-----
    python3 oracle.py                 run every check, exit nonzero on failure
    python3 oracle.py --emit-frozen   print the frozen table as Python source
"""

from __future__ import annotations

import sys
from dataclasses import dataclass
from fractions import Fraction as F
from math import factorial

# ---------------------------------------------------------------------------
# Exact bivariate polynomial algebra (route 1 only).
# A polynomial is a dict {(i, j): Fraction} meaning sum c_ij * v0^i * v1^j.
# Zero coefficients are pruned so that equality of dicts is equality of
# polynomials.
# ---------------------------------------------------------------------------

P_ONE: dict[tuple[int, int], F] = {(0, 0): F(1)}


def p_lin(c: F, cx: F, cy: F) -> dict[tuple[int, int], F]:
    """Linear polynomial ``c + cx * v0 + cy * v1`` with zero terms pruned."""
    out: dict[tuple[int, int], F] = {}
    if c:
        out[(0, 0)] = c
    if cx:
        out[(1, 0)] = cx
    if cy:
        out[(0, 1)] = cy
    return out


def p_add(a: dict, b: dict) -> dict:
    out = dict(a)
    for k, v in b.items():
        nv = out.get(k, F(0)) + v
        if nv:
            out[k] = nv
        else:
            out.pop(k, None)
    return out


def p_scale(a: dict, c: F) -> dict:
    if not c:
        return {}
    return {k: v * c for k, v in a.items()}


def p_mul(a: dict, b: dict) -> dict:
    out: dict[tuple[int, int], F] = {}
    for (i, j), u in a.items():
        for (k, m), v in b.items():
            key = (i + k, j + m)
            nv = out.get(key, F(0)) + u * v
            if nv:
                out[key] = nv
            else:
                out.pop(key, None)
    return out


def p_sum(parts) -> dict:
    out: dict[tuple[int, int], F] = {}
    for p in parts:
        out = p_add(out, p)
    return out


def p_dx(a: dict) -> dict:
    return {(i - 1, j): v * i for (i, j), v in a.items() if i}


def p_dy(a: dict) -> dict:
    return {(i, j - 1): v * j for (i, j), v in a.items() if j}


def p_pow(a: dict, n: int) -> dict:
    out = P_ONE
    for _ in range(n):
        out = p_mul(out, a)
    return out


def p_compose(a: dict, mx: dict, my: dict) -> dict:
    """Substitute ``v0 -> mx``, ``v1 -> my`` into ``a``."""
    return p_sum(
        p_scale(p_mul(p_pow(mx, i), p_pow(my, j)), c) for (i, j), c in a.items()
    )


def _det3(m) -> F:
    return (
        m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
        - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
        + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
    )


def solve3(m, rhs) -> list[F]:
    """Exact Cramer solve of a 3x3 rational system."""
    d = _det3(m)
    if d == 0:
        raise ValueError("degenerate triangle: singular P1 Vandermonde matrix")
    out = []
    for col in range(3):
        mm = [[rhs[r] if c == col else m[r][c] for c in range(3)] for r in range(3)]
        out.append(_det3(mm) / d)
    return out


# ---------------------------------------------------------------------------
# Shared geometry primitives (data, not an evaluation kernel).
# ---------------------------------------------------------------------------

Point = tuple[F, F]
Vel = tuple[F, F]


def signed_det(verts) -> F:
    """Twice the signed area of the listed triangle; positive iff CCW."""
    (x0, y0), (x1, y1), (x2, y2) = verts
    return (x1 - x0) * (y2 - y0) - (x2 - x0) * (y1 - y0)


# ---------------------------------------------------------------------------
# Route 1: analytic polynomial expansion and exact monomial integration.
# ---------------------------------------------------------------------------


class AnalyticRoute:
    name = "analytic-polynomial"

    def basis(self, verts) -> list[dict]:
        """P1 nodal basis as Cartesian polynomials, by exact 3x3 solve."""
        rows = [[F(1), x, y] for (x, y) in verts]
        out = []
        for k in range(3):
            rhs = [F(1) if j == k else F(0) for j in range(3)]
            a, b, c = solve3(rows, rhs)
            out.append(p_lin(a, b, c))
        return out

    def tri_int(self, poly: dict, verts) -> F:
        """Exact integral over the triangle via affine pullback to reference."""
        (x0, y0), (x1, y1), (x2, y2) = verts
        mx = p_lin(x0, x1 - x0, x2 - x0)
        my = p_lin(y0, y1 - y0, y2 - y0)
        q = p_compose(poly, mx, my)
        total = F(0)
        for (m, n), c in q.items():
            total += c * F(factorial(m) * factorial(n), factorial(m + n + 2))
        return abs(signed_det(verts)) * total

    def seg_int(self, poly: dict, p: Point, q: Point) -> F:
        """Exact ``int_0^1 poly(p + t (q - p)) dt`` (parameter measure)."""
        mx = p_lin(p[0], q[0] - p[0], F(0))
        my = p_lin(p[1], q[1] - p[1], F(0))
        composed = p_compose(poly, mx, my)
        total = F(0)
        for (k, j), c in composed.items():
            if j != 0:
                raise AssertionError("line pullback produced a second variable")
            total += c / (k + 1)
        return total

    def _fields(self, verts, u_dofs, m):
        lam = self.basis(verts)
        u = [p_sum(p_scale(lam[k], u_dofs[k][i]) for k in range(3)) for i in (0, 1)]
        return lam, u, lam[m]

    def scaled_outward_normal(self, verts, e: int) -> Point:
        """CCW rule: for a CCW-listed triangle, local edge e -> e+1 is CCW."""
        if signed_det(verts) <= 0:
            raise ValueError("analytic normal rule requires a CCW-listed triangle")
        p, q = verts[e], verts[(e + 1) % 3]
        return (q[1] - p[1], -(q[0] - p[0]))

    def element_volume_terms(self, verts, u_dofs, m, rho) -> dict[str, list[F]]:
        _, u, phi = self._fields(verts, u_dofs, m)
        gphi = (p_dx(phi), p_dy(phi))
        div = p_add(p_dx(u[0]), p_dy(u[1]))
        a_row, c_row, d_row = [], [], []
        for i in (0, 1):
            gu = (p_dx(u[i]), p_dy(u[i]))
            adv = p_add(p_mul(u[0], gu[0]), p_mul(u[1], gu[1]))
            a_row.append(rho * self.tri_int(p_mul(adv, phi), verts))
            con = p_add(p_mul(u[0], gphi[0]), p_mul(u[1], gphi[1]))
            c_row.append(-rho * self.tri_int(p_mul(con, u[i]), verts))
            d_row.append(rho * self.tri_int(p_mul(p_mul(div, u[i]), phi), verts))
        return {"A": a_row, "C": c_row, "D": d_row}

    def element_edge_term(self, verts, u_dofs, m, e, rho, rule=None) -> list[F]:
        if rule is not None:
            raise ValueError("the analytic route integrates edges exactly")
        _, u, phi = self._fields(verts, u_dofs, m)
        nx, ny = self.scaled_outward_normal(verts, e)
        flux = p_add(p_scale(u[0], nx), p_scale(u[1], ny))
        p, q = verts[e], verts[(e + 1) % 3]
        return [rho * self.seg_int(p_mul(p_mul(flux, u[i]), phi), p, q) for i in (0, 1)]


# ---------------------------------------------------------------------------
# Route 2: rational quadrature on point evaluations. No polynomial algebra.
# ---------------------------------------------------------------------------

# Degree-3 exact triangle rule, barycentric points with weights summing to 1.
TRI_D3 = (
    ((F(1, 3), F(1, 3), F(1, 3)), F(-27, 48)),
    ((F(3, 5), F(1, 5), F(1, 5)), F(25, 48)),
    ((F(1, 5), F(3, 5), F(1, 5)), F(25, 48)),
    ((F(1, 5), F(1, 5), F(3, 5)), F(25, 48)),
)

# Facet rules on the parameter interval [0, 1].
EDGE_SIMPSON = ((F(0), F(1, 6)), (F(1, 2), F(4, 6)), (F(1), F(1, 6)))
EDGE_SIMPSON38 = (
    (F(0), F(1, 8)),
    (F(1, 3), F(3, 8)),
    (F(2, 3), F(3, 8)),
    (F(1), F(1, 8)),
)
EDGE_MIDPOINT = ((F(1, 2), F(1)),)
EDGE_TRAPEZOID = ((F(0), F(1, 2)), (F(1), F(1, 2)))

# Constant reference-element P1 gradients for lambda_0 = 1 - xi - eta,
# lambda_1 = xi, lambda_2 = eta.
REF_GRADS = ((F(-1), F(-1)), (F(1), F(0)), (F(0), F(1)))


class QuadratureRoute:
    name = "degree3-quadrature"

    def grads(self, verts) -> list[Point]:
        """Physical P1 gradients by the reference chain rule ``J^-T ghat``."""
        (x0, y0), (x1, y1), (x2, y2) = verts
        a, b = x1 - x0, x2 - x0
        c, d = y1 - y0, y2 - y0
        det = a * d - b * c
        if det == 0:
            raise ValueError("degenerate triangle: singular affine Jacobian")
        out = []
        for gx, gy in REF_GRADS:
            out.append(((d * gx - c * gy) / det, (-b * gx + a * gy) / det))
        return out

    def area(self, verts) -> F:
        return abs(signed_det(verts)) / 2

    def scaled_outward_normal(self, verts, e: int) -> Point:
        """Derived outward normal: rotate the edge, then fix the sign by the
        opposite-vertex test. Independent of how the vertices were listed."""
        p, q, r = verts[e], verts[(e + 1) % 3], verts[(e + 2) % 3]
        dx, dy = q[0] - p[0], q[1] - p[1]
        n = (dy, -dx)
        vx = (p[0] + q[0]) / 2 - r[0]
        vy = (p[1] + q[1]) / 2 - r[1]
        if n[0] * vx + n[1] * vy < 0:
            n = (-n[0], -n[1])
        return n

    @staticmethod
    def _interp(lam, u_dofs, i) -> F:
        return sum((lam[k] * u_dofs[k][i] for k in range(3)), F(0))

    def element_volume_terms(self, verts, u_dofs, m, rho) -> dict[str, list[F]]:
        g = self.grads(verts)
        gu = [
            (
                sum((u_dofs[k][i] * g[k][0] for k in range(3)), F(0)),
                sum((u_dofs[k][i] * g[k][1] for k in range(3)), F(0)),
            )
            for i in (0, 1)
        ]
        div = gu[0][0] + gu[1][1]
        gphi = g[m]
        acc = {"A": [F(0), F(0)], "C": [F(0), F(0)], "D": [F(0), F(0)]}
        for lam, w in TRI_D3:
            uu = (self._interp(lam, u_dofs, 0), self._interp(lam, u_dofs, 1))
            phi = lam[m]
            for i in (0, 1):
                acc["A"][i] += w * (uu[0] * gu[i][0] + uu[1] * gu[i][1]) * phi
                acc["C"][i] -= w * (uu[0] * gphi[0] + uu[1] * gphi[1]) * uu[i]
                acc["D"][i] += w * div * uu[i] * phi
        scale = rho * self.area(verts)
        return {key: [scale * v for v in val] for key, val in acc.items()}

    def element_edge_term(self, verts, u_dofs, m, e, rho, rule=None) -> list[F]:
        rule = EDGE_SIMPSON if rule is None else rule
        nx, ny = self.scaled_outward_normal(verts, e)
        a, b, r = e, (e + 1) % 3, (e + 2) % 3
        out = [F(0), F(0)]
        for t, w in rule:
            lam = [F(0), F(0), F(0)]
            lam[a] = 1 - t
            lam[b] = t
            lam[r] = F(0)
            uu = (self._interp(lam, u_dofs, 0), self._interp(lam, u_dofs, 1))
            phi = lam[m]
            flux = uu[0] * nx + uu[1] * ny
            for i in (0, 1):
                out[i] += w * flux * uu[i] * phi
        return [rho * v for v in out]


# ---------------------------------------------------------------------------
# Witnesses.
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class Mesh:
    name: str
    note: str
    rho: F
    nodes: tuple[Point, ...]
    tris: tuple[tuple[int, int, int], ...]
    dofs: tuple[Vel, ...]


def _divergence_free_sample(nodes) -> tuple[Vel, ...]:
    """Nodal values of an exactly divergence-free affine field:
    u = (1/2 + 3/2 x + 2 y, -4/5 + 5/4 x - 3/2 y), trace of the gradient zero."""
    return tuple(
        (
            F(1, 2) + F(3, 2) * x + F(2) * y,
            F(-4, 5) + F(5, 4) * x - F(3, 2) * y,
        )
        for (x, y) in nodes
    )


def witnesses() -> list[Mesh]:
    w1_nodes = ((F(1, 3), F(1, 2)), (F(7, 3), F(1, 4)), (F(1), F(5, 2)))
    w2_nodes = (
        (F(0), F(0)),
        (F(2), F(-1, 4)),
        (F(5, 2), F(3, 2)),
        (F(1, 3), F(2)),
    )
    w3_nodes = (
        (F(0), F(0)),
        (F(1), F(0)),
        (F(1), F(1)),
        (F(0), F(1)),
        (F(5, 8), F(3, 8)),
    )
    w4_nodes = ((F(-1, 2), F(1, 3)), (F(3, 2), F(-1, 4)), (F(1, 4), F(7, 4)))
    return [
        Mesh(
            name="w1",
            note="single affine triangle, open boundary, flux and divergence both live",
            rho=F(7, 5),
            nodes=w1_nodes,
            tris=((0, 1, 2),),
            dofs=((F(3, 2), F(-1, 2)), (F(-2, 3), F(5, 4)), (F(1, 4), F(7, 3))),
        ),
        Mesh(
            name="w2",
            note="two-triangle quadrilateral, one interior facet, open boundary",
            rho=F(11, 6),
            nodes=w2_nodes,
            tris=((0, 1, 2), (0, 2, 3)),
            dofs=(
                (F(1), F(-3, 4)),
                (F(-1, 2), F(2)),
                (F(5, 3), F(1, 3)),
                (F(-1, 4), F(-5, 6)),
            ),
        ),
        Mesh(
            name="w3",
            note="closed unit square, four triangles, complete zero velocity trace",
            rho=F(9, 8),
            nodes=w3_nodes,
            tris=((0, 1, 4), (1, 2, 4), (2, 3, 4), (3, 0, 4)),
            dofs=(
                (F(0), F(0)),
                (F(0), F(0)),
                (F(0), F(0)),
                (F(0), F(0)),
                (F(4, 3), F(-7, 5)),
            ),
        ),
        Mesh(
            name="w4",
            note="single triangle, exactly divergence-free velocity, nonzero flux",
            rho=F(4, 3),
            nodes=w4_nodes,
            tris=((0, 1, 2),),
            dofs=_divergence_free_sample(w4_nodes),
        ),
    ]


# ---------------------------------------------------------------------------
# Assembly.
# ---------------------------------------------------------------------------

TERMS = ("A", "C", "D", "B_bnd", "B_all")


def boundary_edge_keys(mesh: Mesh) -> set[frozenset[int]]:
    """Global edges incident to exactly one element are the domain boundary."""
    seen: dict[frozenset[int], int] = {}
    for tri in mesh.tris:
        for e in range(3):
            key = frozenset((tri[e], tri[(e + 1) % 3]))
            seen[key] = seen.get(key, 0) + 1
    for key, count in seen.items():
        if count > 2:
            raise ValueError(f"non-manifold edge {sorted(key)} in {mesh.name}")
    return {key for key, count in seen.items() if count == 1}


def assemble(mesh: Mesh, route, edge_rule=None) -> dict[tuple[int, int], dict[str, F]]:
    """Globally assembled rows: (node, component) -> {term: exact value}."""
    bnd = boundary_edge_keys(mesh)
    rows = {
        (m, i): {t: F(0) for t in TERMS} for m in range(len(mesh.nodes)) for i in (0, 1)
    }
    for tri in mesh.tris:
        verts = tuple(mesh.nodes[n] for n in tri)
        u_dofs = tuple(mesh.dofs[n] for n in tri)
        for ml in range(3):
            m = tri[ml]
            vol = route.element_volume_terms(verts, u_dofs, ml, mesh.rho)
            for i in (0, 1):
                for term in ("A", "C", "D"):
                    rows[(m, i)][term] += vol[term][i]
            for e in range(3):
                contrib = route.element_edge_term(
                    verts, u_dofs, ml, e, mesh.rho, edge_rule
                )
                on_boundary = frozenset((tri[e], tri[(e + 1) % 3])) in bnd
                for i in (0, 1):
                    rows[(m, i)]["B_all"] += contrib[i]
                    if on_boundary:
                        rows[(m, i)]["B_bnd"] += contrib[i]
    return rows


def row_key(mesh: Mesh, m: int, i: int) -> str:
    return f"{mesh.name}.phi{m}.u{i}"


def skew_minus_conservative(row: dict[str, F]) -> F:
    return (row["A"] + row["C"]) / 2 - row["C"]


def claimed_rhs(row: dict[str, F]) -> F:
    return row["B_bnd"] / 2 - row["D"] / 2


# ---------------------------------------------------------------------------
# Frozen expected values: (A, C, B_bnd, D, S - C), exact rationals.
# Emitted by `python3 oracle.py --emit-frozen` from the analytic route and
# independently reproduced by the quadrature route on every run. Frozen before
# implementation; the implementation lane must not edit this table.
# ---------------------------------------------------------------------------

FROZEN: dict[str, tuple[str, str, str, str, str]] = {
    "w1.phi0.u0": (
        "-178157/207360",
        "72359/103680",
        "-13643/8640",
        "-1519/69120",
        "-7175/9216",
    ),
    "w1.phi0.u1": (
        "45353/34560",
        "334061/207360",
        "-3325/10368",
        "-1519/69120",
        "-61943/414720",
    ),
    "w1.phi1.u0": (
        "-78743/207360",
        "-9527/25920",
        "-1631/103680",
        "-49/13824",
        "-2527/414720",
    ),
    "w1.phi1.u1": (
        "42847/34560",
        "9821/25920",
        "17087/20736",
        "-637/17280",
        "89257/207360",
    ),
    "w1.phi2.u0": (
        "-18011/25920",
        "-11417/34560",
        "-38969/103680",
        "-49/4320",
        "-37793/207360",
    ),
    "w1.phi2.u1": (
        "15113/8640",
        "-137543/69120",
        "127631/34560",
        "-637/13824",
        "86149/46080",
    ),
    "w2.phi0.u0": (
        "48455/41472",
        "118525/124416",
        "-319/432",
        "-14839/15552",
        "3355/31104",
    ),
    "w2.phi0.u1": (
        "30041/20736",
        "24563/62208",
        "13585/62208",
        "-1925/2304",
        "8195/15552",
    ),
    "w2.phi1.u0": (
        "23353/20736",
        "-11869/20736",
        "1441/1728",
        "-8965/10368",
        "17611/20736",
    ),
    "w2.phi1.u1": (
        "-30239/41472",
        "11561/10368",
        "-25597/6912",
        "-77099/41472",
        "-76483/82944",
    ),
    "w2.phi2.u0": (
        "1331/972",
        "-128381/62208",
        "141053/62208",
        "-1133/972",
        "213565/124416",
    ),
    "w2.phi2.u1": (
        "84139/62208",
        "-75361/124416",
        "24233/31104",
        "-146707/124416",
        "27071/27648",
    ),
    "w2.phi3.u0": (
        "132341/124416",
        "69817/41472",
        "-1265/6912",
        "13585/31104",
        "-38555/124416",
    ),
    "w2.phi3.u1": (
        "58465/124416",
        "-37499/41472",
        "14839/15552",
        "-26125/62208",
        "85481/124416",
    ),
    "w3.phi0.u0": ("-1/240", "-1/120", "0/1", "-1/240", "1/480"),
    "w3.phi0.u1": ("7/1600", "7/800", "0/1", "7/1600", "-7/3200"),
    "w3.phi1.u0": ("-41/240", "-41/120", "0/1", "-41/240", "41/480"),
    "w3.phi1.u1": ("287/1600", "287/800", "0/1", "287/1600", "-287/3200"),
    "w3.phi2.u0": ("1/240", "1/120", "0/1", "1/240", "-1/480"),
    "w3.phi2.u1": ("-7/1600", "-7/800", "0/1", "-7/1600", "7/3200"),
    "w3.phi3.u0": ("41/240", "41/120", "0/1", "41/240", "-41/480"),
    "w3.phi3.u1": ("-287/1600", "-287/800", "0/1", "-287/1600", "287/3200"),
    "w3.phi4.u0": ("0/1", "0/1", "0/1", "0/1", "0/1"),
    "w3.phi4.u1": ("0/1", "0/1", "0/1", "0/1", "0/1"),
    "w4.phi0.u0": (
        "2041/69120",
        "117965/20736",
        "-1173527/207360",
        "0/1",
        "-1173527/414720",
    ),
    "w4.phi0.u1": (
        "331427/103680",
        "-95693/46080",
        "437389/82944",
        "0/1",
        "437389/165888",
    ),
    "w4.phi1.u0": (
        "121361/69120",
        "-570643/77760",
        "5657393/622080",
        "0/1",
        "5657393/1244160",
    ),
    "w4.phi1.u1": (
        "558449/207360",
        "2798119/691200",
        "-2809867/2073600",
        "0/1",
        "-2809867/4147200",
    ),
    "w4.phi2.u0": ("23393/34560", "513097/311040", "-1891/1944", "0/1", "-1891/3888"),
    "w4.phi2.u1": (
        "916409/207360",
        "-340681/172800",
        "6626131/1036800",
        "0/1",
        "6626131/2073600",
    ),
}

FROZEN_TERM_ORDER = ("A", "C", "B_bnd", "D", "SminusC")


def fs(x: F) -> str:
    return f"{x.numerator}/{x.denominator}"


def emit_frozen(meshes: list[Mesh]) -> None:
    route = AnalyticRoute()
    print("FROZEN: dict[str, tuple[str, str, str, str, str]] = {")
    for mesh in meshes:
        rows = assemble(mesh, route)
        for m in range(len(mesh.nodes)):
            for i in (0, 1):
                row = rows[(m, i)]
                vals = (
                    row["A"],
                    row["C"],
                    row["B_bnd"],
                    row["D"],
                    skew_minus_conservative(row),
                )
                joined = ", ".join(f'"{fs(v)}"' for v in vals)
                print(f'    "{row_key(mesh, m, i)}": ({joined}),')
    print("}")


# ---------------------------------------------------------------------------
# Check harness.
# ---------------------------------------------------------------------------


class Checks:
    def __init__(self) -> None:
        self.passed = 0
        self.failed = 0

    def check(self, key: str, ok: bool, detail: str = "") -> bool:
        if ok:
            self.passed += 1
            print(f"{key}=pass")
        else:
            self.failed += 1
            suffix = f" # {detail}" if detail else ""
            print(f"{key}=FAIL{suffix}")
        return ok

    def info(self, key: str, value) -> None:
        print(f"{key}={value}")


# ---------------------------------------------------------------------------
# Quadrature rule certification, independent of the physics rows.
# ---------------------------------------------------------------------------

CERT_TRI = ((F(1, 5), F(-1, 3)), (F(9, 4), F(1, 6)), (F(2, 3), F(11, 5)))


def certify_rules(ck: Checks) -> None:
    analytic = AnalyticRoute()
    quad = QuadratureRoute()
    verts = CERT_TRI
    area = quad.area(verts)

    def quad_tri(poly_exp, rule):
        total = F(0)
        for lam, w in rule:
            x = sum((lam[k] * verts[k][0] for k in range(3)), F(0))
            y = sum((lam[k] * verts[k][1] for k in range(3)), F(0))
            total += w * x ** poly_exp[0] * y ** poly_exp[1]
        return area * total

    exact_to_3 = True
    for deg in range(4):
        for a in range(deg + 1):
            exp = (a, deg - a)
            ref = analytic.tri_int({exp: F(1)}, verts)
            if quad_tri(exp, TRI_D3) != ref:
                exact_to_3 = False
    ck.check("rule.tri_degree3_exact", exact_to_3)
    quartic_gap = any(
        quad_tri((a, 4 - a), TRI_D3) != analytic.tri_int({(a, 4 - a): F(1)}, verts)
        for a in range(5)
    )
    ck.check("rule.tri_degree4_inexact", quartic_gap, "rule is certified to degree 3")

    p, q = (F(-3, 5), F(1, 4)), (F(7, 3), F(2))

    def quad_edge(exp, rule):
        total = F(0)
        for t, w in rule:
            x = p[0] + t * (q[0] - p[0])
            y = p[1] + t * (q[1] - p[1])
            total += w * x ** exp[0] * y ** exp[1]
        return total

    for label, rule, degree in (
        ("simpson", EDGE_SIMPSON, 3),
        ("simpson38", EDGE_SIMPSON38, 3),
        ("midpoint", EDGE_MIDPOINT, 1),
        ("trapezoid", EDGE_TRAPEZOID, 1),
    ):
        ok = True
        for deg in range(degree + 1):
            for a in range(deg + 1):
                exp = (a, deg - a)
                if quad_edge(exp, rule) != analytic.seg_int({exp: F(1)}, p, q):
                    ok = False
        ck.check(f"rule.edge_{label}_degree{degree}_exact", ok)
    for label, rule in (("midpoint", EDGE_MIDPOINT), ("trapezoid", EDGE_TRAPEZOID)):
        cubic_gap = any(
            quad_edge((a, 3 - a), rule) != analytic.seg_int({(a, 3 - a): F(1)}, p, q)
            for a in range(4)
        )
        ck.check(
            f"rule.edge_{label}_fails_on_cubic",
            cubic_gap,
            "a degree-1 facet rule must not integrate the cubic trace",
        )


# ---------------------------------------------------------------------------
# Per-witness audit.
# ---------------------------------------------------------------------------


@dataclass
class WitnessData:
    mesh: Mesh
    analytic: dict
    quad: dict
    quad38: dict
    quad_mid: dict
    quad_trap: dict


def compute(mesh: Mesh) -> WitnessData:
    a_route = AnalyticRoute()
    q_route = QuadratureRoute()
    return WitnessData(
        mesh=mesh,
        analytic=assemble(mesh, a_route),
        quad=assemble(mesh, q_route),
        quad38=assemble(mesh, q_route, EDGE_SIMPSON38),
        quad_mid=assemble(mesh, q_route, EDGE_MIDPOINT),
        quad_trap=assemble(mesh, q_route, EDGE_TRAPEZOID),
    )


def audit_geometry(data: WitnessData, ck: Checks) -> None:
    mesh = data.mesh
    a_route, q_route = AnalyticRoute(), QuadratureRoute()
    ccw = all(signed_det(tuple(mesh.nodes[n] for n in tri)) > 0 for tri in mesh.tris)
    ck.check(f"{mesh.name}.elements_ccw", ccw)
    if not ccw:
        return
    agree = True
    for tri in mesh.tris:
        verts = tuple(mesh.nodes[n] for n in tri)
        for e in range(3):
            if a_route.scaled_outward_normal(verts, e) != q_route.scaled_outward_normal(
                verts, e
            ):
                agree = False
    ck.check(
        f"{mesh.name}.outward_normal_derivations_agree",
        agree,
        "CCW-orientation rule vs opposite-vertex sign test",
    )


def audit_rows(data: WitnessData, ck: Checks) -> None:
    mesh = data.mesh
    for m in range(len(mesh.nodes)):
        for i in (0, 1):
            key = row_key(mesh, m, i)
            ana, qua = data.analytic[(m, i)], data.quad[(m, i)]
            ck.check(
                f"{key}.routes_agree",
                all(ana[t] == qua[t] for t in TERMS),
                "analytic vs degree-3 quadrature",
            )
            ck.check(
                f"{key}.simpson38_agrees",
                all(data.quad38[(m, i)][t] == ana[t] for t in TERMS),
                "any degree-3 facet rule must reproduce the boundary term",
            )
            ck.check(
                f"{key}.interior_facets_cancel",
                ana["B_all"] == ana["B_bnd"],
                "element-edge sum must reduce to the domain boundary",
            )
            lhs = skew_minus_conservative(ana)
            ck.check(
                f"{key}.identity",
                lhs == claimed_rhs(ana),
                f"S-C={fs(lhs)} vs B/2-D/2={fs(claimed_rhs(ana))}",
            )
            frozen = FROZEN.get(key)
            if frozen is None:
                ck.check(f"{key}.frozen_present", False, "no frozen expected value")
                continue
            actual = (ana["A"], ana["C"], ana["B_bnd"], ana["D"], lhs)
            mismatches = [
                f"{name}: got {fs(got)} want {want}"
                for name, got, want in zip(FROZEN_TERM_ORDER, actual, frozen)
                if fs(got) != want
            ]
            ck.check(f"{key}.matches_frozen", not mismatches, "; ".join(mismatches))


def emit_values(data: WitnessData) -> None:
    mesh = data.mesh
    for m in range(len(mesh.nodes)):
        for i in (0, 1):
            key, row = row_key(mesh, m, i), data.analytic[(m, i)]
            for name, val in (
                ("A", row["A"]),
                ("C", row["C"]),
                ("S", (row["A"] + row["C"]) / 2),
                ("B", row["B_bnd"]),
                ("D", row["D"]),
                ("SminusC", skew_minus_conservative(row)),
            ):
                print(f"value.{key}.{name}={fs(val)}")


def audit_structure(data: WitnessData, ck: Checks) -> None:
    """Witness-specific structural properties that make the audit non-vacuous."""
    mesh, rows = data.mesh, data.analytic
    live = [
        row_key(mesh, m, i)
        for m in range(len(mesh.nodes))
        for i in (0, 1)
        if rows[(m, i)]["B_bnd"] != 0
        and rows[(m, i)]["D"] != 0
        and rows[(m, i)]["B_bnd"] != rows[(m, i)]["D"]
    ]
    if mesh.name in ("w1", "w2"):
        ck.check(
            f"{mesh.name}.flux_and_divergence_both_live",
            bool(live),
            "need a row with B!=0, D!=0 and no cancellation",
        )
        ck.info(f"{mesh.name}.live_row_count", len(live))
    if mesh.name == "w3":
        zero_trace = all(
            rows[(m, i)]["B_bnd"] == 0 and rows[(m, i)]["B_all"] == 0
            for m in range(len(mesh.nodes))
            for i in (0, 1)
        )
        ck.check(f"{mesh.name}.zero_normal_flux_boundary", zero_trace)
        reduction = all(
            skew_minus_conservative(rows[(m, i)]) == -rows[(m, i)]["D"] / 2
            for m in range(len(mesh.nodes))
            for i in (0, 1)
        )
        ck.check(
            f"{mesh.name}.closed_boundary_reduction",
            reduction,
            "S-C must reduce to -D/2 when u.n vanishes on the whole boundary",
        )
        nonvacuous = any(
            rows[(m, i)]["D"] != 0 for m in range(len(mesh.nodes)) for i in (0, 1)
        )
        ck.check(
            f"{mesh.name}.reduction_not_vacuous", nonvacuous, "some D must be nonzero"
        )
    if mesh.name == "w4":
        div_free = all(
            rows[(m, i)]["D"] == 0 for m in range(len(mesh.nodes)) for i in (0, 1)
        )
        ck.check(f"{mesh.name}.divergence_free", div_free)
        isolated = all(
            skew_minus_conservative(rows[(m, i)]) == rows[(m, i)]["B_bnd"] / 2
            for m in range(len(mesh.nodes))
            for i in (0, 1)
        )
        ck.check(
            f"{mesh.name}.boundary_term_isolated",
            isolated,
            "S-C must reduce to B/2 for a divergence-free velocity",
        )
        flux = any(
            rows[(m, i)]["B_bnd"] != 0 for m in range(len(mesh.nodes)) for i in (0, 1)
        )
        ck.check(f"{mesh.name}.flux_nonzero", flux, "isolation must not be vacuous")


# ---------------------------------------------------------------------------
# Falsifiers.
# ---------------------------------------------------------------------------


def falsifier_variants(data: WitnessData, m: int, i: int) -> dict[str, F]:
    row = data.analytic[(m, i)]
    return {
        "omit_boundary": -row["D"] / 2,
        "reversed_normal": -row["B_bnd"] / 2 - row["D"] / 2,
        "omit_divergence": row["B_bnd"] / 2,
        "midpoint_boundary": data.quad_mid[(m, i)]["B_bnd"] / 2 - row["D"] / 2,
        "trapezoid_boundary": data.quad_trap[(m, i)]["B_bnd"] / 2 - row["D"] / 2,
    }


FALSIFIERS = (
    "omit_boundary",
    "reversed_normal",
    "omit_divergence",
    "midpoint_boundary",
    "trapezoid_boundary",
)


def audit_falsifiers(all_data: list[WitnessData], ck: Checks) -> None:
    caught: dict[str, list[str]] = {name: [] for name in FALSIFIERS}
    for data in all_data:
        mesh = data.mesh
        per_witness: dict[str, int] = {name: 0 for name in FALSIFIERS}
        for m in range(len(mesh.nodes)):
            for i in (0, 1):
                truth = skew_minus_conservative(data.analytic[(m, i)])
                for name, wrong in falsifier_variants(data, m, i).items():
                    if wrong != truth:
                        per_witness[name] += 1
                        caught[name].append(row_key(mesh, m, i))
        for name in FALSIFIERS:
            ck.info(f"falsifier.{name}.{mesh.name}.rows_detecting", per_witness[name])
    for name in FALSIFIERS:
        ck.check(
            f"falsifier.{name}.detected",
            bool(caught[name]),
            "no witness distinguishes this corruption",
        )
        if caught[name]:
            ck.info(f"falsifier.{name}.first_detecting_row", caught[name][0])
            ck.info(f"falsifier.{name}.total_detecting_rows", len(caught[name]))


def audit_falsifier_distinctness(all_data: list[WitnessData], ck: Checks) -> None:
    """Two corruptions that always produce the same number would not be
    separately falsifiable. Require every pair to differ somewhere."""
    pairs_ok = True
    detail = ""
    for a_idx, name_a in enumerate(FALSIFIERS):
        for name_b in FALSIFIERS[a_idx + 1 :]:
            differs = False
            for data in all_data:
                for m in range(len(data.mesh.nodes)):
                    for i in (0, 1):
                        v = falsifier_variants(data, m, i)
                        if v[name_a] != v[name_b]:
                            differs = True
            if not differs:
                pairs_ok = False
                detail = f"{name_a} indistinguishable from {name_b}"
    ck.check("falsifier.pairwise_distinct", pairs_ok, detail)


# ---------------------------------------------------------------------------
# Entry point.
# ---------------------------------------------------------------------------


def main(argv: list[str]) -> int:
    meshes = witnesses()
    if "--emit-frozen" in argv:
        emit_frozen(meshes)
        return 0

    ck = Checks()
    print("oracle.issue=nkiyohara/eqiora#124")
    print(
        "oracle.claim=S-C=1/2*int_dOmega rho(u.n)u_i phi ds-1/2*int_Omega rho div(u)u_i phi dx"
    )
    print("oracle.arithmetic=exact-rational")
    print("oracle.tolerance=none")
    print(f"oracle.routes={AnalyticRoute.name},{QuadratureRoute.name}")
    print(f"oracle.witnesses={','.join(m.name for m in meshes)}")

    all_data = [compute(mesh) for mesh in meshes]
    for data in all_data:
        mesh = data.mesh
        ck.info(f"{mesh.name}.note", mesh.note)
        ck.info(f"{mesh.name}.rho", fs(mesh.rho))
        ck.info(f"{mesh.name}.elements", len(mesh.tris))
        ck.info(f"{mesh.name}.nodes", len(mesh.nodes))
        ck.info(f"{mesh.name}.boundary_facets", len(boundary_edge_keys(mesh)))
        emit_values(data)
        audit_geometry(data, ck)
        audit_rows(data, ck)
        audit_structure(data, ck)

    certify_rules(ck)
    audit_falsifiers(all_data, ck)
    audit_falsifier_distinctness(all_data, ck)

    rows_total = sum(2 * len(d.mesh.nodes) for d in all_data)
    ck.info("rows.total", rows_total)
    ck.info("checks.passed", ck.passed)
    ck.info("checks.failed", ck.failed)
    status = "pass" if ck.failed == 0 else "FAIL"
    print(f"oracle.status={status}")
    return 0 if ck.failed == 0 else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
