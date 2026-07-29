#!/usr/bin/env python3
r"""Adjudicate the witness-tuple amendment from the two routes' measurements.

Reads only the two frozen route documents and the two amendment measurements.
Assembles nothing, solves nothing, and consults no production or candidate
implementation. Writes ``expected/adjudication.json``.

    python3 amendment/adjudicate.py
    python3 amendment/adjudicate.py --check    # fail if the report would change

Exit status 0 is PASS. The rules it applies are stated in ``README.md``; each
one is a falsifier with a named failure mode, not a summary.
"""

from __future__ import annotations

import argparse
import json
import pathlib
import sys

HERE = pathlib.Path(__file__).resolve().parent
CASE = HERE.parent
OUT = HERE / "expected" / "adjudication.json"

EPS = sys.float_info.epsilon
SUPERSEDED_RTOL = 1e-11
AMENDED_RTOL = 1e-06
NEXT_TIGHTER_RTOL = 1e-07
ATOL = 1e-13
PRESSURE_TOLERANCE_PA = 2e-14 + 5e-7 * 0.0007317073170731707


def load(relative: str) -> dict:
    return json.loads((CASE / relative).read_text(encoding="utf-8"))


class Gate:
    def __init__(self) -> None:
        self.records: list[dict] = []

    def rule(self, name: str, ok: bool, detail: str) -> bool:
        self.records.append({"rule": name, "passed": bool(ok), "detail": detail})
        return bool(ok)

    @property
    def failed(self) -> list[dict]:
        return [r for r in self.records if not r["passed"]]


def adjudicate() -> tuple[dict, Gate]:  # noqa: C901 - one linear argument
    gate = Gate()
    a = load("amendment/expected/route-a-reapplication.json")
    b = load("amendment/expected/route-b-reapplication.json")
    py = load("routes/python/result.json")
    jl = load("routes/julia/expected/julia-route-frozen.json")
    routes = {"python": a, "julia": b}

    # --- 0. the two routes measured the same frozen system ------------------
    for label, doc in routes.items():
        gate.rule(
            f"system.{label}.b_hat_2norm_matches_the_frozen_route_document",
            doc["system"]["b_hat_2norm"]
            == (
                py["observations"]["residuals"]["reduced_rhs_2norm_dimensionless"]
                if label == "python"
                else jl["residuals"]["b_hat_reduced_2norm"]["f64"]
            ),
            repr(doc["system"]["b_hat_2norm"]),
        )
    gate.rule(
        "system.both_routes_share_the_rhs_norm_bit_for_bit",
        a["system"]["b_hat_2norm"] == b["system"]["b_hat_2norm"],
        repr(a["system"]["b_hat_2norm"]),
    )

    # --- 1. the superseded tuple is returned --------------------------------
    # Must-accept probe: the elevated reapplication of fl(x*) is ACCEPTED by the
    # superseded target. This is the corrected probe. There is no representation
    # floor above the superseded target and none is claimed.
    for label, doc in routes.items():
        target = doc["targets"]["superseded_target"]
        gate.rule(
            f"superseded.{label}.elevated_reapplication_is_accepted",
            doc["representation"]["rho_elevated"] <= target,
            f"rho_elevated {doc['representation']['rho_elevated']!r} <= "
            f"{target!r} (margin {1 - doc['representation']['rho_elevated'] / target:.1%})",
        )
        gate.rule(
            f"superseded.{label}.every_f64_reapplication_is_rejected",
            doc["evaluation"]["f64_min"] > target,
            f"min over orderings {doc['evaluation']['f64_min']!r} > {target!r}",
        )
    gate.rule(
        "superseded.verdict_depends_on_the_evaluator_not_the_solution",
        all(
            r["representation"]["rho_elevated"]
            <= r["targets"]["superseded_target"]
            < r["evaluation"]["f64_min"]
            for r in routes.values()
        ),
        "both routes: the same vector and operator are accepted in elevated "
        "precision and rejected by every binary64 summation order tested",
    )
    for label, doc in routes.items():
        gate.rule(
            f"superseded.{label}.is_below_the_ordering_robust_decidable_bound",
            doc["targets"]["superseded_target"]
            < doc["evaluation"]["decidable_bound_rho_plus_gamma"],
            f"{doc['targets']['superseded_target']!r} < "
            f"{doc['evaluation']['decidable_bound_rho_plus_gamma']!r}",
        )

    # --- 2. the amended tuple is decidable ----------------------------------
    for label, doc in routes.items():
        target = doc["targets"]["amended_target"]
        bound = doc["evaluation"]["decidable_bound_rho_plus_gamma"]
        gate.rule(
            f"amended.{label}.clears_the_ordering_robust_decidable_bound",
            target > bound,
            f"{target!r} > {bound!r} by {target / bound:.4g}x",
        )
        gate.rule(
            f"amended.{label}.accepts_every_measured_evaluation_of_the_ideal_point",
            target
            > max(doc["evaluation"]["f64_max"], doc["representation"]["rho_elevated"]),
            f"{target!r} > {doc['evaluation']['f64_max']!r} by "
            f"{target / doc['evaluation']['f64_max']:.4g}x",
        )

    # --- 3. the target is derived mechanically, from both routes ------------
    for label, doc in routes.items():
        rhs = doc["system"]["b_hat_2norm"]
        gate.rule(
            f"amended.{label}.target_is_max_atol_rtol_times_rhs_norm",
            doc["targets"]["amended_target"] == max(ATOL, AMENDED_RTOL * rhs),
            f"max({ATOL!r}, {AMENDED_RTOL!r} * {rhs!r}) = "
            f"{max(ATOL, AMENDED_RTOL * rhs)!r}",
        )
    frozen_py = py["observations"]["residuals"]["solver_selected_target"]
    frozen_jl = jl["residuals"]["selected_target"]
    gate.rule(
        "frozen.both_route_documents_record_the_amended_target_bit_identically",
        frozen_py == frozen_jl == a["targets"]["amended_target"],
        f"python {frozen_py!r}, julia {frozen_jl!r}",
    )
    gate.rule(
        "frozen.python_document_records_the_amended_relative_tolerance",
        py["solver_selection"]["relative_tolerance"] == AMENDED_RTOL,
        repr(py["solver_selection"]["relative_tolerance"]),
    )
    for key, want in (("absolute_tolerance", ATOL), ("max_iterations", 10000)):
        gate.rule(
            f"frozen.{key}_is_unchanged",
            py["solver_selection"][key] == want,
            repr(py["solver_selection"][key]),
        )

    # --- 4. why this decade, and not the next tighter one -------------------
    # The frozen contract's own roundoff allowance is
    # 4096 * eps * (1 + ||A||_inf ||x||_inf + ||b||_inf). The contract-shaped
    # one-roundoff scale is that shape with the 4096 safety factor removed. It
    # is a bound, deliberately independent of the sparsity these two routes
    # happen to have, because a production reapplication path is not readable
    # from here.
    scale = EPS * a["system"]["A_hat_inf_norm"] * a["system"]["x_hat_inf_norm"]
    rhs = a["system"]["b_hat_2norm"]
    gate.rule(
        "decade.amended_clears_the_contract_shaped_one_roundoff_scale",
        max(ATOL, AMENDED_RTOL * rhs) > scale,
        f"{max(ATOL, AMENDED_RTOL * rhs)!r} > {scale!r} by "
        f"{max(ATOL, AMENDED_RTOL * rhs) / scale:.4g}x",
    )
    gate.rule(
        "decade.the_next_tighter_decade_does_not_clear_it",
        max(ATOL, NEXT_TIGHTER_RTOL * rhs) < scale,
        f"rtol {NEXT_TIGHTER_RTOL!r} gives {max(ATOL, NEXT_TIGHTER_RTOL * rhs)!r}, "
        f"only {max(ATOL, NEXT_TIGHTER_RTOL * rhs) / scale:.4g}x the scale",
    )

    # --- 5. what the amended gate does NOT decide ---------------------------
    implication = a["implication"]
    gate.rule(
        "nonclaim.amended_gate_does_not_imply_the_pressure_tolerance",
        implication["at_amended_target"]["times_over_tolerance"] > 1,
        f"a constructed vector whose residual equals the amended target exactly "
        f"still moves a frozen pressure probe by "
        f"{implication['at_amended_target']['max_pressure_probe_shift_Pa']:.6e} Pa, "
        f"{implication['at_amended_target']['times_over_tolerance']:.6g}x the "
        f"{PRESSURE_TOLERANCE_PA:.6e} Pa production tolerance",
    )
    # The two roles a residual target could serve are mutually exclusive here.
    ceiling = (
        implication["at_superseded_target"]["target"]
        * PRESSURE_TOLERANCE_PA
        / implication["at_superseded_target"]["max_pressure_probe_shift_Pa"]
    )
    floor = a["evaluation"]["decidable_bound_rho_plus_gamma"]
    gate.rule(
        "nonclaim.no_relative_tolerance_can_be_both_decidable_and_implicative",
        floor > ceiling,
        f"evaluation-decidability needs target > {floor:.6e}; implying the "
        f"pressure tolerance needs target <= {ceiling:.6e}; disjoint by "
        f"{floor / ceiling:.4g}x, so the residual gate can only be a "
        f"solver-health gate on this witness",
    )

    report = {
        "schema": "eqiora.verify/exact-circular-hole-stokes-2d/amendment/adjudication/v1",
        "verdict": "PASS" if not gate.failed else "RETURN",
        "claim": (
            "For this witness the frozen solve selection's relative tolerance is "
            "amended from 1e-11 to 1e-6, giving the mechanically derived target "
            "max(1e-13, 1e-6 * ||b_hat||_2). It is a case-local solver-health "
            "and stopping threshold. It is not a physical accuracy claim, it "
            "does not alter the strict product predicate, and physical "
            "acceptance remains solely the unchanged dual-derived observations "
            "and balances."
        ),
        "why_the_superseded_value_is_returned": (
            "The superseded target 1.3239627651209673e-12 accepts the "
            "f64-rounded elevated-precision solution when the residual is "
            "reapplied in elevated precision and rejects the same vector under "
            "every binary64 summation order measured, in both routes. Its "
            "verdict is therefore decided by the evaluator's arithmetic rather "
            "than by solution quality. There is no representation floor above "
            "it and none is claimed."
        ),
        "derivation_of_the_decade": (
            "The frozen contract bounds residual roundoff by "
            "4096 * eps * (1 + ||A||_inf * ||x||_inf + ||b||_inf). The "
            "contract-shaped one-roundoff scale is that shape with the 4096 "
            "safety factor removed, eps * ||A||_inf * ||x||_inf. rtol 1e-6 is "
            "the tightest decimal decade whose target exceeds it; rtol 1e-7 "
            "does not. The scale is a bound, not a measurement: these two "
            "routes' own operators are far sparser than it assumes, and their "
            "measured ordering-robust evaluation bound is smaller. The bound is "
            "adopted because a production reapplication path is not readable "
            "from this package."
        ),
        "constants": {
            "superseded_relative_tolerance": SUPERSEDED_RTOL,
            "amended_relative_tolerance": AMENDED_RTOL,
            "absolute_tolerance": ATOL,
            "binary64_eps": EPS,
            "contract_shaped_one_roundoff_scale": scale,
            "amended_target": max(ATOL, AMENDED_RTOL * rhs),
            "superseded_target": max(ATOL, SUPERSEDED_RTOL * rhs),
            "evaluation_decidability_floor_on_target": floor,
            "physical_implication_ceiling_on_target": ceiling,
            "roles_are_disjoint_by": floor / ceiling,
            "pressure_production_tolerance_Pa": PRESSURE_TOLERANCE_PA,
        },
        "rules": gate.records,
        "counts": {
            "total": len(gate.records),
            "passed": len(gate.records) - len(gate.failed),
            "failed": len(gate.failed),
        },
    }
    return report, gate


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument(
        "--check", action="store_true", help="fail if the report would change"
    )
    args = parser.parse_args()
    report, gate = adjudicate()
    payload = json.dumps(report, indent=2) + "\n"
    for record in gate.records:
        print(f"  [{'ok' if record['passed'] else 'FAIL'}] {record['rule']}")
        print(f"         {record['detail']}")
    print(
        f"\n{report['counts']['passed']} passed, {report['counts']['failed']} failed "
        f"-- {report['verdict']}"
    )
    if args.check:
        if not OUT.exists() or OUT.read_text(encoding="utf-8") != payload:
            print(f"FAIL: {OUT.name} would change", file=sys.stderr)
            return 1
        print(f"{OUT.name} reproduced byte for byte")
    else:
        OUT.parent.mkdir(parents=True, exist_ok=True)
        OUT.write_text(payload, encoding="utf-8")
        print(f"wrote {OUT.relative_to(CASE)}")
    return 0 if not gate.failed else 1


if __name__ == "__main__":
    raise SystemExit(main())
