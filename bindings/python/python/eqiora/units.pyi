"""Compiler-owned unit catalog for deterministic Source quantity authoring.

Authority: ``bindings/python/python/eqiora/units.py``.
"""
from fractions import Fraction
from typing import Final, final

@final
class Unit:
    """Compose an immutable bounded structural input-unit expression.

    Authority: ``bindings/python/python/eqiora/units.py::Unit``.
    """
    def __mul__(self, other: Unit, /) -> Unit: ...
    def __truediv__(self, other: Unit, /) -> Unit: ...
    def __pow__(self, exponent: int | Fraction, /) -> Unit: ...
    def prefixed(self, prefix: str) -> Unit: ...

#: Compiler-owned input-unit symbol.
#:
#: Authority: ``bindings/python/python/eqiora/units.py``.
kg: Final[Unit]

#: Compiler-owned input-unit symbol.
#:
#: Authority: ``bindings/python/python/eqiora/units.py``.
m: Final[Unit]

#: Compiler-owned input-unit symbol.
#:
#: Authority: ``bindings/python/python/eqiora/units.py``.
s: Final[Unit]

#: Compiler-owned input-unit symbol.
#:
#: Authority: ``bindings/python/python/eqiora/units.py``.
A: Final[Unit]

#: Compiler-owned input-unit symbol.
#:
#: Authority: ``bindings/python/python/eqiora/units.py``.
K: Final[Unit]

#: Compiler-owned input-unit symbol.
#:
#: Authority: ``bindings/python/python/eqiora/units.py``.
mol: Final[Unit]

#: Compiler-owned input-unit symbol.
#:
#: Authority: ``bindings/python/python/eqiora/units.py``.
cd: Final[Unit]

#: Compiler-owned input-unit symbol.
#:
#: Authority: ``bindings/python/python/eqiora/units.py``.
Hz: Final[Unit]

#: Compiler-owned input-unit symbol.
#:
#: Authority: ``bindings/python/python/eqiora/units.py``.
N: Final[Unit]

#: Compiler-owned input-unit symbol.
#:
#: Authority: ``bindings/python/python/eqiora/units.py``.
Pa: Final[Unit]

#: Compiler-owned input-unit symbol.
#:
#: Authority: ``bindings/python/python/eqiora/units.py``.
J: Final[Unit]

#: Compiler-owned input-unit symbol.
#:
#: Authority: ``bindings/python/python/eqiora/units.py``.
W: Final[Unit]

#: Compiler-owned input-unit symbol.
#:
#: Authority: ``bindings/python/python/eqiora/units.py``.
C: Final[Unit]

#: Compiler-owned input-unit symbol.
#:
#: Authority: ``bindings/python/python/eqiora/units.py``.
V: Final[Unit]

#: Compiler-owned input-unit symbol.
#:
#: Authority: ``bindings/python/python/eqiora/units.py``.
Ohm: Final[Unit]

#: Compiler-owned input-unit symbol.
#:
#: Authority: ``bindings/python/python/eqiora/units.py``.
S: Final[Unit]

#: Compiler-owned input-unit symbol.
#:
#: Authority: ``bindings/python/python/eqiora/units.py``.
F: Final[Unit]

#: Compiler-owned input-unit symbol.
#:
#: Authority: ``bindings/python/python/eqiora/units.py``.
H: Final[Unit]

#: Compiler-owned input-unit symbol.
#:
#: Authority: ``bindings/python/python/eqiora/units.py``.
Wb: Final[Unit]

#: Compiler-owned input-unit symbol.
#:
#: Authority: ``bindings/python/python/eqiora/units.py``.
T: Final[Unit]

#: Compiler-owned input-unit symbol.
#:
#: Authority: ``bindings/python/python/eqiora/units.py``.
g: Final[Unit]

#: Compiler-owned input-unit symbol.
#:
#: Authority: ``bindings/python/python/eqiora/units.py``.
one: Final[Unit]

__all__ = ['Unit', 'kg', 'm', 's', 'A', 'K', 'mol', 'cd', 'Hz', 'N', 'Pa', 'J', 'W', 'C', 'V', 'Ohm', 'S', 'F', 'H', 'Wb', 'T', 'g', 'one']
