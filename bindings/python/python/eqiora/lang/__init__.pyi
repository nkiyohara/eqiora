"""Bounded Python authoring for deterministic Eqiora Language source.

Authority: ``bindings/python/python/eqiora/lang/__init__.py``.
"""

import builtins as _builtins

from collections.abc import Mapping, Sequence
from fractions import Fraction
from decimal import Decimal
from ..units import Unit
from os import PathLike
from typing import Final, final, overload
from .. import FieldRole, ValueType, FiniteSpace, IndexSet

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

    def __add__(self, other: Expression | float | int | complex, /) -> Expression: ...
    def __radd__(self, other: float | int, /) -> Expression: ...
    def __sub__(self, other: Expression | float | int | complex, /) -> Expression: ...
    def __rsub__(self, other: float | int, /) -> Expression: ...
    def __mul__(self, other: Expression | float | int | complex, /) -> Expression: ...
    def __rmul__(self, other: float | int, /) -> Expression: ...
    def __truediv__(self, other: Expression | float | int | complex, /) -> Expression: ...
    def __rtruediv__(self, other: float | int, /) -> Expression: ...
    def __pow__(self, exponent: int, /) -> Expression: ...
    def __bool__(self) -> bool: ...
    def __neg__(self) -> Expression: ...
    def __getitem__(self, index: int) -> Expression: ...

@final
class Clock:
    """Identify one nominal periodic clock in its exact Component.

    Authority: ``bindings/python/python/eqiora/lang/__init__.py::Clock``.
    """

    ...

@final
class Support:
    """Identify one volume or parent-boundary declaration in its exact Source.

    Authority: ``bindings/python/python/eqiora/lang/__init__.py::Support``.
    """

    ...

@final
class PropertyContract:
    """Identify one typed property contract in its exact Source.

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

    def __getitem__(self, name: str) -> PropertyRelease: ...

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

    def counts(self, space: FiniteSpace, components: Sequence[Expression | int]) -> Expression:
        """Construct counts in this Source's exact registered finite basis.

        Authority: ``bindings/python/python/eqiora/lang/__init__.py::Component.counts``.
        """
        ...
    def coordinates(self, space: FiniteSpace, components: Sequence[Expression | int]) -> Expression:
        """Construct signed coordinates in this Source's registered finite basis.

        Authority: ``bindings/python/python/eqiora/lang/__init__.py::Component.coordinates``.
        """
        ...
    def index(self, set: IndexSet, value: Expression | int) -> Expression:
        """Construct an ordinal in this Component's exact registered index set.

        Authority: ``bindings/python/python/eqiora/lang/__init__.py::Component.index``.
        """
        ...
    def index_set(self, name: str, *, extent: int, doc: str | None = None) -> IndexSet:
        """Declare a constant nominal index set; expression extents require authored source.

        Authority: ``bindings/python/python/eqiora/lang/__init__.py::Component.index_set``.
        """
        ...
    def clock(
        self, name: str, *, period_s: Fraction | int,
        phase_s: Fraction | int = 0, doc: str | None = None,
    ) -> Clock: ...
    def initial(self, *equations: tuple[object, object], left: object = None,
                right: object = None, doc: str | None = None) -> None:
        """Add ordered explicit equation pairs or one left/right pair.

        Authority: ``bindings/python/python/eqiora/lang/__init__.py::Component.initial``.
        """
        ...
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
        value_type: ValueType,
        doc: str | None = None,
    ) -> Expression: ...
    def let_alias(
        self,
        name: str,
        expression: Expression | int | float | complex,
        *,
        value_type: ValueType | None = None,
        on: Support | None = None,
        at: Clock | None = None,
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
        on: Support | None = None,
        value_type: ValueType,
        role: FieldRole,
        at: Clock | None = None,
        doc: str | None = None,
    ) -> Expression: ...
    def relation(
        self,
        name: str,
        *,
        on: Support | None = None,
        left: Expression | int | float | complex,
        right: Expression | int | float | complex,
        at: Clock | None = None,
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
        self, name: str, *, component: Component,
        bindings: Mapping[object, object], doc: str | None = None,
    ) -> Mapping[Expression, Expression]: ...
    def clock_requirement(self, name: str, *, doc: str | None = None) -> Clock: ...
    def field_requirement(
        self, name: str, *, value_type: ValueType, role: FieldRole,
        on: Support | None = None, at: Clock | None = None, doc: str | None = None,
    ) -> Expression: ...
    def set_default(self, parameter: Expression, value: Expression | int | float | complex) -> None: ...
    def input(
        self, name: str, *, value_type: ValueType, on: Support | None = None,
        at: Clock | None = None, doc: str | None = None,
    ) -> Expression: ...
    def output(
        self, name: str, *, value_type: ValueType, on: Support | None = None,
        at: Clock | None = None, doc: str | None = None,
    ) -> Expression: ...

@final
class Source:
    """Own a bounded Component hierarchy and freeze it on emission.

    Authority: ``bindings/python/python/eqiora/lang/__init__.py::Source``.
    """

    def space(self, name: str, *, labels: Sequence[str], doc: str | None = None) -> FiniteSpace:
        """Declare an exact ordered basis registered in this Source.

        Authority: ``bindings/python/python/eqiora/lang/__init__.py::Source.space``.
        """
        ...
    def __init__(self) -> None: ...
    def component(
        self,
        name: str,
        *,
        doc: str | None = None,
    ) -> Component: ...
    def model(self, name: str, *, doc: str | None = None) -> Component: ...
    def property_contract(
        self,
        name: str,
        *,
        value_type: ValueType,
        doc: str | None = None,
    ) -> PropertyContract: ...
    def property_release(
        self,
        name: str,
        *,
        implements: PropertyContract,
        value: int | float | complex | Sequence[object],
        source_unit: Unit,
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

class _Math:
    pi: Final[Expression]
    i: Final[Expression]
    @staticmethod
    def complex(real: Expression | float | int | _builtins.complex, imaginary: Expression | float | int | _builtins.complex) -> Expression: ...
    @staticmethod
    def sin(value: Expression | float | int | _builtins.complex) -> Expression: ...
    @staticmethod
    def sqrt(value: Expression | float | int | _builtins.complex) -> Expression: ...

#: Exact language constants used by Source expressions.
#:
#: Authority: ``bindings/python/python/eqiora/lang/__init__.py::math``.
math: _Math

def array(values: Sequence[object]) -> Expression:
    """Construct channel arrays with exact ordered components.

    Authority: ``bindings/python/python/eqiora/lang/__init__.py::array``.
    """
    ...

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
    left: Expression | float | int | complex,
    right: Expression | float | int | complex,
) -> Expression:
    """Return the inner product of two authored expressions.

    Authority: ``bindings/python/python/eqiora/lang/__init__.py::dot``.
    """

    ...

def quotient(left: Expression | int, right: Expression | int) -> Expression:
    """Return the checked integer quotient truncated toward zero.

    Authority: ``bindings/python/python/eqiora/lang/__init__.py::quotient``.
    """
    ...

def remainder(left: Expression | int, right: Expression | int) -> Expression:
    """Return the integer remainder with the dividend's sign.

    Authority: ``bindings/python/python/eqiora/lang/__init__.py::remainder``.
    """
    ...

def ordinal(value: Expression) -> Expression:
    """Explicitly project an index's ordinary integer ordinal.

    Authority: ``bindings/python/python/eqiora/lang/__init__.py::ordinal``.
    """
    ...

def to_real(value: Expression | int) -> Expression:
    """Explicitly convert an integer to a real, with possible precision loss.

    Authority: ``bindings/python/python/eqiora/lang/__init__.py::to_real``.
    """
    ...

def to_integer(value: Expression | float | int) -> Expression:
    """Convert an integral finite dimensionless real within the signed integer range.

    Authority: ``bindings/python/python/eqiora/lang/__init__.py::to_integer``.
    """
    ...

def integrate(
    domain: Support,
    integrand: Expression | float | int | complex,
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

def pre(value: Expression) -> Expression:
    """Read a State's pre-tick value; compiler checks clock and context.

    Authority: ``bindings/python/python/eqiora/lang/__init__.py::pre``.
    """
    ...

def next(value: Expression) -> Expression:
    """Name a State's next-tick value; compiler checks clock and context.

    Authority: ``bindings/python/python/eqiora/lang/__init__.py::next``.
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

def quantity(value: int | float | Decimal, unit: Unit) -> Expression:
    """Author an input quantity; the compiler owns conversion to coherent SI.

    Authority: ``bindings/python/python/eqiora/lang/__init__.py::quantity``.
    """
    ...

__all__ = [
    "equal",
    "not_equal",
    "less",
    "less_equal",
    "greater",
    "greater_equal",
    "logical_not",
    "logical_and",
    "logical_or",

    "Clock",
    "Component",
    "Expression",
    "MaterialComposition",
    "PropertyContract",
    "PropertyRelease",
    "Relation",
    "Source",
    "SourceError",
    "Support",
    "array",
    "coordinate",
    "dot",
    "div",
    "grad",
    "integrate",
    "isotropic_lift",
    "math",
    "normal",
    "ordinal",
    "pre",
    "next",
    "quantity",
    "quotient",
    "remainder",
    "to_real",
    "to_integer",
    "symmetric_part",
    "test",
    "tensor_value",
    "trace",
]


def equal(left: object, right: object) -> Expression:
    """Author a typed equal predicate.

    Authority: ``bindings/python/python/eqiora/lang/__init__.py::equal``.
    """
    ...


def not_equal(left: object, right: object) -> Expression:
    """Author a typed not equal predicate.

    Authority: ``bindings/python/python/eqiora/lang/__init__.py::not_equal``.
    """
    ...


def less(left: object, right: object) -> Expression:
    """Author a typed less predicate.

    Authority: ``bindings/python/python/eqiora/lang/__init__.py::less``.
    """
    ...


def less_equal(left: object, right: object) -> Expression:
    """Author a typed less equal predicate.

    Authority: ``bindings/python/python/eqiora/lang/__init__.py::less_equal``.
    """
    ...


def greater(left: object, right: object) -> Expression:
    """Author a typed greater predicate.

    Authority: ``bindings/python/python/eqiora/lang/__init__.py::greater``.
    """
    ...


def greater_equal(left: object, right: object) -> Expression:
    """Author a typed greater equal predicate.

    Authority: ``bindings/python/python/eqiora/lang/__init__.py::greater_equal``.
    """
    ...


def logical_not(value: object) -> Expression:
    """Author a typed logical not predicate.

    Authority: ``bindings/python/python/eqiora/lang/__init__.py::logical_not``.
    """
    ...


def logical_and(left: object, right: object) -> Expression:
    """Author a typed logical and predicate.

    Authority: ``bindings/python/python/eqiora/lang/__init__.py::logical_and``.
    """
    ...


def logical_or(left: object, right: object) -> Expression:
    """Author a typed logical or predicate.

    Authority: ``bindings/python/python/eqiora/lang/__init__.py::logical_or``.
    """
    ...


def tensor_value(*, frame: Support, components: Sequence[object] | Expression) -> Expression:
    """Construct a uniform spatial value using this Component's explicit frame.

    The compiler checks shape, scalar domain, units, and frame eligibility.
    Channel axes remain explicit array constructions.

    Authority: ``bindings/python/python/eqiora/lang/__init__.py::tensor_value``.
    """
    ...
