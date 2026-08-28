"""Structural SI-unit expressions for :mod:`eqiora.lang` source authoring."""

from __future__ import annotations

_CREATE_UNIT = object()


class Unit:
    """One structural Eqiora Language unit expression."""

    __slots__ = ("_text",)

    def __init__(self, token: object, text: str) -> None:
        if token is not _CREATE_UNIT:
            raise TypeError("units are composed from eqiora.lang.units base values")
        object.__setattr__(self, "_text", text)

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

    def __pow__(self, exponent: int) -> Unit:
        if isinstance(exponent, bool) or not isinstance(exponent, int):
            raise TypeError("unit exponents must be integers")
        if not -32 <= exponent <= 32:
            raise ValueError("unit exponent must be between -32 and 32")
        return Unit(_CREATE_UNIT, f"({self._text} ^ {exponent})")


kg = Unit(_CREATE_UNIT, "kg")
m = Unit(_CREATE_UNIT, "m")
s = Unit(_CREATE_UNIT, "s")

__all__ = ["kg", "m", "s"]
