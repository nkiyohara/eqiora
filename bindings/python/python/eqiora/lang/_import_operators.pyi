"""Typed boundary for the private authoring adapter."""

from . import ModuleRef, Operator
from ._import_references import DeclarationRef

class ImportedOperator(Operator):
    def __init__(self, ref: DeclarationRef) -> None: ...

def operator(imported: ModuleRef, name: str) -> ImportedOperator: ...
