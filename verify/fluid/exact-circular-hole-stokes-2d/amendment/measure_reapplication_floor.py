#!/usr/bin/env python3
r"""Route A: measure the reduced-system residual of the f64-rounded oracle
solution, and what a residual target does and does not decide.

Five different things get confused with each other when a residual target is
argued about. This script measures them separately, on route A's own operator,
and writes them to ``expected/route-a-reapplication.json``.

1. **Solution-representation floor** ``||A_hat fl(x*) - b_hat||_2`` with the
   operator applied in elevated precision. This is what storing the *solution*
   in binary64 costs, and nothing else.
2. **Residual-evaluation error** -- the same vector and operator, but the
   residual formed in binary64, under several summation orders. The difference
   between this and (1) is arithmetic in the evaluator, not solution quality.
3. **Ordering-robust evaluation bound** -- the standard ``gamma_m`` bound
   ``m*u/(1-m*u) * sum_j |A_kj x_j|`` per row, which no summation order can
   exceed. This is the scale a target must clear to be decidable at all.
4. **Normwise backward error** of ``fl(x*)`` (Rigal-Gaches), reported so the
   backward-stability reading can be checked rather than assumed.
5. **Physical implication** -- an explicitly constructed vector whose residual
   equals a given target exactly, and the physical error it still carries. This
   is a falsifier: it shows a residual target cannot imply the pointwise
   physical tolerances on this witness at *any* of the values considered.

    python3 amendment/measure_reapplication_floor.py
    python3 amendment/measure_reapplication_floor.py --check   # fail if it would change

No production or candidate implementation is read, executed or consulted. The
only solver used is route A's own elevated-precision LU, on route A's own
assembled operator.
"""

from __future__ import annotations

import argparse
import json
import math
import pathlib
import sys

HERE = pathlib.Path(__file__).resolve().parent
CASE = HERE.parent
sys.path.insert(0, str(CASE / "mesh"))
sys.path.insert(0, str(CASE / "routes" / "python"))

try:
    import mpmath
except ImportError:  # pragma: no cover - dependency declaration
    print("FATAL: requires mpmath (pip install mpmath)", file=sys.stderr)
    raise SystemExit(2) from None

DPS = 60
mpmath.mp.dps = DPS

import oracle  # noqa: E402

mpmath.mp.dps = DPS

OUT = HERE / "expected" / "route-a-reapplication.json"
EPS = sys.float_info.epsilon
U_ROUND = EPS / 2
SUPERSEDED_RTOL = 1e-11
AMENDED_RTOL = 1e-06
ATOL = 1e-13


def f2(value) -> float:
    return float(mpmath.mpf(value))


def build():
    """Route A's own mesh, assembly and elevated-precision solve."""
    document = json.loads((CASE / "mesh" / "mesh.json").read_text(encoding="utf-8"))
    mesh = oracle.Mesh(document)
    case = oracle.solve_case(mesh, oracle.Checks(), "amendment")
    system = case["system"]
    free = case["free"]
    theta = oracle.THETA

    def scale_of(dof: int):
        return (
            oracle.P if dof >= 2 * system.n_vertices + 2 * system.n_cells else oracle.U
        )

    n = len(free)
    scale = [scale_of(d) for d in free]
    b_hat = [scale[k] * case["rhs"][k] / theta for k in range(n)]
    x_hat = [case["x"][free[k]] / scale[k] for k in range(n)]
    a_hat = mpmath.matrix(n, n)
    for k in range(n):
        for j in range(n):
            if case["matrix"][k, j] != 0:
                a_hat[k, j] = scale[k] * case["matrix"][k, j] * scale[j] / theta
    return case, scale, a_hat, b_hat, x_hat


def two_norm(values) -> mpmath.mpf:
    return mpmath.sqrt(sum((v * v for v in values), mpmath.mpf(0)))


def residual(a_hat, vec, b_hat, n):
    out = []
    for k in range(n):
        acc = -b_hat[k]
        for j in range(n):
            if a_hat[k, j] != 0:
                acc += a_hat[k, j] * vec[j]
        out.append(acc)
    return out


def f64_residual_norm(rows, bf, xf, index, order, kahan=False):
    total = 0.0
    for k in range(len(bf)):
        terms = [rows[k][j] * xf[j] for j in index[k]]
        if order == "reversed":
            terms.reverse()
        elif order == "ascending":
            terms.sort(key=abs)
        elif order == "descending":
            terms.sort(key=abs, reverse=True)
        terms = [-bf[k]] + terms
        if kahan:
            acc = 0.0
            comp = 0.0
            for value in terms:
                shifted = value - comp
                grown = acc + shifted
                comp = (grown - acc) - shifted
                acc = grown
        else:
            acc = 0.0
            for value in terms:
                acc += value
        total += acc * acc
    return math.sqrt(total)


def measure() -> dict:  # noqa: C901 - one linear report
    case, scale, a_hat, b_hat, x_hat = build()
    n = len(x_hat)
    b_norm = two_norm(b_hat)
    x_norm = two_norm(x_hat)
    x_inf = max(abs(v) for v in x_hat)
    a_inf = max(sum(abs(a_hat[k, j]) for j in range(n)) for k in range(n))

    superseded_target = max(ATOL, SUPERSEDED_RTOL * f2(b_norm))
    amended_target = max(ATOL, AMENDED_RTOL * f2(b_norm))

    x64 = [mpmath.mpf(float(v)) for v in x_hat]
    # Certify that the rounding is decided: no component sits near a tie, so the
    # elevated solve's residual error cannot change any rounded bit.
    tie = min(
        abs(
            abs(x_hat[k] - x64[k])
            / mpmath.mpf(2) ** (mpmath.floor(mpmath.log(abs(x_hat[k]), 2)) - 52)
            - mpmath.mpf("0.5")
        )
        for k in range(n)
        if x_hat[k] != 0
    )

    r_exact = residual(a_hat, x_hat, b_hat, n)
    r_rounded = residual(a_hat, x64, b_hat, n)
    rho_elevated = two_norm(r_rounded)

    rows = [[float(a_hat[k, j]) for j in range(n)] for k in range(n)]
    index = [[j for j in range(n) if rows[k][j] != 0.0] for k in range(n)]
    bf = [float(v) for v in b_hat]
    xf = [float(v) for v in x_hat]
    orderings = {
        "natural": f64_residual_norm(rows, bf, xf, index, "natural"),
        "reversed": f64_residual_norm(rows, bf, xf, index, "reversed"),
        "ascending_magnitude": f64_residual_norm(rows, bf, xf, index, "ascending"),
        "descending_magnitude": f64_residual_norm(rows, bf, xf, index, "descending"),
        "kahan_compensated": f64_residual_norm(rows, bf, xf, index, "natural", True),
    }

    # gamma_m bound: no summation order can move a row further than this.
    cancellation = []
    nonzeros = []
    bound_rows = []
    for k in range(n):
        total = mpmath.mpf(0)
        count = 0
        for j in range(n):
            if a_hat[k, j] != 0:
                total += abs(a_hat[k, j] * x64[j])
                count += 1
        cancellation.append(total)
        nonzeros.append(count)
        gamma = (count + 1) * U_ROUND / (1 - (count + 1) * U_ROUND)
        bound_rows.append(gamma * (total + abs(b_hat[k])))
    evaluation_bound = two_norm(bound_rows)
    decidable_bound = rho_elevated + evaluation_bound

    # --- physical implication of a residual target -------------------------
    # z solves A_hat z = u for a unit u aligned by inverse iteration with the
    # smallest singular direction, so ||A_hat z||_2 = 1 with ||z||_2 maximal.
    unit = mpmath.matrix([mpmath.mpf(1) / mpmath.sqrt(n)] * n)
    for _ in range(3):
        solved = mpmath.lu_solve(a_hat, unit)
        unit = solved / two_norm([solved[k] for k in range(n)])
    z = mpmath.lu_solve(a_hat, unit)
    z_norm = two_norm([z[k] for k in range(n)])
    frozen_pressures = [
        p["_raw"]
        for p in oracle.observe(case, oracle.Checks(), "amendment", full=False)[
            "pressure_probes"
        ]
    ]

    def worst_pressure_shift(target: float) -> dict:
        # Choose t so that ||r_rounded + t*u||_2 == target exactly.
        dot = sum((r_rounded[k] * unit[k] for k in range(n)), mpmath.mpf(0))
        c = two_norm(r_rounded) ** 2 - mpmath.mpf(target) ** 2
        t = -dot + mpmath.sqrt(dot * dot - c)
        perturbed = list(case["x"])
        for k, dof in enumerate(case["free"]):
            perturbed[dof] = (x64[k] + t * z[k]) * scale[k]
        moved = dict(case)
        moved["x"] = perturbed
        probes = oracle.observe(moved, oracle.Checks(), "perturbed", full=False)
        shift = max(
            abs(p["_raw"] - frozen_pressures[i])
            for i, p in enumerate(probes["pressure_probes"])
        )
        achieved = two_norm(
            residual(
                a_hat,
                [perturbed[d] / scale[k] for k, d in enumerate(case["free"])],
                b_hat,
                n,
            )
        )
        return {
            "target": target,
            "achieved_residual_2norm": f2(achieved),
            "max_pressure_probe_shift_Pa": f2(shift),
            "pressure_production_tolerance_Pa": 2e-14 + 5e-7 * f2(oracle.P),
            "times_over_tolerance": f2(shift / (2e-14 + 5e-7 * oracle.P)),
        }

    return {
        "schema": "eqiora.verify/exact-circular-hole-stokes-2d/amendment/route-a/v1",
        "route": "python",
        "statement": (
            "Route A measurement of the reduced-system residual carried by the "
            "f64-rounded elevated-precision oracle solution, separated into "
            "representation, evaluation and implication. No production or "
            "candidate implementation was read or executed."
        ),
        "environment": {
            "python": sys.version.split()[0],
            "mpmath": mpmath.__version__,
            "working_dps": DPS,
            "binary64_eps": EPS,
            "unit_roundoff": U_ROUND,
        },
        "system": {
            "reduced_dimension": n,
            "b_hat_2norm": f2(b_norm),
            "b_hat_inf_norm": f2(max(abs(v) for v in b_hat)),
            "x_hat_2norm": f2(x_norm),
            "x_hat_inf_norm": f2(x_inf),
            "A_hat_inf_norm": f2(a_inf),
            "nonzeros_per_row_min": min(nonzeros),
            "nonzeros_per_row_max": max(nonzeros),
            "max_row_cancellation_sum_abs_A_x": f2(max(cancellation)),
            "crude_product_A_inf_times_x_inf": f2(a_inf * x_inf),
        },
        "targets": {
            "superseded_relative_tolerance": SUPERSEDED_RTOL,
            "superseded_target": superseded_target,
            "amended_relative_tolerance": AMENDED_RTOL,
            "amended_target": amended_target,
            "absolute_tolerance": ATOL,
        },
        "representation": {
            "elevated_residual_at_exact_solution": f2(two_norm(r_exact)),
            "min_ulp_distance_from_a_rounding_tie": f2(tie),
            "x_minus_rounded_2norm": f2(
                two_norm([x_hat[k] - x64[k] for k in range(n)])
            ),
            "rho_elevated": f2(rho_elevated),
        },
        "evaluation": {
            "f64_by_summation_order": orderings,
            "f64_min": min(orderings.values()),
            "f64_max": max(orderings.values()),
            "gamma_m_bound_2norm": f2(evaluation_bound),
            "decidable_bound_rho_plus_gamma": f2(decidable_bound),
        },
        "backward_error": {
            "denominator_A2_x2_plus_b2": f2(
                mpmath.mpf(2521.007621520478) * x_norm + b_norm
            ),
            "eta_of_rounded_solution": f2(
                rho_elevated / (mpmath.mpf(2521.007621520478) * x_norm + b_norm)
            ),
            "unit_roundoff": U_ROUND,
            "note": (
                "lambda_max 2521.007621520478 is route B's published value for "
                "the same operator. eta far below the unit roundoff means there "
                "is no normwise backward-stability floor near either target."
            ),
        },
        "implication": {
            "note": (
                "An explicitly constructed vector whose reduced residual equals "
                "the target exactly, and the pointwise pressure error it still "
                "carries. Solver-free: the direction comes from inverse "
                "iteration on the oracle's own operator."
            ),
            "z_2norm_for_unit_residual": f2(z_norm),
            "at_superseded_target": worst_pressure_shift(superseded_target),
            "at_amended_target": worst_pressure_shift(amended_target),
        },
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument(
        "--check", action="store_true", help="fail if the output would change"
    )
    args = parser.parse_args()
    payload = json.dumps(measure(), indent=2) + "\n"
    OUT.parent.mkdir(parents=True, exist_ok=True)
    if args.check:
        if not OUT.exists() or OUT.read_text(encoding="utf-8") != payload:
            print(f"FAIL: {OUT.name} would change", file=sys.stderr)
            return 1
        print(f"{OUT.name} reproduced byte for byte")
        return 0
    OUT.write_text(payload, encoding="utf-8")
    print(f"wrote {OUT.relative_to(CASE)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
