"""Author Eqiora Modules, Components, expressions, and equations in Python.

Authority: ``bindings/python/python/eqiora/lang/__init__.py``.
"""

import builtins as _builtins

from collections.abc import Callable, Mapping, Sequence
from fractions import Fraction
from decimal import Decimal
from ..units import Unit
from os import PathLike
from typing import Final, Literal, final
from .. import FieldRole, ValueType, FiniteSpace, IndexSet, _ModelDeclaration

@final
class Connector:
    """Immutable nominal scalar across/through declaration owned by one Module.

    Authority: ``bindings/python/python/eqiora/lang/_connections.py::Connector``.
    """

@final
class Port:
    """Immutable physical endpoint exposing its declared named quantities.

    Use member(name) when a declared name overlaps a Python attribute.

    Authority: ``bindings/python/python/eqiora/lang/_connections.py::Port``.
    """
    def __getattr__(self, name: str) -> Expression: ...
    def member(self, name: str) -> Expression:
        """Select an exact declared quantity, including Python attribute names.

        Authority: ``bindings/python/python/eqiora/lang/_connections.py::Port.member``.
        """
        ...

@final
class Notation:
    """Validated, immutable declaration notation; accepts one complete `@{...}` island.

    Authority: ``crates/eqiora-python/src/notation.rs::PyNotation``.
    """
    def __init__(self, island: str) -> None: ...
    @property
    def canonical(self) -> str: ...
    def __str__(self) -> str: ...

@final
class Enum:
    """A closed enum declaration shared within its Module.

    Authority: ``bindings/python/python/eqiora/lang/__init__.py::Enum``.
    """
    @property
    def name(self) -> str: ...
    @property
    def members(self) -> tuple[str, ...]: ...
    @property
    def value_type(self) -> ValueType: ...
    def member(self, name: str) -> Expression: ...

@final
class Operator:
    """An immutable typed operator declared by one Module; call with named arguments.

    Authority: ``bindings/python/python/eqiora/lang/__init__.py::Operator``.
    """
    def __call__(self, /, **arguments: object) -> Expression: ...

@final
class ModuleError(ValueError):
    """Reject a structurally invalid Module draft.

    Authority: ``bindings/python/python/eqiora/lang/__init__.py::ModuleError``.
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
    def __eq__(self, other: object) -> bool: ...
    def __ne__(self, other: object) -> bool: ...
    def __lt__(self, other: object) -> bool: ...
    def __le__(self, other: object) -> bool: ...
    def __gt__(self, other: object) -> bool: ...
    def __ge__(self, other: object) -> bool: ...
    def __neg__(self) -> Expression: ...
    def __getitem__(self, index: int | Expression | slice) -> Expression: ...

@final
class Equation:
    """Immutable ordered mathematical equality, never a Python truth value.

    Authority: ``bindings/python/python/eqiora/lang/__init__.py::Equation``.
    """

    @property
    def lhs(self) -> Expression: ...
    @property
    def rhs(self) -> Expression: ...
    def __bool__(self) -> bool: ...

def equation(lhs: object, rhs: object) -> Equation:
    """Construct an explicit equality from typed expressions and literals.

    Authority: ``bindings/python/python/eqiora/lang/__init__.py::equation``.
    """
    ...

@final
class Event:
    """Identify a crossing event in its exact Component.

    Authority: ``bindings/python/python/eqiora/lang/__init__.py::Event``.
    """
    ...

@final
class Clock:
    """Identify one nominal periodic clock in its exact Component.

    Authority: ``bindings/python/python/eqiora/lang/__init__.py::Clock``.
    """

    ...

@final
class Support:
    """Identify one volume or parent-boundary declaration in its exact Module.

    Authority: ``bindings/python/python/eqiora/lang/__init__.py::Support``.
    """

    ...

@final
class PropertyContract:
    """Identify one typed property contract in its exact Module.

    Authority: ``bindings/python/python/eqiora/lang/__init__.py::PropertyContract``.
    """

    ...

@final
class PropertyRelease:
    """Identify one exact constant scalar release in its exact Module.

    Authority: ``bindings/python/python/eqiora/lang/__init__.py::PropertyRelease``.
    """

    ...

@final
class MaterialComposition:
    """Identify one immutable typed material composition in its exact Module.

    Authority: ``bindings/python/python/eqiora/lang/__init__.py::MaterialComposition``.
    """

    def __getitem__(self, name: str) -> PropertyRelease: ...

@final
class Relation:
    """Identify one relation declaration in its exact Module.

    Authority: ``bindings/python/python/eqiora/lang/__init__.py::Relation``.
    """

    ...

@final
class Component:
    """Author a public Component and bind an instance of it.

    Authority: ``bindings/python/python/eqiora/lang/__init__.py::Component``.
    """

    def set_notation(self, name: str, notation: Notation) -> None:
        """Attach notation to an existing named declaration in this Component."""
        ...

    def counts(self, space: FiniteSpace, components: Sequence[Expression | int]) -> Expression:
        """Construct counts in this Module's exact registered finite basis.

        Authority: ``bindings/python/python/eqiora/lang/__init__.py::Component.counts``.
        """
        ...
    def coordinates(self, space: FiniteSpace, components: Sequence[Expression | int]) -> Expression:
        """Construct signed coordinates in this Module's registered finite basis.

        Authority: ``bindings/python/python/eqiora/lang/__init__.py::Component.coordinates``.
        """
        ...
    def index(self, set: IndexSet, value: Expression | int) -> Expression:
        """Construct an ordinal in this Component's exact registered index set.

        Authority: ``bindings/python/python/eqiora/lang/__init__.py::Component.index``.
        """
        ...
    def sum(self, body: Callable[[Expression], object], *, over: IndexSet, name: str = "i") -> Expression:
        """Construct a finite sum; call body once with an exact scoped index."""
    def product(self, body: Callable[[Expression], object], *, over: IndexSet, name: str = "i") -> Expression:
        """Construct a finite product; the compiler checks element types and units."""
    def min(self, body: Callable[[Expression], object], *, over: IndexSet, name: str = "i") -> Expression:
        """Construct a finite minimum of compatible real or integer scalars.

        The callback runs once. Evaluation is eager and retains the first tie.
        """
    def max(self, body: Callable[[Expression], object], *, over: IndexSet, name: str = "i") -> Expression:
        """Construct a finite maximum of compatible real or integer scalars.

        The callback runs once. Evaluation is eager and retains the first tie.
        """
    def index_set(self, name: str, *, extent: int, doc: str | None = None) -> IndexSet:
        """Declare a constant nominal index set; expression extents require authored source.

        Authority: ``bindings/python/python/eqiora/lang/__init__.py::Component.index_set``.
        """
        ...
    def event(self, name: str, guard: Expression | int | float, *,
              direction: Literal["any", "rising", "falling"], doc: str | None = None) -> Event:
        """Declare a crossing event with explicit direction; compiler checks the guard.

        Authority: ``bindings/python/python/eqiora/lang/__init__.py::Component.event``.
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
        at: Clock | Event | None = None,
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
        equality: Equation,
        *additional_equalities: Equation,
        on: Support | None = None,
        at: Clock | Event | None = None,
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
    def port(self, name: str, *, connector: Connector, doc: str | None = None) -> Port:
        """Declare a named physical endpoint using this Module's nominal Connector.

        Authority: ``bindings/python/python/eqiora/lang/__init__.py::Component.port``.
        """
        ...
    def connect(self, *ports: Port, doc: str | None = None) -> None:
        """Declare one conserving physical net; the compiler owns compatibility and signs.

        Authority: ``bindings/python/python/eqiora/lang/__init__.py::Component.connect``.
        """
        ...
    def instance(
        self, name: str, *, component: Component | ComponentRef,
        bindings: Mapping[str, object], doc: str | None = None,
    ) -> Mapping[str, Expression | Port]: ...
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
class ComponentRef:
    """Immutable reference to one public Component in an explicit import.

    Authority: ``bindings/python/python/eqiora/lang/__init__.py::ComponentRef``.
    """

    @property
    def name(self) -> str: ...

@final
class ModuleRef:
    """An explicit module import with immutable Component references.

    Authority: ``bindings/python/python/eqiora/lang/__init__.py::ModuleRef``.
    """

    def component(self, name: str) -> ComponentRef: ...

@final
class Module:
    """Own a compiler-backed module graph and freeze declarations on emission or compilation.

    Authority: ``bindings/python/python/eqiora/lang/__init__.py::Module``.
    """

    def set_notation(self, name: str, notation: Notation) -> None:
        """Attach notation to an existing top-level declaration."""
        ...

    def connector(self, name: str, *, across: tuple[str, ValueType],
                  through: tuple[str, ValueType], doc: str | None = None) -> Connector:
        """Declare a nominal scalar physical connector with named across/through quantities.

        Authority: ``bindings/python/python/eqiora/lang/__init__.py::Module.connector``.
        """
        ...
    def operator(self, name: str, *, inputs: Mapping[str, ValueType], result_type: ValueType,
                 body: Callable[..., object], doc: str | None = None) -> Operator:
        """Declare a closed real-scalar operator from one symbolic callback invocation."""
        ...
    def enum(self, name: str, *, members: Sequence[str], doc: str | None = None) -> Enum:
        """Declare a closed enum shared by occurrences in this Module.

        Authority: ``bindings/python/python/eqiora/lang/__init__.py::Module.enum``.
        """
        ...
    def space(self, name: str, *, labels: Sequence[str], doc: str | None = None) -> FiniteSpace:
        """Declare an exact ordered basis registered in this Module.

        Authority: ``bindings/python/python/eqiora/lang/__init__.py::Module.space``.
        """
        ...
    def __init__(self, name: str, *declarations: _ModelDeclaration) -> None: ...
    @classmethod
    def parse(cls, name: str, source: str) -> Module:
        """Parse one source unit; attach its exact imports explicitly before compilation."""
        ...
    def import_module(
        self, alias: str, module: Module | None = None, *,
        path: str | PathLike[str] | None = None,
    ) -> ModuleRef:
        """Attach exactly one Module or local .eqi path to an explicit import alias."""
        ...
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

    @staticmethod
    def abs(value: object) -> Expression:
        """Author dimension-preserving absolute value.

        Authority: ``bindings/python/python/eqiora/lang/__init__.py::_Math.abs``.
        """
        ...

    @staticmethod
    def min(left: object, right: object) -> Expression:
        """Author binary minimum with first-operand ties.

        Authority: ``bindings/python/python/eqiora/lang/__init__.py::_Math.min``.
        """
        ...

    @staticmethod
    def max(left: object, right: object) -> Expression:
        """Author binary maximum with first-operand ties.

        Authority: ``bindings/python/python/eqiora/lang/__init__.py::_Math.max``.
        """
        ...

    @staticmethod
    def clamp(value: object, lower: object, upper: object) -> Expression:
        """Author a clamp with ordered compatible bounds.

        Authority: ``bindings/python/python/eqiora/lang/__init__.py::_Math.clamp``.
        """
        ...

    @staticmethod
    def sign(value: object) -> Expression:
        """Author dimensionless negative one, zero, or positive one.

        Authority: ``bindings/python/python/eqiora/lang/__init__.py::_Math.sign``.
        """
        ...

    @staticmethod
    def step(value: object) -> Expression:
        """Author zero below zero and one at or above zero.

        Authority: ``bindings/python/python/eqiora/lang/__init__.py::_Math.step``.
        """
        ...

#: Exact language constants used by Module expressions.
#:
#: Authority: ``bindings/python/python/eqiora/lang/__init__.py::math``.
math: _Math

def case(value: object, arms: Sequence[tuple[Expression, object]]) -> Expression:
    """Author ordered closed-member cases; compiler checks exhaustiveness and types.

    Authority: ``bindings/python/python/eqiora/lang/__init__.py::case``.
    """
    ...

def if_else(condition: object, then_value: object, else_value: object) -> Expression:
    """Author a conditional with all operands checked and one branch executed.

    Authority: ``bindings/python/python/eqiora/lang/__init__.py::if_else``.
    """
    ...

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
    """Return the test function associated with one Module Field.

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
    """Return one volume integral over an exact Module Support.

    Authority: ``bindings/python/python/eqiora/lang/__init__.py::integrate``.
    """

    ...

def div(value: Expression) -> Expression:
    """Return the language divergence of one expression.

    Authority: ``bindings/python/python/eqiora/lang/__init__.py::div``.
    """

    ...

def derivative(value: Expression) -> Expression:
    """Author a continuous State derivative; the compiler checks role and activation.

    Authority: ``bindings/python/python/eqiora/lang/__init__.py::derivative``.
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
    "Connector",
    "Port",
    "ComponentRef",
    "Equation",
    "equation",
    "Expression",
    "Enum",
    "Event",
    "MaterialComposition",
    "Notation",
    "Operator",
    "PropertyContract",
    "PropertyRelease",
    "Relation",
    "Module",
    "ModuleError",
    "ModuleRef",
    "Support",
    "array",
    "case",
    "coordinate",
    "dot",
    "div",
    "grad",
    "integrate",
    "if_else",
    "isotropic_lift",
    "math",
    "normal",
    "ordinal",
    "derivative",
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
