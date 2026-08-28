"""Bounded Python authoring for deterministic Eqiora Language source.

Authority: ``bindings/python/python/eqiora/lang/__init__.py``.
"""

from os import PathLike
from typing import final

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
class Component:
    """Author declarations in one bounded public equations-only Component.

    Authority: ``bindings/python/python/eqiora/lang/__init__.py::Component``.
    """

    def volume(
        self,
        name: str,
        *,
        dimensions: int,
        public: bool = True,
        doc: str | None = None,
    ) -> Support: ...
    def boundary(
        self,
        name: str,
        *,
        parent: Support,
        public: bool = True,
        doc: str | None = None,
    ) -> Support: ...
    def parameter(
        self,
        name: str,
        *,
        unit: _Unit,
        public: bool = True,
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
    def relation(
        self,
        name: str,
        *,
        on: Support,
        residual: Expression | int | float,
        doc: str | None = None,
    ) -> None: ...

@final
class Source:
    """Own one Component draft and freeze it on deterministic emission.

    Authority: ``bindings/python/python/eqiora/lang/__init__.py::Source``.
    """

    def __init__(self) -> None: ...
    def component(
        self,
        name: str,
        *,
        public: bool = True,
        doc: str | None = None,
    ) -> Component: ...
    def to_eqi(self) -> str: ...
    def write_eqi(self, path: str | PathLike[str]) -> None: ...

@final
class _Shape: ...

class _Unit:
    def __mul__(self, other: _Unit, /) -> _Unit: ...
    def __truediv__(self, other: _Unit, /) -> _Unit: ...
    def __pow__(self, exponent: int, /) -> _Unit: ...

class _Units:
    kg: _Unit
    m: _Unit
    s: _Unit

#: Structural SI-unit expressions used by Source declarations.
#:
#: Authority: ``bindings/python/python/eqiora/lang/units.py``.
units: _Units

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

__all__ = [
    "Component",
    "Expression",
    "Source",
    "SourceError",
    "Support",
    "coordinate",
    "div",
    "grad",
    "isotropic_lift",
    "normal",
    "spatial_vector",
    "symmetric_part",
    "trace",
    "units",
]
