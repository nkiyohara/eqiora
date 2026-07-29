#!/usr/bin/env python3
"""Prove the witness-tuple amendment moved nothing but the tuple.

The amendment changes the frozen solve selection's relative tolerance and the
target derived from it. Every physical observation, every tolerance and every
selector must be untouched. This checker is the argument that they are.

It works from a **pre-amendment** record, ``expected/physical-invariance.json``,
generated once with ``--freeze`` while the superseded documents were still in
the tree. That record holds, per frozen document:

- ``complement_sha256`` -- sha256 of the canonical form of the whole document
  with every amendment-allowlisted leaf removed. Equality proves that nothing
  outside the allowlist moved, without needing the superseded file;
- ``physical_sha256`` -- the same digest restricted to the named physical
  subtrees, so the claim can be read without reasoning about the complement;
- ``superseded`` -- the pre-amendment value of each allowlisted leaf.

    python3 amendment/check_physical_invariance.py            # verify
    python3 amendment/check_physical_invariance.py --freeze   # (pre-amendment only)

Exit status 0 is PASS. This checker reads only this directory and runs no
solver. It is not a physical oracle: it decides invariance, not correctness.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import pathlib
import sys

HERE = pathlib.Path(__file__).resolve().parent
CASE = HERE.parent
RECORD = HERE / "expected" / "physical-invariance.json"

# Every leaf the amendment is permitted to move, addressed structurally. A list
# element is addressed by the value of its "name" field rather than by index, so
# reordering a list cannot silently widen the allowlist.
AMENDED: dict[str, list[list[str]]] = {
    "routes/python/result.json": [
        ["solver_selection", "relative_tolerance"],
        ["observations", "residuals", "solver_selected_target"],
        ["checks", "measurements", "=frozen.true_residual", "limit"],
        ["checks", "measurements", "=frozen.weak_continuity_residual", "limit"],
        ["checks", "measurements", "=reindexed.true_residual", "limit"],
        ["checks", "measurements", "=reindexed.weak_continuity_residual", "limit"],
    ],
    "routes/julia/expected/julia-route-frozen.json": [
        ["residuals", "selected_target"],
        # The advisory f64 MINRES analogue tracks the frozen tuple, so amending
        # the tuple re-measures it. It is advisory, never an oracle input, and
        # carries no physical observation.
        ["advisory_f64_solve", "disclaimer"],
        ["advisory_f64_solve", "minres_iterations"],
        ["advisory_f64_solve", "minres_recurred_residual"],
        ["advisory_f64_solve", "minres_true_residual"],
        ["advisory_f64_solve", "minres_max_probe_error_pressure_Pa"],
        ["advisory_f64_solve", "minres_max_probe_error_reaction_N_per_m"],
        ["advisory_f64_solve", "minres_max_probe_error_velocity_m_per_s"],
        ["advisory_f64_solve", "minres_max_probe_error_flux_m2_per_s"],
    ],
}

# The subtrees that carry the physical claim: probes, fluxes, reactions,
# balances, scales, the tolerance-bearing structure and the selectors.
PHYSICAL: dict[str, list[list[str]]] = {
    "routes/python/result.json": [
        ["scales"],
        ["dimensions"],
        ["mesh"],
        ["quad_diagonal"],
        ["observations", "velocity_probes"],
        ["observations", "pressure_probes"],
        ["observations", "signed_flux_m2_s"],
        ["observations", "cylinder_reaction_N_m"],
        ["observations", "global_balance_N_m"],
        ["observations", "pressure_reference"],
        ["congruence"],
        ["patch_test"],
        ["falsifiers"],
        ["reindexing_invariance"],
        ["claim_boundary"],
    ],
    "routes/julia/expected/julia-route-frozen.json": [
        ["scales"],
        ["dofs"],
        ["mesh"],
        ["source"],
        ["pressure_reference"],
        ["velocity_probes"],
        ["pressure_probes"],
        ["fluxes"],
        ["reactions"],
        ["supplementary"],
        ["stability"],
        ["coarse_mesh_facts"],
    ],
}

# Physical tolerance constants, restated here and required to hold. These are
# the frozen contract's table; the amendment does not touch them.
TOLERANCES = {
    "velocity_floor_m_s": 2e-12,
    "velocity_scale_m_s": 0.3,
    "pressure_floor_Pa": 2e-14,
    "pressure_scale_Pa": 0.0007317073170731707,
    "flux_floor_m2_s": 2e-13,
    "flux_scale_m2_s": 0.123,
    "reaction_floor_N_m": 2e-14,
    "reaction_scale_N_m": 0.0003,
    "route_to_route_relative": 2e-10,
    "production_relative": 5e-7,
    "flux_sum_limit_m2_s": 1e-08,
    "momentum_balance_limit_N_m": 1e-10,
}


def resolve(node, step: str):
    """One structural step. ``=name`` selects the list element so named."""
    if step.startswith("="):
        for item in node:
            if isinstance(item, dict) and item.get("name") == step[1:]:
                return item
        raise KeyError(step)
    return node[step]


def prune(doc, paths: list[list[str]]):
    """Return a deep copy of ``doc`` with every listed leaf removed."""
    out = json.loads(json.dumps(doc))
    for path in paths:
        node = out
        for step in path[:-1]:
            node = resolve(node, step)
        last = path[-1]
        if last.startswith("="):
            raise ValueError("an amended leaf must be a named field")
        del node[last]
    return out


def select(doc, paths: list[list[str]]):
    picked = []
    for path in paths:
        node = doc
        for step in path:
            node = resolve(node, step)
        picked.append([path, node])
    return picked


def canonical(value) -> bytes:
    """Order-preserving canonical bytes. Floats keep repr, so no bit is lost."""

    def walk(node):
        if isinstance(node, dict):
            return "{" + ",".join(f"{k!r}:{walk(v)}" for k, v in node.items()) + "}"
        if isinstance(node, list):
            return "[" + ",".join(walk(v) for v in node) + "]"
        if isinstance(node, bool) or node is None:
            return repr(node)
        if isinstance(node, float):
            if node != node or node in (float("inf"), -float("inf")):
                raise ValueError(f"non-finite leaf {node!r}")
            return f"f{node!r}/{1 if str(node)[0] == '-' else 0}"
        if isinstance(node, int):
            return f"i{node!r}"
        return f"s{node!r}"

    return walk(value).encode("utf-8")


def digests(relative: str) -> dict:
    doc = json.loads((CASE / relative).read_text(encoding="utf-8"))
    return {
        "complement_sha256": hashlib.sha256(
            canonical(prune(doc, AMENDED[relative]))
        ).hexdigest(),
        "physical_sha256": hashlib.sha256(
            canonical(select(doc, PHYSICAL[relative]))
        ).hexdigest(),
        "amended": {
            "/".join(p): v for p, v in select(doc, [list(q) for q in AMENDED[relative]])
        },
    }


def freeze() -> int:
    record = {
        "purpose": (
            "Pre-amendment record of the frozen documents, generated once while "
            "the superseded relative tolerance 1e-11 was still in the tree. "
            "complement_sha256 covers the whole document minus the "
            "amendment-allowlisted leaves; physical_sha256 covers the named "
            "physical subtrees. Neither may change."
        ),
        "superseded_relative_tolerance": 1e-11,
        "amended_relative_tolerance": 1e-06,
        "tolerances": TOLERANCES,
        "documents": {rel: digests(rel) for rel in AMENDED},
    }
    RECORD.parent.mkdir(parents=True, exist_ok=True)
    RECORD.write_text(json.dumps(record, indent=2) + "\n", encoding="utf-8")
    print(f"froze {RECORD.relative_to(CASE)}")
    for rel, d in record["documents"].items():
        print(f"  {rel}")
        print(f"    complement {d['complement_sha256']}")
        print(f"    physical   {d['physical_sha256']}")
    return 0


def verify() -> int:
    if not RECORD.exists():
        print(f"FATAL: missing {RECORD}", file=sys.stderr)
        return 2
    record = json.loads(RECORD.read_text(encoding="utf-8"))
    passed = failed = 0

    def check(name: str, ok: bool, detail: str = "") -> None:
        nonlocal passed, failed
        passed, failed = passed + int(ok), failed + int(not ok)
        print(f"  [{'ok' if ok else 'FAIL'}] {name:<58s} {detail}")

    for rel, want in record["documents"].items():
        got = digests(rel)
        check(
            f"{rel}::nothing_outside_the_allowlist_moved",
            got["complement_sha256"] == want["complement_sha256"],
            got["complement_sha256"],
        )
        check(
            f"{rel}::physical_subtrees_unchanged",
            got["physical_sha256"] == want["physical_sha256"],
            got["physical_sha256"],
        )
        for path, before in want["amended"].items():
            after = got["amended"][path]
            check(
                f"{rel}::amended[{path}]",
                True,
                f"{before!r} -> {after!r}"
                + ("  (unchanged)" if before == after else ""),
            )

    doc = json.loads((CASE / "routes/python/result.json").read_text(encoding="utf-8"))
    check(
        "solver_selection.relative_tolerance_is_exactly_1e-6",
        doc["solver_selection"]["relative_tolerance"] == 1e-06,
        repr(doc["solver_selection"]["relative_tolerance"]),
    )
    check(
        "solver_selection.absolute_tolerance_is_unchanged_at_1e-13",
        doc["solver_selection"]["absolute_tolerance"] == 1e-13,
        repr(doc["solver_selection"]["absolute_tolerance"]),
    )
    check(
        "solver_selection.max_iterations_is_unchanged_at_10000",
        doc["solver_selection"]["max_iterations"] == 10000,
        repr(doc["solver_selection"]["max_iterations"]),
    )
    # backend, algorithm, preconditioner, reduction and scalar sit outside the
    # allowlist, so the complement digest above already pins them bit-for-bit.

    rhs = doc["observations"]["residuals"]["reduced_rhs_2norm_dimensionless"]
    target = doc["observations"]["residuals"]["solver_selected_target"]
    check(
        "target_is_derived_mechanically_from_the_frozen_rhs_norm",
        target == max(1e-13, 1e-06 * rhs),
        f"max(1e-13, 1e-06 * {rhs!r}) = {max(1e-13, 1e-06 * rhs)!r}",
    )

    for name, value in record["tolerances"].items():
        check(f"tolerance.{name}_unchanged", TOLERANCES[name] == value, repr(value))

    print(f"\n{passed} passed, {failed} failed")
    return 0 if failed == 0 else 1


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument(
        "--freeze", action="store_true", help="write the pre-amendment record"
    )
    args = parser.parse_args()
    return freeze() if args.freeze else verify()


if __name__ == "__main__":
    raise SystemExit(main())
