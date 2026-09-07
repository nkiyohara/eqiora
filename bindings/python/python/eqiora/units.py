"""Compiler-owned input-unit catalog and bounded structural unit expressions.

Unit composition emits syntax; the compiler alone converts values to coherent SI.
"""

from __future__ import annotations
from fractions import Fraction

from ._eqiora import _input_unit_catalog
from ._source_bounds import _MAX_EXPRESSION_DEPTH, _MAX_EXPRESSION_NODES, _MAX_OUTPUT_BYTES

_CREATE_UNIT = object()
_MISSING_UNIT = object()


class Unit:
    """One immutable, bounded Eqiora input-unit expression."""

    __slots__ = ("_text", "_depth", "_nodes", "_prefixable")

    def __init__(self, _token: object = _MISSING_UNIT, _text: str = "",
                 _depth: int = 1, _nodes: int = 1, _prefixable: bool = False) -> None:
        if _token is not _CREATE_UNIT:
            raise TypeError("units are composed from eqiora.units catalog values")
        if _depth > _MAX_EXPRESSION_DEPTH or _nodes > _MAX_EXPRESSION_NODES:
            raise ValueError("unit expression exceeds the 64-depth or 4096-node authoring limit")
        if len(_text.encode("utf-8")) > _MAX_OUTPUT_BYTES:
            raise ValueError("unit expression exceeds the 8388608-byte authoring limit")
        object.__setattr__(self, "_text", _text)
        object.__setattr__(self, "_depth", _depth)
        object.__setattr__(self, "_nodes", _nodes)
        object.__setattr__(self, "_prefixable", _prefixable)

    def __setattr__(self, name: str, value: object) -> None:
        raise AttributeError("Unit values are immutable")

    def _binary(self, other: Unit, operator: str) -> Unit:
        if not isinstance(other, Unit):
            return NotImplemented
        depth = max(self._depth, other._depth) + 1
        nodes = self._nodes + other._nodes + 1
        # Check before concatenation, including repeated self-composition.
        if depth > _MAX_EXPRESSION_DEPTH or nodes > _MAX_EXPRESSION_NODES:
            raise ValueError("unit expression exceeds the 64-depth or 4096-node authoring limit")
        if len(self._text) + len(other._text) + 5 > _MAX_OUTPUT_BYTES:
            raise ValueError("unit expression exceeds the 8388608-byte authoring limit")
        return Unit(_CREATE_UNIT, f"({self._text} {operator} {other._text})", depth, nodes)

    def __mul__(self, other: Unit) -> Unit:
        return self._binary(other, "*")

    def __truediv__(self, other: Unit) -> Unit:
        return self._binary(other, "/")

    def __pow__(self, exponent: int | Fraction) -> Unit:
        if isinstance(exponent, bool) or not isinstance(exponent, (int, Fraction)):
            raise TypeError("unit exponents must be int or fractions.Fraction")
        numerator, denominator = exponent.numerator, exponent.denominator
        if abs(numerator) > 2147483647 or denominator > 2147483647:
            raise ValueError("unit exponent numerator and denominator must be bounded by 2147483647")
        depth = self._depth + 1
        nodes = self._nodes + (2 if denominator == 1 else 4)
        if depth > _MAX_EXPRESSION_DEPTH or nodes > _MAX_EXPRESSION_NODES:
            raise ValueError("unit expression exceeds the 64-depth or 4096-node authoring limit")
        power = str(numerator) if denominator == 1 else f"({numerator} / {denominator})"
        return Unit(_CREATE_UNIT, f"({self._text} ^ {power})", depth, nodes)

    def prefixed(self, prefix: str) -> Unit:
        """Apply one compiler-admitted prefix to one prefixable catalog symbol."""
        if prefix not in _PREFIXES:
            raise ValueError("input-unit prefix is not in the compiler catalog")
        if not self._prefixable:
            raise ValueError("prefixes require a bare prefixable catalog unit; use g for mass")
        return Unit(_CREATE_UNIT, prefix + self._text)


_symbols, _prefixes = _input_unit_catalog()
_PREFIXES = frozenset(_prefixes)
__all__ = ["Unit"]
for _symbol, _prefixable in _symbols:
    _name = "one" if _symbol == "1" else _symbol
    globals()[_name] = Unit(_CREATE_UNIT, _symbol, _prefixable=_prefixable)
    __all__.append(_name)
del _symbols, _prefixes, _symbol, _prefixable, _name
