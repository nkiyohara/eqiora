"""Composable read-only viewer for accepted Eqiora values.

Authority: ``bindings/python/python/eqiora/viewer.py``.
"""

from typing import Self

from . import FieldOutput
from .geometry import Geometry
from .meshing import Mesh

class View:
    """Disposable typed viewer scene; its transport is private and unstable.

    Authority: ``bindings/python/python/eqiora/viewer.py::View``.
    """

    def __init__(self) -> None: ...
    def add(self, value: Geometry | Mesh | FieldOutput, /) -> Self: ...
    def show(self) -> Self: ...
    def close(self) -> None: ...
    def __enter__(self) -> Self: ...
    def __exit__(self, *_exc: object) -> None: ...
    def __repr__(self) -> str: ...

__all__ = ["View"]
