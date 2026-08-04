#!/usr/bin/env python3
"""Derive the exact two-element Q1 sparse-LU-reuse reference.

The scientific authority in this file is fractions.Fraction arithmetic.  Float
conversion is confined to the explicitly labelled binary64 observations.
"""

from __future__ import annotations

import argparse
import difflib
from fractions import Fraction
import json
from pathlib import Path
import struct
from typing import Iterable, Sequence


Q = Fraction
PARAMETER_ORDER = ("source_scale", "diffusion", "boundary_offset")
POINTS = (
    ("p0", (Q(2), Q(1), Q(0))),
    ("p1", (Q(4), Q(1), Q(0))),
    ("p2", (Q(2), Q(5, 4), Q(0))),
)
RESIDUAL_TOLERANCE = Q(1, 2**30)
REFERENCE_BASIS = (
    (Q(1, 2), Q(-1, 2)),  # N_0(xi) = (1 - xi) / 2
    (Q(1, 2), Q(1, 2)),  # N_1(xi) = (1 + xi) / 2
)
ELEMENTS = (
    (0, 1, Q(0), Q(1, 2)),
    (1, 2, Q(1, 2), Q(1)),
)


class SingularMatrix(Exception):
    """Raised when an exact square system has no unique solution."""


def require(condition: bool, message: str) -> None:
    """A self-check that remains active when Python assertions are optimized."""

    if not condition:
        raise AssertionError(message)


def q_json(value: Fraction) -> dict[str, int]:
    require(isinstance(value, Fraction), "exact values must remain Fraction")
    return {"numerator": value.numerator, "denominator": value.denominator}


def q_vector(values: Iterable[Fraction]) -> list[dict[str, int]]:
    return [q_json(value) for value in values]


def poly_trim(coefficients: Sequence[Fraction]) -> tuple[Fraction, ...]:
    result = list(coefficients)
    while len(result) > 1 and result[-1] == 0:
        result.pop()
    return tuple(result)


def poly_add(
    left: Sequence[Fraction], right: Sequence[Fraction]
) -> tuple[Fraction, ...]:
    size = max(len(left), len(right))
    return poly_trim(
        tuple(
            (left[index] if index < len(left) else Q(0))
            + (right[index] if index < len(right) else Q(0))
            for index in range(size)
        )
    )


def poly_scale(
    coefficients: Sequence[Fraction], factor: Fraction
) -> tuple[Fraction, ...]:
    return poly_trim(tuple(factor * coefficient for coefficient in coefficients))


def poly_multiply(
    left: Sequence[Fraction], right: Sequence[Fraction]
) -> tuple[Fraction, ...]:
    result = [Q(0) for _ in range(len(left) + len(right) - 1)]
    for left_degree, left_coefficient in enumerate(left):
        for right_degree, right_coefficient in enumerate(right):
            result[left_degree + right_degree] += (
                left_coefficient * right_coefficient
            )
    return poly_trim(tuple(result))


def poly_derivative(coefficients: Sequence[Fraction]) -> tuple[Fraction, ...]:
    if len(coefficients) == 1:
        return (Q(0),)
    return poly_trim(
        tuple(
            Q(degree) * coefficients[degree]
            for degree in range(1, len(coefficients))
        )
    )


def poly_evaluate(
    coefficients: Sequence[Fraction], argument: Fraction
) -> Fraction:
    result = Q(0)
    for coefficient in reversed(coefficients):
        result = result * argument + coefficient
    return result


def exact_reference_monomial_moment(degree: int) -> Fraction:
    """Integral of xi**degree over [-1, 1]."""

    require(degree >= 0, "monomial degree must be nonnegative")
    if degree % 2:
        return Q(0)
    return Q(2, degree + 1)


def gauss2_monomial_moment(degree: int) -> Fraction:
    """Exact two-point GL moment without approximating sqrt(3).

    The nodes are +/-r with the symbolic identity r**2 = 1/3 and unit
    weights.  Odd powers cancel; even powers are 2*(1/3)**(degree/2).
    """

    require(degree >= 0, "monomial degree must be nonnegative")
    if degree % 2:
        return Q(0)
    return Q(2) * Q(1, 3) ** (degree // 2)


def integrate_reference_polynomial(coefficients: Sequence[Fraction]) -> Fraction:
    return sum(
        (
            coefficient * exact_reference_monomial_moment(degree)
            for degree, coefficient in enumerate(coefficients)
        ),
        Q(0),
    )


def gauss2_integrate_polynomial(coefficients: Sequence[Fraction]) -> Fraction:
    require(
        len(poly_trim(coefficients)) <= 4,
        "two-point Gauss-Legendre authority is limited to degree <= 3",
    )
    return sum(
        (
            coefficient * gauss2_monomial_moment(degree)
            for degree, coefficient in enumerate(coefficients)
        ),
        Q(0),
    )


def bind_parameters(
    values: Sequence[Fraction], order: Sequence[str]
) -> dict[str, Fraction]:
    require(
        tuple(order) == PARAMETER_ORDER,
        "parameter tuple order must be (source_scale, diffusion, boundary_offset)",
    )
    require(len(values) == len(PARAMETER_ORDER), "parameter tuple has wrong arity")
    require(
        all(isinstance(value, Fraction) for value in values),
        "parameters must be exact Fractions",
    )
    return dict(zip(PARAMETER_ORDER, values, strict=True))


def affine_element_map(left: Fraction, right: Fraction) -> dict[str, object]:
    """Return x(xi) = midpoint + jacobian*xi for xi in [-1, 1]."""

    require(right > left, "element map must preserve orientation")
    midpoint = (left + right) / 2
    jacobian = (right - left) / 2
    coefficients = (midpoint, jacobian)
    require(poly_evaluate(coefficients, Q(-1)) == left, "left map endpoint")
    require(poly_evaluate(coefficients, Q(1)) == right, "right map endpoint")
    require(
        poly_derivative(coefficients) == (jacobian,),
        "affine map derivative must equal its Jacobian",
    )
    return {
        "left": left,
        "right": right,
        "coefficients": coefficients,
        "jacobian": jacobian,
    }


def canonicalize_coo(
    nrows: int,
    ncols: int,
    entries: Iterable[tuple[int, int, Fraction]],
) -> dict[str, object]:
    accumulated: dict[tuple[int, int], Fraction] = {}
    for row, column, value in entries:
        require(0 <= row < nrows, "COO row outside matrix")
        require(0 <= column < ncols, "COO column outside matrix")
        require(isinstance(value, Fraction), "COO value is not exact")
        accumulated[(row, column)] = accumulated.get((row, column), Q(0)) + value

    offsets = [0]
    columns: list[int] = []
    values: list[Fraction] = []
    for row in range(nrows):
        row_entries = sorted(
            (
                (column, value)
                for (entry_row, column), value in accumulated.items()
                if entry_row == row
            ),
            key=lambda item: item[0],
        )
        columns.extend(column for column, _ in row_entries)
        values.extend(value for _, value in row_entries)
        offsets.append(len(columns))

    csr = {
        "nrows": nrows,
        "ncols": ncols,
        "offsets": offsets,
        "columns": columns,
        "values": values,
        "canonical": True,
    }
    validate_csr(csr)
    return csr


def validate_csr(csr: dict[str, object]) -> None:
    nrows = int(csr["nrows"])
    ncols = int(csr["ncols"])
    offsets = list(csr["offsets"])
    columns = list(csr["columns"])
    values = list(csr["values"])
    require(len(offsets) == nrows + 1, "CSR offset count")
    require(offsets[0] == 0, "CSR must start at offset zero")
    require(offsets[-1] == len(columns) == len(values), "CSR terminal offset")
    require(
        all(offsets[index] <= offsets[index + 1] for index in range(nrows)),
        "CSR offsets must be monotone",
    )
    for row in range(nrows):
        row_columns = columns[offsets[row] : offsets[row + 1]]
        require(
            all(0 <= column < ncols for column in row_columns),
            "CSR column outside matrix",
        )
        require(
            all(
                row_columns[index] < row_columns[index + 1]
                for index in range(len(row_columns) - 1)
            ),
            "canonical CSR columns must be strictly increasing per row",
        )
    require(
        all(isinstance(value, Fraction) for value in values),
        "CSR values must be exact Fractions",
    )


def canonicalize_vector(
    size: int, entries: Iterable[tuple[int, Fraction]]
) -> list[Fraction]:
    result = [Q(0) for _ in range(size)]
    for index, value in entries:
        require(0 <= index < size, "vector COO index outside vector")
        require(isinstance(value, Fraction), "vector COO value is not exact")
        result[index] += value
    return result


def csr_to_dense(csr: dict[str, object]) -> list[list[Fraction]]:
    validate_csr(csr)
    nrows = int(csr["nrows"])
    ncols = int(csr["ncols"])
    offsets = list(csr["offsets"])
    columns = list(csr["columns"])
    values = list(csr["values"])
    dense = [[Q(0) for _ in range(ncols)] for _ in range(nrows)]
    for row in range(nrows):
        for position in range(offsets[row], offsets[row + 1]):
            dense[row][columns[position]] = values[position]
    return dense


def csr_pattern(csr: dict[str, object]) -> tuple[tuple[int, ...], tuple[int, ...]]:
    validate_csr(csr)
    return tuple(csr["offsets"]), tuple(csr["columns"])


def matvec(
    matrix: Sequence[Sequence[Fraction]], vector: Sequence[Fraction]
) -> list[Fraction]:
    require(
        all(len(row) == len(vector) for row in matrix),
        "matrix-vector dimensions",
    )
    return [
        sum((value * vector[column] for column, value in enumerate(row)), Q(0))
        for row in matrix
    ]


def vector_subtract(
    left: Sequence[Fraction], right: Sequence[Fraction]
) -> list[Fraction]:
    require(len(left) == len(right), "vector subtraction dimensions")
    return [left[index] - right[index] for index in range(len(left))]


def determinant(matrix: Sequence[Sequence[Fraction]]) -> Fraction:
    size = len(matrix)
    require(all(len(row) == size for row in matrix), "determinant needs square matrix")
    work = [list(row) for row in matrix]
    sign = Q(1)
    result = Q(1)
    for column in range(size):
        pivot_row = next(
            (row for row in range(column, size) if work[row][column] != 0),
            None,
        )
        if pivot_row is None:
            return Q(0)
        if pivot_row != column:
            work[column], work[pivot_row] = work[pivot_row], work[column]
            sign = -sign
        pivot = work[column][column]
        result *= pivot
        for row in range(column + 1, size):
            factor = work[row][column] / pivot
            for entry_column in range(column, size):
                work[row][entry_column] -= factor * work[column][entry_column]
    return sign * result


def solve_exact(
    matrix: Sequence[Sequence[Fraction]], rhs: Sequence[Fraction]
) -> list[Fraction]:
    size = len(matrix)
    require(len(rhs) == size, "solve right-hand side dimension")
    require(all(len(row) == size for row in matrix), "solve needs square matrix")
    augmented = [list(matrix[row]) + [rhs[row]] for row in range(size)]
    for column in range(size):
        pivot_row = next(
            (row for row in range(column, size) if augmented[row][column] != 0),
            None,
        )
        if pivot_row is None:
            raise SingularMatrix("exact matrix has a zero pivot after elimination")
        augmented[column], augmented[pivot_row] = (
            augmented[pivot_row],
            augmented[column],
        )
        pivot = augmented[column][column]
        augmented[column] = [value / pivot for value in augmented[column]]
        for row in range(size):
            if row == column:
                continue
            factor = augmented[row][column]
            augmented[row] = [
                augmented[row][entry_column]
                - factor * augmented[column][entry_column]
                for entry_column in range(size + 1)
            ]
    solution = [augmented[row][-1] for row in range(size)]
    require(
        matvec(matrix, solution) == list(rhs),
        "exact solve must have an exact zero residual",
    )
    return solution


def rank_exact(matrix: Sequence[Sequence[Fraction]]) -> int:
    if not matrix:
        return 0
    width = len(matrix[0])
    require(all(len(row) == width for row in matrix), "rank matrix rectangularity")
    work = [list(row) for row in matrix]
    pivot_row = 0
    for column in range(width):
        selected = next(
            (row for row in range(pivot_row, len(work)) if work[row][column] != 0),
            None,
        )
        if selected is None:
            continue
        work[pivot_row], work[selected] = work[selected], work[pivot_row]
        pivot = work[pivot_row][column]
        for row in range(pivot_row + 1, len(work)):
            factor = work[row][column] / pivot
            for entry_column in range(column, width):
                work[row][entry_column] -= factor * work[pivot_row][entry_column]
        pivot_row += 1
        if pivot_row == len(work):
            break
    return pivot_row


def inverse_infinity_norm(matrix: Sequence[Sequence[Fraction]]) -> Fraction:
    size = len(matrix)
    require(size > 0, "inverse norm needs a nonempty matrix")
    columns = []
    for column in range(size):
        unit = [Q(1) if row == column else Q(0) for row in range(size)]
        columns.append(solve_exact(matrix, unit))
    inverse_rows = [
        [columns[column][row] for column in range(size)] for row in range(size)
    ]
    return max(sum((abs(value) for value in row), Q(0)) for row in inverse_rows)


def assemble_unconstrained(
    source_scale: Fraction, diffusion: Fraction
) -> dict[str, object]:
    require(source_scale > 0, "accepted source scale must be positive")
    require(diffusion > 0, "accepted diffusion must be positive")
    matrix_coo: list[tuple[int, int, Fraction, int]] = []
    rhs_coo: list[tuple[int, Fraction, int]] = []
    maps = []
    basis_derivatives = tuple(poly_derivative(basis) for basis in REFERENCE_BASIS)

    for element, (global_left, global_right, left, right) in enumerate(ELEMENTS):
        element_map = affine_element_map(left, right)
        maps.append(element_map)
        jacobian = element_map["jacobian"]
        require(isinstance(jacobian, Fraction), "element Jacobian must be exact")
        physical_derivatives = tuple(
            poly_scale(derivative, Q(1) / jacobian)
            for derivative in basis_derivatives
        )
        global_nodes = (global_left, global_right)

        local_matrix = [[Q(0), Q(0)], [Q(0), Q(0)]]
        local_rhs = [Q(0), Q(0)]
        for local_row in range(2):
            for local_column in range(2):
                integrand = poly_scale(
                    poly_multiply(
                        physical_derivatives[local_row],
                        physical_derivatives[local_column],
                    ),
                    diffusion * jacobian,
                )
                require(
                    len(poly_trim(integrand)) == 1,
                    "Q1 stiffness integrand must be constant",
                )
                quadrature_value = gauss2_integrate_polynomial(integrand)
                exact_value = integrate_reference_polynomial(integrand)
                require(
                    quadrature_value == exact_value,
                    "Gauss rule must exactly integrate stiffness",
                )
                local_matrix[local_row][local_column] = quadrature_value
                matrix_coo.append(
                    (
                        global_nodes[local_row],
                        global_nodes[local_column],
                        quadrature_value,
                        element,
                    )
                )

            load_integrand = poly_scale(
                REFERENCE_BASIS[local_row], source_scale * jacobian
            )
            require(
                len(poly_trim(load_integrand)) <= 2,
                "Q1 source integrand must have degree at most one",
            )
            quadrature_load = gauss2_integrate_polynomial(load_integrand)
            exact_load = integrate_reference_polynomial(load_integrand)
            require(
                quadrature_load == exact_load,
                "Gauss rule must exactly integrate source load",
            )
            local_rhs[local_row] = quadrature_load
            rhs_coo.append((global_nodes[local_row], quadrature_load, element))

        element_length = right - left
        physical_coordinate_stiffness = poly_scale(
            (Q(1), Q(-1), Q(-1), Q(1)), diffusion / element_length
        )
        require(
            tuple(value for row in local_matrix for value in row)
            == physical_coordinate_stiffness,
            "physical-coordinate stiffness cross-check",
        )
        require(
            local_rhs
            == [source_scale * element_length / 2, source_scale * element_length / 2],
            "physical-coordinate load cross-check",
        )

    coo_without_element = [
        (row, column, value) for row, column, value, _ in matrix_coo
    ]
    csr = canonicalize_coo(3, 3, coo_without_element)
    rhs = canonicalize_vector(
        3, ((index, value) for index, value, _ in rhs_coo)
    )
    require(
        canonicalize_coo(3, 3, reversed(coo_without_element)) == csr,
        "canonical CSR must not depend on COO insertion order",
    )
    return {
        "matrix_coo": matrix_coo,
        "rhs_coo": rhs_coo,
        "csr": csr,
        "rhs": rhs,
        "maps": maps,
    }


def eliminate_strong_dirichlet(
    csr: dict[str, object],
    rhs: Sequence[Fraction],
    boundary_values: dict[int, Fraction],
) -> dict[str, object]:
    """Apply exact A_ff*u_f = b_f - A_fb*g after CSR canonicalization."""

    validate_csr(csr)
    require(csr.get("canonical") is True, "elimination requires canonical CSR")
    require(int(csr["nrows"]) == int(csr["ncols"]), "square constrained system")
    size = int(csr["nrows"])
    require(len(rhs) == size, "constraint right-hand side dimension")
    require(
        all(0 <= index < size for index in boundary_values),
        "boundary degree of freedom outside system",
    )
    dense = csr_to_dense(csr)
    free_dofs = [index for index in range(size) if index not in boundary_values]
    boundary_dofs = sorted(boundary_values)
    reduced_rhs = [
        rhs[row]
        - sum(
            (
                dense[row][boundary] * boundary_values[boundary]
                for boundary in boundary_dofs
            ),
            Q(0),
        )
        for row in free_dofs
    ]
    reduced_coo = [
        (reduced_row, reduced_column, dense[global_row][global_column])
        for reduced_row, global_row in enumerate(free_dofs)
        for reduced_column, global_column in enumerate(free_dofs)
    ]
    reduced_csr = canonicalize_coo(
        len(free_dofs), len(free_dofs), reduced_coo
    )
    return {
        "free_dofs": free_dofs,
        "boundary_dofs": boundary_dofs,
        "boundary_values": [boundary_values[index] for index in boundary_dofs],
        "csr": reduced_csr,
        "rhs": reduced_rhs,
    }


def serialize_csr(csr: dict[str, object]) -> dict[str, object]:
    validate_csr(csr)
    return {
        "shape": [int(csr["nrows"]), int(csr["ncols"])],
        "offsets": list(csr["offsets"]),
        "columns": list(csr["columns"]),
        "values": q_vector(csr["values"]),
    }


def serialize_matrix_coo(
    entries: Sequence[tuple[int, int, Fraction, int]]
) -> list[dict[str, object]]:
    return [
        {
            "element": element,
            "row": row,
            "column": column,
            "value": q_json(value),
        }
        for row, column, value, element in entries
    ]


def serialize_rhs_coo(
    entries: Sequence[tuple[int, Fraction, int]]
) -> list[dict[str, object]]:
    return [
        {"element": element, "row": row, "value": q_json(value)}
        for row, value, element in entries
    ]


def binary64_bits(value: float) -> str:
    return "0x" + struct.pack(">d", value).hex()


def binary64_observation(
    matrix: Sequence[Sequence[Fraction]],
    rhs: Sequence[Fraction],
    exact_solution: Sequence[Fraction],
) -> dict[str, object]:
    nearest = [float(value) for value in exact_solution]
    nearest_exact = [Fraction.from_float(value) for value in nearest]
    representation_error = vector_subtract(nearest_exact, exact_solution)
    exact_residual = vector_subtract(matvec(matrix, nearest_exact), rhs)
    evaluated_residual = [
        sum(
            (
                float(matrix[row][column]) * nearest[column]
                for column in range(len(nearest))
            ),
            0.0,
        )
        - float(rhs[row])
        for row in range(len(rhs))
    ]
    return {
        "status": "observation_only_not_scientific_authority",
        "nearest_solution_bits": [binary64_bits(value) for value in nearest],
        "nearest_solution_exact_values": q_vector(nearest_exact),
        "signed_representation_error": q_vector(representation_error),
        "absolute_representation_error": q_vector(
            abs(value) for value in representation_error
        ),
        "exact_residual_of_stored_binary64_solution": q_vector(exact_residual),
        "binary64_evaluated_residual_bits": [
            binary64_bits(value) for value in evaluated_residual
        ],
        "binary64_evaluated_residual_exact_values": q_vector(
            Fraction.from_float(value) for value in evaluated_residual
        ),
        "not_used_for_residual_contract_error_bound": True,
    }


def derive_point(point_id: str, raw_parameters: Sequence[Fraction]) -> dict[str, object]:
    parameters = bind_parameters(raw_parameters, PARAMETER_ORDER)
    require(
        tuple(parameters[name] for name in PARAMETER_ORDER) == tuple(raw_parameters),
        "named parameters must round-trip in the required tuple order",
    )
    source_scale = parameters["source_scale"]
    diffusion = parameters["diffusion"]
    boundary_offset = parameters["boundary_offset"]
    assembly = assemble_unconstrained(source_scale, diffusion)
    constrained = eliminate_strong_dirichlet(
        assembly["csr"],
        assembly["rhs"],
        {0: boundary_offset, 2: boundary_offset},
    )
    reduced_matrix = csr_to_dense(constrained["csr"])
    reduced_rhs = constrained["rhs"]
    exact_determinant = determinant(reduced_matrix)
    require(exact_determinant != 0, f"{point_id} reduced matrix must be nonsingular")
    reduced_solution = solve_exact(reduced_matrix, reduced_rhs)
    true_residual = vector_subtract(
        matvec(reduced_matrix, reduced_solution), reduced_rhs
    )
    require(true_residual == [Q(0)], f"{point_id} true residual")
    full_solution = [boundary_offset, reduced_solution[0], boundary_offset]
    inverse_norm = inverse_infinity_norm(reduced_matrix)
    componentwise_bound = inverse_norm * RESIDUAL_TOLERANCE

    return {
        "id": point_id,
        "parameters": {
            "tuple": q_vector(raw_parameters),
            "named": {
                name: q_json(parameters[name]) for name in PARAMETER_ORDER
            },
        },
        "exact_assembly": {
            "unconstrained_global_coo": {
                "matrix": serialize_matrix_coo(assembly["matrix_coo"]),
                "rhs": serialize_rhs_coo(assembly["rhs_coo"]),
            },
            "canonical_unconstrained_csr": serialize_csr(assembly["csr"]),
            "canonical_unconstrained_rhs": q_vector(assembly["rhs"]),
        },
        "strong_dirichlet_elimination": {
            "method": "free_boundary_block_elimination_not_boundary_row_replacement",
            "free_dofs": constrained["free_dofs"],
            "boundary_dofs": constrained["boundary_dofs"],
            "boundary_values": q_vector(constrained["boundary_values"]),
            "reduced_csr": serialize_csr(constrained["csr"]),
            "reduced_rhs": q_vector(reduced_rhs),
        },
        "exact_classification": {
            "determinant": q_json(exact_determinant),
            "singular": False,
        },
        "exact_solution": {
            "reduced_free_dofs": q_vector(reduced_solution),
            "reconstructed_global_dofs": q_vector(full_solution),
        },
        "exact_true_reduced_residual": {
            "vector": q_vector(true_residual),
            "infinity_norm": q_json(max(abs(value) for value in true_residual)),
        },
        "residual_contract_solution_error_bound": {
            "kind": "componentwise_absolute_bound_from_infinity_norm",
            "formula": "||A^{-1}||_infinity * 2^-30",
            "inverse_infinity_norm": q_json(inverse_norm),
            "absolute_bound_per_free_component": q_json(componentwise_bound),
        },
        "binary64_observation": binary64_observation(
            reduced_matrix, reduced_rhs, reduced_solution
        ),
    }


def derive_mutants(expected_reduced_pattern: tuple[tuple[int, ...], tuple[int, ...]]) -> dict[str, object]:
    structural_csr = {
        "nrows": 2,
        "ncols": 2,
        "offsets": [0, 2, 4],
        "columns": [0, 1, 0, 1],
        "values": [Q(4), Q(-2), Q(-2), Q(4)],
        "canonical": True,
    }
    structural_rhs = [Q(1), Q(1)]
    validate_csr(structural_csr)
    structural_matrix = csr_to_dense(structural_csr)
    structural_determinant = determinant(structural_matrix)
    structural_solution = solve_exact(structural_matrix, structural_rhs)
    structural_residual = vector_subtract(
        matvec(structural_matrix, structural_solution), structural_rhs
    )
    structural_compatible = csr_pattern(structural_csr) == expected_reduced_pattern
    require(not structural_compatible, "2x2 mutant must mismatch accepted structure")
    require(structural_determinant == Q(12), "2x2 mutant determinant")
    require(structural_solution == [Q(1, 2), Q(1, 2)], "2x2 mutant solution")

    singular_csr = {
        "nrows": 1,
        "ncols": 1,
        "offsets": [0, 1],
        "columns": [0],
        "values": [Q(0)],
        "canonical": True,
    }
    singular_rhs = [Q(1)]
    validate_csr(singular_csr)
    singular_matrix = csr_to_dense(singular_csr)
    singular_determinant = determinant(singular_matrix)
    singular_compatible = csr_pattern(singular_csr) == expected_reduced_pattern
    require(singular_compatible, "zero 1x1 mutant must retain accepted structure")
    require(singular_determinant == 0, "zero 1x1 mutant must be singular")
    solve_rejected = False
    try:
        solve_exact(singular_matrix, singular_rhs)
    except SingularMatrix:
        solve_rejected = True
    require(solve_rejected, "exact solver must reject singular mutant")
    coefficient_rank = rank_exact(singular_matrix)
    augmented_rank = rank_exact(
        [singular_matrix[row] + [singular_rhs[row]] for row in range(1)]
    )
    require(
        coefficient_rank == 0 and augmented_rank == 1,
        "zero 1x1 mutant with RHS one must be inconsistent",
    )

    return {
        "required_structure_mismatch": {
            "canonical_csr": serialize_csr(structural_csr),
            "rhs": q_vector(structural_rhs),
            "classification": "not_an_accepted_compatible_structure",
            "compatible_with_accepted_reduced_pattern": structural_compatible,
            "exact_determinant": q_json(structural_determinant),
            "singular": False,
            "exact_solution": q_vector(structural_solution),
            "exact_true_residual": q_vector(structural_residual),
        },
        "required_same_pattern_singular": {
            "canonical_csr": serialize_csr(singular_csr),
            "rhs": q_vector(singular_rhs),
            "classification": "singular_with_retained_structure",
            "compatible_with_accepted_reduced_pattern": singular_compatible,
            "exact_determinant": q_json(singular_determinant),
            "singular": True,
            "coefficient_rank": coefficient_rank,
            "augmented_rank": augmented_rank,
            "exact_solution": None,
            "zero_residual_solution_exists": False,
        },
    }


def run_falsifiers(points: Sequence[dict[str, object]]) -> list[str]:
    by_id = {point["id"]: point for point in points}
    p0 = by_id["p0"]
    p1 = by_id["p1"]
    p2 = by_id["p2"]

    wrong_order = ("diffusion", "source_scale", "boundary_offset")
    rejected_wrong_order = False
    try:
        bind_parameters(POINTS[0][1], wrong_order)
    except AssertionError:
        rejected_wrong_order = True
    require(rejected_wrong_order, "tuple-order mutant must be rejected")
    wrongly_bound = dict(zip(wrong_order, POINTS[0][1], strict=True))
    require(
        wrongly_bound["source_scale"] != Q(2)
        and wrongly_bound["diffusion"] != Q(1),
        "p0 must expose a source/diffusion tuple swap",
    )

    p0_reduced = p0["strong_dirichlet_elimination"]
    p1_reduced = p1["strong_dirichlet_elimination"]
    p2_reduced = p2["strong_dirichlet_elimination"]
    p0_matrix_value = Q(4)
    p0_rhs_value = Q(1)
    p0_solution = Q(1, 4)
    require(
        p0_reduced["reduced_csr"]["values"] == q_vector([p0_matrix_value])
        and p0_reduced["reduced_rhs"] == q_vector([p0_rhs_value]),
        "p0 sign and assembly scaling",
    )
    wrong_sign_rhs = [Q(-1)]
    require(
        vector_subtract(matvec([[p0_matrix_value]], [p0_solution]), wrong_sign_rhs)
        != [Q(0)],
        "negative-source-sign mutant must have nonzero residual",
    )
    require(
        p1_reduced["reduced_rhs"] == q_vector([Q(2)])
        and p1["exact_solution"]["reduced_free_dofs"] == q_vector([Q(1, 2)]),
        "doubling source must double RHS and solution at fixed diffusion",
    )
    require(
        p2_reduced["reduced_csr"]["values"] == q_vector([Q(5)])
        and p2["exact_solution"]["reduced_free_dofs"] == q_vector([Q(1, 5)]),
        "diffusion 5/4 must scale stiffness and inversely scale solution",
    )

    sentinel_assembly = assemble_unconstrained(Q(2), Q(1))
    sentinel_boundary = Q(3, 7)
    sentinel = eliminate_strong_dirichlet(
        sentinel_assembly["csr"],
        sentinel_assembly["rhs"],
        {0: sentinel_boundary, 2: sentinel_boundary},
    )
    require(
        sentinel["rhs"] == [Q(19, 7)],
        "constraint elimination must subtract both boundary-column terms",
    )
    sentinel_solution = solve_exact(csr_to_dense(sentinel["csr"]), sentinel["rhs"])
    require(
        sentinel_solution == [Q(19, 28)]
        and sentinel_solution != [Q(1, 4)],
        "nonzero boundary sentinel must reject omitted boundary coupling",
    )

    p0_assembly = assemble_unconstrained(Q(2), Q(1))
    diagonal_duplicates = [
        value
        for row, column, value, _ in p0_assembly["matrix_coo"]
        if (row, column) == (1, 1)
    ]
    require(
        diagonal_duplicates == [Q(2), Q(2)],
        "two element-diagonal COO contributions must be present",
    )
    require(
        csr_to_dense(p0_assembly["csr"])[1][1] == Q(4)
        and diagonal_duplicates[-1] != Q(4),
        "canonicalization must accumulate duplicates rather than overwrite",
    )
    require(
        p0_assembly["csr"]["offsets"] == [0, 2, 5, 7]
        and p0_assembly["csr"]["columns"] == [0, 1, 0, 1, 2, 1, 2],
        "unconstrained CSR ordering must be canonical",
    )

    return [
        "required parameter tuple order rejects source/diffusion swaps",
        "weak-form positive load rejects a negative-source-sign mutant",
        "source and diffusion scaling identities hold independently",
        "nonzero boundary sentinel requires both A_fb*g contributions",
        "duplicate COO entries accumulate instead of last-write overwrite",
        "canonical CSR rows have deterministic increasing column order",
        "required 2x2 mutant mismatches the accepted reduced pattern",
        "required zero 1x1 mutant retains pattern and is exactly singular",
    ]


def quadrature_proof_json() -> dict[str, object]:
    moments = []
    for degree in range(4):
        exact = exact_reference_monomial_moment(degree)
        gauss = gauss2_monomial_moment(degree)
        require(exact == gauss, f"Gauss moment mismatch at degree {degree}")
        moments.append(
            {
                "degree": degree,
                "exact_integral_over_minus1_plus1": q_json(exact),
                "two_point_rule_moment": q_json(gauss),
                "equal": True,
            }
        )
    return {
        "rule": "two_point_gauss_legendre",
        "symbolic_nodes": ["-1/sqrt(3)", "+1/sqrt(3)"],
        "symbolic_node_identity": "r^2 = 1/3 with nodes -r and +r",
        "weights": q_vector([Q(1), Q(1)]),
        "exact_moment_identities": moments,
        "linearity_conclusion": "exact for every polynomial of degree <= 3",
        "relevant_integrands": {
            "Q1_stiffness_degree": 0,
            "constant_source_times_Q1_basis_degree": 1,
        },
        "floating_nodes_used": False,
    }


def derive_reference() -> dict[str, object]:
    require(PARAMETER_ORDER == ("source_scale", "diffusion", "boundary_offset"), "tuple order")
    require(
        poly_add(REFERENCE_BASIS[0], REFERENCE_BASIS[1]) == (Q(1),),
        "Q1 basis must form a partition of unity",
    )
    require(
        tuple(poly_derivative(basis) for basis in REFERENCE_BASIS)
        == ((Q(-1, 2),), (Q(1, 2),)),
        "Q1 basis derivatives",
    )
    maps = [affine_element_map(left, right) for _, _, left, right in ELEMENTS]
    points = [derive_point(point_id, values) for point_id, values in POINTS]
    expected_pattern = (
        tuple(points[0]["strong_dirichlet_elimination"]["reduced_csr"]["offsets"]),
        tuple(points[0]["strong_dirichlet_elimination"]["reduced_csr"]["columns"]),
    )
    require(
        all(
            (
                tuple(point["strong_dirichlet_elimination"]["reduced_csr"]["offsets"]),
                tuple(point["strong_dirichlet_elimination"]["reduced_csr"]["columns"]),
            )
            == expected_pattern
            for point in points
        ),
        "all accepted points must share the reduced sparsity pattern",
    )
    mutants = derive_mutants(expected_pattern)
    self_checks = run_falsifiers(points)

    expected_bounds = {
        "p0": Q(1, 2**32),
        "p1": Q(1, 2**32),
        "p2": Q(1, 5 * 2**30),
    }
    for point in points:
        actual = point["residual_contract_solution_error_bound"][
            "absolute_bound_per_free_component"
        ]
        require(
            actual == q_json(expected_bounds[point["id"]]),
            f"{point['id']} residual-contract error bound",
        )

    return {
        "schema": "eqiora.faer_sparse_lu_reuse.symbolic_reference.v1",
        "scientific_authority": {
            "arithmetic": "Python standard-library fractions.Fraction",
            "floating_point_role": "binary64 observations only",
            "assembly_route": "reference Q1 polynomials and exact Gauss moment identities",
        },
        "public_claim": {
            "domain": q_vector([Q(0), Q(1)]),
            "uniform_Q1_element_count": 2,
            "PDE": "-div(diffusion * grad(potential)) - source_scale = 0",
            "weak_form": "integral(diffusion*potential_prime*test_prime) = integral(source_scale*test)",
            "parameter_tuple_order": list(PARAMETER_ORDER),
            "absolute_residual_tolerance": q_json(RESIDUAL_TOLERANCE),
            "absolute_residual_tolerance_expression": "2^-30",
        },
        "reference_construction": {
            "reference_coordinate_domain": q_vector([Q(-1), Q(1)]),
            "Q1_basis_coefficients_in_ascending_xi_power": [
                q_vector(basis) for basis in REFERENCE_BASIS
            ],
            "Q1_basis_derivative_coefficients_in_ascending_xi_power": [
                q_vector(poly_derivative(basis)) for basis in REFERENCE_BASIS
            ],
            "affine_element_maps": [
                {
                    "element": element,
                    "global_dofs": [global_left, global_right],
                    "physical_interval": q_vector([element_map["left"], element_map["right"]]),
                    "x_of_xi_coefficients_in_ascending_power": q_vector(
                        element_map["coefficients"]
                    ),
                    "jacobian_dx_dxi": q_json(element_map["jacobian"]),
                }
                for element, (
                    (global_left, global_right, _, _),
                    element_map,
                ) in enumerate(zip(ELEMENTS, maps, strict=True))
            ],
            "quadrature_exactness_proof": quadrature_proof_json(),
        },
        "accepted_points": points,
        "mutants": mutants,
        "self_checks_and_falsifiers": self_checks,
        "claim_boundary": {
            "claims": [
                "exact two-element one-dimensional uniform Q1 reference systems at the three stated parameter tuples",
                "exact two-point Gauss-Legendre integration for the degree-zero stiffness and degree-one load integrands",
                "exact strong Dirichlet free-DOF elimination, solve, determinants, residuals, and stated error bounds",
                "classification of the two required canonical CSR mutants",
            ],
            "nonclaims": [
                "no solver implementation, factorization-reuse behavior, timing, or performance is evaluated",
                "no mesh, element, quadrature, PDE, boundary condition, or parameter tuple beyond the frozen claim is generalized",
                "binary64 observations are not substitutes for exact assembly or the residual-contract error bound",
            ],
        },
    }


def render_reference(reference: dict[str, object]) -> str:
    return json.dumps(reference, indent=2, sort_keys=True) + "\n"


def expected_path() -> Path:
    return Path(__file__).resolve().parents[1] / "expected" / "symbolic.json"


def generate() -> None:
    reference = derive_reference()
    require(reference == derive_reference(), "derivation must be deterministic")
    destination = expected_path()
    destination.write_text(render_reference(reference), encoding="utf-8")
    print(f"generated {destination}")


def check() -> None:
    reference = derive_reference()
    require(reference == derive_reference(), "derivation must be deterministic")
    expected = render_reference(reference)
    destination = expected_path()
    actual = destination.read_text(encoding="utf-8")
    json.loads(actual)
    if actual != expected:
        difference = "".join(
            difflib.unified_diff(
                actual.splitlines(keepends=True),
                expected.splitlines(keepends=True),
                fromfile=str(destination),
                tofile="fresh exact derivation",
            )
        )
        raise AssertionError("checked-in symbolic reference is stale:\n" + difference)
    print(f"checked {destination}: exact deterministic reference matches")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--generate", action="store_true", help="write symbolic.json")
    mode.add_argument("--check", action="store_true", help="check symbolic.json")
    arguments = parser.parse_args()
    if arguments.generate:
        generate()
    else:
        check()


if __name__ == "__main__":
    main()
