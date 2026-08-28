"""JAX typed-FFI projection of Eqiora differentiable programs.

This optional module owns the exact JAX/JAXLIB registration seam. Compiled
primal and derivative execution crosses directly into native Rust; no Python
host callback participates in a declared path.
"""

from __future__ import annotations

import sys
from types import MappingProxyType
from typing import Any

import numpy as np

from . import DifferentiableProgram
from . import _eqiora

if sys.version_info < (3, 12):  # pragma: no cover - JAX cannot be installed
    raise ImportError("eqiora.jax requires Python 3.12 or newer")

try:
    import jax
    import jaxlib
    from jax.experimental.hijax import VJPHiPrimitive
except ImportError as error:  # pragma: no cover - depends on optional install
    raise ImportError(
        "eqiora.jax requires the optional 'jax' dependency; install eqiora[jax]"
    ) from error


_SUPPORTED_VERSION = "0.11.0"
_INTERFACE_VERSION = 1

if (
    jax.__version__ != _SUPPORTED_VERSION
    or jaxlib.__version__ != _SUPPORTED_VERSION
):
    raise ImportError(
        "eqiora.jax requires the exact JAX/JAXLIB 0.11.0 pair; "
        f"found JAX {jax.__version__} and JAXLIB {jaxlib.__version__}"
    )


_TARGET_CAPSULES = _eqiora._jax_ffi_targets()
for _target_name in sorted(_TARGET_CAPSULES):
    jax.ffi.register_ffi_target(
        _target_name,
        _TARGET_CAPSULES[_target_name],
        platform="cpu",
        api_version=1,
    )

_PRIMAL_TARGET = "eqiora_differentiable_primal_v1"
_JVP_TARGET = "eqiora_differentiable_jvp_v1"
_VJP_TARGET = "eqiora_differentiable_vjp_v1"


def _validate_aval(aval: Any, size: int, *, role: str) -> None:
    if tuple(aval.shape) != (size,):
        raise ValueError(f"{role} must have exact static shape ({size},)")
    if np.dtype(aval.dtype) != np.dtype(np.float64):
        raise TypeError(f"{role} must have dtype float64")
    if getattr(aval, "weak_type", False):
        raise TypeError(f"{role} must not have a weak dtype")
    sharding = getattr(aval, "sharding", None)
    mesh = getattr(sharding, "mesh", None)
    if mesh is not None and not mesh.empty:
        raise NotImplementedError(
            "Eqiora JAX explicit sharding and pmap are not supported"
        )


def _validate_concrete_device(value: Any, *, role: str) -> None:
    if isinstance(value, jax.core.Tracer):
        return
    if not isinstance(value, jax.Array):
        raise TypeError(f"{role} must be an already placed JAX array")
    devices = getattr(value, "devices", None)
    if devices is None:  # pragma: no cover - every admitted JAX Array has this
        raise TypeError(f"{role} does not expose JAX device placement")
    selected = devices()
    if len(selected) != 1:
        raise ValueError(f"{role} must be an unsharded single-device array")
    (device,) = tuple(selected)
    if device.platform != "cpu":
        raise ValueError(f"{role} must reside on one host CPU device")


def _static_params(
    program_key: str,
    input_size: int,
    output_size: int,
) -> dict[str, int | str]:
    return {
        "program_key": program_key,
        "input_size": input_size,
        "output_size": output_size,
        "dtype": "float64",
        "device": "host-cpu",
        "interface_version": _INTERFACE_VERSION,
    }


def _ffi_call(
    target: str,
    outputs: Any,
    *inputs: Any,
    program_key: str,
) -> Any:
    input_layouts = [(0,)] * len(inputs)
    output_layouts = (
        [(0,)] * len(outputs) if isinstance(outputs, tuple) else (0,)
    )
    try:
        return jax.ffi.ffi_call(
            target,
            outputs,
            has_side_effect=False,
            input_layouts=input_layouts,
            output_layouts=output_layouts,
            input_output_aliases={},
            custom_call_api_version=4,
        )(*inputs, program_key=program_key)
    except ValueError as error:
        message = str(error)
        if any(
            message.startswith(f"{status}:")
            for status in (
                "INVALID_ARGUMENT",
                "NOT_FOUND",
                "FAILED_PRECONDITION",
                "INTERNAL",
                "DATA_LOSS",
            )
        ):
            raise jax.errors.JaxRuntimeError(message) from error
        raise


class _NoBatching:
    def batch(self, axis_data, args, dims):
        del axis_data, args, dims
        raise NotImplementedError(
            "Eqiora JAX vmap and batched differentiation are not supported"
        )


class _PrimalCall(_NoBatching, VJPHiPrimitive):
    def __init__(self, input_aval, output_aval, params) -> None:
        _validate_aval(input_aval, params["input_size"], role="parameters")
        _validate_aval(output_aval, params["output_size"], role="output")
        self.in_avals = (input_aval,)
        self.out_aval = output_aval
        self.params = params
        super().__init__()

    def expand(self, parameters):
        return _ffi_call(
            _PRIMAL_TARGET,
            self.out_aval,
            parameters,
            program_key=self.program_key,
        )


class _JvpCall(_NoBatching, VJPHiPrimitive):
    def __init__(self, input_aval, output_aval, params) -> None:
        _validate_aval(input_aval, params["input_size"], role="parameters")
        _validate_aval(output_aval, params["output_size"], role="output")
        self.in_avals = (input_aval, input_aval)
        self.out_aval = (output_aval, output_aval)
        self.params = params
        super().__init__()

    def expand(self, parameters, tangent):
        return _ffi_call(
            _JVP_TARGET,
            self.out_aval,
            parameters,
            tangent,
            program_key=self.program_key,
        )


class _VjpCall(_NoBatching, VJPHiPrimitive):
    def __init__(self, input_aval, output_aval, params) -> None:
        _validate_aval(input_aval, params["input_size"], role="parameters")
        _validate_aval(output_aval, params["output_size"], role="cotangent")
        self.in_avals = (input_aval, output_aval)
        self.out_aval = input_aval
        self.params = params
        super().__init__()

    def expand(self, parameters, cotangent):
        return _ffi_call(
            _VJP_TARGET,
            self.out_aval,
            parameters,
            cotangent,
            program_key=self.program_key,
        )


class _Solve(_NoBatching, VJPHiPrimitive):
    def __init__(self, input_aval, output_aval, params) -> None:
        _validate_aval(input_aval, params["input_size"], role="parameters")
        _validate_aval(output_aval, params["output_size"], role="output")
        self.in_avals = (input_aval,)
        self.out_aval = output_aval
        self.params = params
        super().__init__()

    def expand(self, parameters):
        return _PrimalCall(
            self.in_avals[0],
            self.out_aval,
            self.params,
        )(parameters)

    def vjp_fwd(self, nonzero_inputs, parameters):
        del nonzero_inputs
        output = _PrimalCall(
            self.in_avals[0],
            self.out_aval,
            self.params,
        )(parameters)
        return output, parameters

    def vjp_bwd_retval(self, parameters, cotangent):
        return (
            _VjpCall(
                self.in_avals[0],
                self.out_aval,
                self.params,
            )(parameters, cotangent),
        )

    def jvp(self, primals, tangents):
        (parameters,), (tangent,) = primals, tangents
        return _JvpCall(
            self.in_avals[0],
            self.out_aval,
            self.params,
        )(parameters, tangent)


class JaxProgram:
    """Process-local JAX view of one immutable Eqiora program."""

    __slots__ = (
        "__weakref__",
        "_input_size",
        "_output_size",
        "_params",
        "_program",
        "_program_key",
    )

    def __init__(self, program: DifferentiableProgram) -> None:
        if not isinstance(program, DifferentiableProgram):
            raise TypeError("program must be an eqiora.DifferentiableProgram")
        if program.dtype != "float64" or program.device != "cpu:0":
            raise ValueError("this JAX adapter supports host-CPU float64 programs")
        self._program = program
        self._input_size = program.input_shape[0]
        self._output_size = program.output_shape[0]
        self._program_key = program._jax_ffi_register()
        self._params = MappingProxyType(
            _static_params(
                self._program_key,
                self._input_size,
                self._output_size,
            )
        )

    def __setattr__(self, name: str, value: Any) -> None:
        if hasattr(self, name):
            raise AttributeError("Eqiora JAX program configuration is immutable")
        object.__setattr__(self, name, value)

    def __delattr__(self, name: str) -> None:
        del name
        raise AttributeError("Eqiora JAX program configuration is immutable")

    @property
    def program(self) -> DifferentiableProgram:
        """The exact framework-neutral program retained by this adapter."""
        return self._program

    @property
    def input_shape(self) -> tuple[int]:
        return (self._input_size,)

    @property
    def output_shape(self) -> tuple[int]:
        return (self._output_size,)

    def __call__(self, parameters):
        _validate_concrete_device(parameters, role="parameters")
        input_aval = jax.typeof(parameters)
        _validate_aval(input_aval, self._input_size, role="parameters")
        output_aval = input_aval.update(
            shape=(self._output_size,),
            weak_type=False,
        )
        return _Solve(input_aval, output_aval, self._params)(parameters)


def bind(program: DifferentiableProgram) -> JaxProgram:
    """Bind a framework-neutral program to the typed JAX/XLA FFI."""
    return JaxProgram(program)


__all__ = ["JaxProgram", "bind"]
