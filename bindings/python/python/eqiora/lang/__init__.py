"""Bounded Python authoring for deterministic Eqiora Language source.

These values only construct source syntax.  The existing native parser, type checker,
lowerer, and compiler remain the sole authority for mathematical meaning.
"""

from __future__ import annotations

from collections.abc import Mapping
import math as _stdlib_math
import os
from pathlib import Path
import re
import tempfile
import textwrap
from typing import Final

from . import units
from .units import Unit

_NAME = re.compile(r"[A-Za-z_][A-Za-z0-9_]*\Z")
_NAME_PATH = re.compile(r"[A-Za-z_][A-Za-z0-9_]*(?:\.[A-Za-z_][A-Za-z0-9_]*)*\Z")
_MAX_DECLARATIONS = 256
_MAX_IDENTIFIER_BYTES = 1_024
_MAX_EXPRESSION_DEPTH = 64
_MAX_EXPRESSION_NODES = 4096
_MAX_DOC_BYTES = 16_384
_MAX_OUTPUT_BYTES = 8 * 1024 * 1024
_CREATE = object()
_MISSING = object()


class SourceError(ValueError):
    """A source-authoring value violates the bounded authoring contract."""


class _Shape:
    __slots__ = ("_text",)

    def __init__(self, _token: object = _MISSING, _text: str = "") -> None:
        if _token is not _CREATE:
            raise TypeError("shapes are provided by eqiora.lang")
        object.__setattr__(self, "_text", _text)

    def __setattr__(self, name: str, value: object) -> None:
        raise AttributeError("shape values are immutable")


spatial_vector: Final = _Shape(_CREATE, "spatial_vector")


class PropertyContract:
    """An identity-bearing scalar property contract declaration handle."""

    __slots__ = ("_doc", "_name", "_owner", "_unit")

    def __init__(
        self,
        _token: object = _MISSING,
        _owner: object = _MISSING,
        _name_value: str = "",
        _unit: Unit | None = None,
        _doc: tuple[str, ...] = (),
    ) -> None:
        if _token is not _CREATE or _unit is None:
            raise TypeError("property contracts are created by Source")
        object.__setattr__(self, "_owner", _owner)
        object.__setattr__(self, "_name", _name_value)
        object.__setattr__(self, "_unit", _unit)
        object.__setattr__(self, "_doc", _doc)

    def __setattr__(self, name: str, value: object) -> None:
        raise AttributeError("PropertyContract handles are immutable")


class PropertyRelease:
    """An identity-bearing constant scalar property release handle."""

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
        _value: int | float = 0,
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

    def __neg__(self) -> Expression:
        value = f"({self._text})" if self._precedence < 40 else self._text
        return Expression(
            _CREATE,
            f"-{value}",
            self._owner,
            self._depth + 1,
            self._nodes + 1,
            40,
        )


class _Math:
    __slots__ = ()
    pi: Final = Expression(_CREATE, "math.pi", None, 1, 1, 100)

    @staticmethod
    def sin(value: object) -> Expression:
        return _unary("math.sin", value)


math: Final = _Math()


class _Parameter(Expression):
    __slots__ = ("_component", "_name")

    def __init__(self, owner: object, component: object, name: str) -> None:
        super().__init__(_CREATE, name, owner, 1, 1, 100)
        object.__setattr__(self, "_component", component)
        object.__setattr__(self, "_name", name)


class _Field(Expression):
    __slots__ = ("_component", "_name")

    def __init__(self, owner: object, component: object, name: str) -> None:
        super().__init__(_CREATE, name, owner, 1, 1, 100)
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
        owner: object,
        component: object,
        name: str,
        contract: PropertyContract,
    ) -> None:
        super().__init__(_CREATE, name, owner, 1, 1, 100)
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
    if isinstance(value, Expression):
        return value
    return Expression(_CREATE, _number(value), None, 1, 1, 100)


def _owner(left: Expression, right: Expression) -> object | None:
    if (
        left._owner is not None
        and right._owner is not None
        and left._owner is not right._owner
    ):
        raise SourceError("cannot combine expressions from different Source values")
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


def dot(left: object, right: object) -> Expression:
    left_expression = _expression(left)
    right_expression = _expression(right)
    owner = _owner(left_expression, right_expression)
    return Expression(
        _CREATE,
        f"dot({left_expression._text}, {right_expression._text})",
        owner,
        max(left_expression._depth, right_expression._depth) + 1,
        left_expression._nodes + right_expression._nodes + 1,
        100,
    )


def integrate(domain: Support, integrand: object) -> Expression:
    if not isinstance(domain, Support) or domain._kind != "volume":
        raise SourceError("integrate() requires a volume Support")
    expression = _expression(integrand)
    if expression._owner is not None and expression._owner is not domain._owner:
        raise SourceError("integrand and Support must belong to the same Source")
    return Expression(
        _CREATE,
        f"integrate({domain._name}, {expression._text})",
        domain._owner,
        expression._depth + 1,
        expression._nodes + 1,
        100,
    )


def div(value: object) -> Expression:
    return _unary("div", value)


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
    return [f"{indent}// {line}" if line else f"{indent}//" for line in lines]


def _relation_lines(left: Expression, right: Expression | None = None) -> list[str]:
    right_text = "0" if right is None else right._text
    lines = textwrap.wrap(
        f"{left._text} = {right_text};",
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
    """The bounded draft for one public equations-only Component."""

    __slots__ = (
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
        self._owner = _source._owner
        self._component_token = object()
        self._name = _name(_name_value)
        self._doc = (
            _doc_value if isinstance(_doc_value, tuple) else _doc(_doc_value)
        )
        self._names: set[str] = set()
        self._supports: list[tuple[Support, str, object, tuple[str, ...]]] = []
        self._parameters: list[tuple[_Parameter, Unit, tuple[str, ...]]] = []
        self._properties: list[
            tuple[_PropertyRequirement, PropertyContract, tuple[str, ...]]
        ] = []
        self._fields: list[
            tuple[
                Expression, Support, Unit, _Shape | None, object | None, tuple[str, ...]
            ]
        ] = []
        self._relations: list[
            tuple[str, Support, Expression, Expression | None, tuple[str, ...]]
        ] = []
        self._formulations: list[
            tuple[Relation, Expression, Expression, tuple[str, ...]]
        ] = []
        self._instances: list[
            tuple[
                str,
                Component,
                tuple[tuple[Support, Support], ...],
                tuple[tuple[_Parameter, Expression], ...],
                tuple[tuple[_PropertyRequirement, PropertyRelease], ...],
                tuple[str, ...],
            ]
        ] = []
        self._declaration_count = 0

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
        unit: Unit,
        doc: str | None = None,
    ) -> Expression:
        if not isinstance(unit, Unit):
            raise TypeError("unit must be an eqiora.lang.units.Unit")
        admitted = self._add_name(name)
        parameter = _Parameter(self._owner, self._component_token, admitted)
        self._parameters.append((parameter, unit, _doc(doc)))
        return parameter

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
            self._owner, self._component_token, admitted, contract
        )
        self._properties.append((requirement, contract, _doc(doc)))
        return requirement

    def field(
        self,
        name: str,
        *,
        on: Support,
        unit: Unit,
        shape: _Shape | None = None,
        initial: int | float | None = None,
        doc: str | None = None,
    ) -> Expression:
        on = self._support(on)
        if on._kind != "volume":
            raise SourceError(
                "the initial Source vocabulary admits fields on volumes only"
            )
        if not isinstance(unit, Unit):
            raise TypeError("unit must be an eqiora.lang.units.Unit")
        if shape is not None and shape is not spatial_vector:
            raise SourceError("shape must be eqiora.lang.spatial_vector or None")
        if initial is not None:
            _number(initial)
        admitted = self._add_name(name)
        expression = _Field(self._owner, self._component_token, admitted)
        self._fields.append((expression, on, unit, shape, initial, _doc(doc)))
        return expression

    def relation(
        self,
        name: str,
        *,
        on: Support,
        residual: Expression | int | float | None = None,
        left: Expression | int | float | None = None,
        right: Expression | int | float | None = None,
        doc: str | None = None,
    ) -> Relation:
        on = self._support(on)
        has_residual = residual is not None
        has_left = left is not None
        has_right = right is not None
        if (has_residual and (has_left or has_right)) or (
            not has_residual and not (has_left and has_right)
        ):
            raise SourceError(
                "relation requires exactly residual= or the complete left= and right= pair"
            )

        def admit(value: Expression | int | float) -> Expression:
            expression = _expression(value)
            if expression._owner is None:
                return Expression(
                    _CREATE,
                    expression._text,
                    self._owner,
                    expression._depth,
                    expression._nodes,
                    expression._precedence,
                )
            if expression._owner is not self._owner:
                raise SourceError("relation expressions must belong to this Source")
            return expression

        if has_residual:
            assert residual is not None
            left_expression = admit(residual)
            right_expression = None
        else:
            assert left is not None and right is not None
            left_expression = admit(left)
            right_expression = admit(right)
        admitted = self._add_name(name)
        total_nodes = (
            sum(
                item[2]._nodes + (item[3]._nodes if item[3] is not None else 0)
                for item in self._relations
            )
            + left_expression._nodes
            + (right_expression._nodes if right_expression is not None else 0)
        )
        if total_nodes > _MAX_EXPRESSION_NODES:
            raise SourceError(
                f"Component relation expressions exceed the {_MAX_EXPRESSION_NODES}-node limit"
            )
        self._relations.append(
            (admitted, on, left_expression, right_expression, _doc(doc))
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
            if expression._owner is not self._owner:
                raise SourceError("form expressions must belong to this Source")
        if self._formulations:
            raise SourceError("the scalar-primal Source vocabulary admits one form")
        total_nodes = (
            sum(
                item[2]._nodes + (item[3]._nodes if item[3] is not None else 0)
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
        self,
        name: str,
        *,
        component: Component,
        supports: Mapping[Support, Support],
        parameters: Mapping[Expression, Expression | int | float],
        properties: Mapping[Expression, PropertyRelease],
        doc: str | None = None,
    ) -> None:
        if not isinstance(component, Component) or component._owner is not self._owner:
            raise SourceError("instance component must belong to this Source")
        if component is self:
            raise SourceError("a Component cannot instantiate itself")
        if not isinstance(supports, Mapping):
            raise TypeError("supports must be a mapping of target to enclosing Support handles")
        if not isinstance(parameters, Mapping):
            raise TypeError("parameters must be a mapping of target Parameter expressions")
        if not isinstance(properties, Mapping):
            raise TypeError("properties must be a mapping of target property requirements")

        target_supports = [item[0] for item in component._supports]
        target_parameters = [item[0] for item in component._parameters]
        target_properties = [item[0] for item in component._properties]
        if set(supports) != set(target_supports):
            raise SourceError("instance support bindings must be complete and exact")
        if set(parameters) != set(target_parameters):
            raise SourceError("instance Parameter bindings must be complete and exact")
        if set(properties) != set(target_properties):
            raise SourceError("instance property bindings must be complete and exact")

        support_bindings: list[tuple[Support, Support]] = []
        for target in target_supports:
            enclosing = supports[target]
            if (
                not isinstance(enclosing, Support)
                or enclosing._component is not self._component_token
            ):
                raise SourceError(
                    "instance support targets must belong to the enclosing Component"
                )
            support_bindings.append((target, enclosing))

        parameter_bindings: list[tuple[_Parameter, Expression]] = []
        for target in target_parameters:
            value = _expression(parameters[target])
            if value._owner is not None and value._owner is not self._owner:
                raise SourceError("instance Parameter values must belong to this Source")
            parameter_bindings.append((target, value))

        property_bindings: list[tuple[_PropertyRequirement, PropertyRelease]] = []
        for target in target_properties:
            release = properties[target]
            if not isinstance(release, PropertyRelease) or release._owner is not self._owner:
                raise SourceError("instance property releases must belong to this Source")
            if release._contract is not target._contract:
                raise SourceError(
                    "instance property release must implement the exact required contract"
                )
            property_bindings.append((target, release))

        admitted = self._add_name(name)
        self._instances.append(
            (
                admitted,
                component,
                tuple(support_bindings),
                tuple(parameter_bindings),
                tuple(property_bindings),
                _doc(doc),
            )
        )

    def _render(self) -> str:
        lines = _comment(self._doc, "")
        lines.append(f"public component {self._name} {{")
        for requirement, contract, doc in self._properties:
            lines.extend(_comment(doc, "  "))
            lines.append(
                f"  public property {requirement._name}: {contract._name};"
            )
        if self._properties and (
            self._supports
            or self._parameters
            or self._fields
            or self._relations
            or self._instances
        ):
            lines.append("")
        for support, kind, detail, doc in self._supports:
            lines.extend(_comment(doc, "  "))
            if kind == "volume":
                syntax = f"volume(ambient_dimension = {detail})"
            else:
                syntax = f"boundary(parent = {detail._name})"
            lines.append(f"  public support {support._name}: {syntax};")
        if self._supports and (
            self._parameters or self._fields or self._relations or self._instances
        ):
            lines.append("")
        for parameter, unit, doc in self._parameters:
            lines.extend(_comment(doc, "  "))
            lines.append(f"  public parameter {parameter._name}: {unit._text};")
        if self._parameters and (self._fields or self._relations or self._instances):
            lines.append("")
        if self._fields:
            lines.append("  representation space = continuum;")
            for field, support, unit, shape, initial, doc in self._fields:
                lines.extend(_comment(doc, "  "))
                suffix = f" shape {shape._text}" if shape is not None else ""
                initialized = f" = {_number(initial)}" if initial is not None else ""
                lines.append(
                    f"  field {field._text} on {support._name} as space: "
                    f"{unit._text}{suffix}{initialized};"
                )
        if self._fields and (self._relations or self._instances):
            lines.append("")
        for index, (name, support, left, right, doc) in enumerate(self._relations):
            lines.extend(_comment(doc, "  "))
            lines.append(f"  relation {name} continuous on {support._name} {{")
            lines.extend(_relation_lines(left, right))
            lines.append("  }")
            if index + 1 != len(self._relations):
                lines.append("")
        if self._relations and self._instances:
            lines.append("")
        for index, (
            name,
            component,
            support_bindings,
            parameter_bindings,
            property_bindings,
            doc,
        ) in enumerate(self._instances):
            lines.extend(_comment(doc, "  "))
            bindings = [
                f"support {target._name} = {enclosing._name}"
                for target, enclosing in support_bindings
            ]
            bindings.extend(
                f"{target._name} = {value._text}"
                for target, value in parameter_bindings
            )
            bindings.extend(
                f"property {target._name} = {release._name}"
                for target, release in property_bindings
            )
            if bindings:
                lines.append(f"  instance {name}: {component._name}(")
                for binding_index, binding in enumerate(bindings):
                    comma = "," if binding_index + 1 != len(bindings) else ""
                    lines.append(f"    {binding}{comma}")
                lines.append("  );")
            else:
                lines.append(f"  instance {name}: {component._name};")
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
    """A bounded language draft that freezes on its first emission."""

    __slots__ = (
        "_components",
        "_contract",
        "_frozen_text",
        "_owner",
        "_release",
        "_top_names",
    )

    def __init__(self) -> None:
        self._owner = object()
        self._components: list[Component] = []
        self._contract: PropertyContract | None = None
        self._release: PropertyRelease | None = None
        self._top_names: set[str] = set()
        self._frozen_text: str | None = None

    def _ensure_open(self) -> None:
        if self._frozen_text is not None:
            raise SourceError("Source is frozen after emission or compilation")

    def _requires_package_compilation(self) -> bool:
        return self._contract is not None

    def component(
        self,
        name: str,
        *,
        doc: str | None = None,
    ) -> Component:
        self._ensure_open()
        maximum = 2 if self._contract is not None else 1
        if len(self._components) >= maximum:
            if maximum == 1:
                raise SourceError(
                    "Source admits a second Component only for scalar property binding"
                )
            raise SourceError(
                "the scalar property Source vocabulary admits exactly two Components"
            )
        admitted = _name(name)
        if admitted in self._top_names:
            raise SourceError(f"duplicate top-level declaration name {admitted!r}")
        doc_lines = _doc(doc)
        self._top_names.add(admitted)
        component = Component(_CREATE, self, admitted, doc_lines)
        self._components.append(component)
        return component

    def scalar_property_contract(
        self,
        name: str,
        *,
        unit: Unit,
        doc: str | None = None,
    ) -> PropertyContract:
        self._ensure_open()
        if self._components:
            raise SourceError("property declarations must precede Components")
        if self._contract is not None:
            raise SourceError("Source admits exactly one scalar property contract")
        if not isinstance(unit, Unit):
            raise TypeError("unit must be an eqiora.lang.units.Unit")
        admitted = _name(name)
        if admitted in self._top_names:
            raise SourceError(f"duplicate top-level declaration name {admitted!r}")
        doc_lines = _doc(doc)
        self._top_names.add(admitted)
        contract = PropertyContract(
            _CREATE,
            self._owner,
            admitted,
            unit,
            doc_lines,
        )
        self._contract = contract
        return contract

    def scalar_property_release(
        self,
        name: str,
        *,
        implements: PropertyContract,
        value: int | float,
        source_unit: Unit,
        source_scale: int | float,
        citation: str,
        license: str,
        doc: str | None = None,
    ) -> PropertyRelease:
        self._ensure_open()
        if self._components:
            raise SourceError("property declarations must precede Components")
        if self._release is not None:
            raise SourceError("Source admits exactly one scalar property release")
        if (
            not isinstance(implements, PropertyContract)
            or implements._owner is not self._owner
            or implements is not self._contract
        ):
            raise SourceError("release contract must be the exact contract from this Source")
        if not isinstance(source_unit, Unit):
            raise TypeError("source_unit must be an eqiora.lang.units.Unit")
        _number(value)
        _number(source_scale)
        if source_scale <= 0:
            raise SourceError("source_scale must be finite and strictly positive")
        admitted = _name(name)
        if admitted in self._top_names:
            raise SourceError(f"duplicate top-level declaration name {admitted!r}")
        citation_identity = _name_path(citation, "citation identity")
        license_identity = _name_path(license, "license identity")
        doc_lines = _doc(doc)
        self._top_names.add(admitted)
        release = PropertyRelease(
            _CREATE,
            _owner=self._owner,
            _name_value=admitted,
            _contract=implements,
            _value=value,
            _source_unit=source_unit,
            _source_scale=source_scale,
            _citation=citation_identity,
            _license=license_identity,
            _doc=doc_lines,
        )
        self._release = release
        return release

    def to_eqi(self) -> str:
        """Return deterministic UTF-8 Eqiora Language text and freeze this Source."""

        if self._frozen_text is None:
            if not self._components:
                raise SourceError(
                    "Source requires one public Component before emission"
                )
            declarations: list[str] = []
            if self._contract is not None:
                if self._release is None or len(self._components) != 2:
                    raise SourceError(
                        "scalar property Source requires one release, one consumer, and one root Component"
                    )
                consumer, root = self._components
                if (
                    len(consumer._properties) != 1
                    or consumer._instances
                    or root._properties
                    or len(root._instances) != 1
                ):
                    raise SourceError(
                        "scalar property Source requires one consumer requirement and one root instance"
                    )
                instance = root._instances[0]
                if (
                    instance[1] is not consumer
                    or len(instance[4]) != 1
                    or instance[4][0][0] is not consumer._properties[0][0]
                    or instance[4][0][1] is not self._release
                ):
                    raise SourceError(
                        "root instance must bind the exact release to the consumer requirement"
                    )
                declarations.extend(_comment(self._contract._doc, ""))
                declarations.append(
                    f"public property contract {self._contract._name} {{"
                )
                declarations.append(
                    f"  scalar value: {self._contract._unit._text};"
                )
                declarations.append("}")
                declarations.append("")
                declarations.extend(_comment(self._release._doc, ""))
                declarations.append(
                    "public property release "
                    f"{self._release._name} implements {self._contract._name} {{"
                )
                declarations.append(f"  value = {_number(self._release._value)};")
                declarations.append(
                    "  source_unit: "
                    f"{self._release._source_unit._text} = "
                    f"{_number(self._release._source_scale)};"
                )
                declarations.append("  validity = unconditional;")
                declarations.append(f"  citation = {self._release._citation};")
                declarations.append(f"  license = {self._release._license};")
                declarations.append("}")
                declarations.append("")
            declarations.append(
                "\n\n".join(
                    component._render().rstrip("\n")
                    for component in self._components
                )
            )
            text = "\n".join(declarations) + "\n"
            if len(text.encode("utf-8")) > _MAX_OUTPUT_BYTES:
                raise SourceError(
                    f"emitted source exceeds the {_MAX_OUTPUT_BYTES}-byte limit"
                )
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
    "Component",
    "Expression",
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
    "spatial_vector",
    "symmetric_part",
    "test",
    "trace",
    "units",
]
