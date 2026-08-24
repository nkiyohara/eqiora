"""Elevated-precision affine MINI/P1 Stokes algebra for the Gmsh oracle.

The cell formulas are the already accepted exact barycentric formulation from
``exact-circular-hole-stokes-2d``.  This route reuses that formulation; it does
not reuse any mesh-dependent value.  All new values are assembled from the
parsed Gmsh 4.1 artifact.
"""

from __future__ import annotations

from dataclasses import dataclass, field

import mpmath as mp
import numpy as np
import scipy
import scipy.sparse
import scipy.sparse.linalg


@dataclass(frozen=True)
class CellGeometry:
    area: mp.mpf
    grad: tuple[tuple[mp.mpf, mp.mpf], ...]


def cell_geometry(p0, p1, p2) -> CellGeometry:
    twice_area = (p1[0] - p0[0]) * (p2[1] - p0[1]) - (p2[0] - p0[0]) * (p1[1] - p0[1])
    if twice_area <= 0:
        raise ValueError("triangle is not positively oriented")
    gradients = (
        ((p1[1] - p2[1]) / twice_area, (p2[0] - p1[0]) / twice_area),
        ((p2[1] - p0[1]) / twice_area, (p0[0] - p2[0]) / twice_area),
        ((p0[1] - p1[1]) / twice_area, (p1[0] - p0[0]) / twice_area),
    )
    return CellGeometry(twice_area / 2, gradients)


def viscous_p1(geo: CellGeometry, mu):
    """Return ``2 mu int sym(grad u):sym(grad v)`` on P1 vector bases."""
    block = [[[[mp.mpf(0)] * 2 for _ in range(3)] for _ in range(2)] for _ in range(3)]
    for i in range(3):
        for j in range(3):
            dot = sum(geo.grad[j][k] * geo.grad[i][k] for k in range(2))
            for test_component in range(2):
                for trial_component in range(2):
                    value = dot if test_component == trial_component else mp.mpf(0)
                    value += geo.grad[j][test_component] * geo.grad[i][trial_component]
                    block[i][test_component][j][trial_component] = mu * geo.area * value
    return block


def viscous_bubble(geo: CellGeometry, mu):
    """Bubble block for the normalized basis ``beta = 27 l0 l1 l2``."""
    gram = [
        [sum(geo.grad[i][e] * geo.grad[i][d] for i in range(3)) for d in range(2)]
        for e in range(2)
    ]
    trace = gram[0][0] + gram[1][1]
    factor = mu * mp.mpf(81) * geo.area / 20
    return [
        [factor * ((trace if e == d else mp.mpf(0)) + gram[e][d]) for d in range(2)]
        for e in range(2)
    ]


def coupling_p1(geo: CellGeometry):
    """Return ``-int lambda_m div(lambda_j e_d)`` (independent of ``m``)."""
    return [[-(geo.area / 3) * geo.grad[j][d] for d in range(2)] for j in range(3)]


def coupling_bubble(geo: CellGeometry):
    """Return ``-int lambda_m div(beta e_d) = 9 |T| grad(lambda_m)_d / 20``."""
    return [
        [mp.mpf(9) * geo.area * geo.grad[m][d] / 20 for d in range(2)] for m in range(3)
    ]


@dataclass
class System:
    n_vertices: int
    n_cells: int
    matrix: dict[int, dict[int, mp.mpf]] = field(default_factory=dict)
    load: dict[int, mp.mpf] = field(default_factory=dict)

    def add(self, row: int, col: int, value) -> None:
        if value == 0:
            return
        columns = self.matrix.setdefault(row, {})
        columns[col] = columns.get(col, mp.mpf(0)) + value

    def add_load(self, row: int, value) -> None:
        if value != 0:
            self.load[row] = self.load.get(row, mp.mpf(0)) + value

    def vel_p1(self, vertex: int, component: int) -> int:
        return 2 * vertex + component

    def vel_bubble(self, cell: int, component: int) -> int:
        return 2 * self.n_vertices + 2 * cell + component

    def pressure(self, vertex: int) -> int:
        return 2 * self.n_vertices + 2 * self.n_cells + vertex

    @property
    def size(self) -> int:
        return 3 * self.n_vertices + 2 * self.n_cells

    def apply(self, vector) -> list[mp.mpf]:
        result = [mp.mpf(0)] * self.size
        for row, columns in self.matrix.items():
            result[row] = sum(
                (value * vector[column] for column, value in columns.items()),
                mp.mpf(0),
            )
        return result


def assemble(vertices, cells, mu) -> System:
    system = System(len(vertices), len(cells))
    for cell_index, cell in enumerate(cells):
        geo = cell_geometry(*(vertices[vertex] for vertex in cell))
        p1_viscous = viscous_p1(geo, mu)
        p1_coupling = coupling_p1(geo)
        bubble_viscous = viscous_bubble(geo, mu)
        bubble_coupling = coupling_bubble(geo)

        for i in range(3):
            for e in range(2):
                row = system.vel_p1(cell[i], e)
                for j in range(3):
                    for d in range(2):
                        system.add(
                            row,
                            system.vel_p1(cell[j], d),
                            p1_viscous[i][e][j][d],
                        )
                for m in range(3):
                    system.add(
                        row,
                        system.pressure(cell[m]),
                        p1_coupling[i][e],
                    )

        for m in range(3):
            pressure_row = system.pressure(cell[m])
            for j in range(3):
                for d in range(2):
                    system.add(
                        pressure_row,
                        system.vel_p1(cell[j], d),
                        p1_coupling[j][d],
                    )

        for e in range(2):
            bubble_row = system.vel_bubble(cell_index, e)
            for d in range(2):
                system.add(
                    bubble_row,
                    system.vel_bubble(cell_index, d),
                    bubble_viscous[e][d],
                )
            for m in range(3):
                system.add(
                    bubble_row,
                    system.pressure(cell[m]),
                    bubble_coupling[m][e],
                )
        for m in range(3):
            pressure_row = system.pressure(cell[m])
            for d in range(2):
                system.add(
                    pressure_row,
                    system.vel_bubble(cell_index, d),
                    bubble_coupling[m][d],
                )
    return system


def add_constant_traction(system: System, vertices, facets, traction):
    total = [mp.mpf(0), mp.mpf(0)]
    for a, b in facets:
        pa, pb = vertices[a], vertices[b]
        length = mp.sqrt((pb[0] - pa[0]) ** 2 + (pb[1] - pa[1]) ** 2)
        for component in range(2):
            half = length * traction[component] / 2
            system.add_load(system.vel_p1(a, component), half)
            system.add_load(system.vel_p1(b, component), half)
            total[component] += length * traction[component]
    return total


def reduced_inventory(system: System, prescribed):
    free = [dof for dof in range(system.size) if dof not in prescribed]
    rhs = []
    for row in free:
        value = system.load.get(row, mp.mpf(0))
        for column, coefficient in system.matrix.get(row, {}).items():
            if column in prescribed:
                value -= coefficient * prescribed[column]
        rhs.append(value)
    return free, rhs


def condensed_solve(system: System, prescribed):
    """Eliminate bubbles, then use f64 sparse LU only as a refinement preconditioner.

    The accepted Gmsh mesh leaves a 1760-row condensed system.  Its answer is
    therefore accumulated in ambient ``mpmath`` precision by residual-based
    iterative refinement.  SciPy/SuperLU supplies each correction, while every
    residual and update is independently reapplied to the elevated-precision
    sparse matrix.
    """
    bubble = {
        system.vel_bubble(cell, component)
        for cell in range(system.n_cells)
        for component in range(2)
    }
    outer = [
        dof for dof in range(system.size) if dof not in prescribed and dof not in bubble
    ]
    position = {dof: index for index, dof in enumerate(outer)}
    matrix: dict[int, dict[int, mp.mpf]] = {}
    rhs = [mp.mpf(0)] * len(outer)

    def add(row: int, column: int, value) -> None:
        if value == 0:
            return
        columns = matrix.setdefault(row, {})
        columns[column] = columns.get(column, mp.mpf(0)) + value

    for i, row in enumerate(outer):
        rhs[i] = system.load.get(row, mp.mpf(0))
        for column, value in system.matrix.get(row, {}).items():
            if column in prescribed:
                rhs[i] -= value * prescribed[column]
            elif column not in bubble:
                add(i, position[column], value)

    reconstruction = []
    for cell in range(system.n_cells):
        rows = [system.vel_bubble(cell, component) for component in range(2)]
        kb = mp.matrix(2, 2)
        for e in range(2):
            for d in range(2):
                kb[e, d] = system.matrix[rows[e]][rows[d]]
        columns = sorted(
            {
                column
                for row in rows
                for column in system.matrix[row]
                if column not in bubble
            }
        )
        cb = mp.matrix(2, len(columns))
        for e, row in enumerate(rows):
            for j, column in enumerate(columns):
                cb[e, j] = system.matrix[row].get(column, mp.mpf(0))
        kb_inverse = kb**-1
        schur = cb.T * kb_inverse * cb
        # The Schur complement is mathematically symmetric. Evaluate one
        # triangle and copy each value so that finite mp arithmetic preserves
        # that structural invariant exactly.
        for i, row in enumerate(columns):
            for j in range(i, len(columns)):
                column = columns[j]
                value = schur[i, j]
                add(position[row], position[column], -value)
                if i != j:
                    add(position[column], position[row], -value)
        reconstruction.append((rows, columns, kb_inverse, cb))

    row_indices = []
    column_indices = []
    values = []
    for row, columns in matrix.items():
        for column, value in columns.items():
            row_indices.append(row)
            column_indices.append(column)
            values.append(float(value))
    sparse = scipy.sparse.csc_matrix(
        (np.asarray(values), (np.asarray(row_indices), np.asarray(column_indices))),
        shape=(len(outer), len(outer)),
    )
    lu = scipy.sparse.linalg.splu(sparse)
    outer_solution = [
        mp.mpf(float(value)) for value in lu.solve(np.asarray(rhs, dtype=float))
    ]

    refinement = []
    for iteration in range(20):
        residual = []
        for row in range(len(outer)):
            action = sum(
                (
                    value * outer_solution[column]
                    for column, value in matrix.get(row, {}).items()
                ),
                mp.mpf(0),
            )
            residual.append(rhs[row] - action)
        residual_norm = mp.sqrt(sum(value**2 for value in residual))
        rhs_norm = mp.sqrt(sum(value**2 for value in rhs))
        refinement.append(residual_norm)
        if residual_norm <= mp.mpf("1e-48") * (1 + rhs_norm):
            break
        correction = lu.solve(np.asarray([float(value) for value in residual]))
        for index, value in enumerate(correction):
            outer_solution[index] += mp.mpf(float(value))
    else:
        raise ValueError("elevated-precision iterative refinement did not converge")

    solution = [mp.mpf(0)] * system.size
    for dof, value in prescribed.items():
        solution[dof] = value
    for index, dof in enumerate(outer):
        solution[dof] = outer_solution[index]

    for rows, columns, kb_inverse, cb in reconstruction:
        local = mp.matrix(2, 1)
        for e in range(2):
            local[e] = -sum(
                (cb[e, j] * solution[column] for j, column in enumerate(columns)),
                mp.mpf(0),
            )
        values = kb_inverse * local
        for component in range(2):
            solution[rows[component]] = values[component]

    return solution, outer, matrix, rhs, refinement
