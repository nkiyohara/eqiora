#!/usr/bin/env python3
"""Check the frozen scientific agreement and Issue #256 state oracle."""

from __future__ import annotations

import argparse
import hashlib
import json
from fractions import Fraction
from pathlib import Path
import subprocess
import sys


CASE_ROOT = Path(__file__).resolve().parent


def require(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)


def load(relative: str) -> dict[str, object]:
    value = json.loads((CASE_ROOT / relative).read_text(encoding="utf-8"))
    require(isinstance(value, dict), f"{relative} must contain one JSON object")
    return value


def rational_text(value: object) -> Fraction:
    require(isinstance(value, (str, int)), f"expected rational text, received {value!r}")
    return Fraction(str(value))


def rational_pair(value: object) -> Fraction:
    require(isinstance(value, dict), f"expected rational pair, received {value!r}")
    require(
        set(value) == {"numerator", "denominator"},
        f"rational pair has unexpected keys: {value!r}",
    )
    numerator = value["numerator"]
    denominator = value["denominator"]
    require(isinstance(numerator, int), "rational numerator must be an integer")
    require(isinstance(denominator, int), "rational denominator must be an integer")
    return Fraction(numerator, denominator)


def check_frozen_hashes(state: dict[str, object]) -> None:
    precommitment = state["precommitment"]
    require(isinstance(precommitment, dict), "state precommitment must be an object")
    expected = precommitment["frozen_scientific_sha256"]
    require(isinstance(expected, dict), "frozen scientific hashes must be an object")
    for relative, digest in expected.items():
        require(isinstance(relative, str), "scientific hash path must be text")
        require(isinstance(digest, str), "scientific hash must be text")
        actual = hashlib.sha256((CASE_ROOT / relative).read_bytes()).hexdigest()
        require(actual == digest, f"frozen scientific file changed: {relative}")


def check_symbolic_derivation() -> None:
    subprocess.run(
        [
            sys.executable,
            str(CASE_ROOT / "references" / "derive_reference.py"),
            "--check",
        ],
        cwd=CASE_ROOT,
        check=True,
    )


def check_scientific_agreement(
    analytic: dict[str, object], symbolic: dict[str, object], state: dict[str, object]
) -> None:
    require(
        analytic["schema"] == "eqiora.faer-sparse-lu-reuse.analytic-oracle.v1",
        "unexpected analytic schema",
    )
    require(
        symbolic["schema"] == "eqiora.faer_sparse_lu_reuse.symbolic_reference.v1",
        "unexpected symbolic schema",
    )
    analytic_points = analytic["points"]
    symbolic_points = symbolic["accepted_points"]
    state_points = state["fixture"]["points"]
    require(isinstance(analytic_points, list), "analytic points must be a list")
    require(isinstance(symbolic_points, list), "symbolic points must be a list")
    require(isinstance(state_points, list), "state points must be a list")
    require(len(analytic_points) == len(symbolic_points) == len(state_points) == 3, "point count")

    for analytic_point, symbolic_point, state_point in zip(
        analytic_points, symbolic_points, state_points, strict=True
    ):
        require(isinstance(analytic_point, dict), "analytic point must be an object")
        require(isinstance(symbolic_point, dict), "symbolic point must be an object")
        require(isinstance(state_point, dict), "state point must be an object")
        point_id = analytic_point["id"]
        require(point_id == symbolic_point["id"] == state_point["id"], "point IDs disagree")

        analytic_system = analytic_point["reduced_system"]
        symbolic_elimination = symbolic_point["strong_dirichlet_elimination"]
        require(isinstance(analytic_system, dict), f"{point_id} analytic reduced system")
        require(isinstance(symbolic_elimination, dict), f"{point_id} symbolic elimination")
        symbolic_csr = symbolic_elimination["reduced_csr"]
        require(isinstance(symbolic_csr, dict), f"{point_id} symbolic reduced CSR")

        require(
            analytic_system["csr_offsets"]
            == symbolic_csr["offsets"]
            == state_point["csr_offsets"],
            f"{point_id} CSR offsets disagree",
        )
        require(
            analytic_system["csr_columns"]
            == symbolic_csr["columns"]
            == state_point["csr_columns"],
            f"{point_id} CSR columns disagree",
        )
        analytic_values = [rational_text(value) for value in analytic_system["csr_values"]]
        symbolic_values = [rational_pair(value) for value in symbolic_csr["values"]]
        state_values = [rational_text(value) for value in state_point["csr_values"]]
        require(
            analytic_values == symbolic_values == state_values,
            f"{point_id} CSR values disagree",
        )
        analytic_rhs = [rational_text(value) for value in analytic_system["rhs"]]
        symbolic_rhs = [
            rational_pair(value) for value in symbolic_elimination["reduced_rhs"]
        ]
        state_rhs = [rational_text(value) for value in state_point["right_hand_side"]]
        require(analytic_rhs == symbolic_rhs == state_rhs, f"{point_id} RHS disagrees")

        analytic_solution = [rational_text(value) for value in analytic_system["solution"]]
        symbolic_solution = [
            rational_pair(value) for value in symbolic_point["exact_solution"]["reduced_free_dofs"]
        ]
        state_solution = [rational_text(value) for value in state_point["solution"]]
        require(
            analytic_solution == symbolic_solution == state_solution,
            f"{point_id} solution disagrees",
        )
        analytic_residual = [
            rational_text(value) for value in analytic_system["exact_residual"]
        ]
        symbolic_residual = [
            rational_pair(value)
            for value in symbolic_point["exact_true_reduced_residual"]["vector"]
        ]
        require(analytic_residual == symbolic_residual, f"{point_id} residual disagrees")
        require(
            rational_text(analytic_system["determinant"])
            == rational_pair(symbolic_point["exact_classification"]["determinant"]),
            f"{point_id} determinant disagrees",
        )
        require(
            rational_text(analytic_system["residual_conditioned_solution_error_bound"])
            == rational_pair(
                symbolic_point["residual_contract_solution_error_bound"][
                    "absolute_bound_per_free_component"
                ]
            ),
            f"{point_id} residual-conditioned error bound disagrees",
        )

    analytic_mutants = analytic["mutants"]
    symbolic_mutants = symbolic["mutants"]
    require(isinstance(analytic_mutants, dict), "analytic mutants must be an object")
    require(isinstance(symbolic_mutants, dict), "symbolic mutants must be an object")
    for analytic_name, symbolic_name in [
        ("structure_mismatch", "required_structure_mismatch"),
        ("same_pattern_singular", "required_same_pattern_singular"),
    ]:
        left = analytic_mutants[analytic_name]
        right = symbolic_mutants[symbolic_name]
        require(isinstance(left, dict) and isinstance(right, dict), f"{analytic_name} mutant")
        right_csr = right["canonical_csr"]
        require(isinstance(right_csr, dict), f"{symbolic_name} CSR")
        require(left["csr_offsets"] == right_csr["offsets"], f"{analytic_name} offsets")
        require(left["csr_columns"] == right_csr["columns"], f"{analytic_name} columns")
        require(
            [rational_text(value) for value in left["csr_values"]]
            == [rational_pair(value) for value in right_csr["values"]],
            f"{analytic_name} values",
        )
        require(
            [rational_text(value) for value in left["rhs"]]
            == [rational_pair(value) for value in right["rhs"]],
            f"{analytic_name} RHS",
        )
        require(
            rational_text(left["determinant"]) == rational_pair(right["exact_determinant"]),
            f"{analytic_name} determinant",
        )


def check_state_contract(state: dict[str, object]) -> None:
    require(
        state["schema"] == "eqiora.faer-sparse-lu-reuse.state-machine-oracle.v1",
        "unexpected state-machine schema",
    )
    public_surface = state["public_surface"]
    require(isinstance(public_surface, dict), "public surface must be an object")
    require(public_surface["minimum_attempts"] == 2, "minimum attempts")
    require(public_surface["maximum_attempts"] == 64, "maximum attempts")
    require(public_surface["sync"] is False, "owner must remain !Sync")
    require(
        public_surface["methods"]
        == [
            "new",
            "execute",
            "plan",
            "maximum_attempts",
            "attempted_solve_count",
            "accepted_solve_count",
            "symbolic_factorization_count",
            "numeric_factorization_count",
            "symbolic_reuse_identity",
            "numeric_reuse_identity",
        ],
        "public method inventory drifted",
    )
    accepted = state["accepted_sequence"]
    require(isinstance(accepted, dict), "accepted sequence must be an object")
    require(
        accepted["final_counters"]
        == {
            "attempted_solve_count": 3,
            "accepted_solve_count": 3,
            "symbolic_factorization_count": 1,
            "numeric_factorization_count": 2,
        },
        "accepted counter inventory drifted",
    )
    operations = accepted["operations"]
    require(isinstance(operations, list), "accepted operations must be a list")
    require([operation["id"] for operation in operations] == ["p0", "p1", "p2"], "order")
    require(
        state["failure_retention"]["after_singular_counters"]
        == {
            "attempted_solve_count": 2,
            "accepted_solve_count": 1,
            "symbolic_factorization_count": 1,
            "numeric_factorization_count": 1,
        },
        "singular retention counters drifted",
    )
    require(len(state["targeted_mutants"]) == 6, "all six targeted mutants are mandatory")
    encoding = state["identity_encoding"]
    require(encoding["digest_bytes"] == 32, "reuse digests must remain 32 bytes")
    require(
        encoding["counts_and_indices"] == "unsigned-u64-big-endian",
        "count/index encoding drifted",
    )
    require(
        encoding["floating_values"]
        == "ieee-754-binary64-bits-big-endian-with-both-signed-zero-encodings-normalized-to-positive-zero",
        "binary64 encoding drifted",
    )
    require(
        [
            encoding["structure_domain"],
            encoding["coefficient_domain"],
            encoding["policy_domain"],
            encoding["symbolic_domain"],
            encoding["numeric_domain"],
        ]
        == [
            "eqiora.faer-sparse-lu-reuse.structure/v1\\0",
            "eqiora.faer-sparse-lu-reuse.coefficients/v1\\0",
            "eqiora.faer-sparse-lu-reuse.policy/v1\\0",
            "eqiora.faer-sparse-lu-reuse.symbolic/v1\\0",
            "eqiora.faer-sparse-lu-reuse.numeric/v1\\0",
        ],
        "identity domain separation drifted",
    )
    require(
        state["ordering"]["phase_counts_are_order_independent"] == "not-claimed",
        "ordering must not acquire a phase-count claim",
    )
    storage = state["concurrency_and_storage"]
    require(storage["parallelism"] == "Par::Seq", "parallelism drifted")
    require(storage["process_global_state"] is False, "global state is forbidden")
    require(storage["directory_state"] is False, "directory state is forbidden")
    require(storage["persistent_state"] is False, "persistence is forbidden")


def check() -> None:
    analytic = load("expected/analytic.json")
    symbolic = load("expected/symbolic.json")
    state = load("expected/state-machine.json")
    check_frozen_hashes(state)
    check_symbolic_derivation()
    check_scientific_agreement(analytic, symbolic, state)
    check_state_contract(state)
    print("faer sparse-LU reuse scientific agreement and state oracle: checked")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", required=True)
    parser.parse_args()
    check()


if __name__ == "__main__":
    main()
