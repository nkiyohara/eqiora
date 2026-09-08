"""Bounded Python authoring for deterministic Eqiora Language source.

These values only construct source syntax.  The existing native parser, type checker,
lowerer, and compiler remain the sole authority for mathematical meaning.
"""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from fractions import Fraction
from decimal import Decimal, Context, DecimalException, Inexact, Rounded, Overflow, InvalidOperation, MAX_EMAX, MIN_EMIN
import math as _stdlib_math
import os
from pathlib import Path
import re
import tempfile
import textwrap
from typing import Final
from types import MappingProxyType

from .._eqiora import FieldRole, ValueType, FiniteSpace, IndexSet, _nominal_type_source

from ..units import Unit
from .._source_bounds import _MAX_EXPRESSION_DEPTH, _MAX_EXPRESSION_NODES, _MAX_OUTPUT_BYTES

_NAME = re.compile(r"[A-Za-z_][A-Za-z0-9_]*\Z")
_NAME_PATH = re.compile(r"[A-Za-z_][A-Za-z0-9_]*(?:\.[A-Za-z_][A-Za-z0-9_]*)*\Z")
_MAX_DECLARATIONS = 256
_MAX_IDENTIFIER_BYTES = 1_024
_MAX_DOC_BYTES = 16_384
_CREATE = object()
_MISSING = object()


class SourceError(ValueError):
    """A source-authoring value violates the bounded authoring contract."""


class PropertyContract:
    """An identity-bearing typed property contract declaration handle."""

    __slots__ = ("_doc", "_name", "_owner", "_value_type")

    def __init__(
        self,
        _token: object = _MISSING,
        _owner: object = _MISSING,
        _name_value: str = "",
        _value_type: ValueType | None = None,
        _doc: tuple[str, ...] = (),
    ) -> None:
        if _token is not _CREATE or _value_type is None:
            raise TypeError("property contracts are created by Source")
        object.__setattr__(self, "_owner", _owner)
        object.__setattr__(self, "_name", _name_value)
        object.__setattr__(self, "_value_type", _value_type)
        object.__setattr__(self, "_doc", _doc)

    def __setattr__(self, name: str, value: object) -> None:
        raise AttributeError("PropertyContract handles are immutable")


class PropertyRelease:
    """An identity-bearing constant typed property release handle."""

    __slots__ = (
        "_citation",
        "_contract",
        "_doc",
        "_license",
        "_name",
        "_owner",
        "_source_scale",
        "_source_unit",
        "_value",
    )

    def __init__(
        self,
        _token: object = _MISSING,
        *,
        _owner: object = _MISSING,
        _name_value: str = "",
        _contract: PropertyContract | None = None,
        _value: Expression | None = None,
        _source_unit: Unit | None = None,
        _source_scale: int | float = 1,
        _citation: str = "",
        _license: str = "",
        _doc: tuple[str, ...] = (),
    ) -> None:
        if _token is not _CREATE or _contract is None or _source_unit is None:
            raise TypeError("property releases are created by Source")
        object.__setattr__(self, "_owner", _owner)
        object.__setattr__(self, "_name", _name_value)
        object.__setattr__(self, "_contract", _contract)
        object.__setattr__(self, "_value", _value)
        object.__setattr__(self, "_source_unit", _source_unit)
        object.__setattr__(self, "_source_scale", _source_scale)
        object.__setattr__(self, "_citation", _citation)
        object.__setattr__(self, "_license", _license)
        object.__setattr__(self, "_doc", _doc)

    def __setattr__(self, name: str, value: object) -> None:
        raise AttributeError("PropertyRelease handles are immutable")


class MaterialComposition:
    """An immutable binding set from property requirements to exact releases."""

    __slots__ = ("_bindings", "_doc", "_name", "_owner")

    def __init__(
        self,
        _token: object = _MISSING,
        *,
        _owner: object = _MISSING,
        _name_value: str = "",
        _bindings: tuple[tuple[str, PropertyRelease], ...] = (),
        _doc: tuple[str, ...] = (),
    ) -> None:
        if _token is not _CREATE:
            raise TypeError("material compositions are created by Source")
        object.__setattr__(self, "_owner", _owner)
        object.__setattr__(self, "_name", _name_value)
        object.__setattr__(self, "_bindings", _bindings)
        object.__setattr__(self, "_doc", _doc)

    def __getitem__(self, name: str) -> PropertyRelease:
        """Select an exact release through this composition's declared member."""
        for member, release in self._bindings:
            if member == name:
                selected = object.__new__(PropertyRelease)
                for slot in PropertyRelease.__slots__:
                    object.__setattr__(selected, slot, getattr(release, slot))
                object.__setattr__(selected, "_name", f"{self._name}.{member}")
                return selected
        raise KeyError(name)

    def __setattr__(self, name: str, value: object) -> None:
        raise AttributeError("MaterialComposition handles are immutable")


class Expression:
    """A closed Eqiora Language expression; equality is not an equation builder."""

    __slots__ = ("_depth", "_nodes", "_owner", "_precedence", "_text")

    def __init__(
        self,
        _token: object = _MISSING,
        _text: str = "",
        _owner: object | None = None,
        _depth: int = 0,
        _nodes: int = 0,
        _precedence: int = 0,
    ) -> None:
        if _token is not _CREATE:
            raise TypeError(
                "expressions are created by eqiora.lang declarations and operators"
            )
        if _depth > _MAX_EXPRESSION_DEPTH:
            raise SourceError(
                f"expression depth exceeds the {_MAX_EXPRESSION_DEPTH}-node nesting limit"
            )
        if _nodes > _MAX_EXPRESSION_NODES:
            raise SourceError(
                f"expression exceeds the {_MAX_EXPRESSION_NODES}-node limit"
            )
        object.__setattr__(self, "_text", _text)
        object.__setattr__(self, "_owner", _owner)
        object.__setattr__(self, "_depth", _depth)
        object.__setattr__(self, "_nodes", _nodes)
        object.__setattr__(self, "_precedence", _precedence)

    def __setattr__(self, name: str, value: object) -> None:
        raise AttributeError("Expression values are immutable")

    def __add__(self, other: object) -> Expression:
        return _binary(self, "+", other)

    def __radd__(self, other: object) -> Expression:
        return _binary(other, "+", self)

    def __sub__(self, other: object) -> Expression:
        return _binary(self, "-", other)

    def __rsub__(self, other: object) -> Expression:
        return _binary(other, "-", self)

    def __mul__(self, other: object) -> Expression:
        return _binary(self, "*", other)

    def __rmul__(self, other: object) -> Expression:
        return _binary(other, "*", self)

    def __truediv__(self, other: object) -> Expression:
        return _binary(self, "/", other)

    def __rtruediv__(self, other: object) -> Expression:
        return _binary(other, "/", self)

    def __pow__(self, exponent: int) -> Expression:
        if isinstance(exponent, bool) or not isinstance(exponent, int):
            raise TypeError("expression powers require an integer exponent")
        if not -32 <= exponent <= 32:
            raise SourceError("expression exponent must be between -32 and 32")
        base = f"({self._text})" if self._precedence <= 30 else self._text
        return Expression(
            _CREATE,
            f"{base} ^ {exponent}",
            self._owner,
            self._depth + 1,
            self._nodes + 1,
            30,
        )

    def __getitem__(self, index: int) -> Expression:
        if isinstance(index, bool) or not isinstance(index, int):
            raise TypeError("expression indices must be nonnegative integers")
        if index < 0:
            raise SourceError("expression indices must be nonnegative")
        text = _number(index)
        value = f"({self._text})" if self._precedence < 100 else self._text
        return Expression(_CREATE, f"{value}[{text}]", self._owner,
                          self._depth + 1, self._nodes + 2, 100)

    def __bool__(self) -> bool:
        raise TypeError("symbolic Eqiora expressions have no truth value; use explicit predicates")

    def __neg__(self) -> Expression:
        value = f"({self._text})" if self._precedence < 25 else self._text
        return Expression(
            _CREATE,
            f"-{value}",
            self._owner,
            self._depth + 1,
            self._nodes + 1,
            25,
        )


class _Math:
    __slots__ = ()
    pi: Final = Expression(_CREATE, "math.pi", None, 1, 1, 100)
    i: Final = Expression(_CREATE, "math.i", None, 1, 1, 100)

    @staticmethod
    def complex(real: object, imaginary: object) -> Expression:
        left, right = _expression(real), _expression(imaginary)
        return Expression(_CREATE, f"math.complex({left._text}, {right._text})",
                          _owner(left, right), max(left._depth, right._depth) + 1,
                          left._nodes + right._nodes + 1, 100)

    @staticmethod
    def sin(value: object) -> Expression:
        return _unary("math.sin", value)

    @staticmethod
    def sqrt(value: object) -> Expression:
        return _unary("math.sqrt", value)


math: Final = _Math()


class _Parameter(Expression):
    __slots__ = ("_component", "_name")

    def __init__(self, component: object, name: str) -> None:
        super().__init__(_CREATE, name, component, 1, 1, 100)
        object.__setattr__(self, "_component", component)
        object.__setattr__(self, "_name", name)


class _Field(Expression):
    __slots__ = ("_component", "_name")

    def __init__(self, component: object, name: str) -> None:
        super().__init__(_CREATE, name, component, 1, 1, 100)
        object.__setattr__(self, "_component", component)
        object.__setattr__(self, "_name", name)


class Relation:
    """An opaque Source-owned relation declaration handle."""

    __slots__ = ("_component", "_name", "_owner")

    def __init__(
        self,
        _token: object = _MISSING,
        _owner: object = _MISSING,
        _component: object = _MISSING,
        _name: str = "",
    ) -> None:
        if _token is not _CREATE:
            raise TypeError("relations are created by Component.relation()")
        object.__setattr__(self, "_owner", _owner)
        object.__setattr__(self, "_component", _component)
        object.__setattr__(self, "_name", _name)

    def __setattr__(self, name: str, value: object) -> None:
        raise AttributeError("Relation handles are immutable")


class _PropertyRequirement(Expression):
    __slots__ = ("_component", "_contract", "_name")

    def __init__(
        self,
        component: object,
        name: str,
        contract: PropertyContract,
    ) -> None:
        super().__init__(_CREATE, name, component, 1, 1, 100)
        object.__setattr__(self, "_component", component)
        object.__setattr__(self, "_name", name)
        object.__setattr__(self, "_contract", contract)


class Support:
    """An identity-bearing support declaration handle."""

    __slots__ = ("_component", "_kind", "_name", "_owner")

    def __init__(
        self,
        _token: object = _MISSING,
        _owner: object = _MISSING,
        _component: object = _MISSING,
        _name: str = "",
        _kind: str = "",
    ) -> None:
        if _token is not _CREATE:
            raise TypeError(
                "supports are created by Component.volume() or Component.boundary()"
            )
        object.__setattr__(self, "_owner", _owner)
        object.__setattr__(self, "_component", _component)
        object.__setattr__(self, "_name", _name)
        object.__setattr__(self, "_kind", _kind)

    def __setattr__(self, name: str, value: object) -> None:
        raise AttributeError("Support handles are immutable")


class Clock:
    """An immutable nominal periodic clock belonging to one Component.

    Create clocks with Component.clock(); equal periods do not imply identity.
    """

    __slots__ = ("_component", "_name")

    def __init__(self, _token: object = _MISSING, _component: object = _MISSING,
                 _name: str = "") -> None:
        if _token is not _CREATE:
            raise TypeError("clocks are created by Component.clock()")
        object.__setattr__(self, "_component", _component)
        object.__setattr__(self, "_name", _name)

    def __setattr__(self, name: str, value: object) -> None:
        raise AttributeError("Clock handles are immutable")


def _clock_seconds(value: Fraction | int, *, positive: bool) -> Fraction:
    if isinstance(value, bool) or not isinstance(value, (Fraction, int)):
        raise TypeError("clock seconds must be Fraction or int, not float or bool")
    exact = Fraction(value)
    if exact < 0 or (positive and exact == 0):
        raise SourceError("clock period must be positive and phase must be nonnegative")
    if exact.numerator > (1 << 64) - 1 or exact.denominator > (1 << 64) - 1:
        raise SourceError("clock rational numerator and denominator must fit u64")
    return exact


def _number(value: object) -> str:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise TypeError("numeric literals must be finite int or float values, not bool")
    if isinstance(value, float) and not _stdlib_math.isfinite(value):
        raise SourceError("numeric literals must be finite")
    if isinstance(value, int) and value.bit_length() > 1_024:
        raise SourceError("integer literal exceeds the 1024-bit authoring limit")
    text = repr(value)
    if len(text) > 1_024:
        raise SourceError("numeric literal exceeds the 1024-byte authoring limit")
    return text


def _expression(value: object) -> Expression:
    if isinstance(value, bool):
        return Expression(_CREATE, "true" if value else "false", None, 1, 1, 100)
    if isinstance(value, Expression):
        return value
    if isinstance(value, complex):
        return math.complex(value.real, value.imag)
    text = _number(value)
    return Expression(_CREATE, text, None, 1, 1, 25 if text.startswith("-") else 100)


def array(values: Sequence[object]) -> Expression:
    """Construct ordered channel axes; arrays never infer spatial vector roles."""
    def build(items: Sequence[object], depth: int) -> Expression:
        if depth > _MAX_EXPRESSION_DEPTH:
            raise SourceError("array expression depth exceeds the 64-node nesting limit")
        if isinstance(items, (str, bytes)) or not isinstance(items, Sequence):
            raise TypeError("array values must be a nonempty sequence")
        if not items:
            raise SourceError("array values must be nonempty")
        if len(items) >= _MAX_EXPRESSION_NODES:
            raise SourceError("array expression exceeds the 4096-node limit")
        expressions = []
        owner = None
        nodes = 1
        for item in items:
            value = build(item, depth + 1) if isinstance(item, Sequence) and not isinstance(item, (str, bytes)) else _expression(item)
            if owner is not None and value._owner is not None and value._owner is not owner:
                raise SourceError("cannot combine expressions from different Source or Component owners")
            owner = owner if value._owner is None else value._owner
            nodes += value._nodes
            if nodes > _MAX_EXPRESSION_NODES:
                raise SourceError("array expression exceeds the 4096-node limit")
            expressions.append(value)
        return Expression(_CREATE, "[" + ", ".join(value._text for value in expressions) + "]",
                          owner, max(value._depth for value in expressions) + 1, nodes, 100)
    return build(values, 1)


def _literal_expression(value: object) -> Expression:
    nodes = 0
    def literal(item: object, depth: int) -> Expression:
        nonlocal nodes
        nodes += 1
        if nodes > _MAX_EXPRESSION_NODES:
            raise SourceError("literal exceeds the 4096-node limit")
        if depth > _MAX_EXPRESSION_DEPTH:
            raise SourceError("literal depth exceeds the 64-node nesting limit")
        if isinstance(item, Sequence) and not isinstance(item, (str, bytes)):
            if len(item) >= _MAX_EXPRESSION_NODES:
                raise SourceError("literal exceeds the 4096-node limit")
            return array([literal(child, depth + 1) for child in item])
        if isinstance(item, Expression):
            raise TypeError("property release values must be numeric literals")
        return _expression(item)
    return literal(value, 1)


def quantity(value: int | float | Decimal, unit: Unit) -> Expression:
    """Author an exact decimal input quantity, normalized by the compiler.

    Decimal and integer inputs retain their authored decimal value. Floats use
    Python's shortest round-trip decimal spelling, not their exact binary ratio.
    Native numerical inputs remain binary64 coherent-SI values.
    """
    if not isinstance(unit, Unit):
        raise TypeError("unit must be an eqiora.units.Unit")
    if isinstance(value, Decimal):
        if not value.is_finite():
            raise SourceError("quantity literals must be finite")
        # Count significant digits before formatting; never expand an exponent
        # into a potentially enormous fixed-point decimal string.
        try:
            bounded = Context(prec=256, Emax=MAX_EMAX, Emin=MIN_EMIN,
                              traps=[Inexact, Rounded, Overflow, InvalidOperation]).create_decimal(value)
        except DecimalException as error:
            raise SourceError("quantity literal exceeds the 256-byte limit") from error
        literal = str(bounded)
    else:
        literal = _number(value)
    if len(literal.encode("ascii")) > 256:
        raise SourceError("quantity literal exceeds the 256-byte limit")
    text = f"{literal} [{unit._text}]"
    return Expression(_CREATE, text, None, 1, 1, 25 if literal.startswith("-") else 100)


def _owner(left: Expression, right: Expression) -> object | None:
    if (
        left._owner is not None
        and right._owner is not None
        and left._owner is not right._owner
    ):
        raise SourceError("cannot combine expressions from different Source or Component owners")
    return left._owner if left._owner is not None else right._owner


def _binary(left: object, operator: str, right: object) -> Expression:
    left_expr = _expression(left)
    right_expr = _expression(right)
    precedence = 10 if operator in ("+", "-") else 20
    left_text = (
        f"({left_expr._text})"
        if left_expr._precedence < precedence
        else left_expr._text
    )
    right_text = (
        f"({right_expr._text})"
        if right_expr._precedence <= precedence
        else right_expr._text
    )
    return Expression(
        _CREATE,
        f"{left_text} {operator} {right_text}",
        _owner(left_expr, right_expr),
        max(left_expr._depth, right_expr._depth) + 1,
        left_expr._nodes + right_expr._nodes + 1,
        precedence,
    )


def _predicate(left: object, operator: str, right: object) -> Expression:
    left, right = _expression(left), _expression(right)
    return Expression(_CREATE, f"({left._text}) {operator} ({right._text})",
                      _owner(left, right), max(left._depth, right._depth) + 1,
                      left._nodes + right._nodes + 1, 1)


def equal(left: object, right: object) -> Expression:
    """Author equal without evaluating Python equality or truthiness."""
    return _predicate(left, "==", right)


def not_equal(left: object, right: object) -> Expression:
    """Author not equal without evaluating Python equality or truthiness."""
    return _predicate(left, "!=", right)


def less(left: object, right: object) -> Expression:
    """Author less without evaluating Python equality or truthiness."""
    return _predicate(left, "<", right)


def less_equal(left: object, right: object) -> Expression:
    """Author less equal without evaluating Python equality or truthiness."""
    return _predicate(left, "<=", right)


def greater(left: object, right: object) -> Expression:
    """Author greater without evaluating Python equality or truthiness."""
    return _predicate(left, ">", right)


def greater_equal(left: object, right: object) -> Expression:
    """Author greater equal without evaluating Python equality or truthiness."""
    return _predicate(left, ">=", right)


def logical_and(left: object, right: object) -> Expression:
    """Author logical and without evaluating Python equality or truthiness."""
    return _predicate(left, "and", right)


def logical_or(left: object, right: object) -> Expression:
    """Author logical or without evaluating Python equality or truthiness."""
    return _predicate(left, "or", right)


def logical_not(value: object) -> Expression:
    """Author logical negation without evaluating Python truthiness."""
    value = _expression(value)
    return Expression(_CREATE, f"not ({value._text})", value._owner,
                      value._depth + 1, value._nodes + 1, 1)


def _unary(name: str, value: object) -> Expression:
    expression = _expression(value)
    return Expression(
        _CREATE,
        f"{name}({expression._text})",
        expression._owner,
        expression._depth + 1,
        expression._nodes + 1,
        100,
    )


def coordinate(axis: int) -> Expression:
    if isinstance(axis, bool) or not isinstance(axis, int):
        raise TypeError("coordinate axis must be an integer")
    if not 0 <= axis <= 15:
        raise SourceError("coordinate axis must be between 0 and 15")
    return Expression(_CREATE, f"coordinate({axis})", None, 1, 1, 100)


def grad(value: object) -> Expression:
    return _unary("grad", value)


def test(field: object) -> Expression:
    if not isinstance(field, _Field):
        raise SourceError("test() requires a Field from this Source")
    return _unary("test", field)


def _binary_function(name: str, left: object, right: object) -> Expression:
    left_expression = _expression(left)
    right_expression = _expression(right)
    return Expression(
        _CREATE,
        f"{name}({left_expression._text}, {right_expression._text})",
        _owner(left_expression, right_expression),
        max(left_expression._depth, right_expression._depth) + 1,
        left_expression._nodes + right_expression._nodes + 1,
        100,
    )


def dot(left: object, right: object) -> Expression:
    return _binary_function("dot", left, right)


def quotient(left: object, right: object) -> Expression:
    """Exact integer quotient truncated toward zero; overflow and zero divisors reject."""
    return _binary_function("quotient", left, right)


def remainder(left: object, right: object) -> Expression:
    """Exact integer remainder with the dividend's sign when nonzero."""
    return _binary_function("remainder", left, right)


def ordinal(value: Expression) -> Expression:
    """Explicitly read an index's ordinary integer ordinal without changing its set."""
    return _unary("ordinal", value)


def to_real(value: object) -> Expression:
    """Explicitly convert an integer to real; binary64 rounding can lose integer precision."""
    return _unary("to_real", value)


def to_integer(value: object) -> Expression:
    """Convert only an integral, finite, dimensionless real in the signed 64-bit range."""
    return _unary("to_integer", value)


def integrate(domain: Support, integrand: object) -> Expression:
    if not isinstance(domain, Support) or domain._kind != "volume":
        raise SourceError("integrate() requires a volume Support")
    expression = _expression(integrand)
    if expression._owner is not None and expression._owner is not domain._component:
        raise SourceError("integrand and Support must belong to the same Component")
    return Expression(
        _CREATE,
        f"integrate({domain._name}, {expression._text})",
        domain._component,
        expression._depth + 1,
        expression._nodes + 1,
        100,
    )


def div(value: object) -> Expression:
    return _unary("div", value)


def pre(value: Expression) -> Expression:
    """Read a State's pre-tick value; the compiler checks clock and context."""
    return _unary("pre", value)


def next(value: Expression) -> Expression:
    """Name a State's next-tick value; the compiler checks clock and context."""
    return _unary("next", value)


def trace(value: object) -> Expression:
    return _unary("trace", value)


def normal(value: object) -> Expression:
    return _unary("normal", value)


def symmetric_part(value: object) -> Expression:
    return _unary("symmetric_part", value)


def isotropic_lift(value: object) -> Expression:
    return _unary("isotropic_lift", value)


def _name(value: object) -> str:
    if not isinstance(value, str):
        raise TypeError("declaration names must be strings")
    if not _NAME.fullmatch(value) or len(value.encode("utf-8")) > _MAX_IDENTIFIER_BYTES:
        raise SourceError(f"invalid Eqiora Language declaration name {value!r}")
    return value


def _name_path(value: object, label: str) -> str:
    if not isinstance(value, str):
        raise TypeError(f"{label} must be a string")
    if (
        not _NAME_PATH.fullmatch(value)
        or len(value.encode("utf-8")) > _MAX_IDENTIFIER_BYTES
    ):
        raise SourceError(f"invalid Eqiora Language {label} {value!r}")
    return value


def _doc(value: object | None) -> tuple[str, ...]:
    if value is None:
        return ()
    if not isinstance(value, str):
        raise TypeError("doc must be a string or None")
    if len(value.encode("utf-8")) > _MAX_DOC_BYTES:
        raise SourceError(f"doc exceeds the {_MAX_DOC_BYTES}-byte limit")
    if "\r" in value or any(
        character < " " and character not in "\n\t" for character in value
    ):
        raise SourceError("doc contains unsupported control characters")
    return tuple(value.split("\n"))


def _comment(lines: tuple[str, ...], indent: str) -> list[str]:
    return [f"{indent}/// {line}" if line else f"{indent}///" for line in lines]


def _relation_lines(left: Expression, right: Expression) -> list[str]:
    lines = textwrap.wrap(
        f"{left._text} = {right._text};",
        width=88,
        initial_indent="    ",
        subsequent_indent="      ",
        break_long_words=False,
        break_on_hyphens=False,
    )
    for index in range(len(lines) - 1):
        stripped = lines[index].rstrip()
        if stripped[-1:] in ("+", "-", "*", "/"):
            operator = stripped[-1]
            lines[index] = stripped[:-1].rstrip()
            lines[index + 1] = f"      {operator} {lines[index + 1].lstrip()}"
    return lines


class Component:
    """The shared bounded draft for one public Model or Component definition."""

    __slots__ = (
        "_aliases",
        "_kind",
        "_requirements",
        "_defaults",
        "_causal",
        "_clocks",
        "_initials",
        "_index_sets",
        "_component_token",
        "_declaration_count",
        "_doc",
        "_fields",
        "_formulations",
        "_instances",
        "_name",
        "_names",
        "_owner",
        "_parameters",
        "_properties",
        "_relations",
        "_source",
        "_supports",
    )

    def __init__(
        self,
        _token: object = _MISSING,
        _source: Source | None = None,
        _name_value: str = "",
        _doc_value: object | None = None,
    ) -> None:
        if _token is not _CREATE:
            raise TypeError("components are created by Source.component()")
        assert _source is not None
        self._source = _source
        self._kind = "component"
        self._requirements: set[object] = set()
        self._defaults: dict[Expression, Expression] = {}
        self._causal: dict[Expression, str] = {}
        self._owner = _source._owner
        self._component_token = object()
        self._name = _name(_name_value)
        self._doc = (
            _doc_value if isinstance(_doc_value, tuple) else _doc(_doc_value)
        )
        self._names: set[str] = set()
        self._supports: list[tuple[Support, str, object, tuple[str, ...]]] = []
        self._clocks: list[tuple[Clock, Fraction | None, Fraction | None, tuple[str, ...]]] = []
        self._initials: list[tuple[tuple[tuple[Expression, Expression], ...], tuple[str, ...]]] = []
        self._index_sets: list[tuple[IndexSet, tuple[str, ...]]] = []
        self._parameters: list[tuple[_Parameter, str, tuple[str, ...]]] = []
        self._aliases: list[tuple[str, Expression, str | None, Support | None, Clock | None, tuple[str, ...]]] = []
        self._properties: list[
            tuple[_PropertyRequirement, PropertyContract, tuple[str, ...]]
        ] = []
        self._fields: list[
            tuple[
                Expression, Support, str, FieldRole, Clock | None, tuple[str, ...]
            ]
        ] = []
        self._relations: list[
            tuple[str, Support, Expression, Expression, Clock | None, tuple[str, ...]]
        ] = []
        self._formulations: list[
            tuple[Relation, Expression, Expression, tuple[str, ...]]
        ] = []
        self._instances: list[tuple[str, Component, tuple[tuple[str, str], ...], tuple[str, ...]]] = []
        self._declaration_count = 0

    def _type_syntax(self, value_type: ValueType) -> str:
        try:
            return _nominal_type_source(value_type, [space for space, _ in self._source._spaces], [item for item, _ in self._index_sets])
        except ValueError as error:
            raise SourceError(str(error)) from error

    def _nominal_value(self, function: str, name: str, value: Expression) -> Expression:
        if value._owner is not None and value._owner is not self._component_token:
            raise SourceError("nominal value components must belong to this Component")
        return Expression(_CREATE, f"{function}({name}, {value._text})", self._component_token,
                          value._depth + 1, value._nodes + 2, 100)

    def counts(self, space: FiniteSpace, components: Sequence[object]) -> Expression:
        """Construct nonnegative counts in an exact basis registered by this Source."""
        if not isinstance(space, FiniteSpace) or not any(space == item for item, _ in self._source._spaces):
            raise SourceError("count space must belong to this Source")
        return self._nominal_value("counts", space.name, array(components))

    def coordinates(self, space: FiniteSpace, components: Sequence[object]) -> Expression:
        """Construct signed integer coordinates in this Source's exact finite basis."""
        if not isinstance(space, FiniteSpace) or not any(space == item for item, _ in self._source._spaces):
            raise SourceError("coordinate space must belong to this Source")
        return self._nominal_value("coordinates", space.name, array(components))

    def index(self, set: IndexSet, value: object) -> Expression:
        """Construct a checked ordinal in an index set registered by this Component."""
        if not isinstance(set, IndexSet) or not any(set == item for item, _ in self._index_sets):
            raise SourceError("index set must belong to this Component")
        return self._nominal_value("index", set.name, _expression(value))

    def index_set(self, name: str, *, extent: int, doc: str | None = None) -> IndexSet:
        """Declare a constant nominal index set; expression extents require authored source."""
        self._source._ensure_open()
        doc_lines = _doc(doc)
        value = IndexSet(_name(name), extent=extent)
        self._add_name(name)
        self._index_sets.append((value, doc_lines))
        return value

    def _add_name(self, name: object) -> str:
        self._source._ensure_open()
        admitted = _name(name)
        if admitted in self._names:
            raise SourceError(f"duplicate declaration name {admitted!r}")
        if self._declaration_count >= _MAX_DECLARATIONS:
            raise SourceError(
                f"Component exceeds the {_MAX_DECLARATIONS}-declaration limit"
            )
        self._names.add(admitted)
        self._declaration_count += 1
        return admitted

    def _support(self, support: object) -> Support:
        if (
            not isinstance(support, Support)
            or support._component is not self._component_token
        ):
            raise SourceError("support must belong to this Component and Source")
        return support

    def _clock(self, clock: Clock | None) -> Clock | None:
        if clock is not None and (
            not isinstance(clock, Clock) or clock._component is not self._component_token
        ):
            raise SourceError("clock must belong to this Component and Source")
        return clock

    def clock(
        self, name: str, *, period_s: Fraction | int,
        phase_s: Fraction | int = 0, doc: str | None = None,
    ) -> Clock:
        """Declare a nominal clock with exact rational period and phase in seconds."""
        period = _clock_seconds(period_s, positive=True)
        phase = _clock_seconds(phase_s, positive=False)
        doc_lines = _doc(doc)
        admitted = self._add_name(name)
        clock = Clock(_CREATE, self._component_token, admitted)
        self._clocks.append((clock, period, phase, doc_lines))
        return clock

    def initial(self, *equations: tuple[object, object],
                left: object = None, right: object = None, doc: str | None = None) -> None:
        """Add simultaneous equations with explicit sides, or one left/right pair.

        Equations are not guesses or ordered writes. Both sides retain their
        exact type, Component ownership, and authored expression budget.
        """
        self._source._ensure_open()
        if (left is None) != (right is None):
            raise TypeError("initial requires both left and right")
        if left is not None and equations:
            raise TypeError("initial left/right cannot be combined with equation pairs")
        pairs = ((left, right),) if left is not None else equations
        if any(not isinstance(pair, (tuple, list)) or len(pair) != 2 for pair in pairs):
            raise TypeError("each initial equation requires two explicit sides")
        pairs = tuple((_expression(left), _expression(right)) for left, right in pairs)
        expressions = tuple(value for pair in pairs for value in pair)
        if any(value._owner is not None and value._owner is not self._component_token for value in expressions):
            raise SourceError("initial expressions must belong to this Component")
        total_nodes = sum(value._nodes for equations, _ in self._initials for pair in equations for value in pair)
        total_nodes += sum(value._nodes for value in expressions)
        if total_nodes > _MAX_EXPRESSION_NODES:
            raise SourceError(f"Component initial expressions exceed the {_MAX_EXPRESSION_NODES}-node limit")
        doc_lines = _doc(doc)
        if self._declaration_count >= _MAX_DECLARATIONS:
            raise SourceError(f"Component exceeds the {_MAX_DECLARATIONS}-declaration limit")
        self._declaration_count += 1
        self._initials.append((pairs, doc_lines))

    def volume(
        self,
        name: str,
        *,
        dimensions: int,
        doc: str | None = None,
    ) -> Support:
        if isinstance(dimensions, bool) or not isinstance(dimensions, int):
            raise TypeError("volume dimensions must be an integer")
        if not 1 <= dimensions <= 15:
            raise SourceError("volume dimensions must be between 1 and 15")
        admitted = self._add_name(name)
        support = Support(
            _CREATE, self._owner, self._component_token, admitted, "volume"
        )
        self._supports.append((support, "volume", dimensions, _doc(doc)))
        return support

    def boundary(
        self,
        name: str,
        *,
        parent: Support,
        doc: str | None = None,
    ) -> Support:
        parent = self._support(parent)
        if parent._kind != "volume":
            raise SourceError("a boundary parent must be a volume from this Component")
        admitted = self._add_name(name)
        support = Support(
            _CREATE, self._owner, self._component_token, admitted, "boundary"
        )
        self._supports.append((support, "boundary", parent, _doc(doc)))
        return support

    def parameter(
        self,
        name: str,
        *,
        value_type: ValueType,
        doc: str | None = None,
    ) -> Expression:
        if not isinstance(value_type, ValueType):
            raise TypeError("value_type must be an eqiora.ValueType")
        syntax = self._type_syntax(value_type)
        doc_lines = _doc(doc)
        admitted = self._add_name(name)
        parameter = _Parameter(self._component_token, admitted)
        self._parameters.append((parameter, syntax, doc_lines))
        self._requirements.add(parameter)
        return parameter

    def set_default(self, parameter: Expression, value: Expression | int | float | complex) -> None:
        """Set a signature Parameter default after declaring its lexical dependencies."""
        self._source._ensure_open()
        if not isinstance(parameter, _Parameter) or parameter not in self._requirements:
            raise SourceError("default target must be this Component's Parameter requirement")
        expression = _expression(value)
        if expression._owner is not None and expression._owner is not self._component_token:
            raise SourceError("Parameter defaults must belong to this Component")
        total = expression._nodes + sum(value._nodes for target, value in self._defaults.items() if target is not parameter)
        if total > _MAX_EXPRESSION_NODES:
            raise SourceError("Parameter defaults exceed the 4096-node expression limit")
        self._defaults[parameter] = expression

    def clock_requirement(self, name: str, *, doc: str | None = None) -> Clock:
        """Declare a borrowed nominal clock in the external signature."""
        doc_lines = _doc(doc)
        admitted = self._add_name(name)
        clock = Clock(_CREATE, self._component_token, admitted)
        self._clocks.append((clock, None, None, doc_lines))
        self._requirements.add(clock)
        return clock

    def field_requirement(
        self, name: str, *, on: Support | None = None, value_type: ValueType, role: FieldRole,
        at: Clock | None = None, doc: str | None = None,
    ) -> Expression:
        """Declare a borrowed Field; an occurrence never allocates its storage."""
        field = self.field(name, on=on, value_type=value_type, role=role, at=at, doc=doc)
        self._requirements.add(field)
        return field

    def let_alias(
        self,
        name: str,
        expression: Expression | int | float | complex,
        *,
        value_type: ValueType | None = None,
        on: Support | None = None,
        at: Clock | None = None,
        doc: str | None = None,
    ) -> Expression:
        """Name a private immutable expression in this Component's lexical scope.

        The compiler infers type and intrinsic support; aliases add no storage.
        ``on`` asserts the exact inferred support; it cannot move or broadcast
        an expression. ``at`` asserts one exact clock in the inferred dependency
        profile; it grants no pre/next permissions. Context-dependent coordinate,
        trace, or normal aliases are not admitted.
        """
        value = _expression(expression)
        if value._owner is not None and value._owner is not self._component_token:
            raise SourceError("alias expressions must belong to this Component")
        if value_type is not None and not isinstance(value_type, ValueType):
            raise TypeError("value_type must be an eqiora.ValueType")
        if on is not None:
            self._support(on)
        at = self._clock(at)
        syntax = None if value_type is None else self._type_syntax(value_type)
        doc_lines = _doc(doc)
        if sum(item[1]._nodes for item in self._aliases) + value._nodes > _MAX_EXPRESSION_NODES:
            raise SourceError(
                f"Component alias expressions exceed the {_MAX_EXPRESSION_NODES}-node limit"
            )
        admitted = self._add_name(name)
        self._aliases.append((admitted, value, syntax, on, at, doc_lines))
        return Expression(_CREATE, admitted, self._component_token, 1, 1, 100)

    def property(
        self,
        name: str,
        *,
        contract: PropertyContract,
        doc: str | None = None,
    ) -> Expression:
        if not isinstance(contract, PropertyContract) or contract._owner is not self._owner:
            raise SourceError("property contract must belong to this Source")
        admitted = self._add_name(name)
        requirement = _PropertyRequirement(
            self._component_token, admitted, contract
        )
        self._properties.append((requirement, contract, _doc(doc)))
        return requirement

    def field(
        self,
        name: str,
        *,
        on: Support | None = None,
        value_type: ValueType,
        role: FieldRole,
        at: Clock | None = None,
        doc: str | None = None,
    ) -> Expression:
        """Declare a spatial Field, optionally activated by an exact local clock."""
        at = self._clock(at)
        on = None if on is None else self._support(on)
        if on is not None and on._kind != "volume":
            raise SourceError(
                "the initial Source vocabulary admits fields on volumes only"
            )
        if not isinstance(value_type, ValueType):
            raise TypeError("value_type must be an eqiora.ValueType")
        syntax = self._type_syntax(value_type)
        if not isinstance(role, FieldRole):
            raise TypeError("role must be an eqiora.FieldRole")
        doc_lines = _doc(doc)
        admitted = self._add_name(name)
        expression = _Field(self._component_token, admitted)
        self._fields.append((expression, on, syntax, role, at, doc_lines))
        return expression

    def input(
        self, name: str, *, value_type: ValueType, on: Support | None = None,
        at: Clock | None = None, doc: str | None = None,
    ) -> Expression:
        """Declare a causal input; runtime samples are supplied after static compilation."""
        field = self.field(name, on=on, value_type=value_type, role=FieldRole.Variable, at=at, doc=doc)
        self._causal[field] = "input"
        self._requirements.add(field)
        return field

    def output(
        self, name: str, *, value_type: ValueType, on: Support | None = None,
        at: Clock | None = None, doc: str | None = None,
    ) -> Expression:
        """Declare a causal output defined by an ordinary body relation."""
        field = self.field(name, on=on, value_type=value_type, role=FieldRole.Variable, at=at, doc=doc)
        self._causal[field] = "output"
        return field

    def relation(
        self,
        name: str,
        *,
        on: Support | None = None,
        left: Expression | int | float | complex,
        right: Expression | int | float | complex,
        at: Clock | None = None,
        doc: str | None = None,
    ) -> Relation:
        """Declare an equality, optionally active on one exact local clock."""
        at = self._clock(at)
        on = None if on is None else self._support(on)
        def admit(value: Expression | int | float | complex) -> Expression:
            expression = _expression(value)
            if expression._owner is None:
                return Expression(
                    _CREATE,
                    expression._text,
                    self._component_token,
                    expression._depth,
                    expression._nodes,
                    expression._precedence,
                )
            if expression._owner is not self._component_token:
                raise SourceError("relation expressions must belong to this Component")
            return expression

        left_expression = admit(left)
        right_expression = admit(right)
        doc_lines = _doc(doc)
        total_nodes = (
            sum(
                item[2]._nodes + item[3]._nodes
                for item in self._relations
            )
            + left_expression._nodes
            + right_expression._nodes
        )
        if total_nodes > _MAX_EXPRESSION_NODES:
            raise SourceError(
                f"Component relation expressions exceed the {_MAX_EXPRESSION_NODES}-node limit"
            )
        admitted = self._add_name(name)
        self._relations.append(
            (admitted, on, left_expression, right_expression, at, doc_lines)
        )
        return Relation(_CREATE, self._owner, self._component_token, admitted)

    def primal_form(
        self,
        relation: Relation,
        *,
        left: Expression,
        right: Expression,
        doc: str | None = None,
    ) -> None:
        """Attach one natural scalar-primal equality to a Relation."""

        self._source._ensure_open()
        if (
            not isinstance(relation, Relation)
            or relation._component is not self._component_token
        ):
            raise SourceError("form relation must belong to this Component")
        left_expression = _expression(left)
        right_expression = _expression(right)
        for expression in (left_expression, right_expression):
            if expression._owner is not self._component_token:
                raise SourceError("form expressions must belong to this Component")
        if self._formulations:
            raise SourceError("the scalar-primal Source vocabulary admits one form")
        total_nodes = (
            sum(
                item[2]._nodes + item[3]._nodes
                for item in self._relations
            )
            + left_expression._nodes
            + right_expression._nodes
        )
        if total_nodes > _MAX_EXPRESSION_NODES:
            raise SourceError(
                f"Component relation and form expressions exceed the {_MAX_EXPRESSION_NODES}-node limit"
            )
        if self._declaration_count >= _MAX_DECLARATIONS:
            raise SourceError(
                f"Component exceeds the {_MAX_DECLARATIONS}-declaration limit"
            )
        self._declaration_count += 1
        self._formulations.append(
            (relation, left_expression, right_expression, _doc(doc))
        )

    def instance(
        self, name: str, *, component: Component,
        bindings: Mapping[object, object], doc: str | None = None,
    ) -> Mapping[Expression, Expression]:
        """Bind explicit target signature handles to values in this lexical scope."""
        if not isinstance(component, Component) or component._owner is not self._owner:
            raise SourceError("instance component must belong to this Source")
        if component is self or component._kind != "component":
            raise SourceError("an instance requires another Component definition")
        if not isinstance(bindings, Mapping):
            raise TypeError("bindings must map target signature handles to enclosing values")
        targets = (set(component._requirements)
                   | {item[0] for item in component._supports}
                   | {item[0] for item in component._properties})
        required = targets - set(component._defaults)
        if not required <= set(bindings) or not set(bindings) <= targets:
            raise SourceError("instance bindings must satisfy the exact required signature")
        admitted_bindings = []
        for target, value in bindings.items():
            if isinstance(target, Support):
                self._support(value)
                expression = value._name
            elif isinstance(target, Clock):
                if not isinstance(value, Clock):
                    raise SourceError("clock requirement needs an enclosing Clock")
                self._clock(value)
                expression = value._name
            elif component._causal.get(target) == "input":
                value = _expression(value)
                if value._owner is not None and value._owner is not self._component_token:
                    raise SourceError("input bindings must belong to this Component")
                expression = value._text
            elif isinstance(target, _Field):
                if not isinstance(value, _Field) or value._component is not self._component_token:
                    raise SourceError("Field requirement needs an enclosing Field")
                expression = value._name
            elif isinstance(target, _PropertyRequirement):
                if (not isinstance(value, PropertyRelease) or value._owner is not self._owner
                        or value._contract is not target._contract):
                    raise SourceError("property binding requires the exact Source contract release")
                expression = value._name
            else:
                value = _expression(value)
                if value._owner is not None and value._owner is not self._component_token:
                    raise SourceError("instance Parameter values must belong to this Component")
                expression = value._text
            admitted_bindings.append((target._name, expression))
        doc_lines = _doc(doc)
        admitted = self._add_name(name)
        self._instances.append((admitted, component, tuple(admitted_bindings), doc_lines))
        return MappingProxyType({
            field: Expression(_CREATE, f"{admitted}.{field._name}", self._component_token, 1, 1, 100)
            for field, kind in component._causal.items() if kind == "output"
        })

    def _render(self) -> str:
        lines = _comment(self._doc, "")
        signature = []
        for support, kind, detail, doc in self._supports:
            syntax = (f"volume(ambient_dimension = {detail})" if kind == "volume"
                      else f"boundary(parent = {detail._name})")
            signature.extend(_comment(doc, "  "))
            signature.append(f"  support {support._name}: {syntax},")
        for parameter, value_type, doc in self._parameters:
            signature.extend(_comment(doc, "  "))
            default = self._defaults.get(parameter)
            suffix = "" if default is None else f" = {default._text}"
            signature.append(f"  parameter {parameter._name}: {value_type}{suffix},")
        for requirement, contract, doc in self._properties:
            signature.extend(_comment(doc, "  "))
            signature.append(f"  property {requirement._name}: {contract._name},")
        for clock, period, phase, doc in self._clocks:
            if clock in self._requirements:
                signature.extend(_comment(doc, "  "))
                signature.append(f"  clock {clock._name}: periodic,")
        for field, support, value_type, role, clock, doc in self._fields:
            if field in self._requirements or field in self._causal:
                signature.extend(_comment(doc, "  "))
                keyword = self._causal.get(field, "state" if role == FieldRole.State else "variable")
                activation = "" if clock is None else f" at {clock._name}"
                spatial = "" if support is None else f" on {support._name}"
                signature.append(f"  {keyword} {field._name}: {value_type}{spatial}{activation},")
        lines.append(f"public {self._kind} {self._name}(")
        lines.extend(signature)
        lines.append(") {")
        for index_set, doc in self._index_sets:
            lines.extend(_comment(doc, "  "))
            lines.append(f"  indexset {index_set.name} = range({index_set.extent});")
        for clock, period, phase, doc in self._clocks:
            if clock in self._requirements:
                continue
            lines.extend(_comment(doc, "  "))
            lines.append(
                f"  clock {clock._name} = periodic({period.numerator} [s] / {period.denominator}, "
                f"phase = {phase.numerator} [s] / {phase.denominator});"
            )
        for name, expression, value_type, support, clock, doc in self._aliases:
            lines.extend(_comment(doc, "  "))
            assertion = "" if value_type is None else f": {value_type}"
            support_assertion = "" if support is None else f" on {support._name}"
            activation = "" if clock is None else f" at {clock._name}"
            lines.append(f"  let {name}{assertion}{support_assertion}{activation} = {expression._text};")
        if self._aliases and (self._fields or self._relations or self._instances):
            lines.append("")
        if self._fields:
            for field, support, value_type, role, clock, doc in self._fields:
                if field in self._requirements or field in self._causal:
                    continue
                lines.extend(_comment(doc, "  "))
                keyword = "state" if role == FieldRole.State else "variable"
                activation = "" if clock is None else f" at {clock._name}"
                spatial = "" if support is None else f" on {support._name}"
                lines.append(f"  {keyword} {field._text}: {value_type}{spatial}{activation};")
        if self._fields and (self._relations or self._instances):
            lines.append("")
        for equations, doc in self._initials:
            lines.extend(_comment(doc, "  "))
            lines.append("  initial {")
            for left, right in equations:
                lines.extend(_relation_lines(left, right))
            lines.append("  }")
        for index, (name, support, left, right, clock, doc) in enumerate(self._relations):
            lines.extend(_comment(doc, "  "))
            activation = "" if clock is None else f" at {clock._name}"
            spatial = "" if support is None else f" on {support._name}"
            lines.append(f"  relation {name}{spatial}{activation} {{")
            lines.extend(_relation_lines(left, right))
            lines.append("  }")
            if index + 1 != len(self._relations):
                lines.append("")
        if self._relations and self._instances:
            lines.append("")
        for index, (name, component, named_bindings, doc) in enumerate(self._instances):
            lines.extend(_comment(doc, "  "))
            bindings = [f"{target} = {value}" for target, value in named_bindings]
            if bindings:
                lines.append(f"  instance {name}: {component._name}(")
                for binding_index, binding in enumerate(bindings):
                    comma = "," if binding_index + 1 != len(bindings) else ""
                    lines.append(f"    {binding}{comma}")
                lines.append("  );")
            else:
                lines.append(f"  instance {name}: {component._name}();")
            if index + 1 != len(self._instances):
                lines.append("")
        if self._formulations:
            if self._relations or self._instances:
                lines.append("")
            for relation, left, right, doc in self._formulations:
                lines.extend(_comment(doc, "  "))
                lines.append(f"  form primal for {relation._name} {{")
                lines.append(f"    {left._text} = {right._text};")
                lines.append("  }")
        lines.append("}")
        return "\n".join(lines) + "\n"


class Source:
    """Author bounded Model and Component definitions; freeze on first emission."""

    __slots__ = (
        "_components",
        "_contracts",
        "_frozen_text",
        "_owner",
        "_releases",
        "_materials",
        "_top_names",
        "_spaces",
    )

    def __init__(self) -> None:
        self._owner = object()
        self._components: list[Component] = []
        self._contracts: list[PropertyContract] = []
        self._releases: list[PropertyRelease] = []
        self._materials: list[MaterialComposition] = []
        self._top_names: set[str] = set()
        self._spaces: list[tuple[FiniteSpace, tuple[str, ...]]] = []
        self._frozen_text: str | None = None

    def _type_syntax(self, value_type: ValueType) -> str:
        try:
            return _nominal_type_source(value_type, [space for space, _ in self._spaces], [])
        except ValueError as error:
            raise SourceError(str(error)) from error

    def space(self, name: str, *, labels: Sequence[str], doc: str | None = None) -> FiniteSpace:
        """Declare one exact ordered finite basis shared by this Source's components."""
        self._ensure_open()
        doc_lines = _doc(doc)
        if isinstance(labels, str) or not isinstance(labels, Sequence):
            raise TypeError("space labels must be an ordered sequence")
        value = FiniteSpace(_name(name), labels=labels)
        self._add_top_name(name)
        self._spaces.append((value, doc_lines))
        return value

    def _ensure_open(self) -> None:
        if self._frozen_text is not None:
            raise SourceError("Source is frozen after emission or compilation")

    def _add_top_name(self, name: object) -> str:
        admitted = _name(name)
        if admitted in self._top_names:
            raise SourceError(f"duplicate top-level declaration name {admitted!r}")
        if len(self._top_names) >= _MAX_DECLARATIONS:
            raise SourceError(
                f"Source exceeds the {_MAX_DECLARATIONS}-declaration limit"
            )
        self._top_names.add(admitted)
        return admitted

    def component(
        self,
        name: str,
        *,
        doc: str | None = None,
    ) -> Component:
        self._ensure_open()
        doc_lines = _doc(doc)
        admitted = self._add_top_name(name)
        component = Component(_CREATE, self, admitted, doc_lines)
        self._components.append(component)
        return component

    def model(self, name: str, *, doc: str | None = None) -> Component:
        """Author a selected Model using the same signature and body vocabulary."""
        model = self.component(name, doc=doc)
        model._kind = "model"
        return model

    def property_contract(
        self,
        name: str,
        *,
        value_type: ValueType,
        doc: str | None = None,
    ) -> PropertyContract:
        """Declare a complete result type for constant value-only property releases."""
        self._ensure_open()
        if self._components:
            raise SourceError("property declarations must precede Components")
        if not isinstance(value_type, ValueType):
            raise TypeError("value_type must be an eqiora.ValueType")
        self._type_syntax(value_type)
        doc_lines = _doc(doc)
        admitted = self._add_top_name(name)
        contract = PropertyContract(
            _CREATE,
            self._owner,
            admitted,
            value_type,
            doc_lines,
        )
        self._contracts.append(contract)
        return contract

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
    ) -> PropertyRelease:
        """Declare ordered numeric components; the compiler scales every component to SI."""
        self._ensure_open()
        if self._components:
            raise SourceError("property declarations must precede Components")
        if (
            not isinstance(implements, PropertyContract)
            or implements._owner is not self._owner
            or implements not in self._contracts
        ):
            raise SourceError("release contract must be the exact contract from this Source")
        if not isinstance(source_unit, Unit):
            raise TypeError("source_unit must be an eqiora.units.Unit")
        literal = _literal_expression(value)
        _number(source_scale)
        if source_scale <= 0:
            raise SourceError("source_scale must be finite and strictly positive")
        citation_identity = _name_path(citation, "citation identity")
        license_identity = _name_path(license, "license identity")
        doc_lines = _doc(doc)
        admitted = self._add_top_name(name)
        release = PropertyRelease(
            _CREATE,
            _owner=self._owner,
            _name_value=admitted,
            _contract=implements,
            _value=literal,
            _source_unit=source_unit,
            _source_scale=source_scale,
            _citation=citation_identity,
            _license=license_identity,
            _doc=doc_lines,
        )
        self._releases.append(release)
        return release

    def material_composition(
        self,
        name: str,
        *,
        properties: Mapping[str, PropertyRelease],
        doc: str | None = None,
    ) -> MaterialComposition:
        self._ensure_open()
        if not isinstance(properties, Mapping):
            raise TypeError("properties must be a mapping of property requirements to releases")
        if not properties:
            raise SourceError("material composition requires at least one property")
        bindings: list[tuple[str, PropertyRelease]] = []
        for requirement, release in properties.items():
            admitted_requirement = _name(requirement)
            if not isinstance(release, PropertyRelease) or release._owner is not self._owner:
                raise SourceError("material property releases must belong to this Source")
            bindings.append((admitted_requirement, release))
        bindings.sort(key=lambda binding: binding[0])
        doc_lines = _doc(doc)
        admitted = self._add_top_name(name)
        material = MaterialComposition(
            _CREATE,
            _owner=self._owner,
            _name_value=admitted,
            _bindings=tuple(bindings),
            _doc=doc_lines,
        )
        self._materials.append(material)
        return material

    def to_eqi(self) -> str:
        """Return deterministic UTF-8 Eqiora Language text and freeze this Source."""

        if self._frozen_text is None:
            if not self._components:
                raise SourceError(
                    "Source requires at least one public Component before emission"
                )
            declarations: list[str] = []
            for space, doc in self._spaces:
                declarations.extend(_comment(doc, ""))
                declarations.append(f"space {space.name} = orthonormal({', '.join(space.labels)});")
                declarations.append("")
            if self._contracts:
                if not self._releases:
                    raise SourceError(
                        "property Source requires releases"
                    )
                for contract in self._contracts:
                    declarations.extend(_comment(contract._doc, ""))
                    declarations.append(
                        f"public property contract {contract._name}(): {self._type_syntax(contract._value_type)} {{"
                    )
                    declarations.append("  derivatives value_only;")
                    declarations.append("}")
                    declarations.append("")
                for release in self._releases:
                    declarations.extend(_comment(release._doc, ""))
                    declarations.append(
                        "public property release "
                        f"{release._name} implements {release._contract._name} {{"
                    )
                    declarations.append(f"  value = {release._value._text};")
                    declarations.append(
                        "  source_unit: "
                        f"{release._source_unit._text} = "
                        f"{_number(release._source_scale)};"
                    )
                    declarations.append("  validity = unconditional;")
                    declarations.append(f"  citation = {release._citation};")
                    declarations.append(f"  license = {release._license};")
                    declarations.append("}")
                    declarations.append("")
                for material in self._materials:
                    declarations.extend(_comment(material._doc, ""))
                    declarations.append(f"public material composition {material._name} {{")
                    for requirement, release in material._bindings:
                        declarations.append(
                            f"  property {requirement} = {release._name};"
                        )
                    declarations.append("}")
                    declarations.append("")
            rendered_components: list[str] = []
            emitted_bytes = sum(len(line.encode("utf-8")) + 1 for line in declarations) + 1
            for component in self._components:
                rendered = component._render().rstrip("\n")
                emitted_bytes += len(rendered.encode("utf-8")) + (2 if rendered_components else 0)
                if emitted_bytes > _MAX_OUTPUT_BYTES:
                    raise SourceError(
                        f"emitted source exceeds the {_MAX_OUTPUT_BYTES}-byte limit"
                    )
                rendered_components.append(rendered)
            declarations.append("\n\n".join(rendered_components))
            text = "\n".join(declarations) + "\n"
            self._frozen_text = text
        return self._frozen_text

    def write_eqi(self, path: str | os.PathLike[str]) -> None:
        """Atomically replace one regular path with this Source's UTF-8 text."""

        target = Path(os.fspath(path))
        if "\x00" in str(target):
            raise ValueError("output path contains a null byte")
        if target.exists() and (target.is_dir() or target.is_symlink()):
            raise ValueError(
                "output path must be a regular file, not a directory or symlink"
            )
        text = self.to_eqi()
        temporary: str | None = None
        try:
            with tempfile.NamedTemporaryFile(
                mode="w",
                encoding="utf-8",
                newline="\n",
                prefix=f".{target.name}.",
                suffix=".tmp",
                dir=target.parent,
                delete=False,
            ) as stream:
                temporary = stream.name
                stream.write(text)
                stream.flush()
                os.fsync(stream.fileno())
            os.replace(temporary, target)
            temporary = None
        finally:
            if temporary is not None:
                try:
                    os.unlink(temporary)
                except FileNotFoundError:
                    pass


__all__ = [
    "equal",
    "not_equal",
    "less",
    "less_equal",
    "greater",
    "greater_equal",
    "logical_and",
    "logical_or",
    "logical_not",

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
    "trace",
]
