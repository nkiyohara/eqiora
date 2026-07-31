from typing import assert_type

import numpy as np
import numpy.typing as npt

import eqiora
from eqiora.solid import MixedBoundaryElasticityResult


def check_native_modeling() -> None:
    length = eqiora.Dimension(length=1)
    domain = eqiora.Domain.box("rod", (0.0, 1.0))
    continuum = eqiora.Representation.continuum("temperature")
    temperature = eqiora.Field(
        "temperature",
        domain=domain,
        representation=continuum,
        dimension=length,
    )
    conductivity = eqiora.Parameter("conductivity", value=1.0)
    balance = eqiora.Relation(
        "balance",
        domain=domain,
        residual=eqiora.div(conductivity * eqiora.grad(temperature)),
    )

    assert_type(
        eqiora.Model.define(
            "thermal",
            domain,
            continuum,
            temperature,
            conductivity,
            balance,
        ),
        eqiora.Model,
    )


def check_execution(
    model: eqiora.Model,
    realization: eqiora.Realization,
    array: eqiora.Array,
) -> None:
    assert_type(
        eqiora.run(model, end_time=1.0, max_step=0.1),
        eqiora.Result,
    )
    assert_type(
        eqiora.run(model, end_time=1.0, max_step=0.1, realization=None),
        eqiora.Result,
    )
    assert_type(
        eqiora.run(model, realization=realization),
        eqiora.ScalarEllipticResult,
    )

    temporal = eqiora.submit(model, end_time=1.0, max_step=0.1)
    spatial = eqiora.submit(model, realization=realization)
    assert_type(temporal, eqiora.Run[eqiora.Result])
    assert_type(spatial, eqiora.Run[eqiora.ScalarEllipticResult])
    assert_type(temporal.result(), eqiora.Result)
    assert_type(spatial.result(), eqiora.ScalarEllipticResult)
    assert_type(array.numpy(copy=False), npt.NDArray[np.float64])

    eqiora.run(model)  # type: ignore[call-overload]
    eqiora.run(  # type: ignore[call-overload]
        model,
        end_time=1.0,
        max_step=0.1,
        realization=realization,
    )
    eqiora.Run[eqiora.Result](object())  # type: ignore[arg-type]


class _InvalidModelSubclass(eqiora.Model):  # type: ignore[misc]
    pass


def check_structural_result(model: eqiora.Model) -> None:
    result = eqiora.solid.solve_mixed_boundary_elasticity(model)
    assert_type(result, MixedBoundaryElasticityResult)
    assert_type(result.coordinates, npt.NDArray[np.float64])
    assert_type(result.cells, npt.NDArray[np.uint32])
    assert_type(result.displacement, npt.NDArray[np.float64])
