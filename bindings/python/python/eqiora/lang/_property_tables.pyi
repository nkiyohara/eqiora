"""Typed boundary for the private authoring adapter."""

from collections.abc import Sequence
from dataclasses import dataclass
from ..units import Unit
from . import Expression, Module, PropertyContract, PropertyRelease

@dataclass(frozen=True, slots=True)
class TableSource:
    data: str
    axis: str
    axis_unit: object
    validity: tuple[Expression, ...]

def release(module: Module, name: str, *, implements: PropertyContract, data: str, axis_unit: Unit, source_unit: Unit, validity: Sequence[object], citation: str, license: str, branch: str = "single", doc: str | None = None) -> PropertyRelease: ...
def emit(graph: object, release: PropertyRelease, ordinal: int) -> object: ...
