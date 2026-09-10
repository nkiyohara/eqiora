"""Typed boundary for the private authoring adapter."""

from .. import Dimension
from . import Module, ModuleRef

def declare(module: Module, name: str, value: Dimension, doc: str | None) -> Dimension: ...
def imported(reference: ModuleRef, name: str) -> Dimension: ...
