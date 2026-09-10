"""Typed boundary for the private authoring adapter."""

from collections.abc import Callable
from . import Module, ModuleRef

class DeclarationRef:
    def __init__(self, token: object, imported: ModuleRef, kind: str, name: str, descriptor: object, *, origin: Module | None = None) -> None: ...

def reference(imported: ModuleRef, name: str, kind: str, describe: Callable[[object, str], object]) -> DeclarationRef: ...
def same_declaration(left: object, right: object) -> bool: ...
