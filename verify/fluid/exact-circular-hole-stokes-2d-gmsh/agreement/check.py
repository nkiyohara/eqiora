#!/usr/bin/env python3
"""Check the two frozen Gmsh Stokes routes without rerunning either solver."""

from __future__ import annotations

import json
import math
from pathlib import Path


CASE = Path(__file__).resolve().parents[1]
PYTHON = json.loads((CASE / "routes/python/result.json").read_text())
JULIA = json.loads((CASE / "routes/julia/expected/julia-route-frozen.json").read_text())

SOURCE_DIGEST = "b00123472a596e8289820cabaee20d52cdf81b5572fa9ce58ff17cdaa00046d9"
MESH_DIGEST = "5962836788fa785fd0761813c542e9078523796409787d86ad8a006dfef5b62b"
GEO_SHA256 = "81c96068891d6b506827339cd6fecf07eafcb867c76f01747c35d134167d367e"
MSH_SHA256 = "ab7340cec1976f713b5c5deab76fc7d554593126f1c1cd68cc021749911a206a"
TOLERANCES = {
    "velocity": 6.2e-11,
    "pressure": 1.6634146341463415e-13,
    "flux": 2.48e-11,
    "reaction": 8.0e-14,
}


def maximum_difference(left: object, right: object) -> float:
    if isinstance(left, list) and isinstance(right, list):
        assert len(left) == len(right)
        return max((maximum_difference(a, b) for a, b in zip(left, right)), default=0.0)
    return abs(float(left) - float(right))


def within(name: str, left: object, right: object, tolerance: float) -> float:
    difference = maximum_difference(left, right)
    assert difference <= tolerance, (
        f"{name}: route difference {difference:.17g} exceeds {tolerance:.17g}"
    )
    return difference


def main() -> None:
    assert PYTHON["status"] == "frozen-independent-route-a"
    assert PYTHON["ordinary_positive_path_completed_before_falsifiers"] is True
    assert PYTHON["checks"]["failed"] == 0
    assert JULIA["route"] == "julia"
    assert JULIA["checks"]["failed"] == 0

    python_mesh = PYTHON["mesh"]
    julia_mesh = JULIA["mesh"]
    assert JULIA["inputs"]["exact_source_sha256"] == SOURCE_DIGEST
    assert python_mesh["accepted_eqiora_mesh_digest"] == MESH_DIGEST
    assert JULIA["inputs"]["eqiora_mesh_digest_cited_not_recomputed"] == MESH_DIGEST
    assert PYTHON["frozen_inputs"]["geometry_geo"]["sha256"] == GEO_SHA256
    assert JULIA["inputs"]["geo_sha256"] == GEO_SHA256
    assert (
        PYTHON["frozen_inputs"]["gmsh"]["version"]
        == JULIA["gmsh"]["version"]
        == "4.15.2"
    )
    assert (
        PYTHON["frozen_inputs"]["gmsh"]["archive_sha256"]
        == JULIA["inputs"]["official_linux64_archive_sha256"]
    )
    assert (
        PYTHON["frozen_inputs"]["gmsh"]["executable_sha256"]
        == JULIA["inputs"]["official_linux64_executable_sha256"]
    )
    assert python_mesh["sha256"] == MSH_SHA256
    assert JULIA["inputs"]["msh_sha256"] == MSH_SHA256
    assert (
        python_mesh["coordinate_buffer_sha256"]
        == JULIA["inputs"]["coordinate_buffer_sha256_cited_not_recomputed"]
    )
    assert (
        python_mesh["triangle_u32_buffer_sha256"]
        == JULIA["inputs"]["triangle_buffer_sha256_cited_not_recomputed"]
    )
    assert (python_mesh["nodes"], python_mesh["triangles"]) == (662, 1210)
    assert (julia_mesh["vertices"], julia_mesh["triangles"]) == (662, 1210)
    assert python_mesh["boundary_edges"] == julia_mesh["boundary_facets"] == 114
    assert (
        python_mesh["boundary_partition"]
        == {
            "cylinder": JULIA["mapping"]["cylinder_facets"],
            "inlet": JULIA["mapping"]["inlet_facets"],
            "outlet": JULIA["mapping"]["outlet_facets"],
            "walls": JULIA["mapping"]["wall_facets"],
        }
        == {"cylinder": 50, "inlet": 14, "outlet": 2, "walls": 48}
    )
    assert (
        python_mesh["euler_characteristic"] == julia_mesh["euler_characteristic"] == 0
    )
    assert python_mesh["minimum_mean_ratio"] == julia_mesh["minimum_mean_ratio"]
    assert (
        2.0 * python_mesh["minimum_area_m2"]
        == julia_mesh["minimum_signed_measure_scale"]
    )

    for family, expected in TOLERANCES.items():
        python_tolerance = PYTHON["tolerances"]["families"][family]["route_agreement"]
        julia_tolerance = JULIA["tolerances"][
            f"{family}_m_per_s"
            if family == "velocity"
            else {
                "pressure": "pressure_pa",
                "flux": "flux_m2_per_s",
                "reaction": "reaction_n_per_m",
            }[family]
        ]
        assert abs(python_tolerance - expected) <= math.ulp(expected)
        assert abs(julia_tolerance - expected) <= math.ulp(expected)
        TOLERANCES[family] = min(python_tolerance, julia_tolerance)

    python_observations = PYTHON["observations"]
    python_velocity = python_observations["velocity_barycentre_probes"]
    julia_velocity = JULIA["velocity_probes"]
    assert [probe["target_m"] for probe in python_velocity] == [
        probe["target_m"] for probe in julia_velocity
    ]
    assert (
        [probe["tied_cells"] for probe in python_velocity]
        == [probe["exact_tie_count"] for probe in julia_velocity]
        == [1, 1, 1, 1, 1]
    )
    differences = {
        "velocity": within(
            "velocity probes",
            [probe["velocity_m_s"] for probe in python_velocity],
            [probe["velocity_m_per_s"] for probe in julia_velocity],
            TOLERANCES["velocity"],
        )
    }

    python_pressure = python_observations["pressure_geometric_probes"]
    julia_pressure = JULIA["pressure_probes"]
    python_names = [
        name.replace("outer_nearest_x_low_mid", "outer_near_inlet_mid").replace(
            "outer_nearest_x_high_mid", "outer_near_outlet_mid"
        )
        for name in (probe["name"] for probe in python_pressure)
    ]
    assert python_names == [probe["name"] for probe in julia_pressure]
    assert [probe["position_m"] for probe in python_pressure] == [
        probe["vertex_m"] for probe in julia_pressure
    ]
    assert (
        [len(probe["tied_node_tags"]) for probe in python_pressure]
        == [probe.get("exact_tie_count", 1) for probe in julia_pressure]
        == [1, 1, 2, 2, 1, 1]
    )
    differences["pressure"] = within(
        "pressure probes",
        [probe["pressure_Pa"] for probe in python_pressure],
        [probe["pressure_pa"] for probe in julia_pressure],
        TOLERANCES["pressure"],
    )
    for extremum in ("minimum", "maximum"):
        left = python_observations["pressure_global_extrema"][extremum]
        right = JULIA["pressure_extrema"][extremum]
        assert left["position_m"] == right["vertex_m"]
        differences["pressure"] = max(
            differences["pressure"],
            within(
                f"pressure {extremum}",
                left["pressure_Pa"],
                right["pressure_pa"],
                TOLERANCES["pressure"],
            ),
        )

    python_flux = python_observations["signed_flux_m2_s"]
    julia_flux = JULIA["fluxes_m2_per_s"]
    differences["flux"] = within(
        "signed fluxes",
        [python_flux[name] for name in ("inlet", "outlet", "net")],
        [julia_flux[name] for name in ("inlet", "outlet", "net")],
        TOLERANCES["flux"],
    )
    assert (
        abs(python_flux["net"]) <= JULIA["tolerances"]["signed_flux_balance_m2_per_s"]
    )
    assert abs(julia_flux["net"]) <= JULIA["tolerances"]["signed_flux_balance_m2_per_s"]

    python_reaction = python_observations["cylinder_reaction_N_m"]
    julia_reaction = JULIA["forces_n_per_m"]
    differences["reaction"] = within(
        "cylinder reaction",
        python_reaction["constraint_force_on_fluid"],
        julia_reaction["cylinder_constraint_force_on_fluid"],
        TOLERANCES["reaction"],
    )
    differences["reaction"] = max(
        differences["reaction"],
        within(
            "global momentum closure",
            python_observations["momentum_closure_N_m"]["sum"],
            julia_reaction["momentum_closure"],
            TOLERANCES["reaction"],
        ),
    )
    for value in python_observations["momentum_closure_N_m"]["sum"]:
        assert abs(value) <= JULIA["tolerances"]["momentum_closure_n_per_m"]
    for value in julia_reaction["momentum_closure"]:
        assert abs(value) <= JULIA["tolerances"]["momentum_closure_n_per_m"]

    assert (
        PYTHON["observations"]["residual"]["true_reduced_2norm_dimensionless"]
        <= PYTHON["observations"]["residual"]["acceptance_limit"]
    )
    assert (
        JULIA["residuals"]["true_reduced_2norm"]
        <= JULIA["residuals"]["selected_target"]
        + JULIA["residuals"]["roundoff_allowance"]
    )

    assert PYTHON["falsifiers"]
    assert all(falsifier["detected"] is True for falsifier in PYTHON["falsifiers"])
    assert {
        name for name in JULIA["checks"]["names"] if name.startswith("falsifier.")
    } == {
        "falsifier.vector_laplacian_detected",
        "falsifier.pressure_coupling_sign_detected",
        "falsifier.swapped_inlet_outlet_detected",
        "falsifier.reversed_flux_normal_detected",
        "falsifier.suffixed_gmsh_version_rejected",
        "falsifier.algorithm5_changes_exact_mesh",
    }

    for family, difference in differences.items():
        print(
            f"{family}: max route difference={difference:.17g}; "
            f"tolerance={TOLERANCES[family]:.17g}; "
            f"margin={TOLERANCES[family] / max(difference, 5e-324):.6g}x"
        )


if __name__ == "__main__":
    main()
