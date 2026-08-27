from __future__ import annotations

import gc
import os
import shutil
import subprocess
import sys
from pathlib import Path

import numpy as np
import pytest

jax = pytest.importorskip("jax")
jax.config.update("jax_enable_x64", True)
import jax.numpy as jnp
import jaxlib

import eqiora
import eqiora.jax as eqjax
from eqiora import _eqiora


POISSON = """
model jax_differentiated_poisson {
  domain square = box(0, 1, 0, 1);
  domain x_lower = boundary(square, axis = 0, side = lower);
  domain x_upper = boundary(square, axis = 0, side = upper);
  domain y_lower = boundary(square, axis = 1, side = lower);
  domain y_upper = boundary(square, axis = 1, side = upper);
  representation scalar_space = continuum;
  field potential on square as scalar_space: 1 = 0;
  parameter diffusion: 1 = 1;
  parameter wave_number: 1 / m = 3.141592653589793;
  parameter source_scale: 1 / m ^ 2 = 19.739208802178716;
  parameter boundary_offset: 1 = 0;
  relation balance continuous on square {
    -div(diffusion * grad(potential))
      - source_scale * sin(wave_number * coordinate(0))
        * sin(wave_number * coordinate(1)) = 0;
  }
  relation x_lower_value continuous on x_lower { trace(potential) - boundary_offset = 0; }
  relation x_upper_value continuous on x_upper { trace(potential) - boundary_offset = 0; }
  relation y_lower_value continuous on y_lower { trace(potential) - boundary_offset = 0; }
  relation y_upper_value continuous on y_upper { trace(potential) - boundary_offset = 0; }
}
"""


def test_base_import_remains_jax_free() -> None:
    script = """
import sys
import eqiora

assert "jax" not in sys.modules
assert "jaxlib" not in sys.modules
"""
    subprocess.run(
        [sys.executable, "-I", "-c", script],
        check=True,
        text=True,
        capture_output=True,
    )


def differentiable_program(
    method,
    *,
    include_wave_number: bool = False,
) -> eqiora.DifferentiableProgram:
    model = eqiora.compile(source=POISSON)
    realization = eqiora.preview_realization(
        model,
        eqiora.ScalarElliptic(method=method, cells_per_axis=4),
    )
    inputs = [
        model.parameter("source_scale"),
        model.parameter("diffusion"),
        model.parameter("boundary_offset"),
    ]
    if include_wave_number:
        inputs.append(model.parameter("wave_number"))
    return eqiora.diff.compile(
        model,
        realization,
        inputs=inputs,
        output=model.field("potential"),
    )


@pytest.mark.parametrize(
    "method",
    [
        eqiora.ScalarEllipticMethod.FiniteElement,
        eqiora.ScalarEllipticMethod.FiniteVolume,
    ],
)
def test_eager_primal_jvp_and_vjp_match_framework_neutral_actions(method) -> None:
    program = differentiable_program(method)
    solve = eqjax.bind(program)
    parameters = jnp.array([17.0, 1.2, 0.1], dtype=jnp.float64)
    tangent = jnp.array([0.3, -0.2, 0.4], dtype=jnp.float64)
    cotangent = jnp.linspace(
        0.25,
        1.25,
        solve.output_shape[0],
        dtype=jnp.float64,
    )

    output = solve(parameters)
    output.block_until_ready()
    native = program.evaluate(np.asarray(parameters))
    np.testing.assert_allclose(
        np.asarray(output),
        native.primal().output.numpy(copy=False),
        rtol=0.0,
        atol=0.0,
    )
    assert output.dtype == jnp.float64
    assert output.shape == solve.output_shape
    assert output.devices() == {jax.devices("cpu")[0]}

    jvp_output, output_tangent = jax.jvp(
        solve,
        (parameters,),
        (tangent,),
    )
    native_jvp = native.jvp(np.asarray(tangent))
    np.testing.assert_allclose(
        np.asarray(jvp_output),
        native_jvp.output.numpy(copy=False),
        rtol=0.0,
        atol=0.0,
    )
    np.testing.assert_allclose(
        np.asarray(output_tangent),
        native_jvp.tangent.numpy(copy=False),
        rtol=2.0e-11,
        atol=2.0e-12,
    )

    vjp_output, pullback = jax.vjp(solve, parameters)
    input_cotangent = pullback(cotangent)[0]
    native_vjp = native.vjp(np.asarray(cotangent))
    np.testing.assert_allclose(
        np.asarray(vjp_output),
        native_vjp.output.numpy(copy=False),
        rtol=0.0,
        atol=0.0,
    )
    np.testing.assert_allclose(
        np.asarray(input_cotangent),
        native_vjp.input_cotangent.numpy(copy=False),
        rtol=2.0e-11,
        atol=2.0e-12,
    )


@pytest.mark.parametrize(
    "method",
    [
        eqiora.ScalarEllipticMethod.FiniteElement,
        eqiora.ScalarEllipticMethod.FiniteVolume,
    ],
)
def test_jitted_primal_gradient_and_jvp_use_typed_custom_calls(method) -> None:
    program = differentiable_program(method)
    solve = eqjax.bind(program)
    parameters = jnp.array([17.0, 1.2, 0.1], dtype=jnp.float64)
    tangent = jnp.array([0.3, -0.2, 0.4], dtype=jnp.float64)
    cotangent = jnp.linspace(
        0.25,
        1.25,
        solve.output_shape[0],
        dtype=jnp.float64,
    )

    jitted_solve = jax.jit(solve)
    eager_output = solve(parameters)
    compiled_output = jitted_solve(parameters)
    np.testing.assert_allclose(
        np.asarray(compiled_output),
        np.asarray(eager_output),
        rtol=0.0,
        atol=0.0,
    )

    def objective(values):
        return jnp.vdot(solve(values), cotangent)

    eager_gradient = jax.grad(objective)(parameters)
    compiled_gradient = jax.jit(jax.grad(objective))(parameters)
    native = program.evaluate(np.asarray(parameters))
    expected_gradient = native.vjp(np.asarray(cotangent)).input_cotangent
    np.testing.assert_allclose(
        np.asarray(eager_gradient),
        expected_gradient.numpy(copy=False),
        rtol=2.0e-11,
        atol=2.0e-12,
    )
    np.testing.assert_allclose(
        np.asarray(compiled_gradient),
        np.asarray(eager_gradient),
        rtol=2.0e-11,
        atol=2.0e-12,
    )

    def apply_jvp(values, direction):
        return jax.jvp(solve, (values,), (direction,))

    eager_primal, eager_tangent = apply_jvp(parameters, tangent)
    compiled_primal, compiled_tangent = jax.jit(apply_jvp)(parameters, tangent)
    np.testing.assert_allclose(
        np.asarray(compiled_primal),
        np.asarray(eager_primal),
        rtol=0.0,
        atol=0.0,
    )
    np.testing.assert_allclose(
        np.asarray(compiled_tangent),
        np.asarray(eager_tangent),
        rtol=2.0e-11,
        atol=2.0e-12,
    )

    primal_ir = str(jitted_solve.lower(parameters).compiler_ir())
    gradient_ir = str(jax.jit(jax.grad(objective)).lower(parameters).compiler_ir())
    jvp_ir = str(jax.jit(apply_jvp).lower(parameters, tangent).compiler_ir())
    assert "eqiora_differentiable_primal_v1" in primal_ir
    assert "eqiora_differentiable_vjp_v1" in gradient_ir
    assert "eqiora_differentiable_jvp_v1" in jvp_ir
    for lowered in (primal_ir, gradient_ir, jvp_ir):
        assert "stablehlo.custom_call" in lowered
        assert "xla_python_cpu_callback" not in lowered
        assert "pure_callback" not in lowered


def test_zero_actions_and_compiled_executable_lifetime_are_safe() -> None:
    program = differentiable_program(eqiora.ScalarEllipticMethod.FiniteElement)
    solve = eqjax.bind(program)
    parameters = jnp.array([17.0, 1.2, 0.1], dtype=jnp.float64)
    zero_tangent = jnp.zeros(solve.input_shape, dtype=jnp.float64)
    zero_cotangent = jnp.zeros(solve.output_shape, dtype=jnp.float64)

    _, tangent = jax.jvp(solve, (parameters,), (zero_tangent,))
    _, pullback = jax.vjp(solve, parameters)
    input_cotangent = pullback(zero_cotangent)[0]
    np.testing.assert_allclose(np.asarray(tangent), 0.0, rtol=0.0, atol=0.0)
    np.testing.assert_allclose(
        np.asarray(input_cotangent),
        0.0,
        rtol=0.0,
        atol=0.0,
    )

    compiled = jax.jit(solve).lower(parameters).compile()
    pending = compiled(parameters)
    del solve
    del program
    gc.collect()
    pending.block_until_ready()
    replay = compiled(parameters)
    replay.block_until_ready()
    np.testing.assert_allclose(
        np.asarray(replay),
        np.asarray(pending),
        rtol=0.0,
        atol=0.0,
    )


@pytest.mark.parametrize(
    ("parameters", "error"),
    [
        (jnp.ones(3, dtype=jnp.float32), TypeError),
        (jnp.ones((1, 3), dtype=jnp.float64), ValueError),
        (jnp.ones(2, dtype=jnp.float64), ValueError),
        (np.ones(3, dtype=np.float64), TypeError),
        (1.0, TypeError),
    ],
)
def test_abstract_inputs_fail_before_native_execution(parameters, error) -> None:
    solve = eqjax.bind(
        differentiable_program(eqiora.ScalarEllipticMethod.FiniteElement)
    )
    with pytest.raises(error):
        solve(parameters)


def test_nonfinite_and_unknown_program_identity_fail_closed() -> None:
    solve = eqjax.bind(
        differentiable_program(eqiora.ScalarEllipticMethod.FiniteElement)
    )
    nonfinite = jnp.array([17.0, jnp.nan, 0.1], dtype=jnp.float64)
    with pytest.raises(jax.errors.JaxRuntimeError, match="finite"):
        solve(nonfinite).block_until_ready()

    parameters = jnp.array([17.0, 1.2, 0.1], dtype=jnp.float64)
    nonfinite_tangent = jnp.array([0.3, jnp.inf, 0.4], dtype=jnp.float64)
    with pytest.raises(jax.errors.JaxRuntimeError, match="finite"):
        jax.jvp(solve, (parameters,), (nonfinite_tangent,))[1].block_until_ready()
    _, pullback = jax.vjp(solve, parameters)
    nonfinite_cotangent = jnp.full(
        solve.output_shape,
        jnp.nan,
        dtype=jnp.float64,
    )
    with pytest.raises(jax.errors.JaxRuntimeError, match="finite"):
        pullback(nonfinite_cotangent)[0].block_until_ready()

    input_aval = jax.typeof(parameters)
    output_aval = input_aval.update(shape=solve.output_shape, weak_type=False)
    forged = dict(solve._params)
    forged["program_key"] = "0" * 64
    with pytest.raises(jax.errors.JaxRuntimeError, match="not registered"):
        eqjax._Solve(input_aval, output_aval, forged)(parameters).block_until_ready()


def test_sharded_input_is_rejected_without_implicit_gather() -> None:
    if len(jax.devices("cpu")) < 2:
        pytest.skip("the JAX evidence gate supplies two host devices")
    solve = eqjax.bind(
        differentiable_program(
            eqiora.ScalarEllipticMethod.FiniteElement,
            include_wave_number=True,
        )
    )
    mesh = jax.make_mesh((2,), ("partition",), devices=jax.devices("cpu")[:2])
    sharding = jax.sharding.NamedSharding(
        mesh,
        jax.sharding.PartitionSpec("partition"),
    )
    parameters = jax.device_put(
        jnp.array([17.0, 1.2, 0.1, np.pi], dtype=jnp.float64),
        sharding,
    )
    with pytest.raises(ValueError, match="unsharded"):
        solve(parameters)
    with pytest.raises(NotImplementedError, match="sharding"):
        jax.jit(solve)(parameters).block_until_ready()


def test_one_host_cpu_ordinal_is_preserved_without_transfer() -> None:
    if len(jax.devices("cpu")) < 2:
        pytest.skip("the JAX evidence gate supplies two host devices")
    solve = eqjax.bind(
        differentiable_program(eqiora.ScalarEllipticMethod.FiniteElement)
    )
    device = jax.devices("cpu")[1]
    parameters = jax.device_put(
        jnp.array([17.0, 1.2, 0.1], dtype=jnp.float64),
        device,
    )

    eager = solve(parameters)
    compiled = jax.jit(solve)(parameters)
    eager.block_until_ready()
    compiled.block_until_ready()
    assert eager.devices() == {device}
    assert compiled.devices() == {device}
    np.testing.assert_allclose(
        np.asarray(compiled),
        np.asarray(eager),
        rtol=0.0,
        atol=0.0,
    )


def test_unsupported_transformations_fail_explicitly() -> None:
    solve = eqjax.bind(
        differentiable_program(eqiora.ScalarEllipticMethod.FiniteElement)
    )
    parameters = jnp.array([17.0, 1.2, 0.1], dtype=jnp.float64)
    tangent = jnp.array([0.3, -0.2, 0.4], dtype=jnp.float64)
    batch = jnp.stack((parameters, parameters))

    with pytest.raises(NotImplementedError, match="vmap"):
        jax.vmap(solve)(batch)

    def gradient(values):
        return jax.grad(lambda point: jnp.sum(solve(point)))(values)

    with pytest.raises(NotImplementedError):
        jax.vmap(gradient)(batch)

    def forward(values):
        return jax.jvp(solve, (values,), (tangent,))[1]

    with pytest.raises(NotImplementedError):
        jax.vmap(forward)(batch)
    with pytest.raises(NotImplementedError):
        jax.linearize(solve, parameters)
    with pytest.raises(NotImplementedError, match="pmap"):
        jax.pmap(solve)(batch)


def test_registration_identity_is_deterministic_and_deduplicated() -> None:
    program = differentiable_program(eqiora.ScalarEllipticMethod.FiniteElement)
    first = eqjax.bind(program)
    second = eqjax.bind(program)
    assert first._program_key == second._program_key
    assert len(first._program_key) == 64
    assert set(first._program_key) <= set("0123456789abcdef")


def test_bound_program_configuration_is_immutable() -> None:
    solve = eqjax.bind(
        differentiable_program(eqiora.ScalarEllipticMethod.FiniteElement)
    )
    with pytest.raises(AttributeError, match="immutable"):
        solve._program_key = "0" * 64
    with pytest.raises(TypeError):
        solve._params["program_key"] = "0" * 64
    with pytest.raises(AttributeError, match="immutable"):
        del solve._params


def test_exact_jaxlib_header_matches_allowlisted_native_abi(tmp_path: Path) -> None:
    compiler = shutil.which("cc")
    if compiler is None:
        if os.environ.get("EQIORA_REQUIRE_JAX_ABI_PROBE") == "1":
            pytest.fail("the exact JAX ABI gate requires a C compiler")
        pytest.skip("a C compiler is unavailable")
    include = Path(jaxlib.__file__).resolve().parent / "include"
    source = tmp_path / "xla_ffi_layout.c"
    executable = tmp_path / "xla_ffi_layout"
    source.write_text(
        r"""
#include <stddef.h>
#include <stdio.h>
#include "xla/ffi/api/c_api.h"

int main(void) {
  printf("api_major=%d\n", XLA_FFI_API_MAJOR);
  printf("api_minor=%d\n", XLA_FFI_API_MINOR);
  printf("extension_metadata=%d\n", XLA_FFI_Extension_Metadata);
  printf("execution_stage_execute=%d\n", XLA_FFI_ExecutionStage_EXECUTE);
  printf("arg_type_buffer=%d\n", XLA_FFI_ArgType_BUFFER);
  printf("attr_type_string=%d\n", XLA_FFI_AttrType_STRING);
  printf("data_type_f64=%d\n", XLA_FFI_DataType_F64);
  printf("error_invalid_argument=%d\n", XLA_FFI_Error_Code_INVALID_ARGUMENT);
  printf("error_not_found=%d\n", XLA_FFI_Error_Code_NOT_FOUND);
  printf("error_failed_precondition=%d\n",
         XLA_FFI_Error_Code_FAILED_PRECONDITION);
  printf("error_internal=%d\n", XLA_FFI_Error_Code_INTERNAL);
  printf("error_data_loss=%d\n", XLA_FFI_Error_Code_DATA_LOSS);
  printf("extension_base_size=%zu\n", sizeof(XLA_FFI_Extension_Base));
  printf("api_version_size=%zu\n", sizeof(XLA_FFI_Api_Version));
  printf("error_create_args_size=%zu\n", sizeof(XLA_FFI_Error_Create_Args));
  printf("buffer_size=%zu\n", sizeof(XLA_FFI_Buffer));
  printf("args_size=%zu\n", sizeof(XLA_FFI_Args));
  printf("rets_size=%zu\n", sizeof(XLA_FFI_Rets));
  printf("byte_span_size=%zu\n", sizeof(XLA_FFI_ByteSpan));
  printf("attrs_size=%zu\n", sizeof(XLA_FFI_Attrs));
  printf("call_frame_size=%zu\n", sizeof(XLA_FFI_CallFrame));
  printf("call_frame_attrs_offset=%zu\n", offsetof(XLA_FFI_CallFrame, attrs));
  printf("call_frame_future_offset=%zu\n", offsetof(XLA_FFI_CallFrame, future));
  printf("call_frame_required_size=%zu\n", (size_t)XLA_FFI_CallFrame_STRUCT_SIZE);
  printf("metadata_size=%zu\n", sizeof(XLA_FFI_Metadata));
  printf("metadata_traits_offset=%zu\n", offsetof(XLA_FFI_Metadata, traits));
  printf("metadata_state_type_id_offset=%zu\n",
         offsetof(XLA_FFI_Metadata, state_type_id));
  printf("metadata_required_size=%zu\n", (size_t)XLA_FFI_Metadata_STRUCT_SIZE);
  printf("metadata_extension_size=%zu\n", sizeof(XLA_FFI_Metadata_Extension));
  printf("api_error_create_offset=%zu\n",
         offsetof(XLA_FFI_Api, XLA_FFI_Error_Create));
  return 0;
}
""",
        encoding="utf-8",
    )
    subprocess.run(
        [
            compiler,
            "-std=c11",
            "-I",
            str(include),
            str(source),
            "-o",
            str(executable),
        ],
        check=True,
        text=True,
        capture_output=True,
    )
    output = subprocess.run(
        [str(executable)],
        check=True,
        text=True,
        capture_output=True,
    ).stdout
    observed = {
        key: int(value)
        for line in output.splitlines()
        for key, value in [line.split("=", maxsplit=1)]
    }
    assert observed == dict(_eqiora._jax_ffi_abi_layout())


def test_supported_versions_and_evidence_interpreter_are_exact() -> None:
    assert jax.__version__ == "0.11.0"
    assert jaxlib.__version__ == "0.11.0"
    if expected := os.environ.get("EQIORA_TEST_JAX_VERSION"):
        assert jax.__version__ == expected
        assert jaxlib.__version__ == expected
    if expected_python := os.environ.get("EQIORA_TEST_PYTHON_VERSION"):
        assert f"{sys.version_info.major}.{sys.version_info.minor}" == expected_python
