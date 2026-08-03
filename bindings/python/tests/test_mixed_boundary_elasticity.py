from __future__ import annotations

import gc
import hashlib
import json
import subprocess
import sys
from importlib.resources import files
from pathlib import Path

import numpy as np
import pytest

import eqiora


REPOSITORY_ROOT = Path(__file__).resolve().parents[3]
PYTHON_DEMO = REPOSITORY_ROOT / "examples" / "python" / "mixed_boundary_elasticity.py"
MODEL_RESOURCE = files(eqiora).joinpath(
    "examples",
    "mixed-boundary-elasticity.eqi",
)
MODEL_SHA256 = "dd3497c4b412a4171a7bfd18be5963074a093823c11ef2032907335f4779acb5"


def accepted_model() -> eqiora.Model:
    source = MODEL_RESOURCE.read_text(encoding="utf-8")
    assert hashlib.sha256(source.encode()).hexdigest() == MODEL_SHA256
    return eqiora.compile(
        source,
        filename="mixed-boundary-elasticity.eqi",
    )


@pytest.fixture(scope="module")
def accepted() -> tuple[
    eqiora.Model,
    eqiora.solid.MixedBoundaryElasticityResult,
]:
    model = accepted_model()
    return model, eqiora.solid.solve_mixed_boundary_elasticity(model)


def test_result_retains_complete_relational_lineage_and_execution(
    accepted: tuple[
        eqiora.Model,
        eqiora.solid.MixedBoundaryElasticityResult,
    ],
) -> None:
    model, result = accepted
    assert isinstance(result, eqiora.solid.MixedBoundaryElasticityResult)
    assert result.model_digest == model.revision.digest
    assert result.semantic_revision == model.revision.number == 1
    assert result.realization_revision == 1
    assert result.case_id == "solid.mixed-boundary-elasticity-2d"

    run = json.loads(result.run_manifest_json)
    assert run["model_sha256"] == result.model_digest
    assert run["semantic_revision"] == result.semantic_revision
    assert run["realization_sha256"] == result.realization_digest
    assert len(run["output_sha256"]) == 1
    assert len(result.run_digest) == 64
    assert len(result.realization_digest) == 64

    solve = result.solve
    assert solve.algorithm == "conjugate-gradient"
    assert solve.preconditioner == "identity"
    assert solve.reduction == "reproducible"
    assert solve.relative_tolerance == 1.0e-12
    assert solve.absolute_tolerance == 1.0e-14
    assert solve.maximum_iterations == 10_000
    assert solve.true_residual_norm <= solve.residual_target
    assert result.assembly_packets > 0
    assert result.assembly_targets > 0
    assert np.isfinite(result.constrained_reaction).all()
    assert np.isfinite(result.integrated_body_force).all()


def test_q1_arrays_are_complete_coindexed_and_immutable(
    accepted: tuple[
        eqiora.Model,
        eqiora.solid.MixedBoundaryElasticityResult,
    ],
) -> None:
    _, result = accepted
    coordinates = result.coordinates
    cells = result.cells
    displacement = result.displacement

    assert coordinates is result.coordinates
    assert cells is result.cells
    assert displacement is result.displacement
    assert coordinates.shape == (289, 2)
    assert cells.shape == (256, 4)
    assert displacement.shape == (289, 2)
    assert coordinates.dtype == np.float64
    assert cells.dtype == np.uint32
    assert displacement.dtype == np.float64
    assert coordinates.flags.c_contiguous
    assert cells.flags.c_contiguous
    assert displacement.flags.c_contiguous
    assert not coordinates.flags.writeable
    assert not cells.flags.writeable
    assert not displacement.flags.writeable
    assert np.isfinite(coordinates).all()
    assert np.isfinite(displacement).all()
    assert cells.max() < coordinates.shape[0]
    assert all(len(set(cell)) == 4 for cell in cells.tolist())
    assert result.displacement_dimension == (0, 1, 0, 0, 0, 0, 0)
    assert result.bounds == ((0.0, 1.0), (0.0, 1.0))
    assert result.cells_per_axis == 16


def test_array_owners_survive_result_deletion_and_solves_do_not_share_storage() -> None:
    model = accepted_model()
    first = eqiora.solid.solve_mixed_boundary_elasticity(model)
    second = eqiora.solid.solve_mixed_boundary_elasticity(model)
    first_arrays = (first.coordinates, first.cells, first.displacement)
    second_arrays = (second.coordinates, second.cells, second.displacement)

    for left, right in zip(first_arrays, second_arrays, strict=True):
        np.testing.assert_array_equal(left, right)
        assert not np.shares_memory(left, right)

    del first
    gc.collect()
    assert all(array.size > 0 and not array.flags.writeable for array in first_arrays)


def test_foreign_current_model_is_rejected_before_execution() -> None:
    source = MODEL_RESOURCE.read_text(encoding="utf-8").replace(
        "parameter mu: kg / (m * s ^ 2) = 3;",
        "parameter mu: kg / (m * s ^ 2) = 4;",
    )
    foreign = eqiora.compile(
        source,
        filename="foreign-elasticity.eqi",
    )
    with pytest.raises(eqiora.ValidationError) as caught:
        eqiora.solid.solve_mixed_boundary_elasticity(foreign)
    assert any(diagnostic.code == "EQ0807" for diagnostic in caught.value.diagnostics)


def test_surface_does_not_claim_uncomputed_structural_quantities(
    accepted: tuple[
        eqiora.Model,
        eqiora.solid.MixedBoundaryElasticityResult,
    ],
) -> None:
    _, result = accepted
    public = set(dir(result))
    assert public.isdisjoint(
        {
            "stress",
            "strain",
            "traction",
            "exact_solution",
            "error_norm",
            "convergence_order",
        }
    )


def test_numpy_import_is_lazy_until_matrix_projection(tmp_path: Path) -> None:
    script = tmp_path / "lazy_projection.py"
    script.write_text(
        """
import sys
from importlib.resources import files
import eqiora

assert "numpy" not in sys.modules
source = files(eqiora).joinpath(
    "examples", "mixed-boundary-elasticity.eqi"
).read_text()
model = eqiora.compile(
    source,
    filename="mixed-boundary-elasticity.eqi",
)
result = eqiora.solid.solve_mixed_boundary_elasticity(model)
assert "numpy" not in sys.modules
_ = result.coordinates
assert "numpy" in sys.modules
""",
        encoding="utf-8",
    )
    completed = subprocess.run(
        [sys.executable, "-I", str(script)],
        cwd=tmp_path,
        check=False,
        capture_output=True,
        text=True,
    )
    assert completed.returncode == 0, completed.stdout + completed.stderr


def test_checked_in_python_demo_runs_with_packaged_model_resource() -> None:
    if not PYTHON_DEMO.is_file():
        pytest.skip("consumer tree does not carry the checked-in Python example")

    completed = subprocess.run(
        [sys.executable, "-I", str(PYTHON_DEMO)],
        cwd=REPOSITORY_ROOT,
        check=True,
        capture_output=True,
        text=True,
        timeout=120,
    )

    assert completed.stderr == ""
    lines = completed.stdout.splitlines()
    assert len(lines) == 4
    assert len(lines[0]) == 64
    assert all(character in "0123456789abcdef" for character in lines[0])
    assert lines[1].startswith("LinearSolveSummary(")
    assert lines[2].startswith("constrained reaction ") and lines[2].endswith(" N")
    assert lines[3].startswith("integrated body force ") and lines[3].endswith(" N")
