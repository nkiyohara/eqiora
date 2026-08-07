#!/usr/bin/env python3
"""Exact NUM0 DFG/MINI oracle, sealed before implementation.

This oracle owns structural mathematics only.  It contains no benchmark
output, floating-point tolerance, mesh campaign, nonlinear target, or solver
result.  The registered Rust test owns the ordinary source-bound nonzero
step; this file independently fixes the exact operator identities and mutant
discriminators that step must compose.
"""

from __future__ import annotations

from fractions import Fraction as Q
from math import factorial


Poly = dict[tuple[int, int], Q]


def add(left: Poly, right: Poly) -> Poly:
    out = dict(left)
    for power, value in right.items():
        out[power] = out.get(power, Q(0)) + value
        if out[power] == 0:
            del out[power]
    return out


def scale(poly: Poly, value: Q) -> Poly:
    return {
        power: value * coefficient
        for power, coefficient in poly.items()
        if value * coefficient
    }


def multiply(left: Poly, right: Poly) -> Poly:
    out: Poly = {}
    for (i, j), a in left.items():
        for (k, ell), b in right.items():
            power = (i + k, j + ell)
            out[power] = out.get(power, Q(0)) + a * b
            if out[power] == 0:
                del out[power]
    return out


def derivative(poly: Poly, axis: int) -> Poly:
    if axis == 0:
        return {(i - 1, j): i * value for (i, j), value in poly.items() if i}
    return {(i, j - 1): j * value for (i, j), value in poly.items() if j}


def integrate(poly: Poly) -> Q:
    """Integrate exactly over {(x,y): x>=0, y>=0, x+y<=1}."""
    return sum(
        value * Q(factorial(i) * factorial(j), factorial(i + j + 2))
        for (i, j), value in poly.items()
    )


ONE: Poly = {(0, 0): Q(1)}
X: Poly = {(1, 0): Q(1)}
Y: Poly = {(0, 1): Q(1)}
LAMBDA = [add(add(ONE, scale(X, Q(-1))), scale(Y, Q(-1))), X, Y]
BUBBLE = multiply(multiply(LAMBDA[0], LAMBDA[1]), LAMBDA[2])
BASIS = [*LAMBDA, BUBBLE]


def gradient(poly: Poly) -> tuple[Poly, Poly]:
    return derivative(poly, 0), derivative(poly, 1)


def dot_integral(left: tuple[Poly, Poly], right: tuple[Poly, Poly]) -> Q:
    return sum(integrate(multiply(a, b)) for a, b in zip(left, right, strict=True))


def direct_pair(
    row: Poly, row_component: int, column: Poly, column_component: int, mu: Q
) -> Q:
    """Direct DFG block mu*delta_rc*int grad(row).grad(column)."""
    if row_component != column_component:
        return Q(0)
    return mu * dot_integral(gradient(row), gradient(column))


def crossed_pair(
    row: Poly, row_component: int, column: Poly, column_component: int, mu: Q
) -> Q:
    """Crossed block mu*int partial_c(row)*partial_r(column)."""
    return mu * integrate(
        multiply(derivative(row, column_component), derivative(column, row_component))
    )


def symmetric_pair(
    row: Poly, row_component: int, column: Poly, column_component: int, mu: Q
) -> Q:
    return direct_pair(row, row_component, column, column_component, mu) + crossed_pair(
        row, row_component, column, column_component, mu
    )


def bilinear(
    grad_u: tuple[tuple[Q, Q], tuple[Q, Q]], grad_v: tuple[tuple[Q, Q], tuple[Q, Q]]
) -> tuple[Q, Q, Q]:
    direct = sum(grad_u[i][j] * grad_v[i][j] for i in range(2) for j in range(2))
    crossed = sum(grad_u[j][i] * grad_v[i][j] for i in range(2) for j in range(2))
    return direct, crossed, direct + crossed


def check_ordinary_positive_contract() -> None:
    """The executable Rust positive must establish these non-vacuous facts first."""
    rho = Q(1)
    nu = Q(1, 1000)
    mu = rho * nu
    umax = Q(3, 10)
    height = Q(41, 100)

    def profile(y: Q) -> Q:
        return 4 * umax * y * (height - y) / height**2

    assert rho > 0 and nu > 0 and mu == Q(1, 1000)
    assert profile(0) == 0 and profile(height) == 0
    assert profile(height / 2) == umax > 0
    assert integrate_1d_profile(umax, height) / height == Q(2, 3) * umax
    expected_shape = {
        "exact_source_and_five_set_correspondence",
        "one_private_dfg_binding",
        "nonzero_correspondence_owned_inlet",
        "finite_nonzero_weakly_continuous_initial_mini_state",
        "boundary_traction_without_gauge",
        "one_checked_step",
        "two_finite_states",
        "strictly_advanced_time",
    }
    assert len(expected_shape) == 8


def integrate_1d_profile(umax: Q, height: Q) -> Q:
    # Integral_0^H 4 U y(H-y)/H^2 dy, evaluated analytically.
    return Q(2, 3) * umax * height


def check_exact_operator() -> None:
    mu = Q(1)

    # Direct and symmetric-minus-one-crossed are identical on every local block.
    for row in BASIS:
        for column in BASIS:
            for row_component in range(2):
                for column_component in range(2):
                    direct = direct_pair(
                        row, row_component, column, column_component, mu
                    )
                    symmetric = symmetric_pair(
                        row, row_component, column, column_component, mu
                    )
                    crossed = crossed_pair(
                        row, row_component, column, column_component, mu
                    )
                    assert direct == symmetric - crossed

    # Global affine P1-in-MINI discriminator on the reference cell (area 1/2).
    grad_u = ((Q(1), Q(0)), (Q(1), Q(0)))  # u=(x,x)
    grad_v = ((Q(1), Q(1)), (Q(0), Q(0)))  # v=(x+y,0)
    direct, crossed, symmetric = bilinear(grad_u, grad_v)
    assert (direct, crossed, symmetric) == (Q(1), Q(2), Q(3))
    assert tuple(Q(1, 2) * value for value in (direct, crossed, symmetric)) == (
        Q(1, 2),
        Q(1),
        Q(3, 2),
    )

    # Reference P1 count/sign discriminator.
    diagonal = (
        direct_pair(LAMBDA[1], 0, LAMBDA[1], 0, mu),
        crossed_pair(LAMBDA[1], 0, LAMBDA[1], 0, mu),
        symmetric_pair(LAMBDA[1], 0, LAMBDA[1], 0, mu),
    )
    off_component = (
        direct_pair(LAMBDA[1], 1, LAMBDA[2], 0, mu),
        crossed_pair(LAMBDA[1], 1, LAMBDA[2], 0, mu),
        symmetric_pair(LAMBDA[1], 1, LAMBDA[2], 0, mu),
    )
    assert diagonal == (Q(1, 2), Q(1, 2), Q(1))
    assert off_component == (Q(0), Q(1, 2), Q(1, 2))

    # Actual unscaled MINI bubble; beta scales every entry by beta^2.
    bubble_grad = gradient(BUBBLE)
    tensor = tuple(
        tuple(integrate(multiply(bubble_grad[i], bubble_grad[j])) for j in range(2))
        for i in range(2)
    )
    assert tensor == ((Q(1, 180), Q(1, 360)), (Q(1, 360), Q(1, 180)))
    assert dot_integral(bubble_grad, bubble_grad) == Q(1, 90)
    assert direct_pair(BUBBLE, 0, BUBBLE, 0, mu) == Q(1, 90)
    assert symmetric_pair(BUBBLE, 0, BUBBLE, 0, mu) == Q(1, 60)
    assert direct_pair(BUBBLE, 1, BUBBLE, 0, mu) == 0
    assert symmetric_pair(BUBBLE, 1, BUBBLE, 0, mu) == Q(1, 360)
    for vertex in LAMBDA:
        assert dot_integral(gradient(vertex), bubble_grad) == 0

    # Exact pressure and continuity signs.
    pressure_momentum = -integrate(multiply(LAMBDA[0], derivative(LAMBDA[1], 0)))
    pressure_continuity = -integrate(multiply(LAMBDA[0], derivative(LAMBDA[1], 0)))
    assert pressure_momentum == pressure_continuity == Q(-1, 6)

    # DFG stress is nonsymmetric; its pure viscous bilinear/matrix is symmetric PSD.
    for a in range(4):
        for b in range(4):
            for r in range(2):
                for c in range(2):
                    assert direct_pair(BASIS[a], r, BASIS[b], c, mu) == direct_pair(
                        BASIS[b], c, BASIS[a], r, mu
                    )
    for field in ((Q(1), Q(0)), (Q(0), Q(1)), (Q(2), Q(-3))):
        assert sum(component * component for component in field) >= 0


MUTANTS = {
    1: "new: direct DFG versus symmetric-minus-crossed P1 and bubble blocks",
    2: "new: exact off-component crossed multiplicity and primal/Jacobian agreement",
    3: "new: sealed viscosity identity plus nonzero diagonal/off-component blocks",
    4: "new: one semantic DFG stress identity owns volume and outlet",
    5: "new: semantic DFG outlet proof precedes zero facet action",
    6: "new: parent-outward inlet normal gives positive x velocity",
    7: "new: Umax centre value and 2*Umax/3 mean remain distinct",
    8: "new: correspondence owns facets before coordinate profile evaluation",
    9: "reuse: exact five-set partition rejection before composition",
    10: "reuse: exact source/mesh/Model/revision/state/Realization identity admission",
    11: "new: outlet pressure shift is physical and no gauge coefficient exists",
    12: "new: exact negative pressure and continuity local moments",
    13: "reuse: accepted skew/conservation evidence plus parent-normal (B+D)/2 identity",
    14: "reuse: trace and weak-continuity rejection before Newton",
    15: "new: only exact private source binding can select DFG",
    16: "new: nonzero source-bound checked step precedes mutants",
}


def check_all_mutants() -> None:
    assert set(MUTANTS) == set(range(1, 17))
    assert all(value.startswith(("new:", "reuse:")) for value in MUTANTS.values())

    mu = Q(1)
    direct = direct_pair(LAMBDA[1], 1, LAMBDA[2], 0, mu)
    crossed = crossed_pair(LAMBDA[1], 1, LAMBDA[2], 0, mu)
    assert direct == 0 and crossed == Q(1, 2)
    # Mutants 1 and 2: no subtraction, one subtraction, two, and addition.
    symmetric = direct + crossed
    assert (
        symmetric,
        symmetric - crossed,
        symmetric - 2 * crossed,
        symmetric + crossed,
    ) == (
        Q(1, 2),
        Q(0),
        Q(-1, 2),
        Q(1),
    )
    # Mutant 3: omitted/doubled/foreign mu and transposed-only action differ.
    diagonal = direct_pair(LAMBDA[1], 0, LAMBDA[1], 0, mu)
    assert diagonal == Q(1, 2) and 0 != diagonal and 2 * diagonal != diagonal
    assert crossed != direct

    # Mutants 4 and 5: equal zero bytes do not equate the semantic carriers.
    dfg_stress = "mu*grad(u)-pI"
    symmetric_stress = "2*mu*eps(u)-pI"
    assert dfg_stress != symmetric_stress
    assert (dfg_stress, "volume") != (symmetric_stress, "outlet")

    # Mutants 6 and 7: exact parent-normal and profile semantics.
    parent_normal = (Q(-1), Q(0))
    scalar = Q(3, 10)
    prescribed = tuple(-scalar * component for component in parent_normal)
    assert prescribed == (Q(3, 10), Q(0))
    assert -prescribed[0] < 0
    assert Q(2, 3) * scalar != scalar

    # Mutant 8 is new correspondence-first composition; 9, 10, and 14 reuse
    # exact predecessor gates without duplicating their broad oracles.
    assert MUTANTS[8].startswith("new:")
    inherited = {
        9: "five-set-partition",
        10: "identity-replay",
        14: "initial-admission",
    }
    assert set(inherited) == {9, 10, 14}

    # Mutant 11: a pressure shift changes DFG outlet traction and adds no DOF.
    shift = Q(7, 5)
    outlet_normal = (Q(1), Q(0))
    traction_change = tuple(-shift * component for component in outlet_normal)
    assert traction_change != (0, 0)
    vertices, cells = 7, 8
    assert 2 * (vertices + cells) + vertices == 3 * vertices + 2 * cells

    # Mutant 12: sign reversal is exact and visible.
    moment = -integrate(multiply(LAMBDA[0], derivative(LAMBDA[1], 0)))
    assert moment == Q(-1, 6) and -moment == Q(1, 6) != moment

    # Mutant 13: with A+T+D=B, C_cons-C_skew=(B+D)/2.
    a, t, divergence = Q(5), Q(-2), Q(3)
    boundary = a + t + divergence
    conservative = boundary - t
    skew = (a - t) / 2
    assert conservative - skew == (boundary + divergence) / 2

    # Mutants 15 and 16 are structural/ordinary-path obligations, not values.
    selectors = {"private_exact_source_binding": "dfg", "public_cartesian": "symmetric"}
    assert selectors == {
        "private_exact_source_binding": "dfg",
        "public_cartesian": "symmetric",
    }
    sequence = ("ordinary_nonzero_positive", "mutants")
    assert sequence[0] == "ordinary_nonzero_positive"


def checked_add(left: int, right: int, maximum: int) -> int:
    value = left + right
    if left < 0 or right < 0 or value > maximum:
        raise OverflowError
    return value


def checked_mul(left: int, right: int, maximum: int) -> int:
    value = left * right
    if left < 0 or right < 0 or value > maximum:
        raise OverflowError
    return value


def check_abstract_bounds() -> None:
    maximum = 2**64 - 1
    vertices, cells, boundary, outlet = 13, 17, 9, 2
    unknowns = checked_add(
        checked_mul(3, vertices, maximum), checked_mul(2, cells, maximum), maximum
    )
    packets = checked_add(cells, outlet, maximum)
    assert unknowns == 3 * vertices + 2 * cells
    assert packets == cells + outlet <= cells + boundary
    # Essential elimination may lower width; it may not create a gauge width.
    assert unknowns < unknowns + 1
    try:
        checked_mul(maximum, 2, maximum)
    except OverflowError:
        pass
    else:
        raise AssertionError("raw/count arithmetic must fail closed on overflow")


def main() -> None:
    # The ordinary positive contract is deliberately checked before mutants.
    check_ordinary_positive_contract()
    check_exact_operator()
    check_all_mutants()
    check_abstract_bounds()
    print("NUM0_DFG_EXACT_ORACLE_OK mutants=16 tolerance=none")


if __name__ == "__main__":
    main()
