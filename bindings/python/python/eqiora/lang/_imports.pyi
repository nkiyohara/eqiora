"""Typed boundary for the private authoring adapter."""

from collections.abc import Mapping
from . import Component, ComponentRef, Connector, FieldConnector, ModuleRef, Port
from ._import_references import DeclarationRef

class ImportedConnector(Connector):
    def __init__(self, reference: DeclarationRef, first: str, second: str) -> None: ...

class ImportedFieldConnector(FieldConnector):
    def __init__(self, reference: DeclarationRef, first: str, second: str) -> None: ...

def connector(imported: ModuleRef, name: str, *, internal: bool = False) -> ImportedConnector | ImportedFieldConnector: ...
def instance_ports(component: Component, target: ComponentRef, name: str, bindings: Mapping[str, object]) -> dict[str, Port]: ...
