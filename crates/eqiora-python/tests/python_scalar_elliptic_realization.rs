use std::num::NonZeroUsize;

use eqiora::api::{
    ModelDocument, ScalarEllipticExecutionEnvironment, ScalarEllipticIntent, ScalarEllipticMethod,
};
use eqiora::realization::RealizationRevision;
use pyo3::ffi::c_str;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyDictMethods};

const POISSON: &str =
    include_str!("../../../verify/interfaces/python-native-modeling/models/poisson.eqi");

#[test]
fn python_scalar_elliptic_realization_is_opaque_exact_and_fail_closed() -> PyResult<()> {
    Python::initialize();
    Python::attach(|py| {
        let native = pyo3::wrap_pymodule!(_eqiora::_eqiora)(py);
        let locals = PyDict::new(py);
        locals.set_item("eqiora", native.bind(py))?;
        locals.set_item("source", POISSON)?;

        py.run(
            c_str!(
                r#"
import gc
import json
import sys
import threading
from itertools import product

import numpy as np

model = eqiora.compile(source=source)
fem_request = eqiora.ScalarElliptic(
    method=eqiora.ScalarEllipticMethod.FiniteElement,
    cells_per_axis=4,
)
fem = eqiora.preview_realization(model, fem_request)
fem_again = eqiora.preview_realization(model, fem_request)
assert fem == fem_again
assert hash(fem) == hash(fem_again)
assert fem.digest == fem_again.digest
assert fem.model_digest == model.digest
assert fem.realization_revision == 1
assert fem.cells_per_axis == 4
assert fem.workers == 1
assert fem.cell_count == 4
assert fem.field_value_count == 5
assert fem.spatial_dimension == 1
assert fem.field_logical_shape == (5,)
assert fem.to_json() == fem_again.to_json()
assert not hasattr(fem, "portable_graph")
assert not hasattr(fem, "solver")
assert not hasattr(fem, "backend")

fvm = eqiora.preview_realization(
    model,
    eqiora.ScalarElliptic(
        method=eqiora.ScalarEllipticMethod.FiniteVolume,
        cells_per_axis=4,
    ),
)
assert fvm.digest != fem.digest
assert fvm.model_digest == fem.model_digest
assert fvm.field_value_count == 4

fem_result = eqiora.submit_realization(model, fem).result()
assert fem_result.realization == fem
assert fem_result.field.location == eqiora.ScalarFieldLocation.Vertex
assert fem_result.field.value_count == 5
assert fem_result.field.spatial_dimension == 1
assert fem_result.field.logical_shape == fem.field_logical_shape
assert abs(fem_result.field.minimum) <= 1.0e-14
assert abs(fem_result.field.maximum - 0.125) <= 1.0e-12
assert fem_result.balance.relative_imbalance <= 1.0e-12
assert fem_result.solve.true_residual_norm <= fem_result.solve.residual_target
assert len(fem_result.output_fingerprint) == 64
assert isinstance(fem_result.values, eqiora.Array)
assert fem_result.values.ownership == "owned"
assert fem_result.values.origin_copy_occurred is False
assert fem_result.values.shape == (fem_result.field.value_count,)
fem_values = fem_result.values.numpy(copy=False)
assert fem_values is fem_result.values.numpy(copy=False)
assert fem_values.shape == (fem_result.field.value_count,)
assert fem_values.dtype == np.float64
assert not fem_values.flags.writeable
np.testing.assert_allclose(
    fem_values,
    np.array([0.0, 0.09375, 0.125, 0.09375, 0.0]),
    rtol=0.0,
    atol=1.0e-14,
)
assert float(fem_values.min()) == fem_result.field.minimum
assert float(fem_values.max()) == fem_result.field.maximum
assert not hasattr(fem_result, "numpy")
assert not hasattr(fem_result, "__await__")

submitted = eqiora.submit_realization(model, fem)
submitted_result = submitted.result()
assert submitted.result() is submitted_result
assert submitted.status == eqiora.RunStatus.Completed
assert submitted.done
assert submitted.history[0] == eqiora.RunStatus.Created
assert submitted.history[-1] == eqiora.RunStatus.Completed
assert submitted.progress == eqiora.ScalarEllipticRunProgress.SolutionAccepted
assert submitted.cancellation is None
assert submitted.plan_key == fem.digest
assert submitted.model_digest == model.digest
assert submitted.adapter == submitted_result.run_manifest.adapter
assert submitted_result.output_fingerprint == fem_result.output_fingerprint
np.testing.assert_array_equal(
    submitted_result.values.numpy(copy=False), fem_values
)

fvm_result = eqiora.submit_realization(model, fvm).result()
assert fvm_result.field.location == eqiora.ScalarFieldLocation.CellCenter
assert fvm_result.field.value_count == 4
assert fvm_result.field.logical_shape == fvm.field_logical_shape == (4,)
assert fvm_result.solve.true_residual_norm <= fvm_result.solve.residual_target
fvm_values = fvm_result.values.numpy(copy=False)
assert fvm_values.shape == (fvm_result.field.value_count,)
assert not fvm_values.flags.writeable
np.testing.assert_allclose(
    fvm_values,
    np.array([0.0625, 0.125, 0.125, 0.0625]),
    rtol=0.0,
    atol=1.0e-14,
)
assert float(fvm_values.min()) == fvm_result.field.minimum
assert float(fvm_values.max()) == fvm_result.field.maximum

def affine_source(dimension):
    axes = ("x", "y", "z")[:dimension]
    bounds = ", ".join(["0, 1"] * dimension)
    boundary_domains = []
    boundary_relations = []
    exact = " + ".join(
        f"{axis + 1} * inverse_length * coordinate({axis})"
        for axis in range(dimension)
    )
    for axis, name in enumerate(axes):
        for side in ("lower", "upper"):
            boundary_domains.append(
                f"  domain {name}_{side} = boundary(region, axis = {axis}, side = {side});"
            )
            boundary_relations.append(
                f"  relation {name}_{side}_value continuous on {name}_{side} "
                f"{{ trace(potential) - ({exact}) = 0; }}"
            )
    return "\n".join([
        f"model affine_{dimension}d {{",
        f"  domain region = box({bounds});",
        *boundary_domains,
        "  representation scalar_space = continuum;",
        "  field potential on region as scalar_space: 1 = 0;",
        "  parameter inverse_length: 1 / m = 1;",
        "  parameter source_scale: 1 / m ^ 2 = 0;",
        "  relation balance continuous on region {",
        "    -div(grad(potential)) - source_scale = 0;",
        "  }",
        *boundary_relations,
        "}",
    ])

for dimension, cells in ((1, 4), (2, 2), (3, 2)):
    affine_model = eqiora.compile(source=affine_source(dimension))
    for method, location_grid in (
        (eqiora.ScalarEllipticMethod.FiniteElement, np.linspace(0.0, 1.0, cells + 1)),
        (
            eqiora.ScalarEllipticMethod.FiniteVolume,
            (np.arange(cells, dtype=np.float64) + 0.5) / cells,
        ),
    ):
        affine_realization = eqiora.preview_realization(
            affine_model,
            eqiora.ScalarElliptic(method=method, cells_per_axis=cells),
        )
        extent = len(location_grid)
        assert affine_realization.spatial_dimension == dimension
        assert affine_realization.field_logical_shape == (extent,) * dimension
        affine_result = eqiora.submit_realization(affine_model, affine_realization).result()
        assert affine_result.field.logical_shape == (extent,) * dimension
        expected = np.array([
            sum((axis + 1) * coordinate for axis, coordinate in enumerate(point))
            for point in product(location_grid, repeat=dimension)
        ])
        np.testing.assert_allclose(
            affine_result.values.numpy(copy=False), expected, rtol=0.0, atol=2.0e-14
        )

manifest = fem_result.run_manifest
assert manifest.model_digest == model.digest
assert manifest.realization_digest == fem.digest
assert manifest.semantic_revision == model.revision.number
assert manifest.workers == 1
assert manifest.reduction == "reproducible"
assert manifest.output_digests == []
replayed = eqiora.RunManifest.from_json(
    manifest.to_json(), realization=fem
)
assert replayed == manifest
assert hash(replayed) == hash(manifest)
assert replayed.to_json() == manifest.to_json()

try:
    eqiora.RunManifest.from_json(manifest.to_json(), realization=fvm)
except eqiora.CompatibilityError as error:
    assert error.diagnostics[0].code == "EQ0901"
else:
    raise AssertionError("a Run must not replay against a foreign Realization")

forged_adapter = json.loads(manifest.to_json())
forged_adapter["execution"]["adapter"] = "example.forged-adapter"
try:
    eqiora.RunManifest.from_json(
        json.dumps(forged_adapter).encode(), realization=fem
    )
except eqiora.CompatibilityError as error:
    assert error.diagnostics[0].code == "EQ0901"
else:
    raise AssertionError("forged execution provenance must be rejected")

forged_output = json.loads(manifest.to_json())
forged_output["output_sha256"] = ["0" * 64]
try:
    eqiora.RunManifest.from_json(
        json.dumps(forged_output).encode(), realization=fem
    )
except eqiora.CompatibilityError as error:
    assert error.diagnostics[0].code == "EQ0901"
else:
    raise AssertionError("unsupported durable output must be rejected")

child = model.commit(model.preview_value_edit("source_scale", 2.0))
try:
    eqiora.submit_realization(child, fem).result()
except eqiora.ValidationError:
    pass
else:
    raise AssertionError("a foreign Model must not silently re-resolve a Realization")

decay = eqiora.compile(source=
    """
model decay {
  field x: 1 = 1;
  relation hold continuous { derivative(x) = 0; }
}
"""
)
try:
    eqiora.preview_realization(decay, fem_request)
except eqiora.ValidationError:
    pass
else:
    raise AssertionError("a non-spatial Model must fail typed admission")

try:
    eqiora.ScalarElliptic(
        method=eqiora.ScalarEllipticMethod.FiniteElement,
        cells_per_axis=0,
    )
except eqiora.ValidationError as error:
    assert error.diagnostics[0].code == "EQ0807"
else:
    raise AssertionError("zero cells must be rejected")

oversized = eqiora.ScalarElliptic(
    method=eqiora.ScalarEllipticMethod.FiniteElement,
    cells_per_axis=250_000,
)
try:
    eqiora.preview_realization(model, oversized)
except eqiora.ValidationError as error:
    assert error.diagnostics[0].code == "EQ0807"
else:
    raise AssertionError("oversized fields must fail before numerical allocation")

cancellable = eqiora.preview_realization(
    model,
    eqiora.ScalarElliptic(
        method=eqiora.ScalarEllipticMethod.FiniteElement,
        cells_per_axis=50_000,
    ),
)
cancelled = eqiora.submit_realization(model, cancellable)
assert cancelled.cancel()
assert not cancelled.cancel()
try:
    cancelled.result()
except eqiora.CancellationError as error:
    assert error.diagnostics[0].code == "EQ0506"
else:
    raise AssertionError("a cancelled spatial Run must not publish a Result")
assert cancelled.status == eqiora.RunStatus.Cancelled
assert cancelled.done
assert isinstance(
    cancelled.cancellation, eqiora.ScalarEllipticRunCancellation
)
assert cancelled.progress == cancelled.cancellation.progress
assert cancelled.cancellation.plan_key == cancellable.digest
assert cancelled.cancellation.progress in (
    eqiora.ScalarEllipticRunProgress.PlanReplayed,
    eqiora.ScalarEllipticRunProgress.SystemFinalized,
    eqiora.ScalarEllipticRunProgress.SolutionAccepted,
)
assert not cancelled.cancel()

for constructor in (eqiora.Realization, eqiora.ScalarEllipticResult, eqiora.RunManifest):
    try:
        constructor()
    except TypeError:
        pass
    else:
        raise AssertionError("accepted values must not have public constructors")

gil_plan = eqiora.preview_realization(
    model,
    eqiora.ScalarElliptic(
        method=eqiora.ScalarEllipticMethod.FiniteElement,
        cells_per_axis=2048,
    ),
)
gil_ready = threading.Event()
gil_start = threading.Event()
gil_observed = threading.Event()

def observe_spatial_run():
    gil_ready.set()
    assert gil_start.wait(timeout=2.0)
    gil_observed.set()

gil_observer = threading.Thread(target=observe_spatial_run)
gil_observer.start()
assert gil_ready.wait(timeout=2.0)
previous_switch_interval = sys.getswitchinterval()
sys.setswitchinterval(1.0)
try:
    gil_start.set()
    eqiora.submit_realization(model, gil_plan).result()
    gil_observed_before_return = gil_observed.is_set()
finally:
    sys.setswitchinterval(previous_switch_interval)
gil_observer.join(timeout=2.0)
assert not gil_observer.is_alive()
assert gil_observed_before_return, "the synchronous spatial run retained the GIL"

python_output_fingerprint = fem_result.output_fingerprint
del fem_result
gc.collect()
np.testing.assert_allclose(
    fem_values,
    np.array([0.0, 0.09375, 0.125, 0.09375, 0.0]),
    rtol=0.0,
    atol=1.0e-14,
)
assert not fem_values.flags.writeable
"#
            ),
            Some(&locals),
            Some(&locals),
        )?;

        let document = ModelDocument::compile("<memory>", POISSON).unwrap();
        let environment = ScalarEllipticExecutionEnvironment::host_serial();
        let result = document
            .run_scalar_elliptic_plan(
                document
                    .preview_scalar_elliptic_run(
                        ScalarEllipticIntent::new(
                            RealizationRevision::new(1),
                            ScalarEllipticMethod::FiniteElement,
                            NonZeroUsize::new(4).unwrap(),
                            NonZeroUsize::MIN,
                        ),
                        environment,
                    )
                    .unwrap(),
                environment,
            )
            .unwrap();
        let expected: String = result
            .receipt()
            .output()
            .as_bytes()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        assert_eq!(
            locals
                .get_item("python_output_fingerprint")?
                .unwrap()
                .extract::<String>()?,
            expected
        );
        Ok(())
    })
}
