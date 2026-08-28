"""Bounded Python authoring for deterministic Eqiora Language source.

These values only construct source syntax.  The existing native parser, type checker,
lowerer, and compiler remain the sole authority for mathematical meaning.
"""

from __future__ import annotations

import math
import os
from pathlib import Path
import re
import tempfile
import textwrap
from typing import Final

from . import units
from .units import Unit

_NAME = re.compile(r"[A-Za-z_][A-Za-z0-9_]*\Z")
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
    if isinstance(value, float) and not math.isfinite(value):
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


def _relation_lines(residual: Expression) -> list[str]:
    lines = textwrap.wrap(
        f"{residual._text} = 0;",
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
        "_name",
        "_names",
        "_owner",
        "_parameters",
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
        self._doc = _doc(_doc_value)
        self._names: set[str] = set()
        self._supports: list[tuple[Support, str, object, tuple[str, ...]]] = []
        self._parameters: list[tuple[str, Unit, tuple[str, ...]]] = []
        self._fields: list[
            tuple[
                Expression, Support, Unit, _Shape | None, object | None, tuple[str, ...]
            ]
        ] = []
        self._relations: list[tuple[str, Support, Expression, tuple[str, ...]]] = []
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
        public: bool = True,
        doc: str | None = None,
    ) -> Support:
        if public is not True:
            raise SourceError(
                "the initial Source vocabulary admits only public supports"
            )
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
        public: bool = True,
        doc: str | None = None,
    ) -> Support:
        if public is not True:
            raise SourceError(
                "the initial Source vocabulary admits only public supports"
            )
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
        public: bool = True,
        doc: str | None = None,
    ) -> Expression:
        if public is not True:
            raise SourceError(
                "the initial Source vocabulary admits only public parameters"
            )
        if not isinstance(unit, Unit):
            raise TypeError("unit must be an eqiora.lang.units.Unit")
        admitted = self._add_name(name)
        self._parameters.append((admitted, unit, _doc(doc)))
        return Expression(_CREATE, admitted, self._owner, 1, 1, 100)

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
        expression = Expression(_CREATE, admitted, self._owner, 1, 1, 100)
        self._fields.append((expression, on, unit, shape, initial, _doc(doc)))
        return expression

    def relation(
        self,
        name: str,
        *,
        on: Support,
        residual: Expression | int | float,
        doc: str | None = None,
    ) -> None:
        on = self._support(on)
        expression = _expression(residual)
        if expression._owner is None:
            expression = Expression(
                _CREATE,
                expression._text,
                self._owner,
                expression._depth,
                expression._nodes,
                expression._precedence,
            )
        elif expression._owner is not self._owner:
            raise SourceError("relation expressions must belong to this Source")
        admitted = self._add_name(name)
        total_nodes = (
            sum(item[2]._nodes for item in self._relations) + expression._nodes
        )
        if total_nodes > _MAX_EXPRESSION_NODES:
            raise SourceError(
                f"Component relation expressions exceed the {_MAX_EXPRESSION_NODES}-node limit"
            )
        self._relations.append((admitted, on, expression, _doc(doc)))

    def _render(self) -> str:
        lines = _comment(self._doc, "")
        lines.append(f"public component {self._name} {{")
        for support, kind, detail, doc in self._supports:
            lines.extend(_comment(doc, "  "))
            if kind == "volume":
                syntax = f"volume(ambient_dimension = {detail})"
            else:
                syntax = f"boundary(parent = {detail._name})"
            lines.append(f"  public support {support._name}: {syntax};")
        if self._supports and (self._parameters or self._fields or self._relations):
            lines.append("")
        for name, unit, doc in self._parameters:
            lines.extend(_comment(doc, "  "))
            lines.append(f"  public parameter {name}: {unit._text};")
        if self._parameters and (self._fields or self._relations):
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
        if self._fields and self._relations:
            lines.append("")
        for index, (name, support, residual, doc) in enumerate(self._relations):
            lines.extend(_comment(doc, "  "))
            lines.append(f"  relation {name} continuous on {support._name} {{")
            lines.extend(_relation_lines(residual))
            lines.append("  }")
            if index + 1 != len(self._relations):
                lines.append("")
        lines.append("}")
        return "\n".join(lines) + "\n"


class Source:
    """A bounded one-Component draft that freezes on its first emission."""

    __slots__ = ("_component", "_frozen_text", "_owner")

    def __init__(self) -> None:
        self._owner = object()
        self._component: Component | None = None
        self._frozen_text: str | None = None

    def _ensure_open(self) -> None:
        if self._frozen_text is not None:
            raise SourceError("Source is frozen after emission or compilation")

    def component(
        self,
        name: str,
        *,
        public: bool = True,
        doc: str | None = None,
    ) -> Component:
        self._ensure_open()
        if public is not True:
            raise SourceError(
                "the initial Source vocabulary admits one public Component"
            )
        if self._component is not None:
            raise SourceError("Source admits exactly one Component")
        self._component = Component(_CREATE, self, name, doc)
        return self._component

    def to_eqi(self) -> str:
        """Return deterministic UTF-8 Eqiora Language text and freeze this Source."""

        if self._frozen_text is None:
            if self._component is None:
                raise SourceError(
                    "Source requires one public Component before emission"
                )
            text = self._component._render()
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
