"""Structural SI-unit expressions for :mod:`eqiora.lang` source authoring."""

from __future__ import annotations
from fractions import Fraction

_CREATE_UNIT = object()
_MISSING_UNIT = object()


class Unit:
    """One structural Eqiora Language unit expression."""

    __slots__ = ("_text",)
    _text: str

    def __init__(self, _token: object = _MISSING_UNIT, _text: str = "") -> None:
        if _token is not _CREATE_UNIT:
            raise TypeError("units are composed from eqiora.lang.units base values")
        object.__setattr__(self, "_text", _text)

    def __setattr__(self, name: str, value: object) -> None:
        raise AttributeError("Unit values are immutable")

    def __mul__(self, other: Unit) -> Unit:
        if not isinstance(other, Unit):
            return NotImplemented
        return Unit(_CREATE_UNIT, f"({self._text} * {other._text})")

    def __truediv__(self, other: Unit) -> Unit:
        if not isinstance(other, Unit):
            return NotImplemented
        return Unit(_CREATE_UNIT, f"({self._text} / {other._text})")

    def __pow__(self, exponent: int | Fraction) -> Unit:
        if isinstance(exponent, bool) or not isinstance(exponent, (int, Fraction)):
            raise TypeError("unit exponents must be int or fractions.Fraction")
        numerator, denominator = exponent.numerator, exponent.denominator
        if abs(numerator) > 2147483647 or denominator > 2147483647:
            raise ValueError("unit exponent numerator and denominator must be bounded by 2147483647")
        power = str(numerator) if denominator == 1 else f"({numerator} / {denominator})"
        return Unit(_CREATE_UNIT, f"({self._text} ^ {power})")

    def prefixed(self, prefix: str) -> Unit:
        """Apply one supported decimal prefix to a bare input unit."""
        if prefix not in ("n", "u", "m", "k", "M", "G"):
            raise ValueError("input-unit prefix must be n, u, m, k, M, or G")
        if self._text not in _PREFIX_BASES:
            raise ValueError("prefixes require a bare unit other than kg; use g for mass")
        return Unit(_CREATE_UNIT, prefix + self._text)


kg = Unit(_CREATE_UNIT, "kg")
m = Unit(_CREATE_UNIT, "m")
s = Unit(_CREATE_UNIT, "s")
A = Unit(_CREATE_UNIT, "A")
K = Unit(_CREATE_UNIT, "K")
mol = Unit(_CREATE_UNIT, "mol")
cd = Unit(_CREATE_UNIT, "cd")
Hz = Unit(_CREATE_UNIT, "Hz")
N = Unit(_CREATE_UNIT, "N")
Pa = Unit(_CREATE_UNIT, "Pa")
J = Unit(_CREATE_UNIT, "J")
W = Unit(_CREATE_UNIT, "W")
C = Unit(_CREATE_UNIT, "C")
V = Unit(_CREATE_UNIT, "V")
Ohm = Unit(_CREATE_UNIT, "Ohm")
S = Unit(_CREATE_UNIT, "S")
F = Unit(_CREATE_UNIT, "F")
H = Unit(_CREATE_UNIT, "H")
Wb = Unit(_CREATE_UNIT, "Wb")
T = Unit(_CREATE_UNIT, "T")
g = Unit(_CREATE_UNIT, "g")
one = Unit(_CREATE_UNIT, "1")

__all__ = [
    "kg", "m", "s", "A", "K", "mol", "cd", "Hz", "N", "Pa", "J", "W", "C",
    "V", "Ohm", "S", "F", "H", "Wb", "T", "g", "one",
]
_PREFIX_BASES = frozenset(__all__) - {"kg", "one"}
