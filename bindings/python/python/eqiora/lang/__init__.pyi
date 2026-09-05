"""Bounded Python authoring for deterministic Eqiora Language source.

Authority: ``bindings/python/python/eqiora/lang/__init__.py``.
"""

from collections.abc import Mapping
from fractions import Fraction
from os import PathLike
from typing import Final, final, overload

@final
class SourceError(ValueError):
    """Reject a structurally invalid bounded Source draft.

    Authority: ``bindings/python/python/eqiora/lang/__init__.py::SourceError``.
    """

    ...

@final
class Expression:
    """Compose a closed expression without overloading equality as an equation.

    Authority: ``bindings/python/python/eqiora/lang/__init__.py::Expression``.
    """

    def __add__(self, other: Expression | float | int, /) -> Expression: ...
    def __radd__(self, other: float | int, /) -> Expression: ...
    def __sub__(self, other: Expression | float | int, /) -> Expression: ...
    def __rsub__(self, other: float | int, /) -> Expression: ...
    def __mul__(self, other: Expression | float | int, /) -> Expression: ...
    def __rmul__(self, other: float | int, /) -> Expression: ...
    def __truediv__(self, other: Expression | float | int, /) -> Expression: ...
    def __rtruediv__(self, other: float | int, /) -> Expression: ...
    def __pow__(self, exponent: int, /) -> Expression: ...
    def __neg__(self) -> Expression: ...

@final
class Support:
    """Identify one volume or parent-boundary declaration in its exact Source.

    Authority: ``bindings/python/python/eqiora/lang/__init__.py::Support``.
    """

    ...

@final
class PropertyContract:
    """Identify one scalar property contract in its exact Source.

    Authority: ``bindings/python/python/eqiora/lang/__init__.py::PropertyContract``.
    """

    ...

@final
class PropertyRelease:
    """Identify one exact constant scalar release in its exact Source.

    Authority: ``bindings/python/python/eqiora/lang/__init__.py::PropertyRelease``.
    """

    ...

@final
class MaterialComposition:
    """Identify one immutable typed material composition in its exact Source.

    Authority: ``bindings/python/python/eqiora/lang/__init__.py::MaterialComposition``.
    """

    ...

@final
class Relation:
    """Identify one relation declaration in its exact Source.

    Authority: ``bindings/python/python/eqiora/lang/__init__.py::Relation``.
    """

    ...

@final
class Component:
    """Author one bounded public Component and an admitted exact instance binding.

    Authority: ``bindings/python/python/eqiora/lang/__init__.py::Component``.
    """

    def volume(
        self,
        name: str,
        *,
        dimensions: int,
        doc: str | None = None,
    ) -> Support: ...
    def boundary(
        self,
        name: str,
        *,
        parent: Support,
        doc: str | None = None,
    ) -> Support: ...
    def parameter(
        self,
        name: str,
        *,
        unit: _Unit,
        doc: str | None = None,
    ) -> Expression: ...
    def property(
        self,
        name: str,
        *,
        contract: PropertyContract,
        doc: str | None = None,
    ) -> Expression: ...
    def field(
        self,
        name: str,
        *,
        on: Support,
        unit: _Unit,
        shape: _Shape | None = None,
        initial: int | float | None = None,
        doc: str | None = None,
    ) -> Expression: ...
    @overload
    def relation(
        self,
        name: str,
        *,
        on: Support,
        residual: Expression | int | float,
        left: None = None,
        right: None = None,
        doc: str | None = None,
    ) -> Relation: ...
    @overload
    def relation(
        self,
        name: str,
        *,
        on: Support,
        residual: None = None,
        left: Expression | int | float,
        right: Expression | int | float,
        doc: str | None = None,
    ) -> Relation: ...
    def primal_form(
        self,
        relation: Relation,
        *,
        left: Expression,
        right: Expression,
        doc: str | None = None,
    ) -> None: ...
    def instance(
        self,
        name: str,
        *,
        component: Component,
        supports: Mapping[Support, Support],
        parameters: Mapping[Expression, Expression | int | float],
        properties: Mapping[Expression, PropertyRelease] | None = None,
        material: MaterialComposition | None = None,
        doc: str | None = None,
    ) -> None: ...

@final
class Source:
    """Own one baseline or scalar-property draft and freeze it on emission.

    Authority: ``bindings/python/python/eqiora/lang/__init__.py::Source``.
    """

    def __init__(self) -> None: ...
    def component(
        self,
        name: str,
        *,
        doc: str | None = None,
    ) -> Component: ...
    def scalar_property_contract(
        self,
        name: str,
        *,
        unit: _Unit,
        doc: str | None = None,
    ) -> PropertyContract: ...
    def scalar_property_release(
        self,
        name: str,
        *,
        implements: PropertyContract,
        value: int | float,
        source_unit: _Unit,
        source_scale: int | float,
        citation: str,
        license: str,
        doc: str | None = None,
    ) -> PropertyRelease: ...
    def material_composition(
        self,
        name: str,
        *,
        properties: Mapping[str, PropertyRelease],
        doc: str | None = None,
    ) -> MaterialComposition: ...
    def to_eqi(self) -> str: ...
    def write_eqi(self, path: str | PathLike[str]) -> None: ...

@final
class _Shape: ...

class _Unit:
    def __mul__(self, other: _Unit, /) -> _Unit: ...
    def __truediv__(self, other: _Unit, /) -> _Unit: ...
    def __pow__(self, exponent: int | Fraction, /) -> _Unit: ...
    def prefixed(self, prefix: str) -> _Unit: ...

class _Units:
    kg: _Unit
    m: _Unit
    one: _Unit
    s: _Unit
    A: _Unit
    K: _Unit
    mol: _Unit
    cd: _Unit
    Hz: _Unit
    N: _Unit
    Pa: _Unit
    J: _Unit
    W: _Unit
    C: _Unit
    V: _Unit
    Ohm: _Unit
    S: _Unit
    F: _Unit
    H: _Unit
    Wb: _Unit
    T: _Unit
    g: _Unit

#: Structural SI-unit expressions used by Source declarations.
#:
#: Authority: ``bindings/python/python/eqiora/lang/units.py``.
units: _Units

class _Math:
    pi: Final[Expression]
    @staticmethod
    def sin(value: Expression | float | int) -> Expression: ...
    @staticmethod
    def sqrt(value: Expression | float | int) -> Expression: ...

#: Exact language constants used by Source expressions.
#:
#: Authority: ``bindings/python/python/eqiora/lang/__init__.py::math``.
math: _Math

#: The current ambient-dimension-sized continuum vector shape.
#:
#: Authority: ``bindings/python/python/eqiora/lang/__init__.py::spatial_vector``.
spatial_vector: _Shape

def coordinate(axis: int) -> Expression:
    """Return one indexed spatial-coordinate expression.

    Authority: ``bindings/python/python/eqiora/lang/__init__.py::coordinate``.
    """

    ...

def grad(value: Expression) -> Expression:
    """Return the language gradient of one expression.

    Authority: ``bindings/python/python/eqiora/lang/__init__.py::grad``.
    """

    ...

def test(field: Expression) -> Expression:
    """Return the test function associated with one Source Field.

    Authority: ``bindings/python/python/eqiora/lang/__init__.py::test``.
    """

    ...

def dot(
    left: Expression | float | int,
    right: Expression | float | int,
) -> Expression:
    """Return the inner product of two authored expressions.

    Authority: ``bindings/python/python/eqiora/lang/__init__.py::dot``.
    """

    ...

def integrate(
    domain: Support,
    integrand: Expression | float | int,
) -> Expression:
    """Return one volume integral over an exact Source Support.

    Authority: ``bindings/python/python/eqiora/lang/__init__.py::integrate``.
    """

    ...

def div(value: Expression) -> Expression:
    """Return the language divergence of one expression.

    Authority: ``bindings/python/python/eqiora/lang/__init__.py::div``.
    """

    ...

def trace(value: Expression) -> Expression:
    """Return the language boundary trace of one expression.

    Authority: ``bindings/python/python/eqiora/lang/__init__.py::trace``.
    """

    ...

def normal(value: Expression) -> Expression:
    """Return the language outward-normal contraction of one expression.

    Authority: ``bindings/python/python/eqiora/lang/__init__.py::normal``.
    """

    ...

def symmetric_part(value: Expression) -> Expression:
    """Return the language symmetric part of one expression.

    Authority: ``bindings/python/python/eqiora/lang/__init__.py::symmetric_part``.
    """

    ...

def isotropic_lift(value: Expression) -> Expression:
    """Return the language isotropic tensor lift of one expression.

    Authority: ``bindings/python/python/eqiora/lang/__init__.py::isotropic_lift``.
    """

    ...

def quantity(value: int | float, unit: _Unit) -> Expression:
    """Author an input quantity; the compiler owns conversion to coherent SI.

    Authority: ``bindings/python/python/eqiora/lang/__init__.py::quantity``.
    """
    ...

__all__ = [
    "Component",
    "Expression",
    "MaterialComposition",
    "PropertyContract",
    "PropertyRelease",
    "Relation",
    "Source",
    "SourceError",
    "Support",
    "coordinate",
    "dot",
    "div",
    "grad",
    "integrate",
    "isotropic_lift",
    "math",
    "normal",
    "quantity",
    "spatial_vector",
    "symmetric_part",
    "test",
    "trace",
    "units",
]
