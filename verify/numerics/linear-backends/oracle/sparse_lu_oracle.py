#!/usr/bin/env python3
"""Exact-rational oracle for the Eqiora ``SparseLu`` contract (Issue #126).

The companion fixture ``expected/sparse-lu-contract.json`` is frozen evidence
committed before any SparseLu implementation existed. This script re-derives
every value in that fixture from the stored CSR arrays alone, using exact
rational arithmetic, and fails if any recorded value disagrees.

It reads no Eqiora source, imports nothing outside the Python standard library,
and never constructs a binary floating-point number.

The fixture also records one acceptance threshold. The threshold is a choice and
is not derived here; what is derived here is the pair of walls it has to fall
between — the worst case binary64 rounding can produce, and the least wrong
result any frozen wrong route produces — together with every ordering comparison
that places it between them. Those comparisons are counted separately from the
equalities and reported by ``--summary``.

Usage:

    python3 sparse_lu_oracle.py [--fixture PATH] [--summary] [--verbose]
                                [--expect-digest HEX]

Exit status is 0 when every check passes and 1 otherwise. Any corruption of the
fixture is reported as a named check failure or as an explicit abort reason,
never as a traceback.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
import sys
from fractions import Fraction
from typing import Any

CASE_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DEFAULT_FIXTURE = os.path.join(CASE_DIR, "expected", "sparse-lu-contract.json")

SCHEMA = "eqiora.verify.numerics.linear-backends.sparse-lu-oracle/1"
ISSUE = 126

FALSIFIER_IDS = (
    "csr-read-as-csc",
    "omitted-off-diagonal",
    "one-based-column-indices",
    "rhs-permuted",
    "transpose-route-returns-normal-solution",
    "wrong-solution",
)

AXES = ("solver", "operator_property", "preconditioner", "reduction", "scalar")
TEST_PLAN_CASES = (
    "early-initial-guess-accepted",
    "initial-guess-not-accepted-early",
    "principal-positive-solve",
    "rank-deficient-fail-closed",
)
OPERATOR_PROPERTIES = ("General", "SymmetricIndefinite", "SymmetricPositiveDefinite")
PREEXISTING_TAGS = {"CG": 0, "BiCGSTAB": 1, "MINRES": 2}
ADDED_TAG = {"SparseLu": 3}

Matrix = list[list[Fraction]]
Vector = list[Fraction]


class FixtureError(Exception):
    """The fixture cannot be decoded at all."""


class OracleAbort(Exception):
    """A precondition that later checks depend on has failed."""

    def __init__(self, checks: "Checks", reason: str) -> None:
        super().__init__(reason)
        self.checks = checks
        self.reason = reason


# --------------------------------------------------------------------------
# fixture loading
# --------------------------------------------------------------------------


def _reject_float(literal: str) -> Any:
    raise FixtureError(f"binary floating-point literal in fixture: {literal!r}")


def _reject_constant(literal: str) -> Any:
    raise FixtureError(f"non-finite constant in fixture: {literal!r}")


def load_fixture(path: str) -> dict[str, Any]:
    with open(path, encoding="utf-8") as handle:
        return json.load(
            handle, parse_float=_reject_float, parse_constant=_reject_constant
        )


def rat(node: Any) -> Fraction:
    """Decode one ``{"num": int, "den": int}`` object."""
    if not isinstance(node, dict) or set(node) != {"num", "den"}:
        raise FixtureError(f"not a rational object: {node!r}")
    num, den = node["num"], node["den"]
    if not isinstance(num, int) or isinstance(num, bool):
        raise FixtureError(f"rational numerator must be an integer: {node!r}")
    if not isinstance(den, int) or isinstance(den, bool):
        raise FixtureError(f"rational denominator must be an integer: {node!r}")
    if den <= 0:
        raise FixtureError(f"rational denominator must be positive: {node!r}")
    return Fraction(num, den)


def ratvec(nodes: Any) -> Vector:
    if not isinstance(nodes, list):
        raise FixtureError(f"not a vector: {nodes!r}")
    return [rat(node) for node in nodes]


def intlist(nodes: Any, label: str) -> list[int]:
    if not isinstance(nodes, list) or not all(
        isinstance(v, int) and not isinstance(v, bool) for v in nodes
    ):
        raise FixtureError(f"{label} must be a list of integers")
    return list(nodes)


def walk_rationals(node: Any) -> list[dict[str, int]]:
    """Every ``{"num", "den"}`` object anywhere in the fixture."""
    found: list[dict[str, int]] = []
    if isinstance(node, dict):
        if set(node) == {"num", "den"}:
            found.append(node)
        else:
            for key in sorted(node):
                found.extend(walk_rationals(node[key]))
    elif isinstance(node, list):
        for item in node:
            found.extend(walk_rationals(item))
    return found


# --------------------------------------------------------------------------
# exact linear algebra, independent of how the fixture was produced
# --------------------------------------------------------------------------


def csr_to_dense(
    n: int, row_ptr: list[int], col_idx: list[int], values: Vector
) -> Matrix:
    dense: Matrix = [[Fraction(0)] * n for _ in range(n)]
    for row in range(n):
        for k in range(row_ptr[row], row_ptr[row + 1]):
            dense[row][col_idx[k]] = values[k]
    return dense


def csc_to_dense(n: int, ptr: list[int], idx: list[int], values: Vector) -> Matrix:
    """Read the same three arrays as if they were column-major storage."""
    dense: Matrix = [[Fraction(0)] * n for _ in range(n)]
    for col in range(n):
        for k in range(ptr[col], ptr[col + 1]):
            dense[idx[k]][col] = values[k]
    return dense


def transpose(matrix: Matrix) -> Matrix:
    n = len(matrix)
    return [[matrix[j][i] for j in range(n)] for i in range(n)]


def matvec(matrix: Matrix, vector: Vector) -> Vector:
    return [
        sum((row[j] * vector[j] for j in range(len(vector))), Fraction(0))
        for row in matrix
    ]


def vsub(left: Vector, right: Vector) -> Vector:
    return [a - b for a, b in zip(left, right)]


def squared_norm(vector: Vector) -> Fraction:
    return sum((v * v for v in vector), Fraction(0))


def dot(left: Vector, right: Vector) -> Fraction:
    return sum((a * b for a, b in zip(left, right)), Fraction(0))


def bareiss_determinant(matrix: Matrix) -> Fraction:
    """Fraction-free elimination: an elimination route distinct from the solver."""
    n = len(matrix)
    if n == 0:
        return Fraction(1)
    work = [row[:] for row in matrix]
    sign = 1
    previous = Fraction(1)
    for step in range(n - 1):
        if work[step][step] == 0:
            swap = next((r for r in range(step + 1, n) if work[r][step] != 0), None)
            if swap is None:
                return Fraction(0)
            work[step], work[swap] = work[swap], work[step]
            sign = -sign
        for row in range(step + 1, n):
            for col in range(step + 1, n):
                work[row][col] = (
                    work[row][col] * work[step][step]
                    - work[row][step] * work[step][col]
                ) / previous
            work[row][step] = Fraction(0)
        previous = work[step][step]
    return sign * work[n - 1][n - 1]


def gauss_jordan_solve(matrix: Matrix, rhs: Vector) -> Vector | None:
    """Exact solve; ``None`` when the matrix is singular."""
    n = len(matrix)
    work = [row[:] + [rhs[i]] for i, row in enumerate(matrix)]
    for col in range(n):
        pivot = next((r for r in range(col, n) if work[r][col] != 0), None)
        if pivot is None:
            return None
        work[col], work[pivot] = work[pivot], work[col]
        scale = work[col][col]
        work[col] = [v / scale for v in work[col]]
        for row in range(n):
            if row != col and work[row][col] != 0:
                factor = work[row][col]
                work[row] = [a - factor * b for a, b in zip(work[row], work[col])]
    return [work[i][n] for i in range(n)]


def exact_rank(matrix: Matrix) -> int:
    work = [row[:] for row in matrix]
    rows = len(work)
    cols = len(work[0]) if rows else 0
    rank = 0
    for col in range(cols):
        pivot = next((r for r in range(rank, rows) if work[r][col] != 0), None)
        if pivot is None:
            continue
        work[rank], work[pivot] = work[pivot], work[rank]
        scale = work[rank][col]
        work[rank] = [v / scale for v in work[rank]]
        for row in range(rows):
            if row != rank and work[row][col] != 0:
                factor = work[row][col]
                work[row] = [a - factor * b for a, b in zip(work[row], work[rank])]
        rank += 1
    return rank


def is_dyadic(value: Fraction) -> bool:
    """True when the value is exactly representable in binary floating point."""
    den = value.denominator
    return den & (den - 1) == 0


def dyadic_exponent(value: Fraction) -> int | None:
    """The ``k`` with ``value == 2 ** k``, or ``None`` when it is not a power."""
    if value <= 0:
        return None
    num, den = value.numerator, value.denominator
    if num & (num - 1) != 0 or den & (den - 1) != 0:
        return None
    return num.bit_length() - den.bit_length()


def matrix_infinity_norm(matrix: Matrix) -> Fraction:
    return max(
        (sum((abs(v) for v in row), Fraction(0)) for row in matrix), default=Fraction(0)
    )


def vector_infinity_norm(vector: Vector) -> Fraction:
    return max((abs(v) for v in vector), default=Fraction(0))


def exact_inverse(matrix: Matrix) -> Matrix | None:
    """``A**-1`` assembled from independent unit-vector solves."""
    n = len(matrix)
    columns: list[Vector] = []
    for j in range(n):
        unit = [Fraction(1) if i == j else Fraction(0) for i in range(n)]
        column = gauss_jordan_solve(matrix, unit)
        if column is None:
            return None
        columns.append(column)
    return [[columns[j][i] for j in range(n)] for i in range(n)]


def identity(n: int) -> Matrix:
    return [
        [Fraction(1) if i == j else Fraction(0) for j in range(n)] for i in range(n)
    ]


def matmul(left: Matrix, right: Matrix) -> Matrix:
    n = len(left)
    return [
        [
            sum((left[i][k] * right[k][j] for k in range(n)), Fraction(0))
            for j in range(n)
        ]
        for i in range(n)
    ]


def kebab_case(name: str) -> str:
    parts: list[str] = []
    current = ""
    for char in name:
        if char.isupper() and current:
            parts.append(current)
            current = char.lower()
        else:
            current += char.lower()
    if current:
        parts.append(current)
    return "-".join(parts)


def in_range(index: Any, limit: int) -> bool:
    return isinstance(index, int) and not isinstance(index, bool) and 0 <= index < limit


# --------------------------------------------------------------------------
# check bookkeeping
# --------------------------------------------------------------------------


RELATIONS = {
    "<": lambda a, b: a < b,
    "<=": lambda a, b: a <= b,
    ">": lambda a, b: a > b,
    ">=": lambda a, b: a >= b,
}


class Checks:
    def __init__(self) -> None:
        self.records: list[tuple[str, bool, str]] = []
        self.inequalities = 0

    def require(self, name: str, condition: bool, detail: str = "") -> bool:
        self.records.append((name, bool(condition), detail))
        return bool(condition)

    def equal(self, name: str, actual: Any, expected: Any) -> bool:
        ok = actual == expected
        detail = "" if ok else f"derived {actual!r} != recorded {expected!r}"
        return self.require(name, ok, detail)

    def prove(
        self, name: str, left: Any, relation: str, right: Any, detail: str = ""
    ) -> bool:
        """Record one ordering comparison between two exact rational magnitudes.

        Both operands must already be exact. A binary floating-point operand is
        an oracle or fixture defect rather than a failed check, so it aborts the
        decode instead of being reported as a disagreement.
        """
        for side in (left, right):
            if isinstance(side, bool) or not isinstance(side, (Fraction, int)):
                raise FixtureError(f"inexact operand in inequality {name}: {side!r}")
        ok = RELATIONS[relation](left, right)
        self.inequalities += 1
        if not ok:
            detail = detail or f"{left} {relation} {right} is false"
        return self.require(name, ok, "" if ok else detail)

    @property
    def failures(self) -> list[tuple[str, bool, str]]:
        return [record for record in self.records if not record[1]]


# --------------------------------------------------------------------------
# structural checks shared by both systems
# --------------------------------------------------------------------------


def check_csr_structure(
    checks: Checks, tag: str, n: int, csr: dict[str, Any]
) -> Vector:
    """Structural gate. Aborts when densification would be undefined."""
    row_ptr = intlist(csr["row_ptr"], f"{tag}.row_ptr")
    col_idx = intlist(csr["col_idx"], f"{tag}.col_idx")
    values = ratvec(csr["values"])

    checks.equal(f"{tag}.index-base-zero", csr["index_base"], 0)
    usable = [
        checks.equal(f"{tag}.row-ptr-length", len(row_ptr), n + 1),
        checks.require(
            f"{tag}.row-ptr-starts-at-zero", bool(row_ptr) and row_ptr[0] == 0
        ),
        checks.require(
            f"{tag}.row-ptr-ends-at-nnz", bool(row_ptr) and row_ptr[-1] == len(col_idx)
        ),
        checks.equal(
            f"{tag}.value-count-matches-index-count", len(values), len(col_idx)
        ),
        checks.require(
            f"{tag}.row-ptr-non-decreasing",
            all(row_ptr[i] <= row_ptr[i + 1] for i in range(len(row_ptr) - 1)),
        ),
        checks.require(
            f"{tag}.columns-in-range",
            all(0 <= j < n for j in col_idx),
            f"every column index must lie in [0, {n})",
        ),
    ]
    if not all(usable):
        raise OracleAbort(checks, f"{tag} CSR arrays are not interpretable")

    checks.require(
        f"{tag}.no-structurally-empty-row",
        all(row_ptr[i + 1] > row_ptr[i] for i in range(n)),
        "every row must own at least one stored entry",
    )
    checks.require(
        f"{tag}.columns-sorted-and-unique",
        all(
            col_idx[k] < col_idx[k + 1]
            for i in range(n)
            for k in range(row_ptr[i], row_ptr[i + 1] - 1)
        ),
        "column indices must be strictly increasing within each row",
    )
    checks.require(
        f"{tag}.all-stored-values-nonzero",
        all(v != 0 for v in values),
        "canonical storage must not hold an explicit zero",
    )
    checks.require(
        f"{tag}.values-exactly-representable-in-binary64",
        all(is_dyadic(v) for v in values),
        "stored values must round-trip through f64 without error",
    )
    return values


# --------------------------------------------------------------------------
# principal witness
# --------------------------------------------------------------------------


def check_principal(checks: Checks, node: dict[str, Any]) -> dict[str, Any]:
    n = node["n"]
    if not isinstance(n, int) or isinstance(n, bool) or n < 1:
        raise OracleAbort(checks, "principal dimension is not a positive integer")
    csr = node["csr"]
    values = check_csr_structure(checks, "principal", n, csr)
    row_ptr = intlist(csr["row_ptr"], "principal.row_ptr")
    col_idx = intlist(csr["col_idx"], "principal.col_idx")

    matrix = csr_to_dense(n, row_ptr, col_idx, values)
    matrix_t = transpose(matrix)
    rhs = ratvec(node["rhs"])
    solution = ratvec(node["solution"])
    transpose_solution = ratvec(node["transpose_solution"])

    lengths = [
        checks.equal("principal.rhs-length", len(rhs), n),
        checks.equal("principal.solution-length", len(solution), n),
        checks.equal("principal.transpose-solution-length", len(transpose_solution), n),
    ]
    if not all(lengths):
        raise OracleAbort(checks, "principal vectors do not match the dimension")

    structure = node["structure"]
    checks.equal("principal.recorded-nnz", structure["nnz"], len(col_idx))
    checks.equal("principal.square", len(matrix), len(matrix[0]))

    pattern = {
        (i, col_idx[k]) for i in range(n) for k in range(row_ptr[i], row_ptr[i + 1])
    }
    checks.require(
        "principal.diagonal-fully-present",
        all((i, i) in pattern for i in range(n)),
    )
    checks.equal("principal.min-column-index", min(col_idx), 0)
    checks.equal("principal.max-column-index", max(col_idx), n - 1)
    checks.equal(
        "principal.recorded-min-column-index",
        structure["min_column_index"],
        min(col_idx),
    )
    checks.equal(
        "principal.recorded-max-column-index",
        structure["max_column_index"],
        max(col_idx),
    )

    pattern_witnesses = sorted((i, j) for (i, j) in pattern if (j, i) not in pattern)
    checks.require(
        "principal.not-structurally-symmetric",
        len(pattern_witnesses) > 0,
        "the sparsity pattern must differ from its own transpose",
    )
    checks.equal(
        "principal.pattern-asymmetry-witnesses",
        [list(pair) for pair in pattern_witnesses],
        structure["pattern_asymmetry_witnesses"],
    )
    checks.equal(
        "principal.recorded-structurally-symmetric",
        structure["structurally_symmetric"],
        False,
    )

    checks.require(
        "principal.not-numerically-symmetric",
        matrix != matrix_t,
        "the operator must differ from its own transpose",
    )
    witnesses = structure["value_asymmetry_witnesses"]
    checks.require(
        "principal.value-asymmetry-witnesses-are-genuine",
        all(
            isinstance(pair, list)
            and len(pair) == 2
            and in_range(pair[0], n)
            and in_range(pair[1], n)
            and (pair[0], pair[1]) in pattern
            and (pair[1], pair[0]) in pattern
            and matrix[pair[0]][pair[1]] != matrix[pair[1]][pair[0]]
            for pair in witnesses
        ),
        "each recorded witness must be a stored pair whose two values differ",
    )
    checks.equal(
        "principal.recorded-numerically-symmetric",
        structure["numerically_symmetric"],
        False,
    )

    determinant = bareiss_determinant(matrix)
    checks.equal("principal.determinant", determinant, rat(node["determinant"]))
    checks.require(
        "principal.nonsingular",
        determinant != 0,
        "a unique solution requires det A != 0",
    )
    checks.equal(
        "principal.transpose-determinant", bareiss_determinant(matrix_t), determinant
    )

    normal_residual = vsub(rhs, matvec(matrix, solution))
    checks.require(
        "principal.solution-satisfies-normal-route",
        all(component == 0 for component in normal_residual),
        f"b - A x = {[str(v) for v in normal_residual]}",
    )
    checks.equal(
        "principal.normal-route-residual-squared",
        squared_norm(normal_residual),
        rat(node["residuals"]["normal_route_squared"]),
    )

    transpose_residual = vsub(rhs, matvec(matrix_t, transpose_solution))
    checks.require(
        "principal.transpose-solution-satisfies-transpose-route",
        all(component == 0 for component in transpose_residual),
        f"b - A^T y = {[str(v) for v in transpose_residual]}",
    )
    checks.equal(
        "principal.transpose-route-residual-squared",
        squared_norm(transpose_residual),
        rat(node["residuals"]["transpose_route_squared"]),
    )

    checks.equal(
        "principal.solution-is-unique", gauss_jordan_solve(matrix, rhs), solution
    )
    checks.equal(
        "principal.transpose-solution-is-unique",
        gauss_jordan_solve(matrix_t, rhs),
        transpose_solution,
    )

    difference = vsub(solution, transpose_solution)
    checks.require(
        "principal.solutions-differ-in-every-component",
        all(component != 0 for component in difference),
        "x_i != y_i must hold for every i so orientation errors cannot hide",
    )
    checks.equal(
        "principal.componentwise-solution-difference",
        difference,
        ratvec(node["componentwise_solution_difference"]),
    )
    checks.require(
        "principal.solutions-exactly-representable-in-binary64",
        all(is_dyadic(v) for v in solution + transpose_solution + rhs),
        "expected vectors must round-trip through f64 without error",
    )

    return {
        "n": n,
        "matrix": matrix,
        "matrix_t": matrix_t,
        "rhs": rhs,
        "solution": solution,
        "transpose_solution": transpose_solution,
        "determinant": determinant,
        "row_ptr": row_ptr,
        "col_idx": col_idx,
        "values": values,
        "pattern": pattern,
    }


# --------------------------------------------------------------------------
# acceptance threshold
# --------------------------------------------------------------------------


def check_acceptance(
    checks: Checks, principal: dict[str, Any], ctx: dict[str, Any]
) -> dict[str, Any]:
    """The chosen threshold and every bound it is required to sit between.

    The threshold is a choice, so nothing here derives it. Everything here
    derives the two walls it must fall between — what rounding can produce and
    what the frozen wrong routes produce — and proves the choice clears both.
    """
    node = principal["acceptance"]
    matrix, solution, n = ctx["matrix"], ctx["solution"], ctx["n"]

    relative = rat(node["relative_tolerance"])
    checks.equal("acceptance.relative-tolerance-is-exactly-zero", relative, Fraction(0))

    tolerance = rat(node["absolute_tolerance"])
    checks.prove(
        "acceptance.absolute-tolerance-is-positive", tolerance, ">", Fraction(0)
    )
    checks.require(
        "acceptance.absolute-tolerance-is-dyadic",
        is_dyadic(tolerance),
        "the threshold must be exactly representable in binary64",
    )
    checks.equal(
        "acceptance.absolute-tolerance-exponent",
        dyadic_exponent(tolerance),
        node["absolute_tolerance_exponent"],
    )
    squared = tolerance * tolerance
    checks.equal(
        "acceptance.absolute-tolerance-squared",
        squared,
        rat(node["absolute_tolerance_squared"]),
    )

    # The selection rule, restated as the two inequalities that determine it.
    nano = Fraction(1, 10**9)
    checks.prove("acceptance.selection-is-below-1e-9", tolerance, "<", nano)
    checks.prove(
        "acceptance.selection-is-the-largest-such-power-of-two",
        tolerance * 2,
        ">=",
        nano,
    )

    # Lower wall: what a backward-stable binary64 factorization can produce.
    envelope = node["backward_error_envelope"]
    roundoff = rat(envelope["unit_roundoff"])
    checks.equal("acceptance.unit-roundoff-is-binary64", roundoff, Fraction(1, 2**53))
    checks.equal(
        "acceptance.envelope-dimension-factor", envelope["dimension_factor"], 3 * n**3
    )
    checks.equal(
        "acceptance.envelope-growth-factor", envelope["growth_factor"], 2 ** (n - 1)
    )
    matrix_norm = matrix_infinity_norm(matrix)
    solution_norm = vector_infinity_norm(solution)
    checks.equal(
        "acceptance.envelope-matrix-infinity-norm",
        matrix_norm,
        rat(envelope["matrix_infinity_norm"]),
    )
    checks.equal(
        "acceptance.envelope-solution-infinity-norm",
        solution_norm,
        rat(envelope["solution_infinity_norm"]),
    )
    derived = (
        Fraction(envelope["dimension_factor"])
        * envelope["growth_factor"]
        * roundoff
        * matrix_norm
        * solution_norm
    )
    checks.equal("acceptance.envelope-value", derived, rat(envelope["value"]))
    checks.prove("acceptance.tolerance-clears-the-envelope", tolerance, ">", derived)
    checks.prove(
        "acceptance.tolerance-clears-the-envelope-with-margin",
        tolerance,
        ">=",
        derived * envelope["tolerance_exceeds_envelope_by_at_least"],
        "the threshold must sit well above the worst case rounding can produce",
    )

    # Upper wall: the least wrong thing any frozen route produces.
    discrimination = node["discrimination"]
    wrong_routes = [rat(f["residual_squared"]) for f in principal["falsifiers"]]
    wrong_routes.append(
        rat(principal["initial_guesses"]["not_satisfied"]["residual_squared"])
    )
    smallest = min(wrong_routes)
    checks.equal(
        "acceptance.smallest-wrong-route-residual-squared",
        smallest,
        rat(discrimination["smallest_wrong_route_residual_squared"]),
    )
    margin_factor = discrimination["margin_factor_at_least"]
    checks.prove(
        "acceptance.tolerance-rejects-every-wrong-route", squared, "<", smallest
    )
    checks.prove(
        "acceptance.tolerance-rejects-every-wrong-route-with-margin",
        smallest,
        ">=",
        squared * margin_factor,
    )

    # Solution accuracy is a separate statement from residual acceptance.
    forward = node["forward_error_ceiling"]
    ceiling = rat(forward["ceiling"])
    inverse = exact_inverse(matrix)
    if inverse is None:
        checks.require(
            "acceptance.inverse-exists", False, "a forward bound needs a nonsingular A"
        )
        raise OracleAbort(checks, "the principal matrix has no inverse")
    checks.equal(
        "acceptance.inverse-is-a-genuine-inverse", matmul(matrix, inverse), identity(n)
    )
    inverse_norm = matrix_infinity_norm(inverse)
    checks.equal(
        "acceptance.inverse-infinity-norm",
        inverse_norm,
        rat(forward["inverse_infinity_norm"]),
    )
    implied = inverse_norm * tolerance
    checks.equal(
        "acceptance.forward-error-implied-bound", implied, rat(forward["implied_bound"])
    )
    checks.prove(
        "acceptance.forward-error-ceiling-is-positive", ceiling, ">", Fraction(0)
    )
    checks.require("acceptance.forward-error-ceiling-is-dyadic", is_dyadic(ceiling))
    checks.equal(
        "acceptance.forward-error-ceiling-exponent",
        dyadic_exponent(ceiling),
        forward["ceiling_exponent"],
    )
    checks.prove(
        "acceptance.forward-error-ceiling-covers-implied-bound",
        ceiling,
        ">=",
        implied,
        "residual acceptance must actually imply the componentwise ceiling",
    )
    checks.prove(
        "acceptance.forward-error-ceiling-is-the-tightest-power-of-two",
        ceiling / 2,
        "<",
        implied,
        "a tighter power of two would not be implied by residual acceptance",
    )
    checks.equal(
        "acceptance.forward-error-ceiling-tightness-is-recorded",
        forward["ceiling_is_tightest_dyadic_power_of_two"],
        True,
    )

    return {
        "tolerance": tolerance,
        "squared": squared,
        "ceiling": ceiling,
        "margin_factor": margin_factor,
        "relative": relative,
    }


def check_initial_guesses(
    checks: Checks, node: dict[str, Any], ctx: dict[str, Any]
) -> None:
    matrix, rhs, n = ctx["matrix"], ctx["rhs"], ctx["n"]

    satisfied = node["already_satisfied"]
    guess = ratvec(satisfied["vector"])
    if not checks.equal("guess.already-satisfied-length", len(guess), n):
        raise OracleAbort(checks, "initial guess does not match the dimension")
    residual = vsub(rhs, matvec(matrix, guess))
    checks.require(
        "guess.already-satisfied-residual-is-exactly-zero",
        all(component == 0 for component in residual),
        f"b - A x0 = {[str(v) for v in residual]}",
    )
    checks.equal(
        "guess.already-satisfied-residual-squared",
        squared_norm(residual),
        rat(satisfied["residual_squared"]),
    )
    checks.equal("guess.already-satisfied-equals-solution", guess, ctx["solution"])
    checks.prove(
        "guess.already-satisfied-is-accepted-early",
        squared_norm(residual),
        "<=",
        ctx["squared"],
        "the exact solution must clear the squared threshold and skip factorization",
    )

    unsatisfied = node["not_satisfied"]
    other = ratvec(unsatisfied["vector"])
    if not checks.equal("guess.not-satisfied-length", len(other), n):
        raise OracleAbort(checks, "rejected guess does not match the dimension")
    other_residual = vsub(rhs, matvec(matrix, other))
    checks.equal(
        "guess.not-satisfied-residual-squared",
        squared_norm(other_residual),
        rat(unsatisfied["residual_squared"]),
    )
    checks.prove(
        "guess.not-satisfied-residual-is-positive",
        squared_norm(other_residual),
        ">",
        Fraction(0),
        "the rejected guess must have a strictly positive residual",
    )
    checks.prove(
        "guess.not-satisfied-is-not-accepted-early",
        squared_norm(other_residual),
        ">",
        ctx["squared"],
        "the branch is only observable if this guess fails the same threshold",
    )
    checks.prove(
        "guess.not-satisfied-is-not-accepted-early-with-margin",
        squared_norm(other_residual),
        ">=",
        ctx["squared"] * ctx["margin_factor"],
    )


# --------------------------------------------------------------------------
# falsifiers
# --------------------------------------------------------------------------


def check_falsifiers(
    checks: Checks, entries: list[dict[str, Any]], ctx: dict[str, Any]
) -> None:
    n = ctx["n"]
    matrix, matrix_t, rhs = ctx["matrix"], ctx["matrix_t"], ctx["rhs"]
    solution = ctx["solution"]
    by_id = {entry["id"]: entry for entry in entries}

    unique = checks.equal("falsifier.ids-unique", len(by_id), len(entries))
    complete = checks.equal(
        "falsifier.expected-set", sorted(by_id), sorted(FALSIFIER_IDS)
    )
    if not (unique and complete):
        raise OracleAbort(checks, "the falsifier set is not the frozen set")

    for entry in entries:
        residual_squared = rat(entry["residual_squared"])
        checks.prove(
            f"falsifier.{entry['id']}.residual-is-positive",
            residual_squared,
            ">",
            Fraction(0),
            "a falsifier must produce a strictly positive true residual",
        )
        checks.prove(
            f"falsifier.{entry['id']}.is-rejected-by-the-threshold",
            residual_squared,
            ">",
            ctx["squared"],
            "the threshold is useless against a route it would accept",
        )
        checks.prove(
            f"falsifier.{entry['id']}.is-rejected-with-margin",
            residual_squared,
            ">=",
            ctx["squared"] * ctx["margin_factor"],
        )

    check_csc_falsifier(checks, by_id["csr-read-as-csc"], ctx)
    check_transpose_falsifier(
        checks,
        by_id["transpose-route-returns-normal-solution"],
        matrix_t,
        rhs,
        solution,
    )
    check_permutation_falsifier(checks, by_id["rhs-permuted"], ctx)
    check_one_based_falsifier(checks, by_id["one-based-column-indices"], ctx)
    check_omission_falsifier(checks, by_id["omitted-off-diagonal"], ctx)

    entry = by_id["wrong-solution"]
    perturbation = entry["perturbation"]
    if not checks.require(
        "falsifier.wrong-solution.perturbation-index-in-range",
        in_range(perturbation["index"], n),
    ):
        return
    expected_wrong = list(solution)
    expected_wrong[perturbation["index"]] += rat(perturbation["delta"])
    checks.equal(
        "falsifier.wrong-solution.wrong-vector-is-perturbed-solution",
        ratvec(entry["wrong_vector"]),
        expected_wrong,
    )
    checks.equal(
        "falsifier.wrong-solution.residual-squared",
        squared_norm(vsub(rhs, matvec(matrix, expected_wrong))),
        rat(entry["residual_squared"]),
    )


def check_csc_falsifier(
    checks: Checks, entry: dict[str, Any], ctx: dict[str, Any]
) -> None:
    """CSR arrays consumed as column-major storage silently transpose A."""
    misread = csc_to_dense(ctx["n"], ctx["row_ptr"], ctx["col_idx"], ctx["values"])
    checks.equal(
        "falsifier.csr-read-as-csc.misread-equals-transpose", misread, ctx["matrix_t"]
    )
    checks.equal(
        "falsifier.csr-read-as-csc.recorded-misread-claim",
        entry["misread_matrix_equals_transpose"],
        True,
    )
    wrong = ratvec(entry["wrong_vector"])
    checks.equal(
        "falsifier.csr-read-as-csc.wrong-vector-solves-misread-system",
        gauss_jordan_solve(misread, ctx["rhs"]),
        wrong,
    )
    if len(wrong) != ctx["n"]:
        return
    checks.equal(
        "falsifier.csr-read-as-csc.residual-squared",
        squared_norm(vsub(ctx["rhs"], matvec(ctx["matrix"], wrong))),
        rat(entry["residual_squared"]),
    )


def check_transpose_falsifier(
    checks: Checks,
    entry: dict[str, Any],
    matrix_t: Matrix,
    rhs: Vector,
    solution: Vector,
) -> None:
    """The transpose route must not return the normal-route solution."""
    checks.equal(
        "falsifier.transpose-route.wrong-vector-is-normal-solution",
        ratvec(entry["wrong_vector"]),
        solution,
    )
    checks.equal(
        "falsifier.transpose-route.residual-squared",
        squared_norm(vsub(rhs, matvec(matrix_t, solution))),
        rat(entry["residual_squared"]),
    )


def check_permutation_falsifier(
    checks: Checks, entry: dict[str, Any], ctx: dict[str, Any]
) -> None:
    """A right-hand side reordered relative to the matrix rows."""
    n, rhs, matrix = ctx["n"], ctx["rhs"], ctx["matrix"]
    permutation = intlist(entry["permutation"], "falsifier.rhs-permuted.permutation")
    valid = checks.equal(
        "falsifier.rhs-permuted.is-a-permutation", sorted(permutation), list(range(n))
    )
    checks.require(
        "falsifier.rhs-permuted.is-not-the-identity", permutation != list(range(n))
    )
    if not valid:
        return
    permuted = [rhs[permutation[i]] for i in range(n)]
    checks.equal(
        "falsifier.rhs-permuted.permuted-rhs", permuted, ratvec(entry["permuted_rhs"])
    )
    checks.require(
        "falsifier.rhs-permuted.permuted-rhs-differs",
        permuted != rhs,
        "the permutation must actually move the right-hand side",
    )
    wrong = ratvec(entry["wrong_vector"])
    checks.equal(
        "falsifier.rhs-permuted.wrong-vector-solves-permuted-system",
        gauss_jordan_solve(matrix, permuted),
        wrong,
    )
    if len(wrong) != n:
        return
    checks.equal(
        "falsifier.rhs-permuted.residual-squared",
        squared_norm(vsub(rhs, matvec(matrix, wrong))),
        rat(entry["residual_squared"]),
    )


def check_one_based_falsifier(
    checks: Checks, entry: dict[str, Any], ctx: dict[str, Any]
) -> None:
    """One-based column indices, in both directions plus a lenient decode."""
    n, col_idx, row_ptr = ctx["n"], ctx["col_idx"], ctx["row_ptr"]
    negatives = sum(1 for j in col_idx if j - 1 < 0)
    out_of_range = sum(1 for j in col_idx if j + 1 >= n)
    checks.equal(
        "falsifier.one-based.shift-down-negative-count",
        negatives,
        entry["shift_down_negative_count"],
    )
    checks.equal(
        "falsifier.one-based.shift-up-out-of-range-count",
        out_of_range,
        entry["shift_up_out_of_range_count"],
    )
    checks.require(
        "falsifier.one-based.detectable-shifting-down",
        negatives > 0,
        "reading stored zero-based indices as one-based must underflow",
    )
    checks.require(
        "falsifier.one-based.detectable-shifting-up",
        out_of_range > 0,
        "reading one-based indices as zero-based must overflow the column range",
    )

    lenient: Matrix = [[Fraction(0)] * n for _ in range(n)]
    kept = 0
    for row in range(n):
        for k in range(row_ptr[row], row_ptr[row + 1]):
            shifted = col_idx[k] - 1
            if shifted >= 0:
                lenient[row][shifted] = ctx["values"][k]
                kept += 1
    checks.equal(
        "falsifier.one-based.lenient-decode-kept-entries",
        kept,
        entry["lenient_decode_kept_entries"],
    )
    checks.require(
        "falsifier.one-based.lenient-decode-keeps-every-row-nonempty",
        all(any(v != 0 for v in row) for row in lenient),
        "the lenient decode stays structurally plausible, so only arithmetic detects it",
    )
    checks.equal(
        "falsifier.one-based.lenient-decode-determinant",
        bareiss_determinant(lenient),
        rat(entry["lenient_decode_determinant"]),
    )
    checks.equal(
        "falsifier.one-based.residual-squared",
        squared_norm(vsub(ctx["rhs"], matvec(lenient, ctx["solution"]))),
        rat(entry["residual_squared"]),
    )


def check_omission_falsifier(
    checks: Checks, entry: dict[str, Any], ctx: dict[str, Any]
) -> None:
    """A dropped off-diagonal entry."""
    n, matrix, rhs, solution = ctx["n"], ctx["matrix"], ctx["rhs"], ctx["solution"]
    omitted = entry["omitted_entry"]
    row, col = omitted["row"], omitted["col"]
    if not checks.require(
        "falsifier.omitted-off-diagonal.entry-position-in-range",
        in_range(row, n) and in_range(col, n),
    ):
        return
    value = rat(omitted["value"])
    checks.require(
        "falsifier.omitted-off-diagonal.entry-is-stored-off-diagonal",
        row != col and (row, col) in ctx["pattern"] and matrix[row][col] == value,
    )
    checks.require(
        "falsifier.omitted-off-diagonal.entry-breaks-structural-symmetry",
        (col, row) not in ctx["pattern"],
        "the dropped entry is one that has no transpose partner",
    )
    reduced = [r[:] for r in matrix]
    reduced[row][col] = Fraction(0)
    reduced_determinant = bareiss_determinant(reduced)
    checks.equal(
        "falsifier.omitted-off-diagonal.reduced-determinant",
        reduced_determinant,
        rat(entry["reduced_determinant"]),
    )
    checks.require(
        "falsifier.omitted-off-diagonal.reduced-system-still-solvable",
        reduced_determinant != 0,
        "the omission must yield a different answer rather than an obvious failure",
    )
    wrong = ratvec(entry["wrong_vector"])
    checks.equal(
        "falsifier.omitted-off-diagonal.wrong-vector-solves-reduced-system",
        gauss_jordan_solve(reduced, rhs),
        wrong,
    )
    checks.require(
        "falsifier.omitted-off-diagonal.wrong-vector-differs-everywhere",
        len(wrong) == n and all(wrong[i] != solution[i] for i in range(n)),
    )
    checks.equal(
        "falsifier.omitted-off-diagonal.residual-squared",
        squared_norm(vsub(rhs, matvec(reduced, solution))),
        rat(entry["residual_squared"]),
    )


# --------------------------------------------------------------------------
# rank-deficient witness
# --------------------------------------------------------------------------


def check_rank_deficient(
    checks: Checks, node: dict[str, Any], analysed: dict[str, Any]
) -> dict[str, Any]:
    n = node["n"]
    if not isinstance(n, int) or isinstance(n, bool) or n < 3:
        raise OracleAbort(checks, "rank-deficient dimension is not usable")
    csr = node["csr"]
    values = check_csr_structure(checks, "rank-deficient", n, csr)
    matrix = csr_to_dense(
        n,
        intlist(csr["row_ptr"], "rank-deficient.row_ptr"),
        intlist(csr["col_idx"], "rank-deficient.col_idx"),
        values,
    )
    matrix_t = transpose(matrix)
    rhs = ratvec(node["rhs"])
    if not checks.equal("rank-deficient.rhs-length", len(rhs), n):
        raise OracleAbort(checks, "rank-deficient right-hand side length is wrong")

    checks.equal(
        "rank-deficient.recorded-nnz", node["structure"]["nnz"], len(csr["col_idx"])
    )
    determinant = bareiss_determinant(matrix)
    checks.equal("rank-deficient.determinant", determinant, rat(node["determinant"]))
    checks.require(
        "rank-deficient.is-singular", determinant == 0, "the witness must be singular"
    )

    proof = node["rank_deficiency_proof"]
    right_null = ratvec(proof["right_null_vector"])
    left_null = ratvec(proof["left_null_vector"])
    if not checks.require(
        "rank-deficient.null-vector-lengths",
        len(right_null) == n and len(left_null) == n,
    ):
        raise OracleAbort(checks, "null vectors do not match the dimension")

    right_nonzero = checks.require(
        "rank-deficient.right-null-vector-is-nonzero", any(v != 0 for v in right_null)
    )
    checks.require(
        "rank-deficient.right-null-vector-is-annihilated",
        all(v == 0 for v in matvec(matrix, right_null)),
        f"A v = {[str(v) for v in matvec(matrix, right_null)]}",
    )
    left_nonzero = checks.require(
        "rank-deficient.left-null-vector-is-nonzero", any(v != 0 for v in left_null)
    )
    checks.require(
        "rank-deficient.left-null-vector-is-annihilated",
        all(v == 0 for v in matvec(matrix_t, left_null)),
        f"A^T w = {[str(v) for v in matvec(matrix_t, left_null)]}",
    )
    checks.require(
        "rank-deficient.null-vectors-prove-deficiency",
        right_nonzero and left_nonzero,
        "a rank-deficiency proof needs a nonzero vector on each side",
    )

    minor = proof["nonsingular_minor"]
    minor_rows = intlist(minor["rows"], "rank-deficient.minor.rows")
    minor_cols = intlist(minor["cols"], "rank-deficient.minor.cols")
    minor_ok = checks.require(
        "rank-deficient.minor-indices-in-range",
        len(minor_rows) == len(minor_cols)
        and all(in_range(i, n) for i in minor_rows)
        and all(in_range(j, n) for j in minor_cols),
    )
    rank = exact_rank(matrix)
    if minor_ok:
        submatrix = [[matrix[i][j] for j in minor_cols] for i in minor_rows]
        minor_determinant = bareiss_determinant(submatrix)
        checks.equal(
            "rank-deficient.minor-determinant",
            minor_determinant,
            rat(minor["determinant"]),
        )
        checks.require(
            "rank-deficient.minor-is-nonsingular",
            minor_determinant != 0,
            "a nonzero minor of this size forces rank >= its size",
        )
        checks.equal("rank-deficient.rank-matches-minor-bound", rank, len(minor_rows))

    checks.equal("rank-deficient.rank", rank, node["rank"])
    checks.require(
        "rank-deficient.rank-is-deficient", rank < n, "rank must be below the dimension"
    )
    checks.require(
        "rank-deficient.row-dependency",
        [matrix[0][j] + matrix[1][j] for j in range(n)] == matrix[2],
        "row 2 must equal row 0 plus row 1 exactly",
    )
    checks.equal(
        "rank-deficient.left-null-space-is-one-dimensional", n - exact_rank(matrix_t), 1
    )

    consistency = node["consistency"]
    projection = dot(left_null, rhs)
    checks.equal(
        "rank-deficient.left-null-dot-rhs",
        projection,
        rat(consistency["left_null_vector_dot_rhs"]),
    )
    checks.require(
        "rank-deficient.rhs-is-inconsistent",
        projection != 0,
        "the right-hand side must leave the column space so no solution exists",
    )
    checks.equal(
        "rank-deficient.recorded-consistent-flag", consistency["consistent"], False
    )
    minimum = Fraction(0)
    if left_nonzero:
        minimum = projection * projection / squared_norm(left_null)
        checks.equal(
            "rank-deficient.minimum-squared-residual",
            minimum,
            rat(consistency["minimum_squared_residual"]),
        )
        checks.prove(
            "rank-deficient.minimum-squared-residual-is-positive",
            minimum,
            ">",
            Fraction(0),
            "no vector can attain a zero residual, so a solver must fail closed",
        )
        check_rank_deficient_acceptance(
            checks, node["acceptance"], minimum, analysed["tolerance"]
        )
    return {
        "n": n,
        "rank": rank,
        "nnz": len(csr["col_idx"]),
        "minimum": minimum,
    }


def check_rank_deficient_acceptance(
    checks: Checks, node: dict[str, Any], minimum: Fraction, analysed: Fraction
) -> None:
    """The squared acceptance target must sit strictly below what is attainable."""
    tolerance = rat(node["absolute_tolerance"])
    checks.equal(
        "rank-deficient.acceptance-uses-the-analysed-tolerance", tolerance, analysed
    )
    squared = tolerance * tolerance
    checks.equal(
        "rank-deficient.acceptance-tolerance-squared",
        squared,
        rat(node["absolute_tolerance_squared"]),
    )
    checks.equal(
        "rank-deficient.acceptance-records-the-derived-minimum",
        minimum,
        rat(node["minimum_attainable_residual_squared"]),
    )
    checks.prove(
        "rank-deficient.acceptance-target-is-below-the-attainable-minimum",
        squared,
        "<",
        minimum,
        "if the target were reachable the system would not force a closed failure",
    )
    checks.prove(
        "rank-deficient.acceptance-target-is-below-the-minimum-with-margin",
        minimum,
        ">=",
        squared * node["margin_factor_at_least"],
    )


# --------------------------------------------------------------------------
# contract expectations (not mathematical facts)
# --------------------------------------------------------------------------


def tuple_key(entry: dict[str, Any]) -> tuple[str, ...]:
    return tuple(entry.get(axis) for axis in AXES)


def check_test_plan(
    checks: Checks,
    node: dict[str, Any],
    analysed: dict[str, Any],
    positive_keys: set[tuple[str, ...]],
) -> int:
    """Shape of the transcribed plan, and its binding to the analysed numbers.

    Nothing here is proved about Eqiora. The load-bearing check is that every
    tolerance and ceiling the plan declares is exactly the rational the
    mathematics section proved bounds about, so no number can be wired that was
    never analysed, and no analysed number can be silently replaced.
    """
    plan = node["plan"]
    checks.equal("plan.solver", plan["solver"], "SparseLu")
    checks.equal("plan.operator-property", plan["operator_property"], "General")
    checks.equal("plan.preconditioner", plan["preconditioner"], "Identity")
    checks.equal("plan.reduction", plan["reduction"], "Fast")
    checks.equal("plan.scalar", plan["scalar"], "F64")
    checks.equal("plan.maximum-iterations", plan["maximum_iterations"], 1)
    checks.equal(
        "plan.relative-tolerance-is-exactly-zero",
        rat(plan["relative_tolerance"]),
        Fraction(0),
    )
    checks.equal(
        "plan.absolute-tolerance-is-the-analysed-tolerance",
        rat(plan["absolute_tolerance"]),
        analysed["tolerance"],
    )
    checks.require(
        "plan.tuple-is-in-the-positive-capability-set",
        tuple_key(plan) in positive_keys,
        "the plan under test must be one the contract declares supported",
    )

    cases = node["cases"]
    by_id = {case["id"]: case for case in cases}
    unique = checks.equal("plan.case-ids-unique", len(by_id), len(cases))
    complete = checks.equal("plan.case-set", sorted(by_id), sorted(TEST_PLAN_CASES))
    if not (unique and complete):
        raise OracleAbort(checks, "the test plan case set is not the frozen set")

    for case in cases:
        cid = case["id"]
        checks.require(
            f"plan.{cid}.expected-outcome-is-known",
            case["expected"] in ("accepted", "rejected"),
        )
        checks.equal(
            f"plan.{cid}.squared-target-is-the-analysed-tolerance-squared",
            rat(case["expected_residual_squared_at_most"]),
            analysed["squared"],
        )
        checks.require(
            f"plan.{cid}.early-exit-expectation-is-boolean",
            isinstance(case["expected_early_exit"], bool),
        )
        if "componentwise_solution_error_ceiling" in case:
            checks.equal(
                f"plan.{cid}.ceiling-is-the-analysed-ceiling",
                rat(case["componentwise_solution_error_ceiling"]),
                analysed["ceiling"],
            )

    checks.equal(
        "plan.exactly-one-case-takes-the-early-exit",
        [case["id"] for case in cases if case["expected_early_exit"]],
        ["early-initial-guess-accepted"],
    )
    checks.equal(
        "plan.exactly-one-case-fails-closed",
        [case["id"] for case in cases if case["expected"] == "rejected"],
        ["rank-deficient-fail-closed"],
    )
    checks.require(
        "plan.accepted-cases-expect-the-exact-solution",
        all(
            case.get("expected_solution") == "mathematics.principal.solution"
            for case in cases
            if case["expected"] == "accepted"
        ),
    )
    checks.require(
        "plan.accepted-cases-carry-a-componentwise-ceiling",
        all(
            "componentwise_solution_error_ceiling" in case
            for case in cases
            if case["expected"] == "accepted"
        ),
        "residual acceptance alone does not bound the returned components",
    )
    checks.require(
        "plan.accepted-cases-target-the-principal-witness",
        all(
            case["system"] == "mathematics.principal"
            for case in cases
            if case["expected"] == "accepted"
        ),
    )
    checks.equal(
        "plan.fail-closed-case-targets-the-singular-witness",
        by_id["rank-deficient-fail-closed"]["system"],
        "mathematics.rank_deficient",
    )
    checks.require(
        "plan.the-two-guess-cases-differ-only-in-the-guess",
        by_id["early-initial-guess-accepted"]["initial_guess"]
        != by_id["initial-guess-not-accepted-early"]["initial_guess"],
        "a branch is observable only when one guess fires it and another does not",
    )
    return len(cases)


def check_contract(
    checks: Checks, node: dict[str, Any], analysed: dict[str, Any]
) -> dict[str, Any]:
    checks.equal(
        "contract.marked-as-not-proved-here", node["proved_by_this_oracle"], False
    )
    capabilities = node["capabilities"]
    checks.equal("contract.axes", tuple(capabilities["axes"]), AXES)

    positive = capabilities["positive"]
    negative = capabilities["negative"]
    checks.equal("contract.positive-count", len(positive), len(OPERATOR_PROPERTIES))
    checks.equal(
        "contract.positive-properties",
        tuple(sorted(entry["operator_property"] for entry in positive)),
        OPERATOR_PROPERTIES,
    )
    checks.require(
        "contract.positive-tuple-shape",
        all(
            entry["solver"] == "SparseLu"
            and entry["preconditioner"] == "Identity"
            and entry["reduction"] == "Fast"
            and entry["scalar"] == "F64"
            and entry["expected"] == "supported"
            for entry in positive
        ),
        "every positive tuple is SparseLu x property x Identity x Fast x F64",
    )

    checks.equal(
        "contract.negative-covers-jacobi",
        tuple(
            sorted(
                entry["operator_property"]
                for entry in negative
                if entry["preconditioner"] == "Jacobi"
            )
        ),
        OPERATOR_PROPERTIES,
    )
    checks.equal(
        "contract.negative-covers-reproducible",
        tuple(
            sorted(
                entry["operator_property"]
                for entry in negative
                if entry["reduction"] == "Reproducible"
            )
        ),
        OPERATOR_PROPERTIES,
    )
    checks.require(
        "contract.negative-tuple-shape",
        all(
            entry["solver"] == "SparseLu" and entry["expected"] == "rejected"
            for entry in negative
        ),
    )
    positive_keys = {tuple_key(entry) for entry in positive}
    negative_keys = {tuple_key(entry) for entry in negative}
    checks.equal("contract.positive-keys-unique", len(positive_keys), len(positive))
    checks.equal("contract.negative-keys-unique", len(negative_keys), len(negative))
    checks.equal(
        "contract.positive-and-negative-are-disjoint",
        sorted(positive_keys & negative_keys),
        [],
    )

    tags = node["stable_solver_tags"]
    preexisting = tags["preexisting"]
    added = tags["added"]
    checks.equal("contract.preexisting-tags", preexisting, PREEXISTING_TAGS)
    checks.equal("contract.added-tag", added, ADDED_TAG)
    combined = {**preexisting, **added}
    checks.equal("contract.tags-unique", len(set(combined.values())), len(combined))
    if preexisting:
        checks.equal(
            "contract.added-tag-does-not-move-existing-tags",
            added.get("SparseLu"),
            max(preexisting.values()) + 1,
        )
    checks.require(
        "contract.tags-are-non-negative",
        all(isinstance(v, int) and v >= 0 for v in combined.values()),
    )

    artifact = node["artifact"]
    checks.equal(
        "contract.artifact-v1-rejects-sparse-lu", artifact["v1"]["sparse_lu"], "reject"
    )
    encoding = artifact["v2"]["sparse_lu"]
    checks.equal("contract.artifact-v2-encoding", encoding, "sparse-lu")
    checks.equal(
        "contract.artifact-v2-encoding-is-kebab-case-of-variant",
        kebab_case(artifact["v2"]["variant"]),
        encoding,
    )
    checks.require(
        "contract.artifact-v2-encoding-is-lower-kebab",
        isinstance(encoding, str)
        and bool(encoding)
        and all(part.isalnum() and part.islower() for part in encoding.split("-")),
    )

    plan_cases = check_test_plan(checks, node["test_plan"], analysed, positive_keys)
    return {
        "positive": len(positive),
        "negative": len(negative),
        "tags": combined,
        "plan_cases": plan_cases,
    }


# --------------------------------------------------------------------------
# entry point
# --------------------------------------------------------------------------


def sha256_of(path: str) -> str:
    digest = hashlib.sha256()
    with open(path, "rb") as handle:
        for block in iter(lambda: handle.read(65536), b""):
            digest.update(block)
    return digest.hexdigest()


def run(fixture_path: str, expect_digest: str | None) -> tuple[Checks, dict[str, Any]]:
    document = load_fixture(fixture_path)
    checks = Checks()

    checks.equal("fixture.schema", document["schema"], SCHEMA)
    checks.equal("fixture.issue", document["issue"], ISSUE)
    checks.equal("fixture.frozen", document["frozen"], True)

    rationals = walk_rationals(document)
    checks.require(
        "fixture.rationals-are-canonical",
        all(
            node["den"] > 0 and math.gcd(abs(node["num"]), node["den"]) == 1
            for node in rationals
        ),
        "every rational must be in lowest terms with a positive denominator",
    )
    checks.require("fixture.rationals-present", len(rationals) > 0)

    digest = sha256_of(fixture_path)
    if expect_digest is not None:
        checks.equal("fixture.sha256", digest, expect_digest.strip().lower())

    mathematics = document["mathematics"]
    principal_node = mathematics["principal"]
    context = check_principal(checks, principal_node)
    acceptance = check_acceptance(checks, principal_node, context)
    context.update(acceptance)
    check_initial_guesses(checks, principal_node["initial_guesses"], context)
    check_falsifiers(checks, principal_node["falsifiers"], context)
    singular = check_rank_deficient(checks, mathematics["rank_deficient"], acceptance)
    contract = check_contract(checks, document["contract_expectations"], acceptance)

    summary = {
        "fixture": fixture_path,
        "digest": digest,
        "principal": {
            "n": context["n"],
            "nnz": len(context["col_idx"]),
            "determinant": context["determinant"],
            "falsifiers": len(principal_node["falsifiers"]),
        },
        "rank_deficient": singular,
        "acceptance": acceptance,
        "contract": contract,
    }
    return checks, summary


def report_failures(checks: Checks, reason: str | None) -> None:
    for name, _, detail in checks.failures:
        print(f"sparse-lu oracle: FAIL {name}: {detail}", file=sys.stderr)
    if reason is not None:
        print(f"sparse-lu oracle: aborted: {reason}", file=sys.stderr)
    print(
        f"sparse-lu oracle: {len(checks.failures)} of {len(checks.records)} "
        "checks failed",
        file=sys.stderr,
    )


def print_summary(checks: Checks, summary: dict[str, Any]) -> None:
    try:
        shown = os.path.relpath(summary["fixture"], CASE_DIR)
    except ValueError:  # pragma: no cover - separate drives on some platforms
        shown = summary["fixture"]
    principal = summary["principal"]
    singular = summary["rank_deficient"]
    contract = summary["contract"]
    tags = " ".join(
        f"{k}={v}" for k, v in sorted(contract["tags"].items(), key=lambda kv: kv[1])
    )
    acceptance = summary["acceptance"]
    print(
        f"sparse-lu oracle: {len(checks.records)} checks passed, "
        f"{checks.inequalities} of them exact rational inequalities"
    )
    print(f"  fixture         {shown}")
    print(f"  sha256          {summary['digest']}")
    print(
        f"  principal       n={principal['n']} nnz={principal['nnz']} "
        f"det={principal['determinant']} falsifiers={principal['falsifiers']}"
    )
    print(
        f"  rank-deficient  n={singular['n']} nnz={singular['nnz']} "
        f"rank={singular['rank']} min-residual^2={singular['minimum']}"
    )
    print(
        f"  acceptance      rtol={acceptance['relative']} "
        f"atol=2^{dyadic_exponent(acceptance['tolerance'])} "
        f"atol^2={acceptance['squared']} "
        f"ceiling=2^{dyadic_exponent(acceptance['ceiling'])}"
    )
    print(
        f"  contract        {contract['positive']} positive, "
        f"{contract['negative']} negative, {contract['plan_cases']} plan cases, "
        f"tags {tags}"
    )


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(
        description="Exact-rational oracle for the Eqiora SparseLu contract."
    )
    parser.add_argument(
        "--fixture", default=DEFAULT_FIXTURE, help="path to the frozen JSON fixture"
    )
    parser.add_argument(
        "--summary", action="store_true", help="print a concise summary"
    )
    parser.add_argument(
        "--verbose", action="store_true", help="print every check and its result"
    )
    parser.add_argument(
        "--expect-digest", default=None, help="require this sha256 for the fixture"
    )
    args = parser.parse_args(argv)

    try:
        checks, summary = run(args.fixture, args.expect_digest)
    except OracleAbort as abort:
        if args.verbose:
            for name, ok, detail in abort.checks.records:
                print(f"{'pass' if ok else 'FAIL'}  {name}")
        report_failures(abort.checks, abort.reason)
        return 1
    except (FixtureError, OSError, KeyError, TypeError, ValueError) as error:
        print(f"sparse-lu oracle: fixture unusable: {error}", file=sys.stderr)
        return 1

    if args.verbose:
        for name, ok, detail in checks.records:
            suffix = f"  {detail}" if detail and not ok else ""
            print(f"{'pass' if ok else 'FAIL'}  {name}{suffix}")

    if checks.failures:
        report_failures(checks, None)
        return 1

    if args.summary:
        print_summary(checks, summary)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
