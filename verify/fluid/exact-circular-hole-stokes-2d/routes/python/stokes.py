"""Closed-form affine MINI/P1 Stokes assembly, solve and recovery.

Every cell block below is a closed-form barycentric integral. There is no
quadrature loop anywhere in this module: on an affine triangle every integrand
of the admitted weak form is a barycentric monomial, and

    integral_T lambda_0^a lambda_1^b lambda_2^c dA
        = 2 |T| a! b! c! / (a + b + c + 2)!

evaluates each one exactly. See ``README.md`` for the derivation of every block.

The module is written for one non-implementing oracle route. It knows nothing
about Eqiora and calls nothing from it.
"""

from __future__ import annotations

from dataclasses import dataclass, field

import mpmath

# ---------------------------------------------------------------------------
# Local closed-form blocks
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class CellGeometry:
    """Affine triangle: positive area and the three constant P1 gradients."""

    area: mpmath.mpf
    grad: tuple[tuple[mpmath.mpf, mpmath.mpf], ...]


def cell_geometry(p0, p1, p2) -> CellGeometry:
    a2 = (p1[0] - p0[0]) * (p2[1] - p0[1]) - (p2[0] - p0[0]) * (p1[1] - p0[1])
    if a2 <= 0:
        raise ValueError("cell is not positively oriented")
    grad = (
        ((p1[1] - p2[1]) / a2, (p2[0] - p1[0]) / a2),
        ((p2[1] - p0[1]) / a2, (p0[0] - p2[0]) / a2),
        ((p0[1] - p1[1]) / a2, (p1[0] - p0[0]) / a2),
    )
    return CellGeometry(area=a2 / 2, grad=grad)


def viscous_p1(geo: CellGeometry, mu, symmetric_gradient: bool = True):
    """``mu * (delta_de * int grad(phi_j).grad(phi_i) + int d_e phi_j d_d phi_i)``.

    Row is ``(i, e)`` (test), column is ``(j, d)`` (trial). With
    ``symmetric_gradient=False`` the second term is dropped, which is the
    vector-Laplacian falsifier ``mu * int grad(u):grad(v)``.
    """
    g, area = geo.grad, geo.area
    block = [
        [[[mpmath.mpf(0)] * 2 for _ in range(3)] for _ in range(2)] for _ in range(3)
    ]
    for i in range(3):
        for j in range(3):
            dot = g[j][0] * g[i][0] + g[j][1] * g[i][1]
            for e in range(2):
                for d in range(2):
                    value = dot if e == d else mpmath.mpf(0)
                    if symmetric_gradient:
                        value = value + g[j][e] * g[i][d]
                    block[i][e][j][d] = mu * area * value
    return block


def viscous_bubble(
    geo: CellGeometry, mu, bubble_scale, symmetric_gradient: bool = True
):
    """Bubble/bubble viscous block for ``beta = bubble_scale * l0 l1 l2``.

    ``int d_e beta d_d beta = (bubble_scale^2 / 180) * |T| * sum_i g_i[e] g_i[d]``
    because ``int m_i m_j = |T|/90`` for ``i == j``, ``|T|/180`` otherwise and
    ``sum_i g_i = 0``. For ``bubble_scale = 27`` the factor is ``81 |T| / 20``.
    """
    g, area = geo.grad, geo.area
    m = [
        [sum((g[i][e] * g[i][d] for i in range(3)), mpmath.mpf(0)) for d in range(2)]
        for e in range(2)
    ]
    trace = m[0][0] + m[1][1]
    factor = bubble_scale * bubble_scale * area / 180
    block = [[mpmath.mpf(0)] * 2 for _ in range(2)]
    for e in range(2):
        for d in range(2):
            value = trace if e == d else mpmath.mpf(0)
            if symmetric_gradient:
                value = value + m[e][d]
            block[e][d] = mu * factor * value
    return block


def coupling_p1(geo: CellGeometry):
    """``c(phi_j e_d, lambda_m) = -int lambda_m d_d phi_j = -(|T|/3) g_j[d]``.

    Independent of the pressure index ``m`` because ``d_d lambda_j`` is constant.
    """
    g, area = geo.grad, geo.area
    return [[-(area / 3) * g[j][d] for d in range(2)] for j in range(3)]


def coupling_bubble(geo: CellGeometry, bubble_scale):
    """``c(beta e_d, lambda_m) = -int lambda_m d_d beta = (bubble_scale |T| / 60) g_m[d]``.

    ``int lambda_m m_i`` is ``|T|/60`` for ``i == m`` and ``|T|/30`` otherwise,
    and ``sum_{i != m} g_i = -g_m``. For ``bubble_scale = 27`` this is
    ``(9 |T| / 20) g_m[d]``.
    """
    g, area = geo.grad, geo.area
    factor = bubble_scale * area / 60
    return [[factor * g[m][d] for d in range(2)] for m in range(3)]


# ---------------------------------------------------------------------------
# Global assembly
# ---------------------------------------------------------------------------


@dataclass
class Formulation:
    """Every knob a falsifier may flip. Defaults are the frozen formulation."""

    symmetric_gradient: bool = True
    bubble_scale: int = 27
    include_bubble: bool = True
    coupling_sign_momentum: int = 1
    coupling_sign_continuity: int = 1


@dataclass
class System:
    n_vertices: int
    n_cells: int
    matrix: dict[int, dict[int, mpmath.mpf]] = field(default_factory=dict)
    load: dict[int, mpmath.mpf] = field(default_factory=dict)

    def add(self, row: int, col: int, value) -> None:
        if value == 0:
            return
        self.matrix.setdefault(row, {})
        self.matrix[row][col] = self.matrix[row].get(col, mpmath.mpf(0)) + value

    def add_load(self, row: int, value) -> None:
        if value == 0:
            return
        self.load[row] = self.load.get(row, mpmath.mpf(0)) + value

    # degree-of-freedom layout
    def vel_p1(self, vertex: int, comp: int) -> int:
        return 2 * vertex + comp

    def vel_bubble(self, cell: int, comp: int) -> int:
        return 2 * self.n_vertices + 2 * cell + comp

    def pressure(self, vertex: int) -> int:
        return 2 * self.n_vertices + 2 * self.n_cells + vertex

    @property
    def size(self) -> int:
        return 2 * self.n_vertices + 2 * self.n_cells + self.n_vertices

    def apply(self, x: list) -> list:
        out = [mpmath.mpf(0)] * self.size
        for row, cols in self.matrix.items():
            acc = mpmath.mpf(0)
            for col, value in cols.items():
                acc += value * x[col]
            out[row] = acc
        return out


def assemble(vertices, cells, mu, form: Formulation) -> System:
    """Full (unreduced) mixed system: momentum rows, then continuity rows."""
    system = System(n_vertices=len(vertices), n_cells=len(cells))
    for c, cell in enumerate(cells):
        geo = cell_geometry(*[vertices[k] for k in cell])
        kp1 = viscous_p1(geo, mu, form.symmetric_gradient)
        cp1 = coupling_p1(geo)
        for i in range(3):
            for e in range(2):
                row = system.vel_p1(cell[i], e)
                for j in range(3):
                    for d in range(2):
                        system.add(row, system.vel_p1(cell[j], d), kp1[i][e][j][d])
                for m in range(3):
                    system.add(
                        row,
                        system.pressure(cell[m]),
                        form.coupling_sign_momentum * cp1[i][e],
                    )
        # continuity rows: c(u, q) = -int q div(u); transpose of the momentum coupling
        for m in range(3):
            row = system.pressure(cell[m])
            for j in range(3):
                for d in range(2):
                    system.add(
                        row,
                        system.vel_p1(cell[j], d),
                        form.coupling_sign_continuity * cp1[j][d],
                    )
        if form.include_bubble:
            kb = viscous_bubble(geo, mu, form.bubble_scale, form.symmetric_gradient)
            cb = coupling_bubble(geo, form.bubble_scale)
            for e in range(2):
                row = system.vel_bubble(c, e)
                for d in range(2):
                    system.add(row, system.vel_bubble(c, d), kb[e][d])
                for m in range(3):
                    system.add(
                        row,
                        system.pressure(cell[m]),
                        form.coupling_sign_momentum * cb[m][e],
                    )
            for m in range(3):
                row = system.pressure(cell[m])
                for d in range(2):
                    system.add(
                        row,
                        system.vel_bubble(c, d),
                        form.coupling_sign_continuity * cb[m][d],
                    )
    return system


def add_facet_traction(system: System, vertices, facets, traction) -> list:
    """Constant traction on a facet: ``b_endpoint = length * traction / 2``.

    This is the degree-one midpoint rule on the two endpoint P1 bases; bubble,
    pressure and interior rows receive no facet load. Returns the integrated
    applied traction resultant per unit out-of-plane length.
    """
    total = [mpmath.mpf(0), mpmath.mpf(0)]
    for a, b in facets:
        pa, pb = vertices[a], vertices[b]
        length = mpmath.sqrt((pb[0] - pa[0]) ** 2 + (pb[1] - pa[1]) ** 2)
        for comp in range(2):
            half = length * traction[comp] / 2
            system.add_load(system.vel_p1(a, comp), half)
            system.add_load(system.vel_p1(b, comp), half)
            total[comp] += length * traction[comp]
    return total


# ---------------------------------------------------------------------------
# Reduction and solve
# ---------------------------------------------------------------------------


def build_reduced(system: System, prescribed: dict[int, mpmath.mpf]):
    """Eliminate the prescribed rows and columns; no gauge row is ever added.

    Returns ``(matrix, rhs, free)``. The right-hand side carries the essential
    lifting ``-A[free, prescribed] u_prescribed`` together with the body force
    and applied facet traction.
    """
    free = [i for i in range(system.size) if i not in prescribed]
    position = {dof: k for k, dof in enumerate(free)}
    n = len(free)
    a = mpmath.matrix(n, n)
    rhs = mpmath.matrix(n, 1)
    for k, row in enumerate(free):
        rhs[k] = system.load.get(row, mpmath.mpf(0))
        for col, value in system.matrix.get(row, {}).items():
            if col in prescribed:
                rhs[k] -= value * prescribed[col]
            else:
                a[k, position[col]] = value
    return a, rhs, free


def restore(system: System, prescribed, free, solution) -> list:
    x = [mpmath.mpf(0)] * system.size
    for dof, value in prescribed.items():
        x[dof] = value
    for k, dof in enumerate(free):
        x[dof] = solution[k]
    return x


def solve_reduced(system: System, prescribed: dict[int, mpmath.mpf]):
    """Direct dense LU on the reduced mixed system at the ambient precision."""
    a, rhs, free = build_reduced(system, prescribed)
    solution = mpmath.lu_solve(a, rhs)
    return restore(system, prescribed, free, solution), free, a, rhs


def condensed_solve(system: System, prescribed: dict[int, mpmath.mpf]):
    """Independent second route: exact static condensation of the bubble block.

    Bubble rows couple only to their own cell's bubble and pressure unknowns
    (the P1/bubble viscous coupling vanishes because ``sum_i grad(lambda_i) = 0``)
    and carry no load, so ``u_b = -Kb^-1 Cb p`` exactly. Substituting leaves a
    system in the free P1 velocity and pressure unknowns only.
    """
    n_c = system.n_cells
    bubble_dofs = {system.vel_bubble(c, d) for c in range(n_c) for d in range(2)}
    outer = [
        i for i in range(system.size) if i not in prescribed and i not in bubble_dofs
    ]
    position = {dof: k for k, dof in enumerate(outer)}
    n = len(outer)
    a = mpmath.matrix(n, n)
    rhs = mpmath.matrix(n, 1)
    for k, row in enumerate(outer):
        rhs[k] = system.load.get(row, mpmath.mpf(0))
        for col, value in system.matrix.get(row, {}).items():
            if col in bubble_dofs:
                continue
            if col in prescribed:
                rhs[k] -= value * prescribed[col]
            else:
                a[k, position[col]] = value

    schur = {}
    for c in range(n_c):
        rows = [system.vel_bubble(c, d) for d in range(2)]
        if any(system.load.get(r, mpmath.mpf(0)) != 0 for r in rows):
            raise ValueError("bubble rows carry a load; this condensation assumes none")
        kb = mpmath.matrix(2, 2)
        for e in range(2):
            for d in range(2):
                kb[e, d] = system.matrix.get(rows[e], {}).get(rows[d], mpmath.mpf(0))
        cols = sorted(
            {
                col
                for r in rows
                for col in system.matrix.get(r, {})
                if col not in bubble_dofs
            }
        )
        cb = mpmath.matrix(2, len(cols))
        for e in range(2):
            for j, col in enumerate(cols):
                cb[e, j] = system.matrix.get(rows[e], {}).get(col, mpmath.mpf(0))
        kb_inv = kb**-1
        schur[c] = (rows, cols, kb_inv, cb)
        contribution = cb.T * kb_inv * cb
        for i, ci in enumerate(cols):
            for j, cj in enumerate(cols):
                if ci in prescribed or cj in prescribed:
                    continue
                a[position[ci], position[cj]] -= contribution[i, j]

    solution = mpmath.lu_solve(a, rhs)
    x = [mpmath.mpf(0)] * system.size
    for dof, value in prescribed.items():
        x[dof] = value
    for k, dof in enumerate(outer):
        x[dof] = solution[k]
    for c in range(n_c):
        rows, cols, kb_inv, cb = schur[c]
        rhs_c = mpmath.matrix(2, 1)
        for e in range(2):
            rhs_c[e] = -sum(
                (cb[e, j] * x[col] for j, col in enumerate(cols)), mpmath.mpf(0)
            )
        ub = kb_inv * rhs_c
        for e in range(2):
            x[rows[e]] = ub[e]
    return x
